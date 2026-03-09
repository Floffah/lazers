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
    shell.run()
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

    fn run(&mut self) -> ! {
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

        match self.parse_tokens() {
            Ok(0) => {}
            Ok(count) => {
                let command = self.token(0).unwrap_or("");
                if command == "cd" {
                    self.run_cd(count);
                } else if command == "exit" {
                    liblazer::exit(0);
                } else if !command.is_empty() {
                    self.run_command(command, count);
                } else {
                    self.command_not_found(command);
                }
            }
            Err(ParseError::UnmatchedSingleQuote)
            | Err(ParseError::UnmatchedDoubleQuote)
            | Err(ParseError::TrailingBackslash) => {
                println!("lash: parse error");
            }
            Err(ParseError::ResourceUnavailable) => {
                println!("lash: unable to parse command: resource unavailable");
            }
        }

        self.len = 0;
        self.print_prompt();
    }

    fn run_cd(&mut self, count: usize) {
        let path = if count > 1 { self.token(1).unwrap_or("/") } else { "/" };

        match liblazer::chdir(path) {
            Ok(()) => {}
            Err(ChdirError::InvalidPath | ChdirError::NotFound) => {
                println!("lash: directory not found: {}", path);
            }
            Err(ChdirError::ResourceUnavailable) => {
                println!("lash: unable to change directory: resource unavailable");
            }
        }
    }

    fn run_command(&self, command: &str, count: usize) {
        let mut arguments = [""; MAX_COMMAND_ARGS];
        let mut argument_count = 0usize;
        let mut index = 1usize;
        while index < count {
            if argument_count >= arguments.len() {
                println!("lash: unable to parse command: resource unavailable");
                println!();
                return;
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
            Ok(0) => {}
            Ok(status) => {
                println!("lash: command exited with status {}", status);
            }
            Err(SpawnError::FileNotFound) => {
                self.command_not_found(command);
            }
            Err(SpawnError::InvalidPath) => {
                println!("lash: invalid path: {}", spawn_path);
            }
            Err(SpawnError::InvalidExecutable) => {
                println!("lash: invalid executable: {}", spawn_path);
            }
            Err(SpawnError::ResourceUnavailable) => {
                println!("lash: unable to run command: resource unavailable");
            }
        }

        println!();
    }

    fn command_not_found(&self, command: &str) {
        println!("lash: command not found: {}", command);
    }

    fn parse_tokens(&mut self) -> Result<usize, ParseError> {
        let mut state = ParseState::Unquoted;
        let mut source_index = 0usize;
        let mut token_count = 0usize;
        let mut storage_len = 0usize;
        let mut current_len = 0usize;
        let mut token_active = false;

        while source_index < self.len {
            let byte = self.line[source_index];
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
                        if source_index + 1 >= self.len {
                            return Err(ParseError::TrailingBackslash);
                        }
                        if !token_active {
                            self.start_token(token_count, storage_len)?;
                            token_active = true;
                        }
                        source_index += 1;
                        self.push_token_byte(storage_len, self.line[source_index])?;
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
                        if source_index + 1 >= self.len {
                            return Err(ParseError::TrailingBackslash);
                        }
                        source_index += 1;
                        self.push_token_byte(storage_len, self.line[source_index])?;
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
