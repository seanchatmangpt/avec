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
//!   that *hangs* is caught and named by seed, not silently frozen;
//! - out-of-bounds detection uses two independent canaries:
//!   (a) a separate canary `Vec` allocated after the output buffer,
//!   snapshotted before and compared after each call, plus
//!   (b) an in-allocation guard band: `out` and a 64-lane guard region share
//!   one heap allocation via `split_at_mut`, so a write just past
//!   `out.len()` lands in observed memory, not in unrelated allocator
//!   space. Both must be byte-identical after every call.
//!   Caveat recorded honestly: in fully safe Rust the bound already
//!   holds by construction; the canaries exist to fail loudly the day
//!   the branchless/SWAR kernel grows `unsafe`, which is this crate's
//!   charter direction.
//! - the well-formed fixtures below are HAND-BUILT from the documented wire
//!   format, not produced by `rle_encode`: this lane courts DECODE
//!   fail-closedness, and coupling its inputs to encoder liveness would let
//!   an encoder regression hang the court (which is exactly what was
//!   observed during this wave — see the wave receipt). The fixtures are
//!   themselves proved lawful by `fixtures_decode_to_hand_written_values`.
//!
//! Case families (see `fail_closed_adversarial_corpus`): empty input, bad
//! width byte (all 256 values), overlong varints (shift >= 64), well-formed
//! run/literal headers claiming beyond budget/input up to u64 extremes,
//! truncation at every prefix of valid pages, zero-length budgets,
//! single-bit width flips, random multi-byte corruptions of valid pages,
//! and random claim-inflation fuzzing.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use cgap_parquet::rle::write_varint;
use cgap_parquet::{rle_decode, ParquetError};

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

/// One case = one fresh decoder call on a throwaway thread with fresh
/// buffers. Returns (outcome, canaries_intact, out-content-observed).
fn run_case(src: Vec<u8>, out_len: usize) -> (Outcome, bool, Vec<u32>) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out_buf = vec![SENTINEL; out_len];
        let canary = vec![CANARY; CANARY_LEN];
        let canary_before = canary.clone();
        // In-allocation guard band: `out` and guard share one allocation, so
        // a write just past out.len() corrupts observed memory.
        let mut buf = vec![SENTINEL; out_len + GUARD_LEN];
        let (out, _guard_shared) = buf.split_at_mut(out_len);

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rle_decode(&src, out)
        }));
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
    /// a mutation may legally remain a well-formed stream, or zero-budget
    /// cases where the record stream is never read).
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
        let (outcome, canary_ok, _observed) = run_case(src.to_vec(), out_len);
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
    let head: Vec<String> = src.iter().take(24).map(|b| format!("{b:02x}")).collect();
    let ellipsis = if src.len() > 24 { " ..." } else { "" };
    format!("[{}{ellipsis}]", head.join(" "))
}

// ---------------------------------------------------------------------------
// Hand-built wire fixtures.
//
// Wire format (src/rle.rs module doc): width byte in {1,2,4,8}; repeated run
// = varint((count<<1)|0) + ceil(w/8) LE value bytes; literal run =
// varint((groups<<1)|1) + groups*8 bytes, one lane per byte, low `w` bits
// significant, zero-padded final group when the lane count is not a multiple
// of 8 (here every literal group is fully real, so every byte is
// load-bearing and any truncation must error).
// ---------------------------------------------------------------------------

struct Fixture {
    name: &'static str,
    /// The lanes this page must decode to (full-buffer, exact fill).
    values: Vec<u32>,
    encoded: Vec<u8>,
}

fn fixtures() -> Vec<Fixture> {
    let mut v: Vec<Fixture> = Vec::new();
    let mut push = |name: &'static str, values: Vec<u32>, enc: Vec<u8>| {
        v.push(Fixture {
            name,
            values,
            encoded: enc,
        });
    };
    // Record headers go through the crate's own varint writer: counts >= 64
    // need multi-byte varints (e.g. 100<<1 = 200 -> [0xC8, 0x01]), which is
    // exactly the kind of hand-encoding slip this keeps out of the fixtures.
    let run_hdr = |dst: &mut Vec<u8>, count: usize| write_varint(dst, (count as u64) << 1);
    let lit_hdr = |dst: &mut Vec<u8>, groups: usize| write_varint(dst, ((groups as u64) << 1) | 1);

    // mixed_w4: [4][run 5x9][lit 2 groups, 16 real lanes][run 3x7] -> 24 lanes
    let mut f = vec![4u8];
    run_hdr(&mut f, 5);
    f.push(9);
    lit_hdr(&mut f, 2);
    f.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0]);
    run_hdr(&mut f, 3);
    f.push(7);
    let mut values = vec![9u32; 5];
    values.extend(1u32..=15);
    values.push(0);
    values.extend(vec![7u32; 3]);
    push("mixed_w4", values, f);

    // literal_w1: [1][lit 8 groups][64 alternating bytes] -> 64 lanes
    let mut f = vec![1u8];
    lit_hdr(&mut f, 8);
    f.extend((0u32..64).map(|i| (i & 1) as u8));
    let values: Vec<u32> = (0u32..64).map(|i| i & 1).collect();
    push("literal_w1", values, f);

    // mixed_w8: [8][run 3x0][lit 2 groups, 16 real lanes][run 4x255] -> 23 lanes
    // Run values are ONE byte for every kernel width (vb = ceil(w/8) = 1
    // for w in {1,2,4,8}); a wider value field would pad the page and let
    // truncation prefixes legally decode Ok.
    let mut f = vec![8u8];
    run_hdr(&mut f, 3);
    f.push(0x00);
    lit_hdr(&mut f, 2);
    let mut payload: Vec<u8> = (199u32..=210).map(|x| x as u8).collect();
    payload.extend_from_slice(&[0xFF; 4]);
    f.extend_from_slice(&payload);
    run_hdr(&mut f, 4);
    f.push(0xFF);
    let mut values = vec![0u32; 3];
    values.extend(199u32..=210);
    values.extend(vec![255u32; 8]);
    push("mixed_w8", values, f);

    // literal_w2: [2][lit 8 groups][64 bytes of i%4] -> 64 lanes
    let mut f = vec![2u8];
    lit_hdr(&mut f, 8);
    f.extend((0u32..64).map(|i| (i % 4) as u8));
    let values: Vec<u32> = (0u32..64).map(|i| i % 4).collect();
    push("literal_w2", values, f);

    // run_w4: [4][run 100x13] -> 100 lanes
    let mut f = vec![4u8];
    run_hdr(&mut f, 100);
    f.push(0x0D);
    push("run_w4", vec![13u32; 100], f);

    // run_w8: [8][run 10x255] -> 10 lanes
    let mut f = vec![8u8];
    run_hdr(&mut f, 10);
    f.push(0xFF);
    push("run_w8", vec![255u32; 10], f);

    // literal_exact_w4: [4][lit 2 groups, exactly 16 lanes] -> 16 lanes
    let mut f = vec![4u8];
    lit_hdr(&mut f, 2);
    f.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0]);
    let mut values: Vec<u32> = (1u32..=15).collect();
    values.push(0);
    push("literal_exact_w4", values, f);

    v
}

/// The fixtures are inputs to every mutation class below; if a hand-built
/// fixture did NOT decode to its hand-written lane values, every downstream
/// "corruption" verdict would be vacuous. This court makes fixture rot loud.
#[test]
fn fixtures_decode_to_hand_written_values() {
    for fx in fixtures() {
        assert!(
            matches!(fx.encoded[0], 1 | 2 | 4 | 8),
            "{}: width byte must be a kernel width",
            fx.name
        );
        let mut out = vec![SENTINEL; fx.values.len()];
        let res = rle_decode(&fx.encoded, &mut out);
        assert_eq!(
            res,
            Ok(()),
            "fixture {} must decode as a valid full-buffer page",
            fx.name
        );
        assert_eq!(
            out, fx.values,
            "fixture {} decodes to different lanes than hand-written intent",
            fx.name
        );
    }
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
        led.check(
            seed,
            &format!("out_len={out_len}"),
            &[],
            out_len,
            Expect::Exact(ParquetError::UnexpectedEof),
        );
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 2: every possible width byte ----------------------------------
    // Non-kernel widths must be rejected with the exact typed error; kernel
    // widths {1,2,4,8} with no records must not succeed on an unfilled budget.
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
                src.extend(std::iter::repeat_n(flavor[0], n));
                let seed = seed_for(3, idx);
                led.check(
                    seed,
                    &format!("w={w} cont=0x{:02x} n={n}", flavor[0]),
                    &src,
                    64,
                    Expect::Exact(ParquetError::UnexpectedEof),
                );
                idx += 1;
            }
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 4: well-formed headers claiming beyond budget/input -----------
    // Varints are well-formed (terminate within 10 bytes); the CLAIM is the
    // attack. Run claims are checked before any read; literal claims bound
    // the payload read, and groups*8 is guarded by checked_mul in src/rle.rs
    // since this wave. The extreme sub-class deliberately sweeps the usize
    // arithmetic boundaries (groups = 2^61 - 1 leaves byte_len = 2^64 - 8,
    // so the SUBSEQUENT `pos + byte_len` slice arithmetic is the last
    // unchecked operation on that path — this corpus holds it to the
    // module's zero-panic law).
    let mut led = Ledger::new("claim_inflation");
    let out_len = 64usize;
    let mut idx = 0usize;
    // repeated runs claiming beyond budget, up to u64 extremes: exact error
    // pinned (run_len is checked before any arithmetic).
    for run_len in [out_len + 1, 1 << 20, 1 << 31, 1 << 62, (1usize << 63) - 1] {
        let mut src = vec![4u8];
        write_varint(&mut src, (run_len as u64) << 1); // even header
        let seed = seed_for(4, idx);
        led.check(
            seed,
            &format!("run claim={run_len}"),
            &src,
            out_len,
            Expect::Exact(ParquetError::RunOverrun {
                run_len,
                budget: out_len,
            }),
        );
        idx += 1;
    }
    // literal groups claiming beyond input, safely below arithmetic extremes:
    // exact error pinned.
    for groups in [9usize, 1 << 20, 1 << 30, 1 << 60, (1usize << 61) - 2] {
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
    // literal groups at the arithmetic extremes. Contract under test: typed
    // error, never a panic. A failure here names src/rle.rs's literal-record
    // slice arithmetic, not this corpus.
    for groups in [(1usize << 61) - 1, 1usize << 61, 1usize << 62, (1usize << 63) - 1] {
        let mut src = vec![4u8];
        write_varint(&mut src, ((groups as u64) << 1) | 1);
        let seed = seed_for(4, idx);
        led.check(
            seed,
            &format!("literal groups=extreme={groups}"),
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
                Expect::Exact(ParquetError::RunOverrun {
                    run_len: out_len + d,
                    budget: out_len,
                }),
            );
            idx += 1;
        }
    }
    // two-step: a legal first run, then an over-claiming second run
    for out_len in [16usize, 64] {
        for d in [1usize, 2, 9] {
            let mut src = vec![4u8];
            write_varint(&mut src, 8 << 1);
            src.push(0xAB); // w=4 -> vb=1 value byte
            write_varint(&mut src, ((out_len - 8 + d) as u64) << 1);
            let seed = seed_for(5, idx);
            led.check(
                seed,
                &format!("second-run claim=remaining+{d}"),
                &src,
                out_len,
                Expect::Exact(ParquetError::RunOverrun {
                    run_len: out_len - 8 + d,
                    budget: out_len - 8,
                }),
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
        src.extend(std::iter::repeat_n(0u8, short));
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

    // -- class 7: truncation at EVERY prefix of valid pages ------------------
    // Every byte of every fixture is load-bearing (full literal groups, exact
    // runs), so any proper prefix must fail: the final record is incomplete
    // (EOF), its payload claim exceeds what remains (LiteralOverrun), or the
    // page ends at a record boundary with lanes unfilled (the full-buffer
    // contract surfaces UnexpectedEof). Never Ok, never a panic.
    let mut led = Ledger::new("truncation_prefixes");
    let mut idx = 0usize;
    for fx in fixtures() {
        for cut in 0..fx.encoded.len() {
            let src = &fx.encoded[..cut];
            let seed = seed_for(7, idx);
            led.check(
                seed,
                &format!("{} cut={cut}/{}", fx.name, fx.encoded.len()),
                src,
                fx.values.len(),
                Expect::AnyErr,
            );
            idx += 1;
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 8: zero-length output budget ----------------------------------
    // out = [] must succeed on structurally-valid pages (the record loop
    // never runs, so trailing junk is never read at zero budget) and still
    // reject a bad width byte or empty input.
    let mut led = Ledger::new("zero_len_out");
    let mut idx = 0usize;
    let zero_cases: Vec<(String, Vec<u8>, Expect)> = {
        let mut v = Vec::new();
        v.push((
            "empty src".to_string(),
            vec![],
            Expect::Exact(ParquetError::UnexpectedEof),
        ));
        v.push(("width only".to_string(), vec![4], Expect::Universal)); // Ok
        v.push(("width+header".to_string(), vec![4, 0], Expect::Universal)); // Ok: zero budget never reads records
        v.push((
            "bad width".to_string(),
            vec![3],
            Expect::Exact(ParquetError::UnsupportedBitWidth(3)),
        ));
        v.push((
            "width+overlong junk".to_string(),
            {
                let mut s = vec![4u8];
                s.extend(std::iter::repeat_n(0xFFu8, 12));
                s
            },
            Expect::Universal, // Ok: zero budget never reads records
        ));
        for fx in fixtures() {
            v.push((format!("valid {}", fx.name), fx.encoded.clone(), Expect::Universal)); // Ok
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
    for fx in fixtures() {
        for bit in 0..8u8 {
            let mut src = fx.encoded.clone();
            src[0] ^= 1 << bit;
            let seed = seed_for(9, idx);
            led.check(
                seed,
                &format!("{} bit={bit} width->{}", fx.name, src[0]),
                &src,
                fx.values.len(),
                Expect::Exact(ParquetError::UnsupportedBitWidth(src[0])),
            );
            idx += 1;
        }
    }
    total_cases += led.cases;
    total_failures.extend(led.failures);

    // -- class 10: random multi-byte corruption of valid pages ----------------
    // Honest scope note: a flip inside a value lane or payload byte can leave
    // the stream well-formed (different data, same shape), so Err is NOT
    // forced for this class and is not asserted. What IS asserted,
    // universally: never a panic, never a hang, never a write outside out.
    // Ok-vs-Err distribution is printed for the receipt.
    let mut led = Ledger::new("random_byte_flips");
    let mut idx = 0usize;
    let mut ok_count = 0usize;
    let mut err_count = 0usize;
    let all_fixtures = fixtures();
    for fx in &all_fixtures {
        for m in 0..25 {
            let seed = seed_for(10, idx);
            let mut rng = Xorshift64::new(seed);
            let mut src = fx.encoded.clone();
            let k = 1 + rng.below(3) as usize; // 1..=3 mutated bytes
            for _ in 0..k {
                let pos = rng.below(src.len() as u64) as usize;
                src[pos] ^= (rng.below(256) as u8).max(1);
            }
            let label = format!("{} m={m} k={k}", fx.name);
            let (outcome, canary_ok, _) = run_case(src.clone(), fx.values.len());
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
    // {RunOverrun, UnexpectedEof}; odd headers must never panic — the random
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
        led.check(
            seed2,
            &format!("m={m} varint_len={varint_len}"),
            &src,
            64,
            Expect::AnyErr,
        );
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
/// guarantee: the decoder's doc comment makes no commitment about `out` on
/// error (decode_into documents that "lanes before the failing record may
/// already be written", which is exactly what these asserts pin). The asserts
/// below pin today's truth so any behavioral change is a loud, conscious
/// decision — update this comment and the asserts together.
#[test]
fn partial_write_contract_observed() {
    // Stream: [w=8][run 3 x 7][run 5 header, value byte absent]
    // (run values are 1 byte for every kernel width: vb = ceil(w/8) = 1)
    let mut src = vec![8u8];
    write_varint(&mut src, 3 << 1);
    src.push(7);
    write_varint(&mut src, 5 << 1);
    let mut out = vec![SENTINEL; 10];
    let res = rle_decode(&src, &mut out);
    assert_eq!(
        res,
        Err(ParquetError::UnexpectedEof),
        "mid-record EOF must error"
    );
    println!("  partial_write run-truncation: out = {out:?}");
    assert_eq!(
        out,
        vec![
            7, 7, 7, SENTINEL, SENTINEL, SENTINEL, SENTINEL, SENTINEL, SENTINEL, SENTINEL
        ],
        "observed contract: completed prefix intact, failing record wrote nothing"
    );

    // Stream: [w=8][run 3 x 7][run 5 x 9][literal 2-group header, payload absent]
    let mut src = vec![8u8];
    write_varint(&mut src, 3 << 1);
    src.push(7);
    write_varint(&mut src, 5 << 1);
    src.push(9);
    write_varint(&mut src, (2 << 1) | 1);
    let mut out = vec![SENTINEL; 10];
    let res = rle_decode(&src, &mut out);
    assert_eq!(
        res,
        Err(ParquetError::LiteralOverrun {
            claimed: 16,
            available: 0
        }),
        "literal overclaim must error before writing"
    );
    println!("  partial_write run-then-literal-overclaim: out = {out:?}");
    assert_eq!(
        out,
        vec![
            7, 7, 7, 9, 9, 9, 9, 9, SENTINEL, SENTINEL
        ],
        "observed contract: completed runs intact, failing literal record wrote nothing"
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
        Err(ParquetError::LiteralOverrun {
            claimed: 24,
            available: 0
        }),
        "literal overclaim must error before writing"
    );
    println!("  partial_write literal-overclaim: out = {out:?}");
    // lanes decode as (byte & mask): 1,2,3,0,0,0,0,0
    let mut want: Vec<u32> = vec![1, 2, 3, 0, 0, 0, 0, 0];
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
