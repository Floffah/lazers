#!/usr/bin/env python3

import binascii
import struct
import sys


SECTOR_SIZE = 512
GPT_SIGNATURE = b"EFI PART"
EFI_SYSTEM_GUID_BYTES = bytes.fromhex("28732ac11ff8d211ba4b00a0c93ec93b")


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


def update_table(image, table_lba, entry_count, entry_size):
    table_size = entry_count * entry_size
    image.seek(table_lba * SECTOR_SIZE)
    table = bytearray(image.read(table_size))

    for index in range(entry_count):
        entry_offset = index * entry_size
        type_guid = table[entry_offset : entry_offset + 16]
        if any(type_guid):
            table[entry_offset : entry_offset + 16] = EFI_SYSTEM_GUID_BYTES
            break
    else:
        raise RuntimeError("no populated GPT partition entry found to convert to EFI System Partition")

    image.seek(table_lba * SECTOR_SIZE)
    image.write(table)
    return binascii.crc32(table) & 0xFFFFFFFF


def update_header(image, header_lba):
    header, header_size, table_lba, entry_count, entry_size = read_header(image, header_lba)
    table_crc = update_table(image, table_lba, entry_count, entry_size)
    struct.pack_into("<I", header, 88, table_crc)
    struct.pack_into("<I", header, 16, 0)
    header_crc = binascii.crc32(header[:header_size]) & 0xFFFFFFFF
    struct.pack_into("<I", header, 16, header_crc)
    image.seek(header_lba * SECTOR_SIZE)
    image.write(header)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: patch_gpt_esp.py <raw-image>", file=sys.stderr)
        return 1

    image_path = sys.argv[1]
    with open(image_path, "r+b") as image:
        update_header(image, 1)
        image.seek(SECTOR_SIZE)
        primary = image.read(SECTOR_SIZE)
        backup_lba = struct.unpack_from("<Q", primary, 32)[0]
        update_header(image, backup_lba)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
