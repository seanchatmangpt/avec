//! Gage R&R court: repeat-variance of the timing infrastructure itself.
//!
//! Spec: docs/jira/v26.10.31/29-benchmark-harness-plan.md §3 (DMADV Measure gate).
//!
//! Method: N=10,000 repetitions of a fixed ~10µs workload (BLAKE3 over a fixed
//! 4 KiB buffer, `avec_blake3::hash`), per-rep `Instant` deltas (monotonic,
//! CNTVCT-class on Apple Silicon), no sleeps.
//!
//! Stats: mean, median, p1, p99, stddev, coefficient of variation.
//!
//! Trimming rule (documented per spec): sort the samples, drop the top 1% and
//! the bottom 1% (100 samples each side at N=10,000), recompute mean/median/
//! stddev/CV on the remaining 98%. `cv_trimmed` is the gage statistic.
//!
//! Gage criterion: cv_trimmed < 1%. Exit 0 iff pass (fail-closed).
//!
//! Knobs: `--scale N` multiplies the fixed 4 KiB workload buffer by N
//! (default 1; larger workload dilutes fixed scheduler jitter); `--reps N`
//! overrides the sample count (default 10,000).
//!
//! Output: JSON to stdout, human log to stderr.

use std::io::Write;
use std::time::Instant;

const N_REPS_DEFAULT: usize = 10_000;
/// Fixed 4 KiB workload buffer (contents fixed; hash must not be optimized away).
const WORKLOAD_LEN_BASE: usize = 4 * 1024;
/// Gage pass threshold on the trimmed CV.
const GAGE_CV_THRESHOLD_PCT: f64 = 1.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    let idx = (p / 100.0) * (n as f64 - 1.0);
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

/// Named repair (spec §3): larger workload scale dilutes fixed scheduler jitter.
/// `--scale N` multiplies the 4 KiB base buffer by N (default 1).
/// `--reps N` overrides the sample count (default 10,000).
fn parse_args() -> (usize, usize) {
    let mut scale: usize = 1;
    let mut reps: usize = N_REPS_DEFAULT;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--scale" => {
                scale = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| panic!("--scale requires a positive integer"));
            }
            "--reps" => {
                reps = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| panic!("--reps requires a positive integer"));
            }
            other => panic!("unknown arg: {} (supported: --scale N, --reps N)", other),
        }
    }
    (scale.max(1), reps.max(1))
}

fn main() {
    let mut stderr = std::io::stderr();
    let (scale, n_reps) = parse_args();
    let workload_len = WORKLOAD_LEN_BASE * scale;
    let _ = writeln!(stderr, "config: scale={} workload_bytes={} n_reps={}", scale, workload_len, n_reps);

    // Fixed workload buffer.
    let mut buf = vec![0u8; workload_len];
    // Simple deterministic fill (xorshift) — fixed across reps, no runtime deps.
    let mut x: u64 = 0x9E3779B97F4A7C15;
    for b in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }

    // Black box so the optimizer cannot elide the hash.
    std::hint::black_box(&buf);

    // Warmup (not measured): caches, page tables, hasher state.
    let warmup_reps = 500;
    let mut sink: u8 = 0;
    for _ in 0..warmup_reps {
        let d = avec_blake3::hash(std::hint::black_box(&buf));
        sink = sink.wrapping_add(d[0]);
    }
    let _ = writeln!(stderr, "warmup: {} reps, sink={}", warmup_reps, sink);

    let mut samples_us: Vec<f64> = Vec::with_capacity(n_reps);
    for _ in 0..n_reps {
        let t0 = Instant::now();
        let d = avec_blake3::hash(std::hint::black_box(&buf));
        let dt = t0.elapsed().as_secs_f64() * 1e6;
        sink = sink.wrapping_add(d[0]);
        samples_us.push(dt);
    }
    std::hint::black_box(&sink);
    let _ = writeln!(stderr, "measured: {} reps", samples_us.len());

    // ---- Stats ----
    let workload_desc = format!("avec_blake3::hash over fixed {}-byte buffer (scale {})", workload_len, scale);
    let mut sorted = samples_us.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let n = samples_us.len() as f64;
    let mean = samples_us.iter().sum::<f64>() / n;
    let median = percentile(&sorted, 50.0);
    let p1 = percentile(&sorted, 1.0);
    let p99 = percentile(&sorted, 99.0);
    let var = samples_us.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let stddev = var.sqrt();
    let cv_raw = stddev / mean * 100.0;

    // Trimmed subset: drop bottom 1% and top 1% of sorted samples.
    let trim = sorted.len() / 100; // 100 samples at N=10,000
    let trimmed = &sorted[trim..sorted.len() - trim];
    let tn = trimmed.len() as f64;
    let t_mean = trimmed.iter().sum::<f64>() / tn;
    let t_median = percentile(trimmed, 50.0);
    let t_var = trimmed.iter().map(|s| (s - t_mean).powi(2)).sum::<f64>() / (tn - 1.0);
    let t_stddev = t_var.sqrt();
    let cv_trimmed = t_stddev / t_mean * 100.0;

    let gage_pass = cv_trimmed < GAGE_CV_THRESHOLD_PCT;

    // ---- JSON (stdout) ----
    let json = format!(
        r#"{{"timer":"Instant (monotonic, CNTVCT-class on Apple Silicon)","workload":"{}","scale":{},"n_samples":{},"warmup_reps":{},"trim_rule":"drop bottom 1% and top 1% of sorted samples ({} per side at N={})","trimmed_n":{},"stats":{{"mean_us":{:.4},"median_us":{:.4},"p1_us":{:.4},"p99_us":{:.4},"stddev_us":{:.4},"cv_raw_pct":{:.4},"mean_trimmed_us":{:.4},"median_trimmed_us":{:.4},"stddev_trimmed_us":{:.4}}},"cv_raw_pct":{:.4},"cv_trimmed_pct":{:.4},"gage_threshold_pct":{},"gage_pass":{}}}"#,
        workload_desc,
        scale,
        samples_us.len(),
        warmup_reps,
        trim,
        samples_us.len(),
        trimmed.len(),
        mean,
        median,
        p1,
        p99,
        stddev,
        cv_raw,
        t_mean,
        t_median,
        t_stddev,
        cv_raw,
        cv_trimmed,
        GAGE_CV_THRESHOLD_PCT,
        gage_pass
    );
    let _ = writeln!(std::io::stdout(), "{}", json);

    let _ = writeln!(
        stderr,
        "gage: cv_raw={:.4}% cv_trimmed={:.4}% threshold={}% pass={}",
        cv_raw, cv_trimmed, GAGE_CV_THRESHOLD_PCT, gage_pass
    );

    if !gage_pass {
        std::process::exit(1);
    }
}
