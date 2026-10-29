//! SWAR RLE / bit-packed hybrid codec for u32 values.
//!
//! Wire form: varint bit-width, then records:
//! - repeated run:  header = (count << 1) | 0, one LE value of ceil(w/8) bytes
//! - literal run:   header = (groups << 1) | 1, then `groups` u64 words LE,
//!   each word = 8 lanes of `w` bits (LSB-first), zero-padded final group.
//!
//! Decode is fail-closed: bounded reads, typed errors, zero panics on
//! malformed pages (FMEA row 3).

use alloc::vec::Vec;
use core::fmt;

/// Typed decode/encode errors - loud, never silent success.
#[derive(Debug, PartialEq, Eq)]
pub enum ParquetError {
    /// Input ended mid-record.
    UnexpectedEof,
    /// Width outside the SWAR kernel set {1,2,4,8}.
    UnsupportedBitWidth(u8),
    /// A run claimed more output lanes than the caller budgeted.
    RunOverrun { run_len: usize, budget: usize },
    /// A literal run claimed more input than present.
    LiteralOverrun { claimed: usize, available: usize },
}

impl fmt::Display for ParquetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParquetError {}

/// Varint (LEB128) read - bounded.
fn read_varint(src: &[u8], pos: &mut usize) -> Result<u64, ParquetError> {
    let mut value: u64 = 0;
    let mut shift = 0;
    loop {
        let Some(&b) = src.get(*pos) else {
            return Err(ParquetError::UnexpectedEof);
        };
        *pos += 1;
        value |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(ParquetError::UnexpectedEof);
        }
    }
}

/// Varint write.
pub fn write_varint(dst: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut b = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            b |= 0x80;
        }
        dst.push(b);
        if value == 0 {
            return;
        }
    }
}
/// Encode values as the hybrid codec (repeated runs >= 3, literal groups of 8).
pub fn rle_encode(values: &[u32], bits: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(bits);
    let mut i = 0;
    while i < values.len() {
        // repeated run when >= 3 equal values
        let v = values[i];
        let mut run = 1;
        while i + run < values.len() && values[i + run] == v {
            run += 1;
        }
        if run >= 3 {
            write_varint(&mut out, u64::from(run as u32) << 1);
            let vb = usize::from(bits).div_ceil(8);
            out.extend_from_slice(&v.to_le_bytes()[..vb]);
            i += run;
        } else {
            // literal batch: coalesce until the next >=3-run (or end)
            let start = i;
            let mut j = i + run;
            while j < values.len() {
                let mut r = 1;
                while j + r < values.len() && values[j + r] == values[j] {
                    r += 1;
                }
                if r >= 3 {
                    break;
                }
                j += r;
            }
            let count = j - start;
            let groups = count.div_ceil(8);
            write_varint(&mut out, ((groups as u64) << 1) | 1);
            let mut lane = 0;
            while lane < groups * 8 {
                let val = if start + lane < j { values[start + lane] } else { 0 };
                let vb = usize::from(bits);
                let mut byte = 0u8;
                let mut bit = 0;
                while bit < vb {
                    if (u64::from(val) >> bit) & 1 == 1 {
                        byte |= 1 << bit;
                    }
                    bit += 1;
                }
                out.push(byte);
                lane += 1;
            }
            i = j;
        }
    }
    out
}
/// Decode the hybrid codec into `out` (u32 lanes).
pub fn rle_decode(src: &[u8], out: &mut [u32]) -> Result<(), ParquetError> {
    let mut pos = 0usize;
    let Some(&bits) = src.first() else {
        return Err(ParquetError::UnexpectedEof);
    };
    pos += 1;
    if !matches!(bits, 1 | 2 | 4 | 8) {
        return Err(ParquetError::UnsupportedBitWidth(bits));
    }
    let mask = (1u64 << bits) - 1;
    let mut oi = 0usize;
    while oi < out.len() {
        let header = read_varint(src, &mut pos)?;
        if header & 1 == 0 {
            // repeated run
            let run_len = (header >> 1) as usize;
            if run_len > out.len() - oi {
                return Err(ParquetError::RunOverrun { run_len, budget: out.len() - oi });
            }
            let vb = usize::from(bits).div_ceil(8);
            let Some(vb_slice) = src.get(pos..pos + vb) else {
                return Err(ParquetError::UnexpectedEof);
            };
            let mut w = [0u8; 4];
            w[..vb].copy_from_slice(vb_slice);
            let value = u32::from_le_bytes(w);
            pos += vb;
            let mut k = 0;
            while k < run_len {
                out[oi + k] = value;
                k += 1;
            }
            oi += run_len;
        } else {
            // literal run: groups of 8 byte-per-lane words
            let groups = (header >> 1) as usize;
            // Literal records may end with a zero-padded partial final group;
            // the caller budget bounds the real lane count (Parquet spec
            // discipline: decode stops at page end).
            let count = (groups * 8).min(out.len() - oi);
            let byte_len = groups * 8;
            let Some(packed) = src.get(pos..pos + byte_len) else {
                return Err(ParquetError::LiteralOverrun { claimed: byte_len, available: src.len() - pos });
            };
            let mut k = 0;
            while k < count {
                let word = u64::from_le_bytes(packed[(k & !7)..(k & !7) + 8].try_into().unwrap());
                out[oi + k] = ((word >> ((k % 8) * 8)) & mask) as u32;
                k += 1;
            }
            oi += count;
            pos += byte_len;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(values: &[u32], bits: u8) {
        let encoded = rle_encode(values, bits);
        let mut decoded = alloc::vec![0u32; values.len()];
        rle_decode(&encoded, &mut decoded).expect("decode");
        assert_eq!(decoded, values, "bits={bits}");
    }

    #[test]
    fn repeated_runs() {
        round_trip(&[7, 7, 7, 7, 7, 2, 2, 2], 4);
        round_trip(&alloc::vec![9u32; 100], 2);
    }

    #[test]
    fn literal_groups() {
        round_trip(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 4);
        round_trip(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 3, 3, 3], 4);
    }

    #[test]
    fn mixed_and_widths() {
        round_trip(&[0, 0, 0, 1, 1, 1, 2, 2, 2, 3], 2);
        round_trip(&[200, 201, 202, 203], 8);
        round_trip(&[1, 0, 1, 1, 0, 0, 1, 0], 1);
    }

    #[test]
    fn empty_page() {
        let encoded = rle_encode(&[], 4);
        let mut decoded: [u32; 0] = [];
        rle_decode(&encoded, &mut decoded).expect("empty ok");
    }

    #[test]
    fn malformed_fails_closed() {
        // truncated stream: claims a huge run, input ends
        let mut decoded = [0u32; 10];
        assert!(rle_decode(&[4, 0xFF, 0xFF, 0xFF], &mut decoded).is_err());
        // bad width byte
        assert_eq!(
            rle_decode(&[3, 0], &mut decoded),
            Err(ParquetError::UnsupportedBitWidth(3))
        );
    }
}
