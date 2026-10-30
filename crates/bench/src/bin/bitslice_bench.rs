//! Vertical bit-slice pack + pruned range scan bench (lane HWL1).
//!
//! Measure, don't claim: every printed line is the receipt. BitWeaving's
//! published speedups are cited context only — this bin reports what the
//! pruned vertical scanner does on this machine against a local unpruned
//! contrast kernel with identical mask algebra (same four lane masks, same
//! final `!(hi_gt | lo_lt)`, but no bound-update skip, no word skip, no
//! break: it walks all `w` words of every group).
//!
//! House pattern (mirrors packing_bench): sentinel-poisoned outputs in a
//! 3-way pre-flight (pruned == unpruned == materialize-then-filter reference
//! over the real payload) BEFORE any timing, min-of-5 rounds with MIN as the
//! receipt, and black_box elision guards — each timed call consumes a
//! popcount reduction of its own output mask (the mask bench's absurd
//! numbers came from un-consumed output).
//!
//! Payload: 1M random rows per width, widths {2,4,8,12} — cache-resident at
//! these sizes (packed column <= 1.5 MiB), so this is a compute-rate
//! receipt, not a DRAM-bandwidth one. Rates are per packed input bytes
//! (groups * w * 8). Range classes: full [0,m], empty (in-domain inverted
//! lo=m, hi=m-1), top-half [2^(w-1), m], bottom-half [0, 2^(w-1)-1], point
//! (a value present in the data). `skipped` is the pruning-effectiveness
//! column: fraction of the column's words never loaded, from
//! scan_range_vertical_counted on the same bytes.
//!
//! v26.10.29.

use avec_parquet::bitslice::{pack_vertical, scan_range_vertical, scan_range_vertical_counted};
use std::hint::black_box;
use std::time::Instant;

const ROWS: usize = 1_000_000;
const WIDTHS: [u8; 4] = [2, 4, 8, 12];
/// Min-of-N rounds; MIN is the receipt (concurrent lanes make this noisy).
const ROUNDS: usize = 5;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
/// Sentinel far outside any mask word semantics: a partial write cannot
/// survive the equality pre-flight.
const SENT: u64 = 0xA55A_5AA5_0F0F_F0F0;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn next_bounded(&mut self, m: u32) -> u32 {
        ((self.next() >> 32) as u32) & m
    }
}

fn gen_values(n: usize, w: u8) -> Vec<u32> {
    let mut rng = Rng(
        0x9E37_79B9_7F4A_7C15
            ^ (n as u64).wrapping_mul(0xFF51_AFD7_ED55_8CCD)
            ^ (w as u64) << 32,
    );
    (0..n).map(|_| rng.next_bounded(mask_of(w))).collect()
}

fn mask_of(w: u8) -> u32 {
    ((1u64 << w) - 1) as u32
}

fn classes(w: u8, point: u32) -> Vec<(&'static str, u32, u32)> {
    let m = mask_of(w);
    vec![
        ("full", 0, m),
        ("empty", m, m - 1),
        ("top-half", 1 << (w - 1), m),
        ("bottom-half", 0, (1 << (w - 1)) - 1),
        ("point", point, point),
    ]
}

/// Contrast kernel: identical mask algebra to the library scanner but no
/// pruning — every word of every group is loaded and both bound updates run
/// at every level. Used only to quantify what the pruning cascade buys here.
fn scan_unpruned(src: &[u64], w: u8, lo: u32, hi: u32, out_valid: &mut [u64]) {
    let wf = w as usize;
    let domain_max = (1u64 << w) - 1;
    if lo as u64 > domain_max {
        for o in out_valid.iter_mut().take(src.len() / wf) {
            *o = 0;
        }
        return;
    }
    let hi = (hi as u64).min(domain_max) as u32;
    for (g, words) in src.chunks_exact(wf).enumerate() {
        let (mut hi_gt, mut hi_lt, mut lo_gt, mut lo_lt) = (0u64, 0u64, 0u64, 0u64);
        for (lvl, &bits) in words.iter().rev().enumerate() {
            let level = wf - 1 - lvl;
            let hb = (((hi >> level) & 1) as u64).wrapping_neg();
            let lb = (((lo >> level) & 1) as u64).wrapping_neg();
            let eq_hi = !(hi_gt | hi_lt);
            let eq_lo = !(lo_gt | lo_lt);
            hi_gt |= eq_hi & bits & !hb;
            hi_lt |= eq_hi & hb & !bits;
            lo_gt |= eq_lo & bits & !lb;
            lo_lt |= eq_lo & lb & !bits;
        }
        out_valid[g] = !(hi_gt | lo_lt);
    }
}

/// Materialize-then-filter reference over 64-padded values.
fn materialize_reference(values: &[u32], lo: u32, hi: u32) -> Vec<u64> {
    let groups = values.len().div_ceil(64);
    let mut out = vec![0u64; groups];
    for (i, &v) in values.iter().enumerate() {
        if lo <= v && v <= hi {
            out[i / 64] |= 1u64 << (i % 64);
        }
    }
    out
}

fn assert_masks_eq(a: &[u64], b: &[u64], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: length");
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            x, y,
            "{what}: first mask diff at group {i}: {x:#018x} vs {y:#018x}"
        );
    }
}

/// Calibrate one round, then min-of-ROUNDS; returns best seconds per call.
fn time_min_per_iter<F: FnMut() -> u64>(mut f: F) -> f64 {
    let mut iters: u64 = 1;
    let per_call = loop {
        let t0 = Instant::now();
        let mut acc = 0u64;
        for _ in 0..iters {
            acc = acc.wrapping_add(black_box(f()));
        }
        black_box(acc);
        let per = t0.elapsed().as_secs_f64() / iters as f64;
        if per >= 0.05 || iters >= 1 << 24 {
            break per;
        }
        iters = (iters * 2).max(2);
    };
    let iters: u64 = ((0.12 / per_call).ceil() as u64).clamp(1, 1 << 24);
    let mut best = f64::INFINITY;
    for _ in 0..ROUNDS {
        let t0 = Instant::now();
        let mut acc = 0u64;
        for _ in 0..iters {
            acc = acc.wrapping_add(black_box(f()));
        }
        black_box(acc);
        let per = t0.elapsed().as_secs_f64() / iters as f64;
        if per < best {
            best = per;
        }
    }
    best
}

fn main() {
    let profile = if cfg!(debug_assertions) {
        "DEBUG (numbers not receipts)"
    } else {
        "release"
    };
    println!("# bitslice_bench v26.10.29 lane HWL1: rows={ROWS} widths={WIDTHS:?} profile={profile}");
    println!(
        "# min-of-{ROUNDS} rounds, MIN is the receipt; rates = packed input bytes (groups*w*8);"
    );
    println!("# skipped = fraction of column words never loaded (MSB-first pruning cascade);");
    println!("# classes: full[0,m] empty[m,m-1] top-half[2^w-1..,m] bottom-half[0,2^w-1-1] point(present)");
    println!(
        "# {:>2}  {:<11} {:>10} {:>10} {:>8} {:>13} {:>8}",
        "w", "class", "GiB/s", "Mrows/s", "skipped", "unprnd-GiB/s", "spd"
    );

    let mut digest = 0u64;
    for &w in &WIDTHS {
        let values = gen_values(ROWS, w);
        let packed = pack_vertical(&values, w);
        let groups = packed.len() / w as usize;
        let bytes = packed.len() * 8;
        let point = values[ROWS / 2];
        let mut padded = values;
        padded.resize(groups * 64, 0);

        for (name, lo, hi) in classes(w, point) {
            // --- pre-flight: 3-way agreement on real payload, poisoned ---
            let mut pruned = vec![SENT; groups];
            let mut unprnd = vec![SENT; groups];
            scan_range_vertical(&packed, w, lo, hi, &mut pruned);
            scan_unpruned(&packed, w, lo, hi, &mut unprnd);
            let expect = materialize_reference(&padded, lo, hi);
            assert_masks_eq(
                &pruned,
                &expect,
                &format!("pruned vs reference w={w} class={name}"),
            );
            assert_masks_eq(
                &unprnd,
                &expect,
                &format!("unpruned vs reference w={w} class={name}"),
            );

            let touched =
                scan_range_vertical_counted(&packed, w, lo, hi, &mut pruned);
            let skip = 1.0 - touched as f64 / (groups * w as usize) as f64;

            // --- timing: pruned ---
            let secs_p = time_min_per_iter(|| {
                let src: &[u64] = black_box(&packed);
                scan_range_vertical(src, w, lo, hi, &mut pruned);
                pruned.iter().map(|x| x.count_ones() as u64).sum::<u64>()
            });
            // --- timing: unpruned contrast ---
            let secs_u = time_min_per_iter(|| {
                let src: &[u64] = black_box(&packed);
                scan_unpruned(src, w, lo, hi, &mut unprnd);
                unprnd.iter().map(|x| x.count_ones() as u64).sum::<u64>()
            });

            digest = digest.wrapping_add(
                pruned
                    .iter()
                    .map(|x| x.count_ones() as u64)
                    .sum::<u64>(),
            );

            let gp = bytes as f64 / secs_p / GIB;
            let gu = bytes as f64 / secs_u / GIB;
            let mrows = ROWS as f64 / secs_p / 1e6;
            println!(
                "# {:>2}  {:<11} {:>10.1} {:>10.1} {:>7.1}% {:>13.1} {:>7.2}x",
                w,
                name,
                gp,
                mrows,
                skip * 100.0,
                gu,
                secs_u / secs_p
            );
        }
    }
    println!("# mask popcount digest (consumed output): {digest}");
    black_box(digest);
}
