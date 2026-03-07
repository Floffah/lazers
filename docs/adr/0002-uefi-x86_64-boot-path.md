# ADR 0002: Initial Boot Path

## Status

Accepted

## Context

The project needs a first bootable path that is valid for real hardware, not just for emulators. The first milestone only needs to launch a kernel and demonstrate that control passed cleanly beyond firmware services.

## Decision

- Target UEFI first.
- Target `x86_64` long mode first.
- Own the bootloader implementation in-repo.
- Use a freestanding ELF64 kernel image loaded from the EFI System Partition.
- Use QEMU only to execute the same disk and firmware flow that real hardware will use.

## Consequences

- Early bring-up avoids BIOS-era constraints and QEMU-specific shortcuts.
- The loader-to-kernel handoff is fully under repository control.
- The first kernel phase stays deliberately narrow: framebuffer output and halt after `ExitBootServices`.
- Cross-platform image assembly may need host-specific tooling until a repo-local image builder replaces it.

