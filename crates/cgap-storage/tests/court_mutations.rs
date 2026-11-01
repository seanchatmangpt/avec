//! A05 anti-vacuity court (lane doc §7.1): the sweep's green carries zero
//! bits unless each named mutation M1-M4 makes at least one enumerated case
//! produce visible corruption or a wrong-root return. "A mutation run in
//! which the sweep still passes is itself a FAIL — the court is vacuous."
//!
//! Mutations are typed in-module behavior variants ([`MutationMode`]) —
//! divergence D3 — so nothing self-edits at test time; the sha256
//! byte-exact proof for `src/atomic_commit.rs` around this court run is
//! carried in the lane receipt.

#![forbid(unsafe_code)]

mod common;

use common::{base_root, commit, flip_bit_file, new_payload, prior_payload, FileMedium};
use cgap_storage::atomic_commit::MutationMode::{
    M1HeaderCrcSkipped, M2LowestSeq, M3TrailerDigestSkipped, M4SeqRegressionUnchecked, Strict,
};
use cgap_storage::atomic_commit::*;

/// Scenario S2 medium: committed seq 1 (slot A) + commit of seq 2 cut at `k`.
fn s2_file(k: usize) -> FileMedium {
    let mut m = FileMedium::new(base_root());
    commit(&mut m, 1, &prior_payload());
    m.fence_artifacts();
    let _ = write_record_cut(&mut m, 2, &new_payload(), k);
    m
}

/// Aging medium: committed seq 1 (slot A) with one byte XOR-flipped.
fn aged_file(offset: usize, mask: u8) -> FileMedium {
    let mut m = FileMedium::new(base_root());
    commit(&mut m, 1, &prior_payload());
    flip_bit_file(&mut m, 0, offset, mask);
    m
}

#[test]
fn strict_is_the_production_default() {
    // recover() is recover_mode(Strict) everywhere it is called in the
    // courts; pin it directly on nontrivial media.
    for k in [0usize, 30, 60, 100, RECORD_SIZE] {
        let m = s2_file(k);
        assert_eq!(recover(&m), recover_mode(&m, Strict), "k={k}");
    }
    for offset in [0usize, 8, 28, 50, 96, RECORD_SIZE - 1] {
        let m = aged_file(offset, 0x01);
        assert_eq!(recover(&m), recover_mode(&m, Strict), "offset={offset}");
    }
}

#[test]
fn mutation_m1_header_crc_skipped_flips() {
    // M1: header-CRC failure treated as "skip to payload" instead of invalid.
    //
    // Flip locus (measured, see lane receipt): the header-CRC-only-protected
    // byte surface is SEQ (8..16) and the HEADER_CRC field itself (28..32) —
    // every other header byte is re-covered by magic/version/size/reserved
    // checks, and every payload byte by PAYLOAD_CRC + the split BLAKE3. On
    // the doc's expected locus ("torn-header offsets return garbage roots")
    // M1 does NOT flip: torn payloads fail PAYLOAD_CRC regardless, which is
    // the defense-in-depth property pinned by
    // `mutation_m1_write_sweep_pins_backstop` below.
    let mask = 0x01u8;
    let mut flips = 0usize;
    for offset in 0..RECORD_SIZE {
        let m = aged_file(offset, mask);
        let strict = recover(&m).unwrap();
        let mutated = recover_mode(&m, M1HeaderCrcSkipped).unwrap();
        let crc_only_surface = (8..16).contains(&offset) || (28..32).contains(&offset);
        if crc_only_surface {
            assert_eq!(
                strict.slot_verdicts[0],
                SlotVerdict::Damaged,
                "offset={offset}: strict must reject an aged record"
            );
            assert_ne!(
                mutated.root, strict.root,
                "offset={offset}: M1 admitted a CRC-failing record"
            );
            flips += 1;
        } else {
            assert_eq!(
                mutated, strict,
                "offset={offset}: M1 diverged outside the header-CRC-only surface"
            );
        }
    }
    assert!(flips > 0, "VACUOUS COURT: M1 flipped nowhere");
}

#[test]
fn mutation_m1_write_sweep_pins_backstop() {
    // On the torn-write sweep (C0-C4) M1 is fully absorbed: PAYLOAD_CRC plus
    // the split BLAKE3 backstop every state a torn header could hide. Pinned
    // as a design property — if this ever flips, the payload backstop eroded.
    for k in 0..=RECORD_SIZE {
        let m = s2_file(k);
        assert_eq!(
            recover_mode(&m, M1HeaderCrcSkipped),
            recover(&m),
            "k={k}: M1 must not diverge on torn-write states"
        );
    }
}

#[test]
fn mutation_m2_lowest_seq_flips() {
    // M2: picking the LOWEST valid sequence rolls the root back to the prior
    // transaction on every double-commit state. Flip: k = N (the C4 commit).
    let mut flips = 0usize;
    for k in 0..=RECORD_SIZE {
        let m = s2_file(k);
        let strict = recover(&m).unwrap();
        let mutated = recover_mode(&m, M2LowestSeq).unwrap();
        if mutated != strict {
            flips += 1;
            // Every flip must be the C4 case: M2 returns the stale prior root
            // where the fully valid new commit stands.
            assert_eq!(k, RECORD_SIZE, "unexpected M2 flip at k={k}");
            assert_eq!(strict.root, new_payload());
            assert_eq!(mutated.root, prior_payload());
        }
    }
    assert!(flips > 0, "VACUOUS COURT: M2 flipped nowhere");
}

#[test]
fn mutation_m3_trailer_digest_skipped_flips() {
    // M3: BLAKE3 trailer half dropped from validation → torn-tail offsets
    // (trailer digest bytes cut, k in 100..112) accept a record whose digest
    // tail was destroyed. Flip locus: k in 100..=RECORD_SIZE-1.
    let mut flips = 0usize;
    for k in 0..=RECORD_SIZE {
        let m = s2_file(k);
        let strict = recover(&m).unwrap();
        let mutated = recover_mode(&m, M3TrailerDigestSkipped).unwrap();
        if mutated != strict {
            flips += 1;
            assert!(
                (OFF_BLAKE3_TRAILER_ABS..RECORD_SIZE).contains(&k),
                "unexpected M3 flip at k={k}"
            );
            // Strict rejects the torn-tail record; the mutation admits it.
            assert_eq!(strict.slot_verdicts[1], SlotVerdict::Damaged, "k={k}");
            assert_eq!(mutated.slot_verdicts[1], SlotVerdict::Valid, "k={k}");
            assert_eq!(
                mutated.root,
                new_payload(),
                "k={k}: M3 must return the admitted torn-tail root"
            );
        }
    }
    assert!(flips > 0, "VACUOUS COURT: M3 flipped nowhere");
}

#[test]
fn mutation_m4_seq_regression_unchecked_flips() {
    // M4: SeqRegression check removed → a stale valid record rolls the dag
    // root back to base-or-below instead of being refused typed.
    let mut m = FlashMedium::new(base_root());
    m.fence_artifacts();
    let stale = NewRootDescriptor::new([0xDD; 20], 0x4000, 9, 0, 0); // dag 9 == base 9
    let bytes = encode_record(1, &stale);
    for (i, b) in bytes.iter().enumerate() {
        m.write_journal_byte(0, i, *b).unwrap();
    }
    assert_eq!(recover(&m), Err(RecoveryError::SeqRegression));
    // The flip: identical bytes — strict refuses typed, M4 applies the
    // regressing root. At least one of the two violates its required
    // semantics, and the strict outcome is the required one (A03).
    let out = recover_mode(&m, M4SeqRegressionUnchecked).unwrap();
    assert_eq!(out.root, stale, "M4 must admit the regressing record");
    assert_eq!(out.recovered_seq, Some(1));
    assert_eq!(out.report, RecoveryReport::Clean);
}
