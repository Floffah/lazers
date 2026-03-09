#![no_main]
#![no_std]

use liblazer::{self, println, ReadFileError};

const FILE_BUFFER_SIZE: usize = 32 * 1024;

liblazer::entry!(main);

fn main() -> ! {
    let mut args = liblazer::args();
    let _ = args.next();
    let Some(path) = args.next() else {
        println!("cat: missing path");
        liblazer::exit(1);
    };

    let mut buffer = [0u8; FILE_BUFFER_SIZE];
    match liblazer::read_file(path, &mut buffer) {
        Ok(bytes_read) => {
            let _ = liblazer::stdout_write(&buffer[..bytes_read]);
            liblazer::exit(0);
        }
        Err(ReadFileError::InvalidPath) => println!("cat: invalid path: {}", path),
        Err(ReadFileError::NotFound) => println!("cat: file not found: {}", path),
        Err(ReadFileError::NotAFile) => println!("cat: not a file: {}", path),
        Err(ReadFileError::BufferTooSmall) => println!("cat: file too large"),
        Err(ReadFileError::ResourceUnavailable) => println!("cat: unable to read file"),
    }

    liblazer::exit(1);
}
