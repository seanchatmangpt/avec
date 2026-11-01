//! Cross-lane identity court, Wave 9 — the A14 dam over real A04 frames.
//!
//! Wave 2's `identity_court.rs` ran the wire + memory tiers and printed
//! loud `[SKIP]`s for every cross-lane consumer. Wave 9 lands the first
//! real cross-lane tier: `cgap_gate::penstock::ingress` (lane A14's dam,
//! the admit boundary) consuming frames that lane A04's wire tier encoded
//! and lane A04's memory tier (`DmaRing`) carried — byte-for-byte identity
//! between the frame each tier presents and the payload the gate admits.
//!
//! Contract (lane docs A04 §8.1/§8.2 + A14 §2/§4; A17 §2.1 attestation
//! offsets; both crates pin identical offset tables at compile time):
//! - the dam admits exactly the frame payload region `[128 .. 128+len]`
//!   and nothing else;
//! - the same bytes admitted by the gate re-derive the same digest and
//!   length on the cgap-core side (no tier may "repair" a frame);
//! - every typed refusal pair is pinned: where the contracts overlap,
//!   cgap-core and cgap-gate refuse the SAME corrupted bytes with the
//!   contract-mapped refusals (§8.3-style mutation rows — a gate mutated
//!   to always-admit flips every row below);
//! - the one documented divergence (A14 row 6: reserved header flag bits
//!   ADMITTED by the gate, REFUSED by cgap-core `validate()`) is pinned
//!   as a divergence test — recorded, never silent.
//!
//! Real collaborators only (Chicago): real wire bytes, real `DmaRing`
//! region, real `cgap-blake3` digests, real gate `ingress`. No mocks.
//! No sleeps: every observable is returned synchronously.

use cgap_blake3::hash as oabi_digest;
use cgap_core::dma_ring::DmaRing;
use cgap_core::frame::{
    CgapFrameHeader, FrameError, ATTESTATION_LEN, FLAG_BN3, FRAME_HEADER_LEN,
};
use cgap_core::isomorphism::{identity_assert, Tier};
use cgap_gate::penstock::{
    ingress, CGAP_ERR_INVALID_MAGIC, CGAP_ERR_OABI_MISMATCH, CGAP_ERR_P28_SPOLIATION,
    CGAP_OK,
};

mod common;
use common::{leak_aligned_region, payload, slot_region_len, valid_header, wire_frame, SIZES};

// --- A17 §2.1 attestation byte offsets inside the 128 B frame header ----
// (frame offset = 48 + attestation offset; pinned by both crates' const
// offset_of! asserts, re-derived here so a drift between the two crates'
// tables trips THIS court, not just their internal asserts).
const ATT_MAGIC_AT: usize = FRAME_HEADER_LEN - ATTESTATION_LEN; // 48
const ATT_VERSION_AT: usize = ATT_MAGIC_AT + 4; // 52
const ATT_FLAGS_AT: usize = ATT_MAGIC_AT + 6; // 54
const ATT_CEILING_AT: usize = ATT_MAGIC_AT + 8; // 56
const ATT_RECEIPT_AT: usize = ATT_MAGIC_AT + 48; // 96

/// Patch the encoded wire frame at `at` (wire-level corruption: the dam
/// sees the patched bytes, exactly as a hostile transport would deliver).
fn patch(frame: &mut Vec<u8>, at: usize, f: impl FnOnce(&mut [u8])) {
    f(&mut frame[at..]);
}

/// Build one valid wire frame over a deterministic payload of `size` bytes
/// (real BLAKE3 digest, valid Root attestation, BN3 flag — the courts'
/// positive control, same fixture family as `identity_court.rs`).
fn wire_frame_at(size: usize) -> Vec<u8> {
    let p = payload(size);
    let h = valid_header(size as u64, oabi_digest(&p), bn3());
    wire_frame(&h, &p)
}

fn bn3() -> u8 {
    FLAG_BN3
}

// --- Tier plumbing shared by the exit gate ------------------------------

/// Carry `wire` through the real memory tier (claim → publish → read) and
/// rebuild the full frame region from what the consumer actually read.
fn memory_projection(wire: &[u8], size: usize) -> Vec<u8> {
    let region = leak_aligned_region(slot_region_len(size, 2));
    let mut ring = DmaRing::new(region, 1).expect("region partitions into 2 slots");
    {
        let slot = ring.claim().expect("ring has room");
        assert!(wire.len() <= slot.len(), "frame must fit the slot stride");
        slot[..wire.len()].copy_from_slice(wire);
    }
    ring.publish(wire.len()).expect("publish valid frame");
    let (payload_m, header_m) = ring.read().expect("published frame pending");
    assert_eq!(header_m.validate(), Ok(()), "memory tier header");
    let mut mem = vec![0u8; FRAME_HEADER_LEN + size];
    header_m.encode_into(&mut mem).expect("encode memory header");
    mem[FRAME_HEADER_LEN..].copy_from_slice(payload_m);
    ring.release().expect("release");
    assert_eq!(ring.pending(), 0);
    mem
}

// ===========================================================================
// The Wave 9 exit gate: dam over frames, identity across all landed tiers.
// ===========================================================================

#[test]
fn cross_lane_gate_admits_a04_frames_with_tier_identity_per_size() {
    for &size in SIZES.iter() {
        let wire = wire_frame_at(size);
        let payload_bytes = payload(size);

        // -- wire tier (cgap-core): decode → validate → digest re-derivation
        let decoded = CgapFrameHeader::decode(&wire).expect("decode");
        assert_eq!(decoded.validate(), Ok(()), "wire tier size={size}");
        assert_eq!(
            decoded.verify_payload(&payload_bytes, oabi_digest),
            Ok(()),
            "wire digest re-derivation size={size}"
        );
        let core_view = decoded.payload(&wire).expect("payload view");
        assert_eq!(core_view, &payload_bytes[..], "core view size={size}");

        // -- memory tier (cgap-core DmaRing): bit-for-bit identity ----------
        let mem = memory_projection(&wire, size);
        identity_assert(&wire, Tier::Memory, &mem)
            .unwrap_or_else(|m| panic!("tier=memory size={size}: {m}"));
        assert_eq!(
            CgapFrameHeader::decode(&mem)
                .expect("memory decode")
                .verify_payload(&payload_bytes, oabi_digest),
            Ok(()),
            "memory digest re-derivation size={size}"
        );

        // -- gate tier (A14 dam over A04 frames, wire bytes) ----------------
        let mut view = cgap_gate::penstock::FrameView { payload: &[] };
        let st = ingress(&wire, wire.len(), Some(&mut view));
        assert_eq!(st, CGAP_OK, "dam must admit a valid frame, size={size}");
        // Identity: the admitted view is EXACTLY the frame payload region —
        // byte-for-byte equal to what the wire tier and cgap-core's own
        // payload view carry, and never a byte more or less.
        assert_eq!(view.payload, &wire[FRAME_HEADER_LEN..], "gate view == wire payload region, size={size}");
        assert_eq!(view.payload, core_view, "gate view == core payload view, size={size}");
        assert_eq!(view.payload.len(), size, "admitted length is exact, size={size}");

        // -- gate tier over the memory projection ---------------------------
        let mut view_m = cgap_gate::penstock::FrameView { payload: &[] };
        let st_m = ingress(&mem, mem.len(), Some(&mut view_m));
        assert_eq!(
            st_m, CGAP_OK,
            "dam must admit the memory-tier projection, size={size}"
        );
        assert_eq!(
            view_m.payload,
            &mem[FRAME_HEADER_LEN..],
            "gate view == memory payload region, size={size}"
        );

        println!(
            "[SKIP] tier=arrow/parquet reason=disk-tier views not landed (no in-tree \
             adapter consumes cgap-core frames yet); identity court family runs \
             wire+memory+gate mode per A04 §10"
        );
    }
}

// ===========================================================================
// Cross-lane refusal parity (§8.3 mutation rows, both lanes, wire-level)
// ===========================================================================

#[test]
fn mutation_magic_flip_refused_by_both_lanes() {
    let mut frame = wire_frame_at(64);
    patch(&mut frame, 0, |b| b[0..4].copy_from_slice(b"XGAP"));

    // cgap-core: typed FrameError::BadMagic.
    let decoded = CgapFrameHeader::decode(&frame).expect("layout decode");
    assert_eq!(decoded.validate(), Err(FrameError::BadMagic), "core side");
    // A14 dam: typed CGAP_ERR_INVALID_MAGIC.
    assert_eq!(
        ingress(&frame, frame.len(), None),
        CGAP_ERR_INVALID_MAGIC,
        "gate side"
    );
}

#[test]
fn mutation_version_bump_refused_by_both_lanes() {
    let mut frame = wire_frame_at(64);
    patch(&mut frame, 4, |b| b[0] = 2);

    let decoded = CgapFrameHeader::decode(&frame).expect("layout decode");
    assert_eq!(decoded.validate(), Err(FrameError::BadVersion), "core side");
    // A14 row-2 family overload: wrong version is INVALID_MAGIC (recorded
    // in the gate's per-taxonomy table), not a minted code.
    assert_eq!(
        ingress(&frame, frame.len(), None),
        CGAP_ERR_INVALID_MAGIC,
        "gate side"
    );
}

#[test]
fn mutation_zeroed_digest_refused_by_both_lanes() {
    let mut frame = wire_frame_at(64);
    patch(&mut frame, 16, |b| b.fill(0));

    // cgap-core: the embedded zeros != re-derived BLAKE3 → DigestMismatch.
    let decoded = CgapFrameHeader::decode(&frame).expect("layout decode");
    let p = payload(64);
    assert_eq!(
        decoded.verify_payload(&p, oabi_digest),
        Err(FrameError::DigestMismatch),
        "core side"
    );
    // A14 row 3a: the gate is a limiter, not a verifier — all-zero digest
    // is not a CGAP frame at all → CGAP_ERR_OABI_MISMATCH.
    assert_eq!(
        ingress(&frame, frame.len(), None),
        CGAP_ERR_OABI_MISMATCH,
        "gate side"
    );
}

#[test]
fn mutation_payload_len_shift_refused_by_both_lanes() {
    let size = 64;
    let mut frame = wire_frame_at(size);
    // §8.3 identity row: shift payload_byte_len by +1 (declared now exceeds
    // the bytes present).
    patch(&mut frame, 8, |b| b[0] += 1);

    // cgap-core: both length gates refuse, typed PayloadLenMismatch.
    let decoded = CgapFrameHeader::decode(&frame).expect("layout decode");
    assert_eq!(
        decoded.payload(&frame),
        Err(FrameError::PayloadLenMismatch),
        "core payload view"
    );
    assert_eq!(
        decoded.verify_payload(&payload(size), oabi_digest),
        Err(FrameError::PayloadLenMismatch),
        "core verify"
    );
    // A14 row 3b: declared > len - 128 → CGAP_ERR_OABI_MISMATCH.
    assert_eq!(
        ingress(&frame, frame.len(), None),
        CGAP_ERR_OABI_MISMATCH,
        "gate side"
    );
}

#[test]
fn mutation_attestation_corruptions_refused_by_both_lanes() {
    // One byte-level mutation per A17 §2.1 invariant-bearing attestation
    // field; each must be refused by BOTH lanes with the contract-mapped
    // refusal: cgap-core FrameError::AttestationMismatch and the A14 dam
    // CGAP_ERR_P28_SPOLIATION.
    let mutations: [(&str, usize, &dyn Fn(&mut [u8])); 6] = [
        // magic "P28S" → "P28X" (row 4)
        ("att_magic", ATT_MAGIC_AT, &|b: &mut [u8]| b[3] = b'X'),
        // version u16 LE 1 → 2 (row 5)
        ("att_version", ATT_VERSION_AT, &|b: &mut [u8]| b[0] = 2),
        // reserved flag bit 3 set (row 5)
        ("att_flags_reserved", ATT_FLAGS_AT, &|b: &mut [u8]| {
            b[0] |= 0x08
        }),
        // safe-harbor ceiling $50M + 1 cent (row 5)
        ("att_ceiling", ATT_CEILING_AT, &|b: &mut [u8]| {
            let mut c = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
            c += 1;
            b[..8].copy_from_slice(&c.to_le_bytes());
        }),
        // upstream receipt nonzero on a ROOT record (row 5 receipt law)
        ("att_receipt_on_root", ATT_RECEIPT_AT, &|b: &mut [u8]| {
            b[0] = 0xFF
        }),
        // lineage bits all zero (row 5: exactly-one-bit law)
        ("att_lineage_zero", ATT_FLAGS_AT, &|b: &mut [u8]| {
            b[0] &= !0x07
        }),
    ];

    for (name, at, m) in mutations.into_iter() {
        let mut frame = wire_frame_at(64);
        patch(&mut frame, at, m);

        let decoded = CgapFrameHeader::decode(&frame).expect("layout decode");
        assert_eq!(
            decoded.validate(),
            Err(FrameError::AttestationMismatch),
            "core side, mutation `{name}`"
        );
        assert_eq!(
            ingress(&frame, frame.len(), None),
            CGAP_ERR_P28_SPOLIATION,
            "gate side, mutation `{name}`"
        );
    }
}

#[test]
fn mutation_lying_caller_geometry_refused_fail_closed() {
    // The gate's row-1 fail-closed class: a declared length beyond the
    // slice, or a truncated header, reads nothing and refuses. cgap-core's
    // own decode refuses the truncated header with BufferTooSmall — the
    // truncated-header parity pair.
    let frame = wire_frame_at(64);

    // declared len exceeds the slice.
    assert_eq!(
        ingress(&frame, frame.len() + 1, None),
        CGAP_ERR_INVALID_MAGIC,
        "gate side, declared > actual"
    );
    // truncated header (row 1: len < 128).
    assert_eq!(
        ingress(&frame[..127], 127, None),
        CGAP_ERR_INVALID_MAGIC,
        "gate side, len < 128"
    );
    // cgap-core parity: a truncated header has no CgapFrameHeader to
    // decode — typed BufferTooSmall.
    assert_eq!(
        CgapFrameHeader::decode(&frame[..127]),
        Err(FrameError::BufferTooSmall),
        "core side, len < 128"
    );
}

// ===========================================================================
// The recorded divergence (doc yields; pinned, not silent)
// ===========================================================================

#[test]
fn pinned_divergence_gate_admits_reserved_flag_bits_core_refuses() {
    // A14 §4 row 6 divergence: the dam ADMITS reserved header flag bits
    // (FLAGS_RESERVED_MASK is documented as admit-side-tolerated), while
    // cgap-core §6.3 refuses them (FrameError::ReservedBitsSet, "bits 4-7
    // reserved, must be zero"). This test PINS the divergence on both
    // sides: if either lane changes its documented behavior, this court
    // fails and the divergence must be re-recorded.
    let mut frame = wire_frame_at(64);
    patch(&mut frame, 5, |b| b[0] |= 0x10); // flags bit 4

    // gate side: ADMITTED (A14 row 6, recorded divergence).
    let mut view = cgap_gate::penstock::FrameView { payload: &[] };
    assert_eq!(
        ingress(&frame, frame.len(), Some(&mut view)),
        CGAP_OK,
        "gate side: reserved flag bits are admit-side-tolerated per A14 row 6"
    );
    assert_eq!(view.payload.len(), 64);

    // core side: REFUSED (A04 §6.3 / §8.3).
    let decoded = CgapFrameHeader::decode(&frame).expect("layout decode");
    assert_eq!(
        decoded.validate(),
        Err(FrameError::ReservedBitsSet),
        "core side: reserved flag bits must refuse per A04 §6.3"
    );
}
