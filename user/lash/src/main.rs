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
use liblazer::{self, print, println, ChdirError, SpawnError};

const COMMAND_PATH_CAPACITY: usize = LINE_CAPACITY + 5;

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

        let mut command_path = [0u8; COMMAND_PATH_CAPACITY];
        let spawn_path = if command.starts_with('/') || command.as_bytes().contains(&b'/') {
            command
        } else {
            command_path[..5].copy_from_slice(b"/bin/");
            let command_bytes = command.as_bytes();
            command_path[5..5 + command_bytes.len()].copy_from_slice(command_bytes);
            core::str::from_utf8(&command_path[..5 + command_bytes.len()]).unwrap_or(command)
        };

        match liblazer::spawn_wait(spawn_path, &arguments[..argument_count]) {
            Ok(0) => 0,
            Ok(status) => {
                if matches!(mode, ExecutionMode::Interactive) {
                    println!("lash: command exited with status {}", status);
                    println!();
                }
                status
            }
            Err(SpawnError::FileNotFound) => {
                self.command_not_found(command);
                1
            }
            Err(SpawnError::InvalidPath) => {
                println!("lash: invalid path: {}", spawn_path);
                1
            }
            Err(SpawnError::InvalidExecutable) => {
                println!("lash: invalid executable: {}", spawn_path);
                1
            }
            Err(SpawnError::ResourceUnavailable) => {
                println!("lash: unable to run command: resource unavailable");
                1
            }
        }
    }

    fn command_not_found(&self, command: &str) {
        println!("lash: command not found: {}", command);
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
