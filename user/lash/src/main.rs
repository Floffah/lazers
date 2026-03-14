#![no_main]
#![no_std]

//! First bootstrap shell for Lazers.
//!
//! `lash` is intentionally small: it owns prompt display, local line editing,
//! command-name parsing, and synchronous child launch through `spawn_wait`.
//! It now owns its process cwd through `cd`, supports a minimal `exit` built-in,
//! and performs its own argv parsing without exposing shell syntax to the
//! kernel or `liblazer`.

mod builtins;
mod commands;
mod paths;
mod repl;
mod shell;

use shell::Shell;

liblazer::entry!(main);

fn main() -> ! {
    let mut shell = Shell::new();
    shell.start()
}
