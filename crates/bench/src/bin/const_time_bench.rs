//! Const-time data-independence instrument (lane PCL8, charter v26.10.29).
//!
//! Pure measurement substrate for the data-independence audit — NO pass/fail
//! assertions anywhere: every printed number is an observation, the spread
//! line names what was measured, never a verdict.
//!
//! Per kernel, per distribution class, K = 100 deterministic input instances
//! (identical row count for every kernel x class x instance), each timed
//! min-of-3 repetitions; the per-class table and the `spread: max/min` line
//! report the measured ratio with the argmax/argmin class named.
//!
//! Distribution classes (all over the same ROWS lanes):
//! sorted / reverse / clustered / constant / alternating /
//! random-high-entropy / permuted (Fisher-Yates of the sorted class — same
//! value multiset as sorted, different order: the histogram control).
//!
//! Kernels under measurement (house contract list):
//! - [`avec_arrow::bitmap::mask_words`] — u64 SWAR AND (value-independent by
//!   construction; the instrument should show ~1.0x spread),
//! - [`avec_parquet::rle::decode_into`] — full-buffer hybrid decode,
//! - [`avec_parquet::encode_fast::rle_encode_fast`] — run-DETECTION kernel:
//!   data-dependent by design (the >= 3-run scan collapses sorted/constant
//!   shapes into O(distinct runs) records and leaves random shapes as
//!   literal groups), so it is EXPECTED to spread — this instrument reports
//!   the measured magnitude, it does not judge it,
//! - [`avec_parquet::rle::rle_decode`] — full-buffer decode wrapper
//!   (delegate of `decode_into`; the page each class produces IS the
//!   workload),
//! - [`avec_arrow::runend::RunEndEncoded::get`] — binary-search resolve,
//!   swept over all logical rows of a 1,048,576-row / 100-run array (the
//!   accessors-bench REE shape); the class applies to the 100 per-run
//!   values, run ends stay fixed so the search path is identical for every
//!   class,
//! - [`avec_parquet::dictionary::decode_page`] — decode + in-place gather
//!   through a 256-entry dictionary.
//!
//! Method laws (house): deterministic splitmix64 seeds keyed
//! (kernel, class, instance) — replayable byte-identical inputs; the same
//! allocations are reused across classes (kernel scratch buffers are
//! refilled in place per instance, never reallocated; the one exception is
//! the RLE page under the decode kernels, which is the per-instance input
//! produced untimed by the real encoder); `black_box` guards every input
//! and head/mid/tail output consumption so no kernel can be elided;
//! min-of-3 within each instance kills scheduling noise (concurrent-lane
//! discipline).
//!
//! REE note: the public [`avec_arrow::runend::RunEndEncodedBuilder`]
//! collapses equal adjacent runs (Arrow shape law), so a 100-run array
//! cannot carry one identical value in every run — ANY lawful 100-run value
//! sequence changes value at every one of its 99 adjacent boundaries. The
//! `constant` class for the REE kernel is therefore realized at minimum
//! value entropy: strict {C, C+1} alternation (the tightest two-value set
//! that keeps exactly 100 runs). The instrument counts every instance whose
//! built array has `num_runs() != 100` as an anomaly (printed, never
//! asserted) so a shape drift can never masquerade as a timing effect.
//! Every other class is realized directly.

use avec_arrow::bitmap::mask_words;
use avec_arrow::runend::RunEndEncodedBuilder;
use avec_parquet::dictionary::decode_page;
use avec_parquet::encode_fast::rle_encode_fast;
use avec_parquet::rle::{decode_into, rle_decode, rle_encode};
use std::hint::black_box;
use std::time::Instant;

/// Logical rows for EVERY kernel x class x instance (identical row count).
const ROWS: usize = 1 << 20;
/// Deterministic input instances per (kernel, class).
const K: usize = 100;
/// Min-of-N repetitions per instance: least-noisy rep wins.
const REPS: usize = 3;
/// REE shape: 1M-row / 100-run array (accessors-bench shape).
const RUNS: usize = 100;
/// Dictionary cardinality for the dict kernel (width-8 kernel set).
const DICT: usize = 256;

/// Distribution classes, in table order.
const CLASSES: [&str; 7] = [
    "sorted",
    "reverse",
    "clustered",
    "constant",
    "alternating",
    "random-high-entropy",
    "permuted",
];

// ---------------------------------------------------------------------------
// Deterministic randomness (splitmix64) — replayable seeds.
// ---------------------------------------------------------------------------

/// splitmix64: tiny, deterministic, well-distributed; no deps (house
/// freestanding discipline — the only collaborator is the real allocator).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, n)` (modulo bias is irrelevant to an instrument).
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Instance seed keyed by (kernel, class, instance), stirred through one
/// splitmix64 step so nearby keys land far apart in the stream.
fn instance_seed(kernel: usize, class: usize, k: usize) -> u64 {
    let z = (kernel as u64)
        .wrapping_mul(0xA24B_AED5_3CE5_8C4E)
        ^ (class as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (k as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ 0x1234_5678_9ABC_DEF0;
    SplitMix(z).next()
}

// ---------------------------------------------------------------------------
// Distribution generators — one u32-lane semantics shared by the byte-domain
// kernels (rle / encode_fast / dict), a u64-word variant for `mask_words`,
// and a 100-run-value variant for the REE kernel. Same class names, same
// shape semantics everywhere.
// ---------------------------------------------------------------------------

/// Class id in `CLASSES` order.
const C_SORTED: usize = 0;
const C_REVERSE: usize = 1;
const C_CLUSTERED: usize = 2;
const C_CONSTANT: usize = 3;
const C_ALTERNATING: usize = 4;
const C_RANDOM: usize = 5;
const C_PERMUTED: usize = 6;

/// Fill `v` with the named distribution over the byte domain `[0, 256)`.
fn fill_u32(v: &mut [u32], class: usize, seed: u64) {
    let n = v.len();
    match class {
        // Sorted: 256-value ascending staircase (nondecreasing, run-shaped).
        C_SORTED => {
            for (i, x) in v.iter_mut().enumerate() {
                *x = ((i as u64) * 255 / (n - 1) as u64) as u32;
            }
        }
        // Reverse: the same staircase, descending.
        C_REVERSE => {
            for (i, x) in v.iter_mut().enumerate() {
                *x = 255 - ((i as u64) * 255 / (n - 1) as u64) as u32;
            }
        }
        // Clustered: seeded blocks (64..192 lanes) of one value each, values
        // drawn fresh per block — mixed run/literal record structure.
        C_CLUSTERED => {
            let mut r = SplitMix(seed);
            let mut i = 0;
            while i < n {
                let block = 64 + r.below(128) as usize;
                let val = r.below(256) as u32;
                let end = (i + block).min(n);
                v[i..end].fill(val);
                i = end;
            }
        }
        // Constant: one seeded value everywhere (maximal runs).
        C_CONSTANT => {
            let val = SplitMix(seed).below(256) as u32;
            v.fill(val);
        }
        // Alternating: two values, strictly alternating lanes.
        C_ALTERNATING => {
            let lo = SplitMix(seed).below(256) as u32;
            let hi = 255 - lo;
            for (i, x) in v.iter_mut().enumerate() {
                *x = if i & 1 == 0 { lo } else { hi };
            }
        }
        // Random high entropy: uniform over the byte domain.
        C_RANDOM => {
            let mut r = SplitMix(seed);
            for x in v.iter_mut() {
                *x = r.below(256) as u32;
            }
        }
        // Permuted: Fisher-Yates of the sorted staircase — identical value
        // multiset to `sorted`, different order.
        C_PERMUTED => {
            for (i, x) in v.iter_mut().enumerate() {
                *x = ((i as u64) * 255 / (n - 1) as u64) as u32;
            }
            let mut r = SplitMix(seed);
            for i in (1..n).rev() {
                let j = r.below((i + 1) as u64) as usize;
                v.swap(i, j);
            }
        }
        _ => unreachable!("class id out of range"),
    }
}

/// Fill `w` with the named distribution over the full u64 domain (word-level
/// variant for `mask_words` — bit patterns, not byte lanes).
fn fill_u64(w: &mut [u64], class: usize, seed: u64) {
    let n = w.len();
    match class {
        C_SORTED => {
            for (i, x) in w.iter_mut().enumerate() {
                *x = ((i as u128) * u32::MAX as u128 / (n - 1) as u128) as u64;
            }
        }
        C_REVERSE => {
            for (i, x) in w.iter_mut().enumerate() {
                *x = u64::MAX - ((i as u128) * u32::MAX as u128 / (n - 1) as u128) as u64;
            }
        }
        C_CLUSTERED => {
            // Blocks sharing one high word (clustered magnitude), random low.
            let mut r = SplitMix(seed);
            let mut i = 0;
            while i < n {
                let block = 8 + r.below(56) as usize;
                let hi = r.next() & 0xFFFF_FFFF_0000_0000;
                let end = (i + block).min(n);
                for x in &mut w[i..end] {
                    *x = hi | (r.next() & 0xFFFF_FFFF);
                }
                i = end;
            }
        }
        C_CONSTANT => {
            let val = SplitMix(seed).next();
            w.fill(val);
        }
        C_ALTERNATING => {
            let a = SplitMix(seed).next();
            let b = !a;
            for (i, x) in w.iter_mut().enumerate() {
                *x = if i & 1 == 0 { a } else { b };
            }
        }
        C_RANDOM => {
            let mut r = SplitMix(seed);
            for x in w.iter_mut() {
                *x = r.next();
            }
        }
        C_PERMUTED => {
            for (i, x) in w.iter_mut().enumerate() {
                *x = ((i as u128) * u32::MAX as u128 / (n - 1) as u128) as u64;
            }
            let mut r = SplitMix(seed);
            for i in (1..n).rev() {
                let j = r.below((i + 1) as u64) as usize;
                w.swap(i, j);
            }
        }
        _ => unreachable!("class id out of range"),
    }
}

/// Per-run values for the REE kernel: 100 values with adjacent runs forced
/// distinct (the builder collapses equal adjacent runs — shape law).
fn fill_ree_values(vals: &mut [i32], class: usize, seed: u64) {
    let n = vals.len(); // RUNS
    match class {
        C_SORTED | C_PERMUTED => {
            // Distinct ascending staircase (permutation keeps it distinct).
            for (r, v) in vals.iter_mut().enumerate() {
                *v = ((r as u64) * 255 / (n - 1) as u64) as i32;
            }
            if class == C_PERMUTED {
                let mut r = SplitMix(seed);
                for i in (1..n).rev() {
                    let j = r.below((i + 1) as u64) as usize;
                    vals.swap(i, j);
                }
            }
        }
        C_REVERSE => {
            for (r, v) in vals.iter_mut().enumerate() {
                *v = 255 - ((r as u64) * 255 / (n - 1) as u64) as i32;
            }
        }
        C_CLUSTERED => {
            let mut r = SplitMix(seed);
            let mut prev: i32 = -1;
            for v in vals.iter_mut() {
                let mut val = r.below(256) as i32;
                if val == prev {
                    val = (val + 13) & 255; // force adjacent-distinct
                }
                *v = val;
                prev = val;
            }
        }
        C_CONSTANT => {
            // Minimum-entropy lawful constant: strict {C, C+1} alternation —
            // any same-value plateau longer than one run collapses (shape
            // law), so 100 runs force a change at every boundary. See the
            // module doc REE note.
            let c = SplitMix(seed).below(255) as i32;
            for (r, v) in vals.iter_mut().enumerate() {
                *v = if r & 1 == 0 { c } else { c + 1 };
            }
        }
        C_ALTERNATING => {
            let c = SplitMix(seed).below(243) as i32;
            for (r, v) in vals.iter_mut().enumerate() {
                *v = if r & 1 == 0 { c } else { c + 13 };
            }
        }
        C_RANDOM => {
            let mut r = SplitMix(seed);
            let mut prev: i32 = -1;
            for v in vals.iter_mut() {
                let mut val = r.below(256) as i32;
                if val == prev {
                    val = (val + 13) & 255;
                }
                *v = val;
                prev = val;
            }
        }
        _ => unreachable!("class id out of range"),
    }
}

// ---------------------------------------------------------------------------
// Timing + stats.
// ---------------------------------------------------------------------------

/// Min-of-`REPS` wall-clock nanoseconds for `f`; inputs must already be
/// black_box-guarded inside `f`.
fn time_min_ns(mut f: impl FnMut()) -> f64 {
    f(); // warm-up: page in, branch caches (house)
    let mut best = f64::INFINITY;
    for _ in 0..REPS {
        let t = Instant::now();
        f();
        let d = t.elapsed().as_nanos() as f64;
        best = best.min(d);
    }
    best
}

/// (min, median, max) over the per-instance samples (house lower median).
fn stats(mut xs: Vec<f64>) -> (f64, f64, f64) {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (
        xs[0],
        xs[xs.len() / 2],
        xs[xs.len() - 1],
    )
}

/// Head/mid/tail consumption through `black_box` so a decode output cannot
/// be elided (house).
fn consume_u32(o: &[u32]) {
    black_box(u64::from(o[0]) ^ u64::from(o[o.len() / 2]) ^ u64::from(o[o.len() - 1]));
}

/// Same for u64 word buffers (`mask_words` out).
fn consume_u64(o: &[u64]) {
    black_box(o[0] ^ o[o.len() / 2] ^ o[o.len() - 1]);
}

/// One per-class record: instance samples plus the page wire size (0 for
/// kernels without a page input) and the anomaly count (decode failures /
/// REE shape drift) — printed when nonzero, never asserted.
struct ClassStat {
    wire_bytes: u64,
    errors: usize,
    samples: Vec<f64>,
}

/// Print the per-class table + spread line; the spread uses the per-class
/// MIN statistic (most noise-robust) with argmax/argmin classes named.
fn report(kernel: &str, idx: usize, per_class: &[ClassStat]) {
    println!();
    println!(
        "== kernel {idx}/6: {kernel} (rows={ROWS}, K={K} instances/class, min-of-{REPS}) =="
    );
    println!(
        "{:<20} {:>14} {:>14} {:>14} {:>12}",
        "class", "min_ns", "median_ns", "max_ns", "wire_B"
    );
    let mut mins: Vec<(usize, f64)> = Vec::with_capacity(per_class.len());
    for (cid, cs) in per_class.iter().enumerate() {
        let (mn, md, mx) = stats(cs.samples.clone());
        mins.push((cid, mn));
        println!(
            "{:<20} {:>14.0} {:>14.0} {:>14.0} {:>12}",
            CLASSES[cid],
            mn,
            md,
            mx,
            cs.wire_bytes
        );
        if cs.errors != 0 {
            println!(
                "{:<20} anomalies: {} (decode failures / REE shape drift)",
                CLASSES[cid],
                cs.errors
            );
        }
    }
    let (imax, imin) = mins
        .iter()
        .fold((0, 0), |(mx, mn), &(cid, v)| {
            if v > mins[mx].1 {
                (cid, mn)
            } else if v < mins[mn].1 {
                (mx, cid)
            } else {
                (mx, mn)
            }
        });
    let vmax = mins[imax].1;
    let vmin = mins[imin].1;
    let ratio = if vmin > 0.0 { vmax / vmin } else { f64::NAN };
    println!(
        "spread: max/min = {:.3}x (max at {}, min at {})",
        ratio, CLASSES[imax], CLASSES[imin]
    );
}

// ---------------------------------------------------------------------------
// Kernels.
// ---------------------------------------------------------------------------

/// Kernel 1: `bitmap::mask_words` — u64 SWAR AND over ROWS/64 words.
fn bench_mask_words() {
    let words = ROWS / 64;
    let mut valid = vec![0u64; words];
    let mut pred = vec![0u64; words];
    let mut out = vec![0u64; words];
    let mut per_class: Vec<ClassStat> = CLASSES
        .iter()
        .map(|_| ClassStat { wire_bytes: 0, errors: 0, samples: Vec::with_capacity(K) })
        .collect();
    for (cid, _) in CLASSES.iter().enumerate() {
        for k in 0..K {
            let seed = instance_seed(0, cid, k);
            fill_u64(&mut valid, cid, seed);
            fill_u64(&mut pred, cid, seed ^ 0xA5A5_5A5A_5A5A_5A5A);
            let t = time_min_ns(|| {
                mask_words(black_box(&valid), black_box(&pred), black_box(&mut out));
                consume_u64(&out);
            });
            per_class[cid].samples.push(t);
        }
    }
    report("bitmap::mask_words", 1, &per_class);
}

/// Kernel 2: `rle::decode_into` — full-buffer hybrid decode of a class page.
fn bench_decode_into() {
    let mut values = vec![0u32; ROWS];
    let mut out = vec![0u32; ROWS];
    let mut per_class: Vec<ClassStat> = CLASSES
        .iter()
        .map(|_| ClassStat { wire_bytes: 0, errors: 0, samples: Vec::with_capacity(K) })
        .collect();
    for (cid, _) in CLASSES.iter().enumerate() {
        let mut wire_sum = 0u64;
        let mut errors = 0usize;
        for k in 0..K {
            let seed = instance_seed(1, cid, k);
            fill_u32(&mut values, cid, seed);
            let page = rle_encode(&values, 8); // untimed input production
            wire_sum += page.len() as u64;
            let t = time_min_ns(|| {
                match decode_into(black_box(&page), black_box(&mut out)) {
                    Ok(n) => black_box(n),
                    Err(_) => 0,
                };
                consume_u32(&out);
            });
            per_class[cid].samples.push(t);
            // Count-of-decoded-lanes check as pure observation (no assert):
            // any instance that did not fill `out` counts as an error.
            if decode_into(&page, &mut out).unwrap_or(0) != ROWS {
                errors += 1;
            }
        }
        per_class[cid].wire_bytes = wire_sum / K as u64;
        per_class[cid].errors = errors;
    }
    report("rle::decode_into", 2, &per_class);
}

/// Kernel 3: `encode_fast::rle_encode_fast` — run-DETECTION kernel, EXPECTED
/// to spread (the run scan is data-dependent by design; measured, not judged).
fn bench_encode_fast() {
    let mut values = vec![0u32; ROWS];
    let mut per_class: Vec<ClassStat> = CLASSES
        .iter()
        .map(|_| ClassStat { wire_bytes: 0, errors: 0, samples: Vec::with_capacity(K) })
        .collect();
    for (cid, _) in CLASSES.iter().enumerate() {
        let mut wire_sum = 0u64;
        for k in 0..K {
            let seed = instance_seed(2, cid, k);
            fill_u32(&mut values, cid, seed);
            let t = time_min_ns(|| {
                let page = rle_encode_fast(black_box(&values), 8);
                black_box(page.len());
            });
            per_class[cid].samples.push(t);
            wire_sum += rle_encode_fast(&values, 8).len() as u64;
        }
        per_class[cid].wire_bytes = wire_sum / K as u64;
    }
    report("encode_fast::rle_encode_fast", 3, &per_class);
}

/// Kernel 4: `rle::rle_decode` — full-buffer decode (must fill `out`
/// exactly); same class pages as kernel 2's contract, own seeded instances.
fn bench_rle_decode() {
    let mut values = vec![0u32; ROWS];
    let mut out = vec![0u32; ROWS];
    let mut per_class: Vec<ClassStat> = CLASSES
        .iter()
        .map(|_| ClassStat { wire_bytes: 0, errors: 0, samples: Vec::with_capacity(K) })
        .collect();
    for (cid, _) in CLASSES.iter().enumerate() {
        let mut wire_sum = 0u64;
        let mut errors = 0usize;
        for k in 0..K {
            let seed = instance_seed(3, cid, k);
            fill_u32(&mut values, cid, seed);
            let page = rle_encode(&values, 8);
            wire_sum += page.len() as u64;
            let t = time_min_ns(|| {
                match rle_decode(black_box(&page), black_box(&mut out)) {
                    Ok(()) => black_box(0u8),
                    Err(_) => black_box(1u8),
                };
                consume_u32(&out);
            });
            per_class[cid].samples.push(t);
            if rle_decode(&page, &mut out).is_err() {
                errors += 1;
            }
        }
        per_class[cid].wire_bytes = wire_sum / K as u64;
        per_class[cid].errors = errors;
    }
    report("rle::rle_decode", 4, &per_class);
}

/// Kernel 5: `RunEndEncoded::get` — binary-search resolve swept over all
/// logical rows of the 1M-row / 100-run array; per-run values per class.
fn bench_ree_get() {
    // Fixed run ends (accessors-bench shape): 10,485-row runs for r < 24,
    // 10,486-row runs after, so run 99 ends exactly at 2^20.
    let mut ends: Vec<i32> = Vec::with_capacity(RUNS);
    let mut cum = 0usize;
    for r in 0..RUNS {
        cum += 10_485 + usize::from(r >= 24);
        ends.push(cum as i32);
    }
    let mut run_values = vec![0i32; RUNS];
    let mut per_class: Vec<ClassStat> = CLASSES
        .iter()
        .map(|_| ClassStat { wire_bytes: 0, errors: 0, samples: Vec::with_capacity(K) })
        .collect();
    for (cid, _) in CLASSES.iter().enumerate() {
        for k in 0..K {
            let seed = instance_seed(4, cid, k);
            fill_ree_values(&mut run_values, cid, seed);
            let mut b = RunEndEncodedBuilder::<i32>::new();
            for r in 0..RUNS {
                let start = if r == 0 { 0 } else { ends[r - 1] as usize };
                let len = ends[r] as usize - start;
                for _ in 0..len {
                    b.push(run_values[r]);
                }
            }
            let ree = b.finish();
            drop(b);
            // Shape observation (never an assertion): a built array that is
            // not exactly 100 runs has drifted, and its timing row would
            // measure a different shape — counted and printed as an anomaly.
            if ree.num_runs() != RUNS {
                per_class[cid].errors += 1;
            }
            let t = time_min_ns(|| {
                let mut acc: u64 = 0;
                for i in 0..ROWS {
                    acc = acc.wrapping_add(black_box(ree.get(i)) as u64);
                }
                black_box(acc);
            });
            per_class[cid].samples.push(t);
        }
    }
    report("RunEndEncoded::get", 5, &per_class);
}

/// Kernel 6: `dictionary::decode_page` — decode + in-place gather through a
/// 256-entry dictionary; index stream per class (byte domain = dict domain).
fn bench_dict_decode_page() {
    // Distinct-value dictionary (Knuth multiplicative permutation: odd
    // multiplier mod 2^32 is a bijection, so all 256 entries are distinct).
    let dict: Vec<u32> = (0..DICT as u32)
        .map(|j| j.wrapping_mul(2_654_435_761))
        .collect();
    let mut indices = vec![0u32; ROWS];
    let mut out = vec![0u32; ROWS];
    let mut per_class: Vec<ClassStat> = CLASSES
        .iter()
        .map(|_| ClassStat { wire_bytes: 0, errors: 0, samples: Vec::with_capacity(K) })
        .collect();
    for (cid, _) in CLASSES.iter().enumerate() {
        let mut wire_sum = 0u64;
        let mut errors = 0usize;
        for k in 0..K {
            let seed = instance_seed(5, cid, k);
            fill_u32(&mut indices, cid, seed); // byte domain == dict domain
            let page = rle_encode(&indices, 8);
            wire_sum += page.len() as u64;
            let t = time_min_ns(|| {
                match decode_page(black_box(&page), black_box(&dict), black_box(&mut out)) {
                    Ok(()) => black_box(0u8),
                    Err(_) => black_box(1u8),
                };
                consume_u32(&out);
            });
            per_class[cid].samples.push(t);
            if decode_page(&page, &dict, &mut out).is_err() {
                errors += 1;
            }
        }
        per_class[cid].wire_bytes = wire_sum / K as u64;
        per_class[cid].errors = errors;
    }
    report("dictionary::decode_page", 6, &per_class);
}

fn main() {
    println!(
        "avec const-time data-independence instrument — bench v26.10.29, lane PCL8"
    );
    println!(
        "rows={ROWS} (identical for every kernel x class x instance), \
         K={K} instances/class, min-of-{REPS} per instance, seeds splitmix64 \
         (kernel,class,instance)"
    );
    println!("pure instrument: NO pass/fail — every number below is a measurement");
    bench_mask_words();
    bench_decode_into();
    bench_encode_fast();
    bench_rle_decode();
    bench_ree_get();
    bench_dict_decode_page();
}
