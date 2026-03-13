//! Bootstrap storage stack for disk-backed user program loading.
//!
//! This module owns the first end-to-end path from a real block device to a
//! runnable user ELF: PCI discovery of the AHCI controller, read-only SATA
//! sector access, GPT partition discovery, a narrow FAT32 reader, and a small
//! root-filesystem interface used by the kernel bootstrap code.

#[cfg(not(test))]
mod ahci;
#[cfg(test)]
mod ahci {
    use super::util::SECTOR_SIZE;
    use super::StorageError;

    pub(super) const AHCI_MMIO_SIZE: usize = 0x1100;

    #[derive(Clone, Copy)]
    pub(super) struct BlockDevice;

    impl BlockDevice {
        pub(super) fn read_sector(
            &self,
            _lba: u64,
            _buffer: &mut [u8; SECTOR_SIZE],
        ) -> Result<(), StorageError> {
            Err(StorageError::RootFsUnavailable)
        }
    }

    #[derive(Clone, Copy)]
    pub(super) struct AhciController;

    impl AhciController {
        pub(super) fn initialize(_abar: u64) -> Result<Self, StorageError> {
            Err(StorageError::RootFsUnavailable)
        }
    }
}
mod fat32;
mod gpt;
mod path;
#[cfg(not(test))]
mod rootfs;
mod util;

use crate::memory::MemoryError;

pub use path::normalize_path;
#[cfg(not(test))]
pub use rootfs::{
    ensure_root_dir, init_root_fs, read_root_dir, read_root_file, read_root_file_into, RootFs,
};
#[cfg(test)]
pub use test_rootfs::{
    ensure_root_dir, init_root_fs, read_root_dir, read_root_file, read_root_file_into, RootFs,
};

#[derive(Clone, Copy, Debug)]
/// Failures that can occur while discovering and reading the runtime root
/// filesystem.
pub enum StorageError {
    Memory(MemoryError),
    AhciControllerNotFound,
    AhciPortNotFound,
    AhciCommandTimeout,
    AhciTaskFileError,
    InvalidGptHeader,
    InvalidPartitionTable,
    MissingEspPartition,
    MissingSystemPartition,
    InvalidFat32BootSector,
    InvalidPath,
    PathNotAbsolute,
    InvalidShortName,
    FileNotFound,
    NotAFile,
    NotADirectory,
    BufferTooSmall,
    RootFsUnavailable,
}

impl StorageError {
    /// Returns a short static description suitable for boot-time diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Memory(error) => error.as_str(),
            Self::AhciControllerNotFound => "no AHCI controller was found",
            Self::AhciPortNotFound => "no usable SATA disk was found on the AHCI controller",
            Self::AhciCommandTimeout => "an AHCI command timed out",
            Self::AhciTaskFileError => "the AHCI controller reported a task-file error",
            Self::InvalidGptHeader => "the GPT header is invalid",
            Self::InvalidPartitionTable => "the GPT partition table is invalid",
            Self::MissingEspPartition => "the EFI system partition is missing",
            Self::MissingSystemPartition => "the LAZERS-SYSTEM partition is missing",
            Self::InvalidFat32BootSector => {
                "the system partition does not contain a supported FAT32 filesystem"
            }
            Self::InvalidPath => "the requested path is invalid",
            Self::PathNotAbsolute => "the requested path is not absolute",
            Self::InvalidShortName => {
                "the requested path component is not a supported FAT short name"
            }
            Self::FileNotFound => "the requested file was not found",
            Self::NotAFile => "the requested path does not name a regular file",
            Self::NotADirectory => "the requested path does not name a directory",
            Self::BufferTooSmall => "the destination buffer is too small",
            Self::RootFsUnavailable => "the runtime root filesystem is not mounted",
        }
    }
}

#[cfg(test)]
mod test_rootfs {
    use crate::memory;

    use super::StorageError;

    #[derive(Clone, Copy)]
    pub struct RootFs;

    impl RootFs {
        pub fn read_file_into(
            &self,
            _path: &str,
            _buffer: &mut [u8],
        ) -> Result<usize, StorageError> {
            Err(StorageError::RootFsUnavailable)
        }

        pub fn read_file(&self, _path: &str) -> Result<memory::KernelBuffer, StorageError> {
            Err(StorageError::RootFsUnavailable)
        }

        pub fn read_dir(&self, _path: &str, _buffer: &mut [u8]) -> Result<usize, StorageError> {
            Err(StorageError::RootFsUnavailable)
        }

        pub fn ensure_dir(&self, _path: &str) -> Result<(), StorageError> {
            Err(StorageError::RootFsUnavailable)
        }
    }

    pub fn init_root_fs() -> Result<(), StorageError> {
        Err(StorageError::RootFsUnavailable)
    }

    pub fn read_root_file(_path: &str) -> Result<memory::KernelBuffer, StorageError> {
        Err(StorageError::RootFsUnavailable)
    }

    pub fn read_root_file_into(_path: &str, _buffer: &mut [u8]) -> Result<usize, StorageError> {
        Err(StorageError::RootFsUnavailable)
    }

    pub fn read_root_dir(_path: &str, _buffer: &mut [u8]) -> Result<usize, StorageError> {
        Err(StorageError::RootFsUnavailable)
    }

    pub fn ensure_root_dir(_path: &str) -> Result<(), StorageError> {
        Err(StorageError::RootFsUnavailable)
    }
}
