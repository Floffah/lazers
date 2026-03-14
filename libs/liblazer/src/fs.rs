use kernel_abi::Syscall;

use crate::syscalls::{syscall3, syscall4};

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

/// First-step userspace file-reading failures.
pub enum ReadFileError {
    InvalidPath,
    NotFound,
    NotAFile,
    BufferTooSmall,
    ResourceUnavailable,
}

/// Lists one absolute or cwd-relative directory into the provided newline-delimited buffer.
pub fn read_dir(path: &str, buffer: &mut [u8]) -> Result<usize, ReadDirError> {
    let status = syscall4(
        Syscall::ReadDir,
        path.as_ptr() as usize,
        path.len(),
        buffer.as_mut_ptr() as usize,
        buffer.len(),
    );
    match status {
        kernel_abi::read_dir::INVALID_PATH => Err(ReadDirError::InvalidPath),
        kernel_abi::read_dir::NOT_FOUND => Err(ReadDirError::NotFound),
        kernel_abi::read_dir::BUFFER_TOO_SMALL => Err(ReadDirError::BufferTooSmall),
        kernel_abi::read_dir::RESOURCE_UNAVAILABLE => Err(ReadDirError::ResourceUnavailable),
        _ => Ok(status),
    }
}

/// Changes the current process working directory.
pub fn chdir(path: &str) -> Result<(), ChdirError> {
    let status = syscall3(Syscall::Chdir, path.as_ptr() as usize, path.len(), 0);
    match status {
        0 => Ok(()),
        kernel_abi::chdir::INVALID_PATH => Err(ChdirError::InvalidPath),
        kernel_abi::chdir::NOT_FOUND => Err(ChdirError::NotFound),
        kernel_abi::chdir::RESOURCE_UNAVAILABLE => Err(ChdirError::ResourceUnavailable),
        _ => Err(ChdirError::ResourceUnavailable),
    }
}

/// Copies the current process working directory into the provided buffer.
pub fn getcwd(buffer: &mut [u8]) -> Result<usize, GetCwdError> {
    let status = syscall3(
        Syscall::GetCwd,
        buffer.as_mut_ptr() as usize,
        buffer.len(),
        0,
    );
    match status {
        kernel_abi::getcwd::BUFFER_TOO_SMALL => Err(GetCwdError::BufferTooSmall),
        kernel_abi::getcwd::RESOURCE_UNAVAILABLE => Err(GetCwdError::ResourceUnavailable),
        _ => Ok(status),
    }
}

/// Reads one file into the provided buffer using an absolute or cwd-relative path.
pub fn read_file(path: &str, buffer: &mut [u8]) -> Result<usize, ReadFileError> {
    let status = syscall4(
        Syscall::ReadFile,
        path.as_ptr() as usize,
        path.len(),
        buffer.as_mut_ptr() as usize,
        buffer.len(),
    );
    match status {
        kernel_abi::read_file::INVALID_PATH => Err(ReadFileError::InvalidPath),
        kernel_abi::read_file::NOT_FOUND => Err(ReadFileError::NotFound),
        kernel_abi::read_file::NOT_A_FILE => Err(ReadFileError::NotAFile),
        kernel_abi::read_file::BUFFER_TOO_SMALL => Err(ReadFileError::BufferTooSmall),
        kernel_abi::read_file::RESOURCE_UNAVAILABLE => Err(ReadFileError::ResourceUnavailable),
        _ => Ok(status),
    }
}
