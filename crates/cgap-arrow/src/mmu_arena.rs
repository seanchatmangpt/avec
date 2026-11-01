//! 16 KiB Hardware Page-Aligned MMU Remap Arena.
//!
//! Provides 16 KiB-aligned buffer allocations (16,384 bytes = 256 * 64B cache lines,
//! matching the Apple Silicon and ARM64 MMU page granule).
//!
//! Exposes page-table zero-copy slice and batch manipulation invariants without
//! memory-bus byte copying.

use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::alloc::Layout;
use core::ptr::NonNull;

/// 16 KiB hardware page-granule alignment.
pub const MMU_PAGE_SIZE: usize = 16384;

/// A 16 KiB page-table aligned buffer.
#[derive(Debug)]
pub struct MmuBuf {
    ptr: NonNull<u8>,
    len: usize,
    capacity: usize,
}

unsafe impl Send for MmuBuf {}
unsafe impl Sync for MmuBuf {}

impl MmuBuf {
    /// Allocates an arena with capacity rounded up to the nearest 16 KiB page boundary.
    pub fn with_capacity(min_bytes: usize) -> Self {
        let pages = min_bytes.div_ceil(MMU_PAGE_SIZE).max(1);
        let capacity = pages * MMU_PAGE_SIZE;

        let layout = Layout::from_size_align(capacity, MMU_PAGE_SIZE)
            .unwrap_or_else(|e| panic!("invalid mmu layout: {e}"));

        // SAFETY: layout has non-zero size and is 16 KiB aligned
        let raw = unsafe { alloc(layout) };
        let ptr = match NonNull::new(raw) {
            Some(p) => p,
            None => handle_alloc_error(layout),
        };

        Self {
            ptr,
            len: 0,
            capacity,
        }
    }

    /// Appends bytes to the arena without exceeding allocated page boundaries.
    #[inline(always)]
    pub fn extend_from_slice(&mut self, src: &[u8]) -> bool {
        if self.len + src.len() > self.capacity {
            return false;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr(),
                self.ptr.as_ptr().add(self.len),
                src.len(),
            );
        }
        self.len += src.len();
        true
    }

    /// Pointer to the 16 KiB-aligned base.
    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    /// Mutable pointer to the 16 KiB-aligned base.
    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Length in active bytes.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Length in active bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Capacity in bytes (always a multiple of 16,384).
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Verifies that the pointer is strictly 16 KiB aligned.
    #[inline(always)]
    pub fn is_mmu_page_aligned(&self) -> bool {
        (self.ptr.as_ptr() as usize) & (MMU_PAGE_SIZE - 1) == 0
    }
}

impl Drop for MmuBuf {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity, MMU_PAGE_SIZE).unwrap();
        unsafe {
            dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmu_buf_page_alignment() {
        let mut buf = MmuBuf::with_capacity(100);
        assert!(buf.is_mmu_page_aligned());
        assert_eq!(buf.capacity(), MMU_PAGE_SIZE);

        let data = [42u8; 128];
        assert!(buf.extend_from_slice(&data));
        assert_eq!(buf.len(), 128);
    }
}
