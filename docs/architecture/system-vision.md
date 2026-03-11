# System Vision

Lazers is a from-scratch operating system project, but it is not trying to be unusual for its own sake. The goal is to build a system that feels deliberate, understandable, and practical.

## Main Goals

Lazers is being shaped around four core goals:

- work well on modest hardware instead of assuming abundant CPU, memory, or GPU resources
- stay modular so subsystems can be included, removed, or replaced cleanly
- provide sensible defaults so the system feels usable without constant manual setup
- build modern interfaces on purpose instead of inheriting legacy behavior by reflex

These goals affect both code structure and product decisions.

## Engineering Consequences

In practice, those goals push the project toward a few consistent habits:

- prefer simple execution models with visible costs
- keep subsystem APIs explicit so optional features do not become hidden dependencies
- treat the default path as the best-supported path
- reuse older operating-system patterns only when they clearly fit this system
- document major architecture decisions as part of the codebase, not as hidden assumptions

## Architectural Direction

The current direction follows naturally from those goals:

- a modular monolithic kernel is the practical baseline
- higher-level behavior should move into userspace when that makes the system clearer or more replaceable
- shared libraries should exist only where they represent real shared runtime or format logic

This is why Lazers has already grown a real userland shell, disk-backed executables, and a shared user runtime instead of keeping everything inside the kernel.
