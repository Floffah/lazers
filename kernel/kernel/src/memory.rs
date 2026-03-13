//! Physical-page allocation, address-space construction, and user ELF loading.
//!
//! The current memory model is intentionally direct: the kernel owns page-table
//! construction, keeps an identity-mapped physical window for kernel use, and
//! loads user programs into a fixed low virtual layout. That is enough to bring
//! up user mode without committing to a final higher-level VM design yet.

use boot_info::{BootInfo, MemoryRegionKind};
use core::cell::UnsafeCell;
use core::ptr::{copy_nonoverlapping, write_bytes};
use core::slice;
use elf::{ElfError, ElfImage, PF_W, PT_LOAD};

pub const PAGE_SIZE: usize = 4096;
pub const USER_IMAGE_BASE: u64 = 0x0000_0000_0040_0000;
pub const USER_STACK_TOP: u64 = 0x0000_0000_0080_0000;
pub const USER_STACK_PAGES: usize = 16;

const MAX_SEGMENT_PAGES: usize = 128;
const MAX_FREE_RANGES: usize = 128;
const MAX_OWNED_PAGES: usize = MAX_SEGMENT_PAGES + USER_STACK_PAGES + 32;
const MAX_SHARED_KERNEL_MAPPINGS: usize = 16;
const MAX_STARTUP_ARGS: usize = 32;
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_USER: u64 = 1 << 2;
const PAGE_HUGE: u64 = 1 << 7;
const PAGE_TABLE_FLAGS: u64 = PAGE_PRESENT | PAGE_WRITABLE;
const PHYS_WINDOW_START: u64 = 0x0000_0000_0100_0000;

static MEMORY: MemoryCell = MemoryCell::new();

unsafe extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

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
    start_paddr: u64,
    len: usize,
    page_count: usize,
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
            state.free_contiguous_pages(self.start_paddr, self.page_count);
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

    fn push(&mut self, page: u64) -> Result<(), MemoryError> {
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
                state.free_page(self.pages[index]);
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

/// Initializes the kernel allocator and installs the first kernel-owned page
/// table.
///
/// The resulting kernel address space identity-maps a physical window covering
/// all usable memory reported by the loader, plus the framebuffer if it lies
/// outside that window.
pub fn init(boot_info: &BootInfo) -> Result<(), MemoryError> {
    with_state_mut(|state| {
        state.initialize_allocator(boot_info)?;

        let root_paddr = state.allocate_page_pre_switch()?;
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

    crate::arch::load_page_table(kernel_address_space().root_paddr());
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
    let start = with_state_mut(|state| state.allocate_contiguous_pages(page_count))?;
    Ok(KernelBuffer {
        start_paddr: start,
        len: size,
        page_count,
    })
}

/// Allocates one zeroed physical page for kernel-owned structures.
pub fn allocate_kernel_page() -> Result<u64, MemoryError> {
    with_state_mut(|state| state.allocate_page())
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
        state.record_shared_kernel_mapping(start, end, writable)
    })?;

    crate::arch::load_page_table(kernel_address_space().root_paddr());
    Ok(())
}

/// Parses and maps one user ELF into a fresh user address space.
///
/// The loader reuses the shared ELF parser but owns the paging policy: loadable
/// segments must fit inside the fixed user image range, and a fixed user stack
/// is appended above them.
pub fn load_user_program(
    bytes: &[u8],
    startup: &ProgramStartup<'_>,
) -> Result<LoadedUserProgram, MemoryError> {
    let elf = ElfImage::parse(bytes).map_err(MemoryError::Elf)?;
    let entry_point = elf.entry_point();
    if !contains_user_address(entry_point) {
        return Err(MemoryError::UserImageOutOfRange);
    }

    with_state_mut(|state| {
        let mut owned_pages = OwnedPages::empty();
        let result = (|| {
            let root_paddr = state.allocate_page()?;
            owned_pages.push(root_paddr)?;
            let mut builder = AddressSpaceBuilder::new(root_paddr, Some(&mut owned_pages));
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

            state.replay_shared_kernel_mappings(&mut builder)?;

            let mut pages = UserPageMap::new();
            for header_result in elf.program_headers() {
                let header = header_result.map_err(MemoryError::Elf)?;
                if header.kind != PT_LOAD {
                    continue;
                }

                let segment_start = header.virtual_address;
                let segment_end = header
                    .virtual_address
                    .checked_add(header.memory_size)
                    .ok_or(MemoryError::UserImageOutOfRange)?;
                let user_stack_base =
                    USER_STACK_TOP - ((USER_STACK_PAGES as u64) * (PAGE_SIZE as u64));

                if segment_start < USER_IMAGE_BASE
                    || segment_end > user_stack_base
                    || header.memory_size < header.file_size
                {
                    return Err(MemoryError::UserImageOutOfRange);
                }

                let page_start = align_down(segment_start, PAGE_SIZE as u64);
                let page_end = align_up(segment_end, PAGE_SIZE as u64);
                let page_flags = PAGE_PRESENT
                    | PAGE_USER
                    | if (header.flags & PF_W) != 0 {
                        PAGE_WRITABLE
                    } else {
                        0
                    };

                let mut virt = page_start;
                while virt < page_end {
                    if !pages.contains(virt) {
                        let phys = state.allocate_page()?;
                        builder.map_4k(virt, phys, page_flags)?;
                        pages.insert(virt, phys)?;
                        owned_pages.push(phys)?;
                    }
                    virt += PAGE_SIZE as u64;
                }

                let file_range = header.file_range(bytes.len()).map_err(MemoryError::Elf)?;
                pages.copy_into(header.virtual_address, &bytes[file_range])?;
            }

            let user_stack_base = USER_STACK_TOP - ((USER_STACK_PAGES as u64) * (PAGE_SIZE as u64));
            let mut stack_page = user_stack_base;
            while stack_page < USER_STACK_TOP {
                let phys = state.allocate_page()?;
                builder.map_4k(stack_page, phys, PAGE_PRESENT | PAGE_WRITABLE | PAGE_USER)?;
                pages.insert(stack_page, phys)?;
                owned_pages.push(phys)?;
                stack_page += PAGE_SIZE as u64;
            }

            let user_stack_top = write_startup_arguments(&pages, startup)?;

            Ok(LoadedUserProgram {
                address_space: AddressSpace::new(root_paddr),
                entry_point,
                user_stack_top,
                owned_pages: core::mem::replace(&mut owned_pages, OwnedPages::empty()),
            })
        })();

        if result.is_err() {
            owned_pages.release();
        }

        result
    })
}

/// Validates that a user buffer lies entirely within the currently supported
/// user virtual address ranges.
pub fn validate_user_buffer(address: u64, len: usize) -> bool {
    if len == 0 {
        return true;
    }

    let Some(end) = address.checked_add(len as u64) else {
        return false;
    };

    let user_stack_base = USER_STACK_TOP - ((USER_STACK_PAGES as u64) * (PAGE_SIZE as u64));
    address >= USER_IMAGE_BASE
        && end <= USER_STACK_TOP
        && !(end > user_stack_base && address < user_stack_base)
}

/// Borrows an immutable slice from a validated user virtual address range.
pub fn user_slice<'a>(address: u64, len: usize) -> Option<&'a [u8]> {
    if !validate_user_buffer(address, len) {
        return None;
    }

    Some(unsafe { slice::from_raw_parts(address as *const u8, len) })
}

/// Borrows a mutable slice from a validated user virtual address range.
pub fn user_slice_mut<'a>(address: u64, len: usize) -> Option<&'a mut [u8]> {
    if !validate_user_buffer(address, len) {
        return None;
    }

    Some(unsafe { slice::from_raw_parts_mut(address as *mut u8, len) })
}

fn contains_user_address(address: u64) -> bool {
    address >= USER_IMAGE_BASE && address < USER_STACK_TOP
}

fn write_startup_arguments(
    pages: &UserPageMap,
    startup: &ProgramStartup<'_>,
) -> Result<u64, MemoryError> {
    let mut args: [&[u8]; MAX_STARTUP_ARGS] = [&[]; MAX_STARTUP_ARGS];
    args[0] = startup.argv0.as_bytes();
    if core::str::from_utf8(args[0]).is_err() || args[0].contains(&0) {
        return Err(MemoryError::InvalidStartupArguments);
    }

    let mut argc = 1usize;
    let mut cursor = 0usize;
    while cursor < startup.argv_tail.len() {
        let Some(relative_end) = startup.argv_tail[cursor..]
            .iter()
            .position(|byte| *byte == 0)
        else {
            return Err(MemoryError::InvalidStartupArguments);
        };
        let end = cursor + relative_end;
        let arg = &startup.argv_tail[cursor..end];
        if arg.is_empty() || core::str::from_utf8(arg).is_err() || arg.contains(&0) {
            return Err(MemoryError::InvalidStartupArguments);
        }
        if argc >= args.len() {
            return Err(MemoryError::StartupArgumentsTooLarge);
        }
        args[argc] = arg;
        argc += 1;
        cursor = end + 1;
    }

    let mut strings_size = 0usize;
    let mut index = 0usize;
    while index < argc {
        strings_size += args[index].len() + 1;
        index += 1;
    }

    let pointers_len = (argc + 1) * core::mem::size_of::<u64>();
    let strings_start = USER_STACK_TOP
        .checked_sub(strings_size as u64)
        .ok_or(MemoryError::StartupArgumentsTooLarge)?;
    let pointers_start = align_down(
        strings_start
            .checked_sub(pointers_len as u64)
            .ok_or(MemoryError::StartupArgumentsTooLarge)?,
        core::mem::align_of::<u64>() as u64,
    );
    let stack_start = align_down(
        pointers_start
            .checked_sub(core::mem::size_of::<u64>() as u64)
            .ok_or(MemoryError::StartupArgumentsTooLarge)?,
        16,
    );
    let user_stack_base = USER_STACK_TOP - ((USER_STACK_PAGES as u64) * (PAGE_SIZE as u64));
    if stack_start < user_stack_base {
        return Err(MemoryError::StartupArgumentsTooLarge);
    }

    let mut argv_pointers = [0u64; MAX_STARTUP_ARGS + 1];
    let mut string_cursor = strings_start;
    let null_byte = [0u8; 1];
    let mut arg_index = 0usize;
    while arg_index < argc {
        argv_pointers[arg_index] = string_cursor;
        pages.copy_into(string_cursor, args[arg_index])?;
        string_cursor += args[arg_index].len() as u64;
        pages.copy_into(string_cursor, &null_byte)?;
        string_cursor += 1;
        arg_index += 1;
    }
    argv_pointers[argc] = 0;

    let argc_bytes = (argc as u64).to_ne_bytes();
    pages.copy_into(stack_start, &argc_bytes)?;
    let argv_pointer_bytes = unsafe {
        slice::from_raw_parts(
            argv_pointers.as_ptr() as *const u8,
            (argc + 1) * core::mem::size_of::<u64>(),
        )
    };
    pages.copy_into(
        stack_start + core::mem::size_of::<u64>() as u64,
        argv_pointer_bytes,
    )?;

    Ok(stack_start)
}

fn with_state<F, T>(operation: F) -> T
where
    F: FnOnce(&MemoryState) -> T,
{
    unsafe { operation(MEMORY.get()) }
}

fn with_state_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut MemoryState) -> T,
{
    unsafe { operation(MEMORY.get()) }
}

struct MemoryCell {
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

struct MemoryState {
    allocator: PhysicalAllocator,
    kernel_space: Option<AddressSpace>,
    phys_window_end: u64,
    framebuffer_start: u64,
    framebuffer_end: u64,
    shared_kernel_mappings: [SharedKernelMapping; MAX_SHARED_KERNEL_MAPPINGS],
    shared_kernel_mapping_count: usize,
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

    fn initialize_allocator(&mut self, boot_info: &BootInfo) -> Result<(), MemoryError> {
        let regions = unsafe {
            slice::from_raw_parts(boot_info.memory_regions_ptr, boot_info.memory_regions_len)
        };
        self.allocator.reset();
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

            self.allocator.add_region(start, end)?;
            if end > highest_end {
                highest_end = end;
            }
        }

        if highest_end <= PHYS_WINDOW_START {
            return Err(MemoryError::NoUsableMemory);
        }

        let kernel_start = align_down(core::ptr::addr_of!(__kernel_start) as u64, PAGE_SIZE as u64);
        let kernel_end = align_up(core::ptr::addr_of!(__kernel_end) as u64, PAGE_SIZE as u64);
        self.allocator.reserve_range(kernel_start, kernel_end);

        self.phys_window_end = align_up(highest_end, 2 * 1024 * 1024);
        Ok(())
    }

    fn allocate_page_pre_switch(&mut self) -> Result<u64, MemoryError> {
        self.allocator.allocate_page()
    }

    fn allocate_page(&mut self) -> Result<u64, MemoryError> {
        self.allocator.allocate_page()
    }

    fn allocate_contiguous_pages(&mut self, count: usize) -> Result<u64, MemoryError> {
        self.allocator.allocate_contiguous_pages(count)
    }

    fn free_page(&mut self, page: u64) {
        self.allocator.free_contiguous_pages(page, 1);
    }

    fn free_contiguous_pages(&mut self, start: u64, count: usize) {
        self.allocator.free_contiguous_pages(start, count);
    }

    fn record_shared_kernel_mapping(
        &mut self,
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
        while index < self.shared_kernel_mapping_count {
            let mapping = self.shared_kernel_mappings[index];
            if mapping.start <= start && mapping.end >= end && mapping.writable == writable {
                return Ok(());
            }
            index += 1;
        }

        if self.shared_kernel_mapping_count >= self.shared_kernel_mappings.len() {
            return Err(MemoryError::SharedKernelMappingCapacityExceeded);
        }

        self.shared_kernel_mappings[self.shared_kernel_mapping_count] =
            SharedKernelMapping::new(start, end, writable);
        self.shared_kernel_mapping_count += 1;
        Ok(())
    }

    fn replay_shared_kernel_mappings(
        &self,
        builder: &mut AddressSpaceBuilder,
    ) -> Result<(), MemoryError> {
        let mut index = 0;
        while index < self.shared_kernel_mapping_count {
            let mapping = self.shared_kernel_mappings[index];
            let flags = PAGE_PRESENT | if mapping.writable { PAGE_WRITABLE } else { 0 };
            builder.map_identity_4k_range(mapping.start, mapping.end, flags)?;
            index += 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct SharedKernelMapping {
    start: u64,
    end: u64,
    writable: bool,
}

impl SharedKernelMapping {
    const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            writable: false,
        }
    }

    const fn new(start: u64, end: u64, writable: bool) -> Self {
        Self {
            start,
            end,
            writable,
        }
    }
}

struct PhysicalAllocator {
    regions: [PhysicalRegion; MAX_FREE_RANGES],
    count: usize,
}

impl PhysicalAllocator {
    const fn new() -> Self {
        Self {
            regions: [PhysicalRegion::empty(); MAX_FREE_RANGES],
            count: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn add_region(&mut self, start: u64, end: u64) -> Result<(), MemoryError> {
        self.insert_range(start, end);
        Ok(())
    }

    fn allocate_page(&mut self) -> Result<u64, MemoryError> {
        let mut index = 0;
        while index < self.count {
            let region = &mut self.regions[index];
            if let Some(page) = region.allocate_page() {
                if region.is_empty() {
                    self.remove_region(index);
                }
                unsafe {
                    write_bytes(page as *mut u8, 0, PAGE_SIZE);
                }
                return Ok(page);
            }
            index += 1;
        }

        Err(MemoryError::AllocatorExhausted)
    }

    fn allocate_contiguous_pages(&mut self, count: usize) -> Result<u64, MemoryError> {
        let mut index = 0;
        while index < self.count {
            let region = &mut self.regions[index];
            if let Some(start) = region.allocate_contiguous_pages(count) {
                if region.is_empty() {
                    self.remove_region(index);
                }
                unsafe {
                    write_bytes(start as *mut u8, 0, count * PAGE_SIZE);
                }
                return Ok(start);
            }
            index += 1;
        }

        Err(MemoryError::AllocatorExhausted)
    }

    fn free_contiguous_pages(&mut self, start: u64, count: usize) {
        if count == 0 {
            return;
        }

        let bytes = (count as u64) * PAGE_SIZE as u64;
        self.insert_range(start, start + bytes);
    }

    fn reserve_range(&mut self, start: u64, end: u64) {
        if start >= end {
            return;
        }

        let mut index = 0;
        while index < self.count {
            let region = self.regions[index];
            if region.end <= start || region.start >= end {
                index += 1;
                continue;
            }

            if start <= region.start && end >= region.end {
                self.remove_region(index);
                continue;
            }

            if start <= region.start {
                self.regions[index].start = end.min(region.end);
                index += 1;
                continue;
            }

            if end >= region.end {
                self.regions[index].end = start.max(region.start);
                index += 1;
                continue;
            }

            assert!(
                self.count < self.regions.len(),
                "allocator free-range capacity exhausted"
            );

            let right = PhysicalRegion::new(end, region.end);
            self.regions[index].end = start;

            let mut shift = self.count;
            while shift > index + 1 {
                self.regions[shift] = self.regions[shift - 1];
                shift -= 1;
            }
            self.regions[index + 1] = right;
            self.count += 1;
            index += 2;
        }
    }

    fn insert_range(&mut self, start: u64, end: u64) {
        if start >= end {
            return;
        }

        let mut merged_start = start;
        let mut merged_end = end;
        let mut index = 0;

        while index < self.count {
            let region = self.regions[index];
            if region.end < merged_start || region.start > merged_end {
                index += 1;
                continue;
            }

            merged_start = merged_start.min(region.start);
            merged_end = merged_end.max(region.end);
            self.remove_region(index);
        }

        assert!(
            self.count < self.regions.len(),
            "allocator free-range capacity exhausted"
        );

        let mut insert_at = 0;
        while insert_at < self.count && self.regions[insert_at].start < merged_start {
            insert_at += 1;
        }

        let mut shift = self.count;
        while shift > insert_at {
            self.regions[shift] = self.regions[shift - 1];
            shift -= 1;
        }

        self.regions[insert_at] = PhysicalRegion::new(merged_start, merged_end);
        self.count += 1;
    }

    fn remove_region(&mut self, index: usize) {
        let mut shift = index;
        while shift + 1 < self.count {
            self.regions[shift] = self.regions[shift + 1];
            shift += 1;
        }
        if self.count > 0 {
            self.count -= 1;
            self.regions[self.count] = PhysicalRegion::empty();
        }
    }
}

#[derive(Clone, Copy)]
struct PhysicalRegion {
    start: u64,
    end: u64,
}

impl PhysicalRegion {
    const fn empty() -> Self {
        Self { start: 0, end: 0 }
    }

    const fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    fn is_empty(&self) -> bool {
        self.start == 0 || self.start >= self.end
    }

    fn allocate_page(&mut self) -> Option<u64> {
        if self.is_empty() {
            return None;
        }

        let page = self.start;
        let next = self.start.checked_add(PAGE_SIZE as u64)?;
        if next > self.end {
            return None;
        }
        self.start = next;
        Some(page)
    }

    fn allocate_contiguous_pages(&mut self, count: usize) -> Option<u64> {
        if count == 0 || self.is_empty() {
            return None;
        }

        let bytes = (count as u64).checked_mul(PAGE_SIZE as u64)?;
        let end = self.start.checked_add(bytes)?;
        if end > self.end {
            return None;
        }

        let start = self.start;
        self.start = end;
        Some(start)
    }
}

struct AddressSpaceBuilder {
    root_paddr: u64,
    owned_pages: Option<*mut OwnedPages>,
}

impl AddressSpaceBuilder {
    fn new(root_paddr: u64, owned_pages: Option<&mut OwnedPages>) -> Self {
        Self {
            root_paddr,
            owned_pages: owned_pages.map(|owned_pages| owned_pages as *mut OwnedPages),
        }
    }

    fn map_identity_2m_range(
        &mut self,
        start: u64,
        end: u64,
        flags: u64,
    ) -> Result<(), MemoryError> {
        let mut address = align_down(start, 2 * 1024 * 1024);
        let limit = align_up(end, 2 * 1024 * 1024);
        while address < limit {
            self.map_2m(address, address, flags)?;
            address += 2 * 1024 * 1024;
        }
        Ok(())
    }

    fn map_identity_4k_range(
        &mut self,
        start: u64,
        end: u64,
        flags: u64,
    ) -> Result<(), MemoryError> {
        let mut address = align_down(start, PAGE_SIZE as u64);
        let limit = align_up(end, PAGE_SIZE as u64);
        while address < limit {
            self.map_4k(address, address, flags)?;
            address += PAGE_SIZE as u64;
        }
        Ok(())
    }

    fn map_2m(&mut self, virt: u64, phys: u64, flags: u64) -> Result<(), MemoryError> {
        debug_assert_eq!(virt % (2 * 1024 * 1024), 0);
        debug_assert_eq!(phys % (2 * 1024 * 1024), 0);
        let pd = self.ensure_page_directory(virt, flags & PAGE_USER)?;
        let index = pd_index(virt);
        unsafe {
            page_table_mut(pd).entries[index] = phys | flags | PAGE_HUGE;
        }
        Ok(())
    }

    fn map_4k(&mut self, virt: u64, phys: u64, flags: u64) -> Result<(), MemoryError> {
        debug_assert_eq!(virt % PAGE_SIZE as u64, 0);
        debug_assert_eq!(phys % PAGE_SIZE as u64, 0);

        let pt = self.ensure_page_table(virt, flags & PAGE_USER)?;
        let index = pt_index(virt);
        unsafe {
            page_table_mut(pt).entries[index] = phys | flags;
        }
        Ok(())
    }

    fn ensure_page_directory(&mut self, virt: u64, extra_flags: u64) -> Result<u64, MemoryError> {
        let pdpt = self.ensure_table(self.root_paddr, pml4_index(virt), extra_flags)?;
        self.ensure_table(pdpt, pdpt_index(virt), extra_flags)
    }

    fn ensure_page_table(&mut self, virt: u64, extra_flags: u64) -> Result<u64, MemoryError> {
        let pd = self.ensure_page_directory(virt, extra_flags)?;
        self.ensure_table(pd, pd_index(virt), extra_flags)
    }

    fn ensure_table(
        &mut self,
        table_paddr: u64,
        index: usize,
        extra_flags: u64,
    ) -> Result<u64, MemoryError> {
        unsafe {
            let table = page_table_mut(table_paddr);
            let entry = table.entries[index];
            if (entry & PAGE_PRESENT) != 0 {
                if (entry & extra_flags) != extra_flags {
                    table.entries[index] = entry | extra_flags;
                }
                return Ok(entry & ADDRESS_MASK);
            }
        }

        let child = with_state_mut(|state| state.allocate_page())?;
        if let Some(owned_pages) = self.owned_pages {
            unsafe {
                (*owned_pages).push(child)?;
            }
        }
        unsafe {
            let table = page_table_mut(table_paddr);
            table.entries[index] = child | PAGE_TABLE_FLAGS | extra_flags;
        }
        Ok(child)
    }
}

struct UserPageMap {
    pages: [Option<UserPage>; MAX_SEGMENT_PAGES],
}

impl UserPageMap {
    const fn new() -> Self {
        Self {
            pages: [None; MAX_SEGMENT_PAGES],
        }
    }

    fn contains(&self, virt_page: u64) -> bool {
        self.find(virt_page).is_some()
    }

    fn insert(&mut self, virt_page: u64, phys_page: u64) -> Result<(), MemoryError> {
        let mut index = 0;
        while index < self.pages.len() {
            if self.pages[index].is_none() {
                self.pages[index] = Some(UserPage {
                    virt_page,
                    phys_page,
                });
                return Ok(());
            }
            index += 1;
        }

        Err(MemoryError::SegmentOverlapCapacityExceeded)
    }

    fn copy_into(&self, start_address: u64, bytes: &[u8]) -> Result<(), MemoryError> {
        let mut remaining = bytes;
        let mut address = start_address;
        while !remaining.is_empty() {
            let virt_page = align_down(address, PAGE_SIZE as u64);
            let page = self
                .find(virt_page)
                .ok_or(MemoryError::UserImageOutOfRange)?;
            let offset = (address - virt_page) as usize;
            let length = remaining.len().min(PAGE_SIZE - offset);

            unsafe {
                copy_nonoverlapping(
                    remaining.as_ptr(),
                    (page.phys_page as usize + offset) as *mut u8,
                    length,
                );
            }

            remaining = &remaining[length..];
            address += length as u64;
        }

        Ok(())
    }

    fn find(&self, virt_page: u64) -> Option<UserPage> {
        let mut index = 0;
        while index < self.pages.len() {
            if let Some(page) = self.pages[index] {
                if page.virt_page == virt_page {
                    return Some(page);
                }
            }
            index += 1;
        }

        None
    }
}

#[derive(Clone, Copy)]
struct UserPage {
    virt_page: u64,
    phys_page: u64,
}

#[repr(C, align(4096))]
struct PageTable {
    entries: [u64; 512],
}

const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;

unsafe fn page_table_mut(address: u64) -> &'static mut PageTable {
    &mut *(address as *mut PageTable)
}

const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

const fn align_up(value: u64, align: u64) -> u64 {
    if value == 0 {
        0
    } else {
        (value + align - 1) & !(align - 1)
    }
}

const fn pml4_index(address: u64) -> usize {
    ((address >> 39) & 0x1ff) as usize
}

const fn pdpt_index(address: u64) -> usize {
    ((address >> 30) & 0x1ff) as usize
}

const fn pd_index(address: u64) -> usize {
    ((address >> 21) & 0x1ff) as usize
}

const fn pt_index(address: u64) -> usize {
    ((address >> 12) & 0x1ff) as usize
}

const fn elf_error_as_str(error: ElfError) -> &'static str {
    match error {
        ElfError::HeaderTooSmall => "elf header is too small",
        ElfError::InvalidMagic => "elf magic is invalid",
        ElfError::UnsupportedClass => "elf class is unsupported",
        ElfError::UnsupportedEncoding => "elf encoding is unsupported",
        ElfError::UnsupportedType => "elf type is unsupported",
        ElfError::UnsupportedMachine => "elf machine is unsupported",
        ElfError::UnsupportedVersion => "elf version is unsupported",
        ElfError::ProgramHeaderTruncated => "elf program header is truncated",
        ElfError::ProgramHeaderTableOutOfRange => "elf program header table is out of range",
        ElfError::SegmentExtendsPastFile => "elf segment extends past the image",
        ElfError::AddressOutOfRange => "elf address is out of range",
        ElfError::ZeroLoadAddress => "elf load address is zero",
    }
}
