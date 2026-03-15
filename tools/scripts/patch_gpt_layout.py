#!/usr/bin/env python3

import binascii
import struct
import sys


SECTOR_SIZE = 512
GPT_SIGNATURE = b"EFI PART"
EFI_SYSTEM_GUID_BYTES = bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")
MICROSOFT_BASIC_DATA_GUID_BYTES = bytes.fromhex("a2a0d0ebe5b9334487c068b6b72699c7")


def read_header(image, lba):
    image.seek(lba * SECTOR_SIZE)
    header = bytearray(image.read(SECTOR_SIZE))
    if header[:8] != GPT_SIGNATURE:
        raise RuntimeError(f"missing GPT header at LBA {lba}")
    header_size = struct.unpack_from("<I", header, 12)[0]
    table_lba = struct.unpack_from("<Q", header, 72)[0]
    entry_count = struct.unpack_from("<I", header, 80)[0]
    entry_size = struct.unpack_from("<I", header, 84)[0]
    return header, header_size, table_lba, entry_count, entry_size


def encode_partition_name(name: str) -> bytes:
    encoded = name.encode("utf-16-le")
    if len(encoded) > 72:
        raise RuntimeError(f"partition name too long: {name}")
    return encoded + bytes(72 - len(encoded))


def update_table(image, table_lba, entry_count, entry_size, partition_names):
    table_size = entry_count * entry_size
    image.seek(table_lba * SECTOR_SIZE)
    table = bytearray(image.read(table_size))

    populated = []
    for index in range(entry_count):
        entry_offset = index * entry_size
        type_guid = table[entry_offset : entry_offset + 16]
        if any(type_guid):
            populated.append(entry_offset)

    if len(populated) < 2:
        raise RuntimeError("expected at least two populated GPT partition entries")

    table[populated[0] : populated[0] + 16] = EFI_SYSTEM_GUID_BYTES
    table[populated[0] + 56 : populated[0] + 128] = encode_partition_name(partition_names[0])

    table[populated[1] : populated[1] + 16] = MICROSOFT_BASIC_DATA_GUID_BYTES
    table[populated[1] + 56 : populated[1] + 128] = encode_partition_name(partition_names[1])

    image.seek(table_lba * SECTOR_SIZE)
    image.write(table)
    return binascii.crc32(table) & 0xFFFFFFFF


def update_header(image, header_lba, partition_names):
    header, header_size, table_lba, entry_count, entry_size = read_header(image, header_lba)
    table_crc = update_table(image, table_lba, entry_count, entry_size, partition_names)
    struct.pack_into("<I", header, 88, table_crc)
    struct.pack_into("<I", header, 16, 0)
    header_crc = binascii.crc32(header[:header_size]) & 0xFFFFFFFF
    struct.pack_into("<I", header, 16, header_crc)
    image.seek(header_lba * SECTOR_SIZE)
    image.write(header)


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: patch_gpt_layout.py <raw-image> <esp-name> <system-name>", file=sys.stderr)
        return 1

    image_path = sys.argv[1]
    partition_names = (sys.argv[2], sys.argv[3])
    with open(image_path, "r+b") as image:
        update_header(image, 1, partition_names)
        image.seek(SECTOR_SIZE)
        primary = image.read(SECTOR_SIZE)
        backup_lba = struct.unpack_from("<Q", primary, 32)[0]
        update_header(image, backup_lba, partition_names)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
