//! cgap-gate penstock — lane A14 (Wave 1 ABI freeze + Wave 2 core engine).
//!
//! Wave 1 slice: struct/const definitions + size/align static asserts
//! (unchanged from the ABI-freeze receipt; geometry below is frozen).
//! Wave 2 slice (this restart): the [`ingress`] check chain in lane doc
//! §4's normative order, the [`FrameView`] out-view, and the Chicago unit
//! courts (taxonomy rows 1-6, order pinning per §4.5, content
//! independence per §6, zero-alloc per §8.2, generator determinism per
//! §8.1) in the `#[cfg(test)]` module at the bottom of this file.
//! Wave 3 slice (this restart): full lane scope — the §8.4 ratio-fence
//! throughput court (null-check baseline, min-of-N paired rounds) added
//! below; the §8.3 anti-vacuity shims (magic-removal /
//! allocation-injection / order-shuffle) executed against this real file
//! with sha256-fail-closed restore; the DoD 10 status-purity grep court
//! run over the crate.
//!
//! Wave 4 slice (this restart): harden + close DoD. (a) The §8.4 fence is
//! now profile-honest: a debug-profile binary measures the compiler, not
//! the gate (dev witness 2026-11-01: 23.44 ns/frame for the 4-byte null
//! baseline, 3.74x ratio — unoptimized overhead dominates both sides), so
//! debug runs emit a loud `[SKIP]`. Under release, Wave 3's hard ratio
//! assert turned §8.4's advisory TARGET into a gate, contradicting §8.4's
//! own "throughput is a TARGET row in the receipt, never the gate"; the
//! fence is now a measuring court: the TARGET row (< 4 ns/frame AND <= 3x
//! baseline) is measured and reported with PASS/GAP verdict — measured
//! GAP, named, not rounded up (divergence 8) — and a 10x gross-regression
//! ceiling is enforced so a quadratic-reorder regression still flips the
//! court. (b) Process-counter zero-alloc witness (R3b): the
//! `#[global_allocator]` counters bracket a bare `ingress` call with no
//! TLS guard. (c) Cross-file drift courts: the frozen mirror is bound to
//! the canonical `cgap-core/src/frame.rs` pin table (R4) and the status
//! copies are bound value-identical to A19's register in
//! `cgap-ffi/src/sovereign.rs` (DoD 10) — the re-import/re-export forms
//! of DoD 2/10 stay blocked on the off-limits Cargo.toml (divergence 3).
//! (d) The three §8.3 anti-vacuity shims re-executed this session against
//! this file with sha256-fail-closed restore.
//!
//! Wave 5 slice (this restart): defect repair driven by the wave-1-4 court
//! receipts. The A20 wave-3 audit
//! (`docs/jira/v26.11.1/waves/a20-wave3-audit-receipt.md`, mutation M9)
//! witnessed the residual blind-spot class on the dam: a drifted status
//! constant is compared to itself (dam court expects `ingress`'s own
//! exported constant), so only a cross-crate pin court catches the drift.
//! Repair in this file, at the binding point: the status-purity court now
//! binds each of the gate's four codes THREE-WAY — gate mirror == the A19
//! register's NORMATIVE literal (the register of record, lane doc A19
//! §"C signatures (normative)") AND A19's ffi implementation == the same
//! literal — so any single-file or joint (copy-pasted) drift flips this
//! court locally, with no cross-crate court required. Wave-4's form bound
//! gate -> ffi file only and passed under joint drift. Also (ERRC
//! tighten): the verbatim-duplicated row-3a comment block left by the
//! Wave-2 edit is deleted.
//!
//! Wave 6 slice (this restart): defect repair driven by the wave-5 court
//! receipts, re-run discipline first. No wave-5 receipt names a penstock
//! defect (a03-wave5's F-1 is a dag.rs defect, a07-wave5's open items are
//! A07/A20-owned, a20-wave5's M9 was already repaired at the binding point
//! in the wave-5 slice above); what the receipts DO prescribe is the
//! re-run method, and re-running the suite this session caught a live
//! defect the receipts' method exists to catch: the wave-4
//! `process_counter_witness_single_call_zero_alloc` court false-FAILED
//! under the default parallel harness (bracketed delta (1, 59) — another
//! concurrent test's allocation inside a PROCESS-WIDE counter bracket;
//! reran PASS, isolated PASS — nondeterministic false positive). Repair
//! in this file: the witness now reads always-on PER-THREAD allocator-hook
//! counters (`THREAD_ALLOCS` / `THREAD_ALLOC_BYTES`), immune by
//! construction to concurrent tests, same teeth (admit + rows 2/3/4
//! bare-call brackets, renamed
//! `allocator_hook_witness_bare_call_zero_alloc`), plus an in-court
//! witness-liveness positive control (a deliberate allocation MUST move
//! the counters, else the witness is silent-zero — R3b, fail not fake).
//! The §8.3-named allocation-injection shim still has a court to flip:
//! the per-thread witness now flips deterministically under it (shim
//! executed in a sandbox this wave, receipt in the lane report).
//!
//! Wave 7 slice (this restart): defect repair driven by the wave-6 court
//! receipts (`a07-wave6`, `a16-wave6`). Neither names a penstock defect
//! (a07 §5's exit-gate RED is an A07/A09 emitter-classification item; its
//! open items are A05/A20/coordinator-owned; a16's D-items are A16-scoped
//! plus method), so the repair discipline this wave is the receipts'
//! re-run method, executed with no silent delta: full suite re-run both
//! profiles (dev 11/11 exit 0, 55.55 s; release 11/11 exit 0, 7.12 s;
//! `--no-fail-fast` per a16 D-A16-5), the §8.3 anti-vacuity matrix
//! re-executed against THIS file generation in a /tmp sandbox
//! (`/tmp/a14w7-sb1`, canonical sha256 `3f599a69…3903e` byte-exact before
//! and after, every restore sha256-verified): magic-removal FLIP (5
//! courts: witness, order-pin, dam corpus, taxonomy, content-independence
//! — the §8.3-predicted outright-admission class included), escaped
//! allocation-injection FLIP (exactly the two zero-alloc courts; escaped
//! form, dodging the dead-malloc-elision class a20-wave3 M4a witnessed),
//! order-shuffle FLIP (`order_pinning_first_failure_wins` alone, §4.5),
//! and the a20 M9 constant-drift class re-attacked (FLIP:
//! `status_register_is_pure_a19_subset` alone — the wave-5 NORMATIVE-literal
//! repair's teeth witnessed on this generation; a20-wave5's "pin alone"
//! row observed the pre-repair form). §8.4 TARGET row re-measured
//! (release, min-of-11 paired rounds, 109,382-frame subset): 7.55 ns/frame
//! full vs 3.15 ns/frame null baseline = 2.40x — `< 4 ns` GAP and `<= 3x`
//! PASS, named not rounded. Zero defects surfaced, zero repairs to land:
//! the courts held, and this block is the wave's receipt. Audit self-defect
//! recorded: one shim script SyntaxError ran the pristine suite once
//! (11/11 exit 0, not a court run) before M3 applied correctly.
//!
//! Wave 8 slice (this restart): defect repair driven by the wave-7 court
//! receipt (`a08-wave7-audit-receipt.md` — the only wave-7 receipt on disk).
//! Re-read findings: it names NO penstock defect (verdict ALIVE is
//! cgap-bont-scoped); its portable content is method (full-suite re-run with
//! `--no-fail-fast`, mutation matrix against the real file in a sandbox,
//! sha256 fail-closed restores) plus D-A08-5, a `crates/cgap-bont/target-a06/`
//! build-root cleanup violation owned by A06/coordinator, not this file.
//! Method executed with no silent delta: full suite both profiles (dev 11/11
//! exit 0, 55.78 s; release 11/11 exit 0, 7.12 s). §8.4 TARGET row
//! re-measured (release, min-of-11 paired rounds, 109,382-frame subset):
//! 7.60 ns/frame full vs 1.91 ns/frame null baseline = 3.97x — both TARGET
//! rows GAP, named not rounded (the divergence-8 measuring-court form; the
//! 10x gross-regression ceiling holds). §8.3 anti-vacuity matrix
//! re-executed against THIS file generation in `/tmp/a14w8-sb1` (canonical
//! sha256 `21516b9a…b0ba` byte-exact before and after every restore; sandbox
//! baseline 11/11 exit 0): magic-removal FLIP (5 courts: allocator-hook
//! witness, dam corpus, order-pin, taxonomy, content-independence), escaped
//! allocation-injection FLIP (exactly the two zero-alloc courts),
//! order-shuffle FLIP (`order_pinning_first_failure_wins` alone, §4.5), and
//! the M9 constant-drift class FLIP (`status_register_is_pure_a19_subset`
//! alone). Zero defects surfaced, zero repairs to land: every court held and
//! every named shim flips its named court (4/4 classes, zero vacuous, zero
//! skips); this block is the wave's receipt.
//!
//! Citations (values cited, never redefined — the citing tables below carry
//! the normative pointer, not authority):
//!
//! - `CgapFrameHeader` layout: lane A04,
//!   `docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md` §6 (canonical
//!   struct), §6.1 (byte-offset table), §6.3 (flags), §6.4 (static asserts).
//!   "Any drift between this table and `frame.rs` is a defect in `frame.rs`."
//! - `P28LicenseAttestation` (80 B): lane A17,
//!   `docs/jira/v26.11.1/lanes/A17-p28-derivative.md` §2/§2.1/§2.2 —
//!   authoritative per the v26.11.1 dispatch contract ("A17
//!   P28LicenseAttestation (80B)"). A04 §6.2's draft layout (magic "P28\0",
//!   `version: u8`, `sil_level: u8`, `sig_or_id: [u8; 40]`, no explicit
//!   align) YIELDS to it; divergence recorded in the module notes below.
//! - `CgapStatus` values: lane A19,
//!   `docs/jira/v26.11.1/lanes/A19-libcgap-ffi-xip.md` §"C signatures
//!   (normative)" — the register this module copies verbatim below.
//! - License instrument identity: `LICENSE-P28-SIL.md` line 3 (canonical
//!   specification identifier `urn:avec:...`); A17 §3 confirms the URN is
//!   the de-referenced identifier carried out-of-band. A04 §6.2's comment
//!   spells `urn:cgap:...` — instrument wins, doc comment is drift.
//!
//! Divergences recorded (doc/spec yields, not silent):
//!
//! 1. A14's lane doc §3/§4/§5/§8 cites A04 §6.2's draft attestation
//!    (magic "P28\0", `sil_level`, 40 B `sig_or_id`). The authoritative
//!    A17 §2/§2.1 layout differs (magic "P28S", u16 version/flags,
//!    ceiling-cents field, two 32 B digests, align 8). This module
//!    freezes the A17 layout; the lane doc's row 4/5 wording is an
//!    amendment candidate routed with the A19 register amendments.
//! 2. Lane doc's Wave-3 framing (§9.1) is superseded by the dispatch
//!    contract's slice for this restart: Wave 1 = ABI freeze only,
//!    gate = `cargo build -p cgap-gate` exit 0 + the static asserts.
//! 3. DoD 2's literal re-import stays deferred this wave: the off-limits
//!    `crates/cgap-gate/Cargo.toml` declares no `cgap-core` dependency,
//!    so the frozen mirror stands. Cross-checked this session against
//!    `crates/cgap-core/src/frame.rs`'s own compile-time asserts (identical
//!    size/align/offset table; magic `P28S`, frame.rs:338-367) — pin-vs-pin
//!    evidence, not the re-import DoD 2 names.
//! 4. Lane doc DoD 1 names `crates/gate/`; the scaffolded crate is
//!    `crates/cgap-gate/` (coordinator c83619d) — path divergence only.
//! 5. §8.2's "whole-run process counters unchanged (delta == 0)" is
//!    unsatisfiable literally — the test harness allocates outside tracked
//!    scopes. The enforced form is per-call scoped zero-alloc across the
//!    full 10M corpus (strictly stronger on the gate surface itself).
//! 6. The dispatch contract's parenthetical "(courts, WAL, unifier, dam,
//!    sealers)" names components from sibling lanes (WAL = A05, unifier =
//!    A12/A13, sealers = not a term in any A14 surface) absent from this
//!    lane doc; A14 scope per the doc is penstock/dam/courts only.
//! 7. §8.4's bench method names a criterion bench at
//!    `crates/gate/benches/penstock_bench.rs` (PROPOSED). Criterion needs
//!    a dev-dependency (Cargo.toml off-limits — DoD 1's zero-dep rule
//!    stands) and a benches/ dir is outside this lane's one-file
//!    ownership, so the fence executes as an in-suite release-profile
//!    court with the §8.4 method preserved (min-of-N paired rounds,
//!    null-check baseline, `black_box`); criterion-style isolation is a
//!    recorded gap, not a silent substitution.
//! 8. Wave 3 implemented §8.4's ratio fence as a hard `assert!(ratio <=
//!    3.0)`. §8.4 itself rules that reading out: "conformance outranks
//!    throughput — the exit gate is the 10M reject + zero-alloc court;
//!    throughput is a TARGET row in the receipt, never the gate", and R2's
//!    falsifier demands the receipt NAME the gap when the TARGET is
//!    missed. Measured 2026-11-01 (release, min-of-11 paired rounds,
//!    disclosed host): 4.89 ns/frame full vs 1.57 ns/frame baseline =
//!    3.1x — the <= 3x and < 4 ns TARGET rows are GAP, named here, no
//!    rounding up. The court now enforces measurement validity plus a 10x
//!    gross-regression ceiling (a reordered-to-quadratic chain still
//!    fails it); the 3x/4 ns rows are reported, never asserted.
//!
//! Wave-1 deferral (recorded, not silent): lane doc DoD 2 requires the
//! asserts to pin A04's `frame.rs` type "re-imported, never re-declared".
//! A04's canonical type HAS landed this session
//! (`crates/cgap-core/src/frame.rs`), but the off-limits
//! `crates/cgap-gate/Cargo.toml` declares no `cgap-core` dependency, so
//! this module freezes a byte-identical local mirror
//! (`CgapFrameHeaderAbi`, `P28LicenseAttestationAbi`) pinned to the cited
//! tables. Wave 3 adds the dependency, replaces the mirror with the
//! re-import `use cgap_core::frame::{CgapFrameHeader, P28LicenseAttestation}`
//! and the asserts then bind the canonical type directly (DoD 2), at which
//! point the status codes below become re-exports of A19's register
//! (DoD 10).

/// Total frame header length in bytes — A04 §6/§6.1: 128 B total, align 64.
pub const HEADER_LEN: usize = 128;

/// Wire magic "CGAP" at offset 0..4 — A04 §6.1: `43 47 41 50` = "CGAP".
pub const MAGIC: [u8; 4] = *b"CGAP";

/// Format version — A04 §6: "format version, start at 1".
pub const HEADER_VERSION: u8 = 1;

// ---------------------------------------------------------------------------
// Byte-offset table (A04 §6.1, normative offsets cited verbatim)
// ---------------------------------------------------------------------------

/// Offset of `magic` — A04 §6.1.
pub const OFF_MAGIC: usize = 0;
/// Offset of `version` — A04 §6.1.
pub const OFF_VERSION: usize = 4;
/// Offset of `flags` — A04 §6.1.
pub const OFF_FLAGS: usize = 5;
/// Offset of `reserved` — A04 §6.1 ("zero on write, ignored on read").
pub const OFF_RESERVED: usize = 6;
/// Offset of `payload_byte_len` (u64 LE) — A04 §6.1.
pub const OFF_PAYLOAD_BYTE_LEN: usize = 8;
/// Offset of `oabi_digest` (32 B BLAKE3 over payload) — A04 §6.1.
pub const OFF_OABI_DIGEST: usize = 16;
/// Offset of `attestation` (`P28LicenseAttestation`, spans 48..128) — A04 §6.1.
pub const OFF_ATTESTATION: usize = 48;

/// Size of `oabi_digest` — A04 §6.1.
pub const OABI_DIGEST_LEN: usize = 32;
/// Size of the embedded attestation — A17 §2.1 offset table, ends at 80.
pub const ATTESTATION_LEN: usize = 80;

// ---------------------------------------------------------------------------
// Flags (A04 §6.3, normative bit meanings cited verbatim)
// ---------------------------------------------------------------------------

/// bit 0 — A04 §6.3 `FLAG_ARROW` = 0x01.
pub const FLAG_ARROW: u8 = 0x01;
/// bit 1 — A04 §6.3 `FLAG_PARQUET` = 0x02.
pub const FLAG_PARQUET: u8 = 0x02;
/// bit 2 — A04 §6.3 `FLAG_BN3` = 0x04.
pub const FLAG_BN3: u8 = 0x04;
/// bit 3 — A04 §6.3 `FLAG_GITPACK` = 0x08.
pub const FLAG_GITPACK: u8 = 0x08;
/// bits 4-7 reserved, must be zero — A04 §6.3 (taxonomy row 6 is an A19
/// amendment candidate; no code minted — lane doc §5.2).
pub const FLAGS_RESERVED_MASK: u8 = 0xF0;

// ---------------------------------------------------------------------------
// P28 attestation constants (A17 §2/§2.1/§2.2; instrument identity from
// LICENSE-P28-SIL.md line 3)
// ---------------------------------------------------------------------------

/// Attestation magic at header offset 48..52 — A17 §2.1: `b"P28S"`
/// (`0x50 0x32 0x38 0x53`; A17 field name `license_magic`).
pub const P28_LICENSE_MAGIC: [u8; 4] = *b"P28S";
/// Attestation version — A17 §2.1: `u16` LE, currently 1.
pub const P28_VERSION: u16 = 1;
/// Safe-harbor ceiling in cents — A17 §2.1: MUST be <= 5_000_000_000 ($50M).
pub const P28_SAFE_HARBOR_CEILING_CENTS: u64 = 5_000_000_000;
/// Attestation bit 0 — A17 §2.2 `FLAG_ROOT` (derives from the instrument;
/// `upstream_receipt` all-zero).
pub const P28_FLAG_ROOT: u16 = 1 << 0;
/// Attestation bit 1 — A17 §2.2 `FLAG_DERIVATIVE` (`upstream_receipt`
/// MUST be nonzero).
pub const P28_FLAG_DERIVATIVE: u16 = 1 << 1;
/// Attestation bit 2 — A17 §2.2 `FLAG_TRANSPILED` (beam -> WASM).
pub const P28_FLAG_TRANSPILED: u16 = 1 << 2;
/// Attestation bits 3-15 reserved, MUST be zero (nonzero reserved bits are
/// a spoliation condition, A17 §2.2/§6).
pub const P28_ATTESTATION_FLAGS_RESERVED_MASK: u16 = 0xFFF8;
/// Exactly one of bits 0-2 MUST be set — A17 §2.2 lineage-bit law, frozen
/// as a mask (zero or 2+ lineage bits set = malformed = spoliated).
pub const P28_LINEAGE_MASK: u16 = 0x0007;
/// License instrument identity — `LICENSE-P28-SIL.md` line 3 (canonical
/// specification identifier). A04 §6.2's `urn:cgap:...` spelling is drift;
/// see the module citation note.
pub const P28_LICENSE_URN: &str = "urn:avec:license:p28-sil:v2.1.0-sovereign";

// ---------------------------------------------------------------------------
// CgapStatus register values (A19 §"C signatures (normative)")
// ---------------------------------------------------------------------------

/// A19 register `CGAP_OK = 0x00000000u`.
pub const CGAP_OK: u32 = 0x0000_0000;
/// A19 register `CGAP_ERR_INVALID_MAGIC = 0x43470001u` — penstock taxonomy
/// rows 1-2 (lane doc §5). Frozen copy pending Wave-3 re-export; see the
/// module Wave-1 deferral note.
pub const CGAP_ERR_INVALID_MAGIC: u32 = 0x4347_0001;
/// A19 register `CGAP_ERR_OABI_MISMATCH = 0x43470002u` — penstock taxonomy
/// row 3 (lane doc §5). Frozen copy pending Wave-3 re-export.
pub const CGAP_ERR_OABI_MISMATCH: u32 = 0x4347_0002;
/// A19 register `CGAP_ERR_P28_SPOLIATION = 0x503238FFu` — penstock taxonomy
/// rows 4-5 (lane doc §5). Frozen copy pending Wave-3 re-export.
pub const CGAP_ERR_P28_SPOLIATION: u32 = 0x5032_38FF;

// ---------------------------------------------------------------------------
// Frozen ABI mirror structs (Wave 1 only — Wave 3 re-imports the canonical
// types; see the module Wave-1 deferral note)
// ---------------------------------------------------------------------------

/// Wave-1 frozen mirror of A04 §6's canonical `CgapFrameHeader`
/// (`#[repr(C, align(64))]`, 128 B). NON-canonical: A04 owns the canonical
/// type (`crates/cgap-core/src/frame.rs`); this mirror exists only because
/// the off-limits `Cargo.toml` cannot declare the dependency this wave.
/// Field order and types are A04 §6 cited verbatim.
#[repr(C, align(64))]
pub struct CgapFrameHeaderAbi {
    /// 0x43 0x47 0x41 0x50 — ASCII "CGAP" (A04 §6).
    pub magic: [u8; 4],
    /// Format version, start at 1 (A04 §6).
    pub version: u8,
    /// bit 0 Arrow, bit 1 Parquet, bit 2 BN3, bit 3 GitPack (A04 §6).
    pub flags: u8,
    /// Reserved, zero on write, ignored on read (A04 §6).
    pub reserved: [u8; 2],
    /// Total payload byte len, little-endian (A04 §6).
    pub payload_byte_len: u64,
    /// BLAKE3-derived digest over the payload bytes (A04 §6).
    pub oabi_digest: [u8; 32],
    /// Embedded 80-byte P28 license attestation (A04 §6); layout is
    /// A17 §2 (authoritative).
    pub attestation: P28LicenseAttestationAbi,
}

/// Wave-1 frozen mirror of A17 §2's `P28LicenseAttestation`
/// (`#[repr(C, align(8))]`, exactly 80 B, no padding holes — A17 §2.1).
/// NON-canonical: A17 owns the P28 surface (`crates/cgap-p28`); the same
/// re-import deferral applies as for [`CgapFrameHeaderAbi`].
#[repr(C, align(8))]
pub struct P28LicenseAttestationAbi {
    /// offset 0, 4 B — b"P28S" (A17 §2.1).
    pub license_magic: [u8; 4],
    /// offset 4, 2 B — u16 LE, currently 1 (A17 §2.1).
    pub version: u16,
    /// offset 6, 2 B — u16 LE bitfield (A17 §2.2).
    pub attestation_flags: u16,
    /// offset 8, 8 B — u64 LE; MUST be <= 5_000_000_000 (A17 §2.1).
    pub safe_harbor_ceiling_cents: u64,
    /// offset 16, 32 B — BLAKE3 origin-authority digest (A17 §3).
    pub origin_authority_digest: [u8; 32],
    /// offset 48, 32 B — BLAKE3 of parent receipt; zero only if Root
    /// (A17 §2.1).
    pub upstream_receipt: [u8; 32],
}

// ---------------------------------------------------------------------------
// Static asserts (A04 §6.4 pattern: core-only const asserts, no
// `static_assertions` crate — zero-dep rule). These are the Wave-1 gate:
// any drift of the compiled layout from the cited tables is a compile
// error in this file, loud and non-silent.
// ---------------------------------------------------------------------------

const _: () = assert!(core::mem::size_of::<CgapFrameHeaderAbi>() == 128);
const _: () = assert!(core::mem::align_of::<CgapFrameHeaderAbi>() == 64);
const _: () = assert!(core::mem::size_of::<P28LicenseAttestationAbi>() == 80);
const _: () = assert!(core::mem::align_of::<P28LicenseAttestationAbi>() == 8);

// Field offsets pinned with `offset_of!` — the compiled struct layout is
// asserted byte-for-byte against the OFF_* constant table (A04 §6.1), so
// any field reorder, type drift, or alignment change breaks this build.
const _: () = assert!(core::mem::offset_of!(CgapFrameHeaderAbi, magic) == OFF_MAGIC);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeaderAbi, version) == OFF_VERSION);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeaderAbi, flags) == OFF_FLAGS);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeaderAbi, reserved) == OFF_RESERVED);
const _: () =
    assert!(core::mem::offset_of!(CgapFrameHeaderAbi, payload_byte_len) == OFF_PAYLOAD_BYTE_LEN);
const _: () =
    assert!(core::mem::offset_of!(CgapFrameHeaderAbi, oabi_digest) == OFF_OABI_DIGEST);
const _: () =
    assert!(core::mem::offset_of!(CgapFrameHeaderAbi, attestation) == OFF_ATTESTATION);

// A17 §2.1 offset table, machine-checked (stable `offset_of!`, rustc 1.77+).
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestationAbi, license_magic) == 0);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestationAbi, version) == 4);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestationAbi, attestation_flags) == 6);
const _: () =
    assert!(core::mem::offset_of!(P28LicenseAttestationAbi, safe_harbor_ceiling_cents) == 8);
const _: () =
    assert!(core::mem::offset_of!(P28LicenseAttestationAbi, origin_authority_digest) == 16);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestationAbi, upstream_receipt) == 48);

// Table arithmetic is pinned too: the cited tables' offsets must be
// internally consistent with the frozen sizes.
// A04 §6.1: magic(0..4) -> version(4..5) -> flags(5..6) -> reserved(6..8)
//   -> payload_byte_len(8..16) -> oabi_digest(16..48) -> attestation(48..128).
const _: () = assert!(OFF_MAGIC == 0 && MAGIC.len() == 4);
const _: () = assert!(OFF_VERSION == OFF_MAGIC + MAGIC.len());
const _: () = assert!(OFF_FLAGS == OFF_VERSION + 1);
const _: () = assert!(OFF_RESERVED == OFF_FLAGS + 1);
const _: () = assert!(OFF_PAYLOAD_BYTE_LEN == OFF_RESERVED + 2);
const _: () = assert!(OFF_OABI_DIGEST == OFF_PAYLOAD_BYTE_LEN + 8);
const _: () = assert!(OFF_OABI_DIGEST + OABI_DIGEST_LEN == OFF_ATTESTATION);
const _: () = assert!(OFF_ATTESTATION + ATTESTATION_LEN == HEADER_LEN);
// A17 §2.1: 4 + 2 + 2 + 8 + 32 + 32 = 80 — gap-free (A17 §2.1 "no padding
// holes: offsets 0-7, 8-15, 16-47, 48-79 fully occupied").
const _: () = assert!(ATTESTATION_LEN == 4 + 2 + 2 + 8 + 32 + 32);
// Byte-wise const comparison (array `==` is not const-stable on this
// toolchain): the two magics must stay distinct discriminators.
const _: () = assert!(
    P28_LICENSE_MAGIC.len() == 4
        && (MAGIC[0] != P28_LICENSE_MAGIC[0]
            || MAGIC[1] != P28_LICENSE_MAGIC[1]
            || MAGIC[2] != P28_LICENSE_MAGIC[2]
            || MAGIC[3] != P28_LICENSE_MAGIC[3])
);
// "urn:avec:license:p28-sil:v2.1.0-sovereign" — 41 chars; pins the frozen
// URN constant to the instrument's identity string byte-count.
const _: () = assert!(P28_LICENSE_URN.len() == 41);
// Ceiling bound sanity: the A17 §2.1 constraint value itself fits u64 and
// equals $50M in cents.
const _: () = assert!(P28_SAFE_HARBOR_CEILING_CENTS == 5_000_000_000);
// A17 §2.2 lineage bits occupy exactly bits 0-2; the reserved mask is the
// exact complement within the u16 bitfield.
const _: () = assert!(
    P28_LINEAGE_MASK == 0x0007
        && P28_ATTESTATION_FLAGS_RESERVED_MASK == !P28_LINEAGE_MASK
);

// P28 attestation struct fits exactly inside its header window.
const _: () =
    assert!(OFF_ATTESTATION + core::mem::size_of::<P28LicenseAttestationAbi>() == HEADER_LEN);

// ===========================================================================
// Wave 2 — core engine: the ingress check chain (lane doc §4 normative
// order, §6 O(1) argument). no_std-compatible: `core` only, no allocation,
// no panics on caller-controlled input (no unwrap/expect/indexing past
// proven bounds).
// ===========================================================================

/// A17 §2.1: `upstream_receipt` at attestation offset 48, 32 B (48..80).
const P28_RECEIPT_AT: usize = 48;
/// A17 §2.1: `upstream_receipt` is 32 B.
const P28_RECEIPT_LEN: usize = 32;

/// Non-owning, zero-copy admit view — A19's `cgap_frame_view` pattern
/// (lane doc §2/§6; include/cgap.h): the payload slice points INTO the
/// caller's bytes and dies with them. The gate mints no heap state.
///
/// Divergence recorded (not silent): A19's C view carries `schema_id` /
/// `rule_count`; no offset in A04 §6.1 carries those fields, so the gate
/// cannot prove them. The gate's view carries the geometry it can prove;
/// the schema/rule-bearing FFI twin is A19's admit-path surface
/// (`crates/cgap-ffi/src/sovereign.rs`), not the gate's.
#[derive(Debug, Clone, Copy)]
pub struct FrameView<'bytes> {
    /// Zero-copy payload view INTO the caller's bytes (A19 §"cgap_frame_view").
    pub payload: &'bytes [u8],
}

/// The ingress boundary (boundary invariant 4, lane doc §2): the first code
/// the bytes meet. Classifies a frame OK/reject in constant time and space
/// BEFORE any parser, decoder, or allocator sees a byte. Nothing malformed
/// is parsed; the gate is side-effect free (writes at most the caller's
/// out-view; mints no heap state — the gate cannot be the OOM).
///
/// Fixed normative order (lane doc §4; first failure returns, never
/// reordered): magic/version -> O-ABI coherence -> P28 chain presence.
///
/// `len` is the caller-declared buffer length (the transport's truth — a
/// DMA slot hands slot + len). Fail-closed on a lying caller: a declared
/// `len` exceeding the slice's actual length is a row-1-class reject read
/// of nothing; no read ever goes past `bytes.len()`.
///
/// `out` MAY be `None` to probe (A19 `cgap_frame_view` MAY be NULL).
///
/// Returns only A19 register values (lane doc §4, DoD 10): [`CGAP_OK`],
/// [`CGAP_ERR_INVALID_MAGIC`], [`CGAP_ERR_OABI_MISMATCH`],
/// [`CGAP_ERR_P28_SPOLIATION`]. The gate mints no codes.
///
/// Per-taxonomy mapping (lane doc §5; A17 layout per the Wave-1 divergence
/// note — magic `P28S`, not A04 §6.2's draft `P28\0`):
///
/// | row | fault | code |
/// |---:|---|---|
/// | 1 | `len < 128` (or declared len exceeds the slice) | `INVALID_MAGIC` |
/// | 2 | `magic != "CGAP"`; `version != 1` (§6 compare list) | `INVALID_MAGIC` |
/// | 3 | digest all-zero; `payload_byte_len > len - 128` | `OABI_MISMATCH` |
/// | 4 | attestation magic != `P28S` | `P28_SPOLIATION` |
/// | 5 | att version/lineage/reserved/ceiling/receipt laws (A17 §2.2/§6) | `P28_SPOLIATION` |
/// | 6 | header flags bits 4-7 set | ADMITTED — see the row-6 divergence note on [`FLAGS_RESERVED_MASK`] |
#[must_use]
pub fn ingress<'bytes>(
    bytes: &'bytes [u8],
    len: usize,
    out: Option<&mut FrameView<'bytes>>,
) -> u32 {
    // -- Group 1: not-a-CGAP-frame (A19 CGAP_ERR_INVALID_MAGIC) -------------
    // Row 1: truncated header. A frame shorter than the canonical header has
    // no magic to compare (lane doc §5.2: the documented INVALID_MAGIC
    // overload — "not a CGAP frame is exactly what INVALID_MAGIC means").
    if len < HEADER_LEN {
        return CGAP_ERR_INVALID_MAGIC;
    }
    // Caller-geometry fail-closed (row-1 class): a declared length beyond
    // the actual slice is a buffer the gate cannot trust; reject without
    // reading anything. This keeps every later index in-bounds by proof.
    if len > bytes.len() {
        return CGAP_ERR_INVALID_MAGIC;
    }
    // Row 2: magic — the cheapest discriminator, first (lane doc §4: highest
    // early-exit yield; alias safety — A04's offsets mean nothing for
    // non-CGAP bytes). In-bounds by proof: len >= 128 and len <= bytes.len().
    if bytes[OFF_MAGIC..OFF_MAGIC + MAGIC.len()] != MAGIC {
        return CGAP_ERR_INVALID_MAGIC;
    }
    // Version — §6's second fixed compare: wrong version is not this frame
    // grammar (row-2 family overload; recorded, not a minted code).
    if bytes[OFF_VERSION] != HEADER_VERSION {
        return CGAP_ERR_INVALID_MAGIC;
    }

    // -- Group 2: O-ABI coherence (A19 CGAP_ERR_OABI_MISMATCH) --------------
    // Row 3a: all-zero digest field. The gate is a limiter, not a verifier
    // (R1): full BLAKE3 re-derivation belongs to the admit path (A04); here
    // the digest field must merely be present (nonzero) for the frame to be
    // in the CGAP grammar at all.
    let mut digest_all_zero = true;
    let mut i = 0;
    while i < OABI_DIGEST_LEN {
        if bytes[OFF_OABI_DIGEST + i] != 0 {
            digest_all_zero = false;
            break;
        }
        i += 1;
    }
    if digest_all_zero {
        return CGAP_ERR_OABI_MISMATCH;
    }
    // Row 3b: length coherence — declared payload must fit the caller's
    // declared buffer. u64 compare against a u64-widened bound: catches the
    // u64 field that cannot exist on a 32-bit usize (overflow class, MC-2).
    let declared = read_u64_le(bytes, OFF_PAYLOAD_BYTE_LEN);
    if declared > (len - HEADER_LEN) as u64 {
        return CGAP_ERR_OABI_MISMATCH;
    }

    // -- Group 3: P28 chain presence (A19 CGAP_ERR_P28_SPOLIATION) ----------
    // Row 4: attestation magic (A17-authoritative layout, Wave-1 note).
    let a = OFF_ATTESTATION;
    if bytes[a..a + P28_LICENSE_MAGIC.len()] != P28_LICENSE_MAGIC {
        return CGAP_ERR_P28_SPOLIATION;
    }
    // Row 5: attestation fields out of range (A17 §2.2/§6 laws, O(1) each).
    if read_u16_le(bytes, a + 4) != P28_VERSION {
        return CGAP_ERR_P28_SPOLIATION;
    }
    let att_flags = read_u16_le(bytes, a + 6);
    // Reserved attestation bits (3-15) nonzero = spoliation (A17 §2.2/§6).
    if att_flags & P28_ATTESTATION_FLAGS_RESERVED_MASK != 0 {
        return CGAP_ERR_P28_SPOLIATION;
    }
    // Exactly one lineage bit (A17 §2.2: zero or 2+ set = malformed).
    let lineage = att_flags & P28_LINEAGE_MASK;
    if lineage == 0 || lineage.count_ones() != 1 {
        return CGAP_ERR_P28_SPOLIATION;
    }
    // Safe-harbor ceiling bound (A17 §2.1: MUST be <= 5_000_000_000).
    if read_u64_le(bytes, a + 8) > P28_SAFE_HARBOR_CEILING_CENTS {
        return CGAP_ERR_P28_SPOLIATION;
    }
    // Receipt law (A17 §2.2): ROOT (bit 0) derives from the instrument and
    // carries an all-zero upstream_receipt; DERIVATIVE/TRANSPILED carry a
    // real parent/transpiler receipt (nonzero). ROOT xor receipt-zero.
    let receipt_zero = is_zero_at(bytes, a + P28_RECEIPT_AT, P28_RECEIPT_LEN);
    if (lineage == P28_FLAG_ROOT) != receipt_zero {
        return CGAP_ERR_P28_SPOLIATION;
    }

    // -- ADMIT (A19 CGAP_OK): write the caller's out-view, if requested -----
    // `declared <= len - 128 <= bytes.len() - 128` by proof above, so the
    // slice range is in-bounds and the u64 -> usize narrowing is
    // value-preserving on every supported target (32-bit included).
    if let Some(view) = out {
        view.payload = &bytes[HEADER_LEN..HEADER_LEN + declared as usize];
    }
    CGAP_OK
}

/// Panic-free little-endian u64 read at a fixed offset. The caller
/// guarantees `off + 8 <= bytes.len()`; the gate proves that before every
/// call (row-1/geometry checks). No unwrap/expect on caller bytes. The
/// byte-loop shape is load-bearing for codegen: slice/`copy_from_slice`/
/// `iter().all()` forms were measured 2026-11-01 (release, this host) at
/// 6.3 ns/frame vs 4.9 for the byte loop — bounds-check elimination fails
/// across the helper boundary, so the loop stays.
#[inline]
fn read_u64_le(bytes: &[u8], off: usize) -> u64 {
    let mut v = [0u8; 8];
    let mut i = 0;
    while i < 8 {
        v[i] = bytes[off + i];
        i += 1;
    }
    u64::from_le_bytes(v)
}

/// Panic-free little-endian u16 read at a fixed offset (same proof duty;
/// same codegen note as [`read_u64_le`]).
#[inline]
fn read_u16_le(bytes: &[u8], off: usize) -> u16 {
    let mut v = [0u8; 2];
    v[0] = bytes[off];
    v[1] = bytes[off + 1];
    u16::from_le_bytes(v)
}

/// Panic-free all-zero check over `len` bytes at `off` (same proof duty;
/// same codegen note — early-exit byte loop, no slice helpers).
#[inline]
fn is_zero_at(bytes: &[u8], off: usize, len: usize) -> bool {
    let mut i = 0;
    while i < len {
        if bytes[off + i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

// ===========================================================================
// Chicago unit courts (lane doc §8). Real collaborators: the caller-supplied
// byte buffer, the real A04 §6 layout as frozen in this module's ABI mirror,
// and a real counting allocator (hand-written real interface implementation
// over `System` — the Chicago "not a mock" clause; the canonical
// `crates/audit/src/alloc_counter.rs` CountingAlloc cannot be imported this
// wave because the off-limits Cargo.toml carries no cgap-audit dependency —
// divergence recorded, same pattern as the audit module's own I10 note).
// All assertions are final-state: real return codes, real bytes, real
// allocator counters. No mocks, no sleeps (nothing waits on time).
// ===========================================================================

#[cfg(test)]
mod courts {
    use super::*;

    // -- Real counting allocator (§8.2; local mirror of the audit crate's
    // -- CountingAlloc — divergence recorded in the module docs) ----------

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    // Wave-6 repair: always-on PER-THREAD allocation counters. The wave-4
    // form used PROCESS-WIDE atomics (TOTAL_ALLOCS / TOTAL_ALLOC_BYTES) as a
    // bare-call witness and false-failed nondeterministically: the default
    // libtest harness runs tests on parallel threads, and a concurrent
    // test's allocation lands inside the bracket (witnessed 2026-11-01:
    // parallel suite FAILED with delta (1, 59), reran PASS, isolated PASS —
    // the gate itself cannot allocate, so the delta was another test's).
    // Thread-local counters are immune by construction; the witness law
    // carries the same teeth (R3b: the install must be live, not
    // silent-zero) and the in-court liveness step below keeps it honest.
    thread_local! {
        static THREAD_ALLOCS: Cell<usize> = const { Cell::new(0) };
        static THREAD_ALLOC_BYTES: Cell<usize> = const { Cell::new(0) };
    }

    #[derive(Clone, Copy)]
    struct Tracking {
        active: bool,
        allocs: usize,
        bytes: usize,
    }

    thread_local! {
        static TRACKING: Cell<Tracking> =
            const { Cell::new(Tracking { active: false, allocs: 0, bytes: 0 }) };
    }

    struct CountingAlloc;

    #[global_allocator]
    static COUNTING: CountingAlloc = CountingAlloc;

    // Safety: forwards every call to System with identical semantics; the
    // counting path touches only lock-free atomics and a const-init
    // thread-local (no allocation, no panic path inside the hook).
    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            THREAD_ALLOCS.with(|c| c.set(c.get() + 1));
            THREAD_ALLOC_BYTES.with(|c| c.set(c.get() + layout.size()));
            TRACKING.with(|t| {
                let mut s = t.get();
                if s.active {
                    s.allocs += 1;
                    s.bytes += layout.size();
                    t.set(s);
                }
            });
            System.alloc(layout)
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            System.dealloc(ptr, layout);
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            THREAD_ALLOCS.with(|c| c.set(c.get() + 1));
            THREAD_ALLOC_BYTES.with(|c| c.set(c.get() + new_size));
            TRACKING.with(|t| {
                let mut s = t.get();
                if s.active {
                    s.allocs += 1;
                    s.bytes += new_size;
                    t.set(s);
                }
            });
            System.realloc(ptr, layout, new_size)
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            THREAD_ALLOCS.with(|c| c.set(c.get() + 1));
            THREAD_ALLOC_BYTES.with(|c| c.set(c.get() + layout.size()));
            TRACKING.with(|t| {
                let mut s = t.get();
                if s.active {
                    s.allocs += 1;
                    s.bytes += layout.size();
                    t.set(s);
                }
            });
            System.alloc_zeroed(layout)
        }
    }

    /// Scoped tracking guard (§8.2 `with_alloc_tracking` pattern): active
    /// only inside the scope; reports per-thread alloc/byte counts while
    /// alive. Corpus construction stays OUTSIDE the measured region (R3).
    struct TrackGuard;

    impl TrackGuard {
        fn enter() -> Self {
            TRACKING.with(|t| t.set(Tracking { active: true, allocs: 0, bytes: 0 }));
            TrackGuard
        }
        fn counts(&self) -> (usize, usize) {
            TRACKING.with(|t| {
                let s = t.get();
                (s.allocs, s.bytes)
            })
        }
    }

    impl Drop for TrackGuard {
        fn drop(&mut self) {
            TRACKING.with(|t| {
                let mut s = t.get();
                s.active = false;
                t.set(s);
            });
        }
    }

    // -- Deterministic seeded corpus (§8.1; SplitMix64) --------------------

    const CORPUS_SEED: u64 = 0xC6A4_A793_8547_1D01; // splitmix64 gamma
    const CORPUS_LEN: u64 = 10_000_000;

    struct SplitMix64(u64);

    impl SplitMix64 {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
    }

    /// Corpus class ids (lane doc §8.1 mix: 50% garbage / 25% seeded
    /// mutations / 25% valid).
    const CLS_GARBAGE: u64 = 0;
    const CLS_MUTATION: u64 = 1;
    const CLS_VALID: u64 = 2;

    /// Expected verdict per corpus frame — computed by construction (the
    /// generator knows which law it broke); the manifest is this pure
    /// function of (seed, i), not a materialized table.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Expected {
        /// Exactly this A19 register code.
        Code(u32),
        /// Garbage class: must NOT be admitted (any register reject is
        /// conformant — the gate's fixed order owns which one).
        RejectAny,
    }

    /// Mutation classes over the A04 §6.1 / A17 §2.1 offset tables (§8.1).
    const MC_MAGIC: u64 = 0; // row 2
    const MC_TRUNC: u64 = 1; // row 1
    const MC_LEN_OVERRUN: u64 = 2; // row 3
    const MC_DIGEST_ZERO: u64 = 3; // row 3
    const MC_ATT_MAGIC: u64 = 4; // row 4
    const MC_ATT_RANGE: u64 = 5; // row 5
    const MC_ATT_RECEIPT: u64 = 6; // row 5
    const MC_FLAGS_RESERVED: u64 = 7; // row 6 (admitted — divergence note)
    const MC_V_COMBOS: u64 = 8; // CGAP_OK

    /// Fill `buf`'s first `frame_len` bytes as corpus frame `i`, returning
    /// the frame's buffer length (what a transport would hand as `len`) and
    /// the expected verdict. Pure function of (seed, i) — determinism is
    /// courted in `corpus_generator_is_deterministic`.
    fn gen_frame(seed: u64, i: u64, buf: &mut [u8]) -> (usize, Expected) {
        let mut rng = SplitMix64(seed ^ i.wrapping_mul(0x2544_5491_4F6C_DD1D));
        buf.fill(0);
        // Lane doc §8.1 mix — 50% garbage / 25% mutations / 25% valid — over
        // rng % 4: {0,1} garbage, {2} mutation, {3} valid.
        let class = match rng.next() % 4 {
            0 | 1 => CLS_GARBAGE,
            2 => CLS_MUTATION,
            _ => CLS_VALID,
        };
        if class == CLS_GARBAGE {
            // Pure garbage: uniform random bytes over a random length.
            let n = (rng.next() as usize) % buf.len().max(1);
            let mut j = 0;
            while j < n {
                buf[j] = rng.next() as u8;
                j += 1;
            }
            return (n, Expected::RejectAny);
        }
        // Write a valid v1 frame base, then a mutation class (only when
        // class == CLS_MUTATION; the CLS_VALID path returns just below).
        let payload_len = (rng.next() as usize) % 256;
        let declared = payload_len;
        buf[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&MAGIC);
        buf[OFF_VERSION] = HEADER_VERSION;
        let flags = 1u8 << (rng.next() % 4); // one format flag, bits 0-3
        buf[OFF_FLAGS] = flags;
        buf[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8]
            .copy_from_slice(&(declared as u64).to_le_bytes());
        for b in buf[OFF_OABI_DIGEST..OFF_OABI_DIGEST + OABI_DIGEST_LEN].iter_mut() {
            *b = rng.next() as u8;
        }
        buf[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&P28_LICENSE_MAGIC);
        buf[OFF_ATTESTATION + 4..OFF_ATTESTATION + 6]
            .copy_from_slice(&P28_VERSION.to_le_bytes());
        // Exactly one lineage bit; ROOT carries the zero receipt, others a
        // nonzero one (A17 §2.2 receipt law).
        let lineage = match rng.next() % 3 {
            0 => P28_FLAG_ROOT,
            1 => P28_FLAG_DERIVATIVE,
            _ => P28_FLAG_TRANSPILED,
        };
        buf[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&lineage.to_le_bytes());
        buf[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
            .copy_from_slice(&P28_SAFE_HARBOR_CEILING_CENTS.to_le_bytes());
        let origin: [u8; 32] = core::array::from_fn(|_| rng.next() as u8);
        buf[OFF_ATTESTATION + 16..OFF_ATTESTATION + 48].copy_from_slice(&origin);
        if lineage != P28_FLAG_ROOT {
            let receipt: [u8; 32] = core::array::from_fn(|_| rng.next() as u8);
            buf[OFF_ATTESTATION + P28_RECEIPT_AT..OFF_ATTESTATION + P28_RECEIPT_AT + P28_RECEIPT_LEN]
                .copy_from_slice(&receipt);
        }
        let frame_len = HEADER_LEN + payload_len;
        if class != CLS_MUTATION {
            // CLS_VALID (and any residue class): admit unmutated.
            return (frame_len, Expected::Code(CGAP_OK));
        }
        // Mutation dispatch — break exactly one law over the valid base.
        match rng.next() % 9 {
            MC_MAGIC => {
                let bad: [u8; 4] = match rng.next() % 3 {
                    0 => *b"XGAP",
                    1 => *b"cgap",
                    _ => [0u8; 4],
                };
                buf[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&bad);
                (frame_len, Expected::Code(CGAP_ERR_INVALID_MAGIC))
            }
            MC_TRUNC => {
                // Row 1: declared buffer length shorter than the header.
                let n = HEADER_LEN - 1 - (rng.next() as usize) % HEADER_LEN;
                (n, Expected::Code(CGAP_ERR_INVALID_MAGIC))
            }
            MC_LEN_OVERRUN => {
                // Row 3b: declared payload beyond the declared buffer.
                let overrun = declared as u64 + 1 + rng.next() % 4096;
                buf[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8]
                    .copy_from_slice(&overrun.to_le_bytes());
                (frame_len, Expected::Code(CGAP_ERR_OABI_MISMATCH))
            }
            MC_DIGEST_ZERO => {
                for b in buf[OFF_OABI_DIGEST..OFF_OABI_DIGEST + OABI_DIGEST_LEN].iter_mut() {
                    *b = 0;
                }
                (frame_len, Expected::Code(CGAP_ERR_OABI_MISMATCH))
            }
            MC_ATT_MAGIC => {
                let bad: [u8; 4] = match rng.next() % 3 {
                    0 => *b"P29\0",
                    1 => [0u8; 4],
                    _ => *b"p28s",
                };
                buf[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&bad);
                (frame_len, Expected::Code(CGAP_ERR_P28_SPOLIATION))
            }
            MC_ATT_RANGE => {
                // Reserved attestation bits OR zero/two lineage bits OR an
                // over-ceiling value — each a §2.2/§6 spoliation law.
                match rng.next() % 3 {
                    0 => {
                        let f = lineage | (1u16 << (3 + rng.next() % 13));
                        buf[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8]
                            .copy_from_slice(&f.to_le_bytes());
                    }
                    1 => {
                        // Second lineage bit MUST differ from the base's set
                        // bit — re-setting the same bit leaves a valid frame.
                        let f = match rng.next() % 2 {
                            0 => 0u16,
                            _ => {
                                let base_bit = lineage.trailing_zeros();
                                lineage
                                    | (1u16 << ((base_bit + 1 + (rng.next() % 2) as u32) % 3))
                            }
                        };
                        buf[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8]
                            .copy_from_slice(&f.to_le_bytes());
                    }
                    _ => {
                        let over = P28_SAFE_HARBOR_CEILING_CENTS + 1 + rng.next() % 1_000_000;
                        buf[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
                            .copy_from_slice(&over.to_le_bytes());
                    }
                }
                (frame_len, Expected::Code(CGAP_ERR_P28_SPOLIATION))
            }
            MC_ATT_RECEIPT => {
                // Receipt-law flip, direction pinned by the base lineage so
                // the mutation always BREAKS the law it targets: ROOT gains
                // a receipt (ROOT xor receipt-zero violated); non-ROOT loses
                // its receipt.
                if lineage == P28_FLAG_ROOT {
                    let r: [u8; 32] = core::array::from_fn(|_| rng.next() as u8);
                    buf[OFF_ATTESTATION + P28_RECEIPT_AT
                        ..OFF_ATTESTATION + P28_RECEIPT_AT + P28_RECEIPT_LEN]
                        .copy_from_slice(&r);
                } else {
                    let at = OFF_ATTESTATION + P28_RECEIPT_AT;
                    for b in buf[at..at + P28_RECEIPT_LEN].iter_mut() {
                        *b = 0;
                    }
                }
                (frame_len, Expected::Code(CGAP_ERR_P28_SPOLIATION))
            }
            MC_FLAGS_RESERVED => {
                // Row 6: header flags bits 4-7. No A19 code exists (lane doc
                // §5.2 amendment candidate); A04 §6.3 reads them as "ignored
                // on read", so the gate ADMITS — divergence recorded on
                // FLAGS_RESERVED_MASK. The manifest pins that reading so a
                // future A19 amendment flips this court loudly.
                buf[OFF_FLAGS] = flags | 0x10;
                (frame_len, Expected::Code(CGAP_OK))
            }
            MC_V_COMBOS => {
                // Valid flag-combination frame (§8.1 MC-V).
                let combo = (rng.next() as u8) & 0x0F;
                buf[OFF_FLAGS] = combo;
                (frame_len, Expected::Code(CGAP_OK))
            }
            wheel_residue => unreachable!(
                "mutation wheel residue {wheel_residue} outside 0..={MC_V_COMBOS}"
            ),
        }
    }

    /// Run `ingress` over the seeded corpus, asserting every frame's
    /// manifest verdict (per-call zero-alloc included when `track`).
    fn run_corpus(seed: u64, n: u64, track: bool) {
        let mut buf = vec![0u8; 4096]; // scratch, built OUTSIDE tracked scopes
        for i in 0..n {
            let (frame_len, expected) = gen_frame(seed, i, &mut buf);
            let mut view = FrameView { payload: &[] };
            let guard = if track { Some(TrackGuard::enter()) } else { None };
            let st = ingress(&buf, frame_len, Some(&mut view));
            if let Some(g) = guard.as_ref() {
                let (allocs, bytes) = g.counts();
                assert_eq!(
                    (allocs, bytes),
                    (0, 0),
                    "frame {i} class-corpus: gate allocated ({allocs} allocs, {bytes} bytes)"
                );
            }
            drop(guard);
            match expected {
                Expected::Code(code) => {
                    assert_eq!(st, code, "frame {i}: wrong verdict for its manifest class");
                }
                Expected::RejectAny => {
                    assert_ne!(st, CGAP_OK, "frame {i}: garbage frame was ADMITTED");
                }
            }
            if st == CGAP_OK {
                // Admit-path view law: payload geometry matches the header's
                // declared length exactly, and is zero-copy INTO the buffer.
                let declared = u64::from_le_bytes(
                    buf[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8]
                        .try_into()
                        .unwrap(),
                );
                assert_eq!(view.payload.len() as u64, declared, "frame {i}: view geometry");
                assert_eq!(
                    view.payload.as_ptr(),
                    buf[HEADER_LEN..].as_ptr(),
                    "frame {i}: view is not zero-copy into the caller's bytes"
                );
            }
        }
    }

    // -- Courts -------------------------------------------------------------

    /// §8.1: corpus generation is a deterministic function of one u64 seed —
    /// two generations are byte-equal (final-state assertion).
    #[test]
    fn corpus_generator_is_deterministic() {
        let mut a = vec![0u8; 4096];
        let mut b = vec![0u8; 4096];
        for i in 0..2000u64 {
            let (la, ea) = gen_frame(CORPUS_SEED, i, &mut a);
            let (lb, eb) = gen_frame(CORPUS_SEED, i, &mut b);
            assert_eq!((la, ea), (lb, eb), "manifest row {i} not deterministic");
            assert_eq!(&a[..la], &b[..lb], "frame {i} bytes not deterministic");
        }
    }

    /// §5.1/DoD 3: taxonomy rows 1-6 individually injectable and asserted.
    #[test]
    fn taxonomy_rows_are_individually_injectable() {
        let mut f = vec![0u8; 512];
        let valid = |f: &mut Vec<u8>| {
            f.fill(0);
            f[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&MAGIC);
            f[OFF_VERSION] = HEADER_VERSION;
            f[OFF_FLAGS] = FLAG_ARROW;
            f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8].copy_from_slice(&8u64.to_le_bytes());
            f[OFF_OABI_DIGEST] = 0xA5; // nonzero digest field (coherence only)
            f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&P28_LICENSE_MAGIC);
            f[OFF_ATTESTATION + 4..OFF_ATTESTATION + 6].copy_from_slice(&P28_VERSION.to_le_bytes());
            f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&P28_FLAG_ROOT.to_le_bytes());
            f[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
                .copy_from_slice(&P28_SAFE_HARBOR_CEILING_CENTS.to_le_bytes());
        };

        // Row 1: truncated (127, 64, 0-byte buffers).
        for n in [0usize, 64, HEADER_LEN - 1] {
            valid(&mut f);
            assert_eq!(ingress(&f[..n], n, None), CGAP_ERR_INVALID_MAGIC, "row 1, len {n}");
        }
        // Row 1 class: declared len exceeds the slice (lying caller) —
        // fail-closed, reads nothing.
        valid(&mut f);
        assert_eq!(ingress(&f[..200], 500, None), CGAP_ERR_INVALID_MAGIC, "row 1 geometry");

        // Row 2: magic families (§8.1 MC-1) + version.
        for m in [*b"XGAP", *b"cgap", [0u8; 4]] {
            valid(&mut f);
            f[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&m);
            assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_INVALID_MAGIC, "row 2 magic {m:?}");
        }
        valid(&mut f);
        f[OFF_VERSION] = 2;
        assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_INVALID_MAGIC, "row 2 version");

        // Row 3a: all-zero digest.
        valid(&mut f);
        for b in f[OFF_OABI_DIGEST..OFF_OABI_DIGEST + OABI_DIGEST_LEN].iter_mut() {
            *b = 0;
        }
        assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_OABI_MISMATCH, "row 3a");

        // Row 3b: declared payload beyond the declared buffer geometry
        // (128 + 8); boundary exact — 9 > 8.
        valid(&mut f);
        f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8].copy_from_slice(&9u64.to_le_bytes());
        assert_eq!(
            ingress(&f, HEADER_LEN + 8, None),
            CGAP_ERR_OABI_MISMATCH,
            "row 3b overrun"
        );
        // u64 field that cannot exist as a usize (MC-2 overflow class).
        valid(&mut f);
        f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8]
            .copy_from_slice(&(u64::from(u32::MAX) + 1).to_le_bytes());
        assert_eq!(
            ingress(&f, HEADER_LEN + 8, None),
            CGAP_ERR_OABI_MISMATCH,
            "row 3b u64 overflow"
        );

        // Row 4: attestation magic families (§8.1 MC-3).
        for m in [*b"P29\0", [0u8; 4], *b"p28s"] {
            valid(&mut f);
            f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&m);
            assert_eq!(
                ingress(&f, f.len(), None),
                CGAP_ERR_P28_SPOLIATION,
                "row 4 att magic {m:?}"
            );
        }

        // Row 5: A17 §2.2/§6 field laws.
        let expect_spoil = |f: &Vec<u8>, why: &str| {
            assert_eq!(ingress(f, f.len(), None), CGAP_ERR_P28_SPOLIATION, "row 5 {why}");
        };
        valid(&mut f);
        f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&0x0008u16.to_le_bytes());
        expect_spoil(&f, "reserved att bit set");
        valid(&mut f);
        f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&0u16.to_le_bytes());
        expect_spoil(&f, "zero lineage bits");
        valid(&mut f);
        f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8]
            .copy_from_slice(&(P28_FLAG_ROOT | P28_FLAG_DERIVATIVE).to_le_bytes());
        expect_spoil(&f, "two lineage bits");
        valid(&mut f);
        f[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
            .copy_from_slice(&(P28_SAFE_HARBOR_CEILING_CENTS + 1).to_le_bytes());
        expect_spoil(&f, "ceiling exceeded");
        valid(&mut f);
        f[OFF_ATTESTATION + P28_RECEIPT_AT..OFF_ATTESTATION + P28_RECEIPT_AT + P28_RECEIPT_LEN]
            .copy_from_slice(&[7u8; 32]);
        expect_spoil(&f, "ROOT with nonzero receipt");
        valid(&mut f);
        f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8]
            .copy_from_slice(&P28_FLAG_DERIVATIVE.to_le_bytes());
        expect_spoil(&f, "DERIVATIVE with zero receipt");

        // Row 6: header flags bits 4-7 — no A19 code (§5.2); gate ADMITS per
        // the recorded divergence (A04 §6.3 "ignored on read").
        valid(&mut f);
        f[OFF_FLAGS] = FLAG_ARROW | 0xF0;
        assert_eq!(ingress(&f, f.len(), None), CGAP_OK, "row 6 divergence: admitted");

        // Valid frame admits and writes the zero-copy view (§2.3); the view
        // borrow is scoped so the buffer can be rebuilt for the next class.
        {
            let mut view = FrameView { payload: &[] };
            valid(&mut f);
            f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8]
                .copy_from_slice(&8u64.to_le_bytes());
            assert_eq!(ingress(&f, f.len(), Some(&mut view)), CGAP_OK, "admit");
            assert_eq!(view.payload, &f[HEADER_LEN..HEADER_LEN + 8]);
        }
        // Probe mode: out = None.
        valid(&mut f);
        assert_eq!(ingress(&f, f.len(), None), CGAP_OK, "probe (None) admits");
    }

    /// §4.5: the order is load-bearing — frames failing check k+1 but
    /// passing check k return the FIRST failing check's code. These cases
    /// discriminate every adjacent group pair, so a reorder shim flips at
    /// least one assertion (session shim receipt on top of this).
    #[test]
    fn order_pinning_first_failure_wins() {
        let mut f = vec![0u8; 512];
        let base = |f: &mut Vec<u8>| {
            f.fill(0);
            f[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&MAGIC);
            f[OFF_VERSION] = HEADER_VERSION;
            f[OFF_FLAGS] = FLAG_ARROW;
            f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8].copy_from_slice(&8u64.to_le_bytes());
            f[OFF_OABI_DIGEST] = 0xA5;
            f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&P28_LICENSE_MAGIC);
            f[OFF_ATTESTATION + 4..OFF_ATTESTATION + 6].copy_from_slice(&P28_VERSION.to_le_bytes());
            f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&P28_FLAG_ROOT.to_le_bytes());
            f[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
                .copy_from_slice(&P28_SAFE_HARBOR_CEILING_CENTS.to_le_bytes());
        };

        // truncated + bad magic -> row 1 first (length pre-check precedes).
        base(&mut f);
        f[0] = b'X';
        assert_eq!(ingress(&f[..127], 127, None), CGAP_ERR_INVALID_MAGIC);

        // geometry lie + bad magic -> row 1 class first.
        base(&mut f);
        f[0] = b'X';
        assert_eq!(ingress(&f[..200], 400, None), CGAP_ERR_INVALID_MAGIC);

        // bad magic + zeroed digest + broken att -> magic first.
        base(&mut f);
        f[0] = b'X';
        f[OFF_OABI_DIGEST..OFF_OABI_DIGEST + OABI_DIGEST_LEN].fill(0);
        f[OFF_ATTESTATION] = b'P';
        f[OFF_ATTESTATION + 1] = b'2';
        f[OFF_ATTESTATION + 2] = b'9';
        assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_INVALID_MAGIC);

        // §4.5's exact construction: valid magic + zeroed digest + broken
        // attestation -> OABI_MISMATCH (group 2), never SPOLIATION.
        base(&mut f);
        f[OFF_OABI_DIGEST..OFF_OABI_DIGEST + OABI_DIGEST_LEN].fill(0);
        f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(b"P29\0");
        assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_OABI_MISMATCH);

        // overrun len + broken attestation -> OABI (group 2) first.
        base(&mut f);
        f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8]
            .copy_from_slice(&(u64::from(u32::MAX) + 1).to_le_bytes());
        f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(b"P29\0");
        assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_OABI_MISMATCH);

        // valid magic + coherent digest + broken att -> SPOLIATION (group 3).
        base(&mut f);
        f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(b"P29\0");
        assert_eq!(ingress(&f, f.len(), None), CGAP_ERR_P28_SPOLIATION);
    }

    /// §6 content-independence corollary: the reject/admit decision reads
    /// only the first 128 bytes — identical header + different payloads (and
    /// payload byte CONTENT) classify identically. (Latency-within-noise is
    /// the bench row; TARGET-labeled, §8.4 — not asserted here.)
    #[test]
    fn content_independence_same_header_different_payloads() {
        let mut header = vec![0u8; HEADER_LEN];
        header[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&MAGIC);
        header[OFF_VERSION] = HEADER_VERSION;
        header[OFF_FLAGS] = FLAG_BN3;
        header[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8].copy_from_slice(&0u64.to_le_bytes());
        header[OFF_OABI_DIGEST] = 0x5A;
        header[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&P28_LICENSE_MAGIC);
        header[OFF_ATTESTATION + 4..OFF_ATTESTATION + 6].copy_from_slice(&P28_VERSION.to_le_bytes());
        header[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8]
            .copy_from_slice(&P28_FLAG_TRANSPILED.to_le_bytes());
        header[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
            .copy_from_slice(&P28_SAFE_HARBOR_CEILING_CENTS.to_le_bytes());
        // TRANSPILED carries a nonzero transpiler receipt.
        header[OFF_ATTESTATION + P28_RECEIPT_AT..OFF_ATTESTATION + P28_RECEIPT_AT + P28_RECEIPT_LEN]
            .copy_from_slice(&[0x11u8; 32]);

        // Same header; trailing buffers {empty, 1 KiB, 1 MiB} of varying
        // content — payload_byte_len = 0, so trailing bytes are slot slack
        // the gate must never read for the decision.
        let slack = [
            vec![],
            vec![0x00u8; 1024],
            vec![0xFFu8; 4096],
            (0u8..=255).cycle().take(1_048_576).collect::<Vec<u8>>(),
        ];
        let mut first_status = None;
        for (case, extra) in slack.iter().enumerate() {
            let mut frame = header.clone();
            frame.extend_from_slice(extra);
            let mut view = FrameView { payload: &[] };
            let st = ingress(&frame, frame.len(), Some(&mut view));
            assert_eq!(st, CGAP_OK, "case {case}: same header, different payload");
            assert!(view.payload.is_empty(), "case {case}: declared payload is empty");
            first_status = Some(match first_status {
                None => st,
                Some(prev) => {
                    assert_eq!(prev, st, "case {case}: classification is content-dependent");
                    prev
                }
            });
        }
        // And a reject verdict is equally content-independent: same corrupt
        // header over {empty, 1 KiB, 1 MiB} slack.
        header[OFF_MAGIC] = b'X';
        for extra in slack.iter() {
            let mut frame = header.clone();
            frame.extend_from_slice(extra);
            assert_eq!(
                ingress(&frame, frame.len(), None),
                CGAP_ERR_INVALID_MAGIC,
                "reject class is content-dependent"
            );
        }
    }

    /// §8.2 zero-alloc court over the full 10M seeded corpus: per-call
    /// `allocs == 0 && bytes == 0` under the scoped guard (corpus built
    /// outside the measured region, R3). Also covers DoD 4's class verdicts
    /// (0 misclassified) and the admit-path view law.
    #[test]
    fn dam_corpus_10m_zero_alloc_zero_misclassified() {
        run_corpus(CORPUS_SEED, CORPUS_LEN, true);
    }

    /// §8.2 anti-vacuity liveness (positive control, always run, no shim):
    /// the tracking guard + counting allocator are LIVE — an allocation
    /// inside the scope is counted. If this ever fails, the zero-alloc
    /// court above is proving nothing (R3b).
    #[test]
    fn alloc_tracking_hook_is_live() {
        let mut sink = Vec::new();
        let guard = TrackGuard::enter();
        sink.try_reserve(64).expect("reserve is an allocation");
        sink.push(1u8);
        let (allocs, bytes) = guard.counts();
        drop(guard);
        assert!(allocs >= 1, "tracked allocation not counted — court vacuous");
        assert!(bytes >= 64, "tracked bytes not counted — court vacuous");
        drop(sink);
    }

    /// The gate re-reads nothing it was not handed: declared `len` governs.
    /// A buffer with a LONGER slice than declared len admits on the declared
    /// geometry (DMA slot with slack) — and the view stays inside it.
    #[test]
    fn declared_len_governs_not_slice_length() {
        let mut f = vec![0u8; 1024];
        f[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&MAGIC);
        f[OFF_VERSION] = HEADER_VERSION;
        f[OFF_FLAGS] = FLAG_PARQUET;
        f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8].copy_from_slice(&16u64.to_le_bytes());
        f[OFF_OABI_DIGEST] = 0xA5;
        f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&P28_LICENSE_MAGIC);
        f[OFF_ATTESTATION + 4..OFF_ATTESTATION + 6].copy_from_slice(&P28_VERSION.to_le_bytes());
        f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&P28_FLAG_ROOT.to_le_bytes());
        f[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
            .copy_from_slice(&P28_SAFE_HARBOR_CEILING_CENTS.to_le_bytes());
        let mut view = FrameView { payload: &[] };
        // Declared len = 144 (128 hdr + 16 payload); slice holds 1024.
        assert_eq!(ingress(&f, HEADER_LEN + 16, Some(&mut view)), CGAP_OK);
        assert_eq!(view.payload.len(), 16);
        // Declared len = 143: payload 16 no longer fits the declared
        // geometry -> row 3b, even though the SLICE could hold it.
        assert_eq!(ingress(&f, HEADER_LEN + 15, None), CGAP_ERR_OABI_MISMATCH);
    }

    // -- Wave 4: hardening courts (process-counter witness, cross-file ------
    // -- drift pins, status-register purity) --------------------------------

    /// Wave 4 (R3b), Wave 6 repair: allocator-hook witness — the zero-alloc
    /// law witnessed one level down, at the `#[global_allocator]` itself,
    /// with NO scoped tracking guard in play. The counters read here are
    /// THIS TEST'S OWN THREAD's always-on allocator-hook counters: a
    /// bare-call bracket over process-wide atomics false-fails under the
    /// default parallel harness (a concurrent test's allocation lands in
    /// the bracket — witnessed 2026-11-01, delta (1, 59), fail/pass/pass
    /// across three runs), so the witness is per-thread, which is immune
    /// by construction. Proves both that the counting-allocator install is
    /// live at the call site and that the gate allocates nothing on admit
    /// or reject. Complements `alloc_tracking_hook_is_live` (scoped-guard
    /// liveness) with unscoped allocator-hook liveness; the in-court
    /// liveness step at the end keeps THIS witness non-silent-zero (R3b).
    #[test]
    fn allocator_hook_witness_bare_call_zero_alloc() {
        // Minimal valid frame (ROOT lineage, zero receipt); all subsequent
        // mutations are IN PLACE so every measured bracket contains only
        // the `ingress` call and nothing that can allocate.
        let mut f = vec![0u8; 512];
        f[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&MAGIC);
        f[OFF_VERSION] = HEADER_VERSION;
        f[OFF_FLAGS] = FLAG_ARROW;
        f[OFF_PAYLOAD_BYTE_LEN..OFF_PAYLOAD_BYTE_LEN + 8].copy_from_slice(&16u64.to_le_bytes());
        f[OFF_OABI_DIGEST] = 0xA5;
        f[OFF_ATTESTATION..OFF_ATTESTATION + 4].copy_from_slice(&P28_LICENSE_MAGIC);
        f[OFF_ATTESTATION + 4..OFF_ATTESTATION + 6].copy_from_slice(&P28_VERSION.to_le_bytes());
        f[OFF_ATTESTATION + 6..OFF_ATTESTATION + 8].copy_from_slice(&P28_FLAG_ROOT.to_le_bytes());
        f[OFF_ATTESTATION + 8..OFF_ATTESTATION + 16]
            .copy_from_slice(&P28_SAFE_HARBOR_CEILING_CENTS.to_le_bytes());
        let mut view = FrameView { payload: &[] };

        // ADMIT path: process-wide counters must not move.
        let a0 = THREAD_ALLOCS.with(|c| c.get());
        let b0 = THREAD_ALLOC_BYTES.with(|c| c.get());
        let st = ingress(&f, HEADER_LEN + 16, Some(&mut view));
        let a1 = THREAD_ALLOCS.with(|c| c.get());
        let b1 = THREAD_ALLOC_BYTES.with(|c| c.get());
        assert_eq!(st, CGAP_OK, "admit witness frame must admit");
        assert_eq!(
            (a1, b1),
            (a0, b0),
            "thread-hook allocator counters moved across ingress (admit) — the gate allocated"
        );

        // Row 2 reject (magic flip, in place; declared len 512 > 128 so the
        // geometry pre-check passes and the magic compare is reached).
        f[0] = b'X';
        let a0 = THREAD_ALLOCS.with(|c| c.get());
        let b0 = THREAD_ALLOC_BYTES.with(|c| c.get());
        let st = ingress(&f, 512, None);
        let a1 = THREAD_ALLOCS.with(|c| c.get());
        let b1 = THREAD_ALLOC_BYTES.with(|c| c.get());
        assert_eq!(st, CGAP_ERR_INVALID_MAGIC, "row 2 reject code");
        assert_eq!(
            (a1, b1),
            (a0, b0),
            "row 2: thread-hook counters moved across ingress — the gate allocated"
        );
        f[0] = b'C';

        // Row 3 reject (no mutation: a declared buffer of 128+8 cannot hold
        // the header's declared 16-byte payload -> OABI mismatch).
        let a0 = THREAD_ALLOCS.with(|c| c.get());
        let b0 = THREAD_ALLOC_BYTES.with(|c| c.get());
        let st = ingress(&f, HEADER_LEN + 8, None);
        let a1 = THREAD_ALLOCS.with(|c| c.get());
        let b1 = THREAD_ALLOC_BYTES.with(|c| c.get());
        assert_eq!(st, CGAP_ERR_OABI_MISMATCH, "row 3 reject code");
        assert_eq!(
            (a1, b1),
            (a0, b0),
            "row 3: thread-hook counters moved across ingress — the gate allocated"
        );

        // Row 4 reject (attestation magic flip, in place).
        f[OFF_ATTESTATION] = b'P';
        f[OFF_ATTESTATION + 1] = b'2';
        f[OFF_ATTESTATION + 2] = b'9';
        let a0 = THREAD_ALLOCS.with(|c| c.get());
        let b0 = THREAD_ALLOC_BYTES.with(|c| c.get());
        let st = ingress(&f, 512, None);
        let a1 = THREAD_ALLOCS.with(|c| c.get());
        let b1 = THREAD_ALLOC_BYTES.with(|c| c.get());
        assert_eq!(st, CGAP_ERR_P28_SPOLIATION, "row 4 reject code");
        assert_eq!(
            (a1, b1),
            (a0, b0),
            "row 4: thread-hook counters moved across ingress — the gate allocated"
        );

        // Witness liveness (R3b, in-court positive control, no shim): a
        // deliberate allocation on THIS thread must move the counters the
        // brackets just read. If it does not, this witness is silent-zero
        // and proves nothing — fail instead.
        let a2 = THREAD_ALLOCS.with(|c| c.get());
        let mut sink: Vec<u8> = Vec::new();
        sink.try_reserve(64).expect("reserve is an allocation");
        assert!(
            THREAD_ALLOCS.with(|c| c.get()) > a2,
            "allocator-hook counters did not count a live allocation — \
             the bare-call witness above is silent-zero (R3b), court vacuous"
        );
        drop(sink);
    }

    /// Wave 4 (R4 / DoD 2 mitigation): cross-file drift court. The
    /// canonical `CgapFrameHeader` type is owned by lane A04 in cgap-core;
    /// this gate cannot re-import it (off-limits Cargo.toml, divergence 3),
    /// so the frozen mirror is bound to the canonical file's own
    /// compile-time pin table, read at test time. If A04's canonical layout
    /// drifts (size/align/offsets), this court fails loudly and localizes
    /// the drift to `frame.rs` (A04 §6.4's own law: drift is a defect in
    /// `frame.rs`).
    #[test]
    fn canonical_layout_pins_match_cgap_core_frame_rs() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../cgap-core/src/frame.rs");
        let src = std::fs::read_to_string(path)
            .expect("canonical frame.rs must be readable at test time (repo-local file)");
        const PINS: [&str; 10] = [
            "size_of::<CgapFrameHeader>() == 128",
            "align_of::<CgapFrameHeader>() == 64",
            "size_of::<P28LicenseAttestation>() == 80",
            "align_of::<P28LicenseAttestation>() == 8",
            "offset_of!(CgapFrameHeader, magic) == 0",
            "offset_of!(CgapFrameHeader, payload_byte_len) == 8",
            "offset_of!(CgapFrameHeader, oabi_digest) == 16",
            "offset_of!(CgapFrameHeader, attestation) == 48",
            "offset_of!(P28LicenseAttestation, license_magic) == 0",
            "offset_of!(P28LicenseAttestation, upstream_receipt) == 48",
        ];
        for pin in PINS {
            assert!(
                src.contains(pin),
                "canonical A04 layout pin drifted in cgap-core/src/frame.rs: {pin}"
            );
        }
    }

    /// Wave 4 (DoD 10), Wave 5 repair (A20 wave-3 audit, mutation M9):
    /// status-register purity court, mechanical. Parses every
    /// `pub const CGAP_*` declaration in this file and in A19's
    /// register implementation (`cgap-ffi/src/sovereign.rs`), then asserts
    /// (a) the gate's status-constant set is EXACTLY the four codes the
    /// lane doc §4 names — any minted code flips this court — and (b) each
    /// code is bound THREE-WAY: gate mirror == the A19 register's
    /// NORMATIVE literal (the register of record: lane doc A19 §"C
    /// signatures (normative)") AND the ffi file == the same literal.
    /// Wave-4's form bound gate -> ffi only; the A20 audit's M9 shim
    /// witnessed the residual blind-spot class (a drifted constant
    /// compared to itself stays green), and the file-vs-file bind still
    /// passed under a joint (copy-pasted) drift. The normative-literal pin
    /// closes the class: any single-file or joint drift flips this court
    /// locally, with no cross-crate court required. Re-export stays
    /// blocked by the off-limits Cargo.toml (divergence 3); purity is
    /// enforced at value level until the dependency can land.
    #[test]
    fn status_register_is_pure_a19_subset() {
        let gate_src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/penstock.rs"
        ))
        .expect("penstock.rs must be readable at test time (repo-local file)");
        let ffi_src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../cgap-ffi/src/sovereign.rs"
        ))
        .expect("sovereign.rs must be readable at test time (repo-local file)");

        fn parse(src: &str) -> Vec<(String, u32)> {
            let mut out = Vec::new();
            for line in src.lines() {
                let t = line.trim_start();
                let Some(rest) = t.strip_prefix("pub const CGAP_") else {
                    continue;
                };
                let Some((name, ty)) = rest.split_once(':') else {
                    continue;
                };
                let Some((_, val)) = ty.split_once('=') else {
                    continue;
                };
                let hex = val
                    .trim()
                    .trim_end_matches(';')
                    .trim()
                    .replace('_', "");
                let Ok(v) = u32::from_str_radix(hex.trim_start_matches("0x"), 16) else {
                    continue;
                };
                out.push((format!("CGAP_{name}"), v));
            }
            out
        }

        let mut gate_reg = parse(&gate_src);
        gate_reg.sort();
        let mut expected: Vec<(String, u32)> = [
            ("CGAP_OK", CGAP_OK),
            ("CGAP_ERR_INVALID_MAGIC", CGAP_ERR_INVALID_MAGIC),
            ("CGAP_ERR_OABI_MISMATCH", CGAP_ERR_OABI_MISMATCH),
            ("CGAP_ERR_P28_SPOLIATION", CGAP_ERR_P28_SPOLIATION),
        ]
        .iter()
        .map(|(n, v)| (n.to_string(), *v))
        .collect();
        expected.sort();
        assert_eq!(
            gate_reg, expected,
            "gate status register is not exactly the four §4 codes — a code was minted or renamed"
        );

        // The register of record (A19 §"C signatures (normative)"): the
        // normative literals cited verbatim. Any edit here is an A19
        // register amendment and must follow A19's ruling (§5.2 falsifier
        // law), never lead it.
        const NORMATIVE: [(&str, u32); 4] = [
            ("CGAP_OK", 0x0000_0000),
            ("CGAP_ERR_INVALID_MAGIC", 0x4347_0001),
            ("CGAP_ERR_OABI_MISMATCH", 0x4347_0002),
            ("CGAP_ERR_P28_SPOLIATION", 0x5032_38FF),
        ];
        let ffi_reg = parse(&ffi_src);
        for (name, normative) in NORMATIVE.iter() {
            let gate_hit = gate_reg.iter().find(|(n, _)| n == name);
            let ffi_hit = ffi_reg.iter().find(|(n, _)| n == name);
            assert!(
                gate_hit.is_some(),
                "gate status {name} missing from penstock.rs — the mirror must carry all four codes"
            );
            assert!(
                ffi_hit.is_some(),
                "A19 register {name} missing from cgap-ffi/src/sovereign.rs — register drifted"
            );
            assert_eq!(
                gate_hit.unwrap().1,
                *normative,
                "gate status {name} drifted from the A19 normative literal (the M9 blind spot, now pinned)"
            );
            assert_eq!(
                ffi_hit.unwrap().1,
                *normative,
                "A19 register {name} drifted from its own normative literal (cgap-ffi/src/sovereign.rs)"
            );
        }
    }

    // -- §8.4 throughput: ratio fence against the null-check baseline ------

    /// §8.4 null-check baseline: "only the single 4-byte magic compare (the
    /// cheapest lawful check that still rejects garbage)" plus the row-1
    /// geometry pre-check it cannot exist without. Same call shape as
    /// [`ingress`], same harness, same corpus — the only delta is the check
    /// chain.
    fn null_baseline(bytes: &[u8], len: usize) -> u32 {
        if len < HEADER_LEN || bytes[OFF_MAGIC..OFF_MAGIC + MAGIC.len()] != MAGIC {
            CGAP_ERR_INVALID_MAGIC
        } else {
            CGAP_OK
        }
    }

    /// §8.4 ratio fence (DoD 7): full penstock vs null-check baseline over
    /// the same seeded corpus batch, min-of-N PAIRED rounds (the ratio-fence
    /// law's preferred fence — never an absolute-ms bound). The < 4 ns/frame
    /// absolute TARGET is measured and REPORTED, never asserted: conformance
    /// outranks throughput (§8.4 — the exit gate is the dam court, never the
    /// nanoseconds). Corpus subset: frames <= 384 B (all valid/mutation
    /// frames plus short garbage), packed contiguously ONCE outside every
    /// measured region (R3); the subset is a deterministic function of the
    /// corpus seed.
    #[test]
    fn throughput_ratio_fence_null_baseline() {
        use std::hint::black_box;
        use std::time::Instant;

        // §8.4's method measures the gate constant under optimized build
        // flags. A debug-profile binary measures the compiler, not the gate
        // (dev witness 2026-11-01: 23.44 ns/frame for the 4-byte null
        // baseline alone, 3.74x full/baseline ratio — unoptimized overhead
        // dominates both sides). Skip LOUDLY outside release; the fence is
        // enforced by `cargo test -p cgap-gate --release`.
        if cfg!(debug_assertions) {
            println!(
                "[SKIP] throughput ratio fence: debug profile — measurement is a compiler \
                 artifact, not the gate constant; enforce via `cargo test -p cgap-gate --release`"
            );
            return;
        }

        const BENCH_FRAMES: u64 = 200_000;
        const PAIRED_ROUNDS: usize = 11;
        const MAX_FRAME: usize = 384;

        // Deterministic corpus batch — same generator as the dam court —
        // built once, outside the measured region.
        let mut scratch = vec![0u8; 4096];
        let mut buf: Vec<u8> = Vec::with_capacity(1 << 22);
        let mut table: Vec<(u32, u32)> = Vec::with_capacity(BENCH_FRAMES as usize);
        for i in 0..BENCH_FRAMES {
            let (frame_len, _) = gen_frame(CORPUS_SEED, i, &mut scratch);
            if frame_len <= MAX_FRAME {
                table.push((buf.len() as u32, frame_len as u32));
                buf.extend_from_slice(&scratch[..frame_len]);
            }
        }
        assert!(
            table.len() >= BENCH_FRAMES as usize / 2,
            "bench subset degenerate: only {} of {BENCH_FRAMES} frames fit {MAX_FRAME} B",
            table.len()
        );

        // Min-of-N paired rounds: a base round then a full round, repeated,
        // each side's minimum kept (host drift cancels across the pair).
        let mut base_min = u64::MAX;
        let mut full_min = u64::MAX;
        for _ in 0..PAIRED_ROUNDS {
            let t = Instant::now();
            let mut acc = 0u32;
            for &(off, len) in &table {
                acc = acc.wrapping_add(null_baseline(
                    &buf[off as usize..off as usize + len as usize],
                    len as usize,
                ));
            }
            black_box(acc);
            base_min = base_min.min(t.elapsed().as_nanos() as u64);

            let t = Instant::now();
            let mut acc = 0u32;
            for &(off, len) in &table {
                acc = acc.wrapping_add(ingress(
                    &buf[off as usize..off as usize + len as usize],
                    len as usize,
                    None,
                ));
            }
            black_box(acc);
            full_min = full_min.min(t.elapsed().as_nanos() as u64);
        }

        let n = table.len() as f64;
        let base_ns = base_min as f64 / n;
        let full_ns = full_min as f64 / n;
        let ratio = full_ns / base_ns;

        // Measurement validity (the fence's hard teeth): a degenerate
        // subset, a collapsed baseline, or a full chain measured no more
        // expensive than its own cheapest member means the harness — not
        // the gate — is broken. Then the §8.4 gross-regression ceiling: the
        // gate must stay within a constant factor of its cheapest lawful
        // equivalent. 3x was the Wave-3 pick; measured 2026-11-01
        // (release, this host, min-of-11 paired rounds, 100k+ frame
        // subset): the honest constant is ~3.1x (4.89 ns/frame full vs
        // 1.57 ns/frame baseline) — marginally above 3x, with < 4 ns
        // equally missed — so 3x is kept as the §8.4 TARGET row (GAP named,
        // never rounded up) and a 10x ceiling is enforced as the
        // gross-regression fence: a reorder-to-quadratic or bounds-check
        // blowup still flips this court.
        assert!(
            table.len() >= BENCH_FRAMES as usize / 2 && base_ns > 0.0 && full_ns >= base_ns,
            "bench harness degenerate: subset {} frames, base {base_ns} ns, full {full_ns} ns",
            table.len()
        );
        const GROSS_REGRESSION_CEILING: f64 = 10.0;
        assert!(
            ratio <= GROSS_REGRESSION_CEILING,
            "ratio fence violated: full penstock {full_ns:.2} ns/frame is {ratio:.2}x \
             the null-check baseline {base_ns:.2} ns/frame (ceiling {GROSS_REGRESSION_CEILING}x, §8.4)"
        );

        println!("penstock bench: subset = {} frames", table.len());
        println!(
            "null-baseline {base_ns:.2} ns/frame | full penstock {full_ns:.2} ns/frame | ratio {ratio:.2}x"
        );
        let target_abs = full_ns < 4.0;
        let target_ratio = ratio <= 3.0;
        println!(
            "§8.4 TARGET row: < 4 ns/frame = {} | <= 3x baseline = {} | measured {full_ns:.2} ns, {ratio:.2}x",
            if target_abs { "PASS" } else { "GAP" },
            if target_ratio { "PASS" } else { "GAP" },
        );
    }
}
