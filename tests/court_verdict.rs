//! A20 court verdict harness — Wave 3: full lane scope.
//!
//! Spec: `docs/jira/v26.11.1/lanes/A20-court-verdict.md` §3 (consume the
//! sealed sibling surfaces), §5 (corpus constants + generator), §5.2
//! (power-cut interleave), §5.3 (allocation budgets), §6.1/§6.2
//! (mutation-loop ledger record), §6.3 (shim-court non-vacuity), §10 (loud
//! [SKIP] class). Wave 1 froze the record/constant ABI (§-cited constants,
//! `VerdictClass`, `MutantId`, `KillLedgerRow`, compile-time layout
//! asserts, runtime receipt court). Wave 2 landed the core engines on that
//! frozen ABI (generator, BN3 closure, alloc budgets, interleave plans,
//! sha256 KATs). Wave 3 landed the full-scope verdict courts over the
//! sealed sibling surfaces — real collaborators, consumed
//! through their public APIs only, per §3's C4 context:
//!
//! 1. §5.1 deterministic 1M-row SplitMix64 settlement generator with a
//!    pinned corpus sha256 receipt asserted before any court's assertions
//!    run (any court touching the corpus goes through `corpus()`).
//! 2. §5.1 BN3 closure battery: degenerate/edge rows are admitted (they
//!    classify, never crash) and correctly classified, asserted via
//!    structural invariants tied to row content (not by re-deriving the
//!    generation strides, which would be tautological — §2.2).
//! 3. §5.3 allocation budgets over the real shared `cgap_audit`
//!    `CountingAlloc` primitive (crates/audit/src/alloc_counter.rs): the
//!    admission pass is bounded by `ROWS / 1024` allocations; the per-row
//!    verify loop is zero-alloc.
//! 4. §5.2 power-cut interleave engine over the real frozen A05 ABI
//!    (`cgap_storage::atomic_commit`): two interleaved transactions,
//!    per-slot byte plans, stride-limited truncation-offset enumeration
//!    (the stride is a named constant — recorded, never hidden). The
//!    tear-classification step CONSUMES the real A05 `recover` classifier
//!    per offset (A05 landed the surface mid-Wave-3; the earlier
//!    loud-[SKIP] row is retired — see the Wave-3 divergence note further
//!    down and `a05_interleave_engine`).
//!
//! Wave-3 component map (the dispatch slice's "courts, WAL, unifier, dam,
//! sealers"): every named component is CONSUMED here or at the gate, never
//! re-implemented —
//! - **courts**: the 1M-row stress suite below (§5) + the Wave-3
//!   consumption courts over the sealed siblings;
//! - **WAL** (A05): the §5.2 interleave engine consumes the REAL frozen
//!   A05 engine (`cgap_storage::atomic_commit`): `FlashMedium`,
//!   `write_record_cut`, and the public `recover` classifier — two
//!   interleaved transactions, per-slot stride-limited truncation sweep,
//!   per-offset tear classification, root-resolution and row-re-verify
//!   invariants (A05 landed this surface mid-session; the earlier
//!   loud-[SKIP] row is retired, superseded by real consumption);
//! - **unifier** (A12): `a12_unifier_join_court` +
//!   `a12_unifier_fixpoint_court` drive the real
//!   `cgap_bn3::unify` kernel (register ABI, RuleCompiler, nested-loop
//!   join, fixpoint driver) over corpus-derived and hand-pinned stores;
//! - **dam** (A14): `a14_dam_ingress_court` drives the real
//!   `cgap_gate::penstock::ingress` refusal boundary over corpus-derived
//!   valid/adversarial frames (taxonomy rows 1-6, order pinning, content
//!   independence, 1M-frame zero-alloc sweep);
//! - **sealers**: `scripts/boundary_gate.py` §6.4 enforcement wiring
//!   (--deps / --strings) lands in the same wave — the campaign-seal
//!   gates this suite feeds.
//!
//! Zero external dependencies. The only workspace dev-deps are path deps
//! on zero-external crates, each disclosed in `crates/cgap-court/Cargo.toml`:
//! `cgap-audit` (the shared alloc primitive §5.3 directs this lane to
//! consume; linking it donates its `CountingAlloc` `#[global_allocator]`
//! to this test binary — same crate-as-dev-dep pattern as `cgap-ffi`,
//! lane-18 disclosed unblock), `cgap-storage` (the frozen A05 ABI the
//! §5.2 engine builds against), and the Wave-3 consumption set
//! `cgap-core`/`cgap-blake3`/`cgap-bn3`/`cgap-gate` (A04 canonical frame
//! ABI + real BLAKE3 kernel, A12 unifier, A14 penstock).
//!
//! Authoritative anchors are cited, not redefined here: `CgapFrameHeader`
//! 128 B (A04, `crates/cgap-core/src/frame.rs`), `P28LicenseAttestation`
//! 80 B (A17, `crates/cgap-p28/src/attestation.rs`), BN3 layout (A11),
//! SlotId truncation (A06), `CgapStatus` + four sovereign entrypoints
//! (A19), WAL framing (A05: `crates/cgap-storage/src/atomic_commit.rs`).
//!
//! Anti-vacuity, per court (each names the mutation that flips it):
//! - static asserts + `wave1_abi_freeze_receipt`: any frozen-field widen,
//!   narrow, or reorder fails compile (Wave-1 shim witness: `file` widened
//!   one byte -> build-fail; restore sha256 byte-exact).
//! - `corpus_determinism_receipt`: any generator arithmetic change (the
//!   gate's committed SHIM mutant bumps a SplitMix64 mix constant) flips
//!   the pinned hash and fails this court before other courts assert.
//! - `bn3_closure_battery`: a priority-order swap or degenerate-stride
//!   change moves degenerate content onto a row tagged `Normal` (or breaks
//!   a tag's content match) and trips the content<->tag invariant.
//! - `alloc_admission_budget`: an allocation that ESCAPES elision inside
//!   the admission pass (e.g. a per-row `Vec::with_capacity` kept alive
//!   through `std::hint::black_box`) blows the pinned 976 budget. (Wave-4
//!   correction per the wave-3 audit's M4a/M4b witnesses: a dead per-row
//!   `with_capacity` whose malloc LLVM eliminates does NOT flip, and a
//!   literal per-row `Vec::push` amortizes to ~20 allocations — under
//!   budget, also not a flip. The doc now states the law that actually
//!   holds.)
//! - `alloc_verify_zero`: any allocation in the verify loop flips
//!   `AllocReport::is_zero()`.
//! - `a05_interleave_engine`: a slot-length or stride constant tweak trips
//!   the enumeration invariants (count / monotonicity / endpoints); a
//!   validator-chain change (seq order, tear mapping, SeqRegression) is
//!   pinned by the strict-vs-mutation-mode differential and the
//!   per-offset class verdicts over the real `recover`.
//! - `cross_lane_abi_pin`: any frozen-field widen/narrow/reorder on the
//!   A04 canonical frame, the A17 attestation, the A19 status register
//!   values, or the A05 tear/recovery class sets flips a runtime assert
//!   (and a layout drift flips the owning crates' own compile asserts).
//! - `a12_unifier_join_court`: a join-direction swap, register-band width
//!   change, or match-order change flips the hand-pinned exact match set
//!   (count 5, pinned per-match rows + bound values), the chunk-size
//!   independence identity, or the corpus-store oracle count.
//! - `a12_unifier_fixpoint_court`: a round-budget or delta-append change
//!   flips the arithmetic closure counts (496 is the hand-derived sum
//!   31+59+106+164+136, not a kernel output; the earlier "528" here was
//!   stale prose — corrected in Wave 4).
//! - `a14_dam_ingress_court`: removal or reorder of any penstock check
//!   flips the exact per-row expected status (a check removed admits what
//!   the taxonomy expects rejected; a reorder flips the order-pinning
//!   frames).
//! - An unpinned `CORPUS_SHA256_HEX` FAILS the determinism court
//!   (fail-closed), never a passing stub.
//!
//! Wave-2 determinism receipt is pinned under the gate's fixed build
//! profile (`CARGO_PROFILE_DEV_OPT_LEVEL=2`); the generator uses explicit
//! wrapping arithmetic throughout, so the digest is identical under any
//! opt-level.

#![allow(dead_code)] // Wave-4 surface (ledger writers) lives beside the engines.

use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Frozen corpus constants (A20 §5.1, §5.3)
// ---------------------------------------------------------------------------

/// ABI version of the verdict/ledger record set. Bump on any layout change.
pub const VERDICT_ABI_VERSION: u32 = 1;

/// §5.1: generator seed, pinned constant, no entropy/clock/filesystem order.
pub const CORPUS_SEED: u64 = 0xA20C_0FFE_E5A7_E000;

/// §5.1: exactly 1,000,000 rows, deterministic single pass.
pub const CORPUS_ROWS: u64 = 1_000_000;

/// §5.3: admission-path allocation budget, `report.count <= ROWS / 1024`.
pub const ALLOC_BUDGET_DIVISOR: u64 = 1024;

/// Derived §5.3 budget ceiling over the pinned row count (1_000_000 / 1024).
pub const ALLOC_BUDGET_MAX_ALLOCS: u64 = CORPUS_ROWS / ALLOC_BUDGET_DIVISOR;

/// Pinned §5.1 corpus receipt: sha256 over the canonical encoding of the
/// stored 1M-row corpus (little-endian fields, row order 0..CORPUS_ROWS).
/// An unpinned or drifted value FAILS the determinism court (fail-closed) —
/// the corpus itself is a receipted subject (§5.1).
pub const CORPUS_SHA256_HEX: &str =
    "03ed29d285cec5bb381ebdfa045f0aa0a81c0aa6282b6777a20164f784f7facb";

/// §6.2: gate passes iff `kill_rate >= 0.98`, frozen as 980 per mille.
pub const KILL_RATE_MIN_PERMILLE: u64 = 980;

/// sha256 digest width in bytes (§6.1 steps 1/3/6 — baseline, mutant,
/// restore manifests are all sha256 fail-closed).
pub const SHA256_BYTES: usize = 32;

/// sha256 lowercase-hex width (§5.1 determinism receipt is pinned hex).
pub const SHA256_HEX_LEN: usize = 64;

/// Fixed-width field caps for the ledger record (incl. one NUL). Frozen at
/// Wave 1 so the record stays pointer-free; widening is a new ABI version.
pub const FILE_PATH_MAX: usize = 128;
pub const OPERATOR_MAX: usize = 32;
pub const COURT_NAME_MAX: usize = 64;

/// §7 merge matrix consumes 19 lane surfaces plus baseline/shim rows; the
/// passing-court list must hold every court a survivor can report.
pub const MAX_COURTS_PER_RUN: usize = 32;

// ---------------------------------------------------------------------------
// Frozen record ABI (A20 §6.1 steps 3/5/7, §6.2, §10)
// ---------------------------------------------------------------------------

/// Verdict class of one mutation-campaign row (§6.1 step 5, §6.2, §10).
/// Wire discriminants are part of the frozen ABI (ledger JSON emits them).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VerdictClass {
    /// §6.1 step 2 — unmutated baseline row.
    Baseline = 0,
    /// §6.3 — non-vacuity row; the first row of every campaign.
    ShimCourt = 1,
    /// §6.1 step 4 — build failure under the mutant; killed, own class.
    KilledByBuild = 2,
    /// §6.1 step 5 — court failure under the mutant; killed.
    KilledByTest = 3,
    /// §6.2 — triaged survivor, provably observation-identical; excluded
    /// from the kill-rate denominator with a one-line justification.
    Equivalent = 4,
    /// §6.2 — triaged survivor, real assertion gap; a defect row against
    /// the owning lane.
    SurvivorDefect = 5,
    /// §10 — surface absent at seal time; recorded `[SKIP] <reason>`,
    /// never a silent skip, never faked as a pass.
    SkipLoud = 6,
}

/// §6.1 step 3 — mutant identity `(file, line, operator, index)`.
/// Pointer-free fixed-width record; `file`/`operator` are NUL-padded.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MutantId {
    /// offset 0, 128 B — source path, NUL-padded, `FILE_PATH_MAX` cap.
    pub file: [u8; FILE_PATH_MAX],
    /// offset 128, 4 B — 1-based source line of the mutation site.
    pub line: u32,
    /// offset 132, 32 B — operator tag (`+1/-1`, `lt->le`, `bool-neg`,
    /// `ret-remove`, `mask-corrupt`; §6.1 step 3), NUL-padded.
    pub operator: [u8; OPERATOR_MAX],
    /// offset 164, 4 B — occurrence index of the operator on the line.
    pub index: u32,
}

/// §6.1 steps 3/5/7 — one kill-ledger row. Fixed-width, pointer-free so
/// the Wave-4 gate can hash/serialize the record byte-stably.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct KillLedgerRow {
    /// offset 0, 4 B — `VERDICT_ABI_VERSION` stamped per row.
    pub abi_version: u32,
    /// offset 4, 1 B — `VerdictClass` discriminant.
    pub verdict: VerdictClass,
    /// offset 8, 4 B — entries used in `passing_courts` (0 unless
    /// `Equivalent`/`SurvivorDefect`/`SkipLoud`).
    pub passing_count: u32,
    /// offset 12, 168 B — mutant identity (§6.1 step 3).
    pub mutant: MutantId,
    /// offset 180, 64 B — first failing court name, NUL-padded
    /// (§6.1 step 5); all-NUL when the row is not a kill.
    pub killed_by: [u8; COURT_NAME_MAX],
    /// offset 244, 2048 B — survivors record their passing court list
    /// (§6.1 step 5); NUL-padded rows beyond `passing_count`.
    pub passing_courts: [[u8; COURT_NAME_MAX]; MAX_COURTS_PER_RUN],
    /// offset 2292, 32 B — post-mutation file digest (§6.1 step 3).
    pub mutated_sha256: [u8; SHA256_BYTES],
}

// ---------------------------------------------------------------------------
// Compile-time static asserts — the Wave-1 court. A layout drift anywhere
// below fails the build; this is the anti-vacuity surface named above.
// ---------------------------------------------------------------------------

const _: () = assert!(VERDICT_ABI_VERSION == 1);
const _: () = assert!(CORPUS_SEED == 0xA20C_0FFE_E5A7_E000);
const _: () = assert!(CORPUS_ROWS == 1_000_000);
const _: () = assert!(ALLOC_BUDGET_DIVISOR == 1024);
const _: () = assert!(ALLOC_BUDGET_MAX_ALLOCS == 976);
const _: () = assert!(KILL_RATE_MIN_PERMILLE == 980);
const _: () = assert!(SHA256_BYTES == 32);
const _: () = assert!(SHA256_HEX_LEN == SHA256_BYTES * 2);
const _: () = assert!(FILE_PATH_MAX == 128 && FILE_PATH_MAX.count_ones() == 1);
const _: () = assert!(OPERATOR_MAX == 32 && OPERATOR_MAX.count_ones() == 1);
const _: () = assert!(COURT_NAME_MAX == 64 && COURT_NAME_MAX.count_ones() == 1);
const _: () = assert!(MAX_COURTS_PER_RUN >= 19);

const _: () = assert!(core::mem::size_of::<VerdictClass>() == 1);
const _: () = assert!(core::mem::align_of::<VerdictClass>() == 1);

const _: () = assert!(core::mem::size_of::<MutantId>() == 168);
const _: () = assert!(core::mem::align_of::<MutantId>() == 4);
const _: () = assert!(core::mem::offset_of!(MutantId, file) == 0);
const _: () = assert!(core::mem::offset_of!(MutantId, line) == 128);
const _: () = assert!(core::mem::offset_of!(MutantId, operator) == 132);
const _: () = assert!(core::mem::offset_of!(MutantId, index) == 164);

const _: () = assert!(core::mem::size_of::<KillLedgerRow>() == 2324);
const _: () = assert!(core::mem::align_of::<KillLedgerRow>() == 4);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, abi_version) == 0);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, verdict) == 4);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, passing_count) == 8);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, mutant) == 12);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, killed_by) == 180);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, passing_courts) == 244);
const _: () = assert!(core::mem::offset_of!(KillLedgerRow, mutated_sha256) == 2292);

// ---------------------------------------------------------------------------
// Engine 1 — SHA-256 (FIPS 180-4, hand-rolled, zero deps). The §5.1 corpus
// receipt and §6.1 manifests are sha256 by spec; no external crate.
// ---------------------------------------------------------------------------

/// Standard SHA-256 round constants (fractional parts of cube roots of the
/// first 64 primes).
const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Streaming SHA-256 state.
struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Sha256 {
    fn new() -> Self {
        Sha256 {
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    /// One FIPS 180-4 compression function over a full 64-byte block.
    fn compress_block(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = self.h[0];
        let mut b = self.h[1];
        let mut c = self.h[2];
        let mut d = self.h[3];
        let mut e = self.h[4];
        let mut f = self.h[5];
        let mut g = self.h[6];
        let mut h = self.h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        self.h[0] = self.h[0].wrapping_add(a);
        self.h[1] = self.h[1].wrapping_add(b);
        self.h[2] = self.h[2].wrapping_add(c);
        self.h[3] = self.h[3].wrapping_add(d);
        self.h[4] = self.h[4].wrapping_add(e);
        self.h[5] = self.h[5].wrapping_add(f);
        self.h[6] = self.h[6].wrapping_add(g);
        self.h[7] = self.h[7].wrapping_add(h);
    }

    /// Absorb `data`. `total_len` counts message bytes fed (before padding)
    /// and is maintained ONLY here (never in `compress_block` — a block-side
    /// increment double-counts and corrupts the padded length).
    fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress_block(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let (blk, rest) = data.split_at(64);
            self.compress_block(blk.try_into().expect("split_at(64) block"));
            data = rest;
        }
        if !data.is_empty() {
            self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
            self.buf_len += data.len();
        }
    }

    /// FIPS 180-4 padding and final digest.
    fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total_len.wrapping_mul(8);
        let r = (self.total_len % 64) as usize; // bytes in the final partial block
        // Tail = 0x80 || zeros || 8-byte big-endian bit length; sized so the
        // padded message length is a multiple of 64 (zeros in 0..=63).
        let zeros = if r + 1 <= 56 { 56 - r - 1 } else { 120 - r - 1 };
        let mut tail = [0u8; 72];
        tail[0] = 0x80;
        tail[1 + zeros..1 + zeros + 8].copy_from_slice(&bit_len.to_be_bytes());
        let tail_len = 1 + zeros + 8;
        debug_assert_eq!((r + tail_len) % 64, 0);
        self.update(&tail[..tail_len]);
        debug_assert_eq!(self.buf_len, 0);

        let mut out = [0u8; 32];
        for (i, word) in self.h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    /// One-shot convenience.
    fn compute(data: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(data);
        h.finish()
    }
}

// ---------------------------------------------------------------------------
// Engine 2 — §5.1 deterministic generator (SplitMix64, zero entropy).
// ---------------------------------------------------------------------------

/// SplitMix64 (Steele, Lea, Flood 2014), seeded from the pinned
/// `CORPUS_SEED`. Explicit wrapping arithmetic: identical output at any
/// opt-level / overflow-check setting.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// FIBO-aligned settlement row (§5.1): instrument id, notional, rate,
/// settlement date, book, counterparty, plus the derived AR aging bucket and
/// the BN3 closure class tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SettlementRow {
    instrument_id: u64,
    notional: u64,
    rate_bits: u64,
    settle_day: u32,
    book: u64,
    counterparty: u64,
    aging_bucket: u8,
    bn3_class: u8,
}

/// BN3 closure class tags (§5.1: degenerate/edge rows must be *admitted and
/// correctly classified*, not crashed on).
mod bn3 {
    pub const NORMAL: u8 = 0;
    pub const ZERO_NOTIONAL: u8 = 1;
    pub const EXTREME_DATE: u8 = 2;
    pub const DUPLICATE_KEY: u8 = 3;
}

/// Extreme-date sentinel day (§5.1 "extreme dates"): far-future day number.
const EXTREME_SETTLE_DAY: u32 = u32::MAX;

/// BN3 degenerate-row strides: pairwise coprime; a row satisfying several
/// gets exactly one tag by fixed priority duplicate-key > extreme-date >
/// zero-notional. Named constants — recorded, never hidden (§5.2/§8).
const BN3_STRIDE_DUPLICATE: u64 = 70_003;
const BN3_STRIDE_EXTREME: u64 = 12_817;
const BN3_STRIDE_ZERO: u64 = 9_991;

/// Aging buckets (AR aging): five buckets derived per row from row content.
const AGING_BUCKETS: u64 = 5;

/// Canonical little-endian encoding of one row: the byte string the corpus
/// receipt hashes. Layout-independent (no repr(C) coupling); the 8 trailing
/// bytes are zero pad, hashed as-is.
const ROW_ENCODING_BYTES: usize = 54;

/// Absorb every corpus row in generation order through `f`. The single
/// deterministic pass (§5.1): both the stored corpus and the streamed
/// determinism re-check consume THIS function, so their digests compare
/// like-for-like. Duplicate-key rows copy the immediately preceding row's
/// instrument id (row 0 is never degenerate: the `i != 0` guard in
/// `generate_row` keeps every duplicate anchored to a real predecessor).
fn for_each_corpus_row(f: &mut impl FnMut(u64, SettlementRow)) {
    let mut rng = SplitMix64::new(CORPUS_SEED);
    let mut prev_key = 0u64;
    for i in 0..CORPUS_ROWS {
        let mut row = generate_row(i, &mut rng);
        if row.bn3_class == bn3::DUPLICATE_KEY {
            row.instrument_id = prev_key;
        }
        prev_key = row.instrument_id;
        f(i, row);
    }
}

/// Generate row `i` from the PRNG stream plus deterministic degenerate-row
/// overrides. Pure function of (seed, i): no entropy, no clocks, no
/// filesystem order.
fn generate_row(i: u64, rng: &mut SplitMix64) -> SettlementRow {
    let instrument_id = rng.next_u64();
    let notional = rng.next_u64();
    let rate_bits = rng.next_u64();
    let settle_day = (rng.next_u64() % 10_950) as u32; // 30 years of days
    let book = rng.next_u64();
    let counterparty = rng.next_u64();

    let mut row = SettlementRow {
        instrument_id,
        notional,
        rate_bits,
        settle_day,
        book,
        counterparty,
        aging_bucket: 0,
        bn3_class: bn3::NORMAL,
    };

    // BN3 degenerate overrides, fixed priority: duplicate-key > extreme-date
    // > zero-notional. Row 0 is never degenerate (`i != 0` guard): a
    // duplicate needs a real predecessor (0 % stride == 0 would otherwise
    // tag row 0 with no prior row). A tagged row's content must match its
    // tag (asserted contentually by `bn3_closure_battery`, never by
    // re-deriving strides).
    if i != 0 && i % BN3_STRIDE_DUPLICATE == 0 {
        row.bn3_class = bn3::DUPLICATE_KEY; // key copied by for_each_corpus_row
    } else if i != 0 && i % BN3_STRIDE_EXTREME == 0 {
        row.settle_day = EXTREME_SETTLE_DAY;
        row.bn3_class = bn3::EXTREME_DATE;
    } else if i != 0 && i % BN3_STRIDE_ZERO == 0 {
        row.notional = 0;
        row.bn3_class = bn3::ZERO_NOTIONAL;
    }

    // AR aging bucket derived from the final row content (§5.1).
    row.aging_bucket = ((row.notional ^ row.settle_day as u64) % AGING_BUCKETS) as u8;
    row
}

/// Canonical encoding of one row into `out` (fields little-endian; bytes
/// 46..54 are zero pad).
fn encode_row(row: &SettlementRow, out: &mut [u8; ROW_ENCODING_BYTES]) {
    out[0..8].copy_from_slice(&row.instrument_id.to_le_bytes());
    out[8..16].copy_from_slice(&row.notional.to_le_bytes());
    out[16..24].copy_from_slice(&row.rate_bits.to_le_bytes());
    out[24..28].copy_from_slice(&row.settle_day.to_le_bytes());
    out[28..36].copy_from_slice(&row.book.to_le_bytes());
    out[36..44].copy_from_slice(&row.counterparty.to_le_bytes());
    out[44] = row.aging_bucket;
    out[45] = row.bn3_class;
    // 46..54 remain as initialized (zero pad — see `hash_corpus`).
}

/// §5.1 corpus receipt: sha256 over the canonical encodings of the stored
/// rows in generation order.
fn hash_corpus(rows: &[SettlementRow]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let mut enc = [0u8; ROW_ENCODING_BYTES]; // pad bytes zeroed exactly once
    for row in rows {
        encode_row(row, &mut enc);
        hasher.update(&enc);
    }
    hasher.finish()
}

/// The stored corpus (built once per process; courts share it). The pinned
/// receipt and the streamed-determinism check are asserted INSIDE, so any
/// court touching the corpus fails BEFORE its own assertions if the
/// generator or the pin drifted (§5.1, §9.5).
static CORPUS: OnceLock<Vec<SettlementRow>> = OnceLock::new();

fn corpus() -> &'static [SettlementRow] {
    CORPUS.get_or_init(|| {
        let mut rows: Vec<SettlementRow> = Vec::with_capacity(CORPUS_ROWS as usize);
        for_each_corpus_row(&mut |_i, row| rows.push(row));

        let digest = hash_corpus(&rows);
        let pinned = decode_hex_32(CORPUS_SHA256_HEX);
        assert_eq!(
            digest,
            pinned,
            "§5.1 CORPUS RECEIPT MISMATCH: generated sha256 {} != pinned {} \
             (the corpus is a receipted subject; failing before any court asserts)",
            hex32(&digest),
            CORPUS_SHA256_HEX
        );

        // §5.1 determinism: a second independent streamed pass must
        // reproduce the identical digest (pure function of the seed).
        let mut streamed = Sha256::new();
        let mut enc = [0u8; ROW_ENCODING_BYTES];
        for_each_corpus_row(&mut |_i, row| {
            encode_row(&row, &mut enc);
            streamed.update(&enc);
        });
        assert_eq!(
            streamed.finish(),
            digest,
            "§5.1 determinism: streamed regeneration diverged from the stored corpus"
        );
        rows
    })
}

/// §5.1 BN3 closure engine: classify a row against its tag by CONTENT
/// (never by re-deriving the generation stride — §2.2 tautology hazard).
/// Returns true when the row's content matches its declared class.
fn row_matches_tag(row: &SettlementRow, prev_instrument_id: u64) -> bool {
    match row.bn3_class {
        bn3::NORMAL => {
            row.notional != 0
                && row.settle_day != EXTREME_SETTLE_DAY
                && row.instrument_id != prev_instrument_id
        }
        bn3::ZERO_NOTIONAL => row.notional == 0,
        bn3::EXTREME_DATE => row.settle_day == EXTREME_SETTLE_DAY,
        bn3::DUPLICATE_KEY => row.instrument_id == prev_instrument_id,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Engine 3 — §5.2 power-cut interleave over the real frozen A05 ABI.
// ---------------------------------------------------------------------------

use cgap_storage::atomic_commit as a05;

/// Per-slot journal byte plan for one interleaved transaction (§5.2).
#[derive(Clone, Copy, Debug)]
struct SlotPlan {
    /// Transaction index (0 = slot A, 1 = slot B per A20 §5.2).
    txn: usize,
    /// Rows committed by this transaction (0..m1, m1..CORPUS_ROWS).
    rows: u64,
    /// Journal byte length = rows × A05 `RECORD_SIZE` (frozen ABI).
    slot_bytes: u64,
    /// Adversarial truncation offsets k in [0, slot_bytes], stride-limited
    /// (§5.2/§8: the stride is a named constant — recorded, never hidden).
    truncation_offsets: [u64; INTERLEAVE_STRIDE_DIVISIONS + 1],
    /// Entries used in `truncation_offsets`.
    offset_count: usize,
}

/// §5.2 stride: the sweep is stride-limited so per-offset court runtime
/// stays inside the Wave-4 budget; the stride is recorded here and in the
/// gate ledger — never hidden.
const INTERLEAVE_STRIDE_DIVISIONS: usize = 32;

/// Build the truncation sweep for one slot: `DIVISIONS + 1` offsets from
/// k = 0 (C0 PreWrite) to k = N (C4 CompleteRecord), endpoints included.
fn build_slot_plan(txn: usize, rows: u64) -> SlotPlan {
    let slot_bytes = rows * (a05::RECORD_SIZE as u64);
    let mut offsets = [0u64; INTERLEAVE_STRIDE_DIVISIONS + 1];
    let mut count = 0usize;
    for (d, slot) in offsets.iter_mut().enumerate() {
        *slot = slot_bytes * d as u64 / INTERLEAVE_STRIDE_DIVISIONS as u64;
        count += 1;
    }
    SlotPlan {
        txn,
        rows,
        slot_bytes,
        truncation_offsets: offsets,
        offset_count: count,
    }
}

// ---------------------------------------------------------------------------
// Courts — Wave 2. Each names its anti-vacuity mutation (module doc).
// ---------------------------------------------------------------------------

/// Wave-1 runtime receipt of the frozen ABI (the static-assert block above
/// is the load-bearing gate; this court names each frozen value in the test
/// output so the receipt is observable even if the const block is deleted).
#[test]
fn wave1_abi_freeze_receipt() {
    // Discriminant wire values (ledger JSON emits these; §6.2/§10 classes).
    assert_eq!(VerdictClass::Baseline as u8, 0);
    assert_eq!(VerdictClass::ShimCourt as u8, 1);
    assert_eq!(VerdictClass::KilledByBuild as u8, 2);
    assert_eq!(VerdictClass::KilledByTest as u8, 3);
    assert_eq!(VerdictClass::Equivalent as u8, 4);
    assert_eq!(VerdictClass::SurvivorDefect as u8, 5);
    assert_eq!(VerdictClass::SkipLoud as u8, 6);

    // Record geometry (redundant with the static asserts by design).
    assert_eq!(core::mem::size_of::<VerdictClass>(), 1);
    assert_eq!(core::mem::size_of::<MutantId>(), 168);
    assert_eq!(core::mem::align_of::<MutantId>(), 4);
    assert_eq!(core::mem::size_of::<KillLedgerRow>(), 2324);
    assert_eq!(core::mem::align_of::<KillLedgerRow>(), 4);
    assert_eq!(core::mem::offset_of!(KillLedgerRow, mutant), 12);
    assert_eq!(core::mem::offset_of!(KillLedgerRow, killed_by), 180);
    assert_eq!(core::mem::offset_of!(KillLedgerRow, passing_courts), 244);
    assert_eq!(core::mem::offset_of!(KillLedgerRow, mutated_sha256), 2292);

    // Pinned constants (§5.1, §5.3, §6.2).
    assert_eq!(CORPUS_SEED, 0xA20C_0FFE_E5A7_E000);
    assert_eq!(CORPUS_ROWS, 1_000_000);
    assert_eq!(ALLOC_BUDGET_MAX_ALLOCS, 976);
    assert_eq!(KILL_RATE_MIN_PERMILLE, 980);
    assert_eq!(MAX_COURTS_PER_RUN, 32);
}

/// §5.1 + §9.5: the corpus determinism receipt. The pinned sha256 is
/// asserted inside `corpus()` before any row reaches any court; this court
/// additionally pins the receipt visibly and asserts exactly 1M rows with
/// all three BN3 degenerate families present.
/// Anti-vacuity: any generator change (the gate's committed SHIM mutant
/// bumps a SplitMix64 mix constant) flips the hash and fails this court.
#[test]
fn corpus_determinism_receipt() {
    let rows = corpus();
    let digest = hash_corpus(rows);
    assert_eq!(
        hex32(&digest),
        CORPUS_SHA256_HEX,
        "§5.1: regenerated corpus hash diverges from the pinned receipt"
    );
    assert_eq!(rows.len() as u64, CORPUS_ROWS, "§5.1: exactly 1M rows");
    let zero = rows
        .iter()
        .filter(|r| r.bn3_class == bn3::ZERO_NOTIONAL)
        .count();
    let extreme = rows
        .iter()
        .filter(|r| r.bn3_class == bn3::EXTREME_DATE)
        .count();
    let dup = rows
        .iter()
        .filter(|r| r.bn3_class == bn3::DUPLICATE_KEY)
        .count();
    assert!(
        zero > 0 && extreme > 0 && dup > 0,
        "§5.1: all BN3 degenerate families must be present"
    );
}

/// §5.1 BN3 closure battery: every degenerate row is ADMITTED (classified by
/// content, never crashed on) and correctly classified; the converse holds
/// too (rows tagged `Normal` carry no degenerate content). Anti-vacuity: a
/// priority-order swap or degenerate-stride change moves degenerate content
/// onto a `Normal` tag (or breaks a tag's content match) and trips this
/// court.
#[test]
fn bn3_closure_battery() {
    let rows = corpus();
    assert_eq!(rows.len() as u64, CORPUS_ROWS);

    let mut zero = 0u64;
    let mut extreme = 0u64;
    let mut dup = 0u64;
    let mut normal = 0u64;
    let mut prev_key: u64 = rows[0].instrument_id;
    for (i, row) in rows.iter().enumerate() {
        // Row 0 has no predecessor. Anchor its comparison to a key
        // guaranteed different (`iid ^ 1`): the NORMAL content check must
        // hold, and a duplicate-tagged row 0 would (correctly) fail —
        // duplicates need real predecessors.
        let prev = if i == 0 {
            row.instrument_id ^ 1
        } else {
            prev_key
        };
        assert!(
            row_matches_tag(row, prev),
            "row {i}: tag {} does not match content",
            row.bn3_class
        );
        assert!(
            (row.aging_bucket as u64) < AGING_BUCKETS,
            "row {i}: aging bucket {} outside domain",
            row.aging_bucket
        );
        match row.bn3_class {
            bn3::NORMAL => normal += 1,
            bn3::ZERO_NOTIONAL => zero += 1,
            bn3::EXTREME_DATE => extreme += 1,
            bn3::DUPLICATE_KEY => dup += 1,
            other => panic!("row {i}: unknown BN3 class {other}"),
        }
        prev_key = row.instrument_id;
    }
    // The class partition closes over the full corpus (admitted, not
    // crashed): every row lands in exactly one class.
    assert_eq!(normal + zero + extreme + dup, CORPUS_ROWS);
    assert!(zero > 0 && extreme > 0 && dup > 0 && normal > 0);
}

/// §5.3 admission-path budget: `report.count <= ROWS / 1024` (976). The
/// admission pass streams the stored corpus, gates each row through the BN3
/// content check and folds — allocation-free by construction; any heap use
/// added inside flips the budget court. Corpus generation is forced BEFORE
/// the tracking scope opens so its one-time allocation is not misattributed
/// (`with_alloc_tracking` counts this thread's allocations only).
#[test]
fn alloc_admission_budget() {
    let _ = corpus(); // force generation outside the tracked scope
    let (_, report) = cgap_audit::alloc_counter::with_alloc_tracking(|| {
        let rows = corpus(); // already built: no allocation
        let mut admitted: u64 = 0;
        let mut checksum: u64 = 0;
        // Row-0 anchor, exact by construction (Wave-4 hardening): `x ^ 1`
        // differs from `x` for EVERY u64 — xor by a nonzero constant is
        // never a fixpoint — so the row-0 admission check cannot diverge on
        // any input. The wave-3 form (`prev_key` seeded 0 plus an `i == 0`
        // branch) was only w.h.p. equivalent under its own int-bump mutant
        // (diverged solely on a 2^-64 event; triaged Equivalent then).
        let mut prev_key = rows[0].instrument_id ^ 1;
        for (i, row) in rows.iter().enumerate() {
            // Admission decision: tag-vs-content gate; degenerate rows are
            // admitted (classified), never crashed on (§5.1). Row 0 anchors
            // on `iid0 ^ 1`; every later row on the true predecessor id —
            // exactly the DUPLICATE_KEY content contract.
            let admissible = row.bn3_class <= bn3::DUPLICATE_KEY && row_matches_tag(row, prev_key);
            assert!(admissible, "row {i} failed admission");
            admitted += 1;
            checksum = checksum.wrapping_add(row.notional ^ row.instrument_id);
            prev_key = row.instrument_id;
        }
        assert_eq!(admitted, CORPUS_ROWS);
        assert_ne!(checksum, 0, "admission pass must observe the corpus");
    });
    assert!(
        report.count <= ALLOC_BUDGET_MAX_ALLOCS as usize,
        "§5.3: admission pass allocated {} times, pinned budget {}",
        report.count,
        ALLOC_BUDGET_MAX_ALLOCS
    );
}

/// §5.3 per-row verify loop: zero-alloc (`AllocReport::is_zero()`). The
/// loop re-encodes every row into a stack buffer and re-hashes through the
/// streaming sha256 — no heap. The expected digest and any formatting stay
/// OUTSIDE the tracked scope (a String allocation would be misattributed).
/// Anti-vacuity: any heap use inside the loop flips the court.
#[test]
fn alloc_verify_zero() {
    let rows = corpus();
    let expected = hash_corpus(rows);
    let (digest, report) = cgap_audit::alloc_counter::with_alloc_tracking(|| {
        let mut hasher = Sha256::new();
        let mut enc = [0u8; ROW_ENCODING_BYTES];
        for row in rows.iter() {
            encode_row(row, &mut enc);
            hasher.update(&enc);
        }
        hasher.finish()
    });
    assert_eq!(
        hex32(&digest),
        hex32(&expected),
        "§5.3: verify loop must re-derive the corpus receipt"
    );
    assert!(report.is_zero(), "§5.3: verify loop allocated: {report:?}");
}

/// §5.2 interleave engine court: the power-cut harness over the REAL
/// engine (`cgap_storage::atomic_commit` — landed mid-session this wave).
/// Part 1 pins the frozen ABI and the stride-plan enumeration invariants
/// (slot plans, byte-length arithmetic against `RECORD_SIZE`). Part 2 runs
/// the real interleave: two transactions, per-offset cuts over the full
/// [0, 112] stride, the public `recover` classifier per offset, root
/// resolution, and row re-verification — with A05's own named
/// mutation-mode hooks (M2/M4) as the strict-vs-mutant differential.
/// Anti-vacuity: any slot-length or stride constant tweak trips the
/// enumeration invariants; a validator-chain change (seq order, tear
/// mapping, rollback guard) flips the per-offset verdicts or the
/// strict-vs-mutation-mode differential.
#[test]
fn a05_interleave_engine() {
    // A05 frozen ABI receipt (runtime mirror; if A05 ever drops its own
    // static asserts, this court still pins the ABI the engine builds on).
    assert_eq!(a05::RECORD_SIZE, 112);
    assert_eq!(core::mem::size_of::<a05::JournalRecord>(), 112);
    assert_eq!(core::mem::align_of::<a05::JournalRecord>(), 1);
    assert_eq!(core::mem::offset_of!(a05::JournalRecord, header), 0);
    assert_eq!(core::mem::offset_of!(a05::JournalRecord, payload), 48);
    assert_eq!(core::mem::offset_of!(a05::JournalRecord, trailer), 96);
    assert_eq!(a05::HEADER_SIZE, 48);
    assert_eq!(a05::PAYLOAD_SIZE, 48);
    assert_eq!(a05::TRAILER_SIZE, 16);
    assert_eq!(a05::SLOT_COUNT, 2);
    assert_eq!(a05::FIRST_SEQ, 1);
    assert_eq!(a05::BLAKE3_TOTAL_BYTES, 24);

    // §5.2 protocol: txn 1 commits rows 0..m1 into slot A, txn 2 commits
    // rows m1..1M into slot B (interleaved transactions).
    let m1 = CORPUS_ROWS / 2;
    let plan_a = build_slot_plan(0, m1);
    let plan_b = build_slot_plan(1, CORPUS_ROWS - m1);
    // Slot identity is load-bearing (§5.2: txn 1 -> slot A, txn 2 -> slot B):
    // a swapped or zeroed txn index must fail the court.
    assert_eq!(plan_a.txn, 0);
    assert_eq!(plan_b.txn, 1);

    for plan in [&plan_a, &plan_b] {
        // Byte-length arithmetic against the frozen A05 record size.
        assert_eq!(plan.slot_bytes, plan.rows * (a05::RECORD_SIZE as u64));
        assert!(plan.rows > 0);
        // Stride sweep invariants: count, monotonicity, endpoints covered.
        assert_eq!(plan.offset_count, INTERLEAVE_STRIDE_DIVISIONS + 1);
        assert_eq!(plan.truncation_offsets[0], 0, "sweep starts at k = 0");
        assert_eq!(
            plan.truncation_offsets[plan.offset_count - 1],
            plan.slot_bytes,
            "sweep must cover k = N (C4 CompleteRecord class)"
        );
        for w in plan.truncation_offsets[..plan.offset_count].windows(2) {
            assert!(w[0] < w[1], "offsets strictly monotonic");
        }
        // Every offset is a legal truncation point in [0, slot_bytes].
        assert!(
            plan.truncation_offsets[..plan.offset_count]
                .iter()
                .all(|&k| k <= plan.slot_bytes),
            "offsets must stay inside the slot"
        );
    }
    assert_eq!(plan_a.rows + plan_b.rows, CORPUS_ROWS);

    // ------------------------------------------------------------------
    // §5.2 power-cut interleave over the REAL A05 engine: txn 1 commits
    // rows 0..m1 (slot A, seq 1, complete), txn 2 commits rows m1..1M
    // (slot B, seq 2) is cut at every stride offset k. Asserts, per k:
    // the prior root always classifies cleanly (typed refusal never
    // fires), the resolved root is exactly the acknowledged commit (a
    // torn newer txn never wins), every admitted row re-verifies against
    // the recovered root, and the per-slot verdict matches the classifier's
    // own tear-class mapping.
    // ------------------------------------------------------------------
    let rows = corpus();
    let m1 = (CORPUS_ROWS / 2) as usize;

    // Row-range verification fold: the root descriptor carries the fold
    // over (notional ^ instrument_id) and the row count for its range, so
    // "every admitted row re-verifies" is a recomputation over the corpus,
    // not a comparison of the kernel's own output.
    let fold_range = |lo: usize, hi: usize| -> u64 {
        rows[lo..hi]
            .iter()
            .fold(0u64, |acc, r| acc.wrapping_add(r.notional ^ r.instrument_id))
    };
    fn range_root(rows: &[SettlementRow], lo: usize, hi: usize, dag_seq: u64) -> a05::NewRootDescriptor {
        let fold = rows[lo..hi]
            .iter()
            .fold(0u64, |acc, r| acc.wrapping_add(r.notional ^ r.instrument_id));
        let count = (hi - lo) as u64;
        let mut oid = [0u8; 20];
        oid[0..8].copy_from_slice(&fold.to_le_bytes());
        oid[8..16].copy_from_slice(&count.to_le_bytes());
        a05::NewRootDescriptor::new(oid, fold, dag_seq, 0, a05::FLAG_DUAL_WRITE)
    }

    let base = range_root(rows, 0, 1, 0); // pre-journal anchor (dag_seq 0)
    let root1 = range_root(rows, 0, m1, 1); // txn 1: rows 0..m1
    let root2 = range_root(rows, m1, CORPUS_ROWS as usize, 2); // txn 2: m1..1M

    // Strict recovery resolves the prior commit; sanity for the sweep.
    let mut m = a05::FlashMedium::new(base);
    m.fence_artifacts();
    a05::write_record_cut(&mut m, 1, &root1, a05::RECORD_SIZE).expect("txn 1 commit");
    let prior = a05::recover(&m).expect("prior commit classifies cleanly");
    assert_eq!(prior.root, root1, "prior root resolved");
    assert_eq!(prior.recovered_seq, Some(1));
    assert_eq!(prior.report, a05::RecoveryReport::Clean);
    assert_eq!(prior.slot_verdicts, [a05::SlotVerdict::Valid, a05::SlotVerdict::Empty]);

    // The stride sweep (§5.2): every adversarial truncation offset k of
    // txn 2's slot-B record, prior commit complete in slot A. The REAL
    // journal slot holds exactly one 112-byte record (A05's surface), so
    // the sweep plan is built over the real record geometry (rows = 1 →
    // slot_bytes = RECORD_SIZE), not the corpus-scale byte plan above
    // (which stays as the §5.2 arithmetic pin).
    let cut_plan = build_slot_plan(1, 1);
    assert_eq!(cut_plan.slot_bytes, a05::RECORD_SIZE as u64);
    let mut torn_class_offsets = 0u64;
    for &k in cut_plan.truncation_offsets[..cut_plan.offset_count].iter() {
        let k = k as usize;
        let mut medium = a05::FlashMedium::new(base);
        medium.fence_artifacts();
        a05::write_record_cut(&mut medium, 1, &root1, a05::RECORD_SIZE)
            .expect("prior commit always completes");
        a05::write_record_cut(&mut medium, 2, &root2, k).expect("cut write obeys the fence law");
        let outcome = a05::recover(&medium).expect("recovery never refuses a single-fault tear");

        // Per-slot verdicts: A valid; B's verdict matches the classifier's
        // own tear-class mapping for this offset (§5.2 classification).
        let expected_b = match a05::tear_class_for_offset(k) {
            a05::TearClass::PreWrite => a05::SlotVerdict::Empty,
            a05::TearClass::CompleteRecord => a05::SlotVerdict::Valid,
            _ => a05::SlotVerdict::Damaged,
        };
        assert_eq!(
            outcome.slot_verdicts,
            [a05::SlotVerdict::Valid, expected_b],
            "k={k}: slot verdicts must match the tear-class mapping"
        );
        if expected_b == a05::SlotVerdict::Damaged {
            torn_class_offsets += 1;
        }

        // Root resolution: a torn txn-2 never wins; the complete txn-2
        // (k = N) commits. The prior commit is never rolled back to base.
        let (expected_root, expected_seq) = if k < a05::RECORD_SIZE {
            (root1, 1u64)
        } else {
            (root2, 2u64)
        };
        assert_ne!(
            outcome.root, base,
            "k={k}: a torn newer txn must never roll back the prior commit"
        );
        assert_eq!(outcome.root, expected_root, "k={k}: root resolution");
        assert_eq!(outcome.recovered_seq, Some(expected_seq), "k={k}: applied seq");

        // §5.2: every admitted row under the recovered root re-verifies.
        let lo = if expected_seq == 1 { 0 } else { m1 };
        let hi = if expected_seq == 1 { m1 } else { CORPUS_ROWS as usize };
        let fold = fold_range(lo, hi);
        // NewRootDescriptor is repr(C, packed): copy packed fields through
        // locals (a plain copy compiles to an unaligned load; an
        // assert_eq! reference would not).
        let got_fold = outcome.root.parquet_root;
        let got_dag = outcome.root.dag_seq;
        assert_eq!(got_fold, fold, "k={k}: admitted rows re-verify");
        assert_eq!(got_dag, expected_seq, "k={k}: dag_seq advances");

        // Zero-corruption observable: recovery reports Clean — the tear is
        // confined to the journal slot, the resolved root is a real commit.
        assert_eq!(outcome.report, a05::RecoveryReport::Clean, "k={k}: clean recovery");
    }
    assert_eq!(
        torn_class_offsets,
        INTERLEAVE_STRIDE_DIVISIONS as u64 - 1,
        "the sweep must cross all four tear classes (31 torn offsets of 33)"
    );

    // Interleave tail determinism: writing txn 2's FULL record then
    // re-writing it cut at 0 must leave the committed state readable
    // (C4's record is durable; the flash AND-semantics cannot unwrite it —
    // a zero-write "cut" over committed bytes changes nothing).
    let mut tail = a05::FlashMedium::new(base);
    tail.fence_artifacts();
    a05::write_record_cut(&mut tail, 2, &root2, a05::RECORD_SIZE).expect("full txn 2");
    a05::write_record_cut(&mut tail, 2, &root2, 0).expect("zero-byte cut is a no-op");
    let tail_out = a05::recover(&tail).expect("committed tail survives");
    assert_eq!(tail_out.root, root2, "C4 state is monotone under a later no-op cut");
    assert_eq!(tail_out.recovered_seq, Some(2));

    // Anti-vacuity over the CONSUMED classifier (A05's own named mutation
    // hooks, real behavior variants — not mocks): the seq-order mutation
    // (M2) must resolve the OLDER record under a both-valid journal, and
    // the rollback-guard mutation (M4) must admit what strict recovery
    // refuses (SeqRegression).
    let mut both = a05::FlashMedium::new(base);
    both.fence_artifacts();
    a05::write_record_cut(&mut both, 1, &root1, a05::RECORD_SIZE).expect("txn 1");
    a05::write_record_cut(&mut both, 2, &root2, a05::RECORD_SIZE).expect("txn 2");
    let strict = a05::recover(&both).expect("both-valid recovery");
    assert_eq!(strict.root, root2, "strict: highest seq wins");
    assert_eq!(strict.recovered_seq, Some(2));
    let m2 = a05::recover_mode(&both, a05::MutationMode::M2LowestSeq).expect("M2 recovery");
    assert_eq!(m2.root, root1, "M2 flip witness: lowest seq resolved instead");
    assert_eq!(m2.recovered_seq, Some(1));

    let rollback = range_root(rows, 0, 1, 0); // dag_seq == base dag_seq (0)
    let mut regressed = a05::FlashMedium::new(base);
    regressed.fence_artifacts();
    a05::write_record_cut(&mut regressed, 1, &rollback, a05::RECORD_SIZE).expect("regressed rec");
    assert_eq!(
        a05::recover(&regressed).unwrap_err(),
        a05::RecoveryError::SeqRegression,
        "strict: rollback-as-corruption refused typed"
    );
    let m4 = a05::recover_mode(&regressed, a05::MutationMode::M4SeqRegressionUnchecked)
        .expect("M4 drops the guard");
    assert_eq!(m4.root, rollback, "M4 flip witness: rollback admitted");
}

// ---------------------------------------------------------------------------
// Wave 3 — full lane scope: verdict courts over the sealed sibling surfaces
// (A20 §3 C4 context: the verdict lane consumes "public APIs only").
// Divergence disclosed (doc yields): the dispatch slice's "WAL, unifier,
// dam" components belong to lanes A05/A12/A14; this lane's Wave-3 form for
// each is CONSUMPTION, not re-implementation. A05's recovery classifier
// (`recover`/`recover_mode`, `write_record_cut`, `FlashMedium`) landed
// mid-session this wave (mtime witness in the lane receipt); the §5.2
// tear-classification step is therefore CONSUMED for real by
// `a05_interleave_engine` — no skip row remains.
// ---------------------------------------------------------------------------

use cgap_blake3::hash as blake3_hash;
use cgap_core::frame::{
    CgapFrameHeader, FrameError, P28LicenseAttestation, ATTESTATION_LEN, FLAG_BN3,
    FLAG_RESERVED_MASK, FRAME_HEADER_LEN, FRAME_MAGIC, FRAME_VERSION, LICENSE_FLAG_DERIVATIVE,
    LICENSE_FLAG_ROOT, LICENSE_MAGIC, SAFE_HARBOR_CEILING_CENTS_MAX,
};
use cgap_gate::penstock::{
    ingress, CGAP_ERR_INVALID_MAGIC, CGAP_ERR_OABI_MISMATCH, CGAP_ERR_P28_SPOLIATION, CGAP_OK,
    FrameView,
};
use cgap_bn3::sigil::{encode_term, Sigil};
use cgap_bn3::unify as a12;
use cgap_storage::atomic_commit::JournalWriter as _;

/// §5.3/§8: unifier-court store sample — `SAMPLE_STRIDE`-th corpus row into
/// the A12 store. The stride is a named constant, recorded (never hidden,
/// §5.2/§8).
const A12_SAMPLE_ROWS: u64 = 10_000;
const A12_SAMPLE_STRIDE: u64 = CORPUS_ROWS / A12_SAMPLE_ROWS;

/// Term-word universe for the A12 consumption store: `book`/`counterparty`
/// are folded into `UNIVERSE` distinct constants so the corpus-derived
/// store has real joins (raw 56-bit random payloads would never collide —
/// a zero-match store proves nothing about the join).
const A12_UNIVERSE: u64 = 4_096;
/// The store predicate (`p` slot) and the derived predicate (`p2`, the
/// rule head) as SlotConst payloads.
const A12_PRED: u64 = 9_001;
const A12_PRED_DERIVED: u64 = 9_002;

/// Canonical SlotConst term word for `payload` (A11 §5 sigil encoding via
/// the real `encode_term`).
fn a12_const(payload: u64) -> u64 {
    encode_term(Sigil::SlotConst, payload).expect("payload < 2^56")
}

/// Canonical VarUni term word for register `n`.
fn a12_var(n: u64) -> u64 {
    encode_term(Sigil::VarUni, n).expect("n < 256")
}

/// Cross-lane ABI pin court (A20 dispatch anchors, runtime receipts): A04
/// canonical frame geometry, A17 attestation geometry, A19 status register
/// values, A05 tear/recovery class sets.
/// Anti-vacuity: any frozen widen/narrow/reorder on any pinned surface
/// flips a runtime assert here (and the owning crate's own compile asserts
/// break its build).
#[test]
fn cross_lane_abi_pin() {
    use core::mem::{align_of, offset_of, size_of};

    // --- A04: canonical CgapFrameHeader (128 B, align 64) ------------------
    assert_eq!(size_of::<CgapFrameHeader>(), 128);
    assert_eq!(align_of::<CgapFrameHeader>(), 64);
    assert_eq!(offset_of!(CgapFrameHeader, magic), 0);
    assert_eq!(offset_of!(CgapFrameHeader, version), 4);
    assert_eq!(offset_of!(CgapFrameHeader, flags), 5);
    assert_eq!(offset_of!(CgapFrameHeader, reserved), 6);
    assert_eq!(offset_of!(CgapFrameHeader, payload_byte_len), 8);
    assert_eq!(offset_of!(CgapFrameHeader, oabi_digest), 16);
    assert_eq!(offset_of!(CgapFrameHeader, attestation), 48);

    // --- A17: canonical P28LicenseAttestation (80 B, align 8, gap-free) ----
    assert_eq!(size_of::<P28LicenseAttestation>(), 80);
    assert_eq!(align_of::<P28LicenseAttestation>(), 8);
    assert_eq!(offset_of!(P28LicenseAttestation, license_magic), 0);
    assert_eq!(offset_of!(P28LicenseAttestation, version), 4);
    assert_eq!(offset_of!(P28LicenseAttestation, attestation_flags), 6);
    assert_eq!(offset_of!(P28LicenseAttestation, safe_harbor_ceiling_cents), 8);
    assert_eq!(offset_of!(P28LicenseAttestation, origin_authority_digest), 16);
    assert_eq!(offset_of!(P28LicenseAttestation, upstream_receipt), 48);
    assert_eq!(ATTESTATION_LEN, 80);
    assert_eq!(FRAME_HEADER_LEN, 128);

    // --- A14 mirror ≡ A04 canonical (pin-vs-pin, byte geometry) ------------
    assert_eq!(cgap_gate::penstock::HEADER_LEN, FRAME_HEADER_LEN);
    assert_eq!(cgap_gate::penstock::MAGIC, FRAME_MAGIC);
    assert_eq!(cgap_gate::penstock::OFF_ATTESTATION, 48);
    assert_eq!(cgap_gate::penstock::P28_LICENSE_MAGIC, LICENSE_MAGIC);
    assert_eq!(
        cgap_gate::penstock::P28_SAFE_HARBOR_CEILING_CENTS,
        SAFE_HARBOR_CEILING_CENTS_MAX
    );
    // The mirror's own compile asserts bind its struct; the pin court binds
    // the two pins to each other.

    // --- A19: status register values (frozen A14 copies of the register) ---
    assert_eq!(CGAP_OK, 0x0000_0000);
    assert_eq!(CGAP_ERR_INVALID_MAGIC, 0x4347_0001);
    assert_eq!(CGAP_ERR_OABI_MISMATCH, 0x4347_0002);
    assert_eq!(CGAP_ERR_P28_SPOLIATION, 0x5032_38FF);

    // --- A05: the frozen class sets (§5.2 C0-C4 + typed refusals/reports) --
    let tears = [
        a05::TearClass::PreWrite,
        a05::TearClass::TornHeader,
        a05::TearClass::TornPayload,
        a05::TearClass::TornTrailer,
        a05::TearClass::CompleteRecord,
    ];
    for (i, a) in tears.iter().enumerate() {
        for b in &tears[i + 1..] {
            assert_ne!(a, b, "A05 TearClass variants must stay distinct");
        }
    }
    assert_eq!(tears.len(), 5, "C0-C4: five tear classes");
    let refusals = [
        a05::RecoveryError::JournalAmbiguous,
        a05::RecoveryError::SeqRegression,
    ];
    assert_ne!(refusals[0], refusals[1]);
    let reports = [a05::RecoveryReport::Clean, a05::RecoveryReport::RecoveryDegraded];
    assert_ne!(reports[0], reports[1]);
}

/// §3 unifier consumption court, part 1: the join kernel. A hand-pinned
/// store with an exact hand-derived match set (soundness AND completeness,
/// checked against an independent test-side oracle), chunk-size
/// independence, and the kernel's typed refusal surface.
/// Anti-vacuity: a join-direction swap, register-band change, or
/// match-order change flips the pinned rows/counts or the chunk identity.
#[test]
fn a12_unifier_join_court() {
    let p = a12_const(A12_PRED);
    let p2 = a12_const(A12_PRED_DERIVED);

    // -- Hand-pinned store: six statements, hand-derived five matches ------
    // rows:        0        1        2        3        4        5
    // (s,p,o): (1,p,2) (2,p,3) (3,p,4) (2,p,5) (5,p,6) (9,p,9)
    // Rule R(?x p ?y, ?y p ?z => ?x p2 ?z) matches:
    //   (row0,row1): x=1 y=2 z=3      (row0,row3): x=1 y=2 z=5
    //   (row1,row2): x=2 y=3 z=4      (row3,row4): x=2 y=5 z=6
    //   (row5,row5): x=9 y=9 z=9
    let store_s: Vec<u64> = [1u64, 2, 3, 2, 5, 9].iter().map(|&v| a12_const(v)).collect();
    let store_o: Vec<u64> = [2u64, 3, 4, 5, 6, 9].iter().map(|&v| a12_const(v)).collect();
    let store_p: Vec<u64> = vec![p; 6];
    let cols = a12::StatementColumns::new(&store_s, &store_p, &store_o).expect("equal lengths");

    let rule = a12::Rule {
        rule_id: 7,
        body: vec![
            a12::BodyAtom::new(a12_var(0), p, a12_var(1)),
            a12::BodyAtom::new(a12_var(1), p, a12_var(2)),
        ],
        head: [a12_var(0), p2, a12_var(2)],
    };
    let compiled = a12::RuleCompiler::compile(&rule).expect("hand rule compiles");
    assert_eq!(compiled.var_count, 3, "three distinct body variables");
    // Compile preserves the emitting-rule id (A12: `CompiledRule::rule_id`
    // carries the rule's id into `DeltaBatch::rule_id`, the A15 collapse
    // key); an unpinned id drifts silently through the join path.
    // (Wave 5: kills the wave-3 campaign's `rule_id: 7 -> 6` survivor.)
    assert_eq!(compiled.rule_id, 7, "compile preserves the emitting-rule id");
    // The const pool interns the DISTINCT words the rule itself names:
    // p (both body atoms) and p2 (head) — exactly two; store words are
    // store-side, never rule-side.
    assert_eq!(compiled.const_pool.len(), 2);

    let matches = a12::unify_batch(&compiled, &cols, a12::DEFAULT_CHUNK_ROWS)
        .expect("hand-store unify");
    assert_eq!(matches.len, 5, "hand-derived match count");
    // Per-match pin: atom rows, bound values, and the head derivation.
    let expected = [
        ([0u32, 1u32], [1u64, 2u64, 3u64]),
        ([0u32, 3u32], [1u64, 2u64, 5u64]),
        ([1u32, 2u32], [2u64, 3u64, 4u64]),
        ([3u32, 4u32], [2u64, 5u64, 6u64]),
        ([5u32, 5u32], [9u64, 9u64, 9u64]),
    ];
    // Match order is (atom-0 row ascending, join candidate ascending) — the
    // kernel's own documented order (unify.rs eval_chunk); a reorder flips
    // this pin.
    for (m, (rows, vars)) in expected.iter().enumerate() {
        assert_eq!(&matches.atom_rows[0][m], &rows[0], "match {m} atom-0 row");
        assert_eq!(&matches.atom_rows[1][m], &rows[1], "match {m} atom-1 row");
        for v in 0..3 {
            assert_eq!(matches.var_values[v][m], vars[v], "match {m} var {v}");
        }
    }
    // Column alignment invariant (the projector's debug_assert, asserted).
    for col in &matches.var_values {
        assert_eq!(col.len(), matches.len);
    }

    // -- Soundness + completeness against an independent test-side oracle --
    // The oracle is a plain nested hash lookup over the store — a different
    // algorithm computing the same join semantics, used to check the
    // kernel, never linked into it (Chicago: the assert is on final state).
    let mut oracle_count = 0usize;
    let mut by_s: std::collections::HashMap<u64, Vec<usize>> = std::collections::HashMap::new();
    for (i, &s) in store_s.iter().enumerate() {
        by_s.entry(s).or_default().push(i);
    }
    for &o in &store_o {
        if let Some(rows) = by_s.get(&o) {
            oracle_count += rows.len();
        }
    }
    assert_eq!(matches.len as usize, oracle_count, "oracle agrees on count");

    // -- Chunk-size independence: match output is partition-independent ----
    for chunk in [1usize, 2, 3, 6, a12::DEFAULT_CHUNK_ROWS] {
        let m = a12::unify_batch(&compiled, &cols, chunk).expect("unify");
        assert_eq!(m, matches, "chunk size {chunk} changed the match set");
    }

    // -- Corpus-scale store: real joins over 10,000 corpus-derived rows ----
    let rows = corpus();
    let mut big_s = Vec::with_capacity(A12_SAMPLE_ROWS as usize);
    let mut big_o = Vec::with_capacity(A12_SAMPLE_ROWS as usize);
    let mut i = 0u64;
    while i < A12_SAMPLE_ROWS {
        let row = &rows[(i * A12_SAMPLE_STRIDE) as usize];
        big_s.push(a12_const(row.book % A12_UNIVERSE));
        big_o.push(a12_const(row.counterparty % A12_UNIVERSE));
        i += 1;
    }
    let big_p = vec![p; big_s.len()];
    let big_cols = a12::StatementColumns::new(&big_s, &big_p, &big_o).expect("equal lengths");
    let big = a12::unify_batch(&compiled, &big_cols, a12::DEFAULT_CHUNK_ROWS)
        .expect("corpus-store unify");
    assert!(big.len > 0, "corpus store must produce real joins");
    // Soundness: every match chains contentually through the store.
    for m in 0..big.len {
        let r0 = big.atom_rows[0][m] as usize;
        let r1 = big.atom_rows[1][m] as usize;
        assert_eq!(big_o[r0], big_s[r1], "match {m}: join chain broken");
        assert_eq!(big.var_values[0][m], big_s[r0]);
        assert_eq!(big.var_values[1][m], big_s[r1]);
        assert_eq!(big.var_values[2][m], big_o[r1]);
    }
    // Completeness vs the oracle over the big store.
    let mut oracle_big = 0usize;
    let mut by_s_big: std::collections::HashMap<u64, Vec<usize>> = std::collections::HashMap::new();
    for (i, &s) in big_s.iter().enumerate() {
        by_s_big.entry(s).or_default().push(i);
    }
    for &o in &big_o {
        if let Some(rs) = by_s_big.get(&o) {
            oracle_big += rs.len();
        }
    }
    assert_eq!(big.len as usize, oracle_big, "oracle agrees at corpus scale");

    // -- Typed refusal surface (fail-closed boundaries, never silent) ------
    // Empty body.
    let empty = a12::Rule { rule_id: 0, body: vec![], head: [p, p, p] };
    assert_eq!(a12::RuleCompiler::compile(&empty), Err(a12::UnifyError::EmptyRuleBody));
    // FormulaRef in the p slot (A12 ban).
    let formula_p = encode_term(Sigil::FormulaRef, 11).expect("formula word");
    let bad_p = a12::Rule {
        rule_id: 1,
        body: vec![a12::BodyAtom::new(a12_var(0), formula_p, a12_var(1))],
        head: [a12_var(0), p2, a12_var(1)],
    };
    assert_eq!(
        a12::RuleCompiler::compile(&bad_p),
        Err(a12::UnifyError::FormulaRefInPredicate)
    );
    // Head variable no body atom binds.
    let unbound = a12::Rule {
        rule_id: 2,
        body: vec![a12::BodyAtom::new(a12_const(1), p, a12_const(2))],
        head: [a12_var(3), p2, a12_const(4)],
    };
    assert_eq!(
        a12::RuleCompiler::compile(&unbound),
        Err(a12::UnifyError::UnboundHeadVariable)
    );
    // VarUni register >= 256 (hand-built hostile word — the sigil encoder
    // refuses first, so the raw word bypasses it).
    let hostile = (0x01u64 << 56) | 300u64;
    let over = a12::Rule {
        rule_id: 3,
        body: vec![a12::BodyAtom::new(hostile, p, a12_const(2))],
        head: [a12_var(0), p2, a12_var(1)],
    };
    assert_eq!(
        a12::RuleCompiler::compile(&over),
        Err(a12::UnifyError::RegisterOutOfRange(300))
    );
    // Chunk size zero.
    assert_eq!(
        a12::unify_batch(&compiled, &cols, 0),
        Err(a12::UnifyError::InvalidChunkSize)
    );
}

/// §3 unifier consumption court, part 2: the fixpoint driver. A 32-edge
/// chain store; the homiconic transitive rule's closure is an ARITHMETIC
/// ground truth (33-node path graph => 496 derivable pairs at distances
/// 2..=32, count 33-d each) — hand-derived sums, never the kernel's own
/// output.
/// Anti-vacuity: a round-budget or delta-append change shifts the batch
/// count (6 rounds, doubling closure) or the closure totals.
#[test]
fn a12_unifier_fixpoint_court() {
    let p = a12_const(A12_PRED);

    // 32-edge chain over nodes 0..=32 (33 nodes).
    const CHAIN_EDGES: u64 = 32;
    let s: Vec<u64> = (0..CHAIN_EDGES).map(|i| a12_const(i)).collect();
    let o: Vec<u64> = (1..=CHAIN_EDGES).map(|i| a12_const(i)).collect();
    let pcol = vec![p; CHAIN_EDGES as usize];
    let cols = a12::StatementColumns::new(&s, &pcol, &o).expect("equal lengths");

    // HOMICONIC rule: head predicate = body predicate, so derived rows
    // re-join the body and the driver actually closes transitively (the
    // non-homiconic p2 head composes exactly once and stops — witnessed
    // this session: 2 batches, 31 rows, then no new rows).
    let rule = a12::Rule {
        rule_id: 11,
        body: vec![
            a12::BodyAtom::new(a12_var(0), p, a12_var(1)),
            a12::BodyAtom::new(a12_var(1), p, a12_var(2)),
        ],
        head: [a12_var(0), p, a12_var(2)],
    };

    let batches =
        a12::unify_to_fixpoint(&[rule_cloned(&rule)], &cols).expect("chain closure inside round budget");
    // Derived rows carry the body predicate, so derived-with-derived
    // composition compounds: after round r the store reaches distances
    // 1..2^r (round sizes 31, 59, 106, 164, 136 — sum 496), so 5
    // productive rounds + the final zero-new round = 6 batches. Pinned:
    // 6 batches, distance 32 reached within DEFAULT_MAX_ROUNDS.
    assert_eq!(batches.len(), 6, "doubling closure: 5 rounds + final empty");
    assert!(batches.len() as u32 <= a12::DEFAULT_MAX_ROUNDS);

    // Derived ground truth: every pair (i, j) with j - i >= 2 is derived
    // (distance-1 pairs are the store's own 32 edges); distances 2..=32
    // give sum(33-d) = 496. Arithmetic, not kernel output.
    let mut derived: std::collections::HashSet<(u64, u64, u64)> = std::collections::HashSet::new();
    let mut emitted_total = 0usize;
    for batch in &batches {
        assert_eq!(batch.rule_id, 11);
        for r in 0..batch.rows.len() {
            let (s_word, p_word, o_word, _flags) = batch.rows.row(r);
            assert_eq!(p_word, p, "derived rows re-join the body predicate");
            // Decode payloads through the real A11 decoder.
            let (sig, sp) = cgap_bn3::sigil::decode_term(s_word).expect("s word");
            let (_, pp) = cgap_bn3::sigil::decode_term(p_word).expect("p word");
            let (sig_o, op) = cgap_bn3::sigil::decode_term(o_word).expect("o word");
            assert_eq!(sig, Sigil::SlotConst);
            assert_eq!(pp, A12_PRED);
            assert_eq!(sig_o, Sigil::SlotConst);
            // Every emission is a lawful closure member (composition of two
            // edges: distance >= 2, inside the node range).
            assert!(op >= sp && op - sp >= 2 && op <= CHAIN_EDGES);
            derived.insert((sp, p_word, op));
            emitted_total += 1;
        }
    }
    // A12's documented division (unify.rs): the projector never dedups —
    // batches re-emit re-derivations for A15 to collapse. So emissions
    // exceed distinct triples, and the DISTINCT set is exactly the
    // arithmetic closure: sum(33-d) for d in 2..=32 = 496 (edges never
    // derived).
    assert!(
        emitted_total > derived.len(),
        "re-derivations must be re-emitted (A12 M12-7 division); got \
         {emitted_total} emissions over {} distinct triples",
        derived.len()
    );
    assert_eq!(
        derived.len(),
        496,
        "sum(33-d) for d in 2..=32 = 496 distinct derived pairs (edges never derived)"
    );
    assert!(
        !derived.iter().any(|(i, _, j)| j - i == 1),
        "adjacent pairs are store edges, not derivations"
    );
    // Distance histogram over derived pairs: exactly 33-d at distance d.
    for d in 2..=CHAIN_EDGES {
        let count = derived
            .iter()
            .filter(|(i, _, j)| j - i == d)
            .count() as u64;
        assert_eq!(count, CHAIN_EDGES + 1 - d, "distance-{d} derived-pair count");
    }

    // -- Round-budget refusal is typed, never a hang ------------------------
    let budget = a12::unify_to_fixpoint_opt(&[rule_cloned(&rule)], &cols, 2, 4);
    assert_eq!(budget, Err(a12::UnifyError::FixpointBudgetExceeded { rounds: 2 }));
}

/// Clone helper so the budget refusal above reads cleanly (Rule is
/// Clone; this keeps the two call sites symmetric).
fn rule_cloned(rule: &a12::Rule) -> a12::Rule {
    rule.clone()
}

/// §3 dam consumption court: the A14 penstock `ingress` refusal boundary
/// over corpus-derived frames. Valid frames built through the REAL A04
/// encoder and the REAL BLAKE3 kernel admit with the payload view wired;
/// every penstock taxonomy row 1-6 is injected on a valid base frame and
/// must return exactly the pinned A19 code; the group order is pinned;
/// content independence and a 1M-frame zero-alloc sweep close it.
/// Anti-vacuity: removal of any penstock check admits what that row
/// expects rejected; a reorder flips the order-pinning frames.
#[test]
fn a14_dam_ingress_court() {
    // -- Valid-frame material from the corpus (real A04 + real BLAKE3) ------
    let rows = corpus();
    let mut payload = [0u8; ROW_ENCODING_BYTES * 4];
    for (k, row) in rows.iter().take(4).enumerate() {
        let mut chunk = [0u8; ROW_ENCODING_BYTES];
        encode_row(row, &mut chunk);
        payload[k * ROW_ENCODING_BYTES..(k + 1) * ROW_ENCODING_BYTES].copy_from_slice(&chunk);
    }
    let frame = valid_frame(&payload);
    assert_eq!(frame.len(), FRAME_HEADER_LEN + payload.len());

    // Admit: exact status, zero-copy payload view into the caller's bytes.
    let mut view = FrameView { payload: &[] };
    let status = ingress(&frame, frame.len(), Some(&mut view));
    assert_eq!(status, CGAP_OK, "structurally valid frame must admit");
    assert_eq!(view.payload, &payload[..], "view points into the frame");

    // The canonical header decodes back to the built fields, and the A04
    // digest verifier (real BLAKE3 injected) accepts; a corrupted payload
    // byte is a typed DigestMismatch.
    let hdr = CgapFrameHeader::decode(&frame).expect("decode own frame");
    assert_eq!(hdr.magic, FRAME_MAGIC);
    assert_eq!(hdr.version, FRAME_VERSION);
    assert_eq!(hdr.flags, FLAG_BN3);
    assert_eq!(hdr.payload_byte_len, payload.len() as u64);
    hdr.verify_payload(&payload, blake3_hash).expect("digest verifies");
    let mut tampered = frame.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    assert_eq!(
        hdr.verify_payload(&tampered[FRAME_HEADER_LEN..], blake3_hash),
        Err(FrameError::DigestMismatch),
        "corrupted payload byte must be a typed digest refusal"
    );

    // -- Taxonomy rows 1-6 over the SAME base frame (exact codes) ----------
    // row 1: truncated declared length; lying caller length.
    assert_eq!(ingress(&frame, FRAME_HEADER_LEN - 1, None), CGAP_ERR_INVALID_MAGIC);
    assert_eq!(
        ingress(&frame, frame.len() + 1, None),
        CGAP_ERR_INVALID_MAGIC,
        "declared len beyond the slice is rejected unread"
    );
    // row 2: bad magic; bad version.
    let mut bad_magic = frame.clone();
    bad_magic[0] = b'X';
    assert_eq!(ingress(&bad_magic, bad_magic.len(), None), CGAP_ERR_INVALID_MAGIC);
    let mut bad_version = frame.clone();
    bad_version[4] = FRAME_VERSION + 1;
    assert_eq!(ingress(&bad_version, bad_version.len(), None), CGAP_ERR_INVALID_MAGIC);
    // row 3a: all-zero oabi digest.
    let mut zero_digest = frame.clone();
    zero_digest[16..48].fill(0);
    assert_eq!(ingress(&zero_digest, zero_digest.len(), None), CGAP_ERR_OABI_MISMATCH);
    // row 3b: declared payload longer than the buffer allows.
    let mut long_payload = frame.clone();
    long_payload[8..16].copy_from_slice(&(payload.len() as u64 + 1).to_le_bytes());
    assert_eq!(ingress(&long_payload, long_payload.len(), None), CGAP_ERR_OABI_MISMATCH);
    // row 4: attestation magic flipped.
    let mut bad_att_magic = frame.clone();
    bad_att_magic[48] = b'X';
    assert_eq!(ingress(&bad_att_magic, bad_att_magic.len(), None), CGAP_ERR_P28_SPOLIATION);
    // row 5: each A17 field law, one at a time.
    let spoliation_cases: [(&str, Box<dyn Fn(&mut [u8])>); 7] = [
        ("att version", Box::new(|b: &mut [u8]| b[48 + 4..48 + 6].copy_from_slice(&2u16.to_le_bytes()))),
        ("att reserved bit", Box::new(|b: &mut [u8]| b[48 + 6] |= 0x08)),
        ("lineage zero", Box::new(|b: &mut [u8]| b[48 + 6] = 0)),
        ("lineage two bits", Box::new(|b: &mut [u8]| b[48 + 6] = 0x03)),
        ("ceiling over max", Box::new(|b: &mut [u8]| {
            b[48 + 8..48 + 16].copy_from_slice(&(SAFE_HARBOR_CEILING_CENTS_MAX + 1).to_le_bytes())
        })),
        ("ROOT with receipt", Box::new(|b: &mut [u8]| b[48 + 48..48 + 80].fill(1))),
        ("DERIVATIVE without receipt", Box::new(|b: &mut [u8]| {
            b[48 + 6..48 + 8].copy_from_slice(&LICENSE_FLAG_DERIVATIVE.to_le_bytes())
        })),
    ];
    for (name, mutate) in spoliation_cases {
        let mut f = frame.clone();
        mutate(&mut f);
        assert_eq!(
            ingress(&f, f.len(), None),
            CGAP_ERR_P28_SPOLIATION,
            "row 5 '{name}' must be spoliation"
        );
    }
    // row 6 (A14 divergence note): reserved header flag bits ADMIT.
    let mut reserved_flags = frame.clone();
    reserved_flags[5] |= FLAG_RESERVED_MASK;
    assert_eq!(ingress(&reserved_flags, reserved_flags.len(), None), CGAP_OK);

    // -- Group order pinning (§4 normative order, first failure returns) ----
    let mut magic_and_att = frame.clone();
    magic_and_att[0] = b'X';
    magic_and_att[48] = b'X';
    assert_eq!(
        ingress(&magic_and_att, magic_and_att.len(), None),
        CGAP_ERR_INVALID_MAGIC,
        "magic precedes attestation"
    );
    let mut digest_and_att = frame.clone();
    digest_and_att[16..48].fill(0);
    digest_and_att[48] = b'X';
    assert_eq!(
        ingress(&digest_and_att, digest_and_att.len(), None),
        CGAP_ERR_OABI_MISMATCH,
        "oabi coherence precedes attestation"
    );

    // -- Content independence: status does not depend on payload content ---
    // Three internally-consistent frames with payload lengths {0, 1 KiB,
    // 1 MiB}: same status, view lengths track the declared payload.
    for payload_len in [0usize, 1024, 1 << 20] {
        let p = vec![0xA5u8; payload_len];
        let f = valid_frame(&p);
        let mut v = FrameView { payload: &[] };
        assert_eq!(ingress(&f, f.len(), Some(&mut v)), CGAP_OK);
        assert_eq!(v.payload.len(), payload_len);
    }

    // -- §5.3/§8 scale: 1M corpus-derived frames through the real gate,
    //    allocation-free (A20 consumption-side witness of A14 §8.2). The
    //    template declares EXACTLY the sweep payload (one row, 54 B) — a
    //    header over-long for its buffer is correctly refused by row 3b,
    //    which is the gate working, not the court. The per-row content
    //    patch keeps every iteration distinct while the frame stays
    //    structurally valid; every frame must admit.
    let mut sweep_payload = [0u8; ROW_ENCODING_BYTES];
    encode_row(&rows[7], &mut sweep_payload);
    let template = valid_frame(&sweep_payload);
    let mut sweep_frame = [0u8; FRAME_HEADER_LEN + ROW_ENCODING_BYTES];
    sweep_frame.copy_from_slice(&template);
    let mut row_enc = [0u8; ROW_ENCODING_BYTES];
    encode_row(&rows[7], &mut row_enc);
    let (admitted, report) = cgap_audit::alloc_counter::with_alloc_tracking(|| {
        let mut admitted = 0u64;
        for row in rows.iter() {
            // Row-distinct header content: digest field carries the row's
            // notional bytes (the gate is a limiter — it never re-derives
            // the digest, penstock.rs row 3 only requires nonzero).
            sweep_frame[16..24].copy_from_slice(&row.notional.to_le_bytes());
            sweep_frame[24..32].copy_from_slice(&row.instrument_id.to_le_bytes());
            sweep_frame[FRAME_HEADER_LEN..].copy_from_slice(&row_enc);
            let mut view = FrameView { payload: &[] };
            if ingress(&sweep_frame, sweep_frame.len(), Some(&mut view)) == CGAP_OK
                && view.payload.len() == ROW_ENCODING_BYTES
            {
                admitted += 1;
            }
        }
        admitted
    });
    assert_eq!(admitted, CORPUS_ROWS, "every corpus frame admits");
    assert!(report.is_zero(), "1M-frame sweep allocated: {report:?}");
}

/// Deterministic structurally-valid frame over `payload`: the REAL A04
/// encoder (`CgapFrameHeader::encode_into`) with an A17 ROOT attestation
/// and the REAL BLAKE3 kernel digest (`cgap_blake3::hash`). Test material
/// built through the consumed lanes' own public code paths — no
/// re-encoding of the layout in this file.
fn valid_frame(payload: &[u8]) -> Vec<u8> {
    let attestation = P28LicenseAttestation {
        license_magic: LICENSE_MAGIC,
        version: 1,
        attestation_flags: LICENSE_FLAG_ROOT,
        safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
        origin_authority_digest: blake3_hash(b"a20 verdict lane origin authority"),
        upstream_receipt: [0u8; 32], // ROOT: zero receipt (A17 §2.2)
    };
    let header = CgapFrameHeader {
        magic: FRAME_MAGIC,
        version: FRAME_VERSION,
        flags: FLAG_BN3,
        reserved: [0; 2],
        payload_byte_len: payload.len() as u64,
        oabi_digest: blake3_hash(payload),
        attestation,
    };
    let mut frame = vec![0u8; FRAME_HEADER_LEN + payload.len()];
    header
        .encode_into(&mut frame)
        .expect("frame buffer is exactly header + payload");
    frame[FRAME_HEADER_LEN..].copy_from_slice(payload);
    frame
}

// ---------------------------------------------------------------------------
// Hex helpers (§5.1 receipt pin). They allocate (String) and are therefore
// never called inside a tracked alloc scope.
// ---------------------------------------------------------------------------

/// FIPS 180-4 known-answer court for the hand-rolled hash engine plus the
/// streaming identity (byte-at-a-time == one-shot across block boundaries).
/// Anti-vacuity: any change to the compression function, the K constants,
/// or the padding flips a vector. Cross-checked this session against 13
/// Python-hashlib vectors before the corpus receipt was pinned.
#[test]
fn sha256_known_answers() {
    let cases: [(&str, &str); 3] = [
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            hex32(&Sha256::compute(input.as_bytes())),
            expected,
            "FIPS 180-4 vector failed for input len {}",
            input.len()
        );
    }
    // Streaming identity across block boundaries (55/56/57, 63/64/65 ...).
    for len in [1usize, 54, 55, 56, 57, 63, 64, 65, 119, 120, 200, 1000] {
        let data: Vec<u8> = (0..len).map(|i| (i % 251 + 7) as u8).collect();
        let mut one = Sha256::new();
        one.update(&data);
        let one_shot = one.finish();
        let mut byte_at_a_time = Sha256::new();
        for b in &data {
            byte_at_a_time.update(std::slice::from_ref(b));
        }
        assert_eq!(
            byte_at_a_time.finish(),
            one_shot,
            "streaming identity broke at len {len}"
        );
    }
}

fn hex32(digest: &[u8; 32]) -> String {
    let mut s = String::with_capacity(SHA256_HEX_LEN);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Decode a pinned 64-hex-char constant to 32 bytes; a malformed pin is a
/// court failure (fail-closed), never a silent zero digest.
fn decode_hex_32(s: &str) -> [u8; 32] {
    let bytes = s.as_bytes();
    assert_eq!(
        bytes.len(),
        SHA256_HEX_LEN,
        "pinned receipt must be exactly 64 hex chars"
    );
    let mut out = [0u8; SHA256_BYTES];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = (hex_val(bytes[2 * i]) << 4) | hex_val(bytes[2 * i + 1]);
    }
    out
}

fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex digit in pinned receipt"),
    }
}

// ---------------------------------------------------------------------------
// Wave 9 — cross-lane integration: the A04 identity court over real tiers.
// Consumes the sealed A04 surface through its public API only
// (`cgap_core::isomorphism::identity_assert` / `Tier` / `TierMismatch` per
// A04 §8.1/§8.2, and the REAL memory-tier collaborator
// `cgap_core::dma_ring::DmaRing` per A04 §7.3), over corpus-derived frames.
// The executed court of record for lane A04 remains
// `crates/cgap-core/tests/identity_court.rs`; this is the verdict lane's
// cross-lane consumption of that surface (§3 C4 context).
//
// Anti-vacuity, per section (each names the mutation that flips it):
// - fail-line receipt: a TierMismatch Display change (wording, tier token,
//   hex width) flips the exact-line assert.
// - wire round trip: any CgapFrameHeader encode/decode asymmetry (a field
//   not re-encoded, `reserved` not zeroed on write, a byte-order change)
//   breaks re-encode identity and/or digest re-derivation.
// - memory tier: a DmaRing that loses/alters bytes on the claim→publish→
//   read path, or hands out a stale slot, breaks bit-for-bit identity or
//   the digest re-derivation per row.
// - refusal pins: a publish that accepts oversize frames (Overflow), a
//   read that yields garbage for no pending frame, or a release on empty
//   that succeeds — each flips its typed pin.
// - mutation witnesses: flipping one payload byte in a memory projection,
//   shifting `payload_byte_len` by +1, or truncating a projection each
//   MUST produce the typed Err naming tier+offset (A04 §8.3 rows) — these
//   prove the consumed detector actually detects; a weakened
//   identity_assert (early Ok, short compare) flips them.
// - 1M-row sweep: any per-row divergence (identity, digest, ring ordering)
//   fails the sweep; any allocation in the loop flips the zero-alloc pin.
// ---------------------------------------------------------------------------

use cgap_core::dma_ring::{DmaRing, RingError};
use cgap_core::isomorphism::{identity_assert, Tier, TierMismatch};

/// A04 §10 fail-closed region policy: `DmaRing::new` refuses a region that
/// does not start on a 64-byte boundary. `Box<AlignedRegion>` is 64-aligned
/// by the repr promise, so the leaked region satisfies the real refusal
/// gate rather than bypassing it (test-only heap; the crate source stays
/// alloc-free — A04's own source-scan gate enforces that in cgap-core).
#[repr(align(64))]
struct AlignedRegion<const N: usize>([u8; N]);

/// Leak an N-byte, 64-aligned, zeroed region for a real DmaRing. Leaked
/// (`&'static mut`) because the ring borrows the region for its lifetime;
/// the courts below create one ring per section, never resized.
fn leak_region<const N: usize>() -> &'static mut [u8] {
    let boxed = Box::leak(Box::new(AlignedRegion::<N>([0u8; N])));
    &mut boxed.0[..]
}

/// Wire frame geometry for one corpus row: 128 B header + one row encoding.
const A04_FRAME_BYTES: usize = FRAME_HEADER_LEN + ROW_ENCODING_BYTES;
/// Ring geometry: 2 slots × 256 B (slot holds the 182-byte frame with
/// zeroed padding; 256 % 64 == 0 satisfies the §10 slot-stride law).
const A04_REGION_BYTES: usize = 512;
const A04_SLOT_POW2: u8 = 1;

/// The A04 identity court: bit-for-bit frame identity across the wire and
/// memory tiers over corpus-derived frames (A04 §8.1/§8.2), with typed
/// refusal pins and in-test mutation witnesses. Disk tiers (Arrow/Parquet)
/// print a loud [SKIP] — 2-tier mode per A04 §10.
#[test]
fn a04_identity_court() {
    // -- §8.2 fail-line shape receipt (the consumed report type prints the
    //    doc line verbatim; a Display drift flips this exact-line assert) --
    let probe = TierMismatch {
        tier: Tier::Memory,
        offset: 8,
        wire: 0xAB,
        mem: 0xCD,
        disk: 0x00,
    };
    assert_eq!(
        probe.to_string(),
        "IDENTITY_COURT_FAIL tier=memory offset=8 wire=0xab mem=0xcd disk=0x00",
        "§8.2 fail-loud line drifted"
    );

    // -- Wire-tier round trip over corpus rows (real encode path, first 16
    //    rows): encode → decode → validate → re-encode → identity, plus the
    //    digest re-derivation obligation per tier (§8.1). --------------------
    let rows = corpus();
    let attestation = P28LicenseAttestation {
        license_magic: LICENSE_MAGIC,
        version: 1,
        attestation_flags: LICENSE_FLAG_ROOT,
        safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
        origin_authority_digest: blake3_hash(b"a20 verdict lane origin authority"),
        upstream_receipt: [0u8; 32], // ROOT: zero receipt (A17 §2.2)
    };
    for row in rows.iter().take(16) {
        let mut payload = [0u8; ROW_ENCODING_BYTES];
        encode_row(row, &mut payload);
        let header = CgapFrameHeader {
            magic: FRAME_MAGIC,
            version: FRAME_VERSION,
            flags: FLAG_BN3,
            reserved: [0; 2],
            payload_byte_len: ROW_ENCODING_BYTES as u64,
            oabi_digest: blake3_hash(&payload),
            attestation,
        };
        let mut wire = [0u8; A04_FRAME_BYTES];
        header
            .encode_into(&mut wire)
            .expect("wire buffer is header + payload");
        wire[FRAME_HEADER_LEN..].copy_from_slice(&payload);

        let decoded = CgapFrameHeader::decode(&wire).expect("decode own wire");
        assert_eq!(decoded.validate(), Ok(()), "wire tier must validate");
        let mut reencoded = [0u8; FRAME_HEADER_LEN];
        decoded
            .encode_into(&mut reencoded)
            .expect("re-encode header");
        identity_assert(&wire[..FRAME_HEADER_LEN], Tier::Wire, &reencoded)
            .expect("wire round trip is not bit-for-bit identical");
        decoded
            .verify_payload(&payload, blake3_hash)
            .expect("wire digest re-derivation");
    }

    // -- Memory tier over the SAME rows through the REAL DmaRing
    //    (claim → fill → publish → read → re-encode → identity → release),
    //    preceded by the collaborator's typed refusal pins. -------------------
    let ring_region = leak_region::<A04_REGION_BYTES>();
    let mut ring = DmaRing::new(ring_region, A04_SLOT_POW2)
        .expect("512 B / 2 slots = 256 B slots, 64-aligned");
    assert_eq!(ring.capacity(), 2);
    assert_eq!(ring.slot_size(), 256);

    // Refusal pins (fail-closed collaborator surface, A04 §7.3/§8.3):
    // read with nothing pending → None (never garbage);
    // release with nothing pending → typed ReleaseOnEmpty;
    // publish with no live claim → typed PublishWithoutClaim;
    // publish beyond slot capacity → typed Overflow.
    assert_eq!(ring.read(), None, "unpublished read must be None");
    assert_eq!(
        ring.release(),
        Err(RingError::ReleaseOnEmpty),
        "release on empty must refuse typed"
    );
    assert_eq!(
        ring.publish(FRAME_HEADER_LEN),
        Err(RingError::PublishWithoutClaim),
        "publish without claim must refuse typed"
    );
    {
        let slot = ring.claim().expect("ring has room");
        assert!(slot.len() >= A04_FRAME_BYTES);
        slot[..3].copy_from_slice(&[0xDE, 0xAD, 0xBE]);
    }
    assert_eq!(
        ring.publish(A04_REGION_BYTES / 2 + 1),
        Err(RingError::Overflow),
        "oversize publish must refuse typed (257 > 256)"
    );

    // Content-carrying rows: identity across tiers per row.
    let mut mem_frame = [0u8; A04_FRAME_BYTES];
    for row in rows.iter().take(16) {
        let mut payload = [0u8; ROW_ENCODING_BYTES];
        encode_row(row, &mut payload);
        let header = CgapFrameHeader {
            magic: FRAME_MAGIC,
            version: FRAME_VERSION,
            flags: FLAG_BN3,
            reserved: [0; 2],
            payload_byte_len: ROW_ENCODING_BYTES as u64,
            oabi_digest: blake3_hash(&payload),
            attestation,
        };
        let mut wire = [0u8; A04_FRAME_BYTES];
        header
            .encode_into(&mut wire)
            .expect("wire buffer is header + payload");
        wire[FRAME_HEADER_LEN..].copy_from_slice(&payload);

        {
            let slot = ring.claim().expect("ring has room");
            slot[..A04_FRAME_BYTES].copy_from_slice(&wire);
        }
        ring.publish(A04_FRAME_BYTES)
            .expect("publish valid frame");
        let (payload_m, header_m) = ring.read().expect("published frame pending");
        assert_eq!(header_m.validate(), Ok(()), "mem tier must validate");
        header_m
            .encode_into(&mut mem_frame[..FRAME_HEADER_LEN])
            .expect("re-encode mem header");
        mem_frame[FRAME_HEADER_LEN..].copy_from_slice(payload_m);
        identity_assert(&wire, Tier::Memory, &mem_frame)
            .expect("memory tier diverged from wire");
        header_m
            .verify_payload(payload_m, blake3_hash)
            .expect("memory digest re-derivation");
        ring.release().expect("release");
        assert_eq!(ring.pending(), 0, "ring must drain per row");
    }

    // -- In-test mutation witnesses (A04 §8.3 rows; prove the consumed
    //    detector detects — a weakened identity_assert flips these): ------
    // (1) flip one payload byte in the memory projection → Err naming
    //     tier=memory, offset 133 (128+5), with the per-tier bytes.
    let mut payload_w = [0u8; ROW_ENCODING_BYTES];
    encode_row(&rows[0], &mut payload_w);
    let header_w = CgapFrameHeader {
        magic: FRAME_MAGIC,
        version: FRAME_VERSION,
        flags: FLAG_BN3,
        reserved: [0; 2],
        payload_byte_len: ROW_ENCODING_BYTES as u64,
        oabi_digest: blake3_hash(&payload_w),
        attestation,
    };
    let mut wire_w = [0u8; A04_FRAME_BYTES];
    header_w.encode_into(&mut wire_w).expect("encode witness wire");
    wire_w[FRAME_HEADER_LEN..].copy_from_slice(&payload_w);
    let mut mem_w = wire_w;
    mem_w[133] ^= 0x01;
    let m = identity_assert(&wire_w, Tier::Memory, &mem_w).expect_err("flip must be detected");
    assert_eq!(m.tier, Tier::Memory);
    assert_eq!(m.offset, 133, "first divergent byte is payload byte 5");
    assert_eq!(m.wire, wire_w[133]);
    assert_eq!(m.mem, wire_w[133] ^ 0x01);
    assert!(m
        .to_string()
        .starts_with("IDENTITY_COURT_FAIL tier=memory offset=133 "), "{m}");

    // (2) `payload_byte_len` +1 → Err at offset 8 (the u64 LE field's LSB;
    //     A04 §8.3 "shift payload_byte_len by +1 → FAILs at offset 8").
    let mut header_shift = header_w;
    header_shift.payload_byte_len += 1;
    let mut wire_shift = [0u8; A04_FRAME_BYTES];
    header_shift
        .encode_into(&mut wire_shift)
        .expect("encode shifted wire");
    wire_shift[FRAME_HEADER_LEN..].copy_from_slice(&payload_w);
    let m = identity_assert(&wire_w, Tier::Memory, &wire_shift)
        .expect_err("len shift must be detected");
    assert_eq!(m.tier, Tier::Memory);
    assert_eq!(m.offset, 8, "first divergent byte is payload_byte_len LSB");
    assert_ne!(m.wire, m.mem);

    // (3) truncated projection → Err naming the prefix end, never a quiet
    //     pass over a short region.
    let m = identity_assert(&wire_w, Tier::Arrow, &wire_w[..wire_w.len() - 1])
        .expect_err("truncation must be detected");
    assert_eq!(m.tier, Tier::Arrow);
    assert_eq!(m.offset, wire_w.len() - 1);

    // (4) digest obligation on the memory tier: one flipped payload byte
    //     must be a typed DigestMismatch (the digest re-derivation IS an
    //     identity obligation per tier, §8.1). The flip is a DIFFERENT byte
    //     than witness (1)'s — un-flipping 133 would forge Ok(()) and the
    //     witness would assert nothing.
    let mut tampered = mem_w;
    tampered[134] ^= 0x01;
    let hdr_m = CgapFrameHeader::decode(&mem_w).expect("decode mem");
    assert_eq!(
        hdr_m.verify_payload(&tampered[FRAME_HEADER_LEN..], blake3_hash),
        Err(FrameError::DigestMismatch),
        "memory-tier tampering must refuse typed"
    );

    // -- §5 scale: 1M-row identity sweep — every corpus row's frame through
    //    the real ring, identity asserted per row, allocation-free. The
    //    template frame is built once through the REAL encode path; the
    //    per-row digest (real BLAKE3 over the row's canonical encoding) is
    //    patched into the header bytes, keeping every row's wire internally
    //    consistent (publish validates structure; the digest obligation is
    //    asserted per row below). Disk tiers: loud [SKIP], never silent. --
    println!(
        "[SKIP] tier=arrow reason=disk-tier view owned by lane A17; \
         2-tier mode per A04 §10"
    );
    // tier=parquet [SKIP] RETIRED (seal lane, GATE-3 repair): the 1M-row
    // corpus now crosses the real disk tier — committed through the frozen
    // cgap_commit_parquet signature and verified back through the real
    // post-commit read-back check — in `a19_seal_1m_ledger_court` below.
    println!(
        "tier=parquet: retired [SKIP] — 1M-row disk persistence exercised \
         by a19_seal_1m_ledger_court (frozen cgap_commit_parquet / \
         cgap_verify_p28 + footer read-back + decode)"
    );

    let sweep_region = leak_region::<A04_REGION_BYTES>();
    let mut sweep_ring = DmaRing::new(sweep_region, A04_SLOT_POW2)
        .expect("sweep ring construction");
    let mut template = [0u8; A04_FRAME_BYTES];
    let template_header = CgapFrameHeader {
        magic: FRAME_MAGIC,
        version: FRAME_VERSION,
        flags: FLAG_BN3,
        reserved: [0; 2],
        payload_byte_len: ROW_ENCODING_BYTES as u64,
        oabi_digest: [0u8; 32], // patched per row with the real digest
        attestation,
    };
    template_header
        .encode_into(&mut template)
        .expect("template encode");
    let mut row_enc = [0u8; ROW_ENCODING_BYTES];
    let mut mem_row = [0u8; A04_FRAME_BYTES];
    let (verified, report) = cgap_audit::alloc_counter::with_alloc_tracking(|| {
        let mut verified = 0u64;
        for row in rows.iter() {
            encode_row(row, &mut row_enc);
            template[16..48].copy_from_slice(&blake3_hash(&row_enc));
            template[FRAME_HEADER_LEN..].copy_from_slice(&row_enc);

            {
                let slot = sweep_ring.claim().expect("ring has room");
                slot[..A04_FRAME_BYTES].copy_from_slice(&template);
            }
            sweep_ring
                .publish(A04_FRAME_BYTES)
                .expect("sweep publish");
            let (payload_m, header_m) =
                sweep_ring.read().expect("sweep frame pending");
            header_m
                .encode_into(&mut mem_row[..FRAME_HEADER_LEN])
                .expect("sweep re-encode");
            mem_row[FRAME_HEADER_LEN..].copy_from_slice(payload_m);
            identity_assert(&template, Tier::Memory, &mem_row)
                .expect("sweep row identity broke");
            header_m
                .verify_payload(payload_m, blake3_hash)
                .expect("sweep digest re-derivation");
            sweep_ring.release().expect("sweep release");
            verified += 1;
        }
        verified
    });
    assert_eq!(
        verified, CORPUS_ROWS,
        "§5 scale: every corpus row must cross both tiers"
    );
    assert!(report.is_zero(), "1M-row identity sweep allocated: {report:?}");
}

// ===========================================================================
// SEAL (GATE-3) — one-million-row sovereign ledger harness (seal lane).
//
// The coordinates the gate named (gate evidence: exit=3 standing=PARTIAL):
//   1. tests/court_verdict.rs [SKIP] tier=parquet (a04_identity_court) —
//      parquet persistence of the 1M corpus unexercised. RETIRED above:
//      the corpus is committed through the frozen cgap_commit_parquet
//      signature to a real file and verified back from disk here.
//   2. crates/cgap-ffi/src/sovereign.rs CgapCommitParquetFn (:183) /
//      CgapVerifyP28Fn (:194) — invoked by no test in the repo. Both are
//      invoked THROUGH the frozen aliases below: coercing the court's
//      bodies to the aliases is the compile-time signature pin, and the
//      calls go through the fn pointers, never the fn items directly.
//
// Chicago discipline: every collaborator is the real owning-lane kernel —
// cgap_gate::penstock::ingress (A14 admission provenance over all 1M rows),
// cgap_blake3 (per-row digests + the P28 digest predicate), cgap-parquet
// StaticPageWriter/FooterSealer/verify_seal/rle_decode (A02+A03 persistence
// + post-commit read-back + decode), cgap-p28 attestation predicates (A17),
// cgap-crypto keyed receipt sealing under a real Lease (A16). No mocks;
// final state asserted: file bytes on disk, verify verdicts, decoded lane
// values, receipt wire round trip.
//
// Anti-vacuity, per leg (each names the mutation that flips it):
// - generator drift: corpus() asserts the pinned §5.1 sha256 receipt before
//   any leg runs (corpus_determinism_receipt's committed SHIM class), so a
//   generator mutation flips this court before its own asserts.
// - commit/read-back: one flipped BODY byte in a real file copy flips
//   verify_seal to ChecksumMismatch{region: Body}; one flipped FOOTER/META
//   byte flips it to ChecksumMismatch{region: Footer}; a wrong content
//   handle flips it to DigestMismatch. A commit that wrote nothing, a
//   partial flush, or a mis-addressed block cannot pass.
// - lane identity: rle_decode over the committed page bodies, read back
//   from DISK, must reproduce the derived lanes exactly for the first AND
//   last row groups; any encode/decode drift or offset shift flips it.
// - determinism: two commits of the same batch through the frozen ABI
//   produce byte-identical files (any nondeterminism flips).
// - receipt: a flipped block-digest byte in the delta flips
//   verify_unsealed to DigestMismatch.
// - verify_p28: every A17 §5 field law (magic, version, lineage zero/two
//   bits, reserved bit, ceiling, ROOT-with-receipt, DERIVATIVE-no-receipt)
//   plus truncated (79 B) and extended (81 B) records flip to
//   CGAP_ERR_P28_SPOLIATION (A17 §4.2: a non-80-byte input collapses to
//   the §5 spoliation class).
// ===========================================================================

mod a19_seal {
    use super::{
        blake3_hash, corpus, encode_row, ingress, CgapFrameHeader, CGAP_ERR_INVALID_MAGIC,
        CGAP_ERR_P28_SPOLIATION, CGAP_OK, CORPUS_ROWS, FLAG_BN3, FRAME_HEADER_LEN, FRAME_MAGIC,
        FRAME_VERSION, P28LicenseAttestation, ROW_ENCODING_BYTES, SAFE_HARBOR_CEILING_CENTS_MAX,
    };
    use cgap_crypto::chatman::{
        admissions_root, seal_unsealed, verify_unsealed, DeltaS, UnsealedReceipt, VerifyError,
    };
    use cgap_crypto::lease::Lease;
    use cgap_ffi::sovereign::{CgapCommitParquetFn, CgapVerifyP28Fn};
    use cgap_p28::attestation::{
        P28LicenseAttestation as SealAttestation, ATTESTATION_VERSION, FLAG_DERIVATIVE,
        FLAG_ROOT, LICENSE_MAGIC as ATTEST_MAGIC, SAFE_HARBOR_CEILING_MAX_CENTS,
    };
    use cgap_parquet::footer::{block_digest, verify_seal, CorruptRegion, SealError};
    use cgap_parquet::mmap_writer::{FileSink, SlabView, StaticPageWriter};
    use cgap_parquet::rle::{rle_decode, rle_encode};
    use std::ffi::CStr;

    /// Row groups in the sealed block: 8 × 125,000 = 1,000,000 rows
    /// (MAX_ROW_GROUPS is 64 — 8 is well inside the writer's capacity).
    const SEAL_ROW_GROUPS: usize = 8;
    const SEAL_ROWS_PER_GROUP: u32 = (CORPUS_ROWS / SEAL_ROW_GROUPS as u64) as u32;
    /// Tier B buffer for the 1M-row block (body ≈ 1.0 MB + footer ≈ 150 B).
    const SEAL_BLOCK_CAP: usize = 4 * 1024 * 1024;

    /// The committed lane for one corpus row: a deterministic fold of the
    /// row's content (notional ^ instrument_id, byte 3), kept inside the
    /// hybrid packer's kernel width (bits = 8 → values < 256) so the
    /// commit → read-back is lossless for this batch.
    fn lane_of(row: &super::SettlementRow) -> u32 {
        (((row.notional ^ row.instrument_id) >> 3) & 0xFF) as u32
    }

    /// The ROOT P28 attestation sealing the ledger, built through BOTH
    /// pinned A17 mirrors — cgap_core::frame (the A04 frame view) and
    /// cgap_p28::attestation (the A17 canonical record). The parity leg
    /// below asserts the two encodings are byte-identical, so the seal
    /// covers exactly one record, whichever mirror a consumer reads.
    fn seal_attestation() -> [u8; 80] {
        let origin = blake3_hash(b"cgap seal lane origin authority v1");
        let core_att = P28LicenseAttestation {
            license_magic: super::LICENSE_MAGIC,
            version: 1,
            attestation_flags: super::LICENSE_FLAG_ROOT,
            safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
            origin_authority_digest: origin,
            upstream_receipt: [0u8; 32], // ROOT: zero receipt (A17 §2.2)
        };
        let seal_att = SealAttestation {
            license_magic: ATTEST_MAGIC,
            version: ATTESTATION_VERSION,
            attestation_flags: FLAG_ROOT,
            safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_MAX_CENTS,
            origin_authority_digest: origin,
            upstream_receipt: [0u8; 32],
        };
        // Mirror parity + the frozen pin constants agree across crates.
        assert_eq!(super::LICENSE_MAGIC, ATTEST_MAGIC, "P28S magic drift");
        assert_eq!(FLAG_ROOT, super::LICENSE_FLAG_ROOT, "ROOT bit drift");
        let mut wire = [0u8; 80];
        core_att.encode_into(&mut wire).expect("80 B out buffer");
        assert_eq!(wire, seal_att.to_bytes(), "A17 mirror encodings diverged");
        wire
    }

    /// The court's `cgap_commit_parquet` body: the A02+A03 owner kernels
    /// (StaticPageWriter → FooterSealer → Tier H FileSink) composed behind
    /// the FROZEN signature. Host-only allocation (A19 §2): one reusable
    /// u32 chunk buffer + the block buffer + the sealer's metadata Vec.
    /// Null pointer / empty batch → the membrane's INVALID_MAGIC overload
    /// (the penstock row-1 class: "not a committable batch").
    unsafe extern "C" fn seal_commit_parquet(
        lanes: *const i32,
        lane_count: usize,
        path: *const core::ffi::c_char,
    ) -> u32 {
        if lanes.is_null() || path.is_null() || lane_count == 0 {
            return CGAP_ERR_INVALID_MAGIC;
        }
        let path = CStr::from_ptr(path)
            .to_str()
            .expect("NUL-terminated UTF-8 path (A19 §2 caller contract)");
        let file = std::fs::File::create(path).expect("create ledger file");
        let mut buf = vec![0u8; SEAL_BLOCK_CAP];
        let mut writer = StaticPageWriter::new(&mut buf);
        let mut chunk: Vec<u32> = Vec::with_capacity(SEAL_ROWS_PER_GROUP as usize);
        let batch = core::slice::from_raw_parts(lanes, lane_count);
        let mut start = 0usize;
        while start < lane_count {
            let end = (start + SEAL_ROWS_PER_GROUP as usize).min(lane_count);
            chunk.clear();
            chunk.extend(batch[start..end].iter().map(|&v| (v as u32) & 0xFF));
            writer
                .write_slab(&SlabView {
                    lanes: &[chunk.as_slice()],
                    rows: (end - start) as u32,
                    bits: 8,
                })
                .expect("seal block buffer sized for 8 row groups of 1M rows");
            start = end;
        }
        let sealed = writer
            .seal(Some(&seal_attestation()))
            .expect("seal the 1M-row ledger block");
        let scratch = vec![0u8; SEAL_BLOCK_CAP].into_boxed_slice();
        sealed
            .flush_to(FileSink::new(file, scratch))
            .expect("Tier H file commit of the sealed block");
        CGAP_OK
    }

    /// The court's `cgap_verify_p28` body: the A17 owner kernel's two
    /// predicates behind the FROZEN signature — parse (`from_bytes`, §4.2:
    /// any non-80-byte input is the §5 spoliation class) + license
    /// predicate (`validate_intrinsic`, §5.2-§5.5/§5.8) — then the digest
    /// predicate (real BLAKE3 over the evidence, first word LE). Read-only,
    /// allocation-free (A19 §2).
    unsafe extern "C" fn seal_verify_p28(
        evidence: *const u8,
        len: usize,
        out_digest: *mut u64,
    ) -> u32 {
        if evidence.is_null() {
            return CGAP_ERR_INVALID_MAGIC;
        }
        let ev = core::slice::from_raw_parts(evidence, len);
        let Ok(record) = SealAttestation::from_bytes(ev) else {
            return CGAP_ERR_P28_SPOLIATION;
        };
        if record.validate_intrinsic().is_err() {
            return CGAP_ERR_P28_SPOLIATION;
        }
        if !out_digest.is_null() {
            let digest = blake3_hash(ev);
            *out_digest = u64::from_le_bytes(digest[0..8].try_into().expect("8 of 32 B"));
        }
        CGAP_OK
    }

    /// THE SEAL COURT: the 1M-row sovereign ledger, end to end —
    /// admit (A14) → lanes (pinned corpus) → commit (frozen
    /// cgap_commit_parquet ABI) → disk read-back + verify_seal + decode →
    /// keyed receipt (cgap-crypto) → verify (frozen cgap_verify_p28 ABI).
    #[test]
    fn a19_seal_1m_ledger_court() {
        // -- 1. Deterministic seeded corpus (pinned receipt asserted inside
        //       corpus() before any leg of this court runs). -----------------
        let rows = corpus();
        assert_eq!(rows.len() as u64, CORPUS_ROWS);

        // -- 2. Committed lanes: the deterministic content fold over all 1M
        //       rows, inside the packer's kernel width. The histogram floor
        //       binds the fold to corpus content (a constant fold flips). ---
        let lanes: Vec<i32> = rows.iter().map(|r| lane_of(r) as i32).collect();
        assert_eq!(lanes.len() as u64, CORPUS_ROWS);
        assert!(
            lanes.iter().all(|&v| (0..=255).contains(&v)),
            "lanes must fit the declared 8-bit page width"
        );
        let mut seen = [0u64; 256];
        for &v in &lanes {
            seen[v as usize] += 1;
        }
        let distinct = seen.iter().filter(|&&c| c > 0).count();
        assert!(
            distinct >= 128,
            "lane fold degenerate: only {distinct}/256 buckets populated"
        );

        // -- 3. A14 admission provenance: the frame of every committed row
        //       through the REAL ingress boundary; all 1M must be CGAP_OK,
        //       allocation-free. A removed/reordered penstock check flips
        //       the per-row pin. ------------------------------------------------
        let frame_attestation = P28LicenseAttestation {
            license_magic: super::LICENSE_MAGIC,
            version: 1,
            attestation_flags: super::LICENSE_FLAG_ROOT,
            safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
            origin_authority_digest: blake3_hash(b"cgap seal lane origin authority v1"),
            upstream_receipt: [0u8; 32],
        };
        let template_header = CgapFrameHeader {
            magic: FRAME_MAGIC,
            version: FRAME_VERSION,
            flags: FLAG_BN3,
            reserved: [0; 2],
            payload_byte_len: ROW_ENCODING_BYTES as u64,
            oabi_digest: [0u8; 32], // patched per row with the real digest
            attestation: frame_attestation,
        };
        let mut template = vec![0u8; FRAME_HEADER_LEN + ROW_ENCODING_BYTES];
        template_header
            .encode_into(&mut template)
            .expect("template frame encode");
        let (admitted, report) =
            cgap_audit::alloc_counter::with_alloc_tracking(|| {
                let mut admitted = 0u64;
                let mut row_enc = [0u8; ROW_ENCODING_BYTES];
                for row in rows.iter() {
                    encode_row(row, &mut row_enc);
                    template[16..48].copy_from_slice(&blake3_hash(&row_enc));
                    assert_eq!(
                        ingress(&template, template.len(), None),
                        CGAP_OK,
                        "every committed row's frame must be admitted"
                    );
                    admitted += 1;
                }
                admitted
            });
        assert_eq!(admitted, CORPUS_ROWS, "provenance: all 1M frames admitted");
        assert!(report.is_zero(), "provenance sweep allocated: {report:?}");

        // -- 4. COMMIT through the frozen cgap_commit_parquet signature.
        //       The coercion below is the compile-time signature pin; the
        //       call goes through the fn pointer (the frozen ABI itself). ---
        let ledger_path = std::env::temp_dir()
            .join(format!("cgap-seal-gate3-{}.par2", std::process::id()));
        let _ = std::fs::remove_file(&ledger_path);
        let ledger_cpath = format!("{}\0", ledger_path.display());
        let commit: CgapCommitParquetFn = seal_commit_parquet;
        let status =
            unsafe { commit(lanes.as_ptr(), lanes.len(), ledger_cpath.as_ptr().cast()) };
        assert_eq!(
            status, CGAP_OK,
            "cgap_commit_parquet refused the 1M-row admitted batch"
        );

        // -- 5. Disk read-back: the committed file is the deterministic
        //       block. A second commit through the SAME ABI must be
        //       byte-identical (determinism), and the file must equal it. ---
        let twin_path = std::env::temp_dir()
            .join(format!("cgap-seal-gate3-twin-{}.par2", std::process::id()));
        let _ = std::fs::remove_file(&twin_path);
        let twin_cpath = format!("{}\0", twin_path.display());
        let status = unsafe { commit(lanes.as_ptr(), lanes.len(), twin_cpath.as_ptr().cast()) };
        assert_eq!(status, CGAP_OK, "twin commit refused");
        let file_bytes = std::fs::read(&ledger_path).expect("read committed ledger");
        let twin_bytes = std::fs::read(&twin_path).expect("read twin ledger");
        assert_eq!(
            file_bytes, twin_bytes,
            "two commits of the same batch diverged — nondeterministic block"
        );
        assert!(
            file_bytes.len() > CORPUS_ROWS as usize / 2,
            "block implausibly small for 1M rows: {} B",
            file_bytes.len()
        );

        // -- 6. The real post-commit read-back check (A05/A03 contract) over
        //       the FILE bytes. The content address is recomputed from the
        //       file itself: the trailer is checksum(8)+magic(4)+len(4), so
        //       content [0, B+M+80) ends 16 bytes before EOF. -----------------
        let l = file_bytes.len();
        assert!(l > 16 + 96 + 80, "sealed block trailer geometry");
        let content_end = l - 16;
        let digest = block_digest(&file_bytes[..content_end]);
        let verified =
            verify_seal(&file_bytes, &digest).expect("committed ledger must verify from disk");
        assert_eq!(verified.rows, CORPUS_ROWS, "footer rows == 1M");
        assert_eq!(
            verified.row_groups as usize, SEAL_ROW_GROUPS,
            "footer row groups"
        );
        assert_eq!(verified.spans_len, SEAL_ROW_GROUPS, "span table");
        assert!(verified.attestation_present, "P28 attestation in the seal");
        assert_eq!(
            verified.body_len + verified.meta_len + 80,
            content_end,
            "region arithmetic (B + M + 80 + 16 == file length)"
        );

        // -- 6b. The attestation slot in the FILE carries the sealing record
        //        byte-for-byte (the seal is over THIS ledger). ----------------
        let attest_bytes = seal_attestation();
        let in_file = &file_bytes
            [verified.body_len + verified.meta_len..verified.body_len + verified.meta_len + 80];
        assert_eq!(in_file, &attest_bytes[..], "seal attestation slot");

        // -- 6c. Wrong content handle → DigestMismatch (the A03 wrong-object
        //        case must flip; a verify that ignores the handle is vacuous).
        assert_eq!(
            verify_seal(&file_bytes, &[0u8; 32]),
            Err(SealError::DigestMismatch),
            "zero handle must be refused"
        );

        // -- 7. MUTATION LEGS on real disk copies (never the shared tree's
        //        subjects — the copies live in /tmp and are removed below). --
        // 7a. One flipped BODY byte → ChecksumMismatch{region: Body}: the
        //     checksum localizes the corruption to the row data. A verify
        //     that rubber-stamps the 1M-row body cannot pass this leg.
        let mutant_body_path = std::env::temp_dir().join(format!(
            "cgap-seal-gate3-bodyflip-{}.par2",
            std::process::id()
        ));
        let mut body_flip = file_bytes.clone();
        body_flip[verified.body_len / 2] ^= 0x01;
        std::fs::write(&mutant_body_path, &body_flip).expect("write body-flip copy");
        let read_back = std::fs::read(&mutant_body_path).expect("read body-flip copy");
        assert_eq!(read_back, body_flip, "mutation must reach disk");
        assert_eq!(
            verify_seal(&read_back, &digest),
            Err(SealError::ChecksumMismatch {
                region: CorruptRegion::Body
            }),
            "one flipped body byte must flip verify_seal on the BODY region"
        );
        // 7b. One flipped byte in the seal's ATTESTATION SLOT (footer region)
        //     → ChecksumMismatch{region: Footer}. The verifier's verdict
        //     order is shape → attestation → checksum → digest, so a meta-
        //     STRUCTURE byte flip lands on Shape(MetaMalformed) (witnessed
        //     this session); the attestation slot parses clean and localizes
        //     the checksum failure to the footer instead.
        let mut footer_flip = file_bytes.clone();
        footer_flip[verified.body_len + verified.meta_len] ^= 0x01;
        assert_eq!(
            verify_seal(&footer_flip, &digest),
            Err(SealError::ChecksumMismatch {
                region: CorruptRegion::Footer
            }),
            "one flipped attestation-slot byte must flip verify_seal on the FOOTER region"
        );
        // 7c. The unmutated block still verifies after the mutation legs.
        assert!(
            verify_seal(&file_bytes, &digest).is_ok(),
            "mutation legs corrupted the shared subject"
        );

        // -- 8. Lane identity from DISK: the committed page bodies, decoded
        //        through the real decode kernel, reproduce the derived lanes
        //        exactly — first AND last row groups. Any encode/decode drift
        //        or offset shift flips this leg. --------------------------------
        let decode_group = |group: usize, chunk: &[i32]| {
            let expected = rle_encode(
                &chunk.iter().map(|&v| v as u32).collect::<Vec<u32>>(),
                8,
            );
            let span_start = if group == 0 {
                // First page body ends at span0; the header occupies the rest.
                let span0 = verified.spans[0] as usize;
                super::span_start_of(span0, &expected, &file_bytes)
            } else {
                // Last page body ends at body_len.
                verified.body_len - expected.len()
            };
            let mut decoded = vec![0u32; chunk.len()];
            rle_decode(&file_bytes[span_start..span_start + expected.len()], &mut decoded)
                .expect("decode committed page body");
            let round_trip: Vec<i32> = decoded.iter().map(|&v| v as i32).collect();
            assert_eq!(
                round_trip, chunk,
                "row group {group}: decoded lanes diverge from the committed batch"
            );
        };
        decode_group(
            0,
            &lanes[..SEAL_ROWS_PER_GROUP as usize],
        );
        decode_group(
            SEAL_ROW_GROUPS - 1,
            &lanes[lanes.len() - SEAL_ROWS_PER_GROUP as usize..],
        );

        // -- 9. Receipt sealing (cgap-crypto, A16): the committed block
        //        digest binds into a keyed delta under a REAL lease; the
        //        receipt round-trips its 229-B wire and a flipped digest
        //        byte flips verify to DigestMismatch. -------------------------
        let admissions = admissions_root(&[&attest_bytes[..]]);
        let mut lease = Lease::new([7u8; 32], 4, blake3_hash(b"cgap seal lease epoch v1"));
        let delta = DeltaS::new([0u8; 32], digest, CORPUS_ROWS, admissions);
        let receipt = seal_unsealed(delta, &mut lease);
        assert_eq!(lease.current_tick(), 1, "seal consumes exactly one tick");
        verify_unsealed(&receipt).expect("sealed receipt must verify");
        let receipt_bytes = receipt.to_bytes();
        let round_tripped = UnsealedReceipt::from_bytes(&receipt_bytes)
            .expect("receipt wire round trip");
        verify_unsealed(&round_tripped).expect("round-tripped receipt must verify");
        let flipped = {
            let mut d = digest;
            d[0] ^= 0x01;
            DeltaS::new([0u8; 32], d, CORPUS_ROWS, admissions)
        };
        // Tamper leg: a delta flipped BEFORE sealing would seal consistently
        // (the keyed digest covers whatever it is given), so the mutation
        // lands on the SEALED receipt's 229-B wire instead — one flipped
        // covered byte (the bound block digest at wire offset 45 = delta
        // to_root[0]), re-verified from the wire, must flip to
        // DigestMismatch.
        let _ = flipped;
        let mut tampered = receipt_bytes;
        tampered[45] ^= 0x01;
        let bad_receipt = UnsealedReceipt::from_bytes(&tampered)
            .expect("tampered wire still parses (domain tag intact)");
        assert!(matches!(
            verify_unsealed(&bad_receipt),
            Err(VerifyError::DigestMismatch)
        ), "one flipped covered byte must flip the receipt verify");
        let _ = bad_receipt;

        // -- 10. VERIFY through the frozen cgap_verify_p28 signature: the
        //        sealing record verifies green (with and without the digest
        //        slot), the SAME digest verifies from the FILE's attestation
        //        slot, and every A17 §5 field law plus truncated/extended
        //        records flip to CGAP_ERR_P28_SPOLIATION. ----------------------
        let verify: CgapVerifyP28Fn = seal_verify_p28;
        let expected_digest =
            u64::from_le_bytes(blake3_hash(&attest_bytes)[0..8].try_into().expect("8 B"));
        let mut digest_out: u64 = 0;
        let status = unsafe {
            verify(attest_bytes.as_ptr(), attest_bytes.len(), &mut digest_out)
        };
        assert_eq!(status, CGAP_OK, "valid ROOT record must verify");
        assert_eq!(
            digest_out, expected_digest,
            "digest predicate must be real BLAKE3 over the evidence"
        );
        let status = unsafe {
            verify(attest_bytes.as_ptr(), attest_bytes.len(), core::ptr::null_mut())
        };
        assert_eq!(status, CGAP_OK, "out_digest MAY be NULL (A19 §2)");
        // The SAME record read back from the COMMITTED FILE verifies with the
        // SAME digest — the ledger's seal is verifiable from disk alone.
        let mut disk_digest: u64 = 0;
        let status = unsafe { verify(in_file.as_ptr(), 80, &mut disk_digest) };
        assert_eq!(status, CGAP_OK, "seal attestation must verify from disk");
        assert_eq!(disk_digest, expected_digest, "disk digest drifted");
        // Harness guard leg: null evidence is the membrane overload, distinct
        // from the spoliation class (the court never passes null otherwise).
        let status = unsafe { verify(core::ptr::null(), 0, core::ptr::null_mut()) };
        assert_eq!(status, CGAP_ERR_INVALID_MAGIC, "null evidence guard");

        // Spoliation corpus (A17 §5 field laws — the a14 row-5 taxonomy
        // applied to the record itself): every mutation must flip the
        // frozen ABI to CGAP_ERR_P28_SPOLIATION. A verify_p28 that admits
        // any of these is a gate that cannot flip = VACUOUS.
        let spoliation_cases: [(&str, Box<dyn Fn(&mut [u8; 80])>); 8] = [
            ("att magic", Box::new(|b: &mut [u8; 80]| b[0] = b'X')),
            (
                "att version",
                Box::new(|b: &mut [u8; 80]| b[4..6].copy_from_slice(&2u16.to_le_bytes())),
            ),
            ("reserved bit", Box::new(|b: &mut [u8; 80]| b[6] |= 0x08)),
            ("lineage zero", Box::new(|b: &mut [u8; 80]| b[6] = 0)),
            ("lineage two bits", Box::new(|b: &mut [u8; 80]| b[6] = 0x03)),
            (
                "ceiling over max",
                Box::new(|b: &mut [u8; 80]| {
                    b[8..16].copy_from_slice(&(SAFE_HARBOR_CEILING_MAX_CENTS + 1).to_le_bytes())
                }),
            ),
            (
                "ROOT with receipt",
                Box::new(|b: &mut [u8; 80]| b[48..80].fill(1)),
            ),
            (
                "DERIVATIVE without receipt",
                Box::new(|b: &mut [u8; 80]| {
                    b[6..8].copy_from_slice(&FLAG_DERIVATIVE.to_le_bytes())
                }),
            ),
        ];
        for (name, mutate) in &spoliation_cases {
            let mut evidence = attest_bytes;
            mutate(&mut evidence);
            let mut slot: u64 = 0;
            let status = unsafe { verify(evidence.as_ptr(), 80, &mut slot) };
            assert_eq!(
                status, CGAP_ERR_P28_SPOLIATION,
                "spoliation case '{name}' must refuse through the frozen ABI"
            );
        }
        // The origin-authority digest is NOT an intrinsic license law (§5.6
        // recomputes it from instrument context the ABI does not carry —
        // witnessed this session: the flip ADMITS). The digest predicate
        // carries that detection: the record licenses, but the returned
        // digest MUST change (A19 §2's "digest + license predicate" split).
        let mut digest_flipped = attest_bytes;
        digest_flipped[16] ^= 0x01;
        let mut flipped_slot: u64 = 0;
        let status = unsafe { verify(digest_flipped.as_ptr(), 80, &mut flipped_slot) };
        assert_eq!(
            status, CGAP_OK,
            "origin-digest flip is not an intrinsic refusal (§5.6 needs context)"
        );
        assert_ne!(
            flipped_slot, expected_digest,
            "digest predicate must detect an evidence mutation"
        );
        // Truncated (79 B) and extended (81 B): §4.2 collapses both to the
        // §5 spoliation class at the record layer.
        let mut short = attest_bytes;
        let status = unsafe { verify(short.as_ptr(), 79, &mut digest_out) };
        assert_eq!(status, CGAP_ERR_P28_SPOLIATION, "79 B record");
        let mut long = [0u8; 81];
        long[..80].copy_from_slice(&attest_bytes);
        let status = unsafe { verify(long.as_ptr(), 81, &mut digest_out) };
        assert_eq!(status, CGAP_ERR_P28_SPOLIATION, "81 B record");
        let _ = &mut short;

        // -- 11. Lease law: the court's /tmp subjects are removed by their
        //        only leaseholder (this court). --------------------------------
        let _ = std::fs::remove_file(&ledger_path);
        let _ = std::fs::remove_file(&twin_path);
        let _ = std::fs::remove_file(&mutant_body_path);

        println!(
            "SEAL 1M LEDGER: admitted=1000000 (zero-alloc provenance) \
             committed={} B across {} row groups; verify_seal ok; \
             body/footer flips + wrong handle refused; lanes decoded from disk; \
             keyed receipt sealed+verified; verify_p28 green + 8 intrinsic \
             spoliation refusals + digest-predicate mutation pin + 79/81 B refused",
            file_bytes.len(),
            SEAL_ROW_GROUPS
        );
    }
}

/// Locate a first-row-group page body inside the committed bytes: the body
/// ends exactly at `span0`; assert the preceding `expected.len()` bytes
/// equal `expected` (the hybrid wire is byte-identical to rle_encode, the
/// crate's own differential pin) and return the body's start offset.
fn span_start_of(span0: usize, expected: &[u8], file_bytes: &[u8]) -> usize {
    let start = span0 - expected.len();
    assert_eq!(
        &file_bytes[start..span0],
        expected,
        "row group 0 page body != rle_encode wire (header len {start}?)"
    );
    start
}

// == end SEAL (GATE-3) section =============================================
