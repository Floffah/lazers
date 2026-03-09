#![no_std]

//! Minimal Lazers userland runtime support.
//!
//! `liblazer` is the shared bootstrap layer for early user programs. It owns the
//! low-level process entry path, the current `int 0x80` syscall ABI bindings,
//! panic-to-exit behavior, and a tiny stdio-oriented text surface for userland
//! programs that do not yet have a full standard library.

use core::arch::global_asm;
use core::fmt;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::slice;
use core::str;

const SYS_READ: usize = 0;
const SYS_WRITE: usize = 1;
const SYS_YIELD: usize = 2;
const SYS_EXIT: usize = 3;
const SYS_SPAWN_WAIT: usize = 4;
const SYS_READ_DIR: usize = 5;
const SYS_CHDIR: usize = 6;
const SYS_GETCWD: usize = 7;
const MAX_SPAWN_ARG_DATA: usize = 512;
const SPAWN_ERROR_INVALID_PATH: usize = usize::MAX;
const SPAWN_ERROR_FILE_NOT_FOUND: usize = usize::MAX - 1;
const SPAWN_ERROR_INVALID_EXECUTABLE: usize = usize::MAX - 2;
const SPAWN_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
const READ_DIR_ERROR_INVALID_PATH: usize = usize::MAX;
const READ_DIR_ERROR_NOT_FOUND: usize = usize::MAX - 1;
const READ_DIR_ERROR_BUFFER_TOO_SMALL: usize = usize::MAX - 2;
const READ_DIR_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
const CHDIR_ERROR_INVALID_PATH: usize = usize::MAX;
const CHDIR_ERROR_NOT_FOUND: usize = usize::MAX - 1;
const CHDIR_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 2;
const GETCWD_ERROR_BUFFER_TOO_SMALL: usize = usize::MAX;
const GETCWD_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 1;

global_asm!(include_str!("lib.asm"));

unsafe extern "C" {
    fn user_syscall0(number: usize) -> usize;
    fn user_syscall1(number: usize, arg0: usize) -> usize;
    fn user_syscall3(number: usize, arg0: usize, arg1: usize, arg2: usize) -> usize;
    fn user_syscall4(number: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize) -> usize;
}

unsafe extern "Rust" {
    fn __liblazer_main() -> !;
}

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

/// First-step userspace spawn failures.
pub enum SpawnError {
    InvalidPath,
    FileNotFound,
    InvalidExecutable,
    ResourceUnavailable,
}

/// First-step userspace directory-listing failures.
pub enum ReadDirError {
    InvalidPath,
    NotFound,
    BufferTooSmall,
    ResourceUnavailable,
}

/// First-step userspace cwd update failures.
pub enum ChdirError {
    InvalidPath,
    NotFound,
    ResourceUnavailable,
}

/// First-step userspace cwd query failures.
pub enum GetCwdError {
    BufferTooSmall,
    ResourceUnavailable,
}

#[derive(Clone, Copy)]
struct StartupArgs {
    argc: usize,
    argv: *const usize,
}

static mut STARTUP_ARGS: StartupArgs = StartupArgs {
    argc: 0,
    argv: core::ptr::null(),
};

/// Reads bytes from a process-owned descriptor into the provided buffer.
pub fn read(fd: usize, buffer: &mut [u8]) -> usize {
    unsafe { user_syscall3(SYS_READ, fd, buffer.as_mut_ptr() as usize, buffer.len()) }
}

/// Writes bytes to a process-owned descriptor from the provided buffer.
pub fn write(fd: usize, buffer: &[u8]) -> usize {
    unsafe { user_syscall3(SYS_WRITE, fd, buffer.as_ptr() as usize, buffer.len()) }
}

/// Reads from the current process' standard input stream.
pub fn stdin_read(buffer: &mut [u8]) -> usize {
    read(0, buffer)
}

/// Writes to the current process' standard output stream.
pub fn stdout_write(buffer: &[u8]) -> usize {
    write(1, buffer)
}

/// Writes to the current process' standard error stream.
pub fn stderr_write(buffer: &[u8]) -> usize {
    write(2, buffer)
}

/// Cooperatively yields the current process' thread.
pub fn yield_now() {
    unsafe {
        let _ = user_syscall0(SYS_YIELD);
    }
}

/// Terminates the current process and never returns.
pub fn exit(code: usize) -> ! {
    unsafe {
        let _ = user_syscall1(SYS_EXIT, code);
    }

    loop {
        core::hint::spin_loop();
    }
}

/// Returns the current process arguments.
pub fn args() -> Args {
    let startup = unsafe { STARTUP_ARGS };
    Args {
        index: 0,
        argc: startup.argc,
        argv: startup.argv,
    }
}

/// Runs a child process from an absolute or cwd-relative path and blocks until it exits.
pub fn spawn_wait(path: &str, args: &[&str]) -> Result<usize, SpawnError> {
    let mut payload = [0u8; MAX_SPAWN_ARG_DATA];
    let payload_len = serialize_spawn_args(args, &mut payload).map_err(|error| match error {
        SpawnSerializeError::InvalidUtf8 => SpawnError::InvalidPath,
        SpawnSerializeError::TooLarge => SpawnError::ResourceUnavailable,
    })?;
    let status = unsafe {
        user_syscall4(
            SYS_SPAWN_WAIT,
            path.as_ptr() as usize,
            path.len(),
            payload.as_ptr() as usize,
            payload_len,
        )
    };
    match status {
        SPAWN_ERROR_INVALID_PATH => Err(SpawnError::InvalidPath),
        SPAWN_ERROR_FILE_NOT_FOUND => Err(SpawnError::FileNotFound),
        SPAWN_ERROR_INVALID_EXECUTABLE => Err(SpawnError::InvalidExecutable),
        SPAWN_ERROR_RESOURCE_UNAVAILABLE => Err(SpawnError::ResourceUnavailable),
        _ => Ok(status),
    }
}

/// Lists one absolute or cwd-relative directory into the provided newline-delimited buffer.
pub fn read_dir(path: &str, buffer: &mut [u8]) -> Result<usize, ReadDirError> {
    let status = unsafe {
        user_syscall4(
            SYS_READ_DIR,
            path.as_ptr() as usize,
            path.len(),
            buffer.as_mut_ptr() as usize,
            buffer.len(),
        )
    };
    match status {
        READ_DIR_ERROR_INVALID_PATH => Err(ReadDirError::InvalidPath),
        READ_DIR_ERROR_NOT_FOUND => Err(ReadDirError::NotFound),
        READ_DIR_ERROR_BUFFER_TOO_SMALL => Err(ReadDirError::BufferTooSmall),
        READ_DIR_ERROR_RESOURCE_UNAVAILABLE => Err(ReadDirError::ResourceUnavailable),
        _ => Ok(status),
    }
}

/// Changes the current process working directory.
pub fn chdir(path: &str) -> Result<(), ChdirError> {
    let status = unsafe { user_syscall3(SYS_CHDIR, path.as_ptr() as usize, path.len(), 0) };
    match status {
        0 => Ok(()),
        CHDIR_ERROR_INVALID_PATH => Err(ChdirError::InvalidPath),
        CHDIR_ERROR_NOT_FOUND => Err(ChdirError::NotFound),
        CHDIR_ERROR_RESOURCE_UNAVAILABLE => Err(ChdirError::ResourceUnavailable),
        _ => Err(ChdirError::ResourceUnavailable),
    }
}

/// Copies the current process working directory into the provided buffer.
pub fn getcwd(buffer: &mut [u8]) -> Result<usize, GetCwdError> {
    let status = unsafe { user_syscall3(SYS_GETCWD, buffer.as_mut_ptr() as usize, buffer.len(), 0) };
    match status {
        GETCWD_ERROR_BUFFER_TOO_SMALL => Err(GetCwdError::BufferTooSmall),
        GETCWD_ERROR_RESOURCE_UNAVAILABLE => Err(GetCwdError::ResourceUnavailable),
        _ => Ok(status),
    }
}

/// Iterator over the current process arguments.
pub struct Args {
    index: usize,
    argc: usize,
    argv: *const usize,
}

impl Iterator for Args {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.argc {
            return None;
        }

        let pointer = unsafe { *self.argv.add(self.index) } as *const u8;
        let length = c_string_len(pointer);
        let bytes = unsafe { slice::from_raw_parts(pointer, length) };
        let value = str::from_utf8(bytes).ok()?;
        self.index += 1;
        Some(value)
    }
}

/// Writes one formatted string to standard output.
pub fn print(args: fmt::Arguments<'_>) {
    let mut stdout = Stdout;
    let _ = stdout.write_fmt(args);
}

/// Writes one formatted string to standard error.
pub fn eprint(args: fmt::Arguments<'_>) {
    let mut stderr = Stderr;
    let _ = stderr.write_fmt(args);
}

#[doc(hidden)]
pub struct Stdout;

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = stdout_write(s.as_bytes());
        Ok(())
    }
}

#[doc(hidden)]
pub struct Stderr;

impl fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = stderr_write(s.as_bytes());
        Ok(())
    }
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

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1)
}

#[unsafe(no_mangle)]
extern "Rust" fn __liblazer_initialize(stack_top: usize) {
    let argc = unsafe { *(stack_top as *const usize) };
    let argv = unsafe { (stack_top as *const usize).add(1) };
    unsafe {
        STARTUP_ARGS = StartupArgs { argc, argv };
    }
}

enum SpawnSerializeError {
    InvalidUtf8,
    TooLarge,
}

fn serialize_spawn_args(args: &[&str], buffer: &mut [u8]) -> Result<usize, SpawnSerializeError> {
    let mut written = 0usize;
    for arg in args {
        if arg.as_bytes().contains(&0) {
            return Err(SpawnSerializeError::InvalidUtf8);
        }
        let required = arg.len() + 1;
        if written + required > buffer.len() {
            return Err(SpawnSerializeError::TooLarge);
        }
        buffer[written..written + arg.len()].copy_from_slice(arg.as_bytes());
        written += arg.len();
        buffer[written] = 0;
        written += 1;
    }
    Ok(written)
}

fn c_string_len(pointer: *const u8) -> usize {
    let mut length = 0usize;
    loop {
        let byte = unsafe { *pointer.add(length) };
        if byte == 0 {
            return length;
        }
        length += 1;
    }
}
