//! `.bosym` v0 ABI freeze (lane A07, Wave 1 ABI-freeze slice).
//!
//! Wire format is authored in
//! `docs/jira/v26.11.1/lanes/A07-bosym-slab.md` (".bosym Archive Format
//! (v0, PROPOSED)", lines 140-170); this module freezes that ABI as
//! constants + reference layouts + compile-time size/align/offset asserts.
//!
//! Wave 3 (this slice) extends the frozen ABI with the full lane scope per
//! the lane doc: a std-only SHA-256, a fail-closed archive reader, an
//! archive builder, the build-time generator (graph export in → `.bosym`
//! archive + string-free projection out, DoD 1), and the Chicago courts
//! T1-T3 of the lane doc's test plan. cgap-bosym's Cargo.toml is outside
//! this lane's file ownership, so the whole codec is dependency-free std.
//!
//! Status: format v0 implemented (DoD 2). The wire is little-endian; the
//! trailing SHA-256 digest is carried as raw 32 bytes ("big-endian
//! convention" in the lane doc refers to digest hex rendering, not to byte
//! reversal — the bytes are the digest as SHA-256 emits them).
//!
//! Wave 5 (defect-repair slice): no codec change; the SHA-256 court now
//! pins edge-length digests (55/56/57/63/64/119/120/200, one-shot AND
//! 7-byte-chunked incremental) derived from an independent implementation
//! (python3 hashlib) — the previous vectors covered only 0/3/200-split and
//! never exercised finalize()'s padding-loop edges (buf_len 55..=64).
//!
//! Wave 8 (defect repair driven by the wave-6/7 court receipts): no codec
//! change. Recorded here per the wave-5/6 audit ledger ("name the consumer
//! or downgrade to a recorded deferral"): DoD 6 — "execution crates compile
//! from the string-free projection" — is a RECORDED DEFERRAL as of wave 8.
//! No workspace crate consumes [`emit_projection`] output (repo-wide grep,
//! waves 5, 6, and 8 all: zero consumers); wiring a consumer requires
//! editing execution crates owned by other lanes (off-limits to A07), so
//! the deferral is recorded at the generator itself. Zero features added
//! without a court.

use std::fmt;
use std::mem::{align_of, offset_of, size_of};

/// Archive magic: 8 bytes, ASCII `CGAPBSYM` — a mis-routed copy is
/// identifiable on sight (lane doc line 143).
pub const MAGIC: [u8; 8] = *b"CGAPBSYM";

/// Length of [`MAGIC`] in bytes.
pub const MAGIC_LEN: usize = 8;

/// Archive format version: u16 LE, `0` for v0.
pub const VERSION: u16 = 0;

/// Flags bit 0 = compressed. v0: must be `0` (lane doc line 147).
pub const FLAG_COMPRESSED: u16 = 1 << 0;

/// Flags bit 1 = signed. v0: must be `0` (lane doc line 147).
pub const FLAG_SIGNED: u16 = 1 << 1;

/// Bits that MUST be zero in a v0 archive (compression and signing are
/// version bumps, not format breaks — lane doc lines 169-170).
pub const FLAGS_V0_MASK: u16 = FLAG_COMPRESSED | FLAG_SIGNED;

/// Symbol kinds (lane doc line 151: 0=entity, 1=relation, 2=IRI, 3=label,
/// 4=annotation).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// Kind 0.
    Entity = 0,
    /// Kind 1.
    Relation = 1,
    /// Kind 2.
    Iri = 2,
    /// Kind 3.
    Label = 3,
    /// Kind 4.
    Annotation = 4,
}

/// Numeric kind values as `u8` consts (the wire field is a bare `u8`; the
/// enum is the named view). Wire values per lane doc line 151.
impl SymbolKind {
    /// Kind `0` = entity.
    pub const ENTITY: u8 = 0;
    /// Kind `1` = relation.
    pub const RELATION: u8 = 1;
    /// Kind `2` = IRI.
    pub const IRI: u8 = 2;
    /// Kind `3` = label.
    pub const LABEL: u8 = 3;
    /// Kind `4` = annotation.
    pub const ANNOTATION: u8 = 4;
    /// Number of defined kinds.
    pub const COUNT: u8 = 5;
    /// Highest defined kind value.
    pub const MAX: u8 = Self::ANNOTATION;
}

/// Header: `magic`(8) + `version`(u16 LE) + `flags`(u16 LE) +
/// `entry_count`(u32 LE) = 16 bytes (lane doc lines 142-148).
///
/// `#[repr(C)]` reference view of the on-wire header; field offsets are
/// pinned to the wire arithmetic by the static asserts below. Endianness is
/// little-endian on the wire; the reference view does not encode it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BosymHeader {
    /// Bytes 0..8. Must equal [`MAGIC`].
    pub magic: [u8; 8],
    /// Bytes 8..10, u16 LE. Must equal [`VERSION`] for v0.
    pub version: u16,
    /// Bytes 10..12, u16 LE. `FLAGS_V0_MASK` bits must be zero in v0.
    pub flags: u16,
    /// Bytes 12..16, u32 LE. Number of entry records that follow.
    pub entry_count: u32,
}

/// Fixed prefix of an entry record: `sym_id`(u64 LE) + `kind`(u8) +
/// `reserved`([u8; 3], must be zero) + `name_len`(u16 LE) = 14 bytes
/// (lane doc lines 149-153).
///
/// `#[repr(C, packed)]` because the wire prefix is 14 bytes — an unpacked
/// `repr(C)` view would tail-pad to align 8 and diverge from the wire.
/// Field reads from a packed view are the reader logic's concern (gate
/// slice), not this module's; the reference view exists so the layout is
/// pinned by an assert, not by a comment.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BosymEntryPrefix {
    /// Prefix bytes 0..8, u64 LE — the interned ID the kernel uses.
    pub sym_id: u64,
    /// Prefix byte 8 — a [`SymbolKind`] value.
    pub kind: u8,
    /// Prefix bytes 9..12 — must be zero (lane doc line 152).
    pub reserved: [u8; 3],
    /// Prefix bytes 12..14, u16 LE — length of the `name` field.
    pub name_len: u16,
}

/// `sha16`: first 16 bytes of SHA-256 over `name` — the reader's cheap
/// integrity check (lane doc lines 157-158).
pub const SHA16_SIZE: usize = 16;

/// Trailing section: SHA-256 of all preceding bytes (lane doc lines
/// 159-160) so truncation/corruption fails closed on read.
pub const TRAILER_SIZE: usize = 32;

/// SHA-256 digest width, for the trailing section.
pub type Sha256Digest = [u8; TRAILER_SIZE];

/// Header size in bytes: 8 + 2 + 2 + 4.
pub const HEADER_SIZE: usize = MAGIC_LEN + 2 + 2 + 4;

/// Fixed entry-prefix size in bytes: 8 + 1 + 3 + 2.
pub const ENTRY_PREFIX_SIZE: usize = 8 + 1 + 3 + 2;

/// Maximum `name` length: the `name_len` field is u16.
pub const MAX_NAME_LEN: usize = u16::MAX as usize;

/// Minimum total entry size: fixed prefix + empty name + `sha16`.
pub const ENTRY_MIN_SIZE: usize = ENTRY_PREFIX_SIZE + SHA16_SIZE;

/// Maximum total entry size: prefix + max name + `sha16`.
pub const ENTRY_MAX_SIZE: usize = ENTRY_PREFIX_SIZE + MAX_NAME_LEN + SHA16_SIZE;

/// Canonical generator name emitted into every generated-artifact header
/// (DoD 1: "the generator named in the header of every generated file").
pub const GENERATOR_NAME: &str = "cgap-bosym";

/// Canonical generator version recorded alongside [`GENERATOR_NAME`]. The
/// crate version comes from the workspace (`26.10.29` at the v26.11.1
/// fan-out); the format is `cgap-bosym/<crate-version>`.
pub const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// ABI freeze asserts. Each assert cross-checks the reference layout against
// the byte arithmetic above, so a layout drift flips a compile error, not a
// silent wire divergence. Zero runtime code.
// ---------------------------------------------------------------------------

const _: () = assert!(MAGIC.len() == MAGIC_LEN && MAGIC_LEN == 8);

const _: () = assert!(
    HEADER_SIZE == 16,
    ".bosym v0 header is 16 bytes (8 magic + 2 version + 2 flags + 4 entry_count)"
);

const _: () = assert!(ENTRY_PREFIX_SIZE == 14, ".bosym v0 entry prefix is 14 bytes (8 sym_id + 1 kind + 3 reserved + 2 name_len)");

const _: () = assert!(SHA16_SIZE == 16 && TRAILER_SIZE == 32);

const _: () = assert!(ENTRY_MIN_SIZE == 30 && ENTRY_MAX_SIZE == 65_565);

const _: () = assert!(MAX_NAME_LEN == u16::MAX as usize);

const _: () = assert!(
    size_of::<BosymHeader>() == HEADER_SIZE,
    "repr(C) header layout must equal the 16-byte wire header exactly"
);

const _: () = assert!(align_of::<BosymHeader>() == 4, "header alignment is pinned at 4 (largest field u32)");

const _: () = assert!(offset_of!(BosymHeader, magic) == 0 && offset_of!(BosymHeader, version) == 8 && offset_of!(BosymHeader, flags) == 10 && offset_of!(BosymHeader, entry_count) == 12);

const _: () = assert!(
    size_of::<BosymEntryPrefix>() == ENTRY_PREFIX_SIZE,
    "packed prefix layout must equal the 14-byte wire prefix exactly"
);

const _: () = assert!(align_of::<BosymEntryPrefix>() == 1, "packed prefix has byte alignment on the wire");

const _: () = assert!(offset_of!(BosymEntryPrefix, sym_id) == 0 && offset_of!(BosymEntryPrefix, kind) == 8 && offset_of!(BosymEntryPrefix, reserved) == 9 && offset_of!(BosymEntryPrefix, name_len) == 12);

const _: () = assert!(size_of::<SymbolKind>() == 1, "SymbolKind must stay one wire byte");

const _: () = assert!(SymbolKind::ENTITY == SymbolKind::Entity as u8 && SymbolKind::RELATION == SymbolKind::Relation as u8 && SymbolKind::IRI == SymbolKind::Iri as u8 && SymbolKind::LABEL == SymbolKind::Label as u8 && SymbolKind::ANNOTATION == SymbolKind::Annotation as u8);

const _: () = assert!(SymbolKind::COUNT == 5 && SymbolKind::MAX == 4);

const _: () = assert!(FLAG_COMPRESSED != 0 && FLAG_SIGNED != 0 && FLAG_COMPRESSED != FLAG_SIGNED);

const _: () = assert!(FLAGS_V0_MASK == 0b11);

// ---------------------------------------------------------------------------
// SHA-256 (FIPS 180-4). std-only: cgap-bosym's Cargo.toml is outside this
// lane's file ownership, so the codec carries its own digest instead of a
// sha2/blake3 dependency. Verified against published vectors in tests.
// ---------------------------------------------------------------------------

/// Round constants K (FIPS 180-4 §4.2.3, first 64 primes).
const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Incremental SHA-256 hasher (FIPS 180-4).
#[derive(Debug, Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// New hasher at the FIPS 180-4 §5.3.3 initial state.
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    /// Absorb bytes.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len = self.total_len.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    /// Finish and return the 32-byte digest.
    pub fn finalize(mut self) -> Sha256Digest {
        let bit_len = self.total_len.wrapping_mul(8);
        self.update(&[0x80]);
        self.total_len = self.total_len.wrapping_sub(1);
        while self.buf_len != 56 {
            self.update(&[0x00]);
            self.total_len = self.total_len.wrapping_sub(1);
        }
        // Re-length: exactly the 8 length bytes remain; feed them without
        // the counter arithmetic above (padding bookkeeping only).
        let mut block = [0u8; 64];
        block[..56].copy_from_slice(&self.buf[..56]);
        block[56..].copy_from_slice(&bit_len.to_be_bytes());
        self.compress(&block);
        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA_K[i])
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
        let round = [a, b, c, d, e, f, g, h];
        for (s, r) in self.state.iter_mut().zip(round.iter()) {
            *s = s.wrapping_add(*r);
        }
    }
}

/// One-shot SHA-256.
pub fn sha256(data: &[u8]) -> Sha256Digest {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// `sha16` over a name: first [`SHA16_SIZE`] bytes of SHA-256 (lane doc
/// lines 157-158).
pub fn sha16(name: &[u8]) -> [u8; SHA16_SIZE] {
    let d = sha256(name);
    let mut out = [0u8; SHA16_SIZE];
    out.copy_from_slice(&d[..SHA16_SIZE]);
    out
}

// ---------------------------------------------------------------------------
// Typed errors — the reader fails closed on every malformed input class.
// ---------------------------------------------------------------------------

/// Every way a `.bosym` v0 archive can fail to parse. The reader never
/// returns partial results (lane doc T2: "every truncation must fail
/// closed on read with a typed error, never a partial result").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BosymError {
    /// Byte length is shorter than the fixed framing requires.
    Truncated {
        /// Offset where the deficit was detected.
        offset: usize,
        /// Bytes available at `offset`.
        have: usize,
        /// Bytes required at `offset`.
        need: usize,
    },
    /// Leading 8 bytes are not [`MAGIC`].
    BadMagic,
    /// Version outside this reader's support (v0 only).
    UnsupportedVersion(u16),
    /// v0-forbidden flag bits set (compression/signing are version bumps).
    UnsupportedFlags(u16),
    /// An entry's 3 reserved bytes are non-zero (lane doc line 152).
    ReservedNonZero {
        /// Entry index.
        index: usize,
    },
    /// An entry's kind byte is not a defined [`SymbolKind`].
    InvalidKind {
        /// Entry index.
        index: usize,
        /// The offending kind byte.
        kind: u8,
    },
    /// An entry's name field is not valid UTF-8 (format requirement, lane
    /// doc line 154: "name: UTF-8 bytes").
    NameNotUtf8 {
        /// Entry index.
        index: usize,
    },
    /// An entry's trailing `sha16` does not match SHA-256(name)[0..16].
    Sha16Mismatch {
        /// Entry index.
        index: usize,
    },
    /// The trailing SHA-256 over all preceding bytes does not match.
    TrailerMismatch,
    /// Bytes exist after the declared entry set (before the trailer).
    TrailingGarbage {
        /// Extra byte count between the last entry and the trailer.
        extra: usize,
    },
    /// The entry records ran out before `entry_count` was reached.
    EntryCountMismatch {
        /// Declared `entry_count`.
        declared: u32,
        /// Entries actually decodable.
        found: usize,
    },
    /// A sym_id appears twice (an interned-ID collision breaks the
    /// ID → symbol map the sidecar serves).
    DuplicateSymId(u64),
    /// Builder given a name longer than `name_len`'s u16 width.
    NameTooLong(usize),
    /// Graph-export source unreadable (generator input path).
    GraphExportRead(String),
}

impl fmt::Display for BosymError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BosymError::Truncated { offset, have, need } => {
                write!(f, "truncated at offset {offset}: have {have}, need {need}")
            }
            BosymError::BadMagic => write!(f, "bad magic (not CGAPBSYM)"),
            BosymError::UnsupportedVersion(v) => write!(f, "unsupported version {v} (v0 reader)"),
            BosymError::UnsupportedFlags(v) => {
                write!(f, "unsupported flags 0x{v:04x} (v0 requires all flag bits zero)")
            }
            BosymError::ReservedNonZero { index } => {
                write!(f, "entry {index}: reserved bytes non-zero")
            }
            BosymError::InvalidKind { index, kind } => write!(f, "entry {index}: invalid kind {kind}"),
            BosymError::NameNotUtf8 { index } => write!(f, "entry {index}: name is not UTF-8"),
            BosymError::Sha16Mismatch { index } => write!(f, "entry {index}: sha16 mismatch"),
            BosymError::TrailerMismatch => write!(f, "trailing sha256 mismatch"),
            BosymError::TrailingGarbage { extra } => write!(f, "{extra} trailing bytes before sha256"),
            BosymError::EntryCountMismatch { declared, found } => {
                write!(f, "entry_count {declared} but decoded {found} entries")
            }
            BosymError::DuplicateSymId(id) => write!(f, "duplicate sym_id 0x{id:016x}"),
            BosymError::NameTooLong(n) => write!(f, "name length {n} exceeds u16 name_len"),
            BosymError::GraphExportRead(p) => write!(f, "graph export unreadable: {p}"),
        }
    }
}

impl std::error::Error for BosymError {}

// ---------------------------------------------------------------------------
// Entries, builder, reader.
// ---------------------------------------------------------------------------

/// One vocabulary entry: the interned ID, its kind, and the human symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BosymEntry {
    /// The interned ID the kernel uses (lane doc line 150).
    pub sym_id: u64,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Human-readable symbol (UTF-8; may embed NULs — length-prefixed by
    /// design, lane doc lines 164-166).
    pub name: String,
}

impl BosymEntry {
    /// Wire size of this entry: fixed prefix + name + sha16.
    pub fn wire_size(&self) -> usize {
        ENTRY_PREFIX_SIZE + self.name.len() + SHA16_SIZE
    }

    /// Encode this entry to bytes (prefix + name + sha16).
    pub fn encode(&self) -> Result<Vec<u8>, BosymError> {
        let name = self.name.as_bytes();
        if name.len() > MAX_NAME_LEN {
            return Err(BosymError::NameTooLong(name.len()));
        }
        let mut out = Vec::with_capacity(self.wire_size());
        out.extend_from_slice(&self.sym_id.to_le_bytes());
        out.push(self.kind as u8);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&sha16(name));
        Ok(out)
    }
}

/// Canonical byte form of a v0 archive: header + entries + trailing
/// SHA-256 over all preceding bytes. Entries are emitted in the order
/// given (the generator canonicalizes by sorting; see [`GraphExport`]).
pub fn build_archive(entries: &[BosymEntry]) -> Result<Vec<u8>, BosymError> {
    if entries.len() > u32::MAX as usize {
        // entry_count is u32; a Vec that large is unreachable on this
        // hardware, but the bound is stated, not assumed.
        return Err(BosymError::EntryCountMismatch {
            declared: u32::MAX,
            found: entries.len(),
        });
    }
    let mut body = Vec::new();
    body.extend_from_slice(&MAGIC);
    body.extend_from_slice(&VERSION.to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes()); // flags: all zero in v0
    body.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for e in entries {
        body.extend_from_slice(&e.encode()?);
    }
    let digest = sha256(&body);
    body.extend_from_slice(&digest);
    Ok(body)
}

/// Strict, fail-closed parse of a v0 archive. Any deviation from the frame
/// is a typed error; no partial results.
pub fn parse_archive(bytes: &[u8]) -> Result<Vec<BosymEntry>, BosymError> {
    if bytes.len() < HEADER_SIZE + TRAILER_SIZE {
        return Err(BosymError::Truncated {
            offset: 0,
            have: bytes.len(),
            need: HEADER_SIZE + TRAILER_SIZE,
        });
    }
    if bytes[..MAGIC_LEN] != MAGIC {
        return Err(BosymError::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != VERSION {
        return Err(BosymError::UnsupportedVersion(version));
    }
    let flags = u16::from_le_bytes([bytes[10], bytes[11]]);
    if flags & FLAGS_V0_MASK != 0 {
        return Err(BosymError::UnsupportedFlags(flags));
    }
    let entry_count = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let trailer_at = bytes.len() - TRAILER_SIZE;
    let expected = sha256(&bytes[..trailer_at]);
    if bytes[trailer_at..] != expected {
        return Err(BosymError::TrailerMismatch);
    }
    let body = &bytes[HEADER_SIZE..trailer_at];
    let mut entries = Vec::with_capacity(entry_count.min(4096));
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut pos = 0usize;
    for index in 0..entry_count {
        if body.len() - pos < ENTRY_PREFIX_SIZE {
            return Err(BosymError::EntryCountMismatch {
                declared: entry_count as u32,
                found: index,
            });
        }
        let p = &body[pos..pos + ENTRY_PREFIX_SIZE];
        let sym_id = u64::from_le_bytes(p[0..8].try_into().expect("8 bytes"));
        let kind = p[8];
        if p[9] != 0 || p[10] != 0 || p[11] != 0 {
            return Err(BosymError::ReservedNonZero { index });
        }
        let name_len = u16::from_le_bytes([p[12], p[13]]) as usize;
        pos += ENTRY_PREFIX_SIZE;
        if body.len() - pos < name_len + SHA16_SIZE {
            return Err(BosymError::Truncated {
                offset: HEADER_SIZE + pos,
                have: body.len() - pos,
                need: name_len + SHA16_SIZE,
            });
        }
        let kind = match kind {
            SymbolKind::ENTITY => SymbolKind::Entity,
            SymbolKind::RELATION => SymbolKind::Relation,
            SymbolKind::IRI => SymbolKind::Iri,
            SymbolKind::LABEL => SymbolKind::Label,
            SymbolKind::ANNOTATION => SymbolKind::Annotation,
            other => return Err(BosymError::InvalidKind { index, kind: other }),
        };
        let name_bytes = &body[pos..pos + name_len];
        pos += name_len;
        let sha = &body[pos..pos + SHA16_SIZE];
        pos += SHA16_SIZE;
        if sha != &sha16(name_bytes) {
            return Err(BosymError::Sha16Mismatch { index });
        }
        let name = match std::str::from_utf8(name_bytes) {
            Ok(s) => s.to_string(),
            Err(_) => return Err(BosymError::NameNotUtf8 { index }),
        };
        if !seen.insert(sym_id) {
            return Err(BosymError::DuplicateSymId(sym_id));
        }
        entries.push(BosymEntry { sym_id, kind, name });
    }
    if pos != body.len() {
        return Err(BosymError::TrailingGarbage {
            extra: body.len() - pos,
        });
    }
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Generator (DoD 1): graph export in -> .bosym archive + string-free
// projection out.
// ---------------------------------------------------------------------------

/// The ontology surface the generator consumes: an ordered, deduplicated
/// set of entries. Canonical form: sorted by `sym_id`, duplicate IDs
/// refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphExport {
    entries: Vec<BosymEntry>,
}

impl GraphExport {
    /// Canonicalize an export: sort by `sym_id`, refuse duplicate IDs.
    pub fn new(mut entries: Vec<BosymEntry>) -> Result<Self, BosymError> {
        entries.sort_by_key(|e| e.sym_id);
        for w in entries.windows(2) {
            if w[0].sym_id == w[1].sym_id {
                return Err(BosymError::DuplicateSymId(w[0].sym_id));
            }
        }
        Ok(Self { entries })
    }

    /// Canonical entries (sorted by `sym_id`).
    pub fn entries(&self) -> &[BosymEntry] {
        &self.entries
    }

    /// Entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the export carries no symbols.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Render a symbol name into a stable Rust identifier fragment for the
/// string-free projection: non-alphanumerics map to `_`, uppercase.
/// Deterministic and injective enough for projections; the authoritative
/// name mapping lives in the `.bosym` archive, never in the projection.
fn ident_fragment(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out
}

/// Emit the string-free projection: a Rust source file whose only content
/// is (a) a generator header naming [`GENERATOR_NAME`] and the entry count
/// and (b) one `pub const` u64 per symbol. No ontology UTF-8 appears in
/// the projection: symbol names contribute only uppercased identifier
/// fragments (`:` → `_`), never string literals — ADR-002's "numbers and
/// codes" surface. The projection is generated output: never hand-edited.
pub fn emit_projection(export: &GraphExport, for_crate: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Generated by {} v{} — DO NOT EDIT (generated artifact, ADR-002).\n",
        GENERATOR_NAME, GENERATOR_VERSION
    ));
    out.push_str(&format!(
        "// String-free projection of {} symbol(s) for crate `{}`.\n",
        export.len(),
        for_crate
    ));
    out.push_str("// Names are out-of-band in the .bosym archive; this file carries interned IDs only.\n\n");
    for e in export.entries() {
        out.push_str(&format!(
            "/// Interned ID for the out-of-band symbol at index {} (kind {}). See .bosym.\n",
            index_of(export, e.sym_id),
            e.kind as u8
        ));
        out.push_str(&format!(
            "pub const SYM_{}: u64 = 0x{:016X};\n\n",
            ident_fragment(&e.name),
            e.sym_id
        ));
    }
    out
}

fn index_of(export: &GraphExport, sym_id: u64) -> usize {
    export
        .entries()
        .iter()
        .position(|e| e.sym_id == sym_id)
        .unwrap_or(usize::MAX)
}

/// Extract the `fibo:`-family ontology tokens from an ontology facade
/// source (the in-repo "graph export" — the ontology layer here is Rust
/// facade crates, `ontologies/*/src/lib.rs`). Token rule: the literal
/// `fibo:` followed by `A-Za-z0-9_`/`:` characters. This is the same
/// derivation the zero-string gate performs pre-archive (lane doc lines
/// 217-219); it lives here so the generator and the gate consume one rule.
pub fn extract_fibo_tokens(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i + 5 <= bytes.len() {
        if &bytes[i..i + 5] == b"fibo:" {
            // Consume `:segment` groups; a segment is 1+ alnum/underscore.
            let mut j = i + 5;
            let mut last_good = i + 5;
            loop {
                let seg_start = j;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if j == seg_start {
                    break;
                }
                last_good = j;
                if j < bytes.len() && bytes[j] == b':' {
                    j += 1;
                } else {
                    break;
                }
            }
            if last_good > i + 5 {
                out.push(source[i..last_good].to_string());
                i = last_good;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Build the graph export for the in-repo FIBO facade: read
/// `ontologies/fibo/src/lib.rs` (path resolved from this crate's manifest
/// dir), extract ontology tokens, mint PROVISIONAL sym_ids as the low 64
/// bits of SHA-256 over `b"bosym\0" ++ token`.
///
/// PROVISIONAL-ID divergence (recorded, not silent): the kernel's
/// authoritative interned IDs are A06 SlotIds
/// (`u64::from_le_bytes(BLAKE3(b"bont\x01" ++ lexeme)[0..8])`); cgap-bosym
/// cannot depend on cgap-blake3 without editing Cargo.toml, which is
/// outside this lane's file ownership. The .bosym sym_id field is
/// therefore carried as a SHA-256-derived provisional value and MUST be
/// re-minted through the A06 derivation before any kernel-side consumer
/// treats it as a SlotId. Name-based gate matching does not read sym_id.
pub fn fibo_graph_export(fibo_lib_rs: &std::path::Path) -> Result<GraphExport, BosymError> {
    let source = std::fs::read_to_string(fibo_lib_rs).map_err(|e| {
        BosymError::GraphExportRead(format!("{}: {e}", fibo_lib_rs.display()))
    })?;
    let tokens = extract_fibo_tokens(&source);
    let entries = tokens
        .into_iter()
        .map(|name| {
            let mut pre = b"bosym\0".to_vec();
            pre.extend_from_slice(name.as_bytes());
            let d = sha256(&pre);
            let mut le = [0u8; 8];
            le.copy_from_slice(&d[..8]);
            BosymEntry {
                sym_id: u64::from_le_bytes(le),
                kind: SymbolKind::Iri,
                name,
            }
        })
        .collect();
    GraphExport::new(entries)
}

// ---------------------------------------------------------------------------
// Courts (Chicago, real collaborators, assert final state). T1-T3 of the
// lane doc's test plan live here; T4-T12 live in the gate's own court
// suite (scripts/zero_string_gate.py --courts).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_entries() -> Vec<BosymEntry> {
        vec![
            BosymEntry {
                sym_id: 0xE508EC39DB7ECC28,
                kind: SymbolKind::Iri,
                name: "fibo:FndUtlCur:USD".to_string(),
            },
            BosymEntry {
                sym_id: 0x5BE61A3EB4B24583,
                kind: SymbolKind::Entity,
                name: "fibo:SecDbt:Bond\0alias-with-NUL".to_string(),
            },
            BosymEntry {
                sym_id: 0xD558F6825C8F0108,
                kind: SymbolKind::Label,
                name: "fibo:ClAcct:Cr\u{e9}dit-☕-non-ascii".to_string(),
            },
        ]
    }

    /// T1 Round-trip: byte-exact framing and entry-for-entry equality,
    /// including embedded-NUL and non-ASCII names (lane doc lines 313-315).
    #[test]
    fn t1_round_trip_byte_exact() {
        let entries = fixture_entries();
        let bytes = build_archive(&entries).expect("build");
        assert_eq!(bytes.len(), HEADER_SIZE + TRAILER_SIZE + entries.iter().map(|e| e.wire_size()).sum::<usize>());
        let parsed = parse_archive(&bytes).expect("parse");
        assert_eq!(parsed.len(), entries.len());
        for (a, b) in parsed.iter().zip(entries.iter()) {
            assert_eq!(a.sym_id, b.sym_id);
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.name, b.name);
        }
        // Byte-exact re-encode: canonical form is stable.
        let rebuilt = build_archive(&parsed).expect("rebuild");
        assert_eq!(rebuilt, bytes, "re-encode must be byte-identical");
        // Empty-name entry round-trips too (format carries it faithfully).
        let with_empty = vec![BosymEntry {
            sym_id: 1,
            kind: SymbolKind::Annotation,
            name: String::new(),
        }];
        let b2 = build_archive(&with_empty).expect("build empty-name");
        let p2 = parse_archive(&b2).expect("parse empty-name");
        assert_eq!(p2[0].name, "");
    }

    /// T2 Truncation: truncate at EVERY byte offset; every truncation must
    /// fail closed with a typed error, never a partial result (lane doc
    /// lines 316-318).
    #[test]
    fn t2_truncation_fails_closed_at_every_offset() {
        let entries = fixture_entries();
        let bytes = build_archive(&entries).expect("build");
        for cut in 0..bytes.len() {
            let truncated = &bytes[..cut];
            let err = parse_archive(truncated)
                .err()
                .unwrap_or_else(|| panic!("truncation at {cut} parsed OK — gate-leak"));
            // Every failure must be a typed BosymError, not a panic — the
            // assert above already proves Err; Display renders it.
            let _ = err.to_string();
        }
        assert!(parse_archive(&bytes).is_ok());
    }

    /// T3 Corrupt-tail: flip one bit in the trailing sha256; read must
    /// refuse (lane doc lines 319-320). Strengthened: flip one bit in
    /// EVERY byte of the archive — every single-byte mutation must be
    /// refused (the trailer covers all preceding bytes).
    #[test]
    fn t3_bit_flip_anywhere_is_refused() {
        let entries = fixture_entries();
        let bytes = build_archive(&entries).expect("build");
        for byte_idx in 0..bytes.len() {
            for bit in 0..8 {
                let mut mutated = bytes.clone();
                mutated[byte_idx] ^= 1 << bit;
                assert!(
                    parse_archive(&mutated).is_err(),
                    "single-bit flip at byte {byte_idx} bit {bit} was accepted"
                );
            }
        }
    }

    /// SHA-256 correctness against FIPS 180-4 published vectors.
    #[test]
    fn sha256_fips_vectors() {
        let empty = sha256(b"");
        assert_eq!(
            empty,
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55
            ]
        );
        let abc = sha256(b"abc");
        assert_eq!(
            &abc[..8],
            &[0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea]
        );
        assert_eq!(
            &abc[8..16],
            &[0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23]
        );
        assert_eq!(
            &abc[16..24],
            &[0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c]
        );
        assert_eq!(
            &abc[24..],
            &[0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad]
        );
        // Multi-block input (spans >1 compression block), incremental path.
        let long = vec![0x61u8; 200];
        let mut h = Sha256::new();
        h.update(&long[..70]);
        h.update(&long[70..]);
        assert_eq!(h.finalize(), sha256(&long));

        // WAVE-5 (edge-length sweep): finalize()'s two-phase padding loop
        // has its edges exactly at buffered lengths 55..=64 and the next
        // block boundary; the vectors above cover only 0 / 3 / 200. These
        // expected digests were derived from an independent implementation
        // (python3 hashlib) this session, then pinned — never from this
        // implementation's own output.
        let edge_digests: [(&usize, &str); 8] = [
            (&55, "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"),
            (&56, "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"),
            (&57, "f13b2d724659eb3bf47f2dd6af1accc87b81f09f59f2b75e5c0bed6589dfe8c6"),
            (&63, "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34"),
            (&64, "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"),
            (&119, "31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb"),
            (&120, "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c"),
            (&200, "c2a908d98f5df987ade41b5fce213067efbcc21ef2240212a41e54b5e7c28ae5"),
        ];
        for (len, hex) in edge_digests {
            let msg = vec![0x61u8; *len];
            let expect = decode_digest(hex);
            assert_eq!(
                sha256(&msg),
                expect,
                "sha256 of {}×'a' diverged from the independent digest",
                len
            );
            // Same lengths through the buffered incremental path (7-byte
            // updates exercise every buf_len residue class).
            let mut h = Sha256::new();
            for chunk in msg.chunks(7) {
                h.update(chunk);
            }
            assert_eq!(h.finalize(), expect, "chunked sha256 of len {len} diverged");
        }
        // NIST two-block message ("abcdbcdecdefdefgefghfghighijhijkijkljklm
        // klmnlmnomnopnopq", 56 bytes — the canonical padding-boundary
        // vector): digest derived from hashlib, pinned.
        let nist = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
        assert_eq!(nist.len(), 56);
        assert_eq!(
            sha256(nist),
            decode_digest("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"),
            "NIST 56-byte vector diverged"
        );
    }

    /// Hex → 32-byte digest (test-side decode of the independently derived
    /// pinned vectors; panics loudly on a malformed pin).
    fn decode_digest(hex: &str) -> Sha256Digest {
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("valid hex digest pin");
        }
        out
    }

    /// Generator: deterministic, order-canonicalizing, content-sensitive.
    #[test]
    fn generator_is_deterministic_and_canonical() {
        let a = GraphExport::new(fixture_entries()).expect("export");
        let mut shuffled = fixture_entries();
        shuffled.reverse();
        let b = GraphExport::new(shuffled).expect("export reversed");
        let ba = build_archive(a.entries()).expect("build a");
        let bb = build_archive(b.entries()).expect("build b");
        assert_eq!(ba, bb, "entry order must not change archive bytes");
        let mut tweaked = fixture_entries();
        tweaked[0].name.push('X');
        let c = GraphExport::new(tweaked).expect("export tweaked");
        let bc = build_archive(c.entries()).expect("build c");
        assert_ne!(ba, bc, "content change must change archive bytes");
    }

    /// Duplicate sym_id is refused at export construction and at parse.
    #[test]
    fn duplicate_sym_id_refused() {
        let dup = vec![
            BosymEntry {
                sym_id: 7,
                kind: SymbolKind::Entity,
                name: "a".into(),
            },
            BosymEntry {
                sym_id: 7,
                kind: SymbolKind::Entity,
                name: "b".into(),
            },
        ];
        assert_eq!(
            GraphExport::new(dup.clone()).err(),
            Some(BosymError::DuplicateSymId(7))
        );
        // Parser-level: two entries with the same ID, correctly framed and
        // digested (build a raw archive by hand).
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&MAGIC);
        body.extend_from_slice(&VERSION.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&2u32.to_le_bytes());
        for name in ["a", "b"] {
            body.extend_from_slice(&7u64.to_le_bytes());
            body.push(SymbolKind::ENTITY);
            body.extend_from_slice(&[0; 3]);
            body.extend_from_slice(&1u16.to_le_bytes());
            body.extend_from_slice(name.as_bytes());
            body.extend_from_slice(&sha16(name.as_bytes()));
        }
        let digest = sha256(&body);
        body.extend_from_slice(&digest);
        assert_eq!(
            parse_archive(&body).err(),
            Some(BosymError::DuplicateSymId(7))
        );
    }

    /// Typed reader refusals: bad magic, bad version, forbidden flags,
    /// non-zero reserved, invalid kind, non-UTF-8 name, sha16 mismatch.
    #[test]
    fn reader_typed_refusals() {
        let entries = fixture_entries();
        let base = build_archive(&entries).expect("build");

        let mut bad_magic = base.clone();
        bad_magic[0] = b'X';
        assert_eq!(parse_archive(&bad_magic).err(), Some(BosymError::BadMagic));

        let mut bad_ver = base.clone();
        bad_ver[8] = 1;
        assert_eq!(
            parse_archive(&bad_ver).err(),
            Some(BosymError::UnsupportedVersion(1))
        );

        let mut bad_flags = base.clone();
        bad_flags[10] = 1; // FLAG_COMPRESSED set in v0
        assert_eq!(
            parse_archive(&bad_flags).err(),
            Some(BosymError::UnsupportedFlags(1))
        );

        // Reserved byte: rebuild by hand with reserved[0] = 1 and a valid
        // trailer so the refusal is the typed ReservedNonZero, not a
        // trailer mismatch.
        let mut hand: Vec<u8> = Vec::new();
        hand.extend_from_slice(&MAGIC);
        hand.extend_from_slice(&VERSION.to_le_bytes());
        hand.extend_from_slice(&0u16.to_le_bytes());
        hand.extend_from_slice(&1u32.to_le_bytes());
        hand.extend_from_slice(&1u64.to_le_bytes());
        hand.push(SymbolKind::ENTITY);
        hand.extend_from_slice(&[1, 0, 0]); // reserved[0] non-zero
        hand.extend_from_slice(&1u16.to_le_bytes());
        hand.extend_from_slice(b"a");
        hand.extend_from_slice(&sha16(b"a"));
        let digest = sha256(&hand);
        hand.extend_from_slice(&digest);
        assert_eq!(
            parse_archive(&hand).err(),
            Some(BosymError::ReservedNonZero { index: 0 })
        );

        // Invalid kind: same hand-built shape with kind = 9.
        let mut hand9: Vec<u8> = Vec::new();
        hand9.extend_from_slice(&MAGIC);
        hand9.extend_from_slice(&VERSION.to_le_bytes());
        hand9.extend_from_slice(&0u16.to_le_bytes());
        hand9.extend_from_slice(&1u32.to_le_bytes());
        hand9.extend_from_slice(&1u64.to_le_bytes());
        hand9.push(9);
        hand9.extend_from_slice(&[0, 0, 0]);
        hand9.extend_from_slice(&1u16.to_le_bytes());
        hand9.extend_from_slice(b"a");
        hand9.extend_from_slice(&sha16(b"a"));
        let digest = sha256(&hand9);
        hand9.extend_from_slice(&digest);
        assert_eq!(
            parse_archive(&hand9).err(),
            Some(BosymError::InvalidKind { index: 0, kind: 9 })
        );

        // Non-UTF-8 name: entry framed over raw invalid bytes, digest
        // recomputed over the raw body so the typed error is NameNotUtf8.
        let name: &[u8] = &[0x66, 0x69, 0x62, 0x6f, 0x3a, 0xFF, 0xFE];
        let mut hand_bad: Vec<u8> = Vec::new();
        hand_bad.extend_from_slice(&MAGIC);
        hand_bad.extend_from_slice(&VERSION.to_le_bytes());
        hand_bad.extend_from_slice(&0u16.to_le_bytes());
        hand_bad.extend_from_slice(&1u32.to_le_bytes());
        hand_bad.extend_from_slice(&1u64.to_le_bytes());
        hand_bad.push(SymbolKind::IRI);
        hand_bad.extend_from_slice(&[0, 0, 0]);
        hand_bad.extend_from_slice(&(name.len() as u16).to_le_bytes());
        hand_bad.extend_from_slice(name);
        hand_bad.extend_from_slice(&sha16(name));
        let digest = sha256(&hand_bad);
        hand_bad.extend_from_slice(&digest);
        assert_eq!(
            parse_archive(&hand_bad).err(),
            Some(BosymError::NameNotUtf8 { index: 0 })
        );

        // sha16 mismatch: entry framed with a wrong sha16 but a valid
        // archive trailer.
        let mut hand_sha: Vec<u8> = Vec::new();
        hand_sha.extend_from_slice(&MAGIC);
        hand_sha.extend_from_slice(&VERSION.to_le_bytes());
        hand_sha.extend_from_slice(&0u16.to_le_bytes());
        hand_sha.extend_from_slice(&1u32.to_le_bytes());
        hand_sha.extend_from_slice(&1u64.to_le_bytes());
        hand_sha.push(SymbolKind::IRI);
        hand_sha.extend_from_slice(&[0, 0, 0]);
        hand_sha.extend_from_slice(&1u16.to_le_bytes());
        hand_sha.extend_from_slice(b"a");
        hand_sha.extend_from_slice(&[0u8; SHA16_SIZE]);
        let digest = sha256(&hand_sha);
        hand_sha.extend_from_slice(&digest);
        assert_eq!(
            parse_archive(&hand_sha).err(),
            Some(BosymError::Sha16Mismatch { index: 0 })
        );

        // Trailing garbage between last entry and trailer.
        let mut garbage = build_archive(&entries).expect("build");
        let cut = garbage.len() - TRAILER_SIZE;
        garbage.insert(cut, 0xAB);
        let _ = parse_archive(&garbage); // trailer mismatch or garbage — either refusal is fail-closed
        assert!(parse_archive(&garbage).is_err());
    }

    /// Projection emitter (DoD 1): header names the generator; no ontology
    /// UTF-8 string appears in the projection text (the raw name token is
    /// absent; only uppercased identifier fragments remain).
    #[test]
    fn projection_is_string_free_and_self_naming() {
        let export = GraphExport::new(fixture_entries()).expect("export");
        let proj = emit_projection(&export, "cgap-core");
        assert!(
            proj.contains(&format!("Generated by {GENERATOR_NAME}")),
            "generated file must name its generator (DoD 1)"
        );
        assert!(proj.contains(GENERATOR_VERSION));
        for e in export.entries() {
            assert!(
                !proj.contains(&e.name),
                "ontology string {:?} leaked into projection",
                e.name
            );
        }
        assert!(proj.contains("pub const SYM_FIBO_FNDUTLCUR_USD: u64 = 0xE508EC39DB7ECC28;"));
        // Projection compiles as Rust when wrapped in a lib? That is the
        // consumer crate's job; here we assert the surface shape only.
        assert!(proj.matches("pub const SYM_").count() == export.len());
    }

    /// `fibo:` token extraction: the shared derivation rule used by both
    /// the generator and the gate.
    #[test]
    fn fibo_token_extraction() {
        let src = r#"
            /// `fibo:SecDbt:CorporateBond`
            pub const CLASS_CORP_BOND: u32 = 0x1;
            // fibo:FndUtlCur:USD in a comment
            let x = "fibo:FndUtlCur:EUR"; // fibo:FndUtlCur:JPY
            fibo:notasegment fibo::doublecolon fibo:
            "#;
        let tokens = extract_fibo_tokens(src);
        assert!(tokens.contains(&"fibo:SecDbt:CorporateBond".to_string()));
        assert!(tokens.contains(&"fibo:FndUtlCur:USD".to_string()));
        assert!(tokens.contains(&"fibo:FndUtlCur:EUR".to_string()));
        assert!(tokens.contains(&"fibo:FndUtlCur:JPY".to_string()));
        // Degenerate forms are not tokens: bare prefix and empty segment.
        assert!(!tokens.iter().any(|t| t == "fibo:" || t.contains("::")));
        assert_eq!(tokens.len(), 5, "deduped token set: {tokens:?}");
    }

    /// Env-gated emit-to-disk court: when `CGAP_BOSYM_EMIT` names a
    /// writable directory, the REAL archive consumed by the gate's
    /// workspace scan is written by THIS Rust generator — the gate then
    /// parses Rust-produced bytes (cross-implementation witness). Skips
    /// loud when unset.
    #[test]
    fn emit_real_vocabulary_archive_when_gated() {
        let Some(dir) = std::env::var_os("CGAP_BOSYM_EMIT") else {
            eprintln!("SELF-CHECK [SKIP] CGAP_BOSYM_EMIT unset — emit court not requested");
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let fibo_lib = manifest
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("ontologies/fibo/src/lib.rs"))
            .expect("crate two levels under repo root");
        let export = fibo_graph_export(&fibo_lib).expect("graph export");
        assert!(export.len() >= 8, "expected the real FIBO vocabulary, got {}", export.len());
        let bytes = build_archive(export.entries()).expect("build");
        std::fs::write(dir.join("symbols.bosym"), &bytes).expect("write archive");
        // Reader over what we wrote: entry-for-entry identity.
        let read_back = parse_archive(&bytes).expect("parse back");
        assert_eq!(read_back.len(), export.len());
        eprintln!("EMITTED symbols.bosym entries={} bytes={}", export.len(), bytes.len());
    }
}
