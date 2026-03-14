use core::ptr::copy_nonoverlapping;
use core::slice;

use elf::{ElfImage, PF_W, PT_LOAD};

use super::kernel::map_shared_kernel_context;
use super::paging::AddressSpaceBuilder;
use super::state::with_state_mut;
use super::types::{
    AddressSpace, LoadedUserProgram, MemoryError, OwnedPages, ProgramStartup, MAX_SEGMENT_PAGES,
    MAX_STARTUP_ARGS, PAGE_PRESENT, PAGE_SIZE, PAGE_USER, PAGE_WRITABLE, USER_IMAGE_BASE,
    USER_STACK_PAGES, USER_STACK_TOP,
};
use super::util::{align_down, align_up};

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
            let root_paddr = state.allocator.allocate_page()?;
            owned_pages.push(root_paddr)?;
            let mut builder = AddressSpaceBuilder::new(root_paddr, Some(&mut owned_pages));
            map_shared_kernel_context(&mut builder)?;

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
                        let phys = state.allocator.allocate_page()?;
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
                let phys = state.allocator.allocate_page()?;
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

fn contains_user_address(address: u64) -> bool {
    address >= USER_IMAGE_BASE && address < USER_STACK_TOP
}

fn write_startup_arguments(
    pages: &UserPageMap,
    startup: &ProgramStartup<'_>,
) -> Result<u64, MemoryError> {
    let layout = prepare_startup_layout(startup)?;

    let mut argv_pointers = [0u64; MAX_STARTUP_ARGS + 1];
    let mut string_cursor = layout.strings_start;
    let null_byte = [0u8; 1];
    let mut arg_index = 0usize;
    while arg_index < layout.argc {
        argv_pointers[arg_index] = string_cursor;
        pages.copy_into(string_cursor, layout.args[arg_index])?;
        string_cursor += layout.args[arg_index].len() as u64;
        pages.copy_into(string_cursor, &null_byte)?;
        string_cursor += 1;
        arg_index += 1;
    }
    argv_pointers[layout.argc] = 0;

    let argc_bytes = (layout.argc as u64).to_ne_bytes();
    pages.copy_into(layout.stack_start, &argc_bytes)?;
    let argv_pointer_bytes = unsafe {
        slice::from_raw_parts(
            argv_pointers.as_ptr() as *const u8,
            (layout.argc + 1) * core::mem::size_of::<u64>(),
        )
    };
    pages.copy_into(
        layout.stack_start + core::mem::size_of::<u64>() as u64,
        argv_pointer_bytes,
    )?;

    Ok(layout.stack_start)
}

struct StartupLayout<'a> {
    args: [&'a [u8]; MAX_STARTUP_ARGS],
    argc: usize,
    strings_start: u64,
    stack_start: u64,
}

fn prepare_startup_layout<'a>(
    startup: &'a ProgramStartup<'a>,
) -> Result<StartupLayout<'a>, MemoryError> {
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

    Ok(StartupLayout {
        args,
        argc,
        strings_start,
        stack_start,
    })
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

#[cfg(test)]
mod tests {
    use super::prepare_startup_layout;
    use crate::memory::{MemoryError, ProgramStartup, USER_STACK_TOP};

    #[test]
    fn startup_layout_accepts_valid_arguments() {
        let startup = ProgramStartup {
            argv0: "/system/bin/lash",
            argv_tail: b"-c\0echo\0",
        };

        let layout = prepare_startup_layout(&startup).unwrap();
        assert_eq!(layout.argc, 3);
        assert!(layout.stack_start < USER_STACK_TOP);
        assert!(layout.strings_start < USER_STACK_TOP);
    }

    #[test]
    fn startup_layout_rejects_non_utf8_arguments() {
        let startup = ProgramStartup {
            argv0: "/system/bin/lash",
            argv_tail: b"\xff\0",
        };

        assert!(matches!(
            prepare_startup_layout(&startup),
            Err(MemoryError::InvalidStartupArguments)
        ));
    }

    #[test]
    fn startup_layout_rejects_missing_terminator() {
        let startup = ProgramStartup {
            argv0: "/system/bin/lash",
            argv_tail: b"unterminated",
        };

        assert!(matches!(
            prepare_startup_layout(&startup),
            Err(MemoryError::InvalidStartupArguments)
        ));
    }
}
