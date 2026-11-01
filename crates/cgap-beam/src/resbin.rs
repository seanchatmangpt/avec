//! Zero-copy BEAM visibility (lane BEAM-L2).
//!
//! `cgap_batch_binary/1` projects an cgap batch's aligned value buffer into
//! the BEAM as an immutable binary term with **no copy**:
//! `enif_make_resource_binary` wraps the `BatchEnvelope`'s cached payload
//! pointer directly, so the binary term's bytes ARE the batch's bytes — same
//! buffer, same 64-byte alignment (charter M-03), never relocated.
//!
//! Two entry shapes, one projection:
//! - **registry id** — the handle is consumed (registry slot → `None`) and the
//!   `Batch` moves into a fresh VM-owned `BatchEnvelope`, exactly the
//!   `cgap_envelope_new` ledger; the projection then proceeds as below. The
//!   consumption is forced by soundness, not taste: the registry is Rust-side
//!   memory a binary term cannot pin. A non-consuming projection would let
//!   `cgap_free` free the buffer out from under a live binary term — a
//!   use-after-free. With consumption, the envelope's dtor owns terminal
//!   reclamation and the BEAM's refcounts (now carried by the binary term)
//!   are the only lifetime authority.
//! - **envelope term** — `envelope_of` borrows the existing envelope; no
//!   reference changes hands except the binary term's own keep-alive.
//!
//! Refcount ledger (both paths): the binary term holds the resource alive for
//! its whole life; `enif_release_resource` is called **only** on the id path,
//! dropping the NIF-local reference `alloc_envelope` created — on the envelope
//! path a release would drop a reference owned by the *caller's term* and let
//! the resource die under a live argument.
//!
//! Immutability law: BEAM binaries are immutable by assumption, and a
//! resource binary aliases live memory. Therefore builders must be frozen
//! before projection — enforced structurally, not by convention:
//! - the projected pointer is the `Primitive` value buffer produced by
//!   `PrimitiveBuilder::finish()` — frozen-by-construction;
//! - `BatchEnvelope.batch` is private to `resources.rs` and read only by its
//!   dtor — there is no FFI-visible mutator of the payload anywhere in this
//!   membrane;
//! - this module exposes projection only: it never hands out `&mut [u8]` and
//!   never writes a byte of the payload. Once a binary term exists, no code
//!   path in `cgap-beam` can observe or mutate those bytes again.
//!
//! Extern verification (OTP 28.3.1): `enif_make_resource_binary` is declared
//! at `erl_nif_api_funcs.h:134` as
//! `ERL_NIF_TERM enif_make_resource_binary(ErlNifEnv*, void* obj, const void* data, size_t size)`
//! and exported by the target VM (erts-16.2 `nm -gU beam.smp`:
//! `_enif_make_resource_binary` at 0x1001e673c). The contract's second name,
//! `enif_inspect_resource_binary`, **does not exist** in this OTP: absent from
//! `erl_nif.h` and `erl_nif_api_funcs.h` (the inspect family there is
//! `enif_inspect_binary` :41, `enif_inspect_iolist_as_binary` :98,
//! `enif_inspect_iovec` :200) and absent from beam.smp's export table. A
//! resource binary IS an ordinary binary term, so inspecting one on the Rust
//! side would be `enif_inspect_binary` — no NIF here inspects, so no such
//! binding exists, and binding the phantom name would fail the entire
//! `load_nif` (RTLD_NOW resolves every undefined at dlopen).

use crate::resources::{alloc_envelope, envelope_of, BatchEnvelope};
use crate::{enif_get_long, lock_registry, make_atom, ErlNifEnv, ErlNifTerm};
use std::os::raw::{c_int, c_void};

// ---------------------------------------------------------------------------
// enif_* API: undefined at link time, resolved by the BEAM at load_nif time
// (same dlopen model as lib.rs's and resources.rs's extern blocks)
// ---------------------------------------------------------------------------

extern "C" {
    /// `ERL_NIF_TERM enif_make_resource_binary(ErlNifEnv* env, void* obj,
    ///   const void* data, size_t size);` (erl_nif_api_funcs.h:134)
    ///
    /// Creates a binary term whose bytes are `data[0..size)` — no copy — and
    /// which keeps the resource object `obj` alive until the binary itself is
    /// garbage-collected.
    fn enif_make_resource_binary(
        env: *mut ErlNifEnv,
        obj: *mut c_void,
        data: *const c_void,
        size: usize,
    ) -> ErlNifTerm;

    /// `void enif_release_resource(void* obj);` (erl_nif_api_funcs.h:114)
    ///
    /// Redeclared here: resources.rs's binding is private to that module, and
    /// the id path must drop its own `enif_alloc_resource` reference. Identical
    /// C ABI to the resources.rs declaration; both bind the same VM symbol
    /// (`_enif_release_resource`, verified in beam.smp's export table).
    fn enif_release_resource(obj: *mut c_void);
}

/// Projected byte size: `len * 4` (i32) or `len * 8` (i64) — `bit_width / 8`
/// for this codebase's two integer widths. `None` for any width outside
/// {32, 64} (unreachable by `Batch`'s construction; refused, never guessed)
/// or on `usize` overflow.
fn binary_byte_size(len: usize, bit_width: u32) -> Option<usize> {
    match bit_width {
        32 => len.checked_mul(4),
        64 => len.checked_mul(8),
        _ => None,
    }
}

/// Restore `batch` to registry slot `id` (alloc-failure backout: the failure
/// must stay recoverable, not leak or drop the payload).
fn restore_batch(id: i64, batch: crate::Batch) {
    let mut reg = lock_registry();
    if let Some(slot) = reg.get_mut(id as usize) {
        if slot.is_none() {
            *slot = Some(batch);
        }
    }
}

/// `cgap_batch_binary(IdOrEnvelope) -> Binary :: binary() | error`
///
/// Zero-copy projection of a batch's aligned value buffer as a BEAM binary
/// term pinned to a `BatchEnvelope` resource.
///
/// Argument is either a registry id (`cgap_new`/`cgap_new_i64` handle:
/// integer) or an envelope term (from `cgap_envelope_new/1`: reference).
///
/// - **id path (consuming)**: takes the `Batch` out of the registry, wraps it
///   in a fresh VM-owned envelope, projects the binary, and drops the
///   NIF-local allocation reference — afterwards the handle reads as freed
///   (`cgap_len`/`cgap_free` → `error`) and the binary term is the payload's
///   BEAM-side lifetime authority. Same ledger as `cgap_envelope_new/1`.
/// - **envelope path (non-consuming)**: projects the payload behind an
///   existing envelope term; the envelope stays live (double projection of
///   one envelope is legal — the payload is immutable, both binaries alias
///   the same bytes).
///
/// Errors are the atom `error`, never badarg (the `cgap_envelope_len`
/// convention: this NIF accepts two disjoint term classes, so there is no
/// single "wrong type" shape): wrong arity, non-integer non-envelope terms
/// (atoms, floats, lists, foreign refs), negative/unknown/freed ids, and
/// consumed ids.
pub(crate) extern "C" fn cgap_batch_binary(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    if argc != 1 {
        return make_atom(env, "error");
    }

    // --- id path: consume the handle, wrap, project ------------------------
    let mut id = 0i64;
    if unsafe { enif_get_long(env, args[0], &mut id) } != 0 {
        if id < 0 {
            return make_atom(env, "error");
        }
        // Take the Batch out of the registry BEFORE allocating (no
        // dtor-on-garbage window; on failed alloc the Batch is restored, the
        // cgap_envelope_new discipline).
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
        let obj = match alloc_envelope(batch) {
            Ok(obj) => obj,
            Err(batch) => {
                restore_batch(id, batch);
                return make_atom(env, "error");
            }
        };
        // Read the cached metadata straight off the fresh envelope.
        //
        // SAFETY: alloc_envelope fully initialized this enif_alloc_resource
        // block; no term references it yet (a resource is only nameable
        // through a term, and none exists until projection), so the borrow is
        // confined to this NIF call on this scheduler thread.
        let (ptr, len, bit_width) = unsafe {
            let envelope = obj.cast::<BatchEnvelope>();
            ((*envelope).ptr, (*envelope).len, (*envelope).bit_width)
        };
        let Some(size) = binary_byte_size(len, bit_width) else {
            // Unreachable by Batch's construction; if a future width ever
            // lands here, release the NIF-local reference (dtor fires, payload
            // dropped) and refuse loudly instead of projecting a wrong size.
            unsafe { enif_release_resource(obj) };
            return make_atom(env, "error");
        };
        // SAFETY: obj is the live resource the binary term will pin; ptr is
        // the envelope's cached payload base, valid for `size` bytes for the
        // envelope's lifetime (which the binary term now carries).
        let term =
            unsafe { enif_make_resource_binary(env, obj, ptr.as_ptr().cast::<c_void>(), size) };
        // Refcount ledger: the allocation held one NIF-local reference;
        // enif_make_resource_binary added the binary term's own keep-alive.
        // Releasing the NIF-local reference leaves the binary term (and any
        // other term) as the only owners — dtor owns terminal reclamation.
        unsafe { enif_release_resource(obj) };
        return term;
    }

    // --- envelope path: borrow the existing envelope, project --------------
    match envelope_of(env, args[0]) {
        Some(envelope) => {
            // The reference points at the resource block's base
            // (alloc_envelope wrote the BatchEnvelope there), so this cast
            // recovers exactly the obj pointer enif_get_resource validated.
            let obj = envelope as *mut BatchEnvelope as *mut c_void;
            let Some(size) = binary_byte_size(envelope.len, envelope.bit_width) else {
                // No cleanup needed: the caller's term still owns its
                // reference; we took none.
                return make_atom(env, "error");
            };
            // SAFETY: obj is the live resource behind the caller's term (the
            // term pins it for this call); the new binary term adds its own
            // keep-alive. ptr/size read above are the envelope's cached
            // projection surface.
            let term = unsafe {
                enif_make_resource_binary(env, obj, envelope.ptr.as_ptr().cast::<c_void>(), size)
            };
            // Deliberately NO enif_release_resource here: enif_get_resource
            // added no reference — releasing would drop a reference owned by
            // the caller's argument term.
            term
        }
        None => make_atom(env, "error"),
    }
}
