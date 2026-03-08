#![no_main]
#![no_std]

//! First bootstrap shell for Lazers.
//!
//! `lash` is intentionally small: it owns prompt display, local line editing,
//! command-name parsing, and synchronous child launch through `spawn_wait`.
//! It does not yet implement built-ins, argv, cwd, or environment handling.

use liblazer::{self, print, println, SpawnError};

const PROMPT: &str = "/ > ";
const LINE_CAPACITY: usize = 256;

liblazer::entry!(main);

fn main() -> ! {
    let mut shell = Shell::new();
    shell.run()
}

struct Shell {
    line: [u8; LINE_CAPACITY],
    len: usize,
    byte: [u8; 1],
    command_name: [u8; LINE_CAPACITY],
    command_path: [u8; LINE_CAPACITY + 5],
}

impl Shell {
    const fn new() -> Self {
        Self {
            line: [0; LINE_CAPACITY],
            len: 0,
            byte: [0; 1],
            command_name: [0; LINE_CAPACITY],
            command_path: [0; LINE_CAPACITY + 5],
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

        if let Some(command_len) = self.copy_command_token() {
            if let Some(path_len) = self.resolve_command_path(command_len) {
                let path = core::str::from_utf8(&self.command_path[..path_len]).unwrap_or("");
                self.run_command(path, command_len);
            } else {
                let command = core::str::from_utf8(&self.command_name[..command_len]).unwrap_or("");
                self.command_not_found(command);
            }
        }

        self.len = 0;
        self.print_prompt();
    }

    fn copy_command_token(&mut self) -> Option<usize> {
        let mut start = 0;
        while start < self.len && self.line[start] == b' ' {
            start += 1;
        }

        if start == self.len {
            return None;
        }

        let mut end = start;
        while end < self.len && self.line[end] != b' ' {
            end += 1;
        }

        let token_len = end - start;
        self.command_name[..token_len].copy_from_slice(&self.line[start..end]);
        Some(token_len)
    }

    fn resolve_command_path(&mut self, command_len: usize) -> Option<usize> {
        if command_len == 0 {
            return None;
        }

        if self.command_name[0] == b'/' {
            self.command_path[..command_len].copy_from_slice(&self.command_name[..command_len]);
            return Some(command_len);
        }

        let required = 5 + command_len;
        if required > self.command_path.len() {
            return None;
        }

        self.command_path[..5].copy_from_slice(b"/bin/");
        self.command_path[5..required].copy_from_slice(&self.command_name[..command_len]);
        Some(required)
    }

    fn run_command(&self, path: &str, command_len: usize) {
        match liblazer::spawn_wait(path) {
            Ok(0) => {}
            Ok(status) => {
                println!("lash: command exited with status {}", status);
            }
            Err(SpawnError::FileNotFound) => {
                let command = core::str::from_utf8(&self.command_name[..command_len]).unwrap_or("");
                self.command_not_found(command);
            }
            Err(SpawnError::InvalidPath) => {
                println!("lash: invalid path: {}", path);
            }
            Err(SpawnError::InvalidExecutable) => {
                println!("lash: invalid executable: {}", path);
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

    fn print_prompt(&self) {
        print!("{}", PROMPT);
    }
}
