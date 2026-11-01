//! atomic_commit — A05 power-cut WAL journal framing (v26.11.1 moonshot lane
//! A05, crate `cgap-storage`, PROPOSED).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A05-powercut-wal.md` (§3.3 record header,
//! §3.3 trailer, §3.4 new-root descriptor payload, §5 crash matrix).
//!
//! **Wave 3 slice: full lane logic.** Everything the lane doc names for this
//! module is implemented here and courted under `tests/`: CRC-32 + BLAKE3
//! framing (§3.3), new-root descriptor codec (§3.4), the Medium trait with
//! the in-process flash model backend, the `Txn` API (`begin_txn` /
//! `write_record` / `commit_txn`), per-offset recovery (`recover`), and the
//! typed mutation modes M1-M4 (§7.1) the anti-vacuity courts flip. The
//! hosted-file Medium backend lives in `tests/common/mod.rs` (real `std::fs`
//! collaborator) so this module stays `core::`-only and `no_std`-ready; the
//! crate-level attributes themselves remain coordinator-owned in `lib.rs`.
//!
//! **Wave 5 slice: defect repair (court-receipt driven).** The cross-lane
//! CRC-32 identity claim ("same identity `cgap-git` uses") was UNVERIFIED —
//! no court pinned it. `tests/court_crc_identity.rs` now pins it with
//! zlib-derived known answers + a real-collaborator differential against
//! `cgap_git::pack_write::crc32`; stale wave-1 "later wave" prose retired.
//!
//! **Wave 6 slice: defect repair (2026-11-01, court-receipt driven).** The
//! wave-5 audit receipts (a07-wave5, a11-wave4) flagged the stray repo-root
//! `atomic_commit.rs` as this family's: it is the Wave-1 ABI-freeze draft,
//! proven superseded (0 of 116 code lines unique vs this module); deletion
//! is coordinator-owned (outside lane file ownership). The wave-5
//! CRC-identity court's anti-vacuity was unwitnessed on disk (no A05 receipt
//! exists): flips M-C1/M-C2/M-C3 (crc32 final-XOR drop / polynomial drift /
//! init-only, `court_crc_identity.rs` KATs) and layout-abort M-S1
//! (`BLAKE3_HALF_BYTES` 12→16, E0080 static asserts) were executed this
//! session in `/tmp/a05w6-sandbox` — all four flip/abort, every restore
//! sha256 byte-exact; the six pinned zlib KATs re-derived independently, all
//! match. The canonical tree was untouched by the campaign (sha256 witness
//! in the wave-6 lane report).
//!
//! Crate-level attributes (`#![no_std]`, `#![forbid(unsafe_code)]`, matching
//! `cgap-git`) live in `lib.rs`, which is coordinator-owned this campaign;
//! this module is written no_std-ready (`core::` paths only) so the crate
//! attributes can land without touching this file.

// ------------------------------------------------------------------ consts --

/// Record magic, `u32` LE — bytes `'S','C','A','J'` (lane doc §3.3).
pub const RECORD_MAGIC: u32 = 0x4A41_4353;

/// Trailer magic, `u32` LE (lane doc §3.3).
pub const TRAILER_MAGIC: u32 = 0x524A_4353;

/// Record format version (lane doc §3.3).
pub const RECORD_VERSION: u16 = 0x0001;

/// Fixed header size in bytes (lane doc §3.3; also the `HEADER_SIZE` field
/// value frozen in [`RecordHeader::header_size`]).
pub const HEADER_SIZE: usize = 48;

/// Trailer size in bytes (lane doc §3.3).
pub const TRAILER_SIZE: usize = 16;

/// Standard payload size: the new-root descriptor (lane doc §3.4).
pub const PAYLOAD_SIZE: usize = 48;

/// Total encoded record length = 48 + 48 + 16 (lane doc §3.4: "a full
/// record is 112 bytes — small enough to sit inside a single flash page").
pub const RECORD_SIZE: usize = HEADER_SIZE + PAYLOAD_SIZE + TRAILER_SIZE;

/// BLAKE3 digest bytes carried per half. 12 + 12 = 24 bytes = 96-bit prefix
/// of `BLAKE3(payload)`, split header/trailer so a torn write cannot leave a
/// payload whose only digest bytes were the ones destroyed (lane doc §3.3).
pub const BLAKE3_HALF_BYTES: usize = 12;

/// Total BLAKE3 prefix bytes across header + trailer halves (96 bits).
pub const BLAKE3_TOTAL_BYTES: usize = 2 * BLAKE3_HALF_BYTES;

/// First journal sequence number (lane doc §3.3: SEQ "starts at 1").
pub const FIRST_SEQ: u64 = 1;

/// Payload FLAGS bit0 = dual-write — both roots moved together (lane doc
/// §3.4). Remaining bits reserved, must be zero on read (later-wave logic).
pub const FLAG_DUAL_WRITE: u8 = 0b0000_0001;

/// IEEE CRC-32 reflected polynomial — same identity `cgap-git`
/// (`pack_write.rs`, "IEEE CRC-32 (reflected, poly 0xEDB88320)") uses for
/// idx entries; the identity is PINNED by the cross-lane differential court
/// `tests/court_crc_identity.rs` (wave-5 defect repair; was UNVERIFIED).
pub const CRC32_IEEE_POLY: u32 = 0xEDB8_8320;

/// Journal slot count (lane doc §3.1: "double-buffered: two slots A/B,
/// strictly alternating by sequence number").
pub const SLOT_COUNT: usize = 2;

/// RESERVED header field must be zero (lane doc §3.3).
pub const RESERVED_MUST_BE_ZERO: u32 = 0x0000_0000;

// ------------------------------------------------------- field offsets (LE) --

// Fixed header §3.3 offsets 0..48. Every offset below is pinned to the
// struct by a `const _` static assert in the layout section.
/// `MAGIC`, header offset 0.
pub const OFF_MAGIC: usize = 0;
/// `VERSION`, header offset 4.
pub const OFF_VERSION: usize = 4;
/// `HEADER_SIZE`, header offset 6.
pub const OFF_HEADER_SIZE: usize = 6;
/// `SEQ`, header offset 8.
pub const OFF_SEQ: usize = 8;
/// `PAYLOAD_LEN`, header offset 16.
pub const OFF_PAYLOAD_LEN: usize = 16;
/// `PAYLOAD_CRC`, header offset 24.
pub const OFF_PAYLOAD_CRC: usize = 24;
/// `HEADER_CRC`, header offset 28.
pub const OFF_HEADER_CRC: usize = 28;
/// `RESERVED`, header offset 32.
pub const OFF_RESERVED: usize = 32;
/// `BLAKE3_OUT`, header offset 36.
pub const OFF_BLAKE3_OUT: usize = 36;

// New-root descriptor payload §3.4 offsets 0..48.
/// `GIT_OID`, payload offset 0.
pub const OFF_GIT_OID: usize = 0;
/// `PARQUET_ROOT`, payload offset 20.
pub const OFF_PARQUET_ROOT: usize = 20;
/// `DAG_SEQ`, payload offset 28.
pub const OFF_DAG_SEQ: usize = 28;
/// `TXN_TS`, payload offset 36.
pub const OFF_TXN_TS: usize = 36;
/// `FLAGS`, payload offset 44.
pub const OFF_FLAGS: usize = 44;
/// 3-byte pad, payload offset 45.
pub const OFF_PAD: usize = 45;

// Trailer §3.3 offsets 0..16 (relative to trailer start) and absolute
// offsets within a full [`JournalRecord`].
/// `TRAILER_MAGIC`, trailer offset 0 (absolute [`OFF_TRAILER`] + 0).
pub const OFF_TRAILER_MAGIC: usize = 0;
/// `BLAKE3_TRAILER`, trailer offset 4 — the spec table prints the field at
/// trailer-relative 4..16; the 12 digest bytes start at trailer offset 4.
pub const OFF_BLAKE3_TRAILER: usize = 4;
/// Trailer start within a full record: header 48 + payload 48.
pub const OFF_TRAILER: usize = HEADER_SIZE + PAYLOAD_SIZE;
/// `TRAILER_MAGIC`, absolute record offset 96.
pub const OFF_TRAILER_MAGIC_ABS: usize = OFF_TRAILER;
/// `BLAKE3_TRAILER`, absolute record offset 100.
pub const OFF_BLAKE3_TRAILER_ABS: usize = OFF_TRAILER + OFF_BLAKE3_TRAILER;

// --------------------------------------------------------------- structures --

/// Fixed 48-byte journal record header (lane doc §3.3, byte-exact,
/// little-endian on the wire).
///
/// Natural `repr(C)` layout of these field types lands every field on its
/// spec offset with zero padding (asserted below): the only u64s (`seq`,
/// `payload_len`) start at offset 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct RecordHeader {
    /// `MAGIC` = [`RECORD_MAGIC`], offset 0..4.
    pub magic: u32,
    /// `VERSION` = [`RECORD_VERSION`], offset 4..6.
    pub version: u16,
    /// `HEADER_SIZE` = [`HEADER_SIZE`], offset 6..8.
    pub header_size: u16,
    /// `SEQ`, monotonically increasing, starts at [`FIRST_SEQ`], 8..16.
    pub seq: u64,
    /// `PAYLOAD_LEN`, bytes following header+trailer, 16..24.
    pub payload_len: u64,
    /// `PAYLOAD_CRC`, CRC-32 (IEEE) over payload bytes, 24..28.
    pub payload_crc: u32,
    /// `HEADER_CRC`, CRC-32 over header bytes 0..24 and 32..44 (CRC fields
    /// excluded, zero-filled), 28..32.
    pub header_crc: u32,
    /// `RESERVED` = [`RESERVED_MUST_BE_ZERO`], 32..36.
    pub reserved: u32,
    /// `BLAKE3_OUT`: first [`BLAKE3_HALF_BYTES`] bytes of `BLAKE3(payload)`,
    /// 36..48.
    pub blake3_out: [u8; BLAKE3_HALF_BYTES],
}

/// New-root descriptor payload, 48 bytes (lane doc §3.4, byte-exact).
///
/// `#[repr(C, packed)]` is load-bearing: the spec puts the `u64`
/// `parquet_root` at offset 20 (immediately after the 20-byte
/// [`NewRootDescriptor::git_oid`]), and natural C layout would insert 4
/// alignment bytes there, breaking §3.4. Packed keeps every field on its
/// spec offset; alignment asserts below pin `align == 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct NewRootDescriptor {
    /// New Git pack/tree root — `cgap-git`'s `Oid` (`[u8; 20]`), 0..20.
    pub git_oid: [u8; 20],
    /// Parquet footer-set identifier (A02 contract; treated opaquely here
    /// per lane doc §6), 20..28.
    pub parquet_root: u64,
    /// dag-root sequence (A03 contract; strictly monotonic across committed
    /// records — a valid record with `dag_seq <= base` is typed
    /// `SeqRegression`, lane doc §6), 28..36.
    pub dag_seq: u64,
    /// Writer timestamp — advisory, never used for ordering (lane doc
    /// §3.4; time is not a proxy for state), 36..44.
    pub txn_ts: u64,
    /// FLAGS; bit0 = [`FLAG_DUAL_WRITE`], 44..45.
    pub flags: u8,
    /// Pad to 48 bytes, 45..48.
    pub pad: [u8; 3],
}

/// 16-byte record trailer (lane doc §3.3), immediately after the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct RecordTrailer {
    /// `TRAILER_MAGIC` = [`TRAILER_MAGIC`], trailer offset 0..4.
    pub trailer_magic: u32,
    /// Second 12 bytes of `BLAKE3(payload)`; digest split across header and
    /// trailer, 4..16.
    pub blake3_trailer: [u8; BLAKE3_HALF_BYTES],
}

/// Full 112-byte journal record: header + payload + trailer (lane doc §3.3
/// total record size formula). Packed so member offsets equal the absolute
/// offsets asserted below with zero implicit padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, packed)]
pub struct JournalRecord {
    /// Fixed header, record offsets 0..48.
    pub header: RecordHeader,
    /// New-root descriptor payload, record offsets 48..96.
    pub payload: NewRootDescriptor,
    /// Trailer, record offsets 96..112.
    pub trailer: RecordTrailer,
}

/// Torn-write class for cut at byte `k in 0..=N` of the encoded record
/// (lane doc §5 crash matrix, C0-C4). Classified by
/// [`tear_class_for_offset`]; the class set itself is frozen ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TearClass {
    /// C0: `k = 0`, slot unchanged / erased 0xFF → prior root, slot free.
    PreWrite,
    /// C1: `1..=47`, torn header → bad CRC/magic/version → invalid.
    TornHeader,
    /// C2: `48..<48+L`, header ok, payload torn → CRC/BLAKE3 mismatch.
    TornPayload,
    /// C3: `48+L..<N`, trailer bytes missing → BLAKE3 trailer bad.
    TornTrailer,
    /// C4: `k = N`, fully valid record → NEW root committed (not corruption).
    CompleteRecord,
}

/// Typed refusals of `recover` (lane doc §5 F3, §5.1, §6 A03 contract).
/// Refusal values are fail-closed: never guess between conflicting states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    /// F3: identical SEQ in both slots with differing payloads — refuse,
    /// never guess (lane doc §5 F3, §5.1 `JournalAmbiguous`).
    JournalAmbiguous,
    /// A03 contract: a valid record whose `DAG_SEQ` <= base dag seq is
    /// rollback-as-corruption (lane doc §6, §7.1 M4).
    SeqRegression,
}

/// Recovery report flags returned alongside the resolved root (lane doc §5
/// F2: "recovery returns the base root and reports `RecoveryDegraded`").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryReport {
    /// Clean recovery: a single valid journal tail or clean rollback.
    Clean,
    /// F2: both slots unusable — base root returned, degradation reported.
    RecoveryDegraded,
}

// ------------------------------------------------------------ layout court --

// Wave-1 court: compile-time asserts. A court is a court only if it can
// flip; non-vacuity protocol (shim-court: baseline sha256 -> SHIM-COURT ->
// build must abort -> byte-exact restore):
//   M-S1 widen `BLAKE3_HALF_BYTES` 12 -> 16       -> size asserts trip
//   M-O1 drift `OFF_PAYLOAD_CRC` 24 -> 25         -> header offset assert trips
//   M-O2 swap `OFF_DAG_SEQ`/`OFF_TXN_TS` 28/36    -> payload offset asserts trip
// Any mutation run where the build still succeeds is a VACUOUS GATE (FAIL).
// Mutation protocol for the behavior-level sweeps (M1-M4) is lane doc §7.1,
// implemented as typed in-module [`MutationMode`] variants (divergence D3)
// and flipped by `tests/court_mutations.rs`.
const _: () = assert!(core::mem::size_of::<RecordHeader>() == HEADER_SIZE);
const _: () = assert!(core::mem::align_of::<RecordHeader>() == 8);
const _: () = assert!(core::mem::size_of::<NewRootDescriptor>() == PAYLOAD_SIZE);
const _: () = assert!(core::mem::align_of::<NewRootDescriptor>() == 1);
const _: () = assert!(core::mem::size_of::<RecordTrailer>() == TRAILER_SIZE);
const _: () = assert!(core::mem::align_of::<RecordTrailer>() == 4);
const _: () = assert!(core::mem::size_of::<JournalRecord>() == RECORD_SIZE);
const _: () = assert!(core::mem::align_of::<JournalRecord>() == 1);
const _: () = assert!(RECORD_SIZE == 112);
const _: () = assert!(BLAKE3_TOTAL_BYTES == 24);

// Header field offsets vs §3.3 table.
const _: () = assert!(core::mem::offset_of!(RecordHeader, magic) == OFF_MAGIC);
const _: () = assert!(core::mem::offset_of!(RecordHeader, version) == OFF_VERSION);
const _: () = assert!(core::mem::offset_of!(RecordHeader, header_size) == OFF_HEADER_SIZE);
const _: () = assert!(core::mem::offset_of!(RecordHeader, seq) == OFF_SEQ);
const _: () = assert!(core::mem::offset_of!(RecordHeader, payload_len) == OFF_PAYLOAD_LEN);
const _: () = assert!(core::mem::offset_of!(RecordHeader, payload_crc) == OFF_PAYLOAD_CRC);
const _: () = assert!(core::mem::offset_of!(RecordHeader, header_crc) == OFF_HEADER_CRC);
const _: () = assert!(core::mem::offset_of!(RecordHeader, reserved) == OFF_RESERVED);
const _: () = assert!(core::mem::offset_of!(RecordHeader, blake3_out) == OFF_BLAKE3_OUT);

// Payload field offsets vs §3.4 table.
const _: () = assert!(core::mem::offset_of!(NewRootDescriptor, git_oid) == OFF_GIT_OID);
const _: () = assert!(core::mem::offset_of!(NewRootDescriptor, parquet_root) == OFF_PARQUET_ROOT);
const _: () = assert!(core::mem::offset_of!(NewRootDescriptor, dag_seq) == OFF_DAG_SEQ);
const _: () = assert!(core::mem::offset_of!(NewRootDescriptor, txn_ts) == OFF_TXN_TS);
const _: () = assert!(core::mem::offset_of!(NewRootDescriptor, flags) == OFF_FLAGS);
const _: () = assert!(core::mem::offset_of!(NewRootDescriptor, pad) == OFF_PAD);

// Trailer field offsets (trailer-relative) vs §3.3 trailer table.
const _: () = assert!(core::mem::offset_of!(RecordTrailer, trailer_magic) == OFF_TRAILER_MAGIC);
const _: () = assert!(core::mem::offset_of!(RecordTrailer, blake3_trailer) == OFF_BLAKE3_TRAILER);

// Full-record member offsets and absolute trailer offsets.
const _: () = assert!(core::mem::offset_of!(JournalRecord, header) == 0);
const _: () = assert!(core::mem::offset_of!(JournalRecord, payload) == HEADER_SIZE);
const _: () = assert!(core::mem::offset_of!(JournalRecord, trailer) == OFF_TRAILER);
const _: () = assert!(OFF_TRAILER == 96);
const _: () = assert!(OFF_BLAKE3_TRAILER_ABS == 100);

// ---------------------------------------------------------------- divergence --
//
// D1 (doc §3.3 HEADER_CRC range, recorded per contract): the doc's literal
// coverage string "bytes 0..24 and 32..44" contradicts its own coverage
// summary ("SEQ, PAYLOAD_LEN, VERSION, sizes, BLAKE3 half"): BLAKE3_OUT spans
// 36..48, so "32..44" would leave digest bytes 44..48 uncovered. Implemented
// reading: HEADER_CRC covers h[0..24] ++ zeros(24..32) ++ h[32..48] — every
// non-CRC field covered, both CRC fields zero-filled, per the coverage
// summary. RESERVED (must be zero) is included; harmlessly redundant.
//
// D2 (doc §5 F2 k-range vs C4 precedence, recorded): F2 says "cut during
// commit k, plus earlier damage in the other slot → base root + degraded"
// without bounding k < N. At k = N the record is fully valid and C4 ("NEW
// root committed, not corruption") must win — a valid commit is honored even
// when the opposite slot is damaged. Court `sweep_both_slots` therefore
// asserts F2 semantics for k in 0..112 and C4 semantics at k = 112.
//
// D3 (doc §7.1 mutation protocol, recorded): the doc prescribes applying
// mutations M1-M4 "in a scratch copy of the recovery module" with sha256
// byte-exact restore. That protocol edits files outside this lane's owned
// directory (`scripts/`), so the mutations are instead implemented as typed
// in-module behavior variants ([`MutationMode`]) threaded through the same
// validator chain — single-fault semantics, default [`MutationMode::Strict`],
// production `recover()` pinned to Strict by the mutation court. No source
// file is edited at test time; the byte-exact-restore proof (sha256 of this
// file before/after the court run) is carried in the lane receipt instead.

// ------------------------------------------------------------ mutation modes --
//
// SHIM-COURT variants (lane doc §7.1 M1-M4). These are real, executed
// behavior divergences of the recovery validator chain — not mocks — and the
// only difference between `recover()` and `recover_mode(m, mode)` is the
// named dropped/shipped check. A mutation court whose sweep still passes
// under the mutated mode is itself FAIL (vacuous gate).

/// Named single-fault mutation of the recovery validator chain (lane doc
/// §7.1). Default/production mode is [`MutationMode::Strict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationMode {
    /// Production validator chain (no mutation).
    Strict,
    /// M1: treat a header-CRC failure as "skip to payload" instead of
    /// invalid (doc §7.1 M1).
    M1HeaderCrcSkipped,
    /// M2: pick the *lowest* valid sequence instead of highest (doc §7.1 M2).
    M2LowestSeq,
    /// M3: BLAKE3 trailer half dropped from validation (doc §7.1 M3).
    M3TrailerDigestSkipped,
    /// M4: SeqRegression check removed (doc §7.1 M4).
    M4SeqRegressionUnchecked,
}

use MutationMode::{
    M1HeaderCrcSkipped, M2LowestSeq, M3TrailerDigestSkipped, M4SeqRegressionUnchecked, Strict,
};

// --------------------------------------------------------------------- crc32 --
//
// IEEE CRC-32 (reflected, poly 0xEDB88320) — same identity `cgap-git`
// `pack_write.rs` uses for idx entries. Bitwise, table-free, `core`-only:
// at 112-byte records a per-byte loop is well inside budget and keeps E=0.

/// CRC-32 (IEEE, reflected, poly [`CRC32_IEEE_POLY`]) over `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (CRC32_IEEE_POLY & mask);
        }
    }
    !crc
}

/// HEADER_CRC input per the zero-fill rule: `h[0..24] ++ zeros(24..32) ++
/// h[32..48]` — both CRC fields excluded by zero-fill (see divergence D1).
pub fn header_crc_over(header: &[u8; HEADER_SIZE]) -> u32 {
    let mut buf = [0u8; HEADER_SIZE];
    buf[0..OFF_PAYLOAD_CRC].copy_from_slice(&header[0..OFF_PAYLOAD_CRC]);
    buf[OFF_RESERVED..HEADER_SIZE].copy_from_slice(&header[OFF_RESERVED..HEADER_SIZE]);
    crc32(&buf)
}

// ------------------------------------------------------------- payload codec --

impl NewRootDescriptor {
    /// Build the §3.4 descriptor. Pad is zeroed (3-byte pad, doc §3.4).
    pub const fn new(
        git_oid: [u8; 20],
        parquet_root: u64,
        dag_seq: u64,
        txn_ts: u64,
        flags: u8,
    ) -> Self {
        Self {
            git_oid,
            parquet_root,
            dag_seq,
            txn_ts,
            flags,
            pad: [0; 3],
        }
    }

    /// Encode to the byte-exact 48-byte §3.4 wire form (little-endian).
    pub fn to_bytes(&self) -> [u8; PAYLOAD_SIZE] {
        let mut b = [0u8; PAYLOAD_SIZE];
        b[OFF_GIT_OID..OFF_GIT_OID + 20].copy_from_slice(&self.git_oid);
        b[OFF_PARQUET_ROOT..OFF_PARQUET_ROOT + 8].copy_from_slice(&self.parquet_root.to_le_bytes());
        b[OFF_DAG_SEQ..OFF_DAG_SEQ + 8].copy_from_slice(&self.dag_seq.to_le_bytes());
        b[OFF_TXN_TS..OFF_TXN_TS + 8].copy_from_slice(&self.txn_ts.to_le_bytes());
        b[OFF_FLAGS] = self.flags;
        b[OFF_PAD..PAYLOAD_SIZE].copy_from_slice(&self.pad);
        b
    }

    /// Decode from the byte-exact 48-byte §3.4 wire form.
    pub fn from_bytes(b: &[u8; PAYLOAD_SIZE]) -> Self {
        let mut parquet = [0u8; 8];
        let mut dag = [0u8; 8];
        let mut ts = [0u8; 8];
        parquet.copy_from_slice(&b[OFF_PARQUET_ROOT..OFF_PARQUET_ROOT + 8]);
        dag.copy_from_slice(&b[OFF_DAG_SEQ..OFF_DAG_SEQ + 8]);
        ts.copy_from_slice(&b[OFF_TXN_TS..OFF_TXN_TS + 8]);
        let mut git_oid = [0u8; 20];
        git_oid.copy_from_slice(&b[OFF_GIT_OID..OFF_GIT_OID + 20]);
        Self {
            git_oid,
            parquet_root: u64::from_le_bytes(parquet),
            dag_seq: u64::from_le_bytes(dag),
            txn_ts: u64::from_le_bytes(ts),
            flags: b[OFF_FLAGS],
            pad: [b[OFF_PAD], b[OFF_PAD + 1], b[OFF_PAD + 2]],
        }
    }
}

// ------------------------------------------------------------ record framing --

/// Encode a full 112-byte journal record for `seq` carrying `payload`
/// (lane doc §3.3 framing: header CRC §3.3 zero-fill rule, payload CRC-32,
/// 96-bit BLAKE3 prefix split header 12 + trailer 12).
pub fn encode_record(seq: u64, payload: &NewRootDescriptor) -> [u8; RECORD_SIZE] {
    let pb = payload.to_bytes();
    let payload_crc = crc32(&pb);
    let digest = cgap_blake3::hash(&pb);
    let mut out = [0u8; RECORD_SIZE];

    out[HEADER_SIZE..OFF_TRAILER].copy_from_slice(&pb);
    out[OFF_MAGIC..OFF_MAGIC + 4].copy_from_slice(&RECORD_MAGIC.to_le_bytes());
    out[OFF_VERSION..OFF_VERSION + 2].copy_from_slice(&RECORD_VERSION.to_le_bytes());
    out[OFF_HEADER_SIZE..OFF_HEADER_SIZE + 2].copy_from_slice(&(HEADER_SIZE as u16).to_le_bytes());
    out[OFF_SEQ..OFF_SEQ + 8].copy_from_slice(&seq.to_le_bytes());
    out[OFF_PAYLOAD_LEN..OFF_PAYLOAD_LEN + 8].copy_from_slice(&(PAYLOAD_SIZE as u64).to_le_bytes());
    out[OFF_PAYLOAD_CRC..OFF_PAYLOAD_CRC + 4].copy_from_slice(&payload_crc.to_le_bytes());
    // HEADER_CRC needs the BLAKE3 half already placed; compute over the
    // zero-filled template, then store.
    out[OFF_BLAKE3_OUT..OFF_BLAKE3_OUT + BLAKE3_HALF_BYTES]
        .copy_from_slice(&digest[0..BLAKE3_HALF_BYTES]);
    let mut head = [0u8; HEADER_SIZE];
    head.copy_from_slice(&out[0..HEADER_SIZE]);
    let hcrc = header_crc_over(&head);
    out[OFF_HEADER_CRC..OFF_HEADER_CRC + 4].copy_from_slice(&hcrc.to_le_bytes());

    out[OFF_TRAILER..OFF_TRAILER + 4].copy_from_slice(&TRAILER_MAGIC.to_le_bytes());
    out[OFF_BLAKE3_TRAILER_ABS..OFF_BLAKE3_TRAILER_ABS + BLAKE3_HALF_BYTES]
        .copy_from_slice(&digest[BLAKE3_HALF_BYTES..BLAKE3_TOTAL_BYTES]);
    out
}

/// Typed rejection of a slot decode. `Empty` is the erased-slot state (all
/// 0xFF — flash erase semantics, lane doc §7 test 6), distinct from damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeRejection {
    /// All-0xFF slot: erased, no record ever written.
    Empty,
    /// MAGIC mismatch.
    BadMagic,
    /// VERSION mismatch.
    BadVersion,
    /// HEADER_SIZE field mismatch.
    BadHeaderSize,
    /// PAYLOAD_LEN field mismatch (this layout fixes the 48-byte descriptor).
    BadPayloadLen,
    /// RESERVED field nonzero.
    BadReserved,
    /// HEADER_CRC mismatch (the M1 mutation skips this check).
    BadHeaderCrc,
    /// PAYLOAD_CRC mismatch.
    BadPayloadCrc,
    /// BLAKE3 header half mismatch.
    BadBlake3Head,
    /// TRAILER_MAGIC mismatch.
    BadTrailerMagic,
    /// BLAKE3 trailer half mismatch (the M3 mutation skips this check).
    BadBlake3Tail,
}

/// Validate + decode one slot region (strict validator chain).
pub fn decode_slot(bytes: &[u8; RECORD_SIZE]) -> Result<JournalRecord, DecodeRejection> {
    decode_slot_mode(bytes, Strict)
}

/// `decode_slot` under a named mutation mode (court-only; see [`MutationMode`]).
pub fn decode_slot_mode(
    bytes: &[u8; RECORD_SIZE],
    mode: MutationMode,
) -> Result<JournalRecord, DecodeRejection> {
    if bytes.iter().all(|&b| b == 0xFF) {
        return Err(DecodeRejection::Empty);
    }
    let le32 = |o: usize| -> u32 {
        let mut a = [0u8; 4];
        a.copy_from_slice(&bytes[o..o + 4]);
        u32::from_le_bytes(a)
    };
    let le16 = |o: usize| -> u16 {
        let mut a = [0u8; 2];
        a.copy_from_slice(&bytes[o..o + 2]);
        u16::from_le_bytes(a)
    };
    let le64 = |o: usize| -> u64 {
        let mut a = [0u8; 8];
        a.copy_from_slice(&bytes[o..o + 8]);
        u64::from_le_bytes(a)
    };
    let arr12 = |o: usize| -> [u8; BLAKE3_HALF_BYTES] {
        let mut a = [0u8; BLAKE3_HALF_BYTES];
        a.copy_from_slice(&bytes[o..o + BLAKE3_HALF_BYTES]);
        a
    };

    if le32(OFF_MAGIC) != RECORD_MAGIC {
        return Err(DecodeRejection::BadMagic);
    }
    if le16(OFF_VERSION) != RECORD_VERSION {
        return Err(DecodeRejection::BadVersion);
    }
    if le16(OFF_HEADER_SIZE) != HEADER_SIZE as u16 {
        return Err(DecodeRejection::BadHeaderSize);
    }
    if le64(OFF_PAYLOAD_LEN) != PAYLOAD_SIZE as u64 {
        return Err(DecodeRejection::BadPayloadLen);
    }
    if le32(OFF_RESERVED) != RESERVED_MUST_BE_ZERO {
        return Err(DecodeRejection::BadReserved);
    }
    if mode != M1HeaderCrcSkipped {
        // Zero-fill rule: recompute over h[0..24] ++ zeros(24..32) ++ h[32..48].
        let mut tmpl = [0u8; HEADER_SIZE];
        tmpl[0..OFF_PAYLOAD_CRC].copy_from_slice(&bytes[0..OFF_PAYLOAD_CRC]);
        tmpl[OFF_RESERVED..HEADER_SIZE].copy_from_slice(&bytes[OFF_RESERVED..HEADER_SIZE]);
        if crc32(&tmpl) != le32(OFF_HEADER_CRC) {
            return Err(DecodeRejection::BadHeaderCrc);
        }
    }
    let mut pb = [0u8; PAYLOAD_SIZE];
    pb.copy_from_slice(&bytes[HEADER_SIZE..OFF_TRAILER]);
    let payload = NewRootDescriptor::from_bytes(&pb);
    if crc32(&pb) != le32(OFF_PAYLOAD_CRC) {
        return Err(DecodeRejection::BadPayloadCrc);
    }
    let digest = cgap_blake3::hash(&pb);
    if bytes[OFF_BLAKE3_OUT..OFF_BLAKE3_OUT + BLAKE3_HALF_BYTES] != digest[0..BLAKE3_HALF_BYTES] {
        return Err(DecodeRejection::BadBlake3Head);
    }
    if le32(OFF_TRAILER) != TRAILER_MAGIC {
        return Err(DecodeRejection::BadTrailerMagic);
    }
    if mode != M3TrailerDigestSkipped
        && bytes[OFF_BLAKE3_TRAILER_ABS..OFF_BLAKE3_TRAILER_ABS + BLAKE3_HALF_BYTES]
            != digest[BLAKE3_HALF_BYTES..BLAKE3_TOTAL_BYTES]
    {
        return Err(DecodeRejection::BadBlake3Tail);
    }

    Ok(JournalRecord {
        header: RecordHeader {
            magic: le32(OFF_MAGIC),
            version: le16(OFF_VERSION),
            header_size: le16(OFF_HEADER_SIZE),
            seq: le64(OFF_SEQ),
            payload_len: le64(OFF_PAYLOAD_LEN),
            payload_crc: le32(OFF_PAYLOAD_CRC),
            header_crc: le32(OFF_HEADER_CRC),
            reserved: le32(OFF_RESERVED),
            blake3_out: arr12(OFF_BLAKE3_OUT),
        },
        payload,
        trailer: RecordTrailer {
            trailer_magic: le32(OFF_TRAILER),
            blake3_trailer: arr12(OFF_BLAKE3_TRAILER_ABS),
        },
    })
}

/// Journal slot holding sequence number `seq` (doc §3.1: two slots, strictly
/// alternating by sequence number; SEQ starts at [`FIRST_SEQ`], slot A).
pub const fn slot_for_seq(seq: u64) -> usize {
    ((seq.wrapping_sub(1)) % SLOT_COUNT as u64) as usize
}

/// Expected torn-write class for a cut after byte `k` of a standard
/// 112-byte record (lane doc §5 crash matrix C0-C4, `L = PAYLOAD_SIZE`).
/// The courts derive expectations from this mapping — never hardcoded — so
/// a constants drift trips the courts instead of the other way around.
pub const fn tear_class_for_offset(k: usize) -> TearClass {
    if k == 0 {
        TearClass::PreWrite
    } else if k < HEADER_SIZE {
        TearClass::TornHeader
    } else if k < HEADER_SIZE + PAYLOAD_SIZE {
        TearClass::TornPayload
    } else if k < RECORD_SIZE {
        TearClass::TornTrailer
    } else {
        TearClass::CompleteRecord
    }
}

// ------------------------------------------------------------ medium backends --

/// Byte-level medium refusal (flash AND-semantics needs an erase first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediumError {
    /// Flash write would need to SET a bit over a cleared bit: erase first.
    NeedsErase,
    /// Slot index out of range.
    SlotIndex,
    /// Byte offset out of range.
    OffsetRange,
    /// Hosted backend byte write failed on I/O.
    Io,
}

/// Read side of the journal medium (lane doc §4.3 C3 "Medium trait:
/// read/write/truncate — real file and flash backends").
pub trait Medium {
    /// The immutable base (superblock) root: what recovery rolls back to.
    fn read_base_root(&self) -> NewRootDescriptor;
    /// The stored bytes of journal slot `slot` (`0..SLOT_COUNT`).
    fn read_slot(&self, slot: usize) -> [u8; RECORD_SIZE];
    /// Whether the artifact-durability fence has been taken (§9: the commit
    /// protocol must make artifact bytes durable BEFORE the journal record).
    fn artifacts_fenced(&self) -> bool;
}

/// Write side of the journal medium: byte-granular `pwrite` surface used by
/// both the `Txn` commit path and the per-offset cut simulation.
pub trait JournalWriter: Medium {
    /// Write one journal byte (visible to subsequent `Medium::read_slot`).
    fn write_journal_byte(&mut self, slot: usize, offset: usize, value: u8)
        -> Result<(), MediumError>;
    /// Durability fence (§9): models fsync of artifact bytes; must precede
    /// any journal-record write (`TxnError::ArtifactsNotFenced` otherwise).
    fn fence_artifacts(&mut self);
}

/// In-process flash model (lane doc §7: "real in-process implementations, no
/// mocks"): erased bytes are 0xFF, writes can only CLEAR bits (NOR
/// AND-semantics; a write needing to set a bit is typed [`MediumError::NeedsErase`]),
/// and erase is progressive so cut-during-erase offsets are simulatable.
#[derive(Debug, Clone)]
pub struct FlashMedium {
    base: NewRootDescriptor,
    slots: [[u8; RECORD_SIZE]; SLOT_COUNT],
    fenced: bool,
}

impl FlashMedium {
    /// Fresh medium: both slots erased (0xFF), fence untaken.
    pub fn new(base: NewRootDescriptor) -> Self {
        Self {
            base,
            slots: [[0xFF; RECORD_SIZE]; SLOT_COUNT],
            fenced: false,
        }
    }

    /// Erase a whole slot to 0xFF (page/erase-block model).
    pub fn erase_slot(&mut self, slot: usize) -> Result<(), MediumError> {
        if slot >= SLOT_COUNT {
            return Err(MediumError::SlotIndex);
        }
        self.slots[slot] = [0xFF; RECORD_SIZE];
        Ok(())
    }

    /// Progressive erase: set the first `count` bytes of `slot` to 0xFF and
    /// leave the rest as-is — the cut-during-erase fault injector.
    pub fn erase_prefix(&mut self, slot: usize, count: usize) -> Result<(), MediumError> {
        if slot >= SLOT_COUNT {
            return Err(MediumError::SlotIndex);
        }
        for b in self.slots[slot].iter_mut().take(count) {
            *b = 0xFF;
        }
        Ok(())
    }

    /// Aging-corruption fault injector (F1): XOR `mask` into one stored byte.
    /// Models bit-rot on previously committed state (lane doc §5 F1).
    pub fn flip_bit(&mut self, slot: usize, offset: usize, mask: u8) -> Result<(), MediumError> {
        if slot >= SLOT_COUNT {
            return Err(MediumError::SlotIndex);
        }
        if offset >= RECORD_SIZE {
            return Err(MediumError::OffsetRange);
        }
        self.slots[slot][offset] ^= mask;
        Ok(())
    }
}

impl Medium for FlashMedium {
    fn read_base_root(&self) -> NewRootDescriptor {
        self.base
    }
    fn read_slot(&self, slot: usize) -> [u8; RECORD_SIZE] {
        self.slots[slot]
    }
    fn artifacts_fenced(&self) -> bool {
        self.fenced
    }
}

impl JournalWriter for FlashMedium {
    fn write_journal_byte(
        &mut self,
        slot: usize,
        offset: usize,
        value: u8,
    ) -> Result<(), MediumError> {
        if slot >= SLOT_COUNT {
            return Err(MediumError::SlotIndex);
        }
        if offset >= RECORD_SIZE {
            return Err(MediumError::OffsetRange);
        }
        let cur = self.slots[slot][offset];
        if cur & value != value {
            return Err(MediumError::NeedsErase);
        }
        self.slots[slot][offset] = cur & value;
        Ok(())
    }

    fn fence_artifacts(&mut self) {
        self.fenced = true;
    }
}

// ------------------------------------------------------------------- txn api --

/// Typed failure of the transaction path (lane doc §4.3 C3 `begin_txn` /
/// `write_record` / `commit_txn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnError {
    /// §9 hard precondition: artifact bytes were not fenced before the
    /// journal record. The commit protocol refuses, it never reorders.
    ArtifactsNotFenced,
    /// The medium refused a byte write (e.g. flash [`MediumError::NeedsErase`]).
    Medium(MediumError),
    /// Commit-time readback self-check failed to decode.
    SelfCheck(DecodeRejection),
    /// Commit-time readback decoded to a different seq/payload than written.
    SelfCheckMismatch,
    /// `commit_txn` before any `write_record` — nothing to self-check.
    NothingWritten,
}

/// A journal transaction over medium `M` (lane doc §4.3 C3 API surface).
pub struct Txn<'a, M: JournalWriter> {
    medium: &'a mut M,
    seq: u64,
    expected: Option<NewRootDescriptor>,
}

impl<'a, M: JournalWriter> Txn<'a, M> {
    /// `begin_txn`: bind a transaction to `seq` (caller derives
    /// `last_seq + 1` from [`recover`]).
    pub fn new(medium: &'a mut M, seq: u64) -> Self {
        Self {
            medium,
            seq,
            expected: None,
        }
    }

    /// `write_record`: encode and write the full 112-byte record through the
    /// medium, byte by byte (the power cut is external to this call).
    pub fn write_record(&mut self, payload: &NewRootDescriptor) -> Result<(), TxnError> {
        write_record_cut(self.medium, self.seq, payload, RECORD_SIZE)?;
        self.expected = Some(*payload);
        Ok(())
    }

    /// `commit_txn`: post-write readback self-check; on success the record is
    /// acknowledged durable and the committed root is returned.
    pub fn commit_txn(self) -> Result<NewRootDescriptor, TxnError> {
        let expected = match self.expected {
            Some(p) => p,
            None => return Err(TxnError::NothingWritten),
        };
        let slot = slot_for_seq(self.seq);
        let bytes = self.medium.read_slot(slot);
        let rec = decode_slot(&bytes).map_err(TxnError::SelfCheck)?;
        if rec.header.seq != self.seq || rec.payload != expected {
            return Err(TxnError::SelfCheckMismatch);
        }
        Ok(rec.payload)
    }
}

/// `begin_txn` free function (lane doc §4.3 C3 names this API surface).
pub fn begin_txn<M: JournalWriter>(medium: &mut M, seq: u64) -> Txn<'_, M> {
    Txn::new(medium, seq)
}

/// `write_record_cut` is the fault injector the sweep drives: write exactly
/// `k` bytes of the encoded record — the simulated power cut after byte `k`.
/// `k == RECORD_SIZE` is the full unc-cut write (C4); `k == 0` writes nothing
/// (C0). §9 fence precondition enforced here: unfenced artifact bytes are a
/// typed refusal, never a reordering.
pub fn write_record_cut<M: JournalWriter>(
    medium: &mut M,
    seq: u64,
    payload: &NewRootDescriptor,
    k: usize,
) -> Result<(), TxnError> {
    if !medium.artifacts_fenced() {
        return Err(TxnError::ArtifactsNotFenced);
    }
    let bytes = encode_record(seq, payload);
    let slot = slot_for_seq(seq);
    for i in 0..k.min(RECORD_SIZE) {
        medium
            .write_journal_byte(slot, i, bytes[i])
            .map_err(TxnError::Medium)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ recovery --

/// Per-slot decode verdict of a recovery pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotVerdict {
    /// Valid record decoded; carried `seq` noted.
    Valid,
    /// Erased (all-0xFF) slot: no record.
    Empty,
    /// Non-empty bytes that failed validation (torn or aged corruption).
    Damaged,
}

/// Recovery result: the resolved root plus how it was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryOutcome {
    /// The resolved current root: the top record's descriptor, or the base
    /// root on rollback. Never a third value (invariant I1).
    pub root: NewRootDescriptor,
    /// Clean vs [`RecoveryReport::RecoveryDegraded`] (F2 semantics).
    pub report: RecoveryReport,
    /// SEQ of the record applied, if any.
    pub recovered_seq: Option<u64>,
    /// Per-slot verdicts, slot A then slot B.
    pub slot_verdicts: [SlotVerdict; SLOT_COUNT],
}

/// Recovery as a pure function of the medium bytes (lane doc §5.1; I3:
/// deterministic in the post-cut bytes alone — no clock, no entropy).
pub fn recover<M: Medium>(medium: &M) -> Result<RecoveryOutcome, RecoveryError> {
    recover_mode(medium, Strict)
}

/// `recover` under a named mutation mode (court-only; see [`MutationMode`]).
pub fn recover_mode<M: Medium>(
    medium: &M,
    mode: MutationMode,
) -> Result<RecoveryOutcome, RecoveryError> {
    let base = medium.read_base_root();
    let mut verdicts = [SlotVerdict::Empty; SLOT_COUNT];
    let mut cands: [Option<JournalRecord>; SLOT_COUNT] = [None, None];
    for slot in 0..SLOT_COUNT {
        match decode_slot_mode(&medium.read_slot(slot), mode) {
            Ok(rec) => {
                verdicts[slot] = SlotVerdict::Valid;
                cands[slot] = Some(rec);
            }
            Err(DecodeRejection::Empty) => verdicts[slot] = SlotVerdict::Empty,
            Err(_) => verdicts[slot] = SlotVerdict::Damaged,
        }
    }

    // F3: identical SEQ in both slots — dedup on payload identity, refuse on
    // conflict (fail-closed, never guess). Runs in every mutation mode.
    if let (Some(a), Some(b)) = (cands[0], cands[1]) {
        if a.header.seq == b.header.seq && a.payload != b.payload {
            return Err(RecoveryError::JournalAmbiguous);
        }
    }

    // F3 dedup: identical (seq, payload) in both slots collapses to one.
    if let (Some(a), Some(b)) = (cands[0], cands[1]) {
        if b.header.seq == a.header.seq {
            cands[1] = None;
        }
    }

    // Top = highest valid sequence (M2 flips this to lowest).
    let top: Option<JournalRecord> = match (cands[0], cands[1]) {
        (Some(a), Some(b)) => {
            if mode == M2LowestSeq {
                if a.header.seq <= b.header.seq { Some(a) } else { Some(b) }
            } else if a.header.seq >= b.header.seq {
                Some(a)
            } else {
                Some(b)
            }
        }
        (a, b) => a.or(b),
    };

    // A03 contract: a valid record whose DAG_SEQ <= base dag seq is
    // counter-rollback-as-corruption; refuse typed, fail-closed (M4 drops it).
    if mode != M4SeqRegressionUnchecked {
        if let Some(rec) = top {
            if rec.payload.dag_seq <= base.dag_seq {
                return Err(RecoveryError::SeqRegression);
            }
        }
    }

    match top {
        Some(rec) => Ok(RecoveryOutcome {
            root: rec.payload,
            report: RecoveryReport::Clean,
            recovered_seq: Some(rec.header.seq),
            slot_verdicts: verdicts,
        }),
        None => {
            // No valid candidate: base root stands. Degraded only when a slot
            // holds non-conforming bytes (damage), not when slots are erased
            // (fresh journal) — F2 semantics with the erased-state carve-out.
            let degraded = verdicts
                .iter()
                .any(|&v| v == SlotVerdict::Damaged);
            Ok(RecoveryOutcome {
                root: base,
                report: if degraded {
                    RecoveryReport::RecoveryDegraded
                } else {
                    RecoveryReport::Clean
                },
                recovered_seq: None,
                slot_verdicts: verdicts,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(dag: u64) -> NewRootDescriptor {
        NewRootDescriptor::new([7; 20], dag * 16, dag, 0, FLAG_DUAL_WRITE)
    }

    #[test]
    fn encode_decode_roundtrip() {
        for (seq, dag) in [(1u64, 10u64), (2, 11), (3, 12), (u64::MAX - 1, 20)] {
            let rec = p(dag);
            let bytes = encode_record(seq, &rec);
            let wire_payload = &bytes[HEADER_SIZE..OFF_TRAILER];
            let recomputed = crc32(wire_payload);
            let stored = u32::from_le_bytes([
                bytes[24],
                bytes[25],
                bytes[26],
                bytes[27],
            ]);
            assert_eq!(
                stored, recomputed,
                "seq {seq}: stored payload crc {stored:#010x} vs crc over wire bytes {recomputed:#010x}; wire payload {wire_payload:02x?}"
            );
            let decoded =
                decode_slot(&bytes).unwrap_or_else(|e| panic!("seq {seq}: rejected {e:?}"));
            let got_seq = decoded.header.seq;
            assert_eq!(got_seq, seq);
            assert_eq!(decoded.payload, rec);
            // The wire payload region carries the descriptor verbatim.
            assert_eq!(&bytes[HEADER_SIZE..OFF_TRAILER], &rec.to_bytes()[..]);
        }
    }

    #[test]
    fn flash_roundtrip_via_medium() {
        let mut m = FlashMedium::new(p(9));
        m.fence_artifacts();
        write_record_cut(&mut m, 1, &p(10), RECORD_SIZE).unwrap();
        let slot0 = m.read_slot(0);
        let verdict = decode_slot(&slot0);
        assert!(
            matches!(verdict, Ok(_)),
            "flash-stored record must decode: {verdict:?}"
        );
        assert_eq!(recover(&m).unwrap().root, p(10));
    }
}
