//! Shared court fixtures for cgap-core lane A04 (test-only; never compiled
//! into the lib target). Real collaborators only: real heap regions, real
//! subprocess rustc, real cgap-blake3 digests — no mocks (Chicago discipline).

#![allow(dead_code)]

use cgap_core::frame::{
    CgapFrameHeader, P28LicenseAttestation, ATTESTATION_LEN, FLAG_BN3, FRAME_HEADER_LEN,
    FRAME_MAGIC, FRAME_VERSION, LICENSE_FLAG_ROOT, LICENSE_MAGIC,
    SAFE_HARBOR_CEILING_CENTS_MAX,
};

/// §8.2 step 1 payload sizes (exact doc list, incl. the 1 MiB bound).
pub const SIZES: [usize; 8] = [0, 1, 63, 64, 127, 128, 4096, 1024 * 1024];

/// Deterministic xorshift64* payload builder — same seed and size always
/// yield the same bytes (court replayability, §8.2 "deterministic
/// pseudo-random, seeded").
pub fn payload(len: usize) -> Vec<u8> {
    let mut s = 0x9E37_79B9_7F4A_7C15u64 ^ (len as u64).wrapping_mul(0x1000_0000_01B3);
    let mut v = Vec::with_capacity(len);
    for _ in 0..len {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        v.push((s >> 24) as u8);
    }
    v
}

/// 64-byte-aligned region leak for the ring ABI (`&'static mut`). Test-only
/// allocation; the crate source stays alloc-free (source_scan_gate.rs).
pub fn leak_aligned_region(len: usize) -> &'static mut [u8] {
    use std::alloc::{alloc_zeroed, Layout};
    assert!(len > 0 && len % 64 == 0, "region must be a 64-byte multiple");
    unsafe {
        let layout = Layout::from_size_align(len, 64).expect("layout");
        let ptr = alloc_zeroed(layout);
        assert!(!ptr.is_null(), "region alloc failed");
        assert_eq!(ptr as usize % 64, 0, "allocator broke 64-byte alignment");
        std::slice::from_raw_parts_mut(ptr, len)
    }
}

/// Ring region length for `slots` slots each holding `FRAME_HEADER_LEN +
/// payload_len` bytes, rounded to the 64-byte slot stride (§7.2).
pub fn slot_region_len(payload_len: usize, slots: usize) -> usize {
    let frame = FRAME_HEADER_LEN + payload_len;
    let slot = ((frame + 63) / 64 * 64).max(192); // >= 128+1, 64-multiple
    slot * slots
}

/// Valid Root attestation (A17 §2.2: FLAG_ROOT, upstream receipt all-zero,
/// ceiling at the $50M pin). Origin digest is the real BLAKE3 of the frozen
/// license URN (A17 §3: URN carried as de-referenced identifier).
pub fn root_attestation() -> P28LicenseAttestation {
    P28LicenseAttestation {
        license_magic: LICENSE_MAGIC,
        version: 1,
        attestation_flags: LICENSE_FLAG_ROOT,
        safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
        origin_authority_digest: origin_authority_digest(),
        upstream_receipt: [0; 32],
    }
}

pub fn origin_authority_digest() -> [u8; 32] {
    cgap_blake3::hash(URN_LICENSE_P28_SIL.as_bytes())
}

/// FROZEN license URN (docs/jira/v26.11.1/03-rename-receipt.md decision D2).
pub const URN_LICENSE_P28_SIL: &str = "urn:avec:license:p28-sil:v2.1.0-sovereign";

/// Fully valid header over a payload of `payload_byte_len` bytes with the
/// given digest and wire flags — the courts' positive control.
pub fn valid_header(payload_byte_len: u64, digest: [u8; 32], flags: u8) -> CgapFrameHeader {
    CgapFrameHeader {
        magic: FRAME_MAGIC,
        version: FRAME_VERSION,
        flags,
        reserved: [0; 2],
        payload_byte_len,
        oabi_digest: digest,
        attestation: root_attestation(),
    }
}

/// Encode header + payload into one contiguous wire frame (§8.1 wire tier).
pub fn wire_frame(header: &CgapFrameHeader, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0u8; FRAME_HEADER_LEN + payload.len()];
    header.encode_into(&mut frame).expect("frame buffer");
    frame[FRAME_HEADER_LEN..].copy_from_slice(payload);
    frame
}

/// Compile-time size sanity for the fixtures themselves.
const _: () = {
    assert!(FRAME_HEADER_LEN == 128);
    assert!(ATTESTATION_LEN == 80);
};

/// Silence the unused-flag lint for courts that only use some flags.
pub fn bn3_flags() -> u8 {
    FLAG_BN3
}
