//! Wave-1 derivation harness (lane A09, ontologies/fibo).
//!
//! Computes the lane-doc §4 corpus SlotIds/LexHashes with the real in-repo
//! BLAKE3 substrate (cgap-blake3, A06's read-only hash basis) and prints
//! paste-ready Rust literals for `src/lib.rs`. The committed companion
//! `tests/wave1_witness.rs` then asserts the embedded consts against a live
//! re-derivation — real collaborator, assert final state, no mocks.

use cgap_blake3::hash;

/// A06 §5 (cited, not redefined): namespace tag, part of the Wave-1 freeze.
const BONT_NS_V1: &[u8] = b"bont\x01";

/// Lane-doc §4 corpus: (const name, lexeme) in harvest order.
const CORPUS: &[(&str, &[u8])] = &[
    ("USD_SLOT", b"fibo:FndUtlCur:USD"),
    ("EUR_SLOT", b"fibo:FndUtlCur:EUR"),
    ("JPY_SLOT", b"fibo:FndUtlCur:JPY"),
    ("CORP_BOND", b"fibo:SecDbt:CorporateBond"),
    ("BOND", b"fibo:SecDbt:Bond"),
    ("DEBT_INSTR", b"fibo:SecDbt:DebtInstrument"),
    ("ACCT_RECV", b"fibo:ClAcct:AccountsReceivable"),
    ("MATURITY_DATE", b"fibo:SecDbt:hasMaturityDate"),
    ("COUPON_RATE", b"fibo:SecDbt:hasCouponRate"),
];

fn lex_hash(lexeme: &[u8]) -> [u8; 32] {
    let mut tagged = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    tagged.extend_from_slice(BONT_NS_V1);
    tagged.extend_from_slice(lexeme);
    hash(&tagged)
}

#[test]
fn generate_corpus_table() {
    let mut collisions: Vec<&str> = Vec::new();
    let mut seen: std::collections::HashMap<u64, &str> = std::collections::HashMap::new();
    println!("// generated {} — cgap-blake3::hash over BONT_NS_V1 ++ lexeme", line!());
    for (name, lexeme) in CORPUS {
        let d = lex_hash(lexeme);
        let slot = u64::from_le_bytes(d[0..8].try_into().unwrap());
        // A06 §5 producer obligation: emit-time collision check (fail-closed).
        if let Some(prev) = seen.insert(slot, name) {
            collisions.push(name);
            panic!("SlotId collision: {prev} and {name} both -> {slot:#018x}");
        }
        let lex_hex: String = d.iter().map(|b| format!("0x{b:02X}, ")).collect();
        // Print the VALUE hex (`{:#018X}` of the u64), not the digest byte
        // order: SlotId = u64::from_le_bytes(d[0..8]) (A06 §4) — the prior
        // byte-order rendering disagreed with the value it labeled (caught
        // by tests/wave1_witness.rs derivation_court, 2026-11-01).
        println!(
            "pub const {name}: u64 = {slot:#018X}; // SlotId = from_le_bytes(d[0..8])\npub const LEX_{name}: [u8; 32] = [{lex_hex}];"
        );
    }
    assert!(collisions.is_empty());
    assert_eq!(seen.len(), CORPUS.len(), "corpus size drifted");
}
