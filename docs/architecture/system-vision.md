# System Vision

## Goals

- Stay practical on modest hardware rather than assuming abundant CPU, memory, or GPU resources.
- Keep the system modular so subsystems can be included, removed, or replaced without widespread breakage.
- Provide sensible defaults so a full system can feel complete without constant manual configuration.
- Build modern interfaces on purpose instead of inheriting Unix, DOS, or Windows behavior by default.

## Engineering Implications

- Prefer simple execution models and predictable resource usage over abstraction layers that obscure cost.
- Design subsystem APIs so optional features do not become hard dependencies accidentally.
- Make the default path the well-supported path. Configuration should extend behavior, not rescue it.
- Treat compatibility patterns from existing operating systems as options to evaluate, not templates to copy.
- Document the rationale behind foundational architecture choices as they are made.

## Architectural Direction

- A modular monolithic kernel provides a practical baseline while keeping internal boundaries explicit.
- Userspace should absorb higher-level services whenever doing so improves modularity, replaceability, or system clarity.
- Shared libraries should exist to support real reuse, not to hide weak ownership or avoid making an interface decision.
