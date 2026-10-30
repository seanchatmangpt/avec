//! C Data Interface ABI — zero-copy export/import (charter layer avec-abi).
//!
//! `ArrowArray` / `ArrowSchema` exactly as the spec defines them. Release
//! safety (FMEA): the release callback is atomically swapped to NULL before
//! private data drops, so a double release is a typed no-op, never a double
//! free.

use crate::bitmap::Bitmap;
use crate::buffer::Buf;
use crate::primitive::{Primitive, Scalar};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicPtr, Ordering};

/// ARROW_FLAG_NULLABLE (ontology: av:FlagNullable, cFlagValue 2).
pub const AVEC_FLAG_NULLABLE: i64 = 2;
/// ARROW_FLAG_DICTIONARY_ORDERED (av:FlagDictionaryOrdered, 1).
pub const AVEC_FLAG_DICTIONARY_ORDERED: i64 = 1;
/// ARROW_FLAG_MAP_KEYS_SORTED (av:FlagMapKeysSorted, 4).
pub const AVEC_FLAG_MAP_KEYS_SORTED: i64 = 4;

/// struct ArrowSchema (C Data Interface spec).
#[repr(C)]
pub struct ArrowSchema {
    pub format: *const c_char,
    pub name: *const c_char,
    pub metadata: *const c_char,
    pub flags: i64,
    pub n_children: i64,
    pub children: *mut *mut ArrowSchema,
    pub dictionary: *mut ArrowSchema,
    pub release: Option<unsafe fn(*mut ArrowSchema)>,
    pub private_data: *mut c_void,
}

/// struct ArrowArray (C Data Interface spec).
#[repr(C)]
pub struct ArrowArray {
    pub length: i64,
    pub null_count: i64,
    pub offset: i64,
    pub n_buffers: i64,
    pub n_children: i64,
    pub buffers: *mut *const c_void,
    pub children: *mut *mut ArrowArray,
    pub dictionary: *mut ArrowArray,
    pub release: Option<unsafe fn(*mut ArrowArray)>,
    pub private_data: *mut c_void,
}

/// Schema release: CAS swap-to-NULL then free the boxed schema shell
/// (private_data aliases the boxed struct itself).
unsafe fn release_schema(schema: *mut ArrowSchema) {
    let slot: &AtomicPtr<c_void> =
        unsafe { &*(core::ptr::addr_of_mut!((*schema).release) as *mut AtomicPtr<c_void>) };
    let prev = slot.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if prev.is_null() {
        return;
    }
    // SAFETY: first-release winner; the box was created in `export`.
    unsafe { drop(Box::from_raw(schema)) };
}

/// Owned batch: the release callback drops exactly this.
struct OwnedPrimitive<T: Scalar> {
    validity: Option<Bitmap>,
    values: Buf,
    buffers: Box<[*const c_void]>,
    len: usize,
    _marker: core::marker::PhantomData<fn() -> T>,
}

/// SAFETY: single release via atomic swap-to-NULL; the first caller wins,
/// later calls observe `release == NULL` and return without touching memory.
unsafe fn release_array<T: Scalar>(array: *mut ArrowArray) {
    // Option<unsafe fn> is a nullable pointer with a niche: identical layout
    // to *mut c_void. AtomicPtr<c_void> is repr(transparent) over *mut c_void,
    // so an in-place atomic view of the release slot is a pointer pun.
    let slot: &AtomicPtr<c_void> = unsafe {
        &*(core::ptr::addr_of_mut!((*array).release) as *mut AtomicPtr<c_void>)
    };
    let prev = slot.swap(core::ptr::null_mut(), Ordering::AcqRel);
    if prev.is_null() {
        // Already released (or NULL release): typed no-op, never a double free.
        return;
    }
    // SAFETY: first-release winner; private_data is a live Box<OwnedPrimitive<T>>.
    let private = unsafe { (*array).private_data };
    if !private.is_null() {
        // SAFETY: recreated exactly as boxed in `export`.
        unsafe { drop(Box::from_raw(private as *mut OwnedPrimitive<T>)) };
    }
}

/// Export a primitive vector as an owned C Data Interface pair.
///
/// Ownership transfers to the importer: when done it calls `release` on both
/// structs (spec contract), which frees the arena exactly once.
pub fn export<T: Scalar>(
    arr: Primitive<T>,
    format: *const c_char,
) -> (ArrowArray, ArrowSchema) {
    let (validity, values, len) = arr.into_parts();
    let null_count = validity
        .as_ref()
        .map_or(0i64, |bm| bm.null_count() as i64);
    let mut bufs: Vec<*const c_void> = Vec::with_capacity(2);
    bufs.push(match &validity {
        Some(bm) => bm.as_ptr() as *const c_void,
        None => core::ptr::null(),
    });
    bufs.push(values.as_ptr() as *const c_void);
    let owned = Box::into_raw(Box::new(OwnedPrimitive::<T> {
        validity,
        values,
        buffers: bufs.into_boxed_slice(),
        len,
        _marker: core::marker::PhantomData,
    }));
    // SAFETY: owned is a live Box<OwnedPrimitive>; read the buffer slice ptr.
    let buffers_ptr = unsafe { (*owned).buffers.as_mut_ptr() };
    let array = ArrowArray {
        length: len as i64,
        null_count,
        offset: 0,
        n_buffers: 2,
        n_children: 0,
        buffers: buffers_ptr,
        children: core::ptr::null_mut(),
        dictionary: core::ptr::null_mut(),
        release: Some(release_array::<T>),
        private_data: owned as *mut c_void,
    };
    let boxed_schema = Box::into_raw(Box::new(ArrowSchema {
        format,
        name: core::ptr::null(),
        metadata: core::ptr::null(),
        flags: AVEC_FLAG_NULLABLE,
        n_children: 0,
        children: core::ptr::null_mut(),
        dictionary: core::ptr::null_mut(),
        release: Some(release_schema),
        private_data: core::ptr::null_mut(),
    }));
    // SAFETY: boxed_schema is live; private_data aliases the box itself so
    // release_schema can free the shell.
    unsafe {
        (*boxed_schema).private_data = boxed_schema as *mut c_void;
    }
    // SAFETY: read back the finalized box value.
    let schema = unsafe { core::ptr::read(boxed_schema) };
    (array, schema)
}

/// Import a foreign array: wraps the raw pair; dropping calls the foreign
/// release exactly once (importer duty per spec).
pub struct ForeignArray {
    pub array: *mut ArrowArray,
}

impl ForeignArray {
    /// # Safety
    /// `array` must be a live, spec-compliant ArrowArray whose release the
    /// caller now owns (last reference).
    pub unsafe fn take(array: *mut ArrowArray) -> Self {
        ForeignArray { array }
    }

    /// Row count.
    #[must_use]
    pub fn len(&self) -> i64 {
        // SAFETY: liveness guaranteed at construction; single owner.
        unsafe { (*self.array).length }
    }

    /// Buffer slot pointers.
    ///
    /// # Safety
    /// Same liveness contract as [`ForeignArray::take`].
    pub unsafe fn buffer(&self, i: usize) -> *const c_void {
        // SAFETY: liveness guaranteed at construction; bounds asserted.
        unsafe {
            let n = (*self.array).n_buffers as usize;
            assert!(i < n, "buffer index out of range");
            *(*self.array).buffers.add(i)
        }
    }
}

impl Drop for ForeignArray {
    fn drop(&mut self) {
        // SAFETY: single-owner import; release is called exactly once.
        unsafe {
            if let Some(release) = (*self.array).release {
                release(self.array);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::PrimitiveBuilder;

    #[test]
    fn export_then_release_is_single_free() {
        let mut b = PrimitiveBuilder::<i32>::new();
        b.push(Some(42));
        b.push(None);
        b.push(Some(-1));
        let arr = b.finish();
        let format = b"i\0".as_ptr() as *const c_char;
        let (array, _schema) = export(arr, format);
        assert_eq!(array.length, 3);
        assert_eq!(array.null_count, 1);
        assert_eq!(array.n_buffers, 2);
        let ptr = Box::into_raw(Box::new(array));
        // SAFETY: test-owned heap ArrowArray; release twice to prove the CAS
        // swap makes the second call a no-op.
        unsafe {
            let f = (*ptr).release.unwrap();
            f(ptr);
            f(ptr); // double release: typed no-op, no double free
            drop(Box::from_raw(ptr)); // free the struct shell itself
        }
    }
}
