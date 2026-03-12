#![no_std]

//! Shared user/kernel ABI constants for Lazers.
//!
//! This crate is the single source of truth for syscall identifiers and raw
//! result codes used at the trap boundary. It intentionally does not include
//! kernel implementation logic or `liblazer`'s typed wrapper surface.

/// Syscall identifiers used with the current `int 0x80` trap ABI.
#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syscall {
    Read = 0,
    Write = 1,
    Yield = 2,
    Exit = 3,
    SpawnWait = 4,
    ReadDir = 5,
    Chdir = 6,
    GetCwd = 7,
    ReadFile = 8,
    GetEnv = 9,
    SetEnv = 10,
    UnsetEnv = 11,
}

/// Raw result codes for `spawn_wait`.
pub mod spawn_wait {
    pub const INVALID_PATH: usize = usize::MAX;
    pub const FILE_NOT_FOUND: usize = usize::MAX - 1;
    pub const INVALID_EXECUTABLE: usize = usize::MAX - 2;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
}

/// Raw result codes for `read_dir`.
pub mod read_dir {
    pub const INVALID_PATH: usize = usize::MAX;
    pub const NOT_FOUND: usize = usize::MAX - 1;
    pub const BUFFER_TOO_SMALL: usize = usize::MAX - 2;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
}

/// Raw result codes for `chdir`.
pub mod chdir {
    pub const INVALID_PATH: usize = usize::MAX;
    pub const NOT_FOUND: usize = usize::MAX - 1;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 2;
}

/// Raw result codes for `getcwd`.
pub mod getcwd {
    pub const BUFFER_TOO_SMALL: usize = usize::MAX;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 1;
}

/// Raw result codes for `read_file`.
pub mod read_file {
    pub const INVALID_PATH: usize = usize::MAX;
    pub const NOT_FOUND: usize = usize::MAX - 1;
    pub const NOT_A_FILE: usize = usize::MAX - 2;
    pub const BUFFER_TOO_SMALL: usize = usize::MAX - 3;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 4;
}

/// Raw result codes for `get_env`.
pub mod get_env {
    pub const INVALID_KEY: usize = usize::MAX;
    pub const NOT_FOUND: usize = usize::MAX - 1;
    pub const BUFFER_TOO_SMALL: usize = usize::MAX - 2;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
}

/// Raw result codes for `set_env`.
pub mod set_env {
    pub const INVALID_KEY: usize = usize::MAX;
    pub const KEY_TOO_LONG: usize = usize::MAX - 1;
    pub const VALUE_TOO_LONG: usize = usize::MAX - 2;
    pub const CAPACITY_EXCEEDED: usize = usize::MAX - 3;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 4;
}

/// Raw result codes for `unset_env`.
pub mod unset_env {
    pub const INVALID_KEY: usize = usize::MAX;
    pub const NOT_FOUND: usize = usize::MAX - 1;
    pub const RESOURCE_UNAVAILABLE: usize = usize::MAX - 2;
}
