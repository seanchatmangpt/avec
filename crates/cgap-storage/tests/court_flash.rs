//! A05 court: flash erase semantics (lane doc §7 test 6 + §9 risks). The
//! flash model erases to 0xFF (not 0x00), enforces NOR AND-semantics on
//! writes, and erases progressively so cut-during-erase offsets are
//! enumerated like cut-during-write ones.
//!
//! Anti-vacuity: M3 (trailer digest skipped) flips on torn-tail offsets of
//! the same record space (witnessed in `court_mutations.rs`); this court
//! additionally flips on any recovery that assumed zero-fill (the all-zero
//! slot assertion below).

#![forbid(unsafe_code)]

mod common;

use common::{base_root, commit, new_payload, prior_payload};
use cgap_storage::atomic_commit::*;

#[test]
fn flash_erase_semantics() {
    // Fresh flash: both slots erased 0xFF → base root, CLEAN — erased is the
    // never-written state, not damage.
    let m = FlashMedium::new(base_root());
    let out = recover(&m).unwrap();
    assert_eq!(out.root, base_root());
    assert_eq!(out.report, RecoveryReport::Clean);
    assert_eq!(out.slot_verdicts, [SlotVerdict::Empty, SlotVerdict::Empty]);

    // Recovery must not depend on zero-fill (§7 test 6): an all-ZERO slot is
    // not a legal erased state on NOR flash — it is damage, and recovery
    // must say Degraded, never confuse 0x00-fill with emptiness.
    let mut m = FlashMedium::new(base_root());
    for i in 0..RECORD_SIZE {
        m.write_journal_byte(0, i, 0x00).unwrap(); // AND-semantics: 0xFF & 0x00 = 0x00
    }
    let out = recover(&m).unwrap();
    assert_eq!(out.root, base_root());
    assert_eq!(out.report, RecoveryReport::RecoveryDegraded);
    assert_eq!(out.slot_verdicts[0], SlotVerdict::Damaged);
}

#[test]
fn overwrite_requires_erase() {
    // NOR AND-semantics: overwriting slot A (seq 3 reuses slot A) without an
    // erase needs to SET bits (seq 1 -> 3) → typed NeedsErase, zero bytes
    // moved, prior record untouched. After the erase, the commit lands.
    let mut m = FlashMedium::new(base_root());
    commit(&mut m, 1, &prior_payload());
    m.fence_artifacts();

    let err = write_record_cut(&mut m, 3, &new_payload(), RECORD_SIZE);
    assert_eq!(err, Err(TxnError::Medium(MediumError::NeedsErase)));
    let out = recover(&m).unwrap();
    assert_eq!(out.root, prior_payload(), "refused overwrite moved no bytes");
    assert_eq!(out.recovered_seq, Some(1));

    m.erase_slot(slot_for_seq(3)).unwrap();
    write_record_cut(&mut m, 3, &new_payload(), RECORD_SIZE).unwrap();
    let out = recover(&m).unwrap();
    assert_eq!(out.root, new_payload());
    assert_eq!(out.recovered_seq, Some(3));
    assert_eq!(out.report, RecoveryReport::Clean);
}

#[test]
fn cut_during_erase_sweep() {
    // §9 risk: erase is per-block, and a power cut mid-erase leaves a
    // 0xFF/old mix. Every erase offset is enumerated.
    //
    // Two committed records (seq 1 in A, seq 2 in B): cutting the erase of
    // slot A (for the next seq-3 commit) must never touch the standing root
    // — slot B holds it.
    for e in 0..=RECORD_SIZE {
        let mut m = FlashMedium::new(base_root());
        commit(&mut m, 1, &prior_payload());
        commit(&mut m, 2, &new_payload());
        m.erase_prefix(0, e).unwrap();
        let out = recover(&m).unwrap();
        assert_eq!(out.root, new_payload(), "e={e}: seq 2 root must stand");
        assert_eq!(out.recovered_seq, Some(2), "e={e}");
        assert_eq!(out.report, RecoveryReport::Clean, "e={e}");
    }

    // Loss window, pinned as observed behavior (§9 names slot-rotation
    // cadence the open parameter that bounds it): with ONLY seq 1 committed,
    // cutting the erase of its own slot mid-way destroys the record —
    // base root + Degraded for partial erase; a COMPLETED erase (e = N)
    // returns the slot to the never-written state: base root, CLEAN (C0
    // equivalent), never Degraded.
    for e in 1..RECORD_SIZE {
        let mut m = FlashMedium::new(base_root());
        commit(&mut m, 1, &prior_payload());
        m.erase_prefix(0, e).unwrap();
        let out = recover(&m).unwrap();
        assert_eq!(out.root, base_root(), "e={e}");
        assert_eq!(out.report, RecoveryReport::RecoveryDegraded, "e={e}");
    }
    let mut m = FlashMedium::new(base_root());
    commit(&mut m, 1, &prior_payload());
    m.erase_prefix(0, RECORD_SIZE).unwrap();
    let out = recover(&m).unwrap();
    assert_eq!(out.root, base_root());
    assert_eq!(out.report, RecoveryReport::Clean, "fully erased = never written");
    assert_eq!(out.slot_verdicts[0], SlotVerdict::Empty);
}
