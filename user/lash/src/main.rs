#![no_main]
#![no_std]

//! First bootstrap shell for Lazers.
//!
//! `lash` is intentionally small: it owns prompt display, local line editing,
//! command-name parsing, and synchronous child launch through `spawn_wait`.
//! It now owns its process cwd through `cd`, supports a minimal `exit` built-in,
//! and performs its own argv parsing without exposing shell syntax to the
//! kernel or `liblazer`.

use liblazer::{self, print, println, ChdirError, SpawnError};

const LINE_CAPACITY: usize = 256;
const MAX_COMMAND_ARGS: usize = 16;
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
    token_storage: [u8; LINE_CAPACITY],
    token_offsets: [usize; MAX_COMMAND_ARGS],
    token_lengths: [usize; MAX_COMMAND_ARGS],
    cwd: [u8; LINE_CAPACITY],
}

impl Shell {
    const fn new() -> Self {
        Self {
            line: [0; LINE_CAPACITY],
            len: 0,
            byte: [0; 1],
            token_storage: [0; LINE_CAPACITY],
            token_offsets: [0; MAX_COMMAND_ARGS],
            token_lengths: [0; MAX_COMMAND_ARGS],
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
        let mut segment_start = 0usize;
        let mut index = 0usize;
        let mut saw_separator = false;
        let mut state = ParseState::Unquoted;

        while index < line.len() {
            let byte = line[index];
            match state {
                ParseState::Unquoted => match byte {
                    b'\'' => state = ParseState::SingleQuoted,
                    b'"' => state = ParseState::DoubleQuoted,
                    b'\\' => {
                        if index + 1 < line.len() {
                            index += 2;
                            continue;
                        }
                    }
                    b'&' if index + 1 < line.len() && line[index + 1] == b'&' => {
                        let segment = trim_spaces(&line[segment_start..index]);
                        if segment.is_empty() {
                            self.print_parse_error(mode);
                            return 1;
                        }

                        saw_separator = true;
                        let status = self.execute_segment(segment, mode);
                        if status != 0 {
                            return status;
                        }

                        index += 2;
                        segment_start = index;
                        continue;
                    }
                    _ => {}
                },
                ParseState::SingleQuoted => {
                    if byte == b'\'' {
                        state = ParseState::Unquoted;
                    }
                }
                ParseState::DoubleQuoted => match byte {
                    b'"' => state = ParseState::Unquoted,
                    b'\\' => {
                        if index + 1 < line.len() {
                            index += 2;
                            continue;
                        }
                    }
                    _ => {}
                },
            }

            index += 1;
        }

        let final_segment = trim_spaces(&line[segment_start..]);
        if final_segment.is_empty() {
            if saw_separator {
                self.print_parse_error(mode);
                return 1;
            }
            return 0;
        }

        self.execute_segment(final_segment, mode)
    }

    fn execute_segment(&mut self, line: &[u8], mode: ExecutionMode) -> usize {
        match self.parse_tokens(line) {
            Ok(0) => 0,
            Ok(count) => {
                let command = self.token(0).unwrap_or("");
                if command == "cd" {
                    self.run_cd(count)
                } else if command == "exit" {
                    liblazer::exit(0);
                } else if !command.is_empty() {
                    self.run_command(command, count, mode)
                } else {
                    self.command_not_found(command);
                    1
                }
            }
            Err(ParseError::UnmatchedSingleQuote)
            | Err(ParseError::UnmatchedDoubleQuote)
            | Err(ParseError::TrailingBackslash) => {
                self.print_parse_error(mode);
                1
            }
            Err(ParseError::ResourceUnavailable) => {
                println!("lash: unable to parse command: resource unavailable");
                1
            }
        }
    }

    fn run_cd(&mut self, count: usize) -> usize {
        let path = if count > 1 { self.token(1).unwrap_or("/") } else { "/" };

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

    fn run_command(&self, command: &str, count: usize, mode: ExecutionMode) -> usize {
        let mut arguments = [""; MAX_COMMAND_ARGS];
        let mut argument_count = 0usize;
        let mut index = 1usize;
        while index < count {
            if argument_count >= arguments.len() {
                println!("lash: unable to parse command: resource unavailable");
                return 1;
            }
            arguments[argument_count] = self.token(index).unwrap_or("");
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

    fn parse_tokens(&mut self, line: &[u8]) -> Result<usize, ParseError> {
        let mut state = ParseState::Unquoted;
        let mut source_index = 0usize;
        let mut token_count = 0usize;
        let mut storage_len = 0usize;
        let mut current_len = 0usize;
        let mut token_active = false;

        while source_index < line.len() {
            let byte = line[source_index];
            match state {
                ParseState::Unquoted => match byte {
                    b' ' => {
                        if token_active {
                            self.finish_token(token_count, current_len)?;
                            token_count += 1;
                            current_len = 0;
                            token_active = false;
                        }
                    }
                    b'\'' => {
                        if !token_active {
                            self.start_token(token_count, storage_len)?;
                            token_active = true;
                        }
                        state = ParseState::SingleQuoted;
                    }
                    b'"' => {
                        if !token_active {
                            self.start_token(token_count, storage_len)?;
                            token_active = true;
                        }
                        state = ParseState::DoubleQuoted;
                    }
                    b'\\' => {
                        if source_index + 1 >= line.len() {
                            return Err(ParseError::TrailingBackslash);
                        }
                        if !token_active {
                            self.start_token(token_count, storage_len)?;
                            token_active = true;
                        }
                        source_index += 1;
                        self.push_token_byte(storage_len, line[source_index])?;
                        storage_len += 1;
                        current_len += 1;
                    }
                    _ => {
                        if !token_active {
                            self.start_token(token_count, storage_len)?;
                            token_active = true;
                        }
                        self.push_token_byte(storage_len, byte)?;
                        storage_len += 1;
                        current_len += 1;
                    }
                },
                ParseState::SingleQuoted => {
                    if byte == b'\'' {
                        state = ParseState::Unquoted;
                    } else {
                        self.push_token_byte(storage_len, byte)?;
                        storage_len += 1;
                        current_len += 1;
                    }
                }
                ParseState::DoubleQuoted => match byte {
                    b'"' => state = ParseState::Unquoted,
                    b'\\' => {
                        if source_index + 1 >= line.len() {
                            return Err(ParseError::TrailingBackslash);
                        }
                        source_index += 1;
                        self.push_token_byte(storage_len, line[source_index])?;
                        storage_len += 1;
                        current_len += 1;
                    }
                    _ => {
                        self.push_token_byte(storage_len, byte)?;
                        storage_len += 1;
                        current_len += 1;
                    }
                },
            }

            source_index += 1;
        }

        match state {
            ParseState::SingleQuoted => return Err(ParseError::UnmatchedSingleQuote),
            ParseState::DoubleQuoted => return Err(ParseError::UnmatchedDoubleQuote),
            ParseState::Unquoted => {}
        }

        if token_active {
            self.finish_token(token_count, current_len)?;
            token_count += 1;
        }

        Ok(token_count)
    }

    fn start_token(&mut self, token_index: usize, storage_offset: usize) -> Result<(), ParseError> {
        if token_index >= self.token_offsets.len() {
            return Err(ParseError::ResourceUnavailable);
        }
        self.token_offsets[token_index] = storage_offset;
        self.token_lengths[token_index] = 0;
        Ok(())
    }

    fn finish_token(&mut self, token_index: usize, token_len: usize) -> Result<(), ParseError> {
        if token_index >= self.token_lengths.len() {
            return Err(ParseError::ResourceUnavailable);
        }
        self.token_lengths[token_index] = token_len;
        Ok(())
    }

    fn push_token_byte(&mut self, storage_index: usize, byte: u8) -> Result<(), ParseError> {
        if storage_index >= self.token_storage.len() {
            return Err(ParseError::ResourceUnavailable);
        }
        self.token_storage[storage_index] = byte;
        Ok(())
    }

    fn token(&self, index: usize) -> Option<&str> {
        let start = *self.token_offsets.get(index)?;
        let len = *self.token_lengths.get(index)?;
        core::str::from_utf8(&self.token_storage[start..start + len]).ok()
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
enum ParseState {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}

#[derive(Clone, Copy)]
enum ParseError {
    UnmatchedSingleQuote,
    UnmatchedDoubleQuote,
    TrailingBackslash,
    ResourceUnavailable,
}

#[derive(Clone, Copy)]
enum ExecutionMode {
    Interactive,
    Batch,
}

fn trim_spaces(bytes: &[u8]) -> &[u8] {
    let mut start = 0usize;
    let mut end = bytes.len();

    while start < end && bytes[start] == b' ' {
        start += 1;
    }
    while end > start && bytes[end - 1] == b' ' {
        end -= 1;
    }

    &bytes[start..end]
}
