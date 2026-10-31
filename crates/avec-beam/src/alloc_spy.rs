//! Allocation spy (FR-07 acceptance #2, wave-5 lane 13): make hot-path
//! allocation across the NIF boundary observable.
//!
//! `avec_alloc_stats/0` answers `{TotalAllocs :: non_neg_integer(),
//! TotalBytes :: non_neg_integer()}` from two process-global atomic counters.
//!
//! WHAT IS COUNTED — exactly the avec-beam-owned allocation call sites (the
//! places this crate asks the VM for memory on a batch's behalf):
//! - `alloc_envelope` (resources.rs): one `enif_alloc_resource` per
//!   `BatchEnvelope`, `size_of::<BatchEnvelope>()` bytes — hit by every
//!   `avec_envelope_new/1` and by the consuming id path of
//!   `avec_batch_binary/1`.
//! - `avec_version/0`: one `enif_alloc_binary` of the version-string length.
//!
//! WHAT IS NOT COUNTED (documented, not hidden):
//! - `enif_alloc`/`enif_alloc_binary` calls made by the BEAM itself for term
//!   construction (small integers and tuples are immediate/heap terms anyway)
//!   — the spy instruments OUR call sites, not the VM allocator.
//! - `enif_make_resource_binary` — zero-copy by construction (resbin.rs): no
//!   bytes move, so a projection must not show up as an allocation.
//! - The registry `Mutex<Vec<Option<Batch>>>` growth (std allocator inside
//!   the crate's own Vec) — amortized, uncorrelated with per-read cost, and
//!   invisible to the read-path claim under test.
//! - avec-arrow's builder allocations (separate crate, not this membrane's
//!   surface) — they happen inside `avec_new*`, whose allocation behavior is
//!   already measured by the scheduling courts, not the read-path spy.
//!
//! The contract claim this surface makes testable: the READ hot path
//! (`avec_len`, `avec_stats`, `avec_get`, `avec_get_i64`,
//! `avec_envelope_len`, `pointer_of`, `avec_binary_data_pointer`) hits none
//! of the counted sites, so K reads through the bridge must show a delta of
//! exactly {0, 0} — the court in `tests/bridge_contract.rs`.

use crate::{enif_make_long, enif_make_tuple, ErlNifEnv, ErlNifTerm};
use std::os::raw::c_int;
use std::sync::atomic::{AtomicU64, Ordering};

/// Cumulative allocation events at instrumented crate call sites.
static SPY_ALLOCS: AtomicU64 = AtomicU64::new(0);
/// Cumulative bytes requested at those same sites.
static SPY_BYTES: AtomicU64 = AtomicU64::new(0);

/// Record one instrumented allocation of `bytes`. Relaxed is sound: the
/// counters are monotonic diagnostics; no other state is ordered on them and
/// the test snapshots use monotone deltas, not intra-call ordering.
pub(crate) fn record_alloc(bytes: usize) {
    SPY_ALLOCS.fetch_add(1, Ordering::Relaxed);
    SPY_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// `avec_alloc_stats() -> {TotalAllocs, TotalBytes} | error`
///
/// Snapshot of the instrumented allocation counters (see module doc for the
/// exact counted/un counted ledger). Arity-wrong calls are the atom `error`,
/// same convention as `avec_version/0`.
pub(crate) extern "C" fn avec_alloc_stats(
    env: *mut ErlNifEnv,
    argc: c_int,
    _argv: *const ErlNifTerm,
) -> ErlNifTerm {
    if argc != 0 {
        return crate::make_atom(env, "error");
    }
    let allocs = SPY_ALLOCS.load(Ordering::Relaxed);
    let bytes = SPY_BYTES.load(Ordering::Relaxed);
    // Counters are far below i64::MAX by construction (each event is a real
    // VM allocation in this process); the casts are lossless in practice.
    unsafe {
        enif_make_tuple(
            env,
            2,
            enif_make_long(env, allocs as i64),
            enif_make_long(env, bytes as i64),
        )
    }
}
