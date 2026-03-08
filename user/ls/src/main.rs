#![no_main]
#![no_std]

//! First filesystem-facing user command for Lazers.
//!
//! This bootstrap `ls` is intentionally narrow: it lists `/` only, prints one
//! entry per line, and relies on the kernel for newline-delimited directory
//! output instead of implementing its own directory parser.

use liblazer::{self, println, ReadDirError};

const DIRECTORY_BUFFER_SIZE: usize = 1024;

liblazer::entry!(main);

fn main() -> ! {
    let mut buffer = [0u8; DIRECTORY_BUFFER_SIZE];

    match liblazer::read_dir("/", &mut buffer) {
        Ok(bytes_written) => {
            let _ = liblazer::stdout_write(&buffer[..bytes_written]);
            liblazer::exit(0);
        }
        Err(ReadDirError::InvalidPath) => println!("ls: invalid path: /"),
        Err(ReadDirError::NotFound) => println!("ls: directory not found: /"),
        Err(ReadDirError::BufferTooSmall) => println!("ls: output too large"),
        Err(ReadDirError::ResourceUnavailable) => println!("ls: unable to read directory"),
    }

    liblazer::exit(1);
}
