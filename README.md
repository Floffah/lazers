# lazers

`lazers` is a from-scratch operating system project built as a single repository with a modular monolithic kernel architecture.

## Principles

- Use `assembly + Rust`, with assembly kept minimal and Rust as the primary systems language.
- Keep the repository as a monorepo so boot code, kernel, shared libraries, userspace, tooling, and documentation evolve together.
- Build a modular monolithic kernel rather than a giant kernel binary with weak boundaries.
- Keep the system practical on modest hardware by preferring lean abstractions and predictable resource costs.
- Design subsystems so they can be included, excluded, or replaced with minimal collateral impact.
- Favor sensible defaults and low-friction setup so a complete system can feel usable without extensive manual configuration.
- Avoid inheriting Unix, DOS, or Windows conventions by default; adopt or reinvent interfaces based on whether they serve the system well.
- Prefer documented architectural decisions over ad hoc conventions.
- Avoid shortcuts that create hidden coupling or deferred cleanup work.

## Top-level layout

- `boot/`: firmware entry, boot flow, and handoff into the kernel
- `kernel/`: core kernel subsystems, resource management, and architecture-specific kernel code
- `user/`: system services, session components, applications, and GUI-facing runtime pieces
- `libs/`: shared crates and libraries that support modular reuse without blurring subsystem boundaries
- `tools/`: local tooling for builds, images, emulation, debugging, and developer workflows
- `docs/`: architecture notes, ADRs, design principles, and repository documentation

## Build entry point

Use `just` as the primary task runner for repository workflows.

Current boot-path recipes:

- `just setup-toolchain`
- `just build-loader`
- `just build-kernel`
- `just image`
- `just run`
- `just check`
- `just clean`

## Direction

The system is intended to be simple in operation, efficient enough for slower machines, modular enough to trim unneeded functionality, and modern enough to avoid cargo-culting legacy operating-system conventions.

That direction should influence every foundational choice: kernel boundaries, subsystem APIs, configuration strategy, packaging, service model, and GUI architecture.

## Current Boot Path

The repository now implements the first bootable path as:

- a UEFI loader built as `BOOTX64.EFI`
- a freestanding `ELF64` kernel image
- a shared boot information contract between the loader and kernel
- a raw GPT disk image with a FAT32 EFI System Partition

The initial success condition is intentionally narrow: the loader exits boot services, the kernel takes control, paints the framebuffer, and halts.

## Host Notes

The current image-assembly script targets the local macOS host tools available in this environment: `qemu-img`, `hdiutil`, and `diskutil`. The boot artifacts and disk layout remain standard UEFI artifacts even though the host-side assembly process is macOS-specific for now.
