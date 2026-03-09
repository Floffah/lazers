#![no_main]
#![no_std]

//! First bootstrap shell for Lazers.
//!
//! `lash` is intentionally small: it owns prompt display, local line editing,
//! command-name parsing, and synchronous child launch through `spawn_wait`.
//! It now owns its process cwd through `cd`, supports a minimal `exit` built-in,
//! but still does not implement argv or environment handling.

use liblazer::{self, print, println, ChdirError, SpawnError};

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
    path_argument: [u8; LINE_CAPACITY],
    cwd: [u8; LINE_CAPACITY],
}

impl Shell {
    const fn new() -> Self {
        Self {
            line: [0; LINE_CAPACITY],
            len: 0,
            byte: [0; 1],
            command_name: [0; LINE_CAPACITY],
            command_path: [0; LINE_CAPACITY + 5],
            path_argument: [0; LINE_CAPACITY],
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

        if let Some((command_start, command_end)) = self.next_token_bounds(0) {
            let command_len = command_end - command_start;
            self.command_name[..command_len].copy_from_slice(&self.line[command_start..command_end]);

            if self.command_name[..command_len] == *b"cd" {
                self.run_cd(command_end);
            } else if self.command_name[..command_len] == *b"exit" {
                liblazer::exit(0);
            } else if let Some(path_len) = self.resolve_command_path(command_len) {
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

    fn next_token_bounds(&self, mut start: usize) -> Option<(usize, usize)> {
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

        Some((start, end))
    }

    fn resolve_command_path(&mut self, command_len: usize) -> Option<usize> {
        if command_len == 0 {
            return None;
        }

        if self.command_name[0] == b'/' {
            self.command_path[..command_len].copy_from_slice(&self.command_name[..command_len]);
            return Some(command_len);
        }

        if self.command_name[..command_len].contains(&b'/') {
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

    fn run_cd(&mut self, command_end: usize) {
        let path = if let Some((start, end)) = self.next_token_bounds(command_end) {
            let path_len = end - start;
            self.path_argument[..path_len].copy_from_slice(&self.line[start..end]);
            core::str::from_utf8(&self.path_argument[..path_len]).unwrap_or("/")
        } else {
            "/"
        };

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

    fn print_prompt(&mut self) {
        let cwd = match liblazer::getcwd(&mut self.cwd) {
            Ok(len) => core::str::from_utf8(&self.cwd[..len]).unwrap_or("/"),
            Err(_) => "/",
        };
        print!("{} > ", cwd);
    }
}
