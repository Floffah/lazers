use core::mem::size_of;
use core::ptr::{copy_nonoverlapping, read_volatile, write_volatile};

use crate::memory;

use super::util::SECTOR_SIZE;
use super::StorageError;

pub(super) const AHCI_MMIO_SIZE: usize = 0x1100;

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

#[derive(Clone, Copy)]
pub(super) struct BlockDevice {
    controller: AhciController,
}

impl BlockDevice {
    pub(super) const fn new(controller: AhciController) -> Self {
        Self { controller }
    }

    pub(super) fn read_sector(
        &self,
        lba: u64,
        buffer: &mut [u8; SECTOR_SIZE],
    ) -> Result<(), StorageError> {
        self.controller.read_sector(lba, buffer)
    }
}

#[derive(Clone, Copy)]
pub(super) struct AhciController {
    abar: u64,
    port_index: usize,
    command_list_paddr: u64,
    fis_paddr: u64,
    command_table_paddr: u64,
    dma_buffer_paddr: u64,
}

impl AhciController {
    pub(super) fn initialize(abar: u64) -> Result<Self, StorageError> {
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
            if (ports_implemented & (1 << port_index)) != 0
                && controller.port_is_sata_disk(port_index)
            {
                let controller = Self {
                    port_index,
                    ..controller
                };
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
            copy_nonoverlapping(
                self.dma_buffer_paddr as *const u8,
                buffer.as_mut_ptr(),
                SECTOR_SIZE,
            );
        }
        Ok(())
    }

    fn initialize_port(&self) -> Result<(), StorageError> {
        self.stop_port()?;

        self.write_port_reg(
            |port| &mut port.command_list_base,
            self.command_list_paddr as u32,
        );
        self.write_port_reg(
            |port| &mut port.command_list_base_upper,
            (self.command_list_paddr >> 32) as u32,
        );
        self.write_port_reg(|port| &mut port.fis_base, self.fis_paddr as u32);
        self.write_port_reg(
            |port| &mut port.fis_base_upper,
            (self.fis_paddr >> 32) as u32,
        );
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
