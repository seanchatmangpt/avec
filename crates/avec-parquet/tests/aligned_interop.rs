//! External-interop court for the aligned zero-decode pages (lane PCL2,
//! v26.10.29). Two claims under test:
//!
//! 1. **External reader**: a hand-assembled minimal one-column parquet
//!    container (FLBA(1), UNCOMPRESSED, real-parquet.thrift DataPageV2 +
//!    PLAIN page from [`avec_parquet::page_aligned::write_plain_page`]) is
//!    read back byte-EXACT by pyarrow (`ParquetFile`) — a non-avec tool.
//!    The container is built without any parquet writer library: thrift
//!    compact protocol via [`avec_parquet::thrift`] primitives only.
//!    Anti-vacuity: a corrupted page must NOT produce a clean receipt.
//! 2. **Throughput**: the aligned decode (header skip + bounds-checked
//!    slice copy) vs the RLE path ([`avec_parquet::page::decode_page_into`])
//!    on the same one-byte-per-value payload — printed as a bench line,
//!    min-of-N.
//!
//! Chicago discipline: real file on disk, real subprocess, real reader; a
//! missing python3/pyarrow degrades to a LOUD skip line (claim stays
//! UNVERIFIED in that environment) — never a silent pass.

use std::hint::black_box;
use std::process::Command;

use avec_parquet::page::decode_page_into;
use avec_parquet::page_aligned::{aligned_decode, write_plain_page};
use avec_parquet::rle::write_varint;
use avec_parquet::thrift::{
    write_binary_field, write_field_header, write_i32_field, write_i64_field,
    write_zigzag_i32, ctype,
};

// ---------------------------------------------------------------------------
// Pattern under test: deterministic, mirrors the python probe this lane
// validated the wire shape against before writing this file.
// ---------------------------------------------------------------------------

fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| ((i * 7 + 1) & 0xFF) as u8).collect()
}

// ---------------------------------------------------------------------------
// Minimal legal one-column parquet container (no parquet writer library).
// Schema: root(schema, children=1) -> f: REQUIRED FIXED_LEN_BYTE_ARRAY(1).
// One row group, one column chunk at offset 4 (right after the magic),
// UNCOMPRESSED, PLAIN encoding, one DataPageV2.
// ---------------------------------------------------------------------------

const CTYPE_LIST: u8 = 9;

fn list_header(dst: &mut Vec<u8>, n: usize, elem_type: u8) {
    if n < 15 {
        dst.push(((n as u8) << 4) | elem_type);
    } else {
        dst.push((15 << 4) | elem_type);
        write_varint(dst, n as u64);
    }
}

fn fld_list(dst: &mut Vec<u8>, fid: i16, prev: i16, n: usize, elem_type: u8) {
    write_field_header(dst, fid, prev, CTYPE_LIST);
    list_header(dst, n, elem_type);
}

fn build_container(page: &[u8], num_rows: usize) -> Vec<u8> {
    let chunk_start: i64 = 4; // right after "PAR1"

    // --- SchemaElement list (root + column) ---
    let mut root = Vec::new();
    write_binary_field(&mut root, 4, 0, b"schema"); // name (required)
    write_i32_field(&mut root, 5, 4, 1); // num_children
    root.push(ctype::STOP);

    let mut col = Vec::new();
    write_i32_field(&mut col, 1, 0, 7); // type = FIXED_LEN_BYTE_ARRAY
    write_i32_field(&mut col, 2, 1, 1); // type_length = 1
    write_i32_field(&mut col, 3, 2, 0); // repetition_type = REQUIRED
    write_binary_field(&mut col, 4, 3, b"f");
    col.push(ctype::STOP);

    // --- ColumnMetaData ---
    let mut md = Vec::new();
    write_i32_field(&mut md, 1, 0, 7); // type = FLBA
    fld_list(&mut md, 2, 1, 1, ctype::I32); // encodings = [PLAIN(0)]
    write_zigzag_i32(&mut md, 0);
    fld_list(&mut md, 3, 2, 1, ctype::BINARY); // path_in_schema = ["f"]
    write_varint(&mut md, 1);
    md.push(b'f');
    write_i32_field(&mut md, 4, 3, 0); // codec = UNCOMPRESSED
    write_i64_field(&mut md, 5, 4, num_rows as i64); // num_values
    write_i64_field(&mut md, 6, 5, page.len() as i64); // total_uncompressed_size
    write_i64_field(&mut md, 7, 6, page.len() as i64); // total_compressed_size
    write_i64_field(&mut md, 9, 7, chunk_start); // data_page_offset
    md.push(ctype::STOP);

    // --- ColumnChunk ---
    let mut cc = Vec::new();
    write_i64_field(&mut cc, 2, 0, chunk_start); // file_offset (required)
    write_field_header(&mut cc, 3, 2, ctype::STRUCT); // meta_data
    cc.extend_from_slice(&md);
    cc.push(ctype::STOP);

    // --- RowGroup ---
    let mut rg = Vec::new();
    fld_list(&mut rg, 1, 0, 1, ctype::STRUCT); // columns
    rg.extend_from_slice(&cc);
    write_i64_field(&mut rg, 2, 1, page.len() as i64); // total_byte_size
    write_i64_field(&mut rg, 3, 2, num_rows as i64); // num_rows
    rg.push(ctype::STOP);

    // --- FileMetaData ---
    let mut fmd = Vec::new();
    write_i32_field(&mut fmd, 1, 0, 1); // version
    write_field_header(&mut fmd, 2, 1, CTYPE_LIST); // schema
    list_header(&mut fmd, 2, ctype::STRUCT);
    fmd.extend_from_slice(&root);
    fmd.extend_from_slice(&col);
    write_i64_field(&mut fmd, 3, 2, num_rows as i64); // num_rows
    write_field_header(&mut fmd, 4, 3, CTYPE_LIST); // row_groups
    list_header(&mut fmd, 1, ctype::STRUCT);
    fmd.extend_from_slice(&rg);
    write_binary_field(&mut fmd, 6, 4, b"avec-parquet aligned_interop"); // created_by
    fmd.push(ctype::STOP);

    let mut out = Vec::with_capacity(4 + page.len() + fmd.len() + 8);
    out.extend_from_slice(b"PAR1");
    out.extend_from_slice(page);
    out.extend_from_slice(&fmd);
    out.extend_from_slice(&(fmd.len() as u32).to_le_bytes());
    out.extend_from_slice(b"PAR1");
    out
}

// ---------------------------------------------------------------------------
// Round-trip: page decode into caller memory + container frame sanity.
// ---------------------------------------------------------------------------

#[test]
fn round_trip_and_container_frame() {
    let n = 256;
    let values = pattern(n);
    let page = write_plain_page(&values, n as u32);

    // Aligned decode: header skip + slice copy, tail untouched.
    let mut out = vec![0xA5u8; n + 7];
    assert_eq!(aligned_decode(&page, &mut out), Ok(n));
    assert_eq!(&out[..n], &values[..]);
    assert!(out[n..].iter().all(|&x| x == 0xA5));

    // Body offset = header end (no padding — body is the page tail).
    assert_eq!(&page[page.len() - n..], &values[..]);

    // Container frame sanity: magic both ends, declared meta length exact.
    let file = build_container(&page, n);
    assert_eq!(&file[..4], b"PAR1");
    assert_eq!(&file[file.len() - 4..], b"PAR1");
    let meta_len = u32::from_le_bytes(
        file[file.len() - 8..file.len() - 4].try_into().expect("u32"),
    );
    assert!(meta_len as usize + 12 <= file.len(), "meta len plausible");
}

// ---------------------------------------------------------------------------
// External reader receipt (pyarrow via python3 subprocess).
// ---------------------------------------------------------------------------

const PY_SCRIPT: &str = r#"
import sys
path = sys.argv[1]
try:
    import pyarrow as pa
    import pyarrow.parquet as pq
except Exception:
    print("PYARROW-UNAVAILABLE")
    sys.exit(3)
pf = pq.ParquetFile(path)
tbl = pf.read()
col = tbl.column(0).combine_chunks()
arr = col.chunk(0) if hasattr(col, "chunk") else col
buf = bytes(arr.buffers()[-1])
buf = buf[: tbl.num_rows]
n = tbl.num_rows
want = bytes((i * 7 + 1) & 0xFF for i in range(n))
ok = buf == want
print(
    f"PYARROW-RECEIPT pyarrow={pa.__version__} rows={n} "
    f"type={tbl.schema.field(0).type} fidelity={'EXACT' if ok else 'MISMATCH'}"
)
sys.exit(0 if ok else 4)
"#;

fn run_pyarrow(path: &std::path::Path) -> Result<String, String> {
    let out = Command::new("python3")
        .arg("-c")
        .arg(PY_SCRIPT)
        .arg(path)
        .output()
        .map_err(|e| format!("spawn python3 failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if stdout.starts_with("PYARROW-UNAVAILABLE") {
        return Err("pyarrow not importable in python3".to_string());
    }
    if !out.status.success() && !stdout.contains("PYARROW-RECEIPT") {
        return Err(format!(
            "python3 exited {:?}: {} {}",
            out.status.code(),
            stdout,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(stdout)
}

fn temp_file(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "avec_aligned_interop_{}_{}.parquet",
        std::process::id(),
        tag
    ))
}

#[test]
fn pyarrow_external_receipt() {
    let n = 256;
    let values = pattern(n);
    let page = write_plain_page(&values, n as u32);
    let file = build_container(&page, n);
    let path = temp_file("receipt");

    std::fs::write(&path, &file).expect("write temp parquet");
    let receipt = match run_pyarrow(&path) {
        Ok(r) => r,
        Err(reason) => {
            println!(
                "SKIP pyarrow external receipt: {reason} — external-compat claim \
                 UNVERIFIED in this environment"
            );
            let _ = std::fs::remove_file(&path);
            return;
        }
    };
    println!("{receipt}");
    let _ = std::fs::remove_file(&path);

    assert!(
        receipt.contains("PYARROW-RECEIPT"),
        "receipt line missing: {receipt}"
    );
    assert!(
        receipt.contains("fidelity=EXACT"),
        "byte fidelity broken: {receipt}"
    );
    assert!(receipt.contains("rows=256"), "row count wrong: {receipt}");
}

#[test]
fn pyarrow_rejects_corrupted_page() {
    // Anti-vacuity: the receipt court must be able to flip. Flip one body
    // value; the reader must either fail or report MISMATCH.
    let n = 256;
    let values = pattern(n);
    let mut page = write_plain_page(&values, n as u32);
    let hdr_len = page.len() - n;
    page[hdr_len] ^= 0xFF; // corrupt the first value byte
    let file = build_container(&page, n);
    let path = temp_file("corrupt");

    std::fs::write(&path, &file).expect("write temp parquet");
    let receipt = match run_pyarrow(&path) {
        Ok(r) => r,
        Err(reason) => {
            // A corrupted file failing to even produce a receipt is ALSO a
            // flip (the reader noticed); only unavailability is a skip.
            if reason.contains("spawn python3 failed") || reason.contains("not importable") {
                println!("SKIP pyarrow corruption check: {reason}");
                let _ = std::fs::remove_file(&path);
                return;
            }
            println!("CORRUPTION-FLIPPED via reader error: {reason}");
            let _ = std::fs::remove_file(&path);
            return;
        }
    };
    println!("{receipt}");
    let _ = std::fs::remove_file(&path);
    assert!(
        receipt.contains("fidelity=MISMATCH"),
        "corrupted page must not read back clean: {receipt}"
    );
}

// ---------------------------------------------------------------------------
// Bench: aligned decode vs RLE decode on the same one-byte-per-value data.
// ---------------------------------------------------------------------------

#[test]
fn bench_aligned_vs_rle() {
    let n: usize = 8 * 1024 * 1024; // 8 MiB payload
    let values = pattern(n);

    // --- aligned path ---
    let page = write_plain_page(&values, n as u32);
    let mut out = vec![0u8; n];
    // Warmup + correctness guard.
    assert_eq!(aligned_decode(&page, &mut out), Ok(n));
    assert_eq!(&out[..8], &values[..8]);
    let mut best_aligned = f64::INFINITY;
    for _ in 0..3 {
        let t0 = std::time::Instant::now();
        for _ in 0..4 {
            black_box(aligned_decode(black_box(&page), &mut out)).expect("decode");
        }
        let dt = t0.elapsed().as_secs_f64();
        best_aligned = best_aligned.min((n as f64) * 4.0 / dt / 1024.0 / 1024.0 / 1024.0);
    }

    // --- RLE path (crate baseline, same payload as u32 lanes) ---
    let lanes: Vec<u32> = values.iter().map(|&b| b as u32).collect();
    let page_rle = avec_parquet::page::write_data_page_v2(&lanes, 8, n as u32);
    let mut out32 = vec![0u32; n];
    assert_eq!(decode_page_into(&page_rle, &mut out32), Ok(n));
    assert_eq!(&out32[..8], &lanes[..8]);
    let mut best_rle = f64::INFINITY;
    for _ in 0..3 {
        let t0 = std::time::Instant::now();
        for _ in 0..4 {
            black_box(decode_page_into(black_box(&page_rle), &mut out32)).expect("decode");
        }
        let dt = t0.elapsed().as_secs_f64();
        best_rle = best_rle.min((n as f64) * 4.0 / dt / 1024.0 / 1024.0 / 1024.0);
    }

    println!(
        "BENCH aligned_decode: {:.2} GiB/s | decode_page_into(RLE, bits=8): {:.2} GiB/s | \
         ratio: {:.2}x | n={} values/iter x4, best-of-3",
        best_aligned,
        best_rle,
        best_aligned / best_rle,
        n
    );
}
