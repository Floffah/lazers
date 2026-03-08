//! Minimal syscall dispatch for the bootstrap user-mode runtime.
//!
//! The ABI is intentionally tiny: stdio-backed `read`/`write`, cooperative
//! `yield`, process launch, and a narrow directory listing call. This is enough
//! to validate early user programs without prematurely designing a wider kernel
//! API surface.

use crate::arch::TrapFrame;
use crate::memory;

const SYS_READ: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_YIELD: u64 = 2;
const SYS_EXIT: u64 = 3;
const SYS_SPAWN_WAIT: u64 = 4;
const SYS_READ_DIR: u64 = 5;

const SPAWN_ERROR_INVALID_PATH: usize = usize::MAX;
const SPAWN_ERROR_FILE_NOT_FOUND: usize = usize::MAX - 1;
const SPAWN_ERROR_INVALID_EXECUTABLE: usize = usize::MAX - 2;
const SPAWN_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
const READ_DIR_ERROR_INVALID_PATH: usize = usize::MAX;
const READ_DIR_ERROR_NOT_FOUND: usize = usize::MAX - 1;
const READ_DIR_ERROR_BUFFER_TOO_SMALL: usize = usize::MAX - 2;
const READ_DIR_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;

/// Dispatches a syscall trap frame in place.
///
/// The user-mode calling convention places the syscall number in `rax` and the
/// first four arguments in `rdi`, `rsi`, `rdx`, and `rcx`.
pub fn dispatch(frame: &mut TrapFrame) {
    match frame.rax {
        SYS_READ => {
            frame.rax = syscall_read(frame.rdi as usize, frame.rsi, frame.rdx as usize) as u64;
        }
        SYS_WRITE => {
            frame.rax = syscall_write(frame.rdi as usize, frame.rsi, frame.rdx as usize) as u64;
        }
        SYS_YIELD => {
            crate::scheduler::yield_now();
            frame.rax = 0;
        }
        SYS_EXIT => {
            crate::scheduler::exit_current_process(frame.rdi as usize);
        }
        SYS_SPAWN_WAIT => {
            frame.rax = syscall_spawn_wait(frame.rdi, frame.rsi as usize) as u64;
        }
        SYS_READ_DIR => {
            frame.rax =
                syscall_read_dir(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        _ => {
            frame.rax = 0;
        }
    }
}

fn syscall_read(fd: usize, buffer_address: u64, len: usize) -> usize {
    let Some(buffer) = memory::user_slice_mut(buffer_address, len) else {
        return 0;
    };

    crate::scheduler::current_process_read(fd, buffer)
}

fn syscall_write(fd: usize, buffer_address: u64, len: usize) -> usize {
    let Some(buffer) = memory::user_slice(buffer_address, len) else {
        return 0;
    };

    crate::scheduler::current_process_write(fd, buffer)
}

fn syscall_spawn_wait(path_address: u64, path_len: usize) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return SPAWN_ERROR_INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return SPAWN_ERROR_INVALID_PATH;
    };
    if !path.starts_with('/') {
        return SPAWN_ERROR_INVALID_PATH;
    }

    match crate::scheduler::spawn_user_process_and_wait(path) {
        Ok(status) => status,
        Err(crate::scheduler::SpawnError::InvalidPath) => SPAWN_ERROR_INVALID_PATH,
        Err(crate::scheduler::SpawnError::FileNotFound) => SPAWN_ERROR_FILE_NOT_FOUND,
        Err(crate::scheduler::SpawnError::InvalidExecutable) => SPAWN_ERROR_INVALID_EXECUTABLE,
        Err(crate::scheduler::SpawnError::ResourceUnavailable) => SPAWN_ERROR_RESOURCE_UNAVAILABLE,
    }
}

fn syscall_read_dir(
    path_address: u64,
    path_len: usize,
    buffer_address: u64,
    buffer_len: usize,
) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return READ_DIR_ERROR_INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return READ_DIR_ERROR_INVALID_PATH;
    };
    if !path.starts_with('/') {
        return READ_DIR_ERROR_INVALID_PATH;
    }

    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return READ_DIR_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::storage::read_root_dir(path, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(crate::storage::StorageError::PathNotAbsolute | crate::storage::StorageError::InvalidShortName) => {
            READ_DIR_ERROR_INVALID_PATH
        }
        Err(crate::storage::StorageError::FileNotFound | crate::storage::StorageError::NotADirectory) => {
            READ_DIR_ERROR_NOT_FOUND
        }
        Err(crate::storage::StorageError::BufferTooSmall) => READ_DIR_ERROR_BUFFER_TOO_SMALL,
        Err(_) => READ_DIR_ERROR_RESOURCE_UNAVAILABLE,
    }
}
