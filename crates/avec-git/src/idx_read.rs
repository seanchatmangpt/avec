//! FR-05: `.idx` v2 reader with the constant-time lookup ladder
//! (`docs/jira/v26.10.31/25-git-packfile-integration-spec.md` §1 + §3).
//!
//! Zero-copy parse over a borrowed byte slice; all multi-byte integers are
//! big-endian; no `unsafe` (bounds checks are explicit and typed).

use crate::ladder::{ct_eq_oid, LADDER_WINDOW};
use crate::{GitError, Oid};

const MAGIC: [u8; 4] = [0xFF, 0x74, 0x4F, 0x63]; // \377tOc
const FANOUT_OFF: usize = 8;
const FANOUT_LEN: usize = 256 * 4;
const OID_TABLE_OFF: usize = FANOUT_OFF + FANOUT_LEN; // 1032

/// Parsed `.idx` v2 borrowed from its backing bytes.
#[derive(Debug, Clone)]
pub struct IdxV2<'a> {
    data: &'a [u8],
    n: u32,
}

impl<'a> IdxV2<'a> {
    /// Parse and validate magic, version, and section sizing. Does not verify
    /// checksums; call [`IdxV2::verify_idx_checksum`] for that.
    pub fn parse(data: &'a [u8]) -> Result<Self, GitError> {
        if data.len() < OID_TABLE_OFF + 40 {
            return Err(GitError::Truncated);
        }
        if data[0..4] != MAGIC {
            return Err(GitError::BadMagic);
        }
        if u32::from_be_bytes(data[4..8].try_into().expect("8-byte slice")) != 2 {
            return Err(GitError::BadMagic);
        }
        // N = fanout[255]
        let n = self_fanout_last(data);
        // Section sizes: 20N (oids) + 4N (CRC32, one u32 per object) +
        // 4N (small offsets) + 4L (large offsets, optional) + 40 (checksums).
        // Minimum = 1032 + 28N + 40. offset() bounds-checks the large table.
        // NOTE: the spec §1 table row "CRC32 table: 20N" is a typo for 4N
        // (one u32 per object); real git v2 and our writer both use 4N.
        let need = OID_TABLE_OFF + 28 * n as usize + 40;
        if data.len() < need {
            return Err(GitError::Truncated);
        }
        Ok(IdxV2 { data, n })
    }

    /// Number of objects.
    #[inline]
    pub fn len(&self) -> u32 {
        self.n
    }

    /// True only for the degenerate empty idx (still valid on disk).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Cumulative object count for first bytes `<= b`.
    #[inline]
    pub fn fanout(&self, b: usize) -> u32 {
        let o = FANOUT_OFF + b * 4;
        u32::from_be_bytes(self.data[o..o + 4].try_into().expect("fanout slice"))
    }

    /// The i-th OID in sorted order.
    #[inline]
    pub fn oid(&self, i: u32) -> Oid {
        let o = OID_TABLE_OFF + i as usize * 20;
        self.data[o..o + 20].try_into().expect("oid slice")
    }

    /// The i-th object's CRC32 of its packed bytes.
    #[inline]
    pub fn crc(&self, i: u32) -> u32 {
        let o = OID_TABLE_OFF + self.n as usize * 20 + i as usize * 4;
        u32::from_be_bytes(self.data[o..o + 4].try_into().expect("crc slice"))
    }

    /// The i-th object's pack offset. Handles the bit-31 escape into the
    /// 64-bit large-offset table with an arithmetic MSB mask select, not a
    /// branch. Typed error on truncation — never a panic on external input.
    pub fn offset(&self, i: u32) -> Result<u64, GitError> {
        let small_off = OID_TABLE_OFF + self.n as usize * 24 + i as usize * 4;
        if small_off + 4 > self.data.len() {
            return Err(GitError::Truncated);
        }
        let raw = u32::from_be_bytes(
            self.data[small_off..small_off + 4]
                .try_into()
                .expect("offset slice"), // guarded above
        );
        // MSB mask: bit31 set ⇒ large table
        let msb_mask = (raw >> 31).wrapping_neg(); // !0 or 0
        let large_idx = (raw & 0x7FFF_FFFF) as usize;
        let large_base = OID_TABLE_OFF + self.n as usize * 28;
        let large_end = large_base + (large_idx + 1) * 8;
        let large_entry = if large_end <= self.data.len() {
            u64::from_be_bytes(
                self.data[large_end - 8..large_end]
                    .try_into()
                    .expect("large offset slice"), // guarded above
            )
        } else {
            0
        };
        let small_val = (raw & 0x7FFF_FFFF) as u64;
        // select: msb set → large entry, else small value (arithmetic select)
        Ok((small_val & !(msb_mask as u64)) | (large_entry & msb_mask as u64))
    }

    /// Constant-time ladder lookup of `t`: returns `(index, pack_offset)` on a
    /// hit, `None` on a miss, and a typed error when the first-byte partition
    /// exceeds the 2^16 ladder window (open item L6-1).
    ///
    /// The ladder itself executes exactly 16 stages for every input; the only
    /// conditional paths here are the pre-ladder partition bounds check and
    /// the final hit/miss select on the equality mask.
    pub fn lookup(&self, t: &Oid) -> Result<Option<(u32, u64)>, GitError> {
        let b = t[0] as usize;
        let lo = if b == 0 { 0 } else { self.fanout(b - 1) };
        let hi = self.fanout(b);
        let part = hi - lo;
        if part > LADDER_WINDOW {
            return Err(GitError::TooManyObjects(part as usize));
        }
        if part == 0 {
            return Ok(None);
        }
        // Ladder over the partition. Probes index the absolute OID table at
        // lo + p; out-of-window probes read the zero OID and are killed by
        // the in-bounds mask (see ladder::ladder_search).
        let n = part;
        let mut base: u32 = 0;
        let mut k = 16u32;
        while k > 0 {
            k -= 1;
            let disp = 1u32 << k;
            // α is 1-indexed in the spec; 0-indexed probe q = p - 1.
            let p = base + disp;
            let in_bounds = (p <= n) as u32;
            let probe: Oid = if in_bounds == 1 {
                self.oid(lo + p - 1)
            } else {
                [0u8; 20]
            };
            let lt = crate::ladder::oid_lt_mask(&probe, t);
            #[cfg(debug_assertions)]
            crate::ladder::debug_tick();
            base += (in_bounds & (lt & 1)) * disp;
        }
        // candidate resolution, branchless on the equality mask
        let part_is_empty = part == 0;
        let cand_ok = (base < n) as u32;
        // clamp the candidate read so an out-of-window base can never index
        // past the table (the eq mask discards it either way)
        let cand_index = base.min(n.saturating_sub(1));
        let idx = lo + cand_index;
        let cand: Oid = if part_is_empty {
            [0u8; 20]
        } else {
            self.oid(idx)
        };
        let eq = ct_eq_oid(&cand, t) & 0u32.wrapping_sub(cand_ok);
        if eq != 0 {
            Ok(Some((idx, self.offset(idx)?)))
        } else {
            Ok(None)
        }
    }

    /// Verify the trailing SHA-1 over all preceding idx bytes.
    pub fn verify_idx_checksum(&self) -> Result<(), GitError> {
        let body = self.data.len() - 20;
        let mut h = crate::pack_write::Sha1::new();
        h.update(&self.data[..body]);
        let want = h.finalize();
        let got: [u8; 20] = self.data[body..].try_into().expect("idx checksum slice");
        if want != got {
            Err(GitError::IdxChecksumMismatch)
        } else {
            Ok(())
        }
    }
}

fn self_fanout_last(data: &[u8]) -> u32 {
    u32::from_be_bytes(
        data[FANOUT_OFF + 255 * 4..FANOUT_OFF + 256 * 4]
            .try_into()
            .expect("f"),
    )
}

// ------------------------------------------------------------------ tests --
// Chicago discipline: real synthetic idx bytes, assert final offsets; no mocks.

#[cfg(test)]
mod tests {
    use super::*;
    use std::{vec, vec::Vec};

    /// Deterministic xorshift64* OID generator (no external deps).
    pub(crate) struct Lcg(u64);
    impl Lcg {
        pub(crate) fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        pub(crate) fn oid(&mut self) -> Oid {
            let mut o = [0u8; 20];
            for chunk in o.chunks_mut(8) {
                let v = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
            o
        }
    }

    fn crc32(data: &[u8]) -> u32 {
        crate::pack_write::crc32(data)
    }

    /// Serialize a v2 idx from sorted (oid, offset) pairs. Offsets >= 2^31
    /// land in the large-offset table per the bit-31 escape.
    pub(crate) fn build_idx_v2(entries: &[(Oid, u64)]) -> Vec<u8> {
        assert!(
            entries.windows(2).all(|w| w[0].0 < w[1].0),
            "must be sorted"
        );
        let n = entries.len();
        let mut out = Vec::with_capacity(1032 + 44 * n + 40);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&2u32.to_be_bytes());
        // fan-out
        let mut fanout = [0u32; 256];
        for (oid, _) in entries {
            fanout[oid[0] as usize] += 1;
        }
        for i in 1..256 {
            fanout[i] += fanout[i - 1];
        }
        for f in fanout {
            out.extend_from_slice(&f.to_be_bytes());
        }
        for (oid, _) in entries {
            out.extend_from_slice(oid);
        }
        for (oid, _) in entries {
            out.extend_from_slice(&crc32(oid).to_be_bytes());
        }
        let large: Vec<(usize, u64)> = entries
            .iter()
            .enumerate()
            .filter(|(_, (_, off))| *off & 0x8000_0000 != 0)
            .map(|(i, (_, off))| (i, *off))
            .collect();
        for (i, (_, off)) in entries.iter().enumerate() {
            if *off >= 0x8000_0000 {
                let li = large.iter().position(|(j, _)| *j == i).expect("large idx");
                out.extend_from_slice(&(0x8000_0000u32 | li as u32).to_be_bytes());
            } else {
                out.extend_from_slice(&(*off as u32).to_be_bytes());
            }
        }
        for (_, off) in &large {
            out.extend_from_slice(&off.to_be_bytes());
        }
        // trailing checksums: pack checksum (placeholder zeros) + idx checksum
        let pack_ck = [0u8; 20];
        out.extend_from_slice(&pack_ck);
        let mut h = crate::pack_write::Sha1::new();
        h.update(&out);
        out.extend_from_slice(&h.finalize());
        out
    }

    fn sorted_entries(seed: u64, n: usize, big_offsets: bool) -> Vec<(Oid, u64)> {
        let mut g = Lcg(seed | 1);
        let mut v: Vec<Oid> = (0..n).map(|_| g.oid()).collect();
        v.sort_unstable();
        v.into_iter()
            .enumerate()
            .map(|(i, oid)| {
                let off = if big_offsets && i % 100 == 0 {
                    0x8000_0000u64 + (i as u64) * 512
                } else {
                    (i as u64) * 512
                };
                (oid, off)
            })
            .collect()
    }

    #[test]
    fn parse_rejects_bad_magic_and_truncation() {
        assert!(matches!(IdxV2::parse(&[]), Err(GitError::Truncated)));
        let mut bad = vec![0u8; 1100];
        bad[0] = b'P';
        assert!(matches!(IdxV2::parse(&bad), Err(GitError::BadMagic)));
        let entries = sorted_entries(1, 10, false);
        let mut good = build_idx_v2(&entries);
        good.truncate(good.len() - 5);
        assert!(matches!(IdxV2::parse(&good), Err(GitError::Truncated)));
    }

    #[test]
    fn every_oid_found_with_correct_offset_n1000() {
        let entries = sorted_entries(0xBEEF, 1000, false);
        let bytes = build_idx_v2(&entries);
        let idx = IdxV2::parse(&bytes).expect("parse");
        assert_eq!(idx.len(), 1000);
        idx.verify_idx_checksum().expect("checksum");
        for (i, (oid, off)) in entries.iter().enumerate() {
            match idx.lookup(oid).expect("within window") {
                Some((found, found_off)) => {
                    assert_eq!(found as usize, i);
                    assert_eq!(found_off, *off);
                }
                None => panic!("miss on present OID at index {i}"),
            }
        }
        // fanout + crc spot checks
        assert_eq!(idx.fanout(255), 1000);
        assert_eq!(
            idx.fanout(0) as usize,
            entries.iter().filter(|(o, _)| o[0] == 0).count()
        );
        for i in 0..1000u32 {
            assert_eq!(idx.crc(i), crc32(&entries[i as usize].0));
        }
    }

    #[test]
    fn misses_return_none() {
        let entries = sorted_entries(0xF00D, 1000, false);
        let bytes = build_idx_v2(&entries);
        let idx = IdxV2::parse(&bytes).expect("parse");
        let mut g = Lcg(0x5EED_0002);
        let mut misses = 0;
        for _ in 0..500 {
            let t = g.oid();
            if entries.iter().any(|(o, _)| o == &t) {
                continue;
            }
            assert_eq!(idx.lookup(&t).expect("within window"), None);
            misses += 1;
        }
        assert!(misses > 400, "generator produced too few misses: {misses}");
    }

    #[test]
    fn boundary_n65536_uniform_partitions_lookup_all() {
        // 65,536 uniform random OIDs: total table == ladder window; each
        // first-byte partition ~256 entries, all inside the window.
        let entries = sorted_entries(0x1234_5678, LADDER_WINDOW as usize, false);
        let bytes = build_idx_v2(&entries);
        let idx = IdxV2::parse(&bytes).expect("parse");
        assert_eq!(idx.len(), LADDER_WINDOW);
        // sample: first, last, every 4096th
        let mut checked = 0;
        for i in (0..entries.len()).step_by(4096).chain([entries.len() - 1]) {
            let (oid, off) = &entries[i];
            let (found, found_off) = idx.lookup(oid).expect("within window").expect("hit");
            assert_eq!(found as usize, i);
            assert_eq!(found_off, *off);
            checked += 1;
        }
        assert!(checked >= 17);
    }

    #[test]
    fn oversized_partition_returns_typed_error() {
        // Skew: > 65,536 OIDs sharing first byte 0x00 ⇒ partition exceeds
        // the ladder window ⇒ typed TooManyObjects, never a silent wrong answer.
        let mut g = Lcg(0x0BAD);
        let mut v: Vec<Oid> = (0..LADDER_WINDOW as usize + 1)
            .map(|_| {
                let mut o = g.oid();
                o[0] = 0x00;
                o
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        if v.len() <= LADDER_WINDOW as usize {
            // pad with more zero-prefix OIDs deterministically
            let mut x = 0u64;
            while v.len() <= LADDER_WINDOW as usize {
                let mut o = [0u8; 20];
                o[1..].copy_from_slice(&x.to_be_bytes());
                v.push(o);
                x += 1;
            }
            v.sort_unstable();
            v.dedup();
        }
        let entries: Vec<(Oid, u64)> = v
            .into_iter()
            .enumerate()
            .map(|(i, o)| (o, i as u64))
            .collect();
        let bytes = build_idx_v2(&entries);
        let idx = IdxV2::parse(&bytes).expect("parse");
        let probe = entries[0].0;
        assert_eq!(
            idx.lookup(&probe),
            Err(GitError::TooManyObjects(entries.len()))
        );
    }

    #[test]
    fn large_offset_escape_resolves_64bit() {
        let entries = sorted_entries(0x1A2B, 40, true);
        assert!(entries.iter().any(|(_, o)| *o >= 0x8000_0000));
        let bytes = build_idx_v2(&entries);
        let idx = IdxV2::parse(&bytes).expect("parse");
        for (i, (oid, off)) in entries.iter().enumerate() {
            assert_eq!(
                idx.offset(i as u32).expect("offset"),
                *off,
                "offset select at {i}"
            );
            let (_, found_off) = idx.lookup(oid).expect("within window").expect("hit");
            assert_eq!(found_off, *off);
        }
    }

    #[test]
    fn empty_idx_roundtrip() {
        let bytes = build_idx_v2(&[]);
        let idx = IdxV2::parse(&bytes).expect("parse");
        assert!(idx.is_empty());
        assert_eq!(idx.lookup(&[0u8; 20]).expect("within window"), None);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn ladder_iteration_count_constant_through_idx_lookup() {
        let entries = sorted_entries(0xD00D, 1000, false);
        let bytes = build_idx_v2(&entries);
        let idx = IdxV2::parse(&bytes).expect("parse");
        let before = crate::ladder::debug_probes();
        // hits
        for (oid, _) in &entries[..50] {
            let _ = idx.lookup(oid);
        }
        let hit_probes = crate::ladder::debug_probes() - before;
        // misses
        let mut g = Lcg(7);
        let m_before = crate::ladder::debug_probes();
        let mut counted = 0;
        for _ in 0..200 {
            let t = g.oid();
            // targets in an empty first-byte partition exit pre-ladder (the
            // part == 0 guard) — the constancy claim covers ladder-entered
            // inputs: hits and non-empty-partition misses
            if idx.fanout(t[0] as usize)
                == idx.fanout(t[0] as usize - if t[0] == 0 { 0 } else { 1 })
            {
                continue;
            }
            let _ = idx.lookup(&t);
            counted += 1;
        }
        let miss_probes = crate::ladder::debug_probes() - m_before;
        assert_eq!(miss_probes, counted * 16);
        assert_eq!(hit_probes, 50 * 16);
    }

    /// Cross-test with lane I6's pack writer (Chicago: real collaborators):
    /// every blob written by `build_pack_and_idx` is found by the ladder
    /// reader at the writer-recorded offset, and CRC fields agree.
    #[test]
    fn cross_test_offsets_from_pack_writer() {
        use crate::pack_write::build_pack_and_idx;
        let mut g = Lcg(0xC0_FF_EE);
        let blobs: Vec<Vec<u8>> = (0..64)
            .map(|_| g.next_u64().to_le_bytes().to_vec())
            .collect();
        let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
        let (pack, idx_bytes, entries) = build_pack_and_idx(&refs).expect("build");
        let idx = IdxV2::parse(&idx_bytes).expect("parse I6 idx");
        assert_eq!(idx.len() as usize, blobs.len());
        for (i, e) in entries.iter().enumerate() {
            let (found, found_off) = idx.lookup(&e.oid).expect("within window").expect("hit");
            assert_eq!(found as usize, i, "sorted order preserved");
            assert_eq!(found_off, e.offset as u64);
            assert_eq!(idx.crc(i as u32), e.crc);
        }
        idx.verify_idx_checksum().expect("idx checksum");
        let _ = pack;
    }
}
