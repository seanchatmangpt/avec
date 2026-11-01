//! Wave-4 hardening courts (lane A09) — lane doc §8 T7-T12, closed with
//! real collaborators: the committed pack bytes on disk
//! (`fibo_sec_dbt_v1.oabi`), A06's real loader (`cgap_bont::oabi`), the
//! real emitter binary (`CARGO_BIN_EXE_emit-fibo-pack`; the bin target was
//! renamed from the colliding `emit-pack` in the Wave-6 repair, see
//! `Cargo.toml`), and the real hash substrate (`cgap_blake3::hash`). No
//! mocks. Every court names its anti-vacuity mutation; a mutation witness
//! runs the court's predicate against a mutated real subject and asserts
//! it flips.

use std::path::PathBuf;
use std::process::Command;

use cgap_blake3::hash;
use cgap_bont::oabi::{OabiError, OabiPackV1};
use cgap_fibo::{
    ArRecord, BondRecord, ClassSlot, FiboCurrency, ACCT_RECV_SLOT, BOND_SLOT, CLASS_ACCT_RECV,
    CLASS_BOND, CLASS_CORP_BOND, CLASS_DEBT_INSTR, COUPON_RATE_SLOT, CORP_BOND_SLOT,
    DEBT_INSTR_SLOT, EUR_SLOT, JPY_SLOT, LEX_ACCT_RECV, LEX_BOND, LEX_COUPON_RATE,
    LEX_CORP_BOND, LEX_DEBT_INSTR, LEX_EUR, LEX_JPY, LEX_MATURITY_DATE, LEX_USD,
    MATURITY_DATE_SLOT, PACK_DIGEST, USD_SLOT,
};

const BONT_NS_V1: &[u8] = b"bont\x01";

fn pack_bytes() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fibo_sec_dbt_v1.oabi");
    std::fs::read(path).expect("committed pack must exist")
}

fn emitter() -> &'static str {
    env!("CARGO_BIN_EXE_emit-fibo-pack")
}

fn lex_hash(lexeme: &[u8]) -> [u8; 32] {
    let mut tagged = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    tagged.extend_from_slice(BONT_NS_V1);
    tagged.extend_from_slice(lexeme);
    hash(&tagged)
}

/// T7 — schema digest court (§8 T7): committed pack BLAKE3 == PACK_DIGEST.
/// Anti-vacuity: three named mutations, each witnessed flipping the
/// predicate in-process on real mutated bytes — (a) lex-byte flip, (b)
/// append-one-byte, (c) record swap.
#[test]
fn t7_pack_digest_court() {
    let bytes = pack_bytes();
    assert_eq!(bytes.len(), 64 + 9 * 48, "pack length drifted");
    assert_eq!(hash(&bytes), PACK_DIGEST, "pack digest drifted from PACK_DIGEST");

    // (a) flip one byte in record 0's LexHash field.
    let mut m = bytes.clone();
    m[64 + 0 * 48 + 16] ^= 0x01;
    assert_ne!(hash(&m), PACK_DIGEST, "mutation (a) lex-byte flip not discriminated");

    // (b) append one zero byte.
    let mut m = bytes.clone();
    m.push(0);
    assert_ne!(hash(&m), PACK_DIGEST, "mutation (b) byte append not discriminated");

    // (c) swap records 0 and 1 (48-byte blocks).
    let mut m = bytes.clone();
    let (r0, r1) = (64..112, 112..160);
    let b0 = bytes[r0.clone()].to_vec();
    let b1 = bytes[r1.clone()].to_vec();
    m[r0].copy_from_slice(&b1);
    m[r1].copy_from_slice(&b0);
    assert_ne!(hash(&m), PACK_DIGEST, "mutation (c) record swap not discriminated");
}

/// T8 — currency enum court, both directions (§8 T8): forward (frozen
/// discriminants) and reverse (the pack record whose EnumId is 0x0348
/// carries the USD lexeme's LexHash), through the real loader.
/// Anti-vacuity: "change Usd to 0x0349" fails the forward assert;
/// "swap Usd/Eur discriminants" fails the reverse join — witnessed by the
/// explicit not-equal joins below.
#[test]
fn t8_currency_court() {
    assert_eq!(FiboCurrency::Usd as u16, 0x0348);
    assert_eq!(FiboCurrency::Eur as u16, 0x0349);
    assert_eq!(FiboCurrency::Jpy as u16, 0x034A);

    let pack = parse_pack();
    let by_enum = |id: u16| -> Vec<&cgap_bont::oabi::OabiRecordV1> {
        pack.records().iter().filter(|r| r.enum_.0 == id).collect()
    };
    for (id, lex) in [(0x0348, &LEX_USD), (0x0349, &LEX_EUR), (0x034A, &LEX_JPY)] {
        let hits = by_enum(id);
        assert_eq!(hits.len(), 1, "EnumId {id:#06x}: expected exactly one record");
        assert_eq!(hits[0].lex.0, *lex, "EnumId {id:#06x}: LexHash join drifted");
    }
    // Reverse-join mutation witnesses: the USD record does not carry EUR's
    // discriminant and vice versa (the "swap Usd/Eur" mutation would fail
    // exactly here).
    let usd = by_enum(0x0348)[0];
    let eur = by_enum(0x0349)[0];
    assert_ne!(usd.lex.0, eur.lex.0, "USD/EUR records share a LexHash");
    assert_ne!(usd.lex.0, LEX_EUR, "swap mutation not discriminated (USD carries EUR lex)");
    assert_ne!(eur.lex.0, LEX_USD, "swap mutation not discriminated (EUR carries USD lex)");
}

fn parse_pack() -> OabiPackV1 {
    let bytes = pack_bytes();
    OabiPackV1::from_bytes(&bytes).expect("committed pack must load through A06's reader")
}

/// Golden table (T9): (SlotId, LexHash, ClassId, EnumId) per corpus term,
/// in frozen harvest order (lane doc §4 = Wave-1 corpus order). Restated
/// independently here — the corpus list is part of the court, not shared
/// code with the emitter or the Wave-1 witness (independent restatement is
/// the drift detector).
const GOLDEN: &[(u64, [u8; 32], u32, u16)] = &[
    (USD_SLOT, LEX_USD, 0x0000_0000, 0x0348),
    (EUR_SLOT, LEX_EUR, 0x0000_0000, 0x0349),
    (JPY_SLOT, LEX_JPY, 0x0000_0000, 0x034A),
    (CORP_BOND_SLOT, LEX_CORP_BOND, CLASS_CORP_BOND, 0),
    (BOND_SLOT, LEX_BOND, CLASS_BOND, 0),
    (DEBT_INSTR_SLOT, LEX_DEBT_INSTR, CLASS_DEBT_INSTR, 0),
    (ACCT_RECV_SLOT, LEX_ACCT_RECV, CLASS_ACCT_RECV, 0),
    (MATURITY_DATE_SLOT, LEX_MATURITY_DATE, 0x0000_0020, 0),
    (COUPON_RATE_SLOT, LEX_COUPON_RATE, 0x0000_0021, 0),
];

/// T9 — determinism court (§8 T9, A06 T1 applied to the FIBO corpus): the
/// pack records equal the golden table record-for-record in harvest order,
/// every derivation re-hashes through the real substrate, and the real
/// emitter regenerates the committed pack byte-for-byte (`--check`).
/// Anti-vacuity: a truncation-window shift or namespace-tag drop changes
/// the derivations (inherited from A06 T1 semantics — witnessed at A06's
/// courts); here witnessed additionally by the committed-consts join: any
/// drifted const fails the live re-derivation below.
#[test]
fn t9_determinism_court() {
    let pack = parse_pack();
    assert_eq!(pack.records().len(), GOLDEN.len(), "record count drifted");
    for (i, ((slot, lex, class, enum_), rec)) in GOLDEN.iter().zip(pack.records()).enumerate() {
        let live = lex_hash(lexeme_of(i));
        assert_eq!(rec.slot.0, *slot, "record {i}: SlotId drift");
        assert_eq!(rec.lex.0, *lex, "record {i}: LexHash drift");
        assert_eq!(rec.class.0, *class, "record {i}: ClassId drift");
        assert_eq!(rec.enum_.0, *enum_, "record {i}: EnumId drift");
        assert_eq!(rec.slot.0, u64::from_le_bytes(rec.lex.0[0..8].try_into().unwrap()));
        assert_eq!(&live, lex, "record {i}: live re-derivation drifted");
    }
}

/// The frozen lexeme per golden row (harvest order); restated independently.
fn lexeme_of(i: usize) -> &'static [u8] {
    const LEXEMES: &[&[u8]] = &[
        b"fibo:FndUtlCur:USD",
        b"fibo:FndUtlCur:EUR",
        b"fibo:FndUtlCur:JPY",
        b"fibo:SecDbt:CorporateBond",
        b"fibo:SecDbt:Bond",
        b"fibo:SecDbt:DebtInstrument",
        b"fibo:ClAcct:AccountsReceivable",
        b"fibo:SecDbt:hasMaturityDate",
        b"fibo:SecDbt:hasCouponRate",
    ];
    LEXEMES[i]
}

/// T9b — emitter byte-determinism: `--check` regenerates the pack and
/// compares byte-for-byte with the committed file (real subprocess).
#[test]
fn t9b_emitter_check_court() {
    let out = Command::new(emitter()).arg("--check").output().expect("spawn emit-pack");
    assert_eq!(out.status.code(), Some(0), "emit-pack --check exit drift");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("OK 496 bytes"), "unexpected --check output: {stdout}");
}

/// T9c — default emit mode (the bare producer path, wave 5): `emit-pack`
/// with no arguments finds the committed pack byte-exact and reports
/// "(unchanged)", exit 0. The committed file exists here, so this court
/// never writes; a drifted committed pack would exit 4 on this exact path.
/// Anti-vacuity: that drift flip is witnessed against a mutated copy of
/// the pack in a /tmp sandbox shim-court (wave 5, byte-exact restore) —
/// mutating the canonical pack bytes is a checkout-global transition no
/// lane may perform.
#[test]
fn t9c_default_emit_mode_court() {
    let out = Command::new(emitter()).output().expect("spawn emit-pack");
    assert_eq!(out.status.code(), Some(0), "default emit mode exit drift");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("(unchanged)"), "unexpected emit output: {stdout}");
    assert!(stdout.contains("OK 496 bytes"), "unexpected emit output: {stdout}");
}

/// T10 — loader round-trip through A06's reader (§8 T10): the real pack
/// bytes load through `OabiPackV1::from_bytes`, header fields match the
/// frozen container, every record re-derives its SlotId, and the ClassId
/// set is exactly the frozen dense space.
/// Anti-vacuity: three typed-refusal mutations witnessed — corrupted SlotId
/// byte -> SlotDerivationMismatch, out-of-range ClassId ->
/// ClassOutOfRange, nonzero reserved -> ReservedNonZero.
#[test]
fn t10_loader_court() {
    let pack = parse_pack();
    let h = pack.header();
    assert_eq!(h.magic, *b"OABIv1\0\0");
    assert_eq!(h.header_len, 64);
    assert_eq!(h.version_major, 1);
    assert_eq!(h.version_minor, 0);
    assert_eq!(h.flags, 0b11, "LE | LexHash-present flags must be set");
    assert_eq!(h.class_count, 0x0022, "class_count must cover the frozen dense space");
    assert_eq!(h.enum_count, 0x034B, "enum_count must cover the frozen EnumId space");
    assert_eq!(h.slot_count, GOLDEN.len() as u32);
    assert_eq!(h.record_size, 48);
    assert!(h.reserved.iter().all(|&b| b == 0));

    let allowed: &[u32] =
        &[0x0000_0000, 0x0000_0001, 0x0000_0002, 0x0000_0003, 0x0000_0010, 0x0000_0020, 0x0000_0021];
    for (i, rec) in pack.records().iter().enumerate() {
        assert!(allowed.contains(&rec.class.0), "record {i}: ClassId outside frozen space");
        rec.verify_slot_derivation().expect("per-record derivation must hold");
    }

    // Mutation 1: corrupt record 3's SlotId byte -> typed refusal.
    let mut m = pack_bytes();
    m[64 + 3 * 48] ^= 0x01;
    match OabiPackV1::from_bytes(&m) {
        Err(OabiError::SlotDerivationMismatch { index, .. }) => assert_eq!(index, 3),
        other => panic!("mutation 1 not refused as SlotDerivationMismatch: {other:?}"),
    }

    // Mutation 2: record 0's ClassId -> 0xFF... (out of range) -> typed.
    let mut m = pack_bytes();
    m[64 + 0 * 48 + 11] = 0xFF;
    match OabiPackV1::from_bytes(&m) {
        Err(OabiError::ClassOutOfRange { index, .. }) => assert_eq!(index, 0),
        other => panic!("mutation 2 not refused as ClassOutOfRange: {other:?}"),
    }

    // Mutation 3: nonzero reserved byte -> typed.
    let mut m = pack_bytes();
    m[40] = 1;
    match OabiPackV1::from_bytes(&m) {
        Err(OabiError::ReservedNonZero { at_offset }) => assert_eq!(at_offset, 40),
        other => panic!("mutation 3 not refused as ReservedNonZero: {other:?}"),
    }
}

/// Refusal-list probe corpus (§2.2 families) — court-owned since Wave 4
/// (restated independently; the emitter carries no copy — see the Wave-7
/// quarantine court below), so drift between this list and the emitter's
/// module gate is itself a failing assert.
const REFUSED_PROBES: &[&[u8]] = &[
    b"fibo:SecEqu:Equity",         // §2.2 equities
    b"fibo:SecDerv:VanillaOption", // §2.2 derivatives/options
    b"fibo:Loan:ServicingDetail",  // §2.2 loans/loan-servicing
    b"fibo:Be:RegulatoryReport",   // §2.2 regulatory/reporting
    b"fibo:FndRole:AgentRole",     // §2.2 full FND foundations
    b"equity:raw",                 // non-fibo lexeme shape
];

/// T11 allowed-module control — NOT in the frozen corpus; must classify
/// ALLOWED: a gate that refuses everything is the other vacuous gate.
const ALLOWED_CONTROL: &[u8] = b"fibo:SecDbt:PerpetualBond";

/// T11 — refusal-list court (§8 T11), through the real emitter binary:
/// (1) every refused-family probe argv-supplied via `--probe` is refused
/// (typed TermRefused naming §2.2, exit 3) — Wave-7 quarantine repair: the
/// probes live HERE, never in the producer binary; (2) the literal
/// lane-doc mutation "add `fibo:SecEqu:Equity` to the corpus" (`--inject`)
/// fails the emit with a typed TermRefused naming §2.2; (3) injecting an
/// allowed term is admitted (the gate is not a refuse-everything vacuity);
/// (4) no committed pack record carries any refused lexeme's LexHash.
#[test]
fn t11_refusal_list_court() {
    // (1) argv probes: each refused probe -> exit 3, typed §2.2 refusal.
    for lexeme in REFUSED_PROBES {
        let lex = String::from_utf8_lossy(lexeme);
        let out = Command::new(emitter())
            .args(["--probe", lex.as_ref()])
            .output()
            .expect("spawn emit-pack");
        assert_eq!(out.status.code(), Some(3), "probe {lex:?} must be refused");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("[REFUSED]"), "probe {lex:?} not reported refused: {stdout}");
        assert!(stdout.contains("TermRefused"), "probe {lex:?} refusal must be typed: {stdout}");
        assert!(stdout.contains("section=2.2"), "probe {lex:?} refusal must name §2.2: {stdout}");
    }

    // The allowed control is admitted — the gate is not refuse-everything.
    let out = Command::new(emitter())
        .args(["--probe", core::str::from_utf8(ALLOWED_CONTROL).unwrap()])
        .output()
        .expect("spawn emit-pack");
    assert_eq!(out.status.code(), Some(0), "allowed control must be admitted");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("[ALLOWED] fibo:SecDbt:PerpetualBond (control)"), "{stdout}");

    // (2) the literal T11 mutation: inject an equities term into the emit.
    let out = Command::new(emitter())
        .args(["--inject", "fibo:SecEqu:Equity"])
        .output()
        .expect("spawn emit-pack");
    assert_eq!(out.status.code(), Some(3), "injected refused term must fail the emit");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TermRefused"), "typed error must be printed: {stdout}");
    assert!(stdout.contains("module=SecEqu"), "refusal must name the module: {stdout}");
    assert!(stdout.contains("section=2.2"), "refusal must name §2.2: {stdout}");

    // (3) the control inject is admitted (gate is not refuse-everything).
    let out = Command::new(emitter())
        .args(["--inject", "fibo:SecDbt:PerpetualBond"])
        .output()
        .expect("spawn emit-pack");
    assert_eq!(out.status.code(), Some(0), "allowed control inject must be admitted");

    // (4) no committed record carries a refused lexeme's LexHash.
    let pack = parse_pack();
    for lexeme in REFUSED_PROBES {
        let h = lex_hash(lexeme);
        for (i, rec) in pack.records().iter().enumerate() {
            assert_ne!(rec.lex.0, h, "record {i} carries refused lexeme's LexHash");
        }
    }
}

/// T13 — Wave-7 quarantine court (a07-wave6-audit-receipt.md §5 option
/// (b), producer-side): the REAL built emitter binary
/// (`CARGO_BIN_EXE_emit-fibo-pack`) carries ZERO refused-family lexemes
/// and ZERO probe scaffolding in any of its sections. Anti-vacuity,
/// three witnesses on real bytes, all inside this court:
///   (a) positive control 1 — the same scan DOES find a frozen corpus
///       lexeme in the binary (the scan reads the real artifact; if the
///       corpus ever moved out too, this control fails loudly and the
///       gate's keyed EMITTER_BINARIES exemption must be revisited);
///   (b) positive control 2 — the same scan finds the refused lexemes in
///       THIS test binary's own bytes (they live here — the predicate
///       discriminates, it is not a no-op matcher);
///   (c) flip witness — the pre-repair Wave-6 artifact (sha256
///       83c6eeb201139cb340b469b4555098e868745b0fbba152d73a96880a53f82067)
///       contained the refused lexemes (predicate FAILS there); the
///       repaired binary does not (predicate PASSES). No A09 wave-7 lane
///       receipt file exists on disk (the a08-wave7 audit receipt
///       registers exactly this absent-artifact class as D-A08-1), so the
///       original witness is recorded as HISTORY here and the flip is
///       re-executed per repair wave with the named mutation class in a
///       /tmp sandbox: embed a refused lexeme in the emitter's REFERENCED
///       data (the Wave-2..6 corpus shape — an UNUSED const is NOT
///       codegen'd into the binary and flips nothing, witnessed
///       2026-11-01: a planted unused `const` passes, a planted corpus
///       `Term` row fails this court at exit 101), rebuild, this court's
///       predicate flips to FAIL, restore sha256-verified. Wave-8
///       re-witness: executed 2026-11-01, flip witnessed. The court pins
///       the repaired side in-repo.
/// Named mutation class that would fail this court: re-adding a refused
/// probe const to `src/bin/emit-pack.rs` (the Wave-2..6 shape).
#[test]
fn t13_producer_quarantine_court() {
    let bin = std::fs::read(emitter()).expect("real emitter binary must be readable");
    assert!(bin.len() > 100_000, "implausibly small emitter binary");

    let contains = |needle: &[u8]| -> bool { window_contains(&bin, needle) };

    // (a) positive control 1: the frozen corpus IS in the binary.
    assert!(
        contains(b"fibo:FndUtlCur:USD"),
        "scan predicate vacuous: corpus lexeme absent from emitter binary"
    );
    assert!(
        contains(b"fibo:SecDbt:CorporateBond"),
        "scan predicate vacuous: corpus lexeme absent from emitter binary"
    );

    // The repair: ZERO refused lexemes in the producer binary.
    for lexeme in REFUSED_PROBES {
        assert!(
            !contains(lexeme),
            "producer quarantine violated: {:?} embedded in the emitter binary",
            String::from_utf8_lossy(lexeme)
        );
    }
    // Probe scaffolding is gone with it.
    assert!(!contains(b"<probe corpus>"), "probe scaffolding still embedded");
    assert!(!contains(b"--probe-refused"), "retired batch probe mode still advertised");

    // (b) positive control 2: the refused lexemes DO live in this test
    // binary's bytes (court-owned corpus; the predicate discriminates).
    let test_bin =
        std::fs::read(std::env::current_exe().expect("test exe path")).expect("test binary");
    let test_contains = |needle: &[u8]| -> bool { window_contains(&test_bin, needle) };
    for lexeme in REFUSED_PROBES {
        assert!(
            test_contains(lexeme),
            "scan predicate vacuous: refused lexeme absent even from the court's own binary"
        );
    }
}

/// Substring search over arbitrary bytes (the gate's own predicate shape:
/// plain `needle in haystack` over section bytes — here over the whole
/// file, a superset, so a section-scoped miss cannot be a false pass).
fn window_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Deterministic byte image of a `#[repr(C)]` record over zeroed storage
/// (padding deterministically zero; field writes overwrite their spans).
fn record_image<F>(write_fields: F) -> [u8; 56]
where
    F: FnOnce(*mut u8),
{
    let mut uninit = core::mem::MaybeUninit::<[u8; 56]>::zeroed();
    // The closure body carries its own `unsafe` blocks (raw-pointer writes);
    // this call itself is safe, so no outer `unsafe` here.
    write_fields(uninit.as_mut_ptr() as *mut u8);
    unsafe { uninit.assume_init() }
}

/// T12 — record-shape court, pack-join half (§8 T12): BondRecord and
/// ArRecord constructed FROM loaded pack rows have exactly the golden
/// byte images (offset table stated independently of the struct layout).
/// The compile-gate widening mutation is a real compile abort: widening
/// `currency` u16 -> u32 (FiboCurrency repr u16 -> u32) moves ArRecord.lex
/// to offset 20 and fails both the `size_of::<FiboCurrency>() == 2` and
/// `offset_of!(ArRecord, lex) == 18` const asserts in `src/lib.rs`
/// (re-witnessed in a /tmp sandbox shim-court, wave 5, byte-exact restore;
/// the runtime half is pinned in-repo by `layout_court` in
/// tests/wave1_witness.rs).
#[test]
fn t12_record_shape_court() {
    let pack = parse_pack();

    // BondRecord golden: instrument@0, currency@8, slot@16, lex@24.
    let corp = &pack.records()[3];
    assert_eq!(corp.slot.0, CORP_BOND_SLOT);
    let bond = BondRecord {
        instrument: ClassSlot(corp.slot.0),
        currency: FiboCurrency::Usd,
        slot: corp.slot.0,
        lex: corp.lex.0,
    };
    let gold = record_image(|p| unsafe {
        core::ptr::write(p as *mut u64, corp.slot.0);
        core::ptr::write(p.add(8) as *mut u16, 0x0348);
        core::ptr::write(p.add(16) as *mut u64, corp.slot.0);
        core::ptr::copy_nonoverlapping(corp.lex.0.as_ptr(), p.add(24), 32);
    });
    let mut image = [0u8; 56];
    image.copy_from_slice(unsafe {
        core::slice::from_raw_parts((&bond as *const BondRecord) as *const u8, 56)
    });
    assert_eq!(image, gold, "BondRecord byte image drifted");

    // ArRecord golden: class@0, obligor@8, currency@16, lex@18.
    let recv = &pack.records()[6];
    assert_eq!(recv.slot.0, ACCT_RECV_SLOT);
    let ar = ArRecord {
        class: ClassSlot(recv.slot.0),
        obligor_slot: USD_SLOT,
        currency: FiboCurrency::Jpy,
        lex: recv.lex.0,
    };
    let gold = record_image(|p| unsafe {
        core::ptr::write(p as *mut u64, recv.slot.0);
        core::ptr::write(p.add(8) as *mut u64, USD_SLOT);
        core::ptr::write(p.add(16) as *mut u16, 0x034A);
        core::ptr::copy_nonoverlapping(recv.lex.0.as_ptr(), p.add(18), 32);
    });
    let mut image = [0u8; 56];
    image.copy_from_slice(unsafe {
        core::slice::from_raw_parts((&ar as *const ArRecord) as *const u8, 56)
    });
    assert_eq!(image, gold, "ArRecord byte image drifted");
}
