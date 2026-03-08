# Boot Process

## Target

The first bootable target is a real-hardware-valid `UEFI -> loader -> ELF64 kernel` path for `x86_64`.

QEMU is used only as the execution environment for that same path. The disk layout, firmware entry point, loader behavior, and kernel handoff are designed to remain valid on actual UEFI hardware.

## Sequence

1. UEFI firmware loads `/EFI/BOOT/BOOTX64.EFI` from the EFI System Partition.
2. The loader opens `/lazers/kernel.elf` from the same partition using standard UEFI filesystem protocols.
3. The loader parses the kernel as an ELF64 executable and loads each `PT_LOAD` segment at its requested physical address.
4. The loader captures framebuffer details, copies a normalized memory map, finds the ACPI RSDP if present, and allocates the initial kernel stack.
5. The loader exits boot services.
6. The loader jumps to the kernel entry with `rdi = BootInfo` and the stack switched to the allocated kernel stack.
7. The kernel validates `BootInfo`, replaces the firmware page tables with kernel-owned page tables, initializes GDT/TSS/IDT state, renders the initial banner, discovers the AHCI controller, reads GPT, mounts the `LAZERS-SYSTEM` FAT32 partition as `/`, loads `/bin/echo` as a user ELF, and enters a cooperative scheduler with a kernel terminal thread plus one user echo process.

## Contracts

- Firmware target: UEFI only
- CPU mode at handoff: `x86_64` long mode
- Kernel image format: freestanding `ELF64`
- Loader path on disk: `/EFI/BOOT/BOOTX64.EFI`
- Kernel path on disk: `/lazers/kernel.elf`
- Runtime root filesystem: the `LAZERS-SYSTEM` GPT partition mounted as `/`
- Boot services are no longer available after handoff

## Non-Goals For V1

- BIOS boot
- higher-half memory mapping
- timer-driven preemption, SMP, or a userland shell
- filesystem writes, VFAT long names, or `/bin/lash`
- QEMU-specific device handoff or firmware shortcuts
