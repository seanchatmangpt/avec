//! DoD 10 raw-consumer court (lane A10): a consumer that sees ONLY the
//! `.oabi` bytes resolves every slot — this test imports NO cgap_bont and NO
//! cgap_cloud items. Its entire world is:
//!
//! 1. the raw file bytes of `ontologies/cloud/tosca_cimi_v1.oabi`;
//! 2. the public A06 §4 derivation, re-stated here by hand
//!    (`SlotId = u64::from_le_bytes(BLAKE3(b"bont\x01" ++ lexeme)[0..8])`);
//! 3. the four primitive root lexemes + expected slot literals, restated
//!    from the committed pack spec (independent transcription — if lib.rs
//!    goldens drift, this file's literals still pin the pack).
//!
//! Header fields are parsed by byte offset (A06 §6 offset table), records
//! are walked at 48-byte stride. No loader type, no facade type.

use std::fs;

use cgap_blake3::hash;

const PACK_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tosca_cimi_v1.oabi");

/// (lexeme, expected SlotId) for the four primitive roots — restated
/// independently of `cgap_cloud` (consumer knowledge = pack spec).
const ROOT_EXPECTATIONS: &[(&str, u64)] = &[
    ("tosca.nodes.Compute", 0xB854_D7A1_774A_EB63),
    ("tosca.nodes.BlockStorage", 0x287C_79B1_117B_30C5),
    ("cimi.Event", 0xEF1E_1F61_9B71_BD37),
    ("cimi.Credential", 0xB631_C488_3C95_D603),
];

/// Re-derive the full 32-byte LexHash per A06 §4 (hand path).
fn lex_hash(lexeme: &[u8]) -> [u8; 32] {
    let mut tagged = Vec::with_capacity(5 + lexeme.len());
    tagged.extend_from_slice(b"bont\x01");
    tagged.extend_from_slice(lexeme);
    hash(&tagged)
}

#[test]
fn raw_bytes_only_consumer_resolves_all_four_primitives() {
    let bytes = fs::read(PACK_PATH).expect("pack file readable");

    // --- header by offset (A06 §6) ------------------------------------------
    assert_eq!(&bytes[0..8], b"OABIv1\0\0", "magic");
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 64, "header_len");
    assert_eq!(u16::from_le_bytes(bytes[12..14].try_into().unwrap()), 1, "version_major");
    assert_eq!(u16::from_le_bytes(bytes[14..16].try_into().unwrap()), 0, "version_minor");
    let flags = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
    assert_eq!(flags & 0b11, 0b11, "LE + LexHash flags");
    let class_count = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    let enum_count = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let slot_count = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
    let record_size = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
    assert_eq!((class_count, enum_count, slot_count, record_size), (4, 6, 31, 48));
    assert!(bytes[36..64].iter().all(|&b| b == 0), "reserved zero");
    assert_eq!(bytes.len(), 64 + slot_count as usize * 48);

    // --- records by stride; index them by SlotId -----------------------------
    let mut slots: Vec<(u64, [u8; 32])> = Vec::with_capacity(slot_count as usize);
    for i in 0..slot_count as usize {
        let base = 64 + i * 48;
        let slot = u64::from_le_bytes(bytes[base..base + 8].try_into().unwrap());
        let class = u32::from_le_bytes(bytes[base + 8..base + 12].try_into().unwrap());
        assert!(class < class_count, "record {i} class out of range");
        let enum_family = u16::from_le_bytes(bytes[base + 12..base + 14].try_into().unwrap());
        assert!((enum_family as u32) < enum_count, "record {i} enum out of range");
        let mut lex = [0u8; 32];
        lex.copy_from_slice(&bytes[base + 16..base + 48]);
        // §6 per-record obligation, re-checked on the raw bytes.
        assert_eq!(slot, u64::from_le_bytes(lex[0..8].try_into().unwrap()), "record {i} derivation");
        slots.push((slot, lex));
    }

    // --- the four primitive roots resolve from bytes + public derivation ----
    for (lexeme, expected) in ROOT_EXPECTATIONS {
        let d = lex_hash(lexeme.as_bytes());
        let slot = u64::from_le_bytes(d[0..8].try_into().unwrap());
        assert_eq!(&slot, expected, "spec re-derivation != committed literal: {lexeme}");
        assert!(
            slots.iter().any(|(s, l)| s == expected && *l == d),
            "pack has no record for {lexeme} (slot {expected:#018x}) with matching preimage"
        );
    }
    // Totality: 31 distinct slots present, zero duplicates.
    slots.sort_by_key(|(s, _)| *s);
    let dup = slots.windows(2).find(|w| w[0].0 == w[1].0);
    assert!(dup.is_none(), "duplicate slot in pack: {:?}", dup.map(|d| d[0].0));
    assert_eq!(slots.len(), 31);
}
