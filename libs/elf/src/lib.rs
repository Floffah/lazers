#![no_std]

use core::mem::size_of;
use core::ops::Range;
use lzutil::{read_le_u16, read_le_u32, read_le_u64};

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1 << 0;
pub const PF_W: u32 = 1 << 1;
pub const PF_R: u32 = 1 << 2;

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LE: u8 = 1;
const ELF_MACHINE_X86_64: u16 = 0x3e;
const ELF_TYPE_EXECUTABLE: u16 = 2;

#[derive(Clone, Copy, Debug)]
pub struct ElfImage<'a> {
    bytes: &'a [u8],
    header: ElfHeader,
}

impl<'a> ElfImage<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ElfError> {
        let header = ElfHeader::parse(bytes)?;
        Ok(Self { bytes, header })
    }

    pub fn entry_point(&self) -> u64 {
        self.header.entry
    }

    pub fn program_headers(&self) -> ProgramHeaderIter<'a> {
        ProgramHeaderIter {
            bytes: self.bytes,
            next: 0,
            count: self.header.program_header_count,
            table_offset: self.header.program_header_offset,
            entry_size: self.header.program_header_entry_size,
        }
    }
}

pub struct ProgramHeaderIter<'a> {
    bytes: &'a [u8],
    next: u16,
    count: u16,
    table_offset: usize,
    entry_size: usize,
}

impl<'a> Iterator for ProgramHeaderIter<'a> {
    type Item = Result<ProgramHeader, ElfError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }

        let index = self.next as usize;
        self.next += 1;

        let offset = self
            .table_offset
            .checked_add(index.checked_mul(self.entry_size)?)?;
        let bytes = self
            .bytes
            .get(offset..offset.checked_add(self.entry_size)?)?;
        Some(ProgramHeader::parse(bytes))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProgramHeader {
    pub kind: u32,
    pub flags: u32,
    pub offset: u64,
    pub virtual_address: u64,
    pub physical_address: u64,
    pub file_size: u64,
    pub memory_size: u64,
}

impl ProgramHeader {
    fn parse(bytes: &[u8]) -> Result<Self, ElfError> {
        if bytes.len() < size_of::<ElfProgramHeaderLayout>() {
            return Err(ElfError::ProgramHeaderTruncated);
        }

        Ok(Self {
            kind: read_le_u32(bytes, 0),
            flags: read_le_u32(bytes, 4),
            offset: read_le_u64(bytes, 8),
            virtual_address: read_le_u64(bytes, 16),
            physical_address: read_le_u64(bytes, 24),
            file_size: read_le_u64(bytes, 32),
            memory_size: read_le_u64(bytes, 40),
        })
    }

    pub fn file_range(&self, file_len: usize) -> Result<Range<usize>, ElfError> {
        let start = usize::try_from(self.offset).map_err(|_| ElfError::AddressOutOfRange)?;
        let len = usize::try_from(self.file_size).map_err(|_| ElfError::AddressOutOfRange)?;
        let end = start.checked_add(len).ok_or(ElfError::AddressOutOfRange)?;
        if end > file_len {
            return Err(ElfError::SegmentExtendsPastFile);
        }
        Ok(start..end)
    }

    pub fn load_address(&self) -> Result<u64, ElfError> {
        let address = if self.physical_address != 0 {
            self.physical_address
        } else {
            self.virtual_address
        };

        if address == 0 {
            return Err(ElfError::ZeroLoadAddress);
        }

        Ok(address)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ElfError {
    HeaderTooSmall,
    InvalidMagic,
    UnsupportedClass,
    UnsupportedEncoding,
    UnsupportedType,
    UnsupportedMachine,
    UnsupportedVersion,
    ProgramHeaderTruncated,
    ProgramHeaderTableOutOfRange,
    SegmentExtendsPastFile,
    AddressOutOfRange,
    ZeroLoadAddress,
}

#[derive(Clone, Copy, Debug)]
struct ElfHeader {
    entry: u64,
    program_header_offset: usize,
    program_header_entry_size: usize,
    program_header_count: u16,
}

impl ElfHeader {
    fn parse(bytes: &[u8]) -> Result<Self, ElfError> {
        if bytes.len() < size_of::<ElfHeaderLayout>() {
            return Err(ElfError::HeaderTooSmall);
        }

        if bytes[0..4] != ELF_MAGIC {
            return Err(ElfError::InvalidMagic);
        }
        if bytes[4] != ELF_CLASS_64 {
            return Err(ElfError::UnsupportedClass);
        }
        if bytes[5] != ELF_DATA_LE {
            return Err(ElfError::UnsupportedEncoding);
        }
        if bytes[6] != 1 {
            return Err(ElfError::UnsupportedVersion);
        }
        if read_le_u16(bytes, 16) != ELF_TYPE_EXECUTABLE {
            return Err(ElfError::UnsupportedType);
        }
        if read_le_u16(bytes, 18) != ELF_MACHINE_X86_64 {
            return Err(ElfError::UnsupportedMachine);
        }
        if read_le_u32(bytes, 20) != 1 {
            return Err(ElfError::UnsupportedVersion);
        }

        let program_header_offset =
            usize::try_from(read_le_u64(bytes, 32)).map_err(|_| ElfError::AddressOutOfRange)?;
        let program_header_entry_size = usize::from(read_le_u16(bytes, 54));
        let program_header_count = read_le_u16(bytes, 56);
        let table_size = program_header_entry_size
            .checked_mul(program_header_count as usize)
            .ok_or(ElfError::AddressOutOfRange)?;
        let table_end = program_header_offset
            .checked_add(table_size)
            .ok_or(ElfError::AddressOutOfRange)?;
        if table_end > bytes.len() {
            return Err(ElfError::ProgramHeaderTableOutOfRange);
        }

        Ok(Self {
            entry: read_le_u64(bytes, 24),
            program_header_offset,
            program_header_entry_size,
            program_header_count,
        })
    }
}

#[repr(C)]
struct ElfHeaderLayout {
    _unused: [u8; 64],
}

#[repr(C)]
struct ElfProgramHeaderLayout {
    _unused: [u8; 56],
}
