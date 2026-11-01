//! Hand-written domain logic for every `cnv:CustomBehavior` command.
//!
//! GENERATED-ONCE scaffold, hand-filled: bodies are the ported logic of the
//! pre-ggen flat CLI (crates/cli/src/main.rs @ c83619d), function-for-
//! function. The ggen graph is crates/cli/ontology/cgap-cli.ttl.
//!
//! Output note: the flat CLI printed one text line per command; the
//! noun-verb runtime prints the returned Value as JSON, so each handler
//! returns the measured quantity as JSON (crates/cli/docs/command-reference.md).

use clap_noun_verb::Result;
use serde_json::{Map, Value};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// `self version` — manifest version via env! (no drift-prone literal).
pub fn self_version(_inputs: Map<String, Value>) -> Result<Value> {
    Ok(Value::String(env!("CARGO_PKG_VERSION").to_string()))
}

/// 64 MiB of validity words per round (8 Mi x 8 B).
const MASK_WORDS: usize = 8 * 1024 * 1024;
/// min-of-5: contention only slows a round down, so the minimum round is
/// the closest approach to the kernel's true line rate.
const MASK_ROUNDS: usize = 5;

/// `bench mask` — 64 MiB `mask_words` line-rate bench, min-of-5. Ported
/// from the flat CLI's cmd_bench_mask; returns the measured rate as JSON.
pub fn bench_mask(_inputs: Map<String, Value>) -> Result<Value> {
    let valid: Vec<u64> = (0..MASK_WORDS)
        .map(|i| 0xF0F0_F0F0_F0F0_F0F0u64 ^ i as u64)
        .collect();
    let pred = vec![0x5555_5555_5555_5555u64; MASK_WORDS];
    let mut out = vec![0u64; MASK_WORDS];

    // Warm-up outside the timed window: pages `out` in, fills caches.
    cgap_arrow::bitmap::mask_words(&valid, &pred, &mut out);
    std::hint::black_box(&out);

    let bytes = (MASK_WORDS * 8 * 3) as f64;
    let mut min_dt = f64::MAX;
    for _ in 0..MASK_ROUNDS {
        let t0 = std::time::Instant::now();
        cgap_arrow::bitmap::mask_words(&valid, &pred, &mut out);
        let w0 = out[0];
        let wl = out[MASK_WORDS - 1];
        std::hint::black_box((&out, w0, wl));
        min_dt = min_dt.min(t0.elapsed().as_secs_f64());
    }
    Ok(serde_json::json!({ "mask_gib_per_s": bytes / min_dt / GIB }))
}

/// Literal page: 8 Mi values x 4 B read per pass.
const ENC_VALUES: usize = 8 * 1024 * 1024;
/// min-of-5, same receipt rule as bench mask.
const ENC_ROUNDS: usize = 5;

/// `bench parquet` — literal-page w=4 `rle_encode_fast` vs `rle_encode`,
/// min-of-5, correctness gate BEFORE timing. Ported from cmd_bench_parquet.
pub fn bench_parquet(_inputs: Map<String, Value>) -> Result<Value> {
    let values: Vec<u32> = (0..ENC_VALUES).map(|i| (i % 5) as u32).collect();

    let base = cgap_parquet::rle::rle_encode(&values, 4);
    let fast = cgap_parquet::encode_fast::rle_encode_fast(&values, 4);
    if base != fast {
        return Err(clap_noun_verb::NounVerbError::execution_error(format!(
            "rle_encode_fast diverged from rle_encode ({} vs {} bytes)",
            base.len(),
            fast.len()
        )));
    }
    let out_len = base.len();
    let bytes = (ENC_VALUES * 4 + out_len) as f64;

    let mut min_fast = f64::MAX;
    let mut min_base = f64::MAX;
    for _ in 0..ENC_ROUNDS {
        let t0 = std::time::Instant::now();
        let out = cgap_parquet::encode_fast::rle_encode_fast(&values, 4);
        std::hint::black_box(&out);
        min_fast = min_fast.min(t0.elapsed().as_secs_f64());

        let t0 = std::time::Instant::now();
        let out = cgap_parquet::rle::rle_encode(&values, 4);
        std::hint::black_box(&out);
        min_base = min_base.min(t0.elapsed().as_secs_f64());
    }
    Ok(serde_json::json!({
        "rle_encode_fast_gib_per_s": bytes / min_fast / GIB,
        "rle_encode_gib_per_s": bytes / min_base / GIB
    }))
}

/// `align test` — assert 64-byte alignment for Buf sizes 1..1000 step 7.
/// Ported from cmd_align_test; misalignment is a typed error (fail closed).
pub fn align_test(_inputs: Map<String, Value>) -> Result<Value> {
    let sizes: Vec<usize> = (1..1000).step_by(7).collect();
    let total = sizes.len();
    let mut aligned = 0usize;
    for len in sizes {
        let buf = cgap_arrow::buffer::Buf::new(len);
        if (buf.as_ptr() as usize).is_multiple_of(cgap_arrow::buffer::ALIGNMENT) {
            aligned += 1;
        } else {
            return Err(clap_noun_verb::NounVerbError::execution_error(format!(
                "align test FAIL: len {len} misaligned"
            )));
        }
        std::hint::black_box(&buf);
    }
    Ok(serde_json::json!({ "aligned": aligned, "total": total }))
}
