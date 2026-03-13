use super::ahci::BlockDevice;
use super::util::{align_up, read_le_u16, read_le_u32, read_le_u64, SECTOR_SIZE};
use super::StorageError;

const GPT_HEADER_LBA: u64 = 1;
const GPT_HEADER_SIGNATURE: [u8; 8] = *b"EFI PART";

pub(super) const EFI_SYSTEM_PARTITION_GUID: [u8; 16] = [
    0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
];

#[derive(Clone, Copy)]
pub(super) struct GptPartitions {
    partitions: [Option<GptPartition>; 16],
}

impl GptPartitions {
    pub(super) fn read(device: BlockDevice) -> Result<Self, StorageError> {
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

    pub(super) fn find_by_type(&self, type_guid: [u8; 16]) -> Option<GptPartition> {
        self.partitions
            .iter()
            .flatten()
            .copied()
            .find(|partition| partition.type_guid == type_guid)
    }

    pub(super) fn find_by_name(&self, name: &str) -> Option<GptPartition> {
        self.partitions
            .iter()
            .flatten()
            .copied()
            .find(|partition| partition.name_matches(name))
    }
}

#[derive(Clone, Copy)]
pub(super) struct GptPartition {
    type_guid: [u8; 16],
    pub(super) start_lba: u64,
    pub(super) sector_count: u64,
    name: [u16; 36],
}

impl GptPartition {
    pub(super) fn name_matches(&self, name: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::GptPartition;

    #[test]
    fn name_matching_requires_exact_utf16_name_and_zero_tail() {
        let partition = GptPartition {
            type_guid: [0; 16],
            start_lba: 1,
            sector_count: 2,
            name: {
                let mut name = [0u16; 36];
                name[0] = b'L' as u16;
                name[1] = b'A' as u16;
                name[2] = b'Z' as u16;
                name
            },
        };

        assert!(partition.name_matches("LAZ"));
        assert!(!partition.name_matches("LA"));
        assert!(!partition.name_matches("LAZZ"));
    }
}
