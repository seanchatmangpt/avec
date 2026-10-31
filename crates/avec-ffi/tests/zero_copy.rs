//! FR-07 zero-copy integration tests over the avec-ffi C ABI surface.
//!
//! Spec: `docs/jira/v26.10.31/28-beam-nif-handoff.md` §2-3 (boundary
//! contract, ownership/lifetime) and §6 (acceptance tests 1 and 2, plus the
//! lifecycle half of 3).
//!
//! ## FR-07 GAPS — asserted-MISSING surfaces in avec-ffi (loud, not faked)
//!
//! The crate is a C Data Interface (ArrowArray/ArrowSchema) membrane, not a
//! BEAM resource-binary bridge. Per spec §2, FR-07's primary surface is
//! `enif_make_resource_binary` aliasing; none of that exists here. Tested
//! below is the closest existing surface: the `avec_i32_batch` static-slot
//! export and the raw `ArrowArray` buffer view it serves.
//!
//! MISSING (no test is written for these; the surface does not exist):
//! - FR-07.1 resource-binary alias term (`enif_make_resource_binary` /
//!   `pointer_of/1` NIF) — lives (unbuilt) in `crates/avec-beam`, not here.
//! - FR-07.2 allocation-counter assertion around `scan` — no `scan` op, no
//!   subresource view mint exists in this crate.
//! - FR-07.3 supervisor-kill destructor accounting — the `avec_i32_batch`
//!   export slots are `Box::leak`ed by design (process-lifetime); there is
//!   NO free/release entry point for them. The exact-once release path that
//!   DOES exist is the `c_abi::export` release callback; it is exercised in
//!   `release_path_exact_once` below. An avec-side free for the static
//!   slots would need the AVEC-105 turnstile (spec §2) — not implemented.
//! - FR-07.4 read-only/mutable turnstile demotion — no mutable-view mint.

use avec_arrow::c_abi::{export, ArrowArray};
use avec_arrow::primitive::PrimitiveBuilder;

const FMT_I: *const core::ffi::c_char = b"i\0".as_ptr() as *const _;

/// The static export slots (`avec_array_addr`/`avec_schema_addr`/`ROWS`)
/// are process-global singletons: tests that drive `avec_i32_batch` race
/// each other when run in parallel and observed a previous failure
/// (pointer_identity vs alignment overwriting slots mid-assert). Serialize.
static SLOTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Values-buffer pointer of a live exported i32 array (buffers[1]).
/// # Safety
/// `array` must point at a live exported ArrowArray (read-only deref).
unsafe fn values_ptr(array: *const ArrowArray) -> *mut u8 {
    // SAFETY: live export, n_buffers == 2, values at buffers[1].
    unsafe { *(*array).buffers.add(1) as *mut u8 }
}

/// Pointer identity (spec §6 acceptance 1, C-ABI analog): the ArrowArray the
/// FFI hands back IS the one in the static slot — same address, no
/// intermediate allocation or copy between export and foreign inspection.
#[test]
fn pointer_identity_export_matches_slot() {
    let _g = SLOTS.lock().unwrap();
    // SAFETY: crate's own export; slots are process-lifetime leaked by design.
    let ptr = unsafe { avec_ffi::avec_i32_batch(9, 0) };
    assert!(!ptr.is_null());
    assert_eq!(
        ptr as usize,
        avec_ffi::avec_array_addr(),
        "returned ArrowArray pointer must be the live slot pointer (zero-copy identity)"
    );
    assert_ne!(avec_ffi::avec_schema_addr(), 0, "schema slot must be live");
    assert_eq!(avec_ffi::avec_batch_rows(), 9);
}

/// THE zero-copy proof: values are readable through the raw buffer view at
/// the exact contents built, and an in-place mutation of that memory is
/// visible through the FFI digest fold of the same buffer — same memory, no
/// copy on either side of the view. (Volatile writes defeat any optimizer
/// fantasy that the view and the digest read different copies.)
#[test]
fn mutation_through_view_visible_through_ffi_view() {
    let _g = SLOTS.lock().unwrap();
    let n = 130i64;
    // SAFETY: crate's own export.
    let ptr = unsafe { avec_ffi::avec_i32_batch(n, 0) };
    let addr = avec_ffi::avec_array_addr();

    // SAFETY: live export; values buffer holds n i32 lanes at offset 0.
    let vals = unsafe {
        core::slice::from_raw_parts(values_ptr(ptr) as *const i32, n as usize)
    };
    for (i, v) in vals.iter().enumerate() {
        assert_eq!(*v, i as i32, "view must alias the built memory, not a copy");
    }

    let d1 = avec_ffi::avec_digest_of(addr);

    // Mutate ONE lane in place through the same view. Volatile: the write
    // must actually hit the memory the digest folds.
    // SAFETY: buffers[1] is the live values buffer of our own export.
    unsafe {
        core::ptr::write_volatile(values_ptr(ptr).add(4 * 17) as *mut i32, -12345);
    }
    let d2 = avec_ffi::avec_digest_of(addr);
    assert_ne!(
        d2, d1,
        "in-place mutation through the FFI view must be visible through the digest fold — proves pointer/same-memory zero-copy"
    );

    // SAFETY: as above.
    unsafe {
        core::ptr::write_volatile(values_ptr(ptr).add(4 * 17) as *mut i32, 17);
    }
    assert_eq!(
        avec_ffi::avec_digest_of(addr),
        d1,
        "restored memory must fold back to the original digest"
    );
}

/// Alignment propagation (spec §3 "Alignment propagation"): every avec
/// buffer is 64-byte aligned (charter M-03). The values buffer AND the
/// validity bitmap buffer of the exported view must honor it, including the
/// empty-batch sentinel and odd sizes.
#[test]
fn exported_buffer_pointers_are_64_byte_aligned() {
    let _g = SLOTS.lock().unwrap();
    // SAFETY: crate's own export.
    let ptr = unsafe { avec_ffi::avec_i32_batch(0, 0) };
    // SAFETY: live export (empty); values slot is the 64-aligned sentinel.
    let empty_vals = unsafe { values_ptr(ptr) };
    assert_eq!(
        empty_vals as usize % 64,
        0,
        "empty-batch values sentinel must stay 64-byte aligned"
    );

    for n in [1i64, 7, 63, 64, 65] {
        // SAFETY: crate's own export.
        let ptr = unsafe { avec_ffi::avec_i32_batch(n, if n % 2 == 0 { 0 } else { 3 }) };
        // SAFETY: live export, n_buffers == 2.
        unsafe {
            let vals = values_ptr(ptr) as usize;
            assert_eq!(vals % 64, 0, "values buffer must be 64-byte aligned (n={n})");
            let validity = *(*ptr).buffers.add(0) as usize;
            if validity != 0 {
                assert_eq!(
                    validity % 64,
                    0,
                    "validity bitmap buffer must be 64-byte aligned (n={n})"
                );
            }
        }
    }
}

/// Lifecycle (spec §6 acceptance 3, free-path half): the only lawful free in
/// the C-ABI regime is the C Data Interface `release` callback. Assert the
/// exact-once law: first call succeeds and disarms, a second call is a typed
/// no-op — never a double free. Covers array AND schema shells, plus the
/// importer-side `ForeignArray` drop path.
#[test]
fn release_path_exact_once() {
    let mut b = PrimitiveBuilder::<i32>::new();
    for i in 0..65 {
        b.push(if i % 7 == 6 { None } else { Some(i) });
    }
    let (array, schema) = export(b.finish(), FMT_I);
    let array = Box::into_raw(Box::new(array));
    let schema = Box::into_raw(Box::new(schema));

    // SAFETY: boxed export pair we own; release is Some(release_array::<i32>).
    unsafe {
        let release = (*array).release.expect("array release must be armed");
        release(array);
        assert!(
            (*array).release.is_none(),
            "first release must disarm the callback"
        );
        // Second release: typed no-op per the swap-to-NULL law.
        if let Some(release) = (*array).release {
            release(array);
            let _ = release;
        }
        drop(Box::from_raw(array));
    }

    // SAFETY: boxed export pair we own. NOTE the asymmetry with the array
    // side: `release_schema` frees the schema shell ITSELF (private_data
    // aliases the boxed struct), so the caller must NOT also free it — the
    // single release call is the whole free. Any read after this call would
    // be use-after-free, so the shell is simply not touched again.
    unsafe {
        let release = (*schema).release.expect("schema release must be armed");
        release(schema);
    }

    // Importer side: dropping the wrapper calls the foreign release exactly once.
    // SAFETY: live, unreleased export we take ownership of as last reference.
    let mut b2 = PrimitiveBuilder::<i32>::new();
    b2.push(Some(1));
    let (arr2, _schema2) = export(b2.finish(), FMT_I);
    let arr2 = Box::into_raw(Box::new(arr2));
    // SAFETY: live export; ForeignArray becomes the last reference.
    let foreign = unsafe { avec_arrow::c_abi::ForeignArray::take(arr2) };
    assert_eq!(foreign.len(), 1);
    drop(foreign); // release ran once inside Drop; a second free would abort.
}

/// Version surface sanity (membrane handshake, spec §2 context): the C
/// string is NUL-terminated and readable through the raw pointer.
#[test]
fn version_cstr_readable_through_ffi() {
    let p = avec_ffi::avec_version();
    // SAFETY: VERSION_C is a static NUL-terminated array.
    let mut len = 0usize;
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
    }
    assert_eq!(
        len,
        avec_arrow::VERSION.len(),
        "C string length must match VERSION"
    );
}
