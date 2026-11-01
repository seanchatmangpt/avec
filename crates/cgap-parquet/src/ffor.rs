//! Frame-of-reference (FOR) fused with the SWAR bit-unpack lane math.
//!
//! Decode fuses the base add into the [`crate::unpack`] kernels: one pass
//! reads each packed u64 word once and emits base-relative u32 lanes, where
//! the two-pass alternative materializes u64 lanes and then walks them again
//! to add the base. Encode is the frame-of-reference transform proper:
//! `base = min(values)`, the value range must fit the declared width (typed
//! error otherwise), and the deltas are emitted as tight `w`-bit lanes
//! through [`crate::packing::pack_bits`].
//!
//! Lane layout is identical to [`crate::unpack`] / [`crate::packing`]:
//! lane `i` occupies bit range `[i*w, (i+1)*w)`, LSB-first; a packed word
//! holds `64/w` lanes; decode clamps the tail (missing source words read as
//! zero, exactly like the underlying kernels; source words beyond the
//! `out.len()` budget are never read).
//!
//! Widths are the SWAR kernel set {1, 2, 4, 8}. [`ffor_unpack`] has no
//! `Result` in its contract, so an out-of-set width is fail-loud (assert);
//! the typed path for arbitrary widths is [`crate::unpack::unpack_bits`].
//!
//! Charter v26.10.29, lane HWL3.

use crate::rle::ParquetError;
use alloc::vec::Vec;

/// Frame-of-reference fused unpack: `w`-bit lanes from `src`, each lane
/// emitted as `lane.wrapping_add(base)` into the u32 output.
///
/// Replicates the [`crate::unpack`] kernel drivers exactly — same word
/// iteration, same clamped tail — with `wrapping_add(base)` folded into the
/// per-lane expression, so the fused decode equals the unpack-then-add
/// reference lane-for-lane.
///
/// Contract:
/// - `w` must be in the SWAR kernel set {1, 2, 4, 8}; anything else is a
///   loud assert (no `Result` in this signature — the typed dispatcher is
///   [`crate::unpack::unpack_bits`]).
/// - `out.len()` drives the decode: exactly `out.len()` lanes are written.
///   Source words beyond `ceil(out.len() * w / 64)` are never read; missing
///   source words decode as zero lanes (then `+ base`), matching the
///   kernels' clamp.
///
/// The add is full u32 `wrapping_add` — frame-of-reference recovery is u32
/// arithmetic, not an addition masked back down to `w` bits.
pub fn ffor_unpack(src: &[u64], w: u8, base: u32, out: &mut [u32]) {
    assert!(
        matches!(w, 1 | 2 | 4 | 8),
        "ffor_unpack: unsupported bit width {w}; the SWAR kernel set is 1/2/4/8 \
         (typed path: unpack::unpack_bits)"
    );
    match w {
        1 => {
            let n = out.len();
            let mut si = 0usize;
            let mut oi = 0usize;
            while oi < n {
                let packed = if si < src.len() { src[si] } else { 0 };
                let avail = (n - oi).min(64);
                let mut b = 0;
                while b < avail {
                    let lane = ((packed >> b) & 1) as u32;
                    out[oi + b] = lane.wrapping_add(base);
                    b += 1;
                }
                si += 1;
                oi += 64;
            }
        }
        2 => {
            let n = out.len();
            let mut si = 0usize;
            let mut oi = 0usize;
            while oi < n {
                let packed = if si < src.len() { src[si] } else { 0 };
                let avail = (n - oi).min(32);
                let mut l = 0;
                while l < avail {
                    let lane = ((packed >> (l * 2)) & 0b11) as u32;
                    out[oi + l] = lane.wrapping_add(base);
                    l += 1;
                }
                si += 1;
                oi += 32;
            }
        }
        4 => {
            let n = out.len();
            let mut si = 0usize;
            let mut oi = 0usize;
            while oi < n {
                let packed = if si < src.len() { src[si] } else { 0 };
                let avail = (n - oi).min(16);
                let mut l = 0;
                while l < avail {
                    let lane = ((packed >> (l * 4)) & 0b1111) as u32;
                    out[oi + l] = lane.wrapping_add(base);
                    l += 1;
                }
                si += 1;
                oi += 16;
            }
        }
        8 => {
            let n = out.len();
            let mut si = 0usize;
            let mut oi = 0usize;
            while oi < n {
                let packed = if si < src.len() { src[si] } else { 0 };
                let avail = (n - oi).min(8);
                let mut l = 0;
                while l < avail {
                    let lane = ((packed >> (l * 8)) & 0xFF) as u32;
                    out[oi + l] = lane.wrapping_add(base);
                    l += 1;
                }
                si += 1;
                oi += 8;
            }
        }
        _ => unreachable!("width gated by the assert above"),
    }
}

/// Frame-of-reference pack: `base = min(values)`, deltas `value - base`
/// tight-packed at `w` bits via [`crate::packing::pack_bits`].
///
/// Returns `(base, packed_bytes)`. Fail-closed contract:
/// - `w` outside `1..=8` -> [`ParquetError::UnsupportedBitWidth`].
/// - empty `values` -> `Ok((0, Vec::new()))` (the empty frame: no lanes,
///   zero base).
/// - the value range must fit the declared width: `max(values) -
///   min(values)` must fit in `w` bits, else
///   [`ParquetError::LiteralOverrun { claimed, available }`] names the
///   conflict — `claimed` = bit length of the range, `available` = `w`.
///   (This file owns no error variants — `rle.rs` is outside its lane
///   ownership — so the closest existing typed variant is reused and its
///   field semantics at this site are declared here.)
///
/// `pack_bits` masks each lane to `w` bits; after the range check every
/// delta already fits, so the mask is a no-op (defense in depth, kept).
pub fn ffor_pack(values: &[u32], w: u8) -> Result<(u32, Vec<u8>), ParquetError> {
    if w == 0 || w > 8 {
        return Err(ParquetError::UnsupportedBitWidth(w));
    }
    let Some(&base) = values.iter().min() else {
        // Empty frame: no lanes, zero base; decodes to an empty output.
        return Ok((0, Vec::new()));
    };
    let mut max = base;
    for &v in values {
        if v > max {
            max = v;
        }
    }
    let range = max.wrapping_sub(base);
    let needed = 32 - range.leading_zeros();
    if needed > u32::from(w) {
        return Err(ParquetError::LiteralOverrun {
            claimed: needed as usize,
            available: w as usize,
        });
    }
    let mut deltas = Vec::with_capacity(values.len());
    for &v in values {
        deltas.push(v.wrapping_sub(base));
    }
    Ok((base, crate::packing::pack_bits(&deltas, w)))
}

#[cfg(test)]
mod tests {
    use super::ffor_pack;
    use super::ffor_unpack;
    use crate::packing::{pack_bits, unpack_bits_tight};
    use crate::unpack::{unpack_1bit, unpack_2bit, unpack_4bit, unpack_8bit};
    use alloc::vec;
    use alloc::vec::Vec;

    /// splitmix64 — the const_time_bench house RNG (replayable seeds).
    struct SplitMix(u64);

    impl SplitMix {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        /// Uniform in `[0, n)`.
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// The two-pass reference: materialize lanes through the real
    /// `unpack_*bit` kernels, then walk them once to add the base.
    fn reference_unpack_add(src: &[u64], w: u8, base: u32, out: &mut [u32]) {
        let mut lanes = alloc::vec![0u64; out.len()];
        match w {
            1 => unpack_1bit(src, &mut lanes),
            2 => unpack_2bit(src, &mut lanes),
            4 => unpack_4bit(src, &mut lanes),
            8 => unpack_8bit(src, &mut lanes),
            other => panic!("reference: bad width {other}"),
        }
        for (o, &l) in out.iter_mut().zip(&lanes) {
            *o = (l as u32).wrapping_add(base);
        }
    }

    /// Load a byte stream into LE u64 words (zero-padded final word) so the
    /// SWAR kernels (which take `&[u64]`) see the same bit positions —
    /// the packing.rs test-suite helper, replicated here.
    fn le_words(bytes: &[u8]) -> Vec<u64> {
        let mut words = alloc::vec![0u64; bytes.len().div_ceil(8)];
        for (i, chunk) in bytes.chunks(8).enumerate() {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            words[i] = u64::from_le_bytes(buf);
        }
        words
    }

    /// Seeded random packed words for the differential (arbitrary bits: the
    /// reference defines the expected output whatever the bits are).
    fn random_words(n_words: usize, seed: u64) -> Vec<u64> {
        let mut r = SplitMix(seed);
        (0..n_words).map(|_| r.next()).collect()
    }

    /// Differential: fused decode == unpack-then-add reference, across the
    /// kernel widths x sizes spanning every word/tail boundary x bases {0,
    /// seeded} (base 0 additionally proves the plain-unpack identity).
    #[test]
    fn differential_vs_unpack_then_add() {
        for w in [1u8, 2, 4, 8] {
            for &n in &[0usize, 1, 63, 64, 1000, 1_000_000] {
                let per_word = 64 / w as usize;
                let words = n.div_ceil(per_word);
                for base in [0u32, 0xDEAD_BEEF] {
                    let seed = 0xF0A5_0000 ^ (u64::from(w) * 7919) ^ n as u64 ^ u64::from(base);
                    let src = random_words(words, seed);
                    let mut got = vec![0u32; n];
                    ffor_unpack(&src, w, base, &mut got);
                    let mut exp = vec![0u32; n];
                    reference_unpack_add(&src, w, base, &mut exp);
                    assert_eq!(got, exp, "w={w} n={n} base={base:#x}");
                }
            }
        }
    }

    /// Clamp semantics, stated as behavior: source words past the `out`
    /// budget are never read, and a short source zero-fills the missing
    /// words (then adds base), exactly like the underlying kernels.
    #[test]
    fn source_clamping_matches_kernels() {
        for w in [1u8, 2, 4, 8] {
            let per_word = 64 / w as usize;
            let n = per_word + 5; // spills one word plus a tail
            let src = random_words(4, 0xC1A0 ^ u64::from(w));
            let mut got = vec![0u32; n];
            ffor_unpack(&src, w, 0x1111_1111, &mut got);
            let mut exp = vec![0u32; n];
            reference_unpack_add(&src[..2], w, 0x1111_1111, &mut exp);
            assert_eq!(got, exp, "w={w}: extra source words ignored");
            // Short source: missing words are zero lanes + base.
            let mut got_short = vec![0u32; n];
            ffor_unpack(&src[..1], w, 0x1111_1111, &mut got_short);
            let mut exp_short = vec![0u32; n];
            reference_unpack_add(&src[..1], w, 0x1111_1111, &mut exp_short);
            assert_eq!(got_short, exp_short, "w={w}: short source zero-fills");
        }
    }

    /// Known-answer pack: base is the min, deltas pack LSB-first.
    #[test]
    fn pack_known_answer() {
        let (base, bytes) = ffor_pack(&[10, 12, 9, 15], 4).expect("pack");
        assert_eq!(base, 9);
        // deltas [1, 3, 0, 6] at 4 bits, LSB-first: byte0 = 1 | 3<<4 =
        // 0x31, byte1 = 0 | 6<<4 = 0x60 (u16 stream 0x6031, LE bytes).
        assert_eq!(bytes, vec![0x31, 0x60]);
        // And the tight packing itself agrees (independent construction).
        assert_eq!(bytes, pack_bits(&[1, 3, 0, 6], 4));
    }

    /// Pack edge shapes: empty frame, single value, exact range fit.
    #[test]
    fn pack_edges() {
        assert_eq!(ffor_pack(&[], 4).expect("empty"), (0, Vec::new()));
        assert_eq!(ffor_pack(&[7], 8).expect("single"), (7, vec![0x00]));
        assert_eq!(ffor_pack(&[7], 4).expect("single"), (7, vec![0x00]));
        // Range exactly filling the width is lawful (inclusive bound).
        assert_eq!(ffor_pack(&[100, 100 + 15], 4).expect("exact fit").0, 100);
    }

    /// Width gate: out-of-set widths are typed errors on the pack side.
    #[test]
    fn pack_rejects_bad_width() {
        assert_eq!(
            ffor_pack(&[1, 2, 3], 0),
            Err(crate::rle::ParquetError::UnsupportedBitWidth(0))
        );
        assert_eq!(
            ffor_pack(&[1, 2, 3], 9),
            Err(crate::rle::ParquetError::UnsupportedBitWidth(9))
        );
        assert_eq!(
            ffor_pack(&[1, 2, 3], 255),
            Err(crate::rle::ParquetError::UnsupportedBitWidth(255))
        );
    }

    /// Range gate: a value span wider than the declared width is a typed
    /// error naming the bit conflict; the exact fit at the bound passes.
    #[test]
    fn pack_rejects_range_over_width() {
        // [0, 16] spans 17 values -> needs 5 bits at w=4.
        assert_eq!(
            ffor_pack(&[0, 16], 4),
            Err(crate::rle::ParquetError::LiteralOverrun {
                claimed: 5,
                available: 4
            })
        );
        // Full u32 span at w=8: the largest possible conflict.
        assert_eq!(
            ffor_pack(&[0, u32::MAX], 8),
            Err(crate::rle::ParquetError::LiteralOverrun {
                claimed: 32,
                available: 8
            })
        );
        // Single distinct value: zero range fits any width.
        assert!(ffor_pack(&[5, 5, 5], 1).is_ok());
    }

    /// const_time_bench distribution classes (crate bench
    /// const_time_bench.rs `fill_u32`), shape-faithful, narrowed to a value
    /// band `[lo, lo + band)` so the frame-of-reference width shrinks.
    /// Random-high-entropy stays over the full byte domain as the control.
    mod fixtures {
        use super::SplitMix;

        pub const LO: u32 = 1000;
        pub const BAND: u32 = 16; // fits 4 bits: max - min <= 15

        /// C_CLUSTERED: seeded blocks of 64..=191 lanes sharing one value,
        /// values drawn per block from the band.
        pub fn fill_clustered(v: &mut [u32], seed: u64) {
            let mut r = SplitMix(seed);
            let mut i = 0;
            while i < v.len() {
                let block = 64 + r.below(128) as usize;
                let val = LO + r.below(u64::from(BAND)) as u32;
                let end = (i + block).min(v.len());
                v[i..end].fill(val);
                i = end;
            }
        }

        /// C_SORTED: the ascending staircase across the band.
        pub fn fill_sorted(v: &mut [u32]) {
            let n = v.len();
            for (i, x) in v.iter_mut().enumerate() {
                *x = LO + ((i as u64) * u64::from(BAND - 1) / (n - 1) as u64) as u32;
            }
        }

        /// C_RANDOM control: uniform over the full byte domain [0, 256).
        pub fn fill_random(v: &mut [u32], seed: u64) {
            let mut r = SplitMix(seed);
            for x in v.iter_mut() {
                *x = r.below(256) as u32;
            }
        }
    }

    /// Round-trip over FOR-friendly distribution fixtures: ffor_pack ->
    /// (base, bytes) -> LE words -> ffor_unpack recovers the values; the
    /// narrow-band clustered/sorted fixtures prove width shrink (declared
    /// w=4 vs the 8-bit byte domain), asserted on both the value span and
    /// the exact wire size.
    #[test]
    fn round_trip_for_friendly_distributions_shrink_width() {
        const N: usize = 1_000_000;
        let mut values = vec![0u32; N];

        // Clustered: const_time_bench C_CLUSTERED shape, band-narrowed.
        fixtures::fill_clustered(&mut values, 0x51E5_1E5D);
        check_round_trip("clustered", &values);
        // Sorted: const_time_bench C_SORTED shape (staircase), band-narrowed.
        fixtures::fill_sorted(&mut values);
        check_round_trip("sorted", &values);

        // Shared round-trip + shrink proof, asserted per fixture.
        fn check_round_trip(name: &str, values: &[u32]) {
            let n = values.len();
            let (base, bytes) = ffor_pack(values, 4).expect("pack");
            // Width shrink, asserted structurally: the span fits 4 bits
            // (never 8), so the wire is half the w=8 wire, a quarter of raw.
            let max = *values.iter().max().expect("nonempty");
            assert!(max - base <= 15, "{name}: span {max}-{base} must fit w=4");
            assert_eq!(bytes.len(), n * 4 / 8, "{name}: w=4 wire is n/2 bytes");
            assert!(
                (fixtures::LO..fixtures::LO + fixtures::BAND).contains(&base),
                "{name}: base {base} must sit inside the band"
            );
            // Round-trip through the real bit streams.
            let words = le_words(&bytes);
            let mut out = vec![0u32; n];
            ffor_unpack(&words, 4, base, &mut out);
            assert_eq!(out, values, "{name}: round-trip recovered values");
            // The same page at w=8 is lawful but twice the wire: the
            // shrink is FOR's contribution, not the packer's.
            let (_, wide) = ffor_pack(values, 8).expect("pack w=8");
            assert_eq!(wide.len(), n, "{name}: w=8 wire is n bytes");
        }
    }

    /// Non-vacuity control: the full-byte-domain random class genuinely
    /// needs 8 bits — the range gate must fire on it at w=4 (typed error
    /// on real fixture data, not only on constructed ranges).
    #[test]
    fn random_class_requires_full_width_control() {
        const N: usize = 100_000;
        let mut values = vec![0u32; N];
        fixtures::fill_random(&mut values, 0xC072_0C01);
        let max = *values.iter().max().expect("nonempty");
        let min = *values.iter().min().expect("nonempty");
        assert_eq!(
            ffor_pack(&values, 4),
            Err(crate::rle::ParquetError::LiteralOverrun {
                claimed: 32 - (max - min).leading_zeros() as usize,
                available: 4
            })
        );
        // And at its true width the class round-trips.
        let (base, bytes) = ffor_pack(&values, 8).expect("pack w=8");
        let words = le_words(&bytes);
        let mut out = vec![0u32; N];
        ffor_unpack(&words, 8, base, &mut out);
        assert_eq!(out, values);
    }

    /// Bytes emitted by the fused decode round-trip into the tight byte
    /// decoder (packing.rs) unchanged: the two stream representations see
    /// the same bit positions (cross-representation consistency).
    #[test]
    fn fused_decode_matches_tight_decode_plus_add() {
        for w in [1u8, 2, 4, 8] {
            let n = 100usize; // straddles every byte/group boundary
            let mut r = SplitMix(0xBEEF + u64::from(w));
            let base = 0x1234_5678;
            let deltas: Vec<u32> = (0..n).map(|_| r.below(1u64 << w) as u32).collect();
            let bytes = pack_bits(&deltas, w);
            let words = le_words(&bytes);

            let mut tight = vec![0u32; n];
            unpack_bits_tight(&bytes, w, &mut tight).expect("tight decode");
            for lane in tight.iter_mut() {
                *lane = lane.wrapping_add(base);
            }

            let mut fused = vec![0u32; n];
            ffor_unpack(&words, w, base, &mut fused);
            assert_eq!(fused, tight, "w={w}: fused == tight-decode+add");
        }
    }
}

/// In-print throughput bench vs the two-pass unpack+add reference, same
/// conventions as unpack_filter.rs / encode_fast.rs: `black_box` on inputs,
/// outputs consumed after timing, min-of-5 rounds, byte-identical page
/// parity precondition before any timing. Prints only — never an assertion
/// on timing. Logical bytes = the decoded u32 stream (n * 4), identical
/// accounting for both paths so the ratio is apples-to-apples. Scalar SWAR,
/// not SIMD: FastLanes' >100B values/s is SIMD-512 published context, the
/// numbers here are this machine's measured scalar baseline (load-bound
/// under concurrent lanes).
#[cfg(test)]
mod bench {
    extern crate std;

    use super::ffor_unpack;
    use crate::unpack::unpack_8bit;
    use alloc::vec;
    use std::hint::black_box;
    use std::println;
    use std::time::Instant;

    const N: usize = 1_000_000;
    const ROUNDS: usize = 5;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    /// xorshift64 (the unpack_filter bench generator).
    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    /// The two-pass reference: materialize u64 lanes, then a second pass
    /// adds the base on the way into the u32 output.
    fn two_pass(src: &[u64], base: u32, out: &mut [u32]) {
        let mut lanes = alloc::vec![0u64; out.len()];
        unpack_8bit(src, &mut lanes);
        for (o, &l) in out.iter_mut().zip(&lanes) {
            *o = (l as u32).wrapping_add(base);
        }
    }

    #[test]
    fn bench_ffor_vs_unpack_add_two_pass() {
        let per_word = 64 / 8;
        let words = N.div_ceil(per_word);
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let src: alloc::vec::Vec<u64> = (0..words).map(|_| xorshift(&mut state)).collect();
        let base: u32 = 0x9ABC_DEF0;

        let mut out_fused = vec![0u32; N];
        let mut out_two = vec![0u32; N];

        // Parity precondition on the bench page: byte-identical results
        // before any timing, so the comparison measures speed only.
        ffor_unpack(&src, 8, base, &mut out_fused);
        two_pass(&src, base, &mut out_two);
        assert_eq!(out_fused, out_two, "bench page diverged");

        let mut best_fused = f64::INFINITY;
        let mut best_two = f64::INFINITY;
        for _ in 0..ROUNDS {
            let t0 = Instant::now();
            ffor_unpack(
                black_box(&src),
                8,
                black_box(base),
                black_box(&mut out_fused),
            );
            best_fused = best_fused.min(t0.elapsed().as_secs_f64());
            black_box(out_fused[out_fused.len() - 1]);

            let t1 = Instant::now();
            two_pass(black_box(&src), black_box(base), black_box(&mut out_two));
            best_two = best_two.min(t1.elapsed().as_secs_f64());
            black_box(out_two[out_two.len() - 1]);
        }

        let fused_gibs = (N * 4) as f64 / GIB / best_fused;
        let two_gibs = (N * 4) as f64 / GIB / best_two;
        println!(
            "[bench-ffor] ffor w=8: {fused_gibs:.3} GiB/s vs unpack+add two-pass {two_gibs:.3} \
             (n={N}, base={base:#x}, min-of-{ROUNDS}, logical bytes = decoded u32 stream)"
        );
        println!(
            "[bench-ffor] note: scalar SWAR on this machine (FastLanes' >100B values/s is \
             SIMD-512 published context, not comparable); concurrent cargo lanes share the \
             host, absolute numbers are load-bound, the fused/two-pass ratio is the signal"
        );
    }
}
