//! NEON vs portable `mask_words` A/B bench (lane OPT20-L8).
//!
//! Measures [`cgap_arrow::silicon::mask_words_neon`] against the house
//! portable kernel [`cgap_arrow::bitmap::mask_words`] on the SAME buffers:
//! `out[i] = valid[i] & pred[i]` over n u64 words, sizes {1K, 64K, 1M, 8M}
//! words, min-of-7 rounds. This settles whether the NEON path earns its keep
//! on M-series silicon.
//!
//! Bytes-moved convention: 3 slices per call (2 read + 1 write) x 8n bytes
//! = 24n bytes per call; both kernels walk identical buffers, so the ratio
//! is convention-independent.
//!
//! Pre-flight BEFORE any timing, per size: both kernels run into outputs
//! poisoned to a sentinel and must reproduce the scalar reference bitwise
//! (a skipped tail word survives as 0xDEADBEEF.. and fails loudly, exit
//! nonzero, before any number is printed). A vacuity guard asserts the
//! expected output actually differs from the sentinel somewhere, so the
//! poison check cannot pass vacuously. After timing, the output buffer is
//! re-checked against the reference — final-state proof the timed calls
//! really wrote the buffer.
//!
//! On non-aarch64 targets `mask_words_neon` IS the portable fallback by
//! cfg; the dispatch line names the compiled path so a ratio near 1.0 from
//! a fallback is never mistaken for a silicon result.

use cgap_arrow::bitmap::mask_words;
use cgap_arrow::silicon::mask_words_neon;
use std::hint::black_box;
use std::time::Instant;

/// Word counts benched: 1K, 64K, 1M, 8M words.
const SIZES: [usize; 4] = [1024, 64 * 1024, 1024 * 1024, 8 * 1024 * 1024];
/// Min-of-7: best (least-noisy) round wins; concurrent lanes are noisy.
const ROUNDS: usize = 7;
/// Target timed traffic per round (~256 MiB) so `Instant` overhead is noise
/// at the 1K-word size (iters are clamped to [1, 65536]).
const ROUND_BYTES_TARGET: usize = 256 * 1024 * 1024;
/// Poisoned-output sentinel (house: silicon_tests).
const SENTINEL: u64 = 0xDEAD_BEEF_DEAD_BEEF;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Deterministic xorshift64* fill: dense mixed bit patterns, never
/// degenerate all-zeros/all-ones (house pattern, silicon_tests).
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

/// Warm-up, then min-of-ROUNDS timed rounds of `iters` calls; returns the
/// best per-call time in seconds.
fn best_per_call(f: &mut dyn FnMut(), iters: usize) -> f64 {
    f(); // warm-up (page in, branch caches)
    let mut best = f64::INFINITY;
    for _ in 0..ROUNDS {
        let t0 = Instant::now();
        for _ in 0..iters {
            f();
        }
        let dt = t0.elapsed().as_secs_f64();
        if dt < best {
            best = dt;
        }
    }
    best / iters as f64
}

fn main() {
    println!(
        "neon-ab: dispatch = {}",
        if cfg!(target_arch = "aarch64") {
            "aarch64 NEON intrinsics (compile-time cfg)"
        } else {
            "NON-aarch64 — mask_words_neon is the portable fallback by cfg"
        }
    );
    println!(
        "neon-ab: bytes/call = 3 slices x 8n (2 read + 1 write), \
         min-of-{ROUNDS}, same buffers both kernels"
    );

    for &n in &SIZES {
        let valid = pattern(0x9E37_79B9_7F4A_7C15, n);
        let pred = pattern(0xBF58_476D_1CE4_E5B9, n);
        let expect: Vec<u64> = valid
            .iter()
            .zip(&pred)
            .map(|(&v, &p)| v & p)
            .collect();

        // ---- correctness pre-flight: poisoned outputs, bitwise equality ----
        let mut out = vec![SENTINEL; n];
        mask_words_neon(&valid, &pred, &mut out);
        assert_eq!(out, expect, "NEON pre-flight bitwise mismatch, n = {n} words");

        let mut out_port = vec![SENTINEL; n];
        mask_words(&valid, &pred, &mut out_port);
        assert_eq!(out_port, expect, "portable pre-flight bitwise mismatch, n = {n} words");
        assert_eq!(out, out_port, "cross-kernel mismatch, n = {n} words");

        // Vacuity guard: the sentinel must differ from the expected output
        // somewhere, or the poisoned-output check proved nothing.
        assert!(
            expect.iter().any(|&w| w != SENTINEL),
            "poison vacuous: expected == sentinel everywhere, n = {n} words"
        );

        let bytes_per_call = (3 * n * 8) as f64;
        let iters = (ROUND_BYTES_TARGET / (3 * n * 8)).clamp(1, 65536);

        // ---- timing: SAME buffers both kernels (identical cache state) ----
        let t_neon = best_per_call(
            &mut || {
                mask_words_neon(
                    black_box(&valid[..]),
                    black_box(&pred[..]),
                    black_box(&mut out[..]),
                )
            },
            iters,
        );
        let t_port = best_per_call(
            &mut || {
                mask_words(
                    black_box(&valid[..]),
                    black_box(&pred[..]),
                    black_box(&mut out[..]),
                )
            },
            iters,
        );

        // Final-state check: the timed calls really wrote every word.
        assert_eq!(out, expect, "post-timing buffer mismatch, n = {n} words");

        let neon_gib = bytes_per_call / t_neon / GIB;
        let port_gib = bytes_per_call / t_port / GIB;
        println!(
            "neon-ab n={n}: neon {neon_gib:.1} GiB/s portable {port_gib:.1} GiB/s ratio {:.2}",
            neon_gib / port_gib
        );
    }
}
