//! Wave-9 cross-lane integration courts (lane A09, v26.11.1 wave 9 slice:
//! "A04 identity court, A14 dam over A04 frames, A20 1M-row stress, A05
//! power-cut sweep, A07 zero-string gate over all built crates") — the
//! fibo side of each item, against REAL collaborators only: the committed
//! pack bytes on disk, A04's real `cgap-core` frame/attestation/identity
//! surface, A06's real loader, the real hash substrate, and the real
//! emitter binary. No mocks.
//!
//! Slice-item disposition (each names its court):
//! - **A04 identity court** — [`w9_a04_frame_identity_court`]: the pack is
//!   framed by A04's 128 B `CgapFrameHeader` (A17's 80 B attestation
//!   embedded), round-tripped through encode/decode/validate on the wire
//!   tier, identity-asserted across the A04 tiers, and its payload
//!   digest-verified with the real BLAKE3 — the cross-lane identity join
//!   `header.oabi_digest == PACK_DIGEST` is asserted in both directions.
//! - **A14 dam over A04 frames** — UNSUPPORTED for this lane, by design,
//!   with the would-trigger condition named: A14's penstock dam gates
//!   *flows* of A04 frames through `crates/cgap-gate`; a `const` O-ABI
//!   vocabulary pack (one frozen 496-byte artifact, no streams, no ring,
//!   no frame transport) has no flow surface to dam. The A04-frame half
//!   of the item is courted above; the dam half belongs to lanes whose
//!   surfaces emit frames in motion (A14 itself, A19/A17 adapters). This
//!   is a typed UNSUPPORTED, not a skip: re-open this lane's wave-9 slice
//!   only if the pack ever becomes a framed payload in transport.
//! - **A20 1M-row stress** — [`w9_a20_1m_row_stress_court`]: A06's real
//!   loader parses and per-record re-derives 1,000,000 records (48 MB
//!   container) with byte-exact wire round-trip; every corrupted-row
//!   mutation is refused typed with the exact index named.
//! - **A05 power-cut sweep** — [`w9_a05_powercut_sweep_court`]: every
//!   truncation prefix and every single-byte corruption of the real pack
//!   is refused by at least one identity gate (exact-length law, digest
//!   identity, per-record derivation, reserved-zero law) — a power cut at
//!   any instant leaves a state no gate admits, except the complete pack.
//! - **A07 zero-string gate** — [`w9_a07_facade_token_census_court`]: the
//!   facade's `fibo:` token surface (the gate's derivation #4 rule) is
//!   exactly the frozen 9-term corpus and carries zero §2.2-refused
//!   vocabulary; the gate itself is a command receipt (run this session,
//!   see the wave-9 receipt block in `src/lib.rs`).
//!
//! Every in-court mutation is witnessed on REAL mutated bytes (never a
//! stub), matching the wave-4 house pattern; the source-level flip
//! witness for the wave-9 emitter repair lives in the receipt block.

use std::path::PathBuf;

use cgap_blake3::hash;
use cgap_bont::oabi::{
    OabiError, OabiHeaderV1, OabiPackV1, OABI_FLAG_LEXHASH_PRESENT, OABI_FLAG_LITTLE_ENDIAN,
    OABI_MAGIC,
};
use cgap_core::frame::{
    CgapFrameHeader, FrameError, P28LicenseAttestation, ATTESTATION_LEN, FRAME_HEADER_LEN,
    FRAME_MAGIC, FRAME_VERSION, LICENSE_FLAG_ROOT, LICENSE_MAGIC, SAFE_HARBOR_CEILING_CENTS_MAX,
};
use cgap_core::isomorphism::{identity_assert, Tier};
use cgap_fibo::{ClassSlot, PACK_DIGEST};

const BONT_NS_V1: &[u8] = b"bont\x01";

/// §2.2-refused probe corpus — court-owned (same rows as wave4_courts.rs
/// T11/T13; restated independently so this file's predicate is
/// self-contained).
const REFUSED_PROBES: &[&[u8]] = &[
    b"fibo:SecEqu:Equity",         // §2.2 equities
    b"fibo:SecDerv:VanillaOption", // §2.2 derivatives/options
    b"fibo:Loan:ServicingDetail",  // §2.2 loans/loan-servicing
    b"fibo:Be:RegulatoryReport",   // §2.2 regulatory/reporting
    b"fibo:FndRole:AgentRole",     // §2.2 full FND foundations
    b"equity:raw",                 // non-fibo lexeme shape
];

fn pack_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fibo_sec_dbt_v1.oabi")
}

fn pack_bytes() -> Vec<u8> {
    std::fs::read(pack_path()).expect("committed pack must exist")
}

fn lex_hash(lexeme: &[u8]) -> [u8; 32] {
    let mut tagged = Vec::with_capacity(BONT_NS_V1.len() + lexeme.len());
    tagged.extend_from_slice(BONT_NS_V1);
    tagged.extend_from_slice(lexeme);
    hash(&tagged)
}

/// The A09 wave-9 frame fixture: the real committed pack wrapped in a
/// valid A04 `CgapFrameHeader` (root attestation, pack digest embedded).
/// Returns `(frame_bytes, header)`.
fn framed_pack() -> (Vec<u8>, CgapFrameHeader) {
    let bytes = pack_bytes();
    let digest = hash(&bytes);
    assert_eq!(digest, PACK_DIGEST, "pack drift before framing (T7 law)");
    let attestation = P28LicenseAttestation {
        license_magic: LICENSE_MAGIC,
        version: 1,
        attestation_flags: LICENSE_FLAG_ROOT,
        safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_CENTS_MAX,
        origin_authority_digest: hash(b"cgap:fibo:A09:pack"),
        upstream_receipt: [0u8; 32], // Root: zero upstream (A17 §2.2)
    };
    attestation
        .validate()
        .expect("fixture attestation must be valid");
    let header = CgapFrameHeader {
        magic: FRAME_MAGIC,
        version: FRAME_VERSION,
        flags: 0, // no Arrow/Parquet/BN3/GitPack projection flags: raw OABI payload
        reserved: [0u8; 2],
        payload_byte_len: bytes.len() as u64,
        oabi_digest: digest,
        attestation,
    };
    header.validate().expect("fixture header must validate");
    let mut frame = vec![0u8; FRAME_HEADER_LEN + bytes.len()];
    header
        .encode_into(&mut frame)
        .expect("128 B header encodes");
    frame[FRAME_HEADER_LEN..].copy_from_slice(&bytes);
    (frame, header)
}

// ---------------------------------------------------------------------------
// A04 identity court
// ---------------------------------------------------------------------------

/// A04 identity court, fibo side: the committed pack travels the full A04
/// wire tier (encode -> decode -> validate -> payload view -> digest
/// verify) and the full tier-identity assertion — with the cross-lane
/// identity join `header.oabi_digest == cgap_fibo::PACK_DIGEST` pinned in
/// both directions (A04's digest field carries A09's pack identity; A09's
/// committed const equals the digest A04 verifies against).
///
/// Anti-vacuity (each witnessed in-process on real mutated bytes — the
/// court FAILS if any named mutation is NOT refused):
/// (a) payload byte flip -> `DigestMismatch`;
/// (b) magic flip -> `BadMagic`;
/// (c) version bump -> `BadVersion`;
/// (d) reserved flag bit -> `ReservedBitsSet`;
/// (e) over-ceiling attestation -> `AttestationMismatch`;
/// (f) one-byte divergent Memory projection -> `identity_assert` names
///     tier + first divergent offset;
/// (g) truncated frame region -> `PayloadLenMismatch` from `payload()`.
#[test]
fn w9_a04_frame_identity_court() {
    let (frame, header) = framed_pack();
    let payload_len = frame.len() - FRAME_HEADER_LEN;

    // --- the identity join, both directions -------------------------------
    assert_eq!(
        header.oabi_digest,
        PACK_DIGEST,
        "A04 frame digest field must carry A09's committed pack identity"
    );
    assert_eq!(
        hash(&frame[FRAME_HEADER_LEN..]),
        PACK_DIGEST,
        "payload re-hash must equal the committed PACK_DIGEST"
    );

    // --- wire tier round trip ---------------------------------------------
    let decoded = CgapFrameHeader::decode(&frame).expect("framed pack decodes");
    decoded.validate().expect("decoded header validates");
    assert_eq!(decoded, header, "encode/decode round trip is lossless");
    let payload = decoded
        .payload(&frame)
        .expect("declared payload length must be in range");
    assert_eq!(payload.len(), payload_len);
    assert_eq!(payload, &frame[FRAME_HEADER_LEN..]);
    decoded
        .verify_payload(payload, hash)
        .expect("payload digest verifies with the real hash substrate");

    // --- tier identity (A04 §8.1, addr-independent value identity) --------
    // The Memory-tier projection is a fresh allocation holding the same
    // region (what a DMA ring slot hands the consumer: same values,
    // different address).
    let memory_slot = frame.clone();
    identity_assert(&frame, Tier::Memory, &memory_slot)
        .expect("memory projection is byte-identical to the wire region");
    // A re-encoded frame is byte-identical too (deterministic wire form).
    let mut reencoded = vec![0u8; FRAME_HEADER_LEN + payload_len];
    decoded
        .encode_into(&mut reencoded)
        .expect("re-encode of decoded header");
    reencoded[FRAME_HEADER_LEN..].copy_from_slice(payload);
    assert_eq!(
        identity_assert(&frame, Tier::Wire, &reencoded),
        Ok(()),
        "re-encoded wire form is byte-identical (deterministic serialization)"
    );
    // The OABI loader accepts the payload EXACTLY as it arrived through
    // the A04 frame (no per-record transformation on the framed path).
    let reloaded = OabiPackV1::from_bytes(payload).expect("framed payload loads through A06");
    assert_eq!(reloaded.header().slot_count, 9);

    // --- A17 attestation width pin (wave-9 slice names the 80 B spec) ----
    assert_eq!(ATTESTATION_LEN, 80);
    assert_eq!(FRAME_HEADER_LEN, 128);

    // --- anti-vacuity witnesses (real mutated bytes) -----------------------
    // (a) flip one payload byte -> DigestMismatch.
    let mut m = frame.clone();
    let last_payload = m.len() - 1;
    m[last_payload] ^= 0x01;
    let dm = CgapFrameHeader::decode(&m)
        .expect("mutated frame still decodes (layout intact)")
        .validate();
    assert_eq!(dm, Ok(()), "attestation untouched by payload flip");
    let mut m2 = m.clone();
    m2.truncate(FRAME_HEADER_LEN + payload_len);
    let err = header
        .verify_payload(&m2[FRAME_HEADER_LEN..], hash)
        .expect_err("flipped payload byte must fail digest verification");
    assert_eq!(err, FrameError::DigestMismatch);

    // (b) magic flip -> BadMagic.
    let mut m = frame.clone();
    m[0] = b'X';
    let err = CgapFrameHeader::decode(&m)
        .expect("layout decode is validation-free")
        .validate()
        .expect_err("flipped magic must be refused");
    assert_eq!(err, FrameError::BadMagic);

    // (c) version bump -> BadVersion.
    let mut m = frame.clone();
    m[4] = FRAME_VERSION + 1;
    let err = CgapFrameHeader::decode(&m)
        .expect("layout decode is validation-free")
        .validate()
        .expect_err("bumped version must be refused");
    assert_eq!(err, FrameError::BadVersion);

    // (d) reserved flag bit -> ReservedBitsSet.
    let mut m = frame.clone();
    m[5] = 0x10; // FLAG_RESERVED_MASK bit
    let err = CgapFrameHeader::decode(&m)
        .expect("layout decode is validation-free")
        .validate()
        .expect_err("reserved flag bit must be refused");
    assert_eq!(err, FrameError::ReservedBitsSet);

    // (e) over-ceiling attestation -> AttestationMismatch.
    let mut m = frame.clone();
    m[48 + 8..48 + 16].copy_from_slice(&(SAFE_HARBOR_CEILING_CENTS_MAX + 1).to_le_bytes());
    let err = CgapFrameHeader::decode(&m)
        .expect("layout decode is validation-free")
        .validate()
        .expect_err("over-ceiling attestation must be refused");
    assert_eq!(err, FrameError::AttestationMismatch);

    // (f) divergent Memory projection -> tier + first divergent offset.
    let mut m = frame.clone();
    let probe_offset = FRAME_HEADER_LEN + 5;
    m[probe_offset] ^= 0x80;
    let mismatch = identity_assert(&frame, Tier::Memory, &m)
        .expect_err("one divergent byte must be reported");
    assert_eq!(mismatch.tier, Tier::Memory);
    assert_eq!(mismatch.offset, probe_offset);
    assert_eq!(mismatch.wire, frame[probe_offset]);
    assert_eq!(mismatch.mem, frame[probe_offset] ^ 0x80);

    // (g) truncated frame region -> PayloadLenMismatch from payload().
    let mut m = frame.clone();
    m.truncate(frame.len() - 1);
    let decoded_short = CgapFrameHeader::decode(&m).expect("header still present");
    let err = decoded_short
        .payload(&m)
        .expect_err("declared length overruns truncated region");
    assert_eq!(err, FrameError::PayloadLenMismatch);
}

// ---------------------------------------------------------------------------
// A20 1M-row stress
// ---------------------------------------------------------------------------

/// A20 1M-row stress, fibo side: A06's REAL loader (`OabiPackV1::from_bytes`)
/// parses and validates a 1,000,000-record synthetic pack (48 MB container:
/// 64 B header + 1,000,000 x 48 B records), then the full per-record §6
/// obligation (`SlotId == truncate64(LexHash)`) is swept over all rows, and
/// the serialized wire form is byte-identical to the source bytes (the A04
/// triumvirate echo: serialize == disk == memory view).
///
/// The synthetic corpus is derived, never hand-minted: row `i`'s lexeme is
/// `fibo:W9Stress:row{,i:08x}` hashed through the real substrate under the
/// A06 namespace tag. Rows cycle the frozen dense ClassId space
/// (`[0, 0x22)`) and the currency EnumId space (`[0, 0x034B)`), so every
/// row is a valid member of the frozen spaces, at 1M-row scale.
///
/// This is a CORRECTNESS court at 1M scale; elapsed time is printed as an
/// advisory measurement (stated method: whole-court wall clock in the dev
/// profile), never a gate — perf numbers are advisory, conformance is the
/// deliverable.
///
/// Anti-vacuity (witnessed in-process on real mutated bytes):
/// (a) slot byte flipped in the LAST record -> `SlotDerivationMismatch`
///     naming exactly index 999_999;
/// (b) lex byte flipped at index 500_000 -> `SlotDerivationMismatch` at
///     exactly that index;
/// (c) one-byte truncation -> typed `BadLength` (exact-length law);
/// (d) header count/length disagreement (`slot_count` overstated) ->
///     typed `BadLength`.
#[test]
fn w9_a20_1m_row_stress_court() {
    use std::time::Instant;

    const ROWS: u32 = 1_000_000;
    const CLASS_COUNT: u32 = 0x22; // frozen dense space [0, 0x22)
    const ENUM_COUNT: u32 = 0x034B; // currency family [0, 0x034B)

    let t0 = Instant::now();
    let header = OabiHeaderV1 {
        magic: OABI_MAGIC,
        header_len: 64,
        version_major: 1,
        version_minor: 0,
        flags: OABI_FLAG_LITTLE_ENDIAN | OABI_FLAG_LEXHASH_PRESENT,
        class_count: CLASS_COUNT,
        enum_count: ENUM_COUNT,
        slot_count: ROWS,
        record_size: 48,
        reserved: [0u8; 28],
    };
    let mut buf: Vec<u8> = Vec::with_capacity(64 + ROWS as usize * 48);
    buf.extend_from_slice(&header.to_bytes());
    for i in 0..ROWS {
        let lexeme = format!("fibo:W9Stress:row{i:08x}");
        let lex = lex_hash(lexeme.as_bytes());
        let slot = u64::from_le_bytes(lex[0..8].try_into().expect("32-byte digest"));
        buf.extend_from_slice(&slot.to_le_bytes());
        buf.extend_from_slice(&(i % CLASS_COUNT).to_le_bytes());
        buf.extend_from_slice(&((i % ENUM_COUNT) as u16).to_le_bytes());
        buf.extend_from_slice(&[0u8; 2]); // pad
        buf.extend_from_slice(&lex);
    }
    let build_ms = t0.elapsed().as_millis();

    // --- the stress: real loader over 48 MB, full validation ---------------
    let t1 = Instant::now();
    let pack = OabiPackV1::from_bytes(&buf).expect("1M-row synthetic pack loads");
    assert_eq!(pack.header().slot_count, ROWS);
    assert_eq!(pack.header().class_count, CLASS_COUNT);
    assert_eq!(pack.header().enum_count, ENUM_COUNT);
    assert_eq!(pack.records().len() as u64, ROWS as u64);

    // Full per-record re-derivation sweep (§6 loader obligation at scale).
    for (i, rec) in pack.records().iter().enumerate() {
        rec.verify_slot_derivation()
            .unwrap_or_else(|e| panic!("row {i} derivation broke under stress: {e:?}"));
    }
    let sweep_ms = t1.elapsed().as_millis();

    // Wire byte-identity round trip (triumvirate echo).
    assert_eq!(
        pack.to_bytes(),
        buf,
        "serialized wire form must be byte-identical to the source bytes"
    );

    // Positive control: row 0's stored slot is the derivation of its lexeme.
    let rec0 = &pack.records()[0];
    let lex0 = lex_hash(b"fibo:W9Stress:row00000000");
    assert_eq!(rec0.slot.0, u64::from_le_bytes(lex0[0..8].try_into().expect("32-byte digest")));

    // Advisory timing only (stated method; never a gate).
    println!(
        "w9 1M-row stress: build {build_ms} ms, load+sweep {sweep_ms} ms ({} rows, 48 MiB container, dev profile)",
        ROWS
    );

    // --- anti-vacuity witnesses (real mutated bytes) ------------------------
    // (a) flip a slot byte in the LAST record -> exact index named.
    let record_base = |i: u32| 64 + i as usize * 48;
    let mut m = buf.clone();
    let last = record_base(ROWS - 1);
    m[last] ^= 0x01;
    let err = OabiPackV1::from_bytes(&m).expect_err("corrupted last-row slot must be refused");
    match err {
        OabiError::SlotDerivationMismatch { index, .. } => {
            assert_eq!(index as u64, (ROWS - 1) as u64, "must name the exact row");
        }
        other => panic!("expected SlotDerivationMismatch, got {other:?}"),
    }

    // (b) flip a LexHash byte at index 500_000 -> exact index named.
    let mut m = buf.clone();
    let mid_lex = record_base(500_000) + 16;
    m[mid_lex] ^= 0x01;
    let err = OabiPackV1::from_bytes(&m).expect_err("corrupted mid-row lex must be refused");
    match err {
        OabiError::SlotDerivationMismatch { index, .. } => {
            assert_eq!(index as u64, 500_000u64, "must name the exact row");
        }
        other => panic!("expected SlotDerivationMismatch, got {other:?}"),
    }

    // (c) one-byte truncation -> typed BadLength (exact-length law).
    let err = OabiPackV1::from_bytes(&buf[..buf.len() - 1])
        .expect_err("truncated container must be refused");
    assert!(matches!(err, OabiError::BadLength { .. }));

    // (d) overstated slot_count -> typed BadLength.
    let mut m = buf.clone();
    m[28..32].copy_from_slice(&(ROWS + 1).to_le_bytes());
    let err = OabiPackV1::from_bytes(&m).expect_err("overstated count must be refused");
    assert!(matches!(err, OabiError::BadLength { .. }));
}

// ---------------------------------------------------------------------------
// A05 power-cut sweep
// ---------------------------------------------------------------------------

/// A05 power-cut sweep, fibo side: the pack's committed bytes are the only
/// admissible on-disk state. A crash mid-write can leave (i) a truncated
/// prefix or (ii) a same-length torn sector; the sweep walks EVERY offset
/// of the real 496-byte pack and proves both classes are refused by at
/// least one identity gate — exact-length law, per-record derivation,
/// reserved-zero law, and the digest identity anchor
/// (`hash(bytes) == PACK_DIGEST`) that catches what the structural loader
/// cannot see (pad bytes are loader-invisible; the digest gate refuses
/// them).
///
/// Layering witnessed in-court: a pad-byte corruption LOADS clean
/// (`from_bytes == Ok`) and is refused ONLY by the digest anchor — the
/// loader and the identity anchor are complementary, and the sweep's
/// predicate is the conjunction (refused by at least one gate).
///
/// Producer side (this wave's repair): the emitter replaces the
/// destination only via `sync_all` + `rename(2)` after a full temporary
/// write, so a cut during emit leaves the previous committed pack intact
/// (or, on first emit, no pack) — never a torn pack at the committed
/// path. A torn temporary file is orphaned garbage outside the identity
/// surface and is refused by every gate above if it is ever mistaken for
/// the pack. The atomic path is exercised on a fresh sandbox copy (see
/// the wave-9 receipt block in `src/lib.rs`).
///
/// Anti-vacuity: the sweep's predicate is exercised on the FULL pack in
/// the same court (all gates accept exactly the complete bytes); if the
/// predicate refused everything OR accepted anything incomplete, the
/// court fails. The source-level flip witness for the emitter repair is
/// recorded in the receipt block.
#[test]
fn w9_a05_powercut_sweep_court() {
    let bytes = pack_bytes();
    let n = bytes.len();
    assert_eq!(n, 64 + 9 * 48, "pack length drifted");

    // Anchor: the complete pack passes EVERY gate.
    assert!(OabiPackV1::from_bytes(&bytes).is_ok(), "complete pack loads");
    assert_eq!(hash(&bytes), PACK_DIGEST, "complete pack digests true");

    // Sweep 1 — truncation at EVERY offset: a prefix is never admitted.
    for cut in 0..n {
        let prefix = &bytes[..cut];
        let loader = OabiPackV1::from_bytes(prefix);
        assert!(
            loader.is_err(),
            "power-cut law violated: {cut}-byte prefix was admitted"
        );
        assert_ne!(
            hash(prefix),
            PACK_DIGEST,
            "digest anchor accepted a {cut}-byte prefix"
        );
    }

    // Sweep 2 — single-byte corruption at EVERY offset: never admitted by
    // the conjunction of gates.
    for off in 0..n {
        let mut torn = bytes.clone();
        torn[off] ^= 0x01;
        let digest_ok = hash(&torn) == PACK_DIGEST;
        assert!(
            !digest_ok,
            "digest anchor accepted a 1-byte corruption at offset {off}"
        );
        // Layering: record which gates catch each offset class. Pad bytes
        // and reserved bytes are the discriminating classes:
        // - reserved bytes (36..64) -> loader refuses (ReservedNonZero);
        // - pad bytes (record +14..+16) -> loader ACCEPTS, digest refuses;
        // - every other byte -> loader's derivation/range/length laws
        //   refuse it (or the digest anchor does).
        let in_reserved = (36..64).contains(&off);
        let in_pad = off >= 64 && (off - 64) % 48 >= 14 && (off - 64) % 48 < 16;
        let loader_ok = OabiPackV1::from_bytes(&torn).is_ok();
        if in_reserved {
            assert!(!loader_ok, "reserved-byte corruption at {off} must be loader-refused");
        }
        if in_pad {
            assert!(
                loader_ok,
                "pad byte at {off} should be loader-invisible (layering witness)"
            );
            assert!(
                !digest_ok,
                "pad byte at {off} must be refused by the digest anchor"
            );
        }
    }

    // The pad-byte layering witness, spelled out on one offset: loader
    // accepts, digest anchor refuses — the conjunction is the gate.
    let mut pad_torn = bytes.clone();
    pad_torn[64 + 14] ^= 0x01; // record 0, pad byte 0
    assert!(
        OabiPackV1::from_bytes(&pad_torn).is_ok(),
        "layering precondition: pad corruption loads clean"
    );
    assert_ne!(
        hash(&pad_torn),
        PACK_DIGEST,
        "layering: digest anchor must refuse what the loader cannot see"
    );
}

// ---------------------------------------------------------------------------
// A07 zero-string gate — facade token census
// ---------------------------------------------------------------------------

/// A07 zero-string gate, fibo side: the facade's `fibo:` token surface —
/// what the gate's derivation #4 harvests from `ontologies/*/src/lib.rs`
/// (`fibo:` followed by module/class segments) — is EXACTLY the frozen
/// 9-term corpus, and the facade carries ZERO §2.2-refused vocabulary.
/// The gate binary itself is a command receipt (`python3
/// scripts/zero_string_gate.py`, run this session; receipt block in
/// `src/lib.rs`); this court pins the lane-side token law the gate
/// depends on, in-repo.
///
/// Anti-vacuity: the census predicate is exercised both directions —
/// (a) every frozen corpus token IS found (a facade that lost its corpus
/// fails); (b) every §2.2-refused probe is NOT found; (c) the token
/// count is exact (an ADDED token — scope creep past the §2 refusal
/// list — fails the census even if benign-looking).
#[test]
fn w9_a07_facade_token_census_court() {
    let src_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let src = std::fs::read_to_string(&src_path).expect("facade source must be readable");

    // The gate's derivation #4 token rule: `fibo:` + `:`-separated
    // alphanumeric segments (mirrors FIBO_TOKEN_RE in the gate).
    let mut tokens: Vec<String> = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while i + 5 <= b.len() {
        if &b[i..i + 5] == b"fibo:" {
            let start = i;
            let mut j = i + 5;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b':') {
                j += 1;
            }
            // Trim a trailing colon (a segment separator into prose).
            let mut end = j;
            while end > start + 5 && b[end - 1] == b':' {
                end -= 1;
            }
            if end > start + 5 {
                tokens.push(String::from_utf8_lossy(&b[start..end]).into_owned());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    let mut sorted = tokens.clone();
    sorted.sort();
    sorted.dedup();
    // The gate's rule is set semantics (`vocab[tok] = ...` over dict keys —
    // duplicates collapse; lib.rs documents each lexeme twice: doc comment
    // + const context). The census is over the SET, mirroring the gate;
    // a duplicate is not a drift.

    let corpus: Vec<String> = [
        "fibo:FndUtlCur:USD",
        "fibo:FndUtlCur:EUR",
        "fibo:FndUtlCur:JPY",
        "fibo:SecDbt:CorporateBond",
        "fibo:SecDbt:Bond",
        "fibo:SecDbt:DebtInstrument",
        "fibo:ClAcct:AccountsReceivable",
        "fibo:SecDbt:hasMaturityDate",
        "fibo:SecDbt:hasCouponRate",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut corpus_sorted = corpus.clone();
    corpus_sorted.sort();

    // (a) exact set equality: the facade's token surface IS the corpus.
    let mut found = sorted.clone();
    found.sort();
    assert_eq!(
        found, corpus_sorted,
        "facade token surface drifted from the frozen corpus"
    );

    // (b) zero refused vocabulary in the facade.
    for probe in REFUSED_PROBES {
        let probe = String::from_utf8_lossy(probe);
        assert!(
            !src.contains(probe.as_ref()),
            "§2.2-refused vocabulary in the facade: {probe}"
        );
    }

    // (c) exact count: nine terms, no additions.
    assert_eq!(sorted.len(), 9, "facade carries extra fibo: tokens: {sorted:?}");

    // Facade ABI surface pins (the slice's cross-lane authority list):
    // A06 SlotId width stays 64, the pack keeps its committed identity.
    assert_eq!(cgap_fibo::SLOT_ID_BITS, 64);
    assert_eq!(hash(&pack_bytes()), PACK_DIGEST);
    let _ = ClassSlot(0); // type present
}
