#![no_main]
#![no_std]

liblazer::entry!(main);

fn main() -> ! {
    let mut byte = [0u8; 1];

    loop {
        let bytes_read = liblazer::stdin_read(&mut byte);
        if bytes_read == 0 {
            liblazer::yield_now();
            continue;
        }

        match byte[0] {
            b'\n' | 0x7f | 0x20..=0x7e => {
                let _ = liblazer::stdout_write(&byte);
            }
            _ => {}
        }

        liblazer::yield_now();
    }
}
