# ADR 0003: User-Mode Bring-Up Uses Embedded ELF Bytes First

## Status

Accepted

## Context

The first usable shell for `lazers` is intended to be a replaceable userland program, not kernel logic. That means the kernel needs a real process/thread/address-space boundary before it makes sense to build `lash`.

At this stage, the system does not yet have a filesystem, path lookup, or disk-backed executable loading. Waiting for all of that before introducing user mode would delay the kernel/runtime boundary, while implementing a kernel-hosted bootstrap shell would create a temporary execution model that the project already knows it wants to replace.

## Decision

The first user-mode milestone uses:

- one embedded ELF user program linked into the kernel image as raw bytes
- one shared ELF parser crate reused by the loader and the kernel
- one real user process with its own address space and one user thread
- one minimal syscall ABI based on `int 0x80`
- one terminal/stdio path that is shared between kernel service threads and user programs

Only the source of the executable bytes is embedded. Process creation, ELF parsing, address-space setup, thread startup, and stdio attachment are the same categories of work that later disk-backed program loading will reuse.

## Consequences

- The first user-mode text program can be brought up without inventing a throwaway kernel-shell architecture.
- Future work can replace the embedded byte source with disk-backed loading without changing the fundamental process startup path.
- The first shell can be introduced as a real userland executable later, rather than as a privileged kernel special case.
- `int 0x80` is accepted as a bring-up syscall mechanism for now; a later architectural decision can replace it if a different syscall entry path is a better fit.
