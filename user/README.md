# user

This directory owns userspace programs, long-running services, session components, and graphical system functionality, including the bootstrap `lash` shell and the early utility binaries it launches from `/bin`.

Higher-level behavior should live here whenever it does not need kernel privilege, especially when that keeps the system more modular and easier to evolve.
