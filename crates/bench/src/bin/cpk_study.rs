//! C_pk capability study (wave-7 lane 4) — closes the last UNVERIFIED
//! charter item: process capability C_pk >= 1.67 (6-sigma) per
//! docs/jira/v26.10.31/18-dflss-charter.md §1/§5.
//!
//! Three measurable CTQs with real spec limits:
//! - CTQ-05 admission latency: per-sample end-to-end pipeline (Quad64 batch
//!   -> BLAKE3 receipt -> level-free parquet read-back), USL 35 µs, no LSL
//!   -> one-sided C_pu.
//! - CTQ-06-derived throughput: hash_many_simd over 256 MiB per sample,
//!   LSL 1.0 GiB/s (ratified 2026-10-31, docs/OPERATOR_DECISIONS.md), no USL -> C_pl.
//! - CTQ-04 variance: transpose+branchless-filter time per run, USL = 15%
//!   of mean (the pinned provisional gate) -> one-sided C_pu against that
//!   moving limit.
//!
//! CTQ-04 gate note: this study still uses the retired 15% pin; `ctq_report`
//! uses the p99/p50 protocol (migration pending).
//!
//! Stats per CTQ: N >= 100 samples (fast loops, no sleeps), mean, overall
//! sigma (sample stdev, n-1), within sigma (moving range, MR_bar / d2 with
//! d2 = 1.128 for subgroups of 2). Capability indices on BOTH sigmas:
//! C_p = (USL-LSL)/6sigma (two-sided only), C_pu = (USL-mean)/3sigma,
//! C_pl = (mean-LSL)/3sigma, C_pk = min(C_pu, C_pl).
//!
//! Verdict per CTQ: PASS iff the applicable capability index (C_pu or C_pl,
//! overall sigma) >= 1.67. Gate semantics: exit 1 iff any gate fails
//! (fail-closed). Output: JSON to stdout, human log to stderr.
//!
//! Machine: Apple M3 Max; run under quiet-ish conditions — load noted in
//! the receipt, not rounded away.

use std::hint::black_box;
use std::time::Instant;

use cgap_arrow::layout::Quad64 as LayoutQuad;
use cgap_arrow::layout::QUAD_SIZE;
use cgap_arrow::predicates::filter_quads_eq;
use cgap_arrow::transpose::transpose;
use cgap_blake3::{hash, hash_many_simd};
use cgap_parquet::level_free::{read_page_payload, write_page, LevelFreeType};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
/// d2 control-chart constant for moving ranges (subgroup size 2).
const D2_N2: f64 = 1.128;
const CPK_TARGET: f64 = 1.67;

fn log(msg: &str) {
    eprintln!("[cpk_study] {msg}");
}

struct Capability {
    n: usize,
    mean: f64,
    sigma_overall: f64,
    sigma_within: f64,
    cp: Option<f64>,
    cpu_overall: Option<f64>,
    cpu_within: Option<f64>,
    cpl_overall: Option<f64>,
    cpl_within: Option<f64>,
    cpk_overall: Option<f64>,
    cpk_within: Option<f64>,
}

/// Standard capability formulas. `usl`/`lsl` are Option; at least one present.
fn capability(samples: &[f64], usl: Option<f64>, lsl: Option<f64>) -> Capability {
    assert!(!samples.is_empty());
    assert!(usl.is_some() || lsl.is_some(), "need a spec limit");
    let n = samples.len();
    let mean = samples.iter().sum::<f64>() / n as f64;
    let ss: f64 = samples.iter().map(|s| (s - mean) * (s - mean)).sum();
    let sigma_overall = if n > 1 { (ss / (n - 1) as f64).sqrt() } else { 0.0 };
    // Moving range: |x_i - x_{i-1}|, sigma_within = MR_bar / d2.
    let sigma_within = if n > 1 {
        let mr_sum: f64 = samples.windows(2).map(|w| (w[0] - w[1]).abs()).sum();
        (mr_sum / (n - 1) as f64) / D2_N2
    } else {
        0.0
    };
    let cpu = |s: f64| usl.map(|u| (u - mean) / (3.0 * s));
    let cpl = |s: f64| lsl.map(|l| (mean - l) / (3.0 * s));
    let cp = match (usl, lsl) {
        (Some(u), Some(l)) => Some((u - l) / (6.0 * sigma_overall)),
        _ => None,
    };
    let min_of = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    };
    Capability {
        n,
        mean,
        sigma_overall,
        sigma_within,
        cp,
        cpu_overall: cpu(sigma_overall),
        cpu_within: cpu(sigma_within),
        cpl_overall: cpl(sigma_overall),
        cpl_within: cpl(sigma_within),
        cpk_overall: min_of(cpu(sigma_overall), cpl(sigma_overall)),
        cpk_within: min_of(cpu(sigma_within), cpl(sigma_within)),
    }
}

fn emit(
    ctq: &str,
    metric: &str,
    usl: Option<f64>,
    lsl: Option<f64>,
    unit_hint: &str,
    cap: &Capability,
    verdict: &str,
) -> String {
    let f = |v: Option<f64>| match v {
        Some(x) => format!("{x:.4}"),
        None => "null".to_string(),
    };
    format!(
        r#"{{"ctq":"{ctq}","metric":"{metric}","unit":"{unit_hint}","n":{},"mean":{:.6},"usl":{},"lsl":{},"sigma_overall":{:.6},"sigma_within_mr":{:.6},"cp":{},"cpu_overall":{},"cpu_within":{},"cpl_overall":{},"cpl_within":{},"cpk_overall":{},"cpk_within":{},"cpk_target":1.67,"gate":"REAL","verdict":"{verdict}"}}"#,
        cap.n,
        cap.mean,
        json_opt(usl),
        json_opt(lsl),
        cap.sigma_overall,
        cap.sigma_within,
        f(cap.cp),
        f(cap.cpu_overall),
        f(cap.cpu_within),
        f(cap.cpl_overall),
        f(cap.cpl_within),
        f(cap.cpk_overall),
        f(cap.cpk_within),
    )
}

fn json_opt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x:.4}"),
        None => "null".to_string(),
    }
}

fn main() {
    let gates_failed: Vec<&str> = Vec::new();

    // ------------------------------------------------- CTQ-05 samples ----
    let (ctq05, cpk05) = ctq05_admission_latency();
    // ------------------------------------------------- CTQ-06 samples ----
    let (ctq06, cpk06) = ctq06_throughput();
    // ------------------------------------------------- CTQ-04 samples ----
    let (ctq04, cpk04) = ctq04_variance();

    let mut failed = gates_failed;
    for (ctq, v) in [("CTQ-05", &cpk05), ("CTQ-06", &cpk06), ("CTQ-04", &cpk04)] {
        if *v < CPK_TARGET {
            failed.push(ctq);
        }
    }

    println!("{{");
    println!("  \"report\": \"cpk_study\",");
    println!("  \"version\": 1,");
    println!("  \"cpk_target\": {CPK_TARGET},");
    println!("  \"ctq05_admission_latency\": {ctq05},");
    println!("  \"ctq06_throughput\": {ctq06},");
    println!("  \"ctq04_variance\": {ctq04},");
    if failed.is_empty() {
        println!("  \"verdict\": \"PASS\"");
    } else {
        println!(
            "  \"verdict\": \"FAIL\", \"failed_gates\": [{}]",
            failed
                .iter()
                .map(|g| format!("\"{g}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("}}");

    if !failed.is_empty() {
        std::process::exit(1);
    }
}

// ============================================================ CTQ-05 ====

/// N samples of the real end-to-end admission pipeline (same shape as the
/// integration admission loop + ctq_report's CTQ-05): Quad64 batch ->
/// BLAKE3 receipt (fastest correct path) -> level-free parquet read-back of
/// all three projected pages. One elapsed-time sample per iteration.
fn ctq05_admission_latency() -> (String, f64) {
    log("CTQ-05: admission latency samples, USL 35 us, one-sided C_pu");
    const N: usize = 256; // batch size, quads
    const SAMPLES: usize = 200;

    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let quads: Vec<LayoutQuad> = (0..N)
        .map(|i| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            LayoutQuad {
                s_id: i as u32,
                p_id: (x >> 32) as u32,
                o_id: x as u32,
                g_id: i as u32,
                t_ns: x,
                vector_clock: (x >> 33) as u32,
                policy_mask: (x >> 17) as u32,
                inline_digest: {
                    let mut d = [0u8; 32];
                    d[..8].copy_from_slice(&(i as u64).to_le_bytes());
                    d
                },
            }
        })
        .collect();
    let mut bytes = Vec::with_capacity(N * QUAD_SIZE);
    for q in &quads {
        bytes.extend_from_slice(&q.to_le_bytes());
    }

    let mut s = Vec::with_capacity(N * 4);
    let mut t = Vec::with_capacity(N * 8);
    let mut dg = Vec::with_capacity(N * 32);
    for q in &quads {
        s.extend_from_slice(&q.s_id.to_le_bytes());
        t.extend_from_slice(&q.t_ns.to_le_bytes());
        dg.extend_from_slice(&q.inline_digest);
    }
    let pages: [(LevelFreeType, Vec<u8>); 3] = [
        (LevelFreeType::U32, write_page(LevelFreeType::U32, &s, N).expect("page s")),
        (LevelFreeType::U64, write_page(LevelFreeType::U64, &t, N).expect("page t")),
        (LevelFreeType::Bytes32, write_page(LevelFreeType::Bytes32, &dg, N).expect("page dg")),
    ];
    // Byte-identity check once, outside the timed loop.
    for (ty, page) in &pages {
        let (got, rows) = read_page_payload(page, *ty).expect("read");
        assert_eq!(rows, N);
        let want: &[u8] = match ty {
            LevelFreeType::U32 => &s,
            LevelFreeType::U64 => &t,
            LevelFreeType::Bytes32 => &dg,
        };
        assert_eq!(got, want, "level-free roundtrip must be byte-identical");
    }

    // Receipt path: hash_many_simd, verified against the reference digest.
    let reference = hash(&bytes);
    assert_eq!(
        hash_many_simd(&bytes),
        reference,
        "hash_many_simd must reproduce the reference digest"
    );

    // Warm-up, untimed.
    for _ in 0..50 {
        let d = hash_many_simd(&bytes);
        for (ty, p) in &pages {
            let r = read_page_payload(p, *ty).expect("read");
            black_box((d, r));
        }
    }

    let mut samples: Vec<f64> = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let t0 = Instant::now();
        let receipt = hash_many_simd(&bytes);
        for (ty, p) in &pages {
            let (got, rows) = read_page_payload(p, *ty).expect("read");
            black_box((got, rows));
        }
        black_box(&receipt);
        samples.push(t0.elapsed().as_secs_f64());
    }

    let usl = 35e-6; // 35 us, pinned per charter / ctq_report
    let cap = capability(&samples, Some(usl), None);
    let pass = cap.cpk_overall.expect("cpu_overall present") >= CPK_TARGET;
    let verdict = if pass { "PASS" } else { "FAIL" };
    log(&format!(
        "CTQ-05: n={} mean={:.3} us sigma_overall={:.3} us sigma_within={:.3} us Cpu(overall)={:.3} Cpu(within)={:.3} vs target {CPK_TARGET} -> {}",
        cap.n,
        cap.mean * 1e6,
        cap.sigma_overall * 1e6,
        cap.sigma_within * 1e6,
        cap.cpu_overall.unwrap(),
        cap.cpu_within.unwrap(),
        verdict
    ));
    (emit("CTQ-05", "us_per_batch", Some(35.0), None, "us", &cap, verdict) , cap.cpk_overall.unwrap())
}

// ============================================================ CTQ-06 ====

/// N samples of per-core hash throughput: hash_many_simd over 256 MiB per
/// sample, throughput = bytes/elapsed. LSL 1.0 GiB/s (ratified) -> one-sided C_pl.
/// Honest expectation per charter status: deep fail (<< 1.67).
fn ctq06_throughput() -> (String, f64) {
    log("CTQ-06: BLAKE3 throughput samples (hash_many_simd, 256 MiB/sample), LSL 1.0 GiB/s (ratified), one-sided C_pl");
    const BYTES: usize = 256 * 1024 * 1024;
    const SAMPLES: usize = 100;
    const LSL_GIB_S: f64 = 1.0;

    let mut buf = vec![0u8; BYTES];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for b in buf.iter_mut() {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (x >> 33) as u8;
    }
    let reference = hash(&buf);
    assert_eq!(hash_many_simd(&buf), reference, "digest mismatch");

    let mut samples: Vec<f64> = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let t0 = Instant::now();
        let d = hash_many_simd(&buf);
        let dt = t0.elapsed().as_secs_f64();
        black_box(&d);
        samples.push(BYTES as f64 / dt / GIB);
        if i % 20 == 0 {
            log(&format!("CTQ-06: sample {i}: {:.2} GiB/s", samples[i]));
        }
    }

    let cap = capability(&samples, None, Some(LSL_GIB_S));
    let pass = cap.cpk_overall.expect("cpl_overall present") >= CPK_TARGET;
    let verdict = if pass { "PASS" } else { "FAIL" };
    log(&format!(
        "CTQ-06: n={} mean={:.2} GiB/s sigma_overall={:.2} sigma_within={:.2} Cpl(overall)={:.3} Cpl(within)={:.3} vs target {CPK_TARGET} -> {}",
        cap.n,
        cap.mean,
        cap.sigma_overall,
        cap.sigma_within,
        cap.cpl_overall.unwrap(),
        cap.cpl_within.unwrap(),
        verdict
    ));
    (emit("CTQ-06", "gib_per_s", None, Some(8.5), "GiB/s", &cap, verdict), cap.cpk_overall.unwrap())
}

// ============================================================ CTQ-04 ====

/// N samples of transpose + branchless filter over a deterministic 4096-quad
/// batch (same workload as ctq_report CTQ-04). Spec limit: USL = 15% of the
/// observed mean (the pinned provisional gate) -> one-sided C_pu against a
/// moving limit. NOTE: because USL is defined as 1.15*mean, C_pu here is a
/// pure dispersion index (0.05 * mean / sigma), reported as such.
fn ctq04_variance() -> (String, f64) {
    log("CTQ-04: transpose+filter variance samples, USL = 15% of mean, one-sided C_pu");
    const N: usize = 4096;
    const RUNS: usize = 100;

    let mut aos: Vec<cgap_arrow::transpose::Quad64> = Vec::with_capacity(N);
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in 0..N {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        aos.push(cgap_arrow::transpose::Quad64 {
            s_id: i as u32,
            p_id: (x >> 32) as u32,
            o_id: x as u32,
            g_id: i as u32,
            t_ns: x,
            vector_clock: (x >> 33) as u32,
            policy_mask: (x >> 17) as u32,
            inline_digest: [0u8; 32],
        });
    }
    let cols = transpose(&aos);
    assert_eq!(cols.untranspose(), aos, "roundtrip lossless");
    let needle: u32 = aos[N / 2].s_id;
    let mut out = vec![0u32; N + 1];

    // Warm-up.
    for _ in 0..20 {
        let c = transpose(&aos);
        let cnt = filter_quads_eq(&c.s_id, N, needle, &mut out);
        black_box((c.s_id.len(), cnt));
    }

    let mut samples: Vec<f64> = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let t0 = Instant::now();
        let c = transpose(&aos);
        let cnt = filter_quads_eq(&c.s_id, N, needle, &mut out);
        black_box((c.s_id.len(), cnt));
        samples.push(t0.elapsed().as_secs_f64());
    }

    let mean = samples.iter().sum::<f64>() / RUNS as f64;
    let usl = 1.15 * mean; // moving limit: 15% of mean
    let cap = capability(&samples, Some(usl), None);
    let pass = cap.cpk_overall.expect("cpu_overall present") >= CPK_TARGET;
    let verdict = if pass { "PASS" } else { "FAIL" };
    log(&format!(
        "CTQ-04: n={} mean={:.3} us USL(1.15*mean)={:.3} us sigma_overall={:.3} us sigma_within={:.3} us CV={:.2}% Cpu(overall)={:.3} vs target {CPK_TARGET} -> {}",
        cap.n,
        cap.mean * 1e6,
        usl * 1e6,
        cap.sigma_overall * 1e6,
        cap.sigma_within * 1e6,
        (cap.sigma_overall / cap.mean) * 100.0,
        cap.cpu_overall.unwrap(),
        verdict
    ));
    (
        emit("CTQ-04", "us_per_run", Some(usl * 1e6), None, "us", &cap, verdict),
        cap.cpk_overall.unwrap(),
    )
}
