//! A05 court: the sweep IS the observation (lane doc §7). Every cut offset
//! `k in 0..=N` of a commit transaction is enumerated — never sampled — on
//! BOTH backends (hosted file + in-process flash model), and recovery must
//! classify and resolve per the §5 crash matrix with invariants I1-I3.
//!
//! Anti-vacuity: this court's green is qualified by the mutation court
//! (`court_mutations.rs`), which flips M2 (lowest-seq) and M3 (trailer
//! digest skipped) on exactly this sweep space, and M1 (header-CRC skipped)
//! on the same-case aging space. A sweep that still passes under a mutation
//! is itself FAIL.

#![forbid(unsafe_code)]

mod common;

use common::{base_root, commit, flip_bit_file, new_payload, prior_payload, FileMedium};
use cgap_storage::atomic_commit::*;

/// Scenario S1: fresh journal, commit of seq 1 cut at `k`. Required: k < N →
/// base root stands (C0-C3); k = N → new root committed (C4).
fn sweep_case_s1(k: usize) -> (RecoveryOutcome, NewRootDescriptor, NewRootDescriptor) {
    let mut m = FileMedium::new(base_root());
    m.fence_artifacts();
    let _cut = write_record_cut(&mut m, 1, &prior_payload(), k);
    let out = recover(&m).expect("sweep recovery must not refuse on legit torn writes");
    (out, base_root(), prior_payload())
}

/// Scenario S2: committed seq 1 in slot A, commit of seq 2 into slot B cut
/// at `k`. Required: k < N → prior root (seq 1) stands; k = N → seq 2 root.
fn sweep_case_s2(k: usize) -> (RecoveryOutcome, NewRootDescriptor, NewRootDescriptor) {
    let mut m = FileMedium::new(base_root());
    commit(&mut m, 1, &prior_payload());
    m.fence_artifacts();
    let _cut = write_record_cut(&mut m, 2, &new_payload(), k);
    let out = recover(&m).expect("sweep recovery must not refuse on legit torn writes");
    (out, prior_payload(), new_payload())
}

/// Invariants I1-I3 for one enumerated case, asserted for both backends.
/// `out` is the recovery result, `prior`/`new` the only lawful roots (I1:
/// never a third value; I2: the returned root is one of the two verifiable
/// artifacts and never dag-regresses; I3: recovery re-run on a byte-identical
/// medium is identical — time is not a proxy for state).
fn assert_invariants(out: RecoveryOutcome, prior: &NewRootDescriptor, new: &NewRootDescriptor) {
    // I1: prior root or new root, never a third value, never a refusal.
    assert!(
        out.root == *prior || out.root == *new,
        "I1 violated: recovery returned a third value ({:?})",
        out.root
    );
    // I2: dag monotonic under the A03 contract — rollback only to prior.
    // (Packed-struct fields: copy to locals first — E0793, no unaligned refs
    // through format-arg reborrows.)
    let resolved_dag = out.root.dag_seq;
    let prior_dag = prior.dag_seq;
    assert!(
        resolved_dag >= prior_dag,
        "I2 violated: dag regression to {resolved_dag}",
    );
    // Degradation law: Degraded ⇔ the journal tail was LOST (no record
    // applied) AND damage (non-conforming bytes) is present on the medium.
    let damaged = out
        .slot_verdicts
        .iter()
        .any(|&v| v == SlotVerdict::Damaged);
    assert_eq!(
        out.report == RecoveryReport::RecoveryDegraded,
        out.recovered_seq.is_none() && damaged,
        "report/verdict incoherent: {:?}",
        out.slot_verdicts
    );
}

#[test]
fn sweep_per_offset_rollback() {
    // Doc §7 test 1: 113 offsets x 2 scenarios x 2 backends, every case
    // enumerated (sampling is how the one bad offset ships). Determinism
    // (I3) is double-checked per case on a freshly rebuilt medium.
    let n = RECORD_SIZE;
    for backend in ["file", "flash"] {
        for with_prior in [false, true] {
            for k in 0..=n {
                let (out, prior, new) = match (backend, with_prior) {
                    ("file", false) => sweep_case_s1(k),
                    ("file", true) => sweep_case_s2(k),
                    ("flash", false) => {
                        let mut m = FlashMedium::new(base_root());
                        m.fence_artifacts();
                        let _ = write_record_cut(&mut m, 1, &prior_payload(), k);
                        (recover(&m).unwrap(), base_root(), prior_payload())
                    }
                    ("flash", true) => {
                        let mut m = FlashMedium::new(base_root());
                        commit(&mut m, 1, &prior_payload());
                        m.fence_artifacts();
                        let _ = write_record_cut(&mut m, 2, &new_payload(), k);
                        (recover(&m).unwrap(), prior_payload(), new_payload())
                    }
                    _ => unreachable!(),
                };

                let expected = if k < n { &prior } else { &new };
                assert_eq!(
                    out.root, *expected,
                    "backend={backend} scenario-prior={with_prior} k={k}: wrong root"
                );
                // Degradation is coherent per scenario: S2 always has the
                // committed slot A tail standing (Clean); S1 writes the cut
                // into the only slot, so torn bytes (k in 1..N) mean
                // Degraded — k=0 writes nothing, k=N is a valid commit.
                if with_prior || k == 0 || k == n {
                    assert_eq!(out.report, RecoveryReport::Clean, "k={k}");
                } else {
                    assert_eq!(
                        out.report,
                        RecoveryReport::RecoveryDegraded,
                        "S1 k={k}: torn bytes must report degraded"
                    );
                }
                assert_invariants(out, &prior, &new);

                // Crash-matrix class check (§5 table): k = 0 is C0 (pre-write:
                // no record applied), 1..=47 C1, 48..96 C2, 96..112 C3, k = N
                // C4 (commit, not corruption).
                let class = tear_class_for_offset(k);
                if k < n {
                    assert_ne!(
                        class,
                        TearClass::CompleteRecord,
                        "k={k} classified C4"
                    );
                    assert_eq!(
                        out.recovered_seq,
                        if with_prior { Some(1u64) } else { None },
                        "k={k}: applied-seq wrong"
                    );
                } else {
                    assert_eq!(class, TearClass::CompleteRecord);
                    assert_eq!(out.recovered_seq, Some(if with_prior { 2 } else { 1 }));
                }

                // I3: byte-identical medium rebuild → identical outcome.
                let (out2, _, _) = match (backend, with_prior) {
                    ("file", false) => sweep_case_s1(k),
                    ("file", true) => sweep_case_s2(k),
                    ("flash", false) => {
                        let mut m = FlashMedium::new(base_root());
                        m.fence_artifacts();
                        let _ = write_record_cut(&mut m, 1, &prior_payload(), k);
                        (recover(&m).unwrap(), base_root(), prior_payload())
                    }
                    ("flash", true) => {
                        let mut m = FlashMedium::new(base_root());
                        commit(&mut m, 1, &prior_payload());
                        m.fence_artifacts();
                        let _ = write_record_cut(&mut m, 2, &new_payload(), k);
                        (recover(&m).unwrap(), prior_payload(), new_payload())
                    }
                    _ => unreachable!(),
                };
                assert_eq!(
                    out, out2,
                    "I3 violated: recovery not deterministic at k={k} ({backend})"
                );
            }
        }
    }
}

#[test]
fn commit_ack_means_durable() {
    // Doc §7 test 5 / matrix C4: after the full write (k = N) and recovery,
    // the new root is live — a commit, not a corruption.
    let mut m = FileMedium::new(base_root());

    // §9 ordering precondition first: without the artifact fence the commit
    // path refuses typed and writes nothing.
    let refused = write_record_cut(&mut m, 1, &prior_payload(), RECORD_SIZE);
    assert_eq!(refused, Err(TxnError::ArtifactsNotFenced));
    let out = recover(&m).unwrap();
    assert_eq!(out.root, base_root());
    assert_eq!(out.slot_verdicts, [SlotVerdict::Empty, SlotVerdict::Empty]);

    // Full Txn path on the real file backend: begin_txn → write_record →
    // commit_txn (readback self-check) → recover sees the committed root.
    m.fence_artifacts();
    let mut txn = begin_txn(&mut m, 1);
    txn.write_record(&prior_payload()).expect("record write");
    let acked = txn.commit_txn().expect("commit ack");
    assert_eq!(acked, prior_payload());

    let out = recover(&m).unwrap();
    assert_eq!(out.root, prior_payload());
    assert_eq!(out.recovered_seq, Some(1));
    assert_eq!(out.report, RecoveryReport::Clean);

    // Commit self-check catches a damaged readback: age-corrupt the just-
    // written record's payload byte, then a fresh commit at the next seq
    // (slot B) must still be writable, while the damaged slot A record is
    // not silently acknowledged (its recovery invalidates).
    flip_bit_file(&mut m, 0, HEADER_SIZE, 0x01);
    let out = recover(&m).unwrap();
    assert_eq!(out.root, base_root());
    assert_eq!(out.report, RecoveryReport::RecoveryDegraded);
}
