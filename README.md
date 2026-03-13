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

## Checkpoints

This is the high-level roadmap for Lazers as it exists now. It tracks major capability checkpoints rather than every version bump.

### Complete

- [x] Monorepo workspace with separate boot, kernel, shared library, userland, tooling, and architecture areas
- [x] Real `UEFI -> loader -> kernel` boot path on `x86_64`
- [x] Shared `BootInfo` handoff from loader to kernel
- [x] Kernel-owned framebuffer text output
- [x] Keyboard input reaching the running system
- [x] Terminal-style text runtime with process `stdin`, `stdout`, and `stderr`
- [x] Cooperative kernel scheduler with explicit `Process` and `Thread` models
- [x] First real user-mode boundary with syscall handling
- [x] Kernel-owned paging and per-process address spaces
- [x] Shared `ELF` parsing/loading path for boot and runtime executable loading
- [x] Raw GPT disk image with separate `LAZERS-ESP` and `LAZERS-SYSTEM` partitions
- [x] AHCI/SATA disk access in the kernel
- [x] Runtime root filesystem mounted from the system partition
- [x] Disk-backed user executable loading from `/system/bin/...`
- [x] `liblazer` bootstrap runtime for early userland programs
- [x] User-initiated child process spawn and synchronous wait
- [x] First shell as a normal userland program: `lash`
- [x] First external user commands: `echo`, `ls`, and `pwd`
- [x] Process-owned current working directory with inherited cwd on spawn
- [x] First shell built-ins that must affect the shell process itself: `cd` and `exit`
- [x] Userland `argv` exposed through `liblazer::args()`
- [x] Read-only file access from userland beyond directory listing and first file-content command: `cat`
- [ ] Better shell command parsing beyond split-on-space tokenization
- [x] First in-OS status-based userland self-test command: `selftest`
- [ ] Serial console / serial logging support
  - [ ] ^ so in-OS selftest results can be captured reliably on the host
- [ ] Richer command argument support across userland programs
- [ ] More core commands beyond the bootstrap set
- [ ] Better shell/session policy for top-level `lash` exit and eventual halt/shutdown behavior
- [ ] Filesystem write support
- [ ] VFAT long-name support or a deliberate replacement strategy
- [ ] A fuller userland process model beyond synchronous spawn-and-wait
- [ ] Timer-driven preemption
- [ ] SMP / multicore support
- [ ] A more complete userland runtime beyond early `liblazer`
- [ ] A real package of day-to-day user programs beyond the current bootstrap set
- [ ] A real installer/update path for writing Lazers onto target hardware
- [ ] Broader boot/runtime support outside the current `UEFI x86_64` path
- [ ] Long-term filesystem direction beyond the current FAT32-first runtime setup
- [ ] Far future higher-level graphics and windowing stack beyond the text terminal

## Build entry point

Use `just` as the primary task runner for repository workflows.

Important tasks include:
- `just setup-toolchain` - install Rust toolchain components and target specifications
- `just run` - Builds everything, assembles the disk image, and runs it in QEMU with GUI
- `just run-headless` - Same as `just run` but runs QEMU in headless mode and saves a screenshot of the framebuffer output to `build/qemu-headless.png` hopefully after boot (for debugging kernel really)
- `just run-selftest` - Boots the kernel into self-test mode, via the selftest binary.
- `just run-selftest-headless` - Same as `just run-selftest` but captures a headless screenshot
- Future improvement: headless selftest should also emit host-readable serial output so a later `just full-test` target can fail automatically when in-OS tests fail.
- `just check` - runs a monorepo wide `cargo check` 
- `just test` - runs a monorepo wide `cargo test`
- `just clean` - cleans build artifacts across the monorepo

Other tasks for debugging and development include:
- `just build-loader` - builds the UEFI loader only
- `just build-kernel` - builds the kernel only
- `just build-user` - builds the user binaries only
- `just image` - assembles the disk image from the boot, kernel, and user artifacts
- `just image-selftest` - assembles a separate image that launches `/system/bin/selftest` as the first user program

## Running

If running on macOS, make sure you have qemu (homebrew) and rust installed with the correct toolchain (`just setup-toolchain`).
Then all you need to do is run `just run` and everything will be built and you'll see the QEMU window pop up running lazers.

For actual hardware, I've not tested this, but it's probably possible.
Likely all it involves is building it on macOS of course, running `just image`, and then flashing the resulting `build/lazers.img` to a disk drive or USB and booting from that on UEFI (x86) hardware.

## Development

Scripting and tooling currently only supports macOS, but the image will run anywhere QEMU x86 is supported.
