# tools

This directory owns repository-local tooling for image creation, local execution, debugging, inspection, and related workflows.

The current tooling layer builds a raw GPT disk image with two `FAT32` partitions:

- an EFI System Partition that stages `BOOTX64.EFI` and `kernel.elf`
- a system partition that stages `/bin/echo` and future userland binaries

QEMU boots that same image under standard EDK2 firmware through an AHCI/SATA-backed disk path. Keep tools reproducible, documented, and aligned with the `justfile` so developer workflows stay discoverable and low-friction.
