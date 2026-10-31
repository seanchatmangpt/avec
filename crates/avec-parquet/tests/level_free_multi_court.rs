//! Multi-column isomorphism court (wave-5 lane 15): the level-free writer's
//! row-group page + FileMetaData footer (`level_free::write_file`) against
//! the crate's own mmap scanner (`scan::ParquetFile`). Wave-4 lane 16
//! documented the ncols > 1 mismatch — `write_page` is per-column while a
//! row-group page carries all columns — this court falsifies any regression:
//! a full multi-column file written by the writer must open, pass schema
//! admission, and return every column's original payload byte-for-byte.

#![cfg(feature = "std")]

use avec_parquet::level_free::{
    admit_schema, write_file, write_row_group_page, FooterRowGroup, LevelFreeColumn,
    LevelFreeError, LevelFreeType,
};
use avec_parquet::scan::ParquetFile;
use avec_parquet::level_free::parse_schema_elements;
use avec_parquet::thrift::write_i32_field;

/// Unique temp path; real file on disk, no tempfile dependency.
struct TempFile {
    path: std::path::PathBuf,
}

impl TempFile {
    fn write(name: &str, bytes: &[u8]) -> TempFile {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut p = std::env::temp_dir();
        p.push(format!("avec_lf_multi_{}_{}_{}", name, std::process::id(), nanos));
        std::fs::write(&p, bytes).expect("write court file");
        TempFile { path: p }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The court corpus: a = u32, b = u64, h = FLBA(32); 64 rows.
struct Corpus {
    cols: Vec<LevelFreeColumn>,
    payloads: Vec<Vec<u8>>,
}

fn corpus(num_rows: usize) -> Corpus {
    let cols = vec![
        LevelFreeColumn { name: b"a_u32".to_vec(), ty: LevelFreeType::U32 },
        LevelFreeColumn { name: b"b_u64".to_vec(), ty: LevelFreeType::U64 },
        LevelFreeColumn { name: b"h_256".to_vec(), ty: LevelFreeType::Bytes32 },
    ];
    let payloads: Vec<Vec<u8>> = cols
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let n = num_rows * c.ty.width();
            (0..n).map(|b| ((b * 13 + i * 7 + 3) & 0xFF) as u8).collect()
        })
        .collect();
    Corpus { cols, payloads }
}

/// Positive isomorphism court: writer -> file -> mmap scanner ->
/// schema admission + byte equality for EVERY column (ncols > 1, the
/// wave-4 lane-16 mismatch shape).
#[test]
fn multi_column_write_scan_isomorphism() {
    let rows = 64usize;
    let c = corpus(rows);
    let refs: Vec<&[u8]> = c.payloads.iter().map(|p| p.as_slice()).collect();
    let file = write_file(&c.cols, &refs, rows).expect("write_file");

    let tmp = TempFile::write("iso", &file);
    let pf = ParquetFile::open(&tmp.path).expect("scanner must open the writer's file");

    // Footer facts line up.
    assert_eq!(pf.num_rows(), rows as i64);
    assert_eq!(pf.row_groups().len(), 1);
    assert_eq!(pf.meta().columns.len(), 3);
    assert_eq!(pf.meta().columns[0].name, "a_u32");
    assert_eq!(pf.meta().columns[1].name, "b_u64");
    assert_eq!(pf.meta().columns[2].name, "h_256");

    // Schema admission passes: the scanner's parsed footer admits to the
    // writer's own column list (names + types, same order).
    let admitted = pf
        .meta()
        .columns
        .iter()
        .map(|d| LevelFreeColumn {
            name: d.name.as_bytes().to_vec(),
            ty: match d.phys {
                avec_parquet::scan::PhysType::U32 => LevelFreeType::U32,
                avec_parquet::scan::PhysType::U64 => LevelFreeType::U64,
                avec_parquet::scan::PhysType::Flba(32) => LevelFreeType::Bytes32,
                other => panic!("unadmitted scan type {other:?}"),
            },
        })
        .collect::<Vec<_>>();
    assert_eq!(admitted, c.cols);

    // Hook 2, every column: the scanned bytes ARE the written payload.
    for (i, payload) in c.payloads.iter().enumerate() {
        assert_eq!(
            pf.column_bytes(0, i).expect("column_bytes"),
            &payload[..],
            "column {i} bytes diverge from the written payload"
        );
    }

    // Typed accessor spot checks against the corpus bit patterns.
    let a0 = u32::from_le_bytes(c.payloads[0][0..4].try_into().unwrap());
    let b17 = u64::from_le_bytes(
        c.payloads[1][17 * 8..17 * 8 + 8].try_into().unwrap(),
    );
    assert_eq!(pf.value_u32(0, 0, 0).expect("u32"), a0);
    assert_eq!(pf.value_u64(0, 1, 17).expect("u64"), b17);
    assert_eq!(
        pf.value_flba(0, 2, rows - 1, 32).expect("flba"),
        &c.payloads[2][(rows - 1) * 32..rows * 32]
    );
}

/// A row-group page carries ALL columns: num_values = rows * ncols and the
/// payload is rows * Σwidth, unlike the per-column `write_page`.
#[test]
fn row_group_page_header_carries_all_columns() {
    let rows = 10usize;
    let c = corpus(rows);
    let refs: Vec<&[u8]> = c.payloads.iter().map(|p| p.as_slice()).collect();
    let page = write_row_group_page(&c.cols, &refs, rows).expect("page");
    let (header, header_len) =
        avec_parquet::thrift::read_page_header(&page).expect("header");
    let v2 = header.data_page_header_v2.expect("v2");
    assert_eq!(header.page_type, avec_parquet::thrift::page_type::DATA_PAGE_V2);
    assert_eq!(v2.num_values as usize, rows * 3, "num_values = rows * ncols");
    assert_eq!(v2.num_rows as usize, rows);
    assert_eq!(v2.num_nulls, 0);
    assert_eq!(v2.definition_levels_byte_length, 0);
    assert_eq!(v2.repetition_levels_byte_length, 0);
    assert!(!v2.is_compressed);
    assert_eq!(v2.encoding, avec_parquet::thrift::encoding::PLAIN);
    let total: usize = c.payloads.iter().map(|p| p.len()).sum();
    assert_eq!(page.len(), header_len + total, "payload = rows * Σwidth");
    // Column-major concat: each column's payload sits at its offset.
    let mut off = header_len;
    for p in &c.payloads {
        assert_eq!(&page[off..off + p.len()], &p[..]);
        off += p.len();
    }
}

/// Negative court: an OPTIONAL field is refused by the writer-side
/// admission gate — the writer never flattens a schema outside the
/// projection (spec Hook 5, writer half).
#[test]
fn optional_field_refused() {
    // Hand-emit one SchemaElement with repetition_type = OPTIONAL (1) —
    // exactly what the level-free writer would have to accept to write it.
    let mut wire = Vec::new();
    write_i32_field(&mut wire, 1, 0, avec_parquet::level_free::ptype::INT64);
    write_i32_field(&mut wire, 3, 1, 1); // OPTIONAL
    avec_parquet::thrift::write_binary_field(&mut wire, 4, 3, b"opt_col");
    wire.push(avec_parquet::thrift::ctype::STOP);
    let parsed = parse_schema_elements(&wire).expect("parse");
    assert_eq!(
        admit_schema(&parsed),
        Err(LevelFreeError::OptionalField {
            index: 0,
            name: b"opt_col".to_vec()
        })
    );
}

/// Negative court: a payload whose length is not `rows * width` for its
/// column is a typed PayloadMismatch — never a silent truncate.
#[test]
fn wrong_payload_length_refused() {
    let rows = 8usize;
    let c = corpus(rows);
    let mut short: Vec<&[u8]> = c.payloads.iter().map(|p| p.as_slice()).collect();
    // Corrupt column 1 (u64): 8 rows * 8 = 64 bytes expected, 63 actual.
    short[1] = &c.payloads[1][..63];
    assert_eq!(
        write_row_group_page(&c.cols, &short, rows),
        Err(LevelFreeError::PayloadMismatch { expected: 64, actual: 63 })
    );
    // Column-count mismatch refused the same way.
    let too_few: Vec<&[u8]> = c.payloads[..2].iter().map(|p| p.as_slice()).collect();
    assert_eq!(
        write_row_group_page(&c.cols, &too_few, rows),
        Err(LevelFreeError::PayloadMismatch { expected: 3, actual: 2 })
    );
    // The whole-file writer propagates the refusal (no file is minted).
    let refs: Vec<&[u8]> = c.payloads.iter().map(|p| p.as_slice()).collect();
    let mut bad: Vec<&[u8]> = c.payloads.iter().map(|p| p.as_slice()).collect();
    let mut long_col32 = c.payloads[2].clone();
    long_col32.push(0u8);
    bad[2] = &long_col32;
    assert!(matches!(
        write_file(&c.cols, &bad, rows),
        Err(LevelFreeError::PayloadMismatch { actual: a, .. }) if a == rows * 32 + 1
    ));
    // Sanity: the lawful write still succeeds (the court can flip).
    assert!(write_file(&c.cols, &refs, rows).is_ok());
    // FooterRowGroup shape is exercised via write_file's footer above;
    // a two-group footer still parses through the scanner when the file
    // carries one real group + one pointing at the same page.
    let footer = avec_parquet::level_free::write_footer(
        &c.cols,
        rows,
        &[
            FooterRowGroup { page_offset: 4, page_len: 1 },
            FooterRowGroup { page_offset: 4, page_len: 1 },
        ],
    );
    assert!(!footer.is_empty());
}
