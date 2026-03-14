use kernel_abi::Syscall;

use crate::syscalls::{syscall0, syscall1, syscall4};

const MAX_SPAWN_ARG_DATA: usize = 512;

/// First-step userspace spawn failures.
pub enum SpawnError {
    InvalidPath,
    FileNotFound,
    InvalidExecutable,
    ResourceUnavailable,
}

/// Cooperatively yields the current process' thread.
pub fn yield_now() {
    let _ = syscall0(Syscall::Yield);
}

/// Terminates the current process and never returns.
pub fn exit(code: usize) -> ! {
    let _ = syscall1(Syscall::Exit, code);

    loop {
        core::hint::spin_loop();
    }
}

/// Runs a child process from an absolute or cwd-relative path and blocks until it exits.
pub fn spawn_wait(path: &str, args: &[&str]) -> Result<usize, SpawnError> {
    spawn_wait_impl(Syscall::SpawnWait, path, args)
}

/// Runs a child process with inherited stdin and nulled stdout/stderr.
pub fn spawn_wait_silent(path: &str, args: &[&str]) -> Result<usize, SpawnError> {
    spawn_wait_impl(Syscall::SpawnWaitSilent, path, args)
}

fn spawn_wait_impl(syscall: Syscall, path: &str, args: &[&str]) -> Result<usize, SpawnError> {
    let mut payload = [0u8; MAX_SPAWN_ARG_DATA];
    let payload_len = serialize_spawn_args(args, &mut payload).map_err(|error| match error {
        SpawnSerializeError::InvalidUtf8 => SpawnError::InvalidPath,
        SpawnSerializeError::TooLarge => SpawnError::ResourceUnavailable,
    })?;
    let status = syscall4(
        syscall,
        path.as_ptr() as usize,
        path.len(),
        payload.as_ptr() as usize,
        payload_len,
    );
    match status {
        kernel_abi::spawn_wait::INVALID_PATH => Err(SpawnError::InvalidPath),
        kernel_abi::spawn_wait::FILE_NOT_FOUND => Err(SpawnError::FileNotFound),
        kernel_abi::spawn_wait::INVALID_EXECUTABLE => Err(SpawnError::InvalidExecutable),
        kernel_abi::spawn_wait::RESOURCE_UNAVAILABLE => Err(SpawnError::ResourceUnavailable),
        _ => Ok(status),
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
