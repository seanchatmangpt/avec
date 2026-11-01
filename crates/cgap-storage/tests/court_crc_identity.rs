//! A05 Wave-5 court: cross-lane CRC-32 identity (defect repair).
//!
//! Defect (wave 1-4 finding): the journal module claimed its `crc32` has
//! "same identity `cgap-git` (`pack_write.rs`, "IEEE CRC-32 (reflected, poly
//! 0xEDB88320)") uses for idx entries" — a cross-lane identity claim that no
//! court pinned, so it stood UNVERIFIED. The journal's PAYLOAD_CRC /
//! HEADER_CRC semantics would silently diverge from the cgap-git idx-CRC
//! world if either implementation drifted.
//!
//! Repair: pin the identity two ways, both real collaborators, asserting
//! final state:
//!
//! 1. Known-answer: IEEE CRC-32 check values derived from an INDEPENDENT
//!    implementation (Python `zlib.crc32`, executed on the authoring
//!    session 2026-11-01; constants recorded below). These pin poly +
//!    init + reflected final-xor exactly.
//! 2. Differential: `cgap_storage::atomic_commit::crc32` vs
//!    `cgap_git::pack_write::crc32` byte-for-byte over a deterministic
//!    corpus that includes the journal's own real byte shapes (the 48-byte
//!    descriptor, the 112-byte encoded record, the erased 0xFF slot, the
//!    all-zero slot).
//!
//! Anti-vacuity — named mutations that MUST flip this court:
//!
//! - M-C1: drop `crc32`'s final XOR (`!crc`) -> the empty-input KAT flips
//!   (returns 0xFFFF_FFFF, expected 0x0000_0000).
//! - M-C2: drift the polynomial away from 0xEDB88320 -> the "123456789"
//!   check-value KAT flips (0xCBF4_3926).
//! - M-C3: make `crc32` init-only (return `!0xFFFF_FFFF` without
//!   consuming input) -> every non-empty KAT and differential row flips.
//!
//! A run in which this court still passes under a named mutation is a
//! VACUOUS GATE (FAIL). No mocks: both CRCs are the production functions.

#![forbid(unsafe_code)]

use cgap_git::pack_write as git_crc;
use cgap_storage::atomic_commit::{
    encode_record, crc32, FLAG_DUAL_WRITE, NewRootDescriptor, RECORD_SIZE,
};

/// zlib-derived IEEE CRC-32 reference vectors (authoring session evidence;
/// re-derivable with `python3 -c "import zlib; print(hex(zlib.crc32(...)))"`).
#[test]
fn crc32_ieee_known_answers() {
    // Empty input pins init + final-xor (0x0000_0000 = !0xFFFF_FFFF).
    assert_eq!(crc32(b""), 0x0000_0000);
    // The canonical IEEE check value.
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    // Journal-scale shapes: erased slot, all-zero slot, a full byte
    // spectrum, and the SCAJ header prefix as raw bytes.
    assert_eq!(crc32(&[0xFF; RECORD_SIZE]), 0xBDC6_4068);
    assert_eq!(crc32(&[0x00; RECORD_SIZE]), 0x1BBE_ADBD);
    assert_eq!(crc32(&(0x00u8..=0xFF).collect::<Vec<u8>>()), 0x2905_8C73);
    assert_eq!(crc32(b"SCAJ\0\0\0\0\0\0\0\0"), 0x455E_D47A);
}

/// Cross-lane differential: the journal CRC and the cgap-git idx CRC agree
/// on every input — the "same identity" claim is now a pinned invariant.
#[test]
fn crc32_matches_cgap_git_differential() {
    let mut corpus: Vec<Vec<u8>> = Vec::new();
    // Deterministic length sweep 0..=256: every length class including the
    // record framing boundaries (48, 96, 112) and the descriptor size.
    for len in 0..=256usize {
        let v: Vec<u8> = (0..len).map(|i| ((i * 7 + 3) & 0xFF) as u8).collect();
        corpus.push(v);
    }
    // The journal's own real byte shapes.
    let desc = NewRootDescriptor::new([0xAA; 20], 0x1000, 9, 0, FLAG_DUAL_WRITE);
    corpus.push(desc.to_bytes().to_vec()); // 48-byte payload
    corpus.push(encode_record(1, &desc).to_vec()); // 112-byte record
    corpus.push(vec![0xFF; RECORD_SIZE]); // erased slot
    corpus.push(vec![0x00; RECORD_SIZE]); // zero-filled slot
    // Mixed 0xFF/0x00 torn-erase mix (§9 risk class).
    corpus.push(
        (0..RECORD_SIZE)
            .map(|i| if i % 3 == 0 { 0xFF } else { 0x00 })
            .collect(),
    );

    assert!(
        corpus.len() >= 261,
        "corpus must cover the length sweep plus journal shapes"
    );
    for (i, input) in corpus.iter().enumerate() {
        assert_eq!(
            crc32(input),
            git_crc::crc32(input),
            "corpus row {i} (len {}): journal CRC diverged from cgap-git",
            input.len()
        );
    }
}

/// Streaming decomposition: cgap-git's streaming `Crc32` over an arbitrary
/// two-part split equals both one-shots — the identity holds across the
/// streaming surface too, not just the one-shot convenience wrapper.
#[test]
fn crc32_streaming_identity() {
    let payload = encode_record(2, &NewRootDescriptor::new([0xBB; 20], 0x2000, 10, 1, FLAG_DUAL_WRITE));
    let (head, tail) = payload.split_at(48);
    let mut stream = git_crc::Crc32::new();
    stream.update(head);
    stream.update(tail);
    assert_eq!(stream.clone().finalize(), crc32(&payload));
    assert_eq!(stream.finalize(), git_crc::crc32(&payload));
}
