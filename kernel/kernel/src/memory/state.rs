use core::cell::UnsafeCell;

use super::allocator::PhysicalAllocator;
use super::types::{AddressSpace, MAX_SHARED_KERNEL_MAPPINGS, PHYS_WINDOW_START};

pub(super) static MEMORY: MemoryCell = MemoryCell::new();

pub(super) fn with_state<F, T>(operation: F) -> T
where
    F: FnOnce(&MemoryState) -> T,
{
    unsafe { operation(MEMORY.get()) }
}

pub(super) fn with_state_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut MemoryState) -> T,
{
    unsafe { operation(MEMORY.get()) }
}

pub(super) struct MemoryCell {
    state: UnsafeCell<MemoryState>,
}

impl MemoryCell {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(MemoryState::new()),
        }
    }

    unsafe fn get(&self) -> &mut MemoryState {
        &mut *self.state.get()
    }
}

unsafe impl Sync for MemoryCell {}

#[derive(Clone, Copy)]
pub(super) struct SharedKernelMapping {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) writable: bool,
}

impl SharedKernelMapping {
    pub(super) const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            writable: false,
        }
    }

    pub(super) const fn new(start: u64, end: u64, writable: bool) -> Self {
        Self {
            start,
            end,
            writable,
        }
    }
}

pub(super) struct MemoryState {
    pub(super) allocator: PhysicalAllocator,
    pub(super) kernel_space: Option<AddressSpace>,
    pub(super) phys_window_end: u64,
    pub(super) framebuffer_start: u64,
    pub(super) framebuffer_end: u64,
    pub(super) shared_kernel_mappings: [SharedKernelMapping; MAX_SHARED_KERNEL_MAPPINGS],
    pub(super) shared_kernel_mapping_count: usize,
}

impl MemoryState {
    const fn new() -> Self {
        Self {
            allocator: PhysicalAllocator::new(),
            kernel_space: None,
            phys_window_end: PHYS_WINDOW_START,
            framebuffer_start: 0,
            framebuffer_end: 0,
            shared_kernel_mappings: [SharedKernelMapping::empty(); MAX_SHARED_KERNEL_MAPPINGS],
            shared_kernel_mapping_count: 0,
        }
    }
}
