# Text Runtime

## Direction

Interactive text programs should speak through process-style standard streams rather than talking to the framebuffer or keyboard hardware directly.

The long-term model is:

- input drivers produce key events
- a terminal surface owns visible text rendering for a session
- a terminal endpoint carries byte-stream input and output for that session
- programs attach through `stdin`, `stdout`, and `stderr`
- the shell is a future userland program attached to those standard streams

## Current Foundation

The current text runtime keeps the model intentionally small, but it now sits on top of the first real kernel and user execution boundary:

- one fullscreen terminal surface
- one terminal endpoint
- one kernel system process that owns terminal-service work
- one user echo process loaded from `/bin/echo` on the system partition
- process-owned stdio bound through a process-owned handle table
- one kernel terminal thread that handles keyboard polling and terminal flushing
- one user thread that reads and writes through `stdin`/`stdout` syscalls
- one cooperative scheduler with a separate idle thread

This is still a bring-up step, not the final shell/session model. The important constraint is that text-program logic now runs as a real disk-backed user process on top of stdio-backed handles, and early user programs share one `liblazer` runtime crate for startup, panic-to-exit behavior, syscall wrappers, and minimal stdio helpers. A future userland shell should build on that same runtime surface rather than redefining its own bootstrap glue.

## Future Implications

- Child processes should inherit stdio handles by default unless spawn-time overrides replace them.
- A different default shell should be selectable by system configuration once userspace and session management exist.
- SSH sessions, local HDMI text sessions, and future GUI terminal windows should all reuse the same endpoint and stdio model rather than inventing separate shell-facing interfaces.
