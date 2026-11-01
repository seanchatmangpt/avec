//! Cloud O-ABI pack facade — lane A10 (`docs/jira/v26.11.1/lanes/A10-cloud-oabi.md`).
//!
//! Vendor-neutral cloud resource vocabulary from two standards spines
//! (lane doc §2 ADR): OASIS TOSCA (deployment/topology plane) and DMTF CIMI
//! (management plane). The `#![no_std]` facade itself uses NO dependencies —
//! const/types only: the pinned lexeme table (§5, every row citable to its
//! spec term or flagged `LOCAL(A10)`), SlotId goldens + 32-byte LexHash
//! preimage witnesses (A06 §2), the TOSCA<->CIMI equivalence table (§6), the
//! enum-family tables, and pack metadata. The pack producer is the build-time
//! `emit-pack` bin; the container/loader is A06's `cgap-bont` (cited, never
//! redefined).
//!
//! Every `slot`/`lex_hash` golden is derived, never hand-minted:
//! `SlotId = u64::from_le_bytes(BLAKE3(b"bont\x01" ++ lexeme)[0..8])`
//! (A06 §4). Values were produced by `emit-pack --print-table` (collision-
//! checked) and are re-derived on every court run through the real in-repo
//! substrate `cgap_blake3::hash`; a mistyped byte fails the courts.
//! `PACK_DIGEST_HEX` is committed at emit time over the exact on-disk
//! `tosca_cimi_v1.oabi` bytes (§7 T1); a placeholder was refused (an
//! unfaked pack identity — A09 §1 precedent).

#![no_std]

// --- A06 ABI widths: cited, not redefined (A06 §2/§6; loader/container on
// --- disk at `crates/cgap-bont/src/oabi.rs`). --------------------------------

/// SlotId width (A06 §2: `truncate64(BLAKE3(lexeme))`, first 8 bytes LE over
/// namespace tag `b"bont\x01"`).
pub const SLOT_ID_BITS: usize = 64;

/// LexHash width (A06 §2: 256-bit BLAKE3 preimage witness;
/// `SlotId == low64(LexHash)`).
pub const LEX_HASH_LEN: usize = 32;

/// `.oabi` v1 container record size (A06 §6; `OabiRecordV1`).
pub const OABI_V1_RECORD_SIZE: usize = 48;

/// `.oabi` v1 header length (A06 §6; `crates/cgap-bont/src/oabi.rs:48`).
pub const OABI_V1_HEADER_LEN: usize = 64;

/// ClassId width (A06 §2).
pub const CLASS_ID_BITS: usize = 32;

/// EnumId width (A06 §2).
pub const ENUM_ID_BITS: usize = 16;

// --- .oabi v1 header constants, restated for no_std consumers (A06 §6,
// --- cited). The witness court asserts these equal `cgap_bont::oabi`'s. ------

/// Pack magic, header bytes 0..8 (A06 §6).
pub const OABI_MAGIC: [u8; 8] = *b"OABIv1\0\0";

/// Frozen header length in bytes (A06 §6, §11).
pub const OABI_HEADER_LEN: usize = 64;

/// Frozen record size in bytes (A06 §6).
pub const OABI_RECORD_SIZE: usize = 48;

/// Format major version (A06 §6).
pub const OABI_VERSION_MAJOR: u16 = 1;

/// Format minor version (A06 §6).
pub const OABI_VERSION_MINOR: u16 = 0;

/// Flags bit0: little-endian pack (A06 §6/§10).
pub const OABI_FLAG_LE: u32 = 1 << 0;

/// Flags bit1: LexHash present in every record (A06 §6).
pub const OABI_FLAG_LEXHASH: u32 = 1 << 1;

// --- Dense ClassId space: lane doc §6 pins class_count = 4 (Compute, Blob,
// --- EventStream, Vault). Records carry this dense index; the A06 loader
// --- refuses class >= class_count (cgap-bont oabi.rs ClassOutOfRange). -------

/// `.oabi` header class_count (lane doc §6): the four cgap primitives.
pub const CLASS_COUNT: u32 = 4;

/// Class 0 — Compute (TOSCA `tosca.nodes.Compute` / CIMI `Machine`).
pub const CLASS_COMPUTE: u32 = 0;

/// Class 1 — Blob (TOSCA `tosca.nodes.BlockStorage` / CIMI `Volume`).
pub const CLASS_BLOB: u32 = 1;

/// Class 2 — EventStream (CIMI `Event`/`Job`; TOSCA has no native event
/// model — lane doc §5.3).
pub const CLASS_EVENTSTREAM: u32 = 2;

/// Class 3 — Vault (cgap-local class; CIMI `Credential` as neutral root
/// referent — lane doc §5.4).
pub const CLASS_VAULT: u32 = 3;

// --- Enum-family space. `enum_count = 6`: slot 0 is the ENUM_NONE sentinel
// --- (record carries no family) + 5 committed enum-carrying lexemes
// --- (lane doc §5: cimi.Machine.state, cimi.Volume.state, cimi.Event.severity,
// --- cimi.Job.status, cimi.Credential.type). The sentinel must live INSIDE
// --- [0, enum_count) because the A06 loader refuses enum >= enum_count
// --- (cgap-bont oabi.rs EnumOutOfRange). Divergence from the lane doc §6
// --- example formula ("per committed enum-carrying lexemes" = 5) recorded in
// --- the lane report. Variant values are pinned in [`ENUM_FAMILIES`]; the
// --- record field carries the FAMILY id, not the variant. --------------------

/// `.oabi` header enum_count: NONE sentinel + 5 families.
pub const ENUM_COUNT: u32 = 6;

/// Record enum value meaning "no enum family" (sentinel, inside the loaded
/// EnumId space).
pub const ENUM_NONE: u16 = 0;

/// Family 1 — CIMI `Machine` state.
pub const ENUM_FAMILY_MACHINE_STATE: u16 = 1;

/// Family 2 — CIMI `Volume` state.
pub const ENUM_FAMILY_VOLUME_STATE: u16 = 2;

/// Family 3 — CIMI `Event` severity (pinned subset over CIMI's String
/// severity attribute; recorded divergence).
pub const ENUM_FAMILY_EVENT_SEVERITY: u16 = 3;

/// Family 4 — CIMI `Job` status.
pub const ENUM_FAMILY_JOB_STATUS: u16 = 4;

/// Family 5 — CIMI `Credential` type.
pub const ENUM_FAMILY_CREDENTIAL_TYPE: u16 = 5;

/// One pinned enumeration variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnumVariant {
    /// Variant name as pinned by this pack (CIMI spellings upper-cased).
    pub name: &'static str,
    /// Pinned variant value (family-local, dense from 0).
    pub value: u16,
    /// Spec referent for the variant (spec-derived families) or the
    /// LOCAL(A10) flag.
    pub citation: &'static str,
}

/// One enum family: the record-carried EnumId + its pinned variants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnumFamily {
    /// Record-carried family id (`ENUM_NONE` is never a family id).
    pub id: u16,
    /// Family name.
    pub name: &'static str,
    /// Spec referent for the family.
    pub citation: &'static str,
    /// Pinned variants, dense from 0.
    pub variants: &'static [EnumVariant],
}

/// The five committed enum families (lane doc §5 enum-carrying lexemes).
pub const ENUM_FAMILIES: &[EnumFamily] = &[
    EnumFamily {
        id: ENUM_FAMILY_MACHINE_STATE,
        name: "machine.state",
        citation: "DMTF CIMI (DSP0263) Machine resource, state attribute; subset pinned by A10 v1",
        variants: &[
            EnumVariant { name: "STARTED", value: 0, citation: "DMTF CIMI (DSP0263) Machine state STARTED" },
            EnumVariant { name: "STOPPED", value: 1, citation: "DMTF CIMI (DSP0263) Machine state STOPPED" },
            EnumVariant { name: "ERROR", value: 2, citation: "DMTF CIMI (DSP0263) Machine state ERROR" },
        ],
    },
    EnumFamily {
        id: ENUM_FAMILY_VOLUME_STATE,
        name: "volume.state",
        citation: "DMTF CIMI (DSP0263) Volume resource, state attribute",
        variants: &[
            EnumVariant { name: "ONLINE", value: 0, citation: "DMTF CIMI (DSP0263) Volume state ONLINE" },
            EnumVariant { name: "DEGRADED", value: 1, citation: "DMTF CIMI (DSP0263) Volume state DEGRADED" },
            EnumVariant { name: "OFFLINE", value: 2, citation: "DMTF CIMI (DSP0263) Volume state OFFLINE" },
        ],
    },
    EnumFamily {
        id: ENUM_FAMILY_EVENT_SEVERITY,
        name: "event.severity",
        citation: "DMTF CIMI (DSP0263) Event resource, severity attribute (String in CIMI; pinned subset — recorded divergence)",
        variants: &[
            EnumVariant { name: "INFO", value: 0, citation: "A10-pinned subset over CIMI Event severity (String)" },
            EnumVariant { name: "WARNING", value: 1, citation: "A10-pinned subset over CIMI Event severity (String)" },
            EnumVariant { name: "CRITICAL", value: 2, citation: "A10-pinned subset over CIMI Event severity (String)" },
        ],
    },
    EnumFamily {
        id: ENUM_FAMILY_JOB_STATUS,
        name: "job.status",
        citation: "DMTF CIMI (DSP0263) Job resource, status attribute",
        variants: &[
            EnumVariant { name: "QUEUED", value: 0, citation: "DMTF CIMI (DSP0263) Job status Queued" },
            EnumVariant { name: "RUNNING", value: 1, citation: "DMTF CIMI (DSP0263) Job status Running" },
            EnumVariant { name: "SUCCESS", value: 2, citation: "DMTF CIMI (DSP0263) Job status Success" },
            EnumVariant { name: "FAILED", value: 3, citation: "DMTF CIMI (DSP0263) Job status Failed" },
        ],
    },
    EnumFamily {
        id: ENUM_FAMILY_CREDENTIAL_TYPE,
        name: "credential.type",
        citation: "DMTF CIMI (DSP0263) Credential resource, credentialType attribute",
        variants: &[
            EnumVariant { name: "SSH_PUBLIC_KEY", value: 0, citation: "DMTF CIMI (DSP0263) credential templates (ssh-public-key)" },
            EnumVariant { name: "PASSWORD", value: 1, citation: "DMTF CIMI (DSP0263) credential templates (password)" },
        ],
    },
];

// --- Origin: spec-derived vs cgap-local (lane doc §5: local-lexeme creep
// --- fence — `eventstream.channel` and `vault.*` are the only sanctioned
// --- LOCAL rows). -------------------------------------------------------------

/// Row origin (lane doc §5/§9 local-lexeme fence).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// OASIS TOSCA Simple Profile term.
    Tosca,
    /// DMTF CIMI term.
    Cimi,
    /// cgap-local (A10) — flagged, never spec-derived.
    Local,
}

/// One pinned lexeme row: identity + A06 record payload + provenance.
///
/// `slot`/`lex_hash` are the committed goldens:
/// `slot == u64::from_le_bytes(lex_hash[0..8])` (A06 §4); both re-derived by
/// the courts and by `emit-pack` (fail-closed golden-drift refusal).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LexemeRow {
    /// The pinned lexeme (spec string or cgap-local name).
    pub lexeme: &'static str,
    /// Dense primitive class (record ClassId field).
    pub class: u32,
    /// Record EnumId field: family id or [`ENUM_NONE`].
    pub enum_family: u16,
    /// Provenance flag.
    pub origin: Origin,
    /// Spec citation, or `LOCAL(A10)` marker for cgap-local rows.
    pub citation: &'static str,
    /// Committed golden: `truncate64(BLAKE3(b"bont\x01" ++ lexeme))`.
    pub slot: u64,
    /// Committed golden: full 32-byte BLAKE3 preimage witness (A06 §2).
    pub lex_hash: [u8; LEX_HASH_LEN],
}

/// The pinned pack corpus (lane doc §5, all four primitives, in doc order).
///
/// Golden `slot`/`lex_hash` values were produced by `emit-pack --print-table`
/// (collision-checked) and are committed verbatim; `emit-pack` and the courts
/// re-derive every value and REFUSE on drift (fail-closed, DoD 2/6).
pub const LEXEME_TABLE: &[LexemeRow] = &[
    LexemeRow { lexeme: "tosca.nodes.Compute", class: 0, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, node type tosca.nodes.Compute", slot: 0xB854D7A1774AEB63, lex_hash: [0x63, 0xEB, 0x4A, 0x77, 0xA1, 0xD7, 0x54, 0xB8, 0xB6, 0xC7, 0x2A, 0xC2, 0x99, 0x59, 0x7D, 0x29, 0x6A, 0x09, 0x42, 0x80, 0x90, 0xE6, 0xD4, 0x75, 0xAE, 0x27, 0x46, 0xFC, 0x89, 0xE1, 0x0D, 0xD4] },
    LexemeRow { lexeme: "compute.host", class: 0, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.nodes.Compute host capability", slot: 0xEF256A9A6B3617CD, lex_hash: [0xCD, 0x17, 0x36, 0x6B, 0x9A, 0x6A, 0x25, 0xEF, 0xBA, 0x5F, 0x94, 0xBE, 0x34, 0x26, 0x60, 0x93, 0x94, 0xA2, 0xCB, 0x93, 0xB3, 0x05, 0x56, 0x1E, 0xDC, 0x91, 0x26, 0xC9, 0x07, 0xC9, 0x77, 0x4B] },
    LexemeRow { lexeme: "compute.endpoint", class: 0, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.nodes.Compute endpoint capability", slot: 0x5926AD0A569D1527, lex_hash: [0x27, 0x15, 0x9D, 0x56, 0x0A, 0xAD, 0x26, 0x59, 0x1A, 0x51, 0x0C, 0x92, 0x6F, 0x30, 0xDB, 0xCD, 0x99, 0xAD, 0x2C, 0xD9, 0xFB, 0x2A, 0x8C, 0x92, 0x6D, 0xC8, 0xAB, 0x66, 0x8F, 0xAD, 0xDB, 0xA7] },
    LexemeRow { lexeme: "compute.os", class: 0, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.nodes.Compute os capability", slot: 0x5BD241E472F3A357, lex_hash: [0x57, 0xA3, 0xF3, 0x72, 0xE4, 0x41, 0xD2, 0x5B, 0x59, 0xBF, 0x6A, 0x29, 0x36, 0x26, 0xD7, 0xB4, 0x94, 0x89, 0xA9, 0xD8, 0x42, 0x2E, 0xD5, 0xA4, 0xE1, 0xFD, 0xAA, 0x66, 0x72, 0xF4, 0x6A, 0x2E] },
    LexemeRow { lexeme: "cimi.Machine", class: 0, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Machine resource", slot: 0x9D214CB4A0A68EE7, lex_hash: [0xE7, 0x8E, 0xA6, 0xA0, 0xB4, 0x4C, 0x21, 0x9D, 0xB5, 0x72, 0xBC, 0x9B, 0x28, 0x31, 0xFB, 0x84, 0x2A, 0xA8, 0x85, 0x32, 0x2A, 0x8F, 0x2A, 0x46, 0xA8, 0xE8, 0x9E, 0x31, 0x79, 0x4B, 0xF9, 0xBF] },
    LexemeRow { lexeme: "cimi.Machine.state", class: 0, enum_family: ENUM_FAMILY_MACHINE_STATE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Machine resource, state attribute", slot: 0x009C06E26537E1D3, lex_hash: [0xD3, 0xE1, 0x37, 0x65, 0xE2, 0x06, 0x9C, 0x00, 0x35, 0x9B, 0xAD, 0xE4, 0xAC, 0x35, 0x16, 0x5D, 0x7A, 0xA5, 0xAF, 0xFA, 0x4B, 0x8A, 0xEA, 0xF6, 0xC0, 0xE9, 0xC9, 0x9A, 0xED, 0x71, 0xEF, 0x0D] },
    LexemeRow { lexeme: "cimi.Machine.cpu", class: 0, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Machine resource, cpu attribute", slot: 0xD77E147173CD32BF, lex_hash: [0xBF, 0x32, 0xCD, 0x73, 0x71, 0x14, 0x7E, 0xD7, 0x91, 0xB7, 0xC6, 0x83, 0xB2, 0x2A, 0x1E, 0x7E, 0x3C, 0x5D, 0xD4, 0x2E, 0x9B, 0x88, 0x02, 0x12, 0x06, 0x7E, 0x1C, 0xA9, 0xB7, 0x1C, 0x6F, 0x74] },
    LexemeRow { lexeme: "cimi.Machine.memory", class: 0, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Machine resource, memory attribute", slot: 0xD830899582CE5750, lex_hash: [0x50, 0x57, 0xCE, 0x82, 0x95, 0x89, 0x30, 0xD8, 0x02, 0xB7, 0xBD, 0xBC, 0xF0, 0x73, 0xBB, 0x0A, 0x3F, 0xA4, 0x59, 0xD6, 0x21, 0xD7, 0xE4, 0xE5, 0x81, 0x50, 0xA3, 0xED, 0xED, 0x01, 0x46, 0x1E] },
    LexemeRow { lexeme: "cimi.Machine.disks", class: 0, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Machine resource, disks attribute", slot: 0x05A09C95FD63F2A5, lex_hash: [0xA5, 0xF2, 0x63, 0xFD, 0x95, 0x9C, 0xA0, 0x05, 0xBC, 0xD9, 0xD1, 0x85, 0x00, 0x7B, 0x55, 0xF4, 0x90, 0xFA, 0xC6, 0xC8, 0xDC, 0x92, 0xD9, 0x92, 0xEF, 0xBE, 0x3A, 0xEE, 0x6A, 0x5E, 0xFD, 0x39] },
    LexemeRow { lexeme: "tosca.nodes.BlockStorage", class: 1, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, node type tosca.nodes.BlockStorage", slot: 0x287C79B1117B30C5, lex_hash: [0xC5, 0x30, 0x7B, 0x11, 0xB1, 0x79, 0x7C, 0x28, 0x68, 0x0F, 0x97, 0x86, 0xC4, 0x7A, 0x07, 0x2D, 0x6A, 0xBA, 0x40, 0xD4, 0x6F, 0xF8, 0xAE, 0xD7, 0x32, 0xA3, 0x55, 0xF0, 0xB4, 0xD2, 0x91, 0xCB] },
    LexemeRow { lexeme: "blob.size", class: 1, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.nodes.BlockStorage size property", slot: 0x1C2DB3ED1A72ED3E, lex_hash: [0x3E, 0xED, 0x72, 0x1A, 0xED, 0xB3, 0x2D, 0x1C, 0xB7, 0x44, 0x15, 0x8B, 0x8A, 0xB4, 0xCF, 0xE2, 0xD3, 0x8F, 0xCD, 0x2B, 0xBB, 0xD6, 0xCB, 0x31, 0x80, 0x84, 0x2A, 0xEE, 0x05, 0xFC, 0xA5, 0xE7] },
    LexemeRow { lexeme: "blob.volume_id", class: 1, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.nodes.BlockStorage volume_id property", slot: 0xF5B83F5FBF140DBC, lex_hash: [0xBC, 0x0D, 0x14, 0xBF, 0x5F, 0x3F, 0xB8, 0xF5, 0x95, 0x0D, 0x7D, 0xD7, 0xB7, 0x2A, 0xF1, 0xEF, 0xE9, 0x7A, 0x72, 0x2B, 0xE5, 0x59, 0x67, 0x75, 0xBD, 0x83, 0xC8, 0x69, 0xB1, 0x05, 0x42, 0x03] },
    LexemeRow { lexeme: "blob.snapshot_id", class: 1, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.nodes.BlockStorage snapshot_id property", slot: 0xB913801BAD0A6495, lex_hash: [0x95, 0x64, 0x0A, 0xAD, 0x1B, 0x80, 0x13, 0xB9, 0xD3, 0xC4, 0x43, 0xE6, 0x08, 0x8A, 0xE9, 0xEA, 0x85, 0xB5, 0xB8, 0x48, 0x39, 0xB9, 0x3E, 0x23, 0x95, 0x72, 0x41, 0x44, 0x42, 0x0E, 0xF2, 0x47] },
    LexemeRow { lexeme: "blob.attachment", class: 1, enum_family: ENUM_NONE, origin: Origin::Tosca, citation: "OASIS TOSCA Simple Profile in YAML v1.3, tosca.capabilities.Attachment (BlockStorage attachable capability)", slot: 0xF6F75E0411830D01, lex_hash: [0x01, 0x0D, 0x83, 0x11, 0x04, 0x5E, 0xF7, 0xF6, 0x96, 0x87, 0xEB, 0x6D, 0x00, 0x23, 0x9D, 0xC6, 0x3D, 0x38, 0x70, 0x8D, 0x78, 0xFD, 0x2A, 0xB4, 0x86, 0x0A, 0xB2, 0xC8, 0x75, 0x0D, 0xDA, 0x57] },
    LexemeRow { lexeme: "cimi.Volume", class: 1, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Volume resource", slot: 0x2547F3D7D9E3CEBD, lex_hash: [0xBD, 0xCE, 0xE3, 0xD9, 0xD7, 0xF3, 0x47, 0x25, 0x90, 0x0A, 0xF2, 0x87, 0x4F, 0xB2, 0xB6, 0x65, 0xFF, 0xFA, 0xA4, 0xE2, 0x43, 0x24, 0xA2, 0x81, 0xD5, 0x19, 0x80, 0x07, 0xBB, 0x63, 0x7A, 0x73] },
    LexemeRow { lexeme: "cimi.Volume.state", class: 1, enum_family: ENUM_FAMILY_VOLUME_STATE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Volume resource, state attribute", slot: 0xAB8F90F0FF539B52, lex_hash: [0x52, 0x9B, 0x53, 0xFF, 0xF0, 0x90, 0x8F, 0xAB, 0xF1, 0x96, 0x94, 0x4D, 0xD1, 0x1B, 0x91, 0x36, 0xD8, 0xBC, 0x2C, 0x63, 0xF7, 0xA2, 0xF1, 0x90, 0x47, 0x73, 0x35, 0xCB, 0x25, 0xE6, 0x28, 0xBF] },
    LexemeRow { lexeme: "cimi.Volume.capacity", class: 1, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Volume resource, capacity attribute", slot: 0x2EA79F81247A7FAD, lex_hash: [0xAD, 0x7F, 0x7A, 0x24, 0x81, 0x9F, 0xA7, 0x2E, 0x31, 0x34, 0x9B, 0x43, 0x58, 0xAD, 0xF0, 0xE1, 0xD2, 0x66, 0x89, 0x0E, 0xFE, 0x33, 0x9D, 0x3F, 0xE1, 0x1F, 0xB7, 0xFB, 0x1A, 0x3F, 0x74, 0xD8] },
    LexemeRow { lexeme: "cimi.VolumeImage", class: 1, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) VolumeImage resource", slot: 0x4AC2FAEA1A3E069F, lex_hash: [0x9F, 0x06, 0x3E, 0x1A, 0xEA, 0xFA, 0xC2, 0x4A, 0x1C, 0x0F, 0x04, 0xB3, 0x23, 0xF3, 0x28, 0xD6, 0x8B, 0x87, 0x50, 0xA0, 0xE2, 0x4F, 0x1C, 0x3D, 0xE2, 0x01, 0x39, 0xE9, 0x4C, 0xC4, 0xFE, 0x49] },
    LexemeRow { lexeme: "cimi.Event", class: 2, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Event resource", slot: 0xEF1E1F619B71BD37, lex_hash: [0x37, 0xBD, 0x71, 0x9B, 0x61, 0x1F, 0x1E, 0xEF, 0x4F, 0x02, 0x63, 0xB4, 0xA1, 0xF4, 0x1E, 0xE6, 0xFE, 0x5F, 0x86, 0xFA, 0x20, 0x2C, 0x9D, 0xDE, 0x2A, 0xEA, 0x66, 0x6F, 0xB2, 0x4D, 0xE4, 0xFB] },
    LexemeRow { lexeme: "cimi.Event.content", class: 2, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Event resource, content attribute", slot: 0xDF1393116A634E08, lex_hash: [0x08, 0x4E, 0x63, 0x6A, 0x11, 0x93, 0x13, 0xDF, 0x0C, 0x60, 0xB1, 0x25, 0x4C, 0x53, 0x20, 0xB8, 0x1B, 0xA1, 0x31, 0xBA, 0x20, 0x00, 0xB7, 0xD6, 0xF7, 0x98, 0xF2, 0x8E, 0xDE, 0x37, 0xFB, 0x0D] },
    LexemeRow { lexeme: "cimi.Event.severity", class: 2, enum_family: ENUM_FAMILY_EVENT_SEVERITY, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Event resource, severity attribute (String; pinned subset — recorded divergence)", slot: 0x5093557C0458E39D, lex_hash: [0x9D, 0xE3, 0x58, 0x04, 0x7C, 0x55, 0x93, 0x50, 0x10, 0x3C, 0x62, 0xAD, 0x71, 0xC9, 0xC3, 0x89, 0x80, 0x19, 0x97, 0xFF, 0x22, 0xC3, 0x6B, 0x4D, 0x04, 0x07, 0x2D, 0xF8, 0x04, 0x01, 0x14, 0x71] },
    LexemeRow { lexeme: "cimi.Event.timestamp", class: 2, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Event resource, time attribute", slot: 0x58C37A35F9170DE6, lex_hash: [0xE6, 0x0D, 0x17, 0xF9, 0x35, 0x7A, 0xC3, 0x58, 0x8D, 0xBF, 0x0A, 0x92, 0x3C, 0x1E, 0xE4, 0x4F, 0xF7, 0x53, 0x35, 0xF0, 0x7D, 0x29, 0x12, 0xE3, 0x39, 0x8C, 0xA6, 0xDB, 0x88, 0xCF, 0x23, 0xF8] },
    LexemeRow { lexeme: "cimi.Job", class: 2, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Job resource (async operation record)", slot: 0xED618B714CB452E1, lex_hash: [0xE1, 0x52, 0xB4, 0x4C, 0x71, 0x8B, 0x61, 0xED, 0x53, 0x7C, 0x4A, 0xE9, 0x3D, 0x8F, 0xE1, 0x03, 0xCA, 0x52, 0x60, 0x61, 0x6C, 0xCE, 0xF8, 0xC4, 0xF3, 0x2B, 0x57, 0x53, 0x8F, 0x6C, 0x4B, 0xCC] },
    LexemeRow { lexeme: "cimi.Job.status", class: 2, enum_family: ENUM_FAMILY_JOB_STATUS, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Job resource, status attribute", slot: 0x91B84C786D6FF0ED, lex_hash: [0xED, 0xF0, 0x6F, 0x6D, 0x78, 0x4C, 0xB8, 0x91, 0x6B, 0x3F, 0x0E, 0x74, 0x5F, 0xF1, 0x38, 0xF5, 0x0D, 0x32, 0x93, 0xB5, 0xDB, 0x1A, 0x01, 0xF1, 0x0B, 0xC4, 0xEC, 0x49, 0x69, 0x62, 0x2D, 0x6A] },
    LexemeRow { lexeme: "cimi.Job.progress", class: 2, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Job resource, progress attribute", slot: 0xC67AA754455B0566, lex_hash: [0x66, 0x05, 0x5B, 0x45, 0x54, 0xA7, 0x7A, 0xC6, 0xF3, 0x4F, 0xC5, 0xAE, 0x31, 0x17, 0x45, 0x22, 0x7F, 0x7D, 0xD0, 0x63, 0xFA, 0x7A, 0xA6, 0x21, 0x0C, 0x9F, 0x56, 0xD3, 0xCA, 0x49, 0x71, 0xC3] },
    LexemeRow { lexeme: "eventstream.channel", class: 2, enum_family: ENUM_NONE, origin: Origin::Local, citation: "LOCAL(A10)", slot: 0x08288025594315EF, lex_hash: [0xEF, 0x15, 0x43, 0x59, 0x25, 0x80, 0x28, 0x08, 0xB7, 0x14, 0x18, 0x3B, 0x74, 0x48, 0x4A, 0x99, 0x2D, 0x18, 0xC7, 0x37, 0x8B, 0x98, 0x28, 0xFE, 0xEF, 0xC0, 0x66, 0x81, 0xF6, 0xEA, 0x62, 0x59] },
    LexemeRow { lexeme: "cimi.Credential", class: 3, enum_family: ENUM_NONE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Credential resource", slot: 0xB631C4883C95D603, lex_hash: [0x03, 0xD6, 0x95, 0x3C, 0x88, 0xC4, 0x31, 0xB6, 0x03, 0xB8, 0x9A, 0xAB, 0x47, 0x92, 0x69, 0x43, 0x97, 0xB7, 0x6E, 0xBC, 0x32, 0x7A, 0x45, 0x22, 0xCC, 0xF8, 0x30, 0x24, 0x6E, 0x5B, 0x79, 0x0C] },
    LexemeRow { lexeme: "cimi.Credential.type", class: 3, enum_family: ENUM_FAMILY_CREDENTIAL_TYPE, origin: Origin::Cimi, citation: "DMTF CIMI (DSP0263) Credential resource, credentialType attribute", slot: 0xCBC9F1E33CDAF67C, lex_hash: [0x7C, 0xF6, 0xDA, 0x3C, 0xE3, 0xF1, 0xC9, 0xCB, 0x01, 0xCB, 0x3B, 0xCB, 0xAA, 0x2B, 0xE2, 0xAB, 0x0A, 0x1D, 0x49, 0xD5, 0x86, 0x8C, 0x59, 0xA8, 0xF8, 0x1B, 0xDF, 0x35, 0xDF, 0x4A, 0x76, 0xDD] },
    LexemeRow { lexeme: "vault.secret", class: 3, enum_family: ENUM_NONE, origin: Origin::Local, citation: "LOCAL(A10)", slot: 0x7BDF988EF9054AC3, lex_hash: [0xC3, 0x4A, 0x05, 0xF9, 0x8E, 0x98, 0xDF, 0x7B, 0x36, 0x6A, 0xDB, 0xE0, 0x49, 0x7E, 0xB6, 0x48, 0x2B, 0xA8, 0xA3, 0x38, 0x15, 0x2B, 0x12, 0x96, 0x8E, 0x14, 0xF9, 0x3D, 0xA8, 0xE1, 0x0F, 0x84] },
    LexemeRow { lexeme: "vault.lease", class: 3, enum_family: ENUM_NONE, origin: Origin::Local, citation: "LOCAL(A10)", slot: 0xE9C0C33CE04C105C, lex_hash: [0x5C, 0x10, 0x4C, 0xE0, 0x3C, 0xC3, 0xC0, 0xE9, 0x8E, 0xF6, 0xFF, 0xFB, 0xC7, 0xD5, 0xCC, 0xF0, 0x05, 0x09, 0x32, 0x14, 0xC3, 0xFF, 0xFC, 0xF6, 0x69, 0x66, 0x1D, 0xF5, 0x92, 0x55, 0x32, 0xE8] },
    LexemeRow { lexeme: "vault.mount", class: 3, enum_family: ENUM_NONE, origin: Origin::Local, citation: "LOCAL(A10)", slot: 0x6ECD09CF28EC2A49, lex_hash: [0x49, 0x2A, 0xEC, 0x28, 0xCF, 0x09, 0xCD, 0x6E, 0x46, 0x25, 0x0B, 0x71, 0x48, 0x21, 0xC1, 0xCE, 0xEF, 0xAC, 0x94, 0x94, 0x94, 0x7C, 0x1A, 0x8B, 0x85, 0x26, 0x93, 0x9C, 0x36, 0xFE, 0xAE, 0x23] },
];


// --- TOSCA<->CIMI equivalence table (lane doc §6: "the alias lexeme gets its
// --- own LexHash and its own SlotId, and the equivalence is a crate-level
// --- mapping alias_slot -> canonical_slot"). -------------------------------

/// One cross-spec equivalence row (lane doc §6/§9 mapping-ambiguity risk:
/// both lexemes pinned, neither wins).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EquivRow {
    /// Alias lexeme (resolves THROUGH this row).
    pub alias_lexeme: &'static str,
    /// Canonical lexeme (resolution target).
    pub canonical_lexeme: &'static str,
    /// Spec citation for the pairing.
    pub citation: &'static str,
}

/// The four v1 equivalence rows (lane doc §5 equivalence-typed lexemes).
pub const EQUIV_TABLE: &[EquivRow] = &[
    EquivRow {
        alias_lexeme: "cimi.Machine",
        canonical_lexeme: "tosca.nodes.Compute",
        citation: "A10 §5.1: CIMI Machine <-> TOSCA Compute (compute-plane equivalence)",
    },
    EquivRow {
        alias_lexeme: "cimi.Volume",
        canonical_lexeme: "tosca.nodes.BlockStorage",
        citation: "A10 §5.2/§9: BlockStorage vs Volume — witnessed mapping-ambiguity case, both pinned",
    },
    EquivRow {
        alias_lexeme: "cimi.VolumeImage",
        canonical_lexeme: "tosca.nodes.BlockStorage",
        citation: "A10 §5.2: VolumeImage is a Blob-plane image of BlockStorage",
    },
    EquivRow {
        alias_lexeme: "cimi.Job",
        canonical_lexeme: "cimi.Event",
        citation: "A10 §5.3: CIMI Job is an async-operation record on the EventStream class",
    },
];

/// Resolve an alias SlotId to its canonical SlotId (lane doc §6: pure
/// in-crate lookup; alias->canonical only — bidirectional is Wave 2).
pub fn resolve_equivalence(alias_slot: u64) -> Option<u64> {
    EQUIV_TABLE
        .iter()
        .find(|row| slot_of(row.alias_lexeme) == alias_slot)
        .map(|row| slot_of(row.canonical_lexeme))
}

/// Resolve an alias lexeme to its canonical SlotId.
pub fn resolve_equivalence_lexeme(alias_lexeme: &str) -> Option<u64> {
    EQUIV_TABLE
        .iter()
        .find(|row| row.alias_lexeme == alias_lexeme)
        .map(|row| slot_of(row.canonical_lexeme))
}

/// The committed SlotId of a table lexeme (linear scan; pack tables are
/// small and this is a const-table lookup, not a runtime interner).
pub fn slot_of(lexeme: &str) -> u64 {
    match LEXEME_TABLE.iter().find(|row| row.lexeme == lexeme) {
        Some(row) => row.slot,
        None => 0,
    }
}

// --- T2 named goldens (lane doc §7 T2): the four primitive roots + four
// --- equivalence lexemes, committed as individual pub consts for membrane
// --- ergonomics. Re-derived by T2 through the hand-applied A06 §4 spec, not
// --- by calling `cgap_bont::slot_id`. ----------------------------------------

/// `tosca.nodes.Compute` (primitive root, Compute).
pub const TOSCA_COMPUTE_SLOT: u64 = 0xB854D7A1774AEB63;
/// `tosca.nodes.BlockStorage` (primitive root, Blob).
pub const TOSCA_BLOCKSTORAGE_SLOT: u64 = 0x287C79B1117B30C5;
/// `cimi.Event` (primitive root, EventStream).
pub const CIMI_EVENT_SLOT: u64 = 0xEF1E1F619B71BD37;
/// `cimi.Credential` (primitive root, Vault).
pub const CIMI_CREDENTIAL_SLOT: u64 = 0xB631C4883C95D603;
/// `cimi.Machine` (equivalence alias of Compute).
pub const CIMI_MACHINE_SLOT: u64 = 0x9D214CB4A0A68EE7;
/// `cimi.Volume` (equivalence alias of BlockStorage).
pub const CIMI_VOLUME_SLOT: u64 = 0x2547F3D7D9E3CEBD;
/// `cimi.VolumeImage` (equivalence alias of BlockStorage).
pub const CIMI_VOLUME_IMAGE_SLOT: u64 = 0x4AC2FAEA1A3E069F;
/// `cimi.Job` (equivalence alias of Event).
pub const CIMI_JOB_SLOT: u64 = 0xED618B714CB452E1;

// --- Pack metadata (lane doc §6: pack-local metadata lives in the crate,
// --- never as header mutations). -------------------------------------------

/// Pack generation label (lane doc §2/§9: maps to header version_major/minor;
/// a lexeme-set change is a new generation file, never an edit of v1 bytes).
pub const PACK_GENERATION: &str = "tosca_cimi_v1";

/// TOSCA spine citation (pack metadata).
pub const TOSCA_PROFILE: &str = "OASIS TOSCA Simple Profile in YAML v1.3";

/// CIMI spine citation (pack metadata).
pub const CIMI_PROFILE: &str = "DMTF CIMI (DSP0263) v2.0.0";

/// The sanctioned cgap-local lexemes (lane doc §9 local-lexeme fence).
pub const LOCAL_LEXEMES: &[&str] = &["eventstream.channel", "vault.secret", "vault.lease", "vault.mount"];

/// BLAKE3 digest (hex, uppercase) of the exact committed
/// `ontologies/cloud/tosca_cimi_v1.oabi` bytes (lane doc §7 T1 golden
/// digest court). Committed at emit time 2026-11-01 over the 1552-byte
/// pack (64 B header + 31 x 48 B records).
pub const PACK_DIGEST_HEX: &str = "974C73EAC4F8ACF03AD67B9FEA5961574592B8A8F95BD53E7BA82B50EC156472";

// --- Compile-time gates (lane doc DoD 1: A06 widths under static gates).
// --- `static_assertions` is intentionally NOT a dependency (the facade stays
// --- dep-free, A09 precedent); these const asserts are the same compile-time
// --- gate (divergence recorded in the lane report). ---------------------------

const _: () = assert!(SLOT_ID_BITS == 64 && core::mem::size_of::<u64>() == 8);
const _: () = assert!(CLASS_ID_BITS == 32 && ENUM_ID_BITS == 16 && LEX_HASH_LEN == 32);
const _: () = assert!(OABI_HEADER_LEN == 64 && OABI_RECORD_SIZE == 48);
const _: () = assert!(CLASS_COUNT == 4);
const _: () = assert!(ENUM_COUNT == 6 && ENUM_NONE == 0);
const _: () = assert!(CLASS_VAULT < CLASS_COUNT, "class space bound");
const _: () = assert!(
    (ENUM_FAMILY_CREDENTIAL_TYPE as u32) < ENUM_COUNT,
    "enum family space bound",
);
