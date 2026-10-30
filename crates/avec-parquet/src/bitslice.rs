//! Vertical bit-slice layout: pack + MSB-first pruned range scan.
//!
//! v26.10.29, lane HWL1. The layout follows the bit-sliced ("vertical")
//! storage idea popularized by BitWeaving (Li & Patel, SIGMOD 2013); the
//! paper's published speedups are cited context only — this module claims
//! exactly what the differential tests and `bench/src/bin/bitslice_bench.rs`
//! measure on this machine, nothing more.
//!
//! # Layout
//!
//! `pack_vertical(values, w)` packs `values` into groups of 64 tuples. Each
//! group occupies `w` consecutive `u64` words: word `j` of a group holds bit
//! `j` of every tuple in the group, tuple `t` at bit position `t` (LSB word
//! first: word index 0 is the least-significant bit). Bit `t` of word
//! `g*w + j` is tuple `64*g + t`'s bit `j`. A group of 64 w-bit tuples costs
//! `w * 8` bytes regardless of `w`. The final partial group is zero-padded:
//! missing tuples read as value 0, and the scan evaluates them like any
//! other lane (they qualify iff `lo == 0`).
//!
//! ```text
//! values: v0..v63 | v64..v127 | ...
//! group g: [bit0 word][bit1 word] ... [bit w-1 word]
//!   bit t of word j of group g == (v_(64g+t) >> j) & 1
//! ```
//!
//! # Pruning rule (precise)
//!
//! [`scan_range_vertical`] evaluates `lo <= v <= hi` one 64-lane group at a
//! time, walking the group's words from the MSB word (`j = w-1`) down to the
//! LSB word (`j = 0`) and carrying four per-group lane masks:
//!
//! - `hi_gt` / `hi_lt`: lanes whose value prefix (the bits scanned so far,
//!   MSB side) is already strictly greater / strictly less than `hi`'s
//!   prefix at those levels.
//! - `lo_gt` / `lo_lt`: the same for `lo`.
//!
//! A lane is "decided" on a bound once its prefix diverges from the bound's
//! prefix. Decided lanes never return to the undecided state: the
//! still-equal masks `eq_hi = !(hi_gt | hi_lt)` and `eq_lo = !(lo_gt |
//! lo_lt)` only ever lose bits (monotone non-increasing). The pruning
//! cascade is:
//!
//! 1. **Bound-update skip.** At word `j`, only lanes with `eq_b` set can
//!    change bound `b`'s state; a lane with `eq_b == 0` is masked out of the
//!    update entirely. When `eq_b == 0` for the whole group, bound `b`'s
//!    update at this level is skipped (its outcome is already fixed).
//! 2. **Word skip.** Word `j` is loaded from memory iff
//!    `eq_hi | eq_lo != 0`. When both eq masks are empty every lane is
//!    decided on both bounds, so no bit of this word — or of any lower word,
//!    since the eq masks cannot regain bits — can change the outcome. The
//!    scan breaks out of the level loop and never touches the remaining
//!    lower words of the group. ([`scan_range_vertical_counted`] returns the
//!    number of words actually loaded, so callers can measure this.)
//! 3. **Final mask.** `out_valid[g] = !(hi_gt | lo_lt)`: a lane fails iff it
//!    ran strictly past `hi` or strictly under `lo` at some scanned level.
//!    Lanes equal to a bound's full prefix qualify (equality is in-range).
//!
//! Bounds are u32 but values are w-bit (`w <= 32`), so out-of-domain bounds
//! are resolved up front: `lo > 2^w - 1` empties the result (no value can
//! reach it), `hi > 2^w - 1` cannot disqualify anything and is clamped.
//! An in-domain inverted range (`lo > hi`) is empty by the mask algebra and
//! needs no special case.

use alloc::vec::Vec;

/// Pack `values` into the vertical bit-slice layout: `w` output words per
/// 64-tuple group, word `j` holding bit `j` of all tuples in the group (LSB
/// word first), tuple `t` at bit position `t`.
///
/// Contract:
/// - `w` must be in `1..=32`; anything else is a loud assert.
/// - Values are truncated to their low `w` bits; in debug builds a value
///   outside the w-bit domain also fires a debug assert, so generators
///   cannot silently widen the domain.
/// - Output length is exactly `ceil(values.len() / 64) * w` words; a partial
///   final group is zero-padded (missing tuples read as value 0).
pub fn pack_vertical(values: &[u32], w: u8) -> Vec<u64> {
    assert!(
        (1..=32).contains(&w),
        "pack_vertical: bit width w={w} outside 1..=32"
    );
    let wf = w as usize;
    let groups = values.len().div_ceil(64);
    let mut out = alloc::vec![0u64; groups * wf];
    for (g, chunk) in values.chunks(64).enumerate() {
        let base = g * wf;
        for (t, &v) in chunk.iter().enumerate() {
            debug_assert!(
                (v as u64) >> w == 0,
                "pack_vertical: value {v} does not fit in w={w} bits"
            );
            let v = v & mask_of(w);
            if v == 0 {
                continue;
            }
            let mut bit = 0usize;
            let mut v = v;
            while v != 0 {
                let bit_val = (v & 1) as u64;
                out[base + bit] |= bit_val << t;
                v >>= 1;
                bit += 1;
            }
        }
    }
    out
}

/// Scan a vertically packed column (`[`pack_vertical`]` output) for
/// `lo <= v <= hi`, writing one mask word per 64-tuple group into
/// `out_valid`: bit `t` of `out_valid[g]` set means tuple `64*g + t`
/// qualifies.
///
/// Contract:
/// - `w` must be in `1..=32` and `src.len()` must be a multiple of `w`;
///   anything else is a loud assert.
/// - Exactly `src.len() / w` words are written. `out_valid` may be longer
///   (the extra words are left untouched) and must not be shorter.
/// - Padding lanes of a partial final group read as value 0 and qualify iff
///   `lo == 0`; callers needing an exact logical row count must clear the
///   trailing lanes of the last mask word themselves.
///
/// This is the un-instrumented entry point; see
/// [`scan_range_vertical_counted`] for the same kernel returning the
/// word-touch count that measures pruning.
#[inline]
pub fn scan_range_vertical(src: &[u64], w: u8, lo: u32, hi: u32, out_valid: &mut [u64]) {
    let _ = scan_range_vertical_counted(src, w, lo, hi, out_valid);
}

/// The [`scan_range_vertical`] kernel, instrumented: identical semantics,
/// but returns the number of packed words actually loaded across all groups.
/// `words_touched < groups * w` is exactly the pruning effect — the low
/// words that the MSB-first cascade decided without reading.
pub fn scan_range_vertical_counted(
    src: &[u64],
    w: u8,
    lo: u32,
    hi: u32,
    out_valid: &mut [u64],
) -> u64 {
    assert!(
        (1..=32).contains(&w),
        "scan_range_vertical: bit width w={w} outside 1..=32"
    );
    let wf = w as usize;
    assert!(
        src.len() % wf == 0,
        "scan_range_vertical: src.len()={} is not a multiple of w={wf}",
        src.len()
    );
    let groups = src.len() / wf;
    assert!(
        out_valid.len() >= groups,
        "scan_range_vertical: out_valid.len()={} < {groups} groups",
        out_valid.len()
    );

    // Values are w-bit, so bounds above the domain never compare at any
    // scanned level. Resolve them up front: lo past the domain empties the
    // result; hi past the domain cannot disqualify and is clamped.
    let domain_max = (1u64 << w) - 1;
    if lo as u64 > domain_max {
        out_valid[..groups].fill(0);
        return 0;
    }
    let hi = (hi as u64).min(domain_max) as u32;

    let mut touched: u64 = 0;
    for (g, words) in src.chunks_exact(wf).enumerate() {
        let mut hi_gt: u64 = 0;
        let mut hi_lt: u64 = 0;
        let mut lo_gt: u64 = 0;
        let mut lo_lt: u64 = 0;
        // MSB word first; `level` is the actual bit index this word carries.
        for (lvl, &bits) in words.iter().rev().enumerate() {
            let level = wf - 1 - lvl;
            let eq_hi = !(hi_gt | hi_lt);
            let eq_lo = !(lo_gt | lo_lt);
            // Word skip: with every lane decided on both bounds, this word
            // and all lower words (eq masks are monotone) cannot change the
            // outcome — stop touching memory for this group.
            if (eq_hi | eq_lo) == 0 {
                break;
            }
            touched += 1;
            let hb = (((hi >> level) & 1) as u64).wrapping_neg();
            let lb = (((lo >> level) & 1) as u64).wrapping_neg();
            // Bound-update skip: decided lanes are masked out by eq_b; when
            // the whole group is decided on one bound, that update is a
            // no-op and is skipped.
            if eq_hi != 0 {
                hi_gt |= eq_hi & bits & !hb;
                hi_lt |= eq_hi & hb & !bits;
            }
            if eq_lo != 0 {
                lo_gt |= eq_lo & bits & !lb;
                lo_lt |= eq_lo & lb & !bits;
            }
        }
        out_valid[g] = !(hi_gt | lo_lt);
    }
    touched
}

#[inline]
fn mask_of(w: u8) -> u32 {
    ((1u64 << w) - 1) as u32
}

/// 128-lane dual-word vertical bit-slice range scanner.
///
/// Evaluates two consecutive 64-lane groups (128 tuples) in parallel per level,
/// doubling memory throughput and amortizing outer loop overhead while preserving
/// bitwise equality with [`scan_range_vertical`].
pub fn scan_range_vertical_128(
    src: &[u64],
    w: u8,
    lo: u32,
    hi: u32,
    out_valid: &mut [u64],
) {
    assert!((1..=32).contains(&w), "scan_range_vertical_128: w={w} outside 1..=32");
    let wf = w as usize;
    assert!(src.len() % wf == 0, "src length must be multiple of w");
    let groups = src.len() / wf;
    assert!(out_valid.len() >= groups, "out_valid buffer too small");

    let domain_max = (1u64 << w) - 1;
    if lo as u64 > domain_max {
        out_valid[..groups].fill(0);
        return;
    }
    let hi = (hi as u64).min(domain_max) as u32;

    let mut g = 0;
    // Process pairs of 64-lane groups (128 tuples)
    while g + 1 < groups {
        let words0 = &src[g * wf..(g + 1) * wf];
        let words1 = &src[(g + 1) * wf..(g + 2) * wf];

        let mut hi_gt0 = 0u64;
        let mut hi_lt0 = 0u64;
        let mut lo_gt0 = 0u64;
        let mut lo_lt0 = 0u64;

        let mut hi_gt1 = 0u64;
        let mut hi_lt1 = 0u64;
        let mut lo_gt1 = 0u64;
        let mut lo_lt1 = 0u64;

        for (lvl, (&bits0, &bits1)) in words0.iter().rev().zip(words1.iter().rev()).enumerate() {
            let level = wf - 1 - lvl;
            let eq_hi0 = !(hi_gt0 | hi_lt0);
            let eq_lo0 = !(lo_gt0 | lo_lt0);
            let eq_hi1 = !(hi_gt1 | hi_lt1);
            let eq_lo1 = !(lo_gt1 | lo_lt1);

            if ((eq_hi0 | eq_lo0) | (eq_hi1 | eq_lo1)) == 0 {
                break;
            }

            let hb = (((hi >> level) & 1) as u64).wrapping_neg();
            let lb = (((lo >> level) & 1) as u64).wrapping_neg();

            hi_gt0 |= eq_hi0 & bits0 & !hb;
            hi_lt0 |= eq_hi0 & hb & !bits0;
            lo_gt0 |= eq_lo0 & bits0 & !lb;
            lo_lt0 |= eq_lo0 & lb & !bits0;

            hi_gt1 |= eq_hi1 & bits1 & !hb;
            hi_lt1 |= eq_hi1 & hb & !bits1;
            lo_gt1 |= eq_lo1 & bits1 & !lb;
            lo_lt1 |= eq_lo1 & lb & !bits1;
        }

        out_valid[g] = !(hi_gt0 | lo_lt0);
        out_valid[g + 1] = !(hi_gt1 | lo_lt1);
        g += 2;
    }

    // Scalar tail if odd number of groups
    if g < groups {
        let words = &src[g * wf..(g + 1) * wf];
        let mut hi_gt = 0u64;
        let mut hi_lt = 0u64;
        let mut lo_gt = 0u64;
        let mut lo_lt = 0u64;

        for (lvl, &bits) in words.iter().rev().enumerate() {
            let level = wf - 1 - lvl;
            let eq_hi = !(hi_gt | hi_lt);
            let eq_lo = !(lo_gt | lo_lt);
            if (eq_hi | eq_lo) == 0 {
                break;
            }
            let hb = (((hi >> level) & 1) as u64).wrapping_neg();
            let lb = (((lo >> level) & 1) as u64).wrapping_neg();
            hi_gt |= eq_hi & bits & !hb;
            hi_lt |= eq_hi & hb & !bits;
            lo_gt |= eq_lo & bits & !lb;
            lo_lt |= eq_lo & lb & !bits;
        }
        out_valid[g] = !(hi_gt | lo_lt);
    }
}

/// Compressed-domain algebraic filtered summation over 128-lane vertical bit-slices.
///
/// Computes the exact filtered sum of values across two 64-lane groups without unpacking
/// into an intermediate scalar representation:
///
/// $$\sum_{i \in M} v_i = (k \cdot b) + \sum_{j=0}^{w-1} 2^j \cdot \text{popcnt}(B_j \land M)$$
///
/// - `src`: Packed vertical words (groups * w * 8 bytes).
/// - `w`: Bit-width of tuples (1..=32).
/// - `base`: Base offset $b$ of the Frame-of-Reference (FFOR).
/// - `mask0`: Active filter mask for the first 64 lanes.
/// - `mask1`: Active filter mask for the second 64 lanes.
///
/// Operates strictly in vector registers using straight-line popcounts with 0 memory allocation.
#[inline(always)]
pub fn sum_masked_vertical_128(
    src: &[u64],
    w: u8,
    base: i64,
    mask0: u64,
    mask1: u64,
) -> i64 {
    let wf = w as usize;
    let k0 = mask0.count_ones() as i64;
    let k1 = mask1.count_ones() as i64;
    let mut delta_sum: i64 = 0;

    for j in 0..wf {
        let b0 = src[j] & mask0;
        let b1 = src[wf + j] & mask1;
        let c = (b0.count_ones() + b1.count_ones()) as i64;
        delta_sum += c << j;
    }

    ((k0 + k1) * base) + delta_sum
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_scan_range_vertical_128_differential() {
        for &w in &[1, 2, 4, 8, 12, 16] {
            let n = 256;
            let values = gen_values(n, w);
            let packed = pack_vertical(&values, w);
            let groups = packed.len() / w as usize;
            let mut out64 = alloc::vec![0u64; groups];
            let mut out128 = alloc::vec![0u64; groups];

            let lo = 1;
            let hi = (1 << (w.min(4))) - 1;
            scan_range_vertical(&packed, w, lo, hi, &mut out64);
            scan_range_vertical_128(&packed, w, lo, hi, &mut out128);

            assert_eq!(out64, out128, "w={w} differential parity failure between 64 and 128 scanner");
        }
    }

    #[test]
    fn test_sum_masked_vertical_128_algebraic_parity() {
        for &w in &[1, 2, 4, 8, 12] {
            let n = 128;
            let values = gen_values(n, w);
            let packed = pack_vertical(&values, w);
            let base = 100i64;

            let mask0 = 0x5555_5555_5555_5555u64; // alternating lanes
            let mask1 = 0x3333_3333_3333_3333u64;

            // Direct compressed-domain algebraic sum
            let alg_sum = sum_masked_vertical_128(&packed, w, base, mask0, mask1);

            // Ground truth: scalar materialize-and-sum
            let mut true_sum: i64 = 0;
            for i in 0..64 {
                if (mask0 >> i) & 1 == 1 {
                    true_sum += base + values[i] as i64;
                }
            }
            for i in 0..64 {
                if (mask1 >> i) & 1 == 1 {
                    true_sum += base + values[64 + i] as i64;
                }
            }

            assert_eq!(
                alg_sum, true_sum,
                "w={w} compressed-domain algebraic sum failed ground truth parity"
            );
        }
    }

    /// Materialize-then-filter reference over already-64-padded values.
    fn reference_range_mask(values: &[u32], lo: u32, hi: u32) -> Vec<u64> {
        let groups = values.len().div_ceil(64);
        let mut out = alloc::vec![0u64; groups];
        for (i, &v) in values.iter().enumerate() {
            if lo <= v && v <= hi {
                out[i / 64] |= 1u64 << (i % 64);
            }
        }
        out
    }

    fn class_bounds(w: u8, class: &str, point: u32) -> (u32, u32) {
        let m = mask_of(w);
        match class {
            "full" => (0, m),
            "empty" => (m, m - 1), // in-domain inverted: empty by the algebra
            "top-half" => (1 << (w - 1), m),
            "bottom-half" => (0, (1 << (w - 1)) - 1),
            "point" => (point, point),
            other => panic!("unknown class {other}"),
        }
    }

    const MATRIX_WIDTHS: [u8; 4] = [2, 4, 8, 12];
    const MATRIX_SIZES: [usize; 6] = [0, 1, 63, 64, 1000, 1_000_000];
    const MATRIX_CLASSES: [&str; 5] = ["full", "empty", "top-half", "bottom-half", "point"];

    /// Differential: scanned masks bitwise-equal the materialize-then-filter
    /// reference across the full width x size x class matrix.
    #[test]
    fn differential_matrix() {
        for &w in &MATRIX_WIDTHS {
            for &n in &MATRIX_SIZES {
                let values = gen_values(n, w);
                let packed = pack_vertical(&values, w);
                assert_eq!(packed.len(), ((n + 63) / 64) * w as usize);
                let groups = (n + 63) / 64;
                let point = if n > 0 { values[n / 2] } else { 0 };
                // pack zero-pads to a 64 multiple; the reference must see the
                // same padded lanes the scanner sees.
                let mut padded = values;
                padded.resize(groups * 64, 0);
                for class in MATRIX_CLASSES {
                    let (lo, hi) = class_bounds(w, class, point);
                    let expect = reference_range_mask(&padded, lo, hi);
                    let mut got = alloc::vec![0xA55A_5AA5_0F0F_F0F0u64; groups];
                    let touched = scan_range_vertical_counted(&packed, w, lo, hi, &mut got);
                    assert_eq!(
                        &got[..groups],
                        &expect[..],
                        "mask mismatch: w={w} n={n} class={class} lo={lo} hi={hi}"
                    );
                    if groups == 0 {
                        assert_eq!(touched, 0);
                    } else {
                        assert!(
                            touched >= 1 && touched <= (groups * w as usize) as u64,
                            "touch count: w={w} n={n} class={class} touched={touched}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn pack_roundtrip_materialize() {
        for &w in &MATRIX_WIDTHS {
            for &n in &[0, 1, 63, 64, 65, 128, 1000] {
                let values = gen_values(n, w);
                let packed = pack_vertical(&values, w);
                let mut padded = values;
                padded.resize(((n + 63) / 64) * 64, 0);
                let mut back = Vec::with_capacity(padded.len());
                for g in 0..((n + 63) / 64) {
                    for t in 0..64 {
                        let mut v = 0u32;
                        for j in 0..w as usize {
                            v |= (((packed[g * w as usize + j] >> t) & 1) as u32) << j;
                        }
                        back.push(v);
                    }
                }
                assert_eq!(back, padded, "pack roundtrip w={w} n={n}");
            }
        }
    }

    #[test]
    fn inverted_in_domain_range_is_empty() {
        let values = gen_values(4096, 8);
        let packed = pack_vertical(&values, 8);
        let mut got = alloc::vec![0xFFFF_FFFF_FFFF_FFFFu64; 64];
        scan_range_vertical(&packed, 8, 200, 100, &mut got);
        assert!(got.iter().all(|&x| x == 0), "lo>hi must scan empty");
    }

    #[test]
    fn out_of_domain_bounds() {
        let w = 8;
        let values = gen_values(500, w);
        let packed = pack_vertical(&values, w);
        // lo past the domain: empty, and the fast path touches no words.
        let mut got = alloc::vec![0u64; 8];
        let touched = scan_range_vertical_counted(&packed, w, 1 << w, u32::MAX, &mut got);
        assert!(got.iter().all(|&x| x == 0));
        assert_eq!(touched, 0, "empty fast path must not touch words");
        // hi past the domain: same mask as hi = domain max.
        let mut a = alloc::vec![0u64; 8];
        let mut b = alloc::vec![0u64; 8];
        scan_range_vertical(&packed, w, 0, u32::MAX, &mut a);
        scan_range_vertical(&packed, w, 0, mask_of(w), &mut b);
        assert_eq!(a, b, "hi above domain must clamp to domain max");
    }

    #[test]
    fn point_at_domain_boundaries() {
        let w = 8;
        let mut values = gen_values(1024, w);
        values[0] = 0;
        values[511] = mask_of(w);
        let packed = pack_vertical(&values, w);
        let mut padded = values;
        padded.resize(1024, 0);
        for point in [0u32, mask_of(w)] {
            let expect = reference_range_mask(&padded, point, point);
            let mut got = alloc::vec![0u64; 16];
            scan_range_vertical(&packed, w, point, point, &mut got);
            assert_eq!(got, expect, "point {point}");
        }
    }

    #[test]
    fn out_valid_tail_untouched() {
        let values = gen_values(64, 4);
        let packed = pack_vertical(&values, 4);
        let mut got = alloc::vec![0x1111_1111_2222_2222u64; 4];
        scan_range_vertical(&packed, 4, 0, mask_of(4), &mut got);
        assert_eq!(got[0], u64::MAX, "full range qualifies all 64 lanes");
        assert_eq!(
            &got[1..],
            &[0x1111_1111_2222_2222u64; 3],
            "words beyond groups must be left untouched"
        );
    }

    #[test]
    fn kernel_set_edges_w1_w32() {
        for &w in &[1u8, 32] {
            for &n in &[0, 1, 64, 257] {
                let values = gen_values(n, w);
                let packed = pack_vertical(&values, w);
                let mut padded = values;
                padded.resize(((n + 63) / 64) * 64, 0);
                let point = if n > 0 { padded[n / 2] } else { 0 };
                for class in MATRIX_CLASSES {
                    let (lo, hi) = class_bounds(w, class, point);
                    let expect = reference_range_mask(&padded, lo, hi);
                    let mut got = alloc::vec![0u64; ((n + 63) / 64).max(1)];
                    scan_range_vertical(&packed, w, lo, hi, &mut got);
                    got.truncate((n + 63) / 64);
                    assert_eq!(got, expect, "w={w} n={n} class={class}");
                }
            }
        }
    }

    /// Pruning must actually fire on real data: for w=12 random rows, the
    /// kernel must skip at least one word somewhere (the expected touched
    /// count is ~log2(64)+O(1) of 12, so a full-touch run means the cascade
    /// is broken).
    #[test]
    fn pruning_fires_on_random_data() {
        let w = 12;
        let values = gen_values(1_000_000, w);
        let packed = pack_vertical(&values, w);
        let groups = (values.len() + 63) / 64;
        let mut got = alloc::vec![0u64; groups];
        let touched = scan_range_vertical_counted(&packed, w, 0, mask_of(w), &mut got);
        assert!(
            touched < (groups * w as usize) as u64,
            "pruning dead: touched every word ({touched})"
        );
    }
}
