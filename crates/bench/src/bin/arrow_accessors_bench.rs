//! Arrow accessor micro-benches (lane OPT20-L7, cgap version 26.10.29).
//!
//! Four measurements over the real frozen accessors of `cgap-arrow`:
//!
//! - `varlen u32 row-scan`: [`Varlen::row`] sequential full-scan read. The
//!   64 MiB payload buffer is walked row by row, every payload byte folded,
//!   so the rate is over data bytes (the 4 MiB offsets buffer is read too
//!   but not counted — stated here, not hidden).
//! - `views inline-heavy` vs `views foreign-heavy`: [`ViewVec::row`] at
//!   matched ~64 MiB *touched* scale. A row touches its 16-byte descriptor
//!   (payload included when inline) plus, when foreign, its 64-byte data
//!   block slice; rows per shape are sized so a full scan touches ~64 MiB
//!   either way (`rows ~= 64 MiB / (16 + p_foreign * 64)`). The two shapes
//!   differ only in which 90/10 side of the 12-byte inline threshold they
//!   sit on — this is the branch classifier's real per-row cost.
//! - `sliced get full iter` vs `primitive values direct iter`: zero-copy
//!   overhead check. [`slice::slice`] window (offset 7) over an all-valid
//!   64 MiB `Primitive<i64>` (all-valid freeze drops the validity bitmap,
//!   so `get` is pure pointer arithmetic + bounds + `Option` wrap) against
//!   iterating `Primitive::values()` directly. Checksums asserted equal
//!   pre-flight; expect the rates to agree to noise (~0 overhead).
//! - `runend get` + `runend decode_to_dense`: [`RunEndEncoded::get`]
//!   binary-search decode over a 1,048,576-row / 100-run array (run ends
//!   span 400 B, L1-resident: this is CPU-bound per-row decode cost, not a
//!   bandwidth wall) and full dense decode (4 MiB dst written per pass).
//!
//! Conventions (house pattern from page_bench/parquet_bench): counting
//! GlobalAlloc with a non-vacuity probe, min-of-5 rounds (best round wins;
//! concurrent lanes make this noisy), every result sampled through
//! `black_box`, full pre-flight content asserts BEFORE any timing (a
//! pattern bug must surface as a loud assert, never as a rate).
//!
//! Units: every line carries both GiB/s (over the bytes stated in that
//! line's basis) and Mrows/s (over logical rows walked). State the basis,
//! never make the reader guess.

use cgap_arrow::VERSION_C;
use cgap_arrow::primitive::{Primitive, PrimitiveBuilder};
use cgap_arrow::runend::RunEndEncodedBuilder;
use cgap_arrow::slice::slice;
use cgap_arrow::varlen::{Varlen, VarlenBuilder};
use cgap_arrow::views::{ViewVec, ViewVecBuilder};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 64 MiB payload/touched scale per measurement.
const PAYLOAD: usize = 64 * 1024 * 1024;
/// Min-of-N: best (least-noisy) round wins; concurrent lanes make this noisy.
const ROUNDS: usize = 5;
/// Batch iters for the sub-millisecond runend dense decode stage.
const DECODE_ITERS: usize = 8;
/// (a) row width: 64 B rows over 64 MiB data = 1,048,576 rows.
const VARLEN_ROW_W: usize = 64;
const VARLEN_ROWS: usize = PAYLOAD / VARLEN_ROW_W;
/// (b) per-shape row counts: rows ~= 64 MiB touched / (16 + p * 64) B.
const INLINE_HEAVY_ROWS: usize = PAYLOAD * 10 / (10 * 16 + 64);
const FOREIGN_HEAVY_ROWS: usize = PAYLOAD * 10 / (16 + 9 * 64);
/// (b) widths: 12 B = max inline; 64 B foreign payload.
const INLINE_W: usize = 12;
const FOREIGN_W: usize = 64;
/// (c) 8M i64 = 64 MiB values buffer; window is misaligned on purpose.
const I64_ROWS: usize = PAYLOAD / core::mem::size_of::<i64>();
const SLICE_OFFSET: usize = 7;
/// (d) 2^20 logical rows, exactly 100 runs (10,485 rows x24 + 10,486 x76).
const REE_ROWS: usize = 1 << 20;
const REE_RUNS: usize = 100;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

static COUNT: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: delegates to System, counting calls.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNT.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// One measurement line: warm-up, then min-of-ROUNDS timed rounds of
/// `iters` calls each; allocs come from the winning round divided by
/// `iters`. Both rates share the same basis: `bytes` = bytes this stage
/// genuinely moves (stated per call site), `rows` = logical rows walked.
fn bench_line(op: &str, shape: &str, bytes: u64, rows: u64, iters: usize, mut f: impl FnMut()) {
    f(); // warm-up (page in, branch caches)
    let mut best = f64::INFINITY;
    let mut best_allocs = usize::MAX;
    for _ in 0..ROUNDS {
        COUNT.store(0, Ordering::Relaxed);
        let t0 = Instant::now();
        for _ in 0..iters {
            f();
        }
        let dt = t0.elapsed().as_secs_f64();
        let allocs = COUNT.load(Ordering::Relaxed) / iters;
        if dt < best {
            best = dt;
            best_allocs = allocs;
        }
    }
    let mrows = rows as f64 * iters as f64 / best / 1e6;
    if bytes > 0 {
        let gibps = bytes as f64 * iters as f64 / best / GIB;
        println!("{op} {shape}: {gibps:.3} GiB/s, {mrows:.1} Mrows/s, {best_allocs} allocs/iter");
    } else {
        // CPU-bound stage with no honest byte basis: Mrows/s only, no fake
        // GiB/s column.
        println!("{op} {shape}: {mrows:.1} Mrows/s, {best_allocs} allocs/iter");
    }
}

/// Deterministic W-byte row payload for row `i` (same generator used by the
/// builder loop and the pre-flight expectations — no divergence possible).
fn fixed_row<const W: usize>(i: usize) -> [u8; W] {
    let mut row = [0u8; W];
    for (j, slot) in row.iter_mut().enumerate() {
        *slot = (i ^ j) as u8;
    }
    row
}

/// Foreign-row count for the i%10 pattern (`on_zero`: foreign iff i%10==0).
fn foreign_count(rows: usize, on_zero: bool) -> usize {
    let zeros = rows.div_ceil(10); // i % 10 == 0 count
    if on_zero { zeros } else { rows - zeros }
}

fn main() {
    // Non-vacuity probe: a deliberate allocation must bump COUNT before any
    // "0 allocs" line below is trusted (counted nothing != allocated nothing).
    COUNT.store(0, Ordering::Relaxed);
    let probe: Vec<u8> = vec![0xAB; 4096];
    black_box(&probe);
    assert!(
        COUNT.load(Ordering::Relaxed) >= 1,
        "allocator hook not wired: deliberate Vec alloc did not bump COUNT"
    );
    let version = core::str::from_utf8(VERSION_C)
        .unwrap_or("?")
        .trim_end_matches('\0');
    println!("version {version}");

    // ------------------------------------------------------------------
    // (a) Varlen<u32>::row(i) sequential full-scan read, 64 MiB data.
    // ------------------------------------------------------------------
    {
        let mut b = VarlenBuilder::<u32>::new();
        for i in 0..VARLEN_ROWS {
            b.push(Some(&fixed_row::<VARLEN_ROW_W>(i)));
        }
        let v = b.finish();
        drop(b);

        // Pre-flight: shape + content BEFORE any timing.
        assert_eq!(v.len(), VARLEN_ROWS, "varlen row count");
        assert!(v.offsets_monotonic(), "varlen offsets monotonic");
        assert_eq!(v.data().len(), PAYLOAD, "varlen data = 64 MiB");
        assert_eq!(v.row(0), &fixed_row::<VARLEN_ROW_W>(0), "row 0 content");
        assert_eq!(
            v.row(VARLEN_ROWS / 2),
            &fixed_row::<VARLEN_ROW_W>(VARLEN_ROWS / 2),
            "mid content"
        );
        assert_eq!(
            v.row(VARLEN_ROWS - 1),
            &fixed_row::<VARLEN_ROW_W>(VARLEN_ROWS - 1),
            "tail content"
        );

        bench_line(
            "varlen",
            "u32 row-scan (64 MiB data, 1048576 rows x 64 B; bytes = data only)",
            PAYLOAD as u64,
            VARLEN_ROWS as u64,
            1,
            || {
                let v: &Varlen<u32> = black_box(&v);
                let mut acc = 0u8;
                for i in 0..v.len() {
                    for &byte in v.row(i) {
                        acc ^= byte;
                    }
                }
                black_box(acc);
            },
        );
    }

    // ------------------------------------------------------------------
    // (b) ViewVec::row(i): inline-heavy vs foreign-heavy at matched ~64 MiB
    // touched scale (descriptors + payload bytes actually read per scan).
    // ------------------------------------------------------------------
    for (label, rows, foreign_on_zero) in [
        ("inline-heavy (90% <=12B)", INLINE_HEAVY_ROWS, true),
        ("foreign-heavy (90% >12B)", FOREIGN_HEAVY_ROWS, false),
    ] {
        let mut b = ViewVecBuilder::new();
        for i in 0..rows {
            let foreign = if foreign_on_zero { i % 10 == 0 } else { i % 10 != 0 };
            if foreign {
                b.push(Some(&fixed_row::<FOREIGN_W>(i)));
            } else {
                b.push(Some(&fixed_row::<INLINE_W>(i)));
            }
        }
        let v = b.finish();
        drop(b);
        let fc = foreign_count(rows, foreign_on_zero);
        let touched = (rows * 16 + fc * FOREIGN_W) as u64;

        // Pre-flight: shape + one row of each class, content-checked.
        assert_eq!(v.len(), rows, "{label}: row count");
        assert_eq!(
            v.data().len(),
            fc * FOREIGN_W,
            "{label}: data block = foreign rows x 64 B"
        );
        let foreign_i = if foreign_on_zero { 0 } else { 1 };
        let inline_i = if foreign_on_zero { 1 } else { 0 };
        assert_eq!(
            v.row(foreign_i),
            &fixed_row::<FOREIGN_W>(foreign_i),
            "{label}: foreign row content"
        );
        assert_eq!(
            v.row(inline_i),
            &fixed_row::<INLINE_W>(inline_i),
            "{label}: inline row content"
        );

        bench_line(
            "views",
            &format!(
                "{label} (rows {rows}, ~90/10 split; bytes = touched: 16B desc + foreign payload)"
            ),
            touched,
            rows as u64,
            1,
            || {
                let v: &ViewVec = black_box(&v);
                let mut acc = 0u8;
                for i in 0..v.len() {
                    for &byte in v.row(i) {
                        acc ^= byte;
                    }
                }
                black_box(acc);
            },
        );
    }

    // ------------------------------------------------------------------
    // (c) Sliced::get(i) full iteration vs direct Primitive::values()
    // iteration: zero-copy overhead check (expect ~0). All-valid parent
    // (no validity buffer), misaligned window at offset 7.
    // ------------------------------------------------------------------
    {
        let mut b = PrimitiveBuilder::<i64>::new();
        for i in 0..I64_ROWS {
            b.push(Some(i as i64));
        }
        let arr = b.finish();
        drop(b);
        assert_eq!(arr.len(), I64_ROWS, "parent row count");
        assert!(arr.validity().is_none(), "all-valid parent drops bitmap");
        let s = slice(&arr, SLICE_OFFSET, I64_ROWS - SLICE_OFFSET)
            .expect("pre-flight slice bounds");
        assert_eq!(s.len(), I64_ROWS - SLICE_OFFSET);
        assert_eq!(s.get(0), Some(SLICE_OFFSET as i64), "window read-through");
        assert_eq!(
            s.get(s.len() - 1),
            Some((I64_ROWS - 1) as i64),
            "window tail read-through"
        );

        // Checksum agreement pre-flight: window path == direct path over the
        // same cells (real assert, not assumed).
        {
            let mut sum_window: u64 = 0;
            for i in 0..s.len() {
                if let Some(x) = s.get(i) {
                    sum_window = sum_window.wrapping_mul(31).wrapping_add(x as u64);
                }
            }
            let mut sum_direct: u64 = 0;
            for &x in &arr.values()[SLICE_OFFSET..] {
                sum_direct = sum_direct.wrapping_mul(31).wrapping_add(x as u64);
            }
            assert_eq!(sum_window, sum_direct, "window vs direct checksum");
        }

        let window_bytes = ((I64_ROWS - SLICE_OFFSET) * core::mem::size_of::<i64>()) as u64;
        let window_rows = (I64_ROWS - SLICE_OFFSET) as u64;

        bench_line(
            "sliced",
            "get full iter (all-valid, window offset 7, bytes = i64 values walked)",
            window_bytes,
            window_rows,
            1,
            || {
                let s: &_ = black_box(&s);
                let mut sum: u64 = 0;
                for i in 0..s.len() {
                    if let Some(x) = s.get(i) {
                        sum = sum.wrapping_mul(31).wrapping_add(x as u64);
                    }
                }
                black_box(sum);
            },
        );
        bench_line(
            "primitive",
            "values direct iter (same cells, no window work; bytes = i64 values walked)",
            window_bytes,
            window_rows,
            1,
            || {
                let arr: &Primitive<i64> = black_box(&arr);
                let mut sum: u64 = 0;
                for &x in &arr.values()[SLICE_OFFSET..] {
                    sum = sum.wrapping_mul(31).wrapping_add(x as u64);
                }
                black_box(sum);
            },
        );
    }

    // ------------------------------------------------------------------
    // (d) RunEndEncoded<i32>: get(i) binary-search decode + dense decode
    // on a 1,048,576-row / 100-run array.
    // ------------------------------------------------------------------
    {
        // Run ends: 10,485-row runs for r < 24, then 10,486-row runs, so the
        // 100th exclusive end lands exactly on 2^20.
        let mut ends: Vec<i32> = Vec::with_capacity(REE_RUNS);
        let mut cum = 0usize;
        for r in 0..REE_RUNS {
            cum += 10_485 + usize::from(r >= 24);
            ends.push(cum as i32);
        }
        assert_eq!(ends[REE_RUNS - 1], REE_ROWS as i32, "last end == rows");
        let mut b = RunEndEncodedBuilder::<i32>::new();
        for i in 0..REE_ROWS {
            let run = ends.partition_point(|&e| (e as usize) <= i);
            b.push(run as i32);
        }
        let ree = b.finish();
        drop(b);
        assert_eq!(ree.logical_len(), REE_ROWS, "ree logical rows");
        assert_eq!(ree.num_runs(), REE_RUNS, "ree run count");
        assert_eq!(ree.run_ends(), &ends, "ree ends round-trip");
        let expected_get_sum: u64 = ends
            .iter()
            .enumerate()
            .map(|(r, &end)| {
                let start = if r == 0 { 0 } else { ends[r - 1] as usize };
                (r as u64) * ((end as usize - start) as u64)
            })
            .sum();
        assert_eq!(ree.get(0), 0, "first row in run 0");
        assert_eq!(ree.get(REE_ROWS / 2), 50, "mid row run index");
        assert_eq!(
            ree.get(REE_ROWS - 1),
            (REE_RUNS - 1) as i32,
            "last row in run 99"
        );

        bench_line(
            "runend",
            "get binary-search (1M rows, 100 runs; Mrows/s is the basis, no byte basis)",
            0,
            REE_ROWS as u64,
            1,
            || {
                let ree: &_ = black_box(&ree);
                let mut sum: u64 = 0;
                for i in 0..ree.logical_len() {
                    sum = sum.wrapping_add(ree.get(i) as u64);
                }
                assert_eq!(sum, expected_get_sum, "get checksum drift");
            },
        );
        let mut dst: Vec<i32> = vec![0; REE_ROWS];
        // Pre-flight full decode vs independently recomputed dense truth.
        {
            let mut expected = vec![0i32; REE_ROWS];
            let mut row = 0usize;
            for (r, &end) in ends.iter().enumerate() {
                while row < end as usize {
                    expected[row] = r as i32;
                    row += 1;
                }
            }
            ree.decode_to_dense(&mut dst);
            assert_eq!(dst, expected, "dense decode round-trip");
        }

        bench_line(
            "runend",
            "decode_to_dense (1M rows, 100 runs; bytes = 4 MiB dst written, L3-resident)",
            (REE_ROWS * core::mem::size_of::<i32>()) as u64,
            REE_ROWS as u64,
            DECODE_ITERS,
            || {
                let ree: &_ = black_box(&ree);
                let dst: &mut [i32] = black_box(&mut dst);
                ree.decode_to_dense(dst);
                black_box(dst[0]);
            },
        );
    }

    println!(
        "noise: min-of-{ROUNDS} rounds, best round wins; concurrent lanes make these noisy; \
         GiB/s and Mrows/s bases stated per line"
    );
}
