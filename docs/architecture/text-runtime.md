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

The first kernel-hosted text runtime keeps the model intentionally small:

- one fullscreen terminal surface
- one terminal endpoint
- one kernel-hosted text task attached through explicit stdio handles
- one framebuffer text renderer used as the terminal surface backend

This is a kernel bring-up step, not the final ownership model. The important constraint is that the text task already uses stdio-like handles, so a future userland shell can replace it without redesigning the terminal boundary.

## Future Implications

- Child processes should inherit stdio handles by default unless spawn-time overrides replace them.
- A different default shell should be selectable by system configuration once userspace and session management exist.
- SSH sessions, local HDMI text sessions, and future GUI terminal windows should all reuse the same endpoint and stdio model rather than inventing separate shell-facing interfaces.
