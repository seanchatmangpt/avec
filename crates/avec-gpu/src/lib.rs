//! Metal Unified Memory Zero-Copy Bridge for avec.
//!
//! Provides zero-copy wrapping of 64-byte aligned and 16 KiB page-aligned Arrow
//! buffers (`Buf` / `MmuBuf`) directly into Apple Silicon Metal Unified Memory
//! (`MTLResourceStorageModeShared`).
//!
//! # Safety
//! Pointers wrapped into `MetalSharedBuffer` must remain valid for the lifetime
//! of the buffer reference.

use core::ffi::c_void;

extern "C" {
    fn avec_metal_create_shared_buffer(bytes: *mut c_void, length: usize) -> *mut c_void;
    fn avec_metal_release_buffer(buffer_ref: *mut c_void);
}

/// Freestanding wrapper around a zero-copy Metal Unified Memory buffer (`id<MTLBuffer>`).
pub struct MetalSharedBuffer {
    raw: *mut c_void,
    len: usize,
}

impl MetalSharedBuffer {
    /// Creates a new `MTLBuffer` wrapping existing aligned memory with zero bus copies.
    ///
    /// Returns `None` if Metal device initialization fails or if length is 0.
    ///
    /// # Safety
    /// The caller must ensure `ptr` points to at least `len` initialized bytes
    /// and that `ptr` remains valid and unmodified for the lifetime of this buffer.
    pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> Option<Self> {
        if ptr.is_null() || len == 0 {
            return None;
        }

        let raw = avec_metal_create_shared_buffer(ptr as *mut c_void, len);
        if raw.is_null() {
            None
        } else {
            Some(Self { raw, len })
        }
    }

    /// Wraps an `avec_arrow::Buf` with zero memory copies.
    #[inline]
    pub fn from_buf(buf: &avec_arrow::Buf) -> Option<Self> {
        unsafe { Self::from_raw_parts(buf.as_ptr(), buf.len()) }
    }

    /// Wraps an `avec_arrow::mmu_arena::MmuBuf` with zero memory copies.
    #[inline]
    pub fn from_mmu_buf(buf: &avec_arrow::mmu_arena::MmuBuf) -> Option<Self> {
        unsafe { Self::from_raw_parts(buf.as_ptr(), buf.len()) }
    }

    /// Returns the length of the wrapped buffer in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the wrapped buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Raw pointer to the underlying Metal buffer handle (`id<MTLBuffer>`).
    #[inline]
    pub fn raw_handle(&self) -> *mut c_void {
        self.raw
    }
}

impl Drop for MetalSharedBuffer {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                avec_metal_release_buffer(self.raw);
            }
            self.raw = core::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avec_arrow::Buf;

    #[test]
    fn test_metal_shared_buffer_zero_copy() {
        let mut buf = Buf::new(16384);
        for (i, byte) in buf.as_mut_slice().iter_mut().enumerate() {
            *byte = (i & 0xff) as u8;
        }

        let metal_buf = MetalSharedBuffer::from_buf(&buf);
        assert!(metal_buf.is_some(), "Metal unified memory buffer creation failed");
        let metal_buf = metal_buf.unwrap();
        assert_eq!(metal_buf.len(), buf.len());
        assert!(!metal_buf.raw_handle().is_null());
    }
}
