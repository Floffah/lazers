#![no_std]

//! Minimal Lazers userland runtime support.
//!
//! `liblazer` is the shared bootstrap layer for early user programs. It owns the
//! low-level process entry path, the current `int 0x80` syscall ABI bindings,
//! panic-to-exit behavior, and a tiny stdio-oriented text surface for userland
//! programs that do not yet have a full standard library.

mod env;
mod fmt;
mod fs;
mod io;
mod process;
mod runtime;
mod syscalls;

pub use env::{
    get_env, list_env, set_env, unset_env, GetEnvError, ListEnvError, SetEnvError, UnsetEnvError,
};
pub use fmt::{eprint, print};
#[doc(hidden)]
pub use fmt::{Stderr, Stdout};
pub use fs::{
    chdir, getcwd, read_dir, read_file, ChdirError, GetCwdError, ReadDirError, ReadFileError,
};
pub use io::{read, stderr_write, stdin_read, stdout_write, write};
pub use process::{exit, spawn_wait, spawn_wait_silent, yield_now, SpawnError};
pub use runtime::{args, Args};

/// Declares the Rust entrypoint for a Lazers user program.
///
/// The named function becomes the process' runtime entry without requiring each
/// binary to define its own `_start` shim or ABI glue.
#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[unsafe(no_mangle)]
        pub extern "Rust" fn __liblazer_main() -> ! {
            let main: fn() -> ! = $path;
            main()
        }
    };
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::print(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print(core::format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::print(core::format_args!("{}\n", core::format_args!($($arg)*)))
    };
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {
        $crate::eprint(core::format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::eprint(core::format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::eprint(core::format_args!("{}\n", core::format_args!($($arg)*)))
    };
}
