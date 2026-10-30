//! Byte-aligned byteslice transposition + SWAR equality scan (lane HWL2).
//!
//! Layout: `k`-bit values are transposed into `ceil(k/8)` BYTE-ALIGNED
//! sub-columns, column-major — sub-column `j` is one `u8` per row holding
//! bits `[8j, 8j+8)` of the (k-masked) value; the last sub-column keeps only
//! `k - 8*(ceil(k/8)-1)` bits, zero-extended. Two shapes cover the family:
//! an 8-bit primary sub-column (k <= 8 collapses to it) and a remainder
//! secondary (k <= 16).
//!
//! Why this layout exists: the byte-aligned transpose buys O(1) random
//! access — [`point_get`] is `ceil(k/8)` array reads with no bit addressing
//! — at the cost of the transpose pass [`pack_byteslice`] performs once.
//! The essay's "<0.5 cycles/value point lookup" figure is CITED-CONTEXT
//! from the byteslice literature, not a claim of this crate; the bench
//! (`crates/bench/src/bin/byteslice_bench.rs`) prints the measured number.
//!
//! Scan reuse, not a new trick: the per-word compare math is exactly the
//! `w = 8` arm of [`crate::packed_filter::filter_eq_packed`]
//! (packed_filter.rs module docs, lanes 0..8 of a u64), restated below in
//! [`eq_mask_word`] because that kernel's helpers are private and operate
//! on tight-packed lanes. The secondary sub-column is the SAME kernel over
//! the needle's high byte; a row matches iff every sub-column matches, so
//! the full mask is the AND of per-sub-column masks.
//!
//! Byte rules (same shape as the house kernels):
//! 1. Row count is implied: `packed[0].len()` rows; every sub-column must
//!    have that length — ragged input fails LOUD ("ragged sub-columns").
//! 2. `out_valid` word `i` covers rows `[64i, 64i+64)`, LSB-first; bits of
//!    the final word past `n` are ZERO — zero-padded tail bytes can never
//!    leak phantom matches into the mask even when the needle is 0.
//! 3. `out_valid.len() >= n.div_ceil(64)` is a caller contract; a short
//!    buffer fails LOUD (assert, "mask buffer too small"). Words beyond the
//!    mask length are left untouched.
//! 4. Widths outside 1..=16 fail LOUD ("unsupported bit width") — the
//!    kernel family is the 8-bit primary + remainder secondary shape.
//!    `packed.len() != ceil(k/8)` also fails LOUD ("sub-column count").
//! 5. Values are masked to `k` bits at pack time (Parquet packing
//!    semantics, same as [`crate::packing::pack_bits`]); the needle is
//!    masked to `k` bits at scan time (same as `filter_eq_packed`).
//!
//! Charter v26.10.29, lane HWL2 (no_std, zero dependencies).

use alloc::vec::Vec;

/// Number of byte-aligned sub-columns for `k`-bit values: `ceil(k/8)`
/// (1 for k <= 8, 2 for 9..=16).
#[must_use]
pub fn byteslice_columns(k: u8) -> usize {
    usize::from(k.div_ceil(8))
}

/// Transpose `values` into byte-aligned sub-columns (column-major).
///
/// Returns `ceil(k/8)` sub-columns of `values.len()` bytes each; sub-column
/// `j` byte `i` is bits `[8j, 8j+8)` of `values[i] & (2^k - 1)`, with the
/// top sub-column zero-extended (see rule 5). Panics LOUDLY on a width
/// outside 1..=16 ("unsupported bit width").
#[must_use]
pub fn pack_byteslice(values: &[u32], k: u8) -> Vec<Vec<u8>> {
    assert!(
        (1..=16).contains(&k),
        "byteslice: unsupported bit width {k} \
         (kernel family is the 8-bit primary + remainder secondary shape, 1..=16)"
    );
    let cols = byteslice_columns(k);
    let n = values.len();
    let kmask = if k == 16 { 0xFFFF_u32 } else { (1u32 << k) - 1 };
    let mut packed = alloc::vec![alloc::vec![0u8; n]; cols];
    for j in 0..cols {
        let shift = 8 * j;
        let col = &mut packed[j];
        for (i, &v) in values.iter().enumerate() {
            col[i] = ((v & kmask) >> shift) as u8;
        }
    }
    packed
}

/// Equality scan over byte-aligned sub-columns: bit `i` of `out_valid` is
/// set iff row `i` equals `needle`.
///
/// The needle is masked to `k` bits first (rule 5). Per sub-column the scan
/// reuses the `w = 8` SWAR byte-compare of `crate::packed_filter` (see
/// [`eq_mask_word`]); masks are ANDed across sub-columns and the final
/// word's tail bits past `n` are zeroed (rule 2). Panics LOUDLY on the rule
/// 1/3/4 violations ("ragged sub-columns", "mask buffer too small",
/// "unsupported bit width", "sub-column count"); excess mask words are left
/// untouched.
pub fn scan_eq_byteslice(packed: &[Vec<u8>], k: u8, needle: u32, out_valid: &mut [u64]) {
    assert!(
        (1..=16).contains(&k),
        "byteslice: unsupported bit width {k} \
         (kernel family is the 8-bit primary + remainder secondary shape, 1..=16)"
    );
    let cols = byteslice_columns(k);
    assert!(
        packed.len() == cols,
        "byteslice: sub-column count mismatch: need {cols} for k={k}, got {}",
        packed.len()
    );
    let n = packed[0].len();
    assert!(
        packed.iter().all(|c| c.len() == n),
        "byteslice: ragged sub-columns: expected {n} bytes per sub-column"
    );
    let words = n.div_ceil(64);
    assert!(
        out_valid.len() >= words,
        "byteslice: mask buffer too small: need {words} words for {n} rows, got {}",
        out_valid.len()
    );
    if words == 0 {
        return;
    }

    let nmask = needle & kmask_of(k);
    for (j, col) in packed.iter().enumerate() {
        // Needle byte for this sub-column; both sides are zero-extended to
        // the sub-column width, so full-byte equality is exact (no padding
        // contamination: the top sub-column's high bits are ZERO by rule 5).
        let nb = u64::from((nmask >> (8 * j)) as u8);
        for (oi, group) in col.chunks(64).enumerate() {
            // 64 rows per output word, 8 bytes per SWAR partial (8-bit
            // partial mask at bit 8*jj — same partial layout as
            // filter_eq_packed's w=8 arm).
            let mut word = 0u64;
            for (jj, chunk) in group.chunks(8).enumerate() {
                let bytes = load_padded(chunk);
                word |= eq_mask_word(bytes, nb) << (8 * jj);
            }
            if j == 0 {
                out_valid[oi] = word;
            } else {
                out_valid[oi] &= word;
            }
        }
    }

    let tail = n % 64;
    if tail != 0 {
        let last = words - 1;
        out_valid[last] &= (1u64 << tail) - 1;
    }
}

/// O(1) point read of row `i`: `ceil(k/8)` contiguous byte loads, no bit
/// addressing — the layout's reason to exist.
///
/// Returns bits `[8j, 8j+8)` OR'd per sub-column; the top sub-column is
/// zero-extended at pack time (rule 5), so no re-masking happens here.
/// Panics LOUDLY on a width outside 1..=16, a sub-column count mismatch, or
/// a row index past `packed[0].len()` ("row out of range").
#[must_use]
pub fn point_get(packed: &[Vec<u8>], k: u8, i: usize) -> u32 {
    assert!(
        (1..=16).contains(&k),
        "byteslice: unsupported bit width {k} \
         (kernel family is the 8-bit primary + remainder secondary shape, 1..=16)"
    );
    let cols = byteslice_columns(k);
    assert!(
        packed.len() == cols,
        "byteslice: sub-column count mismatch: need {cols} for k={k}, got {}",
        packed.len()
    );
    assert!(
        i < packed[0].len(),
        "byteslice: row {i} out of range for {} rows",
        packed[0].len()
    );
    let mut v = 0u32;
    for (j, col) in packed.iter().enumerate() {
        v |= u32::from(col[i]) << (8 * j);
    }
    v
}

/// `k`-bit value mask; `k == 16` is special-cased so the shift stays
/// inside the u32 range (mirrors the LCG `value` mask shape).
#[inline]
#[must_use]
fn kmask_of(k: u8) -> u32 {
    if k == 16 { 0xFFFF_u32 } else { (1u32 << k) - 1 }
}

/// REUSED MATH, not a new trick — the `w = 8` arm of
/// `crate::packed_filter::filter_eq_packed` (packed_filter.rs lines 108-124
/// and module docs lines 10-24), restated because that kernel's helpers are
/// private and operate on tight-packed lanes. Identical per-lane
/// derivation, lane = one byte here:
/// - broadcast: `nb * 0x0101..01` (packed_filter `broadcast_stride(8)`)
/// - `x = word ^ broadcast`: zero iff the byte matches
/// - nonzero flag at the byte MSB: `(x | ((x & HALF) + HALF)) & MSB`,
///   `HALF = 0x7F7F..`, `MSB = 0x8080..` (packed_filter `half_mask` /
///   `msb_mask(8)`); no carry can cross a byte: `(x & HALF) + HALF <= 0xFE`
/// - gather: `(flags * 0x0102_0408_1020_4080) >> 56` (packed_filter
///   `gather(8)`) — distinct in-window term exponents `8i + 7j + 7` collide
///   only when `7 | (i - i')`, impossible for bytes 0..8.
///
/// Bit `b` of the result is set iff byte `b` of `bytes` equals `nb`.
#[inline]
fn eq_mask_word(bytes: u64, nb: u64) -> u64 {
    let bcast = nb.wrapping_mul(0x0101_0101_0101_0101);
    let x = bytes ^ bcast;
    let half = 0x7F7F_7F7F_7F7F_7F7F;
    let msb = 0x8080_8080_8080_8080;
    let nz_msb = (x | ((x & half) + half)) & msb;
    let flags = (!nz_msb & msb) >> 7;
    flags.wrapping_mul(0x0102_0408_1020_4080) >> 56
}

/// Load a (possibly short) byte chunk into a u64, zero-padding the high
/// bytes — bit positions preserved, never an overread. Restated from
/// `crate::packed_filter::load_padded` (private there).
#[inline]
fn load_padded(chunk: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf[..chunk.len()].copy_from_slice(chunk);
    u64::from_le_bytes(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packed_filter::{filter_eq_packed, packed_filter_words};
    use alloc::vec;

    /// Naive LSB-first tight packer for test inputs only (same shape as the
    /// packed_filter.rs test packer): builds the tight-packed stream the
    /// `filter_eq_packed` differential pairs against, WITHOUT depending on
    /// `crate::packing` so this file compiles and tests standalone while
    /// lib.rs is mid-flight frozen (prior lanes' pattern).
    fn pack_bits_naive(values: &[u32], w: u8) -> Vec<u8> {
        let mut out = vec![0u8; (values.len() * usize::from(w)).div_ceil(8)];
        for (i, &v) in values.iter().enumerate() {
            let bit = i * usize::from(w);
            let nmask = v & kmask_of(w);
            for b in 0..usize::from(w) {
                if (nmask >> b) & 1 == 1 {
                    out[(bit + b) / 8] |= 1 << ((bit + b) % 8);
                }
            }
        }
        out
    }

    /// Deterministic LCG (same shape as the packed_filter test vectors).
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
            let mask = if k == 16 {
                0xFFFFu64
            } else {
                (1u64 << k) - 1
            };
            (self.next() & mask) as u32
        }
    }

    /// Independent reference scan: one row at a time through
    /// [`point_get`] (the O(1) read path, sharing no code with the SWAR
    /// scan) + a naive compare. Third-party agreement by construction.
    fn materialize_then_filter(packed: &[Vec<u8>], k: u8, nmask: u32) -> Vec<u64> {
        let n = packed[0].len();
        let mut out = vec![0u64; n.div_ceil(64)];
        for i in 0..n {
            if point_get(packed, k, i) == nmask {
                out[i / 64] |= 1u64 << (i % 64);
            }
        }
        out
    }

    /// Cell of the differential grid: byteslice scan == materialize-then-
    /// filter (always), == `filter_eq_packed` on the same values tight-
    /// packed (k in {2,4,8}; packed_filter's kernel family is power-of-two
    /// widths 1..=8, so k=12 has no packed-filter pairing — there the
    /// primary-column scan is checked as a SUPERSET of the full mask
    /// instead: a full match implies its low byte matches).
    fn run_case(values: &[u32], k: u8, needle: u32) {
        let n = values.len();
        let packed = pack_byteslice(values, k);
        assert_eq!(packed.len(), byteslice_columns(k), "k={k}: column count");
        assert!(
            packed.iter().all(|c| c.len() == n),
            "k={k} n={n}: sub-column lengths"
        );
        let nmask = needle & kmask_of(k);
        let words = n.div_ceil(64);

        let mut got = vec![0u64; words];
        scan_eq_byteslice(&packed, k, needle, &mut got);
        let want = materialize_then_filter(&packed, k, nmask);
        assert_eq!(got, want, "k={k} needle={nmask} n={n}: vs materialize");

        if matches!(k, 2 | 4 | 8) {
            let tight = pack_bits_naive(values, k);
            assert_eq!(tight.len(), (n * usize::from(k)).div_ceil(8));
            let mut pf = vec![0u64; packed_filter_words(tight.len(), k)];
            filter_eq_packed(&tight, k, nmask, &mut pf);
            // Tight packing is BYTE-granular: when n*k is not a multiple of
            // 8, the stream's zero padding forms complete lanes BEYOND the
            // logical row count (1 value at k=2 fills one byte = 4 lanes),
            // and filter_eq_packed honestly reports those lanes. The shared
            // logical row space is the first n (tight lane i IS value i,
            // LSB-first), so the comparison truncates to it — byteslice
            // emits exactly n bits.
            let tail = n % 64;
            if tail != 0 {
                let last = words - 1;
                pf[last] &= (1u64 << tail) - 1;
            }
            assert_eq!(got, pf, "k={k} needle={nmask} n={n}: vs packed_filter");
        } else {
            // k=12: no packed-filter kernel at that width; the primary
            // (low-byte) scan must SUPERSET the full mask.
            let mut low = vec![0u64; packed_filter_words(n, 8)];
            filter_eq_packed(&packed[0], 8, nmask & 0xFF, &mut low);
            for (w, (&g, &l)) in got.iter().zip(low.iter()).enumerate() {
                assert_eq!(
                    g & !l,
                    0,
                    "k={k} word {w}: full mask not a subset of low-byte mask"
                );
            }
        }
    }

    /// Absent needle: least in-domain value with zero occurrences; `None`
    /// when the vector covers the full `2^k` domain — then a gap is forced
    /// on a copy (same protocol as the packed_filter grid test).
    fn absent_needle(values: &[u32], k: u8) -> Option<u32> {
        let max = (1u32 << k) - 1;
        let mut present = [false; 65536];
        let mut any = false;
        for &v in values {
            present[v as usize] = true;
            any = true;
        }
        if !any {
            return Some(0);
        }
        (0..=max).find(|&v| !present[v as usize])
    }

    #[test]
    fn known_answers() {
        // k=8: bytes [3, 1, 4, 1, 5, 9, 2, 6], needle=1 -> bits 1, 3.
        let values = [3u32, 1, 4, 1, 5, 9, 2, 6];
        let packed = pack_byteslice(&values, 8);
        assert_eq!(packed.len(), 1);
        assert_eq!(packed[0], [3u8, 1, 4, 1, 5, 9, 2, 6]);
        let mut m = [0u64; 1];
        scan_eq_byteslice(&packed, 8, 1, &mut m);
        assert_eq!(m[0], (1u64 << 1) | (1u64 << 3));
        // point_get reads the same layout.
        assert_eq!(point_get(&packed, 8, 0), 3);
        assert_eq!(point_get(&packed, 8, 5), 9);

        // k=2: values [0, 3, 2, 1] -> bytes [0, 3, 2, 1]; needle=2 -> bit 2.
        let values2 = [0u32, 3, 2, 1];
        let packed2 = pack_byteslice(&values2, 2);
        assert_eq!(packed2.len(), 1);
        assert_eq!(packed2[0], [0u8, 3, 2, 1]);
        scan_eq_byteslice(&packed2, 2, 2, &mut m);
        assert_eq!(m[0], 1 << 2);
        assert_eq!(point_get(&packed2, 2, 1), 3);

        // k=12: v=0xABC -> primary 0xBC, secondary 0x0A; v=0xFFF -> 0xFF,
        // 0x0F. needle=0xABC -> bit 0.
        let values12 = [0xABCu32, 0xFFF, 0x001, 0x000];
        let packed12 = pack_byteslice(&values12, 12);
        assert_eq!(packed12.len(), 2);
        assert_eq!(packed12[0], [0xBC, 0xFF, 0x01, 0x00]);
        assert_eq!(packed12[1], [0x0A, 0x0F, 0x00, 0x00]);
        scan_eq_byteslice(&packed12, 12, 0xABC, &mut m);
        assert_eq!(m[0], 1);
        assert_eq!(point_get(&packed12, 12, 1), 0xFFF);

        // Width mask at pack time (rule 5): 0x1F at k=4 stores 0x0F.
        let packed4 = pack_byteslice(&[0x1Fu32], 4);
        assert_eq!(packed4[0][0], 0x0F);
    }

    #[test]
    fn differential_grid() {
        let sizes = [0usize, 1, 63, 64, 1000, 1_000_000];
        for k in [2u8, 4, 8, 12] {
            for &n in &sizes {
                let mut rng = Lcg(0x5EED_0000 + u64::from(k) * 7919 + n as u64);
                let values: Vec<u32> = (0..n).map(|_| rng.value(k)).collect();
                assert_eq!(
                    pack_byteslice(&values, k)[0].len(),
                    n,
                    "k={k}: one byte per row"
                );

                let max = kmask_of(k);
                let mut needles = vec![0, max];
                if n > 0 {
                    needles.push(values[0]); // first
                    needles.push(values[n - 1]); // last
                }
                let mut forced_absent: Option<(Vec<u32>, u32)> = None;
                match absent_needle(&values, k) {
                    Some(absent) => needles.push(absent),
                    None => {
                        let victim = values[0];
                        let repl = (victim + 1) & max;
                        let mut forced = values.clone();
                        for v in forced.iter_mut() {
                            if *v == victim {
                                *v = repl;
                            }
                        }
                        forced_absent = Some((forced, victim));
                    }
                }

                for needle in needles {
                    run_case(&values, k, needle);
                }
                if let Some((forced, victim)) = forced_absent {
                    run_case(&forced, k, victim);
                }
            }
        }
    }

    #[test]
    fn tail_bits_are_zeroed() {
        // 63 rows at k=8: one mask word, bit 63 must stay zero even for a
        // needle the zero-padded tail bytes would satisfy (needle 0: bytes
        // 64.. of the column are padding, and rule 2 kills them).
        let mut values: Vec<u32> = vec![9; 63];
        values[0] = 7;
        let packed = pack_byteslice(&values, 8);
        let mut m = [u64::MAX; 1];
        scan_eq_byteslice(&packed, 8, 9, &mut m);
        // bits 1..62: bit 63 is padding (rule 2), bit 0 is the value-7 row.
        assert_eq!(m[0], ((1u64 << 63) - 1) & !1);
        scan_eq_byteslice(&packed, 8, 0, &mut m);
        assert_eq!(m[0], 0, "needle 0 must not light padding rows");
    }

    #[test]
    fn excess_mask_words_untouched() {
        let packed = pack_byteslice(&[7u32; 64], 8); // 64 rows -> 1 word
        let mut m = [0xDEAD_BEEF_CAFEu64; 3];
        scan_eq_byteslice(&packed, 8, 7, &mut m);
        assert_eq!(m[0], u64::MAX);
        assert_eq!(m[1], 0xDEAD_BEEF_CAFE, "excess word must be untouched");
        assert_eq!(m[2], 0xDEAD_BEEF_CAFE, "excess word must be untouched");
    }

    #[test]
    fn empty_input_writes_nothing() {
        let packed = pack_byteslice(&[], 8);
        assert_eq!(packed, vec![Vec::new()]);
        let mut m: [u64; 0] = [];
        scan_eq_byteslice(&packed, 8, 5, &mut m); // must not panic
        scan_eq_byteslice(&pack_byteslice(&[], 12), 12, 0, &mut m);
    }

    #[test]
    fn point_get_roundtrip_and_secondary() {
        // k=12 round-trip over a small grid incl. secondary-column only
        // differences: values sharing a primary byte but differing high.
        let values = [0x123u32, 0x1A3, 0x0FF, 0xFFF, 0x000, 0x999];
        let packed = pack_byteslice(&values, 12);
        for (i, &v) in values.iter().enumerate() {
            assert_eq!(point_get(&packed, 12, i), v, "row {i}");
        }
    }

    #[test]
    fn needle_masked_to_width() {
        // 0x1FFF at k=12 is 0xFFF after the width mask — masks agree.
        let packed = pack_byteslice(&[0xFFFu32; 32], 12);
        let mut a = [0u64; 1];
        let mut b = [0u64; 1];
        scan_eq_byteslice(&packed, 12, 0x1FFF, &mut a);
        scan_eq_byteslice(&packed, 12, 0x0FFF, &mut b);
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic(expected = "unsupported bit width")]
    fn width_0_refused() {
        let _ = pack_byteslice(&[0u32], 0);
    }

    #[test]
    #[should_panic(expected = "unsupported bit width")]
    fn width_17_refused() {
        let packed = pack_byteslice(&[0u32], 8);
        let mut m = [0u64; 1];
        scan_eq_byteslice(&packed, 17, 0, &mut m);
    }

    #[test]
    #[should_panic(expected = "sub-column count mismatch")]
    fn wrong_column_count_refused() {
        let packed = pack_byteslice(&[0xABCu32], 12);
        scan_eq_byteslice(&packed[..1], 12, 0, &mut [0u64; 1]);
    }

    #[test]
    #[should_panic(expected = "ragged sub-columns")]
    fn ragged_columns_refused() {
        let mut packed = pack_byteslice(&[0xABCu32; 3], 12);
        packed[1].pop();
        scan_eq_byteslice(&packed, 12, 0, &mut [0u64; 1]);
    }

    #[test]
    #[should_panic(expected = "mask buffer too small")]
    fn short_mask_loud() {
        let packed = pack_byteslice(&[7u32; 64], 8);
        scan_eq_byteslice(&packed, 8, 7, &mut [0u64; 0]);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn point_get_oob_loud() {
        let packed = pack_byteslice(&[7u32; 4], 8);
        let _ = point_get(&packed, 8, 4);
    }

    #[test]
    fn helper_arithmetic() {
        assert_eq!(byteslice_columns(1), 1);
        assert_eq!(byteslice_columns(8), 1);
        assert_eq!(byteslice_columns(9), 2);
        assert_eq!(byteslice_columns(12), 2);
        assert_eq!(byteslice_columns(16), 2);
        assert_eq!(kmask_of(12), 0xFFF);
        assert_eq!(kmask_of(16), 0xFFFF);
    }
}
