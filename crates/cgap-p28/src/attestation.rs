//! Lane A17 — P28 license attestation record: Wave 1 ABI freeze + Wave 2 core
//! engines.
//!
//! Spec: `docs/jira/v26.11.1/lanes/A17-p28-derivative.md` §2 (normative
//! struct), §2.1 (byte-offset table, frozen at Wave 1), §2.2
//! (`attestation_flags` bit assignment), §3 (digest derivation), §4.2
//! (transpile transform), §5 (ordered validation). Wave 1 froze the ABI
//! (struct/consts/static asserts, zero logic). Wave 2 lands the core engines:
//! the LE wire codec (`to_bytes`/`from_bytes`, §2.1), the deterministic
//! `origin_authority_digest` derivation over the pinned instrument header
//! (§3), the transpile transform (§4.2), and total ordered validation with a
//! single opaque failure type (§5). This module is pure: no allocation, no
//! I/O on the library path; the halt edge lives in `trap.rs`. Wave 3 lands
//! the §4 link-section carriers: `find_in_elf` (§4.1, minimal ELF64 LE
//! section-table walk) and `find_in_wasm` (§4.2, custom-section walk) return
//! the payload byte span for the loader to slice into `from_bytes`; absence
//! is `None` and is a §5 step-1 validation failure for the caller (§5:
//! "absence is a validation failure, not a silent pass"). The full-form
//! §7.1/§7.2 courts — subprocess-per-mutation over real toolchain-built
//! artifacts — live in `trap.rs` beside the halt edge they witness.
//!
//! Every derivative artifact embeds this 80-byte record naming its license
//! lineage cryptographically (ADR-004, boundary invariant 5). The layout is
//! frozen with A01/A06/A11 at Wave 1; any widening or reorder after the
//! freeze is a breaking ABI change requiring a new lane, not a patch.
//!
//! This definition is the **authoritative** `P28LicenseAttestation` per the
//! v26.11.1 dispatch contract. `cgap-core`'s embedded copy (A04,
//! `crates/cgap-core/src/frame.rs`) is a cross-crate projection of the same
//! frozen table; re-verified pin-for-pin identical 2026-11-01 (wave 6):
//! offsets 0/4/6/8/16/48, size 80, align 8, magic `P28S` all match.

/// offset 0, 4 B — `'P','2','8','S'` = 0x50 0x32 0x38 0x53 (§2.1).
pub const LICENSE_MAGIC: [u8; 4] = *b"P28S";

/// offset 4, 2 B — u16 LE, currently 1 (§2.1). Unknown versions fail closed (§5.3).
pub const ATTESTATION_VERSION: u16 = 1;

/// bit 0 (§2.2): derives directly from the P28-SIL instrument; receipt all-zero.
pub const FLAG_ROOT: u16 = 1 << 0;

/// bit 1 (§2.2): derives from a Root artifact; `upstream_receipt` MUST be nonzero.
pub const FLAG_DERIVATIVE: u16 = 1 << 1;

/// bit 2 (§2.2): transpile target (beam -> WASM); receipt = transpiler's output receipt.
pub const FLAG_TRANSPILED: u16 = 1 << 2;

/// bits 0-2: exactly one MUST be set; zero or 2+ lineage bits = malformed (§2.2, §6).
pub const FLAG_LINEAGE_MASK: u16 = 0b0000_0111;

/// bits 3-15 (§2.2): reserved, MUST be 0; nonzero reserved bits are a
/// spoliation condition (§6).
pub const FLAG_RESERVED_MASK: u16 = 0xFFF8;

/// offset 8, 8 B — `safe_harbor_ceiling_cents` MUST be <= this (§2.1).
/// 5_000_000_000 cents = $50M. Frozen at Wave 1 with the ABI (§8.11).
pub const SAFE_HARBOR_CEILING_MAX_CENTS: u64 = 5_000_000_000;

/// Total record size in bytes (§2.1 offset table, end at 80).
pub const ATTESTATION_LEN: usize = 80;

/// 80-byte P28 license attestation (`#[repr(C, align(8))]`, §2).
///
/// No padding holes: offsets 0-7, 8-15, 16-47, 48-79 are fully occupied;
/// the `align(8)` guarantees the record starts on an 8-byte boundary inside
/// any link section (§2.1). All multi-byte fields little-endian on the wire.
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct P28LicenseAttestation {
    /// offset 0, 4 B — `b"P28S"` (`LICENSE_MAGIC`)
    pub license_magic: [u8; 4],
    /// offset 4, 2 B — u16 LE, currently 1 (`ATTESTATION_VERSION`)
    pub version: u16,
    /// offset 6, 2 B — u16 LE bitfield (§2.2)
    pub attestation_flags: u16,
    /// offset 8, 8 B — u64 LE; MUST be <= 5_000_000_000 ($50M)
    pub safe_harbor_ceiling_cents: u64,
    /// offset 16, 32 B — BLAKE3 over the pinned instrument header (§3)
    pub origin_authority_digest: [u8; 32],
    /// offset 48, 32 B — BLAKE3 of parent receipt; zero only if Root
    pub upstream_receipt: [u8; 32],
}

// ---- ABI freeze (§2.1, §8.2): any widening/reorder here must fail the build ----
// Core-only const asserts, no `static_assertions` crate (zero-dependency rule).

const _: () = assert!(core::mem::size_of::<P28LicenseAttestation>() == ATTESTATION_LEN);
const _: () = assert!(ATTESTATION_LEN == 80);
const _: () = assert!(core::mem::align_of::<P28LicenseAttestation>() == 8);

// §2.1 byte-offset table, machine-checked at compile time (`offset_of!`,
// stable 1.77). Each pin is one table row; a field reorder that preserves
// total size still trips these.
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, license_magic) == 0);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, version) == 4);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, attestation_flags) == 6,);
const _: () =
    assert!(core::mem::offset_of!(P28LicenseAttestation, safe_harbor_ceiling_cents) == 8,);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, origin_authority_digest) == 16,);
const _: () = assert!(core::mem::offset_of!(P28LicenseAttestation, upstream_receipt) == 48,);

// Field-width pins: the digest/receipt spans (16..48, 48..80) must stay
// exactly 32 bytes each or the §2.1 "fully occupied" invariant breaks.
const _: () = assert!(LICENSE_MAGIC.len() == 4);
const _: () = assert!(
    core::mem::size_of::<[u8; 32]>() == 32, // digest + receipt span pins
);

// ===========================================================================
// Wave 2 — core engines: wire codec (§2.1), digest derivation (§3), ordered
// validation (§5), transpile transform (§4.2). Pure/total: no allocation, no
// I/O on the library path. The halt edge is `trap.rs` (§6.2).
// ===========================================================================

/// Opaque spoliation failure (§5, §6). Deliberately carries no variant, no
/// code, no detail: every one of the §5 checks maps to this single halt
/// class so an attacker cannot fingerprint WHICH check failed (§5 preamble:
/// "first failure wins, all failures map to the same halt class"). The
/// process-halting edge for this condition is `trap.rs::spoliation_halt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Spoliation;

impl core::fmt::Display for Spoliation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Non-fingerprinting by construction: one message for every cause.
        f.write_str("p28 spoliation: license attestation invariant violated (fail-closed)")
    }
}

impl std::error::Error for Spoliation {}

impl P28LicenseAttestation {
    /// §2.1 wire serialization — little-endian per the frozen offset table,
    /// exactly 80 bytes, no padding holes.
    pub fn to_bytes(&self) -> [u8; ATTESTATION_LEN] {
        let mut b = [0u8; ATTESTATION_LEN];
        b[0..4].copy_from_slice(&self.license_magic);
        b[4..6].copy_from_slice(&self.version.to_le_bytes());
        b[6..8].copy_from_slice(&self.attestation_flags.to_le_bytes());
        b[8..16].copy_from_slice(&self.safe_harbor_ceiling_cents.to_le_bytes());
        b[16..48].copy_from_slice(&self.origin_authority_digest);
        b[48..80].copy_from_slice(&self.upstream_receipt);
        b
    }

    /// §2.1 wire parse (§4.2: "one parser serves both formats"). Total with
    /// respect to length: any input that is not exactly 80 bytes is a parse
    /// failure — the §5 step-1 "section absent/truncated" case collapses to
    /// this at the record layer. Parsing is pure decode; the semantic §5
    /// checks live in [`Self::validate`]/[`Self::validate_intrinsic`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Spoliation> {
        if bytes.len() != ATTESTATION_LEN {
            return Err(Spoliation);
        }
        let mut rec = Self {
            license_magic: [0u8; 4],
            version: 0,
            attestation_flags: 0,
            safe_harbor_ceiling_cents: 0,
            origin_authority_digest: [0u8; 32],
            upstream_receipt: [0u8; 32],
        };
        rec.license_magic.copy_from_slice(&bytes[0..4]);
        rec.version = u16::from_le_bytes(bytes[4..6].try_into().expect("6..8 is 2 bytes"));
        rec.attestation_flags =
            u16::from_le_bytes(bytes[6..8].try_into().expect("6..8 is 2 bytes"));
        rec.safe_harbor_ceiling_cents =
            u64::from_le_bytes(bytes[8..16].try_into().expect("8..16 is 8 bytes"));
        rec.origin_authority_digest.copy_from_slice(&bytes[16..48]);
        rec.upstream_receipt.copy_from_slice(&bytes[48..80]);
        Ok(rec)
    }

    /// §2.2/§4.2 transpile transform: a byte-preserving copy of the parent
    /// record plus exactly two field edits — the lineage field becomes
    /// [`FLAG_TRANSPILED`] and `upstream_receipt` becomes the transpiler's
    /// output receipt. The §2.2 exactly-one-lineage-bit invariant forces
    /// clearing the parent's lineage bits ("sets bit 2" cannot keep bit 0/1
    /// or the child would be malformed). Asserted by pure-function equality
    /// in the §7.3 positive controls.
    pub fn transpile(parent: &Self, transpiler_output_receipt: [u8; 32]) -> Self {
        let mut child = *parent;
        child.attestation_flags = (parent.attestation_flags & !FLAG_LINEAGE_MASK) | FLAG_TRANSPILED;
        child.upstream_receipt = transpiler_output_receipt;
        child
    }

    /// §5 steps 2-5 and 8 — the record-intrinsic subset that the C-ABI
    /// `cgap_p28_invariant_check` (trap.rs) can run with no context: magic,
    /// version, flag shape, Root/receipt consistency, ceiling. Ordered;
    /// every failure is the one opaque [`Spoliation`] class.
    pub fn validate_intrinsic(&self) -> Result<(), Spoliation> {
        // §5.2 — magic
        if self.license_magic != LICENSE_MAGIC {
            return Err(Spoliation);
        }
        // §5.3 — version (unknown version is fail-closed)
        if self.version != ATTESTATION_VERSION {
            return Err(Spoliation);
        }
        // §5.4 — exactly one lineage bit set; reserved bits all zero
        let lineage = self.attestation_flags & FLAG_LINEAGE_MASK;
        if lineage.count_ones() != 1 {
            return Err(Spoliation);
        }
        if self.attestation_flags & FLAG_RESERVED_MASK != 0 {
            return Err(Spoliation);
        }
        // §5.5 — FLAG_ROOT xor zero receipt: Root ⇒ receipt all-zero;
        // Derivative/Transpiled ⇒ receipt MUST be nonzero.
        let is_root = lineage == FLAG_ROOT;
        let receipt_zero = self.upstream_receipt == [0u8; 32];
        if is_root != receipt_zero {
            return Err(Spoliation);
        }
        // §5.8 — ceiling
        if self.safe_harbor_ceiling_cents > SAFE_HARBOR_CEILING_MAX_CENTS {
            return Err(Spoliation);
        }
        Ok(())
    }

    /// §5 full ordered validation, first failure wins; every failure maps to
    /// the single opaque [`Spoliation`] halt class. `instrument_header` is
    /// the canonical §3 span of the pinned instrument (see
    /// [`instrument_header_span`]); `parent` is the parent artifact's record.
    ///
    /// Fail-closed lineage readings (lane deviation D-A17-2, recorded):
    /// a Derivative/Transpiled record validated **without** its parent is
    /// spoliation — an unverifiable lineage never admits; a Root record
    /// validated **with** a parent context is spoliation — the presented
    /// chain contradicts the lineage claim. There is no
    /// degraded-but-continue mode (§5).
    pub fn validate(
        &self,
        instrument_header: &[u8],
        parent: Option<&Self>,
    ) -> Result<(), Spoliation> {
        // §5.2-§5.5 — intrinsic prefix, in doc order.
        if self.license_magic != LICENSE_MAGIC {
            return Err(Spoliation);
        }
        if self.version != ATTESTATION_VERSION {
            return Err(Spoliation);
        }
        let lineage = self.attestation_flags & FLAG_LINEAGE_MASK;
        if lineage.count_ones() != 1 {
            return Err(Spoliation);
        }
        if self.attestation_flags & FLAG_RESERVED_MASK != 0 {
            return Err(Spoliation);
        }
        let is_root = lineage == FLAG_ROOT;
        let receipt_zero = self.upstream_receipt == [0u8; 32];
        if is_root != receipt_zero {
            return Err(Spoliation);
        }
        // §5.6 — origin authority digest recomputed from the pinned
        // instrument bytes must match.
        if self.origin_authority_digest != origin_authority_digest(instrument_header) {
            return Err(Spoliation);
        }
        // §5.7 — upstream receipt binds to the parent record's wire bytes
        // (§2: "BLAKE3 of parent receipt"), only when the lineage claims a
        // parent; fail-closed when the chain is absent or contradictory.
        match (is_root, parent) {
            (true, None) => {}
            (true, Some(_)) => return Err(Spoliation),
            (false, None) => return Err(Spoliation),
            (false, Some(p)) => {
                if self.upstream_receipt != cgap_blake3::hash(&p.to_bytes()) {
                    return Err(Spoliation);
                }
            }
        }
        // §5.8 — ceiling, last per the §5 ordering.
        if self.safe_harbor_ceiling_cents > SAFE_HARBOR_CEILING_MAX_CENTS {
            return Err(Spoliation);
        }
        Ok(())
    }
}

/// §3 — extract the canonical instrument-header span: bytes from the file
/// start through the first line that is exactly `---`, inclusive of its
/// newline. Pure; the caller supplies the on-disk instrument bytes (the
/// library path does no I/O, §6.2). Returns `None` when no separator line
/// exists — the caller then has no canonical span and must not derive a
/// digest from an unpinned byte range.
pub fn instrument_header_span(full: &[u8]) -> Option<&[u8]> {
    let mut start = 0usize;
    while start < full.len() {
        let end = match full[start..].iter().position(|&b| b == b'\n') {
            Some(p) => start + p + 1,
            None => full.len(),
        };
        let mut line = &full[start..end];
        if line.ends_with(b"\n") {
            line = &line[..line.len() - 1];
        }
        if line.ends_with(b"\r") {
            line = &line[..line.len() - 1];
        }
        if line == b"---" {
            return Some(&full[..end]);
        }
        start = end;
    }
    None
}

/// §3 — deterministic, replayable origin authority digest: BLAKE3-256 over
/// the canonical instrument-header span, via the in-repo `cgap-blake3` crate
/// ("identical to A06's substrate, so all v26.11.1 lanes derive digests from
/// one hash identity"). Two derivations over the same bytes are
/// byte-identical; the §7.3 determinism control pins this against the real
/// instrument on disk.
pub fn origin_authority_digest(instrument_header: &[u8]) -> [u8; 32] {
    cgap_blake3::hash(instrument_header)
}

// ===========================================================================
// Wave 3 — §4 link-section carriers: locate the 80-byte record inside a real
// artifact. `find_in_elf` walks the ELF64 section table for `.p28_invariant`
// (§4.1); `find_in_wasm` walks the module's sections for the `p28_invariant`
// custom section (§4.2, no leading dot). Both return the payload BYTE SPAN
// (§4.3: "cgap-p28 provides from_bytes/to_bytes and the validator" — the
// loader slices the span and hands it to [`P28LicenseAttestation::from_bytes`]).
// Every malformed structure is `None` — for the caller that is §5 step 1:
// the artifact is not admitted, fail-closed, never a silent pass.
// ===========================================================================

/// §4.1 — locate `.p28_invariant` in an ELF image and return its payload byte
/// span. Minimal, allocation-free ELF64 little-endian section-header walk
/// (§9: fields are LE; big-endian targets must byteswap before validate —
/// parsed as absent here, which is fail-closed). Anti-vacuity mutation named
/// by its court: returning `None` unconditionally, or a wrong span, flips
/// `elf_carrier_round_trip_court` and every downstream loader court.
pub fn find_in_elf(elf: &[u8]) -> Option<core::ops::Range<usize>> {
    // e_ident: magic, ELFCLASS64 (1), little-endian data (2), current version.
    if elf.len() < 64 || elf[0..4] != *b"\x7fELF" {
        return None;
    }
    if elf[4] != 2 || elf[5] != 1 || elf[6] != 1 {
        return None;
    }
    let u16_at = |off: usize| -> Option<u16> {
        Some(u16::from_le_bytes(elf.get(off..off + 2)?.try_into().ok()?))
    };
    let u64_at = |off: usize| -> Option<u64> {
        Some(u64::from_le_bytes(elf.get(off..off + 8)?.try_into().ok()?))
    };
    // Fail-closed on extended numbering (e_shnum == 0 escape, SHN_XINDEX):
    // this loader admits only the plain section table.
    let shoff = u64_at(0x28)? as usize;
    let shentsize = u16_at(0x3A)? as usize;
    let shnum = u16_at(0x3C)? as usize;
    let shstrndx = u16_at(0x3E)? as usize;
    if shentsize != 64 || shnum == 0 || shstrndx >= shnum {
        return None;
    }
    let shdr = |i: usize| -> Option<(u32, u64, u64)> {
        // Every offset derived from the untrusted `e_shoff` is checked:
        // unchecked `base + K` overflowed into a debug-build panic on
        // hostile bytes (a03-wave5 F-1 class, repaired wave 6 — a public
        // parser of untrusted bytes must refuse typed, never panic).
        let base = shoff.checked_add(i.checked_mul(shentsize)?)?;
        let name = u32::from_le_bytes(
            elf.get(base..base.checked_add(4)?)?.try_into().ok()?,
        );
        let off = u64_at(base.checked_add(0x18)?)?;
        let size = u64_at(base.checked_add(0x20)?)?;
        Some((name, off, size))
    };
    // Section-header string table (the names C-string pool). NOTE: no
    // sh_name == 0 requirement — Apple clang reuses `.strtab` as the
    // section-name table (e_shstrndx -> a section with a nonzero sh_name),
    // and the ELF spec pins only the table INDEX, not the name convention.
    let (_, stroff, strsize) = shdr(shstrndx)?;
    if strsize == 0 {
        return None;
    }
    let stroff = stroff as usize;
    let strsize = strsize as usize;
    let strtab = elf.get(stroff..stroff.checked_add(strsize)?)?;
    let cstr = |off: u32| -> Option<&[u8]> {
        let start = off as usize;
        let rest = strtab.get(start..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        Some(&rest[..end])
    };
    for i in 0..shnum {
        let (name, off, size) = shdr(i)?;
        if cstr(name) == Some(&b".p28_invariant"[..]) {
            let start = usize::try_from(off).ok()?;
            let end = start.checked_add(usize::try_from(size).ok()?)?;
            return Some(start..end).filter(|r| r.end <= elf.len());
        }
    }
    None
}

/// §4.2 — locate the `p28_invariant` custom section (no leading dot) in a
/// WASM binary and return its payload byte span, so one parser
/// ([`P28LicenseAttestation::from_bytes`]) serves both formats. Walks the
/// section stream: id byte + u32 LEB128 size; id 0 is a custom section whose
/// body is a LEB128-prefixed name followed by the payload. Truncation, LEB
/// overflow, or a size overrun parse as absent — fail-closed.
pub fn find_in_wasm(wasm: &[u8]) -> Option<core::ops::Range<usize>> {
    if wasm.len() < 8 || wasm[0..4] != [0x00, 0x61, 0x73, 0x6D] {
        return None;
    }
    if wasm[4..8] != [0x01, 0x00, 0x00, 0x00] {
        return None;
    }
    let mut pos = 8usize;
    while pos < wasm.len() {
        let id = *wasm.get(pos)?;
        pos += 1;
        let (size, after_len) = leb_u32(wasm, pos)?;
        pos = after_len;
        let end = pos.checked_add(size as usize)?;
        if end > wasm.len() {
            return None;
        }
        if id == 0 {
            let (nlen, name_start) = leb_u32(wasm, pos)?;
            let name_end = name_start.checked_add(nlen as usize)?;
            if name_end <= end && &wasm[name_start..name_end] == b"p28_invariant" {
                return Some(name_end..end);
            }
        }
        pos = end;
    }
    None
}

/// u32 LEB128 decode used by [`find_in_wasm`]: at most 5 bytes (ceil(32/7)),
/// no 64-bit overflow; returns `(value, offset_past_encoding)`.
fn leb_u32(bytes: &[u8], pos: usize) -> Option<(u32, usize)> {
    let mut value: u32 = 0;
    let mut shift = 0u32;
    for i in 0..5 {
        let byte = *bytes.get(pos.checked_add(i)?)?;
        value |= u32::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some((value, pos + i + 1));
        }
        shift += 7;
    }
    None
}

/// Test-only support shared by the A17 courts in this module and by the
/// `trap.rs` loader courts: real-artifact builders (real toolchain paths plus
/// minimal in-test writers), the independent differential ELF witness, and
/// the record factories. Nothing here ships; everything real (subprocesses,
/// real bytes on disk) is used in preference to fixtures (Chicago).
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;

    /// Canonical instrument bytes, read from the real pinned file on disk.
    pub(crate) fn instrument_file_bytes() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../LICENSE-P28-SIL.md"
        ))
        .expect("pinned instrument LICENSE-P28-SIL.md is readable from the repo root")
    }

    /// §3 canonical span of the real pinned instrument.
    pub(crate) fn real_instrument_span() -> Vec<u8> {
        crate::attestation::instrument_header_span(&instrument_file_bytes())
            .map(|s| s.to_vec())
            .expect("canonical instrument has a `---` line")
    }

    pub(crate) fn synthetic_instrument() -> Vec<u8> {
        let mut v = b"# Synthetic instrument header\n".to_vec();
        v.extend_from_slice(b"\nbody\n---\n");
        v.extend_from_slice(b"tail that must not enter the span\n");
        v
    }

    pub(crate) fn root_for(instrument: &[u8]) -> super::P28LicenseAttestation {
        super::P28LicenseAttestation {
            license_magic: super::LICENSE_MAGIC,
            version: super::ATTESTATION_VERSION,
            attestation_flags: super::FLAG_ROOT,
            safe_harbor_ceiling_cents: super::SAFE_HARBOR_CEILING_MAX_CENTS,
            origin_authority_digest: super::origin_authority_digest(instrument),
            upstream_receipt: [0u8; 32],
        }
    }

    pub(crate) fn derivative_from(
        parent: &super::P28LicenseAttestation,
    ) -> super::P28LicenseAttestation {
        super::P28LicenseAttestation {
            license_magic: super::LICENSE_MAGIC,
            version: super::ATTESTATION_VERSION,
            attestation_flags: super::FLAG_DERIVATIVE,
            safe_harbor_ceiling_cents: super::SAFE_HARBOR_CEILING_MAX_CENTS,
            origin_authority_digest: parent.origin_authority_digest,
            upstream_receipt: cgap_blake3::hash(&parent.to_bytes()),
        }
    }

    /// §7.1 D-A17-1 re-derived survivor law: with the ceiling pinned at MAX,
    /// the only tolerated single-bit mutations are CLEAR-flips of the set
    /// bits of the ceiling field (bytes 8..16).
    pub(crate) fn predicted_ceiling_survivors(good: &[u8; 80]) -> Vec<(usize, u8)> {
        let mut v = Vec::new();
        for i in 8..16 {
            for j in 0..8u8 {
                if good[i] & (1 << j) != 0 {
                    v.push((i, j));
                }
            }
        }
        v
    }

    /// Is `name` on PATH and runnable? (Loud-[SKIP] gate for toolchain-backed
    /// courts: a missing tool is a counted skip, never a silent pass.)
    pub(crate) fn tool_available(name: &str) -> bool {
        std::process::Command::new(name)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Per-test scratch directory (unique per process id; callers clean up).
    pub(crate) fn court_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cgap-p28-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create court temp dir");
        dir
    }

    /// §4.1 emission via a REAL toolchain path: Apple clang targeting
    /// x86_64-linux emits a genuine ELF64 relocatable object whose
    /// `.p28_invariant` section carries the record bytes (the `#[used]
    /// #[link_section]` Rust pattern from §4.1, expressed in the C frontend
    /// so a linked Linux executable is not required on this host).
    pub(crate) fn cc_elf(record: &[u8; 80], dir: &Path) -> Vec<u8> {
        let init: String = record.iter().map(|b| format!("{b},")).collect();
        let src = format!(
            "__attribute__((used)) __attribute__((section(\".p28_invariant\"))) \
             unsigned char cgap_p28_att[80] = {{{init}}};\n"
        );
        let cpath = dir.join("p28_att.c");
        let opath = dir.join("p28_att.o");
        std::fs::write(&cpath, src).expect("write C emission source");
        let st = std::process::Command::new("cc")
            .args(["-target", "x86_64-unknown-linux-gnu", "-c", "-o"])
            .arg(&opath)
            .arg(&cpath)
            .output()
            .expect("spawn cc for ELF emission");
        assert!(st.status.success(), "cc ELF emission failed: {st:?}");
        std::fs::read(&opath).expect("read emitted ELF object")
    }

    /// Negative-control ELF from the same real toolchain: no such section.
    pub(crate) fn cc_elf_without_section(dir: &Path) -> Vec<u8> {
        let cpath = dir.join("p28_bare.c");
        let opath = dir.join("p28_bare.o");
        std::fs::write(&cpath, "unsigned char unrelated[4] = {1,2,3,4};\n")
            .expect("write bare C source");
        let st = std::process::Command::new("cc")
            .args(["-target", "x86_64-unknown-linux-gnu", "-c", "-o"])
            .arg(&opath)
            .arg(&cpath)
            .output()
            .expect("spawn cc for bare object");
        assert!(st.status.success(), "cc bare emission failed: {st:?}");
        std::fs::read(&opath).expect("read bare ELF object")
    }

    /// §4.2 emission via a REAL toolchain path: wabt `wat2wasm` emits a real
    /// WASM module whose `p28_invariant` custom section carries the record
    /// bytes. `None` builds the stripped module (§7.2 WASM strip).
    pub(crate) fn wat_wasm(record: Option<&[u8; 80]>, dir: &Path) -> Vec<u8> {
        let mut wat = String::from("(module (func (export \"nop\"))");
        if let Some(rec) = record {
            let esc: String = rec.iter().map(|b| format!("\\{b:02x}")).collect();
            wat.push_str(&format!(" (@custom \"p28_invariant\" \"{esc}\")"));
        }
        wat.push_str(")\n");
        let wpath = dir.join("m.wat");
        let bpath = dir.join("m.wasm");
        std::fs::write(&wpath, wat).expect("write WAT source");
        let st = std::process::Command::new("wat2wasm")
            .arg(&wpath)
            .arg("-o")
            .arg(&bpath)
            .output()
            .expect("spawn wat2wasm");
        assert!(
            st.status.success(),
            "wat2wasm failed: {}",
            String::from_utf8_lossy(&st.stderr)
        );
        std::fs::read(&bpath).expect("read emitted wasm module")
    }

    fn push_leb(out: &mut Vec<u8>, mut v: u32) {
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    /// Second WASM producer (§7.3 differential): the minimal spec module —
    /// magic + version + one custom section — built by the test itself.
    pub(crate) fn hand_wasm(record: Option<&[u8; 80]>) -> Vec<u8> {
        let mut out = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        if let Some(rec) = record {
            let mut sec = Vec::new();
            push_leb(&mut sec, b"p28_invariant".len() as u32);
            sec.extend_from_slice(b"p28_invariant");
            sec.extend_from_slice(rec);
            out.push(0); // custom section id
            push_leb(&mut out, sec.len() as u32);
            out.extend_from_slice(&sec);
        }
        out
    }

    /// Second ELF producer (§7.3 differential): a minimal but structurally
    /// genuine ELF64 LE relocatable — header, `.text`, `.shstrtab`, and the
    /// `.p28_invariant` section when `Some`. `None` is the §7.2 strip form
    /// (section header and contents both removed from the table).
    pub(crate) fn hand_elf(record: Option<&[u8; 80]>) -> Vec<u8> {
        const TEXT: [u8; 16] = [0x90; 16];
        let text_at = 64usize;
        let payload_at = text_at + TEXT.len();
        let with = record.is_some();
        let shstr_at = if with { payload_at + 80 } else { payload_at };
        // "\0" ".text\0" ".shstrtab\0" (".p28_invariant\0")?
        let mut names: Vec<u8> = vec![0];
        names.extend_from_slice(b".text\0");
        names.extend_from_slice(b".shstrtab\0");
        if with {
            names.extend_from_slice(b".p28_invariant\0");
        }
        let shoff_raw = shstr_at + names.len();
        let shoff = shoff_raw.div_ceil(8) * 8;
        let shnum: usize = if with { 4 } else { 3 };

        let mut out = vec![0u8; shoff + shnum * 64];
        out[0..4].copy_from_slice(b"\x7fELF");
        out[4] = 2; // ELFCLASS64
        out[5] = 1; // little-endian
        out[6] = 1; // EV_CURRENT
        out[16..18].copy_from_slice(&1u16.to_le_bytes()); // ET_REL
        out[18..20].copy_from_slice(&0x3Eu16.to_le_bytes()); // EM_X86_64
        out[20..24].copy_from_slice(&1u32.to_le_bytes()); // e_version
        out[40..48].copy_from_slice(&(shoff as u64).to_le_bytes()); // e_shoff
        out[52..54].copy_from_slice(&64u16.to_le_bytes()); // e_ehsize
        out[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
        out[60..62].copy_from_slice(&(shnum as u16).to_le_bytes()); // e_shnum
        out[62..64].copy_from_slice(&2u16.to_le_bytes()); // e_shstrndx = .shstrtab

        out[text_at..text_at + TEXT.len()].copy_from_slice(&TEXT);
        out[shstr_at..shstr_at + names.len()].copy_from_slice(&names);
        if let Some(rec) = record {
            out[payload_at..payload_at + 80].copy_from_slice(rec);
        }

        let mut shdr =
            |idx: usize, name: u32, typ: u32, flags: u64, off: u64, size: u64, align: u64| {
                let base = shoff + idx * 64;
                out[base..base + 4].copy_from_slice(&name.to_le_bytes());
                out[base + 4..base + 8].copy_from_slice(&typ.to_le_bytes());
                out[base + 8..base + 16].copy_from_slice(&flags.to_le_bytes());
                out[base + 24..base + 32].copy_from_slice(&off.to_le_bytes());
                out[base + 32..base + 40].copy_from_slice(&size.to_le_bytes());
                out[base + 48..base + 56].copy_from_slice(&align.to_le_bytes());
            };
        shdr(0, 0, 0, 0, 0, 0, 0); // SHT_NULL
        shdr(1, 1, 1, 0x6, text_at as u64, TEXT.len() as u64, 16); // .text AX
        shdr(2, 7, 3, 0, shstr_at as u64, names.len() as u64, 1); // .shstrtab
        if with {
            // §4.1: SHT_PROGBITS, SHF_ALLOC; addralign 8. Name offset 17:
            // "\0"(0) + ".text\0"(1) + ".shstrtab\0"(7) -> 7+10 = 17.
            shdr(3, 17, 1, 0x2, payload_at as u64, 80, 8);
        }
        out
    }

    /// Independent differential ELF witness (§7.3 anti-tautology): a second
    /// parser — python3's struct module walking the same section table —
    /// must agree with `find_in_elf` on every section (name, offset, size).
    /// Returns `None` when python3 is unavailable (callers loud-[SKIP]).
    pub(crate) fn python_elf_sections(path: &Path) -> Option<Vec<(String, u64, u64)>> {
        let script = "\
import struct,sys
f=open(sys.argv[1],'rb').read()
shoff,=struct.unpack_from('<Q',f,0x28)
es,sn,sx=struct.unpack_from('<HHH',f,0x3A)
so,ss=struct.unpack_from('<QQ',f,shoff+sx*es+0x18)
st=f[so:so+ss]
for i in range(sn):
    b=shoff+i*es
    n,=struct.unpack_from('<I',f,b)
    e=st.index(b'\\x00',n)
    o,s=struct.unpack_from('<QQ',f,b+0x18)
    print(st[n:e].decode(),o,s,sep='\\t')
";
        let out = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(path)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|line| {
                let mut it = line.split('\t');
                let name = it.next().unwrap_or_default().to_string();
                let off = it.next().unwrap_or_default().parse().unwrap_or(0);
                let size = it.next().unwrap_or_default().parse().unwrap_or(0);
                Some((name, off, size))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use std::process::Command;

    /// §2.1 offset table rendered as a runtime witness (court for the table
    /// itself, not the struct — the const pins above are the primary gate).
    /// Anti-vacuity: any edit that changes a field's offset or the total
    /// size flips this test and the build. A test asserting the table rows
    /// is not removable without deleting the table claim.
    #[test]
    fn offset_table_matches_frozen_layout() {
        assert_eq!(size_of::<P28LicenseAttestation>(), ATTESTATION_LEN);
        assert_eq!(
            core::mem::offset_of!(P28LicenseAttestation, license_magic),
            0
        );
        assert_eq!(core::mem::offset_of!(P28LicenseAttestation, version), 4);
        assert_eq!(
            core::mem::offset_of!(P28LicenseAttestation, attestation_flags),
            6
        );
        assert_eq!(
            core::mem::offset_of!(P28LicenseAttestation, safe_harbor_ceiling_cents),
            8
        );
        assert_eq!(
            core::mem::offset_of!(P28LicenseAttestation, origin_authority_digest),
            16
        );
        assert_eq!(
            core::mem::offset_of!(P28LicenseAttestation, upstream_receipt),
            48
        );
    }

    /// Flag-bit freeze (§2.2): the three lineage bits are distinct and live
    /// in bits 0-2; the reserved mask is their exact complement in the u16.
    /// Anti-vacuity: any flag-value edit (shift, mask widening) flips this.
    #[test]
    fn flag_bits_are_frozen() {
        assert_eq!(FLAG_ROOT, 0x0001);
        assert_eq!(FLAG_DERIVATIVE, 0x0002);
        assert_eq!(FLAG_TRANSPILED, 0x0004);
        assert_eq!(
            FLAG_LINEAGE_MASK,
            FLAG_ROOT | FLAG_DERIVATIVE | FLAG_TRANSPILED
        );
        assert_eq!(
            FLAG_LINEAGE_MASK | FLAG_RESERVED_MASK,
            u16::MAX,
            "lineage + reserved must tile the full u16 flag space"
        );
        assert_eq!(FLAG_LINEAGE_MASK & FLAG_RESERVED_MASK, 0);
    }

    // =======================================================================
    // Wave 2 courts — §7 test plan (core-engine form; the subprocess-per-
    // mutation sweep over real ELF/WASM artifacts is the later-wave court).
    // =======================================================================

    // Record factories and the real pinned instrument reader moved to
    // `test_support` (shared with the trap.rs loader courts) — imported via
    // `use super::test_support::*;` above; call sites are unchanged.

    /// §2.1 codec: exact little-endian byte pattern asserted by offset, and
    /// the from_bytes round trip. Anti-vacuity: a codec that reorders fields
    /// or flips endianness fails the hand-computed expectations below.
    #[test]
    fn codec_exact_le_bytes_and_round_trip() {
        let instrument = synthetic_instrument();
        let rec = root_for(&instrument);
        let b = rec.to_bytes();
        assert_eq!(b.len(), ATTESTATION_LEN);
        assert_eq!(&b[0..4], b"P28S");
        assert_eq!(&b[4..6], &[0x01, 0x00], "version 1, u16 LE");
        assert_eq!(&b[6..8], &[0x01, 0x00], "FLAG_ROOT, u16 LE");
        assert_eq!(
            &b[8..16],
            &[0x00, 0xF2, 0x05, 0x2A, 0x01, 0x00, 0x00, 0x00],
            "5_000_000_000 = 0x000000012A05F200, u64 LE"
        );
        assert_eq!(&b[16..48], &rec.origin_authority_digest);
        assert_eq!(&b[48..80], &[0u8; 32]);
        assert_eq!(
            P28LicenseAttestation::from_bytes(&b).expect("80 bytes parse"),
            rec
        );
    }

    /// §5 step 1 (parse-level): any length other than exactly 80 fails —
    /// the truncated/stripped-section case collapses to a parse failure.
    #[test]
    fn from_bytes_rejects_non_80_lengths() {
        let instrument = synthetic_instrument();
        let good = root_for(&instrument).to_bytes();
        let mut long = [0u8; 160];
        long[..ATTESTATION_LEN].copy_from_slice(&good);
        for len in [0usize, 1, 4, 79, 81, 160] {
            assert!(
                P28LicenseAttestation::from_bytes(&long[..len]).is_err(),
                "length {len} must not parse"
            );
        }
    }

    /// §3 span extraction: on the real instrument the span is a prefix that
    /// starts at the H1 and ends at the first `---` line; on synthetic bytes
    /// the span stops at the FIRST separator (later `---` lines are tail).
    #[test]
    fn instrument_span_extraction() {
        let file = instrument_file_bytes();
        let span = instrument_header_span(&file).expect("canonical instrument has a `---` line");
        assert!(span.len() < file.len());
        assert!(span.starts_with(b"# P28 Sovereign Invariant License"));
        assert!(span.ends_with(b"---\n"));
        // Exactly one separator line inside the span (it is the last line).
        let sep_count = span
            .split(|&b| b == b'\n')
            .filter(|line| line == b"---")
            .count();
        assert_eq!(sep_count, 1, "span ends at the FIRST separator");

        let syn = synthetic_instrument();
        let syn_span = instrument_header_span(&syn).expect("synthetic has a separator");
        assert_eq!(
            syn_span,
            &b"# Synthetic instrument header\n\nbody\n---\n"[..]
        );

        assert!(instrument_header_span(b"no separator here\n").is_none());
    }

    /// §3/§7.3: the origin authority digest derives deterministically from
    /// the REAL pinned instrument bytes, is byte-identical across two
    /// independent derivations, and equals the receipt-pinned constant
    /// (instrument drift — a LICENSE-P28-SIL.md edit — fails this test
    /// loudly and forces a re-attestation run, §9 "Instrument drift").
    #[test]
    fn origin_digest_determinism_and_pin() {
        let file = instrument_file_bytes();
        let span = instrument_header_span(&file).expect("canonical instrument has a `---` line");
        let d1 = origin_authority_digest(span);
        let d2 = origin_authority_digest(span);
        assert_eq!(d1, d2, "BLAKE3 derivation is deterministic");
        assert_eq!(d1, PINNED_ORIGIN_AUTHORITY_DIGEST);
        assert_ne!(d1, [0u8; 32]);
    }

    /// §7.3 positive controls: an untampered Root admits; an untampered
    /// Derivative admits against its parent chain (synthetic instrument);
    /// the real-instrument Root admits against the real pinned bytes.
    #[test]
    fn validate_positive_controls() {
        let syn = synthetic_instrument();
        let root = root_for(&syn);
        assert!(root.validate(&syn, None).is_ok());
        let deriv = derivative_from(&root);
        assert!(deriv.validate(&syn, Some(&root)).is_ok());

        let file = instrument_file_bytes();
        let span = instrument_header_span(&file).expect("canonical instrument has a `---` line");
        let real_root = root_for(span);
        assert!(real_root.validate(span, None).is_ok());
    }

    /// §5 steps 2-8, one single-fault violation per step, each from an
    /// otherwise-valid record: every violation maps to the one opaque
    /// [`Spoliation`] class (non-fingerprinting, §5 preamble).
    #[test]
    fn validate_rejects_each_s5_step_violation() {
        let syn = synthetic_instrument();
        let root = root_for(&syn);
        let deriv = derivative_from(&root);

        // §5.2 magic
        let mut bad = root;
        bad.license_magic = *b"P28X";
        assert!(bad.validate(&syn, None).is_err());
        // §5.3 version
        let mut bad = root;
        bad.version = 2;
        assert!(bad.validate(&syn, None).is_err());
        // §5.4 zero lineage bits
        let mut bad = root;
        bad.attestation_flags = 0;
        assert!(bad.validate(&syn, None).is_err());
        // §5.4 two lineage bits
        let mut bad = root;
        bad.attestation_flags = FLAG_ROOT | FLAG_DERIVATIVE;
        assert!(bad.validate(&syn, None).is_err());
        // §5.4 reserved bit set
        let mut bad = root;
        bad.attestation_flags = FLAG_ROOT | 0x0008;
        assert!(bad.validate(&syn, None).is_err());
        // §5.5 Root with nonzero receipt
        let mut bad = root;
        bad.upstream_receipt = [1u8; 32];
        assert!(bad.validate(&syn, None).is_err());
        // §5.5 Derivative with zeroed receipt (§7.2 receipt stripping)
        let mut bad = deriv;
        bad.upstream_receipt = [0u8; 32];
        assert!(bad.validate(&syn, Some(&root)).is_err());
        // §5.6 wrong instrument bytes
        let other = b"# a different instrument\n\n---\n";
        assert!(root.validate(other, None).is_err());
        // §5.6 digest tampered directly
        let mut bad = root;
        bad.origin_authority_digest[0] ^= 0xFF;
        assert!(bad.validate(&syn, None).is_err());
        // §5.7 Derivative without parent (fail-closed, D-A17-2)
        assert!(deriv.validate(&syn, None).is_err());
        // §5.7 wrong parent in the chain
        let wrong_parent = root_for(other);
        assert!(deriv.validate(&syn, Some(&wrong_parent)).is_err());
        // §5.8 ceiling above max
        let mut bad = root;
        bad.safe_harbor_ceiling_cents = SAFE_HARBOR_CEILING_MAX_CENTS + 1;
        assert!(bad.validate(&syn, None).is_err());
        assert!(bad.validate_intrinsic().is_err());
    }

    /// D-A17-2 fail-closed lineage readings: Root + parent context is
    /// spoliation (contradictory chain); validate_intrinsic (no context)
    /// still admits the intrinsically-valid Root.
    #[test]
    fn validate_lineage_context_is_fail_closed() {
        let syn = synthetic_instrument();
        let root = root_for(&syn);
        assert!(root.validate(&syn, Some(&root)).is_err());
        assert!(root.validate_intrinsic().is_ok());
        let deriv = derivative_from(&root);
        assert!(deriv.validate_intrinsic().is_ok());
    }

    /// §7.1 bit-flip sweep, core-engine form: all 640 single-bit mutations of
    /// each valid record must fail validation, EXCEPT the exactly-predicted
    /// tolerated class. Re-derived law (lane deviation D-A17-1): §7.1's
    /// "every mutation must halt" is arithmetically unachievable under §5.8's
    /// `<=` semantics — with the ceiling pinned at MAX, CLEARING any of the
    /// 11 set bits of 5_000_000_000 keeps the ceiling <= MAX and passes. Per
    /// §7.1's own re-derivation clause the court therefore pins: (a) the
    /// positive control admits, (b) exactly 640 mutations run per record
    /// (anti-vacuity — a mutation-skipping shim fails the count), (c) every
    /// non-ceiling bit is killed, (d) survivors are exactly the set-bit class
    /// of the ceiling field at MAX — no other bit may survive.
    #[test]
    fn bit_flip_sweep_court() {
        let syn = synthetic_instrument();
        let root = root_for(&syn);
        let deriv = derivative_from(&root);
        for (name, rec, parent) in [("root", root, None), ("derivative", deriv, Some(&root))] {
            let good = rec.to_bytes();

            // Positive control first: the unmutated record admits, so the
            // sweep cannot pass with a validator that rejects everything.
            assert!(
                P28LicenseAttestation::from_bytes(&good)
                    .and_then(|r| r.validate(&syn, parent))
                    .is_ok(),
                "{name}: positive control must admit before the sweep"
            );

            // Predicted tolerated class from first principles: the SET bits
            // of the ceiling field (bytes 8..16) — clearing any of them
            // keeps the value <= MAX (§5.8), and no other mutation can stay
            // lawful.
            let mut predicted: Vec<(usize, u8)> = Vec::new();
            for i in 8..16 {
                for j in 0..8u8 {
                    if good[i] & (1 << j) != 0 {
                        predicted.push((i, j));
                    }
                }
            }
            // Hand-count pin: popcount(0x12A05F200) = 1+1+2+0+2+4+1+0+0 = 11.
            assert_eq!(
                predicted.len(),
                11,
                "{name}: 5_000_000_000 = 0x12A05F200 has exactly 11 set bits"
            );

            let mut survivors: Vec<(usize, u8)> = Vec::new();
            let mut mutations_run = 0usize;
            for i in 0..ATTESTATION_LEN {
                for j in 0..8u8 {
                    let mut mutated = good;
                    mutated[i] ^= 1 << j;
                    mutations_run += 1;
                    let admits = P28LicenseAttestation::from_bytes(&mutated)
                        .and_then(|r| r.validate(&syn, parent))
                        .is_ok();
                    if admits {
                        survivors.push((i, j));
                    }
                }
            }

            assert_eq!(
                mutations_run, 640,
                "{name}: sweep must run all 640 single-bit mutations (anti-vacuity)"
            );
            survivors.sort_unstable();
            predicted.sort_unstable();
            assert_eq!(
                survivors, predicted,
                "{name}: survivors must be exactly the predicted ceiling set-bit class (D-A17-1); any other surviving bit is an invariant hole"
            );
        }
    }

    /// §7.3: the §4.2 transpile transform asserted as pure-function equality —
    /// output = parent with the lineage field replaced by FLAG_TRANSPILED and
    /// the receipt replaced — plus the §5.7 chain binding: the child admits
    /// against its parent exactly when the new receipt is
    /// BLAKE3(parent wire bytes), and a wrong receipt is spoliation.
    #[test]
    fn transpile_transform_pure_equality() {
        let syn = synthetic_instrument();
        let parent = root_for(&syn);

        // Transform shape with an arbitrary receipt: two field edits, the
        // rest of the record byte-preserved.
        let arbitrary = [0xBBu8; 32];
        let child = P28LicenseAttestation::transpile(&parent, arbitrary);
        assert_eq!(child.attestation_flags, FLAG_TRANSPILED);
        assert_eq!(child.upstream_receipt, arbitrary);
        assert_eq!(child.license_magic, parent.license_magic);
        assert_eq!(child.version, parent.version);
        assert_eq!(
            child.safe_harbor_ceiling_cents,
            parent.safe_harbor_ceiling_cents
        );
        assert_eq!(
            child.origin_authority_digest,
            parent.origin_authority_digest
        );
        // Re-transpiling keeps exactly one lineage bit (bit 2), so the
        // transform cannot mint a multi-lineage record out of a valid one.
        let grandchild = P28LicenseAttestation::transpile(&child, [0xCCu8; 32]);
        assert_eq!(grandchild.attestation_flags, FLAG_TRANSPILED);

        // Chain binding (§5.7): admission requires the receipt bound to the
        // parent record's wire bytes.
        let bound =
            P28LicenseAttestation::transpile(&parent, cgap_blake3::hash(&parent.to_bytes()));
        assert!(
            bound.validate(&syn, Some(&parent)).is_ok(),
            "transpiled child with the §5.7-bound receipt admits against its parent"
        );
        assert!(
            bound.validate(&syn, Some(&bound)).is_err(),
            "wrong parent in the chain is spoliation"
        );
    }

    /// §7.2 stripping court, record-level analogs: truncating the record
    /// (section-strip analog, §5 step 1) and zeroing the parent receipt
    /// (steps 5/7) both fail.
    #[test]
    fn stripping_court_record_level() {
        let syn = synthetic_instrument();
        let root = root_for(&syn);
        let deriv = derivative_from(&root);

        let wire = deriv.to_bytes();
        assert!(
            P28LicenseAttestation::from_bytes(&wire[..79]).is_err(),
            "truncated record fails at parse"
        );
        assert!(
            P28LicenseAttestation::from_bytes(&[]).is_err(),
            "absent section fails at parse"
        );

        let mut stripped = deriv;
        stripped.upstream_receipt = [0u8; 32];
        assert!(
            stripped.validate(&syn, Some(&root)).is_err(),
            "receipt-stripped derivative must not admit"
        );
    }

    // =======================================================================
    // Wave 3 — §4 carrier courts: the record survives a round trip through
    // REAL toolchain-built artifacts (§8 DoD 5/6). Anti-vacuity mutation
    // named per court: any edit to `find_in_elf`/`find_in_wasm` that returns
    // `None`, a wrong span, or a wrong length flips these courts — and the
    // differential witnesses (a second parser for ELF, a second producer for
    // both formats) keep the court from passing on its own parser's model.
    // =======================================================================

    /// Loud-skip gate: a missing tool is a counted `[SKIP]`, never silence.
    fn require_tool(name: &str) -> bool {
        if tool_available(name) {
            true
        } else {
            eprintln!("[SKIP] {name} not on PATH — carrier court not executed (loud, counted)");
            false
        }
    }

    /// §8 DoD 5 — ELF `.p28_invariant` emission + `find_in_elf` round trip on
    /// a real built artifact. Named anti-vacuity mutation: `find_in_elf`
    /// returning `None` or a wrong span flips (a)–(e) below.
    #[test]
    fn elf_carrier_round_trip_court() {
        if !require_tool("cc") || !require_tool("python3") {
            return;
        }
        let span = real_instrument_span();
        let rec = root_for(&span);
        let wire = rec.to_bytes();
        let dir = court_dir("elf-rt");

        // (a) Real toolchain emission: clang emits a genuine ELF64 relocatable
        // carrying the 80 record bytes in `.p28_invariant` (§4.1).
        let elf = cc_elf(&wire, &dir);
        let range = find_in_elf(&elf).expect("cc-emitted ELF must carry .p28_invariant");
        assert_eq!(range.len(), ATTESTATION_LEN);
        assert_eq!(
            &elf[range.clone()],
            &wire[..],
            "round trip: section bytes == record wire bytes"
        );
        assert_eq!(
            P28LicenseAttestation::from_bytes(&elf[range.clone()]).unwrap(),
            rec
        );

        // (b) Differential witness: an independent parser (python struct walk)
        // must agree with `find_in_elf` on offset and size — the court does
        // not pass on its own parser's model alone.
        let secs = python_elf_sections(&dir.join("p28_att.o")).expect("python3 ELF parse");
        let hit = secs
            .iter()
            .find(|(n, _, _)| n == ".p28_invariant")
            .expect("independent parser sees .p28_invariant");
        assert_eq!((hit.1 as usize, hit.2 as usize), (range.start, range.len()));

        // (c) Second producer: the minimal in-test ELF64 builder lands the
        // record identically under the same parser.
        let hand = hand_elf(Some(&wire));
        let hrange = find_in_elf(&hand).expect("hand-built ELF carries the section");
        assert_eq!(&hand[hrange], &wire[..]);

        // (d) Negative control (§5 step 1): an ELF without the section is
        // ABSENT — and absence is a validation failure for the loader.
        assert_eq!(find_in_elf(&cc_elf_without_section(&dir)), None);
        assert_eq!(find_in_elf(&hand_elf(None)), None);

        // (e) Malformed containers fail closed: truncation, empty, and a
        // big-endian e_ident (§9: LE-only parse; BE reads as absent).
        assert_eq!(find_in_elf(&elf[..32]), None, "truncated header");
        assert_eq!(find_in_elf(&[]), None);
        let mut be = elf.clone();
        be[5] = 2; // EI_DATA = ELFDATA2MSB
        assert_eq!(
            find_in_elf(&be),
            None,
            "big-endian parsed as absent (fail-closed)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// §8 DoD 6 — WASM custom-section emission + `find_in_wasm` round trip on
    /// a real built module. Named anti-vacuity mutation: `find_in_wasm`
    /// returning `None`, a wrong span, or accepting a truncated module flips
    /// (a)–(e) below.
    #[test]
    fn wasm_carrier_round_trip_court() {
        if !require_tool("wat2wasm") || !require_tool("wasm-tools") {
            return;
        }
        let span = real_instrument_span();
        let rec = root_for(&span);
        let wire = rec.to_bytes();
        let dir = court_dir("wasm-rt");

        // (a) Real toolchain module: wat2wasm emits a real WASM binary whose
        // `p28_invariant` custom section carries the 80 record bytes (§4.2).
        let wasm = wat_wasm(Some(&wire), &dir);
        let range = find_in_wasm(&wasm).expect("wat2wasm module carries p28_invariant");
        assert_eq!(range.len(), ATTESTATION_LEN);
        assert_eq!(&wasm[range.clone()], &wire[..]);
        assert_eq!(
            P28LicenseAttestation::from_bytes(&wasm[range.clone()]).unwrap(),
            rec
        );

        // (b) Differential witness: wasm-tools (a second, independent binary
        // parser) parses the same module and names the custom section.
        let out = Command::new("wasm-tools")
            .arg("print")
            .arg(dir.join("m.wasm"))
            .output()
            .expect("spawn wasm-tools print");
        assert!(
            out.status.success(),
            "wasm-tools must parse the module: {out:?}"
        );
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("p28_invariant"),
            "independent parser must see the custom section"
        );

        // (c) Second producer: the minimal spec module (header + custom
        // section only) lands the same payload under the same parser.
        let hand = hand_wasm(Some(&wire));
        let hrange = find_in_wasm(&hand).expect("hand-built module carries the section");
        assert_eq!(&hand[hrange], &wire[..]);

        // (d) Negative control (§5 step 1): module without the custom section
        // is absent.
        assert_eq!(find_in_wasm(&wat_wasm(None, &dir)), None);
        assert_eq!(find_in_wasm(&hand_wasm(None)), None);

        // (e) Malformed containers fail closed: truncation and a bad version.
        assert_eq!(find_in_wasm(&wasm[..7]), None, "shorter than the header");
        assert_eq!(
            find_in_wasm(&wasm[..wasm.len() - 1]),
            None,
            "section overrun"
        );
        assert_eq!(find_in_wasm(&[]), None);
        let mut badver = wasm.clone();
        badver[4] = 0x0F;
        assert_eq!(find_in_wasm(&badver), None, "unknown module version absent");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// a03-wave5 F-1 defect-class regression (wave 6): hostile section-table
    /// offsets in untrusted container bytes must fail TYPED (`None`), never
    /// panic in debug arithmetic. Witnessed pre-fix panic:
    /// `attempt to add with overflow` at the `shdr` closure (sandbox probe,
    /// `e_shoff = u64::MAX`/`u64::MAX - 3` with `shentsize=64, shnum=1,
    /// shstrndx=0`). Anti-vacuity: the sane-ELF positive control must still
    /// ADMIT, so a parser mutated to `None`-always fails this court. Named
    /// mutations (both witnessed flipping in /tmp/a17-w6, byte-exact
    /// restore): reverting `shdr`'s checked adds to unchecked `base + K`
    /// (panic → flip), and neutering `find_in_wasm`'s size bounds check —
    /// caught by the overrun-with-plausible-name case below.
    #[test]
    fn hostile_container_offsets_fail_closed_typed() {
        let syn = synthetic_instrument();
        let wire = root_for(&syn).to_bytes();

        // Positive control: the hand-built sane ELF still admits (pins that
        // the hostile refusals below are content-driven, not a parser that
        // rejects everything).
        let hand = hand_elf(Some(&wire));
        assert!(find_in_elf(&hand).is_some(), "sane ELF must admit");

        // 64-byte ELF64 LE header only; every field but e_shoff is lawful.
        fn hostile(shoff: u64) -> Vec<u8> {
            let mut e = vec![0u8; 64];
            e[0..4].copy_from_slice(b"\x7fELF");
            e[4] = 2; // ELFCLASS64
            e[5] = 1; // little-endian
            e[6] = 1; // EV_CURRENT
            e[0x28..0x30].copy_from_slice(&shoff.to_le_bytes()); // e_shoff
            e[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
            e[0x3C..0x3E].copy_from_slice(&1u16.to_le_bytes()); // e_shnum
            e[0x3E..0x40].copy_from_slice(&0u16.to_le_bytes()); // e_shstrndx
            e
        }
        // shdr(0): base = e_shoff (checked_add(0) passes), then `base + 4`
        // / `base + 0x18` / `base + 0x20` would overflow — typed None now.
        assert_eq!(find_in_elf(&hostile(u64::MAX)), None, "base+4 overflow");
        assert_eq!(
            find_in_elf(&hostile(u64::MAX - 3)),
            None,
            "base+4 saturates at usize::MAX"
        );
        assert_eq!(
            find_in_elf(&hostile(u64::MAX - 0x17)),
            None,
            "table start near usize::MAX: name span out of bounds -> absent"
        );
        assert_eq!(
            find_in_elf(&hostile(64)),
            None,
            "lawful table offset beyond the buffer is absent, not a panic"
        );

        // Hostile WASM: 5-byte LEB (continuation on the 5th byte) and a
        // section size of u32::MAX must both parse as absent — typed, no
        // panic, no acceptance.
        let mut leb5 = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        leb5.push(0); // custom section id
        leb5.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x7F]); // u32::MAX, 5-byte LEB
        assert_eq!(find_in_wasm(&leb5), None, "u32::MAX section size absent");
        let mut leb_of = leb5.clone();
        leb_of[9..14].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // 5th continuation bit set
        assert_eq!(
            find_in_wasm(&leb_of),
            None,
            "LEB longer than 5 bytes parses as absent"
        );
        // Overrun with a plausible name: the claimed section size extends
        // past the buffer while the section NAME matches — only the size
        // bounds check refuses this. (A truncated u32::MAX size alone is
        // caught earlier by the name-read bounds; this shape discriminates.)
        let mut overrun = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
        overrun.push(0); // custom section id
        overrun.push(94); // claimed size: 1 len + 13 name + 80 payload
        overrun.push(13); // name length LEB
        overrun.extend_from_slice(b"p28_invariant");
        overrun.extend_from_slice(&[0u8; 10]); // only 10 of 80 payload bytes present
        assert_eq!(
            find_in_wasm(&overrun),
            None,
            "section size overrun past the buffer is absent, not a span"
        );
    }
}

/// §3 receipt pin: BLAKE3-256 of the canonical header span of
/// LICENSE-P28-SIL.md (repo root). Instrument SHA-256 at pin time:
/// 79107c7ade61eb747098f93f1a5641ba665460b76daa1f235bd7894ea4e2484f.
/// Any edit to the instrument flips `origin_digest_determinism_and_pin` and
/// forces a re-attestation run (lane doc §9).
#[cfg(test)]
const PINNED_ORIGIN_AUTHORITY_DIGEST: [u8; 32] = [
    0xe1, 0x44, 0xc4, 0x8a, 0x49, 0xa2, 0xc4, 0xe4, //
    0x09, 0x53, 0xa4, 0xb1, 0xeb, 0x4b, 0x4b, 0x93, //
    0x8e, 0x28, 0x0e, 0x0f, 0x92, 0xb8, 0x68, 0x0f, //
    0xe3, 0xbb, 0x0a, 0xa2, 0x21, 0x27, 0xb0, 0xb4,
];
