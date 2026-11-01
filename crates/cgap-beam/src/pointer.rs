//! Pointer identity (FR-07 MISSING #1, wave-4 lane 12).
//!
//! `pointer_of/1` exposes the backing memory address of a batch as a plain
//! integer so the zero-copy POINTER IDENTITY claim becomes testable from the
//! BEAM side: the binary projected by `cgap_batch_binary/1` must carry the
//! SAME data pointer the envelope advertises — same memory, no copy.
//!
//! The proof shape: `pointer_of(Env)` and the data pointer read back out of
//! the projected binary (via `cgap_binary_data_pointer/1`, a thin
//! `enif_inspect_binary` probe) must be the same integer. Binaries are
//! immutable from the BEAM, so "same bytes" alone proves aliasing only
//! weakly; equal data pointers prove it structurally. The digest-flip
//! live-view variant (mutate the Rust-side buffer, watch `cgap_digest_of`
//! flip while the pointer holds) is deliberately NOT implemented here:
//! `cgap_digest_of` lives in `cgap-ffi` and folds raw `ArrowArray` C-Data-
//! Interface exports, not the BEAM registry's `Batch`, and resbin.rs's
//! immutability law ("never hands out `&mut [u8]`") forbids any mutator of
//! a projected payload in this membrane. The pointer-equality court below
//! is the strong form the digest hook was a stand-in for.
//!
//! Fail-closed: any non-resource / non-integer argument, negative, unknown,
//! or consumed id is the atom `error` — same split as `cgap_batch_binary`.

use crate::resources::{envelope_metadata_of, envelope_of};
use crate::{enif_get_long, make_atom, ErlNifBinary, ErlNifEnv, ErlNifTerm};
use std::os::raw::c_int;

extern "C" {
    /// `ERL_NIF_TERM enif_inspect_binary(ErlNifEnv* env, ERL_NIF_TERM bin_term,
    ///   ErlNifBinary* bin);` (erl_nif_api_funcs.h:41) — fills `bin` with the
    /// term's size and data pointer. Refused (returns 0) for any non-binary
    /// term. A resource binary is an ordinary binary term, so the inspect
    /// family reads it like any other (see resbin.rs's header note: the
    /// phantom `enif_inspect_resource_binary` does not exist in this OTP).
    fn enif_inspect_binary(env: *mut ErlNifEnv, term: ErlNifTerm, bin: *mut ErlNifBinary) -> c_int;
}

/// Cached projection pointer of the argument, without consuming anything.
///
/// - **id path**: borrows the registry `Batch`, computes its payload pointer
///   the same way `alloc_envelope` caches it (`envelope_metadata_of`) — no
///   slot take, the handle stays live (checked below by the contract test).
/// - **envelope path**: reads the pointer cached at construction.
fn pointer_of_arg(env: *mut ErlNifEnv, term: ErlNifTerm) -> Option<usize> {
    // --- id path (borrowing) ----------------------------------------------
    let mut id = 0i64;
    if unsafe { enif_get_long(env, term, &mut id) } != 0 {
        if id < 0 {
            return None;
        }
        let reg = crate::lock_registry();
        let batch = reg.get(id as usize)?;
        let batch = batch.as_ref()?;
        let (ptr, _len, _w) = envelope_metadata_of(batch);
        return Some(ptr.as_ptr() as usize);
    }
    // --- envelope path ------------------------------------------------------
    envelope_of(env, term).map(|envl| envl.ptr.as_ptr() as usize)
}

/// `pointer_of(IdOrEnvelope) -> Address :: non_neg_integer() | error`
///
/// Exposes the batch's aligned value-buffer base address. Accepts either a
/// live registry id (integer, borrowing — the handle stays usable) or an
/// envelope term (reference, from `cgap_envelope_new/1`).
pub(crate) extern "C" fn cgap_pointer_of(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    if argc != 1 {
        return make_atom(env, "error");
    }
    match pointer_of_arg(env, args[0]) {
        Some(addr) => unsafe { crate::enif_make_long(env, addr as i64) },
        None => make_atom(env, "error"),
    }
}

/// `cgap_binary_data_pointer(Bin) -> Address :: non_neg_integer() | error`
///
/// Test probe: the data pointer a binary term carries. For a resource binary
/// projected by `cgap_batch_binary/1` this is the payload address
/// `pointer_of` advertises — equality of the two integers is the pointer-
/// identity proof. Plain (non-resource) binaries return their own data
/// pointer; non-binary terms are the atom `error`.
///
/// Test-only surface, registered like the other contract probes with
/// `flags: 0`.
pub(crate) extern "C" fn cgap_binary_data_pointer(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut bin = ErlNifBinary {
        size: 0,
        data: std::ptr::null_mut(),
        ref_bin: std::ptr::null_mut(),
        __spare__: [std::ptr::null_mut(); 2],
    };
    if argc != 1 || unsafe { enif_inspect_binary(env, args[0], &mut bin) } == 0 {
        return make_atom(env, "error");
    }
    unsafe { crate::enif_make_long(env, bin.data as i64) }
}
