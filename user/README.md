# user

This directory owns userspace programs, long-running services, session components, and graphical system functionality, including the bootstrap `lash` shell, the early utility binaries it launches from `/bin`, and the first in-OS self-test runner `selftest`.

Higher-level behavior should live here whenever it does not need kernel privilege, especially when that keeps the system more modular and easier to evolve.
