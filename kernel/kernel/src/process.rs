//! Process resource ownership and stdio-backed I/O.
//!
//! In the current runtime, a process is the unit that owns an address space,
//! handle table, and standard streams. Threads execute within a process, but
//! they do not duplicate these resources.

use crate::io::{HandleId, KernelObject, StdioHandles, MAX_PROCESS_HANDLES};
use crate::memory::{AddressSpace, OwnedPages};
use crate::thread::ThreadId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Opaque scheduler-assigned identifier for a process slot.
pub struct ProcessId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Runtime lifecycle state for a process slot.
pub enum ProcessState {
    Running,
    Exited(usize),
}

/// Process metadata and owned resources.
pub struct Process {
    #[allow(dead_code)]
    id: ProcessId,
    #[allow(dead_code)]
    name: &'static str,
    address_space: AddressSpace,
    handles: [Option<KernelObject>; MAX_PROCESS_HANDLES],
    stdio: StdioHandles,
    state: ProcessState,
    waiting_thread: Option<ThreadId>,
    owned_pages: OwnedPages,
}

impl Process {
    /// Creates an empty process container with no installed handles.
    pub const fn new(
        id: ProcessId,
        name: &'static str,
        address_space: AddressSpace,
        owned_pages: OwnedPages,
    ) -> Self {
        Self {
            id,
            name,
            address_space,
            handles: [None; MAX_PROCESS_HANDLES],
            stdio: StdioHandles::empty(),
            state: ProcessState::Running,
            waiting_thread: None,
            owned_pages,
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

    /// Marks the process as exited with the provided status code.
    pub fn mark_exited(&mut self, status: usize) {
        self.state = ProcessState::Exited(status);
    }

    /// Remembers which thread is synchronously waiting on this process.
    pub fn set_waiting_thread(&mut self, thread_id: ThreadId) {
        self.waiting_thread = Some(thread_id);
    }

    /// Removes and returns the thread waiting on this process, if one exists.
    pub fn take_waiting_thread(&mut self) -> Option<ThreadId> {
        let thread_id = self.waiting_thread;
        self.waiting_thread = None;
        thread_id
    }

    /// Duplicates this process' stdio bindings into another process-local handle table.
    pub fn inherit_stdio_into(&self, child: &mut Process) -> Option<()> {
        let stdin = child.install_handle(self.resolve_fd(0)?)?;
        let stdout = child.install_handle(self.resolve_fd(1)?)?;
        let stderr = child.install_handle(self.resolve_fd(2)?)?;
        child.set_stdio(StdioHandles::new(stdin, stdout, stderr));
        Some(())
    }

    /// Releases process-owned memory resources back to the kernel allocator.
    pub fn release_resources(self) {
        self.release_owned_pages().release();
    }

    /// Consumes the process and returns the memory pages it owns.
    pub fn release_owned_pages(self) -> OwnedPages {
        self.owned_pages
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
