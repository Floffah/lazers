# boot

This directory owns firmware entry, boot-path coordination, early platform initialization, and the handoff contract into the kernel.

The first implementation is a UEFI `x86_64` loader that reads a freestanding ELF64 kernel from the EFI System Partition, exits boot services, and hands control to the kernel with a shared `BootInfo` contract.
