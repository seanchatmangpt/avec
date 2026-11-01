//! A05 courts for the compound crash classes (lane doc §5 F1-F3 and §6 A03):
//! aging corruption, both-slots-damaged, same-seq ambiguity, and dag-seq
//! regression.
//!
//! Anti-vacuity: M1 (header-CRC skipped) flips on the aging space exercised
//! here (SEQ bytes are the header-CRC-only-protected surface); M4
//! (SeqRegression unchecked) flips on the regression case exercised here.
//! Both flips are witnessed in `court_mutations.rs`.

#![forbid(unsafe_code)]

mod common;

use common::{base_root, commit, flip_bit_file, new_payload, prior_payload, FileMedium};
use cgap_storage::atomic_commit::*;

#[test]
fn aging_corruption_flip() {
    // F1: a bit-flip inside a previously committed record invalidates it;
    // recovery falls back with total rollback of at most that one
    // transaction, zero corruption. Every byte of the committed record is
    // enumerated on both backends — a single surviving offset would be a
    // validator hole (the court flips on it by failing).
    let mask = 0x01u8;
    for backend in ["file", "flash"] {
        for offset in 0..RECORD_SIZE {
            let (out, flash) = match backend {
                "file" => {
                    let mut m = FileMedium::new(base_root());
                    commit(&mut m, 1, &prior_payload());
                    flip_bit_file(&mut m, 0, offset, mask);
                    (recover(&m).unwrap(), false)
                }
                "flash" => {
                    let mut m = FlashMedium::new(base_root());
                    commit(&mut m, 1, &prior_payload());
                    m.flip_bit(0, offset, mask).unwrap();
                    (recover(&m).unwrap(), true)
                }
                _ => unreachable!(),
            };
            let backend_s = if flash { "flash" } else { "file" };
            // At most the one transaction lost: base root stands, the aged
            // record is never applied.
            assert_eq!(
                out.root,
                base_root(),
                "{backend_s} F1 offset {offset}: aged record still applied"
            );
            assert_eq!(out.recovered_seq, None, "{backend_s} offset {offset}");
            assert_eq!(
                out.report,
                RecoveryReport::RecoveryDegraded,
                "{backend_s} offset {offset}: lost committed state must report degraded"
            );
            assert_eq!(out.slot_verdicts[0], SlotVerdict::Damaged);
            // I3: deterministic in the damaged bytes alone.
            let out2 = if flash {
                let mut m = FlashMedium::new(base_root());
                commit(&mut m, 1, &prior_payload());
                m.flip_bit(0, offset, mask).unwrap();
                recover(&m).unwrap()
            } else {
                let mut m = FileMedium::new(base_root());
                commit(&mut m, 1, &prior_payload());
                flip_bit_file(&mut m, 0, offset, mask);
                recover(&m).unwrap()
            };
            assert_eq!(out, out2, "{backend_s} offset {offset}: I3");
        }
    }
}

#[test]
fn sweep_both_slots() {
    // F2: cut during commit of seq 2 at k, PLUS earlier damage in slot A →
    // base root + RecoveryDegraded for every torn offset (divergence D2:
    // doc leaves the F2 k-range open; at k = N the C4 law wins — a fully
    // valid commit is honored, not discarded because the other slot aged).
    for k in 0..=RECORD_SIZE {
        let mut m = FileMedium::new(base_root());
        commit(&mut m, 1, &prior_payload());
        flip_bit_file(&mut m, 0, 0, 0x01); // age-damage the committed record
        m.fence_artifacts();
        let _ = write_record_cut(&mut m, 2, &new_payload(), k);
        let out = recover(&m).unwrap();
        if k < RECORD_SIZE {
            assert_eq!(out.root, base_root(), "F2 k={k}: base root must stand");
            assert_eq!(out.report, RecoveryReport::RecoveryDegraded, "k={k}");
            assert_eq!(out.recovered_seq, None, "k={k}");
        } else {
            // C4 precedence: seq 2 is fully valid, dag 11 > base 9.
            assert_eq!(out.root, new_payload(), "C4 k=N must honor the commit");
            assert_eq!(out.report, RecoveryReport::Clean);
            assert_eq!(out.recovered_seq, Some(2));
        }
    }
}

#[test]
fn both_slots_damaged_reports_degraded() {
    // F2 without a torn commit: both slots hold non-conforming bytes → base
    // root + degraded, typed and observable.
    let mut m = FileMedium::new(base_root());
    commit(&mut m, 1, &prior_payload());
    flip_bit_file(&mut m, 0, 0, 0x01);
    flip_bit_file(&mut m, 1, 0, 0x01); // stale/damaged slot B
    let out = recover(&m).unwrap();
    assert_eq!(out.root, base_root());
    assert_eq!(out.report, RecoveryReport::RecoveryDegraded);
    assert_eq!(out.slot_verdicts, [SlotVerdict::Damaged, SlotVerdict::Damaged]);
}

#[test]
fn ambiguous_refuses() {
    // F3: identical SEQ in both slots. The commit path derives the slot from
    // the seq, so the court writes the raw encoded record into slot B
    // explicitly — modeling a replayed/mirrored write at the storage layer.
    let raw_write_slot = |m: &mut FlashMedium, slot: usize, seq: u64, payload: &NewRootDescriptor| {
        let bytes = encode_record(seq, payload);
        for (i, b) in bytes.iter().enumerate() {
            m.write_journal_byte(slot, i, *b).unwrap();
        }
    };

    // Conflicting payloads → typed refusal, fail-closed, never a guess.
    let mut m = FlashMedium::new(base_root());
    m.fence_artifacts();
    let conflicting = NewRootDescriptor::new([0xCC; 20], 0x3000, 12, 0, 0);
    raw_write_slot(&mut m, 0, 1, &prior_payload());
    raw_write_slot(&mut m, 1, 1, &conflicting);
    assert_eq!(
        recover(&m),
        Err(RecoveryError::JournalAmbiguous),
        "same seq, different payload must refuse"
    );

    // Identical payloads (replayed write) → deterministic tie-break on
    // BLAKE3 equality: dedup, journal resolves normally.
    let mut m = FlashMedium::new(base_root());
    m.fence_artifacts();
    raw_write_slot(&mut m, 0, 1, &prior_payload());
    raw_write_slot(&mut m, 1, 1, &prior_payload());
    let out = recover(&m).unwrap();
    assert_eq!(out.root, prior_payload());
    assert_eq!(out.recovered_seq, Some(1));
    assert_eq!(out.report, RecoveryReport::Clean);
}

#[test]
fn seq_regression_refused() {
    // A03 contract: a valid record whose DAG_SEQ <= base dag seq is
    // counter-rollback-as-corruption → typed SeqRegression, fail-closed.
    for regressed in [8u64, 9] {
        let mut m = FlashMedium::new(base_root());
        m.fence_artifacts();
        let stale = NewRootDescriptor::new([0xDD; 20], 0x4000, regressed, 0, 0);
        write_record_cut(&mut m, 1, &stale, RECORD_SIZE).unwrap();
        assert_eq!(
            recover(&m),
            Err(RecoveryError::SeqRegression),
            "dag {regressed} <= base 9 must refuse"
        );
    }
    // Boundary: dag 10 > 9 resolves normally (the refusal is <=, strict).
    let mut m = FlashMedium::new(base_root());
    m.fence_artifacts();
    write_record_cut(&mut m, 1, &prior_payload(), RECORD_SIZE).unwrap();
    assert_eq!(recover(&m).unwrap().root, prior_payload());
}
