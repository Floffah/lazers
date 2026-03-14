# Cleanup Review

This document captures repository cleanup opportunities that should not change functionality. It is based on a repo-wide review plus a successful `just check` run.

## Findings

### ~~P1: Split the storage stack into submodules~~

~~The storage stack is carrying too many layers in one file. AHCI MMIO, block IO, GPT parsing, FAT32 traversal, root-fs state, and cwd-relative path normalization all live together in [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L1), [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L170), [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L209), [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L521), [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L650), and [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L1171). It works, but it is the clearest candidate for splitting into `ahci`, `gpt`, `fat32`, `path`, and `rootfs` submodules.~~

~~### P1: Split memory management into clearer units~~

~~Memory management is similarly overloaded. Early boot init, kernel mappings, ELF user-program loading, allocator internals, page-table construction, and user-page copy helpers are all bundled into [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L177), [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L268), [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L539), [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L692), [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L914), and [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L1021). This is the second strongest refactor target for maintainability.~~

~~### P1: Separate scheduler core from process services~~

~~The scheduler module mixes core scheduling with process services and filesystem/environment syscalls. Child spawning, stdio access, cwd/env mutation, and directory/file reads sit beside context-switch and runnable-queue logic in [scheduler.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/scheduler.rs#L138), [scheduler.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/scheduler.rs#L296), [scheduler.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/scheduler.rs#L383), and [scheduler.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/scheduler.rs#L516). A thin process-services layer would make the scheduler itself much leaner.~~

~~### P2: Break `lash` into smaller shell components~~

~~`lash` has grown past “small bootstrap shell” territory. REPL, built-ins, PATH search, child execution, and its own path canonicalization are all in [main.rs](/Users/ramsay/Documents/Projects/fun/lazers/user/lash/src/main.rs#L31), [main.rs](/Users/ramsay/Documents/Projects/fun/lazers/user/lash/src/main.rs#L158), [main.rs](/Users/ramsay/Documents/Projects/fun/lazers/user/lash/src/main.rs#L320), and [main.rs](/Users/ramsay/Documents/Projects/fun/lazers/user/lash/src/main.rs#L483). The duplicated path normalization logic against [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L170) is the main cleanup opportunity here.~~

~~### P2: Split `liblazer` into focused modules~~

~~`liblazer` is a single-file runtime that now bundles entry glue, raw syscalls, typed wrappers, env/fs/process APIs, formatting macros, panic behavior, and argv startup parsing in [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/liblazer/src/lib.rs#L1), [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/liblazer/src/lib.rs#L127), [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/liblazer/src/lib.rs#L194), [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/liblazer/src/lib.rs#L228), [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/liblazer/src/lib.rs#L384), and [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/liblazer/src/lib.rs#L479). Splitting this into small modules plus a result-decoding helper or macro would remove a lot of repetition.~~

### P2: Deduplicate low-level utility helpers

Low-level utility logic is duplicated across crates. `align_up` and `align_down` appear in [main.rs](/Users/ramsay/Documents/Projects/fun/lazers/boot/uefi-loader/src/main.rs#L229), [memory.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/memory.rs#L1110), and [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L1159). Little-endian readers appear in [lib.rs](/Users/ramsay/Documents/Projects/fun/lazers/libs/elf/src/lib.rs#L211) and [storage.rs](/Users/ramsay/Documents/Projects/fun/lazers/kernel/kernel/src/storage.rs#L1147). A tiny shared internal utility module would reduce drift.

### P3: Consolidate build and image-assembly discovery logic

Build and discovery logic is repeated. User-package enumeration is duplicated in [justfile](/Users/ramsay/Documents/Projects/fun/lazers/justfile#L19) and [justfile](/Users/ramsay/Documents/Projects/fun/lazers/justfile#L51), then rediscovered again in [build-image.sh](/Users/ramsay/Documents/Projects/fun/lazers/tools/scripts/build-image.sh#L29). Consolidating that logic would reduce maintenance overhead.

### P3: Centralize or document naming mismatches in tooling

Docs consistently talk about `/system/bin` and GPT labels `LAZERS-ESP` / `LAZERS-SYSTEM`, while the image builder stages into `/SYSTEM/BIN` and mounts `LAZERSESP` / `LAZERSSYS` in [build-image.sh](/Users/ramsay/Documents/Projects/fun/lazers/tools/scripts/build-image.sh#L8), [build-image.sh](/Users/ramsay/Documents/Projects/fun/lazers/tools/scripts/build-image.sh#L96), and [patch_gpt_layout.py](/Users/ramsay/Documents/Projects/fun/lazers/tools/scripts/patch_gpt_layout.py#L11). This may be intentional for FAT and macOS, but it should be centralized or documented once instead of being implicit.

### P3: Clean up repository hygiene and docs drift

Repository hygiene has a couple of obvious cleanup items. `.gitignore` still contains a generator error marker and ignores `Cargo.lock` using library-oriented boilerplate in [.gitignore](/Users/ramsay/Documents/Projects/fun/lazers/.gitignore#L298) and [.gitignore](/Users/ramsay/Documents/Projects/fun/lazers/.gitignore#L308). Also, the local docs guidance is much thinner than the repo’s own documentation standard in [docs/README.md](/Users/ramsay/Documents/Projects/fun/lazers/docs/README.md#L1).

## Validation

`just check` passed during the review.
