# Scheduler and Process Services

This page explains how Lazers divides core scheduling from higher-level
process-facing runtime services today.

The important distinction is:

- the scheduler core owns runnable-thread selection and context switching
- process services build spawn, stdio, cwd, env, and runtime file helpers on top of scheduler state

That keeps the ABI-sensitive execution path small while still exposing one
stable `crate::scheduler` facade to the rest of the kernel.

## Scheduler Core

The scheduler core owns:

- the global runnable thread set
- per-thread kernel stacks and saved contexts
- bootstrap entry into the first runnable thread
- cooperative yield, wait, and exit switching
- address-space activation ahead of every switch
- the trampoline path that starts a kernel or user thread

This is the part of the runtime that is directly coupled to the assembly
context-switch harness and the `ThreadContext` layout.

## Process Services

Process services sit on top of scheduler state and process ownership.

They own:

- process creation helpers used during bootstrap
- synchronous child spawn-and-wait
- top-level process exit policy, including shutdown-on-exit for the bootstrap session owner
- stdio reads and writes through the current process
- cwd lookup and mutation
- environment get, set, unset, and listing
- cwd-relative file and directory access through the runtime root filesystem

These are runtime orchestration concerns, not scheduling policy.

## Ownership Boundaries

The ownership model remains:

- `Process` owns address space, stdio, cwd, env, and child-owned pages
- `Thread` owns saved CPU context, run state, wait state, and kernel stack
- the scheduler decides which thread runs next

That means cwd and stdio inheritance still happen at the process level even
though thread switching is a separate concern.

## Assembly Contract

The scheduler still relies on one small assembly harness for cooperative
context switching.

Important invariants:

- `ThreadContext` layout must remain ABI-stable
- the `context_switch(current, next)` contract must not change
- the initial stack/trampoline setup must still land in the Rust thread entry path

Per the repository rules, the assembly harness stays in a separate file next to
the Rust module that includes it.
