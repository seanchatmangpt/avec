//! SWAR bit-unpacking: 1/2/4/8-bit packed runs to u64 lanes.
//!
//! Per-word kernels are pure arithmetic (CC = 1); drivers are bounded index
//! loops with clamped inner writes (no overread past `out.len()`).

/// Unpack 1-bit lanes: each packed bit becomes a {0,1} u64 lane.
pub fn unpack_1bit(src: &[u64], out: &mut [u64]) {
    let n = out.len();
    let mut si = 0usize;
    let mut oi = 0usize;
    while oi < n {
        let packed = if si < src.len() { src[si] } else { 0 };
        let avail = (n - oi).min(64);
        let mut b = 0;
        while b < avail {
            out[oi + b] = (packed >> b) & 1;
            b += 1;
        }
        si += 1;
        oi += 64;
    }
}

/// Unpack 2-bit lanes: each 2-bit field becomes a {0..3} u64 lane.
pub fn unpack_2bit(src: &[u64], out: &mut [u64]) {
    let n = out.len();
    let mut si = 0;
    let mut oi = 0;
    while oi < n {
        let packed = if si < src.len() { src[si] } else { 0 };
        let avail = (n - oi).min(32);
        let mut l = 0;
        while l < avail {
            out[oi + l] = (packed >> (l * 2)) & 0b11;
            l += 1;
        }
        si += 1;
        oi += 32;
    }
}

/// Unpack 4-bit lanes: each nibble becomes a {0..15} u64 lane.
pub fn unpack_4bit(src: &[u64], out: &mut [u64]) {
    let n = out.len();
    let mut si = 0;
    let mut oi = 0;
    while oi < n {
        let packed = if si < src.len() { src[si] } else { 0 };
        let avail = (n - oi).min(16);
        let mut l = 0;
        while l < avail {
            out[oi + l] = (packed >> (l * 4)) & 0b1111;
            l += 1;
        }
        si += 1;
        oi += 16;
    }
}

/// Unpack 8-bit lanes: each byte becomes a u64 lane.
pub fn unpack_8bit(src: &[u64], out: &mut [u64]) {
    let n = out.len();
    let mut si = 0;
    let mut oi = 0;
    while oi < n {
        let packed = if si < src.len() { src[si] } else { 0 };
        let avail = (n - oi).min(8);
        let mut l = 0;
        while l < avail {
            out[oi + l] = (packed >> (l * 8)) & 0xFF;
            l += 1;
        }
        si += 1;
        oi += 8;
    }
}

/// Bit-unpack by width: dispatch over the four SWAR kernels.
/// Malformed widths return a typed error (fail-closed, FMEA row 3).
pub fn unpack_bits(bits_per_value: u8, src: &[u64], out: &mut [u64]) -> Result<(), crate::ParquetError> {
    match bits_per_value {
        1 => unpack_1bit(src, out),
        2 => unpack_2bit(src, out),
        4 => unpack_4bit(src, out),
        8 => unpack_8bit(src, out),
        other => return Err(crate::ParquetError::UnsupportedBitWidth(other)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bit_lanes() {
        let src = [0b1011u64];
        let mut out = [0u64; 4];
        unpack_1bit(&src, &mut out);
        assert_eq!(out[..4], [1, 1, 0, 1]);
    }

    #[test]
    fn two_bit_lanes() {
        let src = [0b11_10_01_00u64];
        let mut out = [0u64; 4];
        unpack_2bit(&src, &mut out);
        assert_eq!(out[..4], [0, 1, 2, 3]);
    }

    #[test]
    fn four_bit_lanes() {
        let src = [0xDCBA_3210u64];
        let mut out = [0u64; 8];
        unpack_4bit(&src, &mut out);
        assert_eq!(out[..8], [0x0, 0x1, 0x2, 0x3, 0xA, 0xB, 0xC, 0xD]);
    }

    #[test]
    fn eight_bit_lanes() {
        let src = [0x40302010u64];
        let mut out = [0u64; 4];
        unpack_8bit(&src, &mut out);
        assert_eq!(out[..4], [0x10, 0x20, 0x30, 0x40]);
    }

    #[test]
    fn reject_bad_width() {
        let mut out = [0u64; 1];
        assert!(unpack_bits(3, &[], &mut out).is_err());
        assert!(unpack_bits(16, &[], &mut out).is_err());
    }

    #[test]
    fn partial_tail_is_clamped() {
        let src = [u64::MAX];
        let mut out = [0u64; 70];
        unpack_1bit(&src, &mut out);
        assert_eq!(out[0], 1);
        assert_eq!(out[63], 1);
        assert_eq!(out[64], 0); // src[1] absent -> zero-padded lanes
    }
}
