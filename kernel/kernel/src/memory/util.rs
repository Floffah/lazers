use elf::ElfError;

pub(super) const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

pub(super) const fn align_up(value: u64, align: u64) -> u64 {
    if value == 0 {
        0
    } else {
        (value + align - 1) & !(align - 1)
    }
}

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

#[cfg(test)]
mod tests {
    use super::{align_down, align_up};

    #[test]
    fn align_helpers_round_as_expected() {
        assert_eq!(align_down(0x1234, 0x1000), 0x1000);
        assert_eq!(align_up(0x1234, 0x1000), 0x2000);
        assert_eq!(align_up(0, 0x1000), 0);
    }
}
