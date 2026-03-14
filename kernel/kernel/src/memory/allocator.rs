use core::ptr::write_bytes;

use super::types::{MemoryError, MAX_FREE_RANGES, PAGE_SIZE};

pub(super) struct PhysicalAllocator {
    regions: [PhysicalRegion; MAX_FREE_RANGES],
    count: usize,
}

impl PhysicalAllocator {
    pub(super) const fn new() -> Self {
        Self {
            regions: [PhysicalRegion::empty(); MAX_FREE_RANGES],
            count: 0,
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn add_region(&mut self, start: u64, end: u64) -> Result<(), MemoryError> {
        self.insert_range(start, end);
        Ok(())
    }

    pub(super) fn allocate_page(&mut self) -> Result<u64, MemoryError> {
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

    pub(super) fn allocate_contiguous_pages(&mut self, count: usize) -> Result<u64, MemoryError> {
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

    pub(super) fn free_contiguous_pages(&mut self, start: u64, count: usize) {
        if count == 0 {
            return;
        }

        let bytes = (count as u64) * PAGE_SIZE as u64;
        self.insert_range(start, start + bytes);
    }

    pub(super) fn reserve_range(&mut self, start: u64, end: u64) {
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

#[cfg(test)]
mod tests {
    use std::boxed::Box;

    use super::{MemoryError, PhysicalAllocator};
    use crate::memory::PAGE_SIZE;

    #[repr(align(4096))]
    struct HostPage([u8; PAGE_SIZE]);

    fn host_region<const N: usize>() -> (Box<[HostPage; N]>, u64, u64) {
        let pages = Box::new(std::array::from_fn(|_| HostPage([0; PAGE_SIZE])));
        let start = pages.as_ptr() as u64;
        let end = start + (N as u64) * PAGE_SIZE as u64;
        (pages, start, end)
    }

    #[test]
    fn merges_adjacent_inserted_ranges() {
        let (_pages, start, _) = host_region::<4>();
        let mut allocator = PhysicalAllocator::new();
        allocator
            .add_region(start, start + PAGE_SIZE as u64)
            .unwrap();
        allocator
            .add_region(start + PAGE_SIZE as u64, start + 3 * PAGE_SIZE as u64)
            .unwrap();

        assert_eq!(allocator.count, 1);
        assert_eq!(allocator.regions[0].start, start);
        assert_eq!(allocator.regions[0].end, start + 3 * PAGE_SIZE as u64);
    }

    #[test]
    fn reserve_range_splits_region() {
        let (_pages, start, end) = host_region::<4>();
        let mut allocator = PhysicalAllocator::new();
        allocator.add_region(start, end).unwrap();

        allocator.reserve_range(start + PAGE_SIZE as u64, start + 2 * PAGE_SIZE as u64);

        assert_eq!(allocator.count, 2);
        assert_eq!(allocator.regions[0].start, start);
        assert_eq!(allocator.regions[0].end, start + PAGE_SIZE as u64);
        assert_eq!(allocator.regions[1].start, start + 2 * PAGE_SIZE as u64);
        assert_eq!(allocator.regions[1].end, end);
    }

    #[test]
    fn allocate_page_zeros_memory_and_exhausts() {
        let (mut pages, start, end) = host_region::<1>();
        let mut allocator = PhysicalAllocator::new();
        allocator.add_region(start, end).unwrap();
        pages[0].0.fill(0xaa);

        let page = allocator.allocate_page().unwrap();
        assert_eq!(page, start);
        assert!(pages[0].0.iter().all(|byte| *byte == 0));
        assert!(matches!(
            allocator.allocate_page(),
            Err(MemoryError::AllocatorExhausted)
        ));
    }

    #[test]
    fn allocate_contiguous_pages_zeros_memory_and_exhausts() {
        let (mut pages, start, end) = host_region::<2>();
        let mut allocator = PhysicalAllocator::new();
        allocator.add_region(start, end).unwrap();
        pages[0].0.fill(0xaa);
        pages[1].0.fill(0xbb);

        let allocated = allocator.allocate_contiguous_pages(2).unwrap();
        assert_eq!(allocated, start);
        assert!(pages
            .iter()
            .all(|page| page.0.iter().all(|byte| *byte == 0)));
        assert!(matches!(
            allocator.allocate_contiguous_pages(1),
            Err(MemoryError::AllocatorExhausted)
        ));
    }
}
