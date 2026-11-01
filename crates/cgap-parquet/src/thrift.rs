//! Minimal Thrift Compact Protocol — just enough to frame Parquet page
//! headers.
//!
//! Scope, stated honestly: this module implements the compact-protocol
//! subset the page framer needs (`crate::page`) — varint, zigzag, field
//! headers (delta and non-delta forms), i16/i32/i64, binary, bool-in-header,
//! and skip for every compact type including nested structs — and nothing
//! more. It is not a full Thrift runtime: no transports, no message frames,
//! no unions, and containers (list/set/map) are skip-only. Wire
//! compatibility targets the compact protocol shape, not a live Apache
//! Thrift peer; the round-trip and hand-crafted-wire courts below are the
//! proof of self-consistency.
//!
//! Fail-closed law (shared with `crate::rle`): every read is a bounded
//! slice operation, every arithmetic step over attacker-controlled input is
//! `checked_*`/`try_from`, and malformed input surfaces as a typed
//! [`ThriftError`] — never a panic, never a silent success. Struct skipping
//! is depth-guarded so a crafted nesting bomb hits a typed error instead of
//! the stack guard page, and every skip loop strictly consumes bytes so no
//! crafted container can spin unboundedly.
//!
//! Error semantics reuse the `ParquetError` mold (typed enum, `Debug +
//! PartialEq + Display`, `std::error::Error` under the `std` feature,
//! loud-not-quiet) because `rle.rs` — which owns `ParquetError` — is outside
//! this lane's file ownership and its variant set cannot be extended.

use alloc::vec::Vec;
use core::fmt;

/// Typed protocol errors — loud, never silent, never a panic.
#[derive(Debug, PartialEq, Eq)]
pub enum ThriftError {
    /// Input ended mid-record (truncated header, truncated varint).
    UnexpectedEof,
    /// A compact type id outside the defined set {1..=12}, or STOP (0) in a
    /// value position where a value was required.
    InvalidType(u8),
    /// A field id could not be derived (delta overflow of i16).
    InvalidFieldId,
    /// A required field was absent from the struct on the wire.
    MissingRequiredField(i16),
    /// A length/offset arithmetic step overflowed (giant declared length).
    LengthOverflow,
    /// Crafted struct nesting exceeded the skip depth guard.
    DepthExceeded,
}

impl fmt::Display for ThriftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ThriftError {}

/// Compact-protocol type ids.
pub mod ctype {
    pub const STOP: u8 = 0;
    pub const BOOLEAN_TRUE: u8 = 1;
    pub const BOOLEAN_FALSE: u8 = 2;
    pub const BYTE: u8 = 3;
    pub const I16: u8 = 4;
    pub const I32: u8 = 5;
    pub const I64: u8 = 6;
    pub const DOUBLE: u8 = 7;
    pub const BINARY: u8 = 8;
    pub const LIST: u8 = 9;
    pub const SET: u8 = 10;
    pub const MAP: u8 = 11;
    pub const STRUCT: u8 = 12;
}

/// Maximum struct-nesting depth the skip path will walk before reporting
/// [`ThriftError::DepthExceeded`]. Real page headers nest two structs deep;
/// the guard exists so crafted nesting bombs hit a typed error instead of
/// the stack guard page.
const MAX_SKIP_DEPTH: u8 = 32;

/// Varint (LEB128) read — bounded, same discipline as `crate::rle`.
pub fn read_varint(src: &[u8], pos: &mut usize) -> Result<u64, ThriftError> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let Some(&b) = src.get(*pos) else {
            return Err(ThriftError::UnexpectedEof);
        };
        *pos += 1;
        value |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(ThriftError::UnexpectedEof);
        }
    }
}

/// Zigzag encode i32. Unsigned-domain arithmetic: no overflow paths.
#[must_use]
pub fn zigzag_encode_i32(n: i32) -> u32 {
    ((n as u32) << 1) ^ ((n >> 31) as u32)
}

/// Zigzag decode i32 from a raw varint value (low 32 bits used).
#[must_use]
pub fn zigzag_decode_i32(v: u64) -> i32 {
    let z = v as u32;
    ((z >> 1) as i32) ^ (-((z & 1) as i32))
}

/// Zigzag encode i64. Unsigned-domain arithmetic: no overflow paths.
#[must_use]
pub fn zigzag_encode_i64(n: i64) -> u64 {
    ((n as u64) << 1) ^ ((n >> 63) as u64)
}

/// Zigzag decode i64 from a raw varint value.
#[must_use]
pub fn zigzag_decode_i64(v: u64) -> i64 {
    ((v >> 1) as i64) ^ (-((v & 1) as i64))
}

/// Zigzag encode i16 (non-delta field ids on the wire).
#[must_use]
pub fn zigzag_encode_i16(n: i16) -> u16 {
    ((n as u16) << 1) ^ ((n >> 15) as u16)
}

/// Zigzag decode i16 from a raw varint value (low 16 bits used).
#[must_use]
pub fn zigzag_decode_i16(v: u64) -> i16 {
    let z = v as u16;
    ((z >> 1) as i16) ^ (-((z & 1) as i16))
}

/// Write an i16 as zigzag varint (non-delta field ids).
pub fn write_zigzag_i16(dst: &mut Vec<u8>, n: i16) {
    crate::rle::write_varint(dst, u64::from(zigzag_encode_i16(n)));
}

/// Write an i32 as zigzag varint.
pub fn write_zigzag_i32(dst: &mut Vec<u8>, n: i32) {
    crate::rle::write_varint(dst, u64::from(zigzag_encode_i32(n)));
}

/// Write an i64 as zigzag varint.
pub fn write_zigzag_i64(dst: &mut Vec<u8>, n: i64) {
    crate::rle::write_varint(dst, zigzag_encode_i64(n));
}

/// Write a compact field header.
///
/// Delta form when the id gap fits 1..=15 (`(delta << 4) | type`), else the
/// non-delta form (bare type byte, then zigzag-i16 field id as varint).
pub fn write_field_header(dst: &mut Vec<u8>, field_id: i16, prev_id: i16, type_id: u8) {
    let delta = i32::from(field_id) - i32::from(prev_id);
    if (1..=15).contains(&delta) {
        dst.push(((delta as u8) << 4) | type_id);
    } else {
        dst.push(type_id);
        write_zigzag_i16(dst, field_id);
    }
}

/// Write a bool field — the value rides in the header type id
/// (`BOOLEAN_TRUE`/`BOOLEAN_FALSE`), no payload bytes.
pub fn write_bool_field(dst: &mut Vec<u8>, field_id: i16, prev_id: i16, value: bool) {
    let ty = if value {
        ctype::BOOLEAN_TRUE
    } else {
        ctype::BOOLEAN_FALSE
    };
    write_field_header(dst, field_id, prev_id, ty);
}

/// Write an i32 field (zigzag varint payload).
pub fn write_i32_field(dst: &mut Vec<u8>, field_id: i16, prev_id: i16, value: i32) {
    write_field_header(dst, field_id, prev_id, ctype::I32);
    write_zigzag_i32(dst, value);
}

/// Write an i64 field (zigzag varint payload).
pub fn write_i64_field(dst: &mut Vec<u8>, field_id: i16, prev_id: i16, value: i64) {
    write_field_header(dst, field_id, prev_id, ctype::I64);
    write_zigzag_i64(dst, value);
}

/// Write a binary field (varint length + raw bytes).
pub fn write_binary_field(dst: &mut Vec<u8>, field_id: i16, prev_id: i16, value: &[u8]) {
    write_field_header(dst, field_id, prev_id, ctype::BINARY);
    crate::rle::write_varint(dst, value.len() as u64);
    dst.extend_from_slice(value);
}

/// Read a field header. Returns `Ok(None)` at the struct's STOP byte, else
/// `(field_id, type_id)`; `prev_id` is the previous field id (delta form).
pub fn read_field_header(
    src: &[u8],
    pos: &mut usize,
    prev_id: i16,
) -> Result<Option<(i16, u8)>, ThriftError> {
    let Some(&b) = src.get(*pos) else {
        return Err(ThriftError::UnexpectedEof);
    };
    *pos += 1;
    let type_id = b & 0x0F;
    if type_id == ctype::STOP {
        return Ok(None);
    }
    let delta = b >> 4;
    let field_id = if delta != 0 {
        let fid = i32::from(prev_id) + i32::from(delta);
        match i16::try_from(fid) {
            Ok(fid) => fid,
            Err(_) => return Err(ThriftError::InvalidFieldId),
        }
    } else {
        zigzag_decode_i16(read_varint(src, pos)?)
    };
    Ok(Some((field_id, type_id)))
}

/// Read an i32 field payload (zigzag varint).
pub fn read_i32(src: &[u8], pos: &mut usize) -> Result<i32, ThriftError> {
    Ok(zigzag_decode_i32(read_varint(src, pos)?))
}

/// Read an i64 field payload (zigzag varint).
pub fn read_i64(src: &[u8], pos: &mut usize) -> Result<i64, ThriftError> {
    Ok(zigzag_decode_i64(read_varint(src, pos)?))
}

/// Read a raw byte (compact `BYTE` type).
pub fn read_byte(src: &[u8], pos: &mut usize) -> Result<u8, ThriftError> {
    match src.get(*pos) {
        Some(&b) => {
            *pos += 1;
            Ok(b)
        }
        None => Err(ThriftError::UnexpectedEof),
    }
}

/// Read a binary field payload as a borrowed slice of `src` — zero
/// allocation by construction (no `Vec`, no copy).
pub fn read_binary<'a>(src: &'a [u8], pos: &mut usize) -> Result<&'a [u8], ThriftError> {
    let raw = read_varint(src, pos)?;
    let Some(len) = usize::try_from(raw).ok() else {
        return Err(ThriftError::LengthOverflow);
    };
    let end = match pos.checked_add(len) {
        Some(end) => end,
        None => return Err(ThriftError::LengthOverflow),
    };
    match src.get(*pos..end) {
        Some(s) => {
            *pos = end;
            Ok(s)
        }
        None => Err(ThriftError::UnexpectedEof),
    }
}

/// Skip one value of `type_id`, starting at `*pos`. Bounded: every branch
/// either consumes at least one byte or returns, so crafted containers hit
/// the source end (or the depth guard) as typed errors instead of looping
/// unboundedly.
pub fn skip_value(src: &[u8], pos: &mut usize, type_id: u8, depth: u8) -> Result<(), ThriftError> {
    match type_id {
        ctype::BOOLEAN_TRUE | ctype::BOOLEAN_FALSE => Ok(()), // payload rides in the header
        ctype::BYTE => {
            *pos = pos.checked_add(1).ok_or(ThriftError::LengthOverflow)?;
            if *pos > src.len() {
                return Err(ThriftError::UnexpectedEof);
            }
            Ok(())
        }
        ctype::I16 | ctype::I32 | ctype::I64 => read_varint(src, pos).map(|_| ()),
        ctype::DOUBLE => {
            *pos = pos.checked_add(8).ok_or(ThriftError::LengthOverflow)?;
            if *pos > src.len() {
                return Err(ThriftError::UnexpectedEof);
            }
            Ok(())
        }
        ctype::BINARY => read_binary(src, pos).map(|_| ()),
        ctype::LIST | ctype::SET => {
            let size_and_type = read_byte(src, pos)?;
            let elem_type = size_and_type & 0x0F;
            if !valid_type_id(elem_type) {
                return Err(ThriftError::InvalidType(elem_type));
            }
            let size_nibble = size_and_type >> 4;
            let count: u64 = if size_nibble == 15 {
                read_varint(src, pos)?
            } else {
                u64::from(size_nibble)
            };
            for _ in 0..count {
                skip_element(src, pos, elem_type, depth)?;
            }
            Ok(())
        }
        ctype::MAP => {
            let count = read_varint(src, pos)?;
            if count == 0 {
                return Ok(());
            }
            let kv = read_byte(src, pos)?;
            let (k_ty, v_ty) = (kv >> 4, kv & 0x0F);
            if !valid_type_id(k_ty) {
                return Err(ThriftError::InvalidType(k_ty));
            }
            if !valid_type_id(v_ty) {
                return Err(ThriftError::InvalidType(v_ty));
            }
            for _ in 0..count {
                skip_element(src, pos, k_ty, depth)?;
                skip_element(src, pos, v_ty, depth)?;
            }
            Ok(())
        }
        ctype::STRUCT => skip_struct(src, pos, depth),
        other => Err(ThriftError::InvalidType(other)),
    }
}

/// Skip one collection element of `elem_type`. Bools as collection
/// elements are stored as one raw byte (compact protocol rule), unlike
/// bool struct fields whose value rides in the field header.
fn skip_element(src: &[u8], pos: &mut usize, elem_type: u8, depth: u8) -> Result<(), ThriftError> {
    match elem_type {
        ctype::BOOLEAN_TRUE | ctype::BOOLEAN_FALSE => {
            let _ = read_byte(src, pos)?;
            Ok(())
        }
        _ => skip_value(src, pos, elem_type, depth),
    }
}

/// Walk a struct's fields, skipping every value, until STOP. Depth-guarded
/// against crafted nesting bombs; field ids are tracked so delta-encoded
/// headers decode lawfully even while their values are discarded.
fn skip_struct(src: &[u8], pos: &mut usize, depth: u8) -> Result<(), ThriftError> {
    if depth >= MAX_SKIP_DEPTH {
        return Err(ThriftError::DepthExceeded);
    }
    let mut prev_id: i16 = 0;
    loop {
        let Some((field_id, type_id)) = read_field_header(src, pos, prev_id)? else {
            return Ok(());
        };
        prev_id = field_id;
        skip_value(src, pos, type_id, depth + 1)?;
    }
}

/// Validate a raw type id is in the defined set {1..=12}.
fn valid_type_id(t: u8) -> bool {
    (1..=12).contains(&t)
}

/// Parquet `PageType` values (subset this framer names).
pub mod page_type {
    pub const DATA_PAGE: i32 = 0;
    pub const INDEX_PAGE: i32 = 1;
    pub const DICTIONARY_PAGE: i32 = 2;
    pub const DATA_PAGE_V2: i32 = 3;
}

/// Parquet `Encoding` values (subset this framer names).
pub mod encoding {
    pub const PLAIN: i32 = 0;
    pub const PLAIN_DICTIONARY: i32 = 2;
    pub const RLE: i32 = 3;
    pub const BIT_PACKED: i32 = 4;
    pub const DELTA_BINARY_PACKED: i32 = 5;
    pub const DELTA_LENGTH_BYTE_ARRAY: i32 = 6;
    pub const DELTA_BYTE_ARRAY: i32 = 7;
    pub const RLE_DICTIONARY: i32 = 8;
    pub const BYTE_STREAM_SPLIT: i32 = 9;
}

/// Minimal `PageHeader` (parquet.thrift field ids preserved: 1 `type`,
/// 2 `uncompressed_page_size`, 3 `compressed_page_size`,
/// 8 `data_page_header_v2`).
///
/// Fields 4 (crc) and 5-7 (other subheaders) are not represented; the
/// reader skips unknown/unrepresented fields leniently (forward-compat),
/// the writer never emits them.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PageHeader {
    /// `PageType` enum value.
    pub page_type: i32,
    pub uncompressed_page_size: i32,
    pub compressed_page_size: i32,
    pub data_page_header_v2: Option<DataPageHeaderV2>,
}

/// Minimal `DataPageHeaderV2` (parquet.thrift field ids 1-7).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataPageHeaderV2 {
    pub num_values: i32,
    pub num_nulls: i32,
    pub num_rows: i32,
    pub definition_levels_byte_length: i32,
    pub repetition_levels_byte_length: i32,
    /// parquet.thrift default is `true` when the field is absent on the
    /// wire; this framer's writer always emits it explicitly (`false` —
    /// pages are stored uncompressed).
    pub is_compressed: bool,
    /// `Encoding` enum value.
    pub encoding: i32,
}

/// Write a minimal `PageHeader` in Thrift Compact Protocol.
pub fn write_page_header(dst: &mut Vec<u8>, h: &PageHeader) {
    let mut prev: i16 = 0;
    write_i32_field(dst, 1, prev, h.page_type);
    prev = 1;
    write_i32_field(dst, 2, prev, h.uncompressed_page_size);
    prev = 2;
    write_i32_field(dst, 3, prev, h.compressed_page_size);
    prev = 3;
    if let Some(v2) = &h.data_page_header_v2 {
        write_field_header(dst, 8, prev, ctype::STRUCT);
        let mut p: i16 = 0;
        write_i32_field(dst, 1, p, v2.num_values);
        p = 1;
        write_i32_field(dst, 2, p, v2.num_nulls);
        p = 2;
        write_i32_field(dst, 3, p, v2.num_rows);
        p = 3;
        // Real parquet.thrift DataPageHeaderV2 field ids (the previous
        // slots were swapped — internally self-consistent, externally
        // rejected by pyarrow/arrow-rs; caught by the PCL2 external-reader
        // receipt. See docs/PHASE_CHANGE_RECEIPTS.md):
        // 4=encoding, 5=definition_levels_byte_length,
        // 6=repetition_levels_byte_length, 7=is_compressed.
        write_i32_field(dst, 4, p, v2.encoding);
        p = 4;
        write_i32_field(dst, 5, p, v2.definition_levels_byte_length);
        p = 5;
        write_i32_field(dst, 6, p, v2.repetition_levels_byte_length);
        p = 6;
        write_bool_field(dst, 7, p, v2.is_compressed);

        dst.push(ctype::STOP); // end of the V2 substruct
    }
    dst.push(ctype::STOP); // end of PageHeader
}

/// Read a minimal `PageHeader`. Returns the header and the number of bytes
/// consumed (the page body starts at `consumed`).
///
/// Lenient forward-compat: fields outside the minimal schema (4 crc, 5-7
/// other subheaders, any future id) are skipped by wire type, including
/// known-id fields carrying an unexpected wire type. Required fields
/// (1, 2, 3) must be present or the read fails with
/// [`ThriftError::MissingRequiredField`].
pub fn read_page_header(src: &[u8]) -> Result<(PageHeader, usize), ThriftError> {
    let mut pos = 0usize;
    let mut page_type: Option<i32> = None;
    let mut uncompressed: Option<i32> = None;
    let mut compressed: Option<i32> = None;
    let mut v2: Option<DataPageHeaderV2> = None;
    let mut prev: i16 = 0;
    loop {
        let Some((field_id, type_id)) = read_field_header(src, &mut pos, prev)? else {
            break;
        };
        prev = field_id;
        match (field_id, type_id) {
            (1, ctype::I32) => page_type = Some(read_i32(src, &mut pos)?),
            (2, ctype::I32) => uncompressed = Some(read_i32(src, &mut pos)?),
            (3, ctype::I32) => compressed = Some(read_i32(src, &mut pos)?),
            (8, ctype::STRUCT) => v2 = Some(read_data_page_header_v2(src, &mut pos)?),
            (_, other) => skip_value(src, &mut pos, other, 0)?,
        }
    }
    let page_type = match page_type {
        Some(v) => v,
        None => return Err(ThriftError::MissingRequiredField(1)),
    };
    let uncompressed = match uncompressed {
        Some(v) => v,
        None => return Err(ThriftError::MissingRequiredField(2)),
    };
    let compressed = match compressed {
        Some(v) => v,
        None => return Err(ThriftError::MissingRequiredField(3)),
    };
    Ok((
        PageHeader {
            page_type,
            uncompressed_page_size: uncompressed,
            compressed_page_size: compressed,
            data_page_header_v2: v2,
        },
        pos,
    ))
}

/// Read the `DataPageHeaderV2` substruct (field ids 1-7; 6 optional with
/// parquet.thrift default `true`).
fn read_data_page_header_v2(src: &[u8], pos: &mut usize) -> Result<DataPageHeaderV2, ThriftError> {
    let mut num_values: Option<i32> = None;
    let mut num_nulls: Option<i32> = None;
    let mut num_rows: Option<i32> = None;
    let mut def_len: Option<i32> = None;
    let mut rep_len: Option<i32> = None;
    let mut is_compressed = true;
    let mut encoding: Option<i32> = None;
    let mut prev: i16 = 0;
    loop {
        let Some((field_id, type_id)) = read_field_header(src, pos, prev)? else {
            break;
        };
        prev = field_id;
        match (field_id, type_id) {
            (1, ctype::I32) => num_values = Some(read_i32(src, pos)?),
            (2, ctype::I32) => num_nulls = Some(read_i32(src, pos)?),
            (3, ctype::I32) => num_rows = Some(read_i32(src, pos)?),
            (4, ctype::I32) => encoding = Some(read_i32(src, pos)?),
            (5, ctype::I32) => def_len = Some(read_i32(src, pos)?),
            (6, ctype::I32) => rep_len = Some(read_i32(src, pos)?),
            (7, ctype::BOOLEAN_TRUE) => is_compressed = true,
            (7, ctype::BOOLEAN_FALSE) => is_compressed = false,
            (_, other) => skip_value(src, pos, other, 0)?,
        }
    }
    let num_values = num_values.ok_or(ThriftError::MissingRequiredField(1))?;
    let num_nulls = num_nulls.ok_or(ThriftError::MissingRequiredField(2))?;
    let num_rows = num_rows.ok_or(ThriftError::MissingRequiredField(3))?;
    let encoding = encoding.ok_or(ThriftError::MissingRequiredField(4))?;
    let definition_levels_byte_length = def_len.ok_or(ThriftError::MissingRequiredField(5))?;
    let repetition_levels_byte_length = rep_len.ok_or(ThriftError::MissingRequiredField(6))?;
    Ok(DataPageHeaderV2 {
        num_values,
        num_nulls,
        num_rows,
        definition_levels_byte_length,
        repetition_levels_byte_length,
        is_compressed,
        encoding,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_round_trip_edges() {
        for n in [0i32, 1, -1, 63, -64, i32::MAX, i32::MIN] {
            let z = zigzag_encode_i32(n);
            assert_eq!(zigzag_decode_i32(u64::from(z)), n, "i32 {n}");
        }
        for n in [0i64, 1, -1, i64::MAX, i64::MIN] {
            let z = zigzag_encode_i64(n);
            assert_eq!(zigzag_decode_i64(z), n, "i64 {n}");
        }
        for n in [0i16, 1, -1, i16::MAX, i16::MIN] {
            let z = zigzag_encode_i16(n);
            assert_eq!(zigzag_decode_i16(u64::from(z)), n, "i16 {n}");
        }
        // Documented wire forms: 0 -> 0, -1 -> 1, 1 -> 2.
        assert_eq!(zigzag_encode_i32(0), 0);
        assert_eq!(zigzag_encode_i32(-1), 1);
        assert_eq!(zigzag_encode_i32(1), 2);
        assert_eq!(zigzag_encode_i32(i32::MIN), 0xFFFF_FFFF);
        assert_eq!(zigzag_encode_i32(i32::MAX), 0xFFFF_FFFE);
    }

    #[test]
    fn varint_bounded_and_exact() {
        let mut buf = Vec::new();
        crate::rle::write_varint(&mut buf, 0);
        crate::rle::write_varint(&mut buf, 127);
        crate::rle::write_varint(&mut buf, 128);
        crate::rle::write_varint(&mut buf, u64::MAX);
        let mut pos = 0;
        assert_eq!(read_varint(&buf, &mut pos), Ok(0));
        assert_eq!(read_varint(&buf, &mut pos), Ok(127));
        assert_eq!(read_varint(&buf, &mut pos), Ok(128));
        assert_eq!(read_varint(&buf, &mut pos), Ok(u64::MAX));
        assert_eq!(pos, 1 + 1 + 2 + 10);
        // Truncated varint: continuation bit set, stream ends.
        let mut pos = 0;
        assert_eq!(
            read_varint(&[0x80], &mut pos),
            Err(ThriftError::UnexpectedEof)
        );
        // Overlong varint: 11 continuation bytes push shift past 64.
        let overlong = [0xFFu8; 11];
        let mut pos = 0;
        assert_eq!(
            read_varint(&overlong, &mut pos),
            Err(ThriftError::UnexpectedEof)
        );
    }

    #[test]
    fn i32_i64_binary_field_round_trip() {
        let mut buf = Vec::new();
        write_i32_field(&mut buf, 1, 0, -1);
        write_i64_field(&mut buf, 2, 1, i64::MIN);
        write_binary_field(&mut buf, 9, 2, b"page-body");
        let mut pos = 0;
        let (f1, ty1) = read_field_header(&buf, &mut pos, 0)
            .expect("f1")
            .expect("some");
        assert_eq!((f1, ty1), (1, ctype::I32));
        assert_eq!(read_i32(&buf, &mut pos), Ok(-1));
        let (f2, ty2) = read_field_header(&buf, &mut pos, f1)
            .expect("f2")
            .expect("some");
        assert_eq!((f2, ty2), (2, ctype::I64));
        assert_eq!(read_i64(&buf, &mut pos), Ok(i64::MIN));
        let (f3, ty3) = read_field_header(&buf, &mut pos, f2)
            .expect("f3")
            .expect("some");
        assert_eq!((f3, ty3), (9, ctype::BINARY));
        assert_eq!(read_binary(&buf, &mut pos), Ok(&b"page-body"[..]));
        assert_eq!(pos, buf.len(), "consumed exactly the wire");
    }

    #[test]
    fn bool_field_rides_in_header() {
        let mut buf = Vec::new();
        write_bool_field(&mut buf, 6, 5, true);
        write_bool_field(&mut buf, 7, 6, false);
        // No payload bytes: only the two header bytes are on the wire.
        assert_eq!(
            buf,
            alloc::vec![
                (1 << 4) | ctype::BOOLEAN_TRUE,
                (1 << 4) | ctype::BOOLEAN_FALSE
            ]
        );
        let mut pos = 0;
        let (f, ty) = read_field_header(&buf, &mut pos, 5)
            .expect("hdr")
            .expect("some");
        assert_eq!((f, ty), (6, ctype::BOOLEAN_TRUE));
        let (f2, ty2) = read_field_header(&buf, &mut pos, f)
            .expect("hdr")
            .expect("some");
        assert_eq!((f2, ty2), (7, ctype::BOOLEAN_FALSE));
        assert_eq!(pos, buf.len());
    }

    #[test]
    fn field_header_delta_and_non_delta_forms() {
        // Delta form: gap 5 from id 3 to id 8.
        let mut buf = Vec::new();
        write_field_header(&mut buf, 8, 3, ctype::STRUCT);
        assert_eq!(buf, alloc::vec![(5 << 4) | ctype::STRUCT]);
        let mut pos = 0;
        assert_eq!(
            read_field_header(&buf, &mut pos, 3),
            Ok(Some((8, ctype::STRUCT)))
        );

        // Non-delta form: a far field id forces the bare type byte plus a
        // zigzag-i16 id varint.
        let mut buf2 = Vec::new();
        write_field_header(&mut buf2, 300, 3, ctype::I32);
        assert_eq!(buf2[0], ctype::I32, "non-delta form: bare type byte");
        let mut pos = 0;
        assert_eq!(
            read_field_header(&buf2, &mut pos, 3),
            Ok(Some((300, ctype::I32)))
        );
        assert_eq!(pos, buf2.len());
    }

    #[test]
    fn page_header_round_trip() {
        let h = PageHeader {
            page_type: page_type::DATA_PAGE_V2,
            uncompressed_page_size: 0x0102_0304,
            compressed_page_size: 999,
            data_page_header_v2: Some(DataPageHeaderV2 {
                num_values: 4096,
                num_nulls: 0,
                num_rows: 4096,
                definition_levels_byte_length: 0,
                repetition_levels_byte_length: 0,
                is_compressed: false,
                encoding: encoding::RLE,
            }),
        };
        let mut wire = Vec::new();
        write_page_header(&mut wire, &h);
        let (back, consumed) = read_page_header(&wire).expect("parse");
        assert_eq!(back, h);
        assert_eq!(
            consumed,
            wire.len(),
            "header must consume its own wire exactly"
        );

        // Absent V2 subheader is a lawful PageHeader shape (other page
        // types carry different subheaders on fields 5-7).
        let h2 = PageHeader {
            page_type: page_type::DICTIONARY_PAGE,
            uncompressed_page_size: 7,
            compressed_page_size: 7,
            data_page_header_v2: None,
        };
        let mut wire2 = Vec::new();
        write_page_header(&mut wire2, &h2);
        let (back2, consumed2) = read_page_header(&wire2).expect("parse");
        assert_eq!(back2, h2);
        assert_eq!(consumed2, wire2.len());
    }

    #[test]
    fn page_header_skips_unrepresented_fields() {
        // Hand-built wire: the three required i32 fields + a crc (field 4,
        // i32) + a nested struct (field 5) + a binary blob (field 6) + a
        // list of i32 (field 7, extended-size form) — all skipped by wire
        // type — then the real V2 subheader at field 8.
        let mut wire = Vec::new();
        let mut prev = 0i16;
        write_i32_field(&mut wire, 1, prev, page_type::DATA_PAGE_V2);
        prev = 1;
        write_i32_field(&mut wire, 2, prev, 10);
        prev = 2;
        write_i32_field(&mut wire, 3, prev, 10);
        prev = 3;
        write_i32_field(&mut wire, 4, prev, 0x1234_5678); // crc — skipped
        prev = 4;
        // field 5: nested struct {1: i64} — skipped with depth 1.
        write_field_header(&mut wire, 5, prev, ctype::STRUCT);
        prev = 5;
        write_i64_field(&mut wire, 1, 0, -77);
        wire.push(ctype::STOP);
        // field 6: binary — skipped.
        write_binary_field(&mut wire, 6, prev, b"skipped-payload");
        prev = 6;
        // field 7: list of three i32 elements — skipped (extended size).
        write_field_header(&mut wire, 7, prev, ctype::LIST);
        prev = 7;
        wire.push((15 << 4) | ctype::I32);
        crate::rle::write_varint(&mut wire, 3);
        write_zigzag_i32(&mut wire, 5);
        write_zigzag_i32(&mut wire, -6);
        write_zigzag_i32(&mut wire, 7);
        // field 8: the real V2 subheader.
        write_field_header(&mut wire, 8, prev, ctype::STRUCT);
        let mut p = 0i16;
        write_i32_field(&mut wire, 1, p, 4);
        p = 1;
        write_i32_field(&mut wire, 2, p, 0);
        p = 2;
        write_i32_field(&mut wire, 3, p, 4);
        p = 3;
        // IDL-true field order: 4=encoding, 5=def_len, 6=rep_len, 7=compressed.
        write_i32_field(&mut wire, 4, p, encoding::RLE);
        p = 4;
        write_i32_field(&mut wire, 5, p, 0);
        p = 5;
        write_i32_field(&mut wire, 6, p, 0);
        p = 6;
        write_bool_field(&mut wire, 7, p, false);
        wire.push(ctype::STOP);
        wire.push(ctype::STOP);

        let (h, consumed) = read_page_header(&wire).expect("skipping parse");
        assert_eq!(h.page_type, page_type::DATA_PAGE_V2);
        assert_eq!(h.uncompressed_page_size, 10);
        assert_eq!(h.compressed_page_size, 10);
        let v2 = h.data_page_header_v2.expect("v2 present");
        assert_eq!(v2.num_values, 4);
        assert!(!v2.is_compressed);
        assert_eq!(v2.encoding, encoding::RLE);
        assert_eq!(consumed, wire.len());
    }

    #[test]
    fn missing_required_fields_fail_typed() {
        // Only field 2 present: missing 1.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 2, 0, 5);
        wire.push(ctype::STOP);
        assert_eq!(
            read_page_header(&wire),
            Err(ThriftError::MissingRequiredField(1))
        );

        // Fields 1 and 3 present, 2 missing.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 1, 0, 3);
        write_i32_field(&mut wire, 3, 1, 5);
        wire.push(ctype::STOP);
        assert_eq!(
            read_page_header(&wire),
            Err(ThriftError::MissingRequiredField(2))
        );

        // PageHeader complete but the V2 substruct lacks `num_nulls` (2):
        // the V2 reader reports its FIRST missing required field in
        // 1,2,3,4,5,7 order, and that error propagates before the outer
        // header's own (already satisfied) required checks.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 1, 0, page_type::DATA_PAGE_V2);
        write_i32_field(&mut wire, 2, 1, 4);
        write_i32_field(&mut wire, 3, 2, 4);
        write_field_header(&mut wire, 8, 3, ctype::STRUCT);
        write_i32_field(&mut wire, 1, 0, 4);
        wire.push(ctype::STOP);
        wire.push(ctype::STOP);
        assert_eq!(
            read_page_header(&wire),
            Err(ThriftError::MissingRequiredField(2))
        );
    }

    #[test]
    fn truncation_at_every_byte_fails_typed() {
        let h = PageHeader {
            page_type: page_type::DATA_PAGE_V2,
            uncompressed_page_size: 1234,
            compressed_page_size: 1234,
            data_page_header_v2: Some(DataPageHeaderV2 {
                num_values: 100,
                num_nulls: 0,
                num_rows: 100,
                definition_levels_byte_length: 0,
                repetition_levels_byte_length: 0,
                is_compressed: false,
                encoding: encoding::RLE,
            }),
        };
        let mut wire = Vec::new();
        write_page_header(&mut wire, &h);
        for cut in 0..wire.len() {
            let err =
                read_page_header(&wire[..cut]).expect_err(&alloc::format!("cut {cut} must fail"));
            assert!(
                matches!(
                    err,
                    ThriftError::UnexpectedEof | ThriftError::MissingRequiredField(_)
                ),
                "cut {cut}: unexpected error {err:?}"
            );
        }
        // Full wire parses.
        assert!(read_page_header(&wire).is_ok());
    }

    #[test]
    fn crafted_nesting_bomb_hits_depth_guard() {
        // 40 nested single-field structs: (delta 1, STRUCT) * 40 then stops.
        let mut wire = alloc::vec![0x1Cu8; 40];
        wire.extend(alloc::vec![0u8; 41]);
        let mut pos = 0usize;
        let err =
            skip_value(&wire, &mut pos, ctype::STRUCT, 0).expect_err("must refuse deep nesting");
        assert_eq!(err, ThriftError::DepthExceeded);
    }

    #[test]
    fn invalid_type_id_in_delta_header_fails_typed() {
        // Field 3 arrives with compact type 13 (undefined), delta-encoded
        // from field 2 — the reader must fail before any payload read.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 1, 0, page_type::DATA_PAGE_V2);
        write_i32_field(&mut wire, 2, 1, 8);
        wire.push((1 << 4) | 13);
        assert_eq!(read_page_header(&wire), Err(ThriftError::InvalidType(13)));
    }

    #[test]
    fn list_element_type_zero_fails_typed() {
        // Field 4 is a list whose element type nibble is 0 (STOP) — invalid
        // as an element type.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 1, 0, 3);
        write_i32_field(&mut wire, 2, 1, 7);
        write_i32_field(&mut wire, 3, 2, 7);
        wire.push((1 << 4) | ctype::LIST);
        wire.push(0x00); // size nibble 0, elem type 0 -> invalid
        assert_eq!(read_page_header(&wire), Err(ThriftError::InvalidType(0)));
    }

    #[test]
    fn skip_binary_giant_length_fails_closed() {
        // Binary field claiming 2^40 bytes against a tiny source.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 1, 0, 3);
        write_i32_field(&mut wire, 2, 1, 7);
        write_i32_field(&mut wire, 3, 2, 7);
        wire.push((1 << 4) | ctype::BINARY);
        crate::rle::write_varint(&mut wire, 1u64 << 40);
        assert_eq!(read_page_header(&wire), Err(ThriftError::UnexpectedEof));
    }

    #[test]
    fn bool_list_elements_are_raw_bytes() {
        // A list of 3 bools as a skipped unknown field: extended-size form,
        // elem type BOOLEAN_TRUE; each element is one raw byte on the wire
        // (collection bools), so exactly 3 bytes are consumed.
        let mut wire = Vec::new();
        write_i32_field(&mut wire, 1, 0, 3);
        write_i32_field(&mut wire, 2, 1, 7);
        write_i32_field(&mut wire, 3, 2, 7);
        wire.push((1 << 4) | ctype::LIST);
        wire.push((15 << 4) | ctype::BOOLEAN_TRUE);
        crate::rle::write_varint(&mut wire, 3);
        wire.extend_from_slice(&[0x01, 0x00, 0x01]);
        wire.push(ctype::STOP);
        let (h, consumed) = read_page_header(&wire).expect("bool list skips as raw bytes");
        assert_eq!(consumed, wire.len());
        assert_eq!(h.uncompressed_page_size, 7);
    }
}
