//! DmaRing unit courts (lane A04 §7/§8.3) — real heap-backed regions, real
//! state assertions, no mocks. Inline src tests are kept alloc-free (the
//! crate source must stay token-clean for tests/source_scan_gate.rs), so the
//! ring's heap-backed courts live here.

use cgap_core::dma_ring::{DmaRing, RingError, CACHE_LINE_BYTES};
use cgap_core::frame::{CgapFrameHeader, FRAME_HEADER_LEN};

mod common;
use common::{leak_aligned_region, payload, valid_header, wire_frame};

fn bn3_header(payload_len: u64) -> CgapFrameHeader {
    valid_header(payload_len, cgap_blake3::hash(&payload(payload_len as usize)), 0x04)
}

/// A 64-aligned region partitioned into exactly `2^slot_pow2` slots, each
/// holding the 128 B header plus `payload_len` payload bytes (rounded to the
/// 64-byte slot stride). One helper so slot counts and `slot_pow2` can never
/// disagree again.
fn ring_region(payload_len: usize, slot_pow2: u8) -> &'static mut [u8] {
    leak_aligned_region(common::slot_region_len(
        payload_len,
        1usize << slot_pow2,
    ))
}

#[test]
fn new_refuses_misaligned_region_fail_closed() {
    // §10: DmaRing::new requires a 64-aligned region; refuses otherwise.
    //
    // Wave-8 repair (A04 defect fix): the old premise — "Vec's backing store
    // carries only 1-alignment" — was address-luck. `[u8]`'s alignment RULE
    // is 1, but the address an allocator hands back is luck (probe, this
    // session: a leaked 4096-byte Vec landed 0xaeb000000, 64-aligned, `new`
    // lawfully ACCEPTED it, and this row failed on the host suite). Same law
    // as the neighboring rows: deterministic regions, never address-luck.
    //
    // Deterministic now: a 64-aligned leak reborrowed at every nonzero offset
    // of the cache-line residue class; each must refuse `MisalignedRegion`.
    // The geometry (4096 B, slot_pow2 2 = four 1024 B slots) is otherwise
    // valid, so ONLY the alignment gate can refuse — the row is load-bearing
    // on exactly that guard.
    for offset in 1..CACHE_LINE_BYTES {
        let aligned = leak_aligned_region(4096 + 64);
        let heap: &'static mut [u8] = &mut aligned[offset..offset + 4096];
        assert_eq!(
            heap.as_ptr() as usize % CACHE_LINE_BYTES,
            offset,
            "premise: the region base is misaligned by construction"
        );
        assert!(
            matches!(
                DmaRing::new(heap, 2),
                Err(RingError::MisalignedRegion)
            ),
            "region at base+{offset} must refuse MisalignedRegion"
        );
    }
}

#[test]
fn new_refuses_undersized_and_undivisible_regions() {
    // Exactly 2 header-sized slots: no payload room → refuse (typed).
    let tight = leak_aligned_region(256);
    assert!(matches!(DmaRing::new(tight, 1), Err(RingError::RegionTooSmall)));

    // 192 aligned B cannot split into 4 x 64-multiple slots → refuse (typed;
    // the aligned leak makes the refusal deterministic, never address-luck).
    let undivisible = leak_aligned_region(192);
    assert!(matches!(
        DmaRing::new(undivisible, 2),
        Err(RingError::RegionTooSmall)
    ));

    // A valid 2-slot region reports its geometry.
    let ok = leak_aligned_region(2 * 192 * 64);
    let ring = DmaRing::new(ok, 1).expect("valid region");
    assert_eq!(ring.slot_size(), 192 * 64);
    assert_eq!(ring.capacity(), 2);
    assert_eq!(ring.pending(), 0);
    assert!(CACHE_LINE_BYTES == 64);
}

#[test]
fn read_unpublished_slot_is_none_never_garbage() {
    // §8.3: read an unpublished slot → None (typed: no frame).
    let region = leak_aligned_region(common::slot_region_len(128, 2));
    let mut ring = DmaRing::new(region, 1).expect("region");
    assert!(ring.read().is_none());
    assert_eq!(ring.pending(), 0);
    // release with nothing pending refuses too (typed, not silent).
    assert_eq!(ring.release(), Err(RingError::ReleaseOnEmpty));
}

#[test]
fn publish_without_claim_refused_typed() {
    let region = ring_region(64, 0);
    let mut ring = DmaRing::new(region, 0).expect("region");
    assert_eq!(ring.publish(FRAME_HEADER_LEN), Err(RingError::PublishWithoutClaim));
    assert!(ring.read().is_none());
}

#[test]
fn publish_oversize_refused_overflow() {
    // §8.3: publish payload len > slot capacity → Overflow; the ring state
    // proves nothing slipped through (head not advanced, no readable frame).
    let region = leak_aligned_region(common::slot_region_len(64, 2));
    let mut ring = DmaRing::new(region, 1).expect("region");
    {
        let slot = ring.claim().expect("room");
        bn3_header(1).encode_into(&mut slot[..FRAME_HEADER_LEN]).expect("encode");
        slot[FRAME_HEADER_LEN] = 0x42;
    }
    assert_eq!(ring.publish(ring.slot_size() + 1), Err(RingError::Overflow));
    // The physical bound is the claim slice itself: exactly slot_size bytes,
    // so a producer cannot write past the slot by construction.
    // Observable ring state: no frame was published by the refused call.
    assert_eq!(ring.pending(), 0);
    assert!(ring.read().is_none());
    // The producer path is undamaged: the same slot publishes validly.
    {
        let slot = ring.claim().expect("re-claim after refusal");
        assert!(slot.iter().all(|&b| b == 0), "claim re-zeroed the slot");
        bn3_header(1).encode_into(&mut slot[..FRAME_HEADER_LEN]).expect("encode");
        slot[FRAME_HEADER_LEN] = 0x42;
    }
    ring.publish(FRAME_HEADER_LEN + 1).expect("valid frame publishes");
    assert_eq!(ring.pending(), 1);
}

#[test]
fn publish_garbage_and_len_mismatch_refused_bad_frame() {
    let region = ring_region(64, 0);
    let mut ring = DmaRing::new(region, 0).expect("region");

    // Garbage header fails the §7.3 "frame must validate" gate.
    {
        let slot = ring.claim().unwrap();
        slot[..FRAME_HEADER_LEN].fill(0xFF);
    }
    assert_eq!(ring.publish(FRAME_HEADER_LEN), Err(RingError::BadFrame));

    // §8.3 frame-header mutation through the ring: magic flip.
    {
        let slot = ring.claim().unwrap();
        let mut h = bn3_header(0);
        h.magic = *b"XGAP";
        h.encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
    }
    assert_eq!(ring.publish(FRAME_HEADER_LEN), Err(RingError::BadFrame));

    // Shorter than a header → refuse before decoding.
    {
        let slot = ring.claim().unwrap();
        bn3_header(0).encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
    }
    assert_eq!(ring.publish(FRAME_HEADER_LEN - 1), Err(RingError::BadFrame));

    // Embedded payload_byte_len disagreeing with the publish length → refuse.
    {
        let slot = ring.claim().unwrap();
        bn3_header(63).encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
    }
    assert_eq!(ring.publish(FRAME_HEADER_LEN + 64), Err(RingError::BadFrame));

    // No refused publish leaked a frame into the consumer side.
    assert_eq!(ring.pending(), 0);
    assert!(ring.read().is_none());
}

#[test]
fn abandoned_claim_leaves_no_stale_bytes() {
    // §10: claim() zero-fills the full slot; an abandoned claim (dropped
    // slice, no publish) cannot leak stale bytes into the next claim.
    let region = ring_region(64, 0);
    let mut ring = DmaRing::new(region, 0).expect("region");
    {
        let slot = ring.claim().unwrap();
        slot.fill(0xFF);
    } // abandoned
    {
        let slot = ring.claim().unwrap();
        assert!(slot.iter().all(|&b| b == 0));
    }
    assert_eq!(ring.pending(), 0);
}

#[test]
fn ring_full_claim_returns_none() {
    let region = leak_aligned_region(common::slot_region_len(8, 2));
    let mut ring = DmaRing::new(region, 1).expect("region");
    for i in 0u8..2 {
        {
            let slot = ring.claim().expect("room before full");
            bn3_header(8).encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
            slot[FRAME_HEADER_LEN..FRAME_HEADER_LEN + 8].copy_from_slice(&[i; 8]);
        }
        ring.publish(FRAME_HEADER_LEN + 8).unwrap();
    }
    assert!(ring.claim().is_none(), "full ring refuses the producer");
    assert_eq!(ring.pending(), 2);
    // Releasing one slot frees exactly one claim.
    ring.release().unwrap();
    assert!(ring.claim().is_some());
}

#[test]
fn roundtrip_bytes_survive_the_handshake() {
    // §8.1 memory-tier identity: what the producer wrote is what the
    // consumer reads — same header fields, same payload bytes.
    let region = leak_aligned_region(common::slot_region_len(200, 2));
    let mut ring = DmaRing::new(region, 1).expect("region");
    let payload = payload(200);
    let wire = wire_frame(&bn3_header(200), &payload);
    {
        let slot = ring.claim().unwrap();
        slot[..wire.len()].copy_from_slice(&wire);
    }
    ring.publish(wire.len()).unwrap();

    let (payload_m, header_m) = ring.read().expect("frame pending");
    assert_eq!(payload_m, &payload[..]);
    assert_eq!(header_m.validate(), Ok(()));
    assert_eq!(header_m.payload_byte_len, 200);
    let mut rebuilt = [0u8; FRAME_HEADER_LEN];
    header_m.encode_into(&mut rebuilt).unwrap();
    assert_eq!(&wire[..FRAME_HEADER_LEN], &rebuilt);
    ring.release().unwrap();
    assert!(ring.read().is_none());
}

#[test]
fn publish_refuses_truncating_wire_lengths() {
    // Wave-5 repair pin (A04 defect fix): the §7.3 publish gate compares the
    // u64 header length WIDENED, never narrowed with `as usize`. On 32-bit
    // targets (riscv32imc, lane DoD 8) a header declaring `2^32 + k` passed
    // this gate against `128 + k` published bytes. These refusal rows are
    // the target-independent law; green on the 64-bit host on both sides of
    // the fix, and the widened form is what makes them 32-bit-exact.
    let region = ring_region(64, 0);
    let mut ring = DmaRing::new(region, 0).expect("region");

    // Embedded 2^32 + 1 against 1 published payload byte -> BadFrame.
    {
        let slot = ring.claim().unwrap();
        let mut h = bn3_header(1);
        h.payload_byte_len = (1u64 << 32) + 1;
        h.encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
    }
    assert_eq!(ring.publish(FRAME_HEADER_LEN + 1), Err(RingError::BadFrame));

    // Embedded u64::MAX against 0 payload bytes -> BadFrame.
    {
        let slot = ring.claim().unwrap();
        let mut h = bn3_header(0);
        h.payload_byte_len = u64::MAX;
        h.encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
    }
    assert_eq!(ring.publish(FRAME_HEADER_LEN), Err(RingError::BadFrame));

    // No refused publish leaked a frame into the consumer side.
    assert_eq!(ring.pending(), 0);
    assert!(ring.read().is_none());
}

#[test]
fn read_refuses_lying_header_written_around_the_api() {
    // Wave-7 pin (A04 defect repair): `read()`'s u64 bound — the wave-5
    // repair's remaining unpinned guard. Defense-in-depth vs a slot written
    // around the ring API: with the gate deleted, M-A (sandbox witness,
    // /tmp/a04-w7-sbx) flipped NOTHING — every court stayed green, so a
    // regression deleting the gate would land silently. This row is the
    // flip witness: gate removed, `read()` builds an out-of-slot slice and
    // PANICS (101) instead of returning the typed no-frame None.
    //
    // Host-invisibility disclosure (same class as the publish-side pin
    // `publish_refuses_truncating_wire_lengths`): the `as usize`-narrowed
    // FORM of this gate is behaviorally identical on the 64-bit host (u64
    // -> usize never truncates there); the widening is load-bearing on
    // 32-bit targets only. This row proves the gate EXISTS and refuses
    // typed on over-capacity declarations; the widened TEXT is pinned by
    // guard_flip_court's anchor self-check (anchor drift fails loud), and
    // row_10 witnesses the removal-class flip via real rustc shims.
    let region = leak_aligned_region(common::slot_region_len(64, 2));
    // Capture the region base before the ring takes ownership — the hostile
    // write below goes straight into the slot bytes, around every ring
    // method (SAFETY: same allocation, 64-aligned leak, test-only; u64
    // store at offset 8 is 8-aligned).
    let base: *mut u8 = region.as_ptr() as *mut u8;
    let mut ring = DmaRing::new(region, 1).expect("region");
    {
        let slot = ring.claim().expect("room");
        bn3_header(1).encode_into(&mut slot[..FRAME_HEADER_LEN]).expect("encode");
        slot[FRAME_HEADER_LEN] = 0x42;
    }
    ring.publish(FRAME_HEADER_LEN + 1).expect("valid frame publishes");
    assert_eq!(ring.pending(), 1);

    // Hostile: declare u64::MAX payload bytes directly in the published
    // slot's header (offset 8, `payload_byte_len` per §6.1).
    unsafe {
        core::ptr::write(base.add(8) as *mut u64, u64::MAX);
    }

    // Typed refusal: no frame, never garbage — and the refusal consumes
    // nothing (the pending frame stays pending; release still discards it).
    assert!(ring.read().is_none(), "lying header must be refused (None), never handed out");
    assert_eq!(ring.pending(), 1, "read() refusal must not consume the frame");
    ring.release().expect("the poisoned frame is discardable");
    assert_eq!(ring.pending(), 0);
}

#[test]
fn new_refuses_slot_pow2_overflow_fail_closed() {
    // Wave-7 pin (A04 defect repair): the `slot_pow2 >= usize::BITS` guard.
    // M-B (sandbox witness, /tmp/a04-w7-sbx) deleted it and flipped NOTHING:
    // `1usize << 64` shift-panicked in debug only because no court ever
    // asked for slot_pow2 = 64. This row pins the typed refusal (and, via
    // guard_flip_court's anchor self-check, the widened guard text).
    let region = leak_aligned_region(common::slot_region_len(64, 2));
    assert!(matches!(
        DmaRing::new(region, u8::MAX),
        Err(RingError::RegionTooSmall)
    ));
}

#[test]
fn wraparound_mask_indexing_two_rounds() {
    // §7.1 power-of-two mask indexing across the wrap boundary.
    let region = leak_aligned_region(common::slot_region_len(4, 2));
    let mut ring = DmaRing::new(region, 1).expect("region");
    for round in 0u8..4 {
        {
            let slot = ring.claim().unwrap();
            bn3_header(4).encode_into(&mut slot[..FRAME_HEADER_LEN]).unwrap();
            slot[FRAME_HEADER_LEN..FRAME_HEADER_LEN + 4].copy_from_slice(&[round; 4]);
        }
        ring.publish(FRAME_HEADER_LEN + 4).unwrap();
        let (payload_m, header_m) = ring.read().unwrap();
        assert_eq!(payload_m, &[round; 4]);
        assert_eq!(header_m.version, 1);
        assert_eq!(header_m.payload_byte_len, 4);
        ring.release().unwrap();
        assert_eq!(ring.pending(), 0);
    }
}
