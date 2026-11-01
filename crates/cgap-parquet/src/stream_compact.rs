//! Branchless stream compaction for validity filtering.
//!
//! Replaces conditional branches `if (valid) { out[j++] = val; }` with
//! hardware popcount (`popcnt` / `cnt`) destination offsets and unconditional
//! stores, eliminating branch misprediction penalties on intermediate-selectivity
//! bit streams. Includes ARM64 NEON in-register table shuffle specialization.

/// Compact valid u64 data lanes into a contiguous output buffer without branching.
/// Inner body conditional branch count: 0.
#[inline(always)]
pub fn compact_lanes_branchless(
    values: &[u64; 8],
    mask_byte: u8,
    out: &mut [u64],
    write_head: &mut usize,
) {
    let mut current_offset = *write_head;

    for i in 0..8 {
        let is_valid = ((mask_byte >> i) & 1) as usize;
        let val = values[i];

        // Unconditional store: write either to the active head or overwrite slot
        out[current_offset] = val;

        // Advance strictly by validity bit (0 or 1)
        current_offset += is_valid;
    }

    *write_head = current_offset;
}

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::*;

/// NEON In-Register Byte Table Shuffle Compaction
///
/// # Safety
///
/// No memory-safety precondition on the caller: `values`, `mask_byte`, and
/// `lut_indices` are NEON registers, not pointers, and `vtbl1_u8` is total
/// (out-of-range LUT lane bytes select 0). Marked `unsafe` because the body
/// executes target-gated NEON intrinsics; callers need only the platform
/// gate (`cfg(target_arch = "aarch64")`) this function already carries.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub unsafe fn neon_table_compact_8(
    values: uint8x8_t,
    mask_byte: u8,
    lut_indices: uint8x8_t,
) -> (uint8x8_t, usize) {
    // lut_indices maps the active bits of mask_byte to packed contiguous lane positions
    // vtbl1_u8 performs a deterministic single-cycle in-register shuffle
    let compacted = unsafe { vtbl1_u8(values, lut_indices) };
    let count = mask_byte.count_ones() as usize;
    (compacted, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_lanes_branchless_various_selectivities() {
        let values = [10, 20, 30, 40, 50, 60, 70, 80];
        let mut out = [0u64; 16];

        // Case 1: 0% valid
        let mut head = 0;
        compact_lanes_branchless(&values, 0b0000_0000, &mut out, &mut head);
        assert_eq!(head, 0);

        // Case 2: 100% valid
        head = 0;
        compact_lanes_branchless(&values, 0b1111_1111, &mut out, &mut head);
        assert_eq!(head, 8);
        assert_eq!(&out[0..8], &values);

        // Case 3: Alternating 50% valid (bits 0, 2, 4, 6)
        head = 0;
        compact_lanes_branchless(&values, 0b0101_0101, &mut out, &mut head);
        assert_eq!(head, 4);
        assert_eq!(&out[0..4], &[10, 30, 50, 70]);
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn neon_table_compact_smoke() {
        unsafe {
            let vals = vld1_u8([1, 2, 3, 4, 5, 6, 7, 8].as_ptr());
            // Select elements 0 and 2
            let lut = vld1_u8([0, 2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF].as_ptr());
            let (compacted, count) = neon_table_compact_8(vals, 0b0000_0101, lut);
            assert_eq!(count, 2);
            let mut out = [0u8; 8];
            vst1_u8(out.as_mut_ptr(), compacted);
            assert_eq!(out[0], 1);
            assert_eq!(out[1], 3);
        }
    }
}
