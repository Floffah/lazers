# libs

This directory owns shared libraries and common crates used across the workspace.

Current shared foundations include:

- `boot-info` for the loader-to-kernel handoff contract
- `elf` for shared executable parsing
- `liblazer` for the first userland runtime surface shared by early user programs

Only place code here when it represents a real shared abstraction with clear ownership, stable boundaries, and a justified dependency footprint.
