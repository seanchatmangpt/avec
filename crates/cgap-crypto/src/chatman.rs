//! Receipt tier: the 112-byte `DeltaS` and the sealed `Receipt` (lane A16).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A16-chatman-pqc.md` §3.1 (`DeltaS`) and
//! §3.3 (`R_receipt`). Wave 1 froze the ABI (structs, consts, layout pins);
//! Wave 2 (this file) lands the core engines of §3.3/§6: the keyed BLAKE3
//! digest, the `DeltaS` codec, the `Sealer` trait seam with the loudly-typed
//! `Unsealed` impl (§4 UNSUPPORTED clause), the `UnsealedReceipt` tier with
//! its `seal_unsealed`/`verify_unsealed` engine, and the `admissions` Merkle
//! root. The PQC (ML-DSA-65) seal itself stays Wave 3, gated on §10's
//! no_std-PQC risk; the frozen `Receipt` shape below is that Wave-3 target.
//!
//! Two Wave-1 spec reconciliations (divergences recorded in the lane
//! receipt):
//! 1. §3.3 states "Receipt size: 117 bytes + seal size"; 117 counts only
//!    `magic(4) + version(1) + DeltaS(112)` and omits `Phi_lease(80)` and
//!    `blake3_digest(32)`. The normative concatenation
//!    `magic || version || DeltaS || Phi_lease || blake3_digest || pqc_seal`
//!    fixes the constant prefix at `4+1+112+80+32 = 229`; this module pins
//!    `229 + 2420 = 2649` for the ML-DSA-65 shape (§4).
//! 2. §3.3's `algorithm_id` "2-byte code" names no registry values; the two
//!    `ALGORITHM_ID_*` constants pin provisional codes (§10 FIPS-drift risk
//!    calls for a pinned id) pending lane-court ratification.

#![allow(dead_code)] // Wave-1 ML-DSA-shape consts wait on the Wave-3 PQC gate.

use crate::lease::{Lease, PhiLease, PHI_LEASE_LEN};
use cgap_blake3::{compress_scalar, hash, CHUNK_END, CHUNK_START, ROOT};

/// `domain_tag`: 8 bytes, ASCII `"CGAPDLT1"` — versioned domain separator
/// preventing cross-protocol signature reuse (§3.1).
pub const DELTA_DOMAIN_TAG: [u8; 8] = *b"CGAPDLT1";

/// `from_root` / `to_root` / `admissions` width: 32-byte BLAKE3 digests
/// (§3.1).
pub const ROOT_LEN: usize = 32;
/// `op_count`: 8 bytes little-endian (§3.1).
pub const OP_COUNT_LEN: usize = 8;
/// Total `DeltaS_admitted` wire size: `8 + 32 + 32 + 8 + 32` bytes (§3.1).
pub const DELTA_LEN: usize = 112;

/// `magic`: 4 bytes `"CGRC"` (§3.3).
pub const RECEIPT_MAGIC: [u8; 4] = *b"CGRC";
/// `version`: 1 byte `0x01` (§3.3).
pub const RECEIPT_VERSION: u8 = 0x01;
/// `blake3_digest` width: 32 bytes (§3.3).
pub const DIGEST_LEN: usize = 32;

/// Constant (seal-independent) receipt prefix: `magic(4) + version(1) +
/// DeltaS(112) + Phi_lease(80) + blake3_digest(32)` bytes (§3.3; see the
/// module-level reconciliation of the doc's "117" figure).
pub const RECEIPT_FIXED_PREFIX_LEN: usize = 229;

/// ML-DSA-65 (FIPS 204) signature size in bytes: 2420 (§4 candidate table).
pub const SEAL_ML_DSA_65_LEN: usize = 2420;
/// Total receipt size for the ML-DSA-65 shape: `229 + 2420` bytes.
pub const RECEIPT_LEN: usize = 2649;

/// `algorithm_id` for ML-DSA-65 (§4 SELECT) — 2-byte code, PROVISIONAL
/// registry value (doc names none; pinned so old receipts stay interpretable
/// across FIPS 204 draft drift, §10).
pub const ALGORITHM_ID_ML_DSA_65: u16 = 0x0001;
/// `algorithm_id` for SLH-DSA-SHA2-128s (§4 FALLBACK) — 2-byte code,
/// PROVISIONAL registry value; no receipt type pins this shape yet.
pub const ALGORITHM_ID_SLH_DSA_SHA2_128S: u16 = 0x0002;

/// The admitted state delta: a canonical 112-byte string, not an abstraction
/// (§3.1).
///
/// `#[repr(C)]`, gap-free by construction: offsets 0, 8, 40, 72, 80; total
/// exactly 112 bytes; little-endian fields, no optional fields (§3.1,
/// "deterministic by construction").
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeltaS {
    /// offset 0, 8 B — ASCII `"CGAPDLT1"` (§3.1).
    pub domain_tag: [u8; 8],
    /// offset 8, 32 B — BLAKE3 digest of the prior state root (§3.1).
    pub from_root: [u8; ROOT_LEN],
    /// offset 40, 32 B — BLAKE3 digest of the new state root (§3.1).
    pub to_root: [u8; ROOT_LEN],
    /// offset 72, 8 B — u64 LE, admitted operations folded into the delta
    /// (§3.1).
    pub op_count: u64,
    /// offset 80, 32 B — BLAKE3 Merkle root over the ordered per-operation
    /// admission receipts; receipts chain (§3.1).
    pub admissions: [u8; ROOT_LEN],
}

/// The sealed receipt: `magic || version || DeltaS_admitted || Phi_lease ||
/// blake3_digest || pqc_seal` (§3.3).
///
/// `#[repr(C, packed)]` — the normative form is a byte concatenation with no
/// padding, and an unpacked `#[repr(C)]` struct would insert 3 pad bytes
/// after `version` (offset 5) to align the nested align-8 structs, violating
/// the §3.1 "deterministic by construction" one-to-one byte mirror. Wave-3
/// consequence, recorded now: field access on a packed struct must copy bytes
/// (`read`/by-value), never take field references.
///
/// Seal shape is frozen to ML-DSA-65 (§4 SELECT) for Wave 1; the SLH-DSA
/// fallback shape and the loudly-typed `Unsealed` receipt (§4 UNSUPPORTED
/// clause) are later-wave type decisions gated on the no_std PQC crate gate.
#[repr(C, packed)]
pub struct Receipt {
    /// offset 0, 4 B — `"CGRC"` (§3.3).
    pub magic: [u8; 4],
    /// offset 4, 1 B — `0x01` (§3.3).
    pub version: u8,
    /// offset 5, 112 B — the admitted delta (§3.3).
    pub delta: DeltaS,
    /// offset 117, 80 B — the lease epoch (§3.3).
    pub phi: PhiLease,
    /// offset 197, 32 B — BLAKE3 (keyed, key = `issued_root`) over
    /// `DeltaS || Phi_lease` (§3.3).
    pub blake3_digest: [u8; DIGEST_LEN],
    /// offset 229, 2420 B — ML-DSA-65 signature over
    /// `blake3_digest || algorithm_id` (§3.3, §4).
    pub pqc_seal: [u8; SEAL_ML_DSA_65_LEN],
}

// §3.1 layout pins for the 112-byte delta.
const _: () = assert!(core::mem::size_of::<DeltaS>() == DELTA_LEN);
const _: () = assert!(core::mem::align_of::<DeltaS>() == 8);
const _: () = assert!(core::mem::offset_of!(DeltaS, domain_tag) == 0);
const _: () = assert!(core::mem::offset_of!(DeltaS, from_root) == 8);
const _: () = assert!(core::mem::offset_of!(DeltaS, to_root) == 40);
const _: () = assert!(core::mem::offset_of!(DeltaS, op_count) == 72);
const _: () = assert!(core::mem::offset_of!(DeltaS, admissions) == 80);
const _: () = assert!(DELTA_LEN == 8 + 2 * ROOT_LEN + OP_COUNT_LEN + ROOT_LEN);

// §3.3 layout pins for the receipt (packed: align 1, offsets are the raw
// concatenation offsets).
const _: () = assert!(core::mem::size_of::<Receipt>() == RECEIPT_LEN);
const _: () = assert!(core::mem::align_of::<Receipt>() == 1);
const _: () = assert!(core::mem::offset_of!(Receipt, magic) == 0);
const _: () = assert!(core::mem::offset_of!(Receipt, version) == 4);
const _: () = assert!(core::mem::offset_of!(Receipt, delta) == 5);
const _: () = assert!(core::mem::offset_of!(Receipt, phi) == 117);
const _: () = assert!(core::mem::offset_of!(Receipt, blake3_digest) == 197);
const _: () = assert!(core::mem::offset_of!(Receipt, pqc_seal) == 229);
const _: () = assert!(
    RECEIPT_LEN == RECEIPT_FIXED_PREFIX_LEN + SEAL_ML_DSA_65_LEN
);
const _: () = assert!(
    RECEIPT_FIXED_PREFIX_LEN == 4 + 1 + DELTA_LEN + PHI_LEASE_LEN + DIGEST_LEN
);

// ---------------------------------------------------------------------------
// Wave 2 — core engines (§3.3 keyed digest, §3.1 codec, §4 Sealer seam,
// §6 seal/verify on the §4 UNSUPPORTED-clause path, §3.1 admissions root).
// ---------------------------------------------------------------------------

/// BLAKE3 `KEYED` chunk flag (`1 << 4`, BLAKE3 spec `keyed_hash` mode). Set on
/// EVERY compression of a keyed hash. cgap-blake3 exports the other flags but
/// no keyed API; the keyed mode lives here, bound to its kernel.
pub const BLAKE3_KEYED_FLAG: u32 = 1 << 4;

/// The keyed engine is single-chunk by construction: the production digest
/// input is exactly `DeltaS || Phi_lease` = 192 B. Longer inputs are rejected
/// loudly, never silently mis-hashed with a truncated tree.
pub const KEYED_MAX_INPUT_LEN: usize = 1024;

/// Errors from [`keyed_digest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestError {
    /// Input exceeds the single-chunk keyed engine bound
    /// [`KEYED_MAX_INPUT_LEN`]. Loud UNSUPPORTED, not a silent downgrade.
    InputTooLong {
        /// Rejected input length.
        len: usize,
        /// Engine bound (1024).
        max: usize,
    },
}

impl core::fmt::Display for DigestError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DigestError::InputTooLong { len, max } => write!(
                f,
                "keyed digest input {len} B exceeds the single-chunk bound \
                 {max} B (A16: multi-chunk keyed tree not implemented — \
                 loud UNSUPPORTED, never silent)"
            ),
        }
    }
}

impl std::error::Error for DigestError {}

/// Keyed BLAKE3 over a single chunk (1..=16 blocks of 64 B), keyed mode per
/// the BLAKE3 spec: the FIRST chunk's chaining value starts at the key words,
/// and every compression carries the `KEYED` flag on top of the chunk/root
/// flags. Bound to cgap-blake3's `compress_scalar` (one kernel, many
/// bindings).
///
/// Pinned by `tests/chatman_courts.rs` KATs: official BLAKE3
/// `test_vectors.json` `keyed_hash` values (key `"whats the Elvish word for
/// friend"`) for lengths 0/1/63/64/1024, plus the production-shape 192 B
/// vector derived with `b3sum --keyed` after b3sum was itself validated
/// byte-identical to the official corpus on the overlapped lengths.
pub fn keyed_digest(key: &[u8; 32], data: &[u8]) -> Result<[u8; DIGEST_LEN], DigestError> {
    if data.len() > KEYED_MAX_INPUT_LEN {
        return Err(DigestError::InputTooLong {
            len: data.len(),
            max: KEYED_MAX_INPUT_LEN,
        });
    }
    let mut key_words = [0u32; 8];
    for (i, w) in key_words.iter_mut().enumerate() {
        let mut b = [0u8; 4];
        b.copy_from_slice(&key[i * 4..i * 4 + 4]);
        *w = u32::from_le_bytes(b);
    }

    let num_blocks = if data.is_empty() {
        1
    } else {
        data.len().div_ceil(64)
    };
    let mut cv = key_words; // keyed mode: first chunk CV = key (BLAKE3 spec)
    let mut block_words;
    for b in 0..num_blocks {
        let start = b * 64;
        let end = (start + 64).min(data.len());
        let block = if data.is_empty() {
            &[][..]
        } else {
            &data[start..end]
        };
        block_words = [0u32; 16];
        for (i, word_bytes) in block.chunks(4).enumerate() {
            let mut buf = [0u8; 4];
            buf[..word_bytes.len()].copy_from_slice(word_bytes);
            block_words[i] = u32::from_le_bytes(buf);
        }

        let mut flags = BLAKE3_KEYED_FLAG;
        if b == 0 {
            flags |= CHUNK_START;
        }
        if b == num_blocks - 1 {
            flags |= CHUNK_END | ROOT;
        }

        let out = compress_scalar(&cv, &block_words, 0, block.len() as u32, flags);
        if b == num_blocks - 1 {
            let mut digest = [0u8; DIGEST_LEN];
            for i in 0..8 {
                digest[i * 4..i * 4 + 4].copy_from_slice(&out[i].to_le_bytes());
            }
            return Ok(digest);
        }
        cv.copy_from_slice(&out[0..8]);
    }
    unreachable!("num_blocks >= 1 always returns inside the loop")
}

impl DeltaS {
    /// Builds a delta with the versioned [`DELTA_DOMAIN_TAG`] pinned (§3.1).
    pub fn new(
        from_root: [u8; ROOT_LEN],
        to_root: [u8; ROOT_LEN],
        op_count: u64,
        admissions: [u8; ROOT_LEN],
    ) -> Self {
        DeltaS {
            domain_tag: DELTA_DOMAIN_TAG,
            from_root,
            to_root,
            op_count,
            admissions,
        }
    }

    /// The canonical 112-byte encoding: `domain_tag || from_root || to_root
    /// || op_count(LE) || admissions` (§3.1 — deterministic by construction,
    /// no optional fields, no serialization choices).
    pub fn to_bytes(&self) -> [u8; DELTA_LEN] {
        let mut out = [0u8; DELTA_LEN];
        out[0..8].copy_from_slice(&self.domain_tag);
        out[8..40].copy_from_slice(&self.from_root);
        out[40..72].copy_from_slice(&self.to_root);
        out[72..80].copy_from_slice(&self.op_count.to_le_bytes());
        out[80..112].copy_from_slice(&self.admissions);
        out
    }

    /// Decodes the canonical 112-byte encoding, refusing any delta whose
    /// `domain_tag` is not `"CGAPDLT1"` (versioned domain separator, §3.1).
    pub fn from_bytes(bytes: &[u8; DELTA_LEN]) -> Result<Self, DeltaError> {
        let mut domain_tag = [0u8; 8];
        domain_tag.copy_from_slice(&bytes[0..8]);
        if domain_tag != DELTA_DOMAIN_TAG {
            return Err(DeltaError::BadDomainTag(domain_tag));
        }
        let mut from_root = [0u8; ROOT_LEN];
        from_root.copy_from_slice(&bytes[8..40]);
        let mut to_root = [0u8; ROOT_LEN];
        to_root.copy_from_slice(&bytes[40..72]);
        let mut op_count_bytes = [0u8; 8];
        op_count_bytes.copy_from_slice(&bytes[72..80]);
        let mut admissions = [0u8; ROOT_LEN];
        admissions.copy_from_slice(&bytes[80..112]);
        Ok(DeltaS {
            domain_tag,
            from_root,
            to_root,
            op_count: u64::from_le_bytes(op_count_bytes),
            admissions,
        })
    }
}

/// Errors from [`DeltaS::from_bytes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeltaError {
    /// `domain_tag` was not `"CGAPDLT1"` — cross-protocol reuse refused (§3.1).
    BadDomainTag([u8; 8]),
}

impl core::fmt::Display for DeltaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DeltaError::BadDomainTag(tag) => {
                write!(f, "delta domain_tag {tag:02x?} != \"CGAPDLT1\" (§3.1)")
            }
        }
    }
}

impl std::error::Error for DeltaError {}

/// `algorithm_id` for the §4 UNSUPPORTED-clause unsealed tier — PROVISIONAL
/// registry value pinned by this lane (doc names none; divergence D2 in the
/// lane receipt). `0x0000` is deliberately the zero code: "no signature
/// algorithm" is not mistakable for an ML-DSA/SLH-DSA code.
pub const ALGORITHM_ID_UNSEALED: u16 = 0x0000;

/// Builds the 34-byte seal body: `digest || algorithm_id(LE)` (§3.3 — the
/// seal covers the digest AND its own algorithm id; §6 "chained, not
/// parallel claims"). Shared by [`Unsealed::seal`] and [`Unsealed::verify_seal`]
/// so both sides compute the same bytes from one construction.
fn digest_then_id_le(digest: [u8; DIGEST_LEN], algorithm_id: u16) -> [u8; DIGEST_LEN + 2] {
    let mut out = [0u8; DIGEST_LEN + 2];
    out[0..DIGEST_LEN].copy_from_slice(&digest);
    out[DIGEST_LEN..].copy_from_slice(&algorithm_id.to_le_bytes());
    out
}

/// The §4 PQC seam: "ship `chatman.rs` against a `Sealer` trait ... The trait
/// seam keeps the court suite (Section 8) valid either way."
///
/// A sealer names its own algorithm (§3.3 `algorithm_id`) and signs over
/// `blake3_digest || algorithm_id`, so the seal covers the digest AND its own
/// algorithm id (chained, not parallel claims; §6).
pub trait Sealer {
    /// 2-byte algorithm id this sealer stamps into seals (§3.3).
    fn algorithm_id(&self) -> u16;
    /// Seal bytes over `digest || algorithm_id` (§3.3).
    fn seal(&self, digest: [u8; DIGEST_LEN]) -> Vec<u8>;
    /// Verify seal bytes over `digest || algorithm_id`. Pure.
    fn verify_seal(&self, digest: [u8; DIGEST_LEN], seal: &[u8]) -> bool;
}

/// The §4 UNSUPPORTED-clause tier: BLAKE3-only binding, LOUDLY typed — the
/// type name, its `algorithm_id` `0x0000`, and the [`UnsealedReceipt`]
/// encoding (magic `"CGRU"`, version `0x00`) all say "no PQC seal here".
/// Never a silent downgrade: an `Unsealed` receipt cannot be mistaken for the
/// Wave-3 ML-DSA-65 [`Receipt`].
///
/// Honest scope (wave-6 tightening, A18 D-A18-4 precedent): `Unsealed` is a
/// MAC-class binding, not a signature. `seal` is a keyed digest re-derivable
/// by anyone holding the epoch key (`issued_root`), and [`verify_seal`]
/// authenticates NOTHING by itself — a third party without the key can still
/// run this comparison; passing it is not authenticity. Asymmetric
/// non-repudiation waits on the §4 PQC seal (Wave 3).
///
/// [`verify_seal`]: Sealer::verify_seal
pub struct Unsealed;

impl Sealer for Unsealed {
    fn algorithm_id(&self) -> u16 {
        ALGORITHM_ID_UNSEALED
    }

    /// The unsealed "seal" is the keyed digest plus its algorithm id — a
    /// MAC-class epoch binding under the lease, NOT a signature (see the
    /// type-level honest-scope note above).
    fn seal(&self, digest: [u8; DIGEST_LEN]) -> Vec<u8> {
        let mut out = Vec::with_capacity(DIGEST_LEN + 2);
        out.extend_from_slice(&digest_then_id_le(digest, ALGORITHM_ID_UNSEALED));
        out
    }

    /// Constant-shape compare (wave-6 repair: fold every byte difference;
    /// same discipline as the A18 `LeaseEpochSealer::verify`). Wave-6 seal
    /// sweep court: EVERY single-byte flip of a 34-B seal must flip the
    /// verdict.
    fn verify_seal(&self, digest: [u8; DIGEST_LEN], seal: &[u8]) -> bool {
        let expected = digest_then_id_le(digest, ALGORITHM_ID_UNSEALED);
        let len_ok = expected.len() == seal.len();
        let diff = expected
            .iter()
            .zip(seal.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        len_ok && diff == 0
    }
}

/// `magic` for the unsealed tier: `"CGRU"` — the sealed receipt's `"CGRC"`
/// with the final byte naming the tier (divergence D2: the doc defines no
/// unsealed encoding; without a distinct magic an unsealed receipt could be
/// read as a truncated [`Receipt`]).
pub const UNSEALED_RECEIPT_MAGIC: [u8; 4] = *b"CGRU";
/// `version` for the unsealed tier: `0x00` (the sealed `0x01` stays §3.3's).
pub const UNSEALED_RECEIPT_VERSION: u8 = 0x00;
/// Unsealed-receipt length: the constant prefix only — `5 + 112 + 80 + 32`.
pub const UNSEALED_RECEIPT_LEN: usize = RECEIPT_FIXED_PREFIX_LEN;

/// The §4 UNSUPPORTED-clause receipt: everything §3.3 pins about the prefix,
/// minus the PQC seal — loudly typed by name, magic, and version.
///
/// `#[repr(C, packed)]` mirrors [`Receipt`]: the wire form is a byte
/// concatenation (no pad bytes); read fields by value, never by reference.
/// Derives are sound on the packed form because every field is `Copy` (the
/// derive copies fields out; no field references exist).
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsealedReceipt {
    /// offset 0, 4 B — `"CGRU"`.
    pub magic: [u8; 4],
    /// offset 4, 1 B — `0x00`.
    pub version: u8,
    /// offset 5, 112 B — the admitted delta (§3.1).
    pub delta: DeltaS,
    /// offset 117, 80 B — the lease epoch (§3.2).
    pub phi: PhiLease,
    /// offset 197, 32 B — keyed BLAKE3 over `DeltaS || Phi_lease`,
    /// key = `issued_root` (§3.3/§6). For this tier it is ALSO the binding
    /// carrier: a digest re-derivable only by holders of the epoch key.
    /// NOT authenticity — [`verify_unsealed`] runs without any key and
    /// proves integrity-plus-epoch-consistency, nothing more (§4
    /// UNSUPPORTED clause; same MAC-not-signature scope the A18 audit
    /// recorded as D-A18-4).
    pub blake3_digest: [u8; DIGEST_LEN],
}

const _: () = assert!(core::mem::size_of::<UnsealedReceipt>() == UNSEALED_RECEIPT_LEN);
const _: () = assert!(core::mem::align_of::<UnsealedReceipt>() == 1);
const _: () = assert!(core::mem::offset_of!(UnsealedReceipt, magic) == 0);
const _: () = assert!(core::mem::offset_of!(UnsealedReceipt, version) == 4);
const _: () = assert!(core::mem::offset_of!(UnsealedReceipt, delta) == 5);
const _: () = assert!(core::mem::offset_of!(UnsealedReceipt, phi) == 117);
const _: () = assert!(core::mem::offset_of!(UnsealedReceipt, blake3_digest) == 197);
const _: () = assert!(UNSEALED_RECEIPT_LEN == 229);

impl UnsealedReceipt {
    /// Canonical 229-byte encoding (§3.1 discipline: fixed order, LE fields,
    /// no choices). Packed-struct reads are by-value copies only.
    pub fn to_bytes(&self) -> [u8; UNSEALED_RECEIPT_LEN] {
        let (magic, version, delta, phi, digest) =
            (self.magic, self.version, self.delta, self.phi, self.blake3_digest);
        let mut out = [0u8; UNSEALED_RECEIPT_LEN];
        out[0..4].copy_from_slice(&magic);
        out[4] = version;
        out[5..117].copy_from_slice(&delta.to_bytes());
        out[117..197].copy_from_slice(&phi.to_bytes());
        out[197..229].copy_from_slice(&digest);
        out
    }

    /// Decodes the canonical encoding; refuses foreign magic/version and
    /// foreign delta domain tags.
    pub fn from_bytes(bytes: &[u8; UNSEALED_RECEIPT_LEN]) -> Result<Self, VerifyError> {
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if magic != UNSEALED_RECEIPT_MAGIC {
            return Err(VerifyError::BadMagic(magic));
        }
        if bytes[4] != UNSEALED_RECEIPT_VERSION {
            return Err(VerifyError::BadVersion(bytes[4]));
        }
        let mut delta_bytes = [0u8; DELTA_LEN];
        delta_bytes.copy_from_slice(&bytes[5..117]);
        let delta = DeltaS::from_bytes(&delta_bytes).map_err(VerifyError::BadDelta)?;
        let mut phi_bytes = [0u8; PHI_LEASE_LEN];
        phi_bytes.copy_from_slice(&bytes[117..197]);
        let phi = PhiLease::from_bytes(&phi_bytes);
        let mut digest = [0u8; DIGEST_LEN];
        digest.copy_from_slice(&bytes[197..229]);
        Ok(UnsealedReceipt {
            magic,
            version: bytes[4],
            delta,
            phi,
            blake3_digest: digest,
        })
    }
}

/// Errors from [`verify_unsealed`] and [`UnsealedReceipt::from_bytes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// Magic was not `"CGRU"` (foreign or sealed-receipt bytes).
    BadMagic([u8; 4]),
    /// Version was not `0x00`.
    BadVersion(u8),
    /// `Phi_lease` carries `tick >= tick_ceiling`: no lease this crate can
    /// build could have emitted it (§5 trap) — authority invariant failure.
    ExhaustedLease {
        /// Claimed tick.
        tick: u64,
        /// Claimed ceiling.
        tick_ceiling: u64,
    },
    /// Keyed digest recompute over `DeltaS || Phi_lease` (key =
    /// `issued_root`) did not equal the stored digest — some covered byte
    /// was mutated, or the receipt was built under a different lease epoch.
    DigestMismatch,
    /// The embedded `DeltaS` failed its own domain-tag check (§3.1) — the
    /// receipt carries a delta that never was an admitted-delta encoding.
    BadDelta(crate::chatman::DeltaError),
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VerifyError::BadMagic(m) => write!(f, "receipt magic {m:02x?} != \"CGRU\""),
            VerifyError::BadVersion(v) => write!(f, "receipt version {v:#04x} != 0x00"),
            VerifyError::ExhaustedLease { tick, tick_ceiling } => write!(
                f,
                "Phi_lease tick {tick} >= tick_ceiling {tick_ceiling} — no \
                 live lease could have sealed this receipt (§5)"
            ),
            VerifyError::DigestMismatch => {
                write!(f, "keyed digest mismatch over DeltaS || Phi_lease")
            }
            VerifyError::BadDelta(e) => write!(f, "receipt carries a non-delta: {e}"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// §6 `seal`, unsealed tier: the ONLY tick consumer. Produces the 229-byte
/// receipt over `delta` under `lease`, consuming exactly one tick (which
/// TRAPS on exhaustion, §5 — the trap aborts the seal, so no receipt with an
/// exhausted tick can escape).
///
/// Digest chain (§3.3/§6): `blake3_digest = keyed_blake3(DeltaS || Phi_lease,
/// key = issued_root)` — keyed by the lease, so a receipt stolen from one
/// lease epoch does not verify under another (§3.3; the rotation story across
/// epochs stays the §3.4 UNKNOWN).
pub fn seal_unsealed(delta: DeltaS, lease: &mut Lease) -> UnsealedReceipt {
    let phi = lease.phi();
    let mut covered = [0u8; DELTA_LEN + PHI_LEASE_LEN];
    covered[0..DELTA_LEN].copy_from_slice(&delta.to_bytes());
    covered[DELTA_LEN..].copy_from_slice(&phi.to_bytes());
    let blake3_digest = keyed_digest(&phi.issued_root, &covered)
        .expect("192 B digest input: single-chunk bound cannot trip");
    lease.tick(); // §6: seal is the only tick consumer; traps on exhaustion
    UnsealedReceipt {
        magic: UNSEALED_RECEIPT_MAGIC,
        version: UNSEALED_RECEIPT_VERSION,
        delta,
        phi,
        blake3_digest,
    }
}

/// §6 `verify`, unsealed tier. PURE — mutates nothing, consumes no tick.
///
/// 1. magic/version (encoding shape, §3.3 step 1);
/// 2. recompute keyed BLAKE3(`DeltaS || Phi_lease`, key = `issued_root`) and
///    require byte equality with `blake3_digest` (§3.3 step 2);
/// 3. require `phi.tick < phi.tick_ceiling` — the §5 trap makes it
///    impossible for a live lease to emit otherwise, so a receipt claiming
///    an exhausted tick is forged by construction (§3.3 step 3's seal check
///    degenerates here: the unsealed tier's binding IS the digest).
pub fn verify_unsealed(receipt: &UnsealedReceipt) -> Result<(), VerifyError> {
    let (magic, version, delta, phi, stored_digest) =
        (receipt.magic, receipt.version, receipt.delta, receipt.phi, receipt.blake3_digest);
    if magic != UNSEALED_RECEIPT_MAGIC {
        return Err(VerifyError::BadMagic(magic));
    }
    if version != UNSEALED_RECEIPT_VERSION {
        return Err(VerifyError::BadVersion(version));
    }
    if phi.tick >= phi.tick_ceiling {
        return Err(VerifyError::ExhaustedLease {
            tick: phi.tick,
            tick_ceiling: phi.tick_ceiling,
        });
    }
    let mut covered = [0u8; DELTA_LEN + PHI_LEASE_LEN];
    covered[0..DELTA_LEN].copy_from_slice(&delta.to_bytes());
    covered[DELTA_LEN..].copy_from_slice(&phi.to_bytes());
    let recomputed = keyed_digest(&phi.issued_root, &covered)
        .expect("192 B digest input: single-chunk bound cannot trip");
    if recomputed != stored_digest {
        return Err(VerifyError::DigestMismatch);
    }
    Ok(())
}

/// §3.1 `admissions`: BLAKE3 Merkle root over the ordered list of per-operation
/// admission receipts (receipts chain).
///
/// Byte-level construction pinned by this lane (divergence D3: the doc names
/// the root but not the tree): leaf_i = UNkeyed BLAKE3(`encoded_receipt_i`)
/// (full-receipt encoding, so the leaf is identity-bound); parent =
/// UNkeyed BLAKE3(`left || right`); an odd final node is promoted; the empty
/// list has the typed zero root. Ordered — reordering receipts changes the
/// root.
pub fn admissions_root(encoded_receipts: &[&[u8]]) -> [u8; ROOT_LEN] {
    if encoded_receipts.is_empty() {
        return [0u8; ROOT_LEN];
    }
    let mut level: Vec<[u8; ROOT_LEN]> = encoded_receipts
        .iter()
        .map(|bytes| hash(bytes))
        .collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let node = match pair {
                [left, right] => {
                    let mut buf = [0u8; 64];
                    buf[0..32].copy_from_slice(left);
                    buf[32..64].copy_from_slice(right);
                    hash(&buf)
                }
                [alone] => *alone,
                _ => unreachable!("chunks(2) yields slices of len 1 or 2"),
            };
            next.push(node);
        }
        level = next;
    }
    level[0]
}
