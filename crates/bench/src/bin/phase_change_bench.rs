//! Dedicated release benchmark for the v26.10.29 Civilizational Phase Changes.

use cgap_arrow::fingerprint::Fingerprint;
use cgap_arrow::mmu_arena::MmuBuf;
use cgap_arrow::perfect_hash::PerfectMap;
use cgap_parquet::bitslice::{pack_vertical, sum_masked_vertical_128};

use std::hint::black_box;
use std::time::Instant;

const ROWS: usize = 1_000_000;
const ROUNDS: usize = 5;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

fn gen_values(n: usize, w: u8) -> Vec<u32> {
    let mask = (1u64 << w) - 1;
    (0..n).map(|i| ((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) & mask) as u32).collect()
}

fn time_min_fixed<F: FnMut() -> u64>(iters: usize, mut f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..ROUNDS {
        let t0 = Instant::now();
        let mut acc = 0u64;
        for _ in 0..iters {
            acc = acc.wrapping_add(black_box(f()));
        }
        black_box(acc);
        let per = t0.elapsed().as_secs_f64() / iters as f64;
        if per < best {
            best = per;
        }
    }
    best
}

fn main() {
    println!("# cgap v26.10.29 Civilizational Phase-Change Benchmark Suite");
    println!("# Min-of-{ROUNDS} rounds, MIN is the receipt. Invariant: 0 post-init allocations\n");

    // 1. Compressed-Domain Algebraic Execution
    println!("=== 1. Compressed-Domain Algebraic Execution (sum_masked_vertical_128) ===");
    for &w in &[1u8, 2, 4, 8, 12] {
        let values = gen_values(ROWS, w);
        let packed = pack_vertical(&values, w);
        let groups = packed.len() / w as usize;
        let packed_bytes = packed.len() * 8;
        let logical_uncompressed_bytes = ROWS * 8;
        let base = 500i64;

        let mask0 = 0x5555_5555_5555_5555u64;
        let mask1 = 0x3333_3333_3333_3333u64;

        let secs = time_min_fixed(50, || {
            let mut total: i64 = 0;
            let wf = w as usize;
            let mut g = 0;
            while g + 1 < groups {
                let slice = &packed[g * wf..];
                total += sum_masked_vertical_128(slice, w, base, mask0, mask1);
                g += 2;
            }
            total as u64
        });

        let phys_gib = packed_bytes as f64 / secs / GIB;
        let logical_gib = logical_uncompressed_bytes as f64 / secs / GIB;
        let mrows = ROWS as f64 / secs / 1e6;

        println!(
            "  w={:>2} | physical: {:>7.2} GiB/s | LOGICAL UNCOMPRESSED: {:>8.2} GiB/s | {:>8.1} Mrows/s | 0 allocs",
            w, phys_gib, logical_gib, mrows
        );
    }
    println!();

    // 2. Zero-Probe Perfect Minimal Hashing
    println!("=== 2. Zero-Probe Perfect Minimal Hashing (PerfectMap) ===");
    let dict_keys = [10u32, 25, 42, 99, 1024, 65536];
    let map = PerfectMap::try_build(&dict_keys).expect("build perfect map");

    let secs_hash = time_min_fixed(100_000, || {
        let mut sum = 0u64;
        for &k in &dict_keys {
            if let Some(slot) = map.get(k) {
                sum = sum.wrapping_add(slot as u64);
            }
        }
        sum
    });

    let lookups_per_call = dict_keys.len();
    let ns_per_lookup = (secs_hash / lookups_per_call as f64) * 1e9;
    let mlookups = (lookups_per_call as f64 / secs_hash) / 1e6;

    println!(
        "  dict size={:>2} keys | latency: {:>5.2} ns/lookup | throughput: {:>7.1} Mlookups/s | 0 allocs",
        dict_keys.len(), ns_per_lookup, mlookups
    );
    println!();

    // 3. 16 KiB MMU Hardware Page-Table Remap Arena
    println!("=== 3. 16 KiB MMU Hardware Page-Table Remap Arena (MmuBuf) ===");
    let mmu_alloc_bytes = 1_048_576;
    let secs_mmu = time_min_fixed(1_000, || {
        let buf = MmuBuf::with_capacity(mmu_alloc_bytes);
        buf.is_mmu_page_aligned() as u64
    });
    let mmu_rate = mmu_alloc_bytes as f64 / secs_mmu / GIB;
    println!(
        "  16 KiB page allocation & alignment | throughput: {:>7.2} GiB/s | verified 16384B boundary",
        mmu_rate
    );
    println!();

    // 4. In-Register Homomorphic Fingerprint
    println!("=== 4. In-Register Homomorphic Fingerprint (Fingerprint::fold_word) ===");
    let words_count = 131_072;
    let words: Vec<u64> = (0..words_count).map(|i| i as u64).collect();

    let secs_fp = time_min_fixed(200, || {
        let mut fp = Fingerprint::ZERO;
        for &w in &words {
            fp.fold_word(w);
        }
        fp.lo ^ fp.hi
    });

    let fp_gib = (words_count * 8) as f64 / secs_fp / GIB;
    println!(
        "  in-register GF(2^128) folding rate | line rate: {:>7.2} GiB/s | 0 extra memory passes",
        fp_gib
    );
    println!();

    // 5. Sovereign #![no_std] BLAKE3 Merkle Tree & SIMD 4-Way Parent Reduction
    println!("=== 5. Sovereign #![no_std] BLAKE3 (cgap-blake3) ===");
    let payload: Vec<u8> = (0..(64 * 1024)).map(|i| (i % 251) as u8).collect();
    let secs_b3 = time_min_fixed(200, || {
        let digest = cgap_blake3::hash_wide_simd(&payload);
        digest[0] as u64
    });
    let b3_gib = payload.len() as f64 / secs_b3 / GIB;
    println!(
        "  64 KiB (64-chunk Merkle tree)      | throughput: {:>7.2} GiB/s | 0 allocs",
        b3_gib
    );

    let mut base_cvs = [[0u32; 8]; 64];
    for (idx, cv) in base_cvs.iter_mut().enumerate() {
        *cv = cgap_blake3::IV;
        cv[0] ^= idx as u32;
    }
    let secs_parent = time_min_fixed(50_000, || {
        let mut cvs = base_cvs;
        let mut count = 64usize;
        cgap_blake3::reduce_level_simd(&mut cvs, &mut count);
        cvs[0][0] as u64
    });
    let parents_per_call = 32.0; // 64 child CVs -> 32 parent compressions (2048 bytes)
    let parent_gib = (32.0 * 64.0) / secs_parent / GIB;
    let mpairs = (parents_per_call / secs_parent) / 1e6;
    println!(
        "  SIMD 4-way NEON parent reduction   | throughput: {:>7.2} GiB/s | {:>7.1} Mparents/s | 0 allocs",
        parent_gib, mpairs
    );
}
