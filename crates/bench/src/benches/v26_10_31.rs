//! v26.10.31 CTQ bench group (lane I18) — Divan harness per
//! `docs/jira/v26.10.31/29-benchmark-harness-plan.md`.
//!
//! Subjects: (a) quad AoS->SoA transpose throughput, (b) branchless
//! predicate filter throughput, (c) avec-blake3 tree-hash throughput
//! (CTQ-06, LSL 8.5 / target 10.2 GiB/s per-core), (d) level-free Parquet
//! page write/scan round-trip (CTQ-03 family).
//!
//! Standing: numbers from this harness are receipts-in-progress, not
//! standing floors, until the divan2receipt converter + gage validation
//! land (plan Sections "Gage R&R" / "divan2receipt").

use avec_arrow::predicates::filter_quads_eq;
use avec_arrow::transpose::{transpose, Quad64};
use avec_blake3::hash_wide_simd;
use avec_parquet::level_free::{write_page, read_page_payload, LevelFreeType};
use divan::{Bencher, BytesCount};
use std::hint::black_box;

fn main() {
    divan::main();
}

const N_QUADS: usize = 1 << 20; // 1M quads = 64 MiB of quad records

/// Deterministic quad fixture (LCG so content is input-shape-invariant).
fn make_quads(n: usize) -> Vec<Quad64> {
    let mut s: u32 = 0x1234_5678;
    let mut lcg = || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        s
    };
    (0..n)
        .map(|_| Quad64 {
            s_id: lcg(),
            p_id: lcg(),
            o_id: lcg(),
            g_id: lcg(),
            t_ns: lcg() as u64 | ((lcg() as u64) << 32),
            vector_clock: lcg(),
            policy_mask: lcg(),
            inline_digest: [0u8; 32],
        })
        .collect()
}

// (a) Quad transpose throughput. Bytes = 64 B/record input stream.
#[divan::bench(name = "quad_transpose", bytes_count = N_QUADS * 64)]
fn bench_transpose(b: Bencher) {
    let aos = make_quads(N_QUADS);
    b.bench(|| {
        let cols = black_box(&aos);
        black_box(transpose(cols));
    });
}

// (b) Predicate filter throughput. Bytes = 16 B of u32 payload per quad
// walked (filter_quads_eq reads 4 x u32 per quad).
#[divan::bench(name = "quad_filter_eq", bytes_count = N_QUADS * 16)]
fn bench_filter(b: Bencher) {
    let mut data = Vec::with_capacity(N_QUADS * 4);
    for q in make_quads(N_QUADS) {
        data.extend_from_slice(&[q.s_id, q.p_id, q.o_id, q.g_id]);
    }
    let mut out = vec![0u32; N_QUADS + 1];
    b.bench(|| {
        black_box(filter_quads_eq(
            black_box(&data),
            N_QUADS,
            black_box(data[7]),
            black_box(&mut out),
        ));
    });
}

// (c) BLAKE3 tree-hash throughput (CTQ-06). 256 MiB buffer, per-core.
#[divan::bench(name = "blake3_tree_hash_256mib", bytes_count = 256 * 1024 * 1024)]
fn bench_blake3(b: Bencher) {
    let mut data = vec![0u8; 256 * 1024 * 1024];
    let mut s: u64 = 0xdead_beef;
    for chunk in data.chunks_mut(4096) {
        let v = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s = v;
        chunk[0..8].copy_from_slice(&v.to_le_bytes());
    }
    b.bench(|| {
        black_box(hash_wide_simd(black_box(&data)));
    });
}

// (d) Level-free Parquet page round-trip (CTQ-03 family). Bytes = logical
// payload walked once per write + once per read.
#[divan::bench(name = "level_free_page_roundtrip_16m_u32", bytes_count = N_QUADS * 4)]
fn bench_page_roundtrip(b: Bencher) {
    let mut payload = Vec::with_capacity(N_QUADS * 4);
    let mut s: u32 = 0xcafe_babe;
    for _ in 0..N_QUADS {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        payload.extend_from_slice(&s.to_le_bytes());
    }
    b.bench(|| {
        let page = black_box(write_page(
            LevelFreeType::U32,
            black_box(&payload),
            N_QUADS,
        ))
        .expect("page write");
        let (got, rows) = read_page_payload(black_box(&page), LevelFreeType::U32).expect("page read");
        black_box((got.len(), rows));
    });
}
