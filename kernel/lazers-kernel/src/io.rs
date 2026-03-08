use crate::terminal::TerminalEndpoint;

pub const MAX_PROCESS_HANDLES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandleId(pub usize);

#[derive(Clone, Copy)]
pub enum KernelObject {
    TerminalEndpoint(&'static TerminalEndpoint),
}

impl KernelObject {
    pub fn read_byte(self) -> Option<u8> {
        match self {
            Self::TerminalEndpoint(endpoint) => endpoint.pop_input_byte(),
        }
    }

    pub fn write_byte(self, byte: u8) -> bool {
        match self {
            Self::TerminalEndpoint(endpoint) => endpoint.push_output_byte(byte),
        }
    }
}

#[derive(Clone, Copy)]
pub struct StdioHandles {
    pub stdin: HandleId,
    pub stdout: HandleId,
    pub stderr: HandleId,
}

impl StdioHandles {
    pub const fn new(stdin: HandleId, stdout: HandleId, stderr: HandleId) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
        }
    }

    pub const fn empty() -> Self {
        Self {
            stdin: HandleId(DEFAULT_INVALID_HANDLE),
            stdout: HandleId(DEFAULT_INVALID_HANDLE),
            stderr: HandleId(DEFAULT_INVALID_HANDLE),
        }
    }
}

const DEFAULT_INVALID_HANDLE: usize = usize::MAX;
