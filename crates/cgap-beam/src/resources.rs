//! Decoupled BEAM resource envelope (lane BEAM-L1).
//!
//! Hand-transcribed erl_nif resource-type ABI from the OTP 28.3.1 header
//! `/Users/sac/.erlmcp/otp-28.3.1/lib/erlang/usr/include/erl_nif.h`:
//! - `ErlNifResourceType` — opaque `struct enif_resource_type_t` (erl_nif.h:213)
//! - `ErlNifResourceDtor` (erl_nif.h:214)
//! - `ErlNifResourceTypeInit {dtor, stop, down, members, dyncall}`
//!   (erl_nif.h:216-225)
//! - `ErlNifResourceFlags`: `ERL_NIF_RT_CREATE = 1`, `ERL_NIF_RT_TAKEOVER = 2`
//!   (erl_nif.h:185-190)
//! - `ErlNifEntry.sizeof_ErlNifResourceTypeInit` (erl_nif.h:152; the C init
//!   macro sets it to `sizeof(ErlNifResourceTypeInit)` at erl_nif.h:440) —
//!   `NIF_ENTRY` in lib.rs reports `size_of::<ErlNifResourceTypeInit>()`.
//!
//! The type is opened with `enif_open_resource_type_x`
//! (erl_nif_api_funcs.h:181), the init-struct form of
//! `enif_open_resource_type` (erl_nif_api_funcs.h:112). All five bound
//! symbols are exported by the target VM (erts-16.2 `nm -gU beam.smp`):
//! `_enif_open_resource_type_x`, `_enif_alloc_resource`,
//! `_enif_release_resource`, `_enif_make_resource`, `_enif_get_resource`.

use crate::{
    enif_get_long, enif_make_long, lock_registry, make_atom, Batch, ErlNifEnv, ErlNifTerm,
};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Opaque `struct enif_resource_type_t` (erl_nif.h:213).
#[repr(C)]
pub(crate) struct ErlNifResourceType {
    _data: [u8; 0],
}

/// `ErlNifResourceTypeInit` (erl_nif.h:216-225), moved here from lib.rs so the
/// whole resource-type surface lives beside its dtor. `NIF_ENTRY` reports
/// `size_of` this struct as `sizeof_ErlNifResourceTypeInit`.
///
/// The `members` field is read only by `enif_init_resource_type`
/// (`ldr w5, [x2, #0x18]` in erts-16.2 beam.smp); `enif_open_resource_type_x`
/// ignores it and hardcodes 3 (dtor/stop/down) — disassembly witness:
/// `_enif_open_resource_type_x: mov w5, #0x3; b _open_resource_type`. Set to
/// the true field count (5) so the struct states exactly what it initializes.
#[repr(C)]
pub(crate) struct ErlNifResourceTypeInit {
    dtor: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void)>,
    stop: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, c_int, c_int)>,
    down: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, *mut ErlNifPid, *mut ErlNifMonitor)>,
    members: c_int,
    dyncall: Option<extern "C" fn(*mut ErlNifEnv, *mut c_void, *mut c_void)>,
}

/// `ErlNifPid` (erl_nif.h:197-199): internal PID term; opaque here.
#[repr(C)]
struct ErlNifPid {
    pid: ErlNifTerm,
}

/// `typedef ErlDrvMonitor ErlNifMonitor;` — opaque (erl_nif.h:202).
#[repr(C)]
struct ErlNifMonitor {
    _opaque: [u8; 0],
}

/// `ERL_NIF_RT_CREATE` / `ERL_NIF_RT_TAKEOVER` (erl_nif.h:185-190). Declared
/// as `c_int` constants rather than a Rust enum mirror because C call sites OR
/// them (`CREATE | TAKEOVER` = 3); the 4-byte enum ABI on this target (clang,
/// LP64) is value-preserving for these flags — same transcription class as
/// `ErlNifEntry.options: u32` in lib.rs.
const ERL_NIF_RT_CREATE: c_int = 1;
const ERL_NIF_RT_TAKEOVER: c_int = 2;

// ---------------------------------------------------------------------------
// enif_* resource API: undefined at link time, resolved by the BEAM at
// load_nif time (same dlopen model as lib.rs's extern block).
// ---------------------------------------------------------------------------

extern "C" {
    /// `ErlNifResourceType* enif_open_resource_type_x(ErlNifEnv* env,
    ///   const char* name_str, const ErlNifResourceTypeInit* init,
    ///   ErlNifResourceFlags flags, ErlNifResourceFlags* tried);`
    /// (erl_nif_api_funcs.h:181) — init-struct form of
    /// `enif_open_resource_type` (:112). `ErlNifResourceFlags` is a C enum:
    /// passed/received as the 4-byte int-sized enum this ABI gives it.
    fn enif_open_resource_type_x(
        env: *mut ErlNifEnv,
        name_str: *const c_char,
        init: *const ErlNifResourceTypeInit,
        flags: c_int,
        tried: *mut c_int,
    ) -> *mut ErlNifResourceType;

    /// `void* enif_alloc_resource(ErlNifResourceType* type, size_t size);`
    /// (erl_nif_api_funcs.h:113) — VM-owned allocation, one reference held.
    fn enif_alloc_resource(ty: *mut ErlNifResourceType, size: usize) -> *mut c_void;

    /// `void enif_release_resource(void* obj);` (erl_nif_api_funcs.h:114)
    fn enif_release_resource(obj: *mut c_void);

    /// `ERL_NIF_TERM enif_make_resource(ErlNifEnv* env, void* obj);`
    /// (erl_nif_api_funcs.h:115) — adds one reference owned by the term.
    fn enif_make_resource(env: *mut ErlNifEnv, obj: *mut c_void) -> ErlNifTerm;

    /// `int enif_get_resource(ErlNifEnv* env, ERL_NIF_TERM term,
    ///   ErlNifResourceType* type, void** objp);` (erl_nif_api_funcs.h:116)
    fn enif_get_resource(
        env: *mut ErlNifEnv,
        term: ErlNifTerm,
        ty: *mut ErlNifResourceType,
        objp: *mut *mut c_void,
    ) -> c_int;
}

// ---------------------------------------------------------------------------
// BatchEnvelope: the decoupled resource envelope
// ---------------------------------------------------------------------------

/// The decoupled resource envelope: VM-owned memory (`enif_alloc_resource`)
/// holding only cached metadata — value-buffer pointer, row count, element bit
/// width — plus the owned `Batch` that keeps the aligned payload alive.
///
/// Readers get `ptr`/`len`/`bit_width` without matching on the `Batch`
/// variant; the dtor drops the owned `Batch` (freeing the aligned `Buf`) when
/// the BEAM garbage-collects the last term referencing the envelope.
///
/// `ptr`/`bit_width` carry `dead_code` allows at wave BEAM-L1: they are the
/// contract's metadata surface for the next lanes (first readers land with
/// them), while `len` already has its first reader (`cgap_envelope_len`).
#[allow(dead_code)]
pub(crate) struct BatchEnvelope {
    /// Value-buffer base pointer, cached at construction from
    /// `Primitive::values_ptr()`. Points into the payload owned by `batch`
    /// and stays valid for the envelope's lifetime. Dangling (but valid for
    /// zero-length reads) when the batch has zero rows.
    pub(crate) ptr: NonNull<u8>,
    /// Row count.
    pub(crate) len: usize,
    /// Element bit width — this codebase's integer type tag (32 | 64).
    pub(crate) bit_width: u32,
    /// The owned payload. Private: ownership ends here; only the dtor touches
    /// this field. Never *read* — the dtor's drop_in_place is the consumer —
    /// hence the targeted dead_code allow.
    #[allow(dead_code)]
    batch: Batch,
}

// SAFETY: BatchEnvelope exclusively owns `batch`, whose aligned `Buf` is the
// sole target of `ptr`; moving the envelope to another thread moves that
// exclusive ownership with it. Rust-side access is bounded to a single NIF
// call via `envelope_of`; cross-scheduler lifetime is carried by the BEAM's
// atomic resource refcount, not by Rust. (NonNull<u8> is !Send by default,
// hence the explicit impl — same precedent as Batch in lib.rs.)
unsafe impl Send for BatchEnvelope {}

// ---------------------------------------------------------------------------
// Open at load
// ---------------------------------------------------------------------------

/// Handle to the opened `BatchEnvelope` resource type. Set exactly once by the
/// load callback, before any NIF on this module can run.
///
/// Relaxed ordering is sound: the BEAM serializes load_nif completion against
/// NIF dispatch (a NIF cannot run before load returns), so the store
/// happens-before every read; the atomic only de-duplicates.
static BATCH_ENVELOPE_TYPE: AtomicPtr<ErlNifResourceType> = AtomicPtr::new(std::ptr::null_mut());

/// `ErlNifResourceDtor` (erl_nif.h:214): runs when the BEAM collects the last
/// term referencing an envelope. Drops the owned `Batch`; the VM frees the
/// envelope's own bytes afterwards.
extern "C" fn batch_envelope_dtor(_env: *mut ErlNifEnv, obj: *mut c_void) {
    // SAFETY: `obj` is an enif_alloc_resource block that alloc_envelope (the
    // only writer) fully initialized; the VM runs the dtor at most once per
    // allocation. drop_in_place releases the Batch but leaves the block's
    // memory to the VM, which frees it after dtors complete.
    unsafe { std::ptr::drop_in_place(obj.cast::<BatchEnvelope>()) };
    // Exactly-once observability: the VM guarantees at-most-once dtors; this
    // counter makes the guarantee BEAM-visible (crate::dtor_stats).
    crate::dtor_stats::note_envelope_dtor();
}

/// Open the `BatchEnvelope` resource type with the init-struct open call.
/// `false` on refusal → the load callback fails the whole `load_nif`
/// (fail-closed boot: no NIF of this module ever runs without the envelope
/// type).
fn open_batch_envelope_type(env: *mut ErlNifEnv) -> bool {
    static INIT: ErlNifResourceTypeInit = ErlNifResourceTypeInit {
        dtor: Some(batch_envelope_dtor),
        stop: None,
        down: None,
        members: 5,
        dyncall: None,
    };
    let mut tried: c_int = 0;
    let ty = unsafe {
        enif_open_resource_type_x(
            env,
            b"BatchEnvelope\0".as_ptr() as *const c_char,
            &INIT,
            ERL_NIF_RT_CREATE | ERL_NIF_RT_TAKEOVER,
            &mut tried,
        )
    };
    if ty.is_null() {
        return false;
    }
    BATCH_ENVELOPE_TYPE.store(ty, Ordering::Relaxed);
    true
}

/// NIF library load callback (`ErlNifEntry.load`): opens the resource type
/// before any NIF can run. Returning nonzero fails the whole `load_nif`.
pub(crate) extern "C" fn nif_load(
    at_load_env: *mut ErlNifEnv,
    _priv_data: *mut *mut c_void,
    _load_info: ErlNifTerm,
) -> c_int {
    // [wave-8 lane-7 unblock] ops_scan's BatchView type must also open at
    // load: this OTP refuses enif_open_resource_type_x outside load/upgrade
    // (falsified on the real BEAM; see ops_scan.rs). Fail-closed boot.
    if crate::ops_scan::open_batch_view_type(at_load_env) && open_batch_envelope_type(at_load_env) {
        0
    } else {
        -1
    }
}

// ---------------------------------------------------------------------------
// pub(crate) helpers for the next lanes
// ---------------------------------------------------------------------------

/// Allocate a VM-owned `BatchEnvelope` holding `batch` (fully initialized —
/// dtor-safe from the moment this returns `Ok`). On failure returns
/// `Err(batch)`: ownership of the payload hands back to the caller, who must
/// restore or drop it — the type system makes forgetting impossible.
pub(crate) fn alloc_envelope(batch: Batch) -> Result<*mut c_void, Batch> {
    let ty = BATCH_ENVELOPE_TYPE.load(Ordering::Relaxed);
    if ty.is_null() {
        return Err(batch);
    }
    let (ptr, len, bit_width) = envelope_metadata_of(&batch);
    let obj = unsafe { enif_alloc_resource(ty, std::mem::size_of::<BatchEnvelope>()) };
    if obj.is_null() {
        return Err(batch);
    }
    // Allocation spy (alloc_spy.rs): this is an instrumented crate alloc site.
    crate::alloc_spy::record_alloc(std::mem::size_of::<BatchEnvelope>());
    // SAFETY: `obj` is a fresh size_of::<BatchEnvelope>() block from the VM's
    // resource allocator, never written before this call.
    unsafe {
        obj.cast::<BatchEnvelope>().write(BatchEnvelope {
            ptr,
            len,
            bit_width,
            batch,
        });
    }
    Ok(obj)
}

/// Cached metadata of a batch: `(ptr, len, bit_width)`. The pointer is
/// `Primitive::values_ptr()` — the aligned payload the envelope decouples
/// its readers from the `Batch` variant with.
pub(crate) fn envelope_metadata_of(batch: &Batch) -> (NonNull<u8>, usize, u32) {
    let raw = match batch {
        Batch::I32(arr) => arr.values_ptr(),
        Batch::I64(arr) => arr.values_ptr(),
    };
    // A zero-row buffer's pointer is valid-for-0 dangling; map a null (if the
    // allocator ever produced one) to a typed dangling NonNull — still valid
    // for zero-length reads.
    let ptr = NonNull::new(raw as *mut u8).unwrap_or(NonNull::dangling());
    (ptr, batch.len(), batch.bit_width())
}

/// Typed borrow of the envelope behind a resource term. `None` for any term
/// that is not a `BatchEnvelope` resource (integers, atoms, foreign refs,
/// foreign resource types).
///
/// SAFETY (returning `&'static mut`): the BEAM pins a resource while any term
/// references it, so the borrow is valid for the enclosing NIF call; the block
/// was fully initialized by [`alloc_envelope`]. The unique reference is
/// unchecked: callers must keep at most one live `envelope_of` borrow per term
/// per NIF call.
pub(crate) fn envelope_of(
    env: *mut ErlNifEnv,
    term: ErlNifTerm,
) -> Option<&'static mut BatchEnvelope> {
    let ty = BATCH_ENVELOPE_TYPE.load(Ordering::Relaxed);
    if ty.is_null() {
        return None;
    }
    let mut obj: *mut c_void = std::ptr::null_mut();
    // SAFETY: ty is the type opened at load (never null past a successful
    // load_nif, guarded above); objp is a valid *mut c_void slot.
    if unsafe { enif_get_resource(env, term, ty, &mut obj) } == 0 || obj.is_null() {
        return None;
    }
    // SAFETY: enif_get_resource validated the term against our resource type;
    // alloc_envelope fully initialized the block; the term pins it alive for
    // the duration of the NIF call.
    Some(unsafe { &mut *(obj.cast::<BatchEnvelope>()) })
}

// ---------------------------------------------------------------------------
// Test-only NIFs (registered alongside the contract surface in NIF_FUNCS)
// ---------------------------------------------------------------------------

/// `cgap_envelope_new(Id) -> Envelope :: reference() | error`
///
/// Consumes the integer handle: the registry slot is taken to `None` (the
/// handle reads as freed afterwards), and the `Batch` moves into a VM-owned
/// `BatchEnvelope`. From here the BEAM owns the payload; its dtor drops the
/// `Batch` when the last referencing term is garbage-collected.
///
/// Non-integer, negative, unknown, or already-consumed ids are the atom
/// `error` (the `cgap_len`/`cgap_free` convention, not `cgap_get`'s badarg).
pub(crate) extern "C" fn cgap_envelope_new(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    // Take the Batch out of the registry BEFORE allocating: on a failed take
    // nothing was allocated (no dtor-on-garbage window); on a failed alloc the
    // Batch is restored to its slot below, never dropped on the error path.
    let batch: Option<Batch> = {
        let mut reg = lock_registry();
        match reg.get_mut(id as usize) {
            Some(slot @ Some(_)) => slot.take(),
            _ => None,
        }
    };
    let Some(batch) = batch else {
        return make_atom(env, "error");
    };
    let obj = match alloc_envelope(batch) {
        Ok(obj) => obj,
        // enif_alloc_resource refused: give the handle its Batch back so the
        // failure is recoverable, not a leak or a drop.
        Err(batch) => {
            let mut reg = lock_registry();
            reg[id as usize] = Some(batch);
            return make_atom(env, "error");
        }
    };
    let term = unsafe { enif_make_resource(env, obj) };
    // Refcount: alloc held one reference, enif_make_resource added the term's.
    // Releasing the creating reference leaves the TERM as sole owner — the
    // dtor must be reachable by GC, otherwise the smoke test's dtor proof
    // would be vacuous.
    unsafe { enif_release_resource(obj) };
    term
}

/// `cgap_envelope_len(Envelope) -> Len :: non_neg_integer() | error`
///
/// Reads only the cached envelope metadata: no `Batch` variant match, no
/// registry access. Non-resource terms are the atom `error`.
pub(crate) extern "C" fn cgap_envelope_len(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    if argc != 1 {
        return make_atom(env, "error");
    }
    match envelope_of(env, args[0]) {
        Some(envelope) => unsafe { enif_make_long(env, envelope.len as i64) },
        None => make_atom(env, "error"),
    }
}
