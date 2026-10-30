//! Gate 2 harness: line-rate bench + alloc interception (CTQ-1/CTQ-5).
//!
//! Measure, don't claim: the min GiB/s line is the receipt. Per-round lines are
//! printed so the variance is visible, not hidden behind an average.
//!
//! NOISE NOTE: concurrent lanes compile and bench on this same machine; numbers
//! are noisy by construction. The per-round spread is the noise made visible,
//! and the MIN round is the receipt — contention only ever slows a round down,
//! so the minimum is the closest approach to the kernel's true line rate.

use avec_arrow::bitmap::mask_words;
use std::hint::black_box;
use std::sync::Barrier;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

/// 64 MiB of validity words per lane.
const WORDS: usize = 8 * 1024 * 1024;
/// Rounds per pattern; the minimum round is the receipt (min-of-N, not mean).
const ROUNDS: usize = 7;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

static COUNT: AtomicUsize = AtomicUsize::new(0);

/// Installed as the process allocator: without this attribute the counter is
/// never wired up and the CTQ-5 zero-alloc gate passes vacuously (counted
/// nothing, not allocated nothing).
#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

struct CountingAlloc;

// SAFETY: delegates to System, counting calls.
unsafe impl std::alloc::GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        COUNT.fetch_add(1, Ordering::Relaxed);
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        unsafe { std::alloc::System.dealloc(p, l) }
    }
}

fn main() {
    // CTQ-5 non-vacuity probe: prove the allocator hook actually counts before
    // any "0 allocs" line is trusted. A deliberate allocation must bump COUNT;
    // if it does not, the global allocator is not wired up and every zero on a
    // gate line below would be "counted nothing", not "allocated nothing".
    COUNT.store(0, Ordering::Relaxed);
    let probe = vec![0u8; 4096];
    black_box(&probe);
    drop(probe);
    let probe_count = COUNT.load(Ordering::Relaxed);
    println!("CTQ-5 probe: deliberate alloc bumped counter to {probe_count} (gate is live)");
    if probe_count == 0 {
        eprintln!("CTQ-5 VACUOUS GATE: deliberate allocation did not increment the alloc counter");
        std::process::exit(1);
    }

    // valid varies with index so the black_box'd sink carries real bits; a
    // uniform vector would make the fold tautologically zero. The kernel is a
    // data-independent AND, so value shape cannot change the timing.
    let valid: Vec<u64> = (0..WORDS)
        .map(|i| 0xF0F0_F0F0_F0F0_F0F0u64 ^ i as u64)
        .collect();
    let mut out = vec![0u64; WORDS];

    let per_round_bytes = bytes_moved(WORDS);
    println!(
        "mask bench: {WORDS} words ({:.0} MiB/vec), {ROUNDS} rounds/pattern. NOTE: concurrent \
         lanes make these numbers noisy — MIN is the receipt",
        per_round_bytes / 3.0 / (1024.0 * 1024.0)
    );

    // ---------------- single-core selectivity sweep (primary receipt) ----------------
    // Sparse-to-dense predicate patterns. mask_words is an unconditional
    // data-independent AND over every word, so selectivity must NOT move the
    // number much — a large spread across this sweep is a finding, not a feature.
    const PATTERNS: [u64; 4] = [
        0x0000_0000_0000_0000, // 0/64 bits  — nothing survives
        0x1111_1111_1111_1111, // 8/64 bits  — sparse
        0x5555_5555_5555_5555, // 32/64 bits — half
        0xFFFF_FFFF_FFFF_FFFF, // 64/64 bits — dense
    ];
    let mut sink = 0u64;
    for pred_word in PATTERNS {
        let pred = vec![pred_word; WORDS];

        // warm-up outside the timed window: pages `out` in, fills caches.
        mask_words(&valid, &pred, &mut out);
        black_box(&out);

        COUNT.store(0, Ordering::Relaxed);
        let mut min_dt = f64::MAX;
        for round in 1..=ROUNDS {
            let t0 = Instant::now();
            mask_words(&valid, &pred, &mut out);
            // black_box on the outputs: the compiler must assume `out` escapes
            // and the reads are observed, so the 64 MiB of stores are real.
            let w0 = out[0];
            let wl = out[WORDS - 1];
            black_box((&out, w0, wl));
            let dt = t0.elapsed().as_secs_f64();
            sink ^= w0 ^ wl;

            let gibps = per_round_bytes / dt / GIB;
            println!(
                "  pred {pred_word:#018x} sel {:>2}/64 round {round}/{ROUNDS}: {dt_us:>7.0} us  {gibps:7.3} GiB/s",
                pred_word.count_ones(),
                dt_us = dt * 1e6,
            );
            min_dt = min_dt.min(dt);
        }

        let min_gibps = per_round_bytes / min_dt / GIB;
        println!(
            "  pred {pred_word:#018x} MIN {min_gibps:7.3} GiB/s ({min_dt_ms:.3} ms/round)",
            min_dt_ms = min_dt * 1e3,
        );

        // CTQ-5 gate: the scan path must be allocation-free. Hard fail.
        let allocs = COUNT.load(Ordering::Relaxed);
        println!("  pred {pred_word:#018x} allocs during scan: {allocs} (gate: 0)");
        if allocs != 0 {
            eprintln!(
                "CTQ-5 FAIL: {allocs} allocations inside the scan loop (pred {pred_word:#018x})",
            );
            std::process::exit(1);
        }
    }

    // ---------------- 16-lane aggregate (secondary line) ----------------
    // Multi-core: 16 lanes x 64 MiB, wall time to evaluate all.
    let lanes: Vec<Vec<u64>> = (0..16)
        .map(|i| vec![u64::from(i as u8) | 0xF0F0_F0F0_F0F0_F0F0; WORDS])
        .collect();
    let mut outs: Vec<Vec<u64>> = (0..16).map(|_| vec![0u64; WORDS]).collect();
    let t1 = Instant::now();
    thread::scope(|s| {
        // 17-party barrier scopes the alloc counter to the kernel window:
        // threads park AFTER their spawn-time allocations (thread spawn itself
        // allocates — witnessed 51 allocs from 16 spawns), main resets COUNT
        // and arrives last, so release happens-after the reset for every
        // thread and only mask_words is counted.
        let barrier = Barrier::new(17);
        for (v, o) in lanes.iter().zip(outs.iter_mut()) {
            let p = &lanes[0];
            let b = &barrier;
            s.spawn(move || {
                b.wait();
                mask_words(v, p, o);
            });
        }
        COUNT.store(0, Ordering::Relaxed);
        barrier.wait();
    });
    let dt16 = t1.elapsed().as_secs_f64();
    black_box(&outs);
    let allocs16 = COUNT.load(Ordering::Relaxed);
    if allocs16 != 0 {
        eprintln!("CTQ-5 FAIL: {allocs16} allocations inside the 16-lane scan");
        std::process::exit(1);
    }
    // Bytes formula for the aggregate: 16 lanes x per-round bytes.
    let agg = (16.0 * per_round_bytes) / dt16 / GIB;
    println!("16-lane wall: {:.0} us, aggregate {agg:.1} GiB/s (secondary, noisy)", dt16 * 1e6);
    println!("sink {sink:#x} lanes0 {}", lanes[0][0] & 1);
}

/// Bytes moved by one `mask_words` pass over `words` words.
///
/// Formula: `words * size_of::<u64>() * (2 reads + 1 write)`
///   = `words * 8 * 3`.
///
/// Per 64-bit word: 8 bytes read from `valid`, 8 bytes read from `pred`, 8 bytes
/// written to `out` — all three unconditionally, because the kernel is a
/// data-independent AND. Selectivity changes how many RESULT bits are set, not
/// how many bytes cross the memory system.
fn bytes_moved(words: usize) -> f64 {
    (words * std::mem::size_of::<u64>() * 3) as f64
}
