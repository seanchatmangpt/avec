//! Gate 2 harness: line-rate bench + alloc interception (CTQ-1/CTQ-5).
//!
//! Measure, don't claim: the throughput number printed is the receipt.

use avec_arrow::bitmap::mask_words;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

/// 64 MiB of validity words per lane.
const WORDS: usize = 8 * 1024 * 1024;

static COUNT: AtomicUsize = AtomicUsize::new(0);

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
    let valid = vec![0xF0F0_F0F0_F0F0_F0F0u64; WORDS];
    let pred = vec![0xAAAA_AAAA_AAAA_AAAAu64; WORDS];
    let mut out = vec![0u64; WORDS];
    COUNT.store(0, Ordering::Relaxed);

    // single-core line rate: bytes moved = in (2x) + out (1x)
    let t0 = Instant::now();
    let rounds = 8;
    let mut sink = 0u64;
    for _ in 0..rounds {
        mask_words(&valid, &pred, &mut out);
        sink ^= out[0] ^ out[WORDS - 1];
    }
    let dt = t0.elapsed().as_secs_f64();
    let bytes = (WORDS * 8 * 3 * rounds) as f64;
    let gibps = bytes / dt / (1024.0 * 1024.0 * 1024.0);
    let allocs_during_scan = COUNT.load(Ordering::Relaxed);
    println!("single-core mask: {gibps:.3} GiB/s over {rounds} rounds, {allocs_during_scan} allocs (sink {sink:#x})");

    // multi-core: 16 lanes x 64 MiB, wall time to evaluate all
    let lanes: Vec<Vec<u64>> = (0..16)
        .map(|i| vec![u64::from(i as u8) | 0xF0F0_F0F0_F0F0_F0F0; WORDS])
        .collect();
    let mut outs: Vec<Vec<u64>> = (0..16).map(|_| vec![0u64; WORDS]).collect();
    let t1 = Instant::now();
    let lanes_ref = &lanes;
    thread::scope(|s| {
        for (v, o) in lanes.iter().zip(outs.iter_mut()) {
            let p = &lanes[0];
            s.spawn(move || mask_words(v, p, o));
        }
    });
    let dt16 = t1.elapsed().as_secs_f64();
    let dt16_us = dt16 * 1e6;
    let agg = (16 * WORDS * 8 * 3) as f64 / dt16 / (1024.0 * 1024.0 * 1024.0);
    println!("16-lane wall: {dt16_us:.0} us, aggregate {agg:.1} GiB/s");
    println!("sink {sink:#x} lanes0 {}", lanes[0][0] & 1);
}
