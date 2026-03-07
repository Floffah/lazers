# boot

This directory owns firmware entry, boot-path coordination, early platform initialization, and the handoff contract into the kernel.

Keep this area narrowly scoped so the transition into the kernel stays explicit, portable, and easy to reason about.
