//! NEON silicon kernels — compile-time dispatch over the aarch64 target.
//!
//! `mask_words_neon` computes `out[i] = valid[i] & pred[i]` (the house
//! `bitmap::mask_words` contract) through Advanced SIMD registers. Dispatch
//! is the `#[cfg]` boundary itself:
//!
//! - **aarch64**: the NEON path runs *unconditionally*. AArch64 (Armv8-A)
//!   makes Advanced SIMD/NEON a baseline-mandatory extension — every
//!   conforming aarch64 core implements it, so a runtime feature probe
//!   would be a tautology. The usual probe (`std::arch::is_aarch64_feature_detected!`)
//!   lives in `std` and cgap-arrow is `no_std`, so compile-time selection is
//!   also the only lawful option. It is exact, not a guess.
//! - **everything else**: the portable SWAR kernel (`crate::bitmap::mask_words`)
//!   keeps the identical output contract.

/// SIMD mask kernel: `out[i] = valid[i] & pred[i]` for `i < min(len)` words.
///
/// NEON path (aarch64): the inner loop strides 4 words = 2 q-registers per
/// iteration. The loop *body* is branch-free by construction — `ld1` / `and`
/// vector / `st1`, no compares, no predication — so the branch-audit surface
/// (the inner 4-word loop) carries zero conditional branches inside the body;
/// the only conditional branch is the loop latch itself. Trip-count control
/// flow is the same class the portable kernel carries after LLVM lowers it to
/// a `memcpy` call.
///
/// Tail: sizes not a multiple of 4 words are exact. The scalar tail loop runs
/// OUTSIDE the vector loop over at most 3 remaining words, via raw pointer
/// arithmetic (no bounds-check branches). COMPILED REALITY (audited): LLVM
/// re-vectorizes this tail into a second 8-word NEON loop plus a scalar
/// residue, so the whole function measures 3 latches + 4 dispatch branches,
/// not the "one latch" the source shape suggests. The inner 4-word vector
/// loop body is branch-free; latches/dispatch are itemized by the branch
/// audit gate over the true callee extent.
///
/// Safety of the pointer arithmetic (this function is safe; the `unsafe`
/// blocks below are the intrinsic calls themselves): `n` is the minimum of
/// the three slice lengths, and every address formed is `base + i` with
/// `i < n`, hence in bounds for all three slices. `vld1q_u64`/`vld1q_u64`
/// require only natural element alignment, which `&[u64]` guarantees.
#[cfg(target_arch = "aarch64")]
pub fn mask_words_neon(valid: &[u64], pred: &[u64], out: &mut [u64]) {
    use core::arch::aarch64::{vandq_u64, vld1q_u64, vst1q_u64};

    let n = valid.len().min(pred.len()).min(out.len());
    // SAFETY: all uses below stay within `0..n` and `n <=` each slice length;
    // see the doc comment for the full bound argument.
    unsafe {
        let vp = valid.as_ptr();
        let pp = pred.as_ptr();
        let op = out.as_mut_ptr();

        // Inner vector loop: 4 words per iteration, branch-free body.
        let mut i = 0;
        while i + 4 <= n {
            let v_lo = vld1q_u64(vp.add(i));
            let p_lo = vld1q_u64(pp.add(i));
            let v_hi = vld1q_u64(vp.add(i + 2));
            let p_hi = vld1q_u64(pp.add(i + 2));
            vst1q_u64(op.add(i), vandq_u64(v_lo, p_lo));
            vst1q_u64(op.add(i + 2), vandq_u64(v_hi, p_hi));
            i += 4;
        }

        // Scalar tail, outside the vector loop: at most 3 words, exact,
        // branch-light (one latch, no bounds checks).
        while i < n {
            *op.add(i) = *vp.add(i) & *pp.add(i);
            i += 1;
        }
    }
}

/// Portable fallback (non-aarch64 targets only): identical output contract,
/// delegated to the house SWAR kernel.
#[cfg(not(target_arch = "aarch64"))]
pub fn mask_words_neon(valid: &[u64], pred: &[u64], out: &mut [u64]) {
    crate::bitmap::mask_words(valid, pred, out)
}

#[cfg(test)]
mod silicon_tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Deterministic xorshift64* fill: dense mixed bit patterns, never
    /// degenerate all-zeros/all-ones. Real data, no mock collaborators.
    fn pattern(seed: u64, n: usize) -> Vec<u64> {
        let mut s = seed | 1;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s.wrapping_mul(0x2545_F491_4F6C_DD1D)
            })
            .collect()
    }

    /// Straightforward scalar reference — the spec, one AND per word.
    fn scalar_reference(valid: &[u64], pred: &[u64], n: usize) -> Vec<u64> {
        (0..n).map(|i| valid[i] & pred[i]).collect()
    }

    /// NEON output must be bitwise-equal to the scalar reference. The output
    /// buffer is poisoned first so a missed write (e.g. a skipped tail word)
    /// fails loudly instead of passing against stale zeros.
    fn assert_neon_matches_scalar(n: usize) {
        let valid = pattern(0x9E37_79B9_7F4A_7C15, n);
        let pred = pattern(0xBF58_476D_1CE4_E5B9, n);
        let mut out = vec![0xDEAD_BEEF_DEAD_BEEF; n];
        mask_words_neon(&valid, &pred, &mut out);
        assert_eq!(out, scalar_reference(&valid, &pred, n), "n = {n} words");

        // Cross-kernel agreement: the house portable kernel on the same
        // inputs must produce the same bits.
        let mut portable = vec![0u64; n];
        crate::bitmap::mask_words(&valid, &pred, &mut portable);
        assert_eq!(out, portable, "n = {n} words vs bitmap::mask_words");
    }

    #[test]
    fn neon_matches_scalar_n0() {
        assert_neon_matches_scalar(0);
    }

    #[test]
    fn neon_matches_scalar_n1() {
        assert_neon_matches_scalar(1);
    }

    #[test]
    fn neon_matches_scalar_n63() {
        assert_neon_matches_scalar(63);
    }

    #[test]
    fn neon_matches_scalar_n64() {
        assert_neon_matches_scalar(64);
    }

    #[test]
    fn neon_matches_scalar_n65() {
        assert_neon_matches_scalar(65);
    }

    #[test]
    fn neon_matches_scalar_n8192() {
        assert_neon_matches_scalar(8192);
    }

    /// Receipt that the tests above really executed the NEON intrinsics on
    /// this target — on a non-aarch64 host the suite would still pass (the
    /// fallback satisfies the same contract), so this test names the
    /// dispatched path loudly instead of letting the claim stay implicit.
    #[test]
    fn dispatch_paths_by_target() {
        if cfg!(target_arch = "aarch64") {
            // NEON path active: exercised by the tests in this module.
        } else {
            panic!(
                "non-aarch64 target: mask_words_neon dispatched to the \
                 bitmap::mask_words fallback, not NEON silicon"
            );
        }
    }
}
