# Text Runtime

## Direction

Interactive text programs should speak through process-style standard streams rather than talking to the framebuffer or keyboard hardware directly.

The long-term model is:

- input drivers produce key events
- a terminal surface owns visible text rendering for a session
- a terminal endpoint carries byte-stream input and output for that session
- programs attach through `stdin`, `stdout`, and `stderr`
- the default shell `lash` runs as a normal user process on top of that terminal endpoint and can launch other programs as child processes

## Current Foundation

The current text runtime keeps the model intentionally small, but it now sits on top of the first real kernel and user execution boundary:

- one fullscreen terminal surface
- one terminal endpoint
- one kernel system process that owns terminal-service work
- one user shell process loaded from `/bin/lash` on the system partition
- process-owned stdio bound through a process-owned handle table
- one kernel terminal thread that handles keyboard polling and terminal flushing
- one user thread that reads and writes through `stdin`/`stdout` syscalls
- one synchronous child-process spawn-and-wait path for future shell-launched programs
- one narrow read-only directory-listing syscall used by `/bin/ls`
- one cooperative scheduler with a separate idle thread

This is still a bring-up step, not the final shell/session model. The important constraint is that text-program logic now runs as a real disk-backed user process on top of stdio-backed handles, and early user programs share one `liblazer` runtime crate for startup, panic-to-exit behavior, syscall wrappers, and minimal stdio helpers. A future userland shell should build on that same runtime surface rather than redefining its own bootstrap glue.

The first shell-facing filesystem command path is also now present: `lash` can launch `/bin/ls`, and `ls` uses a narrow read-only directory-listing syscall rather than any shell built-in behavior.

## Current Userspace Model

- `liblazer` is the shared userland bootstrap crate for early programs.
- User binaries still use `no_std` and `no_main`, with `liblazer` owning `_start`, syscall shims, panic-to-exit behavior, and basic stdio helpers.
- `lash` is the first shell, but it is intentionally minimal:
  - prompt and line editing live in userland
  - command execution is synchronous
  - bare command names resolve to `/bin/<name>`
  - built-ins, argv, cwd, and PATH-like lookup are still out of scope
- `echo` and `ls` are the current example utility programs that prove process launch and root-filesystem access from userland.

## Future Implications

- Child processes should inherit stdio handles by default unless spawn-time overrides replace them.
- A different default shell should be selectable by system configuration once userspace and session management exist.
- SSH sessions, local HDMI text sessions, and future GUI terminal windows should all reuse the same endpoint and stdio model rather than inventing separate shell-facing interfaces.
