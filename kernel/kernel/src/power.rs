//! Generic kernel power control with a first ACPI soft-off backend.

use core::cell::UnsafeCell;
#[cfg(not(test))]
use core::slice;

#[cfg(not(test))]
use crate::memory;
#[cfg(not(test))]
use crate::port_io;

const RSDP_V1_LENGTH: usize = 20;
const RSDP_V2_LENGTH: usize = 36;
const SDT_HEADER_LENGTH: usize = 36;
const FADT_SIGNATURE: &[u8; 4] = b"FACP";
const RSDT_SIGNATURE: &[u8; 4] = b"RSDT";
const XSDT_SIGNATURE: &[u8; 4] = b"XSDT";
const DSDT_SIGNATURE: &[u8; 4] = b"DSDT";

const FADT_DSDT_OFFSET: usize = 40;
const FADT_PM1A_CONTROL_BLOCK_OFFSET: usize = 64;
const FADT_PM1B_CONTROL_BLOCK_OFFSET: usize = 68;
const FADT_PM1_CONTROL_LENGTH_OFFSET: usize = 89;
const FADT_X_DSDT_OFFSET: usize = 140;
const FADT_X_PM1A_CONTROL_BLOCK_OFFSET: usize = 172;
const FADT_X_PM1B_CONTROL_BLOCK_OFFSET: usize = 184;
const GAS_LENGTH: usize = 12;
const GAS_SPACE_ID_SYSTEM_IO: u8 = 1;

const AML_NAME_OP: u8 = 0x08;
const AML_PACKAGE_OP: u8 = 0x12;
const AML_ROOT_PREFIX: u8 = 0x5c;
const AML_PARENT_PREFIX: u8 = 0x5e;
const AML_BYTE_PREFIX: u8 = 0x0a;
const AML_WORD_PREFIX: u8 = 0x0b;
const AML_DWORD_PREFIX: u8 = 0x0c;
const AML_QWORD_PREFIX: u8 = 0x0e;

const PM1_CONTROL_SLEEP_TYPE_SHIFT: u16 = 10;
const PM1_CONTROL_SLEEP_ENABLE: u16 = 1 << 13;

static POWER_STATE: PowerCell = PowerCell::new();

/// Initializes kernel shutdown support from the firmware-provided ACPI tables.
pub fn init(acpi_rsdp_addr: u64) {
    let backend = match discover_acpi_shutdown(acpi_rsdp_addr) {
        Ok(shutdown) => PowerBackend::AcpiSoftOff(shutdown),
        Err(error) => PowerBackend::Unavailable(error),
    };

    with_power_mut(|state| {
        state.backend = backend;
    });
}

/// Requests a system shutdown and never returns.
#[cfg(not(test))]
pub fn shutdown() -> ! {
    let backend = with_power(|state| state.backend);

    match backend {
        PowerBackend::AcpiSoftOff(shutdown) => {
            shutdown.perform();
            print_shutdown_message("firmware did not power off the machine");
            crate::halt_forever();
        }
        PowerBackend::Unavailable(error) => {
            print_shutdown_message(error.as_str());
            crate::halt_forever();
        }
        PowerBackend::Uninitialized => {
            print_shutdown_message("power subsystem was not initialized");
            crate::halt_forever();
        }
    }
}

#[cfg(test)]
pub fn shutdown() -> ! {
    panic!("shutdown is not available in host tests");
}

fn with_power<F, T>(operation: F) -> T
where
    F: FnOnce(&PowerState) -> T,
{
    unsafe { operation(POWER_STATE.get()) }
}

fn with_power_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut PowerState) -> T,
{
    unsafe { operation(POWER_STATE.get()) }
}

struct PowerCell {
    state: UnsafeCell<PowerState>,
}

impl PowerCell {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(PowerState::new()),
        }
    }

    unsafe fn get(&self) -> &mut PowerState {
        &mut *self.state.get()
    }
}

unsafe impl Sync for PowerCell {}

struct PowerState {
    backend: PowerBackend,
}

impl PowerState {
    const fn new() -> Self {
        Self {
            backend: PowerBackend::Uninitialized,
        }
    }
}

#[derive(Clone, Copy)]
enum PowerBackend {
    Uninitialized,
    AcpiSoftOff(AcpiShutdown),
    Unavailable(PowerError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootTableKind {
    Rsdt,
    Xsdt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RootTable {
    kind: RootTableKind,
    address: u64,
}

#[derive(Clone, Copy)]
struct AcpiShutdown {
    pm1a_control_block: u16,
    pm1b_control_block: Option<u16>,
    sleep_type_a: u16,
    sleep_type_b: u16,
}

impl AcpiShutdown {
    #[cfg(not(test))]
    fn perform(self) {
        let command_a =
            (self.sleep_type_a << PM1_CONTROL_SLEEP_TYPE_SHIFT) | PM1_CONTROL_SLEEP_ENABLE;
        unsafe {
            port_io::outw(self.pm1a_control_block, command_a);
            if let Some(pm1b_control_block) = self.pm1b_control_block {
                let command_b =
                    (self.sleep_type_b << PM1_CONTROL_SLEEP_TYPE_SHIFT) | PM1_CONTROL_SLEEP_ENABLE;
                port_io::outw(pm1b_control_block, command_b);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PowerError {
    MissingRsdp,
    InvalidRsdp,
    MissingRootTable,
    InvalidRootTable,
    MissingFadt,
    InvalidFadt,
    MissingDsdt,
    InvalidDsdt,
    MissingPm1ControlBlock,
    UnsupportedPm1ControlBlock,
    MissingS5Package,
    InvalidS5Package,
    MappingFailed,
}

impl PowerError {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MissingRsdp => "ACPI RSDP was not provided by the loader",
            Self::InvalidRsdp => "ACPI RSDP was malformed or failed checksum validation",
            Self::MissingRootTable => "ACPI root system table address was missing",
            Self::InvalidRootTable => "ACPI root system table was malformed or invalid",
            Self::MissingFadt => "ACPI FADT table was not found",
            Self::InvalidFadt => "ACPI FADT table was malformed or unsupported",
            Self::MissingDsdt => "ACPI DSDT table was not found",
            Self::InvalidDsdt => "ACPI DSDT table was malformed or unsupported",
            Self::MissingPm1ControlBlock => "ACPI PM1 control block was not described",
            Self::UnsupportedPm1ControlBlock => {
                "ACPI PM1 control block used an unsupported address format"
            }
            Self::MissingS5Package => "ACPI DSDT did not expose an _S5 shutdown package",
            Self::InvalidS5Package => "ACPI _S5 shutdown package was malformed",
            Self::MappingFailed => "kernel could not map the ACPI table range",
        }
    }
}

#[derive(Clone, Copy)]
struct FadtInfo {
    dsdt_address: u64,
    pm1a_control_block: u16,
    pm1b_control_block: Option<u16>,
}

fn discover_acpi_shutdown(acpi_rsdp_addr: u64) -> Result<AcpiShutdown, PowerError> {
    if acpi_rsdp_addr == 0 {
        return Err(PowerError::MissingRsdp);
    }

    let rsdp = map_bytes(acpi_rsdp_addr, RSDP_V2_LENGTH)?;
    let root_table = parse_rsdp(rsdp)?;
    let root = map_sdt(root_table.address, root_table_signature(root_table.kind))?;
    let fadt_address = find_fadt_address(root, root_table.kind)?;
    let fadt = map_sdt(fadt_address, FADT_SIGNATURE)?;
    let fadt_info = parse_fadt(fadt)?;
    let dsdt = map_sdt(fadt_info.dsdt_address, DSDT_SIGNATURE)?;
    let (sleep_type_a, sleep_type_b) = parse_s5_package(dsdt)?;

    Ok(AcpiShutdown {
        pm1a_control_block: fadt_info.pm1a_control_block,
        pm1b_control_block: fadt_info.pm1b_control_block,
        sleep_type_a,
        sleep_type_b,
    })
}

fn root_table_signature(kind: RootTableKind) -> &'static [u8; 4] {
    match kind {
        RootTableKind::Rsdt => RSDT_SIGNATURE,
        RootTableKind::Xsdt => XSDT_SIGNATURE,
    }
}

#[cfg(not(test))]
fn map_bytes(address: u64, len: usize) -> Result<&'static [u8], PowerError> {
    if len == 0 {
        return Err(PowerError::MappingFailed);
    }
    let end = address
        .checked_add(len as u64)
        .ok_or(PowerError::MappingFailed)?;
    memory::map_kernel_identity_range(address, end, false)
        .map_err(|_| PowerError::MappingFailed)?;
    unsafe { Ok(slice::from_raw_parts(address as *const u8, len)) }
}

#[cfg(test)]
fn map_bytes(_address: u64, _len: usize) -> Result<&'static [u8], PowerError> {
    Err(PowerError::MappingFailed)
}

fn map_sdt(
    address: u64,
    expected_signature: &'static [u8; 4],
) -> Result<&'static [u8], PowerError> {
    if address == 0 {
        return Err(PowerError::MissingRootTable);
    }

    let header = map_bytes(address, SDT_HEADER_LENGTH)?;
    let length = parse_table_length(header).ok_or(PowerError::InvalidRootTable)?;
    let table = map_bytes(address, length)?;
    parse_sdt_header(table, expected_signature).map_err(|error| match error {
        PowerError::InvalidRootTable => PowerError::InvalidRootTable,
        PowerError::InvalidFadt => PowerError::InvalidFadt,
        PowerError::InvalidDsdt => PowerError::InvalidDsdt,
        other => other,
    })?;
    Ok(table)
}

fn parse_rsdp(bytes: &[u8]) -> Result<RootTable, PowerError> {
    if bytes.len() < RSDP_V1_LENGTH
        || &bytes[..8] != b"RSD PTR "
        || checksum(bytes, RSDP_V1_LENGTH) != 0
    {
        return Err(PowerError::InvalidRsdp);
    }

    let revision = bytes[15];
    if revision >= 2 {
        if bytes.len() < RSDP_V2_LENGTH {
            return Err(PowerError::InvalidRsdp);
        }
        let length = read_u32(bytes, 20).ok_or(PowerError::InvalidRsdp)? as usize;
        if length < RSDP_V2_LENGTH || bytes.len() < length || checksum(bytes, length) != 0 {
            return Err(PowerError::InvalidRsdp);
        }
        let xsdt_address = read_u64(bytes, 24).ok_or(PowerError::InvalidRsdp)?;
        if xsdt_address != 0 {
            return Ok(RootTable {
                kind: RootTableKind::Xsdt,
                address: xsdt_address,
            });
        }
    }

    let rsdt_address = read_u32(bytes, 16).ok_or(PowerError::InvalidRsdp)? as u64;
    if rsdt_address == 0 {
        return Err(PowerError::MissingRootTable);
    }

    Ok(RootTable {
        kind: RootTableKind::Rsdt,
        address: rsdt_address,
    })
}

fn parse_table_length(bytes: &[u8]) -> Option<usize> {
    let length = read_u32(bytes, 4)? as usize;
    if length < SDT_HEADER_LENGTH {
        return None;
    }
    Some(length)
}

fn parse_sdt_header(bytes: &[u8], expected_signature: &[u8; 4]) -> Result<(), PowerError> {
    if bytes.len() < SDT_HEADER_LENGTH {
        return Err(table_error(expected_signature));
    }

    let length = parse_table_length(bytes).ok_or_else(|| table_error(expected_signature))?;
    if bytes.len() < length || &bytes[..4] != expected_signature || checksum(bytes, length) != 0 {
        return Err(table_error(expected_signature));
    }

    Ok(())
}

fn table_error(signature: &[u8; 4]) -> PowerError {
    if signature == FADT_SIGNATURE {
        PowerError::InvalidFadt
    } else if signature == DSDT_SIGNATURE {
        PowerError::InvalidDsdt
    } else {
        PowerError::InvalidRootTable
    }
}

fn find_fadt_address(root: &[u8], kind: RootTableKind) -> Result<u64, PowerError> {
    let entry_size = match kind {
        RootTableKind::Rsdt => 4,
        RootTableKind::Xsdt => 8,
    };

    let mut offset = SDT_HEADER_LENGTH;
    while offset + entry_size <= root.len() {
        let table_address = if entry_size == 4 {
            read_u32(root, offset).map(|value| value as u64)
        } else {
            read_u64(root, offset)
        }
        .ok_or(PowerError::InvalidRootTable)?;

        if table_address != 0 {
            let header = map_bytes(table_address, SDT_HEADER_LENGTH)?;
            if header.len() >= 4 && &header[..4] == FADT_SIGNATURE {
                return Ok(table_address);
            }
        }

        offset += entry_size;
    }

    Err(PowerError::MissingFadt)
}

fn parse_fadt(bytes: &[u8]) -> Result<FadtInfo, PowerError> {
    parse_sdt_header(bytes, FADT_SIGNATURE)?;

    if bytes.len() <= FADT_PM1_CONTROL_LENGTH_OFFSET || bytes[FADT_PM1_CONTROL_LENGTH_OFFSET] < 2 {
        return Err(PowerError::InvalidFadt);
    }

    let dsdt_address = select_dsdt_address(bytes)?;
    let pm1a_control_block = select_pm1_control_block(
        bytes,
        FADT_PM1A_CONTROL_BLOCK_OFFSET,
        FADT_X_PM1A_CONTROL_BLOCK_OFFSET,
    )?
    .ok_or(PowerError::MissingPm1ControlBlock)?;
    let pm1b_control_block = select_pm1_control_block(
        bytes,
        FADT_PM1B_CONTROL_BLOCK_OFFSET,
        FADT_X_PM1B_CONTROL_BLOCK_OFFSET,
    )?;

    Ok(FadtInfo {
        dsdt_address,
        pm1a_control_block,
        pm1b_control_block,
    })
}

fn select_dsdt_address(bytes: &[u8]) -> Result<u64, PowerError> {
    if bytes.len() >= FADT_X_DSDT_OFFSET + 8 {
        let x_dsdt = read_u64(bytes, FADT_X_DSDT_OFFSET).ok_or(PowerError::InvalidFadt)?;
        if x_dsdt != 0 {
            return Ok(x_dsdt);
        }
    }

    let dsdt = read_u32(bytes, FADT_DSDT_OFFSET).ok_or(PowerError::InvalidFadt)? as u64;
    if dsdt == 0 {
        return Err(PowerError::MissingDsdt);
    }

    Ok(dsdt)
}

fn select_pm1_control_block(
    bytes: &[u8],
    legacy_offset: usize,
    extended_offset: usize,
) -> Result<Option<u16>, PowerError> {
    if bytes.len() >= extended_offset + GAS_LENGTH {
        match parse_system_io_gas(&bytes[extended_offset..extended_offset + GAS_LENGTH])? {
            Some(address) => return Ok(Some(address)),
            None => {}
        }
    }

    let legacy = read_u32(bytes, legacy_offset).ok_or(PowerError::InvalidFadt)?;
    if legacy == 0 {
        return Ok(None);
    }
    if legacy > u16::MAX as u32 {
        return Err(PowerError::UnsupportedPm1ControlBlock);
    }

    Ok(Some(legacy as u16))
}

fn parse_system_io_gas(bytes: &[u8]) -> Result<Option<u16>, PowerError> {
    if bytes.len() < GAS_LENGTH {
        return Err(PowerError::InvalidFadt);
    }

    let space_id = bytes[0];
    let bit_width = bytes[1];
    let address = read_u64(bytes, 4).ok_or(PowerError::InvalidFadt)?;

    if address == 0 {
        return Ok(None);
    }
    if space_id != GAS_SPACE_ID_SYSTEM_IO || bit_width < 16 || address > u16::MAX as u64 {
        return Err(PowerError::UnsupportedPm1ControlBlock);
    }

    Ok(Some(address as u16))
}

fn parse_s5_package(dsdt: &[u8]) -> Result<(u16, u16), PowerError> {
    parse_sdt_header(dsdt, DSDT_SIGNATURE)?;
    let aml = &dsdt[SDT_HEADER_LENGTH..];
    let mut index = 0usize;
    while index + 6 <= aml.len() {
        let name_start = if aml[index] == AML_NAME_OP {
            index + 1
        } else if index + 1 < aml.len()
            && (aml[index] == AML_ROOT_PREFIX || aml[index] == AML_PARENT_PREFIX)
            && aml[index + 1] == AML_NAME_OP
        {
            index + 2
        } else {
            index += 1;
            continue;
        };

        if name_start + 4 > aml.len() || &aml[name_start..name_start + 4] != b"_S5_" {
            index += 1;
            continue;
        }

        let mut cursor = name_start + 4;
        if cursor >= aml.len() || aml[cursor] != AML_PACKAGE_OP {
            index += 1;
            continue;
        }
        cursor += 1;

        let (_, package_length_bytes) =
            parse_package_length(&aml[cursor..]).ok_or(PowerError::InvalidS5Package)?;
        cursor += package_length_bytes;
        if cursor >= aml.len() {
            return Err(PowerError::InvalidS5Package);
        }

        cursor += 1;
        let (sleep_type_a, consumed_a) =
            parse_aml_integer(&aml[cursor..]).ok_or(PowerError::InvalidS5Package)?;
        cursor += consumed_a;
        let (sleep_type_b, _) =
            parse_aml_integer(&aml[cursor..]).ok_or(PowerError::InvalidS5Package)?;

        return Ok((sleep_type_a & 0x7, sleep_type_b & 0x7));
    }

    Err(PowerError::MissingS5Package)
}

fn parse_package_length(bytes: &[u8]) -> Option<(usize, usize)> {
    let lead = *bytes.first()?;
    let following_bytes = (lead >> 6) as usize;
    if bytes.len() < 1 + following_bytes {
        return None;
    }

    let mut length = (lead & 0x0f) as usize;
    let mut index = 0usize;
    while index < following_bytes {
        length |= (bytes[1 + index] as usize) << (4 + (index * 8));
        index += 1;
    }

    Some((length, 1 + following_bytes))
}

fn parse_aml_integer(bytes: &[u8]) -> Option<(u16, usize)> {
    let opcode = *bytes.first()?;
    match opcode {
        0x00 => Some((0, 1)),
        0x01 => Some((1, 1)),
        AML_BYTE_PREFIX if bytes.len() >= 2 => Some((bytes[1] as u16, 2)),
        AML_WORD_PREFIX if bytes.len() >= 3 => Some((u16::from_le_bytes([bytes[1], bytes[2]]), 3)),
        AML_DWORD_PREFIX if bytes.len() >= 5 => {
            let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
            Some((value as u16, 5))
        }
        AML_QWORD_PREFIX if bytes.len() >= 9 => {
            let value = u64::from_le_bytes([
                bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
            ]);
            Some((value as u16, 9))
        }
        _ => None,
    }
}

fn checksum(bytes: &[u8], length: usize) -> u8 {
    bytes[..length]
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(slice.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let slice = bytes.get(offset..offset + 8)?;
    Some(u64::from_le_bytes(slice.try_into().ok()?))
}

#[cfg(not(test))]
fn print_shutdown_message(reason: &str) {
    crate::kprintln!("shutdown unavailable: {}", reason);
}

#[cfg(test)]
fn print_shutdown_message(_reason: &str) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    #[test]
    fn parse_rsdp_prefers_xsdt_when_available() {
        let mut rsdp = [0u8; RSDP_V2_LENGTH];
        rsdp[..8].copy_from_slice(b"RSD PTR ");
        rsdp[15] = 2;
        rsdp[16..20].copy_from_slice(&(0x1234u32).to_le_bytes());
        rsdp[20..24].copy_from_slice(&(RSDP_V2_LENGTH as u32).to_le_bytes());
        rsdp[24..32].copy_from_slice(&(0x8877_6655_4433_2211u64).to_le_bytes());
        finalize_rsdp_checksum(&mut rsdp);

        assert_eq!(
            parse_rsdp(&rsdp).unwrap(),
            RootTable {
                kind: RootTableKind::Xsdt,
                address: 0x8877_6655_4433_2211
            }
        );
    }

    #[test]
    fn parse_rsdp_falls_back_to_rsdt() {
        let mut rsdp = [0u8; RSDP_V2_LENGTH];
        rsdp[..8].copy_from_slice(b"RSD PTR ");
        rsdp[15] = 0;
        rsdp[16..20].copy_from_slice(&(0x4321u32).to_le_bytes());
        finalize_rsdp_checksum(&mut rsdp);

        assert_eq!(
            parse_rsdp(&rsdp).unwrap(),
            RootTable {
                kind: RootTableKind::Rsdt,
                address: 0x4321
            }
        );
    }

    #[test]
    fn parse_fadt_prefers_extended_system_io_control_blocks() {
        let mut fadt = vec![0u8; 196];
        write_sdt_header(&mut fadt, FADT_SIGNATURE);
        fadt[FADT_PM1_CONTROL_LENGTH_OFFSET] = 2;
        fadt[FADT_DSDT_OFFSET..FADT_DSDT_OFFSET + 4].copy_from_slice(&(0x1000u32).to_le_bytes());
        fadt[FADT_X_DSDT_OFFSET..FADT_X_DSDT_OFFSET + 8]
            .copy_from_slice(&(0x2000u64).to_le_bytes());
        write_gas(
            &mut fadt
                [FADT_X_PM1A_CONTROL_BLOCK_OFFSET..FADT_X_PM1A_CONTROL_BLOCK_OFFSET + GAS_LENGTH],
            0x604,
        );
        finalize_sdt_checksum(&mut fadt);

        let info = parse_fadt(&fadt).unwrap();
        assert_eq!(info.dsdt_address, 0x2000);
        assert_eq!(info.pm1a_control_block, 0x604);
        assert_eq!(info.pm1b_control_block, None);
    }

    #[test]
    fn parse_s5_package_reads_shutdown_sleep_types() {
        let mut dsdt = vec![0u8; SDT_HEADER_LENGTH + 16];
        write_sdt_header(&mut dsdt, DSDT_SIGNATURE);
        let aml = &mut dsdt[SDT_HEADER_LENGTH..];
        aml[..10].copy_from_slice(&[
            AML_NAME_OP,
            b'_',
            b'S',
            b'5',
            b'_',
            AML_PACKAGE_OP,
            0x06,
            0x02,
            AML_BYTE_PREFIX,
            0x05,
        ]);
        aml[10..12].copy_from_slice(&[AML_BYTE_PREFIX, 0x05]);
        finalize_sdt_checksum(&mut dsdt);

        assert_eq!(parse_s5_package(&dsdt).unwrap(), (5, 5));
    }

    #[test]
    fn parse_s5_package_rejects_missing_definition() {
        let mut dsdt = vec![0u8; SDT_HEADER_LENGTH + 8];
        write_sdt_header(&mut dsdt, DSDT_SIGNATURE);
        finalize_sdt_checksum(&mut dsdt);

        assert_eq!(parse_s5_package(&dsdt), Err(PowerError::MissingS5Package));
    }

    fn write_sdt_header(bytes: &mut [u8], signature: &[u8; 4]) {
        let length = bytes.len() as u32;
        bytes[..4].copy_from_slice(signature);
        bytes[4..8].copy_from_slice(&length.to_le_bytes());
        bytes[8] = 1;
    }

    fn write_gas(bytes: &mut [u8], address: u16) {
        bytes[0] = GAS_SPACE_ID_SYSTEM_IO;
        bytes[1] = 16;
        bytes[4..12].copy_from_slice(&(address as u64).to_le_bytes());
    }

    fn finalize_sdt_checksum(bytes: &mut [u8]) {
        bytes[9] = 0;
        let sum = checksum(bytes, bytes.len());
        bytes[9] = bytes[9].wrapping_sub(sum);
    }

    fn finalize_rsdp_checksum(bytes: &mut [u8; RSDP_V2_LENGTH]) {
        bytes[8] = 0;
        let sum_v1 = checksum(bytes, RSDP_V1_LENGTH);
        bytes[8] = bytes[8].wrapping_sub(sum_v1);

        bytes[32] = 0;
        let sum_v2 = checksum(bytes, RSDP_V2_LENGTH);
        bytes[32] = bytes[32].wrapping_sub(sum_v2);
    }
}
