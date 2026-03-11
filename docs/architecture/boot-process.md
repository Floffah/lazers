# Boot Process

Lazers currently boots through a standard `UEFI -> loader -> kernel` path on `x86_64`. The same disk layout and firmware entry path used under QEMU are intended to remain valid on real UEFI hardware.

## What Happens At Boot

The boot sequence is deliberately simple:

1. UEFI firmware loads `/EFI/BOOT/BOOTX64.EFI` from the EFI System Partition.
2. The Lazers loader opens `/lazers/kernel.elf` from that same partition.
3. The loader parses the kernel as an `ELF64` executable and copies each loadable segment to the physical address requested by the image.
4. The loader gathers the framebuffer mode, a normalized memory map, the ACPI RSDP if present, and an initial kernel stack.
5. The loader exits boot services.
6. The loader jumps into the kernel with `rdi = BootInfo`.
7. The kernel takes over paging, initializes core CPU structures, mounts the runtime filesystem, loads the first user program, and starts the scheduler.

By the time the first user process appears, the firmware is out of the picture. Everything after that point is kernel-owned runtime behavior.

## Boot Contracts

The current boot path relies on a small set of stable assumptions:

- firmware target: UEFI only
- CPU mode at handoff: `x86_64` long mode
- kernel image format: freestanding `ELF64`
- loader path on disk: `/EFI/BOOT/BOOTX64.EFI`
- kernel path on disk: `/lazers/kernel.elf`
- runtime root filesystem: the `LAZERS-SYSTEM` GPT partition mounted as `/`
- UEFI boot services are no longer available after kernel entry

These contracts matter because both the loader and the kernel are written around them. If one changes, the other side has to change with it.

## Disk Layout And Program Loading

The boot disk currently has two partitions:

- `LAZERS-ESP`: a FAT32 EFI System Partition containing only boot-critical artifacts
- `LAZERS-SYSTEM`: a FAT32 runtime partition mounted as `/`

The loader only understands the ESP. It loads the kernel and hands off. The kernel then discovers storage for itself, mounts `LAZERS-SYSTEM`, and loads user programs from there through a full runtime path:

`AHCI/SATA -> GPT -> FAT32 -> ELF`

That same runtime path is used for the initial user program and for commands launched later by `lash`.

## Current First User Program

The kernel does not hardcode shell behavior, but it does currently choose one initial user program at build time. The default is `/bin/lash`, and alternate bootstrap programs like `/bin/selftest` can be selected for specialized images.

This keeps session policy narrow for now: the kernel launches one first program, and that program owns the higher-level behavior.

## What This Page Does Not Cover

This page describes the current UEFI boot path only. It does not define:

- BIOS boot
- higher-half kernel mapping
- timer-driven preemption
- SMP startup
- filesystem writes
- QEMU-specific shortcuts or emulator-only handoff behavior
