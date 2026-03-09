# Architecture

This directory is for longer-form design documents about the operating system structure, subsystem boundaries, boot flow, memory model, service model, and GUI architecture.

Start with documentation here when a change introduces new architecture. These documents should describe the current system shape and the current intended direction, rather than preserving milestone-by-milestone history.

Current focus areas include:

- design principles for a modern, non-legacy-first operating system
- repository and subsystem structure
- modular subsystem boundaries
- boot, disk, and executable-loading flow
- runtime filesystem layout and namespace direction
- low-friction configuration and service behavior
- performance and simplicity on modest hardware
- terminal/runtime boundaries that prepare for a future userland shell
