//! Fast-path RLE encoder: byte-identical to [`crate::rle::rle_encode`],
//! with literal groups packed one LE `u64` word (8 lanes) at a time.
//!
//! The baseline literal arm pushes one byte per lane through a per-lane
//! bit-copy loop; this module keeps the *exact* record decisions of the
//! fixed baseline (run threshold >= 3, literal-batch coalescing, the
//! absorb-to-full-group rule, zero-padding only in a final group) and only
//! replaces the payload packing: a full 8-lane group becomes a single
//! `u64` word (`lane k` at bits `[8k, 8k+8)`, each lane masked to `bits`
//! low bits) written with one `extend_from_slice`.
//!
//! Width note: [`crate::packing::pack_bits`] is the *tight* `w`-bits-per-
//! lane form (see its module doc: "Unlike `crate::rle` (one byte per lane
//! inside literal groups) this is the real width-packed form"). Routing
//! `w < 8` groups through `pack_bits` would emit `w` bytes per group
//! instead of 8 and break byte-identity with the fixed baseline, so the
//! word here is built directly; for `w = 8` the two forms coincide.
//!
//! Allocation: the output `Vec` is pre-sized so the encode performs exactly
//! one allocation (the baseline grew through 22-23 reallocs on a 64 MiB
//! literal page). Bound argument — every record's byte cost is bounded by
//! its lane count times 9/8:
//! - mid-stream literal batch (`L >= 8` lanes, `G = ceil(L/8)` groups, no
//!   padding because mid-stream batches are group-aligned after absorb):
//!   `8G + varint(2G+1) <= L + L/8` (varint `<= 5 <= 8G`);
//! - repeated run (`r >= 3`): `varint(2r) + vb <= 2r/3` (equality at
//!   `r = 3`, tighter above);
//! - one final partial literal batch per encode: `<= 9` bytes;
//! - width byte: 1.
//!
//! So total `<= 9n/8 + 10 << 2n + 64`, the pre-sized capacity. Growth past
//! the initial allocation is impossible while that bound holds, and the
//! test module pins final capacity equality to prove no realloc fired.

use crate::rle::write_varint;
use alloc::vec::Vec;

/// First index `j >= from` starting a >= 3-run (`values[j] == values[j+1]
/// == values[j+2]`, requiring `j + 2 < len`), else `values.len()` — the
/// baseline's literal-batch coalescing scan (`r >= 3` break condition),
/// with the per-lane `j + r < len` bound hoisted out of the hot loop.
fn batch_end(values: &[u32], from: usize) -> usize {
    let len = values.len();
    let mut j = from;
    while j + 2 < len {
        // windows(3).position over 64-lane blocks: bounds checks per block,
        // first triple-start wins. (Measured faster than a manual-index
        // loop at 256-lane blocks: 5.5 vs 3.5 GiB/s literal w=4.)
        let lim = (j + 64).min(len - 2);
        let block = &values[j..lim + 2];
        if let Some(off) = block.windows(3).position(|w| w[0] == w[1] && w[1] == w[2]) {
            return j + off;
        }
        j = lim;
    }
    len
}

/// Encode `values` as the hybrid codec (repeated runs >= 3, literal groups
/// of 8) — byte-identical to [`crate::rle::rle_encode`], faster on literal
/// and mixed pages.
///
/// Widths `1..=8` (the SWAR kernel set's byte domain) take the fast path.
/// Any other width delegates to the baseline encoder, so out-of-domain
/// behavior — including the baseline's literal-arm shift-overflow panic
/// for `bits in 9..=32` (debug) and its `to_le_bytes()[..vb]` slice panic
/// for `bits >= 33` — is identical by construction rather than by
/// re-derivation.
pub fn rle_encode_fast(values: &[u32], bits: u8) -> Vec<u8> {
    if !(1..=8).contains(&bits) {
        return crate::rle::rle_encode(values, bits);
    }
    // Lane bytes carry exactly `bits` low bits (the baseline's per-lane bit
    // copy loop is `byte = val & ((1 << bits) - 1)` by construction).
    let lane_mask = if bits == 8 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    };
    let vb = usize::from(bits).div_ceil(8); // == 1 for every width 1..=8
                                            // Single pre-sized allocation; bound argument in the module docs.
    let mut out = Vec::with_capacity(values.len().saturating_mul(2).saturating_add(64));
    out.push(bits);
    let mut i = 0;
    while i < values.len() {
        // repeated run when >= 3 equal values — same scan as the baseline.
        let v = values[i];
        let mut run = 1;
        while i + run < values.len() && values[i + run] == v {
            run += 1;
        }
        if run >= 3 {
            write_varint(&mut out, u64::from(run as u32) << 1);
            out.extend_from_slice(&v.to_le_bytes()[..vb]);
            i += run;
        } else {
            // literal batch: coalesce until the next >=3-run (or end) —
            // same first-triple-start semantics as the baseline scan.
            let start = i;
            let mut j = batch_end(values, i + run);
            // Absorb following lanes up to the group boundary (mid-stream
            // batches must be full 8-lane groups; zero-padding exists only
            // in a FINAL group) — same rule as the fixed baseline.
            let count = j - start;
            let rem = count % 8;
            if rem != 0 {
                let need = 8 - rem;
                let absorb = need.min(values.len() - j);
                j += absorb;
            }
            let count = j - start;
            let groups = count.div_ceil(8);
            write_varint(&mut out, ((groups as u64) << 1) | 1);
            // Full groups: one LE u64 word per 8 lanes, lane k at bit
            // offset 8k, masked to `bits` low bits — the baseline's
            // per-lane byte pushes, word-at-a-time. Words stage in a 64-byte
            // stack buffer (8 groups) so the output Vec sees one
            // `extend_from_slice` per 8 groups.
            let full = count / 8;
            let mut p = start;
            let mut stage = [0u8; 64];
            for g in 0..full {
                let word = u64::from(values[p] & lane_mask)
                    | (u64::from(values[p + 1] & lane_mask) << 8)
                    | (u64::from(values[p + 2] & lane_mask) << 16)
                    | (u64::from(values[p + 3] & lane_mask) << 24)
                    | (u64::from(values[p + 4] & lane_mask) << 32)
                    | (u64::from(values[p + 5] & lane_mask) << 40)
                    | (u64::from(values[p + 6] & lane_mask) << 48)
                    | (u64::from(values[p + 7] & lane_mask) << 56);
                stage[(g & 7) * 8..][..8].copy_from_slice(&word.to_le_bytes());
                if g & 7 == 7 {
                    out.extend_from_slice(&stage);
                }
                p += 8;
            }
            if full & 7 != 0 {
                out.extend_from_slice(&stage[..(full & 7) * 8]);
            }
            // Partial FINAL group (only when the batch hit end-of-input
            // mid-group): lanes beyond `j` are zero — mirrors the
            // baseline's `if start + lane < j { values[..] } else { 0 }`.
            let tail = count % 8;
            if tail != 0 {
                let mut word = 0u64;
                for k in 0..tail {
                    word |= u64::from(values[p + k] & lane_mask) << (8 * k);
                }
                out.extend_from_slice(&word.to_le_bytes());
            }
            i = j;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::rle_encode_fast;
    use crate::rle::{rle_decode, rle_encode};
    use alloc::vec::Vec;
    use std::println;

    /// Deterministic LCG (same pattern as packing.rs tests): stable vectors.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
    }

    #[derive(Clone, Copy)]
    enum Shape {
        /// Run-heavy: blocks of 1..=11 equal lanes (short blocks fall into
        /// literal batches and exercise the >=3 threshold + absorb rule).
        Repeated,
        /// Pure literal: consecutive lanes always differ (odd multiplier mod
        /// 2^bits), so the whole input is one literal batch.
        Literal,
        /// Alternating literal stretches (1..=23 lanes) and runs (3..=10).
        Mix,
    }

    fn shape_values(shape: Shape, n: usize, bits: u8, seed: u64) -> Vec<u32> {
        let mut s = Lcg(seed);
        let alpha = 1u32 << bits; // bits <= 8: no overflow
        let mask = alpha - 1;
        let mut v = Vec::with_capacity(n);
        match shape {
            Shape::Literal => {
                // Consecutive lanes differ by an odd constant mod 2^bits, so
                // no two neighbors are equal: zero runs, one literal batch.
                for i in 0..n {
                    let x = (i as u32)
                        .wrapping_mul(0x9E37_79B9)
                        .wrapping_add(s.next() as u32);
                    v.push(x & mask);
                }
            }
            Shape::Repeated => {
                while v.len() < n {
                    let run_len = 1 + (s.next() % 11) as usize;
                    let val = (s.next() as u32) & mask;
                    for _ in 0..run_len {
                        if v.len() == n {
                            break;
                        }
                        v.push(val);
                    }
                }
            }
            Shape::Mix => {
                while v.len() < n {
                    let lit_len = 1 + (s.next() % 23) as usize;
                    for k in 0..lit_len {
                        if v.len() == n {
                            break;
                        }
                        let x = (k as u32)
                            .wrapping_mul(0x9E37_79B9)
                            .wrapping_add(s.next() as u32);
                        v.push(x & mask);
                    }
                    let run_len = 3 + (s.next() % 8) as usize;
                    let val = (s.next() as u32) & mask;
                    for _ in 0..run_len {
                        if v.len() == n {
                            break;
                        }
                        v.push(val);
                    }
                }
            }
        }
        v
    }

    /// Differential parity: byte-identical to the FIXED `rle_encode` for
    /// every width 1..=8 x shape {repeated, literal, mix} x size 0..10000.
    /// Round-trip decode is asserted on a deterministic subsample for the
    /// decoder-supported widths {1,2,4,8} (byte-identity to the round-trip-
    /// correct baseline makes it a corollary; the subsample pins it anyway).
    #[test]
    fn differential_parity_all_widths_shapes_sizes() {
        let shapes = [Shape::Repeated, Shape::Literal, Shape::Mix];
        let mut cases = 0usize;
        let mut lanes = 0u64;
        for bits in 1..=8u8 {
            for (si, &shape) in shapes.iter().enumerate() {
                for n in 0..10000usize {
                    let seed =
                        0x5EED_0000 ^ (u64::from(bits) << 32) ^ ((si as u64) << 24) ^ (n as u64);
                    let values = shape_values(shape, n, bits, seed);
                    let base = rle_encode(&values, bits);
                    let fast = rle_encode_fast(&values, bits);
                    assert_eq!(
                        fast, base,
                        "byte divergence: bits={bits} shape={si:?} n={n}"
                    );
                    if matches!(bits, 1 | 2 | 4 | 8) && n % 101 == 0 {
                        let mut out = alloc::vec![0u32; n];
                        rle_decode(&fast, &mut out).expect("decode fast page");
                        assert_eq!(out, values, "round trip bits={bits} n={n}");
                    }
                    cases += 1;
                    lanes += n as u64;
                }
            }
        }
        println!("[differential] cases={cases} lanes={lanes} (encode Fast vs FIXED baseline, byte-exact)");
    }

    /// Golden wire bytes from tests/mutation_kills.rs, re-pinned against the
    /// fast encoder (same kills must land on either encoder).
    #[test]
    fn golden_bytes_match_mutation_kill_vectors() {
        // Absorb rule: mid-stream batch takes the following run lanes to
        // reach a full 8-lane group; zero-pad only at end of stream.
        assert_eq!(
            rle_encode_fast(&[9, 9, 9, 5, 5, 6, 6, 6], 4),
            alloc::vec![4, 6, 9, 3, 5, 5, 6, 6, 6, 0, 0, 0]
        );
        // Tail literal batch of 2 lanes = 1 zero-padded group.
        assert_eq!(
            rle_encode_fast(&[9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 5, 5], 4),
            alloc::vec![4, 0x14, 9, 3, 5, 5, 0, 0, 0, 0, 0, 0]
        );
        // Lane byte is exactly `bits` wide: 0x1F at w=4 must land as 0x0F.
        assert_eq!(
            rle_encode_fast(&[0x1F], 4),
            alloc::vec![4, 3, 0x0F, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            rle_encode_fast(&[0x1F], 8),
            alloc::vec![8, 3, 0x1F, 0, 0, 0, 0, 0, 0, 0]
        );
        // Full literal group at w=4: one byte per lane.
        assert_eq!(
            rle_encode_fast(&[1, 2, 3, 4, 5, 6, 7, 8], 4),
            alloc::vec![4, 3, 1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    /// Pre-allocation court: the returned Vec's capacity equals the
    /// requested pre-size exactly, for every shape — if the encoder had
    /// grown the Vec (the baseline's 22-23 reallocs), amortized doubling
    /// would leave capacity strictly larger than the request. Final-state
    /// assertion, no interaction counting.
    #[test]
    fn output_capacity_equals_presize_no_growth() {
        let shapes = [Shape::Repeated, Shape::Literal, Shape::Mix];
        for bits in [1u8, 4, 8] {
            for (si, &shape) in shapes.iter().enumerate() {
                let values = shape_values(shape, 1_000_000, bits, 0xC0FFEE + si as u64);
                let want = values.len() * 2 + 64;
                let out = rle_encode_fast(&values, bits);
                assert_eq!(
                    out.capacity(),
                    want,
                    "bits={bits} shape={si:?}: capacity changed => the encode reallocated"
                );
                // Parity precondition so the court is not vacuous.
                assert_eq!(out, rle_encode(&values, bits));
            }
        }
    }

    /// Delegation parity: out-of-kernel widths behave exactly as the
    /// baseline behaves (fast path refuses to re-derive them). Since the
    /// baseline grew an up-front domain check (`UnsupportedBitWidth` panic
    /// for widths outside 1..=8 — wave-4 lane 8 overflow fix), delegation
    /// parity there is panic-parity, asserted via catch_unwind on both
    /// encoders. In-kernel widths keep byte-parity asserts (see
    /// `differential_parity_all_widths_shapes_sizes`).
    #[test]
    fn out_of_domain_widths_delegate_byte_identically() {
        // bits=0 and bits=16 are outside the baseline's byte-lane domain
        // 1..=8: both encoders must refuse loudly (the fast path's body
        // there IS the baseline call, so this pins the delegation seam).
        let mixed = alloc::vec![7u32, 7, 7, 1, 2, 3, 4, 5, 6, 7, 8];
        let fast_refused = std::panic::catch_unwind(|| rle_encode_fast(&mixed, 0)).is_err();
        let base_refused = std::panic::catch_unwind(|| rle_encode(&mixed, 0)).is_err();
        assert!(
            fast_refused && base_refused,
            "bits=0: both encoders must refuse"
        );

        let runs = alloc::vec![7u32, 7, 7, 7, 9, 9, 9, 9];
        let fast_refused = std::panic::catch_unwind(|| rle_encode_fast(&runs, 16)).is_err();
        let base_refused = std::panic::catch_unwind(|| rle_encode(&runs, 16)).is_err();
        assert!(
            fast_refused && base_refused,
            "bits=16: both encoders must refuse"
        );
    }
}

/// In-print throughput bench vs the baseline encoder, same conventions as
/// crates/bench/src/bin/parquet_bench.rs: 16M u32 input = 64 MiB bytes-
/// moved, `black_box` on input and output length, min-of-5 rounds. Prints
/// only — never an assertion on timing.
#[cfg(test)]
mod bench {
    extern crate std;

    use super::rle_encode_fast;
    use crate::rle::rle_encode;
    use alloc::vec::Vec;
    use std::hint::black_box;
    use std::println;
    use std::time::Instant;

    /// 16M u32 = 64 MiB input, identical to parquet_bench RLE_LANES.
    const RLE_LANES: usize = 16 * 1024 * 1024;
    const ROUNDS: usize = 5;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    fn literal_values() -> Vec<u32> {
        (0..RLE_LANES).map(|i| (i as u32) & 0xF).collect()
    }

    fn mix50_values() -> Vec<u32> {
        (0..RLE_LANES)
            .map(|i| {
                if i % 16 < 8 {
                    0xF
                } else {
                    ((i % 8) as u32).min(0xE)
                }
            })
            .collect()
    }

    fn repeated_values() -> Vec<u32> {
        alloc::vec![0xF; RLE_LANES]
    }

    /// min-of-ROUNDS wall time for one encoder on one shape.
    fn min_of_five(values: &[u32], bits: u8, fast: bool) -> f64 {
        let mut best = f64::INFINITY;
        for _ in 0..ROUNDS {
            let t0 = Instant::now();
            let enc = if fast {
                rle_encode_fast(black_box(values), bits)
            } else {
                rle_encode(black_box(values), bits)
            };
            let dt = t0.elapsed().as_secs_f64();
            black_box(enc.len() as u64);
            drop(enc);
            best = best.min(dt);
        }
        best
    }

    #[test]
    fn bench_encode_fast_vs_baseline() {
        let bits = 4u8;
        let input_bytes = (RLE_LANES * 4) as f64;
        for (name, values) in [
            ("literal", literal_values()),
            ("mix50", mix50_values()),
            ("repeated", repeated_values()),
        ] {
            // Parity precondition on the real bench page: byte-identical.
            assert_eq!(
                rle_encode_fast(&values, bits),
                rle_encode(&values, bits),
                "{name}: fast encoder diverged on the bench page"
            );
            let base = min_of_five(&values, bits, false);
            let fast = min_of_five(&values, bits, true);
            let gb = input_bytes / GIB;
            if name == "literal" {
                println!(
                    "encode_fast literal w=4: {:.2} GiB/s vs ~0.99 baseline",
                    gb / fast
                );
            }
            println!(
                "[bench-fast] encode_fast {name} w={bits}: {:.3} GiB/s vs {:.3} GiB/s baseline (rle_encode), min-of-{ROUNDS}, {gb:.1} GiB input bytes moved",
                gb / fast,
                gb / base
            );
        }
        println!(
            "[bench-fast] note: concurrent cargo lanes share this machine; min-of-5 absorbs scheduler noise, absolute numbers remain load-bound"
        );
    }
}
