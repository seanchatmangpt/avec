//! Lane A16 receipt courts (`docs/jira/v26.11.1/lanes/A16-chatman-pqc.md`
//! §3 byte semantics, §6 engine, §8.1/§8.3/§8.4/§8.5 courts + wave-6
//! hostile-input court).
//!
//! Anchors: official BLAKE3 `test_vectors.json` (key `"whats the Elvish word
//! for friend"`); `b3sum --keyed` validated byte-identical to that corpus on
//! the overlapped lengths and used to fill the production 192-byte vector;
//! `cgap_blake3::hash` as the real unkeyed kernel for Merkle cross-checks.
//! No mocks anywhere.
//!
//! Anti-vacuity flip receipts (REPLAYABLE): executed by
//! `scripts/wave7_shim_battery.py` (in-repo; builds a fresh sandbox copy,
//! requires a green baseline, applies one exact-string shim at a time,
//! restores sha256 byte-exact after every mutation). Latest full run
//! 2026-11-01 wave 7, exit 0, sandbox == repo after battery: M1 flips
//! roundtrip + phi-monotonicity + seal-exhaustion probe; M4 the byte sweep
//! (isolated); M5b chaining + byte sweep + pin + phi-monotonicity +
//! roundtrip; M5 key-ignore flips KAT + digest-coverage + pin but NOT
//! chaining (epoch-swap detection is structural — robustness positive); M6
//! determinism + pin; M7 hostile (isolated); M8 sealer sweep (isolated).
//! Every run exits 101 under every mutation; zero vacuous courts.

use cgap_crypto::chatman::{
    admissions_root, keyed_digest, seal_unsealed, verify_unsealed, DeltaError, DeltaS,
    DigestError, Sealer, Unsealed, UnsealedReceipt, VerifyError, ALGORITHM_ID_UNSEALED,
    DELTA_DOMAIN_TAG, DELTA_LEN, DIGEST_LEN, KEYED_MAX_INPUT_LEN, RECEIPT_LEN,
    UNSEALED_RECEIPT_LEN,
};
use cgap_crypto::lease::{Lease, PhiLease, PHI_LEASE_LEN};

/// Official-corpus inputs: byte i = i % 251.
fn corpus(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

// ----------------------------------------------------------------
// Keyed-digest KAT court (§3.3: the digest IS the lease-bound seal)
// ----------------------------------------------------------------

/// Official `keyed_hash` vectors. `(len, hex)`; lengths 0/1/63/64/1024 are
/// published values; 192 is the production digest-input size
/// (`DeltaS || Phi_lease`), filled by `b3sum --keyed` after b3sum matched the
/// published corpus byte-identically on 0/1/63/64/1024 this session.
const KEYED_KATS: &[(usize, &str)] = &[
    (0, "92b2b75604ed3c761f9d6f62392c8a9227ad0ea3f09573e783f1498a4ed60d26"),
    (1, "6d7878dfff2f485635d39013278ae14f1454b8c0a3a2d34bc1ab38228a80c95b"),
    (63, "bb1eb5d4afa793c1ebdd9fb08def6c36d10096986ae0cfe148cd101170ce37ae"),
    (64, "ba8ced36f327700d213f120b1a207a3b8c04330528586f414d09f2f7d9ccb7e6"),
    (192, "7e39f351b749ca160f515bfb1a934d951eb1373da2cac1ae65618b83694dad12"),
    (1024, "75c46f6f3d9eb4f55ecaaee480db732e6c2105546f1e675003687c31719c7ba4"),
];

const OFFICIAL_KEY: &[u8; 32] = b"whats the Elvish word for friend";

#[test]
fn keyed_digest_matches_official_blake3_vectors() {
    for (len, expected_hex) in KEYED_KATS {
        let input = corpus(*len);
        let got = keyed_digest(OFFICIAL_KEY, &input)
            .unwrap_or_else(|e| panic!("keyed_digest failed at len={len}: {e}"));
        let got_hex: String = got.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            got_hex, *expected_hex,
            "official keyed vector mismatch at len={len}"
        );
    }
}

/// Second-key vector (32 zero bytes over the production 192-byte shape):
/// catches key-handling bugs (e.g. key treated as data). b3sum --keyed,
/// session receipt.
#[test]
fn keyed_digest_second_key_vector() {
    let got = keyed_digest(&[0u8; 32], &corpus(192)).expect("192 B within bound");
    let got_hex: String = got.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        got_hex, "aec7e50a9d3ec3d18f829aebf26e1bfe2d1ca3b1ccd2523df4977cd97b0c2bbf",
        "second-key keyed vector mismatch"
    );
}

/// Loud UNSUPPORTED above the single-chunk bound — never a silent
/// mis-hash with a truncated tree.
#[test]
fn keyed_digest_rejects_over_single_chunk_loudly() {
    let err = keyed_digest(OFFICIAL_KEY, &corpus(1025))
        .expect_err("1025 B must be rejected");
    assert_eq!(
        err,
        DigestError::InputTooLong {
            len: 1025,
            max: KEYED_MAX_INPUT_LEN
        }
    );
    // Exactly at the bound is still fine.
    assert!(keyed_digest(OFFICIAL_KEY, &corpus(1024)).is_ok());
}

// ----------------------------------------------------------------
// §3.1 delta codec
// ----------------------------------------------------------------

#[test]
fn delta_codec_roundtrip_and_domain_tag_enforcement() {
    let delta = DeltaS::new([1u8; 32], [2u8; 32], 77, [3u8; 32]);
    let bytes = delta.to_bytes();
    assert_eq!(bytes.len(), DELTA_LEN);
    assert_eq!(&bytes[0..8], b"CGAPDLT1", "§3.1 domain tag");
    assert_eq!(&bytes[8..40], &[1u8; 32]);
    assert_eq!(&bytes[40..72], &[2u8; 32]);
    assert_eq!(bytes[72..80], [77, 0, 0, 0, 0, 0, 0, 0], "op_count LE");
    assert_eq!(&bytes[80..112], &[3u8; 32]);
    assert_eq!(DeltaS::from_bytes(&bytes).expect("roundtrip"), delta);

    // Foreign domain tag refused (cross-protocol reuse refused, §3.1).
    let mut foreign = bytes;
    foreign[7] = b'2';
    assert_eq!(
        DeltaS::from_bytes(&foreign),
        Err(DeltaError::BadDomainTag(*b"CGAPDLT2"))
    );
    assert_eq!(
        DeltaS::new([0u8; 32], [0u8; 32], 0, [0u8; 32]).domain_tag,
        DELTA_DOMAIN_TAG
    );
}

// ----------------------------------------------------------------
// §6 engine: seal / verify
// ----------------------------------------------------------------

fn fixture_delta(admissions: [u8; 32]) -> DeltaS {
    DeltaS::new([0x11u8; 32], [0x22u8; 32], 3, admissions)
}

fn fixture_lease() -> Lease {
    Lease::new([0xA5u8; 32], 64, [0x5Au8; 32])
}

#[test]
fn seal_then_verify_roundtrip() {
    let mut lease = fixture_lease();
    let receipt = seal_unsealed(fixture_delta([0u8; 32]), &mut lease);
    assert_eq!(verify_unsealed(&receipt), Ok(()));
    assert_eq!(receipt.magic, *b"CGRU");
    assert_eq!(receipt.version, 0x00);
    assert_eq!(lease.current_tick(), 1, "§6: seal is the only tick consumer");
}

/// §6 digest coverage, recomputed in-test from the public codec surfaces:
/// `blake3_digest == keyed_digest(phi.issued_root, delta || phi)`; a foreign
/// issued_root (or any covered byte change) recomputes differently — the
/// digest is genuinely lease-bound (§3.3).
#[test]
fn digest_coverage_is_exactly_delta_concat_phi_keyed_by_issued_root() {
    let mut lease = fixture_lease();
    let delta = fixture_delta([0x33u8; 32]);
    let receipt = seal_unsealed(delta, &mut lease);
    let phi: PhiLease = receipt.phi;

    let mut covered = [0u8; DELTA_LEN + PHI_LEASE_LEN];
    covered[0..DELTA_LEN].copy_from_slice(&delta.to_bytes());
    covered[DELTA_LEN..].copy_from_slice(&phi.to_bytes());
    let expected = keyed_digest(&phi.issued_root, &covered).expect("192 B");
    assert_eq!(receipt.blake3_digest, expected);

    // Foreign epoch: different key -> different digest (lease-bound, §3.3).
    let foreign = keyed_digest(&[0xEEu8; 32], &covered).expect("192 B");
    assert_ne!(foreign, expected);

    // Any covered byte change is visible to the digest (spot checks; the
    // exhaustive sweep is the byte_sweep_court).
    for (start, len) in [(0usize, DELTA_LEN), (DELTA_LEN, PHI_LEASE_LEN)] {
        let mut mutated = covered;
        mutated[start + len / 2] ^= 0x01;
        assert_ne!(
            keyed_digest(&phi.issued_root, &mutated).expect("192 B"),
            expected,
            "covered byte at {start}+{} must change the digest",
            len / 2
        );
    }
}

/// End-to-end receipt-bytes KAT for a fixed fixture — determinism across
/// runs/machines at the byte level. (Pinned from this session's run; the
/// composition is courted elsewhere, this pins drift.)
#[test]
fn full_receipt_bytes_are_pinned_end_to_end() {
    let mut lease = Lease::new([7u8; 32], 8, [9u8; 32]);
    let receipt = seal_unsealed(DeltaS::new([1u8; 32], [2u8; 32], 1, [3u8; 32]), &mut lease);
    let bytes = receipt.to_bytes();
    assert_eq!(bytes.len(), UNSEALED_RECEIPT_LEN);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // Single-line pin (458 hex chars) — no stitching, no ambiguity.
    assert_eq!(
        hex,
        "434752550043474150444c54310101010101010101010101010101010101010101010101010101010101010101020202020202020202020202020202020202020202020202020202020202020201000000000000000303030303030303030303030303030303030303030303030303030303030303070707070707070707070707070707070707070707070707070707070707070700000000000000000800000000000000090909090909090909090909090909090909090909090909090909090909090949b8c6fe2aa3263f5a51e9fcea2642dcdace905fc22525487526787b637e48d1",
        "end-to-end receipt bytes drifted (fixture: lease [7;32]/8/[9;32], \
         delta roots [1;32]->[2;32] op_count 1 admissions [3;32], first seal)"
    );
    assert_eq!(verify_unsealed(&receipt), Ok(()));
}

/// §8.1 (phi side): N seals -> strictly increasing phi ticks, pairwise-distinct
/// 80-byte `Phi_lease` strings, exactly one tick per receipt.
#[test]
fn monotonicity_court_phi_sequence_strictly_increasing_and_distinct() {
    let mut lease = fixture_lease();
    let mut previous_tick: Option<u64> = None;
    let mut phis: Vec<[u8; PHI_LEASE_LEN]> = Vec::new();
    for i in 0..8u64 {
        let receipt = seal_unsealed(fixture_delta([i as u8; 32]), &mut lease);
        let phi_tick: u64 = receipt.phi.tick;
        assert_eq!(phi_tick, i, "phi tick must be the pre-advance value");
        if let Some(prev) = previous_tick {
            assert!(phi_tick > prev, "§8.1 strictly increasing");
        }
        previous_tick = Some(receipt.phi.tick);
        let phi_snapshot: PhiLease = receipt.phi;
        let phi_bytes = phi_snapshot.to_bytes();
        assert!(
            phis.iter().all(|p| p != &phi_bytes),
            "§8.1 each Phi_lease byte string differs"
        );
        phis.push(phi_bytes);
        assert_eq!(verify_unsealed(&receipt), Ok(()));
    }
    assert_eq!(lease.current_tick(), 8);
}

/// §8.2 via the seal path: sealing against an exhausted lease TRAPS the
/// process (the only-tick-consumer path hits the §5 trap).
///
/// Flip receipt (wave 6; re-executed wave 7): flips under M1 (no-advance
/// tick). Under M2 (tick-trap downgraded to warning) this court stays GREEN
/// — the exhaust is caught by the INDEPENDENT second trap in `Lease::phi()`
/// (`lease.rs` defense-in-depth): `seal_unsealed` snapshots phi before
/// advancing, so the phi-side trap fires first. Both traps are courted.
///
/// Wave-7 repair (D-A16-4 class, same session): the parent demands a
/// cgap-crypto authority-trap signature on stderr (either trap message), so
/// a nonzero child exit from an unrelated panic cannot satisfy this court.
#[test]
fn seal_at_exhaustion_traps_in_real_process() {
    if std::env::var("CGAP_A16_CHATMAN_PROBE").as_deref() == Ok("seal_exhaust") {
        let mut lease = Lease::new([7u8; 32], 2, [9u8; 32]);
        let _ = seal_unsealed(fixture_delta([0u8; 32]), &mut lease);
        let _ = seal_unsealed(fixture_delta([1u8; 32]), &mut lease);
        let _ = seal_unsealed(fixture_delta([2u8; 32]), &mut lease); // MUST TRAP
        eprintln!("SHIM-COURT WITNESS: seal survived exhaustion (trap missing)");
        return;
    }
    let exe = std::env::current_exe().expect("current_exe");
    let output = std::process::Command::new(exe)
        .args(["seal_at_exhaustion_traps_in_real_process", "--exact", "--nocapture"])
        .env("CGAP_A16_CHATMAN_PROBE", "seal_exhaust")
        .output()
        .expect("spawn probe subprocess");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "VACUOUS GATE: seal at exhaustion did NOT trap (status {status:?}); \
         stderr: {stderr}",
        status = output.status,
    );
    assert!(
        stderr.contains("refusing to emit Phi_lease")
            || stderr.contains("authority EXHAUSTED"),
        "WRONG-REASON DEATH: probe exited nonzero but its stderr names \
         neither seal-path trap (\"refusing to emit Phi_lease\" / \
         \"authority EXHAUSTED\") — a non-trap panic died instead; \
         stderr: {stderr}",
    );
}

/// §8.3 byte-sweep court: flip EVERY byte of the receipt encoding — magic,
/// version, `DeltaS`, `Phi_lease`, digest — one at a time, two masks;
/// `verify` must fail for EVERY flip with the region-appropriate error.
/// Exhaustive and deterministic, no randomness.
///
/// Anti-vacuity mutation (shim-court executed, wave 6 / M4, isolated flip):
/// `verify` that compares only the FIRST digest byte MUST be caught by this
/// sweep.
#[test]
fn byte_sweep_court_every_single_byte_flip_fails_verify() {
    let mut lease = Lease::new([7u8; 32], 64, [9u8; 32]);
    let receipt = seal_unsealed(fixture_delta([0x44u8; 32]), &mut lease);
    let pristine = receipt.to_bytes();
    assert!(verify_bytes(&pristine).is_ok(), "pristine receipt must verify");

    for offset in 0..UNSEALED_RECEIPT_LEN {
        for mask in [0x01u8, 0xFF] {
            let mut flipped = pristine;
            flipped[offset] ^= mask;
            let verdict = verify_bytes(&flipped);
            assert!(
                verdict.is_err(),
                "§8.3 VACUOUS GATE: flip at offset {offset} (mask {mask:#04x}) \
                 still verifies"
            );
            let expected = expected_error_for_flip(&pristine, offset, mask);
            assert_eq!(
                verdict, expected,
                "flip at offset {offset} (mask {mask:#04x}) must fail with the \
                 region-appropriate error"
            );
        }
    }
}

/// Region-appropriate expected error for a single-byte flip of `pristine`,
/// replicating verify's decision order: magic -> version -> authority
/// (tick/ceiling) -> keyed digest.
fn expected_error_for_flip(
    pristine: &[u8; UNSEALED_RECEIPT_LEN],
    offset: usize,
    mask: u8,
) -> Result<(), VerifyError> {
    if offset < 4 {
        let mut magic = [pristine[0], pristine[1], pristine[2], pristine[3]];
        magic[offset] ^= mask;
        Err(VerifyError::BadMagic(magic))
    } else if offset == 4 {
        Err(VerifyError::BadVersion(pristine[4] ^ mask))
    } else if (5..13).contains(&offset) {
        let mut tag = [0u8; 8];
        tag.copy_from_slice(&pristine[5..13]);
        tag[offset - 5] ^= mask;
        Err(VerifyError::BadDelta(DeltaError::BadDomainTag(tag)))
    } else {
        // Authority check precedes the digest check: flips inside phi.tick
        // (receipt offsets 149..157) or phi.tick_ceiling (157..165) can turn
        // the receipt into a forged-exhausted one before any digest compare.
        let mut tick_bytes = [0u8; 8];
        tick_bytes.copy_from_slice(&pristine[149..157]);
        let mut ceiling_bytes = [0u8; 8];
        ceiling_bytes.copy_from_slice(&pristine[157..165]);
        if (149..157).contains(&offset) {
            tick_bytes[offset - 149] ^= mask;
        }
        if (157..165).contains(&offset) {
            ceiling_bytes[offset - 157] ^= mask;
        }
        let tick = u64::from_le_bytes(tick_bytes);
        let ceiling = u64::from_le_bytes(ceiling_bytes);
        if tick >= ceiling {
            Err(VerifyError::ExhaustedLease { tick, tick_ceiling: ceiling })
        } else {
            Err(VerifyError::DigestMismatch)
        }
    }
}

/// Wire-level verify: decode + verify as one gate (what the sweep targets).
fn verify_bytes(bytes: &[u8; UNSEALED_RECEIPT_LEN]) -> Result<(), VerifyError> {
    UnsealedReceipt::from_bytes(bytes).and_then(|r| verify_unsealed(&r))
}

/// Forged receipts that no live lease could produce are refused: a hand-built
/// `Phi_lease` claiming `tick >= tick_ceiling` fails verification even when
/// its digest matches its own (forged) contents.
#[test]
fn forged_exhausted_lease_receipt_rejected() {
    let phi = PhiLease {
        lease_id: [7u8; 32],
        tick: 8,
        tick_ceiling: 8,
        issued_root: [9u8; 32],
    };
    let delta = fixture_delta([0u8; 32]);
    let mut covered = [0u8; DELTA_LEN + PHI_LEASE_LEN];
    covered[0..DELTA_LEN].copy_from_slice(&delta.to_bytes());
    covered[DELTA_LEN..].copy_from_slice(&phi.to_bytes());
    let digest = keyed_digest(&phi.issued_root, &covered).expect("192 B");
    let receipt = UnsealedReceipt {
        magic: *b"CGRU",
        version: 0x00,
        delta,
        phi,
        blake3_digest: digest,
    };
    assert_eq!(
        verify_unsealed(&receipt),
        Err(VerifyError::ExhaustedLease {
            tick: 8,
            tick_ceiling: 8
        })
    );

    // A ceiling of 0 in a forged phi is exhausted by the same rule. The
    // digest is recomputed for the new phi so ONLY the authority check can
    // fail.
    let mut phi0 = phi;
    phi0.tick = 0;
    phi0.tick_ceiling = 0;
    covered[DELTA_LEN..].copy_from_slice(&phi0.to_bytes());
    let digest0 = keyed_digest(&phi0.issued_root, &covered).expect("192 B");
    let receipt0 = UnsealedReceipt {
        magic: *b"CGRU",
        version: 0x00,
        delta,
        phi: phi0,
        blake3_digest: digest0,
    };
    assert_eq!(
        verify_unsealed(&receipt0),
        Err(VerifyError::ExhaustedLease {
            tick: 0,
            tick_ceiling: 0
        })
    );
}

/// §8.4 chaining court: prior receipts chain into `admissions`; a receipt
/// cannot verify under a different lease's `issued_root` (the keyed digest
/// recompute fails when the epoch binding is swapped).
///
/// Flip receipt (wave 6): flips under M5b (verify digest coverage drops
/// phi+key — shim recomputes an unkeyed hash of the delta alone). A
/// key-ignoring shim alone (M5) does NOT flip this court: `issued_root`
/// sits INSIDE the covered input, so epoch-swap detection is structural,
/// not key-dependent — recorded as a robustness positive.
#[test]
fn chaining_court_receipts_chain_and_epoch_swap_fails() {
    // Receipt 1 under lease epoch 1 (fresh lease, same epoch parameters for
    // both seals — an epoch is the (lease_id, ceiling, issued_root) triple;
    // each seal consumes one tick of its own lease).
    let mut lease1 = Lease::new([0xA1u8; 32], 64, [0xB1u8; 32]);
    let r1 = seal_unsealed(fixture_delta([0u8; 32]), &mut lease1);
    assert_eq!(verify_unsealed(&r1), Ok(()));

    // Receipt 2 folds r1's encoding into admissions and seals under the SAME
    // epoch — the chain verifies.
    let r1_bytes = r1.to_bytes();
    let admissions = admissions_root(&[&r1_bytes]);
    let mut lease1b = Lease::new([0xA1u8; 32], 64, [0xB1u8; 32]);
    let delta2 = fixture_delta(admissions);
    let r2 = seal_unsealed(delta2, &mut lease1b);
    assert_eq!(verify_unsealed(&r2), Ok(()));
    assert_eq!(r2.delta.admissions, admissions);

    // Epoch swap: the SAME receipt with a FOREIGN issued_root cannot verify —
    // the keyed digest recompute under the foreign root diverges (§8.4).
    let mut swapped_phi: PhiLease = r2.phi;
    swapped_phi.issued_root = [0xC1u8; 32];
    let swapped = UnsealedReceipt {
        magic: *b"CGRU",
        version: 0x00,
        delta: r2.delta,
        phi: swapped_phi,
        blake3_digest: r2.blake3_digest,
    };
    assert_eq!(
        verify_unsealed(&swapped),
        Err(VerifyError::DigestMismatch),
        "§8.4: a receipt cannot verify under a different lease's issued_root"
    );

    // And a receipt sealed under a DIFFERENT epoch is a different receipt:
    let mut lease2 = Lease::new([0xA2u8; 32], 64, [0xB2u8; 32]);
    let r2_other = seal_unsealed(delta2, &mut lease2);
    assert_ne!(
        r2.to_bytes(),
        r2_other.to_bytes(),
        "different lease epochs seal different receipts"
    );
}

/// §8.5 determinism court: same inputs, same bytes — receipt encoding is
/// byte-identical across independent runs (no nondeterministic serializer).
///
/// Flip receipt (wave 6 / M6): a per-call counter XORed into byte 0 of
/// `to_bytes` flips this court AND the end-to-end pin.
#[test]
fn determinism_court_identical_inputs_identical_bytes() {
    let build = || {
        let mut lease = Lease::new([0x77u8; 32], 16, [0x88u8; 32]);
        let r1 = seal_unsealed(fixture_delta([1u8; 32]), &mut lease);
        let r2 = seal_unsealed(fixture_delta([2u8; 32]), &mut lease);
        (r1.to_bytes(), r2.to_bytes())
    };
    let (a1, a2) = build();
    let (b1, b2) = build();
    assert_eq!(a1, b1);
    assert_eq!(a2, b2);
    assert_ne!(a1, a2, "distinct deltas seal distinct receipts");
}

/// Sealer seam (§4): the trait object works, the unsealed tier names itself
/// (`0x0000`), and the seal check is total over the seal body.
///
/// Wave-6 repair (this session): `verify_seal` now uses a constant-shape
/// compare (fold every byte difference; A18 `LeaseEpochSealer::verify`
/// precedent), and this court sweeps EVERY byte of the 34-B seal body
/// (`digest || algorithm_id`) with two masks — 68 flipped seals, each MUST be
/// refused. Exhaustive, deterministic, no randomness.
///
/// Anti-vacuity mutation (shim-court executed this session, wave 6):
/// `verify_seal` comparing only the FIRST byte of the seal body MUST flip
/// this sweep (a flip in the algorithm-id bytes would still verify).
#[test]
fn sealer_trait_unsealed_roundtrip_and_flip() {
    let sealer = Unsealed;
    assert_eq!(sealer.algorithm_id(), ALGORITHM_ID_UNSEALED);
    assert_eq!(sealer.algorithm_id(), 0x0000);
    let digest = keyed_digest(OFFICIAL_KEY, &corpus(192)).expect("192 B");
    let seal = sealer.seal(digest);
    assert_eq!(seal.len(), DIGEST_LEN + 2);
    assert!(sealer.verify_seal(digest, &seal), "pristine seal verifies");

    // A foreign digest must not verify under the original seal.
    let mut flipped_digest = digest;
    flipped_digest[17] ^= 0x80;
    assert!(
        !sealer.verify_seal(flipped_digest, &seal),
        "a foreign digest must not verify under the original seal"
    );

    // §8.3-class exhaustive sweep over the 34-B seal body: every single-byte
    // flip (two masks) must flip the verdict.
    for offset in 0..seal.len() {
        for mask in [0x01u8, 0xFF] {
            let mut flipped_seal = seal.clone();
            flipped_seal[offset] ^= mask;
            assert!(
                !sealer.verify_seal(digest, &flipped_seal),
                "VACUOUS GATE: seal flip at offset {offset} (mask {mask:#04x}) \
                 still verifies"
            );
        }
    }

    // Truncated/extended seals are refused, not truncated-accepted.
    assert!(!sealer.verify_seal(digest, &seal[..33]));
    assert!(!sealer.verify_seal(digest, &{
        let mut longer = seal.clone();
        longer.push(0);
        longer
    }));
}

// ----------------------------------------------------------------
// §3.1 admissions Merkle root
// ----------------------------------------------------------------

/// Leaf identity and parent construction, cross-checked against the REAL
/// kernel (`cgap_blake3::hash`), not against itself; parent KAT pinned via
/// b3sum this session; order sensitivity (ordered list, §3.1).
#[test]
fn admissions_root_construction_pinned_to_kernel() {
    let leaf0 = cgap_blake3::hash(b"leaf-zero");
    let leaf1 = cgap_blake3::hash(b"leaf-one");
    let leaf2 = cgap_blake3::hash(b"leaf-two");

    // Single leaf: root == unkeyed leaf digest (kernel-recomputed).
    assert_eq!(admissions_root(&[b"leaf-zero"]), leaf0);

    // Two leaves: parent = hash(leaf0 || leaf1).
    let two = admissions_root(&[b"leaf-zero", b"leaf-one"]);
    let mut pair_block = [0u8; 64];
    pair_block[0..32].copy_from_slice(&leaf0);
    pair_block[32..64].copy_from_slice(&leaf1);
    assert_eq!(two, cgap_blake3::hash(&pair_block));
    let two_hex: String = two.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        two_hex, "2fdeef277c83236db72e1da942c778962d0f86c9f422629c1b77535d5a3cf0a9",
        "b3sum-derived parent KAT"
    );

    // Three leaves: odd final node promoted — hash(hash(l0||l1) || l2).
    let three = admissions_root(&[b"leaf-zero", b"leaf-one", b"leaf-two"]);
    let mut top_block = [0u8; 64];
    top_block[0..32].copy_from_slice(&two);
    top_block[32..64].copy_from_slice(&leaf2);
    assert_eq!(three, cgap_blake3::hash(&top_block));

    // Ordered: reordering the receipts changes the root (§3.1 ordered list).
    let reversed = admissions_root(&[b"leaf-one", b"leaf-zero"]);
    assert_ne!(reversed, two, "admissions is an ORDERED list");
}

/// Empty admissions: typed zero root (divergence D3 — the doc names no empty
/// case; pinned loudly here).
#[test]
fn admissions_root_empty_is_typed_zero() {
    assert_eq!(admissions_root(&[]), [0u8; 32]);
}

/// Sanity: the Wave-1 ML-DSA-65 shape constant survives untouched (the
/// Wave-3 target receipt is 2649 B).
#[test]
fn wave3_receipt_shape_constant_unchanged() {
    assert_eq!(RECEIPT_LEN, 2649);
}

// ----------------------------------------------------------------
// Wave-6 hostile-input court (A03-F1 class: hostile bytes must be
// refused TYPED, never panic)
// ----------------------------------------------------------------

/// Hostile and degenerate byte strings through the public parsers: every
/// case refuses TYPED (named `Err` variant), never panics, never accepts.
/// Covers the A03 audit's F-1 defect class (hostile counts panicking in a
/// public parser) for THIS crate's surfaces: `DeltaS::from_bytes`,
/// `PhiLease::from_bytes`, `UnsealedReceipt::from_bytes`, `verify_unsealed`.
///
/// Named mutation (shim-court executed this session, wave 6): dropping the
/// `UnsealedReceipt::from_bytes` magic check MUST flip this court — the
/// all-zero and all-0xFF cases would then decode and their `BadMagic`
/// asserts fail.
#[test]
fn hostile_receipt_bytes_refused_typed_no_panic() {
    // (1) All-zero 229 B: foreign magic, refused before any parse work.
    let zeros = [0u8; UNSEALED_RECEIPT_LEN];
    assert_eq!(
        UnsealedReceipt::from_bytes(&zeros),
        Err(VerifyError::BadMagic([0u8; 4]))
    );

    // (2) All-0xFF 229 B: same refusal, opposite pole.
    let saturated = [0xFFu8; UNSEALED_RECEIPT_LEN];
    assert_eq!(
        UnsealedReceipt::from_bytes(&saturated),
        Err(VerifyError::BadMagic([0xFFu8; 4]))
    );

    // (3) The SEALED tier's magic ("CGRC") fed to the unsealed parser:
    // cross-tier confusion refused loudly.
    let mut sealed_magic = zeros;
    sealed_magic[0..4].copy_from_slice(b"CGRC");
    assert_eq!(
        UnsealedReceipt::from_bytes(&sealed_magic),
        Err(VerifyError::BadMagic(*b"CGRC"))
    );

    // (4) Right magic, wrong version (the SEALED version byte 0x01).
    let mut wrong_version = zeros;
    wrong_version[0..4].copy_from_slice(b"CGRU");
    wrong_version[4] = 0x01;
    assert_eq!(
        UnsealedReceipt::from_bytes(&wrong_version),
        Err(VerifyError::BadVersion(0x01))
    );

    // (5) Right magic + version, corrupted delta domain tag.
    let mut bad_tag = wrong_version;
    bad_tag[4] = 0x00;
    bad_tag[12] = b'2'; // domain tag region: receipt bytes 5..13
    let tag = {
        let mut t = [0u8; 8];
        t.copy_from_slice(&bad_tag[5..13]);
        t
    };
    assert_eq!(
        UnsealedReceipt::from_bytes(&bad_tag),
        Err(VerifyError::BadDelta(DeltaError::BadDomainTag(tag)))
    );

    // (6) Exhausted-phi hostile shapes parse (decode is structural) but
    // verify refuses TYPED, for both poles: tick = u64::MAX over ceiling 0,
    // and the inverted pole ceiling = u64::MAX with tick = u64::MAX - 1 is
    // NOT exhausted (digest path) — so build both phis, self-consistent
    // digests each, assert the two DIFFERENT typed verdicts.
    let delta = fixture_delta([0x66u8; 32]);
    let mut hostile_phi = PhiLease {
        lease_id: [0xEEu8; 32],
        tick: u64::MAX,
        tick_ceiling: 0,
        issued_root: [0xDDu8; 32],
    };
    let mut covered = [0u8; DELTA_LEN + PHI_LEASE_LEN];
    covered[0..DELTA_LEN].copy_from_slice(&delta.to_bytes());
    covered[DELTA_LEN..].copy_from_slice(&hostile_phi.to_bytes());
    let digest_max = keyed_digest(&hostile_phi.issued_root, &covered).expect("192 B");
    let receipt_max = UnsealedReceipt {
        magic: *b"CGRU",
        version: 0x00,
        delta,
        phi: hostile_phi,
        blake3_digest: digest_max,
    };
    assert_eq!(
        verify_unsealed(&receipt_max),
        Err(VerifyError::ExhaustedLease {
            tick: u64::MAX,
            tick_ceiling: 0
        })
    );

    // Inverted pole: huge ceiling, tick one below it — NOT exhausted, so
    // verify proceeds to the digest gate. Self-consistent digest under the
    // hostile epoch; stored digest all-zeros forces the typed
    // DigestMismatch (no panic from the u64::MAX fields on the arithmetic
    // path — there is no arithmetic on untrusted ticks anywhere).
    hostile_phi.tick_ceiling = u64::MAX;
    hostile_phi.tick = u64::MAX - 1;
    let receipt_huge = UnsealedReceipt {
        magic: *b"CGRU",
        version: 0x00,
        delta,
        phi: hostile_phi,
        blake3_digest: [0u8; DIGEST_LEN],
    };
    assert_eq!(
        verify_unsealed(&receipt_huge),
        Err(VerifyError::DigestMismatch)
    );

    // (7) PhiLease::from_bytes on all-0xFF 80 B: inert snapshot, extreme
    // field values surface, still no panic.
    let hostile_phi_bytes = [0xFFu8; PHI_LEASE_LEN];
    let decoded_phi = PhiLease::from_bytes(&hostile_phi_bytes);
    assert_eq!(decoded_phi.tick, u64::MAX);
    assert_eq!(decoded_phi.tick_ceiling, u64::MAX);
    assert_eq!(decoded_phi.lease_id, [0xFFu8; 32]);
    assert_eq!(decoded_phi.issued_root, [0xFFu8; 32]);

    // (8) DeltaS::from_bytes on all-0xFF 112 B: typed domain-tag refusal.
    assert_eq!(
        DeltaS::from_bytes(&[0xFFu8; DELTA_LEN]),
        Err(DeltaError::BadDomainTag([0xFFu8; 8]))
    );
}
