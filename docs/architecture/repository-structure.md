# Repository Structure

## Direction

The repository is a monorepo for the whole operating system stack. Boot code, kernel code, shared libraries, userspace programs, tooling, and architecture documentation all evolve together so interface changes stay visible and cross-cutting refactors stay cheap.

The kernel direction is a modular monolith. Kernel code lives in one overall system image, but subsystem boundaries should stay explicit and small enough that services, libraries, and higher-level behavior can move out to userspace when that improves clarity or replaceability.

## Current Layout

- `boot/`: firmware entry and bootloader code
- `kernel/`: kernel subsystems, low-level architecture support, memory, storage, runtime, and syscall handling
- `libs/`: shared crates used across boot, kernel, and userspace where reuse is real and deliberate
- `user/`: userspace programs and future higher-level services
- `tools/`: build, image, and emulation workflows
- `docs/`: current-state architecture and repository documentation

## Current Module Boundaries

- The UEFI loader owns firmware interaction and the kernel handoff contract only.
- The kernel owns paging, scheduling, storage, process/thread lifecycle, and syscall dispatch.
- `liblazer` owns the bootstrap userland runtime surface for early user programs.
- User binaries like `lash`, `echo`, and `ls` are normal disk-backed ELF programs staged under `/bin`.

## Documentation Rule

Documentation under `docs/architecture/` should describe the current structure and current intended direction. Git already preserves how the design changed over time, so the repository documentation should optimize for understanding how the system works now.
