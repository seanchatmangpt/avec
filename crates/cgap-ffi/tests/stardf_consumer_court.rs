//! cgap consumer court — the W3 cgap-ffi gate, landed by lane 6, 2026-10-31.
//! Consumes the FINAL stardf-arrow-capi exactly as a foreign consumer: every
//! stream interaction goes through the `ArrowArrayStream` struct's own
//! `extern "C"` function pointers (no Rust shims), release through the C
//! release callbacks, allocation-identity through stardf's ownership-tag
//! surface (stardf_i32_batch/stardf_array_addr/stardf_batch_rows/
//! stardf_pending_tags/stardf_release), with the process allocator being
//! cgap-audit's `CountingAlloc` (real allocator substrate from cgap, linked
//! as a dev dependency of this court).
//!
//! Source of truth: the lane-14 rehearsal
//! (ggen-legacy/stardf/arrow/docs/cgap-rehearsal.md). Courts 1-2 are verbatim
//! from the rehearsal court; court 3's `dlopen` path is re-pointed from the
//! rehearsal scratch crate's own target dir to cgap's workspace target dir,
//! where the path-dep capi's cdylib artifact is built.

use core::ffi::{c_char, c_int, c_void};
use std::sync::Mutex;

use stardf_arrow_capi::{
    stardf_array_addr, stardf_batch_rows, stardf_i32_batch, stardf_pending_tags, stardf_release,
    ArrowArray, ArrowArrayStream, ReleaseOutcome,
};
use stardf_arrow_capi::{
    stardf_arena_free, stardf_arena_intern, stardf_arena_new, stardf_batch_append_quad,
    stardf_cgap_export_stream, stardf_stream_free, StardfBatch,
};

/// cgap-audit's counting allocator is the process allocator for this court
/// binary (the only #[global_allocator]; proves the whole consumer path runs
/// on the cgap substrate's allocator harness, and gives the scoped report
/// used by court 2's export-allocation witness).
use cgap_audit::alloc_counter::{with_alloc_tracking, AllocReport};

// --- dlopen witness: the final cdylib, loaded as a real foreign consumer ---
// Re-pointed from the rehearsal scratch crate: the capi is a path dev-dep, so
// its cdylib artifact lands in the cgap workspace target dir.
const DYLIB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/debug/libstardf_arrow_capi.dylib\0"
);
unsafe extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}
const RTLD_NOW: c_int = 0x2;
const RTLD_LOCAL: c_int = 0x4;

unsafe fn dlsym_fn<'a, T>(handle: *mut c_void, name: &str) -> T {
    let sym = unsafe { dlsym(handle, name.as_ptr() as *const c_char) };
    assert!(
        !sym.is_null(),
        "dlsym missed required export {name} in the final dylib"
    );
    unsafe { core::mem::transmute_copy::<*mut c_void, T>(&sym) }
}

/// Full consumer lifecycle on the rlib path (the cgap-ffi court proper):
/// arena -> intern -> quads -> stardf_cgap_export_stream -> get_schema /
/// get_next (through the C struct pointers) -> release callbacks ->
/// stardf_stream_free -> every ownership tag claimed.
///
/// # Safety
/// Owned handles throughout; function pointers invoked as armed.
unsafe fn consume_stream_lifecycle(quads: usize) -> (usize, AllocReport) {
    let arena = unsafe { stardf_arena_new(1 << 16) };
    assert!(!arena.is_null());
    let mut ids = [0u32; 4];
    for (i, id) in ids.iter_mut().enumerate() {
        let lex = format!("ex:term{i}");
        *id = unsafe { stardf_arena_intern(arena, lex.as_ptr(), lex.len()) };
        assert_ne!(*id, 0);
    }
    let mut batch: *mut StardfBatch = core::ptr::null_mut();
    for i in 0..quads {
        let s = ids[0] + (i as u32 % 2);
        batch = unsafe { stardf_batch_append_quad(arena, s, ids[1], ids[2], ids[3]) };
        assert!(!batch.is_null(), "quad {i} append refused");
    }

    let mut stream: *mut ArrowArrayStream = core::ptr::null_mut();
    assert_eq!(
        unsafe { stardf_cgap_export_stream(batch, &mut stream) },
        0,
        "export_stream must return STATUS_OK"
    );
    assert!(!stream.is_null());

    // get_schema through the C struct pointer (real consumer path).
    let get_schema = (*stream).get_schema.expect("get_schema armed");
    // The capi's own re-export (its pinned cgap-arrow identity) — the exact
    // type the stream's function pointers expect. In the rehearsal this was
    // `cgap_arrow::c_abi::ArrowSchema` because the scratch crate resolved a
    // SINGLE cgap-arrow identity (same git rev as stardf's pin); in cgap-ffi
    // the path-HEAD cgap-arrow dev-dep is a second crate identity, so the
    // struct must come from the capi to match the pointer signatures.
    let mut schema: stardf_arrow_capi::ArrowSchema = core::mem::zeroed();
    assert_eq!(unsafe { get_schema(stream, &mut schema) }, 0, "schema flag");
    assert!(schema.release.is_some(), "handed-out schema armed");
    assert!(!schema.format.is_null());
    let format = unsafe { core::ffi::CStr::from_ptr(schema.format) };
    assert!(!format.to_bytes().is_empty(), "schema format non-empty");
    if let Some(rel) = schema.release {
        unsafe { rel(&mut schema) };
    }

    // get_next through the C struct pointer: ceil(quads/64) batches then EOS.
    let get_next = (*stream).get_next.expect("get_next armed");
    let expected = quads.div_ceil(64);
    let mut got = 0usize;
    loop {
        let mut arr: ArrowArray = core::mem::zeroed();
        assert_eq!(unsafe { get_next(stream, &mut arr) }, 0);
        if arr.release.is_none() {
            assert_eq!(arr.length, 0, "EOS array zeroed");
            break;
        }
        got += 1;
        assert!(arr.length > 0, "yielded batch non-empty");
        assert!(got <= expected, "over-yield (quads={quads})");
        unsafe { (arr.release.expect("armed"))(&mut arr) };
    }
    assert_eq!(got, expected, "exactly ceil(quads/64) batches");

    // Release chain: stream release callback (once), then stardf_stream_free
    // (whose internal re-fire is the typed no-op).
    let release = (*stream).release.expect("stream release armed");
    unsafe { release(stream) };
    unsafe { stardf_stream_free(stream) };
    unsafe { stardf_arena_free(arena) };
    (got, AllocReport::default())
}

/// Real serialization token for courts 1-2: both read-then-assert on stardf's
/// process-global export slots/tag registry (`stardf_pending_tags` deltas),
/// so under libtest's default multi-thread runner their windows must not
/// interleave. The registry itself serializes nothing — it is shared mutable
/// state, not a lock. Court 3 deliberately does NOT take this lock: its
/// dlopen path mutates the dylib's own image-private registry, independent
/// of the rlib-linked copy in this test binary.
static COURT_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn court_1_export_stream_consume_get_schema_get_next_release() {
    let _serial = COURT_LOCK.lock().expect("court lock poisoned by a failed court");
    let before_tags = stardf_pending_tags();
    // SAFETY: owned lifecycle, fully released.
    let (got, report) = unsafe { consume_stream_lifecycle(130) };
    assert_eq!(got, 3, "130 quads -> 3 fixed-64 batches");
    assert_eq!(stardf_pending_tags(), before_tags, "all tags claimed");
    let _ = report;
}

#[test]
fn court_2_allocation_identity_boxed_array_tags() {
    let _serial = COURT_LOCK.lock().expect("court lock poisoned by a failed court");
    let before = stardf_pending_tags();
    // SAFETY: plain allocation into process-global slots.
    let ptr = unsafe { stardf_i32_batch(64, 7) };
    assert!(!ptr.is_null());
    // Zero-copy identity: the returned pointer IS the live export slot.
    assert_eq!(stardf_array_addr(), ptr as usize, "slot identity");
    assert_eq!(stardf_batch_rows(), 64);
    assert_eq!(stardf_pending_tags(), before + 1, "mint minted one tag");

    // Consume the exported array as a foreign consumer: read length, release
    // through stardf's tagged chain, prove the tag is claimed.
    let (len, outcome) = unsafe {
        let len = (*ptr).length;
        (len, stardf_release(ptr))
    };
    assert_eq!(len, 64);
    assert_eq!(outcome, ReleaseOutcome::Released);
    assert_eq!(stardf_array_addr(), 0, "live slot cleared after release");
    assert_eq!(stardf_pending_tags(), before, "release claimed the tag");

    // cgap-audit witness: a fresh mint inside a tracking scope allocates on
    // the cgap substrate's counting allocator (real alloc calls observed),
    // and the honest release returns the tag count to baseline.
    let (_, report) = with_alloc_tracking(|| unsafe { stardf_i32_batch(16, 0) });
    assert!(
        !report.is_zero(),
        "export must allocate on the cgap-audit counting allocator"
    );
    // SAFETY: live stardf-owned export minted above.
    let out = unsafe { stardf_release(stardf_array_addr() as *mut ArrowArray) };
    assert_eq!(out, ReleaseOutcome::Released);
    assert_eq!(stardf_pending_tags(), before);
}

#[test]
fn court_3_dylib_loads_and_final_api_serves() {
    // The final dylib, loaded as a real foreign consumer (dlopen).
    let handle = unsafe { dlopen(DYLIB.as_ptr() as *const c_char, RTLD_NOW | RTLD_LOCAL) };
    assert!(
        !handle.is_null(),
        "final dylib must load: {DYLIB} (dlerror suppressed)"
    );

    // Version string served from the dylib.
    let version: unsafe extern "C" fn() -> *const c_char =
        unsafe { dlsym_fn(handle, "stardf_arrow_version\0") };
    let v = unsafe { core::ffi::CStr::from_ptr(version()) };
    assert_eq!(v.to_bytes(), b"26.9.29", "final dylib version");

    // The release-tagged allocation-identity surface (grown since the C16
    // rehearsal plan: stardf_i32_batch / stardf_array_addr /
    // stardf_batch_rows / stardf_pending_tags / stardf_release) is live in
    // the dylib, and the ownership-tag delta is observable through it.
    let pending_tags: unsafe extern "C" fn() -> usize =
        unsafe { dlsym_fn(handle, "stardf_pending_tags\0") };
    let i32_batch: unsafe extern "C" fn(i64, i64) -> *mut ArrowArray =
        unsafe { dlsym_fn(handle, "stardf_i32_batch\0") };
    let release: unsafe extern "C" fn(*mut ArrowArray) -> i32 =
        unsafe { dlsym_fn(handle, "stardf_release\0") };
    unsafe {
        let before = pending_tags();
        let p = i32_batch(8, 0);
        assert!(!p.is_null(), "dylib mint works");
        assert_eq!(pending_tags(), before + 1, "dylib tag mint");
        let out = release(p);
        assert_eq!(out, 0, "dylib release claims the tag (Released == 0)");
        assert_eq!(pending_tags(), before);
    }
    // SAFETY: handle no longer needed.
    unsafe { dlclose(handle) };
}
