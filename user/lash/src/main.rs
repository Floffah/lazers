#![no_main]
#![no_std]

//! First bootstrap shell for Lazers.
//!
//! `lash` is intentionally small: it owns prompt display, local line editing,
//! command-name parsing, and synchronous child launch through `spawn_wait`.
//! It now owns its process cwd through `cd`, supports a minimal `exit` built-in,
//! and performs its own argv parsing without exposing shell syntax to the
//! kernel or `liblazer`.

use lash::{
    scan_segments, ParseError, SegmentOperator, TokenizedCommand, LINE_CAPACITY,
    MAX_COMMAND_ARGS,
};
use liblazer::{
    self, print, println, ChdirError, GetEnvError, ListEnvError, SetEnvError,
    SpawnError, UnsetEnvError,
};

const COMMAND_PATH_CAPACITY: usize = LINE_CAPACITY + 1 + 128;
const ENV_LIST_BUFFER_CAPACITY: usize = 1024;
const PATH_BUFFER_CAPACITY: usize = 128;

liblazer::entry!(main);

fn main() -> ! {
    let mut shell = Shell::new();
    shell.start()
}

struct Shell {
    line: [u8; LINE_CAPACITY],
    len: usize,
    byte: [u8; 1],
    cwd: [u8; LINE_CAPACITY],
}

impl Shell {
    const fn new() -> Self {
        Self {
            line: [0; LINE_CAPACITY],
            len: 0,
            byte: [0; 1],
            cwd: [0; LINE_CAPACITY],
        }
    }

    fn start(&mut self) -> ! {
        let mut args = liblazer::args();
        let _ = args.next();
        if let Some(option) = args.next() {
            if option == "-c" {
                let Some(command_line) = args.next() else {
                    println!("lash: missing command string");
                    liblazer::exit(1);
                };
                let status = self.execute_line(command_line.as_bytes(), ExecutionMode::Batch);
                liblazer::exit(status);
            }
        }

        self.run_interactive()
    }

    fn run_interactive(&mut self) -> ! {
        println!("Lash !!");
        self.print_prompt();

        loop {
            let bytes_read = liblazer::stdin_read(&mut self.byte);
            if bytes_read == 0 {
                liblazer::yield_now();
                continue;
            }

            match self.byte[0] {
                b'\n' => self.submit_line(),
                0x7f => self.backspace(),
                0x20..=0x7e => self.append_printable(self.byte[0]),
                _ => {}
            }

            liblazer::yield_now();
        }
    }

    fn append_printable(&mut self, byte: u8) {
        if self.len >= LINE_CAPACITY {
            return;
        }

        self.line[self.len] = byte;
        self.len += 1;
        let _ = liblazer::stdout_write(&[byte]);
    }

    fn backspace(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        let _ = liblazer::stdout_write(&[0x7f]);
    }

    fn submit_line(&mut self) {
        println!();
        let mut line = [0u8; LINE_CAPACITY];
        line[..self.len].copy_from_slice(&self.line[..self.len]);
        let _ = self.execute_line(&line[..self.len], ExecutionMode::Interactive);
        self.len = 0;
        self.print_prompt();
    }

    fn execute_line(&mut self, line: &[u8], mode: ExecutionMode) -> usize {
        let segments = match scan_segments(line) {
            Ok(segments) => segments,
            Err(ParseError::UnmatchedSingleQuote)
            | Err(ParseError::UnmatchedDoubleQuote)
            | Err(ParseError::TrailingBackslash)
            | Err(ParseError::InvalidSyntax) => {
                self.print_parse_error(mode);
                return 1;
            }
            Err(ParseError::ResourceUnavailable) => {
                println!("lash: unable to parse command: resource unavailable");
                return 1;
            }
        };

        if segments.count() == 0 {
            return 0;
        }

        let mut last_status = 0usize;
        let mut index = 0usize;
        while index < segments.count() {
            let should_run = if index == 0 {
                true
            } else {
                match segments.operator_before(index).unwrap() {
                    SegmentOperator::And => last_status == 0,
                    SegmentOperator::Or => last_status != 0,
                    SegmentOperator::Sequence => true,
                }
            };

            if should_run {
                last_status = self.execute_segment(segments.segment(line, index).unwrap(), mode);
            }

            index += 1;
        }

        last_status
    }

    fn execute_segment(&mut self, line: &[u8], mode: ExecutionMode) -> usize {
        match TokenizedCommand::parse(line) {
            Ok(tokens) => {
                if tokens.count() == 0 {
                    return 0;
                }
                let command = tokens.token(0).unwrap_or("");
                if command == "cd" {
                    self.run_cd(&tokens)
                } else if command == "env" {
                    self.run_env(&tokens)
                } else if command == "set" {
                    self.run_set(&tokens)
                } else if command == "unset" {
                    self.run_unset(&tokens)
                } else if command == "exit" {
                    liblazer::exit(0);
                } else if !command.is_empty() {
                    self.run_command(command, &tokens, mode)
                } else {
                    self.command_not_found(command);
                    1
                }
            }
            Err(ParseError::UnmatchedSingleQuote)
            | Err(ParseError::UnmatchedDoubleQuote)
            | Err(ParseError::TrailingBackslash)
            | Err(ParseError::InvalidSyntax) => {
                self.print_parse_error(mode);
                1
            }
            Err(ParseError::ResourceUnavailable) => {
                println!("lash: unable to parse command: resource unavailable");
                1
            }
        }
    }

    fn run_cd(&mut self, tokens: &TokenizedCommand) -> usize {
        let path = if tokens.count() > 1 { tokens.token(1).unwrap_or("/") } else { "/" };

        match liblazer::chdir(path) {
            Ok(()) => 0,
            Err(ChdirError::InvalidPath | ChdirError::NotFound) => {
                println!("lash: directory not found: {}", path);
                1
            }
            Err(ChdirError::ResourceUnavailable) => {
                println!("lash: unable to change directory: resource unavailable");
                1
            }
        }
    }

    fn run_env(&self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() != 1 {
            println!("lash: usage: env");
            return 1;
        }

        let mut buffer = [0u8; ENV_LIST_BUFFER_CAPACITY];
        match liblazer::list_env(&mut buffer) {
            Ok(len) => {
                if len > 0 {
                    let _ = liblazer::stdout_write(&buffer[..len]);
                }
                0
            }
            Err(ListEnvError::BufferTooSmall) => {
                println!("lash: unable to list environment: buffer too small");
                1
            }
            Err(ListEnvError::ResourceUnavailable) => {
                println!("lash: unable to list environment: resource unavailable");
                1
            }
        }
    }

    fn run_set(&self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() < 3 {
            println!("lash: usage: set KEY VALUE...");
            return 1;
        }

        let key = tokens.token(1).unwrap_or("");
        let mut value_bytes = [0u8; LINE_CAPACITY];
        let mut value_len = 0usize;
        let mut index = 2usize;
        while index < tokens.count() {
            let token = tokens.token(index).unwrap_or("");
            let token_bytes = token.as_bytes();
            let separator_len = if index == 2 { 0 } else { 1 };
            if value_len + separator_len + token_bytes.len() > value_bytes.len() {
                println!("lash: unable to set environment: value too long");
                return 1;
            }
            if separator_len == 1 {
                value_bytes[value_len] = b' ';
                value_len += 1;
            }
            value_bytes[value_len..value_len + token_bytes.len()].copy_from_slice(token_bytes);
            value_len += token_bytes.len();
            index += 1;
        }

        let value = core::str::from_utf8(&value_bytes[..value_len]).unwrap_or("");
        match liblazer::set_env(key, value) {
            Ok(()) => 0,
            Err(SetEnvError::InvalidKey) => {
                println!("lash: invalid environment key: {}", key);
                1
            }
            Err(SetEnvError::KeyTooLong) => {
                println!("lash: environment key too long: {}", key);
                1
            }
            Err(SetEnvError::ValueTooLong) => {
                println!("lash: environment value too long for key: {}", key);
                1
            }
            Err(SetEnvError::CapacityExceeded) => {
                println!("lash: unable to set environment: capacity exceeded");
                1
            }
            Err(SetEnvError::ResourceUnavailable) => {
                println!("lash: unable to set environment: resource unavailable");
                1
            }
        }
    }

    fn run_unset(&self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() != 2 {
            println!("lash: usage: unset KEY");
            return 1;
        }

        let key = tokens.token(1).unwrap_or("");
        match liblazer::unset_env(key) {
            Ok(()) => 0,
            Err(UnsetEnvError::InvalidKey) => {
                println!("lash: invalid environment key: {}", key);
                1
            }
            Err(UnsetEnvError::NotFound) => {
                println!("lash: environment variable not found: {}", key);
                1
            }
            Err(UnsetEnvError::ResourceUnavailable) => {
                println!("lash: unable to unset environment: resource unavailable");
                1
            }
        }
    }

    fn run_command(&self, command: &str, tokens: &TokenizedCommand, mode: ExecutionMode) -> usize {
        let mut arguments = [""; MAX_COMMAND_ARGS];
        let mut argument_count = 0usize;
        let mut index = 1usize;
        while index < tokens.count() {
            if argument_count >= arguments.len() {
                println!("lash: unable to parse command: resource unavailable");
                return 1;
            }
            arguments[argument_count] = tokens.token(index).unwrap_or("");
            argument_count += 1;
            index += 1;
        }

        if command.starts_with('/') || command.as_bytes().contains(&b'/') {
            return self.run_command_at_path(command, command, &arguments[..argument_count], mode);
        }

        self.run_path_command(command, &arguments[..argument_count], mode)
    }

    fn command_not_found(&self, command: &str) {
        println!("lash: command not found: {}", command);
    }

    fn run_path_command(&self, command: &str, arguments: &[&str], mode: ExecutionMode) -> usize {
        let mut path_buffer = [0u8; PATH_BUFFER_CAPACITY];
        let path_len = match liblazer::get_env("PATH", &mut path_buffer) {
            Ok(len) => len,
            Err(GetEnvError::InvalidKey | GetEnvError::NotFound | GetEnvError::BufferTooSmall)
            | Err(GetEnvError::ResourceUnavailable) => {
                self.command_not_found(command);
                return 1;
            }
        };

        let path = match core::str::from_utf8(&path_buffer[..path_len]) {
            Ok(path) => path,
            Err(_) => {
                self.command_not_found(command);
                return 1;
            }
        };

        let mut candidate_buffer = [0u8; COMMAND_PATH_CAPACITY];
        let mut segment_start = 0usize;
        while segment_start <= path.len() {
            let remainder = &path[segment_start..];
            let segment_len = remainder.find(':').unwrap_or(remainder.len());
            let entry = &remainder[..segment_len];

            if !entry.is_empty() && entry.starts_with('/') {
                let needed = entry.len() + 1 + command.len();
                if needed > candidate_buffer.len() {
                    println!("lash: unable to run command: resource unavailable");
                    return 1;
                }

                candidate_buffer[..entry.len()].copy_from_slice(entry.as_bytes());
                candidate_buffer[entry.len()] = b'/';
                candidate_buffer[entry.len() + 1..needed].copy_from_slice(command.as_bytes());
                let candidate =
                    core::str::from_utf8(&candidate_buffer[..needed]).unwrap_or(command);

                match liblazer::spawn_wait(candidate, arguments) {
                    Ok(0) => return 0,
                    Ok(status) => {
                        if matches!(mode, ExecutionMode::Interactive) {
                            println!("lash: command exited with status {}", status);
                            println!();
                        }
                        return status;
                    }
                    Err(SpawnError::FileNotFound) => {}
                    Err(SpawnError::InvalidPath) => {
                        println!("lash: invalid path: {}", candidate);
                        return 1;
                    }
                    Err(SpawnError::InvalidExecutable) => {
                        println!("lash: invalid executable: {}", candidate);
                        return 1;
                    }
                    Err(SpawnError::ResourceUnavailable) => {
                        println!("lash: unable to run command: resource unavailable");
                        return 1;
                    }
                }
            }

            if segment_start + segment_len >= path.len() {
                break;
            }
            segment_start += segment_len + 1;
        }

        self.command_not_found(command);
        1
    }

    fn run_command_at_path(
        &self,
        display_name: &str,
        path: &str,
        arguments: &[&str],
        mode: ExecutionMode,
    ) -> usize {
        match liblazer::spawn_wait(path, arguments) {
            Ok(0) => 0,
            Ok(status) => {
                if matches!(mode, ExecutionMode::Interactive) {
                    println!("lash: command exited with status {}", status);
                    println!();
                }
                status
            }
            Err(SpawnError::FileNotFound) => {
                self.command_not_found(display_name);
                1
            }
            Err(SpawnError::InvalidPath) => {
                println!("lash: invalid path: {}", path);
                1
            }
            Err(SpawnError::InvalidExecutable) => {
                println!("lash: invalid executable: {}", path);
                1
            }
            Err(SpawnError::ResourceUnavailable) => {
                println!("lash: unable to run command: resource unavailable");
                1
            }
        }
    }

    fn print_parse_error(&self, mode: ExecutionMode) {
        println!("lash: parse error");
        if matches!(mode, ExecutionMode::Interactive) {
            println!();
        }
    }

    fn print_prompt(&mut self) {
        let cwd = match liblazer::getcwd(&mut self.cwd) {
            Ok(len) => core::str::from_utf8(&self.cwd[..len]).unwrap_or("/"),
            Err(_) => "/",
        };
        print!("{} > ", cwd);
    }
}

#[derive(Clone, Copy)]
enum ExecutionMode {
    Interactive,
    Batch,
}
