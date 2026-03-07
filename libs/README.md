# libs

This directory owns shared libraries and common crates used across the workspace.

The first shared crate defines the boot-time handoff contract used by both the UEFI loader and the kernel. Only place code here when it represents a real shared abstraction with clear ownership, stable boundaries, and a justified dependency footprint.
