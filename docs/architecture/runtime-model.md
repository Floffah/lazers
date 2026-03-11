# Runtime Model

This page explains how Lazers runs work after boot has finished and the kernel has taken control of the machine.

At a high level, Lazers currently uses three core runtime concepts:

- a `Process` is the ownership container for a running program
- a `Thread` is the schedulable execution context inside that process
- the `Scheduler` decides which thread runs next

This is still a small runtime, but the basic boundary is already real. User programs do not run as special shell callbacks hidden inside the kernel. They run as processes with their own threads, address spaces, and inherited runtime state.

## Core Concepts

### Process

In Lazers today, a process owns the state that should survive across thread execution and child-process creation.

That includes:

- the process id
- the process address space
- standard streams (`stdin`, `stdout`, and `stderr`)
- the current working directory
- startup argument data
- process lifecycle state

The important idea is that a process owns resources and execution context shared by the program as a whole. This is why cwd and stdio inheritance happen at the process level rather than on individual threads.

### Thread

A thread is the unit the scheduler actually runs.

Each thread owns:

- its saved execution context
- its kernel stack
- its scheduler-visible run state
- a reference to the owning process

Today, Lazers uses one thread per user process. That is a current implementation limit, not the long-term meaning of the type.

### Scheduler

The scheduler owns the runnable set and performs context switches between threads.

The current scheduler is cooperative:

- threads run until they explicitly yield
- threads also stop running when they block or exit
- there is no timer-driven preemption yet

The scheduler also performs the process-level transition required for a switch:

- it activates the next thread's address space
- it updates the kernel trap stack used for that thread
- it restores the saved thread context and resumes execution

## Lifecycle Today

The current runtime lifecycle is intentionally narrow but complete enough to run a real shell and child commands.

### Process Creation

When the kernel launches a user program, it:

1. reads the executable from the mounted root filesystem
2. parses it as an `ELF64` image
3. creates a new process with its own address space
4. installs inherited process state such as stdio and cwd
5. prepares the child startup data, including argv
6. creates one user thread to begin execution at the program entry point

The same model is used for the first user program at boot and for child programs launched later by `lash`.

### Thread Creation

Thread creation currently happens in two main cases:

- the kernel creates service threads, such as the terminal thread
- a user process is given one initial user thread when it is spawned

Each new thread gets its own kernel stack and an initial saved context that allows the scheduler to start it like any other runnable thread.

### Spawn and Wait

Child process launch is currently synchronous.

That means:

- a parent process asks the kernel to launch a child by explicit path
- the child inherits stdio and cwd
- the parent thread blocks until the child exits
- the child exit status is returned to the parent

This is enough for the current shell model, where `lash` launches commands one at a time and waits for each one to finish.

### Exit and Reuse

When a child process exits, Lazers does more than just stop running its thread.

The runtime also:

- records the exit status
- wakes a parent thread that is waiting on that child
- releases the process and thread slot for reuse
- frees the child-owned execution resources

This matters because the current system has a small fixed-capacity runtime. Reuse is what keeps repeated shell command execution from exhausting the process model permanently.

## What The Kernel Owns Today

Process-owned state:

- stdio bindings
- cwd
- argv startup data
- address space
- lifecycle state

Thread-owned state:

- execution context
- kernel stack
- saved scheduler registers
- runnable/blocked/running state

This split is important because it determines where future features belong. For example:

- environment variables should be process-owned
- command-search policy should stay in userland
- thread scheduling policy belongs in the kernel

## What Is Not Implemented Yet

The runtime model is intentionally smaller than a mature desktop or server OS.

The following are explicitly not part of the current design:

- preemptive scheduling
- SMP scheduling
- asynchronous child-process management
- background jobs
- multiple user threads per process as a common case
- generalized process groups or sessions

Those features can be added later, but they should grow from the current process/thread model rather than replace it.

## Why This Matters

This runtime model is the bridge between boot and userland behavior.

It explains why Lazers can already support:

- a real user-mode shell
- cwd inheritance
- argv delivery
- child process spawning
- a userland self-test runner

Even though the system is still early, the execution model is already based on real process and thread boundaries rather than one-off bootstrap shortcuts.
