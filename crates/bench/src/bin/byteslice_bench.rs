//! Byteslice bench (lane HWL2): byte-aligned scan rate + point-get lookup
//! rate. Mirrors the packed_filter_bench house pattern: counting GlobalAlloc
//! with a non-vacuity probe, min-of-5 rounds with MIN as the receipt,
//! black_box elision guards, and a pre-flight differential cross-check
//! (scan mask == materialize-then-filter mask, bitwise) on the real bench
//! payload BEFORE any timing.
//!
//! Payload: ROWS = 2^20 logical rows at EVERY width k in {2, 4, 8, 12};
//! bytes moved = ROWS * ceil(k/8) (the byte-aligned sub-column input), so
//! Mrows/s is directly comparable across widths.
//!
//! The essay's "<0.5 cycles/value" point-lookup figure is CITED-CONTEXT
//! from the byteslice literature, NOT a measurement of this crate — the
//! `point_get` lines below are the measured numbers for this host.

use avec_parquet::byteslice::{pack_byteslice, point_get, scan_eq_byteslice};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 2^20 logical rows at every width (comparable Mrows/s across widths).
const ROWS: usize = 1_048_576;
/// Min-of-N: best (least-noisy) round wins; concurrent lanes make this noisy.
const ROUNDS: usize = 5;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

static COUNT: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: delegates to System, counting calls.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNT.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// Deterministic LCG (same shape as the packing.rs test vectors): stable
/// bench inputs across runs, no deps.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn value(&mut self, k: u8) -> u32 {
        let mask = if k == 16 { 0xFFFFu64 } else { (1u64 << k) - 1 };
        (self.next() & mask) as u32
    }
}

/// One measurement line: warm-up, then min-of-ROUNDS timed rounds. Allocs
/// are from the winning (fastest) round — MIN is the receipt. Returns
/// Mrows/s.
fn bench_line(op: &str, k: u8, bytes_moved: u64, rows: u64, mut f: impl FnMut()) -> f64 {
    f(); // warm-up (page in, branch caches)
    let mut best = f64::INFINITY;
    let mut best_allocs = usize::MAX;
    for _ in 0..ROUNDS {
        COUNT.store(0, Ordering::Relaxed);
        let t0 = Instant::now();
        f();
        let dt = t0.elapsed().as_secs_f64();
        let allocs = COUNT.load(Ordering::Relaxed);
        if dt < best {
            best = dt;
            best_allocs = allocs;
        }
    }
    let gibps = bytes_moved as f64 / best / GIB;
    let mrows = rows as f64 / best / 1e6;
    println!("byteslice {op} k={k}: {gibps:.3} GiB/s | {mrows:.1} Mrows/s | {best_allocs} allocs");
    mrows
}

/// Consume the mask head/mid/tail so the scan output cannot be elided (the
/// packing_bench mask lesson: un-consumed output invents rate).
fn consume_words(m: &[u64]) {
    black_box(m[0] ^ m[m.len() / 2] ^ m[m.len() - 1]);
}

fn main() {
    // Non-vacuity probe: a deliberate allocation must bump COUNT before any
    // "0 allocs" line below is trusted (counted nothing != allocated nothing).
    COUNT.store(0, Ordering::Relaxed);
    let probe: Vec<u8> = vec![0xAB; 4096];
    black_box(&probe);
    assert!(
        COUNT.load(Ordering::Relaxed) >= 1,
        "allocator hook not wired: deliberate Vec alloc did not bump COUNT"
    );

    println!(
        "# byteslice_bench: 2^20 rows per width, min-of-{ROUNDS} rounds, MIN is the receipt; \
         concurrent lanes on this host make individual rounds noisy — the min bounds true \
         capability, a mean would average noise in"
    );
    println!(
        "# point lookup: the essay's <0.5 cycles/value is CITED-CONTEXT, not a measurement; \
         the point_get lines below are this host's numbers (ns/lookup is printed, no cycle \
         assumption)"
    );

    for k in [2u8, 4, 8, 12] {
        let mut rng = Lcg(0x5EED_51CE_0000 + u64::from(k) * 7919);
        let values: Vec<u32> = (0..ROWS).map(|_| rng.value(k)).collect();
        // Needle guaranteed to occur: mid-page value (non-vacuous scan).
        let needle = values[ROWS / 2];

        // ---- pre-flight: build payload + differential cross-check BEFORE
        // any timing: scan mask must equal the materialize-then-filter mask
        // (point_get + naive compare) BITWISE on the real payload ----
        let packed = pack_byteslice(&values, k);
        let cols = packed.len();
        assert_eq!(cols, usize::from(k.div_ceil(8)), "k={k}: column count");
        assert_eq!(packed[0].len(), ROWS, "k={k}: one byte per row");

        let mask_words = ROWS.div_ceil(64);
        let mut mask = vec![0u64; mask_words];
        scan_eq_byteslice(&packed, k, needle, &mut mask);
        let mut ref_mask = vec![0u64; mask_words];
        let mut hits = 0u64;
        for (i, &v) in values.iter().enumerate() {
            if v == needle {
                ref_mask[i / 64] |= 1u64 << (i % 64);
                hits += 1;
            }
        }
        assert_eq!(
            mask, ref_mask,
            "k={k}: scan mask != materialize-then-filter reference"
        );
        assert!(hits > 0, "k={k}: bench needle never hits — vacuous scan");
        println!(
            "byteslice pre-flight k={k}: rows={ROWS}, sub-columns={cols} ({} B), \
             needle={needle} hits {hits} rows (masks bitwise-equal)",
            ROWS * cols
        );

        // ---- scan (SWAR over the byte-aligned sub-columns) ----
        // The mask buffer is allocated once outside the timed region and
        // fully overwritten by scan word 0 of column 0 every round (rule:
        // the kernel WRITES every word, so no zeroing inside the loop —
        // same protocol as the packed_filter_bench packed form).
        let _ = bench_line("scan", k, (ROWS * cols) as u64, ROWS as u64, || {
            let p: &[Vec<u8>] = black_box(&packed);
            let m: &mut [u64] = black_box(&mut mask);
            scan_eq_byteslice(p, k, needle, m);
            consume_words(m);
        });

        // ---- point_get lookup rate: full-row walk per round, O(1)
        // contiguous reads (cols array loads, no bit addressing) ----
        let pb: &[Vec<u8>] = black_box(&packed);
        let kk = black_box(k);
        let lookup = || {
            let mut acc = 0u32;
            for i in 0..ROWS {
                acc = acc.wrapping_add(point_get(pb, kk, i));
            }
            black_box(acc);
        };
        lookup(); // warm-up
        let mut best = f64::INFINITY;
        let mut best_allocs = usize::MAX;
        for _ in 0..ROUNDS {
            COUNT.store(0, Ordering::Relaxed);
            let t0 = Instant::now();
            lookup();
            let dt = t0.elapsed().as_secs_f64();
            let allocs = COUNT.load(Ordering::Relaxed);
            if dt < best {
                best = dt;
                best_allocs = allocs;
            }
        }
        let mlookups = ROWS as f64 / best / 1e6;
        let ns = best / ROWS as f64 * 1e9;
        println!(
            "byteslice point_get k={k}: {mlookups:.1} Mlookups/s | {ns:.3} ns/lookup | \
             {best_allocs} allocs"
        );
    }
}
