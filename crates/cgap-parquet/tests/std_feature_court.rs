//! Anti-vacuity court for the `std` feature gate (wave-7 lane 6).
//!
//! Anomaly under investigation: an orchestrator gate
//! `cargo test -p cgap-parquet --features std` was read as "exit 0 but 0
//! tests collected." Reproduction on this tree shows the std config collects
//! 201 tests and the default config 191, all passing; the "0 tests" line is
//! the doc-test harness section (the LAST `test result` line in the stream),
//! a parse artifact — not a wiring defect. Cargo.toml declares the `std`
//! feature, `scan` is correctly `#[cfg(feature = "std")]`-gated
//! (src/lib.rs:40), and the one std-gated integration file
//! (`level_free_multi_court.rs`) is gated in the correct direction.
//!
//! This court is the non-vacuity witness, not the fix: it is NOT
//! feature-gated (it is counted under BOTH configs), and the std-gated test
//! below exercises the full std-only surface — real file on disk, real mmap,
//! real footer parse, byte-equality on scanned columns — so an edit that
//! inverts the gate (std feature going decorative again, E0433 class) fails
//! loudly here instead of passing vacuously at the gate.

use cgap_parquet::rle::rle_encode;

/// Runs under BOTH feature configs: proves the lib links and a core
/// (no_std-safe) API executes with and without `std`. If a future edit makes
/// the default build fail to collect tests, the default gate count drops and
/// this binary's presence keeps the inventory nonempty.
#[test]
fn lib_links_and_core_api_runs_under_both_configs() {
    let src: Vec<u32> = (0..64)
        .map(|i| (i as u32).wrapping_mul(0x9E37_79B9) & 0xF)
        .collect();
    let enc = rle_encode(&src, 4);
    assert!(!enc.is_empty());
}

#[cfg(feature = "std")]
mod std_only {
    use cgap_parquet::scan::{
        write_fixture_file, FixtureColumn, ParquetFile, PhysType,
    };

    /// Real file on disk (no tempfile dependency — the crate is
    /// zero-dependency by charter); unique per-run path, removed on drop.
    struct TempFile {
        path: std::path::PathBuf,
    }

    impl TempFile {
        fn write(name: &str, bytes: Vec<u8>) -> TempFile {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let mut p = std::env::temp_dir();
            p.push(format!("cgap_std_court_{}_{}_{}", name, std::process::id(), nanos));
            std::fs::write(&p, bytes).expect("write fixture");
            TempFile { path: p }
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// The std-only surface end to end: fixture bytes -> real file -> mmap
    /// open -> footer index -> zero-copy column slice, byte equality.
    /// Chicago-style: real collaborators (filesystem, mmap syscalls), assert
    /// final state.
    #[test]
    fn std_scan_surface_runs_end_to_end_under_std_feature() {
        let rows = 32usize;
        let a: Vec<u32> = (0..rows as u32).map(|i| i.wrapping_mul(0x9E37_79B9)).collect();
        let a_bytes: Vec<u8> = a.iter().flat_map(|x| x.to_le_bytes()).collect();
        let cols = vec![FixtureColumn {
            name: "a".into(),
            phys: PhysType::U32,
            bytes: a_bytes.clone(),
        }];

        let file = write_fixture_file(&cols, rows);
        let tmp = TempFile::write("e2e", file);
        let pf = ParquetFile::open(&tmp.path).expect("std ParquetFile::open");

        assert_eq!(pf.num_rows(), rows as i64);
        assert_eq!(pf.meta().columns.len(), 1);
        assert_eq!(pf.meta().columns[0].name, "a");
        assert_eq!(pf.row_groups().len(), 1);
        assert_eq!(pf.column_bytes(0, 0).expect("column bytes"), &a_bytes[..]);
        for (i, want) in a.iter().enumerate() {
            assert_eq!(pf.value_u32(0, 0, i).expect("value"), *want);
        }
        // Typed refusal on out-of-range row (fail-closed, not a panic).
        assert!(pf.value_u32(0, 0, rows).is_err());
    }
}
