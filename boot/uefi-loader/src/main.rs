#![no_main]
#![no_std]

extern crate alloc;

mod elf;

use alloc::boxed::Box;
use alloc::vec::Vec;
use boot_info::{BootInfo, FramebufferInfo, MemoryRegion, MemoryRegionKind, PixelFormat};
use core::arch::asm;
use core::ptr::{copy_nonoverlapping, write_bytes};
use elf::{ElfError, ElfImage, PT_LOAD};
use uefi::boot::{self, AllocateType, MemoryType};
use uefi::fs::FileSystem;
use uefi::mem::memory_map::MemoryMap;
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat as GopPixelFormat};
use uefi::table::cfg;

const KERNEL_STACK_PAGES: usize = 16;
const MEMORY_REGION_CAPACITY: usize = 1024;
const PAGE_SIZE: usize = 4096;

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    match run() {
        Ok(()) => Status::SUCCESS,
        Err(error) => {
            uefi::println!("lazers loader error: {}", error.as_str());
            error.status()
        }
    }
}

fn run() -> Result<(), BootFailure> {
    let kernel_bytes = read_kernel_image().map_err(BootFailure::from_fs)?;
    let elf = ElfImage::parse(&kernel_bytes).map_err(BootFailure::from_elf)?;
    let framebuffer = query_framebuffer()?;
    let acpi_rsdp_addr = find_acpi_rsdp();
    let mut memory_regions = Vec::<MemoryRegion>::with_capacity(MEMORY_REGION_CAPACITY);
    let mut boot_info = Box::new(BootInfo::new(
        framebuffer,
        core::ptr::null(),
        0,
        acpi_rsdp_addr,
    ));

    load_kernel_segments(&elf, &kernel_bytes)?;
    let kernel_stack_top = allocate_kernel_stack()?;

    let memory_map = unsafe { boot::exit_boot_services(Some(MemoryType::LOADER_DATA)) };

    for descriptor in memory_map.entries() {
        if memory_regions.len() == memory_regions.capacity() {
            return Err(BootFailure::MemoryMapCapacityExceeded);
        }
        memory_regions.push(normalize_memory_region(descriptor));
    }

    boot_info.memory_regions_ptr = memory_regions.as_ptr();
    boot_info.memory_regions_len = memory_regions.len();

    let boot_info_ptr = boot_info.as_mut() as *mut BootInfo;
    let entry_point = elf.entry_point();

    core::mem::forget(memory_regions);
    core::mem::forget(boot_info);

    unsafe {
        jump_to_kernel(entry_point, boot_info_ptr.cast_const(), kernel_stack_top);
    }
}

fn read_kernel_image() -> Result<Vec<u8>, uefi::fs::Error> {
    let fs = boot::get_image_file_system(boot::image_handle()).unwrap();
    let mut fs = FileSystem::new(fs);
    fs.read(cstr16!(r"\lazers\kernel.elf"))
}

fn query_framebuffer() -> Result<FramebufferInfo, BootFailure> {
    let mut gop = boot::get_handle_for_protocol::<GraphicsOutput>()
        .map_err(|_| BootFailure::GraphicsOutputUnavailable)
        .and_then(|handle| {
            boot::open_protocol_exclusive::<GraphicsOutput>(handle)
                .map_err(|_| BootFailure::GraphicsOutputUnavailable)
        })?;

    let mode = gop.current_mode_info();
    let (width, height) = mode.resolution();
    let format = match mode.pixel_format() {
        GopPixelFormat::Rgb => PixelFormat::Rgb,
        GopPixelFormat::Bgr => PixelFormat::Bgr,
        _ => return Err(BootFailure::UnsupportedPixelFormat),
    };

    let mut framebuffer = gop.frame_buffer();
    let info = FramebufferInfo {
        base: framebuffer.as_mut_ptr(),
        size: framebuffer.size(),
        width: u32::try_from(width).map_err(|_| BootFailure::InvalidFramebuffer)?,
        height: u32::try_from(height).map_err(|_| BootFailure::InvalidFramebuffer)?,
        stride: u32::try_from(mode.stride()).map_err(|_| BootFailure::InvalidFramebuffer)?,
        format,
    };

    if !info.is_usable() {
        return Err(BootFailure::InvalidFramebuffer);
    }

    Ok(info)
}

fn find_acpi_rsdp() -> u64 {
    uefi::system::with_config_table(|entries| {
        entries
            .iter()
            .find_map(|entry| {
                if entry.guid == cfg::ACPI2_GUID || entry.guid == cfg::ACPI_GUID {
                    Some(entry.address as usize as u64)
                } else {
                    None
                }
            })
            .unwrap_or(0)
    })
}

fn load_kernel_segments(elf: &ElfImage<'_>, kernel_bytes: &[u8]) -> Result<(), BootFailure> {
    for program_header in elf.program_headers() {
        let program_header = program_header.map_err(BootFailure::from_elf)?;
        if program_header.kind != PT_LOAD || program_header.memory_size == 0 {
            continue;
        }

        let load_address = program_header.load_address().map_err(BootFailure::from_elf)?;
        let file_range = program_header
            .file_range(kernel_bytes.len())
            .map_err(BootFailure::from_elf)?;

        let segment_start = align_down(load_address, PAGE_SIZE as u64);
        let segment_end = align_up(
            load_address
                .checked_add(program_header.memory_size)
                .ok_or(BootFailure::AddressOverflow)?,
            PAGE_SIZE as u64,
        );

        let page_count = usize::try_from((segment_end - segment_start) / PAGE_SIZE as u64)
            .map_err(|_| BootFailure::AddressOverflow)?;

        let segment_base = boot::allocate_pages(
            AllocateType::Address(segment_start),
            MemoryType::LOADER_DATA,
            page_count,
        )
        .map_err(|_| BootFailure::KernelSegmentAllocationFailed)?;

        unsafe {
            write_bytes(segment_base.as_ptr(), 0, page_count * PAGE_SIZE);
        }

        let copy_length = usize::try_from(program_header.file_size)
            .map_err(|_| BootFailure::AddressOverflow)?;
        let copy_end = load_address
            .checked_add(program_header.file_size)
            .ok_or(BootFailure::AddressOverflow)?;
        let mem_end = load_address
            .checked_add(program_header.memory_size)
            .ok_or(BootFailure::AddressOverflow)?;
        if copy_end > mem_end {
            return Err(BootFailure::SegmentFileLargerThanMemory);
        }

        unsafe {
            copy_nonoverlapping(
                kernel_bytes[file_range].as_ptr(),
                load_address as usize as *mut u8,
                copy_length,
            );
        }
    }

    Ok(())
}

fn allocate_kernel_stack() -> Result<u64, BootFailure> {
    let stack = boot::allocate_pages(
        AllocateType::AnyPages,
        MemoryType::LOADER_DATA,
        KERNEL_STACK_PAGES,
    )
    .map_err(|_| BootFailure::KernelStackAllocationFailed)?;

    let top = stack.as_ptr() as usize + (KERNEL_STACK_PAGES * PAGE_SIZE);
    Ok((top & !0x0f) as u64)
}

fn normalize_memory_region(descriptor: &uefi::mem::memory_map::MemoryDescriptor) -> MemoryRegion {
    let kind = match descriptor.ty {
        MemoryType::CONVENTIONAL => MemoryRegionKind::Usable,
        MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => MemoryRegionKind::Loader,
        MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => {
            MemoryRegionKind::BootServices
        }
        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            MemoryRegionKind::RuntimeServices
        }
        MemoryType::ACPI_RECLAIM => MemoryRegionKind::AcpiReclaimable,
        MemoryType::ACPI_NON_VOLATILE => MemoryRegionKind::AcpiNvs,
        MemoryType::MMIO | MemoryType::MMIO_PORT_SPACE | MemoryType::PAL_CODE => {
            MemoryRegionKind::Mmio
        }
        MemoryType::PERSISTENT_MEMORY => MemoryRegionKind::Persistent,
        MemoryType::UNUSABLE => MemoryRegionKind::Unusable,
        _ => MemoryRegionKind::Reserved,
    };

    MemoryRegion {
        start: descriptor.phys_start,
        page_count: descriptor.page_count,
        kind,
        reserved: 0,
    }
}

#[inline]
fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

#[inline]
fn align_up(value: u64, align: u64) -> u64 {
    (value + (align - 1)) & !(align - 1)
}

unsafe fn jump_to_kernel(entry_point: u64, boot_info: *const BootInfo, stack_top: u64) -> ! {
    asm!(
        "cli",
        "mov rsp, {stack_top}",
        "xor rbp, rbp",
        "mov rdi, {boot_info}",
        "jmp {entry_point}",
        stack_top = in(reg) stack_top,
        boot_info = in(reg) boot_info,
        entry_point = in(reg) entry_point,
        options(noreturn)
    )
}

#[derive(Clone, Copy, Debug)]
enum BootFailure {
    FilesystemReadFailed,
    InvalidElf,
    GraphicsOutputUnavailable,
    UnsupportedPixelFormat,
    InvalidFramebuffer,
    KernelSegmentAllocationFailed,
    KernelStackAllocationFailed,
    MemoryMapCapacityExceeded,
    SegmentFileLargerThanMemory,
    AddressOverflow,
}

impl BootFailure {
    fn status(self) -> Status {
        match self {
            Self::FilesystemReadFailed | Self::InvalidElf => Status::LOAD_ERROR,
            Self::GraphicsOutputUnavailable
            | Self::UnsupportedPixelFormat
            | Self::InvalidFramebuffer => Status::UNSUPPORTED,
            Self::KernelSegmentAllocationFailed
            | Self::KernelStackAllocationFailed
            | Self::MemoryMapCapacityExceeded
            | Self::AddressOverflow => Status::OUT_OF_RESOURCES,
            Self::SegmentFileLargerThanMemory => Status::LOAD_ERROR,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::FilesystemReadFailed => "failed to read kernel image",
            Self::InvalidElf => "kernel image is not a supported ELF64 executable",
            Self::GraphicsOutputUnavailable => "graphics output protocol is unavailable",
            Self::UnsupportedPixelFormat => "graphics mode does not expose a direct RGB or BGR framebuffer",
            Self::InvalidFramebuffer => "graphics framebuffer information was invalid",
            Self::KernelSegmentAllocationFailed => "failed to allocate memory for a kernel segment",
            Self::KernelStackAllocationFailed => "failed to allocate the initial kernel stack",
            Self::MemoryMapCapacityExceeded => "normalized memory map exceeded the preallocated buffer",
            Self::SegmentFileLargerThanMemory => "kernel segment file size exceeded its in-memory size",
            Self::AddressOverflow => "kernel image contained addresses that overflowed the loader",
        }
    }

    fn from_elf(error: ElfError) -> Self {
        let _ = error;
        Self::InvalidElf
    }

    fn from_fs(_: uefi::fs::Error) -> Self {
        Self::FilesystemReadFailed
    }
}
