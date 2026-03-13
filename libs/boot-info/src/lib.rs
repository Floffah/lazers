#![no_std]

pub const BOOT_INFO_MAGIC: u64 = u64::from_le_bytes(*b"LAZRBOOT");
pub const BOOT_INFO_VERSION: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub reserved: u32,
    pub framebuffer: FramebufferInfo,
    pub memory_regions_ptr: *const MemoryRegion,
    pub memory_regions_len: usize,
    pub acpi_rsdp_addr: u64,
}

impl BootInfo {
    pub const fn new(
        framebuffer: FramebufferInfo,
        memory_regions_ptr: *const MemoryRegion,
        memory_regions_len: usize,
        acpi_rsdp_addr: u64,
    ) -> Self {
        Self {
            magic: BOOT_INFO_MAGIC,
            version: BOOT_INFO_VERSION,
            reserved: 0,
            framebuffer,
            memory_regions_ptr,
            memory_regions_len,
            acpi_rsdp_addr,
        }
    }

    pub const fn has_valid_header(&self) -> bool {
        self.magic == BOOT_INFO_MAGIC && self.version == BOOT_INFO_VERSION
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FramebufferInfo {
    pub base: *mut u8,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}

impl FramebufferInfo {
    pub const fn is_usable(&self) -> bool {
        !self.base.is_null()
            && self.size != 0
            && self.width != 0
            && self.height != 0
            && self.stride >= self.width
            && self.format.is_direct()
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    Unknown = 0,
    Rgb = 1,
    Bgr = 2,
}

impl PixelFormat {
    pub const fn is_direct(self) -> bool {
        matches!(self, Self::Rgb | Self::Bgr)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub start: u64,
    pub page_count: u64,
    pub kind: MemoryRegionKind,
    pub reserved: u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRegionKind {
    Usable = 0,
    Reserved = 1,
    Loader = 2,
    BootServices = 3,
    RuntimeServices = 4,
    AcpiReclaimable = 5,
    AcpiNvs = 6,
    Mmio = 7,
    Persistent = 8,
    Unusable = 9,
}
