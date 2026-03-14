# Text Runtime

The Lazers text runtime is built around one core rule: interactive programs should use standard streams, not direct access to the framebuffer or keyboard.

That keeps shell behavior in userland, keeps hardware handling in the kernel, and makes it possible for different kinds of text sessions to share the same runtime model later.

## The Model

The intended shape is:

- input drivers produce key events
- a terminal surface owns the visible text session
- a terminal endpoint carries byte-stream input and output for that session
- processes talk through `stdin`, `stdout`, and `stderr`
- the shell is just another user process attached to that endpoint

This is the main architectural boundary that keeps Lazers from turning into “the kernel has a shell built into it.”

## How It Works Today

The current system is still small, but the boundary is already real:

- one fullscreen terminal surface renders text to the framebuffer
- one terminal endpoint carries byte-stream input and output
- one kernel thread owns keyboard polling and screen flushing
- one user shell process is loaded from disk and talks only through stdio
- child processes inherit stdio and run synchronously through spawn-and-wait

The result is that commands like `lash`, `echo`, `ls`, `cat`, and `pwd` are all normal disk-backed user programs. They do not own the screen, and they do not receive hardware key events directly.

## Userland Runtime Surface

Early user programs share one bootstrap runtime crate: `liblazer`.

Today, `liblazer` provides:

- process startup glue
- syscall shims
- a private runtime layer for argv and panic behavior
- panic-to-exit behavior
- basic stdio helpers
- `args()`
- cwd helpers
- simple file and directory access helpers

User programs still run with `no_std` and `no_main`, but they already use a common runtime surface instead of each binary defining its own bootstrap path.

## Shell Behavior Today

`lash` is still intentionally small, but it already proves most of the text-runtime model:

- prompt and line editing live in userland
- built-ins like `cd` and `exit` affect the shell process itself
- external commands are launched as child processes
- cwd and argv are process-level runtime concepts, not shell-only hacks
- shell syntax stays in `lash`; the kernel only executes explicit paths

This is important for maintainability: the shell is policy, while the kernel provides mechanisms.

## Why This Matters

This model is the foundation for several future directions:

- selecting a different default shell
- non-interactive shell execution through `lash -c`
- future SSH-backed text sessions
- future GUI terminal windows
- richer userspace programs that still rely on the same stdio model

The details will evolve, but the main boundary should stay the same: hardware and session plumbing in the kernel, text-program behavior in userspace.
