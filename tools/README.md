# tools

This directory owns repository-local tooling for image creation, local execution, debugging, inspection, and related workflows.

Top-level scripts under `tools/scripts/` are generic entrypoints. Host-specific implementations live under:

- `tools/scripts/macos`
- `tools/scripts/linux`

The current tooling layer builds a raw GPT disk image with two `FAT32` partitions:

- an EFI System Partition that stages `BOOTX64.EFI` and `kernel.elf`
- a system partition that stages `/system/bin/lash`, `/system/bin/echo`, `/system/bin/ls`, `/system/bin/cat`, and future userland binaries

The logical runtime namespace is lowercase:

- runtime executables live under `/system/bin`
- GPT partition names are `LAZERS-ESP` and `LAZERS-SYSTEM`

The current FAT staging path is an implementation detail of image creation. The host-specific mechanism can vary by operating system, but the resulting image contract must stay the same:

- mounted volume names are `LAZERSESP` and `LAZERSSYS`
- staged runtime binaries are copied into `SYSTEM/BIN` with uppercase filenames before the kernel mounts the runtime partition
- GPT metadata is patched after partition creation so the shipped image still exposes `LAZERS-ESP` and `LAZERS-SYSTEM`

QEMU boots that same image under standard EDK2 firmware through an AHCI/SATA-backed disk path. Keep tools reproducible, documented, and aligned with the `justfile` so developer workflows stay discoverable and low-friction.
