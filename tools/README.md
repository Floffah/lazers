# tools

This directory owns repository-local tooling for image creation, local execution, debugging, inspection, and related workflows.

The first tooling layer builds a raw GPT disk image, stages the EFI loader and kernel into a FAT32 EFI System Partition, and boots the result under QEMU with standard EDK2 firmware. Keep tools reproducible, documented, and aligned with the `justfile` so developer workflows stay discoverable and low-friction.
