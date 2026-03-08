//! Bootstrap storage stack for disk-backed user program loading.
//!
//! This module owns the first end-to-end path from a real block device to a
//! runnable user ELF: PCI discovery of the AHCI controller, read-only SATA
//! sector access, GPT partition discovery, a narrow FAT32 reader, and a small
//! root-filesystem interface used by the kernel bootstrap code.

use core::mem::size_of;
use core::cell::UnsafeCell;
use core::ptr::{copy_nonoverlapping, read_volatile, write_volatile};

use crate::memory::{self, MemoryError};
use crate::pci;

const SECTOR_SIZE: usize = 512;
const AHCI_MMIO_SIZE: usize = 0x1100;
const GPT_HEADER_LBA: u64 = 1;
const GPT_HEADER_SIGNATURE: [u8; 8] = *b"EFI PART";
const EFI_SYSTEM_PARTITION_GUID: [u8; 16] =
    [0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b];

const AHCI_GHC_AE: u32 = 1 << 31;
const AHCI_PORT_CMD_ST: u32 = 1 << 0;
const AHCI_PORT_CMD_FRE: u32 = 1 << 4;
const AHCI_PORT_CMD_FR: u32 = 1 << 14;
const AHCI_PORT_CMD_CR: u32 = 1 << 15;
const AHCI_PORT_TFD_BSY: u32 = 1 << 7;
const AHCI_PORT_TFD_DRQ: u32 = 1 << 3;
const AHCI_PORT_IS_TFES: u32 = 1 << 30;
const SATA_STATUS_DEVICE_PRESENT: u32 = 0x3;
const SATA_STATUS_INTERFACE_ACTIVE: u32 = 0x1;
const SATA_SIGNATURE_ATA: u32 = 0x0000_0101;

const ATA_COMMAND_READ_DMA_EXT: u8 = 0x25;
const FIS_TYPE_REG_H2D: u8 = 0x27;

const FAT_DIRECTORY_ENTRY_SIZE: usize = 32;
const FAT_ATTRIBUTE_DIRECTORY: u8 = 1 << 4;
const FAT_ATTRIBUTE_VOLUME_ID: u8 = 1 << 3;
const FAT_ATTRIBUTE_LONG_NAME: u8 = 0x0f;
const FAT_ENTRY_END_OF_CHAIN: u32 = 0x0fff_fff8;
const FAT_ENTRY_BAD_CLUSTER: u32 = 0x0fff_fff7;

static ROOT_FS: RootFsCell = RootFsCell::new();

#[derive(Clone, Copy)]
/// Mounted runtime root filesystem backed by the `LAZERS-SYSTEM` partition.
pub struct RootFs {
    fs: Fat32,
}

impl RootFs {
    /// Reads one absolute-path file from the mounted root filesystem into a
    /// kernel-owned buffer.
    pub fn read_file(&self, path: &str) -> Result<memory::KernelBuffer, StorageError> {
        let file = self.fs.open_absolute(path)?;
        let buffer = memory::allocate_kernel_buffer(file.size as usize).map_err(StorageError::Memory)?;
        let mut buffer = buffer;
        let bytes_read = self.fs.read_file(&file, buffer.as_mut_slice())?;
        debug_assert_eq!(bytes_read, buffer.len());
        Ok(buffer)
    }
}

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
    PathNotAbsolute,
    InvalidShortName,
    FileNotFound,
    NotAFile,
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
            Self::InvalidFat32BootSector => "the system partition does not contain a supported FAT32 filesystem",
            Self::PathNotAbsolute => "the requested path is not absolute",
            Self::InvalidShortName => "the requested path component is not a supported FAT short name",
            Self::FileNotFound => "the requested file was not found",
            Self::NotAFile => "the requested path does not name a regular file",
            Self::BufferTooSmall => "the destination buffer is too small",
            Self::RootFsUnavailable => "the runtime root filesystem is not mounted",
        }
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

fn mount_root_fs() -> Result<RootFs, StorageError> {
    let controller_info = pci::find_ahci_controller().ok_or(StorageError::AhciControllerNotFound)?;
    pci::enable_memory_bus_mastering(controller_info.location);

    let abar_start = align_down(controller_info.abar, memory::PAGE_SIZE as u64);
    let abar_end = align_up(controller_info.abar + AHCI_MMIO_SIZE as u64, memory::PAGE_SIZE as u64);
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

#[derive(Clone, Copy)]
struct BlockDevice {
    controller: AhciController,
}

impl BlockDevice {
    const fn new(controller: AhciController) -> Self {
        Self { controller }
    }

    fn read_sector(&self, lba: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), StorageError> {
        self.controller.read_sector(lba, buffer)
    }
}

#[derive(Clone, Copy)]
struct AhciController {
    abar: u64,
    port_index: usize,
    command_list_paddr: u64,
    fis_paddr: u64,
    command_table_paddr: u64,
    dma_buffer_paddr: u64,
}

impl AhciController {
    fn initialize(abar: u64) -> Result<Self, StorageError> {
        let controller = Self {
            abar,
            port_index: 0,
            command_list_paddr: memory::allocate_kernel_page().map_err(StorageError::Memory)?,
            fis_paddr: memory::allocate_kernel_page().map_err(StorageError::Memory)?,
            command_table_paddr: memory::allocate_kernel_page().map_err(StorageError::Memory)?,
            dma_buffer_paddr: memory::allocate_kernel_page().map_err(StorageError::Memory)?,
        };

        controller.enable_ahci_mode();

        let ports_implemented = controller.read_hba_reg(|hba| &hba.ports_implemented);
        let mut port_index = 0usize;
        while port_index < 32 {
            if (ports_implemented & (1 << port_index)) != 0 && controller.port_is_sata_disk(port_index) {
                let controller = Self { port_index, ..controller };
                controller.initialize_port()?;
                return Ok(controller);
            }
            port_index += 1;
        }

        Err(StorageError::AhciPortNotFound)
    }

    fn read_sector(&self, lba: u64, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), StorageError> {
        self.wait_for_device_ready()?;
        self.write_port_reg(|port| &mut port.interrupt_status, u32::MAX);

        let header = self.command_header_mut();
        *header = HbaCommandHeader::default();
        header.command_fis_length = (size_of::<FisRegisterHostToDevice>() / 4) as u8;
        header.prdt_length = 1;
        header.command_table_base = self.command_table_paddr as u32;
        header.command_table_base_upper = (self.command_table_paddr >> 32) as u32;

        let table = self.command_table_mut();
        *table = HbaCommandTable::default();
        table.prdt_entry.data_base = self.dma_buffer_paddr as u32;
        table.prdt_entry.data_base_upper = (self.dma_buffer_paddr >> 32) as u32;
        table.prdt_entry.byte_count = (SECTOR_SIZE as u32) - 1;

        let fis = FisRegisterHostToDevice {
            fis_type: FIS_TYPE_REG_H2D,
            port_multiplier: 1 << 7,
            command: ATA_COMMAND_READ_DMA_EXT,
            feature_low: 0,
            lba0: (lba & 0xff) as u8,
            lba1: ((lba >> 8) & 0xff) as u8,
            lba2: ((lba >> 16) & 0xff) as u8,
            device: 1 << 6,
            lba3: ((lba >> 24) & 0xff) as u8,
            lba4: ((lba >> 32) & 0xff) as u8,
            lba5: ((lba >> 40) & 0xff) as u8,
            feature_high: 0,
            count_low: 1,
            count_high: 0,
            icc: 0,
            control: 0,
            reserved: [0; 4],
        };
        unsafe {
            copy_nonoverlapping(
                &fis as *const FisRegisterHostToDevice as *const u8,
                table.command_fis.as_mut_ptr(),
                size_of::<FisRegisterHostToDevice>(),
            );
        }

        self.write_port_reg(|port| &mut port.command_issue, 1);

        let mut spins = 0usize;
        loop {
            let interrupt_status = self.read_port_reg(|port| &port.interrupt_status);
            if (interrupt_status & AHCI_PORT_IS_TFES) != 0 {
                return Err(StorageError::AhciTaskFileError);
            }

            let command_issue = self.read_port_reg(|port| &port.command_issue);
            if (command_issue & 1) == 0 {
                break;
            }

            spins += 1;
            if spins > 1_000_000 {
                return Err(StorageError::AhciCommandTimeout);
            }
        }

        unsafe {
            copy_nonoverlapping(self.dma_buffer_paddr as *const u8, buffer.as_mut_ptr(), SECTOR_SIZE);
        }
        Ok(())
    }

    fn initialize_port(&self) -> Result<(), StorageError> {
        self.stop_port()?;

        self.write_port_reg(|port| &mut port.command_list_base, self.command_list_paddr as u32);
        self.write_port_reg(
            |port| &mut port.command_list_base_upper,
            (self.command_list_paddr >> 32) as u32,
        );
        self.write_port_reg(|port| &mut port.fis_base, self.fis_paddr as u32);
        self.write_port_reg(|port| &mut port.fis_base_upper, (self.fis_paddr >> 32) as u32);
        self.write_port_reg(|port| &mut port.interrupt_enable, 0);
        self.write_port_reg(|port| &mut port.serial_ata_error, u32::MAX);
        self.write_port_reg(|port| &mut port.interrupt_status, u32::MAX);

        self.start_port();
        Ok(())
    }

    fn enable_ahci_mode(&self) {
        let ghc = self.read_hba_reg(|hba| &hba.global_host_control);
        self.write_hba_reg(|hba| &mut hba.global_host_control, ghc | AHCI_GHC_AE);
    }

    fn port_is_sata_disk(&self, port_index: usize) -> bool {
        let sata_status = self.read_port_reg_at(port_index, |port| &port.serial_ata_status);
        let device_detection = sata_status & 0x0f;
        let interface_power = (sata_status >> 8) & 0x0f;
        let signature = self.read_port_reg_at(port_index, |port| &port.signature);

        device_detection == SATA_STATUS_DEVICE_PRESENT
            && interface_power == SATA_STATUS_INTERFACE_ACTIVE
            && signature == SATA_SIGNATURE_ATA
    }

    fn stop_port(&self) -> Result<(), StorageError> {
        let command = self.read_port_reg(|port| &port.command_status);
        self.write_port_reg(
            |port| &mut port.command_status,
            command & !(AHCI_PORT_CMD_ST | AHCI_PORT_CMD_FRE),
        );

        let mut spins = 0usize;
        loop {
            let command = self.read_port_reg(|port| &port.command_status);
            if (command & (AHCI_PORT_CMD_CR | AHCI_PORT_CMD_FR)) == 0 {
                return Ok(());
            }

            spins += 1;
            if spins > 1_000_000 {
                return Err(StorageError::AhciCommandTimeout);
            }
        }
    }

    fn start_port(&self) {
        let command = self.read_port_reg(|port| &port.command_status);
        self.write_port_reg(
            |port| &mut port.command_status,
            command | AHCI_PORT_CMD_FRE | AHCI_PORT_CMD_ST,
        );
    }

    fn wait_for_device_ready(&self) -> Result<(), StorageError> {
        let mut spins = 0usize;
        loop {
            let task_file = self.read_port_reg(|port| &port.task_file_data);
            if (task_file & (AHCI_PORT_TFD_BSY | AHCI_PORT_TFD_DRQ)) == 0 {
                return Ok(());
            }

            spins += 1;
            if spins > 1_000_000 {
                return Err(StorageError::AhciCommandTimeout);
            }
        }
    }

    fn command_header_mut(&self) -> &'static mut HbaCommandHeader {
        unsafe { &mut *(self.command_list_paddr as *mut HbaCommandHeader) }
    }

    fn command_table_mut(&self) -> &'static mut HbaCommandTable {
        unsafe { &mut *(self.command_table_paddr as *mut HbaCommandTable) }
    }

    fn read_hba_reg(&self, field: impl FnOnce(&HbaMemory) -> &u32) -> u32 {
        let hba = unsafe { &*(self.abar as *const HbaMemory) };
        unsafe { read_volatile(field(hba)) }
    }

    fn write_hba_reg(&self, field: impl FnOnce(&mut HbaMemory) -> &mut u32, value: u32) {
        let hba = unsafe { &mut *(self.abar as *mut HbaMemory) };
        unsafe { write_volatile(field(hba), value) }
    }

    fn read_port_reg(&self, field: impl FnOnce(&HbaPort) -> &u32) -> u32 {
        self.read_port_reg_at(self.port_index, field)
    }

    fn read_port_reg_at(&self, port_index: usize, field: impl FnOnce(&HbaPort) -> &u32) -> u32 {
        let port = unsafe { &*self.port_ptr(port_index) };
        unsafe { read_volatile(field(port)) }
    }

    fn write_port_reg(&self, field: impl FnOnce(&mut HbaPort) -> &mut u32, value: u32) {
        let port = unsafe { &mut *self.port_ptr(self.port_index) };
        unsafe { write_volatile(field(port), value) }
    }

    fn port_ptr(&self, port_index: usize) -> *mut HbaPort {
        unsafe { &mut (*(self.abar as *mut HbaMemory)).ports[port_index] as *mut HbaPort }
    }
}

#[derive(Clone, Copy)]
struct GptPartitions {
    partitions: [Option<GptPartition>; 16],
}

impl GptPartitions {
    fn read(device: BlockDevice) -> Result<Self, StorageError> {
        let mut header = [0u8; SECTOR_SIZE];
        device.read_sector(GPT_HEADER_LBA, &mut header)?;
        if header[0..8] != GPT_HEADER_SIGNATURE {
            return Err(StorageError::InvalidGptHeader);
        }

        let entry_lba = read_le_u64(&header, 72);
        let entry_count = read_le_u32(&header, 80) as usize;
        let entry_size = read_le_u32(&header, 84) as usize;
        if entry_size < 128 {
            return Err(StorageError::InvalidPartitionTable);
        }

        let mut partitions = [None; 16];
        let mut found = 0usize;
        let table_bytes = entry_count
            .checked_mul(entry_size)
            .ok_or(StorageError::InvalidPartitionTable)?;
        let table_sectors = align_up(table_bytes as u64, SECTOR_SIZE as u64) / SECTOR_SIZE as u64;

        let mut sector = [0u8; SECTOR_SIZE];
        let mut sector_index = 0u64;
        while sector_index < table_sectors {
            device.read_sector(entry_lba + sector_index, &mut sector)?;
            let mut offset = 0usize;
            while offset + entry_size <= SECTOR_SIZE {
                if found >= partitions.len() {
                    break;
                }

                let entry = &sector[offset..offset + entry_size];
                if entry[0..16].iter().any(|byte| *byte != 0) {
                    let start_lba = read_le_u64(entry, 32);
                    let end_lba = read_le_u64(entry, 40);
                    if end_lba < start_lba {
                        return Err(StorageError::InvalidPartitionTable);
                    }

                    partitions[found] = Some(GptPartition {
                        type_guid: entry[0..16].try_into().unwrap(),
                        start_lba,
                        sector_count: (end_lba - start_lba) + 1,
                        name: read_partition_name(entry),
                    });
                    found += 1;
                }

                offset += entry_size;
            }
            sector_index += 1;
        }

        Ok(Self { partitions })
    }

    fn find_by_type(&self, type_guid: [u8; 16]) -> Option<GptPartition> {
        self.partitions
            .iter()
            .flatten()
            .copied()
            .find(|partition| partition.type_guid == type_guid)
    }

    fn find_by_name(&self, name: &str) -> Option<GptPartition> {
        self.partitions
            .iter()
            .flatten()
            .copied()
            .find(|partition| partition.name_matches(name))
    }
}

#[derive(Clone, Copy)]
struct GptPartition {
    type_guid: [u8; 16],
    start_lba: u64,
    sector_count: u64,
    name: [u16; 36],
}

impl GptPartition {
    fn name_matches(&self, name: &str) -> bool {
        let mut expected = [0u16; 36];
        let mut expected_len = 0usize;
        for byte in name.bytes() {
            if expected_len >= expected.len() {
                return false;
            }
            expected[expected_len] = byte as u16;
            expected_len += 1;
        }

        let mut index = 0usize;
        while index < expected_len {
            if self.name[index] != expected[index] {
                return false;
            }
            index += 1;
        }

        while index < self.name.len() {
            if self.name[index] != 0 {
                return false;
            }
            index += 1;
        }

        true
    }
}

#[derive(Clone, Copy)]
struct Fat32 {
    device: BlockDevice,
    partition_start_lba: u64,
    partition_sector_count: u64,
    sectors_per_cluster: u8,
    reserved_sector_count: u16,
    root_cluster: u32,
    first_data_sector: u64,
}

impl Fat32 {
    fn mount(device: BlockDevice, partition: GptPartition) -> Result<Self, StorageError> {
        let mut sector = [0u8; SECTOR_SIZE];
        device.read_sector(partition.start_lba, &mut sector)?;

        let bytes_per_sector = read_le_u16(&sector, 11);
        let sectors_per_cluster = sector[13];
        let reserved_sector_count = read_le_u16(&sector, 14);
        let fat_count = sector[16];
        let root_entry_count = read_le_u16(&sector, 17);
        let total_sectors_16 = read_le_u16(&sector, 19);
        let fat_size_16 = read_le_u16(&sector, 22);
        let total_sectors_32 = read_le_u32(&sector, 32);
        let sectors_per_fat = read_le_u32(&sector, 36);
        let root_cluster = read_le_u32(&sector, 44);

        if bytes_per_sector != SECTOR_SIZE as u16
            || sectors_per_cluster == 0
            || reserved_sector_count == 0
            || fat_count == 0
            || root_entry_count != 0
            || fat_size_16 != 0
            || sectors_per_fat == 0
        {
            return Err(StorageError::InvalidFat32BootSector);
        }

        let total_sectors = if total_sectors_16 != 0 {
            total_sectors_16 as u64
        } else {
            total_sectors_32 as u64
        };
        if total_sectors == 0 || total_sectors > partition.sector_count {
            return Err(StorageError::InvalidFat32BootSector);
        }

        let first_data_sector = partition.start_lba
            + reserved_sector_count as u64
            + (fat_count as u64 * sectors_per_fat as u64);

        Ok(Self {
            device,
            partition_start_lba: partition.start_lba,
            partition_sector_count: partition.sector_count,
            sectors_per_cluster,
            reserved_sector_count,
            root_cluster,
            first_data_sector,
        })
    }

    fn open_absolute(&self, path: &str) -> Result<FatDirectoryEntry, StorageError> {
        if !path.starts_with('/') {
            return Err(StorageError::PathNotAbsolute);
        }

        let mut current_cluster = self.root_cluster;
        let mut components = path.split('/').filter(|component| !component.is_empty()).peekable();
        let Some(_) = components.peek() else {
            return Err(StorageError::NotAFile);
        };

        while let Some(component) = components.next() {
            let entry = self.find_in_directory(current_cluster, component)?;
            if components.peek().is_none() {
                if entry.is_directory {
                    return Err(StorageError::NotAFile);
                }
                return Ok(entry);
            }

            if !entry.is_directory {
                return Err(StorageError::FileNotFound);
            }

            current_cluster = entry.first_cluster;
        }

        Err(StorageError::FileNotFound)
    }

    fn read_file(&self, entry: &FatDirectoryEntry, buffer: &mut [u8]) -> Result<usize, StorageError> {
        if buffer.len() < entry.size as usize {
            return Err(StorageError::BufferTooSmall);
        }

        let mut remaining = entry.size as usize;
        let mut written = 0usize;
        let mut cluster = entry.first_cluster;
        let mut sector = [0u8; SECTOR_SIZE];

        while remaining > 0 {
            let cluster_lba = self.cluster_to_lba(cluster)?;
            let mut sector_index = 0u8;
            while sector_index < self.sectors_per_cluster && remaining > 0 {
                self.device
                    .read_sector(cluster_lba + sector_index as u64, &mut sector)?;
                let bytes = remaining.min(SECTOR_SIZE);
                buffer[written..written + bytes].copy_from_slice(&sector[..bytes]);
                written += bytes;
                remaining -= bytes;
                sector_index += 1;
            }

            if remaining == 0 {
                break;
            }

            cluster = self.next_cluster(cluster)?;
        }

        Ok(written)
    }

    fn find_in_directory(&self, start_cluster: u32, component: &str) -> Result<FatDirectoryEntry, StorageError> {
        let short_name = make_short_name(component)?;
        let mut cluster = start_cluster;
        let mut sector = [0u8; SECTOR_SIZE];

        loop {
            let cluster_lba = self.cluster_to_lba(cluster)?;
            let mut sector_index = 0u8;
            while sector_index < self.sectors_per_cluster {
                self.device
                    .read_sector(cluster_lba + sector_index as u64, &mut sector)?;

                let mut entry_offset = 0usize;
                while entry_offset + FAT_DIRECTORY_ENTRY_SIZE <= SECTOR_SIZE {
                    let entry = &sector[entry_offset..entry_offset + FAT_DIRECTORY_ENTRY_SIZE];
                    let first_byte = entry[0];
                    if first_byte == 0x00 {
                        return Err(StorageError::FileNotFound);
                    }
                    if first_byte == 0xe5 {
                        entry_offset += FAT_DIRECTORY_ENTRY_SIZE;
                        continue;
                    }

                    let attributes = entry[11];
                    if attributes == FAT_ATTRIBUTE_LONG_NAME || (attributes & FAT_ATTRIBUTE_VOLUME_ID) != 0 {
                        entry_offset += FAT_DIRECTORY_ENTRY_SIZE;
                        continue;
                    }

                    if entry[0..11] == short_name {
                        let first_cluster =
                            ((read_le_u16(entry, 20) as u32) << 16) | read_le_u16(entry, 26) as u32;
                        return Ok(FatDirectoryEntry {
                            first_cluster,
                            size: read_le_u32(entry, 28),
                            is_directory: (attributes & FAT_ATTRIBUTE_DIRECTORY) != 0,
                        });
                    }

                    entry_offset += FAT_DIRECTORY_ENTRY_SIZE;
                }

                sector_index += 1;
            }

            cluster = self.next_cluster(cluster)?;
        }
    }

    fn cluster_to_lba(&self, cluster: u32) -> Result<u64, StorageError> {
        if cluster < 2 {
            return Err(StorageError::InvalidFat32BootSector);
        }

        let offset = (cluster as u64 - 2) * self.sectors_per_cluster as u64;
        let lba = self.first_data_sector + offset;
        if lba >= self.partition_start_lba + self.partition_sector_count {
            return Err(StorageError::InvalidFat32BootSector);
        }

        Ok(lba)
    }

    fn next_cluster(&self, cluster: u32) -> Result<u32, StorageError> {
        let fat_offset = cluster as u64 * 4;
        let fat_sector = self.partition_start_lba
            + self.reserved_sector_count as u64
            + (fat_offset / SECTOR_SIZE as u64);
        let sector_offset = (fat_offset % SECTOR_SIZE as u64) as usize;
        let mut sector = [0u8; SECTOR_SIZE];
        self.device.read_sector(fat_sector, &mut sector)?;

        let entry = read_le_u32(&sector, sector_offset) & 0x0fff_ffff;
        if entry == FAT_ENTRY_BAD_CLUSTER || entry < 2 {
            return Err(StorageError::InvalidFat32BootSector);
        }
        if entry >= FAT_ENTRY_END_OF_CHAIN {
            return Err(StorageError::FileNotFound);
        }
        Ok(entry)
    }
}

#[derive(Clone, Copy)]
struct FatDirectoryEntry {
    first_cluster: u32,
    size: u32,
    is_directory: bool,
}

#[repr(C)]
struct HbaMemory {
    capabilities: u32,
    global_host_control: u32,
    interrupt_status: u32,
    ports_implemented: u32,
    version: u32,
    command_completion_coalescing_control: u32,
    command_completion_coalescing_ports: u32,
    enclosure_management_location: u32,
    enclosure_management_control: u32,
    capabilities_extended: u32,
    bios_handoff_control_status: u32,
    reserved: [u8; 0x74],
    vendor: [u8; 0x60],
    ports: [HbaPort; 32],
}

#[repr(C)]
struct HbaPort {
    command_list_base: u32,
    command_list_base_upper: u32,
    fis_base: u32,
    fis_base_upper: u32,
    interrupt_status: u32,
    interrupt_enable: u32,
    command_status: u32,
    reserved0: u32,
    task_file_data: u32,
    signature: u32,
    serial_ata_status: u32,
    serial_ata_control: u32,
    serial_ata_error: u32,
    serial_ata_active: u32,
    command_issue: u32,
    serial_ata_notification: u32,
    fis_based_switching: u32,
    device_sleep: u32,
    reserved1: [u8; 0x28],
}

#[repr(C)]
#[derive(Default)]
struct HbaCommandHeader {
    command_fis_length: u8,
    flags: u8,
    prdt_length: u16,
    bytes_transferred: u32,
    command_table_base: u32,
    command_table_base_upper: u32,
    reserved: [u32; 4],
}

#[repr(C)]
struct HbaCommandTable {
    command_fis: [u8; 64],
    atapi_command: [u8; 16],
    reserved: [u8; 48],
    prdt_entry: HbaPrdtEntry,
}

#[repr(C)]
#[derive(Default)]
struct HbaPrdtEntry {
    data_base: u32,
    data_base_upper: u32,
    reserved: u32,
    byte_count: u32,
}

#[repr(C, packed)]
#[derive(Default)]
struct FisRegisterHostToDevice {
    fis_type: u8,
    port_multiplier: u8,
    command: u8,
    feature_low: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    feature_high: u8,
    count_low: u8,
    count_high: u8,
    icc: u8,
    control: u8,
    reserved: [u8; 4],
}

impl Default for HbaCommandTable {
    fn default() -> Self {
        Self {
            command_fis: [0; 64],
            atapi_command: [0; 16],
            reserved: [0; 48],
            prdt_entry: HbaPrdtEntry::default(),
        }
    }
}

fn make_short_name(component: &str) -> Result<[u8; 11], StorageError> {
    let mut short_name = [b' '; 11];
    let mut parts = component.split('.');
    let name = parts.next().ok_or(StorageError::InvalidShortName)?;
    let extension = parts.next();
    if parts.next().is_some() || name.is_empty() || name.len() > 8 {
        return Err(StorageError::InvalidShortName);
    }

    for (index, byte) in name.bytes().enumerate() {
        short_name[index] = normalize_fat_byte(byte)?;
    }

    if let Some(extension) = extension {
        if extension.is_empty() || extension.len() > 3 {
            return Err(StorageError::InvalidShortName);
        }

        for (index, byte) in extension.bytes().enumerate() {
            short_name[8 + index] = normalize_fat_byte(byte)?;
        }
    }

    Ok(short_name)
}

fn normalize_fat_byte(byte: u8) -> Result<u8, StorageError> {
    if byte.is_ascii_alphanumeric() {
        Ok(byte.to_ascii_uppercase())
    } else {
        match byte {
            b'$' | b'%' | b'\'' | b'-' | b'_' | b'@' | b'~' | b'`' | b'!' | b'(' | b')' | b'{' | b'}' | b'^' | b'#' | b'&' => {
                Ok(byte)
            }
            _ => Err(StorageError::InvalidShortName),
        }
    }
}

fn read_partition_name(entry: &[u8]) -> [u16; 36] {
    let mut name = [0u16; 36];
    let mut index = 0usize;
    while index < name.len() {
        let offset = 56 + (index * 2);
        name[index] = read_le_u16(entry, offset);
        index += 1;
    }
    name
}

fn read_le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_le_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

const fn align_up(value: u64, align: u64) -> u64 {
    if value == 0 {
        0
    } else {
        (value + align - 1) & !(align - 1)
    }
}
