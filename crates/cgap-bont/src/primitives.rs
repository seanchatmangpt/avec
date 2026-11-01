//! A06 — B-ONT v1 scalar primitives: the Wave 1 ABI freeze + Wave 3 derivation
//! and collision-obligation machinery.
//!
//! Spec: `docs/jira/v26.11.1/lanes/A06-bont-scalars.md` §2 (inventory), §4
//! (truncation), §5 (collision obligation), §7 (static asserts), §11 (freeze
//! list). Wave 1 locked the struct/const ABI; this slice lands the §4/§7
//! derivation (`slot_id`/`lex_hash`/`truncate64`), the §5 producer-obligation
//! fixture (`CollisionDetector`), and the §8 courts T1/T4/T5 as inline tests.
//! The `.oabi` container + loader (§6) and courts T2/T3 live in `oabi.rs`.
//!
//! Every ontology term is a fixed-width integer derived once at build time
//! from its lexical form via BLAKE3 (in-repo `cgap-blake3`); the runtime
//! never sees strings on the hot path (§1). Widths here are frozen: any
//! widening after the freeze is a breaking ABI change requiring a new lane,
//! not a patch (§11).
//!
//! Wave 8 defect repair (per `waves/a08-wave7-audit-receipt.md`): zero
//! defects in this file's surfaces — baseline 30/30 exit 0 re-run; all 16
//! named mutation classes re-flipped fresh in the wave-8 sandbox
//! (/tmp/a06-w8.k8yZKp, sha256 fail-closed restores), replacing the
//! evaporated wave-5 sandbox citations; external b3sum 1.8.5 anchor
//! re-verified (`printf 'bont\x01a' | b3sum` =
//! `ffe66d54817825f3...` = GOLDEN[2]). D-A08-5 (52 MB nested build root
//! `crates/cgap-bont/target-a06/`) surfaced to the coordinator for
//! integration deletion; this lane's own runs use the repo-root lease
//! `target-a06/`.
//!
//! Wave 9 (failed-lane restart, 2026-11-01): no new primitive — the wave is
//! cross-lane integration, and A06's consumer-side integration courts live
//! in `oabi.rs` (producer-pack ingestion: the A09/A10 committed packs
//! through the real §6 loader + the §4 truncation window on producer
//! bytes). This file's Wave-9 obligation is the re-executed anti-vacuity
//! matrix: M1 (truncation-window shift), M2 (namespace tag drop), M5/M6
//! (detector collision/dedup branches), M-L/M-R (loader length law /
//! record_size check) and M-T2 (`ClassId` u32->u64 widening, build-refused
//! by the ABI-freeze asserts) re-flipped fresh in the wave-9 sandbox
//! /tmp/a06-w9-sb with sha256 fail-closed restores. The fleet restart runs
//! (A04 identity court, A14 dam over A04 frames, A20 1M-row stress, A05
//! power-cut sweep, A07 zero-string gate over the built crates) are
//! witnessed in the wave-9 restart report. ABI unchanged: `SlotId` u64 /
//! `ClassId` u32 / `EnumId` u16 / `LexHash` 32 B, window `d[0..8]` LE, tag
//! `b"bont\x01"` (§11 freeze).

// alloc-only surface: when `lib.rs` flips to #![no_std] (coordinator-owned,
// DoD §9.1), this file needs no edits beyond the flip itself.
extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt;

/// Term slot: the sparse, collision-vetted identity,
/// `truncate64(BLAKE3(lexeme))` (§2, §4).
///
/// Frozen width 8 bytes, alignment 8. Truncation window frozen at the FIRST
/// 8 bytes of the BLAKE3 root digest, little-endian:
/// `SlotId = u64::from_le_bytes(LexHash[0..8])` (§4). Shifting the window
/// silently remaps every `SlotId` in every pack (§4).
#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SlotId(pub u64);

/// Dense class index assigned by the build-time producer (§2).
///
/// Frozen width 4 bytes; small enough for direct-indexed class tables.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ClassId(pub u32);

/// Dense enumeration-variant index (§2).
///
/// Frozen width 2 bytes; cheap enough to embed in `.oabi` records.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EnumId(pub u16);

/// Full 256-bit BLAKE3 root digest of the namespace-tagged lexeme (§2).
///
/// The truncation *preimage witness*: `SlotId = low64(LexHash)`. Carrying
/// both is what makes the determinism court non-vacuous (§2). Frozen width
/// 32 bytes, alignment 1.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LexHash(pub [u8; 32]);

/// Namespace domain-separation tag hashed ahead of every lexeme (§5).
///
/// The `\x01` tag byte is reserved for namespace v1 and is part of the
/// freeze: a future namespace bump changes the tag rather than silently
/// sharing the v1 ID space.
pub const BONT_NS_V1: &[u8] = b"bont\x01";

/// Frozen truncation window: prefix length (bytes) of the BLAKE3 root digest
/// taken little-endian as the `SlotId` (§4).
pub const SLOT_TRUNC_WINDOW_BYTES: usize = 8;

// ---- ABI freeze: any widening here must fail the build (A06 §7) ----
const _: () = assert!(
    core::mem::size_of::<SlotId>() == 8,
    "ABI freeze (A06 §7): SlotId must stay 8 bytes (u64)",
);
const _: () = assert!(
    core::mem::size_of::<ClassId>() == 4,
    "ABI freeze (A06 §7): ClassId must stay 4 bytes (u32)",
);
const _: () = assert!(
    core::mem::size_of::<EnumId>() == 2,
    "ABI freeze (A06 §7): EnumId must stay 2 bytes (u16)",
);
const _: () = assert!(
    core::mem::size_of::<LexHash>() == 32,
    "ABI freeze (A06 §7): LexHash must stay 32 bytes ([u8; 32])",
);
const _: () = assert!(
    core::mem::align_of::<SlotId>() == 8,
    "ABI freeze (A06 §7): SlotId must stay 8-byte aligned",
);
const _: () = assert!(
    core::mem::align_of::<ClassId>() == 4,
    "ABI freeze (A06 §7): ClassId must stay 4-byte aligned",
);
const _: () = assert!(
    core::mem::align_of::<EnumId>() == 2,
    "ABI freeze (A06 §7): EnumId must stay 2-byte aligned",
);
const _: () = assert!(
    core::mem::align_of::<LexHash>() == 1,
    "ABI freeze (A06 §7): LexHash must stay 1-byte aligned",
);

// ---- §4 truncation + §7 derivation ------------------------------------------

/// Truncation per §4: the FIRST `SLOT_TRUNC_WINDOW_BYTES` bytes of the digest
/// read little-endian. The single place the frozen window is applied; courts
/// T1 (golden table) and T3 (per-record re-derivation) route through it.
#[inline]
pub fn truncate64(hash: &LexHash) -> SlotId {
    SlotId(u64::from_le_bytes(
        hash.0[0..SLOT_TRUNC_WINDOW_BYTES].try_into().expect(
            "LexHash is exactly 32 bytes; the 0..8 window is always in range",
        ),
    ))
}

/// Full BLAKE3 root digest of the namespace-tagged lexeme (§2, §5 tag).
#[inline]
pub fn lex_hash(lexeme: &[u8]) -> LexHash {
    LexHash(cgap_blake3::hash(&tag(lexeme)))
}

/// `slot_id(lexeme)` per §4/§7: `truncate64(BLAKE3(BONT_NS_V1 ++ lexeme))`.
///
/// Pure function of the lexeme bytes — the replay property (§1): any
/// producer regenerates identical IDs from the same lexeme corpus with no
/// registry service and no runtime interning (§3.2).
#[inline]
pub fn slot_id(lexeme: &[u8]) -> SlotId {
    truncate64(&lex_hash(lexeme))
}

/// Namespace-tagged lexeme bytes: `BONT_NS_V1 ++ lexeme` (§5 tag).
#[inline]
fn tag(lexeme: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    buf.extend_from_slice(BONT_NS_V1);
    buf.extend_from_slice(lexeme);
    buf
}

// ---- §5 producer collision obligation ---------------------------------------

/// Hard build error for a SlotId collision: two DISTINCT lexemes whose
/// truncated 64-bit prefixes agree (§5: "any two distinct lexemes yielding
/// equal `SlotId` is a hard build error naming both lexemes"). Names both
/// lexemes; fail-closed — the producer aborts the build, never silently
/// accepting truncation drift. Re-inserting the IDENTICAL lexeme is not a
/// collision (one lexeme has exactly one slot by determinism, §1) and is
/// accepted idempotently — only distinct lexemes fire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollisionError {
    /// The colliding slot prefix.
    pub slot: SlotId,
    /// First-inserted lexeme (raw bytes; lexemes may be non-UTF8).
    pub first: Vec<u8>,
    /// Second-inserted lexeme colliding with `first`.
    pub second: Vec<u8>,
}

impl fmt::Display for CollisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hard build error (A06 §5): SlotId collision 0x{:016x}: lexemes {:?} and {:?} share a truncated prefix",
            self.slot.0,
            LexemeBytes(&self.first),
            LexemeBytes(&self.second),
        )
    }
}

/// Debug rendering for raw lexeme bytes (lexemes may be non-UTF8): plain hex,
/// no string assumption.
pub struct LexemeBytes<'a>(pub &'a [u8]);

impl fmt::Debug for LexemeBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("0x")?;
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

/// Build-time collision detector over the real `slot_id` path (§5).
///
/// Producers feed every lexeme; the FIRST occurrence claims the slot. A
/// LATER DISTINCT lexeme truncating to an occupied prefix is a hard
/// `CollisionError` naming both lexemes; re-feeding the identical lexeme is
/// idempotent `Ok(())` (§5 fires on distinct lexemes only — a duplicate is
/// the same term, not truncation drift). Exhaustive over the actual corpus
/// (§10): once this check passes, residual collision risk in that corpus
/// is zero.
pub struct CollisionDetector {
    /// slot prefix -> first lexeme that claimed it.
    slots: BTreeMap<u64, Vec<u8>>,
}

impl CollisionDetector {
    /// Empty detector.
    pub fn new() -> Self {
        Self { slots: BTreeMap::new() }
    }

    /// Register a lexeme, hashed through the real `slot_id` path.
    ///
    /// Idempotent on the identical lexeme (§5 distinct-lexeme law): the
    /// duplicate `Ok(())` leaves the detector unchanged (same slot, same
    /// first claimant). Two DISTINCT lexemes with an equal truncated
    /// prefix return `Err(CollisionError)` naming both.
    pub fn insert(&mut self, lexeme: &[u8]) -> Result<(), CollisionError> {
        let slot = slot_id(lexeme);
        if let Some(first) = self.slots.get(&slot.0) {
            if first.as_slice() == lexeme {
                return Ok(());
            }
            return Err(CollisionError {
                slot,
                first: first.clone(),
                second: lexeme.to_vec(),
            });
        }
        self.slots.insert(slot.0, lexeme.to_vec());
        Ok(())
    }

    /// Number of distinct slots registered.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// True when no lexeme has been registered.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

impl Default for CollisionDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---- §8 courts (Chicago: real corpora, real buffers, no mocks) --------------

/// Test corpus builder — committed alongside the T1 golden table. Byte-stable
/// by construction: index 0 empty, 1 non-UTF8 byte, 2 one-byte ASCII, 3 a
/// >1 MiB deterministic pattern (§8 T1 corpus requirements).
#[cfg(test)]
fn corpus_lexeme(i: usize) -> Vec<u8> {
    match i {
        0 => Vec::new(),
        1 => vec![0xff],
        2 => vec![b'a'],
        3 => {
            // >1 MiB deterministic pattern lexeme, 1_048_577 bytes.
            let n = 1024 * 1024 + 1;
            (0..n).map(|j| (j % 251) as u8).collect()
        }
        i => format!("bont://x/{i}").into_bytes(),
    }
}

/// The §4/§7 spec applied BY HAND in the test (not via `slot_id`): the T4
/// independent re-derivation path. Catches spec-vs-impl drift (§8 T4).
#[cfg(test)]
fn spec_slot_id(lexeme: &[u8]) -> SlotId {
    let mut buf = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    buf.extend_from_slice(BONT_NS_V1);
    buf.extend_from_slice(lexeme);
    let d: [u8; 32] = cgap_blake3::hash(&buf);
    SlotId(u64::from_le_bytes(d[0..8].try_into().unwrap()))
}

/// Committed golden `SlotId` table: 1000 values, one per `corpus_lexeme(i)`
/// for i in 0..1000, generated once via the T4 hand path from `cgap-blake3`
/// and cross-anchored against external `b3sum` 1.8.5 (golden[2], lexeme `a`:
/// digest prefix ffe66d54817825f3 -> 0xf3257881546de6ff).
#[cfg(test)]
const GOLDEN: [u64; 1000] = [
    0x3b9abbd248f79e66, 0xe0414e7fadbe46dd,
    0xf3257881546de6ff, 0x2673801ba6f8a6be,
    0x871c8b94e58ab388, 0xe90b01a322acb7c1,
    0x75287a4ae5b5e745, 0x67d8a2e45ea9f5a0,
    0x211641dbb22df5c4, 0xb713ff017bd637c6,
    0x288c6eac9fafe997, 0x8e06e202dac52250,
    0x504517361018d234, 0x4c6c0aba659e7acf,
    0x5656ec82b8afe509, 0xd18290077a40a5ef,
    0x9f6e680423447868, 0x786d8ff0044c4042,
    0x418ca8b550ba9b16, 0x5c481883160163ea,
    0x46cab34e7eef9e15, 0x8ebc871405c18391,
    0x2b3fcff7b8f38024, 0x82963d4d5bab9bba,
    0x4f8a8e680c0cd10d, 0x070d6c3d593352ee,
    0xdf0d7174da1945ec, 0xd19fa57fdd4d15ed,
    0x7b098dc637583b34, 0x9b47de6133bfeb54,
    0xcb91ac145fa54c41, 0x02545dab9c886760,
    0x165e6be6c5db91ca, 0x4757327ad6981f90,
    0x329c8c795589b49e, 0x9124f3e16cf0bc83,
    0xa3054744c046e120, 0x45d9322981a059e8,
    0xb3c1caeb9a18c402, 0xabf04e82ee04a21c,
    0x2027336e30cbc387, 0x723e2a68aa3332ff,
    0x83ccc578904fa56c, 0xa4869bf38ce4f840,
    0xfdac25c0ca96af91, 0x5c7138e327b54f45,
    0xc31a1d5038dd6968, 0x2535c0e4b9ceb777,
    0x851c75705318b0e1, 0x965a32b6d4f4e41c,
    0x0316e703508ab03b, 0x285c59e9d24d481a,
    0x94b74c27de1f2886, 0x631542d78004cbba,
    0x22458a0a95429901, 0x2c795250df307884,
    0x81ea7c7302ae8d18, 0xbb1adc918f792422,
    0xb0e1402b204dc7f4, 0x31dd08243dfe756a,
    0xbca615d247a14648, 0x35bf60d58718fea0,
    0x593f8907db2b0543, 0xd45e94c153ef3ae9,
    0xf90a1f50974cfa1c, 0xe3a43c2aa21a3d0e,
    0x3c6670e84dd02a99, 0x83ec7ed1bbc1175f,
    0x27f440ead5c0a6b5, 0x0b714abfe4ae8499,
    0x8c994f8b7c8987cf, 0xbc572934b32bc8b2,
    0x6547f93d1f280480, 0x4940839bf05fb75b,
    0x4d603506b821c1b0, 0xeb47d9499cc93692,
    0x2e4bb17f0dbd33dc, 0x4769a47d4aac434e,
    0xed9a6e77489fa3e9, 0x33810744ed4db013,
    0x0eca4b1cb7d6a662, 0xd108d5bd8b61de84,
    0x365362e45a19e842, 0xa202e8480d96b5c0,
    0x365534eeac4ff890, 0x9f6dc4c0f8fa5188,
    0xa02c4eb2c4351249, 0xeb80d9884122d519,
    0xec4ad8eebeba434a, 0x4caacd2cc3e131a3,
    0x52eafa77249b88eb, 0x6e2e84d9e31ac2dd,
    0x2bfeb78759a004d2, 0x7f4d8e936e573ffa,
    0x50b0ff224a52f87b, 0x8b2d8a93cda22d19,
    0x63a2224cf9ab10ee, 0x113f86b5828877df,
    0xff87f297eae8a2ad, 0xd0a4c6e1187f5e60,
    0x169d665575ae8f01, 0x9491b074759ba980,
    0x0ab57cb94fe25c7a, 0xa9b4a2178ac2a9f3,
    0x7aa898f744653846, 0xf00be48e748d1a94,
    0x78758a9053bd0a07, 0xad480a6d012bfc05,
    0x47f01e41342f4209, 0xd89b9aaded5b27fc,
    0xe93515366a83dae8, 0x24ebec85d1194387,
    0x454605ed6cbcf7be, 0x23abe55d21b9c19a,
    0x81d22be65915544f, 0x4e5d0eeaab01ca96,
    0xa79cb658bad800a9, 0x1cc81b6669c7a570,
    0xd502ffcd5745232d, 0x06409097c2167a9f,
    0x7c80d08911237aa4, 0x0625cfe6a957c3fd,
    0x0fcee1e174266e44, 0xe9758cc67485c822,
    0xe08fff1e0d783549, 0xeea01382ab610cb6,
    0x1e2d42f549a62b69, 0x725c108ad5327ed7,
    0x2942bb653c4fc425, 0x5e1ddfdd18f9c5e0,
    0x140509bd667e250e, 0x6f90e06667a553c4,
    0x14c60506a4bb0210, 0x68fa47e468a4d272,
    0xf11505cbb343300d, 0x16cae471ccf323ad,
    0xc453926f9b45ce6c, 0x8b8e1cea7d457cea,
    0x3a2ceedbef9c7ce6, 0x157c85b3b6ae4edb,
    0x438c4daa05f6bd3b, 0xdb884decc5faadf7,
    0x20e69a7415b88a71, 0xa64a1086c8bd01db,
    0x6d052e385a91718f, 0xdc6c8b95570492de,
    0x07440d9d002df8b2, 0xef02d456cded3218,
    0xf0bf63aaf60d587d, 0x6459df146ea90ca8,
    0x8e7f7ca4f59a0573, 0x746515b785a709a7,
    0xa8fd04ef5bd0e6b7, 0x7ef4c7106c28d5fd,
    0x342b22280b6f4c36, 0x9e59389c75addc53,
    0xd466732b74b73cc2, 0x41b4f58fce4046a5,
    0x263ffe12a0695ab1, 0xb9529c2249705195,
    0x25bf5f959bb21e64, 0x78bc19f4622d669e,
    0x54c7b643f4788343, 0xd0b9ad62d884c919,
    0x2b6fe2d618c427fb, 0x052e723e2ff204f3,
    0x2c95b0e00a9fc4b8, 0x481fce249906da98,
    0xd40564c9c98b6cdf, 0x4ae2092c96ef3901,
    0xb41839bf0921811e, 0x01481295f5096413,
    0xb59967367c9fa4fe, 0x39abacf1f93aab4f,
    0x93e77952110346ae, 0x3a09b3f3181b6261,
    0x9468a73e71d5414b, 0x5632758a07d72c43,
    0xc043444ca132fe84, 0x634932147cc2d883,
    0xf39690efe41a905e, 0x4621b047f52c8900,
    0x3d08f351f876a1e9, 0x805def2c8efa6a27,
    0x37dff627a70fd8be, 0x77f5af3dcfc0ca54,
    0xfae10a7383efdc0b, 0x8d5379a9654dcf96,
    0x60f83a082ea596f6, 0x8daa7fdb60f2b195,
    0xc181033ea2f8b22c, 0x3e7a569ae6c29a35,
    0x2675f436647afc4f, 0x3ddf22543bbf1427,
    0xfb7914c022aadf9f, 0xd98d8eeb7b41e466,
    0xfd57dd2f1d1b79be, 0xb6a46d676c28dea1,
    0x09fcbd7dda3f2834, 0x7ba2c2f16c608a62,
    0xbaffafc74b317827, 0xdebfb13671bf2d66,
    0x923baaf6db51fec1, 0xb2613a71700a02a4,
    0x40f9ee1d32655a3f, 0x92f4bffa67905e7a,
    0x167076cfd05e9b82, 0xc7e080dcf1bf5bf3,
    0x6156f8e490147d92, 0x62a6d0150f67f0a0,
    0xdfdefb1bf380fc27, 0xadec74b0a782caa2,
    0x1bbef83f6486cd18, 0xfbb27b59bf4773ea,
    0x273c35e919d62f83, 0x316d7dfc28f09b61,
    0x71ed5f0543b7d795, 0x69378c4399566586,
    0x9e5256de4c1cefff, 0x00efbad2b2b90218,
    0x8ccf3f4105b86ba7, 0xd145750b02019748,
    0x4d97ad9d774bc4cd, 0x32305539305fad2c,
    0xf7e4a04d1368f4f8, 0x78480894871ec3c5,
    0xf49c8b2a7523a517, 0xb3916dfe9fec49f1,
    0xfbc716ceff05d808, 0x0f115cd6e150126f,
    0xdc1604746fbf3a0f, 0x42a9e0c83362bc7c,
    0x7eaf1fe8df4da448, 0xeab34fdd41b0ebec,
    0x2d388b422677a7e5, 0x48bdffe680c313c5,
    0x7af4e7e5f19de26f, 0x28cd7466cbec6bc8,
    0x6255c2cf5a8a882b, 0xaf975b71b58157b0,
    0x3142d58e4787ae91, 0x4dce213ee828c9a3,
    0x376b593ee4c1a44e, 0x567641dd9b92e465,
    0xf86e4aee9bf625bb, 0xfa8679ea60e2abdb,
    0x65690008d7407d02, 0x3e06b3eb8592c8ec,
    0x4cdbb3e1216e8402, 0x21c0b6ab157cc7b7,
    0xb05404b404260e90, 0x953d757adbd5a412,
    0x13f17338767f0430, 0x10e8e26ebb79e98f,
    0x8dd7eaabca991a38, 0x07aeb9f710b3596e,
    0x8c848e9ca28e4f8d, 0x3d358713999484eb,
    0xed1f0d1b6e3c44b6, 0x45725749bdcea14a,
    0xfe50e4b05cd4fd82, 0xba52a7aa6ad69e85,
    0xe242a1a59421a2aa, 0xbcb9a3b0a2da47bf,
    0xe0189195bf2088bd, 0x2bb37536130c2973,
    0x8856e6135be02a2f, 0x5cf3fe74b8ba7109,
    0x1128d69124082fbd, 0x54d98b7243904afb,
    0xe2d1240d82e00f43, 0x9f61e65296225edd,
    0x3d79a10cb5f9a834, 0x0cf225648cc1334c,
    0xa1ead6ac217d8fe6, 0x3eebf3d1c37747a7,
    0x0a36fa2a7dab723e, 0x5f955b8c0403f3ae,
    0x7aa2bc84a02dc352, 0x297efb5ad54fc999,
    0x2b9e403cc85f49bc, 0x46a07fc148553066,
    0x41221240cbda8918, 0x2f443aa21e669d83,
    0x86c903bf493267cd, 0x1e5f34cd36841389,
    0x36002980f1f29b4c, 0xcb578cdae9333c47,
    0xcfc3621e54aa4a9d, 0xcc6bb31e26d4c509,
    0x106aa8050ab91dee, 0xbe95dca5fec62715,
    0xb4c470d29ded5344, 0xf0c8b41641c90f1f,
    0x1acd397960ab802d, 0x8dd7b057096ebe84,
    0xcf90143d123aff9c, 0x94068647992fce0d,
    0x7c01bff057959d5d, 0xb20151ab52ef4f9c,
    0x899595a740d15b82, 0x21bf725e609abdfc,
    0xfddded3e5726f143, 0x2a46ffbe6263eab4,
    0x90b93e7f0c2b27d5, 0x6ebdc676af467170,
    0x382f828de04a4a17, 0x0e94fa41689fabb4,
    0x4ba12b3318dd3d13, 0xe1ba637769b80249,
    0x5452f6bdc88e2a00, 0x29987dd0155ddbcf,
    0x1ff505e0ee57bd62, 0xa327e65dd15a6011,
    0x1551edc16a49cc9f, 0xed69793cb7a717cf,
    0x56a1be2f5cb8f59d, 0x5aa0e524e2fd4979,
    0x1146e810bdd04239, 0x5aad20d51e5fc13a,
    0x47e4949268cc7b0d, 0x90fe1c5c2f0f0532,
    0xe14b8e817adb06e9, 0x22921faaaae59169,
    0x5642146714183096, 0x99062c52d0b4d443,
    0xd21064488af03773, 0x84302aec12b6b70a,
    0x44c8295c33c15379, 0x89134a87a1feb0b6,
    0x344a872a912b786e, 0x4614944e596a9ed0,
    0xecf8a5f426783aac, 0x17f6acd27fe2be0c,
    0xc215a31c5e209ce8, 0x3072fb6206c32064,
    0x737f7ede3c948aaa, 0x255ff93415bf9383,
    0xa7ac134ab3019934, 0xe40fc0dde2a2cbbe,
    0xdf43fc0c6ca2c026, 0x916135b3ad9117d0,
    0x329ac5c106bbc1c9, 0x72d20380a85425df,
    0x403af2dcce4cf39e, 0x2c9c347e8036a083,
    0x004acb2437f64d24, 0x79a2a3f950f894b1,
    0x88478ff2f0dd7d7d, 0x45ffc757aef09820,
    0x3dd68c5ec095b62c, 0xce9d87bb2ac59118,
    0xd3e646283c8dd5d7, 0xdada7efad048e9ff,
    0xd4a832a46687a2d5, 0x97904432409e320d,
    0x911f39fa28945d21, 0x171acabbe83eceed,
    0x7515ae22ba67c7a5, 0x7c1399a1670e16ca,
    0x63bc22bf2f9cc5a6, 0x36262395fd03d602,
    0xafdbbeba427b5351, 0x1e65c796cc3013fc,
    0x0c6116099d00fca0, 0xdf1df969317992f5,
    0x0a7c2cc0b66c15c2, 0x8dd93301e6fdfb71,
    0xc1e34255c05defb5, 0x00c0e089c71456f7,
    0xe59a30e1b18ec389, 0x773acfaa4d73a7a8,
    0xa7d23c5e4a11e50c, 0x785c6ca5955a7a86,
    0x9a7436caed339154, 0xa0dbf0746544d7a1,
    0x8b3627d953eeb037, 0xb125ceb87e1c6258,
    0x7958cbec750b2753, 0x56b7768e3c1443b5,
    0x9fb543371c783db1, 0x4dc0e44049309ac7,
    0x3900eb430b2cb8e6, 0x21b8afc61ce3fc3f,
    0x88c202198e1e9ee1, 0xa3e7963bf1a80d39,
    0x64b5762182ffa3b2, 0x842951ea7da6b654,
    0xf41cd4aa759479b7, 0x9e54aabb8885a12c,
    0xbd1e245ebd65f7ba, 0xe12e90b8875f2632,
    0x4520206218727e96, 0xefea94099931d4a7,
    0x2890d595c5becba4, 0x7a266d8b8bf0b470,
    0x24f1d445d688649b, 0x54c9ade99389cd30,
    0x563b0b24508d794e, 0xfe8f057f24f2333e,
    0x9bf2686aa9194059, 0xdee9316f836cf2b0,
    0x75ac5490a1541507, 0x84a16f42195a26d1,
    0xb8444aed46772a8b, 0xbb124a865ca84dd2,
    0xafcda552217d80c2, 0xcdfc0b4c8323a7eb,
    0xe7316a1361d87715, 0xb10a7c8b631b5234,
    0x8ecfc6fb4da1a384, 0x1142ee11f7601ea8,
    0x7fca873c068d0c13, 0x97b0c899f6a4c59a,
    0x9c425d00ec99466a, 0x312404c802c550b5,
    0xddd0f2bdade38edf, 0x3e4be20f91e96cdd,
    0x8d6df7c6616ce772, 0x2101719d68358e54,
    0xc61dc70acbe5217c, 0xe71aa8efea348a07,
    0xa4ac4ff1f958c7a8, 0xbd09150c01697741,
    0x7b63be297af2a4e6, 0x46b2a3413d32302e,
    0x626ad90d68fb32df, 0xe52ab4d52651a0fa,
    0xe1ef4157f7e675c7, 0x2d964236f36f756b,
    0x8bd2f2ea40e53934, 0x0225c48943cedece,
    0x03e5ced8dae46a7f, 0xa2b681dca020786a,
    0x34a7f4705898365e, 0xdf0457b17afa22c3,
    0x22fa718caaa2f9af, 0xbd2ab942e557e564,
    0x19552fd578eeb278, 0x3a84209c4723dc0f,
    0x8cbb3a62d069cd67, 0x1f149a0bfbe3f984,
    0x3e0f267650e316b3, 0x3c7404a8fad9ae50,
    0xdcd5771e3ebf2185, 0xf0fe3d0c5dfe5873,
    0x8e4a6ae3a9b7e1b5, 0x2759b22afc2677c9,
    0xac45b6e3cb467495, 0xfee42b7a1ed3b9a2,
    0xef6f0f3c727537d3, 0xc37e24b8a080f251,
    0x996021797128cdc1, 0x3b9db17022fd99a8,
    0xe5f27ddae6f0ee34, 0x934141ebdd94483b,
    0x5859a4b93300c093, 0xedd6d1ec304cb3fb,
    0x08054fbd234de17e, 0x7a4223bacde39abf,
    0x2a064198af0f2d06, 0xbf9a233292b01109,
    0xc6f6b59223971284, 0x83cd3147274688c0,
    0x3b7f219e157caa7d, 0xd43e65d8631e95f7,
    0xa132f231fcde9dba, 0x6755c6d5c024d84b,
    0xd37e2647ee5a351b, 0x0a4fdfbcbcc369a8,
    0x2c0ce1af6276b44f, 0x7055c6f9b07a7e8b,
    0x7e20a6e1f324a9b2, 0x585bbd13a0ff3ad4,
    0x05156603690f6f40, 0xe32bcd6b96795e00,
    0x2823812b8de6dec6, 0xf663b93986ab1b1f,
    0x6473d6963c00cac5, 0x385348530705c076,
    0x3d12aff7bc38d5af, 0xea4f4545d0d5ac67,
    0x64d57985e86821ab, 0x627af628837e7595,
    0x3fc35ef6c51e8fb6, 0xf8d7e64ea79581a3,
    0xda73c6f4c1e981ec, 0x098eed24b90553eb,
    0x70fb90732660921f, 0x0f070d92dfc35fc2,
    0x47a22a5d635f1a5f, 0x9f49c02f953e20b7,
    0x63d533c17b42ea46, 0x9b7a88a4f357d590,
    0xd8f428d5c4a836fd, 0xaef1ab6d6e9b5380,
    0x808080b392df95c9, 0x60cf70b2ff74e4e9,
    0xf98469bce69e6b9f, 0xf295126f7a0f3b02,
    0xcf2105cc912c5182, 0xecf95bbb36af1d1c,
    0xdd2a07770c75b656, 0x5071a1901584ea64,
    0xb2f52fc104152f47, 0xcef01013103eff71,
    0x291cf071e981aed7, 0x0fe53fae0552da1d,
    0x2c204bb56e3356cc, 0x701dbb41a179cabf,
    0xf4189ac109cc80dc, 0x4efd123cb1049d8b,
    0x4ddee2a1c472593e, 0xd94aa89210bfbbca,
    0xcf1de297f9914906, 0x139b3db1443ef67b,
    0xdd576fd90bbf4451, 0xb35548d28fef290d,
    0x2f6d63f5dc3d7a35, 0xbd288164eb986793,
    0x5ac9f4702004f1ed, 0xb64e3b40fa2b4b15,
    0x0fa3e1e2f8fc89bf, 0x18b27c9385437c1d,
    0xddbd95131993df68, 0xa994d01263c614b7,
    0x9f1263bc2e85ee27, 0x4adccd36fc2391bd,
    0x090793aa1c825598, 0x24602eda6faab031,
    0x6c9bc24119991d85, 0x898e2f9c96c48d2d,
    0x025a9e453f5d623d, 0x9fd8e7ff1163c528,
    0x50d46d6851a847f8, 0xb7a87ed0e252ae3f,
    0xdfd13e4dd574a76e, 0xaba5c16daa38864b,
    0xc0b5ba73b3074cb2, 0x0c3097530c092855,
    0x872c4115740b3f2d, 0x7533da54fd21bd1a,
    0x58d78e0c20fb9796, 0xcb9f896856a795b0,
    0x11fdd72e4fa5526b, 0x96086497e9c94ba2,
    0xd37ef0a962bfc901, 0xa0f8859f4294f02f,
    0xe735b747b6c503f1, 0xcfa5f2122211cdfd,
    0xcad50f1662232c11, 0xfa57b78b8bbdbbfe,
    0xb1e6718bb6d18eac, 0x6db3bbd6fb75267b,
    0x8e2f7021c25e332e, 0x212d3e262aa759c5,
    0x4ca43158994dd049, 0xf4240dd43d970501,
    0x9ef6d38df1ca6552, 0x099304c43c7784e0,
    0x64b39fc01cf9c983, 0xc5b394daa8e7d09d,
    0x187040dff7100252, 0xb98fda85185be887,
    0x177cfb5fd19e6122, 0x28580f43daf843b9,
    0xec507703561d4687, 0x47bd0766b11ce2bf,
    0x37059267c11efd98, 0xc55e575daf373794,
    0x57f3b5a40f77c59e, 0xdaaa042e604c1c1b,
    0x6ce0ded45f7d1ce4, 0x7a92622d531bc1c5,
    0xb76364fb4b982bb6, 0xdc18dcb0fb6436f8,
    0xadc7a8fd3c5caf4f, 0xfd2296137a466a92,
    0x07ae0eb0422cb6b7, 0x633b28eea76ef6d1,
    0x337327b39aefb071, 0x92857a2fa9e09b63,
    0xd260820a29b2d272, 0x8d76f0784e46fc90,
    0x366a16fbc8df9ecb, 0x945faea26ead0f18,
    0x736ca4e396357b07, 0x93f9b68d0f8d0b8a,
    0x851973d17666ed23, 0x8d4da9bd4ebda35c,
    0x73e58c050a37c58c, 0x29636d847039501b,
    0xced5f7200ec0496c, 0x1b775a73c78ef074,
    0x048586a29997921b, 0x034ed5832b534b94,
    0xe2f3daa9bbcf879b, 0x96f7ca530ef7e278,
    0xbd5eaa00d643b462, 0xce3deeb7428437a5,
    0xca3acf7f0058c69f, 0x79d4a8df66b9a012,
    0x912b667433d68abc, 0x6b2087f65488797c,
    0xb291cba2c891d311, 0x66e688b32a904bf3,
    0x08483f3f31b3b339, 0x18c8de7828915842,
    0xe63f27f4087e3adf, 0x891d6b1566f22374,
    0xbad359cd59681964, 0x04a842684f0175bf,
    0x8b76c7af9fc730e8, 0x5c70d972e1a6f2a4,
    0x68420a08cc6998ae, 0xfc3a02b85e9e5c6b,
    0x1e481eea8c5c893c, 0x91e04c1ac0c4c60a,
    0xb4c0a789e28d5347, 0xff48192904f739e3,
    0x15605b108d421ae6, 0xa86a2f305020a4a1,
    0x6816b862c72604b7, 0x2b05b0a24c8227f8,
    0x5bb1cba6084f9d0c, 0xcb6c2b6ca7e08ebc,
    0xe32e7934c50735e6, 0x95a3e143f9ce9453,
    0x62f9f16f1d2dcffd, 0x63076bf3e63c9e14,
    0xc57dcddb71b06b37, 0xed71793810703572,
    0xf64782862f4052f8, 0x17a0910ffd5f0a73,
    0x2903ad26ff521758, 0x5cd5779c9e392840,
    0x72a6ee829442d7fd, 0x3707c6008fb8a489,
    0xc7e8a9c0e1b5b34e, 0x6fa22b3a1044c010,
    0x90be736ff16da77c, 0xde6d460701ca5c28,
    0x905f86eb7e0c4da4, 0xf739100f7d712ebc,
    0xb8701315393fed65, 0x395d23a6342037a6,
    0xa37fdaf39d00c4da, 0xbb2caee462426003,
    0x120fab285966f14b, 0xbd6fff8002e94b4f,
    0x202b53a0525d6764, 0x77aa24ee47e28612,
    0xa30fa9cf5f97da25, 0xaf647a7f43cab6d6,
    0xb96356046f00d38e, 0xd1d81abeb1a0b4c8,
    0x20f6019b27afe87e, 0x6737799f4c38f0dc,
    0x7263aba45a2f2e89, 0xda891a47b54ed7b1,
    0xd3409b9eaccfc4fe, 0x8981dc6acd3b10d9,
    0xff16845308e9961b, 0x2eed80dc743cfa3e,
    0x83a11ef30bbf9421, 0x8cb9dbe9ffd940eb,
    0x435580c380aab231, 0x24a7c3fa795b49f2,
    0xcd5f40e39d72ca14, 0x98335df32eb816f1,
    0x4991b2c8227647f5, 0x04dc4ad4647a49d2,
    0xad20936f50eacc5b, 0xe0550c6629ad21ff,
    0xbd977191f0c7f92c, 0x60e86cc5b57ae4c1,
    0xb80d96177a0ceb9d, 0x0582f9ee9cd6b9dc,
    0xf05686ccf88f9252, 0x4ad94f9b8026ac29,
    0x6036d5770e574441, 0xd6017ee21b2f3b2f,
    0x3cd1b89e7fab0077, 0x30bf607016f8ac4d,
    0x9d09fc9e0294610b, 0xb8f6e7dbadbccfcc,
    0xbd6d4bfcab1e6974, 0x022e86a39a5c1c30,
    0x55323c87277e369a, 0x3b64b3f1b9331872,
    0xed884a541669fd84, 0x7de446f66aa1e65c,
    0xd7f43e17ea245581, 0x3d36ab3280f23d64,
    0x01a253cbbe9fd5b3, 0xee1000b6f0e4f898,
    0xa065843d3517f3fe, 0x5d944feb26b784df,
    0xa24c17ca358d8c31, 0x0f500bc324c9fbd3,
    0xd7b9db533f33f3f3, 0x181afb2abbee7f08,
    0x3a8ddb68cf9e6297, 0xd32ce120638d40d9,
    0xd38c4df970bac804, 0x5163592616c81d6c,
    0x2224b17a43a3443d, 0x0f779ad340a280d2,
    0xc016ca786eb9fb68, 0x4e0f367cb0b5475f,
    0x6b99594912f96ce2, 0xab6c8a3a013e0d31,
    0x9808ede6de89dce8, 0xec30b6a56bfea1ef,
    0xcc70f9eee9e6f895, 0xe5496881e2698407,
    0xda9dded9baa33814, 0x6dc52d6945921068,
    0xa09a69cbf3e01173, 0xe8ba3950083a435a,
    0x316681fb087f66e5, 0x404dc382e017bc6f,
    0x73d1483e26f5aa96, 0xff5bb1c87b34f468,
    0x4dd24d50700f8d5b, 0x8b7c3fbaa884ec30,
    0x2e1d9a5244d378d3, 0x761e7c589c7b370e,
    0x144c607eeb66d735, 0xe01f01f6e918d6b9,
    0xc37bce6199184dfe, 0x6fd5916f4dd453a6,
    0x5c1b309e8ba7a60f, 0x62d0f87e9635d859,
    0xaa5b8f475933b141, 0x061b162eda8d5c2f,
    0x3600a8709bfb2ddb, 0xb45db55d20acfcac,
    0x5c56a414488609b6, 0xe250e1a9d65eb8d5,
    0xbfd5bbdfaa6ccedf, 0x2ce7c80de232255d,
    0xf9feaad16aa5c160, 0xd6ef7b29ee021617,
    0xac799f98de064475, 0x6ba8978350895877,
    0xacdc9717a8b78ea6, 0xa02f16ac47953fd3,
    0xbe53906a738d005e, 0x59c9f0180203f5a8,
    0x25bf2849ede8fd46, 0xa1e17964b1dad81d,
    0x31bf6deee8afb0a1, 0x9a6c9177dd52e5fc,
    0x94725efbb93938f8, 0x19914059a648ffb1,
    0x241e31ace37d3a8f, 0xc6b3305c549f0ffe,
    0xb34685478569f157, 0x9cbc1a4ec3d28cf4,
    0x7522d21500f2f080, 0x3819df500b71f889,
    0xc01f8bde5c1431b8, 0x08e626252b66ba3c,
    0x4318c73caf1ce674, 0xf3d3579cc5aebd4d,
    0x235f961f153309be, 0xfa03d11522a8442d,
    0xe605c8f971d69e38, 0x81665f3cae8b955b,
    0x4f4e6efe462f0693, 0xdeb8f92e316d70ae,
    0x39753a37d4c57cff, 0xc8598828b6a45f6c,
    0x84385186bca8c0cb, 0xbb490e6ae6217dd9,
    0xe20190a73d553f64, 0xcf23fbacb0056898,
    0x2800ac2ecdda8fcd, 0x767e05a7af82e964,
    0x42b22f87925c0895, 0x4caaac0c9e994dc7,
    0xc6a929a776b99df1, 0x748aa37829dd4a8e,
    0x35031fcef7205d96, 0x514393ec7018aa3b,
    0x7ee9cb03c11c96f4, 0xcf2361a8dc844140,
    0x09b2db6323b82c1b, 0x7889aee3652e5012,
    0x513b318d6620d517, 0x58765866145e20fc,
    0x07d4bba7d64de6a2, 0x48ec306a4fbc7e70,
    0x3aa574e745cc6a11, 0x2f6e24ac3588df24,
    0x8d89f9b21541115e, 0xa460ca722db0eb5f,
    0xb8ca8e25e502017b, 0x56f458215d97da64,
    0xbe1ebe0505749899, 0x2ddc84692931cfa8,
    0x77ea8d1fb174a517, 0x7c6b90aa7a8657a8,
    0xb260ff3b8c7a68aa, 0x8b95d26bf24b008b,
    0x010a02dbfd36e159, 0x74f6ada530f9f06d,
    0xd25c7d18eb941018, 0xb8fada2ef779b8c7,
    0x0bbaed99cd8454f5, 0x10318e67f5db7cac,
    0x2d733580c0a980ed, 0x651187232ecac8c1,
    0x001ef7f5ad86157d, 0xf1b67814cb715f5d,
    0xa3a49d8e04a2188b, 0xbcfd67176037fd5c,
    0x559a08edf1f9786c, 0xec3fc74af308aae3,
    0x7e3d4023197fe64a, 0x8e2e98641fec5fa3,
    0x5c32fbf76e3e2ceb, 0x32a4be44707feee0,
    0x4c83875c384a265d, 0xeb69d06071a69b3a,
    0x49141ac611d1574a, 0xae41f7b6fd57689d,
    0x431fdcef3403699d, 0xcc10667b0948b55c,
    0xb246841a33d80233, 0x5077aeb1d0c1e7a8,
    0x3a6493f0a67176dc, 0xb9aa29c4151e6fc9,
    0xfde329d35096d6e2, 0xf3b499718d30afcb,
    0xeac74060f3db8366, 0xce0f6a99ddeec23c,
    0x0fe3dc6f6439815c, 0xb5bd66164fd0b289,
    0xcc12f2815bbc5827, 0x80c8109be5656e1b,
    0x072d1915d6a5fc5b, 0xf404f4de53ccd2e1,
    0xa64ce94bd18d9b54, 0x8f857a783fffe1d8,
    0x7bd168208bd4bda0, 0x7cfc4a2539555d57,
    0x9590dc8b34ace70c, 0xdfe6da52c5ff975d,
    0x18fa4d7751669bdc, 0x4778a389dbae4148,
    0x87214bf97fb91804, 0xfd3d7ed208323dd0,
    0xfcb92ece49520d82, 0xb440c9906da469a7,
    0x60d45368e4d92388, 0xea9e051701464b8e,
    0x6e92032ae2bfff90, 0x4684e09936d72249,
    0xb61277855d30d2fb, 0xa500eb6b65e337c6,
    0xd92fa4e3b7128bc3, 0x933a10bb8eed9c59,
    0x79b53ce6154174fa, 0x38ebd0f654519f03,
    0x53bdd912dac8a438, 0x53b9875959d5b4fe,
    0x4787364cade18eac, 0xbd2c79ac653278f0,
    0x238d4447494410ad, 0xbe86da2467b95620,
    0x7942675038bcbc3b, 0x1de6addc99432b7a,
    0x2d94cc1c080f5e77, 0xf8a1487e5ab7bace,
    0x39e05b0d375c46e0, 0x5d60cd7ad08a48ff,
    0x769cc0da672a5647, 0x502e9bef1f8245f4,
    0x5221f4fb3df73d8d, 0x51f422c847ab1d87,
    0x15b96e990a585806, 0x58bb77d3922d2862,
    0xfef65dc6b82188bc, 0x7e31999d8ab1830b,
    0xedb386d5f0ef0a87, 0x3d5a3caea0ab57ad,
    0xc675bf37ccab9d76, 0x6afc8f924217be53,
    0x8e8dbce04f2339b7, 0xd3aab59b3dfc597c,
    0x94f60173875b746c, 0xfcfd01346af42c54,
    0xf8fac4386cb9f45e, 0x620705a4852825ee,
    0x5a44a2a0570a9447, 0xbca8007ed17acae3,
    0xf9dcfd8776b90353, 0x5f073ad9ecad91ac,
    0x8b239c94b56bd277, 0xf23c21913fe47323,
    0x1492640c83484cb5, 0xe495d5b477f5b88c,
    0x5b69e5e5f138db3d, 0x5ad351be8a70a8ca,
    0x8967e9fd559849b8, 0x83d556a43c923e24,
    0x49151db2fa38b31a, 0x3859b1d208f06ab3,
    0x600cb422bb9aa0de, 0x1a902181bce6ceca,
    0x148ec30a618d9539, 0xff29a4fd0fba3468,
    0x8d96f85aaea66896, 0xd1b7dcc01eb8b720,
    0x2dc9cc84005685e9, 0x9603051b48e4a831,
    0x2267dc7466090a8c, 0x6d3f4c3455190c4d,
    0xed03cdadd5229e27, 0x9d05b39a76b17b0f,
    0x1729391ee03b50fa, 0x2a59f60bbb285aa7,
    0x9ac9feda112c6e03, 0x2d3fdc8c86f9d72f,
    0x33febc4949a48a63, 0x6443923029d039bb,
    0xaff833e0121e76e9, 0xe3c3100e5036df24,
    0x131df31211d8a169, 0xdd734cb3cacdff31,
    0x87d630438712d69c, 0xc2e67d3b6e78d3e4,
    0xa32b4e7f5c872bd4, 0x5ca4cc2f25af1b9b,
    0x5430b003475e7640, 0x3521359d3de2ace5,
    0xc9148797dee9ac07, 0xe81c7853e098ccef,
    0x714c1f8433f67c44, 0xd5f8538df9401e44,
    0x64393499dc162139, 0x1678e38447e64d3d,
    0x05b3f9d160ad8fb1, 0xddf7d086689fccf8,
    0x501a69bb0a905b6f, 0xb1723aeb379cc000,
    0xf3e4808b476631a2, 0xb9b27d977ad0e6e0,
    0xc30b4aff016b8758, 0xb04a5bb78329ed0b,
    0xecbe6d1425948492, 0x0f4dc390a2a58c8e,
    0x23def5e76d2b43e1, 0xd7de9e4f2cce8005,
    0x804865003dd7d156, 0xda1b073517b0915a,
    0x3651c5ed9da248e2, 0xf6c3169cfae00064,
    0xac1ef081ccdddfcd, 0xb19ca52e9c17e14e,
    0xd2e7cdebafc9b5c0, 0x7d6b8c0cf59e049b,
    0x31838d6d24e59ed5, 0x0cc55dec163af887,
    0xcbe5c933104ff14a, 0xd37d0677d4d4589e,
    0x510b843eb3c8bc1a, 0xae1f3d8f233ffc26,
    0x74157b452b215358, 0x534a89b3a7594cb1,
    0xf8a50df1054019cc, 0xb0ceda9cc3e00946,
    0xb0acc28cdde9d95e, 0x1eadacfe856ebcec,
    0x01fb973ffca356dd, 0x72f8e8b2ec783632,
    0xaaa1aad3f6125ed5, 0x0d1c20df9788f033,
    0x9ecc416074171d65, 0x18df35cc727ccabe,
    0xb7ed388bab0bf3b2, 0xaec5ebe998692423,
    0xa4f568b214f7d1a5, 0xcda23637e74b4cf7,
    0xce41f3af862ddb94, 0x21f7735e66a5e3b1,
    0xce37a48dbe4ab3d8, 0x96367645327a7a48,
    0xb11379f9fb659564, 0xb05ad1404ef77585,
    0xa6571437bb5b402b, 0x403f25a06c0dc479,
    0x6fabb2b061ebe69b, 0x0ba861e34f188158,
    0x2105af87cf12d5f1, 0x761f971c58302034,
    0x736a232b31ac3504, 0xa529e18afa55d5e0,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// External-anchor known answer (b3sum 1.8.5, run in-session):
    /// BLAKE3(b"bont\x01a") = ffe66d54817825f3...  ->
    /// SlotId = u64::from_le_bytes(ffe66d54817825f3) = 0xf3257881546de6ff.
    #[test]
    fn known_answer_b3sum_anchored() {
        let h = lex_hash(b"a");
        let hex: String = h.0[..8].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "ffe66d54817825f3");
        assert_eq!(slot_id(b"a"), SlotId(0xf325_7881_546d_e6ff));
    }

    /// T1 — determinism court against the committed golden table (§8 T1).
    /// Anti-vacuity: window shift (shim M1) or tag drop (shim M2) remaps
    /// every value — both flips re-witnessed in the wave-8 sandbox
    /// (/tmp/a06-w8.k8yZKp, 2026-11-01; 16/16 named mutation classes flip,
    /// zero vacuous, sha256-verified restores). The wave-5 sandbox
    /// /tmp/a06-w5 cited by earlier waves has evaporated (/tmp volatility);
    /// A06 lane receipt files remain NOT on disk (coordinator open item,
    /// D-A08-1 pattern), so every flip is re-executed, never cited.
    #[test]
    fn t1_determinism_court_against_golden_table() {
        for i in 0..1000usize {
            let lex = corpus_lexeme(i);
            assert_eq!(slot_id(&lex).0, GOLDEN[i], "golden mismatch at i={i}");
        }
    }

    /// T4 — spec re-derivation exit gate (§8 T4): the §4 formula applied by
    /// hand must equal `slot_id` for the whole corpus.
    #[test]
    fn t4_spec_rederivation_matches_impl() {
        for i in 0..1000usize {
            let lex = corpus_lexeme(i);
            assert_eq!(slot_id(&lex), spec_slot_id(&lex), "T4 drift at i={i}");
        }
    }

    /// Window-direction pin: `truncate64` reads bytes 0..8 LE, not any other
    /// octet window. Unit-level companion to the T1 window-shift shim.
    #[test]
    fn truncate64_reads_first_eight_bytes_le() {
        let mut d = [0u8; 32];
        for (j, b) in d.iter_mut().enumerate() {
            *b = (j as u8) + 1; // bytes 0..8 = 01..=08, bytes 24..32 = 25..=32
        }
        let s = truncate64(&LexHash(d));
        assert_eq!(s.0.to_le_bytes(), [1u8, 2, 3, 4, 5, 6, 7, 8]);
    }

    /// T5a — negative control: the detector accepts the full 1000-lexeme
    /// corpus (all golden slots distinct) and counts exactly 1000 distinct
    /// slots. Guards a detector that false-positives — a court that only
    /// ever fires is equally vacuous.
    #[test]
    fn t5a_detector_negative_control_over_corpus() {
        let mut det = CollisionDetector::new();
        for i in 0..1000usize {
            det.insert(&corpus_lexeme(i)).expect("corpus must be collision-free");
        }
        assert_eq!(det.len(), 1000);
    }

    /// The lexeme family the witness pair was searched over: `rho/` ++ the
    /// counter as 8 LE bytes.
    #[cfg(test)]
    fn rho_lexeme(x: u64) -> Vec<u8> {
        let mut lex = b"rho/".to_vec();
        lex.extend_from_slice(&x.to_le_bytes());
        lex
    }

    /// T5b — collision-firing witness (§8 T5): a REAL 64-bit prefix
    /// collision, found in-session by a 12-thread Floyd cycle search over
    /// f(x) = first8le(BLAKE3("bont\x01" ++ "rho/" ++ x_le)) (1305 s wall,
    /// self-verified f(a) == f(b) in the harness; toy-domain validation of
    /// the same extraction logic also in-session). The court re-derives
    /// everything through the REAL `slot_id` path and the detector must
    /// refuse with a `CollisionError` naming BOTH lexemes. Anti-vacuity:
    /// removing the detector's collision branch (shim M5) makes this court
    /// fail — flip re-witnessed in the wave-8 sandbox (/tmp/a06-w8.k8yZKp,
    /// 2026-11-01).
    #[test]
    fn t5b_detector_fires_on_real_collision_pair() {
        let a: u64 = 7_983_680_554_871_822_341;
        let b: u64 = 2_147_190_238_777_461_115;
        assert_ne!(a, b);
        let (lex_a, lex_b) = (rho_lexeme(a), rho_lexeme(b));
        // The pair genuinely collides through the real derivation path:
        assert_eq!(slot_id(&lex_a), slot_id(&lex_b), "witness pair must collide");
        assert_eq!(slot_id(&lex_a).0, 0xca1b01cc59d5a73d);
        let mut det = CollisionDetector::new();
        det.insert(&lex_a).expect("first insert must claim its slot");
        let err = det.insert(&lex_b).unwrap_err();
        assert_eq!(err.slot, slot_id(&lex_a));
        assert_eq!(err.first, lex_a);
        assert_eq!(err.second, lex_b);
    }

    /// T5c — §5 distinct-lexeme law court: re-feeding the IDENTICAL lexeme
    /// is idempotent `Ok(())` (a duplicate is the same term, not truncation
    /// drift), while the witness pair (distinct lexemes, equal prefix) still
    /// fires. Anti-vacuity: shim M6 (dedup branch removed — detector fires
    /// on duplicates) flips this court; shim M5 (collision branch removed)
    /// flips t5b. Both flips witnessed in the wave-8 sandbox
    /// /tmp/a06-w8.k8yZKp, 2026-11-01.
    #[test]
    fn t5c_duplicate_lexeme_reinsert_is_idempotent() {
        let mut det = CollisionDetector::new();
        det.insert(&corpus_lexeme(4)).expect("first insert claims its slot");
        // Identical duplicate: Ok, detector unchanged.
        det.insert(&corpus_lexeme(4))
            .expect("duplicate re-insert must stay Ok per A06 §5 (distinct lexemes only)");
        assert_eq!(det.len(), 1, "duplicate must not mint a second slot");
        // Empty-lexeme duplicate over the same idempotence path.
        det.insert(&corpus_lexeme(0)).expect("empty lexeme claims its slot");
        det.insert(&corpus_lexeme(0)).expect("empty-lexeme duplicate stays Ok");
        assert_eq!(det.len(), 2, "empty-lexeme duplicate must not mint a second slot");
        // The whole corpus (duplicates included) stays collision-free
        // through the dedup path: still exactly 1000 distinct slots.
        for i in 0..1000usize {
            det.insert(&corpus_lexeme(i)).expect("corpus must stay collision-free");
        }
        assert_eq!(det.len(), 1000);
        // Distinct lexemes still collide: re-run the T5b witness pair.
        let a: u64 = 7_983_680_554_871_822_341;
        let b: u64 = 2_147_190_238_777_461_115;
        let mut det2 = CollisionDetector::new();
        det2.insert(&rho_lexeme(a)).expect("witness first insert claims its slot");
        let err = det2.insert(&rho_lexeme(b)).unwrap_err();
        assert_eq!(err.slot, slot_id(&rho_lexeme(a)));
        assert_eq!(err.first, rho_lexeme(a));
        assert_eq!(err.second, rho_lexeme(b));
    }

    /// `CollisionError` renders both lexemes and the slot (§5 naming duty).
    #[test]
    fn collision_error_names_both_lexemes() {
        let err = CollisionError {
            slot: SlotId(0xdead_beef_cafe_f00d),
            first: b"first/lexeme".to_vec(),
            second: vec![0xff, 0x00, 0x0a],
        };
        let msg = format!("{err}");
        assert!(msg.contains("0xdeadbeefcafef00d"), "{msg}");
        assert!(msg.contains("0x66697273742f6c6578656d65"), "{msg}"); // "first/lexeme"
        assert!(msg.contains("0xff000a"), "{msg}");
    }
}
