use kernel_abi::Syscall;

use crate::syscalls::{syscall3, syscall4};

/// First-step userspace environment lookup failures.
pub enum GetEnvError {
    InvalidKey,
    NotFound,
    BufferTooSmall,
    ResourceUnavailable,
}

/// First-step userspace environment update failures.
pub enum SetEnvError {
    InvalidKey,
    KeyTooLong,
    ValueTooLong,
    CapacityExceeded,
    ResourceUnavailable,
}

/// First-step userspace environment removal failures.
pub enum UnsetEnvError {
    InvalidKey,
    NotFound,
    ResourceUnavailable,
}

/// First-step userspace environment-listing failures.
pub enum ListEnvError {
    BufferTooSmall,
    ResourceUnavailable,
}

/// Reads one process-owned environment variable into the provided buffer.
pub fn get_env(key: &str, buffer: &mut [u8]) -> Result<usize, GetEnvError> {
    let status = syscall4(
        Syscall::GetEnv,
        key.as_ptr() as usize,
        key.len(),
        buffer.as_mut_ptr() as usize,
        buffer.len(),
    );
    match status {
        kernel_abi::get_env::INVALID_KEY => Err(GetEnvError::InvalidKey),
        kernel_abi::get_env::NOT_FOUND => Err(GetEnvError::NotFound),
        kernel_abi::get_env::BUFFER_TOO_SMALL => Err(GetEnvError::BufferTooSmall),
        kernel_abi::get_env::RESOURCE_UNAVAILABLE => Err(GetEnvError::ResourceUnavailable),
        _ => Ok(status),
    }
}

/// Inserts or updates one process-owned environment variable.
pub fn set_env(key: &str, value: &str) -> Result<(), SetEnvError> {
    let status = syscall4(
        Syscall::SetEnv,
        key.as_ptr() as usize,
        key.len(),
        value.as_ptr() as usize,
        value.len(),
    );
    match status {
        0 => Ok(()),
        kernel_abi::set_env::INVALID_KEY => Err(SetEnvError::InvalidKey),
        kernel_abi::set_env::KEY_TOO_LONG => Err(SetEnvError::KeyTooLong),
        kernel_abi::set_env::VALUE_TOO_LONG => Err(SetEnvError::ValueTooLong),
        kernel_abi::set_env::CAPACITY_EXCEEDED => Err(SetEnvError::CapacityExceeded),
        kernel_abi::set_env::RESOURCE_UNAVAILABLE => Err(SetEnvError::ResourceUnavailable),
        _ => Err(SetEnvError::ResourceUnavailable),
    }
}

/// Removes one process-owned environment variable.
pub fn unset_env(key: &str) -> Result<(), UnsetEnvError> {
    let status = syscall3(Syscall::UnsetEnv, key.as_ptr() as usize, key.len(), 0);
    match status {
        0 => Ok(()),
        kernel_abi::unset_env::INVALID_KEY => Err(UnsetEnvError::InvalidKey),
        kernel_abi::unset_env::NOT_FOUND => Err(UnsetEnvError::NotFound),
        kernel_abi::unset_env::RESOURCE_UNAVAILABLE => Err(UnsetEnvError::ResourceUnavailable),
        _ => Err(UnsetEnvError::ResourceUnavailable),
    }
}

/// Serializes the current process environment as newline-delimited `KEY=VALUE`
/// entries in insertion order.
pub fn list_env(buffer: &mut [u8]) -> Result<usize, ListEnvError> {
    let status = syscall3(
        Syscall::ListEnv,
        buffer.as_mut_ptr() as usize,
        buffer.len(),
        0,
    );
    match status {
        kernel_abi::list_env::BUFFER_TOO_SMALL => Err(ListEnvError::BufferTooSmall),
        kernel_abi::list_env::RESOURCE_UNAVAILABLE => Err(ListEnvError::ResourceUnavailable),
        _ => Ok(status),
    }
}
