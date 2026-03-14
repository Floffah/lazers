use core::cell::UnsafeCell;

use crate::memory;
use crate::pci;
use lzutil::{align_down, align_up};

use super::ahci::{AhciController, BlockDevice, AHCI_MMIO_SIZE};
use super::fat32::Fat32;
use super::gpt::{GptPartitions, EFI_SYSTEM_PARTITION_GUID};
use super::StorageError;

static ROOT_FS: RootFsCell = RootFsCell::new();

#[derive(Clone, Copy)]
/// Mounted runtime root filesystem backed by the `LAZERS-SYSTEM` partition.
pub struct RootFs {
    fs: Fat32,
}

impl RootFs {
    /// Reads one absolute-path file from the mounted root filesystem into the
    /// provided caller-owned buffer.
    pub fn read_file_into(&self, path: &str, buffer: &mut [u8]) -> Result<usize, StorageError> {
        let file = self.fs.open_absolute(path)?;
        self.fs.read_file(&file, buffer)
    }

    /// Reads one absolute-path file from the mounted root filesystem into a
    /// kernel-owned buffer.
    pub fn read_file(&self, path: &str) -> Result<memory::KernelBuffer, StorageError> {
        let file = self.fs.open_absolute(path)?;
        let buffer =
            memory::allocate_kernel_buffer(file.size as usize).map_err(StorageError::Memory)?;
        let mut buffer = buffer;
        let bytes_read = self.fs.read_file(&file, buffer.as_mut_slice())?;
        debug_assert_eq!(bytes_read, buffer.len());
        Ok(buffer)
    }

    /// Lists one absolute-path directory into a caller-provided newline-delimited buffer.
    pub fn read_dir(&self, path: &str, buffer: &mut [u8]) -> Result<usize, StorageError> {
        self.fs.read_dir(path, buffer)
    }

    /// Validates that the given absolute path resolves to a directory.
    pub fn ensure_dir(&self, path: &str) -> Result<(), StorageError> {
        self.fs.ensure_dir(path)
    }
}

/// Discovers the first AHCI disk, reads GPT, and mounts the `LAZERS-SYSTEM`
/// partition as the runtime root filesystem.
pub fn init_root_fs() -> Result<(), StorageError> {
    let root_fs = mount_root_fs()?;
    with_root_fs_mut(|slot| {
        *slot = Some(root_fs);
    });
    Ok(())
}

/// Reads one absolute-path file from the mounted runtime root filesystem.
pub fn read_root_file(path: &str) -> Result<memory::KernelBuffer, StorageError> {
    with_root_fs(|root_fs| root_fs.read_file(path))
}

/// Reads one absolute-path file from the mounted runtime root filesystem into a
/// caller-owned buffer.
pub fn read_root_file_into(path: &str, buffer: &mut [u8]) -> Result<usize, StorageError> {
    with_root_fs(|root_fs| root_fs.read_file_into(path, buffer))
}

/// Lists one absolute-path directory from the mounted runtime root filesystem.
pub fn read_root_dir(path: &str, buffer: &mut [u8]) -> Result<usize, StorageError> {
    with_root_fs(|root_fs| root_fs.read_dir(path, buffer))
}

/// Validates that one absolute-path directory exists on the mounted runtime root filesystem.
pub fn ensure_root_dir(path: &str) -> Result<(), StorageError> {
    with_root_fs(|root_fs| root_fs.ensure_dir(path))
}

fn mount_root_fs() -> Result<RootFs, StorageError> {
    let controller_info =
        pci::find_ahci_controller().ok_or(StorageError::AhciControllerNotFound)?;
    pci::enable_memory_bus_mastering(controller_info.location);

    let abar_start = align_down(controller_info.abar, memory::PAGE_SIZE as u64);
    let abar_end = align_up(
        controller_info.abar + AHCI_MMIO_SIZE as u64,
        memory::PAGE_SIZE as u64,
    );
    memory::map_kernel_identity_range(abar_start, abar_end, true).map_err(StorageError::Memory)?;

    let controller = AhciController::initialize(controller_info.abar)?;
    let device = BlockDevice::new(controller);
    let partitions = GptPartitions::read(device)?;

    let _esp = partitions
        .find_by_type(EFI_SYSTEM_PARTITION_GUID)
        .ok_or(StorageError::MissingEspPartition)?;
    let system = partitions
        .find_by_name("LAZERS-SYSTEM")
        .ok_or(StorageError::MissingSystemPartition)?;

    let fs = Fat32::mount(device, system)?;
    Ok(RootFs { fs })
}

fn with_root_fs<F, T>(operation: F) -> Result<T, StorageError>
where
    F: FnOnce(RootFs) -> Result<T, StorageError>,
{
    let root_fs = unsafe { *ROOT_FS.get() }.ok_or(StorageError::RootFsUnavailable)?;
    operation(root_fs)
}

fn with_root_fs_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut Option<RootFs>) -> T,
{
    unsafe { operation(ROOT_FS.get()) }
}

struct RootFsCell {
    root_fs: UnsafeCell<Option<RootFs>>,
}

impl RootFsCell {
    const fn new() -> Self {
        Self {
            root_fs: UnsafeCell::new(None),
        }
    }

    unsafe fn get(&self) -> &mut Option<RootFs> {
        &mut *self.root_fs.get()
    }
}

unsafe impl Sync for RootFsCell {}
