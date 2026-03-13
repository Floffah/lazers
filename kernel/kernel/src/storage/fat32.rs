use super::ahci::BlockDevice;
use super::gpt::GptPartition;
use super::util::{read_le_u16, read_le_u32, SECTOR_SIZE};
use super::StorageError;

const FAT_DIRECTORY_ENTRY_SIZE: usize = 32;
const FAT_ATTRIBUTE_DIRECTORY: u8 = 1 << 4;
const FAT_ATTRIBUTE_VOLUME_ID: u8 = 1 << 3;
const FAT_ATTRIBUTE_LONG_NAME: u8 = 0x0f;
const FAT_ENTRY_END_OF_CHAIN: u32 = 0x0fff_fff8;
const FAT_ENTRY_BAD_CLUSTER: u32 = 0x0fff_fff7;

#[derive(Clone, Copy)]
pub(super) struct Fat32 {
    device: BlockDevice,
    partition_start_lba: u64,
    partition_sector_count: u64,
    sectors_per_cluster: u8,
    reserved_sector_count: u16,
    root_cluster: u32,
    first_data_sector: u64,
}

impl Fat32 {
    pub(super) fn mount(
        device: BlockDevice,
        partition: GptPartition,
    ) -> Result<Self, StorageError> {
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

    pub(super) fn open_absolute(&self, path: &str) -> Result<FatDirectoryEntry, StorageError> {
        match self.resolve_absolute(path)? {
            FatResolvedPath::File(entry) => Ok(entry),
            FatResolvedPath::RootDirectory | FatResolvedPath::Directory(_) => {
                Err(StorageError::NotAFile)
            }
        }
    }

    pub(super) fn ensure_dir(&self, path: &str) -> Result<(), StorageError> {
        match self.resolve_absolute(path)? {
            FatResolvedPath::RootDirectory | FatResolvedPath::Directory(_) => Ok(()),
            FatResolvedPath::File(_) => Err(StorageError::NotADirectory),
        }
    }

    pub(super) fn read_file(
        &self,
        entry: &FatDirectoryEntry,
        buffer: &mut [u8],
    ) -> Result<usize, StorageError> {
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

    pub(super) fn read_dir(&self, path: &str, buffer: &mut [u8]) -> Result<usize, StorageError> {
        let start_cluster = match self.resolve_absolute(path)? {
            FatResolvedPath::RootDirectory => self.root_cluster,
            FatResolvedPath::Directory(cluster) => cluster,
            FatResolvedPath::File(_) => return Err(StorageError::NotADirectory),
        };

        let mut written = 0usize;
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
                        return Ok(written);
                    }
                    if first_byte == 0xe5 {
                        entry_offset += FAT_DIRECTORY_ENTRY_SIZE;
                        continue;
                    }

                    let attributes = entry[11];
                    if attributes == FAT_ATTRIBUTE_LONG_NAME
                        || (attributes & FAT_ATTRIBUTE_VOLUME_ID) != 0
                    {
                        entry_offset += FAT_DIRECTORY_ENTRY_SIZE;
                        continue;
                    }

                    let display_name = display_short_name(&entry[0..11]);
                    if !is_dot_entry(&display_name) {
                        let required = display_name.len + 1;
                        if written + required > buffer.len() {
                            return Err(StorageError::BufferTooSmall);
                        }
                        buffer[written..written + display_name.len]
                            .copy_from_slice(&display_name.bytes[..display_name.len]);
                        written += display_name.len;
                        buffer[written] = b'\n';
                        written += 1;
                    }

                    entry_offset += FAT_DIRECTORY_ENTRY_SIZE;
                }

                sector_index += 1;
            }

            cluster = self.next_cluster(cluster)?;
        }
    }

    fn find_in_directory(
        &self,
        start_cluster: u32,
        component: &str,
    ) -> Result<FatDirectoryEntry, StorageError> {
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
                    if attributes == FAT_ATTRIBUTE_LONG_NAME
                        || (attributes & FAT_ATTRIBUTE_VOLUME_ID) != 0
                    {
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

    fn resolve_absolute(&self, path: &str) -> Result<FatResolvedPath, StorageError> {
        if !path.starts_with('/') {
            return Err(StorageError::PathNotAbsolute);
        }

        let mut current_cluster = self.root_cluster;
        let mut components = path
            .split('/')
            .filter(|component| !component.is_empty())
            .peekable();
        let Some(_) = components.peek() else {
            return Ok(FatResolvedPath::RootDirectory);
        };

        while let Some(component) = components.next() {
            let entry = self.find_in_directory(current_cluster, component)?;
            if components.peek().is_none() {
                if entry.is_directory {
                    return Ok(FatResolvedPath::Directory(entry.first_cluster));
                }
                return Ok(FatResolvedPath::File(entry));
            }

            if !entry.is_directory {
                return Err(StorageError::FileNotFound);
            }

            current_cluster = entry.first_cluster;
        }

        Err(StorageError::FileNotFound)
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
pub(super) struct FatDirectoryEntry {
    pub(super) first_cluster: u32,
    pub(super) size: u32,
    is_directory: bool,
}

enum FatResolvedPath {
    RootDirectory,
    Directory(u32),
    File(FatDirectoryEntry),
}

struct DisplayShortName {
    bytes: [u8; 12],
    len: usize,
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

fn display_short_name(entry: &[u8]) -> DisplayShortName {
    let mut bytes = [0u8; 12];
    let mut len = 0usize;

    let mut name_len = 8usize;
    while name_len > 0 && entry[name_len - 1] == b' ' {
        name_len -= 1;
    }

    let mut extension_len = 3usize;
    while extension_len > 0 && entry[8 + extension_len - 1] == b' ' {
        extension_len -= 1;
    }

    let mut index = 0usize;
    while index < name_len {
        bytes[len] = entry[index];
        len += 1;
        index += 1;
    }

    if extension_len != 0 {
        bytes[len] = b'.';
        len += 1;
        index = 0;
        while index < extension_len {
            bytes[len] = entry[8 + index];
            len += 1;
            index += 1;
        }
    }

    DisplayShortName { bytes, len }
}

fn is_dot_entry(name: &DisplayShortName) -> bool {
    matches!(&name.bytes[..name.len], [b'.'] | [b'.', b'.'])
}

fn normalize_fat_byte(byte: u8) -> Result<u8, StorageError> {
    if byte.is_ascii_alphanumeric() {
        Ok(byte.to_ascii_uppercase())
    } else {
        match byte {
            b'$' | b'%' | b'\'' | b'-' | b'_' | b'@' | b'~' | b'`' | b'!' | b'(' | b')' | b'{'
            | b'}' | b'^' | b'#' | b'&' => Ok(byte),
            _ => Err(StorageError::InvalidShortName),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{display_short_name, is_dot_entry, make_short_name};
    use crate::storage::StorageError;

    #[test]
    fn short_name_encoding_uppercases_and_pads() {
        assert!(matches!(
            make_short_name("echo"),
            Ok(value) if value == *b"ECHO       "
        ));
        assert!(matches!(
            make_short_name("cat.txt"),
            Ok(value) if value == *b"CAT     TXT"
        ));
    }

    #[test]
    fn short_name_rejects_unsupported_shapes() {
        assert!(matches!(
            make_short_name(""),
            Err(StorageError::InvalidShortName)
        ));
        assert!(matches!(
            make_short_name("toolongname"),
            Err(StorageError::InvalidShortName)
        ));
        assert!(matches!(
            make_short_name("a.b.c"),
            Err(StorageError::InvalidShortName)
        ));
        assert!(matches!(
            make_short_name("bad*name"),
            Err(StorageError::InvalidShortName)
        ));
    }

    #[test]
    fn display_name_formats_short_entries() {
        let display_name = display_short_name(b"ECHO       ");
        assert_eq!(&display_name.bytes[..display_name.len], b"ECHO");

        let display_name = display_short_name(b"CAT     TXT");
        assert_eq!(&display_name.bytes[..display_name.len], b"CAT.TXT");
    }

    #[test]
    fn dot_entry_detection_matches_dot_and_dotdot() {
        assert!(is_dot_entry(&display_short_name(b".          ")));
        assert!(is_dot_entry(&display_short_name(b"..         ")));
        assert!(!is_dot_entry(&display_short_name(b"ECHO       ")));
    }
}
