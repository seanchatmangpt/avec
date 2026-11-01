//! CTQ-03/05/06 measurement harness (lane I18, wave v26.10.31).
//! Spec: docs/jira/v26.10.31/29-benchmark-harness-plan.md.
//!
//! Four benches:
//! (a) AoS->SoA quad transpose (cgap_arrow::transpose)
//! (b) branchless predicate filter (cgap_arrow::predicates::filter_quads_eq)
//! (c) BLAKE3 tree hash throughput on a 64 MiB buffer (cgap_blake3::hash_wide_simd)
//! (d) parquet level-free page write+read round trip (cgap_parquet::level_free)
//!
//! These are FIRST CTQ datapoints — no gate wiring.
//!
//! Stub-replacement disclosure: this file replaces the lane-I6 compile-freeze
//! stub (`fn main() {}`); I6's disclosure comment is preserved in this header.

// Lane I6 compile-freeze disclosure: the [[bench]] v26_10_31 target was
// declared without its file, freezing all workspace cargo. Replaced by lane
// I18 with the real harness below.

use cgap_arrow::predicates::filter_quads_eq;
use cgap_arrow::transpose::{transpose, Quad64};
use cgap_blake3::hash_wide_simd;
use cgap_parquet::level_free::{read_page_payload, write_page, LevelFreeType};
use divan::counter::BytesCount;
use divan::{black_box, Bencher};

const QUADS: usize = 1 << 16; // 65_536 quads = 4 MiB of 64B records
const HASH_BUF: usize = 64 * 1024 * 1024; // 64 MiB
const PARQUET_ROWS: usize = 1 << 16; // 65_536 x u64 rows

/// Deterministic xorshift corpus generator (matches transpose.rs test style).
fn xorshift(seed: u64) -> impl FnMut() -> u64 {
    let mut x = seed;
    move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    }
}

fn quad_corpus(len: usize) -> Vec<Quad64> {
    let mut next = xorshift(0x9E37_79B9_7F4A_7C15);
    (0..len)
        .map(|_| Quad64 {
            s_id: next() as u32,
            p_id: (next() >> 32) as u32,
            o_id: next() as u32,
            g_id: (next() >> 32) as u32,
            t_ns: next(),
            vector_clock: next() as u32,
            policy_mask: (next() >> 32) as u32 & 0x0FFF_FFFF,
            inline_digest: core::array::from_fn(|b| (next() >> (b % 8)) as u8),
        })
        .collect()
}

// (a) AoS -> SoA transpose of 65_536 x 64-byte quad records (4 MiB in).
#[divan::bench]
fn quad_transpose_64k(b: Bencher) {
    let aos = quad_corpus(QUADS);
    b.bench(|| {
        black_box(transpose(black_box(&aos)));
    });
}

// (b) Branchless quad-equality filter over 65_536 quads of flat u32 data.
// Needle 0 occurs ~once per 2^32; corpus shape is controlled below.
#[divan::bench]
fn predicate_filter_64k(b: Bencher) {
    let mut next = xorshift(0x243F_6A88_85A3_08D3);
    let mut data = Vec::with_capacity(QUADS * 4);
    for q in 0..QUADS {
        // Every 64th quad is a full match so the compaction stream carries
        // real writes without degenerating into all-match.
        if q % 64 == 0 {
            data.extend_from_slice(&[0x0; 4]);
        } else {
            data.push(next() as u32 | 1); // never 0: keeps match rate 1/64
        }
        for _ in 1..4 {
            data.push(next() as u32 | 1);
        }
    }
    // RefCell because divan's Bencher closure is Fn, not FnMut; the cell is
    // per-bench-thread state, not a mock.
    // Mutex because divan's Bencher closure is Fn (Send-bound); uncontended
    // per-iteration lock on real state, not a mock.
    let out = std::sync::Mutex::new(vec![0u32; QUADS + 1]);
    let needle = 0u32;
    b.bench(|| {
        let mut out = out.lock().unwrap();
        let n = filter_quads_eq(black_box(&data), QUADS, needle, &mut out);
        black_box(n);
    });
}

// (c) BLAKE3 tree hash over a 64 MiB buffer (sovereign wide-SIMD path).
#[divan::bench]
fn blake3_hash_64mib(b: Bencher) {
    let mut next = xorshift(0xDEAD_BEEF);
    let buf: Vec<u8> = (0..HASH_BUF).map(|_| next() as u8).collect();
    b.counter(BytesCount::new(HASH_BUF))
        .bench(|| {
            black_box(hash_wide_simd(black_box(&buf)));
        });
}

// (d) Parquet level-free DataPageV2 write + zero-copy read round trip,
// 65_536 u64 rows (512 KiB payload per direction).
#[divan::bench]
fn parquet_level_free_roundtrip_u64_64k(b: Bencher) {
    let mut next = xorshift(0x5EED_5EED_5EED_5EED);
    let payload: Vec<u8> = (0..PARQUET_ROWS * 8).map(|_| next() as u8).collect();
    b.bench(|| {
        let page = write_page(LevelFreeType::U64, black_box(&payload), PARQUET_ROWS)
            .expect("write_page");
        let (got, rows) = read_page_payload(&page, LevelFreeType::U64).expect("read_page_payload");
        black_box((got.len(), rows));
    });
}

fn main() {
    divan::main();
}
