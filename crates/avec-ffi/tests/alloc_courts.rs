//! FR-07 acceptance test 2 — hot-path allocation courts over the avec-ffi
//! C-ABI surface, using `avec_audit::alloc_counter::with_alloc_tracking`.
//!
//! Spec: `docs/jira/v26.10.31/28-beam-nif-handoff.md` §6 acceptance 2.
//!
//! ## What the courts establish (measured, not assumed)
//!
//! Zero-alloc on the hot path (asserted == 0 inside the tracked window):
//! - `avec_digest_of` over a live exported buffer — the raw-buffer fold
//!   reads through `core::slice::from_raw_parts`, no heap traffic.
//! - the release callback (`release_array::<i32>` / `release_schema`) —
//!   frees are not allocations; the callback itself must not allocate on
//!   the way down.
//! - slot reads (`avec_array_addr`/`avec_schema_addr`/`avec_batch_rows`)
//!   and the `avec_version` C-string read.
//!
//! Legitimately allocating (setup, scoped OUTSIDE the tracked window):
//! - `avec_i32_batch` / the export path — the builder allocates value and
//!   validity buffers and the slots are `Box::leak`ed by design. Export
//!   first, then track.
//!
//! Note: linking `avec-audit` as a dev-dependency installs its
//! `CountingAlloc` `#[global_allocator]` into THIS test binary only; the
//! tracking is per-thread, so the `SLOTS` mutex also keeps every tracked
//! window single-threaded by construction.
//!
//! zero_copy.rs lesson applied: the static export slots are process-global —
//! every test that drives `avec_i32_batch` serializes on `SLOTS`.

use avec_audit::alloc_counter::with_alloc_tracking;
use avec_arrow::c_abi::{export, ArrowArray};
use avec_arrow::primitive::PrimitiveBuilder;

const FMT_I: *const core::ffi::c_char = b"i\0".as_ptr() as *const _;

/// Process-global static export slots: tests driving `avec_i32_batch` race
/// otherwise (witnessed failure class in zero_copy.rs). Also keeps each
/// tracked window free of other threads' allocations (tracking is
/// per-thread).
static SLOTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Build+export `rows` as a heap-boxed ArrowArray (setup — allocates).
fn export_i32(rows: &[Option<i32>]) -> *mut ArrowArray {
    let mut b = PrimitiveBuilder::<i32>::new();
    for v in rows {
        b.push(*v);
    }
    let (array, _schema) = export(b.finish(), FMT_I);
    Box::into_raw(Box::new(array))
}

/// Release a boxed array export (outside any tracked window unless noted).
/// # Safety
/// `array` must be a live, unreleased boxed export we own.
unsafe fn release_boxed_array(array: *mut ArrowArray) {
    // SAFETY: live boxed export we own; release is armed.
    unsafe {
        let release = (*array).release.expect("array release must be armed");
        release(array);
        assert!(
            (*array).release.is_none(),
            "first release must disarm the callback"
        );
        drop(Box::from_raw(array));
    }
}

/// FR-07 acceptance 2: digest over an exported buffer must be allocation-free.
/// "View creation" in the C-ABI membrane is the slot-pointer serve
/// (`avec_array_addr`) — pointer identity means no copy; the digest fold must
/// then read that memory without allocating. Setup (builder + export) is
/// scoped OUTSIDE the tracked window.
#[test]
fn digest_over_exported_buffer_is_zero_alloc() {
    let _g = SLOTS.lock().unwrap();
    // Setup (allocates): export OUTSIDE the tracked window. `avec_i32_batch`
    // builds the buffers and Box::leaks the static slots.
    // SAFETY: crate's own export; slots are process-lifetime by design.
    let ptr = unsafe { avec_ffi::avec_i32_batch(130, 3) };
    assert!(!ptr.is_null());

    let (digest, report) = with_alloc_tracking(|| {
        // View creation: the slot serve is a pointer read, not a mint.
        let served = avec_ffi::avec_array_addr();
        assert_eq!(
            served, ptr as usize,
            "slot must serve the exact exported pointer (zero-copy identity)"
        );
        avec_ffi::avec_digest_of(served)
    });

    assert_ne!(digest, 0, "non-empty null-bearing batch must fold nonzero");
    assert!(
        report.is_zero(),
        "digest over a live exported buffer allocated on the hot path: {report:?}"
    );
}

/// Digest stays allocation-free across the all-valid shape (no validity
/// buffer) and the validity-word path, and is deterministic across repeated
/// tracked calls (no hidden lazy init inside the fold).
#[test]
fn digest_shapes_are_zero_alloc_and_deterministic() {
    let _g = SLOTS.lock().unwrap();
    let all_valid = export_i32(&(1..=64).map(Some).collect::<Vec<_>>());
    let with_nulls = export_i32(
        &(0..193).map(|i| if i % 7 == 3 { None } else { Some(i) }).collect::<Vec<_>>(),
    );

    let (dv, rv) = with_alloc_tracking(|| avec_ffi::avec_digest_of(all_valid as usize));
    let (dn, rn) = with_alloc_tracking(|| avec_ffi::avec_digest_of(with_nulls as usize));
    assert_ne!(dv, 0);
    assert_ne!(dn, 0);
    assert_ne!(dv, dn, "validity state must be load-bearing");
    assert!(rv.is_zero(), "all-valid fold allocated: {rv:?}");
    assert!(rn.is_zero(), "validity-word fold allocated: {rn:?}");

    let (dv_again, r_again) = with_alloc_tracking(|| avec_ffi::avec_digest_of(all_valid as usize));
    assert_eq!(dv_again, dv, "fold must be deterministic across calls");
    assert!(r_again.is_zero(), "repeat fold allocated: {r_again:?}");

    // SAFETY: boxed exports we own.
    unsafe {
        release_boxed_array(all_valid);
        release_boxed_array(with_nulls);
    }
}

/// The C Data Interface release callback must not allocate on the way down.
/// Frees are not allocations (the counter only counts alloc/realloc/alloc_
/// zeroed successes), so the assertion is meaningful: any arena rebuild,
/// logging side channel, or lazy teardown allocation inside the callback
/// would trip it.
#[test]
fn release_callback_is_zero_alloc() {
    let array = export_i32(&(0..65).map(|i| if i % 7 == 6 { None } else { Some(i) }).collect::<Vec<_>>());

    // SAFETY: live boxed export we own; released inside the tracked scope.
    let (_, report) = unsafe {
        with_alloc_tracking(|| {
            let release = (*array).release.expect("array release must be armed");
            release(array);
            assert!(
                (*array).release.is_none(),
                "first release must disarm the callback"
            );
        })
    };
    assert!(
        report.is_zero(),
        "release callback allocated on the teardown path: {report:?}"
    );
    // Shell drop (a free) happens outside the tracked scope; exact-once is
    // asserted in zero_copy.rs::release_path_exact_once.
    // SAFETY: boxed shell we own, already disarmed.
    unsafe { drop(Box::from_raw(array)) };
}

/// Slot reads and the version C-string read are allocation-free. Setup
/// (`avec_i32_batch`, which builds and Box::leaks the slots) happens outside
/// the tracked window.
#[test]
fn slot_and_version_reads_are_zero_alloc() {
    let _g = SLOTS.lock().unwrap();
    // SAFETY: crate's own export; slots are process-lifetime leaked by design.
    let ptr = unsafe { avec_ffi::avec_i32_batch(9, 3) };
    assert!(!ptr.is_null());
    assert_eq!(avec_ffi::avec_batch_rows(), 9);

    let (vals, report) = with_alloc_tracking(|| {
        (
            avec_ffi::avec_array_addr(),
            avec_ffi::avec_schema_addr(),
            avec_ffi::avec_batch_rows(),
            avec_ffi::avec_version(),
        )
    });
    assert_ne!(vals.0, 0, "array slot must be live");
    assert_ne!(vals.1, 0, "schema slot must be live");
    // SAFETY: VERSION_C is a static NUL-terminated array; read-only.
    let mut len = 0usize;
    unsafe {
        while *vals.3.add(len) != 0 {
            len += 1;
        }
    }
    assert_eq!(len, avec_arrow::VERSION.len(), "version C string readable");
    assert!(report.is_zero(), "slot/version reads allocated: {report:?}");
}
