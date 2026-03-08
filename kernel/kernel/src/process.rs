//! Process resource ownership and stdio-backed I/O.
//!
//! In the current runtime, a process is the unit that owns an address space,
//! handle table, and standard streams. Threads execute within a process, but
//! they do not duplicate these resources.

use crate::io::{HandleId, KernelObject, StdioHandles, MAX_PROCESS_HANDLES};
use crate::memory::AddressSpace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Opaque scheduler-assigned identifier for a process slot.
pub struct ProcessId(pub usize);

#[derive(Clone, Copy)]
/// Process metadata and owned resources.
pub struct Process {
    #[allow(dead_code)]
    id: ProcessId,
    #[allow(dead_code)]
    name: &'static str,
    address_space: AddressSpace,
    handles: [Option<KernelObject>; MAX_PROCESS_HANDLES],
    stdio: StdioHandles,
}

impl Process {
    /// Creates an empty process container with no installed handles.
    pub const fn new(id: ProcessId, name: &'static str, address_space: AddressSpace) -> Self {
        Self {
            id,
            name,
            address_space,
            handles: [None; MAX_PROCESS_HANDLES],
            stdio: StdioHandles::empty(),
        }
    }

    /// Installs a kernel object into the first free slot of the process handle
    /// table and returns the local handle id for that slot.
    pub fn install_handle(&mut self, object: KernelObject) -> Option<HandleId> {
        let mut index = 0;
        while index < self.handles.len() {
            if self.handles[index].is_none() {
                self.handles[index] = Some(object);
                return Some(HandleId(index));
            }
            index += 1;
        }

        None
    }

    /// Replaces the process' standard stream bindings.
    pub fn set_stdio(&mut self, stdio: StdioHandles) {
        self.stdio = stdio;
    }

    /// Returns the address space that should be activated for this process'
    /// threads.
    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    /// Reads bytes from one of the process' standard streams.
    ///
    /// Only `stdin`, `stdout`, and `stderr` are meaningful file descriptor
    /// numbers at this stage. Unsupported descriptors behave like empty input.
    pub fn read(&self, fd: usize, buffer: &mut [u8]) -> usize {
        let Some(object) = self.resolve_fd(fd) else {
            return 0;
        };

        let mut read = 0;
        while read < buffer.len() {
            let Some(byte) = object.read_byte() else {
                break;
            };
            buffer[read] = byte;
            read += 1;
        }
        read
    }

    /// Writes bytes to one of the process' standard streams.
    pub fn write(&self, fd: usize, buffer: &[u8]) -> usize {
        let Some(object) = self.resolve_fd(fd) else {
            return 0;
        };

        let mut written = 0;
        while written < buffer.len() {
            if !object.write_byte(buffer[written]) {
                break;
            }
            written += 1;
        }
        written
    }

    fn resolve_handle(&self, handle: HandleId) -> Option<KernelObject> {
        self.handles.get(handle.0).and_then(|slot| *slot)
    }

    fn resolve_fd(&self, fd: usize) -> Option<KernelObject> {
        let handle = match fd {
            0 => self.stdio.stdin,
            1 => self.stdio.stdout,
            2 => self.stdio.stderr,
            _ => return None,
        };

        self.resolve_handle(handle)
    }
}
