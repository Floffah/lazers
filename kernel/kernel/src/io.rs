//! Kernel-owned handle and stdio abstractions.
//!
//! This layer is intentionally tiny. It gives processes stable handle-based
//! access to kernel objects without exposing those objects' concrete types to
//! every caller, and it establishes the ownership model that later spawn and
//! inheritance rules will build on.

use crate::terminal::TerminalEndpoint;

/// Maximum number of installable handles per process in the bootstrap runtime.
pub const MAX_PROCESS_HANDLES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Index into a process-local handle table.
pub struct HandleId(pub usize);

#[derive(Clone, Copy)]
/// Kernel object kinds that can currently live behind a process handle.
///
/// More object kinds will be added here as the filesystem, IPC, and process
/// model grow. For now, terminal endpoints are enough to exercise stdio.
pub enum KernelObject {
    TerminalEndpoint(&'static TerminalEndpoint),
    Null,
}

impl KernelObject {
    /// Attempts to read one byte from the underlying object.
    pub fn read_byte(self) -> Option<u8> {
        match self {
            Self::TerminalEndpoint(endpoint) => endpoint.pop_input_byte(),
            Self::Null => None,
        }
    }

    /// Attempts to write one byte to the underlying object.
    pub fn write_byte(self, byte: u8) -> bool {
        match self {
            Self::TerminalEndpoint(endpoint) => endpoint.push_output_byte(byte),
            Self::Null => {
                let _ = byte;
                true
            }
        }
    }
}

#[derive(Clone, Copy)]
/// Standard stream bindings for a process.
///
/// These are process-owned rather than thread-owned so future child processes
/// can inherit or override them cleanly at spawn time.
pub struct StdioHandles {
    pub stdin: HandleId,
    pub stdout: HandleId,
    pub stderr: HandleId,
}

impl StdioHandles {
    /// Constructs an explicit stdio bundle from already-installed handles.
    pub const fn new(stdin: HandleId, stdout: HandleId, stderr: HandleId) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
        }
    }

    /// Returns an invalid placeholder bundle for processes that do not yet own
    /// usable standard streams.
    pub const fn empty() -> Self {
        Self {
            stdin: HandleId(DEFAULT_INVALID_HANDLE),
            stdout: HandleId(DEFAULT_INVALID_HANDLE),
            stderr: HandleId(DEFAULT_INVALID_HANDLE),
        }
    }
}

const DEFAULT_INVALID_HANDLE: usize = usize::MAX;
