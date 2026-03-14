use core::slice;

use elf::ElfError;

use super::state::with_state_mut;
use super::util::elf_error_as_str;

pub const PAGE_SIZE: usize = 4096;
pub const USER_IMAGE_BASE: u64 = 0x0000_0000_0040_0000;
pub const USER_STACK_TOP: u64 = 0x0000_0000_0080_0000;
pub const USER_STACK_PAGES: usize = 16;

pub(super) const MAX_SEGMENT_PAGES: usize = 128;
pub(super) const MAX_FREE_RANGES: usize = 128;
pub(super) const MAX_OWNED_PAGES: usize = MAX_SEGMENT_PAGES + USER_STACK_PAGES + 32;
pub(super) const MAX_SHARED_KERNEL_MAPPINGS: usize = 16;
pub(super) const MAX_STARTUP_ARGS: usize = 32;
pub(super) const PAGE_PRESENT: u64 = 1 << 0;
pub(super) const PAGE_WRITABLE: u64 = 1 << 1;
pub(super) const PAGE_USER: u64 = 1 << 2;
pub(super) const PAGE_HUGE: u64 = 1 << 7;
pub(super) const PAGE_TABLE_FLAGS: u64 = PAGE_PRESENT | PAGE_WRITABLE;
pub(super) const PHYS_WINDOW_START: u64 = 0x0000_0000_0100_0000;

#[derive(Clone, Copy)]
/// Page-table root for either the kernel or a user process.
pub struct AddressSpace {
    root_paddr: u64,
}

impl AddressSpace {
    pub const fn new(root_paddr: u64) -> Self {
        Self { root_paddr }
    }

    pub const fn root_paddr(self) -> u64 {
        self.root_paddr
    }
}

/// Result of loading one user ELF into a newly created address space.
pub struct LoadedUserProgram {
    pub address_space: AddressSpace,
    pub entry_point: u64,
    pub user_stack_top: u64,
    pub owned_pages: OwnedPages,
}

/// Startup data that should be made visible to a new user process.
pub struct ProgramStartup<'a> {
    pub argv0: &'a str,
    pub argv_tail: &'a [u8],
}

/// Reclaimable contiguous kernel buffer backed by physical pages.
pub struct KernelBuffer {
    pub(super) start_paddr: u64,
    pub(super) len: usize,
    pub(super) page_count: usize,
}

impl KernelBuffer {
    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.start_paddr as *const u8, self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.start_paddr as *mut u8, self.len) }
    }

    pub fn release(self) {
        with_state_mut(|state| {
            state
                .allocator
                .free_contiguous_pages(self.start_paddr, self.page_count);
        });
    }
}

/// Physical pages owned by one user process image.
pub struct OwnedPages {
    pages: [u64; MAX_OWNED_PAGES],
    len: usize,
}

impl OwnedPages {
    pub const fn empty() -> Self {
        Self {
            pages: [0; MAX_OWNED_PAGES],
            len: 0,
        }
    }

    pub(super) fn push(&mut self, page: u64) -> Result<(), MemoryError> {
        if self.len >= self.pages.len() {
            return Err(MemoryError::OwnedPageCapacityExceeded);
        }

        self.pages[self.len] = page;
        self.len += 1;
        Ok(())
    }

    pub fn release(self) {
        let mut index = 0;
        while index < self.len {
            with_state_mut(|state| {
                state.allocator.free_contiguous_pages(self.pages[index], 1);
            });
            index += 1;
        }
    }
}

#[derive(Clone, Copy, Debug)]
/// Errors raised while building page tables or loading a user image.
pub enum MemoryError {
    NoUsableMemory,
    AddressSpaceUninitialized,
    AllocatorExhausted,
    InvalidKernelBufferSize,
    SharedKernelMappingCapacityExceeded,
    Elf(ElfError),
    UserImageOutOfRange,
    InvalidStartupArguments,
    StartupArgumentsTooLarge,
    SegmentOverlapCapacityExceeded,
    OwnedPageCapacityExceeded,
}

impl MemoryError {
    /// Returns a short static description suitable for panic and boot output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoUsableMemory => "no usable physical memory is available",
            Self::AddressSpaceUninitialized => "kernel address space is not initialized",
            Self::AllocatorExhausted => "physical page allocator is exhausted",
            Self::InvalidKernelBufferSize => "kernel buffer size is invalid",
            Self::SharedKernelMappingCapacityExceeded => {
                "shared kernel mapping capacity is exhausted"
            }
            Self::Elf(error) => elf_error_as_str(error),
            Self::UserImageOutOfRange => "user image falls outside the fixed user layout",
            Self::InvalidStartupArguments => "user startup arguments are invalid",
            Self::StartupArgumentsTooLarge => {
                "user startup arguments do not fit in the fixed stack"
            }
            Self::SegmentOverlapCapacityExceeded => "user image mapping capacity is exhausted",
            Self::OwnedPageCapacityExceeded => {
                "user image ownership tracking capacity is exhausted"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryError, OwnedPages, MAX_OWNED_PAGES};

    #[test]
    fn owned_pages_capacity_is_enforced() {
        let mut pages = OwnedPages::empty();
        for page in 0..MAX_OWNED_PAGES {
            pages.push((page as u64) * super::PAGE_SIZE as u64).unwrap();
        }

        assert!(matches!(
            pages.push(0xdead_beef),
            Err(MemoryError::OwnedPageCapacityExceeded)
        ));
    }
}
