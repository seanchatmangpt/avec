//! 64-byte aligned buffer - the memory foundation layer.
//!
//! Zero external dependencies: `alloc::alloc` honors alignment requests, so a
//! plain alloc with `Layout::from_size_align(bytes, 64)` gives the hardware
//! invariant directly from the global allocator.

use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::alloc::Layout;
use core::ptr::{self, NonNull};

/// Allocation alignment for every avec buffer (charter M-03).
pub const ALIGNMENT: usize = 64;

/// An owned, 64-byte aligned byte buffer. Uninitialized on alloc: every byte
/// a caller reads must have been written first (callers zero regions they
/// read via [`Buf::new_zeroed`]).
#[derive(Debug)]
pub struct Buf {
    ptr: NonNull<u8>,
    len: usize,
}

impl Buf {
    /// Allocate `len` bytes, 64-byte aligned.
    ///
    /// # Panics
    /// On allocation failure.
    pub fn new(len: usize) -> Self {
        if len == 0 {
            // 64-aligned non-null sentinel: keeps the M-03 pointer law
            // (ptr & 63 == 0) meaningful even for empty buffers; never
            // dereferenced (len 0, no allocation).
            return Buf {
                // SAFETY: address 64 is non-null and 64-byte aligned by construction.
                ptr: unsafe { NonNull::new_unchecked(ALIGNMENT as *mut u8) },
                len: 0,
            };
        }
        let layout = Layout::from_size_align(len, ALIGNMENT)
            .unwrap_or_else(|e| panic!("invalid buffer layout: {e}"));
        // SAFETY: layout has non-zero size.
        let raw = unsafe { alloc(layout) };
        match NonNull::new(raw) {
            Some(ptr) => Buf { ptr, len },
            None => handle_alloc_error(layout),
        }
    }

    /// Allocate zeroed.
    pub fn new_zeroed(len: usize) -> Self {
        let buf = Self::new(len);
        // SAFETY: len bytes are owned.
        unsafe { ptr::write_bytes(buf.as_mut_ptr(), 0, len) };
        buf
    }

    /// Byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when holding no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Base address; 64-byte aligned whenever len > 0 (charter M-03).
    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    /// Mutable base address.
    #[must_use]
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Byte view.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: ptr valid for len bytes for the life of &self.
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len) }
    }

    /// Mutable byte view.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: ptr valid for len bytes for the life of &mut self.
        unsafe { core::slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }

    /// Typed view: `len / size_of::<T>()` elements.
    ///
    /// # Panics
    /// When the byte length is not a multiple of the element size.
    #[must_use]
    pub fn as_typed<T>(&self) -> &[T] {
        let (sz, an) = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
        assert!(
            an <= ALIGNMENT,
            "element alignment exceeds buffer alignment"
        );
        assert_eq!(
            self.len % sz,
            0,
            "buffer length not a multiple of element size"
        );
        // SAFETY: 64-byte base + uniform element size -> aligned for T.
        unsafe { core::slice::from_raw_parts(self.as_ptr() as *const T, self.len / sz) }
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        if self.len == 0 {
            return;
        }
        // SAFETY: allocated with exactly this layout.
        unsafe {
            dealloc(
                self.as_mut_ptr(),
                Layout::from_size_align_unchecked(self.len, ALIGNMENT),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment_invariant() {
        for len in [1usize, 63, 64, 65, 4096, 100_003] {
            let b = Buf::new(len);
            assert_eq!(b.as_ptr() as usize % 64, 0, "len {len} misaligned");
            assert_eq!(b.len(), len);
            // Write-before-read discipline: zero then read back.
            let mut b = b;
            b.as_mut_slice().fill(0xAB);
            assert!(b.as_slice().iter().all(|&x| x == 0xAB));
        }
    }

    #[test]
    fn zeroed_and_typed_views() {
        let b = Buf::new_zeroed(64 * 4);
        assert!(b.as_slice().iter().all(|&x| x == 0));
        let v = b.as_typed::<u32>();
        assert_eq!(v.len(), 64);
        assert!(v.iter().all(|&x| x == 0));
    }

    #[test]
    fn empty_buffer_dangling() {
        let b = Buf::new(0);
        assert!(b.is_empty());
        assert_eq!(b.len(), 0);
    }
}
