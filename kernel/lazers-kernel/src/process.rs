use crate::io::{HandleId, KernelObject, StdioHandles, MAX_PROCESS_HANDLES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessId(pub usize);

#[derive(Clone, Copy)]
pub struct Process {
    #[allow(dead_code)]
    id: ProcessId,
    #[allow(dead_code)]
    name: &'static str,
    handles: [Option<KernelObject>; MAX_PROCESS_HANDLES],
    stdio: StdioHandles,
}

impl Process {
    pub const fn new(id: ProcessId, name: &'static str) -> Self {
        Self {
            id,
            name,
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
    pub fn read_stdin_byte(&self) -> Option<u8> {
        self.resolve_handle(self.stdio.stdin)?.read_byte()
    }

    pub fn write_stdout_byte(&self, byte: u8) -> bool {
        self.resolve_handle(self.stdio.stdout)
            .is_some_and(|object| object.write_byte(byte))
    }

    pub fn write_stderr_byte(&self, byte: u8) -> bool {
        self.resolve_handle(self.stdio.stderr)
            .is_some_and(|object| object.write_byte(byte))
    }

    fn resolve_handle(&self, handle: HandleId) -> Option<KernelObject> {
        self.handles.get(handle.0).and_then(|slot| *slot)
    }
}
