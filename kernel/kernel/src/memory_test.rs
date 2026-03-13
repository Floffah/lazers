#[derive(Clone, Copy, Debug)]
/// Minimal memory error stub for host-side library tests.
pub enum MemoryError {
    UnsupportedInTests,
}

impl MemoryError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedInTests => "memory operations are unavailable in host library tests",
        }
    }
}

/// Minimal kernel buffer stub for host-side library tests.
pub struct KernelBuffer;

impl KernelBuffer {
    pub const fn len(&self) -> usize {
        0
    }

    pub fn as_slice(&self) -> &[u8] {
        &[]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut []
    }

    pub fn release(self) {}
}
