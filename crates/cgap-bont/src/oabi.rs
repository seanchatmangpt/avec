//! A06 — `.oabi` pack format v1: container layout + loader for the B-ONT
//! scalar primitives (Wave 1 ABI freeze + Wave 3 loader and refusal court).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A06-bont-scalars.md` §6 (wire format), §7
//! (layout asserts), §8 T2/T3 (courts), §11 (freeze list). Wave 1 locked the
//! struct/const definitions; this slice lands the §6 loader obligations (§6
//! "Loader checks") and the §8 courts T2 (byte-level size court) and T3
//! (pack-loader refusal court) as inline tests.
//!
//! Section 6 header offset table (64 B total):
//!
//! ```text
//! offset  size  field
//! 0       8     magic          = b"OABIv1\0\0"
//! 8       4     header_len     (u32 LE) = 64
//! 12      2     version_major  = 1
//! 14      2     version minor  = 0
//! 16      4     flags          (bit0: little-endian; bit1: LexHash present)
//! 20      4     class_count    (u32 LE)  — ClassId space is [0, class_count)
//! 24      4     enum_count     (u32 LE)  — EnumId space is [0, enum_count)
//! 28      4     slot_count     (u32 LE)  — SlotId records follow
//! 32      4     record_size    (u32 LE) = 48
//! 36      28    reserved       (zero; must be zero to load)
//! ```
//!
//! Records: `slot_count` x 48 B each, 8-byte aligned, immediately after the
//! header (offset 64). Fixed 48-byte records, no varints in v1; padding is
//! explicit so the `#[repr(C)]` layout IS the on-disk layout byte-for-byte.
//! The loader is a safe field-wise LE parse into owned `Vec<OabiRecordV1>` —
//! an arbitrary `&[u8]` (e.g. a `Vec<u8>`) has no 8-byte alignment guarantee,
//! so zero-copy reinterpretation stays the documented consumer right for
//! mmap'd, aligned regions (§6) and is NOT how the loader reads untrusted
//! bytes.
//!
//! Loader checks beyond the §6 list (each typed, court-tested by a named T3
//! row whose witnessed flip is the §8 anti-vacuity law): record
//! `class`/`enum_` in `[0, class_count)`/`[0, enum_count)` per the §6 header
//! field meanings (T3 rows class-range / enum-range), `version_major == 1`
//! with additive minor-version policy, LE flag bit0 set, unknown flag bits
//! zero. The length law is exact equality
//! (`len == 64 + slot_count*48`); a short header, a slot_count that
//! overstates the record section, an overlong buffer, and a hostile
//! `slot_count` whose byte-count overflows `usize` (32-bit targets) are all
//! typed `BadLength` refusals (T3 rows length-*), never arithmetic panics
//! (A03-wave5 audit class F-1: hostile counts must be refused typed, not
//! overflowed). Accepting any mutated buffer is FAIL (§8 T3).
//!
//! Wave 8 defect repair (per `waves/a08-wave7-audit-receipt.md`): zero
//! defects in the loader — baseline 30/30 exit 0 re-run and every T3 row's
//! check re-flipped fresh in the wave-8 sandbox /tmp/a06-w8.k8yZKp (11/11
//! loader mutations incl. M-L exact-length-law relaxation and the M-T2
//! `ClassId` u32->u64 widening, build-refused by the ABI-freeze assert),
//! sha256 fail-closed restores.

use crate::primitives::{truncate64, ClassId, EnumId, LexHash, SlotId};

/// Pack magic, header bytes 0..8 (§6).
pub const OABI_MAGIC: [u8; 8] = *b"OABIv1\0\0";

/// Frozen header length in bytes (§6, §11).
pub const OABI_HEADER_LEN: usize = 64;

/// Frozen record size in bytes:
/// SlotId(8) | ClassId(4) | EnumId(2) | pad(2) | LexHash(32) (§6).
pub const OABI_RECORD_SIZE: usize = 48;

/// Format major version (§6).
pub const OABI_VERSION_MAJOR: u16 = 1;

/// Format minor version (§6).
pub const OABI_VERSION_MINOR: u16 = 0;

/// Flags bit0: little-endian pack (§6). v1 packs are little-endian only; the
/// bit is reserved now to avoid a v2 header break (§10).
pub const OABI_FLAG_LITTLE_ENDIAN: u32 = 1 << 0;

/// Flags bit1: LexHash present in every record (§6).
pub const OABI_FLAG_LEXHASH_PRESENT: u32 = 1 << 1;

/// `.oabi` v1 header, bytes 0..64 of the pack (§6 offset table above).
///
/// Field order and widths are the frozen §6 table; `reserved` must be zero
/// to load (loader obligation, this slice).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OabiHeaderV1 {
    pub magic: [u8; 8],
    pub header_len: u32,
    pub version_major: u16,
    pub version_minor: u16,
    pub flags: u32,
    pub class_count: u32,
    pub enum_count: u32,
    pub slot_count: u32,
    pub record_size: u32,
    pub reserved: [u8; 28],
}

/// One slot record, 48 B, 8-byte aligned; the in-memory layout IS the
/// on-disk layout byte-for-byte (§6).
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OabiRecordV1 {
    pub slot: SlotId,
    pub class: ClassId,
    pub enum_: EnumId,
    pub _pad: [u8; 2],
    pub lex: LexHash,
}

// ---- Layout freeze: record == pack record, header == pack header (A06 §7) ----
const _: () = assert!(
    core::mem::size_of::<OabiRecordV1>() == OABI_RECORD_SIZE,
    "ABI freeze (A06 §7): OabiRecordV1 must stay 48 bytes",
);
const _: () = assert!(
    core::mem::align_of::<OabiRecordV1>() == 8,
    "ABI freeze (A06 §7): OabiRecordV1 must stay 8-byte aligned",
);
// Frozen field offsets, §6 record table: SlotId(8) | ClassId(4) | EnumId(2)
// | pad(2) | LexHash(32). A reorder or padding change must fail the build.
const _: () = assert!(
    core::mem::offset_of!(OabiRecordV1, slot) == 0,
    "ABI freeze (A06 §6): OabiRecordV1.slot must stay at offset 0",
);
const _: () = assert!(
    core::mem::offset_of!(OabiRecordV1, class) == 8,
    "ABI freeze (A06 §6): OabiRecordV1.class must stay at offset 8",
);
const _: () = assert!(
    core::mem::offset_of!(OabiRecordV1, enum_) == 12,
    "ABI freeze (A06 §6): OabiRecordV1.enum_ must stay at offset 12",
);
const _: () = assert!(
    core::mem::offset_of!(OabiRecordV1, _pad) == 14,
    "ABI freeze (A06 §6): OabiRecordV1._pad must stay at offset 14",
);
const _: () = assert!(
    core::mem::offset_of!(OabiRecordV1, lex) == 16,
    "ABI freeze (A06 §6): OabiRecordV1.lex must stay at offset 16",
);
const _: () = assert!(
    core::mem::size_of::<OabiHeaderV1>() == OABI_HEADER_LEN,
    "ABI freeze (A06 §7): OabiHeaderV1 must stay 64 bytes",
);
const _: () = assert!(
    core::mem::align_of::<OabiHeaderV1>() == 4,
    "ABI freeze (A06 §7): OabiHeaderV1 must keep its natural 4-byte alignment",
);

/// Typed loader refusals, each naming the offending field (§6, §8 T3:
/// "a loader that accepts any mutated buffer is FAIL").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OabiError {
    /// Byte length shorter than the header or not exactly
    /// `OABI_HEADER_LEN + slot_count * OABI_RECORD_SIZE` (fires when
    /// `slot_count` overstates the record section and when trailing bytes
    /// overstate it; `needed: usize::MAX` is the sentinel for a hostile
    /// `slot_count` whose record-section byte count overflows `usize` on
    /// narrow targets — typed refusal, never an arithmetic panic).
    BadLength { needed: usize, got: usize },
    /// Header bytes 0..8 != `OABI_MAGIC`.
    BadMagic,
    /// `header_len != 64`.
    HeaderLen { got: u32 },
    /// `version_major != 1` (minor is additive/forward-accepted).
    UnsupportedVersion { got_major: u16 },
    /// Flags bit0 clear — v1 packs are little-endian only (§10).
    NotLittleEndian,
    /// Unknown flag bits set (bits 2..32 are reserved).
    UnknownFlags { got: u32 },
    /// `record_size != 48`.
    RecordSize { got: u32 },
    /// Nonzero byte in `reserved` (naming the first offending offset).
    ReservedNonZero { at_offset: usize },
    /// Per-record §6 obligation: `SlotId != truncate64(LexHash)` for the
    /// record at `index`.
    SlotDerivationMismatch {
        index: usize,
        record_slot: u64,
        derived_slot: u64,
    },
    /// Record class outside `[0, class_count)` (§6 header field meaning).
    ClassOutOfRange { index: usize, class: u32, class_count: u32 },
    /// Record enum outside `[0, enum_count)` (§6 header field meaning).
    EnumOutOfRange { index: usize, enum_: u16, enum_count: u32 },
}

impl core::fmt::Display for OabiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OabiError::BadLength { needed, got } => {
                write!(f, ".oabi: bad length: needed {needed}, got {got}")
            }
            OabiError::BadMagic => write!(f, ".oabi: bad magic"),
            OabiError::HeaderLen { got } => write!(f, ".oabi: header_len {got} != 64"),
            OabiError::UnsupportedVersion { got_major } => {
                write!(f, ".oabi: version_major {got_major} != 1")
            }
            OabiError::NotLittleEndian => write!(f, ".oabi: LE flag (bit0) not set"),
            OabiError::UnknownFlags { got } => write!(f, ".oabi: unknown flag bits set: 0x{got:08x}"),
            OabiError::RecordSize { got } => write!(f, ".oabi: record_size {got} != 48"),
            OabiError::ReservedNonZero { at_offset } => {
                write!(f, ".oabi: reserved byte at offset {at_offset} is nonzero")
            }
            OabiError::SlotDerivationMismatch {
                index,
                record_slot,
                derived_slot,
            } => write!(
                f,
                ".oabi: record {index}: SlotId 0x{record_slot:016x} != truncate64(LexHash) 0x{derived_slot:016x}",
            ),
            OabiError::ClassOutOfRange { index, class, class_count } => {
                write!(f, ".oabi: record {index}: class {class} >= class_count {class_count}")
            }
            OabiError::EnumOutOfRange { index, enum_, enum_count } => {
                write!(f, ".oabi: record {index}: enum {enum_} >= enum_count {enum_count}")
            }
        }
    }
}

impl OabiHeaderV1 {
    /// The §6 wire header for a pack of `slot_count` records with counts
    /// covering the record classes/enums (court tooling + A09/A10 baseline).
    pub fn new(class_count: u32, enum_count: u32, slot_count: u32) -> Self {
        Self {
            magic: OABI_MAGIC,
            header_len: OABI_HEADER_LEN as u32,
            version_major: OABI_VERSION_MAJOR,
            version_minor: OABI_VERSION_MINOR,
            flags: OABI_FLAG_LITTLE_ENDIAN | OABI_FLAG_LEXHASH_PRESENT,
            class_count,
            enum_count,
            slot_count,
            record_size: OABI_RECORD_SIZE as u32,
            reserved: [0; 28],
        }
    }

    /// Field-wise LE wire encoding of the header (64 bytes).
    pub fn to_bytes(&self) -> [u8; OABI_HEADER_LEN] {
        let mut b = [0u8; OABI_HEADER_LEN];
        b[0..8].copy_from_slice(&self.magic);
        b[8..12].copy_from_slice(&self.header_len.to_le_bytes());
        b[12..14].copy_from_slice(&self.version_major.to_le_bytes());
        b[14..16].copy_from_slice(&self.version_minor.to_le_bytes());
        b[16..20].copy_from_slice(&self.flags.to_le_bytes());
        b[20..24].copy_from_slice(&class_count_field(self));
        b[24..28].copy_from_slice(&self.enum_count.to_le_bytes());
        b[28..32].copy_from_slice(&self.slot_count.to_le_bytes());
        b[32..36].copy_from_slice(&self.record_size.to_le_bytes());
        b[36..64].copy_from_slice(&self.reserved);
        b
    }
}

fn class_count_field(h: &OabiHeaderV1) -> [u8; 4] {
    h.class_count.to_le_bytes()
}

impl OabiRecordV1 {
    /// Field-wise LE wire encoding of one 48-byte record.
    pub fn to_bytes(&self) -> [u8; OABI_RECORD_SIZE] {
        let mut b = [0u8; OABI_RECORD_SIZE];
        b[0..8].copy_from_slice(&self.slot.0.to_le_bytes());
        b[8..12].copy_from_slice(&self.class.0.to_le_bytes());
        b[12..14].copy_from_slice(&self.enum_.0.to_le_bytes());
        b[14..16].copy_from_slice(&self._pad);
        b[16..48].copy_from_slice(&self.lex.0);
        b
    }

    /// §6 loader obligation: re-derive the slot from the record's LexHash and
    /// compare against the stored SlotId.
    pub fn verify_slot_derivation(&self) -> Result<(), (u64, u64)> {
        let derived = truncate64(&self.lex).0;
        if derived != self.slot.0 {
            return Err((self.slot.0, derived));
        }
        Ok(())
    }
}

// ---- §6 loader ----------------------------------------------------------------

/// A parsed `.oabi` v1 pack: validated header + owned record copy.
///
/// Parse is a safe field-wise LE read (§6 loader checks). An arbitrary
/// caller byte buffer has no 8-byte alignment guarantee, so zero-copy
/// reinterpretation stays the documented consumer right for mmap'd,
/// aligned regions (§6) and is deliberately NOT how the loader reads
/// untrusted bytes.
#[derive(Clone, Debug)]
pub struct OabiPackV1 {
    header: OabiHeaderV1,
    records: Vec<OabiRecordV1>,
}

impl OabiPackV1 {
    /// Parse and validate a `.oabi` v1 pack from raw bytes.
    ///
    /// Applies the §6 loader checks (magic, header_len == 64, record_size ==
    /// 48, slot_count consistent with section length, per-record
    /// `SlotId == truncate64(LexHash)`, reserved zero) plus the additive
    /// checks documented at [`OabiError`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, OabiError> {
        if bytes.len() < OABI_HEADER_LEN {
            return Err(OabiError::BadLength { needed: OABI_HEADER_LEN, got: bytes.len() });
        }
        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);
        if magic != OABI_MAGIC {
            return Err(OabiError::BadMagic);
        }
        let u32le = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().expect("in-range header offsets"));
        let u16le = |o: usize| u16::from_le_bytes(bytes[o..o + 2].try_into().expect("in-range header offsets"));
        let header_len = u32le(8);
        if header_len != OABI_HEADER_LEN as u32 {
            return Err(OabiError::HeaderLen { got: header_len });
        }
        let version_major = u16le(12);
        if version_major != OABI_VERSION_MAJOR {
            return Err(OabiError::UnsupportedVersion { got_major: version_major });
        }
        let flags = u32le(16);
        if flags & OABI_FLAG_LITTLE_ENDIAN == 0 {
            return Err(OabiError::NotLittleEndian);
        }
        const KNOWN_FLAGS: u32 = OABI_FLAG_LITTLE_ENDIAN | OABI_FLAG_LEXHASH_PRESENT;
        if flags & !KNOWN_FLAGS != 0 {
            return Err(OabiError::UnknownFlags { got: flags });
        }
        let header = OabiHeaderV1 {
            magic,
            header_len,
            version_major,
            version_minor: u16le(14),
            flags,
            class_count: u32le(20),
            enum_count: u32le(24),
            slot_count: u32le(28),
            record_size: u32le(32),
            reserved: {
                let mut r = [0u8; 28];
                r.copy_from_slice(&bytes[36..64]);
                r
            },
        };
        if let Some(off) = header.reserved.iter().position(|&b| b != 0) {
            return Err(OabiError::ReservedNonZero { at_offset: 36 + off });
        }
        if header.record_size != OABI_RECORD_SIZE as u32 {
            return Err(OabiError::RecordSize { got: header.record_size });
        }
        // Exact-equality length law (§6 "slot_count consistent with section
        // length"), computed in checked arithmetic: a hostile `slot_count`
        // whose record-section byte count overflows `usize` (narrow targets,
        // slot_count near u32::MAX) is a typed `BadLength` refusal with the
        // documented `usize::MAX` sentinel — never a debug overflow panic
        // (A03-wave5 audit class F-1: hostile counts are refused typed).
        let needed = (header.slot_count as usize)
            .checked_mul(OABI_RECORD_SIZE)
            .and_then(|records_bytes| records_bytes.checked_add(OABI_HEADER_LEN))
            .ok_or(OabiError::BadLength { needed: usize::MAX, got: bytes.len() })?;
        if bytes.len() != needed {
            return Err(OabiError::BadLength { needed, got: bytes.len() });
        }
        let mut records = Vec::with_capacity(header.slot_count as usize);
        for i in 0..header.slot_count as usize {
            let base = OABI_HEADER_LEN + i * OABI_RECORD_SIZE;
            let rec = OabiRecordV1 {
                slot: SlotId(u64::from_le_bytes(bytes[base..base + 8].try_into().expect("record window"))),
                class: ClassId(u32::from_le_bytes(bytes[base + 8..base + 12].try_into().expect("record window"))),
                enum_: EnumId(u16::from_le_bytes(bytes[base + 12..base + 14].try_into().expect("record window"))),
                _pad: [bytes[base + 14], bytes[base + 15]],
                lex: LexHash(bytes[base + 16..base + 48].try_into().expect("record window")),
            };
            if rec.class.0 >= header.class_count {
                return Err(OabiError::ClassOutOfRange {
                    index: i,
                    class: rec.class.0,
                    class_count: header.class_count,
                });
            }
            if rec.enum_.0 as u32 >= header.enum_count {
                return Err(OabiError::EnumOutOfRange {
                    index: i,
                    enum_: rec.enum_.0,
                    enum_count: header.enum_count,
                });
            }
            // §6 per-record obligation (defense in depth against producer
            // truncation-window drift — §10): re-derive from the LexHash.
            if let Err((record_slot, derived_slot)) = rec.verify_slot_derivation() {
                return Err(OabiError::SlotDerivationMismatch { index: i, record_slot, derived_slot });
            }
            records.push(rec);
        }
        Ok(Self { header, records })
    }

    /// Validated header.
    pub fn header(&self) -> &OabiHeaderV1 {
        &self.header
    }

    /// Owned copy of the validated records.
    pub fn records(&self) -> &[OabiRecordV1] {
        &self.records
    }

    /// Serialize header + records to canonical wire bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(OABI_HEADER_LEN + self.records.len() * OABI_RECORD_SIZE);
        out.extend_from_slice(&self.header.to_bytes());
        for r in &self.records {
            out.extend_from_slice(&r.to_bytes());
        }
        out
    }
}

// ---- §8 courts T2/T3 (real buffers, real bytes, no mocks) --------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{lex_hash, slot_id};

    /// The §8 T2 golden record: field values chosen to exercise every field,
    /// with a LexHash independent of any real lexeme.
    fn golden_record() -> OabiRecordV1 {
        OabiRecordV1 {
            slot: SlotId(0x0123_4567_89AB_CDEF),
            class: ClassId(7),
            enum_: EnumId(9),
            _pad: [0xEE, 0xDD],
            lex: LexHash(core::array::from_fn(|j| (j as u8) ^ 0xA5)),
        }
    }

    /// Golden 48-byte blob, hand-composed LE bytes (committed; generated
    /// once in-session via the hand path: slot LE | class LE | enum LE |
    /// pad | lex — NOT via the struct).
    const GOLDEN_BLOB_HEX: &str =
        "efcdab8967452301070000000900eedda5a4a7a6a1a0a3a2adacafaea9a8abaab5b4b7b6b1b0b3b2bdbcbfbeb9b8bbba";

    fn golden_blob() -> [u8; 48] {
        let mut b = [0u8; 48];
        for (j, hex_pair) in GOLDEN_BLOB_HEX.as_bytes().chunks(2).enumerate() {
            b[j] = u8::from_str_radix(core::str::from_utf8(hex_pair).unwrap(), 16).unwrap();
        }
        b
    }

    /// T2 — byte-level size court (§8 T2): the struct's raw memory, read via
    /// `slice::from_raw_parts` over the #[repr(C)] value (doc-blessed), must
    /// equal the committed hand-composed golden blob. Any field widening,
    /// reorder, or padding change changes the memory bytes -> court fails.
    #[test]
    fn t2_struct_memory_matches_golden_blob() {
        let rec = golden_record();
        let addr = core::ptr::addr_of!(rec) as *const u8;
        let mem = unsafe { core::slice::from_raw_parts(addr, 48) };
        assert_eq!(mem, &golden_blob(), "OabiRecordV1 memory layout drifted from the golden blob");
    }

    /// T2b — wire round-trip: a derivation-valid record (slot re-derivable
    /// from its LexHash) survives field-wise LE encode -> loader decode
    /// byte-exactly, header included.
    #[test]
    fn t2b_wire_round_trip() {
        let lex = b"bont://court/roundtrip";
        let rec = OabiRecordV1 {
            slot: slot_id(lex),
            class: ClassId(7),
            enum_: EnumId(9),
            _pad: [0xEE, 0xDD],
            lex: lex_hash(lex),
        };
        let header = OabiHeaderV1::new(8, 10, 1);
        let pack = OabiPackV1 { header, records: vec![rec] };
        let bytes = pack.to_bytes();
        assert_eq!(bytes.len(), OABI_HEADER_LEN + 48);
        let parsed = OabiPackV1::from_bytes(&bytes).expect("round-trip must load");
        assert_eq!(parsed.header(), &header);
        assert_eq!(parsed.records(), &[rec]);
    }

    /// Builds a valid 2-record pack for mutation testing: real lexemes,
    /// real slots (derived via the crate path).
    fn valid_pack() -> Vec<u8> {
        let lex_a = b"bont://court/alpha";
        let lex_b = b"bont://court/beta";
        let recs = [&lex_a[..], &lex_b[..]].map(|lex| OabiRecordV1 {
            slot: slot_id(lex),
            class: ClassId(0),
            enum_: EnumId(1),
            _pad: [0, 0],
            lex: lex_hash(lex),
        });
        OabiPackV1 {
            header: OabiHeaderV1::new(1, 2, 2),
            records: recs.to_vec(),
        }
        .to_bytes()
    }

    /// Positive control: the unmutated pack loads (guards a loader that
    /// refuses everything — the refusal court's own vacuity guard).
    #[test]
    fn t3_positive_control_unmutated_pack_loads() {
        let bytes = valid_pack();
        let pack = OabiPackV1::from_bytes(&bytes).expect("valid pack must load");
        assert_eq!(pack.records().len(), 2);
        assert_eq!(pack.records()[0].slot, slot_id(b"bont://court/alpha"));
        assert_eq!(pack.header().slot_count, 2);
    }

    /// T3 — pack-loader refusal court (§8 T3): five doc cases plus additive
    /// typed checks. Each mutation is applied to a REAL pack buffer; the
    /// loader must refuse with the NAMED typed error and that error only.
    #[test]
    fn t3_loader_refusal_court() {
        // (a) bad magic
        let mut bytes = valid_pack();
        bytes[3] ^= 0xFF;
        assert_eq!(OabiPackV1::from_bytes(&bytes).unwrap_err(), OabiError::BadMagic);

        // (b) header_len != 64
        let mut bytes = valid_pack();
        bytes[8..12].copy_from_slice(&32u32.to_le_bytes());
        assert_eq!(OabiPackV1::from_bytes(&bytes).unwrap_err(), OabiError::HeaderLen { got: 32 });

        // (c) record_size != 48
        let mut bytes = valid_pack();
        bytes[32..36].copy_from_slice(&24u32.to_le_bytes());
        assert_eq!(OabiPackV1::from_bytes(&bytes).unwrap_err(), OabiError::RecordSize { got: 24 });

        // (d) one record's SlotId != truncate64(LexHash): flip a slot byte.
        let mut bytes = valid_pack();
        bytes[OABI_HEADER_LEN] ^= 0x01;
        let bad_slot = u64::from_le_bytes(bytes[OABI_HEADER_LEN..OABI_HEADER_LEN + 8].try_into().unwrap());
        let good_slot = slot_id(b"bont://court/alpha").0;
        assert_ne!(bad_slot, good_slot);
        assert_eq!(
            OabiPackV1::from_bytes(&bytes).unwrap_err(),
            OabiError::SlotDerivationMismatch {
                index: 0,
                record_slot: bad_slot,
                derived_slot: good_slot,
            }
        );

        // (e) nonzero reserved byte
        let mut bytes = valid_pack();
        bytes[40] = 0x7F;
        assert_eq!(OabiPackV1::from_bytes(&bytes).unwrap_err(), OabiError::ReservedNonZero { at_offset: 40 });

        // Additive checks (documented at OabiError): version_major bump
        let mut bytes = valid_pack();
        bytes[12..14].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(OabiPackV1::from_bytes(&bytes).unwrap_err(), OabiError::UnsupportedVersion { got_major: 2 });

        // Additive: LE flag cleared
        let mut bytes = valid_pack();
        bytes[16] &= !0x01;
        let le_masked = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        assert_eq!(le_masked & 0x1, 0);
        assert_eq!(OabiPackV1::from_bytes(&bytes).unwrap_err(), OabiError::NotLittleEndian);

        // Additive: unknown flag bit set
        let mut bytes = valid_pack();
        bytes[19] = 0x80; // flags byte3 MSB -> bit31
        let flags_value = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        assert_ne!(flags_value & 0x8000_0000, 0);
        assert_eq!(
            OabiPackV1::from_bytes(&bytes).unwrap_err(),
            OabiError::UnknownFlags { got: flags_value }
        );

        // (f) length: truncated header (32 < 64 bytes)
        let bytes = valid_pack();
        let short = &bytes[..32];
        assert_eq!(
            OabiPackV1::from_bytes(short).unwrap_err(),
            OabiError::BadLength { needed: OABI_HEADER_LEN, got: 32 }
        );

        // (g) length: slot_count overstates the record section (the header
        // claims 3 records; the buffer carries exactly 2)
        let mut bytes = valid_pack();
        bytes[28..32].copy_from_slice(&3u32.to_le_bytes());
        let needed3 = OABI_HEADER_LEN + 3 * OABI_RECORD_SIZE;
        assert_eq!(
            OabiPackV1::from_bytes(&bytes).unwrap_err(),
            OabiError::BadLength { needed: needed3, got: bytes.len() }
        );

        // (g2) length: buffer overstates the record section (one trailing
        // byte behind slot_count records — the exact-equality law refuses
        // it; a `bytes.len() < needed` relaxation is the M-L mutation).
        let mut bytes = valid_pack();
        bytes.push(0xAA);
        let needed2 = OABI_HEADER_LEN + 2 * OABI_RECORD_SIZE;
        assert_eq!(
            OabiPackV1::from_bytes(&bytes).unwrap_err(),
            OabiError::BadLength { needed: needed2, got: bytes.len() }
        );

        // (h) hostile slot_count = u32::MAX on a 2-record buffer: on a
        // target where slot_count * 48 overflows `usize` this is the
        // checked-arithmetic overflow sentinel; on 64-bit the byte-count is
        // 64 + 4294967295*48 = 206158430240 and the exact-length law refuses
        // it. Either way the refusal is typed BadLength, never a panic.
        let mut bytes = valid_pack();
        bytes[28..32].copy_from_slice(&u32::MAX.to_le_bytes());
        let got = bytes.len();
        match OabiPackV1::from_bytes(&bytes).unwrap_err() {
            OabiError::BadLength { got: g, .. } => assert_eq!(g, got),
            other => panic!("hostile slot_count must refuse as BadLength, got {other:?}"),
        }

        // (i) class range: record 0 class 7 >= class_count 1
        let mut bytes = valid_pack();
        bytes[OABI_HEADER_LEN + 8..OABI_HEADER_LEN + 12].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(
            OabiPackV1::from_bytes(&bytes).unwrap_err(),
            OabiError::ClassOutOfRange { index: 0, class: 7, class_count: 1 }
        );

        // (j) enum range: record 0 enum 5 >= enum_count 2
        let mut bytes = valid_pack();
        bytes[OABI_HEADER_LEN + 12..OABI_HEADER_LEN + 14].copy_from_slice(&5u16.to_le_bytes());
        assert_eq!(
            OabiPackV1::from_bytes(&bytes).unwrap_err(),
            OabiError::EnumOutOfRange { index: 0, enum_: 5, enum_count: 2 }
        );
    }

    // ---- Wave 9 cross-lane integration courts (failed-lane restart, 2026-11-01)
    //
    // Wave-9 slice: A06's side of the cross-lane integration wave — the two
    // committed producer packs (A09 `fibo_sec_dbt_v1.oabi`, A10
    // `tosca_cimi_v1.oabi`) must load through the REAL §6 loader under the
    // FULL law (magic / header_len / version / flags / reserved / record_size /
    // exact length / per-record class+enum range / per-record
    // `slot == truncate64(lex)`), every stored slot re-derived through the §4
    // `truncate64` window on producer bytes, and the producer corpus
    // collision-free (§5) on the REAL stored slots. Corpus membership stays
    // the producer lane's closed-world authority (A09 T11 / A10 courts); this
    // lane owns the container law over their bytes.
    //
    // Anti-vacuity, named M-W9P: each court itself mutates the real producer
    // buffer (one LexHash byte in record 0) and requires the real loader to
    // refuse it typed `SlotDerivationMismatch { index: 0, .. }` — a court that
    // cannot see producer-byte drift is FAIL. The shim-level flips over the
    // shared loader/derivation path (M1 window shift, M2 tag drop, M5/M6
    // detector branches, M-L exact-length-law relaxation, M-R record_size
    // skip, M-T2 `ClassId` u32->u64 widening) are re-executed in the wave-9
    // sandbox /tmp/a06-w9-sb with sha256 fail-closed restores; flips are
    // recorded in the wave-9 restart report, never assumed.
    //
    // Edit discipline: A09's `a06_citation_freshness_court` mechanically pins
    // `OABI_HEADER_LEN`'s declaration line (and the OABI_MAGIC..OABI_RECORD_SIZE
    // range) in THIS file — every Wave-9 addition is append-only below the
    // unit courts so the pinned lines never move.

    /// Read-only path of a committed producer pack in the canonical checkout
    /// (the same layout is mirrored by path in the /tmp sandboxes).
    #[cfg(test)]
    fn producer_pack_path(rel: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
    }

    /// Loads one committed producer pack through the real loader and asserts
    /// the shared §6/§5 law: it parses under the full check list, the record
    /// section is non-empty and exactly `slot_count` long, every stored slot
    /// re-derives through the §4 `truncate64` window on the producer's bytes,
    /// and no two records share a slot (§5 collision-freedom over the real
    /// corpus). Returns the raw bytes for the court-specific mutation rows.
    #[cfg(test)]
    fn load_producer_pack(rel: &str) -> (Vec<u8>, OabiPackV1) {
        let path = producer_pack_path(rel);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("committed producer pack {} must be readable: {e}", path.display()));
        let pack = OabiPackV1::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("committed producer pack {rel} must load under the §6 law: {e}"));
        assert!(pack.records().len() > 0, "{rel}: an empty pack is not a corpus");
        assert_eq!(
            pack.header().slot_count as usize,
            pack.records().len(),
            "{rel}: header slot_count must equal the record section"
        );
        let mut slots = std::collections::BTreeSet::new();
        for (i, r) in pack.records().iter().enumerate() {
            assert_eq!(
                truncate64(&r.lex).0,
                r.slot.0,
                "{rel}: record {i} slot must re-derive through the §4 window"
            );
            assert!(
                slots.insert(r.slot.0),
                "{rel}: record {i} duplicates a stored slot (§5 violation)"
            );
        }
        (bytes, pack)
    }

    /// Wave 9 — A09 ingestion court: `ontologies/fibo/fibo_sec_dbt_v1.oabi`
    /// (A09's committed pack, dev-depending on this crate) loads through the
    /// real loader; M-W9P: one flipped LexHash byte in record 0 must be
    /// refused typed.
    #[test]
    fn w9_fibo_pack_ingestion_court() {
        let (bytes, pack) = load_producer_pack("../../ontologies/fibo/fibo_sec_dbt_v1.oabi");
        assert_eq!(pack.header().version_major, OABI_VERSION_MAJOR);
        assert_ne!(
            pack.header().flags & OABI_FLAG_LEXHASH_PRESENT,
            0,
            "producer pack must carry LexHash witnesses"
        );
        let mut mutated = bytes.clone();
        mutated[OABI_HEADER_LEN + 16] ^= 0x01; // record 0, LexHash byte 0 (§6 table)
        match OabiPackV1::from_bytes(&mutated).unwrap_err() {
            OabiError::SlotDerivationMismatch { index: 0, .. } => {}
            other => panic!("M-W9P flip must be typed SlotDerivationMismatch, got {other:?}"),
        }
    }

    /// Wave 9 — A10 ingestion court: `ontologies/cloud/tosca_cimi_v1.oabi`
    /// through the same real-loader law; M-W9P as in the A09 court.
    #[test]
    fn w9_cloud_pack_ingestion_court() {
        let (bytes, pack) = load_producer_pack("../../ontologies/cloud/tosca_cimi_v1.oabi");
        assert_eq!(pack.header().version_major, OABI_VERSION_MAJOR);
        assert_ne!(pack.header().flags & OABI_FLAG_LEXHASH_PRESENT, 0);
        let mut mutated = bytes.clone();
        mutated[OABI_HEADER_LEN + 16] ^= 0x01; // record 0, LexHash byte 0 (§6 table)
        match OabiPackV1::from_bytes(&mutated).unwrap_err() {
            OabiError::SlotDerivationMismatch { index: 0, .. } => {}
            other => panic!("M-W9P flip must be typed SlotDerivationMismatch, got {other:?}"),
        }
    }
}
