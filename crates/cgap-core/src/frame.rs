//! Wire tier: the canonical 128-byte `CgapFrameHeader` (lane A04, Wave 2 core engines).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md` §6 (normative
//! struct + byte-offset table + flags + static asserts). The embedded
//! `P28LicenseAttestation` follows the **A17 lane doc §2** layout — the
//! authoritative definition per the v26.11.1 dispatch contract; A04 §6.2's
//! local draft yields to it (divergence recorded in the lane receipt).
//!
//! Wave 1 froze the ABI (struct/const/static asserts). Wave 2 lands the
//! §6/§8 logic on that frozen ABI: little-endian `encode`/`decode`
//! (§6.1: "All multi-byte integers are little-endian"; §10: "never
//! `transmute` for multi-byte fields"), structural `validate()`
//! (§8.3 `FrameError::BadMagic`/`ReservedBitsSet`), `oabi_digest_verify`
//! (§5.1 component), and A17 §2.2/§6 attestation validity (`FrameError::
//! AttestationMismatch`).

#![allow(dead_code)] // Kernel ABI for consumer lanes (A14/A17/A19); unused here by design.

/// 0x43 0x47 0x41 0x50 — ASCII "CGAP" (§6, `magic` field).
pub const FRAME_MAGIC: [u8; 4] = *b"CGAP";

/// Format version, start at 1 (§6, `version` field).
pub const FRAME_VERSION: u8 = 1;

/// bit 0: Arrow batch present (§6.3).
pub const FLAG_ARROW: u8 = 0x01;

/// bit 1: Parquet page present (§6.3).
pub const FLAG_PARQUET: u8 = 0x02;

/// bit 2: BN3 payload present (§6.3).
pub const FLAG_BN3: u8 = 0x04;

/// bit 3: GitPack payload present (§6.3).
pub const FLAG_GITPACK: u8 = 0x08;

/// bits 4-7 reserved, must be zero (§6.3: "bits 4-7 reserved, must be zero").
pub const FLAG_RESERVED_MASK: u8 = 0xF0;

/// Total wire header size in bytes (§6.1 offset table, "Total: 128 bytes").
pub const FRAME_HEADER_LEN: usize = 128;

/// `P28LicenseAttestation` size in bytes (A17 §2.1 offset table, end at 80).
pub const ATTESTATION_LEN: usize = 80;

// --- P28LicenseAttestation constants (A17 §2.1 / §2.2, cited not redefined) ---

/// offset 0, 4 B — `'P','2','8','S'` (A17 §2.1, 0x50 0x32 0x38 0x53).
pub const LICENSE_MAGIC: [u8; 4] = *b"P28S";

/// A17 §2.2 bit 0: derives directly from the P28-SIL instrument; receipt all-zero.
pub const LICENSE_FLAG_ROOT: u16 = 0x01;

/// A17 §2.2 bit 1: derives from a Root artifact; `upstream_receipt` MUST be nonzero.
pub const LICENSE_FLAG_DERIVATIVE: u16 = 0x02;

/// A17 §2.2 bit 2: transpile target (beam -> WASM); receipt = transpiler output receipt.
pub const LICENSE_FLAG_TRANSPILED: u16 = 0x04;

/// A17 §2.2: exactly one of bits 0-2 MUST be set (lineage bits).
pub const LICENSE_FLAG_LINEAGE_MASK: u16 = 0x07;

/// A17 §2.2 bits 3-15: reserved, MUST be 0 ("nonzero reserved bits are a
/// spoliation condition", A17 §6).
pub const LICENSE_FLAG_RESERVED_MASK: u16 = 0xFFF8;

/// A17 §2.1: `safe_harbor_ceiling_cents` MUST be <= 5_000_000_000 ($50M).
pub const SAFE_HARBOR_CEILING_CENTS_MAX: u64 = 5_000_000_000;

/// 80-byte P28 license attestation embedded in every `CgapFrameHeader`.
///
/// Layout is **A17 §2 / §2.1** (authoritative): `#[repr(C, align(8))]`,
/// exactly 80 bytes, no padding holes (offsets 0-7, 8-15, 16-47, 48-79 fully
/// occupied). Byte-offset table frozen at Wave 1 in A17 §2.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct P28LicenseAttestation {
    /// offset 0, 4 B — `'P','2','8','S'` (0x50 0x32 0x38 0x53)
    pub license_magic: [u8; 4],
    /// offset 4, 2 B — u16 LE, currently 1
    pub version: u16,
    /// offset 6, 2 B — u16 LE bitfield (A17 §2.2)
    pub attestation_flags: u16,
    /// offset 8, 8 B — u64 LE; MUST be <= 5_000_000_000 ($50M)
    pub safe_harbor_ceiling_cents: u64,
    /// offset 16, 32 B — BLAKE3 origin-authority digest
    pub origin_authority_digest: [u8; 32],
    /// offset 48, 32 B — BLAKE3 of parent receipt; zero only if Root
    pub upstream_receipt: [u8; 32],
}

impl P28LicenseAttestation {
    /// A17 §2.2 / §6 validity court (structural, digest-free): magic, version,
    /// exactly-one lineage bit, zero reserved bits, safe-harbor ceiling, and
    /// `upstream_receipt` nonzero iff the record is a derivative/transpile.
    ///
    /// Every refusal is the single typed variant `FrameError::AttestationMismatch`
    /// (§8.3: "corrupt 1 byte of `attestation` → refuses,
    /// `FrameError::AttestationMismatch`") — the field that tripped is visible
    /// via the A17 §2.1 offset table, not a per-field error lattice.
    pub fn validate(&self) -> Result<(), FrameError> {
        if self.license_magic != LICENSE_MAGIC {
            return Err(FrameError::AttestationMismatch);
        }
        if self.version != 1 {
            return Err(FrameError::AttestationMismatch);
        }
        let flags = self.attestation_flags;
        if flags & LICENSE_FLAG_RESERVED_MASK != 0 {
            return Err(FrameError::AttestationMismatch);
        }
        let lineage = flags & LICENSE_FLAG_LINEAGE_MASK;
        if lineage.count_ones() != 1 {
            return Err(FrameError::AttestationMismatch);
        }
        if self.safe_harbor_ceiling_cents > SAFE_HARBOR_CEILING_CENTS_MAX {
            return Err(FrameError::AttestationMismatch);
        }
        let needs_upstream =
            flags & (LICENSE_FLAG_DERIVATIVE | LICENSE_FLAG_TRANSPILED) != 0;
        let has_upstream = self.upstream_receipt != [0u8; 32];
        if needs_upstream != has_upstream {
            return Err(FrameError::AttestationMismatch);
        }
        Ok(())
    }

    /// Serialize into `out` at its start; exactly [`ATTESTATION_LEN`] bytes
    /// written, all multi-byte fields little-endian (A17 §2.1).
    pub fn encode_into(&self, out: &mut [u8]) -> Result<(), FrameError> {
        if out.len() < ATTESTATION_LEN {
            return Err(FrameError::BufferTooSmall);
        }
        out[0..4].copy_from_slice(&self.license_magic);
        out[4..6].copy_from_slice(&self.version.to_le_bytes());
        out[6..8].copy_from_slice(&self.attestation_flags.to_le_bytes());
        out[8..16].copy_from_slice(&self.safe_harbor_ceiling_cents.to_le_bytes());
        out[16..48].copy_from_slice(&self.origin_authority_digest);
        out[48..80].copy_from_slice(&self.upstream_receipt);
        Ok(())
    }

    /// Parse from `src` at its start; all multi-byte fields little-endian
    /// (A17 §2.1). Layout-only: no validation (call [`P28LicenseAttestation::validate`]).
    pub fn decode(src: &[u8]) -> Result<Self, FrameError> {
        if src.len() < ATTESTATION_LEN {
            return Err(FrameError::BufferTooSmall);
        }
        let mut att = P28LicenseAttestation {
            license_magic: [0; 4],
            version: 0,
            attestation_flags: 0,
            safe_harbor_ceiling_cents: 0,
            origin_authority_digest: [0; 32],
            upstream_receipt: [0; 32],
        };
        att.license_magic.copy_from_slice(&src[0..4]);
        let mut w = [0u8; 2];
        w.copy_from_slice(&src[4..6]);
        att.version = u16::from_le_bytes(w);
        w.copy_from_slice(&src[6..8]);
        att.attestation_flags = u16::from_le_bytes(w);
        let mut q = [0u8; 8];
        q.copy_from_slice(&src[8..16]);
        att.safe_harbor_ceiling_cents = u64::from_le_bytes(q);
        att.origin_authority_digest.copy_from_slice(&src[16..48]);
        att.upstream_receipt.copy_from_slice(&src[48..80]);
        Ok(att)
    }
}

/// Canonical wire header: `#[repr(C, align(64))]`, exactly 128 bytes (§6).
///
/// All multi-byte integers are little-endian (§6.1). The layout is gap-free
/// by construction; the `align(64)` keeps the header cache-line aligned for
/// the DMA ring (§7.2) and makes `FRAME_HEADER_LEN` a power-of-two boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct CgapFrameHeader {
    /// offset 0, 4 B — 0x43 0x47 0x41 0x50 = ASCII "CGAP"
    pub magic: [u8; 4],
    /// offset 4, 1 B — format version, start at 1
    pub version: u8,
    /// offset 5, 1 B — bit 0 Arrow, bit 1 Parquet, bit 2 BN3, bit 3 GitPack
    pub flags: u8,
    /// offset 6, 2 B — reserved, zero on write, ignored on read (v1)
    pub reserved: [u8; 2],
    /// offset 8, 8 B — total payload byte length, little-endian
    pub payload_byte_len: u64,
    /// offset 16, 32 B — BLAKE3-derived digest over the payload bytes
    pub oabi_digest: [u8; 32],
    /// offset 48, 80 B — embedded P28 license attestation, spans 48..128
    pub attestation: P28LicenseAttestation,
}

impl CgapFrameHeader {
    /// Serialize into `out` at its start; exactly [`FRAME_HEADER_LEN`] bytes
    /// written, all multi-byte fields little-endian (§6.1), `reserved` zeroed
    /// on write (§6 "must be zero on write").
    pub fn encode_into(&self, out: &mut [u8]) -> Result<(), FrameError> {
        if out.len() < FRAME_HEADER_LEN {
            return Err(FrameError::BufferTooSmall);
        }
        out[0..4].copy_from_slice(&self.magic);
        out[4] = self.version;
        out[5] = self.flags;
        out[6..8].copy_from_slice(&[0u8; 2]); // reserved: zero on write (§6)
        out[8..16].copy_from_slice(&self.payload_byte_len.to_le_bytes());
        out[16..48].copy_from_slice(&self.oabi_digest);
        self.attestation.encode_into(&mut out[48..128])
    }

    /// Parse from `src` at its start; all multi-byte fields little-endian
    /// (§6.1). Layout-only: no validation (call [`CgapFrameHeader::validate`]).
    pub fn decode(src: &[u8]) -> Result<Self, FrameError> {
        if src.len() < FRAME_HEADER_LEN {
            return Err(FrameError::BufferTooSmall);
        }
        let attestation = P28LicenseAttestation::decode(&src[48..128])?;
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&src[0..4]);
        let mut reserved = [0u8; 2];
        reserved.copy_from_slice(&src[6..8]);
        let mut w = [0u8; 8];
        w.copy_from_slice(&src[8..16]);
        Ok(CgapFrameHeader {
            magic,
            version: src[4],
            flags: src[5],
            reserved,
            payload_byte_len: u64::from_le_bytes(w),
            oabi_digest: {
                let mut d = [0u8; 32];
                d.copy_from_slice(&src[16..48]);
                d
            },
            attestation,
        })
    }

    /// Structural validation (§8.3 rows: magic flip → `BadMagic`; reserved
    /// flag bit → `ReservedBitsSet`), plus A17 §2.2 attestation validity.
    ///
    /// `payload_byte_len` and the digest are checked only with a payload
    /// present — see [`CgapFrameHeader::verify_payload`]. `reserved` bytes
    /// are ignored on read (§6 "zero on write, ignored on read (v1)").
    pub fn validate(&self) -> Result<(), FrameError> {
        if self.magic != FRAME_MAGIC {
            return Err(FrameError::BadMagic);
        }
        if self.version != FRAME_VERSION {
            return Err(FrameError::BadVersion);
        }
        if self.flags & FLAG_RESERVED_MASK != 0 {
            return Err(FrameError::ReservedBitsSet);
        }
        self.attestation.validate()?;
        Ok(())
    }

    /// Payload view over a full frame region: `frame[128 .. 128 +
    /// payload_byte_len]`, refusing when the declared length overruns the
    /// region (`PayloadLenMismatch`).
    ///
    /// Wave-5 repair (A04 defect fix): the declared length is a `u64` from
    /// the wire; the gate must never narrow it with `as usize` — on 32-bit
    /// targets (`riscv32imc`, lane DoD 8) a header declaring `2^32` bytes
    /// truncated to 0 and yielded an EMPTY payload view (`Ok`), fail-open on
    /// a corrupt header. `TryFrom` refuses the unrepresentable case and
    /// `checked_add` the end-overflow case: fail-closed on every target.
    pub fn payload<'a>(&self, frame: &'a [u8]) -> Result<&'a [u8], FrameError> {
        let end = usize::try_from(self.payload_byte_len)
            .ok()
            .and_then(|len| FRAME_HEADER_LEN.checked_add(len))
            .ok_or(FrameError::PayloadLenMismatch)?;
        if frame.len() < end {
            return Err(FrameError::PayloadLenMismatch);
        }
        Ok(&frame[FRAME_HEADER_LEN..end])
    }

    /// §5.1 `oabi_digest_verify` (pure fn): re-derive the digest over `payload`
    /// with the caller-supplied digest function and compare to the embedded
    /// `oabi_digest` (§8.3: corrupt payload byte → `DigestMismatch`).
    ///
    /// The digest function is injected, not linked: cgap-core stays
    /// zero-dependency (§3/§4), and the in-tree `cgap-blake3` adapter
    /// (`cgap_blake3::hash`) supplies BLAKE3 at the call site (tests/courts;
    /// the lane-doc-named substrate). `payload_byte_len` must equal
    /// `payload.len()` first (`PayloadLenMismatch` otherwise).
    pub fn verify_payload(
        &self,
        payload: &[u8],
        digest: impl Fn(&[u8]) -> [u8; 32],
    ) -> Result<(), FrameError> {
        if payload.len() as u64 != self.payload_byte_len {
            return Err(FrameError::PayloadLenMismatch);
        }
        if self.oabi_digest != digest(payload) {
            return Err(FrameError::DigestMismatch);
        }
        Ok(())
    }
}

/// Typed frame refusals (§8.3 names `BadMagic`, `ReservedBitsSet`,
/// `AttestationMismatch`, `DigestMismatch`; `BadVersion` and
/// `PayloadLenMismatch`/`BufferTooSmall` are this crate's typed additions for
/// the remaining refusal paths — disclosed, not silent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// §8.3: magic flipped from "CGAP" (e.g. "XGAP").
    BadMagic,
    /// `version != FRAME_VERSION` (§6: "start at 1"; version bump is the only
    /// field-addition path, §6.4).
    BadVersion,
    /// §8.3/§6.3: any of flags bits 4-7 set.
    ReservedBitsSet,
    /// §8.3: attestation field corrupted (A17 §2.2/§6 validity fails).
    AttestationMismatch,
    /// `payload_byte_len` disagrees with the payload actually presented.
    PayloadLenMismatch,
    /// §8.3: re-derived digest differs from the embedded `oabi_digest`.
    DigestMismatch,
    /// Encode/decode buffer shorter than the pinned 128 B / 80 B layout.
    BufferTooSmall,
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match self {
            FrameError::BadMagic => "BadMagic",
            FrameError::BadVersion => "BadVersion",
            FrameError::ReservedBitsSet => "ReservedBitsSet",
            FrameError::AttestationMismatch => "AttestationMismatch",
            FrameError::PayloadLenMismatch => "PayloadLenMismatch",
            FrameError::DigestMismatch => "DigestMismatch",
            FrameError::BufferTooSmall => "BufferTooSmall",
        };
        f.write_str(name)
    }
}

// §6.4 static size asserts (normative) — core-only const asserts, no
// `static_assertions` crate (zero-dependency rule).
const _: () = assert!(core::mem::size_of::<CgapFrameHeader>() == 128);
const _: () = assert!(core::mem::align_of::<CgapFrameHeader>() == 64);
const _: () = assert!(core::mem::size_of::<P28LicenseAttestation>() == 80);
const _: () = assert!(core::mem::align_of::<P28LicenseAttestation>() == 8);

// §6.1 byte-offset table, machine-checked at compile time (`offset_of!`,
// stable 1.77). Each row below is one table line from §6.1 / A17 §2.1;
// a field reorder that preserves total size still trips these pins.
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, magic) == 0);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, version) == 4);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, flags) == 5);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, reserved) == 6);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, payload_byte_len) == 8);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, oabi_digest) == 16);
const _: () = assert!(core::mem::offset_of!(CgapFrameHeader, attestation) == 48);

const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, license_magic) == 0);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, version) == 4);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, attestation_flags) == 6);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, safe_harbor_ceiling_cents) == 8);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, origin_authority_digest) == 16);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, upstream_receipt) == 48);

// §6.1 layout is gap-free by construction: 4+1+1+2+8+32+80 = 128 and
// 4+2+2+8+32+32 = 80 — no padding exists, so byte-for-byte identity (§8.1)
// can never be defeated by uninitialized padding. Pinned here so a future
// field insertion that opens a padding hole fails to compile.
const _: () = assert!(
    4 + 1 + 1 + 2 + 8 + 32 + 80 == core::mem::size_of::<CgapFrameHeader>()
        && 4 + 2 + 2 + 8 + 32 + 32 == core::mem::size_of::<P28LicenseAttestation>()
);

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic BLAKE3 stand-in for unit tests: the digest function is
    /// injected (zero-dep kernel); the cross-tier identity court exercises the
    /// real `cgap-blake3` adapter.
    fn fake_digest(data: &[u8]) -> [u8; 32] {
        let mut d = [0u8; 32];
        let mut acc: u64 = 0x9E3779B97F4A7C15 ^ (data.len() as u64);
        for (i, b) in data.iter().enumerate() {
            acc = acc.rotate_left(7) ^ ((*b as u64) << (i % 8 * 8));
            acc = acc.wrapping_mul(0x100000001B3);
        }
        for (i, slot) in d.iter_mut().enumerate() {
            *slot = (acc >> (i % 8 * 8)) as u8 ^ (i as u8).wrapping_mul(31);
        }
        d
    }

    /// Valid Root attestation (A17 §2.2: FLAG_ROOT, receipt all-zero).
    pub(crate) fn root_attestation() -> P28LicenseAttestation {
        P28LicenseAttestation {
            license_magic: LICENSE_MAGIC,
            version: 1,
            attestation_flags: LICENSE_FLAG_ROOT,
            safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
            origin_authority_digest: [7u8; 32],
            upstream_receipt: [0u8; 32],
        }
    }

    pub(crate) fn valid_header(payload_len: u64) -> CgapFrameHeader {
        CgapFrameHeader {
            magic: FRAME_MAGIC,
            version: FRAME_VERSION,
            flags: FLAG_BN3,
            reserved: [0; 2],
            payload_byte_len: payload_len,
            oabi_digest: [9u8; 32],
            attestation: root_attestation(),
        }
    }

    #[test]
    fn validate_accepts_valid_header_positive_control() {
        let h = valid_header(64);
        assert_eq!(h.validate(), Ok(()));
    }

    #[test]
    fn mutation_magic_flip_refused_badmagic() {
        // §8.3: flip magic to "XGAP" → validate() refuses, FrameError::BadMagic.
        let mut h = valid_header(0);
        h.magic = *b"XGAP";
        assert_eq!(h.validate(), Err(FrameError::BadMagic));
    }

    #[test]
    fn mutation_reserved_flag_bit_refused() {
        // §8.3: set flags bit 4 (reserved) → refuses, FrameError::ReservedBitsSet.
        let mut h = valid_header(0);
        h.flags |= 0x10;
        assert_eq!(h.validate(), Err(FrameError::ReservedBitsSet));
    }

    #[test]
    fn mutation_version_bump_refused() {
        let mut h = valid_header(0);
        h.version = 2;
        assert_eq!(h.validate(), Err(FrameError::BadVersion));
    }

    #[test]
    fn mutation_attestation_corruptions_refused() {
        // §8.3: corrupt 1 byte of attestation → FrameError::AttestationMismatch.
        // One byte per A17 §2.1 invariant-bearing field, one at a time.
        // (The opaque `origin_authority_digest` bytes carry no structural
        // invariant, so their corruption is not detectable here — retired
        // per §8.3's own rule; detection is A17 §3's digest court, not
        // structural validation.)
        for i in 0..6 {
            let mut h = valid_header(0);
            match i {
                0 => h.attestation.license_magic = *b"P28X",
                1 => h.attestation.version = 2,
                2 => h.attestation.attestation_flags |= 0x0008,
                3 => h.attestation.safe_harbor_ceiling_cents += 1,
                4 => h.attestation.upstream_receipt[0] ^= 0xFF, // nonzero receipt on ROOT
                _ => h.attestation.attestation_flags &= !0x07,
            }
            let names = ["magic", "version", "flags_reserved", "ceiling", "upstream", "lineage_zero"];
            assert_eq!(
                h.validate(),
                Err(FrameError::AttestationMismatch),
                "mutation `{}` was not refused",
                names[i]
            );
        }
    }

    #[test]
    fn attestation_lineage_exactly_one_bit() {
        // A17 §2.2: zero or 2+ lineage bits set is malformed.
        let mut h = valid_header(0);
        h.attestation.attestation_flags = LICENSE_FLAG_ROOT | LICENSE_FLAG_DERIVATIVE;
        assert_eq!(h.validate(), Err(FrameError::AttestationMismatch));

        h.attestation.attestation_flags = LICENSE_FLAG_DERIVATIVE;
        h.attestation.upstream_receipt = [1u8; 32];
        assert_eq!(h.validate(), Ok(()));

        // derivative without upstream receipt and vice versa both refuse.
        h.attestation.upstream_receipt = [0u8; 32];
        assert_eq!(h.validate(), Err(FrameError::AttestationMismatch));
        let mut root = valid_header(0);
        root.attestation.attestation_flags = LICENSE_FLAG_ROOT;
        root.attestation.upstream_receipt = [1u8; 32];
        assert_eq!(root.validate(), Err(FrameError::AttestationMismatch));
    }

    #[test]
    fn encode_decode_roundtrip_all_fields() {
        let h = valid_header(0x0102030405060708);
        let mut buf = [0u8; FRAME_HEADER_LEN];
        h.encode_into(&mut buf).unwrap();
        let back = CgapFrameHeader::decode(&buf).unwrap();
        assert_eq!(back.magic, h.magic);
        assert_eq!(back.version, h.version);
        assert_eq!(back.flags, h.flags);
        assert_eq!(back.payload_byte_len, h.payload_byte_len);
        assert_eq!(back.oabi_digest, h.oabi_digest);
        assert_eq!(back.attestation.license_magic, h.attestation.license_magic);
        assert_eq!(back.attestation.version, h.attestation.version);
        assert_eq!(back.attestation.attestation_flags, h.attestation.attestation_flags);
        assert_eq!(
            back.attestation.safe_harbor_ceiling_cents,
            h.attestation.safe_harbor_ceiling_cents
        );
        assert_eq!(back.attestation.origin_authority_digest, h.attestation.origin_authority_digest);
        assert_eq!(back.attestation.upstream_receipt, h.attestation.upstream_receipt);
        // re-encode is byte-stable (wire identity, §8.1)
        let mut buf2 = [0u8; FRAME_HEADER_LEN];
        back.encode_into(&mut buf2).unwrap();
        assert_eq!(buf, buf2);
    }

    #[test]
    fn header_fields_are_little_endian_on_the_wire() {
        // DoD 7 byte-swap court: 0x0102030405060708 must hit the wire as
        // 08 07 06 05 04 03 02 01 (LE); a BE interpretation of those bytes
        // differs from the value — proving the pin is LE, not native.
        let h = valid_header(0x0102030405060708);
        let mut buf = [0u8; FRAME_HEADER_LEN];
        h.encode_into(&mut buf).unwrap();
        assert_eq!(
            &buf[8..16],
            &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
        let decoded = CgapFrameHeader::decode(&buf).unwrap();
        assert_eq!(decoded.payload_byte_len, 0x0102030405060708);
        assert_ne!(
            u64::from_be_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]),
            decoded.payload_byte_len
        );
        // attestation ceiling LE too (A17 §2.1): 5_000_000_000 = 0x12A05F200
        let ceil = SAFE_HARBOR_CEILING_CENTS_MAX.to_le_bytes();
        assert_eq!(&buf[48 + 8..48 + 16], &ceil);
    }

    #[test]
    fn reserved_bytes_zeroed_on_write() {
        let mut h = valid_header(1);
        h.reserved = [0xAA; 2]; // garbage in the struct...
        let mut buf = [0xEE; FRAME_HEADER_LEN];
        h.encode_into(&mut buf).unwrap();
        assert_eq!(&buf[6..8], &[0, 0]); // ...zero on the wire (§6)
        assert_eq!(CgapFrameHeader::decode(&buf).unwrap().reserved, [0, 0]);
    }

    #[test]
    fn verify_payload_digest_mismatch_and_len_mismatch() {
        let payload = [1u8, 2, 3, 4, 5];
        let mut h = valid_header(payload.len() as u64);
        h.oabi_digest = fake_digest(&payload);
        assert_eq!(h.verify_payload(&payload, fake_digest), Ok(()));

        // §8.3: corrupt 1 byte of payload → DigestMismatch.
        let mut corrupt = payload;
        corrupt[2] ^= 0x01;
        assert_eq!(
            h.verify_payload(&corrupt, fake_digest),
            Err(FrameError::DigestMismatch)
        );

        // §8.3: shift payload_byte_len by +1 → PayloadLenMismatch at the
        // length gate (identity court pins offset 8 separately).
        let mut h2 = h;
        h2.payload_byte_len += 1;
        assert_eq!(
            h2.verify_payload(&payload, fake_digest),
            Err(FrameError::PayloadLenMismatch)
        );
    }

    #[test]
    fn payload_view_refuses_wire_lengths_that_cannot_name_an_end() {
        // Wave-5 repair pin (A04 defect fix): `payload_byte_len` is a wire
        // u64; the payload gate must be a widening + checked compare, never
        // `as usize`. On 32-bit targets (riscv32imc, lane DoD 8)
        // `2^32 as usize` truncated to 0 and a corrupt header yielded an
        // EMPTY payload view (`Ok`) — fail-open. These rows pin the
        // target-independent refusal contract.
        let mut frame = [0u8; FRAME_HEADER_LEN + 4];
        valid_header(4).encode_into(&mut frame).unwrap();
        let huge = valid_header(1u64 << 32); // 4 GiB declared
        assert_eq!(huge.payload(&frame), Err(FrameError::PayloadLenMismatch));
        let max = valid_header(u64::MAX);
        assert_eq!(max.payload(&frame), Err(FrameError::PayloadLenMismatch));
        // The lawful view still works beside the repaired gate.
        let ok = valid_header(4);
        assert_eq!(ok.payload(&frame).unwrap().len(), 4);
    }

    #[test]
    fn payload_view_bounds() {
        let mut frame = [0u8; FRAME_HEADER_LEN + 4];
        let h = valid_header(4);
        h.encode_into(&mut frame).unwrap();
        assert_eq!(h.payload(&frame).unwrap().len(), 4);
        let overrun = valid_header(5);
        assert_eq!(overrun.payload(&frame), Err(FrameError::PayloadLenMismatch));
        assert_eq!(valid_header(0).payload(&frame).unwrap(), &frame[128..128]);
    }

    #[test]
    fn short_buffers_refused_typed() {
        let h = valid_header(0);
        assert_eq!(h.encode_into(&mut [0u8; 127]), Err(FrameError::BufferTooSmall));
        assert_eq!(
            CgapFrameHeader::decode(&[0u8; 127]),
            Err(FrameError::BufferTooSmall)
        );
        let a = root_attestation();
        assert_eq!(a.encode_into(&mut [0u8; 79]), Err(FrameError::BufferTooSmall));
        assert_eq!(
            P28LicenseAttestation::decode(&[0u8; 79]),
            Err(FrameError::BufferTooSmall)
        );
    }
}
