//! Physical-page allocation, address-space construction, and user ELF loading.
//!
//! The memory subsystem remains a single kernel-facing API at `crate::memory`,
//! but its internal responsibilities are split into focused modules:
//! bootstrap/kernel mappings, allocator state, page-table construction, user
//! image loading, and validated user-buffer access.

mod allocator;
mod kernel;
mod loader;
mod paging;
mod state;
mod types;
mod user;
mod util;

pub use kernel::{
    allocate_kernel_buffer, allocate_kernel_page, init, kernel_address_space,
    map_kernel_identity_range,
};
pub use loader::load_user_program;
pub use types::{
    AddressSpace, KernelBuffer, LoadedUserProgram, MemoryError, OwnedPages, ProgramStartup,
    PAGE_SIZE, USER_IMAGE_BASE, USER_STACK_PAGES, USER_STACK_TOP,
};
pub use user::{user_slice, user_slice_mut, validate_user_buffer};
