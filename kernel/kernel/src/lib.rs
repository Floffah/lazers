#![no_std]

//! Kernel library root for shared subsystem code.
//!
//! The freestanding kernel binary lives in `main.rs`, but the subsystem module
//! tree is rooted here so it can be reused across the binary entrypoint and the
//! library test target.

#[cfg(test)]
extern crate std;

#[cfg(not(test))]
#[macro_use]
mod macros;

#[cfg(not(test))]
pub mod arch;
#[cfg(not(test))]
pub mod console;
pub mod env;
#[cfg(not(test))]
pub mod font;
#[cfg(not(test))]
pub mod io;
#[cfg(not(test))]
pub mod keyboard;
pub mod memory;
#[cfg(not(test))]
pub mod pci;
#[cfg(not(test))]
pub mod port_io;
pub mod power;
#[cfg(not(test))]
pub mod process;
#[cfg(not(test))]
pub mod scheduler;
#[cfg(not(test))]
pub mod serial;
pub mod storage;
#[cfg(not(test))]
pub mod syscall;
#[cfg(not(test))]
pub mod terminal;
#[cfg(not(test))]
pub mod thread;

#[cfg(not(test))]
use core::arch::asm;

/// Halts the CPU forever.
#[cfg(not(test))]
pub fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!(
                include_str!("halt_forever.lib.asm"),
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}
