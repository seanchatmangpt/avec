//! Constant-time predication primitives and a branchless quad-equality filter.
//!
//! These are the M=1 (single conditional-materialization mode) candidate
//! primitives specified by `docs/jira/v26.10.31/27-branchless-kernel-abi.md`
//! (CSEL/CSETM/CMOV select-and-mask class; "equality masks" kernel inventory
//! row) and the algebra of `docs/jira/v26.10.31/18-m1-branchless-algebra.md`.
//!
//! **Standing: M=1 CANDIDATES, UNVERIFIED.** The branchless property is a
//! property of disassembled `.text`, not of source intent — LLVM may lower
//! these to branches (doc 27 §4 de-optimization hazard). The disassembly
//! audit gate (doc 27 §3) is owned by the scripts lane (I14); these
//! primitives pass only their functional Chicago tests here and must not be
//! treated as M=1 until that gate witnesses clean `.text`.

/// All-ones if `a == b`, all-zeros otherwise (u32; `CSETM` form).
#[inline(always)]
pub fn ct_eq_u32(a: u32, b: u32) -> u32 {
    let diff = a ^ b;
    // diff | (0 - diff) has MSB set iff diff != 0; arithmetic-shift the
    // sign bit into a full mask.
    (((diff | diff.wrapping_neg()) as i32) >> 31) as u32 ^ u32::MAX
}

/// All-ones if `a < b`, all-zeros otherwise (u32, unsigned compare).
#[inline(always)]
pub fn ct_lt_u32(a: u32, b: u32) -> u32 {
    // Widen to 64-bit, subtract: the u64 result is in [0, 2^32) when a >= b
    // and >= 2^32 (MSB set) when a < b. Negate the MSB into a full mask.
    let d = (a as u64).wrapping_sub(b as u64);
    ((d >> 63) as u32).wrapping_neg()
}

/// Two-way select: `b_true` when mask is all-ones, `b_false` when all-zeros.
#[inline(always)]
pub fn ct_select_u32(mask: u32, b_true: u32, b_false: u32) -> u32 {
    (b_true & mask) | (b_false & !mask)
}

/// Two-way select under an all-ones/all-zeros mask (u64).
#[inline(always)]
pub fn ct_select_u64(mask: u64, b_true: u64, b_false: u64) -> u64 {
    (b_true & mask) | (b_false & !mask)
}

/// Two-way select under an all-ones/all-zeros mask (usize).
#[inline(always)]
pub fn ct_select_usize(mask: usize, b_true: usize, b_false: usize) -> usize {
    (b_true & mask) | (b_false & !mask)
}

/// All-ones if `a == b`, all-zeros otherwise (u64 form, for u64 selects).
#[inline(always)]
pub fn ct_eq_u64_mask(a: u64, b: u64) -> u64 {
    let diff = a ^ b;
    (((diff | diff.wrapping_neg()) as i64) >> 63) as u64 ^ u64::MAX
}

/// All-ones if `a < b` unsigned (u64 form).
#[inline(always)]
pub fn ct_lt_u64_mask(a: u64, b: u64) -> u64 {
    // 128-bit widening subtract: MSB set iff a < b.
    let d = (a as u128).wrapping_sub(b as u128);
    ((d >> 127) as u64).wrapping_neg()
}

/// Branchless 4xu32 quad equality: all-ones iff all four lanes match.
///
/// The s&p&o&g form: per-lane inequality masks are combined (AND) into one
/// quad-wide verdict before any use — a single mask datum, no control edge.
#[inline(always)]
pub fn ct_quad_eq_u32(a: [u32; 4], b: [u32; 4]) -> u32 {
    let m0 = ct_eq_u32(a[0], b[0]);
    let m1 = ct_eq_u32(a[1], b[1]);
    let m2 = ct_eq_u32(a[2], b[2]);
    let m3 = ct_eq_u32(a[3], b[3]);
    m0 & m1 & m2 & m3
}

/// Compact the indices of quads whose four lanes all equal `needle` into
/// `out`, returning the number written.
///
/// The compaction loop is branchless in the data: the write position advances
/// by `mask & 1` (quad mask from [`ct_quad_eq_u32`]) and the written index is
/// a select between the true index and a sink slot. No data-dependent branch
/// exists in the loop body; the only conditional is the loop latch (a fixed
/// trip count over `n_quads`, not data).
///
/// `out` must have capacity for at least `n_quads + 1` elements: slot
/// `n_quads` is the sink that non-matching quads write through (a mask-
/// selected address, so the write stream is input-shape-invariant). Only
/// `out[..count]` is meaningful on return.
///
/// Bounds-check elimination: the per-iteration slice indexing (`data[q*4+k]`)
/// lowered to four `b.eq` panic branches plus one `b.hs` bounds check. The
/// raw-pointer form below (`ptr::add` + unchecked deref) removes them — the
/// in-bounds invariant is discharged once by the `debug_assert` + capacity
/// contract instead of per-lane branches.
///
/// Loop-bound branches: a variable-trip loop needs exactly ONE conditional
/// branch at the latch. The do-while shape below (`loop { body; q += 1; if
/// q == trip { break } }`) achieves that: the zero-trip case is handled by
/// clamping `trip = n_quads.max(1)` in ct arithmetic (never a branch) and
/// pointing `dp` at a stack-local dummy quad via a ct-select on pointers
/// when `n_quads == 0`, so the single guaranteed trip reads valid memory and
/// writes the sink slot `out[0]` (capacity contract still `n_quads + 1`).
/// After the loop the sink slot never leaks into `out[..count]` because
/// `count == 0` there. The one remaining `b.eq` latch branch is the honest,
/// irreducible loop-bound branch count of this kernel.
pub fn filter_quads_eq(data: &[u32], n_quads: usize, needle: u32, out: &mut [u32]) -> usize {
    assert!(
        out.len() > n_quads,
        "out needs n_quads+1 capacity (sink slot)"
    );
    debug_assert!(data.len() >= n_quads * 4);
    let mut count: usize = 0;
    let mut q: usize = 0;
    // zero-mask: all-ones iff n_quads == 0 (ct_eq on usize via u64 form).
    let zero_mask = ct_eq_u64_mask(n_quads as u64, 0) as usize;
    // trip = n_quads.max(1) via ct arithmetic: n + (n == 0).
    let trip = n_quads.wrapping_add(zero_mask & 1);
    // The dummy quad read on the guaranteed n_quads==0 trip is built to NEVER
    // match the needle (every lane = needle ^ 1), so its step is 0 naturally
    // and the trip has the same write-to-sink side effect as a real trip.
    let dummy = [needle ^ 1; 4];
    // ct pointer select: real data when n_quads > 0, dummy when n_quads == 0.
    let dsel = ct_select_usize(zero_mask, dummy.as_ptr() as usize, data.as_ptr() as usize);
    let dp = dsel as *const u32;
    let op = out.as_mut_ptr();
    // Safety: q < trip on every body entry. When n_quads > 0: data.len() >=
    // n_quads*4 (debug_assert) so `dp.add(q*4+k)` stays within `data`, and
    // out.len() >= n_quads+1 covers slot in {count, n_quads}. When
    // n_quads == 0: trip == 1, dp points at the stack-local dummy, and the
    // write lands in out[0] (the sink slot for trip 1), which the capacity
    // contract guarantees.
    unsafe {
        loop {
            let a = [
                *dp.add(q * 4),
                *dp.add(q * 4 + 1),
                *dp.add(q * 4 + 2),
                *dp.add(q * 4 + 3),
            ];
            let mask = ct_quad_eq_u32(a, [needle; 4]);
            // Advance write position by mask&1; select between real slot and sink.
            let step = (mask & 1) as usize;
            let slot = ct_select_usize(step.wrapping_neg(), count, n_quads);
            *op.add(slot) = q as u32;
            count = count.wrapping_add(step);
            q += 1;
            if q == trip {
                break;
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    // Exhaustive over a small u32 domain: every primitive vs reference ops.
    #[test]
    fn exhaustive_ct_eq_lt_select_small_domain() {
        // Domain small enough to be exhaustive pairwise: {0, 1, 2, 7, 42,
        // u32::MAX, u32::MAX-1, 0x8000_0000}.
        let dom: [u32; 8] = [0, 1, 2, 7, 42, u32::MAX, u32::MAX - 1, 0x8000_0000];
        for &a in &dom {
            for &b in &dom {
                assert_eq!(
                    ct_eq_u32(a, b),
                    if a == b { u32::MAX } else { 0 },
                    "ct_eq_u32({a}, {b})"
                );
                assert_eq!(
                    ct_lt_u32(a, b),
                    if a < b { u32::MAX } else { 0 },
                    "ct_lt_u32({a}, {b})"
                );
            }
        }
        for &m in &dom {
            let mask_ok = m == 0 || m == u32::MAX;
            if !mask_ok {
                continue;
            }
            for &t in &dom {
                for &f in &dom {
                    let want = if m == u32::MAX { t } else { f };
                    assert_eq!(ct_select_u32(m, t, f), want);
                    assert_eq!(ct_select_u64(m as u64, t as u64, f as u64), want as u64);
                    assert_eq!(
                        ct_select_usize(m as usize, t as usize, f as usize),
                        want as usize
                    );
                }
            }
        }
        // u64 mask primitives vs reference over the same domain.
        for &a in &dom {
            for &b in &dom {
                assert_eq!(
                    ct_eq_u64_mask(a as u64, b as u64),
                    if a == b { u64::MAX } else { 0 }
                );
                assert_eq!(
                    ct_lt_u64_mask(a as u64, b as u64),
                    if a < b { u64::MAX } else { 0 }
                );
            }
        }
    }

    // Full cross-product over ALL of a tiny domain (0..=15), covering
    // every 4-bit input class to the eq/lt primitives.
    #[test]
    fn exhaustive_ct_eq_lt_full_tiny_domain() {
        for a in 0u32..=15 {
            for b in 0u32..=15 {
                assert_eq!(ct_eq_u32(a, b), if a == b { u32::MAX } else { 0 });
                assert_eq!(ct_lt_u32(a, b), if a < b { u32::MAX } else { 0 });
            }
        }
    }

    // Filter correctness on random corpora vs a scalar reference filter.
    #[test]
    fn filter_matches_reference_on_random_corpora() {
        // Deterministic xorshift corpus; no_rand avoids external RNG deps
        // in this no_std crate.
        let mut s: u64 = 0x243F_6A88_85A3_08D3;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        for trial in 0..64 {
            let n_quads = 1 + (next() % 257) as usize;
            let mut data = Vec::with_capacity(n_quads * 4);
            for _ in 0..n_quads * 4 {
                data.push(next() as u32);
            }
            // Heavily salt the corpus with needles and sentinels so
            // all-match, no-match, and mixed shapes all occur.
            if trial % 4 == 0 {
                for v in data.iter_mut() {
                    *v = 0xDEAD_BEEF;
                }
            } else if trial % 4 == 1 {
                // no-match: poison some lanes with a near-needle.
                for v in data.iter_mut().step_by(5) {
                    *v = 0xDEAD_BEEF ^ 1;
                }
            }
            let needles = [0xDEAD_BEEF, next() as u32, 0, u32::MAX];
            for &needle in &needles {
                let mut out = vec![0u32; n_quads + 1];
                let got = filter_quads_eq(&data, n_quads, needle, &mut out);
                let want: Vec<u32> = (0..n_quads)
                    .filter(|&q| {
                        data[q * 4] == needle
                            && data[q * 4 + 1] == needle
                            && data[q * 4 + 2] == needle
                            && data[q * 4 + 3] == needle
                    })
                    .map(|q| q as u32)
                    .collect();
                assert_eq!(got as usize, want.len(), "trial {trial} needle {needle:#x}");
                assert_eq!(&out[..got], &want[..], "trial {trial} needle {needle:#x}");
            }
        }
    }

    #[test]
    fn filter_all_match_and_no_match() {
        let n = 33;
        let all: Vec<u32> = vec![7; n * 4];
        let mut out = vec![0u32; n + 1];
        let got = filter_quads_eq(&all, n, 7, &mut out);
        assert_eq!(got, n);
        assert_eq!(&out[..got], &(0..n as u32).collect::<Vec<_>>()[..]);

        let none: Vec<u32> = (0..(n * 4) as u32).map(|i| 7 + (i & 1)).collect();
        let got = filter_quads_eq(&none, n, 7, &mut out);
        assert_eq!(got, 0);
    }

    #[test]
    fn filter_sentinel_heavy() {
        // Every other quad is the sentinel-pattern needle; in-between quads
        // are near-needles differing in exactly one lane.
        let n = 64;
        let mut data = vec![0u32; n * 4];
        for q in 0..n {
            if q % 2 == 0 {
                data[q * 4..q * 4 + 4].copy_from_slice(&[0xFF; 4]);
            } else {
                data[q * 4..q * 4 + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFE]);
            }
        }
        let mut out = vec![0u32; n + 1];
        let got = filter_quads_eq(&data, n, 0xFF, &mut out);
        let want: Vec<u32> = (0..n as u32).filter(|q| q % 2 == 0).collect();
        assert_eq!(got, want.len());
        assert_eq!(&out[..got], &want[..]);
    }

    #[test]
    fn quad_mask_requires_all_four_lanes() {
        assert_eq!(ct_quad_eq_u32([1, 2, 3, 4], [1, 2, 3, 4]), u32::MAX);
        assert_eq!(ct_quad_eq_u32([1, 2, 3, 5], [1, 2, 3, 4]), 0);
        assert_eq!(ct_quad_eq_u32([0, 0, 0, 0], [0, 0, 0, 0]), u32::MAX);
    }
}
