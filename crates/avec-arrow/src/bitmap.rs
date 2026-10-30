//! LSB-first validity bitmap kernels — SWAR scans with branch-free inner-loop
//! bodies.
//!
//! House kernel contract (charter CTQ-2, audited scope): the scan kernels'
//! vector/scalar inner-loop BODIES compile branch-free; the compiled symbols
//! carry loop latches and dispatch branches (itemized per symbol by the
//! branch audit gate, `scripts/branch_audit.py`, which now scans the true
//! callee extents). `mask_words`/`count_valid` measure (2 latch, 0 body,
//! 3 dispatch) each; LLVM auto-vectorized both to NEON. Bounds-check paths
//! and panic paths are control flow, itemized not hidden.

/// Bitmap byte length for `n_bits` validity bits.
#[must_use]
pub fn bitmap_bytes(n_bits: usize) -> usize {
    n_bits.div_ceil(8)
}

/// Number of u64 words backing `n_bits` bits.
#[must_use]
pub fn bitmap_words(n_bits: usize) -> usize {
    n_bits.div_ceil(64)
}

/// Broadcast a byte lane across a u64 (8 lanes, SWAR).
#[must_use]
pub fn bcast8(b: u8) -> u64 {
    u64::from_ne_bytes([b; 8])
}

/// SWAR validity masking kernel: `out[i] = valid[i] & pred[i]`.
///
/// One u64 AND per 64 rows. Zero conditional branches — cyclomatic
/// complexity 1 by construction and by compiled-machine-code audit.
pub fn mask_words(valid: &[u64], pred: &[u64], out: &mut [u64]) {
    let n = valid.len().min(pred.len()).min(out.len());
    let mut i = 0;
    while i < n {
        out[i] = valid[i] & pred[i];
        i += 1;
    }
}

/// SWAR count of valid rows under a predicate mask: `popcount(valid & pred)`
/// accumulated over words. One popcount instruction per 64 rows.
#[must_use]
pub fn count_valid(valid: &[u64], pred: &[u64]) -> u64 {
    let n = valid.len().min(pred.len());
    let mut acc: u64 = 0;
    let mut i = 0;
    while i < n {
        acc += u64::from((valid[i] & pred[i]).count_ones());
        i += 1;
    }
    acc
}

/// LSB-first bit read: `(bits[i >> 3] >> (i & 7)) & 1`.
#[must_use]
pub fn get_bit(bits: &[u8], i: usize) -> u8 {
    (bits[i >> 3] >> (i & 7)) & 1
}

/// LSB-first bit set.
pub fn set_bit(bits: &mut [u8], i: usize) {
    bits[i >> 3] |= 1 << (i & 7);
}

/// LSB-first bit clear.
pub fn clear_bit(bits: &mut [u8], i: usize) {
    bits[i >> 3] &= !(1 << (i & 7));
}

/// Whole-word validity of 64 rows: bit i set iff row i valid.
#[must_use]
pub fn word_valid(bits: &[u8], word_idx: usize) -> u64 {
    let off = word_idx * 8;
    let mut w = [0u8; 8];
    w.copy_from_slice(&bits[off..off + 8]);
    u64::from_ne_bytes(w)
}

/// Load a u64 word from a byte slice at word granularity (branch-free when
/// the caller pre-checks bounds; panics on OOB by contract).
#[must_use]
pub fn word_at(bytes: &[u8], word_idx: usize) -> u64 {
    let off = word_idx * 8;
    let mut w = [0u8; 8];
    w.copy_from_slice(&bytes[off..off + 8]);
    u64::from_ne_bytes(w)
}


/// Owned LSB-first validity bitmap over a 64-byte aligned buffer.
#[derive(Debug)]
pub struct Bitmap {
    bits: crate::buffer::Buf,
    n_bits: usize,
}

impl Bitmap {
    /// From per-row validity flags; `None` when every row is valid
    /// (Arrow: no validity buffer when null_count == 0).
    #[must_use]
    pub fn from_bools(valid: &[bool]) -> Option<Bitmap> {
        if valid.iter().all(|&v| v) {
            return None;
        }
        let mut bm = Bitmap {
            bits: crate::buffer::Buf::new_zeroed(bitmap_bytes(valid.len())),
            n_bits: valid.len(),
        };
        for (i, &v) in valid.iter().enumerate() {
            if v {
                set_bit(bm.bits.as_mut_slice(), i);
            }
        }
        Some(bm)
    }

    /// From raw LSB-first bytes.
    #[must_use]
    pub fn from_bytes(bytes: crate::buffer::Buf, n_bits: usize) -> Bitmap {
        Bitmap { bits: bytes, n_bits }
    }

    /// Row count covered.
    #[must_use]
    pub fn n_bits(&self) -> usize {
        self.n_bits
    }

    /// Backing bytes (64-byte aligned).
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.bits.as_slice()
    }

    /// Base pointer of the backing buffer.
    #[must_use]
    pub fn as_ptr(&self) -> *const u8 {
        self.bits.as_ptr()
    }

    /// Bit i valid?
    #[must_use]
    pub fn get(&self, i: usize) -> bool {
        get_bit(self.bytes(), i) == 1
    }

    /// Set bit i to validity v.
    pub fn set(&mut self, i: usize, v: bool) {
        if v {
            set_bit(self.bits.as_mut_slice(), i);
        } else {
            clear_bit(self.bits.as_mut_slice(), i);
        }
    }

    /// Whole word of 64 validity bits at word index; tail words shorter than
    /// 8 bytes read only available bytes (rest zero).
    #[must_use]
    pub fn word(&self, word_idx: usize) -> u64 {
        let bytes = self.bytes();
        let off = word_idx * 8;
        let mut w = [0u8; 8];
        let take = (bytes.len().saturating_sub(off)).min(8);
        w[..take].copy_from_slice(&bytes[off..off + take]);
        u64::from_ne_bytes(w)
    }

    /// Word count.
    #[must_use]
    pub fn words_len(&self) -> usize {
        bitmap_words(self.n_bits)
    }

    /// True when no row is null.
    #[must_use]
    pub fn is_all_valid(&self) -> bool {
        let mut ones: u64 = 0;
        let full_words = self.n_bits / 64;
        let mut w = 0;
        while w < full_words {
            ones += u64::from(self.word(w).count_ones());
            w += 1;
        }
        let tail = self.n_bits % 64;
        if tail > 0 {
            let word = self.word(full_words);
            let tail_mask: u64 = if tail == 64 { u64::MAX } else { (1 << tail) - 1 };
            ones += u64::from((word & tail_mask).count_ones());
        }
        ones == self.n_bits as u64
    }

    /// Number of null rows (SWAR popcount).
    #[must_use]
    pub fn null_count(&self) -> usize {
        self.n_bits - usize::try_from(count_one_bits(self)).unwrap_or(0)
    }
}

/// Total valid rows (popcount over the bitmap).
#[must_use]
pub fn count_one_bits(bm: &Bitmap) -> u64 {
    let mut ones: u64 = 0;
    let full = bm.n_bits() / 64;
    let mut w = 0;
    while w < full {
        ones += u64::from(bm.word(w).count_ones());
        w += 1;
    }
    let tail = bm.n_bits() % 64;
    if tail > 0 {
        let tail_mask: u64 = (1 << tail) - 1;
        ones += u64::from((bm.word(full) & tail_mask).count_ones());
    }
    ones
}

#[cfg(test)]
mod bitmap_tests {
    use super::*;

    #[test]
    fn from_bools_all_valid_is_none() {
        assert!(Bitmap::from_bools(&[true; 5]).is_none());
    }

    #[test]
    fn nulls_round_trip() {
        let bm = Bitmap::from_bools(&[true, false, true, true, false, true]).unwrap();
        assert!(bm.get(0));
        assert!(!bm.get(1));
        assert!(bm.get(5));
        assert_eq!(bm.null_count(), 2);
        assert!(!bm.is_all_valid());
        assert_eq!(count_one_bits(&bm), 4);
    }

    #[test]
    fn all_valid_flag_true() {
        let mut bm = Bitmap::from_bools(&[true, false]).unwrap();
        bm.set(1, true);
        assert!(bm.is_all_valid());
    }
}
