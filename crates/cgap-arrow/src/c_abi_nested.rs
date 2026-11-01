//! Nested/child C-ABI export support (wave-3 lane W3-NESTED owns this file).
//!
//! Struct-of-children export over the C Data Interface: a parent
//! `ArrowArray` / `ArrowSchema` pair with `n_children > 0` and schema format
//! `+s`, reusing the atomic release-swap (CAS) discipline of
//! [`crate::c_abi`].
//!
//! Ownership table — who frees what, exactly once:
//!
//! | object                       | freed by                                    |
//! |------------------------------|---------------------------------------------|
//! | parent array private arena   | `release_struct_array` (CAS winner only)    |
//! | parent array shell           | whoever boxes the returned value            |
//! | parent schema private arena  | `release_struct_schema` (CAS winner only)   |
//! | parent schema shell          | whoever boxes the returned value            |
//! | child array private data     | the child's own release (CAS-guarded)       |
//! | child array shell            | `release_struct_array` cascade              |
//! | child schema shell + private | the child's own release (house convention:  |
//! |                              | frees the aliased shell)                    |
//!
//! FMEA: the parent release slot is CAS-swapped to NULL *before* the
//! cascade, so a double parent release returns before touching any child;
//! each child release is itself a CAS swap-to-NULL, so the cascade can
//! never release a child twice.

use crate::c_abi::{ArrowArray, ArrowSchema, CGAP_FLAG_NULLABLE};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicPtr, Ordering};

/// Array-side private arena: owns the heap children-pointer array and the
/// single validity slot of the struct layout. Child shells are owned by the
/// cascade (see the module ownership table).
struct OwnedStructArray {
    children: Box<[*mut ArrowArray]>,
    buffers: Box<[*const c_void]>,
}

/// Schema-side private arena: owns the heap children-pointer array.
struct OwnedStructSchema {
    children: Box<[*mut ArrowSchema]>,
}

/// Swap the release slot to NULL and return the previous value.
///
/// Target-class split (blocker fix, v26.11.1): `AtomicPtr::swap` is
/// cfg-gated out on targets without pointer RMW atomics
/// (`target_has_atomic = "ptr"` is false on e.g. `riscv32imc` — no A
/// extension), so the release cascade must take the portable path there.
///
/// - Pointer-atomic targets: one real atomic RMW (`swap`, AcqRel) — the
///   exactly-once guarantee is the hardware RMW itself.
/// - No-RMW targets: `load`/`store` still exist (atomic load-store is
///   available); the serialization point is the target's execution model
///   (single-hart, no threads — `std::thread` does not exist on
///   `--target riscv32imc-unknown-none-elf`), so no other execution context
///   can interleave between the load and the store.
///
/// Both paths preserve the release discipline unchanged: the slot reads
/// NULL to every later observer strictly before any cascade work runs.
fn release_swap(slot: &AtomicPtr<c_void>) -> *mut c_void {
    #[cfg(target_has_atomic = "ptr")]
    {
        slot.swap(core::ptr::null_mut(), Ordering::AcqRel)
    }
    #[cfg(not(target_has_atomic = "ptr"))]
    {
        let prev = slot.load(Ordering::Acquire);
        slot.store(core::ptr::null_mut(), Ordering::Release);
        prev
    }
}

/// SAFETY: single release via atomic swap-to-NULL (same discipline as
/// `c_abi`: `Option<unsafe fn>` is a nullable pointer with a niche, so an
/// in-place `AtomicPtr<c_void>` view of the release slot is a pointer pun).
/// The first caller wins, cascades every child release exactly once, frees
/// the child shells and the private arena. Later calls observe
/// `release == NULL` and return without touching memory. The parent shell
/// itself is freed by whoever boxed the returned struct.
unsafe fn release_struct_array(array: *mut ArrowArray) {
    let slot: &AtomicPtr<c_void> =
        unsafe { &*(core::ptr::addr_of_mut!((*array).release) as *mut AtomicPtr<c_void>) };
    // Parent slot swapped to NULL BEFORE the cascade (FMEA, module head):
    // a concurrent/reentrant second release observes NULL and returns
    // before touching any child. Target-gated RMW via `release_swap`.
    let prev = release_swap(slot);
    if prev.is_null() {
        // Already released: typed no-op, never a double free.
        return;
    }
    let private = unsafe { (*array).private_data };
    if !private.is_null() {
        // SAFETY: recreated exactly as boxed in `export_struct`.
        let owned = unsafe { Box::from_raw(private as *mut OwnedStructArray) };
        for &child in owned.children.iter() {
            // SAFETY: the shell was boxed in `export_struct` and is still
            // alive: the parent release slot was live until the swap above.
            if let Some(release) = unsafe { (*child).release } {
                // SAFETY: the child's own release is CAS-guarded, so exactly
                // one call ever frees its private data.
                unsafe { release(child) };
            }
            // SAFETY: the shell box from `export_struct`; a child release
            // frees only its private data, never the shell.
            unsafe { drop(Box::from_raw(child)) };
        }
        drop(owned);
    }
}

/// SAFETY: as [`release_struct_array`], on the schema side. The winner
/// cascades every child schema release exactly once (the house child schema
/// release frees its aliased shell) and frees the schema-side arena. The
/// parent schema shell is NOT freed here — same convention as the array
/// side: whoever boxes the returned struct frees it after release.
unsafe fn release_struct_schema(schema: *mut ArrowSchema) {
    let slot: &AtomicPtr<c_void> =
        unsafe { &*(core::ptr::addr_of_mut!((*schema).release) as *mut AtomicPtr<c_void>) };
    // Parent slot swapped to NULL BEFORE the cascade (FMEA, module head):
    // a concurrent/reentrant second release observes NULL and returns
    // before touching any child. Target-gated RMW via `release_swap`.
    let prev = release_swap(slot);
    if prev.is_null() {
        // Already released: typed no-op, never a double free.
        return;
    }
    let private = unsafe { (*schema).private_data };
    if !private.is_null() {
        // SAFETY: recreated exactly as boxed in `export_struct`.
        let owned = unsafe { Box::from_raw(private as *mut OwnedStructSchema) };
        for &child in owned.children.iter() {
            // SAFETY: the shell was boxed in `export_struct` and is still
            // alive until this cascade.
            if let Some(release) = unsafe { (*child).release } {
                // SAFETY: the child release is CAS-guarded; exactly one call
                // ever frees the child shell + its private data.
                unsafe { release(child) };
            }
        }
        drop(owned);
    }
}

/// Export a struct column with `children.len()` child columns as an owned
/// C Data Interface pair (schema format `+s`).
///
/// The parent array carries length `len`, no validity (`null_count == 0`,
/// one null validity slot — struct layout has no values buffer, so
/// `n_buffers == 1`), and a heap children-pointer array; the parent schema
/// carries `name` and the child schemas wired as spec children.
///
/// Ownership transfers to the importer per the module ownership table:
/// releasing the parent pair cascades every child release exactly once.
///
/// # Contract on `children`
/// Each element is a live self-owned pair (e.g. from
/// [`crate::c_abi::export`]); each child schema must carry a release
/// callback. Child array lengths are the producer's responsibility and are
/// not re-checked here (zero-copy export: no revalidation pass).
pub fn export_struct(
    children: Vec<(ArrowArray, ArrowSchema)>,
    len: i64,
    name: *const c_char,
) -> (ArrowArray, ArrowSchema) {
    let n = children.len();
    let mut arr_shells: Vec<*mut ArrowArray> = Vec::with_capacity(n);
    let mut schema_shells: Vec<*mut ArrowSchema> = Vec::with_capacity(n);
    for (arr, schema) in children {
        // The by-value child shells become heap-resident; the array shell is
        // freed by the parent array cascade, the schema shell by the child's
        // own (house) schema release.
        arr_shells.push(Box::into_raw(Box::new(arr)));
        let sp = Box::into_raw(Box::new(schema));
        // SAFETY: live box; private_data is re-aliased to this shell so the
        // house release frees exactly this allocation.
        unsafe { (*sp).private_data = sp as *mut c_void };
        schema_shells.push(sp);
    }
    // Struct layout: one validity slot, no values buffer (spec: n_buffers 1).
    let buffers: Box<[*const c_void]> = alloc::vec![core::ptr::null()].into_boxed_slice();
    let owned_arr = Box::into_raw(Box::new(OwnedStructArray {
        children: arr_shells.into_boxed_slice(),
        buffers,
    }));
    let owned_schema = Box::into_raw(Box::new(OwnedStructSchema {
        children: schema_shells.into_boxed_slice(),
    }));
    // SAFETY: both arenas are live boxes; read their interior pointers.
    let (children_ptr, buffers_ptr) = unsafe {
        (
            (*owned_arr).children.as_mut_ptr(),
            (*owned_arr).buffers.as_mut_ptr(),
        )
    };
    // SAFETY: live box.
    let schema_children_ptr = unsafe { (*owned_schema).children.as_mut_ptr() };
    let array = ArrowArray {
        length: len,
        null_count: 0,
        offset: 0,
        n_buffers: 1,
        n_children: n as i64,
        buffers: buffers_ptr,
        children: children_ptr,
        dictionary: core::ptr::null_mut(),
        release: Some(release_struct_array),
        private_data: owned_arr as *mut c_void,
    };
    let schema = ArrowSchema {
        format: c"+s".as_ptr(),
        name,
        metadata: core::ptr::null(),
        flags: CGAP_FLAG_NULLABLE,
        n_children: n as i64,
        children: schema_children_ptr,
        dictionary: core::ptr::null_mut(),
        release: Some(release_struct_schema),
        private_data: owned_schema as *mut c_void,
    };
    (array, schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c_abi::export;
    use crate::primitive::PrimitiveBuilder;
    use core::ffi::CStr;
    use core::sync::atomic::AtomicUsize;

    /// Real i32/i64 children with nulls: i32 [1, NULL, 3, 4, NULL],
    /// i64 [NULL, 2, 3, NULL, 5] — 5 rows each, null_count 2 each.
    fn build_children() -> ((ArrowArray, ArrowSchema), (ArrowArray, ArrowSchema)) {
        let mut b32 = PrimitiveBuilder::<i32>::new();
        b32.push(Some(1));
        b32.push(None);
        b32.push(Some(3));
        b32.push(Some(4));
        b32.push(None);
        let mut b64 = PrimitiveBuilder::<i64>::new();
        b64.push(None);
        b64.push(Some(2));
        b64.push(Some(3));
        b64.push(None);
        b64.push(Some(5));
        let c32 = export(b32.finish(), c"i".as_ptr());
        let c64 = export(b64.finish(), c"l".as_ptr());
        (c32, c64)
    }

    #[test]
    fn struct_export_bitwise_abi_walk() {
        let (c32, c64) = build_children();
        let (array, schema) = export_struct(alloc::vec![c32, c64], 5, c"root".as_ptr());

        // Parent array header: raw field reads.
        assert_eq!(array.length, 5);
        assert_eq!(array.null_count, 0);
        assert_eq!(array.offset, 0);
        assert_eq!(array.n_buffers, 1);
        assert_eq!(array.n_children, 2);
        assert!(array.release.is_some());
        // Struct layout: exactly one (validity) buffer slot and it is null
        // (this export carries no parent validity).
        // SAFETY: one-slot buffers arena from `export_struct`.
        let parent_validity = unsafe { *array.buffers };
        assert!(parent_validity.is_null());

        // Parent schema: +s format, name wired, two schema children.
        assert_eq!(schema.n_children, 2);
        assert_eq!(schema.flags, CGAP_FLAG_NULLABLE);
        assert!(schema.release.is_some());
        assert!(schema.metadata.is_null());
        assert!(schema.dictionary.is_null());
        assert!(array.dictionary.is_null());

        // SAFETY: children arenas are two-entry and shells are alive
        // pre-release; format/name are 'static C strings from this test.
        unsafe {
            assert_eq!(CStr::from_ptr(schema.format).to_bytes(), b"+s");
            assert_eq!(CStr::from_ptr(schema.name).to_bytes(), b"root");
            let child0 = *array.children.add(0);
            let child1 = *array.children.add(1);
            let s0 = *schema.children.add(0);
            let s1 = *schema.children.add(1);

            // --- child 0: i32 [1, NULL, 3, 4, NULL] ---
            assert_eq!((*child0).length, 5);
            assert_eq!((*child0).null_count, 2);
            assert_eq!((*child0).offset, 0);
            assert_eq!((*child0).n_buffers, 2);
            assert_eq!((*child0).n_children, 0);
            let v0 = *(*child0).buffers.add(0) as *const u8; // validity
            let d0 = *(*child0).buffers.add(1) as *const u8; // values
                                                             // LSB-first validity bits: rows 0,2,3 valid -> 0b0000_1101.
            assert_eq!(core::slice::from_raw_parts(v0, 1), &[0x0D]);
            // Bitwise value bytes, little-endian, null slots carry ZERO.
            assert_eq!(
                core::slice::from_raw_parts(d0, 5 * 4),
                &[
                    1, 0, 0, 0, //
                    0, 0, 0, 0, //
                    3, 0, 0, 0, //
                    4, 0, 0, 0, //
                    0, 0, 0, 0,
                ]
            );
            // Typed re-read of the same buffer: bitwise i32 equality.
            let t0 = d0 as *const i32;
            assert_eq!(*t0, 1);
            assert_eq!(*t0.add(1), 0);
            assert_eq!(*t0.add(2), 3);
            assert_eq!(*t0.add(3), 4);
            assert_eq!(*t0.add(4), 0);

            // --- child 1: i64 [NULL, 2, 3, NULL, 5] ---
            assert_eq!((*child1).length, 5);
            assert_eq!((*child1).null_count, 2);
            assert_eq!((*child1).offset, 0);
            assert_eq!((*child1).n_buffers, 2);
            let v1 = *(*child1).buffers.add(0) as *const u8;
            let d1 = *(*child1).buffers.add(1) as *const u8;
            // LSB-first validity bits: rows 1,2,4 valid -> 0b0001_0110.
            assert_eq!(core::slice::from_raw_parts(v1, 1), &[0x16]);
            assert_eq!(
                core::slice::from_raw_parts(d1, 5 * 8),
                &[
                    0, 0, 0, 0, 0, 0, 0, 0, //
                    2, 0, 0, 0, 0, 0, 0, 0, //
                    3, 0, 0, 0, 0, 0, 0, 0, //
                    0, 0, 0, 0, 0, 0, 0, 0, //
                    5, 0, 0, 0, 0, 0, 0, 0,
                ]
            );

            // --- schema children wiring ---
            assert_eq!(CStr::from_ptr((*s0).format).to_bytes(), b"i");
            assert_eq!(CStr::from_ptr((*s1).format).to_bytes(), b"l");
            assert_eq!((*s0).n_children, 0);
            assert_eq!((*s1).n_children, 0);
            assert!((*s0).release.is_some());
            assert!((*s1).release.is_some());

            // Full teardown: each parent released once; child arrays and
            // child schemas are freed by the cascades, parent shells by the
            // box holder (this test).
            let pa = Box::into_raw(Box::new(array));
            ((*pa).release.unwrap())(pa);
            drop(Box::from_raw(pa));
            let ps = Box::into_raw(Box::new(schema));
            ((*ps).release.unwrap())(ps);
            drop(Box::from_raw(ps));
        }
    }

    // --- cascade exactly-once court -------------------------------------
    //
    // Hand-written REAL release implementations (same CAS discipline as the
    // house `release_array` / `release_schema`): each records its own
    // non-null -> null swap into test-owned atomics and frees real heap
    // state. Not mocks: they are real implementations of the release
    // contract, and assertions read final recorded state, not call stacks.
    // Post-cascade per-child slot reads are deliberately absent: the
    // cascade frees the child shells (spec: children die with the parent),
    // so the null-swap on each child is observed by the child's own CAS
    // and recorded here.

    static CASCADE_RELEASES: AtomicUsize = AtomicUsize::new(0);
    static CASCADE_NULL_SWAPS: AtomicUsize = AtomicUsize::new(0);
    static SCHEMA_NULL_SWAPS: AtomicUsize = AtomicUsize::new(0);

    unsafe fn counting_child_release(array: *mut ArrowArray) {
        let slot: &AtomicPtr<c_void> =
            unsafe { &*(core::ptr::addr_of_mut!((*array).release) as *mut AtomicPtr<c_void>) };
        let prev = slot.swap(core::ptr::null_mut(), Ordering::AcqRel);
        if prev.is_null() {
            return;
        }
        CASCADE_NULL_SWAPS.fetch_add(1, Ordering::AcqRel);
        CASCADE_RELEASES.fetch_add(1, Ordering::AcqRel);
        let private = unsafe { (*array).private_data };
        if !private.is_null() {
            // SAFETY: real heap payload boxed by the test before export_struct.
            unsafe { drop(Box::from_raw(private as *mut u64)) };
        }
    }

    unsafe fn counting_child_schema_release(schema: *mut ArrowSchema) {
        let slot: &AtomicPtr<c_void> =
            unsafe { &*(core::ptr::addr_of_mut!((*schema).release) as *mut AtomicPtr<c_void>) };
        let prev = slot.swap(core::ptr::null_mut(), Ordering::AcqRel);
        if prev.is_null() {
            return;
        }
        SCHEMA_NULL_SWAPS.fetch_add(1, Ordering::AcqRel);
        // SAFETY: house convention — `export_struct` aliased private_data to
        // the boxed shell, so the first release frees exactly once.
        unsafe { drop(Box::from_raw(schema)) };
    }

    fn counting_child(len: i64) -> ArrowArray {
        ArrowArray {
            length: len,
            null_count: 0,
            offset: 0,
            n_buffers: 0,
            n_children: 0,
            buffers: core::ptr::null_mut(),
            children: core::ptr::null_mut(),
            dictionary: core::ptr::null_mut(),
            release: Some(counting_child_release),
            private_data: Box::into_raw(Box::new(0x5Au64)) as *mut c_void,
        }
    }

    fn counting_child_schema() -> ArrowSchema {
        ArrowSchema {
            format: c"i".as_ptr(),
            name: core::ptr::null(),
            metadata: core::ptr::null(),
            flags: CGAP_FLAG_NULLABLE,
            n_children: 0,
            children: core::ptr::null_mut(),
            dictionary: core::ptr::null_mut(),
            release: Some(counting_child_schema_release),
            private_data: core::ptr::null_mut(),
        }
    }

    #[test]
    fn parent_double_release_cascades_children_exactly_once() {
        CASCADE_RELEASES.store(0, Ordering::Relaxed);
        CASCADE_NULL_SWAPS.store(0, Ordering::Relaxed);
        SCHEMA_NULL_SWAPS.store(0, Ordering::Relaxed);

        let children = alloc::vec![
            (counting_child(5), counting_child_schema()),
            (counting_child(5), counting_child_schema()),
        ];
        let (array, schema) = export_struct(children, 5, c"root".as_ptr());

        // Pre-cascade: the ABI children resolve to live release callbacks.
        // SAFETY: two-entry arena, shells alive.
        let (c0, c1) = unsafe { (*array.children.add(0), *array.children.add(1)) };
        unsafe {
            assert!((*c0).release.is_some());
            assert!((*c1).release.is_some());
        }

        // Test-owned parent box; released twice to prove the cascade runs
        // exactly once and the second call is a typed no-op.
        let pa = Box::into_raw(Box::new(array));
        unsafe {
            let f = (*pa).release.unwrap();
            f(pa); // cascade: every child null-swapped + freed exactly once
            assert!((*pa).release.is_none()); // parent slot swapped to null
            f(pa); // double release: no re-cascade
            assert!((*pa).release.is_none());
        }
        assert_eq!(CASCADE_RELEASES.load(Ordering::Acquire), 2);
        assert_eq!(CASCADE_NULL_SWAPS.load(Ordering::Acquire), 2);
        // SAFETY: parent shell owned by the box holder (this test).
        unsafe { drop(Box::from_raw(pa)) };

        // Schema side: same double-release discipline.
        // Test-owned parent schema box.
        let ps = Box::into_raw(Box::new(schema));
        unsafe {
            let g = (*ps).release.unwrap();
            g(ps); // cascades child schema releases (each frees its shell)
            assert!((*ps).release.is_none());
            g(ps); // double: typed no-op
            assert!((*ps).release.is_none());
        }
        assert_eq!(SCHEMA_NULL_SWAPS.load(Ordering::Acquire), 2);
        // SAFETY: parent shell owned by the box holder.
        unsafe { drop(Box::from_raw(ps)) };
    }

    #[test]
    fn all_real_children_release_through_cascade() {
        let (c32, c64) = build_children();
        let (array, schema) = export_struct(alloc::vec![c32, c64], 5, c"struct".as_ptr());
        // SAFETY: test-owned boxes; the real generic child releases run
        // through both cascades — a double free would abort the process.
        unsafe {
            let pa = Box::into_raw(Box::new(array));
            let f = (*pa).release.unwrap();
            f(pa);
            f(pa);
            assert!((*pa).release.is_none());
            drop(Box::from_raw(pa));
            let ps = Box::into_raw(Box::new(schema));
            let g = (*ps).release.unwrap();
            g(ps);
            g(ps);
            assert!((*ps).release.is_none());
            drop(Box::from_raw(ps));
        }
    }

    #[test]
    fn empty_struct_exports_and_releases() {
        let (array, schema) = export_struct(Vec::new(), 0, c"empty".as_ptr());
        assert_eq!(array.length, 0);
        assert_eq!(array.n_children, 0);
        assert_eq!(schema.n_children, 0);
        assert_eq!(array.n_buffers, 1);
        // SAFETY: test-owned boxes; the empty cascade walks an empty arena.
        unsafe {
            let pa = Box::into_raw(Box::new(array));
            ((*pa).release.unwrap())(pa);
            drop(Box::from_raw(pa));
            let ps = Box::into_raw(Box::new(schema));
            ((*ps).release.unwrap())(ps);
            drop(Box::from_raw(ps));
        }
    }
}
