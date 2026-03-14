use core::slice;

use super::types::{PAGE_SIZE, USER_IMAGE_BASE, USER_STACK_PAGES, USER_STACK_TOP};

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

#[cfg(test)]
mod tests {
    use super::validate_user_buffer;
    use crate::memory::{USER_IMAGE_BASE, USER_STACK_PAGES, USER_STACK_TOP};

    #[test]
    fn accepts_zero_length_ranges() {
        assert!(validate_user_buffer(0, 0));
    }

    #[test]
    fn accepts_ranges_within_user_image() {
        assert!(validate_user_buffer(USER_IMAGE_BASE, 64));
    }

    #[test]
    fn rejects_overflowing_ranges() {
        assert!(!validate_user_buffer(u64::MAX - 4, 8));
    }

    #[test]
    fn rejects_ranges_that_cross_into_stack_gap() {
        let user_stack_base = USER_STACK_TOP - ((USER_STACK_PAGES as u64) * 4096);
        assert!(!validate_user_buffer(user_stack_base - 16, 32));
    }

    #[test]
    fn accepts_ranges_entirely_within_stack() {
        let user_stack_base = USER_STACK_TOP - ((USER_STACK_PAGES as u64) * 4096);
        assert!(validate_user_buffer(user_stack_base, 64));
    }
}
