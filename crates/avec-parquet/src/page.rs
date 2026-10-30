//! Minimal Parquet page framing: `DataPageV2`-style header + RLE body.
//!
//! Scope, stated honestly: this is a *minimal DataPageV2-style framing*, NOT
//! the full Parquet format. There are no row groups, no column/file
//! metadata, no footer magic, no repetition/definition levels (the V2 level
//! byte lengths must be zero), and no compression (the `is_compressed` bit
//! must be false). What it does provide is the true wire shape of the layer
//! it implements: a Thrift Compact Protocol `PageHeader`
//! ([`crate::thrift`], real parquet.thrift field ids) followed by an
//! RLE/bit-packed hybrid body from [`crate::rle`], decodable straight into
//! caller-owned memory.
//!
//! Zero-allocation decode law: [`decode_page_into`] parses the header by
//! borrowing from the page slice and hands the body to
//! [`crate::rle::decode_into`], which writes lanes directly into the
//! caller's buffer. The path contains no `Vec`, no copy-out, no
//! intermediate buffer — measured, not asserted, by the counting-global-
//! allocator court in `tests/page_zero_alloc.rs` (it lives in a separate
//! test binary because `rle.rs`'s in-lib court already owns the lib test
//! target's single `#[global_allocator]`; rustc: "cannot define multiple
//! global allocators").
//!
//! Error mapping (typed, documented once here): the return type is fixed to
//! `ParquetError` by this module's charter, so page-level conditions map
//! onto it as follows —
//! - header that cannot be parsed (truncation, unknown compact type, bad
//!   field id, missing required field, nesting bomb): every
//!   [`ThriftError`] collapses to [`ParquetError::UnexpectedEof`] — at page
//!   granularity the page is simply not decodable;
//! - page shape outside the minimal framer (non-V2 page type, non-RLE
//!   encoding, `is_compressed = true`, non-zero level byte lengths):
//!   [`ParquetError::UnsupportedBitWidth`] carries the offending shape
//!   value — the page's type/encoding axis is the "width" this kernel
//!   does not support;
//! - header claims that exceed the caller's budget (`num_values` greater
//!   than `out.len()`, or a negative lane claim, which is an unbounded
//!   claim): [`ParquetError::RunOverrun`];
//! - body that yields fewer lanes than the header claims (truncated body):
//!   [`ParquetError::UnexpectedEof`], matching `rle_decode`'s full-buffer
//!   precedent.

use crate::encode_fast::rle_encode_fast;
use crate::rle::{decode_into, ParquetError};
use crate::thrift::{
    encoding, page_type, read_page_header, write_page_header, DataPageHeaderV2, PageHeader,
    ThriftError,
};
use alloc::vec::Vec;

/// Saturate a byte count into the header's i32 envelope. Pages over
/// `i32::MAX` bytes are outside this minimal framer's envelope; the header
/// then reports the saturation ceiling rather than wrapping silently.
fn size_i32(v: usize) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// Write a complete data page (V2-style): thrift `PageHeader` +
/// RLE hybrid body of `values` at bit width `bits`.
///
/// `bits` outside the SWAR kernel set {1, 2, 4, 8} does not panic and is
/// not silently rewritten: the page is emitted as written, and
/// [`decode_page_into`] refuses it with
/// [`ParquetError::UnsupportedBitWidth`] (courted below). Values must fit
/// the declared width — out-of-range values are truncated by the hybrid
/// codec's packer, same as `crate::rle`.
pub fn write_data_page_v2(values: &[u32], bits: u8, num_rows: u32) -> Vec<u8> {
    // Pre-sized fast encoder: byte-identical output (240k-case differential),
    // kills the output-Vec growth (23 of the 28 literal-shape allocs).
    let body = rle_encode_fast(values, bits);
    let header = PageHeader {
        page_type: page_type::DATA_PAGE_V2,
        uncompressed_page_size: size_i32(body.len()),
        compressed_page_size: size_i32(body.len()), // stored uncompressed
        data_page_header_v2: Some(DataPageHeaderV2 {
            num_values: size_i32(values.len()),
            num_nulls: 0,
            num_rows: i32::try_from(num_rows).unwrap_or(i32::MAX),
            definition_levels_byte_length: 0,
            repetition_levels_byte_length: 0,
            is_compressed: false,
            encoding: encoding::RLE,
        }),
    };
    let mut out = Vec::new();
    write_page_header(&mut out, &header);
    out.extend_from_slice(&body);
    out
}

/// Decode a page directly into `out`, writing lanes in place.
///
/// Contract:
/// - `Ok(num_values)` — the page's `num_values` lanes were written to
///   `out[0..num_values]`; `out[num_values..]` is untouched (a budget larger
///   than the page is lawful; a budget smaller is
///   [`ParquetError::RunOverrun`]).
/// - Typed error on corrupt/truncated headers, unsupported page shape, or a
///   truncated body — never a panic on external input (mapping documented
///   in the module docs). On error, lanes before the failing point may
///   already be written; the error is loud, never a silent partial success.
///
/// Zero intermediate storage: header parse borrows from `page`, the body is
/// decoded in place by [`crate::rle::decode_into`].
pub fn decode_page_into(page: &[u8], out: &mut [u32]) -> Result<usize, ParquetError> {
    // 1. Header parse: any protocol-level failure is, at page granularity,
    //    an undecodable page.
    let (header, header_len) = match read_page_header(page) {
        Ok(ok) => ok,
        Err(ThriftError::UnexpectedEof) => return Err(ParquetError::UnexpectedEof),
        Err(
            ThriftError::InvalidType(_)
            | ThriftError::InvalidFieldId
            | ThriftError::MissingRequiredField(_)
            | ThriftError::LengthOverflow
            | ThriftError::DepthExceeded,
        ) => {
            return Err(ParquetError::UnexpectedEof);
        }
    };
    // Invariant of read_page_header: header_len <= page.len(); the bounded
    // get keeps that law local instead of trusting it.
    let body = match page.get(header_len..) {
        Some(body) => body,
        None => return Err(ParquetError::UnexpectedEof),
    };

    // 2. Page shape: everything outside the minimal framer's envelope is a
    //    typed refusal, with the offending shape value carried for the log.
    fn unsupported_shape(v: i32) -> ParquetError {
        ParquetError::UnsupportedBitWidth(v as u8)
    }
    if header.page_type != page_type::DATA_PAGE_V2 {
        return Err(unsupported_shape(header.page_type));
    }
    let Some(v2) = header.data_page_header_v2 else {
        // A DATA_PAGE_V2 without its V2 subheader is malformed.
        return Err(ParquetError::UnexpectedEof);
    };
    if v2.encoding != encoding::RLE {
        return Err(unsupported_shape(v2.encoding));
    }
    if v2.is_compressed {
        // No codec surface in this framer; compressed bodies would decode
        // to garbage lanes, so refuse loudly.
        return Err(unsupported_shape(1));
    }
    if v2.definition_levels_byte_length != 0 || v2.repetition_levels_byte_length != 0 {
        // V2 bodies lead with the level sections; without decoding them the
        // value section cannot be located. Refuse rather than misparse.
        return Err(unsupported_shape(v2.definition_levels_byte_length));
    }
    if v2.num_nulls < 0 || v2.num_rows < 0 {
        return Err(ParquetError::UnexpectedEof);
    }

    // 3. Budget: the header's lane claim is bounded by the caller's slice.
    if v2.num_values < 0 {
        // A negative claim cannot fit any budget: the unbounded overrun.
        return Err(ParquetError::RunOverrun { run_len: usize::MAX, budget: out.len() });
    }
    let num_values = v2.num_values as usize;
    if num_values > out.len() {
        return Err(ParquetError::RunOverrun { run_len: num_values, budget: out.len() });
    }

    // 4. Body: direct-slice decode, zero intermediate storage. A page that
    //    yields fewer lanes than its header claims is truncated -> EOF.
    let lanes = match out.get_mut(..num_values) {
        Some(lanes) => lanes,
        None => return Err(ParquetError::UnexpectedEof),
    };
    let n = decode_into(body, lanes)?;
    if n != num_values {
        return Err(ParquetError::UnexpectedEof);
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::{decode_page_into, write_data_page_v2};
    use crate::rle::{rle_encode, ParquetError};
    use crate::thrift::{
        ctype, encoding, page_type, read_page_header, write_page_header, DataPageHeaderV2,
        PageHeader,
    };
    use alloc::vec::Vec;

    const SENTINEL: u32 = 0xDEAD_BEEF;

    fn round_trip(values: &[u32], bits: u8) {
        let num_rows = values.len() as u32;
        let page = write_data_page_v2(values, bits, num_rows);

        // Framing is honest: the header consumes exactly its own wire, and
        // the body is byte-identical to the hybrid codec's output.
        let (_, header_len) = read_page_header(&page).expect("header");
        assert_eq!(&page[header_len..], &rle_encode(values, bits)[..], "bits={bits}");

        let mut out = alloc::vec![SENTINEL; values.len()];
        let n = decode_page_into(&page, &mut out).expect("decode");
        assert_eq!(n, values.len(), "bits={bits}");
        assert_eq!(out, values, "bits={bits}");
    }

    #[test]
    fn round_trip_repeated_literal_mixed_all_widths() {
        // Repeated-dominant shape.
        round_trip(&[7, 7, 7, 7, 7, 7, 7, 7, 2, 2, 2, 2], 4);
        round_trip(&alloc::vec![9u32; 257], 2);
        round_trip(&alloc::vec![0u32; 4096], 1);
        // Literal-dominant shapes (no two neighbors equal).
        for bits in [1u8, 2, 4, 8] {
            let alpha = 1u32 << bits;
            let values: Vec<u32> = (0..300).map(|i| (i as u32 * 7 + 1) % alpha).collect();
            round_trip(&values, bits);
        }
        // Mixed: literal stretches with embedded runs (the encoder absorbs
        // short runs into literal groups — both arms exercised on decode).
        let mut mixed: Vec<u32> = (0..64).map(|i| (i * 13 + 3) as u32 & 0xFF).collect();
        for chunk in mixed.chunks_mut(16) {
            let first = chunk[0];
            chunk[..3].fill(first);
        }
        round_trip(&mixed, 8);
    }

    #[test]
    fn empty_page_round_trip() {
        let page = write_data_page_v2(&[], 4, 0);
        let mut out: [u32; 0] = [];
        assert_eq!(decode_page_into(&page, &mut out), Ok(0));
    }

    #[test]
    fn oversize_budget_tail_untouched() {
        let values: Vec<u32> = (0..16).collect();
        let page = write_data_page_v2(&values, 8, 16);
        let mut out = alloc::vec![SENTINEL; 40];
        assert_eq!(decode_page_into(&page, &mut out), Ok(16));
        assert_eq!(&out[..16], values.as_slice());
        assert!(out[16..].iter().all(|&x| x == SENTINEL), "tail untouched");
    }

    #[test]
    fn truncated_at_every_prefix_fails_closed() {
        // Mixed-shape page: literal stretch + run + literal.
        let mut values: Vec<u32> = (0..40).collect();
        values[20..30].fill(3);
        let page = write_data_page_v2(&values, 4, 40);
        let mut out = alloc::vec![0u32; values.len()];

        // Anti-vacuity: the intact page decodes before the sweep.
        assert_eq!(decode_page_into(&page, &mut out), Ok(40));

        for cut in 0..page.len() {
            // Plain is_err() on purpose: a cut inside a literal record is
            // reported by the hybrid decoder as LiteralOverrun, a cut in
            // the header as thrift-level EOF — every typed outcome is a
            // lawful fail-closed answer, none is a silent partial success.
            assert!(
                decode_page_into(&page[..cut], &mut out).is_err(),
                "prefix {cut} of {} must be Err",
                page.len()
            );
        }
        // And the intact page still decodes after the sweep.
        assert_eq!(decode_page_into(&page, &mut out), Ok(40));
    }

    #[test]
    fn header_region_truncation_on_large_page_fails_closed() {
        let values: Vec<u32> = (0..4096).map(|i| (i * 7 + 1) as u32 & 0xFF).collect();
        let page = write_data_page_v2(&values, 8, 4096);
        let (_, header_len) = read_page_header(&page).expect("header");
        let mut out = alloc::vec![0u32; values.len()];
        for cut in 0..header_len {
            assert!(
                decode_page_into(&page[..cut], &mut out).is_err(),
                "header prefix {cut}/{header_len} must be Err"
            );
        }
        assert_eq!(decode_page_into(&page, &mut out), Ok(4096));
    }

    #[test]
    fn corrupt_headers_fail_typed() {
        let values: Vec<u32> = (0..10).collect();
        let mut out = alloc::vec![0u32; 10];

        // Wrong page type (DATA_PAGE v1 instead of V2).
        let mut page = write_data_page_v2(&values, 8, 10);
        page[1] = page_type::DATA_PAGE as u8; // field 1's zigzag payload: 0 -> DATA_PAGE
        assert_eq!(
            decode_page_into(&page, &mut out),
            Err(ParquetError::UnsupportedBitWidth(page_type::DATA_PAGE as u8))
        );

        // Non-RLE encoding (BIT_PACKED = 4): field 7 of the V2 substruct.
        // Hand-build: identical to the writer's header but with encoding 4.
        let body = rle_encode(&values, 8);
        let mut header = Vec::new();
        write_page_header(
            &mut header,
            &PageHeader {
                page_type: page_type::DATA_PAGE_V2,
                uncompressed_page_size: body.len() as i32,
                compressed_page_size: body.len() as i32,
                data_page_header_v2: Some(DataPageHeaderV2 {
                    num_values: 10,
                    num_nulls: 0,
                    num_rows: 10,
                    definition_levels_byte_length: 0,
                    repetition_levels_byte_length: 0,
                    is_compressed: false,
                    encoding: encoding::BIT_PACKED,
                }),
            },
        );
        header.extend_from_slice(&body);
        assert_eq!(
            decode_page_into(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(encoding::BIT_PACKED as u8))
        );

        // V2 page type without the V2 subheader.
        let mut header = Vec::new();
        write_page_header(
            &mut header,
            &PageHeader {
                page_type: page_type::DATA_PAGE_V2,
                uncompressed_page_size: 4,
                compressed_page_size: 4,
                data_page_header_v2: None,
            },
        );
        assert_eq!(
            decode_page_into(&header, &mut out),
            Err(ParquetError::UnexpectedEof)
        );
    }

    #[test]
    fn compressed_or_leveled_pages_refuse() {
        let values: Vec<u32> = (0..8).collect();
        let mut out = alloc::vec![0u32; 8];

        // is_compressed = true.
        let body = rle_encode(&values, 4);
        let mut header = Vec::new();
        write_page_header(
            &mut header,
            &PageHeader {
                page_type: page_type::DATA_PAGE_V2,
                uncompressed_page_size: body.len() as i32,
                compressed_page_size: body.len() as i32,
                data_page_header_v2: Some(DataPageHeaderV2 {
                    num_values: 8,
                    num_nulls: 0,
                    num_rows: 8,
                    definition_levels_byte_length: 0,
                    repetition_levels_byte_length: 0,
                    is_compressed: true,
                    encoding: encoding::RLE,
                }),
            },
        );
        header.extend_from_slice(&body);
        assert!(matches!(
            decode_page_into(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(_))
        ));

        // definition_levels_byte_length != 0.
        let mut header = Vec::new();
        write_page_header(
            &mut header,
            &PageHeader {
                page_type: page_type::DATA_PAGE_V2,
                uncompressed_page_size: body.len() as i32,
                compressed_page_size: body.len() as i32,
                data_page_header_v2: Some(DataPageHeaderV2 {
                    num_values: 8,
                    num_nulls: 8,
                    num_rows: 8,
                    definition_levels_byte_length: 5,
                    repetition_levels_byte_length: 0,
                    is_compressed: false,
                    encoding: encoding::RLE,
                }),
            },
        );
        header.extend_from_slice(&body);
        assert!(matches!(
            decode_page_into(&header, &mut out),
            Err(ParquetError::UnsupportedBitWidth(_))
        ));
    }

    #[test]
    fn budget_claims_map_to_run_overrun() {
        let values: Vec<u32> = (0..10).collect();
        let page = write_data_page_v2(&values, 8, 10);

        // Claim (10) exceeds budget (3).
        let mut out = alloc::vec![0u32; 3];
        assert_eq!(
            decode_page_into(&page, &mut out),
            Err(ParquetError::RunOverrun { run_len: 10, budget: 3 })
        );

        // Negative num_values: the unbounded claim. Hand-built header with
        // num_values = -1 (field 1 of the V2 substruct).
        let body = rle_encode(&values, 8);
        let mut header = Vec::new();
        write_page_header(
            &mut header,
            &PageHeader {
                page_type: page_type::DATA_PAGE_V2,
                uncompressed_page_size: body.len() as i32,
                compressed_page_size: body.len() as i32,
                data_page_header_v2: Some(DataPageHeaderV2 {
                    num_values: -1,
                    num_nulls: 0,
                    num_rows: 10,
                    definition_levels_byte_length: 0,
                    repetition_levels_byte_length: 0,
                    is_compressed: false,
                    encoding: encoding::RLE,
                }),
            },
        );
        header.extend_from_slice(&body);
        let mut out = alloc::vec![0u32; 100];
        assert_eq!(
            decode_page_into(&header, &mut out),
            Err(ParquetError::RunOverrun { run_len: usize::MAX, budget: 100 })
        );
    }

    #[test]
    fn negative_shape_fields_fail_closed() {
        let values: Vec<u32> = (0..4).collect();
        let mut out = alloc::vec![0u32; 4];
        let body = rle_encode(&values, 4);
        let mut header = Vec::new();
        write_page_header(
            &mut header,
            &PageHeader {
                page_type: page_type::DATA_PAGE_V2,
                uncompressed_page_size: body.len() as i32,
                compressed_page_size: body.len() as i32,
                data_page_header_v2: Some(DataPageHeaderV2 {
                    num_values: 4,
                    num_nulls: -3,
                    num_rows: 4,
                    definition_levels_byte_length: 0,
                    repetition_levels_byte_length: 0,
                    is_compressed: false,
                    encoding: encoding::RLE,
                }),
            },
        );
        header.extend_from_slice(&body);
        assert_eq!(decode_page_into(&header, &mut out), Err(ParquetError::UnexpectedEof));
    }

    #[test]
    fn truncated_body_yields_eof_not_partial_ok() {
        // Full page for 10 lanes; cut into the body at every offset. The
        // body prefixes are also covered by the every-prefix sweep above;
        // here assert the exact typed outcome at a record boundary.
        let values: Vec<u32> = (0..10).collect();
        let page = write_data_page_v2(&values, 8, 10);
        let (_, header_len) = read_page_header(&page).expect("header");
        let mut out = alloc::vec![0u32; 10];
        // Body cut right after the width byte: the record that follows
        // claims lanes the truncated stream cannot deliver.
        let cut = header_len + 1;
        assert_eq!(
            decode_page_into(&page[..cut], &mut out),
            Err(ParquetError::UnexpectedEof)
        );
    }

    #[test]
    fn out_of_domain_bits_round_trips_to_typed_refusal() {
        // bits = 3 is outside the kernel set {1,2,4,8}: the writer emits,
        // the decoder refuses — no panic, no silent success.
        let page = write_data_page_v2(&[1, 2, 3, 4], 3, 4);
        let mut out = alloc::vec![0u32; 4];
        assert_eq!(
            decode_page_into(&page, &mut out),
            Err(ParquetError::UnsupportedBitWidth(3))
        );
    }

    #[test]
    fn trailing_bytes_after_body_are_ignored() {
        let values: Vec<u32> = (0..8).collect();
        let mut page = write_data_page_v2(&values, 8, 8);
        page.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let mut out = alloc::vec![0u32; 8];
        assert_eq!(decode_page_into(&page, &mut out), Ok(8));
        assert_eq!(out, values);
    }

    #[test]
    fn wire_shape_is_real_thrift_compact() {
        // Byte-level receipt for the framing: field ids and compact types
        // of a minimal V2 page header are exactly the parquet.thrift shape.
        let page = write_data_page_v2(&[5, 5, 5], 4, 3);
        // Field 1 (page_type), delta-encoded, type I32.
        assert_eq!(page[0], (1 << 4) | ctype::I32);
        // DATA_PAGE_V2 = 3 -> zigzag 6.
        assert_eq!(page[1], 6);
        let (_, header_len) = read_page_header(&page).expect("header");
        // Body: width byte then one repeated-run record of 3 lanes.
        assert_eq!(page[header_len], 4);
        // And the whole header is small, delta-encoded, self-delimiting.
        assert!(header_len < 32, "header len {header_len}");
    }

}
