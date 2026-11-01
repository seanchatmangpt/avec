//! Open/attach + zero-copy scan (28-beam-nif-handoff.md API surface, ops 1-2
//! of 5; wave-8 lane 7).
//!
//! `cgap_open/1` attaches a read-view to an existing batch: it reuses the
//! envelope/registry metadata (`resources.rs`) and aliases the SAME aligned
//! payload buffer — zero bytes copied, zero hot-path payload movement. The
//! returned handle is a `BatchView` resource: cached `(ptr, len, bit_width)`
//! plus a kept reference to the PARENT resource that owns the payload, so the
//! view's lifetime provably never outlives the memory it reads.
//!
//! `cgap_scan/3` returns a zero-copy subresource: a resource-binary slice view
//! over the parent payload's byte range `[start, start+len)`, pinned to the
//! view resource itself. No new buffer (`enif_make_resource_binary`, resbin.rs
//! extern-verification receipt applies verbatim); the counted-site allocation
//! delta of a scan is exactly {0, 0} (alloc_spy.rs ledger).
//!
//! TWO REGIMES, ONE LEDGER LAW:
//! - **id path (consuming)**: soundness, not taste — the registry is Rust-side
//!   memory a BEAM term cannot pin (resbin.rs's law, restated). The `Batch`
//!   moves into a fresh VM-owned `BatchEnvelope` (the exact
//!   `cgap_envelope_new` ledger: take-then-allocate, restore on alloc
//!   failure), the view keeps that envelope alive via `enif_keep_resource`,
//!   and the NIF-local allocation reference is released. Afterwards the id
//!   reads as freed: a SECOND `cgap_open` of the same id, or any read through
//!   it, is the atom `error` — open-after-consume is refused, never a silent
//!   alias of dead registry memory.
//! - **envelope path (non-consuming)**: `envelope_of` borrows the caller's
//!   envelope, the view keeps IT alive, and the envelope stays fully usable.
//!   Opening the same envelope twice is LEGAL — the payload is immutable and
//!   both views alias the same bytes (the cgap_batch_binary double-projection
//!   law, carried one layer up).
//!
//! Pointer semantics (the identity court in the contract test): the parent
//! envelope's payload base is exposed by `pointer_of/1` (pointer.rs), and the
//! data pointer of a scan slice — read back via `cgap_binary_data_pointer/1`
//! — is EXACTLY `base + start`. Equal integers, no copy, offset arithmetic
//! witnessed on the wire.
//!
//! Resource-type opening: `BatchView` is opened by the load callback
//! (`resources.rs::nif_load`, minimal disclosed lane-7 unblock). The
//! lazy-first-call open plan was FALSIFIED on the real BEAM: this OTP returns
//! null from `enif_open_resource_type_x` outside load/upgrade (witnessed
//! `open->error` with no alloc-site delta). Opening at load is the same
//! fail-closed boot discipline as the envelope type.
//!
//! Fail-closed split (the `cgap_get` convention):
//! - wrong arity, non-integer `start`/`len` -> `badarg` (typed VM exception);
//! - negative/unknown/consumed id, non-view handle, out-of-range byte window,
//!   `start + len` overflow, resource-type open failure -> the atom `error`.
//!
//! Scope note (lane 7 of the five-op surface): `flush_to_parquet`,
//! `seal_to_packfile`, and `receipt_query` are NOT here — ops 3-5 belong to
//! lane 8. Only `cgap_open`/`cgap_scan` are registered from this module.

// `clashing_extern_declarations`: enif_open_resource_type_x is re-transcribed
// here with this module's own (identical-layout) ErlNifResourceTypeInit — the
// resources.rs binding is private to that module (house precedent: resbin.rs
// redeclares enif_release_resource, pointer.rs declares enif_inspect_binary).
#![allow(clashing_extern_declarations)]

use crate::alloc_spy;
use crate::resources::{alloc_envelope, envelope_of, BatchEnvelope};
use crate::{enif_get_long, lock_registry, make_atom, ErlNifEnv, ErlNifTerm};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};

// ---------------------------------------------------------------------------
// erl_nif ABI: re-transcribed where the owning module's bindings are private
// (house precedent: resbin.rs redeclares enif_release_resource, pointer.rs
// declares enif_inspect_binary). Same dlopen model as lib.rs's extern block.
// ---------------------------------------------------------------------------

/// Opaque `struct enif_resource_type_t` (erl_nif.h:213).
#[repr(C)]
struct ErlNifResourceType {
    _data: [u8; 0],
}

/// `ErlNifResourceTypeInit` (erl_nif.h:216-225) — byte-identical transcription
/// of resources.rs's (which is private to that module). `members` is ignored by
/// `enif_open_resource_type_x` (it hardcodes 3; disassembly witness in
/// resources.rs's header) and is set to the true field count, same as there.
#[repr(C)]
struct ErlNifResourceTypeInit {
    dtor: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void)>,
    stop: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, c_int, c_int)>,
    down: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, *mut c_void, *mut c_void)>,
    members: c_int,
    dyncall: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, *mut c_void)>,
}

/// `ERL_NIF_RT_CREATE` (erl_nif.h:185-190). No TAKEOVER: this type is created
/// once per VM lifetime and never upgraded.
const ERL_NIF_RT_CREATE: c_int = 1;

extern "C" {
    /// `ErlNifResourceType* enif_open_resource_type_x(ErlNifEnv*,
    ///   const char*, const ErlNifResourceTypeInit*, ErlNifResourceFlags,
    ///   ErlNifResourceFlags*);` (erl_nif_api_funcs.h:181)
    fn enif_open_resource_type_x(
        env: *mut ErlNifEnv,
        name_str: *const c_char,
        init: *const ErlNifResourceTypeInit,
        flags: c_int,
        tried: *mut c_int,
    ) -> *mut ErlNifResourceType;

    /// `void* enif_alloc_resource(ErlNifResourceType*, size_t);` (:113)
    fn enif_alloc_resource(ty: *mut ErlNifResourceType, size: usize) -> *mut c_void;

    /// `void enif_release_resource(void*);` (:114)
    fn enif_release_resource(obj: *mut c_void);

    /// `ERL_NIF_TERM enif_make_resource(ErlNifEnv*, void*);` (:115)
    fn enif_make_resource(env: *mut ErlNifEnv, obj: *mut c_void) -> ErlNifTerm;

    /// `int enif_get_resource(ErlNifEnv*, ERL_NIF_TERM,
    ///   ErlNifResourceType*, void**);` (:116)
    fn enif_get_resource(
        env: *mut ErlNifEnv,
        term: ErlNifTerm,
        ty: *mut ErlNifResourceType,
        objp: *mut *mut c_void,
    ) -> c_int;

    /// `int enif_keep_resource(void* obj);` (erl_nif_api_funcs.h:118, OTP 25+)
    /// — adds a reference to a resource from Rust, so the view can pin its
    /// parent past the lifetime of any term that mentions it.
    fn enif_keep_resource(obj: *mut c_void) -> c_int;

    /// `ERL_NIF_TERM enif_make_resource_binary(ErlNifEnv*, void* obj,
    ///   const void* data, size_t size);` (erl_nif_api_funcs.h:134) — the
    /// zero-copy projection primitive (full extern-verification receipt in
    /// resbin.rs's header: present in this OTP; the `enif_inspect_resource_
    /// binary` name does not exist and is not bound here).
    fn enif_make_resource_binary(
        env: *mut ErlNifEnv,
        obj: *mut c_void,
        data: *const c_void,
        size: usize,
    ) -> ErlNifTerm;
}

// ---------------------------------------------------------------------------
// BatchView: the read-view resource
// ---------------------------------------------------------------------------

/// The read-view resource: cached parent metadata (the envelope's
/// `(ptr, len, bit_width)`, copied by value at open — metadata is three
/// small Copy fields, not payload) plus the KEPT parent resource pointer.
///
/// Ownership law: `parent` carries one reference acquired by
/// `enif_keep_resource`; the dtor releases it. While any `BatchView` term is
/// alive, the parent envelope — and therefore the aligned payload — is pinned,
/// so every scan through the view reads live memory, never reclaimed memory.
#[repr(C)]
struct BatchView {
    /// KEPT parent resource (a `BatchEnvelope` block). Never dereferenced
    /// through this field — the metadata below is the read surface; the
    /// pointer exists to be released exactly once by the dtor.
    parent: *mut c_void,
    /// Payload base (same integer `pointer_of` advertises for the parent).
    ptr: NonNull<u8>,
    /// Row count (elements, not bytes).
    len: usize,
    /// Element bit width (32 | 64) — the byte-window bound derives from it.
    bit_width: u32,
}

// SAFETY: the view owns no payload — it holds a kept VM reference to the
// parent resource, whose own atomic refcount is the lifetime authority, plus
// Copy metadata. Moving it between threads moves nothing that Rust must
// track. (NonNull<u8> is !Send by default; explicit impl per crate precedent
// — Batch in lib.rs, BatchEnvelope in resources.rs.)
unsafe impl Send for BatchView {}

/// Dtor: runs when the BEAM collects the last view term. Releases the kept
/// parent reference — the parent envelope's own dtor fires when THIS was the
/// last reference to it.
extern "C" fn batch_view_dtor(_env: *mut ErlNifEnv, obj: *mut c_void) {
    // SAFETY: obj is an enif_alloc_resource block fully initialized by
    // alloc_view; the VM runs the dtor at most once per allocation.
    let view = unsafe { std::ptr::read(obj.cast::<BatchView>()) };
    // SAFETY: parent held exactly one reference acquired by
    // enif_keep_resource at open; this is its unique release.
    unsafe { enif_release_resource(view.parent) };
    // No drop needed beyond the read above: all fields are Copy-like raw
    // data (no Drop types inside BatchView).
}

/// The `BatchView` resource type, opened by the load callback (see
/// open_batch_view_type: this OTP refuses enif_open_resource_type_x outside
/// load/upgrade — the lazy-open plan was falsified on the real BEAM, witness
/// in the lane-7 receipt).
static BATCH_VIEW_TYPE: AtomicPtr<ErlNifResourceType> = AtomicPtr::new(std::ptr::null_mut());

/// Open the `BatchView` resource type. Called from the load callback
/// (`resources.rs::nif_load` — minimal disclosed unblock, lane 7): this OTP
/// refuses `enif_open_resource_type_x` outside load/upgrade, so the
/// original lazy-first-call open plan is falsified. `false` on refusal →
/// the load callback fails the whole `load_nif` (fail-closed boot, same
/// discipline as the envelope type).
pub(crate) fn open_batch_view_type(env: *mut ErlNifEnv) -> bool {
    static INIT: ErlNifResourceTypeInit = ErlNifResourceTypeInit {
        dtor: Some(batch_view_dtor),
        stop: None,
        down: None,
        members: 5,
        dyncall: None,
    };
    let mut tried: c_int = 0;
    let ty = unsafe {
        enif_open_resource_type_x(
            env,
            b"BatchView\0".as_ptr() as *const c_char,
            &INIT,
            ERL_NIF_RT_CREATE,
            &mut tried,
        )
    };
    if ty.is_null() {
        return false;
    }
    BATCH_VIEW_TYPE.store(ty, Ordering::Relaxed);
    true
}

/// The opened type, or `None` (callers fail closed with the atom `error`).
fn view_type() -> Option<*mut ErlNifResourceType> {
    let ty = BATCH_VIEW_TYPE.load(Ordering::Relaxed);
    if ty.is_null() {
        None
    } else {
        Some(ty)
    }
}

/// Allocate a VM-owned `BatchView` over `parent` (kept immediately: past this
/// call the parent is pinned regardless of what the caller does with its own
/// terms). On failure returns `None` with NO reference taken or left over —
/// every step backs out its own effect.
///
/// Ordering: the parent is kept BEFORE the view block is allocated, so a
/// failed allocation releases the keep it took; the counted alloc record
/// lands only after the block is fully written (a counted alloc whose dtor
/// could never run would corrupt the exactly-once spy ledger).
///
/// Allocation spy (alloc_spy.rs): this is an instrumented crate alloc site,
/// same ledger discipline as `alloc_envelope`.
fn alloc_view(
    parent: *mut c_void,
    ptr: NonNull<u8>,
    len: usize,
    bit_width: u32,
) -> Option<*mut c_void> {
    let ty = BATCH_VIEW_TYPE.load(Ordering::Relaxed);
    if ty.is_null() {
        return None;
    }
    // SAFETY: parent is a live resource pointer; keep_resource only adds a
    // reference (returns 0 only on null — parent is non-null by construction).
    if unsafe { enif_keep_resource(parent) } == 0 {
        return None;
    }
    let obj = unsafe { enif_alloc_resource(ty, std::mem::size_of::<BatchView>()) };
    if obj.is_null() {
        // Back out the keep: the parent never knew this view existed.
        // SAFETY: exactly the reference the keep above took.
        unsafe { enif_release_resource(parent) };
        return None;
    }
    // SAFETY: obj is a fresh size_of::<BatchView>() block, never written
    // before this call.
    unsafe {
        obj.cast::<BatchView>().write(BatchView {
            parent,
            ptr,
            len,
            bit_width,
        });
    }
    alloc_spy::record_alloc(std::mem::size_of::<BatchView>());
    Some(obj)
}

/// Projected payload byte size: `len * 4` (i32) or `len * 8` (i64). `None`
/// for any width outside {32, 64} (unreachable by `Batch`'s construction;
/// refused, never guessed) or on `usize` overflow — same law as
/// resbin.rs's `binary_byte_size` (private there, re-stated here).
fn payload_byte_size(len: usize, bit_width: u32) -> Option<usize> {
    match bit_width {
        32 => len.checked_mul(4),
        64 => len.checked_mul(8),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// NIFs
// ---------------------------------------------------------------------------

/// `cgap_open(IdOrEnvelope) -> View :: reference() | error`
///
/// Attach a read-view to an existing batch: zero bytes copied, the handle
/// aliases the same buffer `pointer_of` advertises.
///
/// - **id path (consuming)**: the `Batch` moves from the registry slot into a
///   fresh `BatchEnvelope` (alloc_envelope ledger: take-then-allocate,
///   restore-on-alloc-failure), the view keeps that envelope alive, and the
///   NIF-local reference is released. The id reads as freed afterwards; a
///   second open of it is the atom `error` (open-after-consume refused).
/// - **envelope path (non-consuming)**: the caller's envelope is borrowed and
///   kept; double-open of the same envelope is legal (immutable payload,
///   both views alias the same bytes).
///
/// Errors: the atom `error` for wrong arity, non-id non-envelope terms,
/// negative/unknown/consumed ids, resource-type open failure, or view
/// allocation failure (id path: the fresh envelope is released — payload
/// reclaimed by its dtor; the failure is fail-closed, never a leak).
pub(crate) extern "C" fn cgap_open(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    if argc != 1 {
        return make_atom(env, "error");
    }
    if view_type().is_none() {
        return make_atom(env, "error");
    }

    // --- id path: consume into an envelope, keep it, project the view ------
    let mut id = 0i64;
    if unsafe { enif_get_long(env, args[0], &mut id) } != 0 {
        if id < 0 {
            return make_atom(env, "error");
        }
        // Take-then-allocate (the cgap_envelope_new / resbin discipline): on
        // a failed take nothing was allocated; on a failed envelope alloc the
        // Batch is restored to its slot, recoverable.
        let batch: Option<crate::Batch> = {
            let mut reg = lock_registry();
            match reg.get_mut(id as usize) {
                Some(slot @ Some(_)) => slot.take(),
                _ => None,
            }
        };
        let Some(batch) = batch else {
            return make_atom(env, "error");
        };
        let parent = match alloc_envelope(batch) {
            Ok(obj) => obj,
            Err(batch) => {
                // Restore (resources.rs's restore discipline, local copy —
                // resbin.rs's restore_batch is private to that module).
                let mut reg = lock_registry();
                if let Some(slot) = reg.get_mut(id as usize) {
                    if slot.is_none() {
                        *slot = Some(batch);
                    }
                }
                return make_atom(env, "error");
            }
        };
        // Read cached metadata off the fresh envelope.
        //
        // SAFETY: alloc_envelope fully initialized the block; no term can
        // name it yet, so the borrow is confined to this NIF call.
        let (ptr, len, bit_width) = unsafe {
            let e = parent.cast::<BatchEnvelope>();
            ((*e).ptr, (*e).len, (*e).bit_width)
        };
        let Some(obj) = alloc_view(parent, ptr, len, bit_width) else {
            // View refused: release the NIF-local envelope reference — its
            // dtor drops the Batch (payload reclaimed). Fail-closed, no leak.
            unsafe { enif_release_resource(parent) };
            return make_atom(env, "error");
        };
        // Ledger: alloc_envelope held one NIF-local reference; the view's
        // keep holds the parent now. Release the NIF-local reference — the
        // view is the payload's lifetime authority from here.
        unsafe { enif_release_resource(parent) };
        let term = unsafe { enif_make_resource(env, obj) };
        // The allocation held one view reference; the term added its own.
        unsafe { enif_release_resource(obj) };
        return term;
    }

    // --- envelope path: borrow the caller's envelope, keep it --------------
    match envelope_of(env, args[0]) {
        Some(envelope) => {
            let obj = alloc_view(
                envelope as *mut BatchEnvelope as *mut c_void,
                envelope.ptr,
                envelope.len,
                envelope.bit_width,
            );
            match obj {
                Some(obj) => {
                    let term = unsafe { enif_make_resource(env, obj) };
                    unsafe { enif_release_resource(obj) };
                    term
                }
                // No cleanup needed: the caller's term still owns its
                // reference; we took none (keep_resource runs only after a
                // successful alloc inside alloc_view).
                None => make_atom(env, "error"),
            }
        }
        None => make_atom(env, "error"),
    }
}

/// `cgap_scan(View, Start, Len) -> Binary :: binary() | error`
///
/// Zero-copy subresource: a binary slice view over the parent payload's byte
/// range `[Start, Start+Len)` — `enif_make_resource_binary` pinned to the
/// view resource, no new buffer, no counted-site allocation (the {0,0} court
/// in the contract test). The binary keeps the view (and through it the
/// parent payload) alive for its whole life.
///
/// Bounds are fail-closed and byte-precise: the window must satisfy
/// `Start >= 0`, `Len >= 0`, `Start + Len <= payload_byte_size` (checked
/// add; overflow refuses), else the atom `error`. Non-integer `Start`/`Len`
/// is `badarg` (the `cgap_get` convention); a term that is not a live view
/// is the atom `error`.
pub(crate) extern "C" fn cgap_scan(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let (mut start, mut wlen) = (0i64, 0i64);
    if argc != 3
        || unsafe { enif_get_long(env, args[1], &mut start) } == 0
        || unsafe { enif_get_long(env, args[2], &mut wlen) } == 0
    {
        return unsafe { crate::enif_make_badarg(env) };
    }
    let ty = BATCH_VIEW_TYPE.load(Ordering::Relaxed);
    if ty.is_null() || start < 0 || wlen < 0 {
        return make_atom(env, "error");
    }
    let mut obj: *mut c_void = std::ptr::null_mut();
    // SAFETY: ty is the type this module opened (non-null past the guard
    // above); objp is a valid slot.
    if unsafe { enif_get_resource(env, args[0], ty, &mut obj) } == 0 || obj.is_null() {
        return make_atom(env, "error");
    }
    // SAFETY: enif_get_resource validated the term against our type and the
    // term pins the block for this call; alloc_view fully initialized it.
    let view = unsafe { &*(obj.cast::<BatchView>()) };
    let Some(total) = payload_byte_size(view.len, view.bit_width) else {
        return make_atom(env, "error");
    };
    let Some(end) = (start as usize).checked_add(wlen as usize) else {
        return make_atom(env, "error");
    };
    if end > total {
        return make_atom(env, "error");
    }
    // SAFETY: obj is the live view the binary term will pin; base + start is
    // within the payload for `end <= total` proven above, and the binary's
    // keep-alive holds the parent payload for the binary's whole life.
    unsafe {
        enif_make_resource_binary(
            env,
            obj,
            view.ptr.as_ptr().add(start as usize).cast::<c_void>(),
            wlen as usize,
        )
    }
}
