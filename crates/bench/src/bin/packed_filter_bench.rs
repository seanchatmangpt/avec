//! Packed-domain equality filter bench (lane PCL3).
//!
//! Measure, don't claim: every printed line is the receipt, and the design
//! doc's 300-600 GiB/s "effective" figure is an EXTRAPOLATION — the numbers
//! below replace it. Mirrors the packing_bench house pattern: counting
//! GlobalAlloc with a non-vacuity probe, min-of-5 rounds with MIN as the
//! receipt, black_box elision guards, and a pre-flight bitwise cross-check
//! of BOTH forms on the real bench payload before any timing.
//!
//! Payload: PACKED_BYTES = 8 MiB of tight packed input at EVERY width, so
//! packed-input GiB/s is directly comparable across widths; the logical row
//! count is PACKED_BYTES*8/w (64 Mi rows at w=1 down to 8 Mi at w=8).
//!
//! METRIC LABELS (both forms report both, labeled):
//! - `packed GiB/s`: packed input bytes / second — the wire-rate metric.
//! - `Mrows/s`: logical rows filtered / second (= 8/w packed rows per byte).
//!
//! The `EFFECTIVE` summary line is the packed-domain form's logical-rows/s:
//! end-to-end rows admitted per second with NO materialization.
//!
//! WHERE PACKED-DOMAIN WINS AND LOSES (the honest paragraph, printed below):
//! packed-domain wins when the page stays packed — the mask is produced in
//! one pass over the wire bytes and no unpacked copy of the page is ever
//! allocated. It LOSES when selectivity or the query forces materialization
//! anyway: if the caller needs the VALUES of the matching rows (projection,
//! aggregation, additional predicates), the packed scan only defers the
//! decode, and once a materialized page exists, materialize-then-mask
//! amortizes one decode across every predicate evaluated on it — repeated
//! packed-domain scans over the same page re-read and re-decode bits each
//! time instead.

use cgap_arrow::bitmap::set_bit;
use cgap_parquet::packed_filter::{filter_eq_packed, packed_rows};
use cgap_parquet::packing::{pack_bits, unpack_bits_tight};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 8 MiB packed input at every width (comparable GiB/s across widths).
const PACKED_BYTES: usize = 8 * 1024 * 1024;
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
    fn value(&mut self, w: u8) -> u32 {
        let mask = if w == 8 { 0xFFu64 } else { (1u64 << w) - 1 };
        (self.next() & mask) as u32
    }
}

/// One measurement line: warm-up, then min-of-ROUNDS timed rounds.
/// Allocs are from the winning (fastest) round — MIN is the receipt.
/// `bytes_moved` is the PACKED input size; `rows` the logical row count.
/// Both metrics are labeled on the line. Returns the logical Mrows/s.
fn bench_line(op: &str, w: u8, bytes_moved: u64, rows: u64, mut f: impl FnMut()) -> f64 {
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
    let packed_gibps = bytes_moved as f64 / best / GIB;
    let mrows = rows as f64 / best / 1e6;
    println!(
        "packed_filter {op} w={w}: {packed_gibps:.3} packed GiB/s | {mrows:.1} Mrows/s \
         (logical) | {best_allocs} allocs"
    );
    mrows
}

/// Consume the mask head/mid/tail so the filter output cannot be elided
/// (the packing_bench mask lesson: un-consumed output invents rate).
fn consume_words(m: &[u64]) {
    black_box(m[0] ^ m[m.len() / 2] ^ m[m.len() - 1]);
}

fn consume_bytes(b: &[u8]) {
    black_box(
        u64::from(b[0]) ^ u64::from(b[b.len() / 2]) ^ u64::from(b[b.len() - 1]),
    );
}

/// Ones across the byte-form validity mask (non-vacuity: the bench needle
/// must actually hit the payload).
fn mask_ones(bytes: &[u8]) -> u64 {
    bytes.iter().map(|b| u64::from(b.count_ones())).sum()
}

/// LSB-first u64 word k of a byte-form validity mask (zero-padded tail), so
/// the byte-form mask and the word-form mask compare bit-for-bit.
fn word_of(bytes: &[u8], k: usize) -> u64 {
    let off = k * 8;
    let mut buf = [0u8; 8];
    let take = (bytes.len().saturating_sub(off)).min(8);
    buf[..take].copy_from_slice(&bytes[off..off + take]);
    u64::from_le_bytes(buf)
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
        "# packed_filter_bench: 8 MiB packed input per width; min-of-{ROUNDS} rounds, \
         MIN is the receipt; concurrent lanes on this host make individual rounds \
         noisy — the min bounds true capability, a mean would average noise in"
    );
    println!(
        "# WHERE PACKED-DOMAIN WINS: the page stays packed — one pass over the wire \
         bytes emits the mask, no unpacked copy of the page is ever allocated. WHERE \
         IT LOSES: when selectivity or the query forces materialization anyway \
         (projection/aggregation/more predicates need the VALUES), the packed scan \
         only defers the decode; with a materialized page in hand, \
         materialize-then-mask amortizes ONE decode across every predicate, and \
         repeated packed-domain scans of the same page re-decode bits each time."
    );

    let mut effective = [(0u8, 0f64); 4];

    for (wi, w) in [1u8, 2, 4, 8].into_iter().enumerate() {
        let rows = packed_rows(PACKED_BYTES, w);
        let mut rng = Lcg(0x5EED_F11E_0000 + u64::from(w) * 7919);
        let values: Vec<u32> = (0..rows).map(|_| rng.value(w)).collect();
        // Needle guaranteed to occur: mid-page value (non-vacuous filter).
        let needle = values[rows / 2];

        // ---- pre-flight: build payload + cross-check BEFORE any timing ----
        let packed = pack_bits(&values, w);
        assert_eq!(packed.len(), PACKED_BYTES, "w={w}: packed byte count");

        // Form B reference state: materialize, then per-row compare through
        // the house byte-bitmap API (set_bit), the way a materializing
        // caller actually builds validity.
        let mut materialized = vec![0u32; rows];
        unpack_bits_tight(&packed, w, &mut materialized).expect("pre-flight tight decode");

        let mask_bytes = rows.div_ceil(8);
        let mut mask_ref = vec![0u8; mask_bytes];
        for (i, lane) in materialized.iter().enumerate() {
            if *lane == needle {
                set_bit(&mut mask_ref, i);
            }
        }

        let mask_words = rows.div_ceil(64);
        let mut mask_fast = vec![0u64; mask_words];
        filter_eq_packed(&packed, w, needle, &mut mask_fast);
        for k in 0..mask_words {
            assert_eq!(
                mask_fast[k],
                word_of(&mask_ref, k),
                "w={w} word {k}: packed-domain mask != materialize-then-mask reference"
            );
        }
        let ones = mask_ones(&mask_ref);
        assert!(ones > 0, "w={w}: bench needle never hits — vacuous filter");
        println!(
            "packed_filter pre-flight w={w}: rows={rows}, packed={} B, mask={} B, \
             needle={needle} hits {ones} rows (masks bitwise-equal)",
            packed.len(),
            mask_bytes
        );

        // ---- packed-domain filter (the mask-only SWAR path) ----
        let mrows_packed = bench_line("packed-filter", w, PACKED_BYTES as u64, rows as u64, || {
            let b: &[u8] = black_box(&packed);
            let m: &mut [u64] = black_box(&mut mask_fast);
            filter_eq_packed(b, w, needle, m);
            consume_words(m);
        });

        // ---- materialize-then-mask (unpack_bits_tight + per-row compare) ----
        // Mask zeroing is inside the timed region: form B must produce a
        // FRESH mask per round, exactly as a form B caller would.
        let mrows_mat = bench_line("materialize-mask", w, PACKED_BYTES as u64, rows as u64, || {
            let b: &[u8] = black_box(&packed);
            let vals: &mut [u32] = black_box(&mut materialized);
            unpack_bits_tight(b, w, vals).expect("tight decode");
            let m: &mut [u8] = black_box(&mut mask_ref);
            m.fill(0);
            for (i, lane) in vals.iter().enumerate() {
                if *lane == needle {
                    set_bit(m, i);
                }
            }
            consume_bytes(m);
        });

        let speedup = mrows_packed / mrows_mat;
        println!(
            "packed_filter summary w={w}: packed-domain is {speedup:.2}x the \
             materialize-then-mask logical-rows/s"
        );
        effective[wi] = (w, mrows_packed);

        drop(materialized);
        drop(values);
        drop(packed);
        drop(mask_fast);
        drop(mask_ref);
    }

    println!("# EFFECTIVE logical-rows/s (packed-domain filter, no materialization):");
    for (w, mrows) in effective {
        println!("#   effective w={w}: {mrows:.1} Mrows/s");
    }
}
