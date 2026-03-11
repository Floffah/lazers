# Filesystem Layout

This page describes two related things:

- how the Lazers disk is laid out today
- what the long-term runtime filesystem should look like

The important rule is that the runtime filesystem should reflect how the machine is used, not how the firmware happens to boot it.

## Disk Layout Today

The current disk image is intentionally small and easy to reason about:

- `LAZERS-ESP`: the EFI System Partition used only for boot artifacts
- `LAZERS-SYSTEM`: the runtime root filesystem mounted as `/`

The ESP exists for firmware and the loader. The runtime system exists for the kernel and userspace.

Today that means:

- the loader lives at `/EFI/BOOT/BOOTX64.EFI` on the ESP
- the kernel image lives at `/lazers/kernel.elf` on the ESP
- normal runtime paths resolve only against `LAZERS-SYSTEM`
- shipped commands are currently staged under `/bin`
- `lash` resolves bare command names to `/bin/<name>`

This is a practical bootstrap layout, not the intended final namespace.

## Runtime Layout Direction

The intended top-level runtime layout is:

- `/system`: operating-system-provided files required for the machine to function
- `/system/bin`: OS-provided commands and programs shipped with Lazers
- `/system/config`: machine-level configuration owned by the operating system
- `/apps`: installed application payloads that are not part of the base system
- `/apps/bin`: user-installed or package-installed executables
- `/home`: user home directories, including `/home/root` instead of a separate `/root`
- `/sys`: a future virtual system interface for live kernel, device, and runtime information

The main ideas behind this layout are:

- `/system` is for static operating-system content
- `/apps` is for installed application content
- `/home` is for user data
- `/sys` is reserved for live system information, not static files

This is why Lazers does not plan to use `/sys` as a store for built-in binaries. That name is more useful for a future virtual runtime interface.

## What This Means For The Kernel

The kernel should stay narrow here:

- it mounts the runtime root filesystem
- it resolves explicit paths
- it loads executables from those explicit paths

The kernel should not own command-search policy. That belongs in userspace, especially once environment variables and `PATH` exist.

## Migration From `/bin`

The current `/bin` layout is a bootstrap convenience. The intended direction is to move OS-provided commands into `/system/bin`.

The planned sequence is:

1. keep `/bin` while the early shell and command set are still stabilizing
2. add inherited environment variables and shell-side `PATH`
3. make the shell prefer `/system/bin` and `/apps/bin`
4. move shipped binaries from `/bin` to `/system/bin`
5. decide whether `/bin` remains as a temporary compatibility path or disappears

## Intentionally Deferred

This page does not define:

- package formats
- writable filesystem policy
- logs, caches, or temporary-file locations
- device-file compatibility layers
- versioned application trees under `/apps`

Those choices should be made when the surrounding package and storage model is ready.
