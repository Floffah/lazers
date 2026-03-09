# Lazers

Lazers is a from-scratch operating system project so I can learn about OS design and implementation by building one myself. This is a learning & portfolio project.
If this were ever to be finished it would stand as a practical, modular, modern OS that can run on real hardware, and is specifically for lazy developers (laz-ers) who want to write code not configure their environment.

Thought I'd make it public in case anyone else finds it interesting or useful, but it's really just a personal learning project and not intended for anyone else's use. The code is MIT licensed, so if you want to copy anything, go for it.

## Principles

- Lazers is primarily written in Rust with a small amount of assembly harnesses.
- Structured as a monorepo, but boot, kernel, and userspace code are separate, but may share libraries when it makes sense.
  - The kernel specifically is a modular monolith (or aims to be), but userspace is on its own and can evolve separately.
  - Structure is self-explanatory
- Avoiding UNIX/DOS heritage where possible, but not dogmatically. If a convention serves the system well, it can be adopted; if it doesn't, it can be reinvented.
- Low coupling where possible and minimal shortcuts

## Build entry point

Use `just` as the primary task runner for repository workflows.

Important tasks include:
- `just setup-toolchain` - install Rust toolchain components and target specifications
- `just run` - Builds everything, assembles the disk image, and runs it in QEMU with GUI
- `just run-headless` - Same as `just run` but runs QEMU in headless mode and saves a screenshot of the framebuffer output to `build/qemu-headless.png` hopefully after boot (for debugging kernel really)
- `just check` - runs a monorepo wide `cargo check` 
- `just clean` - cleans build artifacts across the monorepo

Other tasks for debugging and development include:
- `just build-loader` - builds the UEFI loader only
- `just build-kernel` - builds the kernel only
- `just build-user` - builds the user binaries only
- `just image` - assembles the disk image from the boot, kernel, and user artifacts

## Development

Scripting and tooling currently only supports macOS, but the image will run anywhere QEMU x86 is supported.
