# Memory Management

This page explains how Lazers memory management is structured today inside the kernel.

The important distinction is:

- `crate::memory` is the stable kernel-facing facade
- the implementation is split internally by responsibility rather than by call site

That keeps the rest of the kernel on one clear API while making the memory code easier to maintain and extend.

## Public Surface Today

The public memory surface is intentionally small.

It owns:

- kernel bootstrap memory initialization
- kernel physical-page allocation for bootstrap subsystems
- user address-space construction
- user ELF loading
- validated user-buffer borrowing for syscalls

Other kernel code should continue to depend on `crate::memory` rather than reaching into internal submodules.

## Internal Layering

The implementation is split into these internal layers:

1. `types`: shared public types, layout constants, and the main error type
2. `state`: the global memory state cell and raw shared fields
3. `allocator`: physical free-range management and page allocation
4. `paging`: page-table construction and mapping helpers
5. `kernel`: boot-time allocator initialization, kernel address-space setup, and shared kernel mappings
6. `loader`: user ELF loading, fixed user layout policy, and startup argument installation
7. `user`: user-buffer validation and borrowing helpers
8. `util`: alignment and decode helpers used across the subsystem

This is still one subsystem. The split is about maintainability and testability, not about introducing separate kernel packages.

## What Is Intentionally Private

The following remain internal implementation details for now:

- the physical allocator types
- the page-table builder
- shared kernel mapping bookkeeping
- startup-argument layout internals

That gives the kernel room to evolve paging and allocation policy without forcing broad API churn across the codebase.

## Host Testability

Host-side library tests now compile the real memory module tree.

The only targeted test seams are:

- page-table activation, which is a no-op in host tests
- boot-only kernel image reservation, which depends on freestanding linker symbols

Pure logic such as allocator behavior, address validation, and startup argument layout can therefore be unit-tested without replacing the full memory module with a stub.
