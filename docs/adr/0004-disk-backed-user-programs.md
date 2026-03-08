# ADR 0004: Disk-Backed User Programs From a System Partition

## Status

Accepted

## Context

The first user-mode milestone embedded the initial echo program directly into the kernel image. That was useful for proving user-mode execution and the first syscall path, but it was not the final executable-loading architecture. The next milestone needs a real disk-backed path that survives into the future shell and application model.

The repository already boots from a GPT disk image through UEFI and stages the loader plus kernel on a FAT32 EFI System Partition. The runtime now needs a separate system volume so normal paths resolve against OS content rather than boot firmware artifacts.

## Decision

- Keep boot on a `FAT32` EFI System Partition.
- Add a second `FAT32` system partition named `LAZERS-SYSTEM`.
- Treat the system partition as the runtime root filesystem `/`.
- Keep the kernel on the EFI System Partition for now so the bootloader only needs the firmware-provided FAT path.
- Replace the embedded user ELF source with disk-backed loading from `/bin/echo`.
- Access the disk through a kernel-owned `AHCI/SATA -> GPT -> FAT32 -> ELF` chain.
- Keep FAT32 read-only and short-name-only for this milestone.

## Consequences

### Positive

- User programs now load from the same disk model the final system will use.
- `/bin/lash` can later reuse the same executable-loading path instead of introducing another transition step.
- The EFI System Partition stays a boot implementation detail rather than the normal runtime namespace.

### Negative

- The kernel now owns an initial storage stack earlier than it otherwise would.
- FAT32 is intentionally limited and will likely be replaced or expanded later.
- QEMU and the kernel both need to agree on an AHCI/SATA-backed disk path rather than a simpler USB-storage setup.
