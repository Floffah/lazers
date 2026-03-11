# Storage and Loading

This page explains how Lazers finds disks, mounts its runtime filesystem, and loads user programs after the kernel has taken over.

The important distinction is:

- the loader is only responsible for boot artifacts
- the kernel is responsible for runtime storage

That keeps boot small and keeps normal program loading inside the real operating system rather than inside firmware scaffolding.

## Storage Path Today

Once the kernel is running, the storage path is:

`AHCI/SATA -> GPT -> FAT32 -> ELF`

In practical terms, that means:

1. the kernel discovers a usable AHCI/SATA-backed disk
2. it reads the GPT partition table
3. it identifies the runtime system partition
4. it mounts that partition as the root filesystem at `/`
5. it resolves files and executables from that mounted filesystem

This is the same path used for the first user program at boot and for child programs launched later by `lash`.

## Boot Partition vs Runtime Partition

Lazers currently uses two partitions with different jobs.

### `LAZERS-ESP`

This is the EFI System Partition.

It exists for:

- firmware boot discovery
- the UEFI loader
- the kernel image itself

Today it contains:

- `/EFI/BOOT/BOOTX64.EFI`
- `/lazers/kernel.elf`

The runtime system does not treat this partition as its normal root filesystem.

### `LAZERS-SYSTEM`

This is the runtime system partition.

It exists for:

- the mounted root filesystem
- shipped user programs
- files that userland accesses through runtime syscalls

Today it is mounted as `/`, and normal runtime paths are always resolved against it.

## Filesystem Model Today

The current runtime filesystem is intentionally simple.

Important constraints:

- the filesystem is FAT32
- short names are the current truth
- the root filesystem is mounted as `/`
- the runtime path is read-only today

That means Lazers can already:

- load user programs from disk
- list directories
- read file contents
- resolve cwd-relative and absolute paths

But it does not yet support:

- writes
- file descriptors
- a general VFS layer
- richer filesystem metadata
- package installation logic

## How Executable Loading Works

User programs are normal disk-backed ELF executables.

The runtime load path is:

1. resolve the requested path against the mounted root filesystem
2. read the file from FAT32
3. parse it as `ELF64`
4. create a process and address space
5. map the program image into user memory
6. prepare argv and user stack state
7. create the initial user thread

This same model is used for:

- the first user program chosen at boot
- commands launched by `lash`
- the userland `selftest` runner

The kernel does not special-case shell commands as built-ins hidden in privileged code. They are loaded through the same disk-backed executable path as other user programs.

## How Current Commands Depend On This Path

The existing command set already depends on the runtime storage stack.

- `lash` is loaded from disk as the default shell
- `ls` resolves a directory and lists it through the mounted root filesystem
- `cat` resolves a file path and reads file contents through the runtime filesystem
- `pwd` depends on cwd state, which is meaningful only because the runtime filesystem is mounted as `/`
- `selftest` can validate cwd and executable loading using the same runtime path

That is why the storage path is a core part of the OS architecture, not just a filesystem implementation detail.

## What Contributors Should Keep In Mind

The current storage model is deliberately narrow:

- boot and runtime storage are separate concerns
- the root filesystem is the only normal runtime namespace
- the kernel resolves explicit paths; it does not own shell command-search policy
- user programs are loaded through the same path they use to access other runtime files

This matters when changing higher-level behavior. If a feature sounds like shell policy, it probably should not be pushed into the kernel storage layer.

## What This Page Does Not Cover

This page describes the current read-only runtime storage model only.

It does not define:

- writable filesystems
- open/read/close file descriptor APIs
- package management
- `/system` vs `/apps` migration details beyond current direction
- multiple mounted runtime filesystems
- output capture or pipe-backed I/O
