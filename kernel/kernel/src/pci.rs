//! Minimal PCI configuration-space access for bootstrap device discovery.
//!
//! The storage stack currently uses this module only to find and enable the
//! first AHCI controller. It intentionally stops at the conventional x86
//! configuration-space mechanism rather than growing a general PCI subsystem.

use core::arch::asm;

const CONFIG_ADDRESS_PORT: u16 = 0x0cf8;
const CONFIG_DATA_PORT: u16 = 0x0cfc;

const CLASS_MASS_STORAGE: u8 = 0x01;
const SUBCLASS_SATA: u8 = 0x06;
const PROG_IF_AHCI: u8 = 0x01;

const PCI_COMMAND_OFFSET: u8 = 0x04;
const PCI_CLASS_OFFSET: u8 = 0x08;
const PCI_HEADER_TYPE_OFFSET: u8 = 0x0c;
const PCI_BAR5_OFFSET: u8 = 0x24;

const PCI_COMMAND_MEMORY_SPACE: u16 = 1 << 1;
const PCI_COMMAND_BUS_MASTER: u16 = 1 << 2;

#[derive(Clone, Copy)]
/// Bus/device/function location of a discovered PCI function.
pub struct PciDeviceLocation {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
}

#[derive(Clone, Copy)]
/// AHCI controller metadata needed by the storage stack.
pub struct AhciControllerInfo {
    pub location: PciDeviceLocation,
    pub abar: u64,
}

/// Scans conventional PCI configuration space for the first AHCI controller.
pub fn find_ahci_controller() -> Option<AhciControllerInfo> {
    let mut bus = 0u16;
    while bus <= u8::MAX as u16 {
        let mut slot = 0u8;
        while slot < 32 {
            let location = PciDeviceLocation {
                bus: bus as u8,
                slot,
                function: 0,
            };

            let vendor = read_u16(location, 0x00);
            if vendor != 0xffff {
                let header_type = read_u8(location, PCI_HEADER_TYPE_OFFSET) & 0x80;
                let function_count = if header_type != 0 { 8 } else { 1 };

                let mut function = 0u8;
                while function < function_count {
                    let location = PciDeviceLocation {
                        bus: bus as u8,
                        slot,
                        function,
                    };

                    if read_u16(location, 0x00) == 0xffff {
                        function += 1;
                        continue;
                    }

                    let class_reg = read_u32(location, PCI_CLASS_OFFSET);
                    let class = ((class_reg >> 24) & 0xff) as u8;
                    let subclass = ((class_reg >> 16) & 0xff) as u8;
                    let prog_if = ((class_reg >> 8) & 0xff) as u8;

                    if class == CLASS_MASS_STORAGE
                        && subclass == SUBCLASS_SATA
                        && prog_if == PROG_IF_AHCI
                    {
                        let abar = read_u32(location, PCI_BAR5_OFFSET) as u64 & 0xffff_fff0;
                        if abar != 0 {
                            return Some(AhciControllerInfo { location, abar });
                        }
                    }

                    function += 1;
                }
            }

            slot += 1;
        }
        bus += 1;
    }

    None
}

/// Enables memory-space decoding and bus mastering for a discovered controller.
pub fn enable_memory_bus_mastering(location: PciDeviceLocation) {
    let command = read_u16(location, PCI_COMMAND_OFFSET);
    let updated = command | PCI_COMMAND_MEMORY_SPACE | PCI_COMMAND_BUS_MASTER;
    write_u16(location, PCI_COMMAND_OFFSET, updated);
}

fn read_u8(location: PciDeviceLocation, offset: u8) -> u8 {
    let shift = (offset & 0x03) * 8;
    ((read_u32(location, offset & !0x03) >> shift) & 0xff) as u8
}

fn read_u16(location: PciDeviceLocation, offset: u8) -> u16 {
    let shift = (offset & 0x02) * 8;
    ((read_u32(location, offset & !0x03) >> shift) & 0xffff) as u16
}

fn write_u16(location: PciDeviceLocation, offset: u8, value: u16) {
    let aligned = offset & !0x03;
    let shift = (offset & 0x02) * 8;
    let mut register = read_u32(location, aligned);
    register &= !(0xffff << shift);
    register |= (value as u32) << shift;
    write_u32(location, aligned, register);
}

fn read_u32(location: PciDeviceLocation, offset: u8) -> u32 {
    let address = pci_config_address(location, offset);
    unsafe {
        outl(CONFIG_ADDRESS_PORT, address);
        inl(CONFIG_DATA_PORT)
    }
}

fn write_u32(location: PciDeviceLocation, offset: u8, value: u32) {
    let address = pci_config_address(location, offset);
    unsafe {
        outl(CONFIG_ADDRESS_PORT, address);
        outl(CONFIG_DATA_PORT, value);
    }
}

fn pci_config_address(location: PciDeviceLocation, offset: u8) -> u32 {
    (1u32 << 31)
        | ((location.bus as u32) << 16)
        | ((location.slot as u32) << 11)
        | ((location.function as u32) << 8)
        | ((offset as u32) & 0xfc)
}

unsafe fn inl(port: u16) -> u32 {
    let value: u32;
    asm!(
        "in eax, dx",
        out("eax") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

unsafe fn outl(port: u16, value: u32) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        options(nomem, nostack, preserves_flags)
    );
}
