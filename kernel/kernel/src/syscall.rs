//! Minimal syscall dispatch for the bootstrap user-mode runtime.
//!
//! The ABI is intentionally tiny: stdio-backed `read`/`write`, cooperative
//! `yield`, and `exit`. This is enough to validate the first user process
//! without prematurely designing a wider kernel API surface.

use crate::arch::TrapFrame;
use crate::memory;

const SYS_READ: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_YIELD: u64 = 2;
const SYS_EXIT: u64 = 3;
const SYS_SPAWN_WAIT: u64 = 4;

/// Dispatches a syscall trap frame in place.
///
/// The user-mode calling convention places the syscall number in `rax` and the
/// first three arguments in `rdi`, `rsi`, and `rdx`.
pub fn dispatch(frame: &mut TrapFrame) {
    match frame.rax {
        SYS_READ => {
            frame.rax = syscall_read(frame.rdi as usize, frame.rsi, frame.rdx as usize) as u64;
        }
        SYS_WRITE => {
            frame.rax = syscall_write(frame.rdi as usize, frame.rsi, frame.rdx as usize) as u64;
        }
        SYS_YIELD => {
            crate::scheduler::yield_now();
            frame.rax = 0;
        }
        SYS_EXIT => {
            crate::scheduler::exit_current_process(frame.rdi as usize);
        }
        SYS_SPAWN_WAIT => {
            frame.rax = syscall_spawn_wait(frame.rdi, frame.rsi as usize) as u64;
        }
        _ => {
            frame.rax = 0;
        }
    }
}

fn syscall_read(fd: usize, buffer_address: u64, len: usize) -> usize {
    let Some(buffer) = memory::user_slice_mut(buffer_address, len) else {
        return 0;
    };

    crate::scheduler::current_process_read(fd, buffer)
}

fn syscall_write(fd: usize, buffer_address: u64, len: usize) -> usize {
    let Some(buffer) = memory::user_slice(buffer_address, len) else {
        return 0;
    };

    crate::scheduler::current_process_write(fd, buffer)
}

fn syscall_spawn_wait(path_address: u64, path_len: usize) -> usize {
    let Some(path_bytes) = memory::user_slice(path_address, path_len) else {
        return usize::MAX;
    };
    let Ok(path) = core::str::from_utf8(path_bytes) else {
        return usize::MAX;
    };
    if !path.starts_with('/') {
        return usize::MAX;
    }

    crate::scheduler::spawn_user_process_and_wait(path).unwrap_or(usize::MAX)
}
