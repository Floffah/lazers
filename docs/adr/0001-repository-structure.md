# ADR 0001: Repository Structure

## Status

Accepted

## Context

The project is starting from an empty repository and needs an initial structure that supports low-level boot work, kernel development, shared libraries, future userspace components, and project documentation.

Because this is a from-scratch operating system, early repository choices will strongly influence build tooling, refactoring cost, and architectural clarity.

## Decision

- Use a monorepo.
- Use a modular monolithic kernel architecture.
- Use `just` as the top-level task runner for local workflows.
- Establish the documentation structure before adding implementation code.

## Consequences

- Cross-cutting refactors remain easier because related code lives in one repository.
- Build and image-generation workflows can be centralized instead of coordinated across multiple repositories.
- Kernel boundaries still need to be designed deliberately; choosing a monolithic kernel does not justify weak internal structure.
- Architectural reasoning is expected to be documented as the codebase evolves.
