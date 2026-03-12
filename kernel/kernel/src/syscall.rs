//! Minimal syscall dispatch for the bootstrap user-mode runtime.
//!
//! The ABI is intentionally tiny: stdio-backed `read`/`write`, cooperative
//! `yield`, process launch, cwd management, and narrow filesystem calls. This
//! is enough to validate early user programs without prematurely designing a
//! wider kernel API surface.

use crate::arch::TrapFrame;
use crate::memory;

const SYS_READ: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_YIELD: u64 = 2;
const SYS_EXIT: u64 = 3;
const SYS_SPAWN_WAIT: u64 = 4;
const SYS_READ_DIR: u64 = 5;
const SYS_CHDIR: u64 = 6;
const SYS_GETCWD: u64 = 7;
const SYS_READ_FILE: u64 = 8;
const SYS_GET_ENV: u64 = 9;
const SYS_SET_ENV: u64 = 10;
const SYS_UNSET_ENV: u64 = 11;

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
const READ_FILE_ERROR_INVALID_PATH: usize = usize::MAX;
const READ_FILE_ERROR_NOT_FOUND: usize = usize::MAX - 1;
const READ_FILE_ERROR_NOT_A_FILE: usize = usize::MAX - 2;
const READ_FILE_ERROR_BUFFER_TOO_SMALL: usize = usize::MAX - 3;
const READ_FILE_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 4;
const GET_ENV_ERROR_INVALID_KEY: usize = usize::MAX;
const GET_ENV_ERROR_NOT_FOUND: usize = usize::MAX - 1;
const GET_ENV_ERROR_BUFFER_TOO_SMALL: usize = usize::MAX - 2;
const GET_ENV_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 3;
const SET_ENV_ERROR_INVALID_KEY: usize = usize::MAX;
const SET_ENV_ERROR_KEY_TOO_LONG: usize = usize::MAX - 1;
const SET_ENV_ERROR_VALUE_TOO_LONG: usize = usize::MAX - 2;
const SET_ENV_ERROR_CAPACITY_EXCEEDED: usize = usize::MAX - 3;
const SET_ENV_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 4;
const UNSET_ENV_ERROR_INVALID_KEY: usize = usize::MAX;
const UNSET_ENV_ERROR_NOT_FOUND: usize = usize::MAX - 1;
const UNSET_ENV_ERROR_RESOURCE_UNAVAILABLE: usize = usize::MAX - 2;

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
            frame.rax = syscall_spawn_wait(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        SYS_READ_DIR => {
            frame.rax =
                syscall_read_dir(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        SYS_CHDIR => {
            frame.rax = syscall_chdir(frame.rdi, frame.rsi as usize) as u64;
        }
        SYS_GETCWD => {
            frame.rax = syscall_getcwd(frame.rdi, frame.rsi as usize) as u64;
        }
        SYS_READ_FILE => {
            frame.rax =
                syscall_read_file(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        SYS_GET_ENV => {
            frame.rax =
                syscall_get_env(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        SYS_SET_ENV => {
            frame.rax =
                syscall_set_env(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        SYS_UNSET_ENV => {
            frame.rax = syscall_unset_env(frame.rdi, frame.rsi as usize) as u64;
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

fn syscall_spawn_wait(path_address: u64, path_len: usize, argv_address: u64, argv_len: usize) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return SPAWN_ERROR_INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return SPAWN_ERROR_INVALID_PATH;
    };
    let Some(argv_tail) = memory::user_slice(argv_address, argv_len) else {
        return SPAWN_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::spawn_user_process_and_wait(path, argv_tail) {
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

    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return READ_DIR_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_read_dir(path, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(
            crate::storage::StorageError::InvalidPath
            | crate::storage::StorageError::PathNotAbsolute
            | crate::storage::StorageError::InvalidShortName,
        ) => {
            READ_DIR_ERROR_INVALID_PATH
        }
        Err(crate::storage::StorageError::FileNotFound | crate::storage::StorageError::NotADirectory) => {
            READ_DIR_ERROR_NOT_FOUND
        }
        Err(crate::storage::StorageError::BufferTooSmall) => READ_DIR_ERROR_BUFFER_TOO_SMALL,
        Err(_) => READ_DIR_ERROR_RESOURCE_UNAVAILABLE,
    }
}

fn syscall_chdir(path_address: u64, path_len: usize) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return CHDIR_ERROR_INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return CHDIR_ERROR_INVALID_PATH;
    };

    match crate::scheduler::current_process_chdir(path) {
        Ok(()) => 0,
        Err(
            crate::storage::StorageError::InvalidPath
            | crate::storage::StorageError::PathNotAbsolute
            | crate::storage::StorageError::InvalidShortName,
        ) => CHDIR_ERROR_INVALID_PATH,
        Err(crate::storage::StorageError::FileNotFound | crate::storage::StorageError::NotADirectory) => {
            CHDIR_ERROR_NOT_FOUND
        }
        Err(_) => CHDIR_ERROR_RESOURCE_UNAVAILABLE,
    }
}

fn syscall_getcwd(buffer_address: u64, buffer_len: usize) -> usize {
    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return GETCWD_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_getcwd(buffer) {
        Some(bytes_written) => bytes_written,
        None if buffer_len == 0 => GETCWD_ERROR_BUFFER_TOO_SMALL,
        None => GETCWD_ERROR_BUFFER_TOO_SMALL,
    }
}

fn syscall_read_file(
    path_address: u64,
    path_len: usize,
    buffer_address: u64,
    buffer_len: usize,
) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return READ_FILE_ERROR_INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return READ_FILE_ERROR_INVALID_PATH;
    };

    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return READ_FILE_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_read_file(path, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(
            crate::storage::StorageError::InvalidPath
            | crate::storage::StorageError::PathNotAbsolute
            | crate::storage::StorageError::InvalidShortName,
        ) => READ_FILE_ERROR_INVALID_PATH,
        Err(crate::storage::StorageError::FileNotFound) => READ_FILE_ERROR_NOT_FOUND,
        Err(crate::storage::StorageError::NotAFile) => READ_FILE_ERROR_NOT_A_FILE,
        Err(crate::storage::StorageError::BufferTooSmall) => READ_FILE_ERROR_BUFFER_TOO_SMALL,
        Err(_) => READ_FILE_ERROR_RESOURCE_UNAVAILABLE,
    }
}

fn syscall_get_env(
    key_address: u64,
    key_len: usize,
    buffer_address: u64,
    buffer_len: usize,
) -> usize {
    let Some(key_bytes) = memory::user_slice(key_address, key_len) else {
        return GET_ENV_ERROR_INVALID_KEY;
    };
    let Ok(key) = core::str::from_utf8(key_bytes) else {
        return GET_ENV_ERROR_INVALID_KEY;
    };
    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return GET_ENV_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_get_env(key, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey)
        | Err(crate::scheduler::EnvironmentAccessError::KeyTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::ValueTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => GET_ENV_ERROR_INVALID_KEY,
        Err(crate::scheduler::EnvironmentAccessError::NotFound) => GET_ENV_ERROR_NOT_FOUND,
        Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall) => GET_ENV_ERROR_BUFFER_TOO_SMALL,
        Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable) => {
            GET_ENV_ERROR_RESOURCE_UNAVAILABLE
        }
    }
}

fn syscall_set_env(
    key_address: u64,
    key_len: usize,
    value_address: u64,
    value_len: usize,
) -> usize {
    let Some(key_bytes) = memory::user_slice(key_address, key_len) else {
        return SET_ENV_ERROR_INVALID_KEY;
    };
    let Ok(key) = core::str::from_utf8(key_bytes) else {
        return SET_ENV_ERROR_INVALID_KEY;
    };
    let Some(value_bytes) = memory::user_slice(value_address, value_len) else {
        return SET_ENV_ERROR_RESOURCE_UNAVAILABLE;
    };
    let Ok(value) = core::str::from_utf8(value_bytes) else {
        return SET_ENV_ERROR_RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_set_env(key, value) {
        Ok(()) => 0,
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey) => SET_ENV_ERROR_INVALID_KEY,
        Err(crate::scheduler::EnvironmentAccessError::KeyTooLong) => SET_ENV_ERROR_KEY_TOO_LONG,
        Err(crate::scheduler::EnvironmentAccessError::ValueTooLong) => SET_ENV_ERROR_VALUE_TOO_LONG,
        Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => {
            SET_ENV_ERROR_CAPACITY_EXCEEDED
        }
        Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable)
        | Err(crate::scheduler::EnvironmentAccessError::NotFound)
        | Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall) => {
            SET_ENV_ERROR_RESOURCE_UNAVAILABLE
        }
    }
}

fn syscall_unset_env(key_address: u64, key_len: usize) -> usize {
    let Some(key_bytes) = memory::user_slice(key_address, key_len) else {
        return UNSET_ENV_ERROR_INVALID_KEY;
    };
    let Ok(key) = core::str::from_utf8(key_bytes) else {
        return UNSET_ENV_ERROR_INVALID_KEY;
    };

    match crate::scheduler::current_process_unset_env(key) {
        Ok(()) => 0,
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey)
        | Err(crate::scheduler::EnvironmentAccessError::KeyTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::ValueTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => UNSET_ENV_ERROR_INVALID_KEY,
        Err(crate::scheduler::EnvironmentAccessError::NotFound) => UNSET_ENV_ERROR_NOT_FOUND,
        Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable)
        | Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall) => {
            UNSET_ENV_ERROR_RESOURCE_UNAVAILABLE
        }
    }
}
