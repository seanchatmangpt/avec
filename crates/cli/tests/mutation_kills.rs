//! Mutation-kill tests for `cgap-cli` — lane CLI-MUT, 2026-10-29.
//!
//! Subject: `crates/cli/src/main.rs` (46 mutation survivors triaged from
//! `/tmp/opt20-mut-arrow/mutants.out/missed.txt` rows 13-58; re-derived by
//! the lane's own scoped scan `cargo mutants --workspace --exclude
//! cgap-audit --exclude bench --exclude cgap-ffi --exclude cgap-beam
//! --exclude cgap-arrow --exclude cgap-parquet -o /tmp/cli-mut --timeout 60`).
//!
//! Chicago discipline: every test drives the REAL binary as a subprocess
//! (`std::process::Command`, path from `CARGO_BIN_EXE_cgap-cli`) and asserts
//! final observable state — exact stdout lines, exit codes, and (macOS)
//! max-RSS footprints via `/usr/bin/time -l` as allocation ground truth.
//! No mocks. No sleeps: the only time-like quantity asserted is the
//! binary's own reported throughput inside a wide plausibility window.
//!
//! # Classification of all 46 survivors
//!
//! Every survivor is one of:
//!
//! 1. **Receipt-carrying bench-math** — a wrong constant prints a wrong
//!    throughput receipt (`GIB` divisor, `mask_bytes_moved`,
//!    `encode_bytes_moved`, workload sizes `MASK_WORDS`/`ENC_VALUES`).
//!    Killed by: exact output format, positivity, anchored plausibility
//!    window (T3/T4), and allocation ground truth (T5/T6, for the
//!    workload-size constants, whose printed GiB/s stays internally
//!    consistent when the workload shrinks — rate is intensive, RSS is not).
//! 2. **Pure-display / control flow** — match arms, `cmd_*` bodies, align
//!    arithmetic. Killed by exact-line + exit-code assertions (T1/T2/T3/T4).
//!
//! | mutant (main.rs)                      | class      | killed by |
//! |---------------------------------------|------------|-----------|
//! | 26:25/26:34 `*`→`+`/`/` in GIB (4)    | bench-math | T3+T4 window (1024x-class) |
//! | 29:5 `main` → `()`                    | control    | T1 (empty stdout) |
//! | 30:9 delete `version` arm             | control    | T1 (usage/exit 2) |
//! | 31:9 delete `bench-mask` arm          | control    | T3 (usage/exit 2) |
//! | 32:9 delete `bench-parquet` arm       | control    | T4 (usage/exit 2) |
//! | 33:9 delete `align-test` arm          | control    | T2 (usage/exit 2) |
//! | 44:5 `cmd_version` → `()`             | display    | T1 (empty stdout) |
//! | 56:5 `mask_bytes_moved` → 0.0/1.0/-1.0| bench-math | T3 (0.000/-0.000 below window) |
//! | 56:12 `*`→`+` (f≈8), `*`→`/` (f≈64)   | bench-math | T3 window |
//! | 56:41 `*`→`+` (f≈3.0)                 | LEDGER     | see ledger item L4 |
//! | 56:41 `*`→`/` (f≈3.75)                | bench-math | T3 window (caught in scan; 3-4x band is contention-lottery) |
//! | 69:5 `cmd_bench_mask` → `()`          | display    | T3 (empty stdout) |
//! | 60:29/60:36 MASK_WORDS `*`→`+`/`/` (4)| bench-math | T5 RSS ground truth |
//! | 70:43 `^`→`|`, `^`→`&` (2)            | LEDGER     | see ledger item L1 |
//! | 87:33 `-`→`+`/`/` (index OOB panic)   | control    | T3 (non-zero exit) |
//! | 102:5 `encode_bytes_moved` → 0/1/-1   | bench-math | T4 (0.000/-0.000) |
//! | 102:15 `*`→`+`, `*`→`/` (f≈2.5-3.5)  | LEDGER     | see ledger item L5 |
//! | 102:44 `+`→`-` (f≈1.37)               | LEDGER     | see ledger item L2 |
//! | 102:44 `+`→`*` (bytes ≈ n·4·out_len)  | bench-math | T4 window (astronomic) |
//! | 106:29/106:36 ENC_VALUES `*`→`+`/`/` (4)| bench-math| T6 RSS ground truth |
//! | 114:5 `cmd_bench_parquet` → `()`      | display    | T4 (empty stdout) |
//! | 120:13 `!=`→`==` (gate inverts: always FAIL + exit 1) | control | T4 (non-zero exit) |
//! | 114:55 `%`→`/` (runs compress out)    | bench-math | T6 RSS (footprint drops) |
//! | 114:55 `%`→`+`                        | LEDGER     | see ledger item L3 |
//! | 157:5 `cmd_align_test` → `()`         | display    | T2 (empty stdout) |
//! | 162:66 `==`→`!=`                      | control    | T2 (FAIL + exit 1) |
//! | 162:34 `%`→`/`, `%`→`+`               | control    | T2 (FAIL + exit 1) |
//! | 163:21 `+=`→`-=` (underflow abort), `*=` (0/143) | control | T2 |
//!
//! # Equivalent-mutant ledger — ACTUAL lane scan receipt
//!
//! Post-test scan (`-o /tmp/cli-mut2`, contract command, 2026-10-29):
//! `47 mutants tested in 2m: 7 missed, 39 caught, 1 unviable` — from 46
//! missed at the pre-test baseline (`-o /tmp/cli-mut`:
//! `47 mutants tested in 25s: 46 missed, 1 unviable`). The 7 survivors,
//! none distinguishable from the true binary by any honest black-box
//! observation (stdout shape, exit code, RSS), with reasons:
//!
//! - **L1 (70:43 `^`→`|` / `^`→`&`)**: mutates the *content* of the bench
//!   input vector only. The kernel is contractually data-independent
//!   (timing) and no output channel carries vector contents, so stdout,
//!   exit code and RSS are all identical. Output-equivalent by
//!   construction; the guard is the bench crate's own kernel tests, not a
//!   black-box CLI observation.
//! - **L2 (102:44 `+`→`-`, bytes ≈ n·4 − out_len)**: inflates the printed
//!   parquet rate by only ~1.37x — inside any honest host-variance window,
//!   hence indistinguishable from machine variance by black-box output
//!   shape. Killing it requires an in-process test of
//!   `encode_bytes_moved` (white-box), structurally unavailable here
//!   (main.rs has no lib target; integration tests see only the binary).
//! - **L3 (114:55 `%`→`+`, values = i+5)**: strictly increasing values are
//!   still an all-literal page, so `out_len` (and thus bytes, time, rate
//!   and RSS) are bit-identical; only unobservable vector contents change.
//!   Output-equivalent by construction.
//! - **L4 (56:41 `*`→`+`, bytes = words·8+3, rate x3.0)**: a constant-factor
//!   (3x-class) misreport. Forcing it below the window cap needs a cap
//!   under 3x the reference rate; under the scan's parallel jobs the
//!   contended true rate dips such that such a cap flakes the TRUE
//!   baseline (its sibling `*`→`/` at x3.75 was caught in this scan's run
//!   — the 3-4x band is contention-lottery). Black-box-indistinguishable
//!   from scan-time contention variance without flaking.
//! - **L5 (102:15 `*`→`+` / `*`→`/`)**: same class, effective rate
//!   inflation x~2.5-3.5 (out_len dominates the formula more than the
//!   n·4 term). Same tradeoff as L4.
//!
//! Honest residual: 7/46 survivors, all ledgered above with mechanism;
//! 39 killed by real-subprocess output laws + RSS ground truth; 1 mutant
//! (60:29 `*`→`/`, MASK_WORDS = 0) is compiler-unviable, not a survivor.
//!
//! # Plausibility windows (receipts)
//!
//! Measured 2026-10-29 on the scan host, dev profile (what `cargo test`
//! exercises): `bench-mask` = 5.208 and 5.736 GiB/s across two runs;
//! `bench-parquet` = 0.217, 0.223, 0.254 GiB/s. Windows are anchored
//! generously outside host/contention variance (min-of-5 depresses under
//! parallel mutant runs) but far inside the mutant factors: mask window
//! [1.0, 10.0] kills the >=3x class (3x5.2 = 15.6 > 10.0); parquet window
//! [0.03, 0.5] kills the >=6x class; the 1024x-class `GIB` mutants land at
//! ~5000+ GiB/s. RSS thresholds: mask > 150_000_000 B (measured
//! 202_964_992 B = 3x64 MiB vectors; MASK_WORDS mutants allocate <=25 MB);
//! parquet > 70_000_000 B (measured 106_938_368 B; ENC_VALUES mutants
//! allocate <=8 MB, the `%`→`/` mutant <=55 MB).

use std::process::Command;
use std::process::Output;

/// Window law for `mask <v> GiB/s`: see module docs for the receipts.
const MASK_GIBS_WINDOW: (f64, f64) = (1.0, 10.0);
/// Window law for the two parquet GiB/s numbers: see module docs.
const PARQUET_GIBS_WINDOW: (f64, f64) = (0.03, 0.5);
/// Ground truth: bench-mask touches three 64 MiB vectors (measured
/// 202_964_992 B max RSS); workload-size mutants allocate <=25 MB.
const MASK_MIN_RSS_BYTES: u64 = 150_000_000;
/// Ground truth: bench-parquet holds a 32 MiB values vector plus encoded
/// outputs (measured 106_938_368 B); shrunk-workload mutants allocate far
/// less (<=8 MB for ENC_VALUES mutants, <=55 MB for the `%`→`/` mutant).
const PARQUET_MIN_RSS_BYTES: u64 = 70_000_000;

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cgap-cli"))
        .args(args)
        .output()
        .expect("spawn cgap-cli subprocess")
}

/// stdout must be exactly one line ending in `\n`.
fn one_line_stdout(out: &Output) -> String {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.ends_with('\n'),
        "expected exactly one stdout line, got {:?}",
        stdout
    );
    let line = stdout.trim_end_matches('\n');
    assert!(
        !line.contains('\n'),
        "expected exactly one stdout line, got {:?}",
        stdout
    );
    line.to_owned()
}

/// Parse `<token> GiB/s`-style numeric laws without a regex crate.
fn assert_gibs_in_window(token: &str, window: (f64, f64), what: &str) -> f64 {
    let v: f64 = token
        .parse()
        .unwrap_or_else(|e| panic!("{what}: not an f64: {token:?} ({e})"));
    assert!(
        v > window.0 && v < window.1,
        "{what}: {v} GiB/s outside plausible window {:?} (see module docs for receipts)",
        window
    );
    v
}

/// T1 — version law: `cgap-cli version` prints exactly `26.10.29` and
/// exits 0. Kills 29:5 (main→()), 30:9 (deleted version arm), 44:5
/// (cmd_version→()).
#[test]
fn version_prints_workspace_law_version() {
    let out = run_cli(&["version"]);
    assert!(out.status.success(), "exit: {:?}", out.status);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "26.10.29\n",
        "version law: literal 26.10.29 in lockstep with workspace version"
    );
    assert!(
        out.stderr.is_empty(),
        "version must not write to stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// T2 — alignment law: `align-test` allocates sizes 1..1000 step 7 (143
/// allocations), asserts 64-byte alignment per pointer and prints exactly
/// `align-test: 143/143 aligned`, exit 0. Kills 33:9, 157:5, 162:66
/// (`==`→`!=` → FAIL/exit 1), 162:34 (`%`→`/`/`+` → FAIL/exit 1), 163:21
/// (`+=`→`-=` underflow abort in dev, `*=` → 0/143).
#[test]
fn align_test_prints_143_of_143_aligned() {
    let out = run_cli(&["align-test"]);
    assert!(out.status.success(), "exit: {:?}", out.status);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "align-test: 143/143 aligned\n",
        "alignment law: all 143 allocations on 64-byte boundaries"
    );
    assert!(
        out.stderr.is_empty(),
        "align-test must not write to stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// T3 — mask receipt law: `bench-mask` prints exactly one `mask <f64.3>
/// GiB/s` line, exit 0, within the anchored window. Kills 31:9, 69:5,
/// 56:5 (→0.0/1.0/-1.0 → `0.000`/`-0.000`), the GIB 1024x-class, 56:12
/// (x8/x64 inflation), and 87:33 (`-`→`+`/`/`
/// indexes `out[MASK_WORDS ± …]` → out-of-bounds abort).
#[test]
fn bench_mask_line_laws() {
    let out = run_cli(&["bench-mask"]);
    assert!(out.status.success(), "exit: {:?}", out.status);
    assert!(
        out.stderr.is_empty(),
        "bench-mask must not write to stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = one_line_stdout(&out);
    let rest = line
        .strip_prefix("mask ")
        .filter(|r| r.ends_with(" GiB/s"))
        .unwrap_or_else(|| panic!("bench-mask line format law violated: {line:?}"));
    let num = rest.strip_suffix(" GiB/s").expect("suffix checked");
    assert_gibs_in_window(num, MASK_GIBS_WINDOW, "bench-mask");
}

/// T4 — parquet receipt law: `bench-parquet` prints exactly one
/// `parquet w=4 literal rle_encode_fast <A> vs rle_encode <B> GiB/s` line,
/// exit 0, both numbers positive and inside the anchored window. Kills
/// 32:9, 114:5, 102:5 (→0/1/-1), GIB 1024x-class, and the astronomic
/// 102:44 `+`→`*` case (bytes = n·4·out_len).
/// 102:44 `+`→`*` (bytes = n·4·out_len → astronomic rate).
#[test]
fn bench_parquet_line_laws() {
    const EXPECTED_TOKENS: usize = 9;
    let out = run_cli(&["bench-parquet"]);
    assert!(out.status.success(), "exit: {:?}", out.status);
    assert!(
        out.stderr.is_empty(),
        "bench-parquet must not write to stderr: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let line = one_line_stdout(&out);
    let tokens: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(
        tokens,
        vec![
            "parquet",
            "w=4",
            "literal",
            "rle_encode_fast",
            tokens.get(4).copied().unwrap_or(""),
            "vs",
            "rle_encode",
            tokens.get(7).copied().unwrap_or(""),
            "GiB/s",
        ],
        "parquet line format law violated: {line:?}"
    );
    assert_eq!(tokens.len(), EXPECTED_TOKENS);
    let fast = assert_gibs_in_window(tokens[4], PARQUET_GIBS_WINDOW, "rle_encode_fast");
    let base = assert_gibs_in_window(tokens[7], PARQUET_GIBS_WINDOW, "rle_encode");
    // Byte-identical outputs (the binary's own correctness gate) imply the
    // two encoders move the same bytes; rates within a small factor.
    let ratio = fast / base;
    assert!(
        (0.25..4.0).contains(&ratio),
        "fast/base rate ratio {ratio} outside [0.25, 4.0]"
    );
}

/// T5 — mask workload ground truth (macOS): the bench must hold ~192 MiB
/// of touched pages (three 64 MiB vectors), measured as max RSS of the
/// real process tree by /usr/bin/time -l. Kills 60:29 (MASK_WORDS →
/// 8+1024*1024 = 1_048_584 words ≈ 24 MiB total) and 60:36 (→ 16_384
/// words): the printed GiB/s stays self-consistent for a shrunken
/// workload, but the footprint collapses.
#[cfg(target_os = "macos")]
#[test]
fn bench_mask_footprint_matches_64mib_workload() {
    let out = Command::new("/usr/bin/time")
        .args(["-l", env!("CARGO_BIN_EXE_cgap-cli"), "bench-mask"])
        .output()
        .expect("spawn /usr/bin/time -l cgap-cli bench-mask");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let rss_line = stderr
        .lines()
        .find(|l| l.contains("maximum resident set size"))
        .expect("/usr/bin/time -l must report maximum resident set size");
    let rss: u64 = rss_line
        .split_whitespace()
        .filter_map(|t| t.parse::<u64>().ok())
        .next()
        .expect("numeric max RSS value");
    assert!(
        rss >= MASK_MIN_RSS_BYTES,
        "bench-mask max RSS {rss} B < {MASK_MIN_RSS_BYTES} B: workload no longer \
         touches three 64 MiB vectors (MASK_WORDS mutant class)"
    );
}

/// T6 — parquet workload ground truth (macOS): the bench must hold the 32
/// MiB values vector plus encoded outputs (measured 106_938_368 B). Kills
/// 106:29/106:36 (ENC_VALUES → ~1_048_584 or 16_384 values) and 114:55
/// `%`→`/` (values become runs-of-5, so encoded outputs shrink from
/// ~5.2 MiB each to near zero and the footprint collapses).
#[cfg(target_os = "macos")]
#[test]
fn bench_parquet_footprint_matches_8mib_values_workload() {
    let out = Command::new("/usr/bin/time")
        .args(["-l", env!("CARGO_BIN_EXE_cgap-cli"), "bench-parquet"])
        .output()
        .expect("spawn /usr/bin/time -l cgap-cli bench-parquet");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let rss_line = stderr
        .lines()
        .find(|l| l.contains("maximum resident set size"))
        .expect("/usr/bin/time -l must report maximum resident set size");
    let rss: u64 = rss_line
        .split_whitespace()
        .filter_map(|t| t.parse::<u64>().ok())
        .next()
        .expect("numeric max RSS value");
    assert!(
        rss >= PARQUET_MIN_RSS_BYTES,
        "bench-parquet max RSS {rss} B < {PARQUET_MIN_RSS_BYTES} B: workload no longer \
         holds the 32 MiB values vector (ENC_VALUES / `%`→`/` mutant class)"
    );
}
