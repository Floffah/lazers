#![no_std]

pub const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

pub const fn align_up(value: u64, align: u64) -> u64 {
    if value == 0 {
        0
    } else {
        (value + align - 1) & !(align - 1)
    }
}

pub fn read_le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

pub fn read_le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

pub fn read_le_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::{align_down, align_up, read_le_u16, read_le_u32, read_le_u64};

    #[test]
    fn align_helpers_round_as_expected() {
        assert_eq!(align_down(0x1234, 0x1000), 0x1000);
        assert_eq!(align_up(0x1234, 0x1000), 0x2000);
        assert_eq!(align_up(0, 0x1000), 0);
    }

    #[test]
    fn little_endian_readers_decode_at_offsets() {
        let bytes = [0xaa, 0x34, 0x12, 0x78, 0x56, 0xef, 0xcd, 0xab, 0x90, 0x55];

        assert_eq!(read_le_u16(&bytes, 1), 0x1234);
        assert_eq!(read_le_u32(&bytes, 3), 0xcdef_5678);
        assert_eq!(read_le_u64(&bytes, 1), 0x90ab_cdef_5678_1234);
    }
}
