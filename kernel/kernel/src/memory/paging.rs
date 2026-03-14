#[cfg(test)]
use core::ptr::write_bytes;
use lzutil::{align_down, align_up};

use super::state::with_state_mut;
use super::types::{MemoryError, OwnedPages, PAGE_HUGE, PAGE_PRESENT, PAGE_SIZE, PAGE_TABLE_FLAGS};
use super::util::{pd_index, pdpt_index, pml4_index, pt_index};

pub(super) const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;

pub(super) struct AddressSpaceBuilder {
    root_paddr: u64,
    owned_pages: Option<*mut OwnedPages>,
}

impl AddressSpaceBuilder {
    pub(super) fn new(root_paddr: u64, owned_pages: Option<&mut OwnedPages>) -> Self {
        Self {
            root_paddr,
            owned_pages: owned_pages.map(|owned_pages| owned_pages as *mut OwnedPages),
        }
    }

    pub(super) fn map_identity_2m_range(
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

    pub(super) fn map_identity_4k_range(
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

    pub(super) fn map_2m(&mut self, virt: u64, phys: u64, flags: u64) -> Result<(), MemoryError> {
        debug_assert_eq!(virt % (2 * 1024 * 1024), 0);
        debug_assert_eq!(phys % (2 * 1024 * 1024), 0);
        let pd = self.ensure_page_directory(virt, flags & super::types::PAGE_USER)?;
        let index = pd_index(virt);
        unsafe {
            page_table_mut(pd).entries[index] = phys | flags | PAGE_HUGE;
        }
        Ok(())
    }

    pub(super) fn map_4k(&mut self, virt: u64, phys: u64, flags: u64) -> Result<(), MemoryError> {
        debug_assert_eq!(virt % PAGE_SIZE as u64, 0);
        debug_assert_eq!(phys % PAGE_SIZE as u64, 0);

        let pt = self.ensure_page_table(virt, flags & super::types::PAGE_USER)?;
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

        let child = with_state_mut(|state| state.allocator.allocate_page())?;
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

#[repr(C, align(4096))]
pub(super) struct PageTable {
    entries: [u64; 512],
}

pub(super) unsafe fn page_table_mut(address: u64) -> &'static mut PageTable {
    &mut *(address as *mut PageTable)
}

#[cfg(test)]
pub(super) unsafe fn initialize_empty_page_table(address: u64) {
    write_bytes(address as *mut u8, 0, PAGE_SIZE);
}

#[cfg(test)]
mod tests {
    use super::{initialize_empty_page_table, AddressSpaceBuilder, ADDRESS_MASK};
    use crate::memory::state::with_state_mut;
    use crate::memory::util::{pd_index, pdpt_index, pml4_index, pt_index};
    use crate::memory::{MemoryError, OwnedPages, PAGE_SIZE};
    use std::boxed::Box;

    #[repr(align(4096))]
    struct HostPage([u8; PAGE_SIZE]);

    fn host_pages<const N: usize>() -> Box<[HostPage; N]> {
        Box::new(std::array::from_fn(|_| HostPage([0; PAGE_SIZE])))
    }

    #[test]
    fn table_index_helpers_extract_expected_bits() {
        let address = 0x1234_5678_9abc_u64;
        assert_eq!(pml4_index(address), 36);
        assert_eq!(pdpt_index(address), 209);
        assert_eq!(pd_index(address), 179);
        assert_eq!(pt_index(address), 393);
    }

    #[test]
    fn map_4k_populates_page_tables() {
        let pages = host_pages::<5>();
        let root = pages[0].0.as_ptr() as u64;
        let pdpt = pages[1].0.as_ptr() as u64;
        let pd = pages[2].0.as_ptr() as u64;
        let pt = pages[3].0.as_ptr() as u64;
        let phys = pages[4].0.as_ptr() as u64;
        let virt = 0x0040_3000_u64;

        unsafe {
            initialize_empty_page_table(root);
            initialize_empty_page_table(pdpt);
            initialize_empty_page_table(pd);
            initialize_empty_page_table(pt);
        }

        with_state_mut(|state| {
            state.allocator.reset();
            state
                .allocator
                .add_region(pdpt, pt + PAGE_SIZE as u64)
                .unwrap();
        });

        let mut owned_pages = OwnedPages::empty();
        let mut builder = AddressSpaceBuilder::new(root, Some(&mut owned_pages));
        builder
            .map_4k(
                virt,
                phys,
                super::super::types::PAGE_PRESENT | super::super::types::PAGE_USER,
            )
            .unwrap();

        unsafe {
            let root_entry = super::page_table_mut(root).entries[pml4_index(virt)];
            let pdpt_entry = super::page_table_mut(pdpt).entries[pdpt_index(virt)];
            let pd_entry = super::page_table_mut(pd).entries[pd_index(virt)];
            let pt_entry = super::page_table_mut(pt).entries[pt_index(virt)];
            assert_eq!(root_entry & ADDRESS_MASK, pdpt);
            assert_eq!(pdpt_entry & ADDRESS_MASK, pd);
            assert_eq!(pd_entry & ADDRESS_MASK, pt);
            assert_eq!(pt_entry & ADDRESS_MASK, phys);
        }
    }

    #[test]
    fn map_4k_reports_allocator_exhaustion() {
        let pages = host_pages::<1>();
        let root = pages[0].0.as_ptr() as u64;
        unsafe {
            initialize_empty_page_table(root);
        }
        with_state_mut(|state| state.allocator.reset());

        let mut builder = AddressSpaceBuilder::new(root, None);
        assert!(matches!(
            builder.map_4k(0x4000, 0x8000, super::super::types::PAGE_PRESENT),
            Err(MemoryError::AllocatorExhausted)
        ));
    }
}
