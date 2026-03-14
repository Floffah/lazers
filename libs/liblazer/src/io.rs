use kernel_abi::Syscall;

use crate::syscalls::syscall3;

/// Reads bytes from a process-owned descriptor into the provided buffer.
pub fn read(fd: usize, buffer: &mut [u8]) -> usize {
    syscall3(
        Syscall::Read,
        fd,
        buffer.as_mut_ptr() as usize,
        buffer.len(),
    )
}

/// Writes bytes to a process-owned descriptor from the provided buffer.
pub fn write(fd: usize, buffer: &[u8]) -> usize {
    syscall3(Syscall::Write, fd, buffer.as_ptr() as usize, buffer.len())
}

/// Reads from the current process' standard input stream.
pub fn stdin_read(buffer: &mut [u8]) -> usize {
    read(0, buffer)
}

/// Writes to the current process' standard output stream.
pub fn stdout_write(buffer: &[u8]) -> usize {
    write(1, buffer)
}

/// Writes to the current process' standard error stream.
pub fn stderr_write(buffer: &[u8]) -> usize {
    write(2, buffer)
}
