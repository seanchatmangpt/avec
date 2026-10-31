//! FR-06 throughput bench (AVEC-111 / v26.10.31 spec §4-6) — plain std, no sleeps,
//! min-of-N Instant-based rounds. CTQ-06 candidate receipt generator.
//!
//! Default: REPORT ONLY. Pass `--gate` to exit non-zero below the ratified LSL (1.0 GiB/s, 2026-10-31).

use std::time::Instant;

const BYTES: usize = 256 * 1024 * 1024; // 256 MiB
const ROUNDS: usize = 5;
const LSL_GIB_S: f64 = 1.0;

fn main() {
    let gate = std::env::args().any(|a| a == "--gate");
    let model = std::env::var("BLAKE3_BENCH_MACHINE")
        .unwrap_or_else(|_| String::from("unknown"));

    let mut buf = vec![0u8; BYTES];
    let mut x: u64 = 0x9E3779B97F4A7C15;
    for b in buf.iter_mut() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (x >> 33) as u8;
    }

    let mut best = f64::INFINITY; // seconds
    for round in 0..ROUNDS {
        let t0 = Instant::now();
        let digest = avec_blake3::hash_wide_simd(&buf);
        let dt = t0.elapsed().as_secs_f64();
        std::hint::black_box(&digest);
        if round == 0 || dt < best {
            best = dt;
        }
        eprintln!(
            "round {}: {:.3} s, {:.2} GiB/s",
            round,
            dt,
            (BYTES as f64) / dt / (1024.0 * 1024.0 * 1024.0)
       );
    }

    let gib_s = (BYTES as f64) / best / (1024.0 * 1024.0 * 1024.0);
    println!(
        "RESULT machine={} bytes={} rounds={} min_time={:.4}s throughput={:.2} GiB/s gate={}",
        model, BYTES, ROUNDS, best, gib_s, gate
    );

    if gate && gib_s < LSL_GIB_S {
        eprintln!(
            "GATE FAIL: throughput {:.2} GiB/s < LSL {:.2} GiB/s",
            gib_s, LSL_GIB_S
        );
        std::process::exit(1);
    }
}
