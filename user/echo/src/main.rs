#![no_main]
#![no_std]

use liblazer::{print, println};

liblazer::entry!(main);

fn main() -> ! {
    let mut args = liblazer::args();
    let _ = args.next();

    let mut first = true;
    for arg in args {
        if !first {
            print!(" ");
        }
        print!("{}", arg);
        first = false;
    }

    println!();
    liblazer::exit(0);
}
