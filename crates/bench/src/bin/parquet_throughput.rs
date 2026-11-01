//! Parquet level-free mmap-scan throughput baseline (wave-7 lane 16).
//!
//! The 74 GiB/s "sustained decode" claim (spec
//! `docs/jira/v26.10.31/24-parquet-projection-spec.md` lines 61/100) is
//! LEGACY-UNVERIFIED: no bin ever measured the level-free *scan* path end
//! to end. This bin produces the real number and states honestly what each
//! mode measures:
//!
//! - `accessor-scan`: full read of every column through the typed row
//!   accessors (`value_flba` per row). Since the wave-8 slice-cache fix,
//!   the page header is parsed once per (row_group, column) and the slice
//!   bounds memoized, so this measures typed-accessor dispatch over cached
//!   bounds amortized over a 32-byte row read — it is NOT a byte-throughput
//!   number and will be far below any memory ceiling. Reported for
//!   honesty, not for the headline.
//! - `payload-sum`: per column, ONE `column_bytes` call, then sum every
//!   byte of the returned zero-copy slice (u64-lane fold, black_boxed).
//!   This is the memcpy-equivalent ceiling comparator the spec number
//!   should be judged against: page-cache-hot mmap reads at memory speed.
//!
//! Fixture: 4 x FLBA(32) columns, 2^23 rows, 256 MiB per column, 1 GiB
//! page payload exactly, written with the crate's own
//! `level_free::write_file` (writer/reader isomorphism). Payload bytes are
//! a cheap keyed pattern (never zero — a zero page would let the fold
//! short-circuit into a sum of zeros; the accumulation is data-independent
//! but the pattern keeps the read honest against any zero-page elision).
//!
//! Machine-load caveat: this ran on a shared laptop under concurrent
//! fan-out lanes; treat the min-of-rounds as the defensible figure, the
//! mean as the load witness.

use cgap_parquet::level_free::{write_file, LevelFreeColumn, LevelFreeType};
use cgap_parquet::scan::ParquetFile;
use std::fs;
use std::hint::black_box;
use std::time::Instant;

const ROWS: usize = 1 << 23; // 8,388,608
const COLS: usize = 4;
const WIDTH: usize = 32; // FLBA(32)
const PAYLOAD: usize = ROWS * COLS * WIDTH; // 1 GiB exactly
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Min-of-N rounds; concurrent lanes make the host noisy.
const ROUNDS: usize = 7;
/// Accessor mode reads run over cached slice bounds: 2 rounds, disclosed.
const ROUNDS_ACCESSOR: usize = 2;

fn main() {
    println!("== parquet level-free scan throughput baseline ==");
    println!(
        "fixture: {COLS} x FLBA(32) cols, {ROWS} rows, {} MiB/col, payload {} bytes",
        ROWS * WIDTH / (1024 * 1024),
        PAYLOAD
    );

    // --- build the fixture -------------------------------------------------
    let cols: Vec<LevelFreeColumn> = (0..COLS)
        .map(|i| LevelFreeColumn {
            name: format!("c{i}_b32").into_bytes(),
            ty: LevelFreeType::Bytes32,
        })
        .collect();
    let mut payloads: Vec<Vec<u8>> = Vec::with_capacity(COLS);
    for i in 0..COLS {
        let key = 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(i as u64 + 1);
        payloads.push(
            (0..ROWS * WIDTH)
                .map(|j| {
                    let v = (j as u64)
                        .wrapping_mul(key)
                        .wrapping_add(0xD1B5_4A32_D192_ED03);
                    (v >> 13) as u8
                })
                .collect(),
        );
    }
    let refs: Vec<&[u8]> = payloads.iter().map(|p| p.as_slice()).collect();
    let file_bytes = write_file(&cols, &refs, ROWS).expect("level_free write_file");
    // Page payload (1 GiB) + PAR1 magic + header bytes + footer.
    assert!(file_bytes.len() > PAYLOAD, "file must hold the full payload");
    let on_disk = file_bytes.len();

    let mut path = std::env::temp_dir();
    path.push(format!("cgap_parquet_tp_{}.parquet", std::process::id()));
    fs::write(&path, &file_bytes).expect("write fixture file");
    let path_copy = path.clone();

    // Free the writer's heap copies so the scan measures only the mmap,
    // not our heap.
    drop(payloads);
    drop(file_bytes);

    let scan = ParquetFile::open(&path).expect("open+map+footer");
    assert_eq!(scan.num_rows(), ROWS as i64);
    assert_eq!(scan.meta().columns.len(), COLS);
    assert_eq!(scan.row_groups().len(), 1);
    println!("open: ok (footer cached, {} bytes on disk)", on_disk);

    // --- mode: payload-sum (ceiling comparator, the headline) --------------
    // One column_bytes per column, then touch every byte of the slice.
    let run_payload_sum = || -> u64 {
        let mut acc: u64 = 0;
        for c in 0..COLS {
            let s = black_box(scan.column_bytes(0, c).expect("column_bytes"));
            let (lanes, tail) = s.as_chunks::<8>();
            for lane in lanes {
                acc = acc.wrapping_add(u64::from_le_bytes(*lane));
            }
            for b in tail {
                acc = acc.wrapping_add(*b as u64);
            }
        }
        acc
    };

    // Warm the map (first-touch page faults must not sit inside a round).
    let warm = run_payload_sum();
    assert_ne!(warm, u64::MAX, "fold ran");

    let mut times: Vec<f64> = Vec::with_capacity(ROUNDS);
    let mut checksum = 0u64;
    for _ in 0..ROUNDS {
        let t = Instant::now();
        checksum = checksum.wrapping_add(run_payload_sum());
        times.push(t.elapsed().as_secs_f64());
    }
    let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "PAYLOAD-SUM: {ROUNDS} rounds, min {:.1} GiB/s, mean {:.1} GiB/s (checksum {:016x})",
        PAYLOAD as f64 / GIB / min,
        PAYLOAD as f64 / GIB / mean,
        checksum
    );

    // --- mode: accessor-scan (typed per-row reads, honesty number) ---------
    let run_accessor = || -> u64 {
        let mut acc: u64 = 0;
        for c in 0..COLS {
            for r in 0..ROWS {
                let v = black_box(scan.value_flba(0, c, r, WIDTH).expect("value_flba"));
                acc = acc.wrapping_add(v[0] as u64);
            }
        }
        acc
    };
    let _ = run_accessor(); // warm-up
    let mut atimes: Vec<f64> = Vec::with_capacity(ROUNDS_ACCESSOR);
    for _ in 0..ROUNDS_ACCESSOR {
        let t = Instant::now();
        let ck = run_accessor();
        atimes.push(t.elapsed().as_secs_f64());
        black_box(ck);
    }
    let amin = atimes.iter().cloned().fold(f64::INFINITY, f64::min);
    let amean = atimes.iter().sum::<f64>() / atimes.len() as f64;
    println!(
        "ACCESSOR-SCAN: {ROUNDS_ACCESSOR} rounds (33.5M per-row value_flba calls over cached \
         slice bounds — page header parsed once per (row_group, column)), min {:.3} GiB/s, \
         mean {:.3} GiB/s — measures accessor dispatch over cached bounds, \
         NOT decode throughput",
        PAYLOAD as f64 / GIB / amin,
        PAYLOAD as f64 / GIB / amean,
    );

    let _ = fs::remove_file(path_copy);
    println!("done (fixture removed)");
}
