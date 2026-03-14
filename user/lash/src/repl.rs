use lash::LINE_CAPACITY;
use liblazer::{self, print, println};

use crate::shell::{ExecutionMode, Shell};

impl Shell {
    pub(crate) fn run_interactive(&mut self) -> ! {
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

    pub(crate) fn append_printable(&mut self, byte: u8) {
        if self.len >= LINE_CAPACITY {
            return;
        }

        self.line[self.len] = byte;
        self.len += 1;
        let _ = liblazer::stdout_write(&[byte]);
    }

    pub(crate) fn backspace(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        let _ = liblazer::stdout_write(&[0x7f]);
    }

    pub(crate) fn submit_line(&mut self) {
        println!();
        let mut line = [0u8; LINE_CAPACITY];
        line[..self.len].copy_from_slice(&self.line[..self.len]);
        let _ = self.execute_line(&line[..self.len], ExecutionMode::Interactive);
        self.len = 0;
        self.print_prompt();
    }

    pub(crate) fn print_prompt(&mut self) {
        let cwd = match liblazer::getcwd(&mut self.cwd) {
            Ok(len) => core::str::from_utf8(&self.cwd[..len]).unwrap_or("/"),
            Err(_) => "/",
        };
        print!("{} > ", cwd);
    }
}
