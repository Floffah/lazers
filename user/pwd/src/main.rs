#![no_main]
#![no_std]

use liblazer::{self, println, GetCwdError};

const CWD_BUFFER_SIZE: usize = 256;

liblazer::entry!(main);

fn main() -> ! {
    let mut buffer = [0u8; CWD_BUFFER_SIZE];

    match liblazer::getcwd(&mut buffer) {
        Ok(len) => {
            let cwd = core::str::from_utf8(&buffer[..len]).unwrap_or("/");
            println!("{}", cwd);
            liblazer::exit(0);
        }
        Err(GetCwdError::BufferTooSmall) => {
            println!("pwd: path too large");
        }
        Err(GetCwdError::ResourceUnavailable) => {
            println!("pwd: unable to read working directory");
        }
    }

    liblazer::exit(1);
}
