//! Full-pipeline page bench (lane OPT-L5).
//!
//! End-to-end DataPageV2 framing over the hybrid codec:
//! [`avec_parquet::page::write_data_page_v2`] (thrift header + RLE body) and
//! [`avec_parquet::page::decode_page_into`] (header parse + in-place body
//! decode), over repeated / literal / mix50 shapes at 64 MiB logical
//! payload. Mirrors the parquet_bench house pattern: counting GlobalAlloc,
//! min-of-5, black_box elision guards, and a full round-trip pre-flight per
//! shape BEFORE any timing (the RunOverrun lesson: a pattern bug surfaced as
//! a decode error — here it surfaces as a loud pre-flight assert instead).
//!
//! Bytes-moved conventions (64 MiB logical payload = 16M u32 lanes, width 4):
//! - `write`, `write_body`: input bytes (the 64 MiB the encoder walks).
//! - `decode`, `decode_body`: output bytes (the 64 MiB of lanes written).
//! - `write_hdr`, `decode_hdr`: the header's own wire bytes (a header stage
//!   processes only its few dozen bytes; per-wire-byte rate, batched 1024x
//!   so timer overhead does not dominate).
//! The `page lens` line carries the measured header/body wire lengths so
//! stage times are derivable from the printed rates.

use avec_parquet::page::{decode_page_into, write_data_page_v2};
use avec_parquet::rle::{decode_into, rle_encode};
use avec_parquet::thrift::{read_page_header, PageHeader};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 16M u32 lanes = 64 MiB logical payload per measurement.
const LANES: usize = 16 * 1024 * 1024;
/// Canonical nibble width (matches parquet_bench's RLE shapes).
const BITS: u8 = 4;
/// Max 4-bit value; every shape's lanes are drawn from 0..=VMAX.
const VMAX: u32 = (1 << BITS) - 1;
/// Untouched-lane sentinel: far above VMAX, so an unwritten lane can never
/// pass the round-trip equality even if `out` held matching data earlier.
const SENTINEL: u32 = 0x1234_ABCD;
/// Min-of-N: best (least-noisy) round wins; concurrent lanes make this noisy.
const ROUNDS: usize = 5;
/// Batch iters for the tiny header stages (tens of ns each).
const HDR_ITERS: usize = 1024;
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
/// `iters` calls each. Allocs are from the winning round, divided by
/// `iters` (exact for the 0-alloc decode stages; integral for write stages
/// which run at iters=1).
fn bench_line(op: &str, shape: &str, bytes_moved: u64, iters: usize, mut f: impl FnMut()) {
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
    let gibps = bytes_moved as f64 * iters as f64 / best / GIB;
    println!("page {op} {shape}: {gibps:.3} GiB/s, {best_allocs} allocs");
}

/// Sample head/middle/tail lanes through black_box so decode output
/// cannot be elided.
fn consume_u32(o: &[u32]) {
    black_box(u64::from(o[0]) ^ u64::from(o[o.len() / 2]) ^ u64::from(o[o.len() - 1]));
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

    let payload = (LANES * 4) as u64; // 64 MiB
    let num_rows = LANES as u32;

    for (shape, make) in [
        (
            "repeated",
            Box::new(|vmax: u32| vec![vmax; LANES]) as Box<dyn Fn(u32) -> Vec<u32>>,
        ),
        (
            "literal",
            Box::new(|vmax: u32| {
                (0..LANES).map(|i| (i as u32) & vmax).collect::<Vec<u32>>()
            }) as Box<dyn Fn(u32) -> Vec<u32>>,
        ),
        (
            "mix50",
            Box::new(|vmax: u32| {
                // 16-lane block: 8 identical (repeated record) + 8 strictly
                // sub-vmax lanes (one literal group) => exactly 50/50 lanes.
                // Literals stay below vmax: a trailing lane equal to vmax
                // merges with the next block's run, shifting record alignment
                // and padding a literal batch past budget (the RunOverrun
                // lesson — caught here by the pre-flight, not by a decode
                // error mid-timing).
                (0..LANES)
                    .map(|i| {
                        if i % 16 < 8 {
                            vmax
                        } else {
                            ((i % 8) as u32).min(vmax - 1)
                        }
                    })
                    .collect::<Vec<u32>>()
            }) as Box<dyn Fn(u32) -> Vec<u32>>,
        ),
    ] {
        let values = make(VMAX);
        assert_eq!(values.len(), LANES);

        // ---- pre-flight: full round-trip BEFORE any timing ----
        let page = write_data_page_v2(&values, BITS, num_rows);
        let (header, hdr_len) = read_page_header(&page).expect("pre-flight header parse");
        let v2 = header.data_page_header_v2.as_ref().expect("V2 subheader");
        assert_eq!(v2.num_values, LANES as i32, "{shape}: header lane claim");
        assert_eq!(v2.num_rows, num_rows as i32, "{shape}: header row claim");
        assert!(hdr_len < page.len(), "{shape}: header must precede a body");
        let body: &[u8] = &page[hdr_len..];
        assert!(!body.is_empty(), "{shape}: body must be non-empty");

        let mut out = vec![SENTINEL; LANES];

        // Split-stage agreement: the body stage alone must land every lane.
        {
            let n_body = decode_into(body, &mut out).expect("pre-flight body decode");
            assert_eq!(n_body, LANES, "{shape}: body lane count");
            assert_eq!(out, values, "{shape}: body-stage disagreement");
        }

        // Full-pipeline round-trip with sentinel-poisoned output: a decode
        // that silently wrote a prefix could hide behind the body stage's
        // already-correct lanes; the sentinel makes any unwritten lane loud.
        out.fill(SENTINEL);
        let n = decode_page_into(&page, &mut out).expect("pre-flight full decode");
        assert_eq!(n, LANES, "{shape}: full-decode lane count");
        assert_eq!(out, values, "{shape}: full-pipeline round-trip mismatch");

        println!(
            "page lens {shape}: header={hdr_len} B, body={} B, lanes={LANES} (64 MiB logical)",
            page.len() - hdr_len
        );

        // Header object reused by the write_hdr stage (the writer emits the
        // same header shape; the pre-flight parse is the proof).
        let header: PageHeader = header;

        // ---- write side ----
        bench_line("write", shape, payload, 1, || {
            let v: &[u32] = black_box(&values);
            let p = write_data_page_v2(v, BITS, num_rows);
            black_box(p.len());
            drop(p);
        });
        bench_line("write_body", shape, payload, 1, || {
            let v: &[u32] = black_box(&values);
            let b = rle_encode(v, BITS);
            black_box(b.len());
            drop(b);
        });
        let mut hdr_buf: Vec<u8> = Vec::with_capacity(64);
        bench_line("write_hdr", shape, hdr_len as u64, HDR_ITERS, || {
            let h: &PageHeader = black_box(&header);
            hdr_buf.clear();
            avec_parquet::thrift::write_page_header(&mut hdr_buf, h);
            black_box(hdr_buf.len());
        });

        // ---- decode side ----
        bench_line("decode", shape, payload, 1, || {
            let p: &[u8] = black_box(&page);
            let o: &mut [u32] = black_box(&mut out);
            let n = decode_page_into(p, o).expect("decode");
            black_box(n);
            consume_u32(o);
        });
        bench_line("decode_hdr", shape, hdr_len as u64, HDR_ITERS, || {
            let p: &[u8] = black_box(&page);
            let (h, hl) = read_page_header(p).expect("hdr parse");
            black_box(hl);
            black_box(h.page_type);
        });
        bench_line("decode_body", shape, payload, 1, || {
            let b: &[u8] = black_box(body);
            let o: &mut [u32] = black_box(&mut out);
            let n = decode_into(b, o).expect("body decode");
            black_box(n);
            consume_u32(o);
        });

        drop(out);
        drop(values);
        drop(page);
    }
}
