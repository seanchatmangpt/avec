//! `avec-cli` — std-hosted CLI over the avec columnar kernels.
//!
//! Subcommands (std::env::args matching, no clap):
//!
//! - `version`      — prints `26.10.29`
//! - `bench-mask`   — 64 MiB `mask_words` line-rate bench, min-of-5,
//!   prints the single line `mask <X.XXX> GiB/s`
//! - `bench-parquet` — literal-page w=4 `rle_encode_fast` vs `rle_encode`
//!   comparison, prints one comparison line
//! - `align-test`   — allocates `Buf` sizes 1..1000 step 7, asserts 64-byte
//!   alignment on every pointer, prints `align-test: 143/143 aligned`
//!
//! Dependencies: std + two zero-dep repo-local path crates (avec-arrow,
//! avec-parquet). No external crates.

use avec_arrow::bitmap::mask_words;
use avec_arrow::buffer::Buf;
use avec_parquet::encode_fast::rle_encode_fast;
use avec_parquet::rle::rle_encode;
use std::hint::black_box;
use std::process::exit;
use std::time::Instant;

/// Version law: literal, in lockstep with the root workspace version.
const VERSION: &str = "26.10.29";
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("version") => cmd_version(),
        Some("bench-mask") => cmd_bench_mask(),
        Some("bench-parquet") => cmd_bench_parquet(),
        Some("align-test") => cmd_align_test(),
        _ => {
            eprintln!(
                "usage: avec-cli <version|bench-mask|bench-parquet|align-test>"
            );
            exit(2);
        }
    }
}

fn cmd_version() {
    println!("{VERSION}");
}

/// Bytes moved by one `mask_words` pass over `words` words.
///
/// Formula: `words * size_of::<u64>() * (2 reads + 1 write)` = `words * 8 * 3`.
///
/// Per 64-bit word: 8 bytes read from `valid`, 8 bytes read from `pred`,
/// 8 bytes written to `out` — all three unconditionally, because the kernel
/// is a data-independent AND. Selectivity changes how many RESULT bits are
/// set, not how many bytes cross the memory system.
fn mask_bytes_moved(words: usize) -> f64 {
    (words * std::mem::size_of::<u64>() * 3) as f64
}

/// 64 MiB of validity words per round (8 Mi x 8 B).
const MASK_WORDS: usize = 8 * 1024 * 1024;
/// min-of-5: contention only ever slows a round down, so the minimum round
/// is the closest approach to the kernel's true line rate.
const MASK_ROUNDS: usize = 5;

fn cmd_bench_mask() {
    // valid varies with index so the black_box'd sink carries real bits; a
    // uniform vector would make the fold tautologically zero. The kernel is
    // a data-independent AND, so value shape cannot change the timing.
    let valid: Vec<u64> = (0..MASK_WORDS)
        .map(|i| 0xF0F0_F0F0_F0F0_F0F0u64 ^ i as u64)
        .collect();
    let pred = vec![0x5555_5555_5555_5555u64; MASK_WORDS];
    let mut out = vec![0u64; MASK_WORDS];

    // Warm-up outside the timed window: pages `out` in, fills caches.
    mask_words(&valid, &pred, &mut out);
    black_box(&out);

    let bytes = mask_bytes_moved(MASK_WORDS);
    let mut min_dt = f64::MAX;
    for _ in 0..MASK_ROUNDS {
        let t0 = Instant::now();
        mask_words(&valid, &pred, &mut out);
        // black_box on the outputs: the compiler must assume `out` escapes
        // and the reads are observed, so the 64 MiB of stores are real.
        let w0 = out[0];
        let wl = out[MASK_WORDS - 1];
        black_box((&out, w0, wl));
        min_dt = min_dt.min(t0.elapsed().as_secs_f64());
    }

    println!("mask {:.3} GiB/s", bytes / min_dt / GIB);
}

/// Bytes moved by one RLE encode pass over `values` (a literal page).
///
/// Formula: `values.len() * size_of::<u32>()` bytes READ (the input values)
/// plus the encoded output bytes WRITTEN (`out.len()`, measured — both
/// encoders are byte-identical so the write side is the same for either).
/// The encoded size is data-dependent, so it is measured, not modeled.
fn encode_bytes_moved(n_values: usize, out_len: usize) -> f64 {
    (n_values * std::mem::size_of::<u32>() + out_len) as f64
}

/// Literal page size: 8 Mi values x 4 B = 32 MiB read per pass.
const ENC_VALUES: usize = 8 * 1024 * 1024;
/// min-of-5, same receipt rule as bench-mask.
const ENC_ROUNDS: usize = 5;

fn cmd_bench_parquet() {
    // w = 4: lanes carry 4 bits. Values cycle 0..5 so no 3 consecutive
    // values are ever equal — every record is a literal group, never a run:
    // this is the literal-page input the comparison line names.
    let values: Vec<u32> = (0..ENC_VALUES).map(|i| (i % 5) as u32).collect();

    // Correctness gate before timing: the fast path is contractually
    // byte-identical to the baseline. Time nothing if that fails.
    let base = rle_encode(&values, 4);
    let fast = rle_encode_fast(&values, 4);
    if base != fast {
        eprintln!(
            "bench-parquet FAIL: rle_encode_fast output diverged from rle_encode ({} vs {} bytes)",
            base.len(),
            fast.len()
        );
        exit(1);
    }
    let out_len = base.len();

    // min-of-5 per encoder; inputs and outputs are identical so the
    // comparison is apples-to-apples.
    let mut min_fast = f64::MAX;
    let mut min_base = f64::MAX;
    for _ in 0..ENC_ROUNDS {
        let t0 = Instant::now();
        let out = rle_encode_fast(&values, 4);
        black_box(&out);
        min_fast = min_fast.min(t0.elapsed().as_secs_f64());

        let t0 = Instant::now();
        let out = rle_encode(&values, 4);
        black_box(&out);
        min_base = min_base.min(t0.elapsed().as_secs_f64());
    }

    let bytes = encode_bytes_moved(ENC_VALUES, out_len);
    println!(
        "parquet w=4 literal rle_encode_fast {:.3} vs rle_encode {:.3} GiB/s",
        bytes / min_fast / GIB,
        bytes / min_base / GIB
    );
}

fn cmd_align_test() {
    // Sizes 1, 8, 15, ..., 995 — 143 allocations, each must land on a
    // 64-byte boundary (charter M-03 pointer law: ptr & 63 == 0).
    let sizes: Vec<usize> = (1..1000).step_by(7).collect();
    let total = sizes.len();
    let mut aligned = 0usize;
    for len in sizes {
        let buf = Buf::new(len);
        if (buf.as_ptr() as usize).is_multiple_of(avec_arrow::buffer::ALIGNMENT) {
            aligned += 1;
        } else {
            eprintln!("align-test FAIL: len {len} misaligned");
            exit(1);
        }
        black_box(&buf);
        drop(buf);
    }
    println!("align-test: {aligned}/{total} aligned");
}
