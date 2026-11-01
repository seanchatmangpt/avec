//! FIBO O-ABI pack facade — lane A09, Wave 1 (ABI freeze).
//!
//! Wave-1 scope: struct/const definitions, size/align static asserts, zero
//! logic. Every identity is either cited from A06 (SlotId/ClassId/EnumId/
//! LexHash widths, `.oabi` v1 container — this lane consumes, never
//! redefines) or derived, never hand-minted: every `*_SLOT` value is
//! `u64::from_le_bytes(BLAKE3(b"bont\x01" ++ lexeme)[0..8])`. Values were
//! emitted by `tests/wave1_derivation.rs` (collision-checked generator) and
//! are re-derived on every run by `tests/wave1_witness.rs` through the real
//! in-repo substrate `cgap_blake3::hash`; a mistyped byte fails the witness.
//!
//! Wave 2-3 added the harvester/emitter (`src/bin/emit-pack.rs`) and the
//! emitted `fibo_sec_dbt_v1.oabi`. Wave 4 committed `PACK_DIGEST` (lane
//! doc §7) at emit time — never a placeholder — plus the §8 courts T7-T12
//! in `tests/wave4_courts.rs`. Provenance (lane doc §5): release id frozen
//! below; the distribution SHA-256 is `UNVERIFIED` until the pinned
//! distribution artifact is admitted (see
//! [`PACK_PROVENANCE_DISTRIBUTION_SHA256`]).
//!
//! Wave 6 repaired the Wave-2 emitter's bin-target name: `emit-pack`
//! collided with lane A10's identically-named bin (both workspace members
//! link one shared `target/debug/emit-pack` path, so a joint invocation
//! left A10's binary at the path and fibo courts t9b/t9c/t11 executed the
//! wrong emitter — witnessed failing, exit 101, 2026-11-01). The target is
//! now `emit-fibo-pack` (source path unchanged); the exactness courts
//! T9b/T9c/T11 are the flipping courts that pin the repair.
//!
//! Wave 7 executed the audit option (b) residue the wave-6 receipts left
//! open with this lane (`docs/jira/v26.11.1/waves/a07-wave6-audit-receipt.md`
//! §5, open item 1; also named in the zero-string gate's EMITTER_BINARIES
//! justification): producer-side quarantine of the emitter's own strings.
//! The §2.2-refused probe corpus and the allowed control no longer compile
//! into `emit-fibo-pack` — refused vocabulary has no place in a shipped
//! artifact; the T11 court owns the probes and drives them through
//! `--probe <lexeme>` argv, and the new T13 quarantine court pins the
//! repaired binary byte-level (zero refused lexemes embedded; corpus
//! positive-control keeps the scan non-vacuous). The frozen corpus itself
//! stays embedded: it is the producer's input (ADR-002 PRODUCER role),
//! scoped by the gate's keyed EMITTER_BINARIES exemption.
//!
//! Wave 8 (defect repair driven by the wave-7-era court receipts) repaired
//! two receipt-named defect classes and pinned both mechanically: (1) the
//! A06 line citations here and in the emitter had gone stale a second
//! time (`oabi.rs:47` -> actual `:61`; emitter `:44-65` -> actual
//! `:58-78` — A06's file moved DURING the wave-8 session, live proof of
//! the rot) — repaired and pinned by
//! `tests/wave8_courts.rs::a06_citation_freshness_court`, which re-derives
//! the decl lines from the real `crates/cgap-bont/src/oabi.rs` every run;
//! (2) the T13 flip witness cited a "Wave-7 lane receipt" file that does
//! not exist on disk (D-A08-1 absent-artifact class) — the citation is
//! repaired and the flip re-executed in a /tmp sandbox with the named
//! mutation class (see the T13 docstring and the wave-8 lane report).
//!
//! Wave 9 (2026-11-01, cross-lane integration slice: "A04 identity court,
//! A14 dam over A04 frames, A20 1M-row stress, A05 power-cut sweep, A07
//! zero-string gate over all built crates") — the fibo side of each item,
//! landed as `tests/wave9_integration_courts.rs` plus this receipt. All
//! commands below ran THIS session against the canonical tree
//! (`CARGO_TARGET_DIR=$PWD/target-a09` unless stated):
//!
//! - **A04 identity court** (`w9_a04_frame_identity_court`): the committed
//!   pack framed by A04's 128 B `CgapFrameHeader` (A17's 80 B attestation,
//!   root lineage), encode/decode/validate round trip, tier identity via
//!   A04's real `identity_assert`, payload digest verified with the real
//!   BLAKE3, and the cross-lane identity join
//!   `header.oabi_digest == PACK_DIGEST` pinned both directions. Seven
//!   in-court witnesses on real mutated bytes (DigestMismatch / BadMagic /
//!   BadVersion / ReservedBitsSet / AttestationMismatch / tier+offset
//!   mismatch / PayloadLenMismatch), plus the 128/80 width pins.
//! - **A14 dam over A04 frames**: UNSUPPORTED for this lane, by design
//!   (typed, not a skip): the penstock dam gates frame *flows* through
//!   `crates/cgap-gate`; a const O-ABI pack has no flow surface. The
//!   A04-frame half is courted above; re-opens only if the pack ever
//!   becomes a framed payload in transport.
//! - **A20 1M-row stress** (`w9_a20_1m_row_stress_court`): A06's real
//!   loader over a 1,000,000-record (48 MiB) derived synthetic container —
//!   full validation + per-record derivation sweep + byte-exact wire
//!   round-trip; four corrupted/truncated witnesses refused typed with
//!   exact indices. Advisory timing (dev profile, stated method: whole-
//!   court wall clock): build 2612 ms, load+sweep 173 ms. Correctness at
//!   scale is the gate; timing is never a gate.
//! - **A05 power-cut sweep** (`w9_a05_powercut_sweep_court`): every
//!   truncation prefix and every single-byte corruption of the real pack
//!   refused by the conjunction of identity gates; the pad-byte layering
//!   witness proves the digest anchor catches what the structural loader
//!   cannot see. Producer repair this wave: the emitter's first-emit
//!   write is now crash-consistent (temp + `sync_all` + `rename(2)`) —
//!   executed in the sandbox on a fresh copy: exit 0, emitted bytes
//!   sha256 `31f754549f562cc9…` == committed pack, no `*.w9tmp` orphan,
//!   `--check` exit 0 after.
//! - **A07 zero-string gate**: `python3 scripts/zero_string_gate.py` ->
//!   exit 0 (scanned=49, crates=22, hits=0, emitter_hits=2 = the keyed
//!   EMITTER_BINARIES producer exemption, vocab=9, source=graph-derivation),
//!   plus the in-repo facade token census court (`w9_a07…`) pinning the
//!   exact 9-token surface. `RUSTFLAGS="-D warnings" cargo check -p
//!   cgap-fibo --all-targets` -> exit 0. Full lane suite: 20/20, exit 0.
//! - **Anti-vacuity matrix** (fail-closed sandbox `/tmp/a09w9-sb2`, saved-
//!   copy restores, sha256-verified after every row; all flips exit 101 or
//!   typed exit 4): M-A04 zeroing the frame digest field in
//!   `cgap-core/src/frame.rs` -> w9_a04 FAILED (round-trip pin); M-A20b
//!   disabling the loader derivation check in `cgap-bont/oabi.rs` ->
//!   w9_a20 FAILED (corrupted-row witness); M-A07 injecting a refused
//!   token into the facade -> w9_a07 census FAILED; M-PACK flipping pack
//!   byte 100 -> t7 FAILED + `--check` exit 4 (typed PackDrift, first
//!   differing byte @100) + w9_a05 anchor FAILED; post-restore re-prove
//!   4/4 green, every restore sha256 byte-exact.
//! - **Disclosed process defects this wave**: (1) the first mutation-
//!   driver run was VOID — it ran `cargo test` from the canonical cwd, so
//!   its "flips" were the canonical green suite (no canonical file was
//!   mutated — the driver only mutates /tmp paths; the run rebuilt cgap-
//!   fibo artifacts into the shared default `target/`, build artifacts
//!   only); the matrix above is the corrected re-run. (2) A first M-A20
//!   mutation form did not compile (E0282) — a compile-abort, not a
//!   flip; replaced by the compiling M-A20b form. (3) Sandbox is a flat
//!   4-crate closure with manifest path-wiring (`../../crates/*` ->
//!   `../*`), the canonical LICENSE.md copied in, and cgap-core's
//!   cgap-gate dev-dep elided (gate crate not in closure; dev-dep only
//!   affects cgap-core's own test targets). The wave-8
//!   `a06_citation_freshness_court` is canonical-pinned (`../../crates/…`
//!   read) and cannot run in the flat sandbox — expected layout
//!   limitation, not a court defect.
//! - **Divergence (recorded, not silent)**: no
//!   `docs/jira/v26.11.1/waves/a09-wave9-audit-receipt.md` was written —
//!   this lane's dispatch restricts writes to `ontologies/fibo/`; sibling
//!   lanes' wave-9 receipts live in `waves/`, so this module doc is the
//!   lane receipt (A09's convention since wave 5). The dispatch's slice
//!   text ("Wave 9 - cross-lane integration…") is the authority for this
//!   wave's shape; the lane doc §11 still names the wave-2 producer
//!   window — the doc yields.

#![no_std]

/// Pinned FIBO release id (PACK_PROVENANCE, lane doc §5).
pub const FIBO_RELEASE: &str = "FBC-2024Q4";

/// Full-pack BLAKE3 digest — the identity of `fibo_sec_dbt_v1.oabi`
/// (lane doc §6/§7). Verified at test time by court T7
/// (`tests/wave4_courts.rs`) and after every edit by
/// `emit-fibo-pack --check` (drift -> exit 4). Any pack byte change flips
/// this.
///
/// Value: `d7495fba8022ee1944243b92518de4a3a729009d12e34cb41ada67789fd68067`
/// (printed by `emit-pack` at Wave-4 emit, 2026-11-01).
pub const PACK_DIGEST: [u8; 32] = [
    0xD7, 0x49, 0x5F, 0xBA, 0x80, 0x22, 0xEE, 0x19, //
    0x44, 0x24, 0x3B, 0x92, 0x51, 0x8D, 0xE4, 0xA3, //
    0xA7, 0x29, 0x00, 0x9D, 0x12, 0xE3, 0x4C, 0xB4, //
    0x1A, 0xDA, 0x67, 0x78, 0x9F, 0xD6, 0x80, 0x67,
];

/// PACK_PROVENANCE, distribution half (lane doc §5.1): SHA-256 of the
/// pinned FBC-2024Q4 distribution zip. The pinned distribution is NOT
/// present on this host, so the digest is recorded as a status, never a
/// fabricated hash — this is the DoD-3 BLOCKED residue, recorded in place
/// (this const is the record; no separate lane-report file exists):
/// everything else about the pack (release id above, pack digest above)
/// is derived and courted.
pub const PACK_PROVENANCE_DISTRIBUTION_SHA256: &str = "UNVERIFIED:no-distribution-on-disk";

// --- A06 ABI widths: cited, not redefined (A06 §2/§6; on disk at
// --- `crates/cgap-bont/src/oabi.rs`). --------------------------------------

/// SlotId width (A06 §2: `truncate64(BLAKE3(lexeme))`, d[0..8] LE over
/// namespace tag `b"bont\x01"`).
pub const SLOT_ID_BITS: usize = 64;

/// LexHash width (A06 §2: 256-bit BLAKE3 preimage witness;
/// `SlotId == low64(LexHash)`).
pub const LEX_HASH_LEN: usize = 32;

/// `.oabi` v1 container record size (A06 §6; `OabiRecordV1`, NOT the
/// facade record shapes below).
pub const OABI_V1_RECORD_SIZE: usize = 48;

/// `.oabi` v1 header length (A06 §6; `OABI_HEADER_LEN` at
/// `crates/cgap-bont/src/oabi.rs:61` — line freshened and mechanically
/// pinned in wave 8: `tests/wave8_courts.rs::a06_citation_freshness_court`
/// re-derives the decl line from the real file on disk every run, so a
/// stale citation is a failing test, not prose. History: the `:48`
/// citation went stale after A06's edits (repaired by hand in wave 6),
/// then `:47` went stale the same way (repaired + pinned in wave 8) — the
/// line-citation rot class the a07-wave5 audit receipt names, which is why
/// the pin is mechanical and not another hand edit).
pub const OABI_V1_HEADER_LEN: usize = 64;

/// ClassId width (A06 §2).
pub const CLASS_ID_BITS: usize = 32;

/// EnumId width (A06 §2).
pub const ENUM_ID_BITS: usize = 16;

// --- Dense ClassId space: lane-assigned in harvest order (lane doc §4),
// --- frozen; append-only at the end (§10), renumber = pack version bump. --

/// ClassId 0 — enum-family terms (currencies) with no FIBO class row.
pub const CLASS_NONE: u32 = 0x0000_0000;

/// `fibo:SecDbt:CorporateBond`
pub const CLASS_CORP_BOND: u32 = 0x0000_0001;

/// `fibo:SecDbt:Bond`
pub const CLASS_BOND: u32 = 0x0000_0002;

/// `fibo:SecDbt:DebtInstrument`
pub const CLASS_DEBT_INSTR: u32 = 0x0000_0003;

/// `fibo:ClAcct:AccountsReceivable`
pub const CLASS_ACCT_RECV: u32 = 0x0000_0010;

/// `fibo:SecDbt:hasMaturityDate`
pub const ATTR_MATURITY_DATE: u32 = 0x0000_0020;

/// `fibo:SecDbt:hasCouponRate`
pub const ATTR_COUPON_RATE: u32 = 0x0000_0021;

// --- Currency enum family (EnumId = u16, A06 §2). Discriminants are the
// --- lane doc §4 proposed freeze values; T8 verifies both directions. ------

/// Currency enum family — `EnumId = u16` per A06 §2.
#[repr(u16)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FiboCurrency {
    /// ISO-4217 `USD` — `fibo:FndUtlCur:USD`
    Usd = 0x0348,
    /// ISO-4217 `EUR` — `fibo:FndUtlCur:EUR`
    Eur = 0x0349,
    /// ISO-4217 `JPY` — `fibo:FndUtlCur:JPY`
    Jpy = 0x034A,
}

/// Class slot — `#[repr(C, align(8))]` wrapper keeping slot tables
/// mmap-reinterpretable (lane doc §7).
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClassSlot(pub u64);

// --- SlotId consts: derived, harvest order (lane doc §4). Each value is
// --- `u64::from_le_bytes(BLAKE3(b"bont\x01" ++ lexeme)[0..8])`; the LEX_*
// --- arrays are the full 32-byte preimage witnesses (A06 §2).

/// `fibo:FndUtlCur:USD`
pub const USD_SLOT: u64 = 0xE508EC39DB7ECC28;

/// Preimage witness for [`USD_SLOT`] (A06 §2: SlotId is its low 64 bits).
pub const LEX_USD: [u8; LEX_HASH_LEN] = [
    0x28, 0xCC, 0x7E, 0xDB, 0x39, 0xEC, 0x08, 0xE5, //
    0xC1, 0x8B, 0x7F, 0xB7, 0xF8, 0x7D, 0x3E, 0x1B, //
    0x73, 0xB2, 0x9C, 0xE8, 0x74, 0x72, 0x30, 0x67, //
    0xB0, 0x31, 0x83, 0x74, 0xF7, 0xC7, 0xE8, 0x45,
];

/// `fibo:FndUtlCur:EUR`
pub const EUR_SLOT: u64 = 0x5BE61A3EB4B24583;

/// Preimage witness for [`EUR_SLOT`].
pub const LEX_EUR: [u8; LEX_HASH_LEN] = [
    0x83, 0x45, 0xB2, 0xB4, 0x3E, 0x1A, 0xE6, 0x5B, //
    0x1C, 0x8D, 0x89, 0xE2, 0x0A, 0xBE, 0xB3, 0x1F, //
    0x47, 0xFF, 0xD8, 0x28, 0x4C, 0x95, 0x6C, 0xCA, //
    0x9D, 0x8D, 0x3A, 0x53, 0xEB, 0x03, 0xA1, 0x18,
];

/// `fibo:FndUtlCur:JPY`
pub const JPY_SLOT: u64 = 0xD558F6825C8F0108;

/// Preimage witness for [`JPY_SLOT`].
pub const LEX_JPY: [u8; LEX_HASH_LEN] = [
    0x08, 0x01, 0x8F, 0x5C, 0x82, 0xF6, 0x58, 0xD5, //
    0xC7, 0x72, 0xCD, 0xEB, 0xA9, 0xD8, 0xB7, 0xDD, //
    0x39, 0xB6, 0xA9, 0x0F, 0x4A, 0x6A, 0xF7, 0xD8, //
    0xED, 0xF9, 0x95, 0xE5, 0x0D, 0x21, 0x8B, 0xF0,
];

/// `fibo:SecDbt:CorporateBond`
pub const CORP_BOND_SLOT: u64 = 0x0021B3425CD01316;

/// Preimage witness for [`CORP_BOND_SLOT`].
pub const LEX_CORP_BOND: [u8; LEX_HASH_LEN] = [
    0x16, 0x13, 0xD0, 0x5C, 0x42, 0xB3, 0x21, 0x00, //
    0xE0, 0xB0, 0x12, 0xA3, 0xDD, 0xD9, 0x81, 0xBA, //
    0x7D, 0x77, 0x85, 0x43, 0xD8, 0x6D, 0x78, 0xB4, //
    0xC3, 0xF0, 0xE9, 0xDA, 0x1A, 0x39, 0x1F, 0xA7,
];

/// `fibo:SecDbt:Bond` (parent class).
pub const BOND_SLOT: u64 = 0x1448E20B35D39961;

/// Preimage witness for [`BOND_SLOT`].
pub const LEX_BOND: [u8; LEX_HASH_LEN] = [
    0x61, 0x99, 0xD3, 0x35, 0x0B, 0xE2, 0x48, 0x14, //
    0x7F, 0x4E, 0x18, 0xB3, 0x93, 0xB6, 0x14, 0xD4, //
    0x11, 0x04, 0xDC, 0xC2, 0x76, 0x37, 0xAA, 0x6B, //
    0x39, 0x35, 0x3B, 0xE2, 0x94, 0x38, 0x6D, 0x31,
];

/// `fibo:SecDbt:DebtInstrument`
pub const DEBT_INSTR_SLOT: u64 = 0xF7A1F2D0BB46AC3E;

/// Preimage witness for [`DEBT_INSTR_SLOT`].
pub const LEX_DEBT_INSTR: [u8; LEX_HASH_LEN] = [
    0x3E, 0xAC, 0x46, 0xBB, 0xD0, 0xF2, 0xA1, 0xF7, //
    0x91, 0x94, 0xBE, 0xC1, 0x6C, 0xB6, 0x5C, 0x37, //
    0x97, 0x47, 0x7E, 0x6C, 0x00, 0x45, 0x16, 0x14, //
    0xCD, 0x7E, 0x1D, 0xA4, 0x75, 0xF6, 0x08, 0xBB,
];

/// `fibo:ClAcct:AccountsReceivable`
pub const ACCT_RECV_SLOT: u64 = 0x0868221319EDAACB;

/// Preimage witness for [`ACCT_RECV_SLOT`].
pub const LEX_ACCT_RECV: [u8; LEX_HASH_LEN] = [
    0xCB, 0xAA, 0xED, 0x19, 0x13, 0x22, 0x68, 0x08, //
    0x57, 0xC8, 0x41, 0x35, 0xFD, 0x69, 0x30, 0xA6, //
    0x9C, 0x71, 0x73, 0x97, 0x2C, 0x6A, 0x67, 0x8F, //
    0x79, 0x90, 0x82, 0x7C, 0xE5, 0x89, 0xBB, 0xAD,
];

/// `fibo:SecDbt:hasMaturityDate`
pub const MATURITY_DATE_SLOT: u64 = 0x39293E1D838433F1;

/// Preimage witness for [`MATURITY_DATE_SLOT`].
pub const LEX_MATURITY_DATE: [u8; LEX_HASH_LEN] = [
    0xF1, 0x33, 0x84, 0x83, 0x1D, 0x3E, 0x29, 0x39, //
    0x48, 0x31, 0x07, 0x56, 0x57, 0x25, 0x37, 0x8B, //
    0x5F, 0x20, 0xEC, 0x7A, 0x87, 0x43, 0xD7, 0xDB, //
    0x26, 0x0E, 0xE5, 0x72, 0x7F, 0x41, 0x92, 0xE5,
];

/// `fibo:SecDbt:hasCouponRate`
pub const COUPON_RATE_SLOT: u64 = 0x7767F3C34C6638D8;

/// Preimage witness for [`COUPON_RATE_SLOT`].
pub const LEX_COUPON_RATE: [u8; LEX_HASH_LEN] = [
    0xD8, 0x38, 0x66, 0x4C, 0xC3, 0xF3, 0x67, 0x77, //
    0xB5, 0xEC, 0xE7, 0x16, 0x5C, 0x21, 0xDC, 0x06, //
    0x84, 0x65, 0x6B, 0x3A, 0x44, 0x69, 0x17, 0x95, //
    0x24, 0x2E, 0x21, 0x65, 0x60, 0xF4, 0xA0, 0x09,
];

// --- Facade record shapes (lane doc §7). #[repr(C)] so on-disk ==
// --- in-memory (A06 §3). Field lists are the doc sketch verbatim.

/// Bond record shape — `#[repr(C)]` so on-disk == in-memory (A06 §3).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct BondRecord {
    /// Class of the instrument ([`CLASS_CORP_BOND`], [`CLASS_BOND`], ...).
    pub instrument: ClassSlot,
    /// Denomination currency.
    pub currency: FiboCurrency,
    /// Instrument SlotId.
    pub slot: u64,
    /// LexHash witness.
    pub lex: [u8; LEX_HASH_LEN],
}

/// AR ledger record shape.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct ArRecord {
    /// Class — [`CLASS_ACCT_RECV`].
    pub class: ClassSlot,
    /// Obligor SlotId.
    pub obligor_slot: u64,
    /// Denomination currency.
    pub currency: FiboCurrency,
    /// LexHash witness.
    pub lex: [u8; LEX_HASH_LEN],
}

// --- ABI freeze asserts (compile-time; lane doc §7 style). Sizes and
// --- offsets follow from #[repr(C)] layout of the field lists above:
// --- BondRecord: instrument@0, currency@8, slot@16, lex@24 -> 56 B;
// --- ArRecord: class@0, obligor@8, currency@16, lex@18 -> 56 B.
// --- (The lane doc §7 sketch asserts 48 B, which its own field list cannot
// --- reach — unpadded total is 50 B; 48 B is A06's OabiRecordV1 container
// --- record, a different type. Divergence recorded here (and in the wave-5
// --- lane report; no separate report file was ever committed — see the
// --- `*_mutation_witness` courts, which carry the witnesses in-repo).

const _: () = assert!(core::mem::size_of::<ClassSlot>() == 8);
const _: () = assert!(core::mem::align_of::<ClassSlot>() == 8);
const _: () = assert!(core::mem::size_of::<FiboCurrency>() == 2);
const _: () = assert!(core::mem::align_of::<FiboCurrency>() == 2);

const _: () = assert!(core::mem::size_of::<BondRecord>() == 56);
const _: () = assert!(core::mem::align_of::<BondRecord>() == 8);
const _: () = assert!(core::mem::offset_of!(BondRecord, instrument) == 0);
const _: () = assert!(core::mem::offset_of!(BondRecord, currency) == 8);
const _: () = assert!(core::mem::offset_of!(BondRecord, slot) == 16);
const _: () = assert!(core::mem::offset_of!(BondRecord, lex) == 24);

const _: () = assert!(core::mem::size_of::<ArRecord>() == 56);
const _: () = assert!(core::mem::align_of::<ArRecord>() == 8);
const _: () = assert!(core::mem::offset_of!(ArRecord, class) == 0);
const _: () = assert!(core::mem::offset_of!(ArRecord, obligor_slot) == 8);
const _: () = assert!(core::mem::offset_of!(ArRecord, currency) == 16);
const _: () = assert!(core::mem::offset_of!(ArRecord, lex) == 18);
