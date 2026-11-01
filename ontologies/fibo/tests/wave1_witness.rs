//! Wave-1 witness courts (lane A09) — assert the embedded consts against a
//! live re-derivation through the real in-repo BLAKE3 substrate
//! (`cgap_blake3::hash`, the same read-only basis A06 cites). Real
//! collaborator, assert final state, no mocks. Companion to
//! `tests/wave1_derivation.rs` (the generator that produced the values).

use cgap_blake3::hash;
use cgap_fibo::{
    ArRecord, BondRecord, ClassSlot, FiboCurrency, ACCT_RECV_SLOT, BOND_SLOT, CLASS_ACCT_RECV,
    CLASS_BOND, CLASS_CORP_BOND, CLASS_DEBT_INSTR, CLASS_NONE, ATTR_COUPON_RATE,
    ATTR_MATURITY_DATE, COUPON_RATE_SLOT, CORP_BOND_SLOT, DEBT_INSTR_SLOT, EUR_SLOT, JPY_SLOT,
    LEX_ACCT_RECV, LEX_BOND, LEX_COUPON_RATE, LEX_CORP_BOND, LEX_DEBT_INSTR, LEX_EUR, LEX_JPY,
    LEX_MATURITY_DATE, LEX_USD, MATURITY_DATE_SLOT, USD_SLOT,
};

/// A06 §5 (cited, not redefined): namespace tag v1, part of the freeze.
const BONT_NS_V1: &[u8] = b"bont\x01";

/// Lane-doc §4 corpus: (committed slot const, committed lex witness, lexeme)
/// in harvest order. Restated independently here — the corpus list itself is
/// part of the court, not shared code with the generator.
const CORPUS: &[(u64, [u8; 32], &[u8])] = &[
    (USD_SLOT, LEX_USD, b"fibo:FndUtlCur:USD"),
    (EUR_SLOT, LEX_EUR, b"fibo:FndUtlCur:EUR"),
    (JPY_SLOT, LEX_JPY, b"fibo:FndUtlCur:JPY"),
    (CORP_BOND_SLOT, LEX_CORP_BOND, b"fibo:SecDbt:CorporateBond"),
    (BOND_SLOT, LEX_BOND, b"fibo:SecDbt:Bond"),
    (DEBT_INSTR_SLOT, LEX_DEBT_INSTR, b"fibo:SecDbt:DebtInstrument"),
    (ACCT_RECV_SLOT, LEX_ACCT_RECV, b"fibo:ClAcct:AccountsReceivable"),
    (MATURITY_DATE_SLOT, LEX_MATURITY_DATE, b"fibo:SecDbt:hasMaturityDate"),
    (COUPON_RATE_SLOT, LEX_COUPON_RATE, b"fibo:SecDbt:hasCouponRate"),
];

fn lex_hash(lexeme: &[u8]) -> [u8; 32] {
    let mut tagged = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    tagged.extend_from_slice(BONT_NS_V1);
    tagged.extend_from_slice(lexeme);
    hash(&tagged)
}

/// Court 1 (lane doc T9 analog, Wave-1 slice): every committed `*_SLOT` and
/// `LEX_*` equals a live re-derivation from its lexeme, in harvest order;
/// `SlotId == low64(LexHash)` (A06 §2); no SlotId collision in the corpus
/// (A06 §5 producer obligation, Wave-1 instance).
///
/// Anti-vacuity: any one-byte edit to a committed const, a lexeme, the
/// namespace tag, or the truncation window flips this court — witnessed
/// mechanically in-repo by [`derivation_mutation_witness`] below. (The
/// prior comment cited mutations "M1"/"M3" in a lane-report file that was
/// never committed — absent-artifact defect, repaired wave 5.)
#[test]
fn derivation_court() {
    let mut seen = std::collections::HashMap::new();
    for (i, (slot_const, lex_const, lexeme)) in CORPUS.iter().enumerate() {
        let d = lex_hash(lexeme);
        let slot = u64::from_le_bytes(d[0..8].try_into().unwrap());
        assert_eq!(
            slot, *slot_const,
            "corpus[{i}] SlotId drift: lexeme {:?}",
            String::from_utf8_lossy(lexeme)
        );
        assert_eq!(
            &d[..], lex_const,
            "corpus[{i}] LexHash drift: lexeme {:?}",
            String::from_utf8_lossy(lexeme)
        );
        // A06 §2: SlotId is the low 64 bits of LexHash (LE).
        assert_eq!(slot, u64::from_le_bytes(lex_const[0..8].try_into().unwrap()));
        if let Some(prev) = seen.insert(slot, i) {
            panic!("SlotId collision: corpus[{prev}] and corpus[{i}] both -> {slot:#018x}");
        }
    }
    assert_eq!(seen.len(), 9, "corpus size drifted");
}

/// Court 1b — derivation mutation witness (T9-named mutations, made
/// mechanical and in-repo). The lane doc §8 requires every court to be
/// witnessed FAILING under a named mutation before it counts as passing;
/// the wave-1 slice carried that witness only as a prose claim citing a
/// lane-report file that was never committed (absent-artifact defect,
/// repaired wave 5). This court IS the witness: each named mutation is
/// applied to the real derivation of the real corpus through the real
/// substrate, and the equality predicate [`derivation_court`] pins must
/// flip for the mutated row.
///
/// Named mutations, all lane-doc T9 class:
/// - W1: namespace-tag drop — `hash(lexeme)` without the `b"bont\x01"`
///   prefix (A06 T1 semantics);
/// - W2: truncation-window shift — `d[1..9]` instead of `d[0..8]`;
/// - W3: lexeme byte flip — one byte of the lexeme mutated.
///
/// Anti-vacuity of this witness itself: a mutation that does NOT flip
/// (e.g. a no-op "mutation") fails this court by construction, because
/// every case is an `assert_ne` against a value the base court pins.
#[test]
fn derivation_mutation_witness() {
    for (i, (slot_const, lex_const, lexeme)) in CORPUS.iter().enumerate() {
        let ctx = |m: &str| format!("W-mutation {m} corpus[{i}] {:?}", String::from_utf8_lossy(lexeme));

        // W1: namespace-tag drop.
        let d = hash(lexeme);
        assert_ne!(
            u64::from_le_bytes(d[0..8].try_into().unwrap()),
            *slot_const,
            "{}",
            ctx("W1 ns-tag drop: SlotId unchanged")
        );
        assert_ne!(&d[..], lex_const, "{}", ctx("W1 ns-tag drop: LexHash unchanged"));

        // W2: truncation-window shift.
        let d = lex_hash(lexeme);
        let shifted = u64::from_le_bytes(d[1..9].try_into().unwrap());
        assert_ne!(shifted, *slot_const, "{}", ctx("W2 window shift: SlotId unchanged"));

        // W3: one-byte lexeme flip.
        let mut flipped = lexeme.to_vec();
        flipped[0] ^= 0x01;
        let d = lex_hash(&flipped);
        assert_ne!(
            u64::from_le_bytes(d[0..8].try_into().unwrap()),
            *slot_const,
            "{}",
            ctx("W3 lexeme flip: SlotId unchanged")
        );
        assert_ne!(&d[..], lex_const, "{}", ctx("W3 lexeme flip: LexHash unchanged"));
    }
}

/// Court 2 (lane doc T8 forward half, Wave-1 slice): currency discriminants
/// equal the frozen golden values.
///
/// Anti-vacuity mutation (T8-named): "change `Usd` to `0x0349`" fails the
/// forward asserts above (collision with Eur's golden value); the swap
/// mutation is witnessed mechanically by the reverse EnumId->LexHash joins
/// in `tests/wave4_courts.rs::t8_currency_court`.
#[test]
fn currency_discriminant_court() {
    assert_eq!(FiboCurrency::Usd as u16, 0x0348);
    assert_eq!(FiboCurrency::Eur as u16, 0x0349);
    assert_eq!(FiboCurrency::Jpy as u16, 0x034A);
    // Dense distinctness: the three EnumIds are pairwise different.
    assert_ne!(FiboCurrency::Usd as u16, FiboCurrency::Eur as u16);
    assert_ne!(FiboCurrency::Eur as u16, FiboCurrency::Jpy as u16);
}

/// Court 3 (lane doc §4 dense spaces, Wave-1 slice): the lane-assigned
/// ClassIds equal the frozen table, pairwise distinct.
#[test]
fn dense_classid_court() {
    assert_eq!(CLASS_NONE, 0x0000_0000);
    assert_eq!(CLASS_CORP_BOND, 0x0000_0001);
    assert_eq!(CLASS_BOND, 0x0000_0002);
    assert_eq!(CLASS_DEBT_INSTR, 0x0000_0003);
    assert_eq!(CLASS_ACCT_RECV, 0x0000_0010);
    assert_eq!(ATTR_MATURITY_DATE, 0x0000_0020);
    assert_eq!(ATTR_COUPON_RATE, 0x0000_0021);
    let all = [
        CLASS_NONE, CLASS_CORP_BOND, CLASS_BOND, CLASS_DEBT_INSTR, CLASS_ACCT_RECV,
        ATTR_MATURITY_DATE, ATTR_COUPON_RATE,
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b, "dense ClassId collision: {a:#x} vs {b:#x}");
        }
    }
}

/// Court 4 (lane doc T12 runtime half): the frozen layout re-asserted at
/// test time — sizes, aligns, field offsets. The compile-time asserts in
/// `src/lib.rs` pin the same values; this court makes a regression visible
/// as a test failure, not only as a build break.
#[test]
fn layout_court() {
    assert_eq!(core::mem::size_of::<ClassSlot>(), 8);
    assert_eq!(core::mem::align_of::<ClassSlot>(), 8);
    assert_eq!(core::mem::size_of::<FiboCurrency>(), 2);
    assert_eq!(core::mem::size_of::<BondRecord>(), 56);
    assert_eq!(core::mem::align_of::<BondRecord>(), 8);
    assert_eq!(core::mem::offset_of!(BondRecord, instrument), 0);
    assert_eq!(core::mem::offset_of!(BondRecord, currency), 8);
    assert_eq!(core::mem::offset_of!(BondRecord, slot), 16);
    assert_eq!(core::mem::offset_of!(BondRecord, lex), 24);
    assert_eq!(core::mem::size_of::<ArRecord>(), 56);
    assert_eq!(core::mem::align_of::<ArRecord>(), 8);
    assert_eq!(core::mem::offset_of!(ArRecord, class), 0);
    assert_eq!(core::mem::offset_of!(ArRecord, obligor_slot), 8);
    assert_eq!(core::mem::offset_of!(ArRecord, currency), 16);
    assert_eq!(core::mem::offset_of!(ArRecord, lex), 18);
}

/// Byte-image of a `BondRecord` constructed over zero-initialized storage:
/// padding bytes are deterministically zero, so the full 56-byte image is
/// reproducible. (Reading padding is sound here: `zeroed()` initializes every
/// byte, field writes overwrite exactly their spans.)
fn bond_record_bytes(record: BondRecord) -> [u8; 56] {
    let mut uninit = core::mem::MaybeUninit::<BondRecord>::zeroed();
    let p = uninit.as_mut_ptr();
    unsafe {
        core::ptr::addr_of_mut!((*p).instrument).write(record.instrument);
        core::ptr::addr_of_mut!((*p).currency).write(record.currency);
        core::ptr::addr_of_mut!((*p).slot).write(record.slot);
        core::ptr::addr_of_mut!((*p).lex).write(record.lex);
        core::mem::transmute(uninit.assume_init())
    }
}

/// Byte-image of an `ArRecord` over zero-initialized storage (same argument).
fn ar_record_bytes(record: ArRecord) -> [u8; 56] {
    let mut uninit = core::mem::MaybeUninit::<ArRecord>::zeroed();
    let p = uninit.as_mut_ptr();
    unsafe {
        core::ptr::addr_of_mut!((*p).class).write(record.class);
        core::ptr::addr_of_mut!((*p).obligor_slot).write(record.obligor_slot);
        core::ptr::addr_of_mut!((*p).currency).write(record.currency);
        core::ptr::addr_of_mut!((*p).lex).write(record.lex);
        core::mem::transmute(uninit.assume_init())
    }
}

/// Court 5 (lane doc T12, Wave-1 slice): the real in-memory byte image of
/// each record shape equals the committed golden blob — on-disk == in-memory
/// (A06 §3), including endianness and zero padding.
///
/// Golden blobs are stated field-value -> byte-offset tables, not derived
/// from the struct under test, so a layout change flips this court.
///
/// Anti-vacuity mutation (T12-named): widening `currency` u16 -> u32 fails
/// this court's build (type mismatch in the canary construction) and the
/// `offset_of!`/`size_of` const asserts in `src/lib.rs` — the compile-gate
/// half re-witnessed in a /tmp sandbox shim-court (wave 5, byte-exact
/// restore); the runtime half is pinned in-repo by [`layout_court`].
#[test]
fn record_byte_round_trip_court() {
    // --- BondRecord golden: instrument@0, currency@8, slot@16, lex@24.
    let mut gold = [0u8; 56];
    gold[0..8].copy_from_slice(&CORP_BOND_SLOT.to_le_bytes());
    gold[8..10].copy_from_slice(&0x0348u16.to_le_bytes());
    gold[16..24].copy_from_slice(&CORP_BOND_SLOT.to_le_bytes());
    gold[24..56].copy_from_slice(&LEX_CORP_BOND);
    let canary = BondRecord {
        instrument: ClassSlot(CORP_BOND_SLOT),
        currency: FiboCurrency::Usd,
        slot: CORP_BOND_SLOT,
        lex: LEX_CORP_BOND,
    };
    assert_eq!(bond_record_bytes(canary), gold, "BondRecord byte image drifted");

    // --- ArRecord golden: class@0, obligor@8, currency@16, lex@18.
    let mut gold = [0u8; 56];
    gold[0..8].copy_from_slice(&ACCT_RECV_SLOT.to_le_bytes());
    gold[8..16].copy_from_slice(&USD_SLOT.to_le_bytes());
    gold[16..18].copy_from_slice(&0x034Au16.to_le_bytes());
    gold[18..50].copy_from_slice(&LEX_ACCT_RECV);
    let canary = ArRecord {
        class: ClassSlot(ACCT_RECV_SLOT),
        obligor_slot: USD_SLOT,
        currency: FiboCurrency::Jpy,
        lex: LEX_ACCT_RECV,
    };
    assert_eq!(ar_record_bytes(canary), gold, "ArRecord byte image drifted");
}
