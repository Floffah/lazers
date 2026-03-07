# Repository Instructions

## Working Style

- If anything is ambiguous, missing, or internally inconsistent, stop immediately and ask for clarification instead of guessing.
- Prefer clean, maintainable solutions over expedient shortcuts.
- Do not introduce temporary hacks, undocumented workarounds, or hidden coupling with the intent to clean them up later.
- If a compromise is genuinely unavoidable, call it out explicitly, document the tradeoff in the same change, and leave a concrete follow-up path.
- Keep foundational decisions documented as they are made so the codebase does not accumulate silent architectural debt.

## Repository Expectations

- Use `just` as the primary task runner for local workflows.
- Treat this repository as a monorepo.
- Treat the kernel architecture as a modular monolith unless an explicit architectural decision changes that direction.
- Keep documentation close to major structural decisions. Use `docs/adr/` for architecture decisions and the nearest relevant `README.md` for local context.
- Favor small, explicit module boundaries and coherent naming from the beginning rather than relying on later cleanup passes.
- Preserve the core product direction in implementation decisions: lean enough for modest hardware, modular enough to remove unneeded subsystems cleanly, and usable with sensible defaults rather than constant manual configuration.
- Do not import Unix, DOS, or Windows conventions by reflex. Reuse an inherited pattern only when it is a deliberate fit for this system.
