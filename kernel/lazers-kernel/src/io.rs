use crate::terminal::TerminalEndpoint;

#[derive(Clone, Copy)]
pub enum IoHandle {
    TerminalInput(&'static TerminalEndpoint),
    TerminalOutput(&'static TerminalEndpoint),
}

impl IoHandle {
    pub const fn terminal_input(endpoint: &'static TerminalEndpoint) -> Self {
        Self::TerminalInput(endpoint)
    }

    pub const fn terminal_output(endpoint: &'static TerminalEndpoint) -> Self {
        Self::TerminalOutput(endpoint)
    }

    pub fn read_byte(self) -> Option<u8> {
        match self {
            Self::TerminalInput(endpoint) => endpoint.pop_input_byte(),
            Self::TerminalOutput(_) => None,
        }
    }

    pub fn write_byte(self, byte: u8) -> bool {
        match self {
            Self::TerminalInput(_) => false,
            Self::TerminalOutput(endpoint) => endpoint.push_output_byte(byte),
        }
    }
}

#[derive(Clone, Copy)]
pub struct StdioHandles {
    pub stdin: IoHandle,
    pub stdout: IoHandle,
    #[allow(dead_code)]
    pub stderr: IoHandle,
}

impl StdioHandles {
    pub const fn new(stdin: IoHandle, stdout: IoHandle, stderr: IoHandle) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
        }
    }
}
