//! ops_persist.rs — persistence NIFs (ops 3-5 of 5,
//! `docs/jira/v26.10.31/28-beam-nif-handoff.md` §"API Surface Sketch"):
//!
//! - `cgap_flush_to_parquet/1` — serialize the registered batch through
//!   cgap_parquet's level-free writer (`level_free::write_file`) to a temp
//!   file, return the path binary.
//! - `cgap_seal_to_packfile/1` — wrap the parquet bytes as a blob via
//!   cgap_git's `pack_write::build_pack_and_idx`, flush -> seal -> the
//!   20-byte pack trailer (packfile checksum).
//! - `cgap_receipt_query/1` — the BLAKE3 digest of the batch payload bytes
//!   via `cgap_blake3::hash` (the receipt stand-in for this milestone).
//!
//! All three are fail-closed: wrong arity / non-integer handle -> badarg;
//! unknown or freed handle, or any I/O / encode failure at any stage ->
//! the atom `error`, with the temp file removed before the error term is
//! returned (a failed flush leaves no partial file visible — handoff spec
//! §"Every error path returns before any partial write is visible").
//!
//! Temp-file uniqueness: `temp_dir()/cgap_flush_{pid}_{nanos}.parquet` —
//! pid + wall-clock nanoseconds; two flushes of the same batch in one
//! process produce two distinct paths with identical bytes (court-asserted
//! in `tests/bridge_contract.rs::t_persist_roundtrip`).

use crate::{lock_registry, make_atom, Batch, ErlNifBinary, ErlNifEnv, ErlNifTerm};
use std::os::raw::c_int;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many bytes of parquet we are willing to buffer for one NIF call.
/// Registry batches cap at MAX_N rows (1<<28), i64 worst case is 2 GiB —
/// above this cap the op refuses rather than buffering (fail-closed).
const MAX_FLUSH_BYTES: usize = 1 << 30;

/// Batch payload as little-endian fixed-width bytes: the level-free PLAIN
/// encoding is the 1:1 Arrow-buffer mapping (handoff spec Hook 2), so the
/// payload IS the value buffer reinterpreted — i32 ramp -> INT32 column,
/// i64 ramp -> INT64 column.
fn batch_payload(batch: &Batch) -> Vec<u8> {
    match batch {
        Batch::I32(a) => a.values().iter().flat_map(|v| v.to_le_bytes()).collect(),
        Batch::I64(a) => a.values().iter().flat_map(|v| v.to_le_bytes()).collect(),
    }
}

/// One REQUIRED flat integer column named `value` — the schema every
/// registry batch projects to.
fn batch_column(batch: &Batch) -> cgap_parquet::level_free::LevelFreeColumn {
    cgap_parquet::level_free::LevelFreeColumn {
        name: b"value".to_vec(),
        ty: match batch {
            Batch::I32(_) => cgap_parquet::level_free::LevelFreeType::U32,
            Batch::I64(_) => cgap_parquet::level_free::LevelFreeType::U64,
        },
    }
}

/// Encode the batch as a complete level-free parquet file (PAR1-framed,
/// single row group), or a typed reason for the `error` atom.
fn encode_parquet(batch: &Batch) -> Result<Vec<u8>, &'static str> {
    let payload = batch_payload(batch);
    let col = batch_column(batch);
    let width = col.ty.width();
    let num_rows = payload.len() / width;
    if batch.len() != num_rows {
        return Err("row_count_mismatch");
    }
    if payload.len() > MAX_FLUSH_BYTES {
        return Err("flush_too_large");
    }
    let file =
        cgap_parquet::level_free::write_file(std::slice::from_ref(&col), &[&payload], num_rows)
            .map_err(|_| "parquet_encode_failed")?;
    // The seal court demands the PAR1 framing be re-checked before a blob is
    // minted from these bytes (fail-closed, not a writer-trust assumption).
    if file.len() < 8 || &file[..4] != b"PAR1" || &file[file.len() - 4..] != b"PAR1" {
        return Err("parquet_framing");
    }
    Ok(file)
}

/// pid + wall-clock-nanos unique temp path under `std::env::temp_dir()`.
fn temp_parquet_path() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "cgap_flush_{}_{}.parquet",
        std::process::id(),
        nanos
    ))
}

/// `cgap_flush_to_parquet(parquet_id) -> {ok, Path :: binary()} | error`
///
/// Serializes the registered batch through the level-free writer to a temp
/// file. CPU-bound (encode + fs write), registered dirty.
pub(crate) extern "C" fn cgap_flush_to_parquet(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { crate::enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    // Encode under the registry borrow; write OUTSIDE the lock (no I/O under
    // the registry mutex).
    let file = {
        let reg = lock_registry();
        match reg.get(id as usize) {
            Some(Some(batch)) => match encode_parquet(batch) {
                Ok(f) => f,
                Err(_) => return make_atom(env, "error"),
            },
            _ => return make_atom(env, "error"),
        }
    };
    let path = temp_parquet_path();
    if std::fs::write(&path, &file).is_err() {
        let _ = std::fs::remove_file(&path);
        return make_atom(env, "error");
    }
    // A PathBuf built from temp_dir + formatted decimal components; the
    // encoded bytes are exactly the on-disk path bytes on this platform.
    make_ok_binary(env, path_as_bytes(&path))
}

/// The path's raw encoded bytes (`OsStr::as_encoded_bytes`, stable 1.74).
fn path_as_bytes(p: &std::path::Path) -> &[u8] {
    p.as_os_str().as_encoded_bytes()
}

/// `cgap_seal_to_packfile(handle) -> {ok, Oid :: binary()} | error`
///
/// Encodes the batch to parquet bytes (same encoder as
/// `cgap_flush_to_parquet`), packs them as one git blob via
/// `cgap_git::pack_write::build_pack_and_idx`, and returns the 20-byte pack
/// trailer — the packfile checksum that names the packfile's content.
/// CPU-bound, registered dirty.
pub(crate) extern "C" fn cgap_seal_to_packfile(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { crate::enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    match seal_bytes(id) {
        Some(oid) => make_ok_binary(env, &oid),
        None => make_atom(env, "error"),
    }
}

/// Shared seal path: registry borrow -> encode -> pack, all fail-closed.
fn seal_bytes(id: i64) -> Option<[u8; 20]> {
    let reg = lock_registry();
    let batch = reg.get(id as usize)?.as_ref()?;
    let file = encode_parquet(batch).ok()?;
    let (pack, _idx, entries) = cgap_git::pack_write::build_pack_and_idx(&[&file]).ok()?;
    if entries.len() != 1 || pack.len() < 20 {
        return None;
    }
    pack[pack.len() - 20..].try_into().ok()
}

/// `cgap_receipt_query(handle) -> Digest :: 32-byte binary()` (bare binary,
/// per the handoff surface; unknown/freed handle is the atom `error`).
///
/// BLAKE3 digest of the batch payload bytes — the receipt stand-in. Pure
/// metadata-class read, normal scheduler.
pub(crate) extern "C" fn cgap_receipt_query(
    env: *mut ErlNifEnv,
    argc: c_int,
    argv: *const ErlNifTerm,
) -> ErlNifTerm {
    let args = unsafe { std::slice::from_raw_parts(argv, argc.max(0) as usize) };
    let mut id = 0i64;
    if argc != 1 || unsafe { crate::enif_get_long(env, args[0], &mut id) } == 0 || id < 0 {
        return make_atom(env, "error");
    }
    match receipt_digest(id) {
        Some(d) => make_plain_binary(env, &d),
        None => make_atom(env, "error"),
    }
}

/// Registry-borrow -> BLAKE3 of the payload, fail-closed.
fn receipt_digest(id: i64) -> Option<[u8; 32]> {
    let reg = lock_registry();
    let batch = reg.get(id as usize)?.as_ref()?;
    Some(cgap_blake3::hash(&batch_payload(batch)))
}

/// Make an `{ok, Binary}` tuple from `bytes` (enif_alloc_binary + copy +
/// enif_make_binary + enif_make_tuple). Fail-closed: an alloc refusal is
/// the atom `error`, never a null-data binary.
fn make_ok_binary(env: *mut ErlNifEnv, bytes: &[u8]) -> ErlNifTerm {
    let bin_term = make_plain_binary(env, bytes);
    unsafe { crate::enif_make_tuple(env, 2, make_atom(env, "ok"), bin_term) }
}

/// Make a bare binary term from `bytes`; an alloc refusal is the atom
/// `error` (fail-closed, never a null-data binary).
fn make_plain_binary(env: *mut ErlNifEnv, bytes: &[u8]) -> ErlNifTerm {
    let mut bin = ErlNifBinary {
        size: 0,
        data: std::ptr::null_mut(),
        ref_bin: std::ptr::null_mut(),
        __spare__: [std::ptr::null_mut(); 2],
    };
    if unsafe { crate::enif_alloc_binary(bytes.len(), &mut bin) } == 0 {
        return make_atom(env, "error");
    }
    // SAFETY: enif_alloc_binary handed us `bytes.len()` writable bytes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), bin.data, bytes.len()) };
    unsafe { crate::enif_make_binary(env, &mut bin) }
}
