# Filesystem Layout

## Direction

Lazers should have a filesystem layout that is easy to understand at a glance, avoids historical Unix baggage where it does not help, and still separates boot-critical files from the normal runtime filesystem cleanly.

The runtime root filesystem should describe the machine as the user experiences it, not the firmware boot path. Boot artifacts, firmware fallbacks, and other implementation details should stay out of the normal runtime namespace unless explicitly exposed later for privileged maintenance work.

## Current State

Today the disk layout is intentionally small:

- `LAZERS-ESP`: the EFI System Partition used only for firmware-visible boot artifacts
- `LAZERS-SYSTEM`: the runtime root filesystem mounted as `/`

Current runtime conventions are still bootstrap-oriented:

- the loader lives at `/EFI/BOOT/BOOTX64.EFI` on the ESP
- the kernel image lives at `/lazers/kernel.elf` on the ESP
- normal runtime paths resolve against the `LAZERS-SYSTEM` partition only
- shipped user programs are currently staged under `/bin`
- `lash` resolves bare command names to `/bin/<name>`

This keeps the first system image simple, but it is not the intended long-term hierarchy.

## Intended Runtime Layout

The intended top-level runtime layout is:

- `/system`: operating-system-provided files required for the machine to function
- `/system/bin`: OS-provided commands and programs shipped with Lazers
- `/system/config`: machine-level configuration owned by the operating system
- `/apps`: installed application payloads that are not part of the base system
- `/apps/bin`: user-installed or package-installed executables
- `/home`: user home directories, including `/home/root` instead of a separate `/root`
- `/sys`: a future virtual system interface for live kernel, device, and runtime information

Important implications:

- `/system` is for static OS content, not dynamic runtime state
- `/sys` is reserved for virtual runtime/system exposure and must not be used as a static file store
- `/apps` is the preferred replacement for Unix-style `/usr`
- `/home` is the user-facing data root

## Migration Path

The current `/bin` bootstrap layout should eventually be replaced by `/system/bin`.

The intended sequence is:

1. Keep `/bin` as the bootstrap command location while the early shell and command set are still stabilizing.
2. Add process-owned environment variables and shell-side `PATH` resolution.
3. Change the default shell command search path to prefer `/system/bin` and `/apps/bin`.
4. Move OS-provided binaries from `/bin` to `/system/bin`.
5. Decide whether `/bin` remains as a temporary compatibility alias or disappears entirely.

Kernel responsibilities should stay narrow during that transition:

- the kernel executes explicit paths
- the shell resolves bare command names using policy such as `PATH`
- filesystem layout policy should remain in userspace wherever possible

## Non-Goals

This document does not define:

- package formats or installers
- writable filesystem policy
- logs, caches, or temporary-file locations
- device-file compatibility layers
- whether `/apps` will eventually support versioned package trees

Those decisions should be made when the surrounding runtime and package model are ready.
