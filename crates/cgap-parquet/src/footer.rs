//! Lane A02 — file-level footer framing: `FooterSealer`, `verify_seal`,
//! `SealedBlock`.
//!
//! Layer 2 of the ADR: appends, over the body a
//! [`crate::mmap_writer::StaticPageWriter`] flushed into a buffer, the
//! metadata block, the P28 attestation slot, the block checksum, and the
//! magic + footer-length trailer — producing a sealed, replay-verifiable
//! block that A03 (git DAG) and A05 (atomic commit) can address by digest.
//!
//! # Byte layout (§6.1; all integers little-endian)
//!
//! ```text
//! | region      | offset  | size | content                             |
//! |-------------|---------|------|-------------------------------------|
//! | body        | 0       | B    | row groups (pages)                  |
//! | metadata    | B       | M    | meta block, compact protocol (§6.2) |
//! | attestation | B+M     | 80   | P28 record, verbatim (opaque here)  |
//! | checksum    | B+M+80  | 8    | FNV-1a-64 over [0, B+M+80)          |
//! | magic       | B+M+88  | 4    | b"PAR2"                             |
//! | footer len  | B+M+92  | 4    | u32 = 96 + M (self-describing)      |
//! ```
//!
//! Total block = `B + 96 + M`. From the block end `L`: footer_len at
//! `L-4`, magic at `L-8`, checksum at `L-16`, attestation at
//! `L-96..L-16`, meta at `L-footer_len..L-footer_len+M` with
//! `M = footer_len - 96`, body `[0, L - footer_len)`. At least one row
//! group is required (`B >= 1`); sealing an empty body is a typed refusal.
//!
//! # Documented implementation choices (§4 "decide at implementation")
//!
//! - **Checksum/digest**: FNV-1a-64 ([`fnv1a64`], fixed basis/prime, no
//!   dependency — the XXH64-class alternative needs either a crate or a
//!   larger constant table for no gain at this fault class). The 32-byte
//!   content address ([`block_digest`]) is four FNV-1a-64 passes over the
//!   same span with fixed distinct seeds (`basis ^ i * golden`). This is a
//!   non-cryptographic integrity digest and content address for the
//!   campaign's trust model, chosen by the no-dep constraint as the lane
//!   doc directs; it is NOT a collision-resistant hash and these docs do
//!   not claim otherwise.
//! - **Meta field 5**: the body digest prefix is the 8-byte FNV-1a-64 of
//!   the body region (little-endian) — the "prefix" §6.2 names, and the
//!   exact value C1/C2 corruption localization compares.
//! - **Verdict order** in [`verify_seal`]: shape -> attestation -> checksum
//!   -> digest. The §6.3 text asks for a single artifact where "the
//!   checksum verdict still passes" while the attestation verdict fails;
//!   because the §6.1 checksum spans the attestation slot, that pair is
//!   unsatisfiable on one artifact — the channels are separated by verdict
//!   order instead, and the C3 court additionally witnesses channel
//!   independence by clearing the gate flag (the checksum channel then
//!   fires, localized to the trailer). Divergence recorded in the lane
//!   receipt.
//! - **Meta block allocation**: the seal path builds the meta block with
//!   the crate's compact-protocol writer family (which emits into a
//!   `Vec`); allocation is lawful here — the zero-alloc law scopes to the
//!   flush path (C4), not the seal path.
//! - **List encoding**: meta field 4's elements are bare zigzag-i32
//!   varints (compact-protocol list elements carry no field headers);
//!   counts <= 14 ride the list header byte, larger counts take the
//!   0xF-marker + varint long form.
//! - **Meta flags validation**: only bit0 is defined (§6.2). A negative
//!   value or any undefined bit is
//!   [`SealError::Shape`]`(`[`ShapeFault::MetaMalformed`]`)` — a wrapped
//!   `as u32` cast must not silently set or clear the attestation gate.
//! - **Capacity/shape refusals** reuse [`ParquetError::RunOverrun`] with
//!   documented claim/requirement semantics (the `CapacityExceeded`
//!   variant would need `crate::rle` churn outside this lane's file
//!   ownership).

use crate::rle::ParquetError;
use crate::thrift::{
    ctype, read_binary, read_field_header, read_i32, read_i64, read_varint, skip_value,
    write_binary_field, write_field_header, write_i32_field, write_i64_field, write_zigzag_i16,
    zigzag_decode_i16, zigzag_encode_i32,
};
use alloc::vec::Vec;

/// Block magic (§6.1): this is a cgap-private Parquet-v2-derived container
/// and says so — it does NOT claim full-Parquet compatibility (the full
/// thrift `FileMetaData` was rejected by ADR §3 alternative 3).
pub const MAGIC: [u8; 4] = *b"PAR2";

/// Attestation slot size (§6.3): the A17-owned `P28LicenseAttestation` is
/// consumed verbatim as an opaque 80 bytes; this lane defines the framing.
pub const ATTESTATION_LEN: usize = 80;

/// Fixed trailer bytes: attestation 80 + checksum 8 + magic 4 + len 4.
pub const TRAILER_FIXED: usize = 96;

/// FNV-1a 64-bit offset basis.
pub const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// Golden-ratio constant for derived digest seeds.
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

// ---- Wave-1 ABI freeze: compile-time pins on the §6.1 wire layout. Any
// edit that moves a region boundary or the trailer width breaks the build
// here, not at verify time. ----
const _: () = {
    assert!(MAGIC.len() == 4, "magic is a fixed 4-byte tag");
    assert!(
        TRAILER_FIXED == ATTESTATION_LEN + 8 + MAGIC.len() + 4,
        "trailer = attestation + checksum + magic + footer-length"
    );
    assert!(TRAILER_FIXED == 96, "§6.1 trailer is 96 bytes on the wire");
    assert!(
        core::mem::size_of::<[u8; 32]>() == 4 * core::mem::size_of::<u64>(),
        "block_digest is 4 FNV-1a-64 lanes of 8 bytes"
    );
};

/// FNV-1a-64 over `data` (default basis).
pub fn fnv1a64(data: &[u8]) -> u64 {
    fnv1a64_seeded(data, FNV_OFFSET_BASIS)
}

/// FNV-1a-64 over `data` with an explicit seed.
pub fn fnv1a64_seeded(data: &[u8], seed: u64) -> u64 {
    let mut h = seed;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// The 32-byte content address: four FNV-1a-64 passes with fixed distinct
/// seeds (documented choice; non-cryptographic — see module docs).
pub fn block_digest(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..4u64 {
        let seed = FNV_OFFSET_BASIS ^ i.wrapping_mul(GOLDEN);
        out[(i * 8) as usize..(i * 8 + 8) as usize]
            .copy_from_slice(&fnv1a64_seeded(data, seed).to_le_bytes());
    }
    out
}

/// The row-group summary the writer hands the sealer (spans are copied in;
/// the borrow ends at construction).
#[derive(Debug, Clone, Copy)]
pub struct BlockSummary<'a> {
    pub row_groups: u32,
    pub rows: u64,
    /// Body byte span of each row group, in order (§6.2 field 4).
    pub spans: &'a [u32],
}

/// Layer-2 sealer over a fixed buffer. Constructed from a writer or from
/// raw parts; consumes itself in [`FooterSealer::seal`].
pub struct FooterSealer<'a> {
    buf: &'a mut [u8],
    body_len: usize,
    row_groups: u32,
    rows: u64,
    spans: Vec<u32>,
    attestation: Option<[u8; ATTESTATION_LEN]>,
}

impl<'a> FooterSealer<'a> {
    /// Seal over `buf`, whose first `body_len` bytes are the body.
    ///
    /// Refusals (typed): zero row groups, an empty body (`B >= 1`, §6.1 —
    /// covers the all-zero-span summary `seal()` would otherwise emit as a
    /// block [`verify_seal`] must refuse), any span above the §6.2 field-4
    /// wire bound (`i32::MAX` — wave-7 repair W7-F1), spans not summing to
    /// `body_len`, or `body_len` exceeding the buffer — all
    /// [`ParquetError::RunOverrun`] with documented semantics.
    pub fn new(
        buf: &'a mut [u8],
        body_len: usize,
        summary: BlockSummary<'_>,
        attestation: Option<&[u8; ATTESTATION_LEN]>,
    ) -> Result<Self, ParquetError> {
        if body_len == 0 {
            // §6.1: at least one row group means a non-empty body. A
            // row_groups >= 1 summary over an empty body would seal a block
            // verify_seal refuses (EmptyBody) — refuse at the seal boundary
            // instead (wave-6 repair W6-F1; court: zero_body_seal court).
            return Err(ParquetError::RunOverrun {
                run_len: 0,
                budget: 1,
            });
        }
        if summary.row_groups == 0 {
            // §6.1: at least one row group required.
            return Err(ParquetError::RunOverrun {
                run_len: 0,
                budget: 1,
            });
        }
        if summary.spans.len() != summary.row_groups as usize {
            return Err(ParquetError::RunOverrun {
                run_len: summary.spans.len(),
                budget: summary.row_groups as usize,
            });
        }
        for &span in summary.spans {
            if span > i32::MAX as u32 {
                // W7-F1 seal-boundary arm: §6.2 field 4 is a zigzag i32 — a
                // span above i32::MAX wraps to a negative wire value that
                // verify_seal's parse refuses (u32::try_from). Seal refuses
                // here rather than emit a writable-but-unverifiable block
                // (the W6-F1 asymmetry class at a second bound).
                return Err(ParquetError::RunOverrun {
                    run_len: span as usize,
                    budget: i32::MAX as usize,
                });
            }
        }
        let span_sum: u64 = summary.spans.iter().map(|s| u64::from(*s)).sum();
        if span_sum != body_len as u64 {
            return Err(ParquetError::RunOverrun {
                run_len: span_sum as usize,
                budget: body_len,
            });
        }
        if body_len > buf.len() {
            return Err(ParquetError::RunOverrun {
                run_len: body_len,
                budget: buf.len(),
            });
        }
        Ok(Self {
            buf,
            body_len,
            row_groups: summary.row_groups,
            rows: summary.rows,
            spans: summary.spans.to_vec(),
            attestation: attestation.map(|a| {
                let mut slot = [0u8; ATTESTATION_LEN];
                slot.copy_from_slice(a);
                slot
            }),
        })
    }

    /// Append the footer over the buffer's body and return the sealed
    /// block (bytes borrowed from the buffer + the content digest).
    pub fn seal(self) -> Result<SealedBlock<'a>, ParquetError> {
        let meta = self.build_meta_block();
        let m = meta.len();
        let body_len = self.body_len;
        // Checked end-to-end: a wrap in any intermediate sum would pass a
        // range check it must not (same overflow class as the writer's
        // cursor arithmetic).
        let content_end = body_len
            .checked_add(m)
            .and_then(|v| v.checked_add(ATTESTATION_LEN))
            .ok_or(ParquetError::RunOverrun {
                run_len: usize::MAX,
                budget: self.buf.len(),
            })?;
        let total =
            content_end.checked_add(16).ok_or(ParquetError::RunOverrun {
                // + checksum 8 + magic 4 + len 4
                run_len: usize::MAX,
                budget: self.buf.len(),
            })?;
        if total > self.buf.len() {
            return Err(ParquetError::RunOverrun {
                run_len: total,
                budget: self.buf.len(),
            });
        }
        let mut pos = body_len;
        self.buf[pos..pos + m].copy_from_slice(&meta);
        pos += m;
        match &self.attestation {
            Some(slot) => self.buf[pos..pos + ATTESTATION_LEN].copy_from_slice(slot),
            None => self.buf[pos..pos + ATTESTATION_LEN].fill(0),
        }
        pos += ATTESTATION_LEN;
        // Checksum spans [0, B+M+80).
        let checksum = fnv1a64(&self.buf[..pos]);
        self.buf[pos..pos + 8].copy_from_slice(&checksum.to_le_bytes());
        pos += 8;
        self.buf[pos..pos + 4].copy_from_slice(&MAGIC);
        pos += 4;
        let footer_len = (TRAILER_FIXED + m) as u32;
        self.buf[pos..pos + 4].copy_from_slice(&footer_len.to_le_bytes());
        pos += 4;
        debug_assert_eq!(pos, total);
        let digest = block_digest(&self.buf[..content_end]);
        Ok(SealedBlock {
            bytes: &self.buf[..total],
            digest,
        })
    }

    /// The §6.2 metadata block, framed with the crate's compact-protocol
    /// writer family (footer-local field ids; NOT parquet.thrift ids).
    fn build_meta_block(&self) -> Vec<u8> {
        let mut meta = Vec::new();
        // 1: format version (i16 zigzag) = 1.
        write_field_header(&mut meta, 1, 0, ctype::I16);
        write_zigzag_i16(&mut meta, 1);
        // 2: row-group count (i32 zigzag), >= 1.
        write_i32_field(&mut meta, 2, 1, self.row_groups as i32);
        // 3: total row count (i64 zigzag).
        write_i64_field(&mut meta, 3, 2, self.rows as i64);
        // 4: list<i32> of per-row-group body spans (bare zigzag elements).
        write_field_header(&mut meta, 4, 3, ctype::LIST);
        let n = self.spans.len();
        if n <= 14 {
            meta.push(((n as u8) << 4) | ctype::I32);
        } else {
            meta.push(0xF0 | ctype::I32);
            crate::rle::write_varint(&mut meta, n as u64);
        }
        for span in &self.spans {
            crate::rle::write_varint(
                &mut meta,
                u64::from(zigzag_encode_i32(*span as i32)),
            );
        }
        // 5: body digest prefix (FNV-1a-64 of the body region, LE).
        let body_digest = fnv1a64(&self.buf[..self.body_len]);
        write_binary_field(&mut meta, 5, 4, &body_digest.to_le_bytes());
        // 6: flags — bit0: attestation present.
        let flags = if self.attestation.is_some() { 1 } else { 0 };
        write_i32_field(&mut meta, 6, 5, flags);
        meta.push(ctype::STOP);
        meta
    }
}

/// A sealed block: the bytes (borrowed from the writer's buffer) and the
/// 32-byte content digest — the handle A03 addresses and A05 commits.
#[derive(Debug, Clone, Copy)]
pub struct SealedBlock<'a> {
    bytes: &'a [u8],
    digest: [u8; 32],
}

impl<'a> SealedBlock<'a> {
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Hand the block to a [`crate::mmap_writer::BlockSink`] backend
    /// (Tier H file handoff; A03/A05 seam). The sink's window must hold
    /// the whole block; a smaller window is a typed refusal, never a
    /// silent partial commit. A window LONGER than the block is lawful —
    /// the trait contract is "at least `len`", and page-rounded mmap
    /// windows round UP — so exactly the block length is copied and
    /// committed, never the window length (wave-8 repair W8-F1: the
    /// pre-repair form panicked on the lawful oversized window).
    pub fn flush_to<S: crate::mmap_writer::BlockSink>(
        &self,
        mut sink: S,
    ) -> Result<(), ParquetError> {
        let n = self.bytes.len();
        let window = sink.window(n);
        if window.len() < n {
            return Err(ParquetError::RunOverrun {
                run_len: n,
                budget: window.len(),
            });
        }
        window[..n].copy_from_slice(&self.bytes[..n]);
        sink.commit(n)
    }
}

/// Shape faults (distinguishable from corruption verdicts — DoD 6).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ShapeFault {
    /// Block shorter than the fixed trailer.
    TooShort,
    /// Magic is not `PAR2`.
    BadMagic,
    /// Footer-length field out of range for the block.
    BadFooterLen,
    /// Body region empty.
    EmptyBody,
    /// Metadata block unparseable or an unsupported format version.
    MetaMalformed,
    /// Metadata spans do not sum to the body extent.
    SpanMismatch,
}

/// Which region a checksum failure localized to (via the §6.2 field-5
/// body-digest comparison).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CorruptRegion {
    Body,
    Footer,
}

/// Typed, distinguishable `verify_seal` failures (shape vs corruption vs
/// attestation vs content address).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SealError {
    Shape(ShapeFault),
    ChecksumMismatch {
        region: CorruptRegion,
    },
    /// Flags bit0 set but the attestation slot is all zero (§6.3).
    AttestationMissing,
    /// The supplied content address does not match the block (the A03
    /// wrong-object case).
    DigestMismatch,
}

/// The successful verdict: what `verify_seal` read back from the bytes.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Verified {
    pub body_len: usize,
    pub meta_len: usize,
    pub row_groups: u32,
    pub rows: u64,
    /// First four row-group spans (full count in `spans_len`).
    pub spans: [u32; 4],
    pub spans_len: usize,
    pub attestation_present: bool,
}

/// Verify a sealed block read back from anywhere (file, object store,
/// buffer): the post-commit read-back check (A05 contract) and the
/// content-address check (A03 contract).
///
/// Verdict order: shape -> attestation -> checksum -> digest.
pub fn verify_seal(bytes: &[u8], digest: &[u8; 32]) -> Result<Verified, SealError> {
    // 1. Shape: fixed trailer, magic, self-describing footer length.
    if bytes.len() < TRAILER_FIXED {
        return Err(SealError::Shape(ShapeFault::TooShort));
    }
    let l = bytes.len();
    if bytes[l - 8..l - 4] != MAGIC {
        return Err(SealError::Shape(ShapeFault::BadMagic));
    }
    let footer_len =
        u32::from_le_bytes([bytes[l - 4], bytes[l - 3], bytes[l - 2], bytes[l - 1]]) as usize;
    if footer_len < TRAILER_FIXED || footer_len > l {
        return Err(SealError::Shape(ShapeFault::BadFooterLen));
    }
    let body_len = l - footer_len;
    if body_len == 0 {
        return Err(SealError::Shape(ShapeFault::EmptyBody));
    }
    let m = footer_len - TRAILER_FIXED;
    let meta = &bytes[body_len..body_len + m];

    // 2. Metadata parse (typed shape fault on anything malformed).
    let parsed = parse_meta(meta)?;
    let span_sum: u64 = parsed.spans.iter().map(|s| u64::from(*s)).sum();
    if span_sum != body_len as u64 {
        return Err(SealError::Shape(ShapeFault::SpanMismatch));
    }

    // 3. Attestation: flags bit0 gates the slot (§6.3).
    let attest = &bytes[body_len + m..body_len + m + ATTESTATION_LEN];
    let attest_all_zero = attest.iter().all(|&b| b == 0);
    if parsed.flags & 1 == 1 && attest_all_zero {
        return Err(SealError::AttestationMissing);
    }

    // 4. Checksum over [0, B+M+80), localized via the body digest.
    let content_end = body_len + m + ATTESTATION_LEN;
    let stored = u64::from_le_bytes([
        bytes[content_end],
        bytes[content_end + 1],
        bytes[content_end + 2],
        bytes[content_end + 3],
        bytes[content_end + 4],
        bytes[content_end + 5],
        bytes[content_end + 6],
        bytes[content_end + 7],
    ]);
    if fnv1a64(&bytes[..content_end]) != stored {
        let region = if fnv1a64(&bytes[..body_len]) == u64::from(parsed.body_digest) {
            CorruptRegion::Footer
        } else {
            CorruptRegion::Body
        };
        return Err(SealError::ChecksumMismatch { region });
    }

    // 5. Content address.
    if block_digest(&bytes[..content_end]) != *digest {
        return Err(SealError::DigestMismatch);
    }

    let mut spans = [0u32; 4];
    for (i, s) in parsed.spans.iter().take(4).enumerate() {
        spans[i] = *s;
    }
    Ok(Verified {
        body_len,
        meta_len: m,
        row_groups: parsed.row_groups,
        rows: parsed.rows,
        spans,
        spans_len: parsed.spans.len(),
        attestation_present: parsed.flags & 1 == 1,
    })
}

#[derive(Debug)]
struct ParsedMeta {
    row_groups: u32,
    rows: u64,
    spans: Vec<u32>,
    body_digest: u64,
    flags: u32,
}

/// Parse the §6.2 metadata block: footer-local fields 1-6, compact
/// protocol, forward-compat skip of unknown fields, required-field
/// fail-closed.
fn parse_meta(meta: &[u8]) -> Result<ParsedMeta, SealError> {
    let malformed = SealError::Shape(ShapeFault::MetaMalformed);
    let mut version: Option<i16> = None;
    let mut row_groups: Option<i32> = None;
    let mut rows: Option<i64> = None;
    let mut spans: Option<Vec<u32>> = None;
    let mut body_digest: Option<u64> = None;
    let mut flags: Option<i32> = None;
    let mut pos = 0usize;
    let mut prev: i16 = 0;
    loop {
        let Some((fid, ty)) = read_field_header(meta, &mut pos, prev).map_err(|_| malformed)?
        else {
            break;
        };
        prev = fid;
        match (fid, ty) {
            (1, ctype::I16) => {
                let raw = read_varint(meta, &mut pos).map_err(|_| malformed)?;
                version = Some(zigzag_decode_i16(raw));
            }
            (2, ctype::I32) => row_groups = Some(read_i32(meta, &mut pos).map_err(|_| malformed)?),
            (3, ctype::I64) => rows = Some(read_i64(meta, &mut pos).map_err(|_| malformed)?),
            (4, ctype::LIST) => {
                let Some(&hdr) = meta.get(pos) else {
                    return Err(malformed);
                };
                pos += 1;
                let elem_ty = hdr & 0x0F;
                if elem_ty != ctype::I32 {
                    return Err(malformed);
                }
                let mut n = (hdr >> 4) as usize;
                if n == 15 {
                    let raw = read_varint(meta, &mut pos).map_err(|_| malformed)?;
                    n = usize::try_from(raw).map_err(|_| malformed)?;
                }
                let mut list = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    let v = read_i32(meta, &mut pos).map_err(|_| malformed)?;
                    let u = u32::try_from(v).map_err(|_| malformed)?;
                    list.push(u);
                }
                spans = Some(list);
            }
            (5, ctype::BINARY) => {
                let raw = read_binary(meta, &mut pos).map_err(|_| malformed)?;
                if raw.len() != 8 {
                    return Err(malformed);
                }
                body_digest = Some(u64::from_le_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ]));
            }
            (6, ctype::I32) => flags = Some(read_i32(meta, &mut pos).map_err(|_| malformed)?),
            (_, other) => skip_value(meta, &mut pos, other, 0).map_err(|_| malformed)?,
        }
    }
    let (version, row_groups, rows, spans, body_digest, flags) = match (
        version,
        row_groups,
        rows,
        spans,
        body_digest,
        flags,
    ) {
        (Some(v), Some(rg), Some(r), Some(s), Some(d), Some(f)) => (v, rg, r, s, d, f),
        _ => return Err(malformed),
    };
    if version != 1 {
        return Err(malformed);
    }
    if row_groups <= 0 || rows < 0 {
        return Err(malformed);
    }
    // Flags are a bitmask with exactly one defined bit (§6.2: bit0 attestation
    // present). A negative value or any undefined bit is a forged/malformed
    // block — fail closed rather than letting a wrapped cast silently set or
    // clear the attestation gate.
    if flags < 0 || flags & !1 != 0 {
        return Err(malformed);
    }
    if spans.len() != row_groups as usize {
        return Err(malformed);
    }
    Ok(ParsedMeta {
        row_groups: row_groups as u32,
        rows: rows as u64,
        spans,
        body_digest,
        flags: flags as u32,
    })
}

#[cfg(test)]
mod courts {
    use super::*;
    use crate::mmap_writer::{SlabView, StaticPageBuffer, StaticPageWriter};
    use alloc::rc::Rc;
    use core::cell::RefCell;

    const CAP: usize = 65_536;

    /// The 80-byte P28 fixture, derived from the frozen A17 ABI constants
    /// (crates/cgap-p28/src/attestation.rs): magic `P28S`, version 1,
    /// FLAG_ROOT bit0, ceiling <= $50M, 32+32 B digests (Root: zero
    /// upstream receipt). Framing lane only — the struct is A17's.
    fn p28_attest_fixture() -> [u8; 80] {
        let mut a = [0u8; 80];
        a[0..4].copy_from_slice(b"P28S");
        a[4..6].copy_from_slice(&1u16.to_le_bytes());
        a[6..8].copy_from_slice(&1u16.to_le_bytes());
        a[8..16].copy_from_slice(&5_000_000_000u64.to_le_bytes());
        for (i, b) in a[16..48].iter_mut().enumerate() {
            *b = (i as u8) ^ 0x5A;
        }
        a[48..80].copy_from_slice(&[0u8; 32]);
        a
    }

    fn seal_block<'b>(
        buf: &'b mut [u8],
        values: &[u32],
        bits: u8,
        attest: Option<&[u8; 80]>,
    ) -> SealedBlock<'b> {
        let mut w = StaticPageWriter::new(buf);
        w.write_slab(&SlabView {
            lanes: &[values],
            rows: values.len() as u32,
            bits,
        })
        .expect("write_slab");
        w.seal(attest).expect("seal")
    }

    fn clean_block() -> (Vec<u8>, [u8; 32]) {
        let mut b = StaticPageBuffer::<CAP>::new();
        let block = seal_block(b.as_mut_slice(), &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 8, None);
        (block.bytes().to_vec(), *block.digest())
    }

    // ---- shape courts ----

    #[test]
    fn every_truncation_fails_closed() {
        let (clean, digest) = clean_block();
        assert!(verify_seal(&clean, &digest).is_ok());
        for cut in 0..clean.len() {
            assert!(
                verify_seal(&clean[..cut], &digest).is_err(),
                "prefix {cut}/{} must refuse",
                clean.len()
            );
        }
        // And the intact block still verifies after the sweep.
        assert!(verify_seal(&clean, &digest).is_ok());
    }

    #[test]
    fn magic_and_footer_len_flips_are_shape_faults() {
        let (clean, digest) = clean_block();
        let l = clean.len();

        // MUTATION: flip one magic byte.
        let mut mutated = clean.clone();
        mutated[l - 8] ^= 0x01;
        assert_eq!(
            verify_seal(&mutated, &digest),
            Err(SealError::Shape(ShapeFault::BadMagic))
        );

        // MUTATION: corrupt the footer-length field.
        let mut mutated = clean.clone();
        mutated[l - 1] = 0xFF;
        assert_eq!(
            verify_seal(&mutated, &digest),
            Err(SealError::Shape(ShapeFault::BadFooterLen))
        );

        // Zero row groups at seal: typed refusal at FooterSealer::new.
        let mut b2 = StaticPageBuffer::<CAP>::new();
        match FooterSealer::new(
            b2.as_mut_slice(),
            0,
            BlockSummary {
                row_groups: 0,
                rows: 0,
                spans: &[],
            },
            None,
        ) {
            Err(ParquetError::RunOverrun { run_len: 0, budget: 1 }) => {}
            Err(other) => panic!("zero-RG seal: wrong error {other:?}"),
            Ok(_) => panic!("zero-RG seal must refuse"),
        }
    }

    // ---- wave-6 repairs (W6-F1 seal/verify asymmetry, W6-F2 hostile corpus) ----

    /// W6-F1: §6.1's `B >= 1` law enforced at the seal boundary. The
    /// row_groups >= 1 / all-zero-span summary previously slipped through
    /// to `seal()`, which emitted a block `verify_seal` must refuse —
    /// seal/verify asymmetry.
    #[test]
    fn zero_body_seal_refused_and_verify_refuses_empty_body() {
        let mut b = StaticPageBuffer::<CAP>::new();
        match FooterSealer::new(
            b.as_mut_slice(),
            0,
            BlockSummary {
                row_groups: 1,
                rows: 0,
                spans: &[0],
            },
            None,
        ) {
            Err(ParquetError::RunOverrun { run_len: 0, budget: 1 }) => {}
            Err(other) => panic!("zero-body seal: wrong error {other:?}"),
            Ok(_) => panic!("zero-body seal must refuse"),
        }

        // The verify side of the same law: a minimum-shape block whose
        // body is empty (footer_len == l == 96) is EmptyBody, never Ok.
        let mut empty = alloc::vec![0u8; TRAILER_FIXED];
        empty[TRAILER_FIXED - 8..TRAILER_FIXED - 4].copy_from_slice(&MAGIC);
        empty[TRAILER_FIXED - 4..].copy_from_slice(&(TRAILER_FIXED as u32).to_le_bytes());
        assert_eq!(
            verify_seal(&empty, &[0u8; 32]),
            Err(SealError::Shape(ShapeFault::EmptyBody))
        );
    }

    /// W7-F1: the seal boundary refuses a span above the §6.2 field-4 wire
    /// bound (zigzag i32 — the non-negative representable range is
    /// [0, i32::MAX]). A span in (i32::MAX, u32::MAX] wraps to a NEGATIVE
    /// wire i32 that `verify_seal`'s parse must refuse (u32::try_from), so
    /// sealing it would emit a writable-but-unverifiable block — the W6-F1
    /// asymmetry class at a second bound. The court pins the exact refusal
    /// payload (run_len = offending span, budget = i32::MAX): the span-sum
    /// check refuses with a different budget, so only the per-span check
    /// produces this verdict and a neutered check flips the court.
    ///
    /// Writer-arm disclosure: the same bound in
    /// `StaticPageWriter::write_slab` (up-front, before any byte) is real
    /// but its exact-bound court is BLOCKED by scale — a > 2 GiB lane would
    /// be needed in-process. The verify side of the same law is pinned by
    /// the hostile-corpus `neg_span` member (u32::try_from ->
    /// MetaMalformed).
    #[test]
    fn w7_f1_span_wire_bound_refused_at_seal_with_exact_payload() {
        let mut b = StaticPageBuffer::<CAP>::new();
        let span = (i32::MAX as u32) + 1; // first span the wire cannot carry

        // BASELINE/BEFORE: span-sum == body_len, so ONLY the per-span wire
        // bound can refuse; the payload pins the check.
        match FooterSealer::new(
            b.as_mut_slice(),
            span as usize,
            BlockSummary {
                row_groups: 1,
                rows: 0,
                spans: &[span],
            },
            None,
        ) {
            Err(ParquetError::RunOverrun { run_len, budget }) => {
                assert_eq!(run_len, span as usize, "run_len = offending span");
                assert_eq!(budget, i32::MAX as usize, "budget = wire bound");
            }
            Err(other) => panic!("W7-F1: wrong error {other:?}"),
            Ok(_) => panic!("W7-F1: span above i32::MAX must refuse at seal"),
        }

        // MUTATION/AFTER (flip witness): the same summary at the lawful
        // bound — the per-span check must PASS it (buffer check fires
        // instead, budget = CAP, not i32::MAX). A check bounded below
        // i32::MAX would refuse here too and the payloads would not
        // distinguish; a neutered check would refuse the BASELINE arm with
        // the span-sum payload (budget = body_len) — both vacuity classes
        // are payload-visible.
        let lawful = i32::MAX as u32;
        match FooterSealer::new(
            b.as_mut_slice(),
            lawful as usize,
            BlockSummary {
                row_groups: 1,
                rows: 0,
                spans: &[lawful],
            },
            None,
        ) {
            Err(ParquetError::RunOverrun { run_len, budget }) => {
                assert_eq!(run_len, i32::MAX as usize);
                assert_eq!(budget, CAP, "buffer check fired, not the wire bound");
            }
            Err(other) => panic!("W7-F1 lawful-bound arm: wrong error {other:?}"),
            Ok(_) => panic!("W7-F1 lawful-bound arm: buffer must refuse 2 GiB body"),
        }
    }

    /// W6-F2: `verify_seal` is a public parser of untrusted bytes (the
    /// a03-wave5 F-1 discipline applied to this lane): the hostile corpus
    /// must refuse typed, never panic. Members are carved from real
    /// `seal()` output; reassembled members recompute the honest checksum
    /// so the verdict is driven by the parse/attestation layers, not the
    /// integrity layer. Pinned members name their exact verdicts — a
    /// parser that went vacuous flips this court.
    #[test]
    fn hostile_sealed_block_corpus_fails_closed_with_typed_verdicts() {
        let (clean, digest) = clean_block();
        let l = clean.len();

        // BEFORE: the generator is real — the clean block verifies.
        assert!(verify_seal(&clean, &digest).is_ok());

        // Reassemble body + hostile meta + attestation + freshly computed
        // checksum + magic + footer_len (the integrity layer then agrees;
        // any refusal is a parse-layer typed verdict).
        let real_footer_len =
            u32::from_le_bytes(clean[l - 4..l].try_into().unwrap()) as usize;
        let body = l - real_footer_len;
        let reassemble = |meta: &[u8]| -> (Vec<u8>, [u8; 32]) {
            let mut blk = Vec::new();
            blk.extend_from_slice(&clean[..body]);
            let m = meta.len();
            blk.extend_from_slice(meta);
            blk.extend_from_slice(&clean[l - 96..l - 16]); // attestation slot
            let checksum = fnv1a64(&blk);
            blk.extend_from_slice(&checksum.to_le_bytes());
            blk.extend_from_slice(&MAGIC);
            blk.extend_from_slice(&((TRAILER_FIXED + m) as u32).to_le_bytes());
            (blk.clone(), block_digest(&blk))
        };
        let re_err = |meta: &[u8]| -> SealError {
            let (blk, dg) = reassemble(meta);
            verify_seal(&blk, &dg).unwrap_err()
        };

        // Hostile meta: legal version field (field 1, value 1) followed by
        // the member's attack bytes. Built for real — the version bytes are
        // APPENDED to the attack, not written into a discarded Vec (the
        // original form of this helper dropped them, which vacuated every
        // member pin to "missing version"; the M-F2 flip witness below
        // exists precisely because that court could not flip).
        let hostile_meta = |attack: &[u8]| -> Vec<u8> {
            let mut meta = Vec::new();
            write_field_header(&mut meta, 1, 0, ctype::I16);
            write_zigzag_i16(&mut meta, 1);
            meta.extend_from_slice(attack);
            meta
        };
        // The legal tail (fields 2-6 + STOP) reused from the version1_tail
        // helper.
        let legal_tail = |spans: &[i32], flags: i32| -> Vec<u8> {
            version1_tail(1, 8, spans, fnv1a64(&clean[..1]), flags)
        };

        // Pinned member: nesting bomb through the UNKNOWN-field skip path —
        // a field-7 STRUCT whose body is 40 nested-struct header bytes; the
        // delegated depth guard (thrift.rs MAX_SKIP_DEPTH = 32) must type it
        // MetaMalformed, not blow the stack. Checksum is honest here.
        let bomb = hostile_meta(&{
            let mut a = Vec::new();
            write_field_header(&mut a, 7, 1, ctype::STRUCT);
            for _ in 0..40 {
                a.push(0xFC); // delta-15, STRUCT: one nesting level per byte
            }
            a
        });
        assert_eq!(
            re_err(&bomb),
            SealError::Shape(ShapeFault::MetaMalformed),
            "nesting bomb must hit the depth guard as a typed fault"
        );

        // Pinned member: giant declared span count — the list long form
        // (0xF marker) with a huge varint count; the bounded reader must
        // refuse at the first out-of-bounds element, typed.
        let giant_n = hostile_meta(&{
            let mut a = Vec::new();
            write_i32_field(&mut a, 2, 1, 1);
            write_i64_field(&mut a, 3, 2, 8);
            write_field_header(&mut a, 4, 3, ctype::LIST);
            a.push(0xF0 | ctype::I32);
            crate::rle::write_varint(&mut a, u64::MAX);
            for _ in 0..3 {
                crate::rle::write_varint(&mut a, u64::from(zigzag_encode_i32(8)));
            }
            a
        });
        assert_eq!(
            re_err(&giant_n),
            SealError::Shape(ShapeFault::MetaMalformed),
            "giant list count must refuse typed"
        );

        // Pinned member: negative span — u32::try_from fails, typed. (The
        // only fault is the span value; fields and flags are legal.)
        let neg_span = hostile_meta(&legal_tail(&[-1], 0));
        assert_eq!(re_err(&neg_span), SealError::Shape(ShapeFault::MetaMalformed));

        // Pinned member: forged flags on an otherwise legal block — the
        // span IS the body extent, so the flags value is the only fault and
        // the pinned verdict is attributable to the flags validation alone
        // (neutering that validation flips this member to Ok, witnessed by
        // the M-F2 sandbox mutation).
        let forged_flags = hostile_meta(&legal_tail(&[body as i32], i32::MIN));
        assert_eq!(
            re_err(&forged_flags),
            SealError::Shape(ShapeFault::MetaMalformed)
        );

        // Pinned members over the clean block: footer_len boundaries.
        let set_footer_len = |bytes: &[u8], fl: u32| -> Vec<u8> {
            let mut m = bytes.to_vec();
            let n = m.len();
            m[n - 4..].copy_from_slice(&fl.to_le_bytes());
            m
        };
        for (fl, want) in [
            (95u32, SealError::Shape(ShapeFault::BadFooterLen)),
            (96, SealError::Shape(ShapeFault::MetaMalformed)), // m=0: empty meta
            (l as u32, SealError::Shape(ShapeFault::EmptyBody)),
            (l as u32 + 1, SealError::Shape(ShapeFault::BadFooterLen)),
            (u32::MAX, SealError::Shape(ShapeFault::BadFooterLen)),
        ] {
            let mutated = set_footer_len(&clean, fl);
            assert_eq!(
                verify_seal(&mutated, &digest).unwrap_err(),
                want,
                "footer_len={fl} verdict drifted"
            );
            // RESTORE witness: the unmutated block still verifies between
            // members.
            assert!(verify_seal(&clean, &digest).is_ok());
        }

        // Fail-closed sweep: EVERY single-byte flip of the whole sealed
        // block is refused (the checksum spans everything, so any flip that
        // survives shape must fail the checksum; flips that break shape
        // fire earlier — all Err either way).
        for i in 0..l {
            let mut flipped = clean.clone();
            flipped[i] ^= 0x01;
            assert!(
                verify_seal(&flipped, &digest).is_err(),
                "single-byte flip at {i} must not verify"
            );
        }

        // The digest channel stays honest across the sweep.
        let mut wrong_digest = digest;
        wrong_digest[31] ^= 0x01;
        assert_eq!(
            verify_seal(&clean, &wrong_digest),
            Err(SealError::DigestMismatch)
        );
    }

    // ---- wave-8 repair court (W8-F1) ----

    /// Real Tier-H-shaped sink collaborator: a page-rounded window backend.
    /// `BlockSink::window` promises "at least `len` bytes", and a real mmap
    /// backend hands back a page-aligned mapping whose length rounds UP to
    /// the page multiple — so `window(len)` legitimately returns MORE than
    /// `len`. Hand-written real implementation (Chicago: a real
    /// collaborator, same class as the in-crate `StaticWindow`/`FileSink`,
    /// not a mock). Post-commit final state is observable through the
    /// shared committed store; `commit` copies the committed prefix into
    /// it, so the court asserts on real state, never on call counts.
    struct PageRoundingSink {
        window_buf: Vec<u8>,
        page: usize,
        committed: Rc<RefCell<Option<(usize, Vec<u8>)>>>,
    }

    impl PageRoundingSink {
        /// A sink whose window rounds `len` up to a `page`-byte multiple,
        /// capped at `cap` bytes of backing.
        fn new(cap: usize, page: usize, committed: Rc<RefCell<Option<(usize, Vec<u8>)>>>) -> Self {
            Self {
                window_buf: alloc::vec![0u8; cap],
                page,
                committed,
            }
        }
    }

    impl crate::mmap_writer::BlockSink for PageRoundingSink {
        fn window(&mut self, len: usize) -> &mut [u8] {
            let rounded = len.div_ceil(self.page) * self.page;
            let n = rounded.min(self.window_buf.len());
            &mut self.window_buf[..n]
        }

        fn commit(self, len: usize) -> Result<(), ParquetError> {
            if len > self.window_buf.len() {
                return Err(ParquetError::RunOverrun {
                    run_len: len,
                    budget: self.window_buf.len(),
                });
            }
            *self.committed.borrow_mut() = Some((len, self.window_buf[..len].to_vec()));
            Ok(())
        }
    }

    /// W8-F1 (wave 8, derived by re-reading — no wave-7 receipt for this
    /// lane exists on disk): `flush_to` checked only the SHORT-window arm
    /// and then did `window.copy_from_slice(self.bytes)`. On a LAWFUL
    /// backend whose window is LONGER than the block (the trait contract
    /// says "at least `len`"; page-rounded mmap windows are the expected
    /// Tier H shape), that copy panicked on a length mismatch — a silent
    /// time-bomb on external input. Repair: copy exactly the block length
    /// out of the window; short windows stay a typed refusal.
    ///
    /// Anti-vacuity mutation MUT-FOOT-9: revert `flush_to` to
    /// `window.copy_from_slice(self.bytes)`. The oversized-window arm then
    /// panics (copy length mismatch) and the court fails — the court
    /// distinguishes repaired from reverted. The refusal arm pins the
    /// exact refusal payload (run_len = block len, budget = window cap),
    /// so a neutered length check flips it too. Skips: none.
    #[test]
    fn w8_f1_flush_to_oversized_window_commits_block_len_only() {
        let mut b = StaticPageBuffer::<CAP>::new();
        let block = seal_block(b.as_mut_slice(), &[4, 9, 2, 6, 5, 3, 5, 8], 8, None);
        let n = block.bytes().len();

        // BEFORE (clean read-back): the same block verifies from the
        // buffer — the generator is real.
        let digest = *block.digest();
        assert!(verify_seal(block.bytes(), &digest).is_ok());

        // ARM 1 — oversized page-rounded window (the defect's trigger
        // shape): the backing is the page-rounded size, so `window(n)`
        // returns MORE than `n` — must commit exactly the block length,
        // block-byte-exact.
        let rounded = n.div_ceil(4096) * 4096;
        let shared = Rc::new(RefCell::new(None));
        let sink = PageRoundingSink::new(rounded, 4096, Rc::clone(&shared));
        block.flush_to(sink).expect("oversized window must flush typed Ok");
        let (committed_len, committed_bytes) =
            (*shared.borrow()).clone().expect("commit ran");
        assert_eq!(committed_len, n, "commit must carry the BLOCK length");
        assert_eq!(committed_bytes, block.bytes(), "committed bytes byte-exact");

        // ARM 2 — short window: typed refusal with the exact payload
        // (MUT-FOOT-9's neutered-check flip witness).
        let shared_short = Rc::new(RefCell::new(None));
        let short = PageRoundingSink::new(n - 1, 4096, Rc::clone(&shared_short));
        assert_eq!(
            block.flush_to(short).unwrap_err(),
            ParquetError::RunOverrun {
                run_len: n,
                budget: n - 1,
            },
            "short window must refuse with exact payload"
        );
        assert!(
            shared_short.borrow().is_none(),
            "refused flush must not commit"
        );

        // ARM 3 — exact-size in-crate sink (no_std StaticWindow): the
        // window returned for `n` is exactly `n`; copy and commit land.
        let mut win = crate::mmap_writer::StaticWindow::<2048>::new();
        crate::mmap_writer::BlockSink::window(&mut win, n).copy_from_slice(block.bytes());
        crate::mmap_writer::BlockSink::commit(win, n).expect("exact window commit");

        // RESTORE witness: the block still verifies from the buffer.
        assert!(verify_seal(block.bytes(), &digest).is_ok());
    }

    #[test]
    fn wrong_digest_is_digest_mismatch() {
        let (clean, mut digest) = clean_block();
        digest[0] ^= 0xFF;
        assert_eq!(verify_seal(&clean, &digest), Err(SealError::DigestMismatch));
    }

    #[test]
    fn meta_field5_is_body_digest_and_summary_round_trips() {
        let mut b = StaticPageBuffer::<CAP>::new();
        let block = seal_block(b.as_mut_slice(), &[5, 6, 7, 8, 9, 10, 11, 12], 8, None);
        let bytes = block.bytes();
        let l = bytes.len();
        let footer_len = u32::from_le_bytes(bytes[l - 4..l].try_into().unwrap()) as usize;
        let body_len = l - footer_len;
        let m = footer_len - TRAILER_FIXED;
        let parsed = parse_meta(&bytes[body_len..body_len + m]).expect("meta");
        assert_eq!(parsed.body_digest, fnv1a64(&bytes[..body_len]));
        assert_eq!(parsed.row_groups, 1);
        assert_eq!(parsed.rows, 8);
        assert_eq!(parsed.spans, alloc::vec![body_len as u32]);
        assert_eq!(parsed.flags, 0);

        // With the attestation: flags bit0 set, and Verified reports it.
        let block = seal_block(
            b.as_mut_slice(),
            &[5, 6, 7, 8, 9, 10, 11, 12],
            8,
            Some(&p28_attest_fixture()),
        );
        let v = verify_seal(block.bytes(), block.digest()).expect("verify");
        assert!(v.attestation_present);
        assert_eq!(v.rows, 8);
        assert_eq!(v.spans_len, 1);
        assert_eq!(v.spans[0] as usize, v.body_len);
        assert_eq!(v.meta_len, m);
    }

    #[test]
    fn meta_version_and_required_fields_fail_closed() {
        let body_digest = fnv1a64(&[0u8; 8]);
        // Version 2 -> unsupported.
        let mut meta = Vec::new();
        write_field_header(&mut meta, 1, 0, ctype::I16);
        write_zigzag_i16(&mut meta, 2);
        meta.extend(version1_tail(1, 8, &[4], body_digest, 0));
        assert_eq!(
            parse_meta(&meta).unwrap_err(),
            SealError::Shape(ShapeFault::MetaMalformed)
        );

        // Missing required field 3 (rows).
        let mut meta = Vec::new();
        write_field_header(&mut meta, 1, 0, ctype::I16);
        write_zigzag_i16(&mut meta, 1);
        write_i32_field(&mut meta, 2, 1, 1);
        // field 3 omitted; field 4 delta is then 4-2 = 2.
        write_field_header(&mut meta, 4, 2, ctype::LIST);
        meta.push((1u8 << 4) | ctype::I32);
        crate::rle::write_varint(&mut meta, u64::from(zigzag_encode_i32(4)));
        write_binary_field(&mut meta, 5, 4, &body_digest.to_le_bytes());
        write_i32_field(&mut meta, 6, 5, 0);
        meta.push(ctype::STOP);
        assert_eq!(
            parse_meta(&meta).unwrap_err(),
            SealError::Shape(ShapeFault::MetaMalformed)
        );

        // Flags fail-closed (MUT-FOOT-8): a negative value or an undefined
        // bit is a malformed block, never a wrapped cast that silently
        // sets/clears the attestation gate. Baseline flags=0 parses (the
        // flips must be attributable to the flags mutation alone).
        let base = || {
            let mut meta = Vec::new();
            write_field_header(&mut meta, 1, 0, ctype::I16);
            write_zigzag_i16(&mut meta, 1);
            meta.extend(version1_tail(1, 8, &[4], body_digest, 0));
            meta
        };
        assert!(parse_meta(&base()).is_ok(), "flags=0 baseline must parse");
        for forged in [i32::MIN, -1, 2, 3, 0x8000_0000u32 as i32] {
            let mut meta = Vec::new();
            write_field_header(&mut meta, 1, 0, ctype::I16);
            write_zigzag_i16(&mut meta, 1);
            meta.extend(version1_tail(1, 8, &[4], body_digest, forged));
            assert_eq!(
                parse_meta(&meta).unwrap_err(),
                SealError::Shape(ShapeFault::MetaMalformed),
                "forged flags {forged} must refuse"
            );
        }
        // And the legal bit0 value still parses.
        let mut meta = Vec::new();
        write_field_header(&mut meta, 1, 0, ctype::I16);
        write_zigzag_i16(&mut meta, 1);
        meta.extend(version1_tail(1, 8, &[4], body_digest, 1));
        assert!(parse_meta(&meta).is_ok(), "flags=1 must parse");
    }

    /// Fields 2-6 + STOP for a hand-built meta block (test helper).
    fn version1_tail(rg: i32, rows: i64, spans: &[i32], body_digest: u64, flags: i32) -> Vec<u8> {
        let mut meta = Vec::new();
        write_i32_field(&mut meta, 2, 1, rg);
        write_i64_field(&mut meta, 3, 2, rows);
        write_field_header(&mut meta, 4, 3, ctype::LIST);
        meta.push((spans.len() as u8) << 4 | ctype::I32);
        for s in spans {
            crate::rle::write_varint(&mut meta, u64::from(zigzag_encode_i32(*s)));
        }
        write_binary_field(&mut meta, 5, 4, &body_digest.to_le_bytes());
        write_i32_field(&mut meta, 6, 5, flags);
        meta.push(ctype::STOP);
        meta
    }

    // ---- A04 identity court (wave 9 cross-lane slice) ----

    /// The A04 §6 `CgapFrameHeader` identity court at the byte level.
    /// cgap-parquet is a zero-dependency crate (crate charter forbids a
    /// cgap-core dep edge), so the frozen 128 B frame ABI is pinned here BY
    /// VALUE with citations to the authoritative source
    /// (crates/cgap-core/src/frame.rs; static asserts :346-354), not
    /// redefined: magic "CGAP"@0, version 1@4, flags@5 (bit1 =
    /// FLAG_PARQUET 0x02), reserved 0@6..8, payload_byte_len u64 LE@8..16,
    /// oabi_digest[32]@16..48, attestation 80 B@48..128 (A17 §2.1).
    ///
    /// A02 composes with A04: the sealed block is the frame payload, and
    /// A02's 32-byte content address ([`block_digest`]) is the injected
    /// identity digest — frame.rs `verify_payload` semantics: the header's
    /// digest slot must equal the injected digest over the payload view
    /// [128..128+len], and payload_byte_len must equal the payload length.
    ///
    /// Honest scoping (divergence recorded, not silent): A04's lane-doc
    /// digest substrate is BLAKE3 (cgap_blake3::hash). cgap-parquet cannot
    /// link it (zero-dep charter); the frame contract itself is
    /// digest-agnostic at the injection seam (frame.rs:291 takes `impl
    /// Fn(&[u8]) -> [u8; 32]`), and this court witnesses that seam with
    /// A02's own non-cryptographic digest. No BLAKE3 equality is claimed.
    ///
    /// Anti-vacuity MUT-IDENT-1: flipping one payload byte must break the
    /// re-derived identity; flipping one header-digest byte must break the
    /// comparison while the payload re-derivation stays honest. Either arm
    /// going green under a neutered check means the court cannot flip —
    /// FAIL. RESTORE is byte-exact (the original frame never mutates; both
    /// mutations are on clones).
    #[test]
    fn a04_identity_court_sealed_block_frames_with_oabi_digest() {
        let mut b = StaticPageBuffer::<CAP>::new();
        let block = seal_block(
            b.as_mut_slice(),
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            8,
            Some(&p28_attest_fixture()),
        );
        let sealed = block.bytes().to_vec();
        let digest = *block.digest();

        // BEFORE: the generator is real — the sealed block verifies
        // standalone (A02 layer) before it is framed (A04 layer).
        assert!(verify_seal(&sealed, &digest).is_ok());

        // Frame ABI by value (citations: cgap-core/src/frame.rs).
        let mut frame = Vec::new();
        frame.extend_from_slice(b"CGAP"); // magic @0 (frame.rs:20)
        frame.push(1); // version @4 (frame.rs:23)
        frame.push(0x02); // flags @5, bit1 FLAG_PARQUET (frame.rs:29)
        frame.extend_from_slice(&[0u8; 2]); // reserved @6..8, zero on write (frame.rs:207)
        frame.extend_from_slice(&(sealed.len() as u64).to_le_bytes()); // payload_byte_len @8..16 (frame.rs:189)
        frame.extend_from_slice(&digest); // oabi_digest @16..48 (frame.rs:191)
        frame.extend_from_slice(&p28_attest_fixture()); // attestation @48..128 (A17 §2.1)
        assert_eq!(frame.len(), 128, "frame header is exactly 128 B (frame.rs:41,346)");
        frame.extend_from_slice(&sealed);

        // Identity 1: payload view and its declared length (frame.rs:271-280,
        // :296 — payload() refuses a length overrun; here the declaration
        // must be exact).
        let declared = u64::from_le_bytes(frame[8..16].try_into().unwrap());
        assert_eq!(declared as usize, sealed.len(), "payload_byte_len exact");
        assert_eq!(&frame[128..], &sealed[..], "payload view IS the sealed block");

        // Identity 2: oabi_digest == injected digest over the payload view's
        // CONTENT region (frame.rs:291-303 verify_payload semantics,
        // injection seam). A02's digest span is [0, B+M+80) — the content
        // region EXCLUDING the fixed 16-byte trailer (footer.rs §6.1: the
        // digest is "the same function as checksum, widened"; SealedBlock::
        // digest is taken over [..content_end)), i.e. sealed.len() - 16.
        let content_end = sealed.len() - 16;
        assert_eq!(&frame[16..48], &digest[..], "header carries A02's digest");
        assert_eq!(
            block_digest(&frame[128..128 + content_end]),
            digest,
            "re-derived identity holds"
        );

        // A17 §2.2 structural validity of the embedded attestation, pinned
        // by value (frame.rs:49-68 are the cited constants; same rules the
        // authoritative `P28LicenseAttestation::validate` enforces,
        // frame.rs:101-124): magic, version 1, exactly one lineage bit in
        // 0x07, reserved 0xFFF8 clear, ceiling <= $50M, upstream zero iff
        // ROOT (bit0).
        let attest = &frame[48..128];
        assert_eq!(&attest[0..4], b"P28S");
        assert_eq!(u16::from_le_bytes(attest[4..6].try_into().unwrap()), 1);
        let aflags = u16::from_le_bytes(attest[6..8].try_into().unwrap());
        assert_eq!(aflags & 0xFFF8, 0, "reserved lineage bits zero (A17 §2.2)");
        assert_eq!(
            (aflags & 0x07).count_ones(),
            1,
            "exactly one lineage bit (A17 §2.2)"
        );
        let ceiling = u64::from_le_bytes(attest[8..16].try_into().unwrap());
        assert!(ceiling <= 5_000_000_000, "safe-harbor ceiling (A17 §2.2)");
        let upstream = &attest[48..80];
        if aflags & 0x07 == 0x01 {
            assert!(
                upstream.iter().all(|&x| x == 0),
                "ROOT: upstream receipt zero (frame.rs:88)"
            );
        } else {
            assert!(
                upstream.iter().any(|&x| x != 0),
                "derivative requires nonzero upstream (A17 §2.2)"
            );
        }

        // MUT-IDENT-1 arm 1: one payload byte flipped -> re-derived identity
        // differs AND the header comparison refuses (the frame is not
        // admitted).
        let mut mutated = frame.clone();
        mutated[128 + 3] ^= 0x01;
        assert_ne!(
            block_digest(&mutated[128..128 + content_end]),
            digest,
            "flip arm 1: re-derived identity differs"
        );
        assert_ne!(
            &mutated[16..48],
            &block_digest(&mutated[128..128 + content_end])[..],
            "flip arm 1: header vs payload identity broken"
        );

        // MUT-IDENT-1 arm 2: one header-digest byte flipped -> comparison
        // refuses while the payload re-derivation stays honest.
        let mut hdr_flip = frame.clone();
        hdr_flip[20] ^= 0x01; // inside oabi_digest @16..48
        assert_ne!(&hdr_flip[16..48], &digest[..], "flip arm 2");
        assert_eq!(
            block_digest(&hdr_flip[128..128 + content_end]),
            digest,
            "flip arm 2: payload untouched"
        );

        // RESTORE (byte-exact): the pristine frame still verifies as a block
        // and re-derives its identity.
        assert!(verify_seal(&frame[128..], &digest).is_ok());
        assert_eq!(block_digest(&frame[128..128 + content_end]), digest);
    }

    // ---- C7 external-reader court (MUT-FOOT-7), Tier H ----

    #[cfg(all(test, feature = "std"))]
    mod tier_h {
        use super::*;
        use alloc::string::ToString;
        use std::{eprintln, format, string::String};

        /// The historical PCL2 defect as a byte-level mutation: relocate
        /// the encoding payload from the real-IDL slot 4 to the swapped
        /// slot 7 (byte-for-byte the header shape `thrift.rs` emitted
        /// before the receipt fixed it). The external reader must PASS on
        /// the real block and FAIL on the mutated one. Skips loud if
        /// python3 is absent.
        #[test]
        fn c7_external_reader_accepts_real_rejects_swapped_v2_ids() {
            let mut b = StaticPageBuffer::<CAP>::new();
            let block = seal_block(
                b.as_mut_slice(),
                &[1, 2, 3, 4, 5, 6, 7, 8],
                8,
                Some(&p28_attest_fixture()),
            );
            let clean = block.bytes().to_vec();

            // The mutated first page: swapped V2 substruct (4=def_len,
            // 5=rep_len, 6=is_compressed bool, 7=encoding).
            let values = [1u32, 2, 3, 4, 5, 6, 7, 8];
            let real_page = crate::page::write_data_page_v2(&values, 8, 8);
            let (_, header_len) = crate::thrift::read_page_header(&real_page).expect("hdr");
            let body_len = real_page.len() - header_len;
            let mut swapped_page = Vec::new();
            {
                use crate::thrift::{
                    ctype, encoding, page_type, write_bool_field, write_field_header,
                    write_i32_field,
                };
                write_i32_field(&mut swapped_page, 1, 0, page_type::DATA_PAGE_V2);
                write_i32_field(&mut swapped_page, 2, 1, body_len as i32);
                write_i32_field(&mut swapped_page, 3, 2, body_len as i32);
                write_field_header(&mut swapped_page, 8, 3, ctype::STRUCT);
                write_i32_field(&mut swapped_page, 1, 0, 8); // num_values
                write_i32_field(&mut swapped_page, 2, 1, 0); // num_nulls
                write_i32_field(&mut swapped_page, 3, 2, 8); // num_rows
                write_i32_field(&mut swapped_page, 4, 3, 0); // def_len (swapped)
                write_i32_field(&mut swapped_page, 5, 4, 0); // rep_len (swapped)
                write_bool_field(&mut swapped_page, 6, 5, false);
                write_i32_field(&mut swapped_page, 7, 6, encoding::RLE);
                swapped_page.push(ctype::STOP);
                swapped_page.push(ctype::STOP);
            }
            swapped_page.extend_from_slice(&real_page[header_len..]);
            assert_eq!(
                swapped_page.len(),
                real_page.len(),
                "PCL2 mutation must be length-preserving"
            );

            let mutated = {
                let mut m = clean.clone();
                m[..real_page.len()].copy_from_slice(&swapped_page);
                m
            };

            // Real collaborator: an external process parsing the block's
            // wire (footer meta + first page header) with an independent
            // compact-protocol reader.
            let Some(python) = which_python3() else {
                eprintln!("[SKIP] C7 external-reader court: python3 not found on PATH");
                return;
            };
            let dir = std::env::temp_dir();
            // Unique per run: two concurrent test-binary runs in a shared
            // checkout must not clobber each other's artifacts (fan-out
            // hygiene; wave-6 repair W6-F3).
            let real_path = dir.join(format!("a02_c7_real_{}.par2", std::process::id()));
            let mut_path = dir.join(format!("a02_c7_mutated_{}.par2", std::process::id()));
            std::fs::write(&real_path, &clean).expect("write real");
            std::fs::write(&mut_path, &mutated).expect("write mutated");

            let script = r#"
import struct, sys

class R:
    def __init__(self, data, pos=0):
        self.d = data; self.p = pos
    def byte(self):
        b = self.d[self.p]; self.p += 1; return b
    def varint(self):
        r = 0; s = 0
        while True:
            b = self.byte()
            r |= (b & 0x7F) << s
            if not (b & 0x80):
                return r
            s += 7
    def zz32(self):
        v = self.varint()
        return (v >> 1) ^ -(v & 1)
    def header(self, prev):
        b = self.byte()
        ty = b & 0x0F
        if ty == 0:
            return None
        delta = b >> 4
        fid = prev + delta if delta else self.zz32()
        return fid, ty

def parse_fields(r, structs_deep=False):
    prev = 0; f = {}
    while True:
        h = r.header(prev)
        if h is None:
            break
        fid, ty = h; prev = fid
        if ty == 12:
            sub = {}
            f[fid] = ('struct', parse_fields(r))
        elif ty == 5:
            f[fid] = ('i32', r.zz32())
        elif ty == 6:
            f[fid] = ('i64', r.zz32())
        elif ty == 4:
            f[fid] = ('i16', r.zz32())
        elif ty == 8:
            ln = r.varint()
            f[fid] = ('bin', bytes(r.d[r.p:r.p+ln])); r.p += ln
        elif ty == 9:
            hb = r.byte(); n = hb >> 4
            if n == 15:
                n = r.varint()
            ety = hb & 0x0F
            items = []
            for _ in range(n):
                if ety == 5:
                    items.append(r.zz32())
                else:
                    raise SystemExit("unexpected list elem type %d" % ety)
            f[fid] = ('list', items)
        elif ty in (1, 2):
            f[fid] = ('bool', ty == 1)
        elif ty == 3:
            f[fid] = ('byte', r.byte())
        else:
            raise SystemExit("unexpected type %d" % ty)
    return f

def verdict(path):
    d = open(path, 'rb').read()
    L = len(d)
    if L < 96:
        return "FAIL: too short"
    if d[L-8:L-4] != b'PAR2':
        return "FAIL: magic"
    flen = struct.unpack('<I', d[L-4:L])[0]
    if flen < 96 or flen > L:
        return "FAIL: footer len"
    B = L - flen
    if B <= 0:
        return "FAIL: empty body"
    M = flen - 96
    meta = parse_fields(R(d, B))
    if meta.get(1, (None, None))[1] != 1:
        return "FAIL: meta version"
    rg = meta.get(2, (None, 0))[1]
    if not isinstance(rg, int) or isinstance(rg, bool) or rg < 1:
        return "FAIL: row group count"
    ph = parse_fields(R(d, 0))
    st = ph.get(8)
    if not st or st[0] != 'struct':
        return "FAIL: no v2 substruct"
    s = st[1]
    enc = s.get(4)
    comp = s.get(7)
    if not enc or enc[0] != 'i32' or enc[1] != 3:
        return "FAIL: field 4 encoding != RLE (i32): %r" % (enc,)
    if not comp or comp[0] != 'bool' or comp[1]:
        return "FAIL: field 7 is_compressed not explicit false (bool): %r" % (comp,)
    if s.get(5) != ('i32', 0) or s.get(6) != ('i32', 0):
        return "FAIL: level byte lengths nonzero: %r %r" % (s.get(5), s.get(6))
    return "PASS"

print("VERDICT:", verdict(sys.argv[1]))
"#;
            let run = |path: &std::path::Path| -> Result<String, String> {
                use std::process::{Command, Stdio};
                let out = Command::new(&python)
                    .arg("-c")
                    .arg(script)
                    .arg(path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .map_err(|e| format!("spawn: {e}"))?;
                Ok(String::from_utf8_lossy(&out.stdout).to_string())
            };
            let real_out = run(&real_path).expect("python run (real)");
            let mut_out = run(&mut_path).expect("python run (mutated)");
            let _ = std::fs::remove_file(&real_path);
            let _ = std::fs::remove_file(&mut_path);

            eprintln!("C7 real: {}", real_out.trim());
            eprintln!("C7 mutated: {}", mut_out.trim());
            assert!(
                real_out.contains("VERDICT: PASS"),
                "external reader must accept the real block: {real_out}"
            );
            assert!(
                mut_out.contains("VERDICT: FAIL"),
                "MUT-FOOT-7 must flip the external verdict: {mut_out}"
            );
        }

        fn which_python3() -> Option<std::ffi::OsString> {
            let path = std::env::var_os("PATH")?;
            for dir in std::env::split_paths(&path) {
                let cand = dir.join("python3");
                if cand.is_file() {
                    return Some(cand.into_os_string());
                }
            }
            None
        }
    }
}
