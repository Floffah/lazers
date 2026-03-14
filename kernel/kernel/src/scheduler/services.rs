use crate::env::EnvironmentError;
use crate::process::{Process, MAX_CWD_LEN};
use crate::storage;

use super::core::wait_for_child;
use super::state::{with_scheduler, with_scheduler_mut};

/// First-step child process launch failures surfaced to userspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnError {
    InvalidPath,
    FileNotFound,
    InvalidExecutable,
    ResourceUnavailable,
}

/// Failures surfaced by current-process environment helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentAccessError {
    InvalidKey,
    NotFound,
    BufferTooSmall,
    KeyTooLong,
    ValueTooLong,
    CapacityExceeded,
    ResourceUnavailable,
}

struct NormalizedPath {
    bytes: [u8; MAX_CWD_LEN],
    len: usize,
}

impl NormalizedPath {
    fn resolve(cwd: &str, path: &str) -> Result<Self, storage::StorageError> {
        let mut bytes = [0u8; MAX_CWD_LEN];
        let len = storage::normalize_path(cwd, path, &mut bytes)?;
        Ok(Self { bytes, len })
    }

    fn as_str(&self) -> Result<&str, storage::StorageError> {
        core::str::from_utf8(&self.bytes[..self.len])
            .map_err(|_| storage::StorageError::InvalidPath)
    }
}

/// Loads a child executable from the runtime root filesystem, runs it with
/// inherited stdio and cwd, and blocks until it exits.
pub fn spawn_user_process_and_wait(path: &str, argv_tail: &[u8]) -> Result<usize, SpawnError> {
    spawn_user_process_and_wait_with_stdio(path, argv_tail, false)
}

/// Loads a child executable and runs it with inherited stdin and nulled stdout/stderr.
pub fn spawn_user_process_and_wait_silent(
    path: &str,
    argv_tail: &[u8],
) -> Result<usize, SpawnError> {
    spawn_user_process_and_wait_with_stdio(path, argv_tail, true)
}

fn spawn_user_process_and_wait_with_stdio(
    path: &str,
    argv_tail: &[u8],
    silent_stdio: bool,
) -> Result<usize, SpawnError> {
    let current = with_scheduler(|scheduler| scheduler.current_thread)
        .ok_or(SpawnError::ResourceUnavailable)?;
    let parent_process_id = with_scheduler(|scheduler| scheduler.thread(current).process_id());
    let normalized_path = with_scheduler(|scheduler| {
        NormalizedPath::resolve(scheduler.process(parent_process_id).cwd(), path)
    })
    .map_err(map_storage_spawn_error)?;
    let normalized_path = normalized_path.as_str().map_err(map_storage_spawn_error)?;

    let file = match storage::read_root_file(normalized_path) {
        Ok(file) => file,
        Err(storage::StorageError::FileNotFound | storage::StorageError::NotAFile) => {
            return Err(SpawnError::FileNotFound);
        }
        Err(
            storage::StorageError::InvalidPath
            | storage::StorageError::PathNotAbsolute
            | storage::StorageError::InvalidShortName,
        ) => {
            return Err(SpawnError::InvalidPath);
        }
        Err(_) => {
            return Err(SpawnError::ResourceUnavailable);
        }
    };
    let startup = crate::memory::ProgramStartup {
        argv0: normalized_path,
        argv_tail,
    };
    let program = match crate::memory::load_user_program(file.as_slice(), &startup) {
        Ok(program) => program,
        Err(_) => {
            file.release();
            return Err(SpawnError::InvalidExecutable);
        }
    };
    file.release();

    let child_process = match with_scheduler_mut(|scheduler| {
        scheduler.spawn_child_process(current, program, silent_stdio)
    }) {
        Ok(process_id) => process_id,
        Err(program) => {
            program.owned_pages.release();
            return Err(SpawnError::ResourceUnavailable);
        }
    };

    wait_for_child(child_process).ok_or(SpawnError::ResourceUnavailable)
}

/// Reads from the current thread's process-owned standard streams.
pub fn current_process_read(fd: usize, buffer: &mut [u8]) -> usize {
    with_current_process(|process| process.read(fd, buffer)).unwrap_or(0)
}

/// Writes to the current thread's process-owned standard streams.
pub fn current_process_write(fd: usize, buffer: &[u8]) -> usize {
    with_current_process(|process| process.write(fd, buffer)).unwrap_or(0)
}

/// Copies the current process cwd into a caller-provided buffer.
pub fn current_process_getcwd(buffer: &mut [u8]) -> Option<usize> {
    with_current_process(|process| process.copy_cwd_into(buffer)).flatten()
}

/// Looks up one environment variable in the current process and copies it into
/// a caller-provided buffer.
pub fn current_process_get_env(
    key: &str,
    buffer: &mut [u8],
) -> Result<usize, EnvironmentAccessError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let process = scheduler.process(process_id);
        let value = process.env(key).map_err(map_environment_error)?;
        let Some(value) = value else {
            return Err(EnvironmentAccessError::NotFound);
        };

        if buffer.len() < value.len() {
            return Err(EnvironmentAccessError::BufferTooSmall);
        }

        buffer[..value.len()].copy_from_slice(value.as_bytes());
        Ok(value.len())
    })
}

/// Serializes the current process environment into a caller-provided buffer.
pub fn current_process_list_env(buffer: &mut [u8]) -> Result<usize, EnvironmentAccessError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        scheduler
            .process(process_id)
            .list_env_into(buffer)
            .map_err(map_environment_error)
    })
}

/// Inserts or updates one environment variable on the current process.
pub fn current_process_set_env(key: &str, value: &str) -> Result<(), EnvironmentAccessError> {
    with_scheduler_mut(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        scheduler
            .process_mut(process_id)
            .set_env(key, value)
            .map_err(map_environment_error)
    })
}

/// Removes one environment variable from the current process.
pub fn current_process_unset_env(key: &str) -> Result<(), EnvironmentAccessError> {
    with_scheduler_mut(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        match scheduler
            .process_mut(process_id)
            .remove_env(key)
            .map_err(map_environment_error)?
        {
            true => Ok(()),
            false => Err(EnvironmentAccessError::NotFound),
        }
    })
}

/// Resolves and installs a new cwd for the current process.
pub fn current_process_chdir(path: &str) -> Result<(), storage::StorageError> {
    with_scheduler_mut(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(storage::StorageError::RootFsUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let normalized_path = NormalizedPath::resolve(scheduler.process(process_id).cwd(), path)?;
        let normalized_path = normalized_path.as_str()?;

        storage::ensure_root_dir(normalized_path)?;
        scheduler
            .process_mut(process_id)
            .set_cwd(normalized_path)
            .ok_or(storage::StorageError::InvalidPath)
    })
}

/// Reads one runtime path file from the current process' cwd context.
pub fn current_process_read_file(
    path: &str,
    buffer: &mut [u8],
) -> Result<usize, storage::StorageError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(storage::StorageError::RootFsUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let normalized_path = NormalizedPath::resolve(scheduler.process(process_id).cwd(), path)?;
        storage::read_root_file_into(normalized_path.as_str()?, buffer)
    })
}

/// Lists one runtime path directory from the current process' cwd context.
pub fn current_process_read_dir(
    path: &str,
    buffer: &mut [u8],
) -> Result<usize, storage::StorageError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(storage::StorageError::RootFsUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let normalized_path = NormalizedPath::resolve(scheduler.process(process_id).cwd(), path)?;
        storage::read_root_dir(normalized_path.as_str()?, buffer)
    })
}

fn with_current_process<F, T>(operation: F) -> Option<T>
where
    F: FnOnce(&Process) -> T,
{
    with_scheduler(|scheduler| {
        let current = scheduler.current_thread?;
        let process_id = scheduler.thread(current).process_id();
        Some(operation(scheduler.process(process_id)))
    })
}

pub(super) fn map_environment_error(error: EnvironmentError) -> EnvironmentAccessError {
    match error {
        EnvironmentError::InvalidKey => EnvironmentAccessError::InvalidKey,
        EnvironmentError::KeyTooLong => EnvironmentAccessError::KeyTooLong,
        EnvironmentError::ValueTooLong => EnvironmentAccessError::ValueTooLong,
        EnvironmentError::CapacityExceeded => EnvironmentAccessError::CapacityExceeded,
    }
}

fn map_storage_spawn_error(error: storage::StorageError) -> SpawnError {
    match error {
        storage::StorageError::InvalidPath
        | storage::StorageError::PathNotAbsolute
        | storage::StorageError::InvalidShortName => SpawnError::InvalidPath,
        storage::StorageError::FileNotFound | storage::StorageError::NotAFile => {
            SpawnError::FileNotFound
        }
        _ => SpawnError::ResourceUnavailable,
    }
}
