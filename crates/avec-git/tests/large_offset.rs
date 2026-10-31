//! Wave-14 lane 11 (recovery) — bit-31 large-offset escape court for
//! `avec_git::idx_read::IdxV2` and the `write_idx_v2` refusal surface.
//!
//! Court contract (lane dispatch):
//!
//! 1. max small-offset `2^31-1` resolves via the small table;
//! 2. offsets `2^31`, `2^31+4096`, `2^32-1` in the large-offset table resolve
//!    via the escape;
//! 3. the writer refuses offsets `>= 2^31` typed (`OffsetTooLarge`), and still
//!    accepts the `2^31-1` boundary;
//! 4. a corrupted MSB-on-small-entry (escape claimed with no large table, or
//!    with an escape index beyond the table) is a TYPED error — never a panic,
//!    never a silent wrong offset;
//! 5. a 3-object mixed small/boundary/large idx resolves every lookup exactly.
//!
//! Plus one beyond-spec probe (case 6): the large-offset table exists so packs
//! can exceed 4 GiB — offsets above `2^32` must resolve as full 64-bit values,
//! not low-32 truncations.
//!
//! Chicago discipline: spec-conformant synthetic `.idx` v2 bytes (a 2 GiB+ pack
//! is not manufacturable in-test; the escape is a byte-format property), real
//! reader/writer over those bytes, assert final resolved values. No mocks.
//!
//! REGRESSION COURTS (cases 4 and 6 were defect witnesses, red on purpose
//! until the 2026-10-31 `src/idx_read.rs` fix landed):
//! `offset()`'s arithmetic select used to mask the 64-bit large entry with a
//! 32-bit mask (`msb_mask as u64` = `0x0000_0000_FFFF_FFFF`), so (a) an escape
//! with an absent/out-of-range large entry silently resolved to `Ok(0)` instead
//! of a typed error, and (b) any large offset `>= 2^32` silently truncated to
//! its low 32 bits. The fix negates the MSB mask in u64 (full-width select, no
//! truncation) and returns `Err(GitError::Truncated)` on the corrupt escape.
//! Case 4 pins the typed-error contract; case 6 pins full 64-bit resolution;
//! `tests/ladder_mutations.rs`'s "typed 0-or-garbage" matrix row was
//! re-derived in the same change.
//!
//! Run: `cargo test -p avec-git --test large_offset --release`

use avec_git::idx_read::IdxV2;
use avec_git::pack_write::{crc32, write_idx_v2, IdxEntry, Sha1};
use avec_git::{GitError, Oid};

/// Largest offset encodable in the small table (`2^31 - 1`).
const SMALL_MAX: u64 = 0x7FFF_FFFF;
/// Smallest offset that must escape to the large table (`2^31`).
const LARGE_MIN: u64 = 0x8000_0000;

/// Section layout mirror (big-endian file): magic 4 + version 4 + fanout
/// 1024. Kept in sync with `src/idx_read.rs`; the byte-length assertions
/// below fail loudly if the layout ever drifts.
const OID_TABLE_OFF: usize = 8 + 256 * 4; // 1032

// ------------------------------------------------------------- fixtures ----

/// Deterministic, collision-free OID: bytes encode `(tag, i)` directly, so
/// distinct inputs give distinct OIDs with no hashing.
fn oid_for(tag: u64, i: u64) -> Oid {
    let mut o = [0u8; 20];
    o[0..8].copy_from_slice(&tag.to_be_bytes());
    o[8..16].copy_from_slice(&i.to_be_bytes());
    o[16] = 0xA5; // family marker
    o
}

/// Sorted `(oid, offset)` rows, one per requested offset (deterministic).
fn entries_for(tag: u64, offsets: &[u64]) -> Vec<(Oid, u64)> {
    let mut v: Vec<(Oid, u64)> = offsets
        .iter()
        .enumerate()
        .map(|(i, &off)| (oid_for(tag, i as u64), off))
        .collect();
    v.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    v
}

/// Serialize a spec-conformant `.idx` v2 (magic, version, fan-out, OID table,
/// CRC table, small-offset table with bit-31 escape words, large-offset table,
/// pack checksum placeholder, trailing idx SHA-1). Offsets `>= 2^31` are
/// appended to the large table in row order, exactly as git does.
fn build_idx(entries: &[(Oid, u64)]) -> Vec<u8> {
    assert!(
        entries.windows(2).all(|w| w[0].0 < w[1].0),
        "rows must be strictly sorted"
    );
    let n = entries.len();
    let mut out = Vec::with_capacity(OID_TABLE_OFF + 44 * n + 40);
    out.extend_from_slice(&[0xFF, b't', b'O', b'c']);
    out.extend_from_slice(&2u32.to_be_bytes());
    let mut fan = [0u32; 256];
    for (oid, _) in entries {
        fan[oid[0] as usize] += 1;
    }
    for i in 1..256 {
        fan[i] += fan[i - 1];
    }
    for f in fan {
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
        .filter(|(_, (_, off))| *off >= LARGE_MIN)
        .map(|(i, (_, off))| (i, *off))
        .collect();
    for (i, (_, off)) in entries.iter().enumerate() {
        let word = if *off >= LARGE_MIN {
            let rank = large
                .iter()
                .position(|(j, _)| *j == i)
                .expect("large rows are in row order");
            0x8000_0000u32 | rank as u32
        } else {
            *off as u32
        };
        out.extend_from_slice(&word.to_be_bytes());
    }
    for (_, off) in &large {
        out.extend_from_slice(&off.to_be_bytes());
    }
    out.extend_from_slice(&[0u8; 20]); // pack checksum: reader-external
    let mut h = Sha1::new();
    h.update(&out);
    out.extend_from_slice(&h.finalize());
    out
}

/// Recompute the trailing idx SHA-1 after a one-bit mutation, so the court
/// isolates the escape-bit defect from checksum effects (shim-court hygiene).
fn reseal(bytes: &mut [u8]) {
    let body = bytes.len() - 20;
    let mut h = Sha1::new();
    h.update(&bytes[..body]);
    let sum = h.finalize();
    bytes[body..].copy_from_slice(&sum);
}

/// Raw big-endian small-offset table word for `row`.
fn small_word(bytes: &[u8], n: usize, row: usize) -> u32 {
    let o = OID_TABLE_OFF + 24 * n + row * 4;
    u32::from_be_bytes(bytes[o..o + 4].try_into().expect("small word in bounds"))
}

/// Raw big-endian large-offset table entry for `slot`.
fn large_word(bytes: &[u8], n: usize, slot: usize) -> u64 {
    let o = OID_TABLE_OFF + 28 * n + slot * 8;
    u64::from_be_bytes(bytes[o..o + 8].try_into().expect("large word in bounds"))
}

/// One full-precision assertion pass over every row: direct `offset(i)` and
/// ladder `lookup(oid)` both resolve the exact 64-bit offset.
fn assert_all_rows_exact(idx: &IdxV2<'_>, entries: &[(Oid, u64)]) {
    for (i, (oid, off)) in entries.iter().enumerate() {
        assert_eq!(
            idx.offset(i as u32).expect("offset(i) in bounds"),
            *off,
            "offset() row {i}"
        );
        let (row, found) = idx
            .lookup(oid)
            .expect("lookup within ladder window")
            .unwrap_or_else(|| panic!("lookup missed present row {i}"));
        assert_eq!(row as usize, i, "lookup row {i}");
        assert_eq!(found, *off, "lookup offset row {i}");
    }
}

// ---------------------------------------------------------------- cases ----

/// Case 1: `2^31-1` is the largest small-table offset and resolves without
/// the large table (the file physically contains none).
#[test]
fn case1_max_small_offset_2p31_minus_1_resolves_via_small_table() {
    let entries = entries_for(0x11, &[12, 0x0001_0000, SMALL_MAX]);
    let bytes = build_idx(&entries);
    let n = entries.len();
    // Small-only file: exactly no room for a large-offset table.
    assert_eq!(bytes.len(), OID_TABLE_OFF + 28 * n + 40);
    let row = entries
        .iter()
        .position(|(_, off)| *off == SMALL_MAX)
        .expect("boundary row present");
    assert_eq!(
        small_word(&bytes, n, row),
        SMALL_MAX as u32,
        "boundary offset stored plain, MSB clear — small table, no escape"
    );
    let idx = IdxV2::parse(&bytes).expect("parse small-only idx");
    idx.verify_idx_checksum().expect("idx checksum valid");
    assert_all_rows_exact(&idx, &entries);
    println!(
        "RECEIPT case1: n={n} boundary=0x{:08X} raw_word=0x{:08X} resolved exact via small table",
        SMALL_MAX,
        small_word(&bytes, n, row)
    );
}

/// Case 2: `2^31`, `2^31+4096`, `2^32-1` all resolve through the bit-31
/// escape into the large-offset table.
#[test]
fn case2_escape_offsets_2p31_2p31_plus_4096_2p32_minus_1() {
    let offsets = [LARGE_MIN, LARGE_MIN + 4096, 0xFFFF_FFFF];
    let entries = entries_for(0x22, &offsets);
    let n = entries.len();
    let bytes = build_idx(&entries);
    // Every row is an escape word whose low bits are its large-table rank,
    // and the large table physically holds the full 64-bit values.
    for (i, (_, off)) in entries.iter().enumerate() {
        let w = small_word(&bytes, n, i);
        assert_eq!(w & 0x8000_0000, 0x8000_0000, "row {i} escape bit set");
        assert_eq!(w & 0x7FFF_FFFF, i as u32, "row {i} large rank");
        assert_eq!(large_word(&bytes, n, i), *off, "row {i} large entry bytes");
    }
    let idx = IdxV2::parse(&bytes).expect("parse large-table idx");
    idx.verify_idx_checksum().expect("idx checksum valid");
    assert_all_rows_exact(&idx, &entries);
    println!(
        "RECEIPT case2: 3/3 escape rows exact (0x80000000, 0x80001000, 0xFFFFFFFF)"
    );
}

/// Case 3 (re-assert): the writer refuses every offset `>= 2^31` as a typed
/// `OffsetTooLarge`, accepts the `2^31-1` boundary, and emits no idx when any
/// row offends.
#[test]
fn case3_writer_refuses_offsets_at_or_above_2p31_typed() {
    let cs: Oid = [7u8; 20];
    let mk = |off: u32| IdxEntry {
        oid: oid_for(0x33, off as u64),
        offset: off,
        crc: 0,
    };
    // Boundary accept, and the emitted idx round-trips through the reader.
    let mut ok = vec![mk(SMALL_MAX as u32)];
    let ok_bytes = write_idx_v2(&mut ok, &cs).expect("2^31-1 must be accepted");
    let idx = IdxV2::parse(&ok_bytes).expect("boundary idx parses");
    assert_eq!(
        idx.offset(0).expect("offset"),
        SMALL_MAX,
        "writer boundary offset reads back exact"
    );
    for off in [LARGE_MIN as u32, LARGE_MIN as u32 + 4096, u32::MAX] {
        let mut e = vec![mk(off)];
        match write_idx_v2(&mut e, &cs) {
            Err(GitError::OffsetTooLarge) => {}
            other => panic!(
                "offset 0x{off:08X}: expected OffsetTooLarge, got {:?}",
                other.map(|_| ())
            ),
        }
    }
    // One offending row among legal ones is refused; nothing is emitted.
    let mut mixed = vec![mk(12), mk(LARGE_MIN as u32), mk(24)];
    assert!(matches!(
        write_idx_v2(&mut mixed, &cs),
        Err(GitError::OffsetTooLarge)
    ));
    println!(
        "RECEIPT case3: 0x7FFFFFFF accepted+round-trips; 0x80000000/0x80001000/0xFFFFFFFF refused OffsetTooLarge; mixed refused"
    );
}

/// Case 4: a corrupted MSB-on-small-entry must be a TYPED error — never a
/// panic, never a silent offset. Two variants: (A) escape claimed with no
/// large table in the file; (B) escape index beyond the large table that does
/// exist. The mutation is exactly one bit; the trailing checksum is resealed
/// so nothing else is invalid.
#[test]
fn case4_corrupt_msb_on_small_entry_is_typed_error_never_panic() {
    // Variant A: no large table at all.
    let entries = entries_for(0x44, &[12, 340, SMALL_MAX]);
    let n = entries.len();
    let mut bytes = build_idx(&entries);
    let row = 1; // raw word 0x0000_0154 -> after flip 0x8000_0154, rank 340
    bytes[OID_TABLE_OFF + 24 * n + row * 4] |= 0x80;
    reseal(&mut bytes);
    let idx = IdxV2::parse(&bytes).expect("structure still parses");
    idx.verify_idx_checksum()
        .expect("resealed checksum verifies (mutation isolated)");
    let o = idx.offset(row as u32);
    let l = idx.lookup(&entries[row].0);
    assert!(
        matches!(o, Err(_)),
        "variant A offset(): expected typed error, got {:?}",
        o
    );
    assert!(
        matches!(l, Err(_)),
        "variant A lookup(): expected typed error, got {:?}",
        l
    );

    // Variant B: large table present (1 entry), escape index beyond it.
    let entries_b = entries_for(0x45, &[12, LARGE_MIN, 24]);
    let mut bytes_b = build_idx(&entries_b);
    bytes_b[OID_TABLE_OFF + 24 * 3] |= 0x80; // row 0: 12 -> rank 12, table has rank 0 only
    reseal(&mut bytes_b);
    let idx_b = IdxV2::parse(&bytes_b).expect("structure still parses");
    let o_b = idx_b.offset(0);
    let l_b = idx_b.lookup(&entries_b[0].0);
    assert!(
        matches!(o_b, Err(_)),
        "variant B offset(): expected typed error, got {:?}",
        o_b
    );
    assert!(
        matches!(l_b, Err(_)),
        "variant B lookup(): expected typed error, got {:?}",
        l_b
    );
    println!(
        "RECEIPT case4: 2 variants (absent large table, escape index beyond table) both typed errors, no panic"
    );
}

/// Case 5: 3-object mixed small / boundary / large idx — every lookup exact.
#[test]
fn case5_mixed_three_object_idx_small_boundary_large() {
    let entries = entries_for(0x55, &[12, SMALL_MAX, LARGE_MIN + 4096]);
    let n = entries.len();
    let bytes = build_idx(&entries);
    // Exactly one large-table entry for the one large row.
    assert_eq!(bytes.len(), OID_TABLE_OFF + 28 * n + 8 + 40);
    let idx = IdxV2::parse(&bytes).expect("parse mixed idx");
    idx.verify_idx_checksum().expect("idx checksum valid");
    assert_eq!(idx.len(), 3);
    assert_eq!(idx.fanout(255), 3);
    // Escape words only where the offset requires them.
    for (i, (_, off)) in entries.iter().enumerate() {
        let w = small_word(&bytes, n, i);
        assert_eq!((w & 0x8000_0000) != 0, *off >= LARGE_MIN, "row {i} MSB iff large");
    }
    assert_all_rows_exact(&idx, &entries);
    println!(
        "RECEIPT case5: small=12 boundary=0x7FFFFFFF large=0x80001000 — 3/3 lookups exact"
    );
}

/// Case 6 (beyond-spec probe): offsets above `2^32` are the reason the
/// large-offset table is 64-bit — they must resolve exactly, not truncated to
/// their low 32 bits.
#[test]
fn case6_offsets_above_2p32_resolve_as_full_64bit() {
    let entries = entries_for(0x66, &[0x1_0000_0000, 0x1_0000_1000, LARGE_MIN]);
    let bytes = build_idx(&entries);
    let idx = IdxV2::parse(&bytes).expect("parse 64-bit idx");
    idx.verify_idx_checksum().expect("idx checksum valid");
    assert_all_rows_exact(&idx, &entries);
    println!("RECEIPT case6: 0x1_0000_0000 and 0x1_0000_1000 resolved 64-bit exact");
}
