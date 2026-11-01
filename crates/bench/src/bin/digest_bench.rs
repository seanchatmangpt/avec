//! Hashed-mask digest A/B bench (lane PCL5).
//!
//! Measures [`cgap_arrow::digest::mask_words_hashed`] against the house
//! portable kernel [`cgap_arrow::bitmap::mask_words`] on the SAME buffers:
//! `out[i] = valid[i] & pred[i]` over 8 Mi u64 words (64 MiB per slice,
//! 192 MiB walked per call: 2 read + 1 write), min-of-5 rounds. This
//! measures the design doc's 0%-penalty claim — a hypothesis, expected to
//! come in as a small nonzero (one rotate + xor per word on the read path).
//!
//! Mirrors the neon_ab_bench house pattern: poisoned-sentinel pre-flight
//! with bitwise equality BEFORE any timing, a vacuity guard so the poison
//! check cannot pass vacuously, min-of-5 with a warm-up round, and a
//! post-timing final-state re-check proving the timed calls really wrote
//! the buffer.

use cgap_arrow::bitmap::mask_words;
use cgap_arrow::digest::mask_words_hashed;
use std::hint::black_box;
use std::time::Instant;

/// 8 Mi words x 8 B = 64 MiB per slice; 3 slices = 192 MiB per call.
const WORDS: usize = 8 * 1024 * 1024;
/// Min-of-5: best (least-noisy) round wins; concurrent lanes are noisy.
const ROUNDS: usize = 5;
/// Poisoned-output sentinel (house: silicon_tests, neon_ab_bench).
const SENTINEL: u64 = 0xDEAD_BEEF_DEAD_BEEF;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Deterministic xorshift64* fill: dense mixed bit patterns, never
/// degenerate all-zeros/all-ones (house pattern, neon_ab_bench).
fn pattern(seed: u64, n: usize) -> Vec<u64> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s.wrapping_mul(0x2545_F491_4F6C_DD1D)
        })
        .collect()
}

/// Warm-up call, then min-of-ROUNDS timed rounds; writes the best wall time
/// into `best`. One 192 MiB call per round; `Instant` overhead is noise at
/// that size.
fn timed_rounds(kernel: &mut dyn FnMut(), best: &mut f64) {
    kernel(); // warm-up (page in, branch caches)
    for _ in 0..ROUNDS {
        let t0 = Instant::now();
        kernel();
        let dt = t0.elapsed().as_secs_f64();
        if dt < *best {
            *best = dt;
        }
    }
}

fn main() {
    println!(
        "digest-bench: 3 slices x {WORDS} words x 8 B = 192 MiB/call \
         (2 read + 1 write), min-of-{ROUNDS}, same buffers both kernels"
    );

    let valid = pattern(0x9E37_79B9_7F4A_7C15, WORDS);
    let pred = pattern(0xBF58_476D_1CE4_E5B9, WORDS);
    let expect: Vec<u64> = valid.iter().zip(&pred).map(|(&v, &p)| v & p).collect();

    // ---- correctness pre-flight: poisoned outputs, bitwise equality ----
    let mut out_h = vec![SENTINEL; WORDS];
    let digest = mask_words_hashed(black_box(&valid), black_box(&pred), black_box(&mut out_h));
    assert_eq!(
        out_h, expect,
        "hashed pre-flight bitwise mismatch over {WORDS} words"
    );

    let mut out_b = vec![SENTINEL; WORDS];
    mask_words(black_box(&valid), black_box(&pred), black_box(&mut out_b));
    assert_eq!(
        out_b, expect,
        "baseline pre-flight bitwise mismatch over {WORDS} words"
    );
    assert_eq!(out_h, out_b, "cross-kernel mismatch over {WORDS} words");

    // Vacuity guard: the sentinel must differ from the expected output
    // somewhere, or the poisoned-output check proved nothing.
    assert!(
        expect.iter().any(|&w| w != SENTINEL),
        "poison vacuous: expected == sentinel everywhere"
    );

    // Non-vacuity: the digest of this fixed input is a fixed value; it is
    // printed so a run folding nothing (all-zero) cannot hide.
    assert_ne!(digest, 0, "digest vacuous: mixed patterns folded to zero");
    println!("digest-bench: fixed-input digest = {digest:#018x}");

    let bytes_per_call = (3 * WORDS * 8) as f64;

    // ---- timing: SAME buffers both kernels (identical cache state) ----
    // One 192 MiB call per round; Instant overhead is noise at that size.
    let mut t_hashed = f64::INFINITY;
    let mut t_base = f64::INFINITY;
    timed_rounds(
        &mut || {
            mask_words_hashed(black_box(&valid), black_box(&pred), black_box(&mut out_h));
        },
        &mut t_hashed,
    );
    timed_rounds(
        &mut || {
            mask_words(black_box(&valid), black_box(&pred), black_box(&mut out_b));
        },
        &mut t_base,
    );

    // Final-state check: the timed calls really wrote every word.
    assert_eq!(out_h, expect, "post-timing hashed buffer mismatch");
    assert_eq!(out_b, expect, "post-timing baseline buffer mismatch");

    let hashed_gib = bytes_per_call / t_hashed / GIB;
    let base_gib = bytes_per_call / t_base / GIB;
    let penalty = (t_hashed - t_base) / t_base * 100.0;
    println!("hashed mask: {hashed_gib:.1} GiB/s vs {base_gib:.1} baseline, penalty {penalty:.1}%");
}
