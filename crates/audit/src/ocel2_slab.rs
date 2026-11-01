//! OCEL 2.0 columnar flight recorder — lane A15, full-lane slice (Wave 3).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A15-powl-ocel2.md` — §4 (normative binary
//! schema, zero strings at the boundary), §3.3 (append path), §6 (courts).
//! Wave 1 froze the row ABI (`Ocel2SlabHeader` / `OcelEventVector` /
//! `OcelObjectRow` + layout pins); this slice lands the logic on it:
//!
//! - `Ocel2InternTable` — §4.3 interning, versioned, capped (overflow is a
//!   typed error, never a wraparound).
//! - `Ocel2ColumnarLog` — the §4.1/§4.2 columnar tables with the real
//!   append path (`append`), segment-addressed `object_ids`, and the object
//!   registry.
//! - `compress_u64_column` / `compress_u16_column` — per-column compression
//!   via the EXISTING cgap-parquet RLE kernels (`cgap_parquet::rle_encode`),
//!   byte-plane transposed, with the row count PINNED in every frame header
//!   (§4.4) so a silently dropped row cannot decode successfully.
//! - `seal_to_slab` / `open_slab` — the real on-disk slab: fixed 64 B
//!   header (magic, schema + intern versions, pinned row counts) + payload,
//!   integrity-sealed with BLAKE3 (`cgap_arrow::blake3::hash` — same
//!   algorithm spec as A03's `cgap-blake3`, reached through this crate's
//!   existing `cgap-arrow` dependency; recorded as lane divergence D4') and
//!   re-opened with fail-closed validation.
//! - `audit_ocel2_append` — the cdylib export (§1.1: the linker cannot
//!   elide an exported symbol, so the append is real compiled behavior).
//!
//! Wave 4 (harden + close DoD): the open path is hardened against FORGED
//! slab bytes — decoder budgets derived from the header or decoded segment
//! sums are refused past [`OCEL2_MAX_DECODER_ROWS`] BEFORE any allocation
//! (a forged row count can no longer request a multi-gigabyte buffer) and
//! granted budgets are fallible reservations (`try_reserve_exact`), so
//! allocation failure surfaces typed, never aborts. The header's
//! intern-table version is now load-bearing (checked, was decorative), and
//! trailing authenticated-but-uninterpreted payload bytes are refused
//! instead of silently ignored. Forged-header / intern-version /
//! trailing-bytes courts close the set (each flips: baseline code either
//! aborts on allocation or accepts the forgery).
//!
//! Wave 7 (defect repair driven by the wave-6 court receipts): two real
//! defects in the intern table, each witnessed by a court that FLIPPED on
//! the pre-repair baseline (both runs cited in the wave receipt) —
//! D1: `decode` accepted non-ascending codes; `next_code` derives from the
//! LAST entry, so a decoded `[2, 1]` table restarted minting at 2 and the
//! next `intern` collided with the existing code-2 entry (code <-> string
//! bijection broken through the public typed API). Now refused typed at
//! the wire, and minting scans forward for the first free code (covers the
//! sibling path: a sparse/saturated table whose `next_code` lands on an
//! existing code).
//! D2: `intern` accepted strings longer than the u16 wire width and
//! `encode`'s `len u16 LE` silently WRAPPED (70,000 B -> 4,464 B on the
//! wire) — the exact "overflow is a typed error, not a wraparound" class
//! §8 bans. Now refused typed at admission
//! ([`Ocel2Error::InternStringTooLong`]); the 65,535 B boundary value
//! round-trips exactly (court-pinned).
//!
//! Wave 8 (defect repair, driven by re-derived wave-7 findings — no A15
//! wave-7 receipt file exists on disk, the divergence is recorded in the
//! wave receipt): two repairs here, two in `powl.rs` (D3/D6), each with a
//! court that flips on the pre-repair baseline (flips witnessed in the
//! mutation sandbox, runs SB-A15-M-D4/M-D5/M-D1/M-D2 cited in the wave
//! receipt).
//! - D4 (intern bijection, D1 sibling): `decode` accepted SPARSE-but-
//!   ascending codes; a table like [10, 65535] saturated `next_code` onto
//!   the taken last code and the forward-only mint scan then answered
//!   `InternTableFull` while 65,533 codes were free. Decode now enforces
//!   the DENSE ascending law (codes 1..=n — exactly what `encode` emits;
//!   the saturated full table is caught by intern's cap check). The
//!   Wave-7 D1 court's expected refusal message is amended accordingly
//!   (dense implies strictly-ascending; strictly stronger law), disclosed
//!   in that court's doc.
//! - D5 (registry exactness): `validate_log` accepted duplicate object
//!   rows — a key collision the append path cannot emit and that
//!   `open_slab`'s last-wins index build silently absorbed. Now refused
//!   typed; `open_slab` inherits the refusal through its `validate_log`
//!   gate.
//!
//! Wave 9 (cross-lane integration): the fleet's wave-9 themes applied to
//! THIS lane's framing — cited, not redefined, and applied where this
//! lane's lease actually reaches:
//! - A04 identity court: a sealed slab's wire identity is a pure function
//!   of the log — seal → open → re-seal is byte-exact, two independent
//!   seals of one log are byte-identical, and the seal receipt's checksum
//!   equals BLAKE3 re-derived from the on-disk payload bytes.
//! - A14 dam (§4 A14: fixed normative order, "first failure returns, never
//!   reordered"; classify BEFORE decode allocation): the open path's
//!   refusal precedence is pinned over doubly-violating slabs (earlier law
//!   fires over the later), and a refused open is measured to allocate
//!   strictly less than a full decode open on the real allocator.
//! - A05 power-cut sweep (A05: "the court sweeps k in 0..=N"; "a green
//!   sweep proves nothing unless the harness can fail"): every prefix and
//!   every single-byte tear of a real sealed slab file fails closed typed,
//!   with exact tear-class counts witnessed; documented v1 ignore-on-read
//!   header bytes are pinned as the ONLY tolerated flips.
//! - A20 stress at the A15 surface: 1,000,000 admitted transitions through
//!   the REAL append path, sealed, re-opened, column-equal over all 1M
//!   rows (POWL verify over the same rows lives in `powl.rs`).
//!
//! Scope boundary (§8, recorded): this is the columnar AT-REST form of the
//! OCEL 2.0 data model (events, objects, qualified relations) — NOT the
//! XML/JSON interchange serializer; that exporter is a follow-up lane.
//! At-rest integers are little-endian (§2.2 boundary invariant). The
//! zero-string law governs the DATA boundary: the append path carries only
//! fixed-width integers (proved alloc-free by the string-leak court); the
//! one sanctioned string site is the intern table at admission time (§4.3)
//! and the C-ABI path argument, which is transport, not OCEL data.

use core::mem::{align_of, offset_of, size_of};
use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt;
use std::path::{Path, PathBuf};

use cgap_arrow::blake3;
use cgap_parquet::{rle_decode, rle_encode, ParquetError};

/// §4.4 slab header magic — ASCII `OCEL2SLAB` (9 bytes; the doc fixes the
/// literal, not the width — width frozen here at Wave 1).
pub const OCEL2_MAGIC: [u8; 9] = *b"OCEL2SLAB";

/// §4.4 schema version, start at 1.
pub const OCEL2_SCHEMA_VERSION: u16 = 1;

/// §4.4 intern-table version, start at 1.
pub const OCEL2_INTERN_TABLE_VERSION: u16 = 1;

/// §4.3: qualifier code 0 = none.
pub const OCEL2_QUALIFIER_NONE: u16 = 0;

/// §8 (risks): the intern vocabulary is capped at 65535 usable codes
/// (code 0 reserved for `OCEL2_QUALIFIER_NONE`); overflow is a typed error,
/// not a wraparound — enforced by [`Ocel2InternTable::intern`].
pub const OCEL2_INTERN_TABLE_CAP: usize = 65_536;

/// Inline object-ref capacity of one event vector. The doc leaves the
/// per-event object list open ("flat column; segmented by object_count
/// prefix sums", §4.1); Wave 1 froze the width at 8 to pin size/align.
/// Changing it is an ABI break (Wave-1 divergence).
pub const OCEL2_MAX_OBJECTS_PER_EVENT: usize = 8;

/// §4.1 `attr_blob`: "u64 words" — 4 words (32 B) frozen at Wave 1.
pub const OCEL2_ATTR_WORDS: usize = 4;

/// Fixed header width in bytes ([`Ocel2SlabHeader`], gap-free, align 8).
pub const OCEL2_HEADER_BYTES: usize = 64;

/// Fixed object-table row width in bytes ([`OcelObjectRow`]).
pub const OCEL2_OBJECT_ROW_BYTES: usize = 24;

/// Wave-4 hardening: decoder-side policy ceiling on any row budget derived
/// from untrusted slab bytes (header row counts, decoded segment sums). A
/// forged count is refused BEFORE allocation — deterministically on every
/// host, no allocator dependence — instead of attempting a multi-gigabyte
/// reservation. Seal-side counts are unaffected (the seal pins the true
/// count); a slab legitimately beyond 2^24 rows per budget needs a format-v2
/// ceiling raise (recorded divergence D5).
pub const OCEL2_MAX_DECODER_ROWS: usize = 1 << 24;

/// §4.4 fixed-width slab header: magic, schema version, intern-table
/// version, event/object row counts (pinned BEFORE compression; the
/// compression court compares against it), and a BLAKE3 checksum over the
/// payload that follows the header.
///
/// Layout is gap-free by construction: the normative magic literal is 9
/// bytes, so one explicit pad byte keeps every following field naturally
/// aligned with the magic still at offset 0. Offsets 0-8, 9, 10-11, 12-13,
/// 16-19, 20-23, 24-31, 32-63 fully occupied; 14..16 is the struct-padding
/// gap (Wave-9 D9: MUST be zero on the wire — refused typed on open; the
/// encoder writes it zero). Exactly 64 bytes, align 8.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct Ocel2SlabHeader {
    /// offset 0, 9 B — `OCEL2_MAGIC` (`OCEL2SLAB`)
    pub magic: [u8; 9],
    /// offset 9, 1 B — explicit alignment pad after the 9-byte magic;
    /// zero on write, ignored on read (v1)
    pub magic_pad: u8,
    /// offset 10, 2 B — u16 LE at rest; `OCEL2_SCHEMA_VERSION`
    pub schema_version: u16,
    /// offset 12, 2 B — u16 LE at rest; `OCEL2_INTERN_TABLE_VERSION`
    pub intern_table_version: u16,
    /// offset 16, 4 B — u32 LE; event rows pinned before compression (§4.4)
    pub event_row_count: u32,
    /// offset 20, 4 B — u32 LE; object rows pinned before compression
    pub object_row_count: u32,
    /// offset 24, 8 B — zero on write, ignored on read (v1)
    pub reserved: [u8; 8],
    /// offset 32, 32 B — BLAKE3 digest over the bytes after the header
    /// (§4.4; computed by `cgap_arrow::blake3::hash` — divergence D4')
    pub checksum: [u8; 32],
}

// §4.4 layout pins — A04 house pattern: core-only const asserts, no
// `static_assertions` crate (zero-dependency rule). A field reorder that
// preserves total size still trips the offset pins.
const _: () = assert!(size_of::<Ocel2SlabHeader>() == 64);
const _: () = assert!(align_of::<Ocel2SlabHeader>() == 8);
const _: () = assert!(size_of::<Ocel2SlabHeader>() == 32 + 32); // gap-free self-check
const _: () = assert!(OCEL2_MAGIC.len() == 9);
const _: () = assert!(OCEL2_INTERN_TABLE_CAP == u16::MAX as usize + 1);

const _: () = assert!(offset_of!(Ocel2SlabHeader, magic) == 0);
const _: () = assert!(offset_of!(Ocel2SlabHeader, magic_pad) == 9);
const _: () = assert!(offset_of!(Ocel2SlabHeader, schema_version) == 10);
const _: () = assert!(offset_of!(Ocel2SlabHeader, intern_table_version) == 12);
const _: () = assert!(offset_of!(Ocel2SlabHeader, event_row_count) == 16);
const _: () = assert!(offset_of!(Ocel2SlabHeader, object_row_count) == 20);
const _: () = assert!(offset_of!(Ocel2SlabHeader, reserved) == 24);
const _: () = assert!(offset_of!(Ocel2SlabHeader, checksum) == 32);

/// §4.1 event vector, inline fixed-width row form: what an admitted
/// transition hands to the append path. The at-rest form is the columnar
/// encoding (§4.1); this row ABI is the producer-side interface.
///
/// Layout is gap-free by construction: 24 B fixed prefix + inline object
/// refs + attribute words; exactly 120 bytes, align 8, no padding holes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct OcelEventVector {
    /// offset 0, 8 B — monotonically increasing, slab-local (§4.1)
    pub event_id: u64,
    /// offset 8, 8 B — UNIX epoch nanoseconds (§4.1); advisory ordering
    /// only — POWL sequence checks use slab order (§8, timestamp risk)
    pub ts_ns: u64,
    /// offset 16, 2 B — interned activity code (§4.1); 0 is reserved
    pub activity: u16,
    /// offset 18, 2 B — interned qualifier code, 0 = `OCEL2_QUALIFIER_NONE`
    /// (§4.3)
    pub qualifier: u16,
    /// offset 20, 2 B — length of the object ref list for this event;
    /// MUST be <= `OCEL2_MAX_OBJECTS_PER_EVENT` (enforced by
    /// [`Ocel2ColumnarLog::append`] as a typed error)
    pub object_count: u16,
    /// offset 22, 2 B — zero on write, ignored on read (v1)
    pub reserved: u16,
    /// offset 24, 64 B — inline object refs; the at-rest column is flat,
    /// segmented by `object_count` prefix sums (§4.1)
    pub object_ids: [u64; OCEL2_MAX_OBJECTS_PER_EVENT],
    /// offset 88, 32 B — fixed-width attribute words, typed by the intern
    /// table (§4.1); no strings
    pub attr_blob: [u64; OCEL2_ATTR_WORDS],
}

const _: () = assert!(size_of::<OcelEventVector>() == 120);
const _: () = assert!(align_of::<OcelEventVector>() == 8);
const _: () = assert!(
    size_of::<OcelEventVector>()
        == 24 + OCEL2_MAX_OBJECTS_PER_EVENT * 8 + OCEL2_ATTR_WORDS * 8
); // gap-free self-check

const _: () = assert!(offset_of!(OcelEventVector, event_id) == 0);
const _: () = assert!(offset_of!(OcelEventVector, ts_ns) == 8);
const _: () = assert!(offset_of!(OcelEventVector, activity) == 16);
const _: () = assert!(offset_of!(OcelEventVector, qualifier) == 18);
const _: () = assert!(offset_of!(OcelEventVector, object_count) == 20);
const _: () = assert!(offset_of!(OcelEventVector, reserved) == 22);
const _: () = assert!(offset_of!(OcelEventVector, object_ids) == 24);
const _: () = assert!(offset_of!(OcelEventVector, attr_blob) == 88);

/// §4.2 object table row, inline fixed-width form. Layout gap-free:
/// offsets 0-7, 8-11, 12-15, 16-17, 18-23 fully occupied; exactly 24
/// bytes, align 8.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct OcelObjectRow {
    /// offset 0, 8 B — object id (§4.2)
    pub object_id: u64,
    /// offset 8, 4 B — u32; index of the first event referencing this
    /// object (§4.2)
    pub first_event_idx: u32,
    /// offset 12, 4 B — u32; POWL-check-relevant (§4.2)
    pub last_event_idx: u32,
    /// offset 16, 2 B — interned object type (§4.2)
    pub object_type: u16,
    /// offset 18, 6 B — zero on write, ignored on read (v1); pads the row
    /// to 24 B with no interior hole
    pub reserved: [u8; 6],
}

const _: () = assert!(size_of::<OcelObjectRow>() == 24);
const _: () = assert!(align_of::<OcelObjectRow>() == 8);
const _: () = assert!(size_of::<OcelObjectRow>() == 8 + 4 + 4 + 2 + 6); // gap-free self-check

const _: () = assert!(offset_of!(OcelObjectRow, object_id) == 0);
const _: () = assert!(offset_of!(OcelObjectRow, first_event_idx) == 8);
const _: () = assert!(offset_of!(OcelObjectRow, last_event_idx) == 12);
const _: () = assert!(offset_of!(OcelObjectRow, object_type) == 16);
const _: () = assert!(offset_of!(OcelObjectRow, reserved) == 18);

// ---------------------------------------------------------------------------
// Wave 3: typed errors and status codes
// ---------------------------------------------------------------------------

/// Typed slab error. No `panic!` on external input anywhere in this module
/// (§7.8): every failure mode of the append/seal/open surface is a value
/// here.
#[derive(Debug)]
pub enum Ocel2Error {
    /// §8 risk: intern vocabulary exhausted (65536 codes; 0 reserved).
    InternTableFull,
    /// Wave-7 (§8: "overflow is a typed error, not a wraparound"): a string
    /// longer than the u16 wire width — `encode` stores `len u16 LE` per
    /// entry, so a longer string cannot be represented without wrapping the
    /// length (a 70,000 B string encoded as 4,464 B). Refused at admission
    /// ([`Ocel2InternTable::intern`]).
    InternStringTooLong {
        /// byte length of the refused string
        len: usize,
    },
    /// Activity/qualifier/type code 0 is reserved (§4.3).
    InternCodeZero,
    /// Event carries more object refs than `OCEL2_MAX_OBJECTS_PER_EVENT`.
    TooManyObjects { got: usize, max: usize },
    /// §4.1: event ids are monotonically increasing, slab-local.
    EventIdNotMonotonic { got: u64, last: u64 },
    /// Unknown object id in `set_object_type`.
    UnknownObject { object_id: u64 },
    /// Event row count exceeds the u32 slab-header width.
    RowCountOverflow { field: &'static str, got: usize },
    /// Slab file does not start with `OCEL2_MAGIC`.
    MagicMismatch,
    /// Schema version differs from `OCEL2_SCHEMA_VERSION`.
    SchemaMismatch { got: u16, expected: u16 },
    /// BLAKE3 of the payload differs from the header checksum (§4.4).
    ChecksumMismatch { expected: [u8; 32], got: [u8; 32] },
    /// Bounded read ran past the end of the buffer.
    Truncated { need: usize, got: usize },
    /// A column frame's pinned row count disagrees with the decode budget
    /// (the compression court's drop-detection point).
    RowPinMismatch { pinned: usize, got: usize },
    /// The RLE kernel refused a frame (typed pass-through of the
    /// cgap-parquet kernel error).
    PlaneDecode(ParquetError),
    /// Intern-table bytes are not valid UTF-8.
    InvalidUtf8,
    /// Structural violation found by [`validate_log`].
    MalformedLog(&'static str),
    /// Wave-4: a decoder budget derived from untrusted slab bytes (header
    /// row count, decoded segment sum, object registry size) exceeded
    /// [`OCEL2_MAX_DECODER_ROWS`] or what the allocator grants — refused
    /// typed, never an allocation abort.
    CapacityExceeded {
        /// which budget (names the column/registry)
        what: &'static str,
        /// the refused row count
        requested: usize,
    },
    /// Wave-4: the header's intern-table version disagrees with
    /// `OCEL2_INTERN_TABLE_VERSION` (the field is load-bearing on open).
    InternVersionMismatch {
        /// version read from the header
        got: u16,
        /// `OCEL2_INTERN_TABLE_VERSION`
        expected: u16,
    },
    /// Wave-4: the payload carries authenticated-but-uninterpreted bytes
    /// after the last object row — refused, never silently ignored.
    TrailingBytes {
        /// number of bytes past the last decoded object row
        got: usize,
    },
    /// Filesystem error on the seal/open path.
    Io(std::io::Error),
}

impl fmt::Display for Ocel2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ocel2Error::InternTableFull => write!(f, "intern table full (cap {OCEL2_INTERN_TABLE_CAP})"),
            Ocel2Error::InternStringTooLong { len } => write!(
                f,
                "intern string {len} B exceeds the u16 wire width ({})",
                u16::MAX
            ),
            Ocel2Error::InternCodeZero => write!(f, "intern code 0 is reserved"),
            Ocel2Error::TooManyObjects { got, max } => {
                write!(f, "event carries {got} object refs, max {max}")
            }
            Ocel2Error::EventIdNotMonotonic { got, last } => {
                write!(f, "event id {got} not greater than last {last}")
            }
            Ocel2Error::UnknownObject { object_id } => write!(f, "unknown object id {object_id}"),
            Ocel2Error::RowCountOverflow { field, got } => {
                write!(f, "{field} row count {got} exceeds u32 header width")
            }
            Ocel2Error::MagicMismatch => write!(f, "bad slab magic"),
            Ocel2Error::SchemaMismatch { got, expected } => {
                write!(f, "schema version {got}, expected {expected}")
            }
            Ocel2Error::ChecksumMismatch { .. } => write!(f, "BLAKE3 checksum mismatch"),
            Ocel2Error::Truncated { need, got } => write!(f, "truncated slab: need {need}, got {got}"),
            Ocel2Error::RowPinMismatch { pinned, got } => {
                write!(f, "row pin mismatch: frame pins {pinned}, budget {got}")
            }
            Ocel2Error::PlaneDecode(e) => write!(f, "RLE plane decode: {e}"),
            Ocel2Error::InvalidUtf8 => write!(f, "intern table is not valid UTF-8"),
            Ocel2Error::MalformedLog(why) => write!(f, "malformed log: {why}"),
            Ocel2Error::CapacityExceeded { what, requested } => {
                write!(
                    f,
                    "{what} budget {requested} exceeds the decoder ceiling ({OCEL2_MAX_DECODER_ROWS}) or the allocator's grant"
                )
            }
            Ocel2Error::InternVersionMismatch { got, expected } => {
                write!(f, "intern table version {got}, expected {expected}")
            }
            Ocel2Error::TrailingBytes { got } => {
                write!(f, "{got} trailing payload bytes after the last object row")
            }
            Ocel2Error::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for Ocel2Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Ocel2Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// C-ABI status codes for [`audit_ocel2_append`]. Local to this lane until
/// A19's `CgapStatus` lands (recorded divergence: the A19 spec is cited,
/// not redefined — this enum is a placeholder with its own wire values and
/// is NOT claimed to be `CgapStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Ocel2Status {
    /// append + seal succeeded
    Ok = 0,
    /// a required pointer argument was null
    NullArg = 1,
    /// the path argument was empty
    PathInvalid = 2,
    /// an event vector failed append validation
    AppendFailed = 3,
    /// the slab seal failed (io/encode)
    SealFailed = 4,
    /// zero events requested — nothing to seal
    EmptyAppend = 5,
}

impl Ocel2Status {
    /// Wire code for the C ABI `status_out` u32.
    #[must_use]
    pub fn code(self) -> u32 {
        self as u32
    }
}

fn status_of(err: &Ocel2Error) -> Ocel2Status {
    match err {
        Ocel2Error::Io(_) | Ocel2Error::InvalidUtf8 | Ocel2Error::RowCountOverflow { .. } => {
            Ocel2Status::SealFailed
        }
        _ => Ocel2Status::AppendFailed,
    }
}

// ---------------------------------------------------------------------------
// Wave 3: intern table (§4.3)
// ---------------------------------------------------------------------------

/// Versioned intern table: canonical string -> u16 code. Code 0 is reserved
/// (`OCEL2_QUALIFIER_NONE`); codes are assigned densely from 1. The table
/// is the ONE sanctioned string site (§4.3): interning happens at admission,
/// behind the boundary; the slab carries the table so a slab is
/// self-describing, and the (follow-up-lane) interchange exporter is the
/// only other string site.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ocel2InternTable {
    entries: Vec<(u16, String)>,
    next_code: u16,
}

impl Ocel2InternTable {
    /// Empty table; first interned code is 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_code: 1,
        }
    }

    /// Interns `s`, returning its code. Idempotent: the same string maps to
    /// the same code. Typed error at the vocabulary cap (§8) — never a
    /// wraparound; typed error for a string longer than the u16 wire width
    /// (Wave-7 D2: `encode` stores `len u16 LE` — longer strings cannot be
    /// represented without wrapping the length). Minting scans forward from
    /// `next_code` for the first free code (Wave-7: a decoded table's
    /// `next_code` may land on an existing code when its codes are sparse
    /// or saturate `u16::MAX` — reuse, never wraparound; `InternTableFull`
    /// when no code is free).
    pub fn intern(&mut self, s: &str) -> Result<u16, Ocel2Error> {
        if let Some((code, _)) = self.entries.iter().find(|(_, e)| e == s) {
            return Ok(*code);
        }
        if s.len() > u16::MAX as usize {
            return Err(Ocel2Error::InternStringTooLong { len: s.len() });
        }
        if self.entries.len() + 1 >= OCEL2_INTERN_TABLE_CAP {
            return Err(Ocel2Error::InternTableFull);
        }
        let mut code = self.next_code;
        while self.get(code).is_some() {
            code = code
                .checked_add(1)
                .ok_or(Ocel2Error::InternTableFull)?; // 65535 -> typed full, never wraps to 0
        }
        self.entries.push((code, s.to_owned()));
        self.next_code = code.saturating_add(1);
        Ok(code)
    }

    /// Canonical string for `code`, if present.
    #[must_use]
    pub fn get(&self, code: u16) -> Option<&str> {
        self.entries
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, s)| s.as_str())
    }

    /// Existing code for `s`, if already interned (never assigns).
    #[must_use]
    pub fn code_of(&self, s: &str) -> Option<u16> {
        self.entries
            .iter()
            .find(|(_, e)| e == s)
            .map(|(c, _)| *c)
    }

    /// Number of interned entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing is interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Wire form: `count u32 LE` then per entry `code u16 LE`, `len u16 LE`,
    /// `bytes`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for (code, s) in &self.entries {
            let b = s.as_bytes();
            out.extend_from_slice(&code.to_le_bytes());
            out.extend_from_slice(&(b.len() as u16).to_le_bytes());
            out.extend_from_slice(b);
        }
    }

    /// Decodes the wire form from `src` (starting at `*pos`), advancing
    /// `pos`. Bounded reads; typed errors; duplicate codes and code 0 are
    /// refused.
    pub fn decode(&mut self, src: &[u8], pos: &mut usize) -> Result<(), Ocel2Error> {
        let count = read_u32(src, pos)? as usize;
        if count >= OCEL2_INTERN_TABLE_CAP {
            return Err(Ocel2Error::MalformedLog("intern count exceeds cap"));
        }
        self.entries.clear();
        self.entries.reserve(count);
        let mut prev_code = 0u16;
        for _ in 0..count {
            let code = read_u16(src, pos)?;
            let len = read_u16(src, pos)? as usize;
            let end = pos
                .checked_add(len)
                .ok_or(Ocel2Error::Truncated { need: len, got: src.len() })?;
            let bytes = src
                .get(*pos..end)
                .ok_or(Ocel2Error::Truncated { need: len, got: src.len() - *pos })?;
            *pos = end;
            if code == 0 {
                return Err(Ocel2Error::InternCodeZero);
            }
            // Wave-7 D1 as tightened by Wave-8 D4 (defect repair): codes
            // MUST be DENSE ascending from 1 — exactly the wire form
            // `encode` emits. Strictly-increasing alone left a sibling of
            // the D1 hole open: a sparse decoded table whose highest code
            // is 65535 ([10, 65535]) saturates `next_code` onto the taken
            // last code (65535.saturating_add(1) == 65535), and the
            // forward-only mint scan then overflows to `InternTableFull`
            // while 65,533 codes are free — the same code <-> string
            // bijection break class as D1. Density closes it: a decoded
            // table of n entries occupies exactly codes 1..=n, so
            // `next_code` = n + 1 is always free below the cap, and the
            // saturated full table (n == 65535) is caught by intern's cap
            // check instead. Refusal is a sealed-slab exactness law (same
            // discipline as the trailing-bytes refusal: a slab decodes to
            // exactly what its encoder emits).
            if code != prev_code.wrapping_add(1) {
                return Err(Ocel2Error::MalformedLog("intern codes not dense ascending"));
            }
            prev_code = code;
            let s = core::str::from_utf8(bytes).map_err(|_| Ocel2Error::InvalidUtf8)?;
            if self.code_of(s).is_some() || self.get(code).is_some() {
                return Err(Ocel2Error::MalformedLog("duplicate intern entry"));
            }
            self.entries.push((code, s.to_owned()));
        }
        self.next_code = self
            .entries
            .last()
            .map_or(1, |(c, _)| c.saturating_add(1));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Wave 3: the columnar log (§4.1/§4.2)
// ---------------------------------------------------------------------------

/// The columnar OCEL 2.0 log: fixed-width columns, flat segmented
/// `object_ids`, object registry, intern table. Fields are public READ
/// access (the POWL check path reads them directly, allocation-free);
/// mutating them by hand bypasses the append invariants — [`validate_log`]
/// exists to check received/hand-built logs, and [`verify`](crate::powl::verify)
/// runs it before any check.
#[derive(Debug, Clone, Default)]
pub struct Ocel2ColumnarLog {
    /// §4.1 event id column (monotonic, slab-local)
    pub event_id: Vec<u64>,
    /// §4.1 timestamp column (UNIX ns, advisory ordering)
    pub ts_ns: Vec<u64>,
    /// §4.1 interned activity column (0 reserved)
    pub activity: Vec<u16>,
    /// §4.1 interned qualifier column (0 = none)
    pub qualifier: Vec<u16>,
    /// §4.1 per-event object-ref counts (segments the flat column)
    pub object_count: Vec<u16>,
    /// §4.1 flat object-ref column, segmented by `object_count` prefix sums
    pub object_ids: Vec<u64>,
    /// §4.2 object table
    pub objects: Vec<OcelObjectRow>,
    /// §4.3 intern table (versioned in the slab header)
    pub intern: Ocel2InternTable,
    /// derived: object id -> row index in `objects`
    pub(crate) object_index: HashMap<u64, u32>,
}

impl Ocel2ColumnarLog {
    /// Empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty log with capacity preallocated for `events` event rows,
    /// `object_refs` flat object refs, and `objects` registry rows. The
    /// zero-allocation append court relies on this.
    #[must_use]
    pub fn with_capacity(events: usize, object_refs: usize, objects: usize) -> Self {
        Self {
            event_id: Vec::with_capacity(events),
            ts_ns: Vec::with_capacity(events),
            activity: Vec::with_capacity(events),
            qualifier: Vec::with_capacity(events),
            object_count: Vec::with_capacity(events),
            object_ids: Vec::with_capacity(object_refs),
            objects: Vec::with_capacity(objects),
            intern: Ocel2InternTable::new(),
            object_index: HashMap::with_capacity(objects),
        }
    }

    /// Number of event rows.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.event_id.len()
    }

    /// Preallocates capacity (see [`Self::with_capacity`]).
    pub fn reserve(&mut self, events: usize, object_refs: usize, objects: usize) {
        self.event_id.reserve(events);
        self.ts_ns.reserve(events);
        self.activity.reserve(events);
        self.qualifier.reserve(events);
        self.object_count.reserve(events);
        self.object_ids.reserve(object_refs);
        self.objects.reserve(objects);
        self.object_index.reserve(objects);
    }

    /// Appends one admitted-transition vector (the §3.3 append path).
    /// Validation is typed, never a panic: reserved activity code, object
    /// overflow, and event-id regression are all refused. With capacity
    /// reserved (see [`Self::with_capacity`]) this path performs ZERO
    /// allocations — the string-leak court's assert.
    pub fn append(&mut self, v: &OcelEventVector) -> Result<(), Ocel2Error> {
        if v.activity == 0 {
            return Err(Ocel2Error::MalformedLog("activity code 0 is reserved"));
        }
        let count = v.object_count as usize;
        if count > OCEL2_MAX_OBJECTS_PER_EVENT {
            return Err(Ocel2Error::TooManyObjects {
                got: count,
                max: OCEL2_MAX_OBJECTS_PER_EVENT,
            });
        }
        if let Some(&last) = self.event_id.last() {
            if v.event_id <= last {
                return Err(Ocel2Error::EventIdNotMonotonic {
                    got: v.event_id,
                    last,
                });
            }
        }
        let event_row = self.event_id.len();
        let Ok(event_row_u32) = u32::try_from(event_row) else {
            return Err(Ocel2Error::RowCountOverflow {
                field: "event",
                got: event_row,
            });
        };
        self.event_id.push(v.event_id);
        self.ts_ns.push(v.ts_ns);
        self.activity.push(v.activity);
        self.qualifier.push(v.qualifier);
        self.object_count.push(v.object_count);
        self.object_ids.extend_from_slice(&v.object_ids[..count]);
        for &object_id in &v.object_ids[..count] {
            match self.object_index.get(&object_id) {
                Some(&row) => self.objects[row as usize].last_event_idx = event_row_u32,
                None => {
                    let row = self.objects.len();
                    self.object_index.insert(object_id, row as u32);
                    self.objects.push(OcelObjectRow {
                        object_id,
                        first_event_idx: event_row_u32,
                        last_event_idx: event_row_u32,
                        object_type: 0,
                        reserved: [0; 6],
                    });
                }
            }
        }
        Ok(())
    }

    /// Sets the interned object type of a registered object (§4.2). Typed
    /// error on an unknown id — append registered every referenced object.
    pub fn set_object_type(&mut self, object_id: u64, object_type: u16) -> Result<(), Ocel2Error> {
        let row = self
            .object_index
            .get(&object_id)
            .ok_or(Ocel2Error::UnknownObject { object_id })?;
        self.objects[*row as usize].object_type = object_type;
        Ok(())
    }

    /// The `object_ids` segment of event `i` (§4.1: segmented by
    /// `object_count` prefix sums). Saturating bounds — never panics, even
    /// on a hand-built log whose segments are inconsistent.
    #[must_use]
    pub fn object_ids_for(&self, i: usize) -> &[u64] {
        let mut start = 0usize;
        for k in 0..i.min(self.object_count.len()) {
            start = start.saturating_add(self.object_count[k] as usize);
        }
        let end = start.saturating_add(
            self.object_count
                .get(i)
                .copied()
                .unwrap_or(0) as usize,
        );
        let end = end.min(self.object_ids.len());
        let start = start.min(end);
        &self.object_ids[start..end]
    }
}

/// Structural validation of a (possibly hand-built or received) log:
/// equal column lengths, monotonic event ids (§4.1), reserved activity
/// code, segment arithmetic (`sum(object_count) == object_ids.len()`), and
/// object-registry coverage. Allocates (the check path never runs this —
/// [`verify`](crate::powl::verify) validates BEFORE indexing).
pub fn validate_log(log: &Ocel2ColumnarLog) -> Result<(), Ocel2Error> {
    let n = log.event_id.len();
    let cols = [
        log.ts_ns.len(),
        log.activity.len(),
        log.qualifier.len(),
        log.object_count.len(),
    ];
    if cols.iter().any(|&c| c != n) {
        return Err(Ocel2Error::MalformedLog("column length mismatch"));
    }
    for w in log.event_id.windows(2) {
        if w[1] <= w[0] {
            return Err(Ocel2Error::MalformedLog("event ids not monotonic"));
        }
    }
    for &a in log.activity.iter() {
        if a == 0 {
            return Err(Ocel2Error::MalformedLog("activity code 0 is reserved"));
        }
    }
    let mut refs = 0usize;
    for &c in &log.object_count {
        refs = refs
            .checked_add(c as usize)
            .ok_or(Ocel2Error::MalformedLog("segment sum overflow"))?;
    }
    if refs != log.object_ids.len() {
        return Err(Ocel2Error::MalformedLog("segment sum != object_ids len"));
    }
    // Registry coverage: every referenced object id must have a §4.2 row.
    let mut rows: Vec<u64> = log.objects.iter().map(|r| r.object_id).collect();
    rows.sort_unstable();
    // Wave-8 D5 (defect repair): duplicate object ids are a registry key
    // collision — the append path cannot emit one (its `object_index`
    // dedups), so a duplicate row is a wire form the producer cannot
    // produce, and `open_slab`'s index build would SILENTLY absorb it by
    // last-wins overwrite. Refused typed instead.
    if rows.windows(2).any(|w| w[0] == w[1]) {
        return Err(Ocel2Error::MalformedLog("duplicate object row"));
    }
    // Wave-9 defect repair D7: the registry-coverage walk previously called
    // `object_ids_for(i)` per event, recomputing the segment prefix sum from
    // row 0 each call — O(n^2) in event rows (the A20 1M-row stress could
    // not complete pre-repair; post-repair timing witness in the wave
    // report). One linear pass with a running segment offset; the LAW is
    // unchanged (every referenced object id must have a §4.2 row — same
    // refusal, pinned by the malformed-shape courts). Segment arithmetic is
    // in-bounds by proof here: `refs` was checked-add above (overflow
    // refused) and equals `object_ids.len()` (mismatch refused), so every
    // prefix segment ends at or before `object_ids.len()`.
    let mut seg = 0usize;
    for &count in &log.object_count {
        let end = seg + count as usize;
        for &oid in &log.object_ids[seg..end] {
            if rows.binary_search(&oid).is_err() {
                return Err(Ocel2Error::MalformedLog("object row missing for reference"));
            }
        }
        seg = end;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Wave 3: per-column compression — byte-plane transposed RLE over the
// EXISTING cgap-parquet kernels, row count pinned per frame (§4.4).
// ---------------------------------------------------------------------------

/// Frame layout (u64 column): `n u64 LE` then 8 planes, each
/// `len u32 LE` + `rle_encode(plane, 8)` bytes. Plane p holds byte p of
/// each value (LSB-first). Row pin: the decoder refuses any decode budget
/// that disagrees with the frame's `n` — a silently dropped row cannot
/// decode (the compression court's flip point).
pub fn compress_u64_column(values: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + values.len() * 8 + 8 * 8);
    out.extend_from_slice(&(values.len() as u64).to_le_bytes());
    let mut lanes: Vec<u32> = Vec::with_capacity(values.len());
    for plane in 0..8 {
        lanes.clear();
        for &v in values {
            lanes.push(((v >> (plane * 8)) & 0xFF) as u32);
        }
        let enc = rle_encode(&lanes, 8);
        out.extend_from_slice(&(enc.len() as u32).to_le_bytes());
        out.extend_from_slice(&enc);
    }
    out
}

/// Decodes a [`compress_u64_column`] frame into `out` (full-buffer,
/// row-pinned). Returns the number of frame bytes consumed. Typed errors
/// only.
pub fn decompress_u64_column(frame: &[u8], out: &mut [u64]) -> Result<usize, Ocel2Error> {
    let mut pos = 0usize;
    let n = read_u64(frame, &mut pos)?;
    if out.len() != n as usize {
        return Err(Ocel2Error::RowPinMismatch {
            pinned: n as usize,
            got: out.len(),
        });
    }
    out.fill(0);
    let mut lanes: Vec<u32> = vec![0; n as usize];
    for plane in 0..8 {
        let len = read_u32(frame, &mut pos)? as usize;
        let end = pos
            .checked_add(len)
            .ok_or(Ocel2Error::Truncated { need: len, got: frame.len() })?;
        let src = frame
            .get(pos..end)
            .ok_or(Ocel2Error::Truncated { need: len, got: frame.len() - pos })?;
        pos = end;
        rle_decode(src, &mut lanes).map_err(Ocel2Error::PlaneDecode)?;
        for (k, lane) in lanes.iter().enumerate() {
            out[k] |= u64::from(*lane) << (plane * 8);
        }
    }
    Ok(pos)
}

/// Frame layout (u16 column): `n u64 LE` then 2 byte-planes.
pub fn compress_u16_column(values: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + values.len() * 2 + 4 * 4);
    out.extend_from_slice(&(values.len() as u64).to_le_bytes());
    let mut lanes: Vec<u32> = Vec::with_capacity(values.len());
    for plane in 0..2 {
        lanes.clear();
        for &v in values {
            lanes.push(((v >> (plane * 8)) & 0xFF) as u32);
        }
        let enc = rle_encode(&lanes, 8);
        out.extend_from_slice(&(enc.len() as u32).to_le_bytes());
        out.extend_from_slice(&enc);
    }
    out
}

/// Decodes a [`compress_u16_column`] frame into `out` (full-buffer,
/// row-pinned). Returns the number of frame bytes consumed.
pub fn decompress_u16_column(frame: &[u8], out: &mut [u16]) -> Result<usize, Ocel2Error> {
    let mut pos = 0usize;
    let n = read_u64(frame, &mut pos)?;
    if out.len() != n as usize {
        return Err(Ocel2Error::RowPinMismatch {
            pinned: n as usize,
            got: out.len(),
        });
    }
    out.fill(0);
    let mut lanes: Vec<u32> = vec![0; n as usize];
    for plane in 0..2 {
        let len = read_u32(frame, &mut pos)? as usize;
        let end = pos
            .checked_add(len)
            .ok_or(Ocel2Error::Truncated { need: len, got: frame.len() })?;
        let src = frame
            .get(pos..end)
            .ok_or(Ocel2Error::Truncated { need: len, got: frame.len() - pos })?;
        pos = end;
        rle_decode(src, &mut lanes).map_err(Ocel2Error::PlaneDecode)?;
        for (k, lane) in lanes.iter().enumerate() {
            out[k] |= (*lane as u16) << (plane * 8);
        }
    }
    Ok(pos)
}

// ---------------------------------------------------------------------------
// Wave 3: slab seal / open (§4.4, §3.3)
// ---------------------------------------------------------------------------

/// Receipt of a successful [`seal_to_slab`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ocel2SlabSeal {
    /// event rows pinned in the header (equals the log's event count)
    pub event_row_count: u32,
    /// object rows pinned in the header
    pub object_row_count: u32,
    /// BLAKE3 digest written into the header (over the payload)
    pub checksum: [u8; 32],
    /// total bytes written to the file
    pub bytes_written: u64,
}

/// Payload layout v1 (after the 64 B header):
/// `intern section` + `frame(event_id)` + `frame(ts_ns)` +
/// `frame(activity u16)` + `frame(qualifier u16)` +
/// `frame(object_count u16)` + `frame(object_ids flat u64)` +
/// `object_row_count x 24 B object rows`. The checksum covers ALL payload
/// bytes; the row counts are cross-pinned header-against-frame at open.
///
/// Recorded scope note (§9/§8): A02's Parquet mmap substrate is an
/// intentionally-empty pre-wave module, so this v1 slab is a standalone
/// container file whose column FRAMES are compressed with the cgap-parquet
/// RLE kernels — the Parquet adjacency that exists today. Incremental
/// file-level append (true flight-recorder tail writes) is a follow-up;
/// `seal_to_slab` is a full-slab snapshot seal.
pub fn seal_to_slab(log: &Ocel2ColumnarLog, path: &Path) -> Result<Ocel2SlabSeal, Ocel2Error> {
    let Ok(event_row_count) = u32::try_from(log.event_count()) else {
        return Err(Ocel2Error::RowCountOverflow {
            field: "event",
            got: log.event_count(),
        });
    };
    let Ok(object_row_count) = u32::try_from(log.objects.len()) else {
        return Err(Ocel2Error::RowCountOverflow {
            field: "object",
            got: log.objects.len(),
        });
    };
    let mut payload = Vec::new();
    log.intern.encode(&mut payload);
    payload.extend_from_slice(&compress_u64_column(&log.event_id));
    payload.extend_from_slice(&compress_u64_column(&log.ts_ns));
    payload.extend_from_slice(&compress_u16_column(&log.activity));
    payload.extend_from_slice(&compress_u16_column(&log.qualifier));
    payload.extend_from_slice(&compress_u16_column(&log.object_count));
    payload.extend_from_slice(&compress_u64_column(&log.object_ids));
    for row in &log.objects {
        payload.extend_from_slice(&row.object_id.to_le_bytes());
        payload.extend_from_slice(&row.first_event_idx.to_le_bytes());
        payload.extend_from_slice(&row.last_event_idx.to_le_bytes());
        payload.extend_from_slice(&row.object_type.to_le_bytes());
        payload.extend_from_slice(&row.reserved);
    }
    let checksum = blake3::hash(&payload);
    let header = Ocel2SlabHeader {
        magic: OCEL2_MAGIC,
        magic_pad: 0,
        schema_version: OCEL2_SCHEMA_VERSION,
        intern_table_version: OCEL2_INTERN_TABLE_VERSION,
        event_row_count,
        object_row_count,
        reserved: [0; 8],
        checksum,
    };
    let mut blob = Vec::with_capacity(OCEL2_HEADER_BYTES + payload.len());
    blob.extend_from_slice(&encode_header(&header));
    blob.extend_from_slice(&payload);
    std::fs::write(path, &blob).map_err(Ocel2Error::Io)?;
    Ok(Ocel2SlabSeal {
        event_row_count,
        object_row_count,
        checksum,
        bytes_written: blob.len() as u64,
    })
}

/// Wave-4: one decoder budget — ceiling-checked, then a fallible exact
/// reservation, then filled. A forged row count is refused as
/// [`Ocel2Error::CapacityExceeded`] BEFORE any allocation; an over-ceiling
/// grant the allocator refuses surfaces typed too. `resize` runs only after
/// the reservation succeeded, so it cannot reallocate.
fn budget_vec<T: Clone>(what: &'static str, n: usize, fill: T) -> Result<Vec<T>, Ocel2Error> {
    if n > OCEL2_MAX_DECODER_ROWS {
        return Err(Ocel2Error::CapacityExceeded { what, requested: n });
    }
    let mut v: Vec<T> = Vec::new();
    v.try_reserve_exact(n)
        .map_err(|_| Ocel2Error::CapacityExceeded { what, requested: n })?;
    v.resize(n, fill);
    Ok(v)
}

/// Opens and fully validates a slab written by [`seal_to_slab`]:
/// magic, schema version, BLAKE3 payload checksum, frame-vs-header row
/// cross-pins, bounded reads throughout, and [`validate_log`] on the
/// decoded columns. Any mismatch is a typed error — never a panic, never a
/// silent pass.
pub fn open_slab(path: &Path) -> Result<Ocel2ColumnarLog, Ocel2Error> {
    let bytes = std::fs::read(path).map_err(Ocel2Error::Io)?;
    if bytes.len() < OCEL2_HEADER_BYTES {
        return Err(Ocel2Error::Truncated {
            need: OCEL2_HEADER_BYTES,
            got: bytes.len(),
        });
    }
    let header = decode_header(&bytes[..OCEL2_HEADER_BYTES])?;
    if header.magic != OCEL2_MAGIC {
        return Err(Ocel2Error::MagicMismatch);
    }
    if header.schema_version != OCEL2_SCHEMA_VERSION {
        return Err(Ocel2Error::SchemaMismatch {
            got: header.schema_version,
            expected: OCEL2_SCHEMA_VERSION,
        });
    }
    // Wave-4: the intern-table version is load-bearing — a header that
    // disagrees with this build's table version is refused before any
    // decode (previously written but never checked).
    if header.intern_table_version != OCEL2_INTERN_TABLE_VERSION {
        return Err(Ocel2Error::InternVersionMismatch {
            got: header.intern_table_version,
            expected: OCEL2_INTERN_TABLE_VERSION,
        });
    }
    let payload = &bytes[OCEL2_HEADER_BYTES..];
    let got = blake3::hash(payload);
    if got != header.checksum {
        return Err(Ocel2Error::ChecksumMismatch {
            expected: header.checksum,
            got,
        });
    }
    let events = header.event_row_count as usize;
    let mut pos = 0usize;
    let mut intern = Ocel2InternTable::new();
    intern.decode(payload, &mut pos)?;
    // Each frame decoder returns the bytes it consumed, so the cursor
    // walks the payload exactly once (frame-vs-header row counts are
    // cross-pinned inside the decoders via the resize budgets below). Every
    // budget is a Wave-4 fallible reservation: ceiling-checked, then
    // `try_reserve_exact` — a forged count is typed, never an abort.
    let mut event_id = budget_vec("event_id", events, 0u64)?;
    pos += decompress_u64_column(&payload[pos..], &mut event_id)?;
    let mut ts_ns = budget_vec("ts_ns", events, 0u64)?;
    pos += decompress_u64_column(&payload[pos..], &mut ts_ns)?;
    let mut activity = budget_vec("activity", events, 0u16)?;
    pos += decompress_u16_column(&payload[pos..], &mut activity)?;
    let mut qualifier = budget_vec("qualifier", events, 0u16)?;
    pos += decompress_u16_column(&payload[pos..], &mut qualifier)?;
    let mut object_count = budget_vec("object_count", events, 0u16)?;
    pos += decompress_u16_column(&payload[pos..], &mut object_count)?;
    // The flat object-refs column is pinned to its own row count, which
    // must equal the segment sum of `object_count`.
    let mut refs = 0usize;
    for &c in &object_count {
        refs = refs
            .checked_add(c as usize)
            .ok_or(Ocel2Error::MalformedLog("segment sum overflow"))?;
    }
    let mut object_ids = budget_vec("object_ids", refs, 0u64)?;
    pos += decompress_u64_column(&payload[pos..], &mut object_ids)?;
    let object_rows = header.object_row_count as usize;
    if object_rows > OCEL2_MAX_DECODER_ROWS {
        return Err(Ocel2Error::CapacityExceeded {
            what: "object rows",
            requested: object_rows,
        });
    }
    let mut objects: Vec<OcelObjectRow> = Vec::new();
    objects
        .try_reserve_exact(object_rows)
        .map_err(|_| Ocel2Error::CapacityExceeded {
            what: "object rows",
            requested: object_rows,
        })?;
    for _ in 0..header.object_row_count {
        let object_id = read_u64(payload, &mut pos)?;
        let first_event_idx = read_u32(payload, &mut pos)?;
        let last_event_idx = read_u32(payload, &mut pos)?;
        let object_type = read_u16(payload, &mut pos)?;
        let mut reserved = [0u8; 6];
        for slot in &mut reserved {
            *slot = read_u8(payload, &mut pos)?;
        }
        objects.push(OcelObjectRow {
            object_id,
            first_event_idx,
            last_event_idx,
            object_type,
            reserved,
        });
    }
    // Wave-4: authenticated-but-uninterpreted payload bytes are a format
    // violation, not slack — refused so a sealed slab is exactly its
    // decoded content (the trailing-bytes court proves the checksum is not
    // the refusal reason by re-sealing honestly).
    if pos != payload.len() {
        return Err(Ocel2Error::TrailingBytes {
            got: payload.len() - pos,
        });
    }
    let mut object_index: HashMap<u64, u32> = HashMap::new();
    object_index
        .try_reserve(object_rows)
        .map_err(|_| Ocel2Error::CapacityExceeded {
            what: "object index",
            requested: object_rows,
        })?;
    for (i, row) in objects.iter().enumerate() {
        object_index.insert(row.object_id, i as u32);
    }
    let log = Ocel2ColumnarLog {
        event_id,
        ts_ns,
        activity,
        qualifier,
        object_count,
        object_ids,
        objects,
        intern,
        object_index,
    };
    validate_log(&log)?;
    Ok(log)
}

fn encode_header(h: &Ocel2SlabHeader) -> [u8; OCEL2_HEADER_BYTES] {
    let mut b = [0u8; OCEL2_HEADER_BYTES];
    b[0..9].copy_from_slice(&h.magic);
    b[9] = h.magic_pad;
    b[10..12].copy_from_slice(&h.schema_version.to_le_bytes());
    b[12..14].copy_from_slice(&h.intern_table_version.to_le_bytes());
    b[16..20].copy_from_slice(&h.event_row_count.to_le_bytes());
    b[20..24].copy_from_slice(&h.object_row_count.to_le_bytes());
    b[24..32].copy_from_slice(&h.reserved);
    b[32..64].copy_from_slice(&h.checksum);
    b
}

fn decode_header(b: &[u8]) -> Result<Ocel2SlabHeader, Ocel2Error> {
    let mut pos = 0usize;
    let mut magic = [0u8; 9];
    for slot in &mut magic {
        *slot = read_u8(b, &mut pos)?;
    }
    let magic_pad = read_u8(b, &mut pos)?;
    let schema_version = read_u16(b, &mut pos)?;
    let intern_table_version = read_u16(b, &mut pos)?;
    // Wave-9 defect repair D9 (found by the A05 power-cut flip sweep): the
    // 2 B struct-padding gap at offsets 14..16 was silently tolerated on
    // read — `encode_header` never writes it (zeroed buffer), but a FORGED
    // header with garbage there opened fine, an undocumented tolerance of
    // exactly the class the wave-4 trailing-bytes law bans for the payload.
    // Sealed-slab exactness: the gap is load-bearing (must be zero), refused
    // typed. The v1-documented ignore-on-read bytes (magic_pad offset 9,
    // reserved 24..32) keep their documented tolerance — pinned by exact
    // count in the flip sweep.
    if b[14] != 0 || b[15] != 0 {
        return Err(Ocel2Error::MalformedLog("header gap bytes 14..16 not zero"));
    }
    pos = 16;
    let event_row_count = read_u32(b, &mut pos)?;
    let object_row_count = read_u32(b, &mut pos)?;
    pos = 32;
    let mut checksum = [0u8; 32];
    for slot in &mut checksum {
        *slot = read_u8(b, &mut pos)?;
    }
    Ok(Ocel2SlabHeader {
        magic,
        magic_pad,
        schema_version,
        intern_table_version,
        event_row_count,
        object_row_count,
        reserved: [0; 8],
        checksum,
    })
}

fn read_u8(src: &[u8], pos: &mut usize) -> Result<u8, Ocel2Error> {
    let b = src
        .get(*pos)
        .ok_or(Ocel2Error::Truncated { need: 1, got: src.len() })?;
    *pos += 1;
    Ok(*b)
}

fn read_u16(src: &[u8], pos: &mut usize) -> Result<u16, Ocel2Error> {
    let end = pos
        .checked_add(2)
        .ok_or(Ocel2Error::Truncated { need: 2, got: src.len() })?;
    let s = src
        .get(*pos..end)
        .ok_or(Ocel2Error::Truncated {
            need: 2,
            got: src.len() - *pos,
        })?;
    *pos = end;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32(src: &[u8], pos: &mut usize) -> Result<u32, Ocel2Error> {
    let end = pos
        .checked_add(4)
        .ok_or(Ocel2Error::Truncated { need: 4, got: src.len() })?;
    let s = src
        .get(*pos..end)
        .ok_or(Ocel2Error::Truncated {
            need: 4,
            got: src.len() - *pos,
        })?;
    *pos = end;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u64(src: &[u8], pos: &mut usize) -> Result<u64, Ocel2Error> {
    let end = pos
        .checked_add(8)
        .ok_or(Ocel2Error::Truncated { need: 8, got: src.len() })?;
    let s = src
        .get(*pos..end)
        .ok_or(Ocel2Error::Truncated {
            need: 8,
            got: src.len() - *pos,
        })?;
    *pos = end;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}

// ---------------------------------------------------------------------------
// Wave 3: cdylib export (§1.1, §7.9) — the append path as real compiled
// behavior the linker cannot elide.
// ---------------------------------------------------------------------------

fn write_status(out: *mut u32, status: Ocel2Status) {
    if !out.is_null() {
        // # Safety: caller contract — non-null `status_out` points to a
        // writable u32.
        unsafe { *out = status.code() };
    }
}

/// Appends `n` admitted-transition vectors into a fresh log and seals it to
/// the slab file at `path`. Returns the pinned event row count (== `n`) on
/// success, 0 on ANY failure; `status_out` (nullable) receives the
/// [`Ocel2Status`] wire code for disambiguation (return value 0 with
/// status `Ok` cannot occur: an empty append is refused
/// `EmptyAppend`).
///
/// # Safety
/// `path` must point to a valid NUL-terminated C string; `events` must
/// bound `n` readable `OcelEventVector` values; `status_out`, if non-null,
/// must point to a writable u32.
#[no_mangle]
pub unsafe extern "C" fn audit_ocel2_append(
    path: *const core::ffi::c_char,
    events: *const OcelEventVector,
    n: usize,
    status_out: *mut u32,
) -> u64 {
    if path.is_null() || (n > 0 && events.is_null()) {
        write_status(status_out, Ocel2Status::NullArg);
        return 0;
    }
    if n == 0 {
        write_status(status_out, Ocel2Status::EmptyAppend);
        return 0;
    }
    // Transport site: the C string is a FILE PATH argument, not OCEL event
    // data; lossy conversion here does not put a string on the data
    // boundary (the string-leak court targets the append path proper).
    let path_cstr = unsafe { CStr::from_ptr(path) };
    let lossy = path_cstr.to_string_lossy();
    if lossy.is_empty() {
        write_status(status_out, Ocel2Status::PathInvalid);
        return 0;
    }
    let path_buf = PathBuf::from(lossy.into_owned());
    let events_slice = unsafe { core::slice::from_raw_parts(events, n) };
    let mut log = Ocel2ColumnarLog::new();
    log.reserve(n, n, n);
    for v in events_slice {
        if let Err(e) = log.append(v) {
            write_status(status_out, status_of(&e));
            return 0;
        }
    }
    match seal_to_slab(&log, &path_buf) {
        Ok(seal) => {
            write_status(status_out, Ocel2Status::Ok);
            u64::from(seal.event_row_count)
        }
        Err(e) => {
            write_status(status_out, status_of(&e));
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Courts (§6.1): compression, OCEL append, zero-string boundary, and the
// mutation anti-vacuity courts over REAL slab files on disk.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_counter::with_alloc_tracking;

    const A: u16 = 1;
    const B: u16 = 2;
    const C: u16 = 3;

    fn vec_for(event_id: u64, ts_ns: u64, activity: u16, object_id: u64) -> OcelEventVector {
        OcelEventVector {
            event_id,
            ts_ns,
            activity,
            qualifier: 0,
            object_count: 1,
            reserved: 0,
            object_ids: [object_id, 0, 0, 0, 0, 0, 0, 0],
            attr_blob: [0; OCEL2_ATTR_WORDS],
        }
    }

    /// Unique real temp dir per court run (no tempfile dep; std only).
    fn court_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "cgap-a15-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_log() -> Ocel2ColumnarLog {
        // 10 events over 3 activities, 3 objects, one multi-ref event,
        // one non-zero qualifier, one typed object — round-trip targets.
        let mut log = Ocel2ColumnarLog::with_capacity(10, 11, 3);
        log.intern.intern("order").unwrap();
        log.intern.intern("item").unwrap();
        log.intern.intern("approve").unwrap();
        for i in 0..10u64 {
            let activity = [A, B, C][i as usize % 3];
            let object = [1000u64, 2000, 3000][(i % 3) as usize];
            let mut v = vec_for(i + 1, 1_700_000_000_000u64 + i * 1000, activity, object);
            if i == 4 {
                // a second object ref on event 5
                v.object_count = 2;
                v.object_ids[1] = 3000;
            }
            if i == 7 {
                v.qualifier = 2;
            }
            log.append(&v).unwrap();
        }
        log.set_object_type(2000, 2).unwrap();
        log
    }

    fn assert_logs_equal(a: &Ocel2ColumnarLog, b: &Ocel2ColumnarLog) {
        assert_eq!(a.event_count(), b.event_count());
        assert_eq!(a.event_id, b.event_id, "event_id column");
        assert_eq!(a.ts_ns, b.ts_ns, "ts_ns column");
        assert_eq!(a.activity, b.activity, "activity column");
        assert_eq!(a.qualifier, b.qualifier, "qualifier column");
        assert_eq!(a.object_count, b.object_count, "object_count column");
        assert_eq!(a.object_ids, b.object_ids, "object_ids flat column");
        assert_eq!(a.objects.len(), b.objects.len(), "object row count");
        for (ra, rb) in a.objects.iter().zip(b.objects.iter()) {
            assert_eq!(ra.object_id, rb.object_id);
            assert_eq!(ra.first_event_idx, rb.first_event_idx);
            assert_eq!(ra.last_event_idx, rb.last_event_idx);
            assert_eq!(ra.object_type, rb.object_type);
        }
        assert_eq!(a.intern.len(), b.intern.len(), "intern entries");
        for code in 1..=a.intern.len() as u16 {
            assert_eq!(a.intern.get(code), b.intern.get(code), "intern {code}");
        }
    }

    /// §6.1 OCEL append court (DoD 3): append via the REAL append path,
    /// seal to a REAL file, re-open, compare column-equal.
    #[test]
    fn ocel_append_court_seal_reopen_column_equal() {
        let dir = court_dir("append");
        let path = dir.join("slab.ocel2");
        let log = sample_log();
        let seal = seal_to_slab(&log, &path).unwrap();
        assert_eq!(seal.event_row_count, 10);
        assert_eq!(seal.object_row_count, 3);
        assert!(seal.checksum != [0u8; 32]);
        assert!(seal.bytes_written > OCEL2_HEADER_BYTES as u64);
        let reopened = open_slab(&path).unwrap();
        assert_logs_equal(&log, &reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §6.1 Compression court + `compress-drop` anti-vacuity (DoD 5):
    /// 10 rows round-trip; the seeded drop mutant (9-row column decoded
    /// against a 10-row budget) MUST be refused by the row pin.
    #[test]
    fn compression_court_round_trip_and_drop_mutation() {
        let values: Vec<u64> = (0..10u64)
            .map(|i| (i.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ (i << 40))
            .collect();
        let frame = compress_u64_column(&values);
        let mut out = vec![0u64; 10];
        decompress_u64_column(&frame, &mut out).unwrap();
        assert_eq!(out, values);
        // Encoder determinism: same column -> identical frame bytes.
        assert_eq!(frame, compress_u64_column(&values));

        // ANTI-VACUITY `compress-drop`: the mutant is the compressor applied
        // to the 9-row prefix (the "silent row drop"). The row pin MUST
        // refuse it — if the pin check were removed, this assert fails.
        let dropped = compress_u64_column(&values[..9]);
        let err = decompress_u64_column(&dropped, &mut out).unwrap_err();
        assert!(
            matches!(err, Ocel2Error::RowPinMismatch { pinned: 9, got: 10 }),
            "drop mutant must be refused by the row pin, got {err:?}"
        );
        // Real restore: the original frame still decodes exactly.
        decompress_u64_column(&frame, &mut out).unwrap();
        assert_eq!(out, values);

        // u16 column: same court shape.
        let acts: Vec<u16> = (1..=10u16).map(|i| (i * 7) % 251).collect();
        let frame16 = compress_u16_column(&acts);
        let mut out16 = vec![0u16; 10];
        decompress_u16_column(&frame16, &mut out16).unwrap();
        assert_eq!(out16, acts);
        let dropped16 = compress_u16_column(&acts[..9]);
        assert!(matches!(
            decompress_u16_column(&dropped16, &mut out16),
            Err(Ocel2Error::RowPinMismatch { pinned: 9, got: 10 })
        ));
    }

    /// §6.1 `ocel-mutate` anti-vacuity (DoD 6): three mutation classes over
    /// the REAL file — payload byte (checksum), header row count
    /// (cross-pin), magic — each MUST be caught; each restore is byte-exact
    /// and the re-open MUST pass again.
    #[test]
    fn ocel_mutate_court_catches_all_classes_and_restores_byte_exact() {
        let dir = court_dir("mutate");
        let path = dir.join("slab.ocel2");
        let log = sample_log();
        seal_to_slab(&log, &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(original.len() > OCEL2_HEADER_BYTES + 70);

        let mutate = |offset: usize| {
            let mut bytes = original.clone();
            bytes[offset] ^= 0x01;
            std::fs::write(&path, &bytes).unwrap();
        };

        // 1. Payload byte flip -> BLAKE3 checksum mismatch.
        mutate(OCEL2_HEADER_BYTES + 70);
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::ChecksumMismatch { .. })
        ));
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        assert!(open_slab(&path).is_ok(), "re-open after restore");

        // 2. Header event_row_count flip -> header-vs-frame cross-pin.
        mutate(16);
        assert!(
            matches!(open_slab(&path), Err(Ocel2Error::RowPinMismatch { .. }))
                || matches!(open_slab(&path), Err(Ocel2Error::Truncated { .. })),
            "header count mutation must be refused by a typed pin/truncation error"
        );
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        assert!(open_slab(&path).is_ok(), "re-open after restore");

        // 3. Magic byte flip -> magic mismatch.
        mutate(0);
        assert!(matches!(open_slab(&path), Err(Ocel2Error::MagicMismatch)));
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        let reopened = open_slab(&path).unwrap();
        assert_logs_equal(&log, &reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §6.1 `string-leak` court (DoD 7): the append boundary performs ZERO
    /// allocations on a reserved log — any `String`/`format!` materialized
    /// on the data path flips the count. Control arm proves the court
    /// measures: an unreserved append does allocate.
    #[test]
    fn string_leak_court_append_is_allocation_free() {
        let v = vec_for(1, 1000, A, 100);
        let mut reserved = Ocel2ColumnarLog::with_capacity(1, 1, 1);
        let (_, report) = with_alloc_tracking(|| reserved.append(&v));
        assert!(
            report.is_zero(),
            "append path allocated on a reserved log: {report:?}"
        );
        // Control (anti-vacuity): unreserved log -> real allocations.
        let mut unreserved = Ocel2ColumnarLog::new();
        let (_, control) = with_alloc_tracking(|| unreserved.append(&v));
        assert!(
            !control.is_zero(),
            "control append allocated nothing; the court would be vacuous"
        );
    }

    /// §7.9: the export is exercised through the REAL compiled symbol (the
    /// cdylib export linked from this crate's object), not only the Rust
    /// path.
    #[test]
    fn export_court_appends_through_the_real_symbol() {
        extern "C" {
            fn audit_ocel2_append(
                path: *const core::ffi::c_char,
                events: *const OcelEventVector,
                n: usize,
                status_out: *mut u32,
            ) -> u64;
        }
        let dir = court_dir("export");
        let path = dir.join("slab.ocel2");
        let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let events = [
            vec_for(1, 100, A, 1000),
            vec_for(2, 200, B, 1000),
            vec_for(3, 300, C, 2000),
        ];
        let mut status: u32 = u32::MAX;
        let sealed = unsafe {
            audit_ocel2_append(cpath.as_ptr(), events.as_ptr(), 3, &mut status)
        };
        assert_eq!(sealed, 3, "export must return the pinned event row count");
        assert_eq!(status, Ocel2Status::Ok.code());
        let log = open_slab(&path).unwrap();
        assert_eq!(log.event_count(), 3);
        assert_eq!(log.event_id, [1, 2, 3]);
        assert_eq!(log.activity, [A, B, C]);
        assert_eq!(log.objects.len(), 2);

        // Failure path: null path -> typed status, 0 rows.
        let mut status2: u32 = u32::MAX;
        let n2 = unsafe { audit_ocel2_append(core::ptr::null(), events.as_ptr(), 3, &mut status2) };
        assert_eq!(n2, 0);
        assert_eq!(status2, Ocel2Status::NullArg.code());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Typed refusal table for [`validate_log`] (each malformed shape is
    /// caught, none panics).
    #[test]
    fn validate_log_refuses_malformed_shapes() {
        let mut log = sample_log();
        log.event_id[3] = log.event_id[2]; // monotonicity broken
        assert!(matches!(
            validate_log(&log),
            Err(Ocel2Error::MalformedLog("event ids not monotonic"))
        ));

        let mut log2 = sample_log();
        log2.activity[1] = 0; // reserved code
        assert!(matches!(
            validate_log(&log2),
            Err(Ocel2Error::MalformedLog("activity code 0 is reserved"))
        ));

        let mut log3 = sample_log();
        log3.object_ids.pop(); // segment sum mismatch
        assert!(matches!(
            validate_log(&log3),
            Err(Ocel2Error::MalformedLog("segment sum != object_ids len"))
        ));

        let mut log4 = sample_log();
        log4.qualifier.clear(); // column length mismatch
        assert!(matches!(
            validate_log(&log4),
            Err(Ocel2Error::MalformedLog("column length mismatch"))
        ));
    }

    /// Intern table court: idempotence, cap refusal (§8: typed error, never
    /// wraparound), wire round-trip.
    #[test]
    fn intern_table_court_idempotent_and_capped() {
        let mut t = Ocel2InternTable::new();
        let a = t.intern("approve").unwrap();
        let b = t.intern("approve").unwrap();
        assert_eq!(a, b, "interning is idempotent");
        assert_ne!(a, 0, "code 0 is reserved");
        assert_eq!(t.code_of("approve"), Some(a));
        assert_eq!(t.get(a), Some("approve"));

        let mut wire = Vec::new();
        t.encode(&mut wire);
        let mut t2 = Ocel2InternTable::new();
        let mut pos = 0usize;
        t2.decode(&wire, &mut pos).unwrap();
        assert_eq!(pos, wire.len());
        assert_eq!(t2.get(a), Some("approve"));

        // Code 0 in the wire form is refused.
        let mut bad: Vec<u8> = Vec::new();
        bad.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        bad.extend_from_slice(&0u16.to_le_bytes()); // code 0 — reserved
        bad.extend_from_slice(&1u16.to_le_bytes()); // len 1
        bad.push(b'x');
        let mut t3 = Ocel2InternTable::new();
        let mut pos3 = 0usize;
        assert!(matches!(t3.decode(&bad, &mut pos3), Err(Ocel2Error::InternCodeZero)));
    }

    /// Wave-4 court helper: rewrite ONLY the 64 B header of a real sealed
    /// slab through `f`, payload untouched (the header is not covered by
    /// the payload BLAKE3, so this is exactly the forgery surface the
    /// header-field checks must catch).
    fn rewrite_header(path: &Path, f: impl FnOnce(&mut Ocel2SlabHeader)) {
        let bytes = std::fs::read(path).unwrap();
        let mut header = decode_header(&bytes[..OCEL2_HEADER_BYTES]).unwrap();
        f(&mut header);
        let mut out = encode_header(&header).to_vec();
        out.extend_from_slice(&bytes[OCEL2_HEADER_BYTES..]);
        std::fs::write(path, &out).unwrap();
    }

    /// Wave-4 forged-header court (§6.1 `ocel-mutate` extension): row
    /// counts forged past [`OCEL2_MAX_DECODER_ROWS`] must be refused TYPED
    /// (`CapacityExceeded`) BEFORE allocation. Anti-vacuity: baseline code
    /// (`vec![0; count]`) attempts a 34 GB / 103 GB reservation here — the
    /// abort of this test process is the flip. Each restore is byte-exact
    /// and re-opens clean (real file state, shim-court discipline).
    #[test]
    fn forged_header_counts_refused_typed_and_restored() {
        let dir = court_dir("forge");
        let path = dir.join("slab.ocel2");
        let log = sample_log();
        seal_to_slab(&log, &path).unwrap();
        let original = std::fs::read(&path).unwrap();

        // 1. event budget forged to u32::MAX.
        rewrite_header(&path, |h| h.event_row_count = u32::MAX);
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::CapacityExceeded { what: "event_id", .. })
        ));
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        assert!(open_slab(&path).is_ok(), "re-open after restore");

        // 2. object budget forged to u32::MAX (same class, registry path —
        //    refused before the row loop, so no forged rows are read).
        rewrite_header(&path, |h| h.object_row_count = u32::MAX);
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::CapacityExceeded { what: "object rows", .. })
        ));
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        let reopened = open_slab(&path).unwrap();
        assert_logs_equal(&log, &reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave-4 intern-version court: a header claiming a foreign
    /// intern-table version must be refused BEFORE decode. Anti-vacuity:
    /// baseline never checked the field (it was decorative) — this test
    /// fails on baseline code. Restore byte-exact, re-open clean.
    #[test]
    fn intern_version_mismatch_refused_then_restored() {
        let dir = court_dir("internver");
        let path = dir.join("slab.ocel2");
        seal_to_slab(&sample_log(), &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        rewrite_header(&path, |h| h.intern_table_version = 99);
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::InternVersionMismatch {
                got: 99,
                expected: OCEL2_INTERN_TABLE_VERSION
            })
        ));
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        assert!(open_slab(&path).is_ok(), "re-open after restore");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave-4 trailing-bytes court: payload extended past the last object
    /// row must be refused even with an HONEST recomputed checksum — the
    /// trailing check specifically must fire, not the BLAKE3 gate.
    /// Anti-vacuity: baseline accepted trailing bytes silently (open
    /// succeeds), so this test fails on baseline code. Restore byte-exact,
    /// re-open column-equal.
    #[test]
    fn trailing_payload_bytes_refused_then_restored() {
        let dir = court_dir("trailing");
        let path = dir.join("slab.ocel2");
        let log = sample_log();
        seal_to_slab(&log, &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let header = decode_header(&original[..OCEL2_HEADER_BYTES]).unwrap();
        let mut forged_payload = original[OCEL2_HEADER_BYTES..].to_vec();
        forged_payload.push(0x42);
        let mut forged = encode_header(&Ocel2SlabHeader {
            checksum: blake3::hash(&forged_payload),
            ..header
        })
        .to_vec();
        forged.extend_from_slice(&forged_payload);
        std::fs::write(&path, &forged).unwrap();
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::TrailingBytes { got: 1 })
        ));
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        let reopened = open_slab(&path).unwrap();
        assert_logs_equal(&log, &reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave-7 defect repair D1 (§4.3 interning bijection): `decode` accepted
    /// wire forms whose codes were NOT strictly increasing — every per-entry
    /// check passed (non-zero, unique codes, unique strings, valid UTF-8) —
    /// and `next_code` is derived from the LAST entry's code, so a decoded
    /// table ending in code 1 restarted minting at 2, colliding with the
    /// existing code-2 entry. Court: such wire MUST be refused typed.
    /// Non-vacuity: on the pre-repair baseline this test FAILS (decode
    /// returned Ok and the minted collision was real).
    ///
    /// Wave-8 amendment (disclosed): the refusal is now the DENSE-ascending
    /// law (D4, strictly stronger — dense implies ascending), so the
    /// expected message changed from "intern codes not strictly increasing"
    /// to "intern codes not dense ascending". The [2, 1] fixture is refused
    /// under both laws; the flip is re-witnessed this session.
    #[test]
    fn intern_decode_refuses_nonmonotonic_codes() {
        // Per-entry-legal wire, codes [2, 1] — the collision shape.
        let mut wire: Vec<u8> = Vec::new();
        wire.extend_from_slice(&2u32.to_le_bytes()); // count = 2
        for (code, s) in [(2u16, "a"), (1u16, "b")] {
            wire.extend_from_slice(&code.to_le_bytes());
            wire.extend_from_slice(&(s.len() as u16).to_le_bytes());
            wire.extend_from_slice(s.as_bytes());
        }
        let mut t = Ocel2InternTable::new();
        let mut pos = 0usize;
        let err = t
            .decode(&wire, &mut pos)
            .expect_err("non-ascending intern codes must be refused");
        assert!(
            matches!(
                err,
                Ocel2Error::MalformedLog("intern codes not dense ascending")
            ),
            "expected the dense-ascending refusal, got {err:?}"
        );

        // Real restore: the honest dense-ascending wire (what `encode`
        // emits) still decodes and interns WITHOUT collision after it.
        let mut honest: Vec<u8> = Vec::new();
        honest.extend_from_slice(&2u32.to_le_bytes());
        for (code, s) in [(1u16, "a"), (2u16, "b")] {
            honest.extend_from_slice(&code.to_le_bytes());
            honest.extend_from_slice(&(s.len() as u16).to_le_bytes());
            honest.extend_from_slice(s.as_bytes());
        }
        let mut t2 = Ocel2InternTable::new();
        let mut pos2 = 0usize;
        t2.decode(&honest, &mut pos2).expect("honest wire decodes");
        assert_eq!(pos2, honest.len());
        assert_eq!(t2.code_of("a"), Some(1));
        assert_eq!(t2.code_of("b"), Some(2));
        let c = t2.intern("c").expect("mint after honest decode");
        assert_eq!(c, 3, "minting continues past the decoded codes");
        assert_ne!(t2.get(c), t2.get(1));
        assert_ne!(t2.get(c), t2.get(2));
    }

    /// Wave-7 defect repair D2 (§8: "overflow is a typed error, not a
    /// wraparound"): `intern` accepted strings longer than the u16 wire
    /// width, and `encode` stores `len u16 LE` per entry — a 70,000-byte
    /// string silently wrapped to 4,464 on the wire. Court: over-ceiling
    /// strings are refused typed at admission; the boundary value
    /// (65,535 B) is legal and round-trips exactly. Non-vacuity: on the
    /// pre-repair baseline this test FAILS (intern returned Ok and the
    /// encode wraparound was real).
    #[test]
    fn intern_string_length_refused_typed_boundary_value_round_trips() {
        // Boundary value: exactly the u16 wire ceiling is legal.
        let mut t = Ocel2InternTable::new();
        let max_str = "y".repeat(u16::MAX as usize);
        let code = t.intern(&max_str).expect("65,535 B string is legal");
        let mut wire = Vec::new();
        t.encode(&mut wire);
        let mut t2 = Ocel2InternTable::new();
        let mut pos = 0usize;
        t2.decode(&wire, &mut pos).expect("boundary wire decodes");
        assert_eq!(t2.get(code).map(str::len), Some(u16::MAX as usize));

        // Above the ceiling: typed refusal (was: silent wraparound — the
        // baseline flip is witnessed in the Wave-7 receipt: intern returned
        // Ok and encode wrapped 70,000 -> 4,464).
        let too_long = "z".repeat(u16::MAX as usize + 1);
        let err = t
            .intern(&too_long)
            .expect_err("over-ceiling string must be refused");
        assert!(
            matches!(err, Ocel2Error::InternStringTooLong { len: 65_536 }),
            "expected InternStringTooLong {{ len: 65536 }}, got {err:?}"
        );
        // Refused entry left the table unchanged (real state assert).
        assert_eq!(t.len(), 1);
        let again = t.intern(&max_str).expect("still idempotent after refusal");
        assert_eq!(again, code);
    }

    /// Wave-8 defect repair D4 (§4.3, D1 sibling): `decode` accepted
    /// SPARSE-but-ascending code tables. A decoded table whose highest code
    /// saturates `next_code` ([10, 65535] -> next_code = 65535, already
    /// taken) made the forward-only mint scan overflow to
    /// `InternTableFull` while 65,533 codes were free — a wrong typed
    /// answer of the same bijection-break class as D1. Repair: decode
    /// enforces the DENSE ascending law (codes 1..=n, exactly the wire
    /// form `encode` emits). Non-vacuity: on the wave-7 baseline this test
    /// FAILS — the sparse wires decoded Ok (witnessed in the wave receipt,
    /// sandbox run SB-A15-M-D4).
    #[test]
    fn intern_decode_refuses_sparse_codes() {
        // Sparse head: first code 2 (code 1 skipped).
        let mut head: Vec<u8> = Vec::new();
        head.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        head.extend_from_slice(&2u16.to_le_bytes());
        head.extend_from_slice(&1u16.to_le_bytes());
        head.extend_from_slice(b"a");
        let mut t = Ocel2InternTable::new();
        let mut pos = 0usize;
        assert!(
            matches!(
                t.decode(&head, &mut pos),
                Err(Ocel2Error::MalformedLog("intern codes not dense ascending"))
            ),
            "sparse head must be refused by the density law"
        );

        // Sparse gap: [1, 3].
        let mut gap: Vec<u8> = Vec::new();
        gap.extend_from_slice(&2u32.to_le_bytes()); // count = 2
        for (code, s) in [(1u16, "a"), (3u16, "c")] {
            gap.extend_from_slice(&code.to_le_bytes());
            gap.extend_from_slice(&(s.len() as u16).to_le_bytes());
            gap.extend_from_slice(s.as_bytes());
        }
        let mut t2 = Ocel2InternTable::new();
        let mut pos2 = 0usize;
        assert!(
            matches!(
                t2.decode(&gap, &mut pos2),
                Err(Ocel2Error::MalformedLog("intern codes not dense ascending"))
            ),
            "sparse gap must be refused by the density law"
        );

        // The saturation witness: sparse ascending ending at 65535. On the
        // wave-7 baseline this decoded Ok and the next intern returned
        // InternTableFull with 65,533 codes free; post-repair the wire is
        // refused at decode.
        let mut sat: Vec<u8> = Vec::new();
        sat.extend_from_slice(&2u32.to_le_bytes()); // count = 2
        for (code, s) in [(10u16, "a"), (65535u16, "b")] {
            sat.extend_from_slice(&code.to_le_bytes());
            sat.extend_from_slice(&(s.len() as u16).to_le_bytes());
            sat.extend_from_slice(s.as_bytes());
        }
        let mut t3 = Ocel2InternTable::new();
        let mut pos3 = 0usize;
        assert!(
            matches!(
                t3.decode(&sat, &mut pos3),
                Err(Ocel2Error::MalformedLog("intern codes not dense ascending"))
            ),
            "saturation shape must be refused by the density law"
        );

        // Honest control: the exact wire `encode` emits stays legal, and
        // minting continues densely after it.
        let mut t4 = Ocel2InternTable::new();
        let a = t4.intern("a").unwrap();
        let b = t4.intern("b").unwrap();
        let mut honest = Vec::new();
        t4.encode(&mut honest);
        let mut t5 = Ocel2InternTable::new();
        let mut pos5 = 0usize;
        t5.decode(&honest, &mut pos5).expect("dense wire decodes");
        assert_eq!(pos5, honest.len());
        assert_eq!(t5.get(a), Some("a"));
        assert_eq!(t5.get(b), Some("b"));
        let c = t5.intern("c").expect("mint after dense decode");
        assert_eq!(c, 3, "minting continues densely past the decoded codes");
    }

    /// Wave-8 defect repair D5 (§4.2 registry): `validate_log` accepted
    /// duplicate object rows — a registry key collision the append path
    /// cannot emit (`object_index` dedups) and that `open_slab`'s index
    /// build silently absorbed by last-wins overwrite. Repair: validate
    /// refuses the collision typed. Non-vacuity: on the wave-7 baseline
    /// this test FAILS (the duplicate validated Ok — witnessed in the wave
    /// receipt, sandbox run SB-A15-M-D5).
    #[test]
    fn validate_log_refuses_duplicate_object_rows() {
        let mut log = sample_log();
        let dup = log.objects[0].clone();
        log.objects.push(dup); // registry key collision
        assert!(
            matches!(
                validate_log(&log),
                Err(Ocel2Error::MalformedLog("duplicate object row"))
            ),
            "duplicate registry rows must be refused"
        );
        // Control: the append-produced registry (collision-free) validates.
        assert!(validate_log(&sample_log()).is_ok());
    }

    // -----------------------------------------------------------------------
    // Wave 9 — cross-lane integration courts. Each cites the owning lane's
    // spec and applies it to THIS lane's framing; each names its flip.
    // -----------------------------------------------------------------------

    /// Wave 9 — A04 identity court (A04 lane: the exit gate is a bit-for-bit
    /// identity assertion on every tier; a one-byte layout change fires it),
    /// applied to the slab tiers: a sealed slab's wire identity is a pure
    /// function of the log. Flip: any nondeterminism in seal (HashMap order,
    /// allocator addresses leaking into the wire) breaks the byte-exact
    /// equality; any decode/re-encode drift breaks the open→re-seal leg.
    #[test]
    fn a04_identity_court_seal_open_reseal_bit_exact() {
        let dir = court_dir("w9-identity");
        let pa = dir.join("a.ocel2");
        let pb = dir.join("b.ocel2");
        let pc = dir.join("c.ocel2");
        let log = sample_log();

        let seal_a = seal_to_slab(&log, &pa).unwrap();
        let bytes_a = std::fs::read(&pa).unwrap();

        // Tier 1: two independent seals of ONE log are byte-identical (the
        // wire form is a pure function of the log, not of the run).
        let seal_c = seal_to_slab(&log, &pc).unwrap();
        assert_eq!(seal_a, seal_c, "seal receipt must be a pure function of the log");
        assert_eq!(std::fs::read(&pc).unwrap(), bytes_a, "seal bytes must be identical");

        // Tier 2: open → re-seal is byte-exact (the decoded log re-encodes
        // to the same wire bytes — no round-trip drift, bit-for-bit).
        let reopened = open_slab(&pa).unwrap();
        let seal_b = seal_to_slab(&reopened, &pb).unwrap();
        assert_eq!(seal_a, seal_b, "open->re-seal receipt must be identical");
        assert_eq!(
            std::fs::read(&pb).unwrap(),
            bytes_a,
            "open->re-seal must be bit-for-bit identical"
        );
        assert_logs_equal(&log, &reopened);

        // Tier 3: the seal receipt's checksum is the on-disk payload's
        // identity — BLAKE3 re-derived from the file bytes equals the header
        // field (the receipt names the wire, not the in-memory log).
        let header = decode_header(&bytes_a[..OCEL2_HEADER_BYTES]).unwrap();
        assert_eq!(
            header.checksum,
            blake3::hash(&bytes_a[OCEL2_HEADER_BYTES..]),
            "header checksum must equal BLAKE3 over the on-disk payload"
        );
        assert_eq!(header.event_row_count, 10);
        assert_eq!(header.object_row_count, 3);

        // Anti-vacuity control: a DIFFERENT log must produce a DIFFERENT
        // wire identity (a court that cannot see a changed subject is
        // decoration). One ts tick changes payload bytes, the checksum, and
        // the file.
        let mut changed = sample_log();
        changed.ts_ns[0] += 1;
        let seal_changed = seal_to_slab(&changed, &pb).unwrap();
        assert_ne!(seal_changed.checksum, seal_a.checksum);
        assert_ne!(std::fs::read(&pb).unwrap(), bytes_a);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave 9 — A14 dam court (A14 §4: the ingress boundary classifies in a
    /// FIXED normative order, "first failure returns, never reordered",
    /// before any parser or allocator sees a byte), applied to the slab
    /// open path: doubly-violating slabs are refused by the EARLIER law,
    /// and a refusal performs strictly less allocation than a full decode
    /// (classify before decode, measured on the real CountingAlloc).
    ///
    /// Flip: reordering two refusal checks in `open_slab`, or decoding
    /// before classifying, flips the precedence asserts; removing an
    /// earlier gate hands its violation to the later law and flips the
    /// expected error. Non-vacuity controls: each single violation fires
    /// its own (later) law when the earlier laws pass.
    #[test]
    fn a14_dam_court_refusal_precedence_fixed_order() {
        let dir = court_dir("w9-dam");
        let path = dir.join("slab.ocel2");
        let log = sample_log();
        seal_to_slab(&log, &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut bytes = original.clone();

        // Row 0 — geometry beats magic: a 63-byte file with a wrong magic
        // byte 0 is refused TRUNCATED (the size dam reads nothing else —
        // there is no magic to compare before the file is header-sized).
        bytes[0] ^= 0xFF;
        bytes.truncate(OCEL2_HEADER_BYTES - 1);
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            matches!(open_slab(&path), Err(Ocel2Error::Truncated { .. })),
            "size dam must fire before magic"
        );

        // Row 1 — magic beats schema: wrong magic AND wrong schema =>
        // MagicMismatch (the alias-safety discriminator runs first).
        let mut bytes = original.clone();
        bytes[0] ^= 0xFF;
        bytes[10] = 0xFF;
        bytes[11] = 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            matches!(open_slab(&path), Err(Ocel2Error::MagicMismatch)),
            "magic dam must fire before the schema law"
        );

        // Row 2 — schema beats intern-version.
        let mut bytes = original.clone();
        bytes[10] = 0xFF;
        bytes[11] = 0xFF;
        bytes[12] = 0xFF;
        bytes[13] = 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(
            matches!(open_slab(&path), Err(Ocel2Error::SchemaMismatch { .. })),
            "schema dam must fire before the intern-version law"
        );

        // Row 3 — intern-version beats checksum: a foreign intern version
        // AND a corrupted payload byte => InternVersionMismatch (the
        // version dam precedes the payload hash).
        let mut bytes = original.clone();
        bytes[12] = 0xFF;
        bytes[13] = 0xFF;
        bytes[OCEL2_HEADER_BYTES + 5] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::InternVersionMismatch { .. })
        ));

        // Row 4 — checksum beats capacity: a corrupted payload AND a forged
        // u32::MAX event row count => ChecksumMismatch (the hash dam
        // precedes the decoder budgets — the forged budget is never even
        // consulted until the payload authenticates).
        let mut bytes = original.clone();
        bytes[OCEL2_HEADER_BYTES + 5] ^= 0xFF;
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::ChecksumMismatch { .. })
        ));

        // Single-violation controls (the later laws fire when the earlier
        // laws pass — the precedence table is total, not vacuous).
        let mut bytes = original.clone();
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::CapacityExceeded { what: "event_id", .. })
        ));
        let mut bytes = original.clone();
        bytes[OCEL2_HEADER_BYTES + 5] ^= 0xFF;
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            open_slab(&path),
            Err(Ocel2Error::ChecksumMismatch { .. })
        ));

        // Dam allocation law: a REFUSED open allocates strictly less than a
        // full DECODE open (classify before decode). Both measured on the
        // real global allocator.
        std::fs::write(&path, &original).unwrap();
        let (_, clean_report) = with_alloc_tracking(|| open_slab(&path));
        assert!(
            clean_report.count > 0,
            "control: a full open must allocate (decode buffers); court vacuous otherwise"
        );
        let refused_bytes = {
            let mut b = original.clone();
            b[0] ^= 0xFF;
            b
        };
        std::fs::write(&path, &refused_bytes).unwrap();
        let (dam_verdict, dam_report) = with_alloc_tracking(|| open_slab(&path));
        assert!(matches!(dam_verdict, Err(Ocel2Error::MagicMismatch)));
        assert!(
            dam_report.count < clean_report.count,
            "dam refusal must not allocate decode buffers (dam {dam_report:?} vs clean {clean_report:?})"
        );

        // Real restore, byte-exact, re-open clean (shim-court discipline).
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        assert_logs_equal(&log, &open_slab(&path).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave 9 — A05 power-cut sweep (A05: "a power cut at ANY byte offset
    /// `k in 0..N` ... must roll [back]"; "the court sweeps k in 0..=N";
    /// anti-vacuity: "a green sweep proves nothing unless the harness can
    /// fail"). Applied to the slab framing: every PROPER PREFIX of a real
    /// sealed slab (a cut at any byte offset) fails closed typed; every
    /// single-byte flip of the header or payload fails closed; the ONLY
    /// tolerated flips are the v1-documented ignore-on-read bytes
    /// (`magic_pad` offset 9, `reserved` offsets 24..32 — "zero on write,
    /// ignored on read (v1)"), pinned here as the documented tolerance, loud
    /// by exact count. The committed full file is data, not corruption — it
    /// opens and equals (A05's C4-vs-corruption distinction).
    ///
    /// Flip: an open that accepts torn bytes flips the every-prefix assert;
    /// a removed checksum gate shifts the exact tear-class counts; an
    /// undocumented tolerance flips the flip-sweep asserts.
    #[test]
    fn a05_power_cut_sweep_every_prefix_and_byte_fail_closed() {
        let dir = court_dir("w9-powercut");
        let path = dir.join("slab.ocel2");
        let log = sample_log();
        seal_to_slab(&log, &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let n = original.len();
        assert!(n > OCEL2_HEADER_BYTES + 64);

        // Sweep 1 — every prefix (cut at k in 0..n). Exact tear-class counts:
        // k in 0..64 is a header-length cut (Truncated); k >= 64 shortens
        // the authenticated payload (ChecksumMismatch).
        let mut truncated = 0u32;
        let mut checksum = 0u32;
        for k in 0..n {
            std::fs::write(&path, &original[..k]).unwrap();
            match open_slab(&path) {
                Ok(_) => panic!("power-cut sweep: torn prefix of {k} B opened Ok"),
                Err(Ocel2Error::Truncated { .. }) => truncated += 1,
                Err(Ocel2Error::ChecksumMismatch { .. }) => checksum += 1,
                Err(e) => panic!("power-cut sweep: unexpected tear class at k={k}: {e}"),
            }
        }
        assert_eq!(truncated, OCEL2_HEADER_BYTES as u32, "header-cut class count");
        assert_eq!(checksum, (n - OCEL2_HEADER_BYTES) as u32, "payload-cut class count");

        // Sweep 2 — every single header-byte flip. Exactly the 9 v1
        // ignore-on-read bytes tolerate a flip (documented); all other 55
        // header bytes must fail the open.
        let mut tolerated = 0u32;
        let mut refused = 0u32;
        for i in 0..OCEL2_HEADER_BYTES {
            let mut bytes = original.clone();
            bytes[i] ^= 0x01;
            std::fs::write(&path, &bytes).unwrap();
            if open_slab(&path).is_ok() {
                let ignored = i == 9 || (24..32).contains(&i);
                assert!(
                    ignored,
                    "header byte {i} flip accepted — undocumented tolerance"
                );
                tolerated += 1;
            } else {
                refused += 1;
            }
        }
        assert_eq!(refused, 55, "load-bearing header bytes must all fail closed");
        assert_eq!(tolerated, 9, "exactly the documented v1 ignore-on-read bytes");

        // Sweep 3 — every single payload-byte flip is authenticated out.
        let mut flipped = 0u32;
        for i in OCEL2_HEADER_BYTES..n {
            let mut bytes = original.clone();
            bytes[i] ^= 0x01;
            std::fs::write(&path, &bytes).expect("write torn slab");
            assert!(
                open_slab(&path).is_err(),
                "payload byte {i} flip must be refused"
            );
            flipped += 1;
        }
        assert_eq!(flipped as usize, n - OCEL2_HEADER_BYTES);

        // The committed full file opens and equals (cut-vs-commit law).
        std::fs::write(&path, &original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original, "restore byte-exact");
        assert_logs_equal(&log, &open_slab(&path).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Wave 9 — A20 stress at the A15 surface: 1,000,000 admitted
    /// transitions through the REAL append path, sealed to a REAL file,
    /// re-opened, and column-equal over ALL rows. Measured, not targeted:
    /// wall-clock per phase is printed (method: dev profile, this host,
    /// single run) — the numbers are observations, never a spec claim.
    ///
    /// Flip: any dropped/mutated row on the seal/open path breaks the full
    /// column equality; the row pin and checksum gate (courts above) are
    /// the refusal points this volume exercises for real.
    #[test]
    fn a20_stress_one_million_rows_append_seal_reopen() {
        const ROWS: u64 = 1_000_000;
        const OBJECTS: u64 = 1_000;
        let dir = court_dir("w9-stress");
        let path = dir.join("stress.ocel2");
        let mut log = Ocel2ColumnarLog::with_capacity(ROWS as usize, ROWS as usize, OBJECTS as usize);

        let t0 = std::time::Instant::now();
        for i in 0..ROWS {
            // Binding b = 1000 + ((i / 2) % 1000): each object's events are
            // (A, B) pairs in slab order — the POWL stress court (powl.rs)
            // verifies Sequence over the same construction.
            let activity = if i % 2 == 0 { A } else { B };
            let object = 1000 + ((i / 2) % OBJECTS);
            log.append(&vec_for(i + 1, 1_700_000_000_000 + i, activity, object))
                .unwrap();
        }
        let append_s = t0.elapsed().as_secs_f64();

        assert_eq!(log.event_count(), ROWS as usize);
        assert_eq!(log.objects.len(), OBJECTS as usize);
        // Every binding saw 500 (A,B) pairs: first/last event indices pinned.
        let first = &log.objects[0];
        let last = &log.objects[OBJECTS as usize - 1];
        assert_eq!(first.first_event_idx, 0);
        assert_eq!(last.last_event_idx, (ROWS - 1) as u32);

        let t1 = std::time::Instant::now();
        let seal = seal_to_slab(&log, &path).unwrap();
        let seal_s = t1.elapsed().as_secs_f64();
        assert_eq!(seal.event_row_count, ROWS as u32);
        assert_eq!(seal.object_row_count, OBJECTS as u32);

        let t2 = std::time::Instant::now();
        let reopened = open_slab(&path).unwrap();
        let open_s = t2.elapsed().as_secs_f64();
        assert_eq!(reopened.event_count(), ROWS as usize);
        assert_logs_equal(&log, &reopened);

        // Spot pins beyond the full-column equality: first, middle, last.
        assert_eq!(reopened.event_id[0], 1);
        assert_eq!(reopened.event_id[ROWS as usize / 2], (ROWS / 2 + 1));
        assert_eq!(reopened.event_id[ROWS as usize - 1], ROWS);
        assert_eq!(reopened.activity[0], A);
        assert_eq!(reopened.activity[ROWS as usize - 1], B);

        println!(
            "a20-1m-stress(ocel2): rows=1000000 objects=1000 \
             append={append_s:.3}s seal={seal_s:.3}s open={open_s:.3}s \
             slab_bytes={} (dev profile, single run, this host — measured, not targeted)",
            seal.bytes_written
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
