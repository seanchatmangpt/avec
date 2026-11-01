//! Tight bit-packing: w-bit lanes (1..=8) packed LSB-first into bytes.
//!
//! Layout matches Parquet bit-packed encoding: value `i` occupies bit range
//! `[i*w, (i+1)*w)` of the byte stream, bit 0 of byte 0 first. The final
//! byte is zero-padded in its high bits. Unlike [`crate::rle`] (one byte per
//! lane inside literal groups) this is the real width-packed form.
//!
//! Decode is fail-closed: unsupported widths are a typed
//! [`ParquetError::UnsupportedBitWidth`]; any mismatch between the byte
//! slice and the exact byte requirement of `out.len()` lanes — too few
//! bytes, or trailing garbage beyond the requirement — is a typed
//! [`ParquetError::LiteralOverrun`] naming both counts. Decode writes
//! exactly `out.len()` lanes, never more.

use crate::rle::ParquetError;
use alloc::vec::Vec;

/// Pack `values` as tight `w`-bit lanes, LSB-first.
///
/// Values are masked to `w` bits (Parquet packing semantics: the declared
/// bit width bounds the value domain). Returns exactly
/// `ceil(values.len() * w / 8)` bytes, zero-padded in the final byte.
///
/// Widths outside `1..=8` return an empty vector; the paired
/// [`unpack_bits_tight`] rejects such widths with a typed error, so an
/// invalid-width packing can never decode as its input values (loud at the
/// boundary that carries a typed error, never a panic here).
pub fn pack_bits(values: &[u32], w: u8) -> Vec<u8> {
    if w == 0 || w > 8 {
        return Vec::new();
    }
    let wb = u32::from(w);
    let mask = if w == 8 { u32::MAX } else { (1u32 << w) - 1 };
    let mut out = Vec::with_capacity((values.len() * usize::from(w)).div_ceil(8));
    let mut acc: u64 = 0;
    let mut nbits: u32 = 0;
    for &v in values {
        acc |= u64::from(v & mask) << nbits;
        nbits += wb;
        while nbits >= 8 {
            out.push((acc & 0xFF) as u8);
            acc >>= 8;
            nbits -= 8;
        }
    }
    if nbits > 0 {
        out.push((acc & 0xFF) as u8);
    }
    out
}

/// Unpack exactly `out.len()` lanes from tight `w`-bit packing, LSB-first.
///
/// Fail-closed contract:
/// - `w` outside `1..=8` -> [`ParquetError::UnsupportedBitWidth`].
/// - `bytes.len()` must equal exactly `ceil(out.len() * w / 8)`. Fewer
///   bytes, or trailing bytes beyond that requirement (garbage trailing a
///   claim the output budget does not admit), are
///   [`ParquetError::LiteralOverrun`] with `claimed` = the exact byte
///   requirement and `available` = `bytes.len()`.
///
/// On success `out` holds exactly `out.len()` decoded lanes.
pub fn unpack_bits_tight(bytes: &[u8], w: u8, out: &mut [u32]) -> Result<(), ParquetError> {
    if w == 0 || w > 8 {
        return Err(ParquetError::UnsupportedBitWidth(w));
    }
    let wb = u32::from(w);
    // u64 arithmetic: the product cannot overflow before the bound check.
    let required = (out.len() as u64 * u64::from(w)).div_ceil(8);
    if bytes.len() as u64 != required {
        return Err(ParquetError::LiteralOverrun {
            claimed: required as usize,
            available: bytes.len(),
        });
    }
    let mask = (1u64 << wb) - 1;
    let mut acc: u64 = 0;
    let mut nbits: u32 = 0;
    let mut bi = 0usize;
    for lane in out.iter_mut() {
        while nbits < wb {
            acc |= u64::from(bytes[bi]) << nbits;
            bi += 1;
            nbits += 8;
        }
        *lane = (acc & mask) as u32;
        acc >>= wb;
        nbits -= wb;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unpack::{unpack_1bit, unpack_2bit, unpack_4bit, unpack_8bit};
    use alloc::vec;

    /// Deterministic LCG (xorshift-free, no deps): stable test vectors.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn value(&mut self, w: u8) -> u32 {
            let mask = if w == 8 { 0xFFu64 } else { (1u64 << w) - 1 };
            (self.next() & mask) as u32
        }
    }

    fn round_trip(values: &[u32], w: u8) {
        let packed = pack_bits(values, w);
        assert_eq!(
            packed.len(),
            (values.len() * usize::from(w)).div_ceil(8),
            "packed byte count, w={w}"
        );
        let mut decoded = alloc::vec![0u32; values.len()];
        unpack_bits_tight(&packed, w, &mut decoded).expect("decode");
        assert_eq!(decoded, values, "w={w}");
    }

    #[test]
    fn known_answer_w3() {
        // lanes 5,3,7,2 at 3 bits, LSB-first: stream bits
        // 101 011 111 010 -> byte0 = 0xDD, byte1 (padded) = 0x05.
        let packed = pack_bits(&[5, 3, 7, 2], 3);
        assert_eq!(packed, vec![0xDD, 0x05]);
        let mut out = [0u32; 4];
        unpack_bits_tight(&packed, 3, &mut out).expect("decode");
        assert_eq!(out, [5, 3, 7, 2]);
    }

    #[test]
    fn known_answer_w1() {
        let packed = pack_bits(&[1, 1, 0, 1], 1);
        assert_eq!(packed, vec![0b1011]);
        let mut out = [0u32; 4];
        unpack_bits_tight(&packed, 1, &mut out).expect("decode");
        assert_eq!(out, [1, 1, 0, 1]);
    }

    #[test]
    fn round_trip_all_widths_all_sizes() {
        for w in 1..=8u8 {
            for &n in &[0usize, 1, 7, 8, 9, 64, 1000] {
                let mut rng = Lcg(0x5EED_0000 + u64::from(w) * 7919 + n as u64);
                let values: Vec<u32> = (0..n).map(|_| rng.value(w)).collect();
                round_trip(&values, w);
            }
        }
    }

    #[test]
    fn round_trip_extreme_values() {
        // min and max of each width at a size that straddles byte/group
        // boundaries in every width.
        for w in 1..=8u8 {
            // In-domain extremes: width w bounds values to w bits.
            let max = (1u32 << w) - 1;
            let values = alloc::vec![0u32, max, 0, max, 1, max - 1, 0, max, 0, 1];
            round_trip(&values, w);
        }
    }

    /// Load a byte stream into LE u64 words (zero-padded final word) so the
    /// SWAR kernels (which take `&[u64]`) see the same bit positions.
    fn le_words(bytes: &[u8]) -> Vec<u64> {
        let mut words = alloc::vec![0u64; bytes.len().div_ceil(8)];
        for (i, chunk) in bytes.chunks(8).enumerate() {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            words[i] = u64::from_le_bytes(buf);
        }
        words
    }

    #[test]
    fn swar_cross_check() {
        // For w in {1,2,4,8} a lane sits at bit [i*w, (i+1)*w) of the byte
        // stream; LE-loading bytes into u64 words preserves bit positions,
        // so unpack_Xbit lanes must equal unpack_bits_tight lanes exactly.
        let n = 100usize; // not a multiple of 8/16/32/64: exercises tail
        for (w, swar) in [
            (1u8, &unpack_1bit as &dyn Fn(&[u64], &mut [u64])),
            (2, &unpack_2bit),
            (4, &unpack_4bit),
            (8, &unpack_8bit),
        ] {
            let mut rng = Lcg(0xC0FFEE + u64::from(w));
            let values: Vec<u32> = (0..n).map(|_| rng.value(w)).collect();
            let packed = pack_bits(&values, w);
            let words = le_words(&packed);

            let mut tight = alloc::vec![0u32; n];
            unpack_bits_tight(&packed, w, &mut tight).expect("tight decode");

            let mut lanes = alloc::vec![0u64; n];
            swar(&words, &mut lanes);

            for i in 0..n {
                assert_eq!(
                    u64::from(tight[i]),
                    lanes[i],
                    "w={w} lane {i}: tight vs SWAR"
                );
            }
        }
    }

    #[test]
    fn unsupported_widths_fail_closed() {
        let mut out = [0u32; 4];
        assert_eq!(
            unpack_bits_tight(&[0u8; 4], 0, &mut out),
            Err(ParquetError::UnsupportedBitWidth(0))
        );
        assert_eq!(
            unpack_bits_tight(&[0u8; 4], 9, &mut out),
            Err(ParquetError::UnsupportedBitWidth(9))
        );
        assert_eq!(
            unpack_bits_tight(&[0u8; 4], 255, &mut out),
            Err(ParquetError::UnsupportedBitWidth(255))
        );
        // pack side cannot carry a typed error: loud-empty, and the pairing
        // decode refuses the width, so it can never decode as its input.
        assert_eq!(pack_bits(&[1, 2, 3], 0), Vec::<u8>::new());
        assert_eq!(pack_bits(&[1, 2, 3], 9), Vec::<u8>::new());
    }

    #[test]
    fn insufficient_bytes_overrun() {
        // 10 lanes x 3 bits = 30 bits -> 4 bytes required; 3 is short.
        let mut out = [0u32; 10];
        let bytes = [0u8; 3];
        assert_eq!(
            unpack_bits_tight(&bytes, 3, &mut out),
            Err(ParquetError::LiteralOverrun {
                claimed: 4,
                available: 3
            })
        );
    }

    #[test]
    fn trailing_garbage_refused() {
        // Same 4-byte requirement; a 5th trailing byte is refused, not
        // silently consumed.
        let mut out = [0u32; 10];
        let mut bytes = [0u8; 5];
        bytes[4] = 0xFF; // garbage that would corrupt if honored
        assert_eq!(
            unpack_bits_tight(&bytes, 3, &mut out),
            Err(ParquetError::LiteralOverrun {
                claimed: 4,
                available: 5
            })
        );
    }

    #[test]
    fn empty_output_strict() {
        let mut out: [u32; 0] = [];
        assert_eq!(unpack_bits_tight(&[], 4, &mut out), Ok(()));
        // Nonempty bytes with a zero-lane budget is trailing garbage.
        assert_eq!(
            unpack_bits_tight(&[7], 4, &mut out),
            Err(ParquetError::LiteralOverrun {
                claimed: 0,
                available: 1
            })
        );
    }

    #[test]
    fn values_masked_to_width() {
        // Declared width bounds the domain: high bits are masked (Parquet
        // packing semantics), documented and asserted here.
        let packed = pack_bits(&[0x1F], 4);
        assert_eq!(packed, vec![0x0F]);
        let mut out = [0u32; 1];
        unpack_bits_tight(&packed, 4, &mut out).expect("decode");
        assert_eq!(out, [0x0F]);
    }
}
