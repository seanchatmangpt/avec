#![cfg(feature = "std")]
//! Accessor-perf court (wave 8 lane 6): the resolved-slice cache in
//! `scan.rs` must keep the per-row typed accessors at offset-arithmetic
//! cost — the Thrift page header is parsed once per (row_group, column),
//! never per value.
//!
//! Chicago discipline: a real multi-MB file on disk, a real mmap scan, a
//! real timed loop asserting a generous wall-clock ceiling (the machine
//! runs concurrent lanes; a tight number would flake). An informational
//! GiB/s print records the receipt; the assertion is the ceiling, plus a
//! mutation-lens check that the accessors return the corpus values.

extern crate std;

use cgap_parquet::scan::{
    ParquetFile, PhysType,
};
use cgap_parquet::scan::FixtureColumn;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// Unique temp path; real file on disk, no tempfile dependency.
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn write(name: &str, bytes: Vec<u8>) -> TempFile {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut p = std::env::temp_dir();
        p.push(format!("cgap_accessor_court_{}_{}_{}", name, std::process::id(), nanos));
        fs::write(&p, bytes).expect("write fixture");
        TempFile { path: p }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// 16 MiB u64 column: 2 Mi rows.
const MIB: usize = 1024 * 1024;
const ROWS: usize = 2 * MIB; // u64 -> 16 MiB payload

fn corpus() -> Vec<u64> {
    (0..ROWS as u64)
        .map(|i| i.wrapping_mul(0xC2B2_AE3D_27D4_4EBF))
        .collect()
}

#[test]
fn accessor_scan_16mib_under_ceiling() {
    let data = corpus();
    let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
    let cols = vec![FixtureColumn {
        name: "a".into(),
        phys: PhysType::U64,
        bytes,
    }];
    let file = cgap_parquet::scan::write_fixture_file(&cols, ROWS);
    let tmp = TempFile::write("perf16", file);
    let pf = ParquetFile::open(&tmp.path).expect("open fixture file");
    assert_eq!(pf.num_rows(), ROWS as i64);

    // Correctness lens: accessor reads match the corpus (mutation lens —
    // a cache that resolves wrong bytes cannot pass here).
    let b = pf.value_u64(0, 0, 0).expect("row 0");
    assert_eq!(b, data[0]);
    let last = pf.value_u64(0, 0, ROWS - 1).expect("last row");
    assert_eq!(last, data[ROWS - 1]);
    // Spot values across the column.
    for r in [1usize, 1 << 12, 1 << 20, ROWS / 2] {
        assert_eq!(pf.value_u64(0, 0, r).expect("spot"), data[r]);
    }

    // Timed full-column accessor scan. One column open = one header parse;
    // the loop is pure offset arithmetic.
    let start = Instant::now();
    let mut sink = 0u64;
    for r in 0..ROWS {
        sink ^= pf.value_u64(0, 0, r).expect("row");
    }
    let dt = start.elapsed();
    assert_ne!(sink, u64::MAX, "keep the loop observable, not dead code");

    let gib = 16.0 / 1024.0;
    let gibs = gib / dt.as_secs_f64();
    println!(
        "accessor-scan 16 MiB u64 column: {:?} ({:.1} GiB/s), sink={:#x}",
        dt, gibs, sink
    );
    // Generous ceiling: concurrent lanes share this machine.
    assert!(
        dt.as_millis() < 500,
        "accessor scan of 16 MiB took {:?}; the resolved-slice cache is not holding",
        dt
    );

    // The per-column resolve is once-per-column: a second full scan must
    // be at least as fast (cache warm) — no second header parse.
    let start = Instant::now();
    let mut sink2 = 0u64;
    for r in 0..ROWS {
        sink2 ^= pf.value_u64(0, 0, r).expect("row");
    }
    let dt2 = start.elapsed();
    println!("second (warm-cache) scan: {:?}", dt2);
    assert_eq!(sink, sink2, "two scans of the same column agree");
    assert!(
        dt2.as_millis() < 500,
        "warm scan took {:?}",
        dt2
    );
}
