---
title: Architecture
---

# Architecture

This section explains how Lazers is structured today and what major design choices shape the system.

If you are new to the project, the most useful reading order is:

1. [System Vision](/architecture/system-vision)
2. [Boot Process](/architecture/boot-process)
3. [Runtime Model](/architecture/runtime-model)
4. [User and Kernel Interface](/architecture/user-kernel-interface)
5. [Text Runtime](/architecture/text-runtime)
6. [Storage and Loading](/architecture/storage-and-loading)
7. [Filesystem Layout](/architecture/filesystem-layout)
8. [Repository Structure](/architecture/repository-structure)

Together, those pages answer the most common architecture questions:

- how the machine boots
- how the runtime execution model works after boot
- how user programs talk to the kernel
- how terminal and shell behavior are split across kernel and userspace
- how the system finds files and executables at runtime
- how the runtime filesystem is intended to evolve
- where code belongs in the repository
