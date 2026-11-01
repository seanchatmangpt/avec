//! Mutation-court killing tests for cgap-parquet (lane W2-MUT-PARQUET).
//!
//! Chicago discipline: real bytes in, real bytes/typed errors out, final
//! state asserted exactly — no mocks (nothing to mock: pure kernels).
//!
//! Every test cites the mutant(s) it kills as `file:line:col` rows from
//! `docs/mutation-court-backlog.txt` (crates/cgap-parquet section). Golden
//! wire bytes derive from the codec spec in `crates/cgap-parquet/src/rle.rs`:
//! a leading width byte, then records — repeated run
//! `header = (count << 1) | 0` plus one LE value of ceil(w/8) bytes; literal
//! run `header = (groups << 1) | 1` plus one byte per lane (low `w` bits,
//! LSB-first), zero-padded final group.

use cgap_parquet::rle::{rle_decode, rle_encode, ParquetError};
use cgap_parquet::thrift::{ctype, skip_value, ThriftError};
use cgap_parquet::unpack::{unpack_2bit, unpack_4bit, unpack_8bit};

/// Kills rle.rs:29:9 — `Display::fmt` replaced by `Ok(Default::default())`
/// renders the empty string instead of the Debug text.
#[test]
fn display_forwards_to_debug_string() {
    assert_eq!(ParquetError::UnsupportedBitWidth(3).to_string(), "UnsupportedBitWidth(3)");
    assert_eq!(ParquetError::UnexpectedEof.to_string(), "UnexpectedEof");
}

/// Kills rle.rs:90:27 (`i + run` -> `i * run` mis-segments the literal
/// batch), rle.rs:93:29 (scan-ahead `<` -> `>` swallows the next >=3-run
/// into the literal batch), rle.rs:93:56 (`values[j + r]` -> `values[j - r]`
/// mis-detects run starts), rle.rs:103:59 (literal tag `| 1` -> `^ 1` flips
/// the odd-group header to an even/run header).
///
/// Round-trip tests cannot see any of these (every mutation still encodes a
/// self-consistent page for this input), so the wire bytes are pinned
/// exactly.
#[test]
fn golden_bytes_run_vs_literal_boundary() {
    let encoded = rle_encode(&[9, 9, 9, 5, 5, 6, 6, 6], 4);
    // Wire-format fix: literal batches mid-stream are FULL 8-lane groups —
    // the batch absorbs the following run lanes (5,5 + 6,6,6 -> one group)
    // because the record form has no intra-record lane count and a partial
    // group mid-stream would over-decode pad lanes over the next record.
    // Zero-padding exists only in a FINAL group (budget clamp at end of
    // stream). Old (defective) bytes pinned [.., 5,5, 0*6, 6,6].
    assert_eq!(encoded, vec![4, 6, 9, 3, 5, 5, 6, 6, 6, 0, 0, 0]);
    // Budget-bounded decode recovers the real lanes.
    let mut out = [0u32; 5];
    rle_decode(&encoded, &mut out).expect("decode");
    assert_eq!(out, [9, 9, 9, 5, 5]);
}

/// Kills rle.rs:101:27 (`j - start` -> `j + start` inflates the literal
/// count; the group count only diverges when the batch starts at a nonzero
/// offset, so the tail batch sits at offset 10 and the mutant claims 3
/// groups instead of 1).
#[test]
fn golden_bytes_literal_tail_group_count() {
    // 9 x10 -> run(10); then a literal batch of exactly 2 lanes = 1 group.
    assert_eq!(
        rle_encode(&[9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 5, 5], 4),
        vec![4, 0x14, 9, 3, 5, 5, 0, 0, 0, 0, 0, 0]
    );
}

/// Kills rle.rs:110:27 (`bit < vb` -> `bit <= vb` copies bit `w` of the value
/// into the lane byte): a lane byte carries exactly `w` bits; bit `w` must
/// never reach the wire (the decoder masks it off, so only byte-level asserts
/// can see this mutant).
#[test]
fn golden_bytes_lane_byte_is_lane_width() {
    // 0x1F has bit 4 set; with bits=4 the lane byte must be 0x0F, not 0x1F.
    assert_eq!(rle_encode(&[0x1F], 4), vec![4, 3, 0x0F, 0, 0, 0, 0, 0, 0, 0]);
    // bits=8 keeps the full byte (wire-consistency, not a kill).
    assert_eq!(rle_encode(&[0x1F], 8), vec![8, 3, 0x1F, 0, 0, 0, 0, 0, 0, 0]);
}

/// Kills rle.rs:103:59 on a full 8-lane group (groups=1, odd: `| 1` -> `^ 1`
/// clears the tag bit, decode then reads the page as runs and fails closed).
#[test]
fn golden_bytes_full_literal_group() {
    let encoded = rle_encode(&[1, 2, 3, 4, 5, 6, 7, 8], 4);
    assert_eq!(encoded, vec![4, 3, 1, 2, 3, 4, 5, 6, 7, 8]);
    let mut out = [0u32; 8];
    rle_decode(&encoded, &mut out).expect("decode");
    assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8]);
}

/// Kills rle.rs:141:36 (overrun guard `out.len() - oi` -> `out.len() + oi`
/// never fires; the mutant then writes past `out` and panics) and
/// rle.rs:142:82 (`-` -> `+` / `-` -> `/` in the RunOverrun `budget` field;
/// both asserts use oi > 0 so the value, not a div-by-zero panic, is the
/// kill).
#[test]
fn run_overrun_is_typed_and_exact() {
    // bits=1; header varint 0x14 = 20 -> run_len 10 > budget 4.
    let err = rle_decode(&[0x01, 0x14], &mut [0u32; 4]).unwrap_err();
    assert_eq!(err, ParquetError::RunOverrun { run_len: 10, budget: 4 });

    // Second record overruns (oi = 2): budget must be exactly 6 - 2 = 4.
    // bytes: width=1 | run(2 x value 1) | header 20 (run_len 10).
    let err = rle_decode(&[0x01, 0x04, 0x01, 0x14], &mut [0u32; 6]).unwrap_err();
    assert_eq!(err, ParquetError::RunOverrun { run_len: 10, budget: 4 });
}

/// Kills rle.rs:167:99 (`src.len() - pos` -> `+` / `/` in the LiteralOverrun
/// `available` field; pos = 2 > 0, so the value assert kills both operators).
#[test]
fn literal_overrun_is_typed_and_exact() {
    // bits=4; literal header claims 1 group = 8 bytes; 3 bytes remain
    // (available = 5 - 2 = 3).
    let err = rle_decode(&[0x04, 0x03, 0xAA, 0xBB, 0xCC], &mut [0u32; 8]).unwrap_err();
    assert_eq!(err, ParquetError::LiteralOverrun { claimed: 8, available: 3 });
}

/// Kills unpack.rs:30:28 (`si < src.len()` -> `<=`: reads src[len] once the
/// words are exhausted) and unpack.rs:31:24 (`(n - oi).min(32)` ->
/// `(n + oi).min(32)`: the clamp stops bounding, the mutant writes past
/// `out` and panics). Both panic under the mutant; the asserts pin the
/// zero-padding contract on the healthy code.
#[test]
fn unpack_2bit_zero_pad_beyond_src() {
    let mut out = [0u64; 40];
    unpack_2bit(&[u64::MAX], &mut out);
    assert_eq!(out[..32], [3u64; 32]); // 0b11 fields of an all-ones word
    assert_eq!(out[32..], [0u64; 8]); // zero-padded once src is exhausted
}

/// Kills unpack.rs:37:12 (`si += 1` -> `si *= 1`: the source word index
/// never advances, so every output word re-reads src[0]).
#[test]
fn unpack_2bit_advances_source_word() {
    let mut out = [0u64; 64];
    unpack_2bit(&[0, u64::MAX], &mut out);
    assert_eq!(out[..32], [0u64; 32]); // word A = 0
    assert_eq!(out[32..], [3u64; 32]); // word B fields, not a repeat of A
}

/// Kills unpack.rs:48:28 (OOB read at si == len) and unpack.rs:49:24
/// (inflated avail -> OOB write past a 20-lane `out`).
#[test]
fn unpack_4bit_zero_pad_beyond_src() {
    let mut out = [0u64; 20];
    unpack_4bit(&[u64::MAX], &mut out);
    assert_eq!(out[..16], [0xFu64; 16]);
    assert_eq!(out[16..], [0u64; 4]);
}

/// Kills unpack.rs:55:12 (`si *= 1` repeats src[0] for every output word).
#[test]
fn unpack_4bit_advances_source_word() {
    let mut out = [0u64; 32];
    unpack_4bit(&[0, u64::MAX], &mut out);
    assert_eq!(out[..16], [0u64; 16]);
    assert_eq!(out[16..], [0xFu64; 16]);
}

/// Kills unpack.rs:66:28 (OOB read at si == len) and unpack.rs:67:24
/// (inflated avail -> OOB write past a 10-lane `out`).
#[test]
fn unpack_8bit_zero_pad_beyond_src() {
    let mut out = [0u64; 10];
    unpack_8bit(&[0x40302010], &mut out);
    assert_eq!(out[..8], [0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0]);
    assert_eq!(out[8..], [0u64; 2]);
}

/// Kills unpack.rs:73:12 (`si *= 1` repeats src[0] for every output word).
#[test]
fn unpack_8bit_advances_source_word() {
    let mut out = [0u64; 16];
    unpack_8bit(&[0x1111_1111_1111_1111, 0x2222_2222_2222_2222], &mut out);
    assert_eq!(out[..8], [0x11u64; 8]);
    assert_eq!(out[8..], [0x22u64; 8]);
}

/// Kills thrift.rs:50:9 (Display::fmt -> Ok(Default::default()) renders the
/// empty string instead of the Debug text).
#[test]
fn thrift_display_forwards_to_debug_string() {
    assert_eq!(
        ThriftError::MissingRequiredField(4).to_string(),
        "MissingRequiredField(4)"
    );
}

/// Kills thrift.rs:271:21 x3 and 279:21 x3 — the BYTE/DOUBLE skip bounds
/// (`*pos > src.len()` replaced by `==` / `<` / `>=`): an in-range skip must
/// succeed (`<` fires spuriously otherwise) and a skip landing exactly at
/// the source end must succeed (`==` / `>=` fire there).
#[test]
fn thrift_byte_double_skip_bounds_are_exact() {
    // BYTE mid-wire: payload followed by more bytes — `<` would error here.
    let mut pos = 0;
    assert_eq!(skip_value(&[0xAB, 0xFF, 0x05], &mut pos, ctype::BYTE, 0), Ok(()));
    assert_eq!(pos, 1);
    // BYTE at exact end: payload is the last byte — `==` / `>=` would error.
    let mut pos = 0;
    assert_eq!(skip_value(&[0x42], &mut pos, ctype::BYTE, 0), Ok(()));
    assert_eq!(pos, 1);
    // DOUBLE mid-wire: 8 payload bytes, then a tail byte.
    let mut pos = 0;
    assert_eq!(
        skip_value(&[0u8; 9], &mut pos, ctype::DOUBLE, 0),
        Ok(())
    );
    assert_eq!(pos, 8);
    // DOUBLE at exact end: exactly the 8 payload bytes.
    let mut pos = 0;
    assert_eq!(skip_value(&[0u8; 8], &mut pos, ctype::DOUBLE, 0), Ok(()));
    assert_eq!(pos, 8);
}

/// Kills the MAP skip-path cluster: thrift.rs:304:22 (count==0 early-return
/// inverted), 308:36 (`kv >> 4` -> `<<`), 308:45 x2 (`kv & 0x0F` -> `|`/`^`),
/// 309:16 and 312:16 (`!valid_type_id` negation deleted). No other test
/// exercises an in-range MAP skip; the exact consumed-length and Ok asserts
/// pin every one of these.
#[test]
fn thrift_map_skip_exact_wire_and_empty() {
    // count=1; kv=0x35 -> key type BYTE(3), value type I32(5); key payload
    // 0xAB; value payload varint 0xAB 0x05 (LEB128 continuation + stop).
    let wire = [0x01, 0x35, 0xAB, 0x05];
    let mut pos = 0;
    assert_eq!(skip_value(&wire, &mut pos, ctype::MAP, 0), Ok(()));
    assert_eq!(pos, 4, "map skip must consume exactly the declared wire");

    // Empty map: ONLY the count byte is consumed — the kv byte must not be
    // read (the 304:22 inversion reads it and hits end-of-input or, on a
    // longer wire, desyncs the position).
    let mut pos = 0;
    assert_eq!(skip_value(&[0x00], &mut pos, ctype::MAP, 0), Ok(()));
    assert_eq!(pos, 1);
}

/// Kills thrift.rs:352:45 (`depth + 1` -> `depth - 1` in skip_struct): with
/// u8 wrap, depth 0 -> 255 trips the >=32 guard at the SECOND nested struct,
/// so a lawful 3-deep nested struct must still skip cleanly. The existing
/// bomb test only asserts DepthExceeded (which the mutant still produces,
/// two levels early), and the only other struct skip in the suite nests one
/// level deep, where the mutant is invisible.
///
/// Wire receipt: field headers are (delta << 4) | type; I32 type nibble is 5
/// (BOOLEAN_TRUE is 1 — 0x11 bytes parse as bool headers and leave
/// num_values unset), deltas are id GAPS (consecutive ids = delta 1).
#[test]
fn thrift_three_deep_struct_skip_succeeds() {
    // PageHeader { 1: type=3, 2: size=7, 3: size=7,
    //   4: STRUCT { 1: STRUCT { 1: i32(5) } }, 8: V2 subheader }.
    let wire: Vec<u8> = vec![
        0x15, 0x06, // field 1, I32, zigzag(3)=6
        0x15, 0x0E, // field 2, I32, zigzag(7)=14 (delta 1 from id 1)
        0x15, 0x0E, // field 3, I32, zigzag(7)=14 (delta 1 from id 2)
        0x1C, // field 4, STRUCT (delta 1 from id 3)
        0x1C, //   inner field 1, STRUCT (delta 1 from 0)
        0x15, 0x0A, //     innermost field 1, I32, zigzag(5)=10
        0x00, //   inner STOP
        0x00, // outer struct STOP
        0x4C, // field 8, STRUCT (delta 4 from id 4: 4+4=8)
        0x15, 0x08, // V2 field 1 num_values=4 (I32 nibble 5, delta 1)
        0x15, 0x00, // field 2 num_nulls=0
        0x15, 0x08, // field 3 num_rows=4
        0x15, 0x06, // field 4 encoding, zigzag(3)=6 (RLE)
        0x15, 0x00, // field 5 def_len=0
        0x15, 0x00, // field 6 rep_len=0
        0x12, // field 7, BOOLEAN_FALSE (delta 1, type nibble 2)
        0x00, // V2 STOP
        0x00, // PageHeader STOP
    ];
    let (h, consumed) = cgap_parquet::thrift::read_page_header(&wire).expect("3-deep skip");
    assert_eq!(h.page_type, 3);
    assert_eq!(h.uncompressed_page_size, 7);
    assert_eq!(h.compressed_page_size, 7);
    let v2 = h.data_page_header_v2.expect("v2");
    assert_eq!(v2.num_values, 4);
    assert_eq!(v2.num_nulls, 0);
    assert_eq!(v2.num_rows, 4);
    assert_eq!(v2.encoding, 3);
    assert!(!v2.is_compressed);
    assert_eq!(consumed, wire.len(), "header must consume its own wire");
}
