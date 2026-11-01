//! The identity court — lane A04 exit gate (§8.2), Wave 2 execution.
//!
//! Payload P (8 doc-pinned sizes) → wire frame F_w (encode) → memory frame
//! F_m (publish/read through the real DmaRing) → bit-for-bit identity over
//! the full frame region, plus each tier's re-derived digest equals the
//! embedded digest (real `cgap-blake3`, the lane-doc-named substrate).
//!
//! Disk tier (Arrow/Parquet) views are owned by lanes A17/A19 and are NOT
//! landed: per §10 ("until then the court runs in wire+memory-only mode,
//! 2-tier mode, receipted as PARTIAL_ALIVE") the court runs 2-tier and
//! prints a loud `[SKIP]` per missing tier — never a silent pass.
//!
//! Fail-loudness (§8.2): on divergence every failure names the tier, the
//! first divergent byte offset, and the per-tier bytes via
//! `TierMismatch`'s `IDENTITY_COURT_FAIL ...` Display line. The mutation
//! tests below prove the court can fail (anti-vacuity, §8.3).
//!
//! §8.3 mutation-row map (where each row executes):
//!   identity flip-1-byte      → `mutation_flip_one_byte_fails_at_named_offset` (here)
//!   identity payload_len +1   → `mutation_payload_len_shift_fails_at_offset_8` (here)
//!   header pad/drop 127/129 B → tests/static_assert_court.rs (real rustc)
//!   magic flip → BadMagic     → src/frame.rs `mutation_magic_flip_refused_badmagic`
//!   flags bit 4 reserved      → src/frame.rs `mutation_reserved_flag_bit_refused`
//!   publish > slot capacity   → tests/unit_dma_ring.rs `publish_oversize_refused_overflow`
//!   read unpublished slot     → tests/unit_dma_ring.rs `read_unpublished_slot_is_none_never_garbage`
//!   attestation corruption    → src/frame.rs `mutation_attestation_corruptions_refused`
//!   payload byte corruption   → src/frame.rs `verify_payload_digest_mismatch_and_len_mismatch`

use cgap_blake3::hash as oabi_digest;
use cgap_core::frame::{CgapFrameHeader, FRAME_HEADER_LEN};
use cgap_core::isomorphism::{identity_assert, Tier};

mod common;
use common::{
    leak_aligned_region, payload, slot_region_len, valid_header, wire_frame, SIZES,
};

#[test]
fn exit_gate_wire_memory_identity_per_size() {
    for &size in SIZES.iter() {
        let payload = payload(size);
        let digest = oabi_digest(&payload);
        let header = valid_header(size as u64, digest, common::bn3_flags());

        // --- wire tier: encode → decode → validate → re-encode identity ----
        let wire = wire_frame(&header, &payload);
        let decoded = CgapFrameHeader::decode(&wire).expect("decode");
        assert_eq!(decoded.validate(), Ok(()), "wire tier size={size}");
        let mut reencoded = [0u8; FRAME_HEADER_LEN];
        decoded.encode_into(&mut reencoded).expect("re-encode");
        assert_eq!(&wire[..FRAME_HEADER_LEN], &reencoded, "size={size}");
        assert_eq!(
            decoded.verify_payload(&payload, oabi_digest),
            Ok(()),
            "wire digest re-derivation size={size}"
        );
        assert_eq!(decoded.payload(&wire).unwrap(), &payload[..]);

        // --- memory tier: real DmaRing claim→publish→read→release ----------
        let region = leak_aligned_region(slot_region_len(size, 2));
        let mut ring = cgap_core::dma_ring::DmaRing::new(region, 1)
            .expect("region partitions into 2 slots");
        {
            let slot = ring.claim().expect("ring has room");
            assert!(wire.len() <= slot.len());
            slot[..wire.len()].copy_from_slice(&wire);
        }
        ring.publish(wire.len()).expect("publish valid frame");
        let (payload_m, header_m) = ring.read().expect("published frame pending");
        assert_eq!(header_m.validate(), Ok(()));
        let mut mem_frame = vec![0u8; FRAME_HEADER_LEN + size];
        header_m.encode_into(&mut mem_frame).expect("encode");
        mem_frame[FRAME_HEADER_LEN..].copy_from_slice(payload_m);
        if let Err(m) = identity_assert(&wire, Tier::Memory, &mem_frame) {
            panic!("tier=memory size={size}: {m}");
        }
        assert_eq!(
            header_m.verify_payload(payload_m, oabi_digest),
            Ok(()),
            "memory digest re-derivation size={size}"
        );
        ring.release().expect("release");
        assert_eq!(ring.pending(), 0);

        // --- disk tiers: loud skip, 2-tier mode (A04 §10) ------------------
        println!(
            "[SKIP] tier=arrow reason=disk-tier view owned by lane A17; 2-tier mode per A04 §10"
        );
        println!(
            "[SKIP] tier=parquet reason=disk-tier page view owned by lane A19; 2-tier mode per A04 §10"
        );
    }
}

#[test]
fn mutation_flip_one_byte_fails_at_named_offset() {
    // §8.3 identity row: flip 1 byte in any tier's layout → court FAILs,
    // names tier + first divergent offset. Flip payload byte 5 (offset 133).
    let size = 4096;
    let payload = payload(size);
    let header = valid_header(size as u64, oabi_digest(&payload), common::bn3_flags());
    let wire = wire_frame(&header, &payload);

    let mut mem_frame = wire.clone();
    let flip_offset = FRAME_HEADER_LEN + 5;
    mem_frame[flip_offset] ^= 0x01;

    let err = identity_assert(&wire, Tier::Memory, &mem_frame).expect_err("must flip");
    assert_eq!(err.tier, Tier::Memory);
    assert_eq!(err.offset, flip_offset);
    assert_eq!(err.wire, wire[flip_offset]);
    assert_eq!(err.mem, wire[flip_offset] ^ 0x01);
    // The fail-loud line (§8.2) names tier, offset, and both bytes.
    let line = err.to_string();
    assert!(line.starts_with("IDENTITY_COURT_FAIL tier=memory offset=133 "), "{line}");
}

#[test]
fn mutation_payload_len_shift_fails_at_offset_8() {
    // §8.3 identity row: shift payload_byte_len by +1 in the header →
    // identity court FAILs at offset 8 (the u64 LE field's first byte).
    let size = 4096;
    let payload = payload(size);
    let mut header = valid_header(size as u64, oabi_digest(&payload), common::bn3_flags());
    header.payload_byte_len += 1;
    let wire = wire_frame(&valid_header(size as u64, oabi_digest(&payload), common::bn3_flags()), &payload);
    let mutated = wire_frame(&header, &payload);

    let err = identity_assert(&wire, Tier::Memory, &mutated).expect_err("must flip");
    assert_eq!(err.tier, Tier::Memory);
    assert_eq!(err.offset, 8, "first divergent byte is payload_byte_len's LSB");
    assert_ne!(err.wire, err.mem);
}

#[test]
fn identity_court_length_divergence_is_loud() {
    // A tier projection that loses the payload tail cannot pass: the court
    // names the prefix-end offset rather than comparing a truncated region.
    let size = 128;
    let payload = payload(size);
    let header = valid_header(size as u64, oabi_digest(&payload), common::bn3_flags());
    let wire = wire_frame(&header, &payload);
    let truncated = &wire[..wire.len() - 1];
    let err = identity_assert(&wire, Tier::Memory, truncated).expect_err("must flip");
    assert_eq!(err.offset, wire.len() - 1);
}
