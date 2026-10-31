//! Destructor-exactly-once observability (wave-5 lane 14, FR-07 acceptance #3).
//!
//! A process-global monotonic counter of BatchEnvelope destructor runs,
//! incremented from [`crate::resources`]'s dtor and read back BEAM-side via
//! `avec_dtor_stats/0 -> {Drops :: non_neg_integer()}`. The exact-once law is
//! then testable from the BEAM side: create M envelopes, drop them all, GC,
//! and the counter delta must be exactly M — not 0 (dtor unreachable) and not
//! 2M (double destruction).

use crate::{enif_make_long, enif_make_tuple, make_atom, ErlNifEnv, ErlNifTerm};
use std::os::raw::c_int;
use std::sync::atomic::{AtomicU64, Ordering};

/// Total number of BatchEnvelope destructor invocations since the dylib was
/// loaded. Relaxed: the counter is a diagnostic aggregate, and each
/// increment/read site needs no cross-variable ordering.
static DTOR_RUNS: AtomicU64 = AtomicU64::new(0);

/// Called exactly once per envelope from the resource dtor.
pub(crate) fn note_envelope_dtor() {
    DTOR_RUNS.fetch_add(1, Ordering::Relaxed);
}

/// Current counter value, clamped to the i64 range the BEAM term can carry.
pub(crate) fn dtor_runs() -> i64 {
    i64::try_from(DTOR_RUNS.load(Ordering::Relaxed)).unwrap_or(i64::MAX)
}

/// `avec_dtor_stats() -> {Drops :: non_neg_integer()}` — the destructor-exactly-
/// once observability surface. Never fails: arity is enforced by the stub.
pub(crate) extern "C" fn avec_dtor_stats(
    env: *mut ErlNifEnv,
    argc: c_int,
    _argv: *const ErlNifTerm,
) -> ErlNifTerm {
    if argc != 0 {
        return make_atom(env, "error");
    }
    unsafe { enif_make_tuple(env, 1, enif_make_long(env, dtor_runs())) }
}
