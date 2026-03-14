use boot_info::{BootInfo, MemoryRegionKind};

use super::paging::AddressSpaceBuilder;
use super::state::{with_state, with_state_mut, MemoryState};
use super::types::{
    AddressSpace, KernelBuffer, MemoryError, PAGE_PRESENT, PAGE_SIZE, PAGE_WRITABLE,
    PHYS_WINDOW_START,
};
use super::util::{align_down, align_up};

#[cfg(not(test))]
unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

/// Initializes the kernel allocator and installs the first kernel-owned page
/// table.
///
/// The resulting kernel address space identity-maps a physical window covering
/// all usable memory reported by the loader, plus the framebuffer if it lies
/// outside that window.
pub fn init(boot_info: &BootInfo) -> Result<(), MemoryError> {
    with_state_mut(|state| {
        initialize_allocator(state, boot_info)?;

        let root_paddr = state.allocator.allocate_page()?;
        let mut builder = AddressSpaceBuilder::new(root_paddr, None);
        builder.map_identity_2m_range(
            PHYS_WINDOW_START,
            state.phys_window_end,
            PAGE_PRESENT | PAGE_WRITABLE,
        )?;

        let framebuffer_start = align_down(boot_info.framebuffer.base as u64, PAGE_SIZE as u64);
        let framebuffer_end = align_up(
            boot_info.framebuffer.base as u64 + boot_info.framebuffer.size as u64,
            PAGE_SIZE as u64,
        );
        if framebuffer_start < PHYS_WINDOW_START || framebuffer_end > state.phys_window_end {
            builder.map_identity_4k_range(
                framebuffer_start,
                framebuffer_end,
                PAGE_PRESENT | PAGE_WRITABLE,
            )?;
        }

        state.framebuffer_start = framebuffer_start;
        state.framebuffer_end = framebuffer_end;
        state.kernel_space = Some(AddressSpace::new(root_paddr));
        Ok(())
    })?;

    load_active_page_table(kernel_address_space().root_paddr());
    Ok(())
}

/// Returns the kernel's canonical address space.
pub fn kernel_address_space() -> AddressSpace {
    with_state(|state| {
        state
            .kernel_space
            .expect("kernel address space not initialized")
    })
}

/// Allocates a zeroed, contiguous kernel buffer backed by physical pages.
///
/// This is primarily used by bootstrap subsystems such as storage that need a
/// stable scratch buffer without yet having a general kernel heap.
pub fn allocate_kernel_buffer(size: usize) -> Result<KernelBuffer, MemoryError> {
    if size == 0 {
        return Err(MemoryError::InvalidKernelBufferSize);
    }

    let page_count = (align_up(size as u64, PAGE_SIZE as u64) / PAGE_SIZE as u64) as usize;
    let start = with_state_mut(|state| state.allocator.allocate_contiguous_pages(page_count))?;
    Ok(KernelBuffer {
        start_paddr: start,
        len: size,
        page_count,
    })
}

/// Allocates one zeroed physical page for kernel-owned structures.
pub fn allocate_kernel_page() -> Result<u64, MemoryError> {
    with_state_mut(|state| state.allocator.allocate_page())
}

/// Extends the active kernel address space with an identity-mapped region.
///
/// This is used for MMIO ranges discovered after the initial paging setup, such
/// as the AHCI controller's ABAR region.
pub fn map_kernel_identity_range(start: u64, end: u64, writable: bool) -> Result<(), MemoryError> {
    let flags = PAGE_PRESENT | if writable { PAGE_WRITABLE } else { 0 };
    with_state_mut(|state| {
        let kernel_space = state
            .kernel_space
            .ok_or(MemoryError::AddressSpaceUninitialized)?;
        let mut builder = AddressSpaceBuilder::new(kernel_space.root_paddr(), None);
        builder.map_identity_4k_range(start, end, flags)?;
        record_shared_kernel_mapping(state, start, end, writable)
    })?;

    load_active_page_table(kernel_address_space().root_paddr());
    Ok(())
}

pub(super) fn map_shared_kernel_context(
    builder: &mut AddressSpaceBuilder,
) -> Result<(), MemoryError> {
    with_state(|state| {
        builder.map_identity_2m_range(
            PHYS_WINDOW_START,
            state.phys_window_end,
            PAGE_PRESENT | PAGE_WRITABLE,
        )?;

        if state.framebuffer_start != 0
            && (state.framebuffer_start < PHYS_WINDOW_START
                || state.framebuffer_end > state.phys_window_end)
        {
            builder.map_identity_4k_range(
                state.framebuffer_start,
                state.framebuffer_end,
                PAGE_PRESENT | PAGE_WRITABLE,
            )?;
        }

        replay_shared_kernel_mappings(state, builder)
    })
}

fn initialize_allocator(state: &mut MemoryState, boot_info: &BootInfo) -> Result<(), MemoryError> {
    let regions = unsafe {
        core::slice::from_raw_parts(boot_info.memory_regions_ptr, boot_info.memory_regions_len)
    };
    state.allocator.reset();
    state.shared_kernel_mapping_count = 0;

    let mut highest_end = 0u64;
    for region in regions {
        if region.kind != MemoryRegionKind::Usable {
            continue;
        }

        let start = region.start.max(PHYS_WINDOW_START);
        let end = align_down(
            region.start + region.page_count.saturating_mul(PAGE_SIZE as u64),
            PAGE_SIZE as u64,
        );
        if end <= start {
            continue;
        }

        state.allocator.add_region(start, end)?;
        if end > highest_end {
            highest_end = end;
        }
    }

    if highest_end <= PHYS_WINDOW_START {
        return Err(MemoryError::NoUsableMemory);
    }

    if let Some((kernel_start, kernel_end)) = kernel_image_range() {
        state.allocator.reserve_range(kernel_start, kernel_end);
    }

    state.phys_window_end = align_up(highest_end, 2 * 1024 * 1024);
    Ok(())
}

fn record_shared_kernel_mapping(
    state: &mut MemoryState,
    start: u64,
    end: u64,
    writable: bool,
) -> Result<(), MemoryError> {
    let start = align_down(start, PAGE_SIZE as u64);
    let end = align_up(end, PAGE_SIZE as u64);
    if start >= end {
        return Ok(());
    }

    let mut index = 0;
    while index < state.shared_kernel_mapping_count {
        let mapping = state.shared_kernel_mappings[index];
        if mapping.start <= start && mapping.end >= end && mapping.writable == writable {
            return Ok(());
        }
        index += 1;
    }

    if state.shared_kernel_mapping_count >= state.shared_kernel_mappings.len() {
        return Err(MemoryError::SharedKernelMappingCapacityExceeded);
    }

    state.shared_kernel_mappings[state.shared_kernel_mapping_count] =
        super::state::SharedKernelMapping::new(start, end, writable);
    state.shared_kernel_mapping_count += 1;
    Ok(())
}

fn replay_shared_kernel_mappings(
    state: &MemoryState,
    builder: &mut AddressSpaceBuilder,
) -> Result<(), MemoryError> {
    let mut index = 0;
    while index < state.shared_kernel_mapping_count {
        let mapping = state.shared_kernel_mappings[index];
        let flags = PAGE_PRESENT | if mapping.writable { PAGE_WRITABLE } else { 0 };
        builder.map_identity_4k_range(mapping.start, mapping.end, flags)?;
        index += 1;
    }
    Ok(())
}

#[cfg(not(test))]
fn kernel_image_range() -> Option<(u64, u64)> {
    Some((
        align_down(core::ptr::addr_of!(__kernel_start) as u64, PAGE_SIZE as u64),
        align_up(core::ptr::addr_of!(__kernel_end) as u64, PAGE_SIZE as u64),
    ))
}

#[cfg(test)]
fn kernel_image_range() -> Option<(u64, u64)> {
    None
}

#[cfg(not(test))]
fn load_active_page_table(root_paddr: u64) {
    crate::arch::load_page_table(root_paddr);
}

#[cfg(test)]
fn load_active_page_table(_root_paddr: u64) {}
