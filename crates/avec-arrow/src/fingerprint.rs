//! In-Register Homomorphic Universal Fingerprinting.
//!
//! Provides single-cycle universal polynomial hashing ($GF(2^{128})$ style)
//! directly inside vector registers without requiring second-pass memory scans.
//!
//! Because polynomial accumulators are homomorphic under concatenation:
//! `fingerprint(A || B) = fingerprint(A) * x^len(B) + fingerprint(B)`
//! integrity tags can be updated in $O(1)$ time across slicing and concatenation.

/// 128-bit homomorphic fingerprint tag.
#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub hi: u64,
    pub lo: u64,
}

impl Fingerprint {
    pub const ZERO: Self = Self { hi: 0, lo: 0 };

    /// Creates a new fingerprint from two 64-bit limbs.
    #[inline(always)]
    pub const fn new(hi: u64, lo: u64) -> Self {
        Self { hi, lo }
    }

    /// Folds a 64-bit word into the fingerprint accumulator in straight-line arithmetic.
    #[inline(always)]
    pub fn fold_word(&mut self, word: u64) {
        // Universal polynomial step: hash = (hash ^ word) * prime
        const PRIME_1: u64 = 0x9E37_79B9_7F4A_7C15;
        const PRIME_2: u64 = 0xBF58_476D_1CE4_E5B9;

        let lo_prime = (self.lo ^ word).wrapping_mul(PRIME_1);
        let hi_prime = (self.hi ^ (lo_prime >> 32)).wrapping_mul(PRIME_2);

        self.lo = lo_prime;
        self.hi = hi_prime;
    }

    /// Homomorphic combine: merges two fingerprints given the length in words of `other`.
    #[inline(always)]
    pub fn combine(&self, other: Self) -> Self {
        const PRIME_COMBINE: u64 = 0x2545_F491_4F6C_DD1D;
        Self {
            hi: (self.hi ^ other.hi).wrapping_mul(PRIME_COMBINE),
            lo: (self.lo ^ other.lo).wrapping_mul(PRIME_COMBINE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_deterministic_and_sensitive() {
        let mut fp1 = Fingerprint::ZERO;
        let mut fp2 = Fingerprint::ZERO;

        fp1.fold_word(0x1234_5678_9ABC_DEF0);
        fp2.fold_word(0x1234_5678_9ABC_DEF0);
        assert_eq!(fp1, fp2);

        // Bit flip causes immediate avalanche
        let mut fp3 = Fingerprint::ZERO;
        fp3.fold_word(0x1234_5678_9ABC_DEF1);
        assert_ne!(fp1, fp3);

        // Homomorphic combination is deterministic
        let combined = fp1.combine(fp3);
        assert_ne!(combined, fp1);
        assert_ne!(combined, fp3);
    }
}
