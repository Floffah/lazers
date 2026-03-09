#![no_main]
#![no_std]

//! First filesystem-facing user command for Lazers.
//!
//! This bootstrap `ls` stays intentionally narrow: it lists one directory,
//! prints one entry per line, and relies on the kernel for newline-delimited
//! directory output instead of implementing its own directory parser.

use liblazer::{self, println, ReadDirError};

const DIRECTORY_BUFFER_SIZE: usize = 1024;

liblazer::entry!(main);

fn main() -> ! {
    let mut buffer = [0u8; DIRECTORY_BUFFER_SIZE];
    let mut args = liblazer::args();
    let _ = args.next();
    let path = args.next().unwrap_or(".");

    match liblazer::read_dir(path, &mut buffer) {
        Ok(bytes_written) => {
            let _ = liblazer::stdout_write(&buffer[..bytes_written]);
            liblazer::exit(0);
        }
        Err(ReadDirError::InvalidPath) => println!("ls: invalid path: {}", path),
        Err(ReadDirError::NotFound) => println!("ls: directory not found: {}", path),
        Err(ReadDirError::BufferTooSmall) => println!("ls: output too large"),
        Err(ReadDirError::ResourceUnavailable) => println!("ls: unable to read directory"),
    }

    liblazer::exit(1);
}
