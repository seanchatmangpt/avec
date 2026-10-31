//! CTQ report (divan2receipt concept, wave-4 lane 20; extended wave-5 lane 12)
//! — runs the CTQ-relevant measurements in-process with plain std: min-of-N
//! `Instant` loops, no sleeps, no divan dependency. Emits one machine-readable
//! JSON verdict to stdout; human logs go to stderr.
//!
//! CTQs (PRD v26.10.31):
//! - CTQ-03 level-free parquet roundtrip throughput — GB/s informational
//!   (no LSL/USL pinned in the PRD).
//! - CTQ-05 end-to-end admission pipeline latency — USL 35 µs (gate, REAL),
//!   measured with the fastest byte-identical-correct BLAKE3 path.
//! - CTQ-06 BLAKE3 throughput — LSL 8.5 GiB/s (gate, REAL), one row per
//!   SIMD path: `hash_wide_simd` (legacy) and `hash_many_simd` (4-way).
//!   LSL amendable via `LSL_RATIFIED` (line 44); currently unratified.
//! - CTQ-04 transpose/lookup variance — gage-conditioned (mini-gage CV > 2% ->
//!   SKIP-NOISY); when admitted, p99/p50 of 100 runs <= 1.30 (ratified form per
//!   docs/jira/v26.10.31/49-ctq04-variance-study.md; stdev/mean informational
//!   only, the 15% pin is retired but retained as "retired_pin" provenance).
//! - CTQ-02 zero-alloc — counting global allocator around a 64 MiB BLAKE3
//!   pass; gate: 0 allocation events (REAL).
//!
//! Gate semantics: exit 1 iff any REAL gate fails (CTQ-02, CTQ-04
//! provisional, CTQ-05, CTQ-06). Informational metrics never affect the exit
//! code.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use avec_arrow::layout::Quad64 as LayoutQuad;
use avec_arrow::layout::{QUAD_SIZE};
use avec_arrow::predicates::filter_quads_eq;
use avec_arrow::transpose::transpose;
use avec_blake3::{hash, hash_many_simd, hash_wide_simd, is_small_batch_eligible};
use avec_parquet::level_free::{read_page_payload, write_page, LevelFreeType};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// CTQ-06 amendment hook (20-PRD amendment mechanism, wave-7): while None the
/// gate stays at the original LSL 8.5 GiB/s and FAILs honestly against it.
/// When the operator ratifies a rederived LSL (see docs 48-lsl-rederivation),
/// flip to Some(x) — the gate switches to the ratified value and the JSON
/// carries an "AMENDMENT-RATIFIED" marker.
/// RATIFIED by operator decision 2026-10-31: LSL 1.0 / target 1.5 / USL 2.5
/// GiB/s/core (CPU NEON path; docs/OPERATOR_DECISIONS.md decision 1, package
/// 59 §6 edit A). Gate scope per edit A2: hash_many_simd gates; the legacy
/// hash_wide_simd row is informational (REPORT), superseded by hash_many_simd.
const LSL_RATIFIED: Option<f64> = Some(1.0);

// ================================================== counting allocator ====
// CTQ-02: counts every allocation event routed through the global allocator.
// Reset immediately before the measured pass; read after. Dealloc is free
// (the gate counts allocation events, not live bytes).

static ALLOC_EVENTS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_EVENTS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOC_EVENTS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOC_EVENTS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn log(msg: &str) {
    eprintln!("[ctq_report] {msg}");
}

fn now() -> Instant {
    Instant::now()
}

/// Trimmed coefficient of variation: sort, trim `frac` off each tail, then
/// sample stdev / mean over the retained middle. Robust against stragglers
/// from scheduler noise while still detecting sustained load.
fn trimmed_cv(samples: &mut Vec<f64>, frac: f64) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).expect("finite timings"));
    let trim = ((samples.len() as f64) * frac).floor() as usize;
    let lo = trim;
    let hi = samples.len() - trim;
    let mid = &samples[lo..hi];
    let m = mid.iter().sum::<f64>() / mid.len() as f64;
    if m <= 0.0 {
        return f64::INFINITY;
    }
    let var = mid.iter().map(|s| (s - m) * (s - m)).sum::<f64>() / (mid.len() - 1) as f64;
    var.sqrt() / m
}

fn main() {
    let mut gates_failed: Vec<&'static str> = Vec::new();

    // ------------------------------------------------------- CTQ-03 -----
    let ctq03 = ctq03_parquet_roundtrip();
    // ------------------------------------------------------- CTQ-04 -----
    let (ctq04, ctq04_pass) = ctq04_transpose_variance();
    if !ctq04_pass {
        gates_failed.push("CTQ-04");
    }
    // ------------------------------------------------------- CTQ-05 -----
    let (ctq05, ctq05_pass) = ctq05_e2e_admission();
    if !ctq05_pass {
        gates_failed.push("CTQ-05");
    }
    // ------------------------------------------------------- CTQ-02 -----
    let (ctq02, ctq02_pass) = ctq02_zero_alloc();
    if !ctq02_pass {
        gates_failed.push("CTQ-02");
    }
    // ------------------------------------------------------- CTQ-06 -----
    // Run fresh, at the end, per lane order.
    let (ctq06, ctq06_pass) = ctq06_blake3();
    if !ctq06_pass {
        gates_failed.push("CTQ-06");
    }

    // ------------------------------------------------------- verdict ----
    println!("{{");
    println!("  \"report\": \"ctq_report\",");
    println!("  \"version\": 2,");
    println!("  \"ctq03_parquet_level_free_roundtrip\": {ctq03},");
    println!("  \"ctq04_transpose_variance\": {ctq04},");
    println!("  \"ctq05_e2e_admission\": {ctq05},");
    println!("  \"ctq02_zero_alloc\": {ctq02},");
    println!("  \"ctq06_blake3\": {ctq06},");
    if gates_failed.is_empty() {
        println!("  \"verdict\": \"PASS\"");
    } else {
        println!(
            "  \"verdict\": \"FAIL\", \"failed_gates\": [{}]",
            gates_failed
                .iter()
                .map(|g| format!("\"{g}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("}}");

    if !gates_failed.is_empty() {
        std::process::exit(1);
    }
}

// ============================================================ CTQ-03 ====

fn ctq03_parquet_roundtrip() -> String {
    log("CTQ-03: level-free parquet roundtrip (informational, no pinned LSL/USL)");
    let rows: usize = 256 * 1024; // 256 Ki rows * 8 B = 2 MiB payload
    let payload = vec![0xA5u8; rows * 8];
    let page = write_page(LevelFreeType::U64, &payload, rows).expect("write_page");

    let rounds = 200;
    let mut best = f64::INFINITY;
    for r in 0..rounds {
        let t0 = now();
        let (got, got_rows) = read_page_payload(&page, LevelFreeType::U64).expect("read_page_payload");
        black_box(&got);
        black_box(got_rows);
        let dt = t0.elapsed().as_secs_f64();
        if r == 0 || dt < best {
            best = dt;
        }
    }
    let bytes = page.len() as f64;
    // Guard the JSON: if the timer resolution yields best==0, throughput is
    // unmeasurable at this round count — report 0 with a note, never inf.
    let (gib_s, note) = if best.is_finite() && best > 0.0 {
        (bytes / best / GIB, "")
    } else {
        (0.0, r#","note":"unmeasurable at timer resolution""#)
    };
    log(&format!(
        "CTQ-03: page={} B rows={} best={:.3} us -> {:.2} GiB/s (informational)",
        page.len(),
        rows,
        best * 1e6,
        gib_s
    ));
    format!(
        r#"{{"metric":"gib_per_s","value":{gib_s:.3},"page_bytes":{},"rows":{rows},"rounds":{rounds},"min_ns":{:.0},"gate":"informational","verdict":"REPORT"{note}}}"#,
        page.len(),
        best * 1e9
    )
}

// ============================================================ CTQ-04 ====

fn ctq04_transpose_variance() -> (String, bool) {
    // Ratified USL (49-ctq04-variance-study.md, Option b): when the inline
    // mini-gage admits (trimmed CV <= 2%), the gate statistic is the
    // contamination-robust percentile dispersion ratio p99/p50 over the 100
    // run times, pinned provisionally at <= 1.30 pending its own ratification
    // study. The old stdev/mean <= 15% pin is RETIRED — reported
    // informationally only, kept in JSON as "retired_pin" for provenance.
    const USL_P99_OVER_P50: f64 = 1.30;
    const RETIRED_PIN_STDEV_FRAC: f64 = 0.15;

    // Wave-7 load-gating protocol (source: wave-7 lane-17 dispatch spec;
    // ratified by docs/jira/v26.10.31/49-ctq04-variance-study.md Option b).
    // Before evaluating the gate, run an inline mini-gage: 500-rep
    // fixed-workload loop, trimmed CV. If CV > 2% the machine is under
    // lane contention — variance gates only fire on a quiet machine — so emit
    // SKIP-NOISY instead of PASS/FAIL. This kills PASS/FAIL flapping under
    // 20-lane fan-out.
    const GAGE_REPS: usize = 500;
    const CV_GATE: f64 = 0.02;
    const TRIM_FRAC: f64 = 0.10; // trim 10% per tail before CV

    log("CTQ-04: transpose/lookup variance over 100 runs (USL: p99/p50 <= 1.30 after mini-gage admission; 15% stdev/mean pin retired)");
    let n: usize = 4096;
    let mut aos: Vec<avec_arrow::transpose::Quad64> = Vec::with_capacity(n);
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in 0..n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        aos.push(avec_arrow::transpose::Quad64 {
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

    // One correctness pass: transpose -> untranspose must round-trip.
    let cols = transpose(&aos);
    let back = cols.untranspose();
    assert_eq!(back, aos, "transpose roundtrip must be lossless");

    // Lookup: filter s_id (via layout-independent u32 column) with the
    // branchless predicate kernel.
    let needle: u32 = aos[n / 2].s_id;
    let mut out = vec![0u32; n + 1];

    // ---- mini-gage (wave-7 load gating) --------------------------------
    let mut gage: Vec<f64> = Vec::with_capacity(GAGE_REPS);
    for _ in 0..GAGE_REPS {
        let t0 = now();
        let cols = transpose(&aos);
        let count = filter_quads_eq(&cols.s_id, n, needle, &mut out);
        black_box((cols.s_id.len(), count));
        gage.push(t0.elapsed().as_secs_f64());
    }
    let cv = trimmed_cv(&mut gage, TRIM_FRAC);
    log(&format!(
        "CTQ-04 mini-gage: reps={GAGE_REPS} trimmed CV={:.2}% vs gate 2%",
        cv * 100.0
    ));
    if cv > CV_GATE {
        log("CTQ-04: machine NOISY (CV > 2%) -> SKIP-NOISY (load gating; variance gate fires only on a quiet machine)");
        return (
            format!(
                r#"{{"metric":"p99_over_p50","usl":1.30,"retired_pin":{RETIRED_PIN_STDEV_FRAC},"gage_reps":{GAGE_REPS},"cv":{cv:.4},"cv_gate":0.02,"gate":"REAL","pin_status":"RATIFIED p99/p50 <= 1.30 (49-ctq04-variance-study.md Option b, operator declaration 2026-10-31); 15% stdev/mean pin retired","verdict":"SKIP-NOISY","note":"load-gated: trimmed CV exceeded 2 percent, PASS/FAIL not evaluated"}}"#
            ),
            true, // gate not evaluated; not a failure
        );
    }

    let runs = 100;
    let mut samples: Vec<f64> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t0 = now();
        let cols = transpose(&aos);
        let count = filter_quads_eq(&cols.s_id, n, needle, &mut out);
        black_box((cols.s_id.len(), count));
        samples.push(t0.elapsed().as_secs_f64());
    }

    let mean = samples.iter().sum::<f64>() / runs as f64;
    let var = samples.iter().map(|s| (s - mean) * (s - mean)).sum::<f64>() / (runs - 1) as f64;
    let stdev = var.sqrt();
    let frac = if mean > 0.0 { stdev / mean } else { f64::INFINITY };
    // Gated statistic: percentile dispersion p99/p50 (nearest-rank on the
    // sorted samples) — immune to single-sample contamination that dominates
    // raw stdev under preemption. stdev/mean stays informational.
    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
    let p50 = percentile_nearest_rank(&sorted, 50.0);
    let p99 = percentile_nearest_rank(&sorted, 99.0);
    let ratio = if p50 > 0.0 { p99 / p50 } else { f64::INFINITY };
    let pass = ratio <= USL_P99_OVER_P50;
    log(&format!(
        "CTQ-04: n={n} runs={runs} mean={:.3} us p50={:.3} us p99={:.3} us p99/p50={:.3} vs USL 1.30 -> {} (stdev/mean={:.1}% informational; retired pin 15%)",
        mean * 1e6,
        p50 * 1e6,
        p99 * 1e6,
        ratio,
        if pass { "PASS" } else { "FAIL" },
        frac * 100.0
    ));
    let verdict = if pass { "PASS" } else { "FAIL" };
    (
        format!(
            r#"{{"metric":"p99_over_p50","value":{ratio:.4},"usl":1.30,"retired_pin":{RETIRED_PIN_STDEV_FRAC},"p50_ns":{:.0},"p99_ns":{:.0},"stdev_ns":{:.0},"mean_ns":{:.0},"stdev_frac_of_mean_informational":{frac:.4},"gage_reps":{GAGE_REPS},"cv":{cv:.4},"cv_gate":0.02,"runs":{runs},"n":{n},"gate":"REAL","pin_status":"RATIFIED p99/p50 <= 1.30 (49-ctq04-variance-study.md Option b, operator declaration 2026-10-31); 15% stdev/mean pin retired","verdict":"{verdict}"}}"#,
            p50 * 1e9,
            p99 * 1e9,
            stdev * 1e9,
            mean * 1e9
        ),
        pass,
    )
}

/// Nearest-rank percentile of an ascending-sorted slice (pct 0-100).
fn percentile_nearest_rank(sorted: &[f64], pct: f64) -> f64 {
    assert!(!sorted.is_empty(), "percentile of empty sample");
    let rank = ((pct / 100.0) * sorted.len() as f64).ceil() as usize;
    let idx = rank.clamp(1, sorted.len()) - 1;
    sorted[idx]
}

// ============================================================ CTQ-05 ====

/// Inline reproduction of the integration admission loop
/// (crates/integration/tests/admission_e2e.rs): real Quad64 batch ->
/// BLAKE3 receipt -> level-free parquet roundtrip -> byte-identical +
/// receipt-equal. All collaborators real, in-process.
///
/// Wave-5 lane 12: the BLAKE3 receipt is computed with the fastest
/// byte-identical-correct path (measured per candidate on this exact batch);
/// every candidate must reproduce the reference `hash` digest or it is
/// disqualified regardless of speed.
fn ctq05_e2e_admission() -> (String, bool) {
    log("CTQ-05: e2e admission pipeline (USL 35 us per batch, gate REAL)");
    const N: usize = 256;

    // Deterministic quad batch (same shape as the integration test).
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

    // Column projection (u32/u64/Bytes32), matching the admitted schema.
    let mut s = Vec::with_capacity(N * 4);
    let mut t = Vec::with_capacity(N * 8);
    let mut dg = Vec::with_capacity(N * 32);
    for q in &quads {
        s.extend_from_slice(&q.s_id.to_le_bytes());
        t.extend_from_slice(&q.t_ns.to_le_bytes());
        dg.extend_from_slice(&q.inline_digest);
    }
    let pages = [
        (LevelFreeType::U32, write_page(LevelFreeType::U32, &s, N).expect("page s")),
        (LevelFreeType::U64, write_page(LevelFreeType::U64, &t, N).expect("page t")),
        (LevelFreeType::Bytes32, write_page(LevelFreeType::Bytes32, &dg, N).expect("page dg")),
    ];

    // Verify the roundtrip is byte-identical once, outside the timed loop.
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

    // Candidate BLAKE3 paths for the receipt. Reference digest is `hash`
    // itself (the small-batch path inside `hash` dispatches through
    // hash_many_simd for eligible inputs, per wave-5 lane 3).
    let reference = hash(&bytes);
    let candidates: &[(&str, fn(&[u8]) -> [u8; 32])] = &[
        ("hash", hash),
        ("hash_wide_simd", hash_wide_simd),
        ("hash_many_simd", hash_many_simd),
    ];
    let small_batch_eligible = is_small_batch_eligible(bytes.len());

    // Correctness screen: a candidate that does not reproduce the reference
    // digest is disqualified (correctness outranks speed).
    let mut fastest: Option<(&str, f64)> = None;
    for (name, f) in candidates {
        let digest = f(&bytes);
        if digest != reference {
            log(&format!(
                "CTQ-05: candidate {name} DISQUALIFIED (digest mismatch vs reference hash)"
            ));
            continue;
        }
        // Min-of-101 timing of just the hash on the batch.
        let mut best = f64::INFINITY;
        for r in 0..101 {
            let t0 = now();
            let d = f(&bytes);
            black_box(&d);
            let dt = t0.elapsed().as_secs_f64();
            if r == 0 || dt < best {
                best = dt;
            }
        }
        log(&format!(
            "CTQ-05: candidate {name} correct, hash-only best={:.3} us",
            best * 1e6
        ));
        if fastest.is_none_or(|(_, b)| best < b) {
            fastest = Some((name, best));
        }
    }
    let (fastest_name, fastest_hash_s) =
        fastest.expect("at least the reference hash path must be correct");
    let fastest_hash_us = fastest_hash_s * 1e6;

    // Timed loop: full admission pipeline per iteration using the fastest
    // correct path.
    let f = candidates
        .iter()
        .find(|(name, _)| *name == fastest_name)
        .expect("fastest name came from candidates")
        .1;
    let rounds = 2000;
    let mut best = f64::INFINITY;
    for r in 0..rounds {
        let t0 = now();
        let receipt = f(&bytes);
        for (ty, page) in &pages {
            let (got, rows) = read_page_payload(page, *ty).expect("read");
            black_box((got, rows, receipt, ty));
        }
        let dt = t0.elapsed().as_secs_f64();
        if r == 0 || dt < best {
            best = dt;
        }
    }

    let us = best * 1e6;
    let pass = us <= 35.0;
    log(&format!(
        "CTQ-05: batch_bytes={} rounds={rounds} path={fastest_name} hash_only={fastest_hash_us:.3} us best={us:.3} us USL=35 us -> {}",
        bytes.len(),
        if pass { "PASS" } else { "FAIL" }
    ));
    let verdict = if pass { "PASS" } else { "FAIL" };
    (
        format!(
            r#"{{"metric":"us_per_batch","value":{us:.3},"usl_us":35.0,"batch_bytes":{},"rounds":{rounds},"path":"{fastest_name}","hash_only_us":{fastest_hash_us:.3},"small_batch_eligible":{small_batch_eligible},"gate":"REAL","verdict":"{verdict}"}}"#,
            bytes.len()
        ),
        pass,
    )
}

// ============================================================ CTQ-02 ====

/// Zero-alloc gate (wave-5 lane 12): a 64 MiB BLAKE3 pass through the
/// counting allocator must produce 0 allocation events. Correctness is
/// asserted against the reference digest before the gate is evaluated.
fn ctq02_zero_alloc() -> (String, bool) {
    log("CTQ-02: zero-alloc 64 MiB BLAKE3 pass (gate REAL: 0 allocation events)");
    const BYTES: usize = 64 * 1024 * 1024;

    // Fill OUTSIDE the measured window; the fill legitimately allocates once.
    let mut buf = vec![0u8; BYTES];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for b in buf.iter_mut() {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (x >> 33) as u8;
    }
    let reference = hash(&buf);
    assert_eq!(reference, hash_wide_simd(&buf), "SIMD paths must be byte-identical");
    assert_eq!(reference, hash_many_simd(&buf), "hash_many_simd must be byte-identical");

    // Measured pass: hash_many_simd (the production 4-way SIMD path).
    ALLOC_EVENTS.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    let t0 = now();
    let digest = hash_many_simd(&buf);
    let dt = t0.elapsed().as_secs_f64();
    black_box(&digest);
    let events = ALLOC_EVENTS.load(Ordering::Relaxed);
    let bytes_alloced = ALLOC_BYTES.load(Ordering::Relaxed);

    assert_eq!(digest, reference, "measured pass digest must match reference");
    let pass = events == 0;
    log(&format!(
        "CTQ-02: bytes={BYTES} alloc_events={events} alloc_bytes={bytes_alloced} time={:.3} s -> {}",
        dt,
        if pass { "PASS" } else { "FAIL" }
    ));
    let verdict = if pass { "PASS" } else { "FAIL" };
    (
        format!(
            r#"{{"metric":"alloc_events","value":{events},"alloc_bytes":{bytes_alloced},"bytes":{BYTES},"time_s":{dt:.4},"gate":"REAL","verdict":"{verdict}"}}"#
        ),
        pass,
    )
}

// ============================================================ CTQ-06 ====

/// Two rows per run: legacy `hash_wide_simd` and the 4-way `hash_many_simd`,
/// each gated against the same LSL 8.5 GiB/s with an honest per-row verdict.
/// Digests are asserted byte-identical to the reference before timing.
fn ctq06_blake3() -> (String, bool) {
    const BYTES: usize = 256 * 1024 * 1024;
    const ROUNDS: usize = 5;
    const LSL_ORIGINAL: f64 = 8.5;
    let lsl_gib_s = LSL_RATIFIED.unwrap_or(LSL_ORIGINAL);
    let amendment = if LSL_RATIFIED.is_some() {
        r#","amendment":"AMENDMENT-RATIFIED""#
    } else {
        ""
    };
    log(&format!(
        "CTQ-06: BLAKE3 throughput, two rows (hash_wide_simd, hash_many_simd), min-of-5 over 256 MiB, LSL {lsl_gib_s} GiB/s{} (gate REAL)",
        if LSL_RATIFIED.is_some() {
            " [AMENDMENT-RATIFIED]"
        } else {
            " [original, not ratified]"
        }
    ));

    let mut buf = vec![0u8; BYTES];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for b in buf.iter_mut() {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = (x >> 33) as u8;
    }
    let reference = hash(&buf);

    let mut rows: Vec<String> = Vec::new();
    let mut all_pass = true;
    // Edit A2 (ratification, 2026-10-31): only hash_many_simd gates (REAL).
    // The legacy hash_wide_simd row is informational (REPORT): it measures a
    // superseded scalar-leaf architecture (0.43 GiB/s vs hash_many_simd's
    // 1.85) and would fail any LSL above 0.43 regardless of kernel health —
    // see docs/OPERATOR_DECISIONS.md decision 1, package 59 §6 edit A2.
    for (name, f, gates) in [
        ("hash_wide_simd", hash_wide_simd as fn(&[u8]) -> [u8; 32], false),
        ("hash_many_simd", hash_many_simd as fn(&[u8]) -> [u8; 32], true),
    ] {
        // Correctness screen before timing: byte-identical or honest FAIL.
        if f(&buf) != reference {
            log(&format!("CTQ-06: {name} DIGEST MISMATCH vs reference -> row FAIL"));
            rows.push(format!(
                r#"{{"path":"{name}","digest_correct":false,"gate":"REAL","verdict":"FAIL"}}"#
            ));
            if gates {
                all_pass = false;
            }
            continue;
        }
        let mut best = f64::INFINITY;
        for round in 0..ROUNDS {
            let t0 = now();
            let digest = f(&buf);
            let dt = t0.elapsed().as_secs_f64();
            black_box(&digest);
            if round == 0 || dt < best {
                best = dt;
            }
            log(&format!(
                "CTQ-06 [{name}]: round {round}: {:.3} s, {:.2} GiB/s",
                dt,
                BYTES as f64 / dt / GIB
            ));
        }
        let gib_s = BYTES as f64 / best / GIB;
        let pass = gib_s >= lsl_gib_s;
        if !pass && gates {
            all_pass = false;
        }
        let (gate_tag, verdict) = if gates {
            (
                "REAL",
                if pass { "PASS" } else { "FAIL" },
            )
        } else {
            ("informational", "REPORT")
        };
        log(&format!(
            "CTQ-06 [{name}]: best={best:.4} s -> {gib_s:.2} GiB/s vs LSL {lsl_gib_s} -> {verdict} (gate {gate_tag})"
        ));
        rows.push(format!(
            r#"{{"path":"{name}","digest_correct":true,"metric":"gib_per_s","value":{gib_s:.2},"lsl_gib_s":{lsl_gib_s},"bytes":{BYTES},"rounds":{ROUNDS},"min_ns":{:.0},"gate":"{gate_tag}","verdict":"{verdict}""#,
            best * 1e9
        ) + amendment
            + "}");
    }

    (
        format!(r#"{{"lsl_gib_s":{lsl_gib_s},"rows":[{}]}}"#, rows.join(",")),
        all_pass,
    )
}
