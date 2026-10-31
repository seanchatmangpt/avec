//! Equality filtering directly on tight packed `w`-bit lanes — SWAR
//! broadcast/compare/mask arithmetic that emits a validity bitmask WITHOUT
//! materializing unpacked lanes.
//!
//! Input is the real byte stream (LSB-first tight packing, the exact layout
//! [`crate::packing::pack_bits`] emits: lane `i` occupies bit range
//! `[i*w, (i+1)*w)`), read as little-endian u64 words — the same word
//! alignment as the SWAR unpack kernels, so bit positions are preserved.
//!
//! Lane math per word (exact, exhaustively derived, no borrow across lanes):
//! - `x = word ^ broadcast(needle)` — zero iff the lane matches.
//! - per-lane nonzero flag at the lane MSB:
//!   `nz = (x | ((x & HALF) + HALF)) & MSB`, where `HALF = 2^(w-1) - 1` per
//!   lane and `MSB = 2^(w-1)` per lane. `(x & HALF) + HALF <= 2^w - 2`, so a
//!   carry can never cross a lane boundary; `MSB` of the result is set iff
//!   `x_lane != 0` (bit 0 of the lane alone would miss `x_lane >= 2^(w-1)`,
//!   which the `| x` term restores). `w = 1` is its own kernel: the lane IS
//!   the flag, `word ^ broadcast` is the dense mask.
//! - gather: the per-lane flags (at lane LSBs after `>> (w-1)`) are
//!   compressed to one bit per lane. `w = 8` uses the exact multiply gather
//!   `(flags * 0x0102040810204080) >> 56` (distinct in-window term exponents:
//!   `8i + 7j + 7` collide only when `7 | (i - i')`, impossible for lanes
//!   `0..8`); `w` in {2, 4} use a fixed-trip independent-term gather
//!   `acc |= (flags >> (k*(w-1))) & (1 << k)`.
//!
//! Byte rules (same shape as the house kernels):
//! 1. Row count is implied: `packed_rows(src.len(), w)` complete lanes; the
//!    final partial lane of padding bits is never emitted.
//! 2. `out_valid` word `i` covers lanes `[64i, 64i+64)`, LSB-first; bit `j`
//!    set iff lane `64i+j == (needle masked to w bits)`. Bits of the final
//!    word past `packed_rows` are ZERO — padding bits can never leak into
//!    the mask even when the needle is 0.
//! 3. `out_valid.len() >= packed_filter_words(src.len(), w)` is a caller
//!    contract; a short buffer fails LOUD (assert, "mask buffer too small").
//!    Words beyond the mask length are left untouched.
//! 4. Widths outside {1, 2, 4, 8} fail LOUD ("unsupported bit width") — the
//!    kernel family is power-of-two widths in 1..=8.
//!
//! The needle is masked to `w` bits (Parquet packing semantics: the declared
//! bit width bounds the value domain, same as `pack_bits`).
//!
//! Charter v26.10.29, lane PCL3 (no_std, zero dependencies).

/// Complete `w`-bit lanes contained in `src_len` bytes.
#[must_use]
pub fn packed_rows(src_len: usize, w: u8) -> usize {
    src_len * 8 / usize::from(w)
}

/// Number of `u64` mask words `filter_eq_packed` writes for `src_len` bytes
/// at width `w`: `ceil(packed_rows(src_len, w) / 64)`.
#[must_use]
pub fn packed_filter_words(src_len: usize, w: u8) -> usize {
    packed_rows(src_len, w).div_ceil(64)
}

/// Load a (possibly short) byte chunk into a u64, zero-padding the high
/// bytes — bit positions preserved, never an overread.
#[inline]
fn load_padded(chunk: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf[..chunk.len()].copy_from_slice(chunk);
    u64::from_le_bytes(buf)
}

/// Equality filter over tight packed lanes: bit `i` of `out_valid` is set
/// iff lane `i` of the packed stream equals `needle`.
///
/// See the module docs for the full byte rules. Panics LOUDLY on a width
/// outside {1,2,4,8} ("unsupported bit width") or a mask buffer shorter
/// than [`packed_filter_words`] ("mask buffer too small"); excess mask
/// words are left untouched.
pub fn filter_eq_packed(src: &[u8], w: u8, needle: u32, out_valid: &mut [u64]) {
    assert!(
        matches!(w, 1 | 2 | 4 | 8),
        "packed_filter: unsupported bit width {w} (kernel family is power-of-two widths 1..=8)"
    );
    let words = packed_filter_words(src.len(), w);
    assert!(
        out_valid.len() >= words,
        "packed_filter: mask buffer too small: need {words} words for {} bytes at w={w}, got {}",
        src.len(),
        out_valid.len()
    );
    if words == 0 {
        return;
    }

    let wb = u64::from(w);
    let bytes_per_word = 8 * usize::from(w); // 64 lanes * w bits / 8
    let n = packed_rows(src.len(), w);
    let tail_bits = n % 64;

    for (oi, group) in src.chunks(bytes_per_word).enumerate() {
        // `w` u64 words per output word; partial j-th partial-mask sits at
        // bit (j * 64/w).
        let bits_per_partial = 64 / usize::from(w);
        let mut out_word: u64 = 0;
        for (j, chunk) in group.chunks(8).enumerate() {
            let word = load_padded(chunk);
            let partial = match w {
                1 => {
                    // Lane IS the flag: x = lane ^ needle is 1 iff the lane
                    // DIFFERS, so the match mask is the complement.
                    let bcast = if needle & 1 == 1 { u64::MAX } else { 0 };
                    !(word ^ bcast)
                }
                2 | 4 | 8 => {
                    let nmask = u64::from(needle) & ((1 << wb) - 1);
                    let bcast = nmask * broadcast_stride(wb);
                    let x = word ^ bcast;
                    let half = half_mask(wb);
                    let msb = msb_mask(wb);
                    // Exact per-lane nonzero flag at the lane MSB; no borrow
                    // can cross a lane: (x & HALF) + HALF <= 2^w - 2. The
                    // MATCH flag is the complement (x == 0 iff lane==needle).
                    let nz_msb = (x | ((x & half) + half)) & msb;
                    let flags = (!nz_msb & msb) >> (wb - 1); // match flags at lane LSBs
                    gather(flags, w, bits_per_partial)
                }
                _ => unreachable!("width gated by assert above"),
            };
            out_word |= partial << (j * bits_per_partial);
        }
        out_valid[oi] = out_word;
    }

    if tail_bits != 0 {
        let last = words - 1;
        out_valid[last] &= (1u64 << tail_bits) - 1;
    }
}

/// Per-lane stride constant for broadcasting `needle` across a word: needle
/// sits in lane 0, so multiplying by the lane-stride ones replicates it.
#[inline]
#[must_use]
fn broadcast_stride(wb: u64) -> u64 {
    match wb {
        2 => 0x5555_5555_5555_5555,
        4 => 0x1111_1111_1111_1111,
        8 => 0x0101_0101_0101_0101,
        _ => 1,
    }
}

#[inline]
#[must_use]
fn half_mask(wb: u64) -> u64 {
    match wb {
        2 => 0x5555_5555_5555_5555,
        4 => 0x7777_7777_7777_7777,
        _ => 0x7F7F_7F7F_7F7F_7F7F,
    }
}

#[inline]
#[must_use]
fn msb_mask(wb: u64) -> u64 {
    match wb {
        2 => 0xAAAA_AAAA_AAAA_AAAA,
        4 => 0x8888_8888_8888_8888,
        _ => 0x8080_8080_8080_8080,
    }
}

/// Compress per-lane flags (at lane LSBs, stride `wb`) to a dense
/// `bits_per_partial`-bit partial mask.
///
/// - `w = 8`: exact multiply gather, `(flags * 0x0102040810204080) >> 56`.
/// - `w in {2, 4}`: fixed-trip loop of independent terms
///   `acc |= (flags >> (k*(w-1))) & (1 << k)` — no loop-carried dependency.
#[inline]
#[must_use]
fn gather(flags: u64, w: u8, bits_per_partial: usize) -> u64 {
    match w {
        8 => (flags.wrapping_mul(0x0102_0408_1020_4080)) >> 56,
        4 => {
            let mut acc = 0u64;
            let mut k = 0usize;
            while k < bits_per_partial {
                acc |= (flags >> (3 * k)) & (1u64 << k);
                k += 1;
            }
            acc
        }
        2 => {
            let mut acc = 0u64;
            let mut k = 0usize;
            while k < bits_per_partial {
                acc |= (flags >> k) & (1u64 << k);
                k += 1;
            }
            acc
        }
        // w = 1 never gathers (already dense); any other width is gated by
        // the caller's assert.
        _ => flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Deterministic LCG (same shape as packing.rs test vectors).
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

    /// Naive independent packer for test inputs only: writes each lane's
    /// bits one at a time. Same documented tight layout as
    /// `crate::packing::pack_bits` (lane `i` at bit `[i*w, (i+1)*w)`,
    /// LSB-first, final byte's high bits zero) — the layout is pinned by the
    /// hand-computed byte vectors in `known_answers`, and the bench
    /// pre-flight binds this same format to the real `pack_bits` output.
    /// Kept local (instead of calling `crate::packing`) so the whole file
    /// compiles and tests standalone under `rustc --test` while lib.rs is
    /// mid-flight frozen.
    fn pack_bits_naive(values: &[u32], w: u8) -> Vec<u8> {
        let mut out = alloc::vec![0u8; (values.len() * usize::from(w)).div_ceil(8)];
        for (i, &v) in values.iter().enumerate() {
            let bit = i * usize::from(w);
            let nmask = v & ((1u32 << w) - 1);
            for b in 0..usize::from(w) {
                if (nmask >> b) & 1 == 1 {
                    out[(bit + b) / 8] |= 1 << ((bit + b) % 8);
                }
            }
        }
        out
    }

    /// Independent reference: naive LSB-first bit reader + per-lane compare,
    /// building the mask one lane at a time. Shares no code with the SWAR
    /// kernels (third-party agreement by construction).
    fn reference_filter(packed: &[u8], w: u8, needle: u32, out_valid: &mut [u64]) {
        let n = packed_rows(packed.len(), w);
        for word in out_valid.iter_mut() {
            *word = 0;
        }
        let nmask = needle & ((1u32 << w) - 1);
        for lane in 0..n {
            let bit = lane * usize::from(w);
            let byte = bit / 8;
            let off = bit % 8;
            // Read w bits LSB-first across the (at most 2-byte) boundary;
            // the high byte is absent at the end of the stream.
            let hi = if byte + 1 < packed.len() {
                u16::from(packed[byte + 1]) << 8
            } else {
                0
            };
            let acc = u16::from(packed[byte]) | hi;
            let v = ((acc >> off) & ((1u16 << w) - 1)) as u32;
            if v == nmask {
                out_valid[lane / 64] |= 1u64 << (lane % 64);
            }
        }
    }

    fn run_case(values: &[u32], w: u8, needle: u32) {
        let packed = pack_bits_naive(values, w);
        let words = packed_filter_words(packed.len(), w);
        let mut got = alloc::vec![0u64; words];
        filter_eq_packed(&packed, w, needle, &mut got);

        let mut want = alloc::vec![0xDEAD_BEEF_CAFEu64; words];
        reference_filter(&packed, w, needle, &mut want);

        assert_eq!(got, want, "w={w} needle={needle} packed={packed:?}");
    }

    /// Absent needle: least in-domain value with zero occurrences. `None`
    /// when the vector covers the full `2^w` domain — possible at any width
    /// once `n >= 2^w`; the grid then forces a gap on a copy instead.
    fn absent_needle(values: &[u32], w: u8) -> Option<u32> {
        let max = (1u32 << w) - 1;
        let mut present = [false; 256];
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
        // w=8: bytes [3, 1, 4, 1, 5, 9, 2, 6], needle=1 -> bits 1, 3.
        let bytes = [3u8, 1, 4, 1, 5, 9, 2, 6];
        let mut m = [0u64; 1];
        filter_eq_packed(&bytes, 8, 1, &mut m);
        assert_eq!(m[0], (1u64 << 1) | (1u64 << 3));

        // w=1: bits 1,1,0,1 in byte 0x0B, needle=1 -> bits 0,1,3.
        filter_eq_packed(&[0x0B], 1, 1, &mut m);
        assert_eq!(m[0], 0b1011);

        // w=1 needle=0: lanes 0..7 of 0x0B are 1,1,0,1,0,0,0,0 (a byte holds
        // eight 1-bit lanes) -> matches at 2,4,5,6,7.
        filter_eq_packed(&[0x0B], 1, 0, &mut m);
        assert_eq!(m[0], 0xF4);

        // w=2: lanes 0,1,2,3 in byte 0b11_10_01_00, needle=2 -> bit 2.
        filter_eq_packed(&[0b11_10_01_00u8], 2, 2, &mut m);
        assert_eq!(m[0], 1 << 2);

        // w=4: lanes 0xA,0xB,0xC,0xD in 0xDCBA, needle=0xC -> bit 2.
        filter_eq_packed(&[0xBA, 0xDC], 4, 0xC, &mut m);
        assert_eq!(m[0], 1 << 2);

        // w=4 needle=0: lanes A,0,0,0 in [0x0A, 0x00] -> bits 1,2,3. (Every
        // byte holds whole lanes for w in {1,2,4}; zero-padding exists only
        // beyond the stream, where the tail mask kills it — see the tail
        // test.)
        filter_eq_packed(&[0x0A, 0x00], 4, 0, &mut m);
        assert_eq!(m[0], 0b1110);
    }

    #[test]
    fn grid_widths_sizes_needles() {
        let sizes = [0usize, 1, 63, 64, 1000, 1_000_000];
        for w in [1u8, 2, 4, 8] {
            for &n in &sizes {
                let mut rng = Lcg(0x5EED_0000 + u64::from(w) * 7919 + n as u64);
                let values: Vec<u32> = (0..n).map(|_| rng.value(w)).collect();
                let packed = pack_bits_naive(&values, w);
                assert_eq!(
                    packed.len(),
                    (n * usize::from(w)).div_ceil(8),
                    "byte count w={w} n={n}"
                );

                let max = (1u32 << w) - 1;
                let mut needles = alloc::vec![0u32, max];
                if n > 0 {
                    needles.push(values[0]); // first
                    needles.push(values[n - 1]); // last
                }
                // Absent needle: the least in-domain value with zero
                // occurrences. When the LCG vector happens to cover the full
                // 2^w domain (possible at ANY width once n >= 2^w), force a
                // gap on a copy: every occurrence of victim = values[0] is
                // replaced by (victim+1) & max, so needle = victim is
                // genuinely absent from the vector actually filtered.
                let mut forced_absent: Option<(Vec<u32>, u32)> = None;
                match absent_needle(&values, w) {
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
                    run_case(&values, w, needle);
                }
                if let Some((forced, victim)) = forced_absent {
                    run_case(&forced, w, victim);
                }
            }
        }
    }

    #[test]
    fn tail_bits_are_zeroed() {
        // 63 lanes at w=8: one mask word, bit 63 must stay zero even for
        // needle values the byte padding would satisfy.
        let packed = [9u8; 63];
        let mut m = [u64::MAX; 1];
        filter_eq_packed(&packed, 8, 9, &mut m);
        assert_eq!(m[0], (1u64 << 63) - 1, "bit 63 is padding, not a lane");

        // Padding bits are zero: needle=0 must NOT light the padding lanes.
        let packed0 = [0u8; 63];
        let mut m0 = [0u64; 1];
        filter_eq_packed(&packed0, 8, 0, &mut m0);
        assert_eq!(m0[0], (1u64 << 63) - 1);
    }

    #[test]
    fn excess_mask_words_untouched() {
        let packed = [7u8; 64]; // w=8: 64 lanes -> exactly 1 word
        let mut m = [0xDEAD_BEEF_CAFEu64; 3];
        filter_eq_packed(&packed, 8, 7, &mut m);
        assert_eq!(m[0], u64::MAX);
        assert_eq!(m[1], 0xDEAD_BEEF_CAFE, "excess word must be untouched");
        assert_eq!(m[2], 0xDEAD_BEEF_CAFE, "excess word must be untouched");
    }

    #[test]
    fn empty_input_writes_nothing() {
        let mut m: [u64; 0] = [];
        filter_eq_packed(&[], 8, 5, &mut m); // must not panic
        filter_eq_packed(&[], 1, 0, &mut m);
    }

    #[test]
    #[should_panic(expected = "unsupported bit width")]
    fn width_0_refused() {
        let mut m = [0u64; 1];
        filter_eq_packed(&[0u8; 8], 0, 0, &mut m);
    }

    #[test]
    #[should_panic(expected = "unsupported bit width")]
    fn width_3_refused() {
        let mut m = [0u64; 8];
        filter_eq_packed(&[0u8; 8], 3, 0, &mut m);
    }

    #[test]
    #[should_panic(expected = "unsupported bit width")]
    fn width_9_refused() {
        let mut m = [0u64; 1];
        filter_eq_packed(&[0u8; 8], 9, 0, &mut m);
    }

    #[test]
    #[should_panic(expected = "mask buffer too small")]
    fn short_mask_loud() {
        filter_eq_packed(&[7u8; 64], 8, 7, &mut [0u64; 0]);
    }

    #[test]
    fn needle_masked_to_width() {
        // 0x1F at w=4 is 0xF after the width mask — masks must agree.
        let packed = [0x1Fu8; 32];
        let mut a = [0u64; 4];
        let mut b = [0u64; 4];
        filter_eq_packed(&packed, 4, 0x1F, &mut a);
        filter_eq_packed(&packed, 4, 0x0F, &mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn helper_arithmetic() {
        assert_eq!(packed_rows(0, 8), 0);
        assert_eq!(packed_rows(64, 8), 64);
        assert_eq!(packed_rows(8, 1), 64);
        assert_eq!(packed_rows(1, 8), 1);
        assert_eq!(packed_filter_words(63, 8), 1);
        assert_eq!(packed_filter_words(64, 8), 1);
        assert_eq!(packed_filter_words(65, 8), 2);
        assert_eq!(packed_filter_words(8, 1), 1);
        assert_eq!(packed_filter_words(0, 2), 0);
    }

    #[test]
    fn one_million_lanes_word_aligned() {
        // Non-tail size: word-aligned row count, exercised in the grid too;
        // kept explicit so a grid regression can't hide behind its size.
        let w = 8u8;
        let n = 1_000_000usize; // % 64 == 16 -> tail masking exercised
        let mut rng = Lcg(0xFEED_1000);
        let values: Vec<u32> = (0..n).map(|_| rng.value(w)).collect();
        run_case(&values, w, values[n / 2]);
    }
}
