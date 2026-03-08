use crate::io::{HandleId, KernelObject, StdioHandles, MAX_PROCESS_HANDLES};
use crate::memory::AddressSpace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessId(pub usize);

#[derive(Clone, Copy)]
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
    pub const fn new(id: ProcessId, name: &'static str, address_space: AddressSpace) -> Self {
        Self {
            id,
            name,
            address_space,
            handles: [None; MAX_PROCESS_HANDLES],
            stdio: StdioHandles::empty(),
        }
    }

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

    pub fn set_stdio(&mut self, stdio: StdioHandles) {
        self.stdio = stdio;
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

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
