# Repository Structure

Lazers is developed as a monorepo. Boot code, kernel code, shared libraries, userspace programs, tooling, and architecture docs all live together so that cross-cutting changes stay visible and interfaces evolve in one place.

This is important for an operating system project: a change to boot, memory, process startup, executable loading, or userspace runtime often spans several layers at once.

## Top-Level Layout

The repository is split into a few clear areas:

- `boot/`: firmware entry and bootloader code
- `kernel/`: the kernel itself, including architecture support, memory, storage, scheduling, and syscalls
- `libs/`: shared crates used across boot, kernel, and userspace when that reuse is deliberate and real
- `user/`: user programs and future higher-level services
- `tools/`: image-building, emulation, and developer workflows
- `docs/`: documentation site content and architecture reference material

## Architectural Boundaries

The repository structure reflects the intended subsystem boundaries:

- the UEFI loader owns firmware interaction and the kernel handoff contract
- the kernel owns paging, process/thread lifecycle, storage discovery, and syscall dispatch
- `liblazer` provides the early shared runtime for user programs
- user binaries are normal disk-backed ELF executables, not special built-ins hidden inside the kernel

This does not mean the boundaries are frozen. Lazers is intentionally being built as a modular monolith, so higher-level behavior can move into userspace when that improves modularity or replaceability.

## What Contributors Should Expect

The easiest way to navigate a change is to ask which layer owns the behavior:

- firmware handoff or boot artifact loading belongs in `boot/`
- privileged runtime mechanisms belong in `kernel/`
- cross-userland runtime support belongs in `libs/`
- shell commands and higher-level behavior belong in `user/`
- build and run workflows belong in `tools/`

That rule keeps ownership clearer than spreading policy across the kernel by accident.

## Documentation Rule

Architecture documents in `docs/architecture/` should describe the system as it exists now and the direction it is intentionally heading. They are not meant to duplicate Git history or preserve milestone-by-milestone narrative.
