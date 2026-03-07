# kernel

This directory owns the kernel proper, organized as explicit subsystems rather than a single undifferentiated code blob.

The first kernel target is a freestanding `x86_64` ELF image entered by the UEFI loader after `ExitBootServices`. Architecture-specific code should remain clearly separated from portable kernel logic, and subsystem boundaries should support clean inclusion, exclusion, and replacement where practical.
