//! Fused SWAR bit-unpacking + equality predicate: unpack 4/8-bit lanes AND
//! emit the admission bitmask (`lane == needle`) in ONE pass over the packed
//! words, eliminating the materialize-then-mask second pass.
//!
//! Byte rules, differentially tested against the two-pass reference over
//! widths {4,8} x sizes {0,1,63,64,1000,1M} x needles {min,max,absent,first,last}:
//! 1. `out` is identical to the plain unpackers in `unpack.rs` (same lane
//!    math, short-src zero padding, tail clamped to `out.len()`).
//! 2. `valid` is the admission bitmask: word `i` covers lanes
//!    `64i..64i+63`, LSB-first; bit `j` set iff lane `64i+j == needle`;
//!    bits past lane `n-1` are zero.
//!
//! Requires `valid.len() >= out.len().div_ceil(64)`; a short mask buffer is
//! a caller contract violation and fails LOUD (assert), never a silent
//! truncation. Words of `valid` beyond the mask length are left untouched.
//!
//! Charter v26.10.29, layer 3 (no_std, zero dependencies).

/// One lane loop for both widths (`W` in {4, 8}): identical shift/mask lane
/// math to `unpack.rs`, with the equality compare folded into the same loop
/// that writes the lane. One kernel, two bindings — no second implementation
/// of the lane calculus to drift.
///
/// Mask-word grouping: `W` packed words cover exactly 64 lanes
/// (`W * 64/W = 64`), so one `u64` mask word is accumulated per group and
/// flushed to `valid[vi]` before the next group starts.
#[inline]
fn unpack_filtered_kernel<const W: u8>(
    src: &[u64],
    needle: u32,
    out: &mut [u64],
    valid: &mut [u64],
) {
    debug_assert!(W == 4 || W == 8, "kernel defined for 4/8-bit lanes only");
    let n = out.len();
    let mask_words = n.div_ceil(64);
    let w = W as usize;
    assert!(
        valid.len() >= mask_words,
        "unpack {w}-bit filtered: mask buffer too small: valid.len() = {}, need >= {mask_words} words for {n} lanes",
        valid.len()
    );
    let lanes_per_word = 64 / w; // 16 lanes per packed word (w=4) or 8 (w=8)
    let lane_mask: u64 = if W == 4 { 0xF } else { 0xFF };
    let needle_wide = needle as u64;
    let mut si = 0usize; // packed word cursor
    let mut vi = 0usize; // mask word cursor
    let mut oi = 0usize; // lane cursor
    while oi < n {
        // One mask word: W packed words x lanes_per_word lanes = 64 lanes.
        let mut m = 0u64;
        let mut s = 0usize; // packed words folded into this mask word
        while s < w && oi < n {
            // Short src zero-pads, exactly as the plain unpackers.
            let packed = if si < src.len() { src[si] } else { 0 };
            let avail = (n - oi).min(lanes_per_word);
            let mut l = 0;
            while l < avail {
                let lane = (packed >> (l * w)) & lane_mask;
                out[oi + l] = lane;
                m |= ((lane == needle_wide) as u64) << (s * lanes_per_word + l);
                l += 1;
            }
            si += 1;
            oi += avail;
            s += 1;
        }
        valid[vi] = m;
        vi += 1;
    }
}

/// Fused 4-bit unpack + admission mask: `out` as `unpack_4bit`, and `valid`
/// with bit `j` of word `i` set iff lane `64i+j == needle` (LSB-first, tail
/// bits zero).
#[inline]
pub fn unpack_4bit_filtered(src: &[u64], needle: u32, out: &mut [u64], valid: &mut [u64]) {
    unpack_filtered_kernel::<4>(src, needle, out, valid);
}

/// Fused 8-bit unpack + admission mask: `out` as `unpack_8bit`, and `valid`
/// with bit `j` of word `i` set iff lane `64i+j == needle` (LSB-first, tail
/// bits zero).
#[inline]
pub fn unpack_8bit_filtered(src: &[u64], needle: u32, out: &mut [u64], valid: &mut [u64]) {
    unpack_filtered_kernel::<8>(src, needle, out, valid);
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{unpack_4bit_filtered, unpack_8bit_filtered};
    use crate::unpack::{unpack_4bit, unpack_8bit};
    use alloc::vec;
    use alloc::vec::Vec;

    /// UnpackFn/FusedFn: fn-pointer aliases so the sweep can bind either
    /// width without a generic layer.
    type UnpackFn = fn(&[u64], &mut [u64]);
    type FusedFn = fn(&[u64], u32, &mut [u64], &mut [u64]);

    const W4: u8 = 4;
    const W8: u8 = 8;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    /// Deterministic packed stream: `ceil(n / lanes_per_word)` words of
    /// xorshift output. Any bit pattern is a valid packed stream.
    fn gen_src(n: usize, lanes_per_word: usize) -> Vec<u64> {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        (0..n.div_ceil(lanes_per_word))
            .map(|_| xorshift(&mut state))
            .collect()
    }

    /// Oracle lanes: the settled plain unpacker, unmodified.
    fn unpack_ref(unpack: UnpackFn, src: &[u64], n: usize) -> Vec<u64> {
        let mut out = vec![0u64; n];
        unpack(src, &mut out);
        out
    }

    /// Oracle mask: unpack-then-mask reference (LSB-first, one word per 64
    /// lanes, tail bits zero).
    fn mask_ref(lanes: &[u64], needle: u32) -> Vec<u64> {
        let mut valid = vec![0u64; lanes.len().div_ceil(64)];
        for (i, &lane) in lanes.iter().enumerate() {
            if lane == needle as u64 {
                valid[i / 64] |= 1 << (i % 64);
            }
        }
        valid
    }

    #[test]
    fn hand_computed_masks() {
        // w=8 oracle from unpack.rs: [0x40302010] -> [0x10,0x20,0x30,0x40,0,0,0,0]
        let mut out = [0xAAu64; 8];
        let mut valid = [0u64; 1];
        unpack_8bit_filtered(&[0x4030_2010u64], 0x30, &mut out, &mut valid);
        assert_eq!(out, [0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0]);
        assert_eq!(valid, [0b100]); // only lane 2 (0x30) matched

        // w=4 oracle from unpack.rs: [0xDCBA_3210] -> [0,1,2,3,0xA,0xB,0xC,0xD]
        let mut out = [0xAAu64; 8];
        let mut valid = [0u64; 1];
        unpack_4bit_filtered(&[0xDCBA_3210u64], 0xA, &mut out, &mut valid);
        assert_eq!(out, [0x0, 0x1, 0x2, 0x3, 0xA, 0xB, 0xC, 0xD]);
        assert_eq!(valid, [1 << 4]); // only lane 4 (0xA) matched
    }

    #[test]
    fn zero_pad_lanes_enter_the_mask() {
        // src holds 2 words but n=20 lanes need 3 words: lanes 16..19 come
        // from zero-padded words and are real lanes for the mask.
        let src = [0u64, u64::MAX];
        let mut out = [0xAAu64; 20];
        let mut valid = [0u64; 1];
        unpack_8bit_filtered(&src, 0, &mut out, &mut valid);
        assert_eq!(&out[..8], &[0u64; 8]); // word 0 = all-zero bytes
        assert_eq!(&out[8..16], &[0xFF; 8]); // word 1 = all-0xFF bytes
        assert_eq!(&out[16..20], &[0; 4]); // padded words -> zero lanes
                                           // bits 0..7 set (0 lanes == needle 0), bits 8..15 clear (0xFF),
                                           // bits 16..19 set (padded zero lanes), tail bits 20..63 zero.
        assert_eq!(valid[0], 0xF_0000 | 0xFF);
    }

    #[test]
    fn mask_words_beyond_n_are_untouched() {
        let src = gen_src(64, 8);
        let mut out = [0u64; 64];
        let mut valid = [7u64; 3]; // one mask word needed, three provided
        unpack_8bit_filtered(&src, 0x99, &mut out, &mut valid);
        assert_eq!(valid[1], 7);
        assert_eq!(valid[2], 7);
        let lanes = unpack_ref(unpack_8bit as UnpackFn, &src, 64);
        assert_eq!(valid[0], mask_ref(&lanes, 0x99)[0]);
    }

    #[test]
    #[should_panic(expected = "mask buffer too small")]
    fn short_valid_fails_loud() {
        let mut out = [0u64; 64];
        let mut valid = [0u64; 0]; // need 1 mask word for 64 lanes
        unpack_8bit_filtered(&[0u64; 8], 0, &mut out, &mut valid);
    }

    /// Byte rules, exhaustively: fused out == plain unpacker out and fused
    /// mask == unpack-then-mask reference, over widths x sizes x needles.
    #[test]
    fn differential_vs_two_pass() {
        const SIZES: [usize; 6] = [0, 1, 63, 64, 1000, 1_000_000];
        let mut sweep_cases = 0usize;
        let mut sweep_hits = 0u64; // mask bits set across the whole sweep
        let mut sweep_zero_hit_cases = 0usize; // cases whose mask is empty
        for (w, unpack, fused) in [
            (W4, unpack_4bit as UnpackFn, unpack_4bit_filtered as FusedFn),
            (W8, unpack_8bit as UnpackFn, unpack_8bit_filtered as FusedFn),
        ] {
            let max_lane = (1u32 << w) - 1;
            let absent = 1u32 << w; // out of lane domain -> cannot match
            for &n in &SIZES {
                let src = gen_src(n, (64 / w as usize));
                let lanes = unpack_ref(unpack, &src, n);
                let mut needles: Vec<u32> = vec![0, max_lane, absent]; // min, max, absent
                if n > 0 {
                    needles.push(lanes[0] as u32); // first
                    needles.push(lanes[n - 1] as u32); // last
                }
                for &needle in &needles {
                    let mut fused_out = vec![0xAAu64; n];
                    let mut fused_valid = vec![0xAAu64; n.div_ceil(64)];
                    fused(&src, needle, &mut fused_out, &mut fused_valid);
                    let expect_valid = mask_ref(&lanes, needle);
                    assert_eq!(
                        fused_out, lanes,
                        "w={w} n={n} needle={needle}: out diverges from plain unpack"
                    );
                    assert_eq!(
                        fused_valid, expect_valid,
                        "w={w} n={n} needle={needle}: mask diverges from unpack-then-mask"
                    );
                    let hits: u64 = expect_valid.iter().map(|x| x.count_ones() as u64).sum();
                    sweep_hits += hits;
                    if hits == 0 {
                        sweep_zero_hit_cases += 1;
                    }
                    sweep_cases += 1;
                }
            }
        }
        // Non-vacuity: the sweep must exercise both mask outcomes.
        assert!(sweep_cases >= 56, "sweep ran {sweep_cases} cases");
        assert!(sweep_hits > 0, "no lane ever matched any needle");
        assert!(sweep_zero_hit_cases > 0, "no needle ever came up empty");
    }

    /// Partial-tail case not covered by power-of-two sizes: 65 lanes span
    /// two mask words, the second holding a single lane.
    #[test]
    fn differential_tail_65_lanes() {
        for (w, unpack, fused) in [
            (W4, unpack_4bit as UnpackFn, unpack_4bit_filtered as FusedFn),
            (W8, unpack_8bit as UnpackFn, unpack_8bit_filtered as FusedFn),
        ] {
            let src = gen_src(65, (64 / w as usize));
            let lanes = unpack_ref(unpack, &src, 65);
            let needle = lanes[64] as u32; // the lone tail lane's value
            let mut fused_out = vec![0xAAu64; 65];
            let mut fused_valid = vec![0xAAu64; 2];
            fused(&src, needle, &mut fused_out, &mut fused_valid);
            assert_eq!(fused_out, lanes);
            assert_eq!(fused_valid, mask_ref(&lanes, needle));
            assert_eq!(fused_valid[1], 1); // bit 64, nothing else
        }
    }
}

/// In-print throughput bench vs the two-pass reference, same conventions as
/// encode_fast.rs: `black_box` on inputs, outputs consumed after timing,
/// min-of-5 rounds. Prints only — never an assertion on timing. Logical
/// bytes = the unpacked u64 stream (n * 8), identical accounting for both
/// paths so the ratio is apples-to-apples.
#[cfg(test)]
mod bench {
    extern crate std;

    use super::{unpack_4bit_filtered, unpack_8bit_filtered};
    use crate::unpack::{unpack_4bit, unpack_8bit};
    use alloc::vec;
    use alloc::vec::Vec;
    use std::hint::black_box;
    use std::println;
    use std::time::Instant;

    type UnpackFn = fn(&[u64], &mut [u64]);
    type FusedFn = fn(&[u64], u32, &mut [u64], &mut [u64]);

    const N: usize = 1_000_000;
    const ROUNDS: usize = 5;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    fn gen_src(w: u8) -> Vec<u64> {
        let lanes_per_word = 64 / w as usize;
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        (0..N.div_ceil(lanes_per_word))
            .map(|_| xorshift(&mut state))
            .collect()
    }

    /// The two-pass reference: materialize lanes, then mask them.
    fn two_pass(unpack: UnpackFn, src: &[u64], needle: u32, out: &mut [u64], valid: &mut [u64]) {
        unpack(src, out);
        valid.fill(0);
        let needle_wide = needle as u64;
        for (i, &lane) in out.iter().enumerate() {
            if lane == needle_wide {
                valid[i / 64] |= 1 << (i % 64);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn min_of_five(
        unpack: UnpackFn,
        fused: FusedFn,
        src: &[u64],
        needle: u32,
        out: &mut [u64],
        valid: &mut [u64],
        use_fused: bool,
    ) -> f64 {
        let mut best = f64::INFINITY;
        for _ in 0..ROUNDS {
            let t0 = Instant::now();
            if use_fused {
                fused(black_box(src), black_box(needle), out, valid);
            } else {
                two_pass(unpack, black_box(src), black_box(needle), out, valid);
            }
            best = best.min(t0.elapsed().as_secs_f64());
        }
        // Consume both outputs so the timed stores cannot be elided.
        black_box(out[out.len() - 1] ^ valid[valid.len() - 1]);
        best
    }

    #[test]
    fn bench_fused_vs_two_pass() {
        for (w, unpack, fused, needle) in [
            (
                4u8,
                unpack_4bit as UnpackFn,
                unpack_4bit_filtered as FusedFn,
                0x9u32,
            ),
            (
                8u8,
                unpack_8bit as UnpackFn,
                unpack_8bit_filtered as FusedFn,
                0x9Au32,
            ),
        ] {
            let src = gen_src(w);
            let mut out_fused = vec![0u64; N];
            let mut valid_fused = vec![0u64; N.div_ceil(64)];
            let mut out_two = vec![0u64; N];
            let mut valid_two = vec![0u64; N.div_ceil(64)];
            // Parity precondition on the bench page: byte-identical.
            fused(&src, needle, &mut out_fused, &mut valid_fused);
            two_pass(unpack, &src, needle, &mut out_two, &mut valid_two);
            assert_eq!(out_fused, out_two, "w={w}: bench page out diverged");
            assert_eq!(valid_fused, valid_two, "w={w}: bench page mask diverged");

            let fused_gibs = (N * 8) as f64
                / GIB
                / min_of_five(
                    unpack,
                    fused,
                    &src,
                    needle,
                    &mut out_fused,
                    &mut valid_fused,
                    true,
                );
            let two_gibs = (N * 8) as f64
                / GIB
                / min_of_five(
                    unpack,
                    fused,
                    &src,
                    needle,
                    &mut out_two,
                    &mut valid_two,
                    false,
                );
            println!(
                "[bench-unpack-filter] unpack+filter w={w} fused vs two-pass: {fused_gibs:.3} vs {two_gibs:.3} GiB/s logical (n={N}, needle=0x{needle:X}, min-of-{ROUNDS})"
            );
        }
        println!(
            "[bench-unpack-filter] note: concurrent cargo lanes share this machine; min-of-5 absorbs scheduler noise, absolute numbers remain load-bound"
        );
    }
}
