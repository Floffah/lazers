//! Minimal syscall dispatch for the bootstrap user-mode runtime.
//!
//! The ABI is intentionally tiny: stdio-backed `read`/`write`, cooperative
//! `yield`, process launch, cwd management, and narrow filesystem calls. This
//! is enough to validate early user programs without prematurely designing a
//! wider kernel API surface.

use crate::arch::TrapFrame;
use crate::memory;
use kernel_abi::{self, Syscall};

/// Dispatches a syscall trap frame in place.
///
/// The user-mode calling convention places the syscall number in `rax` and the
/// first four arguments in `rdi`, `rsi`, `rdx`, and `rcx`.
pub fn dispatch(frame: &mut TrapFrame) {
    match frame.rax {
        value if value == Syscall::Read as u64 => {
            frame.rax = syscall_read(frame.rdi as usize, frame.rsi, frame.rdx as usize) as u64;
        }
        value if value == Syscall::Write as u64 => {
            frame.rax = syscall_write(frame.rdi as usize, frame.rsi, frame.rdx as usize) as u64;
        }
        value if value == Syscall::Yield as u64 => {
            crate::scheduler::yield_now();
            frame.rax = 0;
        }
        value if value == Syscall::Exit as u64 => {
            crate::scheduler::exit_current_process(frame.rdi as usize);
        }
        value if value == Syscall::SpawnWait as u64 => {
            frame.rax = syscall_spawn_wait(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        value if value == Syscall::SpawnWaitSilent as u64 => {
            frame.rax =
                syscall_spawn_wait_silent(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize)
                    as u64;
        }
        value if value == Syscall::ReadDir as u64 => {
            frame.rax =
                syscall_read_dir(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        value if value == Syscall::Chdir as u64 => {
            frame.rax = syscall_chdir(frame.rdi, frame.rsi as usize) as u64;
        }
        value if value == Syscall::GetCwd as u64 => {
            frame.rax = syscall_getcwd(frame.rdi, frame.rsi as usize) as u64;
        }
        value if value == Syscall::ReadFile as u64 => {
            frame.rax =
                syscall_read_file(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        value if value == Syscall::GetEnv as u64 => {
            frame.rax =
                syscall_get_env(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        value if value == Syscall::SetEnv as u64 => {
            frame.rax =
                syscall_set_env(frame.rdi, frame.rsi as usize, frame.rdx, frame.rcx as usize) as u64;
        }
        value if value == Syscall::UnsetEnv as u64 => {
            frame.rax = syscall_unset_env(frame.rdi, frame.rsi as usize) as u64;
        }
        value if value == Syscall::ListEnv as u64 => {
            frame.rax = syscall_list_env(frame.rdi, frame.rsi as usize) as u64;
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
        return kernel_abi::spawn_wait::INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return kernel_abi::spawn_wait::INVALID_PATH;
    };
    let Some(argv_tail) = memory::user_slice(argv_address, argv_len) else {
        return kernel_abi::spawn_wait::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::spawn_user_process_and_wait(path, argv_tail) {
        Ok(status) => status,
        Err(crate::scheduler::SpawnError::InvalidPath) => kernel_abi::spawn_wait::INVALID_PATH,
        Err(crate::scheduler::SpawnError::FileNotFound) => kernel_abi::spawn_wait::FILE_NOT_FOUND,
        Err(crate::scheduler::SpawnError::InvalidExecutable) => {
            kernel_abi::spawn_wait::INVALID_EXECUTABLE
        }
        Err(crate::scheduler::SpawnError::ResourceUnavailable) => {
            kernel_abi::spawn_wait::RESOURCE_UNAVAILABLE
        }
    }
}

fn syscall_spawn_wait_silent(
    path_address: u64,
    path_len: usize,
    argv_address: u64,
    argv_len: usize,
) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return kernel_abi::spawn_wait::INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return kernel_abi::spawn_wait::INVALID_PATH;
    };
    let Some(argv_tail) = memory::user_slice(argv_address, argv_len) else {
        return kernel_abi::spawn_wait::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::spawn_user_process_and_wait_silent(path, argv_tail) {
        Ok(status) => status,
        Err(crate::scheduler::SpawnError::InvalidPath) => kernel_abi::spawn_wait::INVALID_PATH,
        Err(crate::scheduler::SpawnError::FileNotFound) => kernel_abi::spawn_wait::FILE_NOT_FOUND,
        Err(crate::scheduler::SpawnError::InvalidExecutable) => {
            kernel_abi::spawn_wait::INVALID_EXECUTABLE
        }
        Err(crate::scheduler::SpawnError::ResourceUnavailable) => {
            kernel_abi::spawn_wait::RESOURCE_UNAVAILABLE
        }
    }
}

fn syscall_read_dir(
    path_address: u64,
    path_len: usize,
    buffer_address: u64,
    buffer_len: usize,
) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return kernel_abi::read_dir::INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return kernel_abi::read_dir::INVALID_PATH;
    };

    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return kernel_abi::read_dir::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_read_dir(path, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(
            crate::storage::StorageError::InvalidPath
            | crate::storage::StorageError::PathNotAbsolute
            | crate::storage::StorageError::InvalidShortName,
        ) => {
            kernel_abi::read_dir::INVALID_PATH
        }
        Err(crate::storage::StorageError::FileNotFound | crate::storage::StorageError::NotADirectory) => {
            kernel_abi::read_dir::NOT_FOUND
        }
        Err(crate::storage::StorageError::BufferTooSmall) => kernel_abi::read_dir::BUFFER_TOO_SMALL,
        Err(_) => kernel_abi::read_dir::RESOURCE_UNAVAILABLE,
    }
}

fn syscall_chdir(path_address: u64, path_len: usize) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return kernel_abi::chdir::INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return kernel_abi::chdir::INVALID_PATH;
    };

    match crate::scheduler::current_process_chdir(path) {
        Ok(()) => 0,
        Err(
            crate::storage::StorageError::InvalidPath
            | crate::storage::StorageError::PathNotAbsolute
            | crate::storage::StorageError::InvalidShortName,
        ) => kernel_abi::chdir::INVALID_PATH,
        Err(crate::storage::StorageError::FileNotFound | crate::storage::StorageError::NotADirectory) => {
            kernel_abi::chdir::NOT_FOUND
        }
        Err(_) => kernel_abi::chdir::RESOURCE_UNAVAILABLE,
    }
}

fn syscall_getcwd(buffer_address: u64, buffer_len: usize) -> usize {
    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return kernel_abi::getcwd::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_getcwd(buffer) {
        Some(bytes_written) => bytes_written,
        None if buffer_len == 0 => kernel_abi::getcwd::BUFFER_TOO_SMALL,
        None => kernel_abi::getcwd::BUFFER_TOO_SMALL,
    }
}

fn syscall_read_file(
    path_address: u64,
    path_len: usize,
    buffer_address: u64,
    buffer_len: usize,
) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return kernel_abi::read_file::INVALID_PATH;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return kernel_abi::read_file::INVALID_PATH;
    };

    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return kernel_abi::read_file::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_read_file(path, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(
            crate::storage::StorageError::InvalidPath
            | crate::storage::StorageError::PathNotAbsolute
            | crate::storage::StorageError::InvalidShortName,
        ) => kernel_abi::read_file::INVALID_PATH,
        Err(crate::storage::StorageError::FileNotFound) => kernel_abi::read_file::NOT_FOUND,
        Err(crate::storage::StorageError::NotAFile) => kernel_abi::read_file::NOT_A_FILE,
        Err(crate::storage::StorageError::BufferTooSmall) => kernel_abi::read_file::BUFFER_TOO_SMALL,
        Err(_) => kernel_abi::read_file::RESOURCE_UNAVAILABLE,
    }
}

fn syscall_get_env(
    key_address: u64,
    key_len: usize,
    buffer_address: u64,
    buffer_len: usize,
) -> usize {
    let Some(key_bytes) = memory::user_slice(key_address, key_len) else {
        return kernel_abi::get_env::INVALID_KEY;
    };
    let Ok(key) = core::str::from_utf8(key_bytes) else {
        return kernel_abi::get_env::INVALID_KEY;
    };
    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return kernel_abi::get_env::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_get_env(key, buffer) {
        Ok(bytes_written) => bytes_written,
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey)
        | Err(crate::scheduler::EnvironmentAccessError::KeyTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::ValueTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => kernel_abi::get_env::INVALID_KEY,
        Err(crate::scheduler::EnvironmentAccessError::NotFound) => kernel_abi::get_env::NOT_FOUND,
        Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall) => {
            kernel_abi::get_env::BUFFER_TOO_SMALL
        }
        Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable) => {
            kernel_abi::get_env::RESOURCE_UNAVAILABLE
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
        return kernel_abi::set_env::INVALID_KEY;
    };
    let Ok(key) = core::str::from_utf8(key_bytes) else {
        return kernel_abi::set_env::INVALID_KEY;
    };
    let Some(value_bytes) = memory::user_slice(value_address, value_len) else {
        return kernel_abi::set_env::RESOURCE_UNAVAILABLE;
    };
    let Ok(value) = core::str::from_utf8(value_bytes) else {
        return kernel_abi::set_env::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_set_env(key, value) {
        Ok(()) => 0,
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey) => kernel_abi::set_env::INVALID_KEY,
        Err(crate::scheduler::EnvironmentAccessError::KeyTooLong) => kernel_abi::set_env::KEY_TOO_LONG,
        Err(crate::scheduler::EnvironmentAccessError::ValueTooLong) => {
            kernel_abi::set_env::VALUE_TOO_LONG
        }
        Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => {
            kernel_abi::set_env::CAPACITY_EXCEEDED
        }
        Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable)
        | Err(crate::scheduler::EnvironmentAccessError::NotFound)
        | Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall) => {
            kernel_abi::set_env::RESOURCE_UNAVAILABLE
        }
    }
}

fn syscall_unset_env(key_address: u64, key_len: usize) -> usize {
    let Some(key_bytes) = memory::user_slice(key_address, key_len) else {
        return kernel_abi::unset_env::INVALID_KEY;
    };
    let Ok(key) = core::str::from_utf8(key_bytes) else {
        return kernel_abi::unset_env::INVALID_KEY;
    };

    match crate::scheduler::current_process_unset_env(key) {
        Ok(()) => 0,
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey)
        | Err(crate::scheduler::EnvironmentAccessError::KeyTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::ValueTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => {
            kernel_abi::unset_env::INVALID_KEY
        }
        Err(crate::scheduler::EnvironmentAccessError::NotFound) => kernel_abi::unset_env::NOT_FOUND,
        Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable)
        | Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall) => {
            kernel_abi::unset_env::RESOURCE_UNAVAILABLE
        }
    }
}

fn syscall_list_env(buffer_address: u64, buffer_len: usize) -> usize {
    let Some(buffer) = memory::user_slice_mut(buffer_address, buffer_len) else {
        return kernel_abi::list_env::RESOURCE_UNAVAILABLE;
    };

    match crate::scheduler::current_process_list_env(buffer) {
        Ok(bytes_written) => bytes_written,
        Err(crate::scheduler::EnvironmentAccessError::BufferTooSmall)
        | Err(crate::scheduler::EnvironmentAccessError::CapacityExceeded) => {
            kernel_abi::list_env::BUFFER_TOO_SMALL
        }
        Err(crate::scheduler::EnvironmentAccessError::InvalidKey)
        | Err(crate::scheduler::EnvironmentAccessError::KeyTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::ValueTooLong)
        | Err(crate::scheduler::EnvironmentAccessError::NotFound)
        | Err(crate::scheduler::EnvironmentAccessError::ResourceUnavailable) => {
            kernel_abi::list_env::RESOURCE_UNAVAILABLE
        }
    }
}
