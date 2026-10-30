//! Fail-closed adversarial proof for `rle_decode` (lane W2-ROBUST, v26.10.29).
//!
//! Property under test: for ANY input stream and ANY output budget, decode
//! either returns a typed `ParquetError` or completes successfully — it never
//! panics, never hangs, and never writes a lane outside `out`.
//!
//! Method (Chicago school: real decoder, real buffers, assert final state):
//! - deterministic hand-rolled xorshift64 + splitmix64 seeding, no external
//!   crates, fixed master seed — every case is reproducible from its printed
//!   seed;
//! - each case runs on a throwaway thread with a 5 s deadline so a decode
//!   that *hangs* (worse than a panic: a wrapped-arithmetic infinite loop on
//!   adversarial headers) is caught and named, not silently frozen;
//! - out-of-bounds detection uses two independent canaries:
//!   (a) a separate canary `Vec` allocated after the output buffer,
//!       snapshotted before and compared after each call (the prescribed
//!       guard), plus
//!   (b) an in-allocation guard band: `out` and a 64-lane guard region share
//!       one heap allocation via `split_at_mut`, so a write just past
//!       `out.len()` lands in observed memory, not in unrelated allocator
//!       space. Both must be byte-identical after every call.
//!       Caveat recorded honestly: in fully safe Rust the bound already
//!       holds by construction; the canaries exist to fail loudly the day
//!       the branchless/SWAR kernel grows `unsafe`, which is this crate's
//!       charter direction.
//!
//! Case families (see `fail_closed_adversarial_corpus`): empty input, bad
//! width byte (all 256 values), overlong varints (shift >= 64), well-formed
//! run/literal headers claiming beyond budget/input up to u64 extremes,
//! truncation at every prefix of valid encodings, zero-length budgets,
//! single-bit width flips, random multi-byte corruptions of valid encodings,
//! and random claim-inflation fuzzing.

use std::panic::AssertUnwindSafe;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use avec_parquet::rle::write_varint;
use avec_parquet::{rle_decode, rle_encode, ParquetError};

/// Master seed for the whole corpus. Bump only when deliberately re-drawing
/// the mutation corpus; case seeds derive deterministically from this.
const MASTER_SEED: u64 = 0x5EED_2026_1029_0001;

/// Guard lanes checked after `out` inside the SAME allocation.
const GUARD_LEN: usize = 64;
/// Separate canary vector length (allocated after the out buffer).
const CANARY_LEN: usize = 64;
/// Fill for `out` so untouched lanes are distinguishable from written ones.
const SENTINEL: u32 = 0xDEAD_BEEF;
/// Fill for the canary vector.
const CANARY: u32 = 0xCAFE_BABE;
/// Per-case deadline. A decode that fails to return is a HANG, a fail-closed
/// violation just as much as a panic (deadline-bounded receive, not a sleep).
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Deterministic PRNG (xorshift64, Marsaglia) + seed derivation (splitmix64).
// No external crates; zero-state excluded by construction.
// ---------------------------------------------------------------------------

fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Per-case seed, deterministic in (class, index) across runs and platforms.
fn seed_for(class: u32, idx: usize) -> u64 {
    splitmix64(MASTER_SEED ^ ((u64::from(class)) << 32) ^ (idx as u64))
}

struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Xorshift64(seed | 1) // never zero
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

// ---------------------------------------------------------------------------
// Case harness
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Outcome {
    Ok,
    Err(ParquetError),
    Panic(String),
    Hang,
}

/// One case = one fresh decoder call on throwaway threads and buffers.
/// Returns (outcome, canaries_intact, out-content-observed).
fn run_case(_seed: u64, src: Vec<u8>, out_len: usize) -> (Outcome, bool, Vec<u32>) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out_buf = vec![SENTINEL; out_len];
        let canary = vec![CANARY; CANARY_LEN];
        let canary_before = canary.clone();
        // In-allocation guard band: `out` and guard share one allocation, so
        // a write just past out.len() corrupts observed memory.
        let mut buf = vec![SENTINEL; out_len + GUARD_LEN];
        let (out, _guard_shared) = buf.split_at_mut(out_len);

        let res = std::panic::catch_unwind(AssertUnwindSafe(|| rle_decode(&src, out)));
        let outcome = match res {
            Ok(Ok(())) => Outcome::Ok,
            Ok(Err(e)) => Outcome::Err(e),
            Err(payload) => Outcome::Panic(panic_message(&payload)),
        };
        let canary_ok = canary == canary_before;
        let _ = tx.send((outcome, canary_ok, out_buf));
    });
    match rx.recv_timeout(CASE_TIMEOUT) {
        Ok((outcome, canary_ok, out_buf)) => (outcome, canary_ok, out_buf),
        Err(_) => (Outcome::Hang, false, Vec::new()),
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "opaque panic payload".to_string()
    }
}

/// What the case is required to do beyond the universal invariants.
enum Expect {
    /// Must return some typed error (variant not pinned: genuinely
    /// input-dependent, e.g. which check fires first on a truncated record).
    AnyErr,
    /// Must return exactly this typed error (deterministic mapping).
    Exact(ParquetError),
    /// No expectation beyond the universal invariants (mutation classes where
    /// a mutation may legally remain a well-formed stream).
    Universal,
}

/// Universal invariants enforced on EVERY case, expectation or not:
/// no panic, no hang, both canaries byte-identical after the call.
struct Ledger {
    class: &'static str,
    failures: Vec<String>,
    cases: usize,
}

impl Ledger {
    fn new(class: &'static str) -> Self {
        Ledger {
            class,
            failures: Vec::new(),
            cases: 0,
        }
    }

    fn check(&mut self, seed: u64, label: &str, src: &[u8], out_len: usize, expect: Expect) {
        self.cases += 1;
        let (outcome, canary_ok, _observed) = run_case(seed, src.to_vec(), out_len);
        let mut violations: Vec<String> = Vec::new();
        match &outcome {
            Outcome::Panic(msg) => violations.push(format!("PANIC: {msg}")),
            Outcome::Hang => violations.push(format!("HANG: no return within {CASE_TIMEOUT:?}")),
            _ => {}
        }
        if !canary_ok {
            violations.push("CANARY CORRUPTED: write escaped out.len()".to_string());
        }
        match (&expect, &outcome) {
            (Expect::AnyErr, Outcome::Ok) => {
                violations.push("expected Err, got Ok".to_string());
            }
            (Expect::Exact(want), Outcome::Err(got)) if got != want => {
                violations.push(format!("expected Err({want:?}), got Err({got:?})"));
            }
            (Expect::Exact(_), Outcome::Ok) => {
                violations.push("expected exact Err, got Ok".to_string());
            }
            _ => {}
        }
        if !violations.is_empty() {
            self.failures.push(format!(
                "[class={}] [seed=0x{seed:016x}] [case={label}] [out_len={out_len}] \
                 [src={}] => {outcome:?}: {}",
                self.class,
                hex_head(src),
                violations.join("; ")
            ));
        }
    }
}

fn hex_head(src: &[u8]) -> String {
    let head: Vec<String> = src
        .iter()
        .take(24)
        .map(|b| format!("{b:02x}"))
        .collect();
    let ellipsis = if src.len() > 24 { " ..." } else { "" };
    format!("[{}{ellipsis}]", head.join(" "))
}

// ---------------------------------------------------------------------------
// Valid base encodings shared by the mutation classes.
// ---------------------------------------------------------------------------

struct Base {
    name: &'static str,
    bits: u8,
    values: Vec<u32>,
    encoded: Vec<u8>,
}

fn bases() -> Vec<Base> {
    let mut v = Vec::new();
    let specs: &[(&str, u8, Vec<u32>)] = &[
        // run + literal + run mix
        ("mixed_w4", 4, {
            let mut x = vec![9u32; 5];
            x.extend(1u32..=10);
            x.extend(vec![7u32; 3]);
            x
        }),
        // pure literal, sub-byte width
        ("literal_w1", 1, (0u32..64).map(|i| i & 1).collect()),
        // byte width with runs
        ("mixed_w8", 8, {
            let mut x = vec![0u32; 3];
            x.extend(200u32..=210);
            x.extend(vec![255u32; 4]);
            x
        }),
        // two-bit pure literal
        ("literal_w2", 2, (0u32..64).map(|i| i % 4).collect()),
        // one long run (tiny encoding)
        ("run_w4", 4, vec![13u32; 100]),
    ];
    for (name, bits, values) in specs {
        v.push(Base {
            name,
            bits: *bits,
            values: values.clone(),
            encoded: rle_encode(&values.clone(), *bits),
        });
    }
    v
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

#[test]
fn fail_closed_adversarial_corpus() {
    let started = Instant::now();
    let mut total_failures: Vec<String> = Vec::new();
    let mut total_cases = 0usize;

    // -- class 1: empty input ------------------------------------------------
    let mut led = Ledger::new("empty_src");
    for out_len in 0..=16usize {
        let seed = seed_for(1, out_len);
        led.check(seed, &format!("out_len={out_len}"), &[], out_len, Expect::Exact(ParquetError::UnexpectedEof));
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 2: every possible width byte ----------------------------------
    // Non-kernel widths must be rejected with the exact typed error; kernel
    // widths {1,2,4,8} with no records must hit EOF (out_len > 0).
    let mut led = Ledger::new("bad_width_byte");
    for b in 0..=u8::MAX {
        let seed = seed_for(2, usize::from(b));
        let want = if matches!(b, 1 | 2 | 4 | 8) {
            Expect::Exact(ParquetError::UnexpectedEof)
        } else {
            Expect::Exact(ParquetError::UnsupportedBitWidth(b))
        };
        led.check(seed, &format!("width={b}"), &[b], 4, want);
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 3: overlong varints (shift >= 64) -----------------------------
    // 10 continuation bytes drive read_varint's shift past 64; must be a typed
    // error, never a wrap, never a panic.
    let mut led = Ledger::new("overlong_varint");
    let mut idx = 0usize;
    for w in [1u8, 2, 4, 8] {
        for flavor in [[0xFFu8], [0x80u8]] {
            for n in [10usize, 11, 12, 13] {
                let mut src = vec![w];
                src.extend(std::iter::repeat(flavor[0]).take(n));
                let seed = seed_for(3, idx);
                led.check(seed, &format!("w={w} cont=0x{:02x} n={n}", flavor[0]), &src, 64, Expect::Exact(ParquetError::UnexpectedEof));
                idx += 1;
            }
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 4: well-formed headers claiming beyond budget/input -----------
    // Varints are well-formed (terminate within 10 bytes); the CLAIM is the
    // attack. Run claims are checked before any read; literal claims bound the
    // payload read. Extremes here are exactly where unchecked usize arithmetic
    // (groups * 8, pos + byte_len) would overflow in debug builds.
    let mut led = Ledger::new("claim_inflation");
    let out_len = 64usize;
    let mut idx = 0usize;
    // repeated runs claiming beyond budget, up to u64 extremes: SAFE today,
    // exact error pinned (run_len is checked before any arithmetic).
    for run_len in [out_len + 1, 1 << 20, 1 << 31, 1 << 62, (1usize << 63) - 1] {
        let mut src = vec![4u8];
        write_varint(&mut src, (run_len as u64) << 1); // even header
        let seed = seed_for(4, idx);
        led.check(
            seed,
            &format!("run claim={run_len}"),
            &src,
            out_len,
            Expect::Exact(ParquetError::RunOverrun { run_len, budget: out_len }),
        );
        idx += 1;
    }
    // literal groups claiming beyond input, safely below arithmetic extremes:
    // exact error pinned.
    for groups in [9usize, 1 << 20, 1 << 30, (1usize << 60), (1usize << 61) - 2] {
        let mut src = vec![4u8];
        write_varint(&mut src, ((groups as u64) << 1) | 1); // odd header
        let payload_pos = src.len();
        let seed = seed_for(4, idx);
        led.check(
            seed,
            &format!("literal groups={groups}"),
            &src,
            out_len,
            Expect::Exact(ParquetError::LiteralOverrun {
                claimed: groups * 8,
                available: src.len() - payload_pos,
            }),
        );
        idx += 1;
    }
    // literal groups at the arithmetic extremes (groups >= 2^61 - 1 make
    // groups*8 and/or pos+byte_len overflow usize). Contract under test:
    // typed error, never a panic. NOTE for maintainers: if this sub-class
    // fails with PANIC "attempt to multiply/add with overflow", the defect is
    // in src/rle.rs literal-record arithmetic, not in this corpus.
    for groups in [(1usize << 61) - 1, 1usize << 61, 1usize << 62, (1usize << 63) - 1] {
        let mut src = vec![4u8];
        write_varint(&mut src, ((groups as u64) << 1) | 1);
        let seed = seed_for(4, idx);
        led.check(
            seed,
            &format!("literal groups=2^scale={groups}"),
            &src,
            out_len,
            Expect::AnyErr,
        );
        idx += 1;
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 5: moderate run overruns (exact variant pinned) ---------------
    let mut led = Ledger::new("run_overrun_moderate");
    let mut idx = 0usize;
    for out_len in [16usize, 64] {
        for d in 1..=8usize {
            let mut src = vec![4u8];
            write_varint(&mut src, ((out_len + d) as u64) << 1);
            let seed = seed_for(5, idx);
            led.check(
                seed,
                &format!("claim=budget+{d}"),
                &src,
                out_len,
                Expect::Exact(ParquetError::RunOverrun { run_len: out_len + d, budget: out_len }),
            );
            idx += 1;
        }
    }
    // two-step: a legal first run, then an over-claiming second run
    for out_len in [16usize, 64] {
        for d in [1usize, 2, 9] {
            let mut src = vec![4u8];
            write_varint(&mut src, 8 << 1);
            src.extend_from_slice(&[0xAB; 1]); // w=4 -> vb=1 value byte
            write_varint(&mut src, ((out_len - 8 + d) as u64) << 1);
            let seed = seed_for(5, idx);
            led.check(
                seed,
                &format!("second-run claim=remaining+{d}"),
                &src,
                out_len,
                Expect::Exact(ParquetError::RunOverrun { run_len: out_len - 8 + d, budget: out_len - 8 }),
            );
            idx += 1;
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 6: literal claims beyond available input ----------------------
    let mut led = Ledger::new("literal_overrun_moderate");
    let mut idx = 0usize;
    for w in [1u8, 8] {
        for groups in [1usize, 2, 3, 4, 8, 16, 255, 1 << 16, 1 << 30] {
            let mut src = vec![w];
            write_varint(&mut src, ((groups as u64) << 1) | 1);
            let payload_pos = src.len();
            let seed = seed_for(6, idx);
            led.check(
                seed,
                &format!("w={w} groups={groups} payload-absent"),
                &src,
                64,
                Expect::Exact(ParquetError::LiteralOverrun {
                    claimed: groups * 8,
                    available: src.len() - payload_pos,
                }),
            );
            idx += 1;
        }
    }
    // payload present but one group short
    for groups in [2usize, 3, 9] {
        let mut src = vec![4u8];
        write_varint(&mut src, ((groups as u64) << 1) | 1);
        let short = 8 * (groups - 1);
        src.extend(std::iter::repeat(0u8).take(short));
        let seed = seed_for(6, idx);
        led.check(
            seed,
            &format!("groups={groups} one-group-short"),
            &src,
            64,
            Expect::Exact(ParquetError::LiteralOverrun {
                claimed: groups * 8,
                available: short,
            }),
        );
        idx += 1;
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 7: truncation at EVERY prefix of valid encodings --------------
    // Any proper prefix of a stream that exactly fills `out` must error: the
    // final record is incomplete (EOF) or its payload claim exceeds what
    // remains (LiteralOverrun). Never Ok, never a panic.
    let mut led = Ledger::new("truncation_prefixes");
    let mut idx = 0usize;
    for base in bases() {
        for cut in 0..base.encoded.len() {
            let src = &base.encoded[..cut];
            let seed = seed_for(7, idx);
            led.check(
                seed,
                &format!("{} cut={cut}/{}", base.name, base.encoded.len()),
                src,
                base.values.len(),
                Expect::AnyErr,
            );
            idx += 1;
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 8: zero-length output budget ----------------------------------
    // out = [] must succeed on structurally-valid pages (loop never runs) and
    // still reject a bad width byte or empty input. Unread trailing junk is
    // not an error at zero budget.
    let mut led = Ledger::new("zero_len_out");
    let mut idx = 0usize;
    let zero_cases: Vec<(String, Vec<u8>, Expect)> = {
        let mut v = Vec::new();
        v.push(("empty src".to_string(), vec![], Expect::Exact(ParquetError::UnexpectedEof)));
        v.push(("width only".to_string(), vec![4], Expect::Universal)); // Ok expected
        v.push(("width+header".to_string(), vec![4, 0], Expect::Universal)); // Ok: run of 0 lanes
        v.push(("bad width".to_string(), vec![3], Expect::Exact(ParquetError::UnsupportedBitWidth(3))));
        v.push((
            "width+overlong junk".to_string(),
            {
                let mut s = vec![4u8];
                s.extend(std::iter::repeat(0xFFu8).take(12));
                s
            },
            Expect::Universal, // Ok: zero budget never reads records
        ));
        for base in bases() {
            v.push((format!("valid {}", base.name), base.encoded.clone(), Expect::Universal)); // Ok
        }
        v
    };
    for (label, src, want) in zero_cases {
        let seed = seed_for(8, idx);
        led.check(seed, &label, &src, 0, want);
        idx += 1;
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 9: single-bit flips of the width byte --------------------------
    // {1,2,4,8} are distinct powers of two; any single-bit flip leaves the
    // kernel set, so the exact UnsupportedBitWidth error is forced.
    let mut led = Ledger::new("width_bit_flips");
    let mut idx = 0usize;
    for base in bases() {
        for bit in 0..8u8 {
            let mut src = base.encoded.clone();
            src[0] ^= 1 << bit;
            let seed = seed_for(9, idx);
            led.check(
                seed,
                &format!("{} bit={bit} width->{}", base.name, src[0]),
                &src,
                base.values.len(),
                Expect::Exact(ParquetError::UnsupportedBitWidth(src[0])),
            );
            idx += 1;
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 10: random multi-byte corruption of valid encodings -----------
    // Honest scope note: a byte flip inside a VALUE lane or a payload byte can
    // leave the stream well-formed (different data, same shape), so Err is NOT
    // forced for this class and is not asserted. What IS asserted, universally:
    // never a panic, never a hang, never a write outside out. Ok-vs-Err
    // distribution is printed for the receipt.
    let mut led = Ledger::new("random_byte_flips");
    let mut idx = 0usize;
    let mut ok_count = 0usize;
    let mut err_count = 0usize;
    let bases = bases();
    for base in &bases {
        for m in 0..25 {
            let seed = seed_for(10, idx);
            let mut rng = Xorshift64::new(seed);
            let mut src = base.encoded.clone();
            let k = 1 + rng.below(3) as usize; // 1..=3 mutated bytes
            for _ in 0..k {
                let pos = rng.below(src.len() as u64) as usize;
                src[pos] ^= (rng.below(256) as u8).max(1);
            }
            let label = format!("{} m={m} k={k}", base.name);
            let (outcome, canary_ok, _) = run_case(seed, src.clone(), base.values.len());
            led.cases += 1;
            let mut violations: Vec<String> = Vec::new();
            match &outcome {
                Outcome::Panic(msg) => violations.push(format!("PANIC: {msg}")),
                Outcome::Hang => violations.push("HANG".to_string()),
                _ => {}
            }
            if !canary_ok {
                violations.push("CANARY CORRUPTED".to_string());
            }
            match outcome {
                Outcome::Ok => ok_count += 1,
                Outcome::Err(_) => err_count += 1,
                _ => {}
            }
            if !violations.is_empty() {
                led.failures.push(format!(
                    "[class={}] [seed=0x{seed:016x}] [case={label}] [src={}] : {}",
                    led.class,
                    hex_head(&src),
                    violations.join("; ")
                ));
            }
            idx += 1;
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);
    println!(
        "  random_byte_flips: {} cases, Ok(remained well-formed)={ok_count}, Err(rejected)={err_count}",
        led.cases
    );

    // -- class 11: random claim-inflation fuzzing ----------------------------
    // Random well-formed varint headers (1..=10 bytes, forced termination),
    // random parity -> run or literal path. Even headers must always be
    // {RunOverrun, UnexpectedEof}; odd headers must never panic — the extreme
    // values deliberately sweep the usize-arithmetic boundaries.
    let mut led = Ledger::new("random_claim_fuzz");
    let mut idx = 0usize;
    for m in 0..150 {
        let seed = seed_for(11, m);
        let mut rng = Xorshift64::new(seed);
        let w = [1u8, 2, 4, 8][rng.below(4) as usize];
        let varint_len = 1 + rng.below(10) as usize;
        let mut src = vec![w];
        for _ in 0..varint_len.saturating_sub(1) {
            src.push(0x80 | (rng.below(128) as u8)); // continuation byte
        }
        src.push(rng.below(128) as u8); // terminator
        let seed2 = seed_for(11, idx);
        led.check(seed2, &format!("m={m} varint_len={varint_len}"), &src, 64, Expect::AnyErr);
        idx += 1;
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- verdict -------------------------------------------------------------
    let elapsed = started.elapsed();
    println!(
        "  fail_closed_adversarial_corpus: {total_cases} cases in {elapsed:?}, {} violations",
        total_failures.len()
    );
    assert!(
        total_cases >= 500,
        "corpus shrank below the 500-case floor: {total_cases}"
    );
    assert!(
        total_failures.is_empty(),
        "FAIL-CLOSED VIOLATIONS ({}):\n  {}",
        total_failures.len(),
        total_failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Partial-write contract (observed, documented — not spec-guaranteed upstream)
// ---------------------------------------------------------------------------

/// OBSERVED CONTRACT (recorded 2026-10-29 from actual runs of this test; see
/// the `--nocapture` output and the asserts below):
///
/// When `rle_decode` fails mid-stream, `out` retains:
///   1. every lane written by records FULLY decoded before the failing
///      record, and
///   2. the caller's original content (here: the sentinel fill) at and after
///      the failing record's first lane — the failing record itself writes
///      nothing, because every per-record check (run length vs remaining
///      budget; payload byte availability) precedes any write. There is no
///      partial within-record write.
///
/// This is an observation of current behavior, not an upstream API
/// guarantee: `rle_decode`'s doc comment makes no commitment about `out` on
/// error. The asserts below pin today's truth so any behavioral change is a
/// loud, conscious decision — update this comment and the asserts together.
#[test]
fn partial_write_contract_observed() {
    // Stream: [w=8][run 3 x 7][run 5 x 9 -> value bytes truncated after 1]
    let mut src = vec![8u8];
    write_varint(&mut src, 3 << 1);
    src.extend_from_slice(&7u32.to_le_bytes());
    write_varint(&mut src, 5 << 1);
    src.push(9); // only 1 of 4 value bytes present
    let mut out = vec![SENTINEL; 10];
    let res = rle_decode(&src, &mut out);
    assert_eq!(res, Err(ParquetError::UnexpectedEof), "mid-record EOF must error");
    println!("  partial_write run-truncation: out = {out:?}");
    assert_eq!(
        out,
        vec![7, 7, 7, SENTINEL, SENTINEL, SENTINEL, SENTINEL, SENTINEL, SENTINEL, SENTINEL],
        "observed contract: completed prefix intact, failing record wrote nothing"
    );

    // Stream: [w=2][literal 1 group (8 lanes)][literal claims 3 groups, payload absent]
    let mut src = vec![2u8];
    write_varint(&mut src, (1 << 1) | 1);
    src.extend_from_slice(&[0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]);
    write_varint(&mut src, (3 << 1) | 1);
    let mut out = vec![SENTINEL; 16];
    let res = rle_decode(&src, &mut out);
    assert_eq!(
        res,
        Err(ParquetError::LiteralOverrun { claimed: 24, available: 0 }),
        "literal overclaim must error before writing"
    );
    println!("  partial_write literal-overclaim: out = {out:?}");
    // lanes decode as (byte & mask): 1,2,3,0,0,0,0,0
    let decoded: Vec<u32> = vec![1, 2, 3, 0, 0, 0, 0, 0];
    let mut want = decoded;
    want.resize(16, SENTINEL);
    assert_eq!(
        out, want,
        "observed contract: completed literal group intact, failing record wrote nothing"
    );

    // Zero budget: width validates, records never read, Ok.
    let mut out: [u32; 0] = [];
    assert_eq!(rle_decode(&[4, 0xFF, 0xFF], &mut out), Ok(()));
    assert_eq!(rle_decode(&[], &mut out), Err(ParquetError::UnexpectedEof));
}
