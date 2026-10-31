//! Aligned zero-decode pages: `DataPageV2` whose body is *already* the
//! Arrow payload layout.
//!
//! Scope, stated honestly: a sibling to [`crate::page`]'s minimal V2
//! framer, with the RLE value codec replaced by nothing at all — the page
//! body is UNCOMPRESSED (`is_compressed = false`) PLAIN
//! ([`crate::thrift::encoding::PLAIN`]) fixed-width one-byte values, so the
//! payload is byte-for-byte the Arrow `FixedSizeBinary(1)` buffer a reader
//! materializes. Decode is a header skip plus a bounds-checked
//! `copy_from_slice` — no unpacking, no run expander, no intermediate
//! buffer.
//!
//! Real parquet.thrift field layout: this module assembles its page header
//! from [`crate::thrift`] primitives (`write_i32_field`, `write_bool_field`,
//! `read_field_header`, `skip_value`, ...) rather than calling
//! [`crate::thrift::write_page_header`], because that composite writer
//! emits the `DataPageHeaderV2` tail in slots **swapped relative to the
//! real parquet.thrift** (verified against the thrift-generated
//! `DataPageHeaderV2` codec in arrow-rs 53.4.1 `format.rs`):
//!
//! ```text
//! real parquet.thrift DataPageHeaderV2:       crate::thrift::PageHeader emits:
//!   4: required i32  encoding                   4: definition_levels_byte_length
//!   5: required i32  definition_levels_byte_len 5: repetition_levels_byte_length
//!   6: required i32  repetition_levels_byte_len 6: is_compressed (bool wire)
//!   7: optional bool is_compressed (=true)      7: encoding
//! ```
//!
//! The crate's own reader maps the same swapped slots, so crate-internal
//! round-trips pass while every external reader refuses the pages at thrift
//! deserialization (pyarrow 23.0.1: `Couldn't deserialize thrift:
//! TProtocolException: Invalid data — Deserializing page header failed.`,
//! because the required i32 at id 6/7 meets a bool wire type or vice
//! versa). Measured this session: a page written with the swapped layout
//! was rejected by pyarrow 23.0.1 and by arrow-rs 53.4.1; the real-IDL
//! layout below reads back byte-EXACT in both. The swap lives in
//! `crate::thrift` (outside this module's lane ownership) — reported to
//! the coordinator as a lane finding.
//!
//! Wire law (interop reality check, honored by construction): a Parquet
//! page is header + body, contiguous. Standard readers locate the body at
//! header end and consume `compressed_page_size` bytes as values. Any
//! padding inserted between header and body (the 64 B-aligned variant) is
//! consumed as value bytes by every standard reader and corrupts the tail
//! of the payload — padding is KNOWN-SUSPECT for interop and this module's
//! pages carry none.
//!
//! Error mapping (typed, documented once here; the enum is owned by
//! `crate::rle` and cannot be extended): mirrors [`crate::page`] —
//! - header that cannot be parsed (truncation, unknown compact type, bad
//!   field id, missing required field, nesting bomb): every [`ThriftError`]
//!   collapses to [`ParquetError::UnexpectedEof`];
//! - page shape outside the aligned framer (non-V2 page type, missing V2
//!   subheader, non-PLAIN encoding, `is_compressed != false` — absent
//!   counts as `true` per the parquet.thrift default, non-zero level byte
//!   lengths, negative `num_nulls`/`num_rows`): the offending shape value
//!   rides [`ParquetError::UnsupportedBitWidth`]; missing V2 subheader is
//!   [`ParquetError::UnexpectedEof`];
//! - header claims exceeding the budget (`num_values > out.len()`, or a
//!   negative lane claim): [`ParquetError::RunOverrun`];
//! - body shorter than the header's claim (truncated body):
//!   [`ParquetError::UnexpectedEof`].

use crate::rle::ParquetError;
use crate::thrift::{
    ctype, encoding, page_type, read_field_header, read_i32, skip_value, write_bool_field,
    write_field_header, write_i32_field, ThriftError,
};
use alloc::vec::Vec;

/// Saturate a byte/lane count into the header's i32 envelope. Pages over
/// `i32::MAX` bytes are outside this framer's envelope; the header then
/// reports the saturation ceiling rather than wrapping silently. Same
/// discipline as [`crate::page`].
fn size_i32(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// Parsed `DataPageHeaderV2` (real parquet.thrift ids). `is_compressed`
/// is `None` when absent on the wire (parquet.thrift default: `true`).
struct AlignedV2 {
    num_values: Option<i32>,
    num_nulls: Option<i32>,
    num_rows: Option<i32>,
    encoding: Option<i32>,
    def_len: Option<i32>,
    rep_len: Option<i32>,
    is_compressed: Option<bool>,
}

/// Write the real-parquet.thrift `PageHeader` for an aligned V2 page:
/// fields 1/2/3 (type, sizes), field 8 `DataPageHeaderV2` substruct with
/// the real ids `4 = encoding`, `5 = definition_levels_byte_length`,
/// `6 = repetition_levels_byte_length`, `7 = is_compressed` (explicit
/// `false`), then STOP/STOP. Field ids ascend; delta headers apply.
fn write_aligned_header(dst: &mut Vec<u8>, num_values: usize, num_rows: u32, body_len: usize) {
    let mut prev: i16 = 0;
    write_i32_field(dst, 1, prev, page_type::DATA_PAGE_V2);
    prev = 1;
    write_i32_field(dst, 2, prev, size_i32(body_len));
    prev = 2;
    write_i32_field(dst, 3, prev, size_i32(body_len));
    prev = 3;
    // Field 8: DataPageHeaderV2 substruct.
    write_field_header(dst, 8, prev, ctype::STRUCT);
    let mut p: i16 = 0;
    write_i32_field(dst, 1, p, size_i32(num_values));
    p = 1;
    write_i32_field(dst, 2, p, 0); // num_nulls: required column
    p = 2;
    write_i32_field(dst, 3, p, i32::try_from(num_rows).unwrap_or(i32::MAX));
    p = 3;
    write_i32_field(dst, 4, p, encoding::PLAIN); // real idl: 4 = encoding
    p = 4;
    write_i32_field(dst, 5, p, 0); // definition_levels_byte_length
    p = 5;
    write_i32_field(dst, 6, p, 0); // repetition_levels_byte_length
    p = 6;
    write_bool_field(dst, 7, p, false); // is_compressed, explicit
    dst.push(ctype::STOP); // end V2 substruct
    dst.push(ctype::STOP); // end PageHeader
}

/// Read the outer PageHeader (fields 1/2/3 + optional field-8 substruct).
/// Fields outside this minimal schema are skipped by wire type
/// (forward-compat). Returns `(page_type, uncompressed, compressed, v2,
/// consumed)`; the page body starts at `consumed`.
fn read_page_header_outer(
    src: &[u8],
) -> Result<(i32, i32, i32, Option<AlignedV2>, usize), ThriftError> {
    let mut pos = 0usize;
    let mut page_type_v: Option<i32> = None;
    let mut uncompressed: Option<i32> = None;
    let mut compressed: Option<i32> = None;
    let mut v2: Option<AlignedV2> = None;
    let mut prev: i16 = 0;
    loop {
        let Some((field_id, type_id)) = read_field_header(src, &mut pos, prev)? else {
            break;
        };
        prev = field_id;
        match (field_id, type_id) {
            (1, ctype::I32) => page_type_v = Some(read_i32(src, &mut pos)?),
            (2, ctype::I32) => uncompressed = Some(read_i32(src, &mut pos)?),
            (3, ctype::I32) => compressed = Some(read_i32(src, &mut pos)?),
            (8, ctype::STRUCT) => v2 = Some(read_v2_substruct(src, &mut pos)?),
            (_, other) => skip_value(src, &mut pos, other, 0)?,
        }
    }
    let page_type_v = page_type_v.ok_or(ThriftError::MissingRequiredField(1))?;
    let uncompressed = uncompressed.ok_or(ThriftError::MissingRequiredField(2))?;
    let compressed = compressed.ok_or(ThriftError::MissingRequiredField(3))?;
    Ok((page_type_v, uncompressed, compressed, v2, pos))
}

/// DataPageHeaderV2 substruct, real parquet.thrift ids: 1 num_values,
/// 2 num_nulls, 3 num_rows, 4 encoding, 5 def_len, 6 rep_len,
/// 7 is_compressed (optional bool, absent => default true).
fn read_v2_substruct(src: &[u8], pos: &mut usize) -> Result<AlignedV2, ThriftError> {
    let mut f = AlignedV2 {
        num_values: None,
        num_nulls: None,
        num_rows: None,
        encoding: None,
        def_len: None,
        rep_len: None,
        is_compressed: None,
    };
    let mut prev: i16 = 0;
    loop {
        let Some((field_id, type_id)) = read_field_header(src, pos, prev)? else {
            break;
        };
        prev = field_id;
        match (field_id, type_id) {
            (1, ctype::I32) => f.num_values = Some(read_i32(src, pos)?),
            (2, ctype::I32) => f.num_nulls = Some(read_i32(src, pos)?),
            (3, ctype::I32) => f.num_rows = Some(read_i32(src, pos)?),
            (4, ctype::I32) => f.encoding = Some(read_i32(src, pos)?),
            (5, ctype::I32) => f.def_len = Some(read_i32(src, pos)?),
            (6, ctype::I32) => f.rep_len = Some(read_i32(src, pos)?),
            (7, ctype::BOOLEAN_TRUE) => f.is_compressed = Some(true),
            (7, ctype::BOOLEAN_FALSE) => f.is_compressed = Some(false),
            (_, other) => skip_value(src, pos, other, 0)?,
        }
    }
    Ok(f)
}

/// Write a complete aligned data page: real-parquet.thrift `PageHeader`
/// (assembled from [`crate::thrift`] primitives) followed by `values`
/// verbatim as the page body.
///
/// Wire shape: `page_type = DATA_PAGE_V2`, `num_values = values.len()`,
/// `num_nulls = 0`, real-idl `4 = encoding = PLAIN`, `5/6 = level byte
/// lengths = 0`, `7 = is_compressed = false` (explicit). No header/body
/// padding — the body starts at header end, the only offset every standard
/// reader agrees on.
///
/// `num_rows` labels the page (`num_rows >= values.len()` is lawful for a
/// writer that knows more about its own layout; this framer treats rows ==
/// values, same as [`crate::page::write_data_page_v2`]).
pub fn write_plain_page(values: &[u8], num_rows: u32) -> Vec<u8> {
    let mut out = Vec::new();
    write_aligned_header(&mut out, values.len(), num_rows, values.len());
    out.extend_from_slice(values);
    out
}

/// Decode an aligned page directly into `out`: header skip plus a single
/// bounds-checked `copy_from_slice` of the body. Zero intermediate storage
/// — the body is already the payload layout.
///
/// Contract (mirrors [`crate::page::decode_page_into`]):
/// - `Ok(num_values)` — the page's `num_values` bytes were written to
///   `out[0..num_values]`; `out[num_values..]` is untouched (a budget
///   larger than the page is lawful; a smaller budget is
///   [`ParquetError::RunOverrun`]).
/// - Typed error on corrupt/truncated headers, unsupported page shape, or
///   a truncated body — never a panic on external input.
pub fn aligned_decode(page: &[u8], out: &mut [u8]) -> Result<usize, ParquetError> {
    // 1. Header parse: any protocol-level failure is, at page granularity,
    //    an undecodable page.
    let (page_type_v, _uncompressed, _compressed, v2, header_len) =
        match read_page_header_outer(page) {
            Ok(ok) => ok,
            Err(_) => return Err(ParquetError::UnexpectedEof),
        };
    // Invariant of the header reader: header_len <= page.len(); the bounded
    // get keeps that law local instead of trusting it.
    let body = match page.get(header_len..) {
        Some(body) => body,
        None => return Err(ParquetError::UnexpectedEof),
    };

    // 2. Page shape: everything outside the aligned framer's envelope is a
    //    typed refusal, with the offending shape value carried for the log.
    fn unsupported_shape(v: i32) -> ParquetError {
        ParquetError::UnsupportedBitWidth(v as u8)
    }
    if page_type_v != page_type::DATA_PAGE_V2 {
        return Err(unsupported_shape(page_type_v));
    }
    let Some(v2) = v2 else {
        return Err(ParquetError::UnexpectedEof);
    };
    let Some(encoding_v) = v2.encoding else {
        // required real-idl field 4 absent
        return Err(ParquetError::UnexpectedEof);
    };
    if encoding_v != encoding::PLAIN {
        return Err(unsupported_shape(encoding_v));
    }
    // Absent is_compressed means true per the parquet.thrift default.
    if v2.is_compressed.unwrap_or(true) {
        return Err(unsupported_shape(1));
    }
    // Level byte lengths (real ids 5/6) must be zero: a required column has
    // no level sections, and without decoding them the value section cannot
    // be located. Negative values are malformed too.
    let def_len = v2.def_len.unwrap_or(0);
    let rep_len = v2.rep_len.unwrap_or(0);
    if def_len != 0 || rep_len != 0 || def_len < 0 || rep_len < 0 {
        return Err(unsupported_shape(def_len));
    }
    if v2.num_nulls.unwrap_or(0) < 0 || v2.num_rows.unwrap_or(0) < 0 {
        return Err(ParquetError::UnexpectedEof);
    }

    // 3. Budget: the header's lane claim is bounded by the caller's slice.
    let Some(num_values) = v2.num_values else {
        return Err(ParquetError::UnexpectedEof);
    };
    if num_values < 0 {
        // A negative claim cannot fit any budget: the unbounded overrun.
        return Err(ParquetError::RunOverrun {
            run_len: usize::MAX,
            budget: out.len(),
        });
    }
    let num_values = num_values as usize;
    if num_values > out.len() {
        return Err(ParquetError::RunOverrun {
            run_len: num_values,
            budget: out.len(),
        });
    }

    // 4. Body: the aligned claim — one bounds-checked slice. A body shorter
    //    than the header claims is truncated -> EOF, never a partial copy
    //    reported as success.
    if body.len() < num_values {
        return Err(ParquetError::UnexpectedEof);
    }
    out[..num_values].copy_from_slice(&body[..num_values]);
    Ok(num_values)
}

#[cfg(test)]
mod tests {
    use super::{aligned_decode, write_plain_page};
    use crate::rle::ParquetError;
    use crate::thrift::{
        ctype, encoding, page_type, read_varint, write_bool_field, write_field_header,
        write_i32_field,
    };
    use alloc::vec;
    use alloc::vec::Vec;

    const SENTINEL: u8 = 0xA5;

    /// Test-only header writer with shape knobs (same real-IDL layout).
    fn header_with(
        ptype: i32,
        enc: i32,
        compressed: bool,
        def_len: i32,
        rep_len: i32,
        nvals: i32,
        nrows: i32,
        body_len: i32,
    ) -> Vec<u8> {
        let mut b = Vec::new();
        let mut prev: i16 = 0;
        write_i32_field(&mut b, 1, prev, ptype);
        prev = 1;
        write_i32_field(&mut b, 2, prev, body_len);
        prev = 2;
        write_i32_field(&mut b, 3, prev, body_len);
        prev = 3;
        write_field_header(&mut b, 8, prev, ctype::STRUCT);
        let mut p: i16 = 0;
        write_i32_field(&mut b, 1, p, nvals);
        p = 1;
        write_i32_field(&mut b, 2, p, 0);
        p = 2;
        write_i32_field(&mut b, 3, p, nrows);
        p = 3;
        write_i32_field(&mut b, 4, p, enc);
        p = 4;
        write_i32_field(&mut b, 5, p, def_len);
        p = 5;
        write_i32_field(&mut b, 6, p, rep_len);
        p = 6;
        write_bool_field(&mut b, 7, p, compressed);
        b.push(ctype::STOP);
        b.push(ctype::STOP);
        b
    }

    fn round_trip(values: &[u8], num_rows: u32) {
        let page = write_plain_page(values, num_rows);

        // Framing is honest: the body is the values verbatim
        // (already-Arrow-layout claim, asserted at the byte level) and the
        // header parses with the real-IDL field ids.
        let mut out = alloc::vec![SENTINEL; values.len() + 7];
        let n = aligned_decode(&page, &mut out).expect("decode");
        assert_eq!(n, values.len());
        assert_eq!(&out[..values.len()], values);
        assert!(
            out[values.len()..].iter().all(|&x| x == SENTINEL),
            "tail untouched"
        );

        // Body offset == header end (no padding), body == values. The
        // writer is deterministic, so the body is the last values.len()
        // bytes of the page.
        assert_eq!(&page[page.len() - values.len()..], values);
    }

    #[test]
    fn round_trip_shapes() {
        round_trip(&[], 0);
        round_trip(&[7], 1);
        round_trip(&[0, 255, 1, 254, 127, 128], 6);
        round_trip(&vec![0xAB; 4096], 4096);
        let pattern: Vec<u8> = (0..1000u32).map(|i| (i * 7 + 1) as u8).collect();
        round_trip(&pattern, 1000);
    }

    #[test]
    fn wire_shape_is_real_thrift_compact_real_idl() {
        // Byte-level receipt: field 1 = DATA_PAGE_V2 (delta-encoded I32,
        // zigzag 6); V2 subheader rides field 8 as STRUCT; real-idl
        // encoding at id 4 = PLAIN(0); is_compressed explicitly FALSE at
        // id 7 (bool wire); level lengths 0 at ids 5/6.
        let page = write_plain_page(&[5, 5, 5], 3);
        assert_eq!(page[0], (1 << 4) | ctype::I32);
        assert_eq!(page[1], 6); // zigzag(DATA_PAGE_V2 = 3)
        let mut pos = 2usize;
        // field 2, 3: sizes = 3 -> zigzag 6.
        for _ in 0..2 {
            assert_eq!(page[pos], (1 << 4) | ctype::I32);
            let (raw, adv) = {
                let mut p = pos + 1;
                let raw = read_varint(&page, &mut p).expect("varint");
                (raw, p)
            };
            assert_eq!((raw >> 1) as i64, 3, "size zigzags to 3");
            pos = adv;
        }
        assert_eq!(page[pos], (5 << 4) | ctype::STRUCT, "field 8, delta 5");
        pos += 1;
        // Substruct: ids 1,2,3 (3/0/3), 4 = PLAIN(0), 5 = 0, 6 = 0,
        // 7 = BOOL_FALSE, STOP, STOP.
        let expected_tail: Vec<u8> = {
            let mut e = Vec::new();
            let mut p: i16 = 0;
            write_i32_field(&mut e, 1, p, 3);
            p = 1;
            write_i32_field(&mut e, 2, p, 0);
            p = 2;
            write_i32_field(&mut e, 3, p, 3);
            p = 3;
            write_i32_field(&mut e, 4, p, encoding::PLAIN);
            p = 4;
            write_i32_field(&mut e, 5, p, 0);
            p = 5;
            write_i32_field(&mut e, 6, p, 0);
            p = 6;
            write_bool_field(&mut e, 7, p, false);
            e.push(ctype::STOP);
            e
        };
        assert_eq!(&page[pos..pos + expected_tail.len()], &expected_tail[..]);
        assert_eq!(page[pos + expected_tail.len()], ctype::STOP);
        // Body follows the header immediately: the values verbatim.
        assert_eq!(&page[pos + expected_tail.len() + 1..], &[5u8, 5, 5][..]);
    }

    #[test]
    fn oversize_budget_tail_untouched() {
        let values: Vec<u8> = (0..16u8).collect();
        let page = write_plain_page(&values, 16);
        let mut out = alloc::vec![SENTINEL; 40];
        assert_eq!(aligned_decode(&page, &mut out), Ok(16));
        assert_eq!(&out[..16], values.as_slice());
        assert!(out[16..].iter().all(|&x| x == SENTINEL));
    }

    #[test]
    fn truncated_at_every_prefix_fails_closed() {
        let values: Vec<u8> = (0..64u8)
            .map(|i| i.wrapping_mul(7).wrapping_add(1))
            .collect();
        let page = write_plain_page(&values, 64);
        let mut out = alloc::vec![0u8; 64];

        // Anti-vacuity: the intact page decodes before the sweep.
        assert_eq!(aligned_decode(&page, &mut out), Ok(64));

        for cut in 0..page.len() {
            assert!(
                aligned_decode(&page[..cut], &mut out).is_err(),
                "prefix {cut} of {} must be Err",
                page.len()
            );
        }
        // And the intact page still decodes after the sweep.
        assert_eq!(aligned_decode(&page, &mut out), Ok(64));
    }

    #[test]
    fn corrupt_headers_fail_typed() {
        let values: Vec<u8> = (0..10u8).collect();
        let mut out = alloc::vec![0u8; 10];

        // Wrong page type (DATA_PAGE v1 instead of V2): flip field 1's
        // zigzag payload (6 -> 0).
        let mut page = write_plain_page(&values, 10);
        page[1] = 0;
        assert_eq!(
            aligned_decode(&page, &mut out),
            Err(ParquetError::UnsupportedBitWidth(
                page_type::DATA_PAGE as u8
            ))
        );

        // Non-PLAIN encoding at real idl 4 (RLE = 3).
        let mut header = header_with(3, encoding::RLE, false, 0, 0, 10, 10, 10);
        header.extend_from_slice(&values);
        assert_eq!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(encoding::RLE as u8))
        );

        // V2 page type without the V2 subheader.
        let mut header = Vec::new();
        write_i32_field(&mut header, 1, 0, page_type::DATA_PAGE_V2);
        write_i32_field(&mut header, 2, 1, 4);
        write_i32_field(&mut header, 3, 2, 4);
        header.push(ctype::STOP);
        assert_eq!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnexpectedEof)
        );
    }

    #[test]
    fn compressed_or_leveled_pages_refuse() {
        let values: Vec<u8> = (0..8u8).collect();
        let mut out = alloc::vec![0u8; 8];

        // is_compressed = true (real idl 7).
        let mut header = header_with(3, encoding::PLAIN, true, 0, 0, 8, 8, 8);
        header.extend_from_slice(&values);
        assert!(matches!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(_))
        ));

        // is_compressed ABSENT: parquet.thrift default true -> refuse.
        let mut header = header_with(3, encoding::PLAIN, false, 0, 0, 8, 8, 8);
        header.truncate(header.len() - 3); // drop bool field byte + 2 STOPs
        header.push(ctype::STOP);
        header.push(ctype::STOP);
        header.extend_from_slice(&values);
        assert!(matches!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(_))
        ));

        // definition_levels_byte_length != 0 (real idl 5).
        let mut header = header_with(3, encoding::PLAIN, false, 5, 0, 8, 8, 8);
        header.extend_from_slice(&values);
        assert!(matches!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(_))
        ));

        // repetition_levels_byte_length != 0 (real idl 6).
        let mut header = header_with(3, encoding::PLAIN, false, 0, 5, 8, 8, 8);
        header.extend_from_slice(&values);
        assert!(matches!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(_))
        ));
    }

    #[test]
    fn budget_claims_map_to_run_overrun() {
        let values: Vec<u8> = (0..10u8).collect();
        let page = write_plain_page(&values, 10);

        // Claim (10) exceeds budget (3).
        let mut out = alloc::vec![0u8; 3];
        assert_eq!(
            aligned_decode(&page, &mut out),
            Err(ParquetError::RunOverrun {
                run_len: 10,
                budget: 3
            })
        );

        // Negative num_values: the unbounded claim. Hand-built header with
        // num_values = -1 (real idl 1) over a plain body.
        let mut header = header_with(3, encoding::PLAIN, false, 0, 0, -1, 10, 10);
        header.extend_from_slice(&values);
        let mut out = alloc::vec![0u8; 100];
        assert_eq!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::RunOverrun {
                run_len: usize::MAX,
                budget: 100
            })
        );
    }

    #[test]
    fn negative_shape_fields_fail_closed() {
        let values: Vec<u8> = (0..4u8).collect();
        let mut out = alloc::vec![0u8; 4];
        // num_nulls = -3 (real idl 2): hand-built header.
        let mut b = Vec::new();
        let mut prev: i16 = 0;
        write_i32_field(&mut b, 1, prev, page_type::DATA_PAGE_V2);
        prev = 1;
        write_i32_field(&mut b, 2, prev, 4);
        prev = 2;
        write_i32_field(&mut b, 3, prev, 4);
        prev = 3;
        write_field_header(&mut b, 8, prev, ctype::STRUCT);
        let mut p: i16 = 0;
        write_i32_field(&mut b, 1, p, 4);
        p = 1;
        write_i32_field(&mut b, 2, p, -3);
        p = 2;
        write_i32_field(&mut b, 3, p, 4);
        p = 3;
        write_i32_field(&mut b, 4, p, encoding::PLAIN);
        p = 4;
        write_i32_field(&mut b, 5, p, 0);
        p = 5;
        write_i32_field(&mut b, 6, p, 0);
        p = 6;
        write_bool_field(&mut b, 7, p, false);
        b.push(ctype::STOP);
        b.push(ctype::STOP);
        b.extend_from_slice(&values);
        assert_eq!(
            aligned_decode(&b, &mut out),
            Err(ParquetError::UnexpectedEof)
        );
    }

    #[test]
    fn truncated_body_yields_eof_not_partial_ok() {
        // Header claims 10 values; body carries 9 — EOF, never a silent
        // partial copy.
        let mut header = header_with(3, encoding::PLAIN, false, 0, 0, 10, 10, 10);
        header.extend_from_slice(&(0..9u8).collect::<Vec<u8>>());
        let mut out = alloc::vec![0u8; 10];
        assert_eq!(
            aligned_decode(&header, &mut out),
            Err(ParquetError::UnexpectedEof)
        );
    }

    #[test]
    fn trailing_bytes_after_body_are_ignored() {
        let values: Vec<u8> = (0..8u8).collect();
        let mut page = write_plain_page(&values, 8);
        page.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let mut out = alloc::vec![0u8; 8];
        assert_eq!(aligned_decode(&page, &mut out), Ok(8));
        assert_eq!(out, values);
    }
}
