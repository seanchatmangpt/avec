//! Mutation-detection digest kernels (lane PCL5).
//!
//! A cheap, order-sensitive checksum folded alongside the house SWAR mask
//! kernel: every folded word contributes one rotate + one xor, so any
//! single-bit flip that reaches the folded words changes the digest (the
//! differential test below flips bits at several positions and requires a
//! digest change per flip).
//!
//! THIS IS NOT A CRYPTOGRAPHIC HASH. `fold_word` is a rotate-xor accumulator
//! with no avalanche stage, no keying and no collision resistance. Its only
//! claim: detects ACCIDENTAL mutation (transport bit rot, buffer aliasing,
//! a stale snapshot replayed as fresh). It will lose to an adversary. No
//! external crypto crate is pulled in — the zero-dependency law holds
//! (`lib.rs`: `no_std` + `alloc` only).
//!
//! The digest is a function of the masked OUTPUT words, not raw inputs:
//! flipping a bit in `valid` beneath a zero `pred` bit changes no output
//! bit and therefore no digest bit. That is the AND kernel's semantics,
//! pinned by a test, not a defect.

use crate::primitive::Primitive;

/// Rotate-xor fold: one rotate + one xor per word. Order-sensitive (each
/// word's contribution lands at a different rotation), NOT cryptographic.
#[inline]
fn fold_word(acc: u64, w: u64) -> u64 {
    acc.rotate_left(1) ^ w
}

/// SWAR validity masking kernel with digest accumulation: `out[i] =
/// valid[i] & pred[i]` — byte-for-byte the [`crate::bitmap::mask_words`]
/// loop — while folding every written word into the digest.
///
/// The inner-loop body stays branch-free (one AND, one fold, one store; the
/// fold is pure register arithmetic, zero data-dependent control flow). The
/// loop latch/dispatch shape matches [`crate::bitmap::mask_words`] exactly.
///
/// Length mismatch semantics are identical to [`crate::bitmap::mask_words`]:
/// all three slices fold to their common minimum length.
///
/// Returns the digest of all folded words. Digest law: all-zero output
/// (e.g. all-zero `pred`) folds to `0`. NOT cryptographic — see module docs.
pub fn mask_words_hashed(valid: &[u64], pred: &[u64], out: &mut [u64]) -> u64 {
    let n = valid.len().min(pred.len()).min(out.len());
    let mut acc: u64 = 0;
    let mut i = 0;
    while i < n {
        out[i] = valid[i] & pred[i];
        acc = fold_word(acc, out[i]);
        i += 1;
    }
    acc
}

/// Digest of a frozen [`Primitive<i32>`]: the same rotate-xor fold over the
/// value buffer and the validity words, via the public accessors only
/// (`values()`, `validity().word(w)` / `words_len()`).
///
/// Canonical fold order (fixed so a receipt is replayable):
/// 1. value words — i32 lanes folded pairwise low-half/high-half into one
///    u64 word each (platform-independent: lanes are indexed, never
///    re-sliced as bytes); an odd tail lane zero-extends into the low half.
/// 2. validity words — `Bitmap::word(w)` for `w` in `0..words_len()` (the
///    short tail word zero-fills; see `Bitmap::word`). All-valid arrays
///    carry no validity buffer (Arrow law) and fold no validity words.
///
/// Digest law: empty array folds to `0`. Null slots are written ZERO at
/// build time (`Scalar::ZERO`), so the digest is a deterministic function
/// of the physical buffers. NOT cryptographic — see module docs.
pub fn digest_of(arr: &Primitive<i32>) -> u64 {
    let mut acc: u64 = 0;

    let vals = arr.values();
    let n = vals.len();
    let mut i = 0;
    while i + 1 < n {
        let w = (vals[i] as u32 as u64) | ((vals[i + 1] as u32 as u64) << 32);
        acc = fold_word(acc, w);
        i += 2;
    }
    if i < n {
        acc = fold_word(acc, vals[i] as u32 as u64);
    }

    if let Some(bm) = arr.validity() {
        let mut w = 0;
        while w < bm.words_len() {
            acc = fold_word(acc, bm.word(w));
            w += 1;
        }
    }

    acc
}

#[cfg(test)]
mod digest_tests {
    use super::*;
    use crate::bitmap::mask_words;
    use crate::primitive::PrimitiveBuilder;

    /// valid has 1s at odd bits, pred at {2,3 mod 4}; out = 0x8888... per
    /// word, so flips at odd positions propagate through the AND into out.
    const VALID: u64 = 0xAAAA_AAAA_AAAA_AAAA;
    const PRED: u64 = 0xCCCC_CCCC_CCCC_CCCC;

    #[test]
    fn hashed_mask_matches_mask_words() {
        let valid = [VALID; 4];
        let pred = [PRED; 4];
        let mut got = [0x1234_5678_9ABC_DEF0u64; 4];
        let d = mask_words_hashed(&valid, &pred, &mut got);
        let mut want = [0u64; 4];
        mask_words(&valid, &pred, &mut want);
        assert_eq!(got, want, "hashed variant must not perturb mask output");
        assert_ne!(d, 0, "non-zero output must fold to a non-zero digest");
    }

    #[test]
    fn digest_zero_for_all_zero_pred() {
        let valid = [u64::MAX; 4];
        let pred = [0u64; 4];
        let mut out = [0xF7u64; 4];
        let d = mask_words_hashed(&valid, &pred, &mut out);
        assert_eq!(out, [0u64; 4], "AND law: all-zero pred masks everything");
        assert_eq!(d, 0, "all-zero output folds to zero digest");
    }

    #[test]
    fn single_bit_flip_in_pred_changes_digest() {
        let valid = [VALID; 4];
        let pred = [PRED; 4];
        let mut out = [0u64; 4];
        let base = mask_words_hashed(&valid, &pred, &mut out);

        // Odd positions: valid has a 1 there, so the flip reaches `out`.
        for bit in [1usize, 7, 31, 63] {
            let mut flipped = pred;
            flipped[2] ^= 1 << bit;
            let mut out2 = [0u64; 4];
            let d = mask_words_hashed(&valid, &flipped, &mut out2);
            assert_ne!(d, base, "pred bit {bit} flip must change the digest");
            assert_ne!(out2, out, "pred bit {bit} flip must change the mask");
        }
    }

    #[test]
    fn single_bit_flip_in_valid_changes_digest_where_pred_is_one() {
        let valid = [VALID; 4];
        let pred = [PRED; 4];
        let mut out = [0u64; 4];
        let base = mask_words_hashed(&valid, &pred, &mut out);

        for bit in [3usize, 11, 31, 63] {
            let mut flipped = valid;
            flipped[2] ^= 1 << bit;
            let mut out2 = [0u64; 4];
            let d = mask_words_hashed(&flipped, &pred, &mut out2);
            assert_ne!(d, base, "valid bit {bit} flip must change the digest");
            assert_ne!(out2, out, "valid bit {bit} flip must change the mask");
        }
    }

    /// Pin the masking law: the digest folds the MASKED OUTPUT. A valid-bit
    /// flip beneath a zero pred bit changes no output bit, hence no digest
    /// bit. Asserting the invariance pins intended semantics.
    #[test]
    fn digest_is_a_function_of_the_masked_output() {
        let valid = [VALID; 4];
        let pred = [PRED; 4];
        let mut out = [0u64; 4];
        let base = mask_words_hashed(&valid, &pred, &mut out);

        // Bit 0: valid=0 AND pred=0. Toggling valid cannot change out[2].
        let mut flipped = valid;
        flipped[2] ^= 1;
        let mut out2 = [0u64; 4];
        let d = mask_words_hashed(&flipped, &pred, &mut out2);
        assert_eq!(out2, out, "masked-out flip must not perturb output");
        assert_eq!(d, base, "masked-out flip must not perturb digest");
    }

    /// Roundtrip receipt: digest before vs after a simulated in-flight
    /// mutation differs. Value bytes and validity bits each proven
    /// load-bearing in isolation (Chicago: assert on real folded state).
    #[test]
    fn roundtrip_receipt_detects_value_mutation() {
        let mut b1 = PrimitiveBuilder::<i32>::new();
        b1.push(Some(1));
        b1.push(None);
        b1.push(Some(3));
        b1.push(Some(i32::MAX));
        let a1 = b1.finish();
        let d1 = digest_of(&a1);

        // Same rows, one value byte-stream differs; validity identical.
        let mut b2 = PrimitiveBuilder::<i32>::new();
        b2.push(Some(1));
        b2.push(None);
        b2.push(Some(4)); // mutated lane
        b2.push(Some(i32::MAX));
        let a2 = b2.finish();
        let d2 = digest_of(&a2);

        // Sanity: the two arrays really differ only in that value lane.
        assert_eq!(a1.len(), a2.len());
        assert_eq!(
            a1.validity().map(crate::bitmap::Bitmap::null_count),
            a2.validity().map(crate::bitmap::Bitmap::null_count)
        );
        assert_ne!(d1, d2, "value mutation must change the digest");
    }

    #[test]
    fn roundtrip_receipt_detects_validity_mutation() {
        // Slot 1 holds value 0 in BOTH arrays. Row 1 valid in a2 makes it
        // all-valid, so Arrow DROPS the validity buffer (from_bools -> None):
        // a1 folds one validity word, a2 folds none. Same value bytes, same
        // row count, different digest — validity state is load-bearing.
        let mut b1 = PrimitiveBuilder::<i32>::new();
        b1.push(Some(1));
        b1.push(None);
        b1.push(Some(3));
        let a1 = b1.finish();

        let mut b2 = PrimitiveBuilder::<i32>::new();
        b2.push(Some(1));
        b2.push(Some(0)); // same zero bytes, now valid
        b2.push(Some(3));
        let a2 = b2.finish();

        assert!(a2.validity().is_none(), "all-valid array must drop bitmap");

        assert_eq!(a1.values(), a2.values(), "value buffers must be identical");
        assert_ne!(
            digest_of(&a1),
            digest_of(&a2),
            "validity mutation must change the digest"
        );
    }

    #[test]
    fn digest_is_stable_and_empty_is_zero() {
        let mut b1 = PrimitiveBuilder::<i32>::new();
        b1.push(Some(-5));
        b1.push(Some(7));
        let a = b1.finish();
        assert_eq!(digest_of(&a), digest_of(&a), "digest must be deterministic");

        let empty = PrimitiveBuilder::<i32>::new().finish();
        assert_eq!(digest_of(&empty), 0, "empty array folds to zero");
    }
}
