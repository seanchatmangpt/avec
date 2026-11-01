//! Pack v2 and idx v2 emission plus an independent in-crate parser used by
//! the Chicago tests (real bytes in, real bytes asserted).
//!
//! v26.10.31 FR-04 flip: object bodies are emitted as zlib streams (RFC
//! 1950/1951, in-crate `crate::deflate`) — git pack v2 requires this. The
//! varint header carries the UNCOMPRESSED size; the body is the zlib stream.

use crate::deflate::zlib_compress;
use crate::{GitError, Oid};
use alloc::string::ToString;
use alloc::vec::Vec;

// ---------------------------------------------------------------- SHA-1 --
// Minimal SHA-1 (FIPS 180-1), in-crate: the workspace lock carries no sha1
// crate and the lane contract forbids new external dependencies.

/// Streaming SHA-1.
#[derive(Clone)]
pub struct Sha1 {
    state: [u32; 5],
    len_bits: u64,
    buf: [u8; 64],
    buf_len: usize,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    pub fn new() -> Self {
        Sha1 {
            state: [
                0x6745_2301,
                0xEFCD_AB89,
                0x98BA_DCFE,
                0x1032_5476,
                0xC3D2_E1F0,
            ],
            len_bits: 0,
            buf: [0u8; 64],
            buf_len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.len_bits = self.len_bits.wrapping_add((data.len() as u64) * 8);
        while !data.is_empty() {
            let n = core::cmp::min(64 - self.buf_len, data.len());
            self.buf[self.buf_len..self.buf_len + n].copy_from_slice(&data[..n]);
            self.buf_len += n;
            data = &data[n..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
    }

    /// Pad, absorb the length, and return the 20-byte digest.
    pub fn finalize(mut self) -> [u8; 20] {
        let bits = self.len_bits;
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0]);
        }
        // update() polluted len_bits with padding; overwrite the length field
        // from the saved value.
        self.buf[56..64].copy_from_slice(&bits.to_be_bytes());
        let block = self.buf;
        self.compress(&block);
        let mut out = [0u8; 20];
        for (i, w) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.state;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
    }
}

/// One-shot SHA-1.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize()
}

/// Git object name: SHA-1 over `"<type> <size>\0<content>"`.
pub fn git_object_oid(kind: &str, content: &[u8]) -> Oid {
    let mut h = Sha1::new();
    h.update(kind.as_bytes());
    h.update(b" ");
    h.update(content.len().to_string().as_bytes());
    h.update(b"\0");
    h.update(content);
    h.finalize()
}

// ----------------------------------------------------------------- CRC32 --
// IEEE CRC-32 (reflected, poly 0xEDB88320), const-built table.

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = build_crc_table();

/// Streaming IEEE CRC-32.
#[derive(Clone)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Crc32 {
    pub fn new() -> Self {
        Crc32 { state: 0xFFFF_FFFF }
    }

    pub fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.state = (self.state >> 8) ^ CRC_TABLE[((self.state ^ b as u32) & 0xFF) as usize];
        }
    }

    pub fn finalize(self) -> u32 {
        !self.state
    }
}

/// One-shot IEEE CRC-32.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = Crc32::new();
    c.update(data);
    c.finalize()
}

// ------------------------------------------------------------- varint ----

/// Encode a pack object header (type + size) varint per pack-format:
/// bit 7 continuation, bits 6..4 type, bits 3..0 low size bits, then 7-bit
/// little-endian size continuation bytes.
pub fn encode_object_header(obj_type: u8, size: u64) -> alloc::vec::Vec<u8> {
    assert!((1..=4).contains(&obj_type) || obj_type == 6 || obj_type == 7);
    let mut out = alloc::vec::Vec::with_capacity(8);
    let mut c = ((obj_type & 0x7) << 4) | (size as u8 & 0x0F);
    let mut s = size >> 4;
    while s != 0 {
        out.push(c | 0x80);
        c = (s as u8) & 0x7F;
        s >>= 7;
    }
    out.push(c);
    out
}

// ----------------------------------------------------------- pack write --

/// One packed object record: git OID, byte offset in the pack, entry CRC32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdxEntry {
    pub oid: Oid,
    pub offset: u32,
    pub crc: u32,
}

/// Maximum pack object count: spec Section 3 cap (open item L6-1).
pub const MAX_OBJECTS: usize = 65_536;

/// Serialize a v2 pack of zlib-compressed blob objects (type 3).
///
/// Each body is a zlib stream of the blob's bytes; the object header size
/// field is the uncompressed length, per pack-format. Conformant for real
/// `git fsck` / `verify-pack` / `cat-file`.
pub fn write_pack_v2(blobs: &[&[u8]]) -> Result<alloc::vec::Vec<u8>, GitError> {
    if blobs.len() > MAX_OBJECTS {
        return Err(GitError::TooManyObjects(blobs.len()));
    }
    let mut out = alloc::vec::Vec::new();
    out.extend_from_slice(b"PACK");
    out.extend_from_slice(&2u32.to_be_bytes());
    out.extend_from_slice(&(blobs.len() as u32).to_be_bytes());
    for blob in blobs {
        if blob.len() > u32::MAX as usize {
            return Err(GitError::SizeOverflow);
        }
        out.extend_from_slice(&encode_object_header(3, blob.len() as u64));
        out.extend_from_slice(&zlib_compress(blob));
    }
    let sum = sha1(&out);
    out.extend_from_slice(&sum);
    Ok(out)
}

/// Serialize a v2 `.idx` over sorted `(oid, offset, crc)` records per spec
/// Section 1: magic, version, 256-row fan-out, OID table, CRC table,
/// small-offset table, pack checksum, idx checksum. Large-offset table
/// omitted (no offsets exceed 2^31-1 in v1; `OffsetTooLarge` otherwise).
pub fn write_idx_v2(
    entries: &mut [IdxEntry],
    pack_checksum: &Oid,
) -> Result<alloc::vec::Vec<u8>, GitError> {
    if entries.len() > MAX_OBJECTS {
        return Err(GitError::TooManyObjects(entries.len()));
    }
    entries.sort_by_key(|e| e.oid);
    for e in entries.iter() {
        if e.offset & 0x8000_0000 != 0 || e.offset as u64 >= 0x8000_0000 {
            return Err(GitError::OffsetTooLarge);
        }
    }
    let n = entries.len();
    let mut out = alloc::vec::Vec::with_capacity(1032 + 44 * n + 40);
    out.extend_from_slice(&[0xFF, 0x74, 0x4F, 0x63]);
    out.extend_from_slice(&2u32.to_be_bytes());

    // fan-out: cumulative count of OIDs with first byte <= i
    let mut counts = [0u32; 256];
    for e in entries.iter() {
        counts[e.oid[0] as usize] += 1;
    }
    let mut cum = 0u32;
    for c in counts.iter_mut() {
        cum += *c;
        *c = cum;
    }
    for c in counts {
        out.extend_from_slice(&c.to_be_bytes());
    }
    for e in entries.iter() {
        out.extend_from_slice(&e.oid);
    }
    for e in entries.iter() {
        out.extend_from_slice(&e.crc.to_be_bytes());
    }
    for e in entries.iter() {
        out.extend_from_slice(&e.offset.to_be_bytes());
    }
    out.extend_from_slice(pack_checksum);
    let idx_sum = sha1(&out);
    out.extend_from_slice(&idx_sum);
    Ok(out)
}

/// Build pack + idx + the per-object `(oid, offset, crc)` table from blob
/// contents in one pass. This is the primary entry point.
pub fn build_pack_and_idx(
    blobs: &[&[u8]],
) -> Result<
    (
        alloc::vec::Vec<u8>,
        alloc::vec::Vec<u8>,
        alloc::vec::Vec<IdxEntry>,
    ),
    GitError,
> {
    let pack = write_pack_v2(blobs)?;
    let pack_checksum: Oid = pack[pack.len() - 20..].try_into().unwrap();
    let mut entries = Vec::with_capacity(blobs.len());
    let mut off = 12usize;
    for blob in blobs {
        let start = off;
        let hdr = encode_object_header(3, blob.len() as u64);
        let body = zlib_compress(blob);
        off += hdr.len() + body.len();
        let mut crc = Crc32::new();
        crc.update(&hdr);
        crc.update(&body);
        entries.push(IdxEntry {
            oid: git_object_oid("blob", blob),
            offset: start as u32,
            crc: crc.finalize(),
        });
    }
    let idx = write_idx_v2(&mut entries, &pack_checksum)?;
    Ok((pack, idx, entries))
}

/// `PackBuilder` mirrors the spec Section 4 append-then-regenerate flow.
#[derive(Default)]
pub struct PackBuilder {
    blobs: alloc::vec::Vec<alloc::vec::Vec<u8>>,
}

impl PackBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one blob; growth is tail-only.
    pub fn push(&mut self, blob: &[u8]) {
        self.blobs.push(blob.to_vec());
    }

    pub fn len(&self) -> usize {
        self.blobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blobs.is_empty()
    }

    /// Regenerate pack + idx from the full pending set.
    pub fn finish(&self) -> Result<(alloc::vec::Vec<u8>, alloc::vec::Vec<u8>), GitError> {
        let refs: alloc::vec::Vec<&[u8]> = self.blobs.iter().map(|b| b.as_slice()).collect();
        let (pack, idx, _) = build_pack_and_idx(&refs)?;
        Ok((pack, idx))
    }
}

// ------------------------------------------------------------ pack read --
// Independent parser (separate code path from the writer) for Chicago tests.

/// A parsed stored object: type code, claimed size, entry offset, CRC32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedObject {
    pub obj_type: u8,
    pub size: u64,
    pub offset: u32,
    pub crc: u32,
}

fn read_varint_header(data: &[u8], pos: usize) -> Result<(u8, u64, usize), GitError> {
    let mut i = pos;
    let first = *data.get(i).ok_or(GitError::Truncated)?;
    i += 1;
    let obj_type = (first >> 4) & 0x7;
    if obj_type == 0 || obj_type == 5 {
        return Err(GitError::BadVarint);
    }
    let mut size = (first & 0x0F) as u64;
    let mut shift = 4u32;
    let mut cont = first & 0x80 != 0;
    while cont {
        let b = *data.get(i).ok_or(GitError::Truncated)?;
        i += 1;
        size |= ((b & 0x7F) as u64) << shift;
        shift += 7;
        cont = b & 0x80 != 0;
    }
    Ok((obj_type, size, i - pos))
}

/// Parse a v2 pack of zlib-compressed objects: verify magic, version, every
/// object header/body boundary (inflating each body and checking its
/// declared uncompressed size), and the trailing SHA-1 over all preceding
/// bytes. Returns per-object records in file order.
pub fn parse_pack_v2(pack: &[u8]) -> Result<alloc::vec::Vec<ParsedObject>, GitError> {
    if pack.len() < 32 {
        return Err(GitError::Truncated);
    }
    if &pack[0..4] != b"PACK" || u32::from_be_bytes(pack[4..8].try_into().unwrap()) != 2 {
        return Err(GitError::BadMagic);
    }
    let count = u32::from_be_bytes(pack[8..12].try_into().unwrap()) as usize;
    let body_end = pack.len() - 20;
    let expect: Oid = pack[body_end..].try_into().unwrap();
    if sha1(&pack[..body_end]) != expect {
        return Err(GitError::PackChecksumMismatch);
    }
    let mut pos = 12usize;
    let mut objs = alloc::vec::Vec::with_capacity(count);
    for _ in 0..count {
        let (obj_type, size, hdr_len) = read_varint_header(pack, pos)?;
        if size > u32::MAX as u64 {
            return Err(GitError::SizeOverflow);
        }
        let body_start = pos + hdr_len;
        // Malformed header: the object header can claim more bytes than the
        // pack body holds, which would make the zlib-window range below
        // start past its end (panic on external input). Refuse as truncated.
        if body_start + 2 > pack.len() - 20 {
            return Err(GitError::Truncated);
        }
        // Inflate the zlib body to find its true extent and validate the
        // declared uncompressed size.
        let (data, used) = crate::deflate::inflate(&pack[body_start + 2..pack.len() - 20])
            .map_err(|_| GitError::BadVarint)?;
        let used = used + 2 + 4; // zlib header + deflate stream + adler32 trailer
        if data.len() as u64 != size {
            return Err(GitError::BadVarint);
        }
        let body_end = body_start + used;
        if body_end > pack.len() - 20 {
            return Err(GitError::Truncated);
        }
        let mut crc = Crc32::new();
        crc.update(&pack[pos..body_end]);
        objs.push(ParsedObject {
            obj_type,
            size,
            offset: pos as u32,
            crc: crc.finalize(),
        });
        pos = body_end;
    }
    if pos != pack.len() - 20 {
        return Err(GitError::Truncated);
    }
    Ok(objs)
}

/// Parse a v2 `.idx`: verify magic, version, both trailing checksums, fan-out
/// monotonicity, OID sort order, and return `(oids, crcs, offsets)`.
pub fn parse_idx_v2(
    idx: &[u8],
) -> Result<
    (
        alloc::vec::Vec<Oid>,
        alloc::vec::Vec<u32>,
        alloc::vec::Vec<u32>,
    ),
    GitError,
> {
    if idx.len() < 1032 + 40 {
        return Err(GitError::Truncated);
    }
    if idx[0..4] != [0xFF, 0x74, 0x4F, 0x63]
        || u32::from_be_bytes(idx[4..8].try_into().unwrap()) != 2
    {
        return Err(GitError::BadMagic);
    }
    let idx_sum_end = idx.len();
    let idx_sum: Oid = idx[idx_sum_end - 20..].try_into().unwrap();
    if sha1(&idx[..idx_sum_end - 20]) != idx_sum {
        return Err(GitError::IdxChecksumMismatch);
    }
    let pack_sum: Oid = idx[idx_sum_end - 40..idx_sum_end - 20].try_into().unwrap();
    let n_table_end = 8 + 1024;
    let mut fanout = [0u32; 256];
    for i in 0..256 {
        fanout[i] = u32::from_be_bytes(idx[8 + i * 4..8 + i * 4 + 4].try_into().unwrap());
    }
    for w in fanout.windows(2) {
        if w[1] < w[0] {
            return Err(GitError::BadMagic);
        }
    }
    let n = fanout[255] as usize;
    let oids_end = n_table_end + 20 * n;
    let crcs_end = oids_end + 4 * n;
    let offs_end = crcs_end + 4 * n;
    if idx.len() < offs_end + 40 {
        return Err(GitError::Truncated);
    }
    let stored_pack_sum: Oid = idx[offs_end..offs_end + 20].try_into().unwrap();
    if pack_sum != stored_pack_sum {
        return Err(GitError::BadMagic);
    }
    let mut oids = alloc::vec::Vec::with_capacity(n);
    for i in 0..n {
        oids.push(
            idx[n_table_end + i * 20..n_table_end + i * 20 + 20]
                .try_into()
                .unwrap(),
        );
    }
    for w in oids.windows(2) {
        if w[1] <= w[0] {
            return Err(GitError::BadMagic);
        }
    }
    let mut crcs = alloc::vec::Vec::with_capacity(n);
    let mut offs = alloc::vec::Vec::with_capacity(n);
    for i in 0..n {
        crcs.push(u32::from_be_bytes(
            idx[oids_end + i * 4..oids_end + i * 4 + 4]
                .try_into()
                .unwrap(),
        ));
        offs.push(u32::from_be_bytes(
            idx[crcs_end + i * 4..crcs_end + i * 4 + 4]
                .try_into()
                .unwrap(),
        ));
    }
    Ok((oids, crcs, offs))
}

// ---------------------------------------------------------------- tests --

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn blob_bytes(n: usize, seed: u8) -> Vec<u8> {
        (0..n).map(|i| (i as u8).wrapping_add(seed)).collect()
    }

    #[test]
    fn sha1_known_vectors() {
        // FIPS 180-1 / common vectors
        assert_eq!(
            &hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(&hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        let long = vec![b'a'; 1_000_000];
        assert_eq!(
            &hex(&sha1(&long)),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn crc32_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    fn hex(b: &[u8]) -> alloc::string::String {
        let mut s = alloc::string::String::new();
        for byte in b {
            s.push_str(&alloc::format!("{:02x}", byte));
        }
        s
    }

    #[test]
    fn pack_roundtrip_counts_offsets_checksums() {
        let blobs: Vec<Vec<u8>> = (0..16).map(|i| blob_bytes(64 * (i + 1), i as u8)).collect();
        let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
        let (pack, idx, entries) = build_pack_and_idx(&refs).unwrap();

        assert_eq!(&pack[0..4], b"PACK");
        assert_eq!(u32::from_be_bytes(pack[4..8].try_into().unwrap()), 2);
        assert_eq!(u32::from_be_bytes(pack[8..12].try_into().unwrap()), 16);

        // independent parser: counts, types, sizes, checksum
        let parsed = parse_pack_v2(&pack).unwrap();
        assert_eq!(parsed.len(), 16);
        for (p, blob) in parsed.iter().zip(blobs.iter()) {
            assert_eq!(p.obj_type, 3);
            assert_eq!(p.size, blob.len() as u64);
            // zlib body at the recorded offset must inflate back to the blob
            let hdr_len = encode_object_header(3, p.size).len();
            let start = p.offset as usize + hdr_len;
            let next = parsed
                .iter()
                .find(|q| q.offset > p.offset)
                .map(|q| q.offset as usize)
                .unwrap_or(pack.len() - 20);
            let back = crate::deflate::zlib_decompress(&pack[start..next]).unwrap();
            assert_eq!(back, *blob);
        }

        // idx: OIDs sorted, fan-out cumulative, offsets match entries
        let (oids, crcs, offs) = parse_idx_v2(&idx).unwrap();
        assert_eq!(oids.len(), 16);
        let mut sorted = entries.clone();
        sorted.sort_by_key(|e| e.oid);
        for i in 0..16 {
            assert_eq!(oids[i], sorted[i].oid);
            assert_eq!(offs[i], sorted[i].offset);
            assert_eq!(crcs[i], sorted[i].crc);
            assert_eq!(
                crcs[i],
                parsed.iter().find(|p| p.offset == offs[i]).unwrap().crc
            );
        }
    }

    #[test]
    fn builder_matches_direct() {
        let blobs: Vec<Vec<u8>> = (0..5).map(|_| blob_bytes(100, 7)).collect();
        let mut b = PackBuilder::new();
        for blob in &blobs {
            b.push(blob);
        }
        assert_eq!(b.len(), 5);
        let (p1, i1) = b.finish().unwrap();
        let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
        let (p2, i2, _) = build_pack_and_idx(&refs).unwrap();
        assert_eq!(p1, p2);
        assert_eq!(i1, i2);
    }

    #[test]
    fn corrupted_pack_trailer_fails_checksum() {
        let blob = blob_bytes(32, 1);
        let mut pack = write_pack_v2(&[&blob]).unwrap();
        let last = pack.len() - 1;
        pack[last] ^= 0xFF;
        match parse_pack_v2(&pack) {
            Err(GitError::PackChecksumMismatch) => {}
            other => panic!("expected checksum mismatch, got {:?}", other),
        }
    }

    #[test]
    fn corrupted_idx_trailer_fails_checksum() {
        let blob = blob_bytes(32, 2);
        let (pack, mut idx, _) = build_pack_and_idx(&[&blob]).unwrap();
        let _ = pack;
        let last = idx.len() - 1;
        idx[last] ^= 0x01;
        match parse_idx_v2(&idx) {
            Err(GitError::IdxChecksumMismatch) => {}
            other => panic!("expected idx checksum mismatch, got {:?}", other),
        }
    }

    #[test]
    fn object_count_cap_enforced() {
        assert_eq!(MAX_OBJECTS, 65_536);
        // Cap path exercised through write_idx_v2 with a fabricated oversized
        // table (no 65k-blob allocation needed).
        let mut entries: Vec<IdxEntry> = (0..MAX_OBJECTS + 1)
            .map(|i| IdxEntry {
                oid: core::array::from_fn(|j| ((i + j) & 0xFF) as u8),
                offset: 12,
                crc: 0,
            })
            .collect();
        let cs: Oid = [0u8; 20];
        match write_idx_v2(&mut entries, &cs) {
            Err(GitError::TooManyObjects(n)) => assert_eq!(n, MAX_OBJECTS + 1),
            other => panic!("expected cap error, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn bad_magic_rejected() {
        let blob = blob_bytes(16, 3);
        let mut pack = write_pack_v2(&[&blob]).unwrap();
        pack[3] = b'X';
        assert_eq!(parse_pack_v2(&pack), Err(GitError::BadMagic));
    }

    #[test]
    fn empty_pack_is_valid() {
        let pack = write_pack_v2(&[]).unwrap();
        assert_eq!(pack.len(), 32); // 12 header + 20 trailer
        let parsed = parse_pack_v2(&pack).unwrap();
        assert!(parsed.is_empty());
        let (_, idx, _) = build_pack_and_idx(&[]).unwrap();
        let (oids, _, _) = parse_idx_v2(&idx).unwrap();
        assert!(oids.is_empty());
    }
}
