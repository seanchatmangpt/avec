//! Fundamental branchless arithmetic, comparison, alignment, SWAR, and memory primitives.
//!
//! Designed for `#![no_std]` high-throughput columnar execution without
//! branch predictor stalls, pipeline flushes, or secret-dependent address patterns.

// --- 1. Arithmetic & Bit-Level Core Primitives ----------------------------

/// Synthesize a 64-bit mask from a boolean condition (`0x00...0` or `0xFF...F`).
#[inline(always)]
pub fn branchless_mask_bool(cond: bool) -> u64 {
    (cond as u64).wrapping_neg()
}

/// Synthesize a 64-bit mask from the sign bit of an i64.
#[inline(always)]
pub fn branchless_mask_sign(x: i64) -> i64 {
    x >> 63
}

/// Branchless minimum between two signed 64-bit integers.
#[inline(always)]
pub fn branchless_min(a: i64, b: i64) -> i64 {
    let diff = a.wrapping_sub(b);
    let mask = diff >> 63;
    b.wrapping_add(diff & mask)
}

/// Branchless maximum between two signed 64-bit integers.
#[inline(always)]
pub fn branchless_max(a: i64, b: i64) -> i64 {
    let diff = a.wrapping_sub(b);
    let mask = diff >> 63;
    a.wrapping_sub(diff & mask)
}

/// Branchless absolute value for signed 64-bit integers.
#[inline(always)]
pub fn branchless_abs(x: i64) -> i64 {
    let mask = x >> 63;
    (x ^ mask).wrapping_sub(mask)
}

/// Branchless clamp to range `[min, max]`.
#[inline(always)]
pub fn branchless_clamp(val: i64, min: i64, max: i64) -> i64 {
    branchless_min(max, branchless_max(min, val))
}

/// Branchless conditional selection (MUX): selects `on_true` if cond is true, else `on_false`.
#[inline(always)]
pub fn branchless_select(cond: bool, on_true: u64, on_false: u64) -> u64 {
    let mask = branchless_mask_bool(cond);
    (on_true & mask) | (on_false & !mask)
}

// --- 2. Equality, Comparison & Zero Detection -----------------------------

/// Evaluates to 1 if `x != 0`, 0 if `x == 0`.
#[inline(always)]
pub fn is_nonzero(x: u64) -> u64 {
    (x | (!x).wrapping_add(1)) >> 63
}

/// Branchless equality: returns 1 if `a == b`, else 0.
#[inline(always)]
pub fn branchless_eq(a: u64, b: u64) -> u64 {
    let diff = a ^ b;
    1 ^ is_nonzero(diff)
}

/// Branchless signum: returns 1 if `x > 0`, -1 if `x < 0`, and 0 if `x == 0`.
#[inline(always)]
pub fn branchless_signum(x: i64) -> i64 {
    (x > 0) as i64 - (x < 0) as i64
}

// --- 3. Bitfield, Alignment & Power-of-Two Mechanics ----------------------

/// Power-of-two verification (`is_power_of_two`).
#[inline(always)]
pub fn is_p2(x: u64) -> bool {
    (x != 0) & ((x & (x.wrapping_sub(1))) == 0)
}

/// Branchless round up to power of two.
#[inline(always)]
pub fn round_up_p2(mut x: u64) -> u64 {
    if x == 0 {
        return 1;
    }
    x = x.wrapping_sub(1);
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x |= x >> 32;
    x.wrapping_add(1)
}

/// Round up to nearest 64-byte boundary.
#[inline(always)]
pub fn align_64(ptr: usize) -> usize {
    (ptr + 63) & !63
}

/// Hardware 64-byte alignment test.
#[inline(always)]
pub fn is_aligned_64(ptr: usize) -> bool {
    (ptr & 63) == 0
}

/// Power-of-2 ring modulo: `index & (CAPACITY - 1)`.
#[inline(always)]
pub fn ring_modulo(index: u64, capacity: usize) -> usize {
    debug_assert!(capacity.is_power_of_two());
    (index & (capacity as u64 - 1)) as usize
}

// --- 4. Bit-Manipulation & SWAR -------------------------------------------

/// Branchless bit extraction.
#[inline(always)]
pub fn get_bit(word: u64, bit_idx: usize) -> u64 {
    (word >> bit_idx) & 1
}

/// Branchless bit insertion.
#[inline(always)]
pub fn set_bit(word: &mut u64, bit_idx: usize, val: bool) {
    let mask = 1u64 << bit_idx;
    *word = (*word & !mask) | ((val as u64) << bit_idx);
}

/// Isolate lowest set bit (`x & -x`).
#[inline(always)]
pub fn lowest_set_bit(x: u64) -> u64 {
    x & (!x).wrapping_add(1)
}

/// Clear lowest set bit (`x & (x - 1)`).
#[inline(always)]
pub fn clear_lowest_set_bit(x: u64) -> u64 {
    x & x.wrapping_sub(1)
}

/// Broadside zero-byte detection across 8 parallel byte lanes in a 64-bit word
/// (Alan Mycroft's SWAR algorithm).
#[inline(always)]
pub fn has_zero_byte(v: u64) -> bool {
    ((v.wrapping_sub(0x0101010101010101)) & !v & 0x8080808080808080) != 0
}

/// Broadside byte-value match: returns mask with MSB set in matching byte lanes.
#[inline(always)]
pub fn match_byte_needle(word: u64, needle: u8) -> u64 {
    let broadcast = (needle as u64) * 0x0101010101010101;
    let xor = word ^ broadcast;
    (xor.wrapping_sub(0x0101010101010101)) & !xor & 0x8080808080808080
}

/// SWAR 64-bit reversal.
#[inline(always)]
pub fn reverse_bits_swar(mut x: u64) -> u64 {
    x = ((x >> 1) & 0x5555555555555555) | ((x & 0x5555555555555555) << 1);
    x = ((x >> 2) & 0x3333333333333333) | ((x & 0x3333333333333333) << 2);
    x = ((x >> 4) & 0x0F0F0F0F0F0F0F0F) | ((x & 0x0F0F0F0F0F0F0F0F) << 4);
    x = ((x >> 8) & 0x00FF00FF00FF00FF) | ((x & 0x00FF00FF00FF00FF) << 8);
    x = ((x >> 16) & 0x0000FFFF0000FFFF) | ((x & 0x0000FFFF0000FFFF) << 16);
    x.rotate_left(32)
}

// --- 5. Memory & Compaction Primitives ------------------------------------

/// Branchless output compaction step: unconditionally stores, then conditionally advances index.
#[inline(always)]
pub fn branchless_compact_step(val: u64, is_valid: bool, out: &mut [u64], write_head: &mut usize) {
    out[*write_head] = val;
    *write_head += is_valid as usize;
}

/// Broadside in-register table lookup (oblivious gather) for 16 entries.
#[inline(always)]
pub fn oblivious_gather_u32(table: &[u32; 16], target_idx: u32) -> u32 {
    let mut out = 0u32;
    let mut i = 0;
    while i < 16 {
        let mask = ((target_idx == i as u32) as u32).wrapping_neg();
        out |= table[i] & mask;
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arithmetic_primitives() {
        assert_eq!(branchless_mask_bool(true), !0);
        assert_eq!(branchless_mask_bool(false), 0);

        assert_eq!(branchless_min(10, 20), 10);
        assert_eq!(branchless_min(20, 10), 10);
        assert_eq!(branchless_min(-5, 5), -5);

        assert_eq!(branchless_max(10, 20), 20);
        assert_eq!(branchless_max(20, 10), 20);
        assert_eq!(branchless_max(-5, 5), 5);

        assert_eq!(branchless_abs(42), 42);
        assert_eq!(branchless_abs(-42), 42);
        assert_eq!(branchless_abs(0), 0);

        assert_eq!(branchless_clamp(5, 10, 20), 10);
        assert_eq!(branchless_clamp(15, 10, 20), 15);
        assert_eq!(branchless_clamp(25, 10, 20), 20);

        assert_eq!(branchless_select(true, 0xAA, 0x55), 0xAA);
        assert_eq!(branchless_select(false, 0xAA, 0x55), 0x55);
    }

    #[test]
    fn test_comparison_and_zero() {
        assert_eq!(is_nonzero(0), 0);
        assert_eq!(is_nonzero(1), 1);
        assert_eq!(is_nonzero(0x8000_0000_0000_0000), 1);

        assert_eq!(branchless_eq(42, 42), 1);
        assert_eq!(branchless_eq(42, 43), 0);

        assert_eq!(branchless_signum(100), 1);
        assert_eq!(branchless_signum(-100), -1);
        assert_eq!(branchless_signum(0), 0);
    }

    #[test]
    fn test_alignment_and_p2() {
        assert!(is_p2(1));
        assert!(is_p2(1024));
        assert!(!is_p2(0));
        assert!(!is_p2(1025));

        assert_eq!(round_up_p2(1000), 1024);
        assert_eq!(round_up_p2(1024), 1024);
        assert_eq!(round_up_p2(0), 1);

        assert_eq!(align_64(0), 0);
        assert_eq!(align_64(1), 64);
        assert_eq!(align_64(64), 64);
        assert_eq!(align_64(65), 128);
        assert!(is_aligned_64(128));
        assert!(!is_aligned_64(129));

        assert_eq!(ring_modulo(1025, 1024), 1);
        assert_eq!(ring_modulo(2048, 1024), 0);
    }

    #[test]
    fn test_bit_and_swar() {
        let mut w = 0u64;
        set_bit(&mut w, 3, true);
        assert_eq!(w, 8);
        assert_eq!(get_bit(w, 3), 1);
        assert_eq!(get_bit(w, 2), 0);

        assert_eq!(lowest_set_bit(0b10100), 0b00100);
        assert_eq!(clear_lowest_set_bit(0b10100), 0b10000);

        assert!(has_zero_byte(0x1234_5600_789A_BCDE));
        assert!(!has_zero_byte(0x1234_5678_9ABC_DEF0));

        let needle_word = 0xAA00_AA00_AA00_AA00;
        let matched = match_byte_needle(needle_word, 0xAA);
        assert_ne!(matched, 0);

        let bits = 0x8000_0000_0000_0001;
        assert_eq!(reverse_bits_swar(bits), 0x8000_0000_0000_0001);
    }

    #[test]
    fn test_memory_and_oblivious() {
        let mut out = [0u64; 4];
        let mut head = 0;
        branchless_compact_step(10, true, &mut out, &mut head);
        branchless_compact_step(20, false, &mut out, &mut head);
        branchless_compact_step(30, true, &mut out, &mut head);
        assert_eq!(head, 2);
        assert_eq!(&out[0..2], &[10, 30]);

        let tbl = [
            10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160,
        ];
        assert_eq!(oblivious_gather_u32(&tbl, 5), 60);
        assert_eq!(oblivious_gather_u32(&tbl, 15), 160);
        assert_eq!(oblivious_gather_u32(&tbl, 16), 0); // Out of domain -> 0
    }
}
