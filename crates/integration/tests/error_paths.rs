//! Error-path coverage sweep (wave-14 lane 16): every typed error variant of
//! the storage plane — `avec_git::GitError`, `avec_git::inflate::InflateError`,
//! `avec_parquet::scan::ScanError`, `avec_parquet::level_free::LevelFreeError`,
//! plus `avec_audit`'s `AllocReport` — constructed and asserted at least once
//! from OUTSIDE the owning crates (integration court, not a unit test).
//!
//! Per-variant provenance for variants already covered by existing tests is
//! tabulated in `docs/jira/v26.10.31/81-error-path-coverage.md`; this file
//! newly constructs every variant that had no existing constructing court.
//!
//! Chicago discipline: real bytes in, real typed errors out, exact-variant
//! asserts (never `is_err()` where the variant is deterministic). No mocks.
//! Two dead-variant findings (`ScanError::Page`,
//! `InflateError::BadDistanceCode`) have no producing API path today; the
//! courts construct them directly to pin their `Display`/equality contract
//! and the coverage matrix flags them.

#![forbid(unsafe_code)]

use avec_audit::alloc_counter::with_alloc_tracking;
use avec_git::deflate::zlib_compress;
use avec_git::deflate::zlib_decompress;
use avec_git::idx_read::IdxV2;
use avec_git::inflate::{InflateError, inflate, zlib_decode};
use avec_git::pack_write::{
    IdxEntry, MAX_OBJECTS, build_pack_and_idx, encode_object_header, parse_idx_v2, parse_pack_v2,
    sha1, write_idx_v2, write_pack_v2,
};
use avec_git::GitError;
use avec_parquet::level_free::{
    LevelFreeError, LevelFreeType, WireSchemaElement, admit_schema, ptype, read_page_payload,
    repetition, write_page,
};
use avec_parquet::rle::ParquetError;
use avec_parquet::scan::{FixtureColumn, ParquetFile, PhysType, ScanError, write_fixture_file};
use avec_parquet::thrift::{
    DataPageHeaderV2, PageHeader, ThriftError, ctype, read_page_header, write_binary_field,
    write_field_header, write_i32_field, write_i64_field, write_page_header,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------- helpers --

/// Unique temp file (pid + nanos); real bytes on disk, deleted on drop.
struct TempFile {
    path: PathBuf,
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

impl TempFile {
    fn write(name: &str, bytes: &[u8]) -> TempFile {
        let mut p = std::env::temp_dir();
        p.push(format!("avec_errpath_{name}_{}_{}", std::process::id(), nanos()));
        std::fs::write(&p, bytes).expect("write court file");
        TempFile { path: p }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Assemble a v2 pack from raw entry bytes with a VALID trailing SHA-1, so
/// the parser reaches the per-object error paths under test.
fn pack_with_valid_trailer(entries: Vec<Vec<u8>>) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(b"PACK");
    p.extend_from_slice(&2u32.to_be_bytes());
    p.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for e in &entries {
        p.extend_from_slice(e);
    }
    let sum = sha1(&p);
    p.extend_from_slice(&sum);
    p
}

/// A deterministic u32-column fixture (one column, `rows` rows).
fn u32_fixture(rows: usize) -> (Vec<FixtureColumn>, Vec<u8>) {
    let bytes: Vec<u8> = (0..rows * 4)
        .map(|i| (i.wrapping_mul(7).wrapping_add(3)) as u8)
        .collect();
    let cols = vec![FixtureColumn {
        name: "a".into(),
        phys: PhysType::U32,
        bytes: bytes.clone(),
    }];
    (cols, bytes)
}

/// Hand-written minimal footer (single FLBA column, one row group,
/// `data_page_offset = 4`, `num_rows = 1`) with CORRECT compact-protocol
/// prev-id tracking. `rep` and `type_length` are parameters so one builder
/// yields the OPTIONAL-leaf (`UnsupportedSchema`) and negative-width
/// (`SchemaOverflow`) refusal shapes.
fn footer_flba(rep: i32, type_length: i32) -> Vec<u8> {
    // SchemaElement root: name (4) then num_children (5).
    let mut root = Vec::new();
    write_binary_field(&mut root, 4, 0, b"schema");
    write_i32_field(&mut root, 5, 4, 1);
    root.push(ctype::STOP);

    // SchemaElement leaf: type (1), type_length (2), repetition (3), name (4).
    let mut col = Vec::new();
    write_i32_field(&mut col, 1, 0, 7); // FIXED_LEN_BYTE_ARRAY
    write_i32_field(&mut col, 2, 1, type_length);
    write_i32_field(&mut col, 3, 2, rep);
    write_binary_field(&mut col, 4, 3, b"c");
    col.push(ctype::STOP);

    // ColumnMetaData: type (1), num_values (5), sizes (6, 7), offset (9).
    let mut md = Vec::new();
    write_i32_field(&mut md, 1, 0, 7);
    write_i64_field(&mut md, 5, 1, 1);
    write_i64_field(&mut md, 6, 5, 8);
    write_i64_field(&mut md, 7, 6, 8);
    write_i64_field(&mut md, 9, 7, 4); // data_page_offset
    md.push(ctype::STOP);

    // ColumnChunk: file_offset (2) + meta_data (3, struct).
    let mut cc = Vec::new();
    write_i64_field(&mut cc, 2, 0, 4);
    write_field_header(&mut cc, 3, 2, ctype::STRUCT);
    cc.extend_from_slice(&md);
    cc.push(ctype::STOP);

    // RowGroup: columns (1, list of 1 struct) + total_byte_size (2) + rows (3).
    let mut rg = Vec::new();
    write_field_header(&mut rg, 1, 0, ctype::LIST);
    rg.push((1u8 << 4) | ctype::STRUCT); // list header: 1 struct element
    rg.extend_from_slice(&cc);
    write_i64_field(&mut rg, 2, 1, 8);
    write_i64_field(&mut rg, 3, 2, 1);
    rg.push(ctype::STOP);

    // FileMetaData: version (1) + schema (2) + num_rows (3) + row_groups (4).
    let mut fmd = Vec::new();
    write_i32_field(&mut fmd, 1, 0, 1);
    write_field_header(&mut fmd, 2, 1, ctype::LIST);
    fmd.push((2u8 << 4) | ctype::STRUCT); // root + 1 leaf
    fmd.extend_from_slice(&root);
    fmd.extend_from_slice(&col);
    write_i64_field(&mut fmd, 3, 2, 1);
    write_field_header(&mut fmd, 4, 3, ctype::LIST);
    fmd.push((1u8 << 4) | ctype::STRUCT); // 1 row group
    fmd.extend_from_slice(&rg);
    fmd.push(ctype::STOP);

    let mut out = Vec::new();
    out.extend_from_slice(b"PAR1");
    out.extend_from_slice(&[0u8; 8]); // page region: unparseable, never read
    out.extend_from_slice(&fmd);
    out.extend_from_slice(&(fmd.len() as u32).to_le_bytes());
    out.extend_from_slice(b"PAR1");
    out
}

/// Replace the page header of a `write_fixture_file` output in place,
/// mutating the `DataPageHeaderV2` through `f` (payload + footer preserved).
fn rewrite_page_header(
    file: &[u8],
    f: impl FnOnce(&mut PageHeader, &mut DataPageHeaderV2),
) -> Vec<u8> {
    let (header, header_len) = read_page_header(&file[4..]).expect("page header parses");
    let mut h = header;
    let mut v2 = h.data_page_header_v2.clone().expect("v2 header present");
    f(&mut h, &mut v2);
    h.data_page_header_v2 = Some(v2);
    let mut rewritten = Vec::new();
    write_page_header(&mut rewritten, &h);
    let mut out = Vec::with_capacity(file.len() + rewritten.len());
    out.extend_from_slice(&file[..4]);
    out.extend_from_slice(&rewritten);
    out.extend_from_slice(&file[4 + header_len..]);
    out
}

/// LSB-first bit writer for hand-built DEFLATE dynamic-block headers (the
/// only construction route to two `InflateError` variants).
struct BitWriter {
    out: Vec<u8>,
    nbits: usize,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter { out: Vec::new(), nbits: 0 }
    }

    fn push(&mut self, nbits: u32, value: u32) {
        for i in 0..nbits {
            let bit = (value >> i) & 1;
            let idx = self.nbits / 8;
            if idx == self.out.len() {
                self.out.push(0);
            }
            self.out[idx] |= (bit << (self.nbits % 8)) as u8;
            self.nbits += 1;
        }
    }
}

/// A DEFLATE dynamic-block stream whose code-length code has exactly one
/// symbol with a 1-bit code; every one of the 258 header length values is
/// that symbol. `sym` is the CLEN_ORDER index given the 1-bit code.
fn dynamic_block_with_single_clen(sym: usize) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.push(1, 1); // BFINAL = 1
    w.push(2, 2); // BTYPE = 2 (dynamic Huffman)
    w.push(5, 0); // HLIT - 257 = 0  ->  257 literal codes
    w.push(5, 0); // HDIST - 1 = 0   ->  1 distance code
    w.push(4, 0); // HCLEN - 4 = 4   ->  clen symbols 16, 17, 18, 0 covered
    for i in 0..4usize {
        w.push(3, if i == sym { 1 } else { 0 }); // 3-bit clen code lengths
    }
    // 257 literal-length values + 1 distance value, each 1 bit (the code).
    for _ in 0..258 {
        w.push(1, 0);
    }
    w.out
}

// ------------------------------------------------------- GitError courts --

/// Pack-reader refusals: checksum, magic, truncation, varint header, size
/// overflow, and the FDICT-zlib refusal that maps onto `BadVarint`.
#[test]
fn git_pack_reader_error_paths() {
    let blob: Vec<u8> = (0..48u32).map(|i| (i.wrapping_mul(7).wrapping_add(1)) as u8).collect();

    // PackChecksumMismatch: one flipped body byte fails the trailer SHA-1.
    let mut pack = write_pack_v2(&[&blob]).expect("valid pack");
    pack[15] ^= 0xFF;
    assert_eq!(parse_pack_v2(&pack), Err(GitError::PackChecksumMismatch));

    // BadMagic: magic byte corrupted (magic is checked before the trailer).
    let mut pack = write_pack_v2(&[&blob]).expect("valid pack");
    pack[3] = b'X';
    assert_eq!(parse_pack_v2(&pack), Err(GitError::BadMagic));

    // Truncated: a pack shorter than its 32-byte minimum structure.
    let mut short = write_pack_v2(&[&blob]).expect("valid pack");
    short.truncate(17);
    assert_eq!(parse_pack_v2(&short), Err(GitError::Truncated));

    // BadVarint: object header with reserved type code 5 (0x50 = type 5,
    // low size bits 0) behind a valid pack checksum.
    let bad_type = pack_with_valid_trailer(vec![vec![0x50]]);
    assert_eq!(parse_pack_v2(&bad_type), Err(GitError::BadVarint));

    // SizeOverflow: declared uncompressed size 2^32 exceeds the u32 the idx
    // small-offset table admits; refused before any body is inflated.
    let huge = pack_with_valid_trailer(vec![encode_object_header(3, 1u64 << 32)]);
    assert_eq!(parse_pack_v2(&huge), Err(GitError::SizeOverflow));

    // BadVarint via the zlib surface: FDICT set in the RFC 1950 header
    // (0x78 0x20 passes the CM and %31 checks, then refuses dictionary
    // streams — unsupported here).
    assert_eq!(
        zlib_decompress(&[0x78, 0x20, 0, 0, 0, 0]),
        Err(GitError::BadVarint)
    );
}

/// Idx-reader and writer-side refusals: idx checksum, idx magic/version,
/// truncation on both readers, the object-count cap, and the large-offset
/// refusal.
#[test]
fn git_idx_and_writer_error_paths() {
    let blob: Vec<u8> = (0..32u32).map(|i| (i.wrapping_mul(11).wrapping_add(5)) as u8).collect();
    let (_, idx, _) = build_pack_and_idx(&[&blob]).expect("pack + idx");

    // IdxChecksumMismatch: one flipped trailing byte.
    let mut corrupted = idx.clone();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0x01;
    assert_eq!(parse_idx_v2(&corrupted), Err(GitError::IdxChecksumMismatch));

    // BadMagic on both idx readers: version field bumped 2 -> 3 behind a
    // valid magic.
    let mut badver = idx.clone();
    badver[7] = 3;
    assert_eq!(parse_idx_v2(&badver), Err(GitError::BadMagic));
    assert_eq!(IdxV2::parse(&badver).map(|_| ()), Err(GitError::BadMagic));

    // Truncated on both idx readers: structurally short buffers.
    let short_idx = [0u8; 100];
    assert_eq!(parse_idx_v2(&short_idx), Err(GitError::Truncated));
    assert!(matches!(IdxV2::parse(&[]), Err(GitError::Truncated)));
    // A real idx cut mid-table: the fanout-declared sections no longer fit.
    let mut cut = idx.clone();
    cut.truncate(cut.len() - 5);
    assert!(matches!(IdxV2::parse(&cut), Err(GitError::Truncated)));

    // TooManyObjects: one entry past the MAX_OBJECTS cap (fabricated table,
    // no 65k blobs allocated; the cap fires before any serialization).
    let mut entries: Vec<IdxEntry> = (0..=MAX_OBJECTS)
        .map(|i| IdxEntry {
            oid: core::array::from_fn(|j| ((i.wrapping_add(j)) & 0xFF) as u8),
            offset: 12,
            crc: 0,
        })
        .collect();
    assert_eq!(
        write_idx_v2(&mut entries, &[0u8; 20]),
        Err(GitError::TooManyObjects(MAX_OBJECTS + 1))
    );

    // OffsetTooLarge: an offset at/above 2^31 is refused by the
    // small-offset-table writer (large-offset table not implemented).
    let mut entries = vec![IdxEntry { oid: [7u8; 20], offset: 0x8000_0000, crc: 0 }];
    assert_eq!(
        write_idx_v2(&mut entries, &[0u8; 20]),
        Err(GitError::OffsetTooLarge)
    );
}

// ---------------------------------------------------- InflateError courts --

/// Stream-level inflate refusals: truncation, zlib header, reserved block
/// type, code-length set, overlong backreference, adler mismatch.
#[test]
fn inflate_stream_error_paths() {
    let payload = b"error-path adler court payload 0123456789".to_vec();

    // Truncated: one header byte, no blocks; and a raw stream cut mid-header.
    assert_eq!(zlib_decode(&[0x78]), Err(InflateError::Truncated));
    assert_eq!(inflate(&[0x03]), Err(InflateError::Truncated));

    // BadZlibHeader: CM != 8.
    assert_eq!(
        zlib_decode(&[0x00, 0x00, 0, 0, 0, 0]),
        Err(InflateError::BadZlibHeader)
    );

    // BadBlockType: BFINAL=1, BTYPE=11 (reserved).
    assert_eq!(inflate(&[0x07]), Err(InflateError::BadBlockType(3)));

    // BadCodeLengths: stored block whose ~len complement check fails.
    assert_eq!(
        inflate(&[0x01, 0x02, 0x00, 0x00, 0x00, b'h', b'i']),
        Err(InflateError::BadCodeLengths)
    );

    // OverlongBackreference: fixed-Huffman distance 2 decoded over an empty
    // output window (0 bytes written).
    match inflate(&[0x03, 0x42]) {
        Err(InflateError::OverlongBackreference { distance, written }) => {
            assert_eq!((distance, written), (2, 0));
        }
        other => panic!("expected OverlongBackreference {{distance:2, written:0}}, got {other:?}"),
    }

    // AdlerMismatch: corrupt ONLY the trailer byte of a valid zlib stream
    // (the DEFLATE body still decodes; the stored adler then disagrees).
    let z = zlib_compress(&payload);
    assert_eq!(zlib_decode(&z).as_deref(), Ok(&payload[..]));
    let mut bad = z.clone();
    let n = bad.len();
    bad[n - 1] ^= 0x80;
    match zlib_decode(&bad) {
        Err(InflateError::AdlerMismatch { expected, actual }) => {
            assert_ne!(expected, actual, "corrupted trailer must flip the adler");
        }
        other => panic!("expected AdlerMismatch, got {other:?}"),
    }

    // BadDistanceCode (DEAD BRANCH — finding): unreachable through any
    // decoder path. `dynamic_trees` caps HDIST at 30 and the fixed distance
    // tree emits exactly 30 5-bit codes (symbols 0..=29), so the decoder's
    // `dsym >= 30` guard can never hold. No API path constructs it; the
    // Display contract is pinned directly and the matrix flags the branch.
    let dead = InflateError::BadDistanceCode;
    assert_eq!(dead, InflateError::BadDistanceCode);
    assert!(dead.to_string().contains("30/31"), "{dead}");
}

/// Dynamic-header inflate refusals, built bit-by-bit: a code-length repeat
/// with no prior length, and a block with no end-of-block code.
#[test]
fn inflate_dynamic_header_error_paths() {
    // BadCodeLengthRepeat: the ONLY clen symbol with a code is 16 (a
    // repeat-with-no-prior); the very first decoded length value is it.
    let stream = dynamic_block_with_single_clen(0);
    assert_eq!(inflate(&stream), Err(InflateError::BadCodeLengthRepeat));

    // MissingEndOfBlock: the only clen symbol is 0 (zero length), so all 258
    // header length values decode to zero and symbol 256 has length 0.
    let stream = dynamic_block_with_single_clen(3);
    assert_eq!(inflate(&stream), Err(InflateError::MissingEndOfBlock));
}

// ------------------------------------------------------- ScanError courts --

/// Footer/open-side scan refusals: missing file, empty file, bad magic,
/// zeroed metadata length, OPTIONAL leaf, negative FLBA width.
#[test]
fn scan_open_error_paths() {
    // Io: the path does not exist; File::open fails before any mmap.
    let mut missing = std::env::temp_dir();
    missing.push(format!("avec_errpath_absent_{}_{}", std::process::id(), nanos()));
    assert_eq!(ParquetFile::open(&missing).map(|_| ()), Err(ScanError::Io));

    // EmptyFile: a zero-length file has no footer to map.
    let tmp = TempFile::write("empty", &[]);
    assert_eq!(ParquetFile::open(&tmp.path).map(|_| ()), Err(ScanError::EmptyFile));

    let (cols, _) = u32_fixture(8);
    let file = write_fixture_file(&cols, 8);

    // MalformedFooter: leading magic corrupted.
    let mut badmagic = file.clone();
    badmagic[0] = b'X';
    let tmp = TempFile::write("badmagic", &badmagic);
    assert_eq!(
        ParquetFile::open(&tmp.path).map(|_| ()),
        Err(ScanError::MalformedFooter)
    );

    // FooterOverflow: declared metadata length zeroed (no footer to parse
    // is overflow-classed, never clamped).
    let mut zlen = file.clone();
    let n = zlen.len();
    zlen[n - 8..n - 4].fill(0);
    let tmp = TempFile::write("zlen", &zlen);
    assert_eq!(
        ParquetFile::open(&tmp.path).map(|_| ()),
        Err(ScanError::FooterOverflow)
    );

    // UnsupportedSchema: an OPTIONAL leaf is refused at open (Hook 5, scan
    // half; carries the offending repetition id).
    let tmp = TempFile::write("optional", &footer_flba(1, 32));
    assert_eq!(
        ParquetFile::open(&tmp.path).map(|_| ()),
        Err(ScanError::UnsupportedSchema(1))
    );

    // SchemaOverflow: a negative FLBA type_length cannot resolve as usize.
    let tmp = TempFile::write("negwidth", &footer_flba(0, -1));
    assert_eq!(
        ParquetFile::open(&tmp.path).map(|_| ()),
        Err(ScanError::SchemaOverflow)
    );
}

/// Page-side scan refusals: level bytes present, compressed page, offset
/// mismatch on the public accessors, a thrift-protocol failure, and the
/// dead `Page` variant (no producing code path — contract pinned directly).
#[test]
fn scan_page_error_paths() {
    let rows = 8usize;
    let (cols, _) = u32_fixture(rows);
    let file = write_fixture_file(&cols, rows);

    // LevelsPresent: poison the definition-level byte length (Hook 1).
    let poisoned = rewrite_page_header(&file, |h, v2| {
        let _ = h;
        v2.definition_levels_byte_length = 1;
    });
    let tmp = TempFile::write("levels", &poisoned);
    let pf = ParquetFile::open(&tmp.path).expect("footer still parses");
    assert_eq!(pf.column_bytes(0, 0), Err(ScanError::LevelsPresent(1)));

    // UnsupportedPage: flip the compressed flag (reject id 1).
    let poisoned = rewrite_page_header(&file, |_h, v2| {
        v2.is_compressed = true;
    });
    let tmp = TempFile::write("compressed", &poisoned);
    let pf = ParquetFile::open(&tmp.path).expect("footer still parses");
    assert_eq!(pf.column_bytes(0, 0), Err(ScanError::UnsupportedPage(1)));

    // OffsetMismatch via public accessors on a VALID file: row out of range
    // and row group out of range are typed refusals, never neighbor reads.
    let tmp = TempFile::write("valid", &file);
    let pf = ParquetFile::open(&tmp.path).expect("open");
    assert_eq!(pf.value_u32(0, 0, rows), Err(ScanError::OffsetMismatch));
    assert_eq!(pf.column_bytes(1, 0), Err(ScanError::OffsetMismatch));

    // Thrift: a page region of delta-form field headers (0x81: bool type,
    // delta 8) skews the field-id walk into unknown-field skips; the parse
    // then consumes the footer's STOP byte as the end of the page-header
    // struct, so no page_type was ever read -> mapped through
    // ScanError::Thrift(MissingRequiredField(1)), never a misparse.
    let mut garbage_page = footer_flba(0, 32);
    for b in &mut garbage_page[4..12] {
        *b = 0x81;
    }
    let tmp = TempFile::write("thrift", &garbage_page);
    let pf = ParquetFile::open(&tmp.path).expect("footer parses; only the page is garbage");
    assert_eq!(
        pf.column_bytes(0, 0),
        Err(ScanError::Thrift(ThriftError::MissingRequiredField(1)))
    );

    // Page (DEAD VARIANT — finding): no code path in avec-parquet constructs
    // ScanError::Page; only its Display arm exists (scan.rs). No public API
    // can produce it, so the contract is pinned directly here and the matrix
    // flags it for deletion or wiring.
    let dead = ScanError::Page(ParquetError::UnexpectedEof);
    assert_eq!(dead, ScanError::Page(ParquetError::UnexpectedEof));
    assert!(dead.to_string().contains("UnexpectedEof"));
}

// --------------------------------------------------- LevelFreeError courts --

/// Writer/admission refusals of the level-free projection: one per schema
/// shape outside the projection, plus page-shape refusals.
#[test]
fn level_free_error_paths() {
    let elem = |name: &[u8], ty: Option<i32>, tlen: Option<i32>, rep: Option<i32>| {
        WireSchemaElement {
            field_type: ty,
            type_length: tlen,
            repetition_type: rep,
            name: name.to_vec(),
            num_children: None,
        }
    };

    // OptionalField.
    assert_eq!(
        admit_schema(&[elem(b"opt", Some(ptype::INT32), None, Some(repetition::OPTIONAL))]),
        Err(LevelFreeError::OptionalField { index: 0, name: b"opt".to_vec() })
    );

    // RepeatedField.
    assert_eq!(
        admit_schema(&[elem(b"rep", Some(ptype::INT64), None, Some(repetition::REPEATED))]),
        Err(LevelFreeError::RepeatedField { index: 0, name: b"rep".to_vec() })
    );

    // NestedField: a group (no physical type) after the root — refused at
    // index 1, proving the index names the offending element.
    let root = WireSchemaElement {
        field_type: None,
        type_length: None,
        repetition_type: None,
        name: b"schema".to_vec(),
        num_children: Some(1),
    };
    let group = WireSchemaElement {
        field_type: None,
        type_length: None,
        repetition_type: Some(repetition::REQUIRED),
        name: b"grp".to_vec(),
        num_children: Some(2),
    };
    assert_eq!(
        admit_schema(&[root, group]),
        Err(LevelFreeError::NestedField { index: 1, name: b"grp".to_vec() })
    );

    // UnsupportedType: DOUBLE is outside {INT32, INT64, FLBA(32)}.
    assert!(matches!(
        admit_schema(&[elem(b"f", Some(ptype::DOUBLE), None, Some(repetition::REQUIRED))]),
        Err(LevelFreeError::UnsupportedType { index: 0, code: ptype::DOUBLE, .. })
    ));

    // UnsupportedWidth: FLBA(16) is not the admitted width 32.
    assert!(matches!(
        admit_schema(&[elem(
            b"g",
            Some(ptype::FIXED_LEN_BYTE_ARRAY),
            Some(16),
            Some(repetition::REQUIRED)
        )]),
        Err(LevelFreeError::UnsupportedWidth { index: 0, length: 16, .. })
    ));

    // UnknownRepetition: code 3 is outside the parquet.thrift range.
    assert!(matches!(
        admit_schema(&[elem(b"u", Some(ptype::INT32), None, Some(3))]),
        Err(LevelFreeError::UnknownRepetition { index: 0, code: 3, .. })
    ));

    // UndecodablePage: a garbage page fails its header parse on read-back.
    assert_eq!(
        read_page_payload(&[0x81u8; 16], LevelFreeType::U32),
        Err(LevelFreeError::UndecodablePage)
    );

    // PayloadMismatch: 9 rows of u32 declare 36 bytes; 40 offered.
    assert_eq!(
        write_page(LevelFreeType::U32, &[0u8; 40], 9),
        Err(LevelFreeError::PayloadMismatch { expected: 36, actual: 40 })
    );
}

// ------------------------------------------------------ AllocReport court --

/// `avec_audit::alloc_counter::AllocReport` from a foreign crate: the
/// tracking scopes report zero/nonzero/nested-delta counts, and a typed
/// error path (checksum refusal) is allocation-free.
#[test]
fn alloc_report_scopes_and_error_path() {
    // Zero-alloc scope.
    let (v, report) = with_alloc_tracking(|| {
        let mut acc = 0u64;
        for i in 0..1000u64 {
            acc = acc.wrapping_mul(3).wrapping_add(i);
        }
        acc
    });
    assert_ne!(v, u64::MAX, "closure must not be optimized into nothing");
    assert!(report.is_zero(), "stack-only scope allocated: {report:?}");
    assert_eq!(report.bytes, 0);

    // Allocating scope: count and bytes both move.
    let (len, report) = with_alloc_tracking(|| {
        let mut v = Vec::with_capacity(0);
        for i in 0..100u64 {
            v.push(i);
        }
        v.len()
    });
    assert_eq!(len, 100);
    assert!(report.count >= 1, "expected allocations, got {report:?}");
    assert!(report.bytes >= 800, "100 pushed u64s: {report:?}");

    // Nested scopes: the inner report excludes the outer's allocations.
    let ((_inner_v, inner), outer) = with_alloc_tracking(|| {
        let _pad = vec![0u8; 4096];
        with_alloc_tracking(|| vec![1u8; 8])
    });
    assert_eq!(_inner_v.len(), 8);
    assert!(inner.count >= 1, "inner scope lost its alloc: {inner:?}");
    assert!(outer.count > inner.count, "outer {outer:?} vs inner {inner:?}");

    // Error path is allocation-free: parse_pack_v2 refuses on the trailer
    // checksum BEFORE building the object table, and SHA-1 works on the
    // stack — a typed refusal costs zero allocations.
    let blob: Vec<u8> = (0..48u32).map(|i| (i.wrapping_mul(7).wrapping_add(1)) as u8).collect();
    let mut pack = write_pack_v2(&[&blob]).expect("valid pack");
    pack[15] ^= 0xFF;
    let (res, report) = with_alloc_tracking(|| parse_pack_v2(&pack));
    assert_eq!(res, Err(GitError::PackChecksumMismatch));
    assert!(report.is_zero(), "checksum refusal allocated: {report:?}");
}
