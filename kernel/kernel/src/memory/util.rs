use elf::ElfError;

pub(super) const fn pml4_index(address: u64) -> usize {
    ((address >> 39) & 0x1ff) as usize
}

pub(super) const fn pdpt_index(address: u64) -> usize {
    ((address >> 30) & 0x1ff) as usize
}

pub(super) const fn pd_index(address: u64) -> usize {
    ((address >> 21) & 0x1ff) as usize
}

pub(super) const fn pt_index(address: u64) -> usize {
    ((address >> 12) & 0x1ff) as usize
}

pub(super) const fn elf_error_as_str(error: ElfError) -> &'static str {
    match error {
        ElfError::HeaderTooSmall => "elf header is too small",
        ElfError::InvalidMagic => "elf magic is invalid",
        ElfError::UnsupportedClass => "elf class is unsupported",
        ElfError::UnsupportedEncoding => "elf encoding is unsupported",
        ElfError::UnsupportedType => "elf type is unsupported",
        ElfError::UnsupportedMachine => "elf machine is unsupported",
        ElfError::UnsupportedVersion => "elf version is unsupported",
        ElfError::ProgramHeaderTruncated => "elf program header is truncated",
        ElfError::ProgramHeaderTableOutOfRange => "elf program header table is out of range",
        ElfError::SegmentExtendsPastFile => "elf segment extends past the image",
        ElfError::AddressOutOfRange => "elf address is out of range",
        ElfError::ZeroLoadAddress => "elf load address is zero",
    }
}
