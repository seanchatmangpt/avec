//! Lane A18 — ASC 450-20 / SOX 404 liability-proof emitter: full mechanism.
//!
//! Spec: `docs/jira/v26.11.1/lanes/A18-asc450-proof.md` §4 (ADR), §5 (168-byte
//! record, frozen at Wave 1), §6 (courts C1-C7). Wave 1 froze the ABI (struct
//! + consts + layout pins). Wave 3 (this file) lands the full lane scope: the
//! [`Asc450Meter`] accumulator with checked, overflow-rejecting accounting
//! (§4.3), boundary-exact threshold semantics — emit iff `prev <= ceiling &&
//! next > ceiling`, never at `next == ceiling` (§4.2) — the one-shot latch
//! with monotone seq (§4.2/§4.4), the proof digest chain (`prev_proof_digest`
//! = BLAKE3 over the full 168 B of the prior proof; genesis all-zero), the
//! sealed meter state with reset detection (§4.4), the seal bridge to A16's
//! sealer machinery, and the court suite C1-C7 (§6) with named anti-vacuity
//! mutations.
//!
//! What "signed" means here (divergence D3, recorded in the lane receipt):
//! the A18 doc §5 says "Ed25519-class"; no asymmetric primitive exists in
//! this fan-out (A16's ML-DSA-65 PQC seal is its Wave-3 gate; A16 landed the
//! §4-UNSUPPORTED-clause keyed tier instead). Hand-rolling Ed25519 is
//! rejected by A16's ADR. The seal bridge therefore binds A16's REAL landed
//! machinery: keyed BLAKE3 (kernel = cgap-blake3, keyed engine =
//! cgap-crypto) under the lease epoch's `issued_root`, domain-separated,
//! 64 B = two 32-B keyed digests. Every tamper-evidence law below (C2's
//! any-flip-fails sweep) holds fully; what is NOT claimed is asymmetric
//! non-repudiation: verification requires the epoch key (MAC, not
//! signature). The type is named `LeaseEpochSealer`, not "Ed25519", so the
//! honest scope is in the name.
//!
//! `SignedProof` is the 168-byte fixed-layout record emitted once per
//! safe-harbor ceiling crossing (§5). It is a signed technical receipt of a
//! threshold crossing only; per lane doc §2 it carries no probability or
//! estimability fields and asserts nothing about legal character.
//!
//! Crate-level decisions (manifest, module registry) are A17's authority.
//! The default ceiling is pinned equal to A17's frozen
//! `SAFE_HARBOR_CEILING_MAX_CENTS` so the two ABI freezes cannot drift.
//!
//! Wave 9 (cross-lane integration slice) adds three courts binding this
//! module to the fleet's authoritative disciplines — cite, not redefine:
//! A04 identity (identity is the bytes; the chain anchor re-derives from
//! the A17 80-B record and every bit of it counts), A05 power-cut
//! (a cut at any point of a run is either byte-transparent through the
//! sealed state or byte-visibly a fork — never silent), and A20 scale
//! (1,000,000-event stress with exact terminal-state asserts). The A07
//! zero-string gate runs over the built workspace crates as a lane
//! command (artifact scan; not expressible as an in-module court).

/// offset 0, 4 B — magic "A450" stored little-endian: wire bytes
/// `0x30 0x35 0x34 0x41` (§5), i.e. `u32::from_le_bytes(ASC450_MAGIC)` is
/// `0x4134_3530` = ASCII 'A','4','5','0' MSB-to-LSB.
pub const ASC450_MAGIC: [u8; 4] = [0x30, 0x35, 0x34, 0x41];

/// offset 4, 2 B — u16 LE, currently 1 (§5). A version bump is a new record
/// shape, not a patch of this one.
pub const RECORD_VERSION: u16 = 1;

/// offset 6, 2 B — reserved, MUST be 0 (§5).
pub const RESERVED: u16 = 0;

/// offset 104 — the seal covers bytes 0..104 exactly (§5); the record's
/// own signature field is outside the sealed span. Any byte flip anywhere in
/// 0..168 must fail verification (§5, court C2).
pub const SIGNED_SPAN: usize = 104;

/// Total record size in bytes (§5 offset table, end at 168).
pub const PROOF_LEN: usize = 168;

/// offset 72 field value at meter genesis (§5): `prev_proof_digest` is
/// all-zero for a meter's first proof.
pub const GENESIS_PREV_PROOF_DIGEST: [u8; 32] = [0u8; 32];

/// Default `safe_harbor_ceiling_cents` (§1): $50,000,000 in cents. Pinned
/// equal to A17's `SAFE_HARBOR_CEILING_MAX_CENTS` — the ceiling this module
/// meters can never default above the attestation format's frozen maximum.
pub const DEFAULT_SAFE_HARBOR_CEILING_CENTS: u64 = 5_000_000_000;

// ===========================================================================
// Wave 3 — errors (§4.2/§4.3 typed rejection; never a silent drop)
// ===========================================================================

/// Typed failure of the emitter path (§4.2 overflow rejection, §4.3
/// wrap-reject, §6 court channels). No variant is recoverable-by-ignore:
/// every caller-visible error leaves the meter byte-unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asc450Error {
    /// A checked add would overflow (§4.2/§4.3): the event is REJECTED,
    /// accumulator and all counters unchanged, nothing emitted. Saturation is
    /// rejected by design — a saturated add could mask a crossing near the
    /// exact boundary this module exists to watch (§4.2).
    Overflow,
    /// Meter genesis refused: the supplied A17 attestation failed its
    /// intrinsic validation (A17 §5 steps 2-5, 8). Fail-closed: the proof
    /// chain never anchors to a spoliated record (§4.4 countermeasure 1).
    AttestationInvalid,
    /// A parsed proof record is structurally invalid (magic, version, or
    /// reserved field) or has the wrong length (§5 shape law).
    MalformedProof(&'static str),
    /// Seal verification failed: the recomputed epoch binding over bytes
    /// 0..104 does not match the record's signature field — some covered byte
    /// was mutated, or the record was sealed under a different lease epoch
    /// (court C2).
    SealVerifyFailed,
    /// A sealed meter state failed verification (forged/mutated/foreign
    /// epoch) and a recovery from it was REFUSED (§4.4 countermeasure 3,
    /// fail-closed).
    SealedStateInvalid,
}

impl core::fmt::Display for Asc450Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Asc450Error::Overflow => f.write_str(
                "asc450: checked add would overflow; event rejected, meter \
                 unchanged (§4.2 — saturation must never mask a crossing)",
            ),
            Asc450Error::AttestationInvalid => f.write_str(
                "asc450: meter genesis refused; attestation failed intrinsic \
                 validation (§4.4-1 fail-closed)",
            ),
            Asc450Error::MalformedProof(why) => write!(
                f,
                "asc450: proof record structurally invalid: {why} (§5)"
            ),
            Asc450Error::SealVerifyFailed => f.write_str(
                "asc450: epoch seal verification failed over bytes 0..104 \
                 (§5, court C2)",
            ),
            Asc450Error::SealedStateInvalid => f.write_str(
                "asc450: sealed meter state failed verification; recovery \
                 refused fail-closed (§4.4-3)",
            ),
        }
    }
}

impl std::error::Error for Asc450Error {}

// ===========================================================================
// Wave 3 — sealed meter state (§4.4 countermeasure 3; layout divergence D4)
// ===========================================================================

/// Sealed-state magic: `"A4MS"` (ASC-450 meter state). The A18 doc gives the
/// sealed state's semantics (§4.4) but no byte layout — the layout is pinned
/// by this lane (divergence D4) with the same discipline as §5.
pub const STATE_MAGIC: [u8; 4] = *b"A4MS";

/// Sealed-state record version, currently 1 (divergence D4).
pub const STATE_VERSION: u16 = 1;

/// Sealed-state reserved field, MUST be 0 (divergence D4).
pub const STATE_RESERVED: u16 = 0;

/// Byte 40 of the sealed state: latch flag. `0` = never emitted, `1` =
/// latched (a crossing proof was emitted). Any other value is structural
/// spoliation (divergence D4).
pub const STATE_LATCHED_CLEAR: u8 = 0;
/// Latch-set byte value (divergence D4).
pub const STATE_LATCHED_SET: u8 = 1;

/// End of the sealed-state sealed span (divergence D4): everything before
/// the signature field.
pub const STATE_SIGNED_SPAN: usize = 105;

/// Total sealed-state record size (divergence D4).
pub const STATE_LEN: usize = 169;

/// 169-byte sealed meter state (`#[repr(C, packed)]`, divergence D4).
///
/// The §4.4 countermeasure-3 artifact: the sealer signs this digest on every
/// emission and on every explicit [`Asc450Meter::seal_state`]; a consumer can
/// demand a fresh sealed state at any time, and a reset between two sealed
/// states shows as a non-monotone `accumulated_cents` at equal-or-higher
/// `last_seq` between two valid seals (§4.4; court C2b detects it from these
/// bytes alone).
///
/// Fixed layout, little-endian, fully occupied (offsets 0-3, 4-5, 6-7, 8-15,
/// 16-23, 24-31, 32-39, 40, 41-72, 73-104, 105-168 — no padding holes).
#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SealedMeterState {
    /// offset 0, 4 B — `STATE_MAGIC`
    pub magic: [u8; 4],
    /// offset 4, 2 B — u16 LE, currently 1
    pub version: u16,
    /// offset 6, 2 B — u16 LE, MUST be 0
    pub reserved: u16,
    /// offset 8, 8 B — u64 LE; the meter's configured ceiling
    pub ceiling_cents: u64,
    /// offset 16, 8 B — u64 LE; `accumulated_cents` at seal time
    pub accumulated_cents: u64,
    /// offset 24, 8 B — u64 LE; accepted-event count at seal time
    pub event_count: u64,
    /// offset 32, 8 B — u64 LE; monotone event seq at seal time
    pub last_seq: u64,
    /// offset 40, 1 B — latch flag (`STATE_LATCHED_*`)
    pub latched: u8,
    /// offset 41, 32 B — BLAKE3 of the A17 P28 attestation (matches the
    /// meter's genesis anchor)
    pub attestation_digest: [u8; 32],
    /// offset 73, 32 B — digest of the last proof this meter emitted
    /// (genesis all-zero)
    pub prev_proof_digest: [u8; 32],
    /// offset 105, 64 B — epoch seal over bytes 0..105
    pub signature: [u8; 64],
}

// Sealed-state layout pins (divergence D4): same compile-time discipline as
// the §5 record — any widen/reorder fails the build.
const _: () = assert!(core::mem::size_of::<SealedMeterState>() == STATE_LEN);
const _: () = assert!(STATE_LEN == 169);
const _: () = assert!(core::mem::align_of::<SealedMeterState>() == 1);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, magic) == 0);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, version) == 4);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, reserved) == 6);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, ceiling_cents) == 8);
const _: () = assert!(
    core::mem::offset_of!(SealedMeterState, accumulated_cents) == 16
);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, event_count) == 24);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, last_seq) == 32);
const _: () = assert!(core::mem::offset_of!(SealedMeterState, latched) == 40);
const _: () = assert!(
    core::mem::offset_of!(SealedMeterState, attestation_digest) == 41
);
const _: () = assert!(
    core::mem::offset_of!(SealedMeterState, prev_proof_digest) == 73
);
const _: () = assert!(
    core::mem::offset_of!(SealedMeterState, signature) == STATE_SIGNED_SPAN
);
const _: () = assert!(
    4 + 2 + 2 + 8 + 8 + 8 + 8 + 1 + 32 + 32 + 64 == STATE_LEN,
    "sealed-state fields must fully occupy the record (no padding holes)"
);
const _: () = assert!(STATE_MAGIC.len() == 4);

impl SealedMeterState {
    /// Canonical 169-byte encoding (fixed order, LE fields, no choices).
    /// Packed-struct reads are by-value copies only.
    pub fn to_bytes(&self) -> [u8; STATE_LEN] {
        let (
            magic,
            version,
            reserved,
            ceiling,
            accumulated,
            event_count,
            last_seq,
            latched,
            att,
            prev,
            sig,
        ) = (
            self.magic,
            self.version,
            self.reserved,
            self.ceiling_cents,
            self.accumulated_cents,
            self.event_count,
            self.last_seq,
            self.latched,
            self.attestation_digest,
            self.prev_proof_digest,
            self.signature,
        );
        let mut b = [0u8; STATE_LEN];
        b[0..4].copy_from_slice(&magic);
        b[4..6].copy_from_slice(&version.to_le_bytes());
        b[6..8].copy_from_slice(&reserved.to_le_bytes());
        b[8..16].copy_from_slice(&ceiling.to_le_bytes());
        b[16..24].copy_from_slice(&accumulated.to_le_bytes());
        b[24..32].copy_from_slice(&event_count.to_le_bytes());
        b[32..40].copy_from_slice(&last_seq.to_le_bytes());
        b[40] = latched;
        b[41..73].copy_from_slice(&att);
        b[73..105].copy_from_slice(&prev);
        b[105..169].copy_from_slice(&sig);
        b
    }

    /// Decodes the canonical 169-byte encoding. Structural, fail-closed:
    /// wrong length, foreign magic/version, nonzero reserved, or a latch
    /// byte outside `STATE_LATCHED_{CLEAR,SET}` is refused.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Asc450Error> {
        if bytes.len() != STATE_LEN {
            return Err(Asc450Error::MalformedProof("state length"));
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if magic != STATE_MAGIC {
            return Err(Asc450Error::MalformedProof("state magic"));
        }
        if u16::from_le_bytes([bytes[4], bytes[5]]) != STATE_VERSION {
            return Err(Asc450Error::MalformedProof("state version"));
        }
        if u16::from_le_bytes([bytes[6], bytes[7]]) != STATE_RESERVED {
            return Err(Asc450Error::MalformedProof("state reserved"));
        }
        let latched = bytes[40];
        if latched != STATE_LATCHED_CLEAR && latched != STATE_LATCHED_SET {
            return Err(Asc450Error::MalformedProof("state latch byte"));
        }
        Ok(Self {
            magic,
            version: u16::from_le_bytes([bytes[4], bytes[5]]),
            reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
            ceiling_cents: u64::from_le_bytes(
                bytes[8..16].try_into().expect("8..16 is 8 bytes"),
            ),
            accumulated_cents: u64::from_le_bytes(
                bytes[16..24].try_into().expect("16..24 is 8 bytes"),
            ),
            event_count: u64::from_le_bytes(
                bytes[24..32].try_into().expect("24..32 is 8 bytes"),
            ),
            last_seq: u64::from_le_bytes(
                bytes[32..40].try_into().expect("32..40 is 8 bytes"),
            ),
            latched,
            attestation_digest: bytes[41..73].try_into().expect("41..73 is 32 B"),
            prev_proof_digest: bytes[73..105].try_into().expect("73..105 is 32 B"),
            signature: bytes[105..169].try_into().expect("105..169 is 64 B"),
        })
    }

    /// Recomputes the epoch seal over bytes 0..105 and compares it to the
    /// record's signature field. PURE — mutates nothing, consumes no lease
    /// authority.
    pub fn verify<S: ProofSealer>(&self, sealer: &S) -> bool {
        let b = self.to_bytes();
        let mut span = [0u8; STATE_SIGNED_SPAN];
        span.copy_from_slice(&b[0..STATE_SIGNED_SPAN]);
        let sig = self.signature;
        sealer.verify_state(&span, &sig)
    }
}

// ===========================================================================
// Wave 3 — seal bridge (§4 "Seal bridge — trait"; divergence D3)
// ===========================================================================

/// Domain-separation tags for the keyed epoch seal (divergence D3). All 20
/// bytes; the tag prefixes every keyed digest so a proof seal can never be
/// replayed as a state seal or vice versa.
pub const SEAL_DOMAIN_PROOF: [u8; 20] = *b"cgap-asc450/proof-v1";
/// State-seal domain tag (divergence D3).
pub const SEAL_DOMAIN_STATE: [u8; 20] = *b"cgap-asc450/state-v1";
/// Meta-seal domain tag: the second keyed digest binds the first (chained,
/// not parallel claims — A16 §6 discipline).
pub const SEAL_DOMAIN_META: [u8; 20] = *b"cgap-asc450/meta-v1\x00";

/// The §4 seal bridge: seals fixed-span bytes into the 64-B signature field.
///
/// Contract: [`ProofSealer::seal`] mints authority (may consume lease
/// ticks); [`ProofSealer::verify`] is PURE — recompute + compare, mutates
/// nothing, consumes nothing. The [`LeaseEpochSealer`] below is the real A16
/// binding (divergence D3); the trait seam keeps the court suite valid for
/// A16's future PQC sealer.
pub trait ProofSealer {
    /// Seal the 104-B signed span of a [`SignedProof`].
    fn seal(&mut self, signed: &[u8; SIGNED_SPAN]) -> [u8; 64];
    /// Pure verification of a proof seal (no authority consumed).
    fn verify(&self, signed: &[u8; SIGNED_SPAN], signature: &[u8; 64]) -> bool;
    /// Seal the 105-B signed span of a [`SealedMeterState`].
    fn seal_state(&mut self, signed: &[u8; STATE_SIGNED_SPAN]) -> [u8; 64];
    /// Pure verification of a state seal (no authority consumed).
    fn verify_state(
        &self,
        signed: &[u8; STATE_SIGNED_SPAN],
        signature: &[u8; 64],
    ) -> bool;
}

/// The real A16-bound sealer (divergence D3): keyed BLAKE3 under the lease
/// epoch's `issued_root`, with A16's §5 lease authority — every seal consumes
/// exactly one [`Lease`] tick, and exhaustion TRAPS (A16 §5 trap discipline:
/// minting a sealed artifact with spent authority is an invariant failure,
/// not a recoverable condition). Verification is pure and consumes nothing.
///
/// Composition: `key1 = keyed_blake3(SEAL_DOMAIN_* || signed_bytes)`,
/// `key2 = keyed_blake3(SEAL_DOMAIN_META || key1)`,
/// `signature = key1 || key2` — 64 B, chained, domain-separated.
pub struct LeaseEpochSealer {
    lease: cgap_crypto::lease::Lease,
}

impl LeaseEpochSealer {
    /// Wraps a live A16 lease epoch; the epoch's `issued_root` is the seal
    /// key (A16 §3.3 keying, §5 mechanics).
    pub fn new(lease: cgap_crypto::lease::Lease) -> Self {
        LeaseEpochSealer { lease }
    }

    /// The 64-B epoch binding over a byte string: two domain-separated keyed
    /// BLAKE3 digests. Shared by all four trait methods.
    fn epoch_binding(
        &self,
        domain: &[u8; 20],
        signed: &[u8],
    ) -> [u8; 64] {
        let key = *self.lease.issued_root();
        let mut buf = [0u8; 20 + 256];
        buf[0..20].copy_from_slice(domain);
        buf[20..20 + signed.len()].copy_from_slice(signed);
        let input = &buf[..20 + signed.len()];
        let d1 = cgap_crypto::chatman::keyed_digest(&key, input)
            .expect("domain||span <= 276 B: single-chunk keyed bound (1024) cannot trip");
        let mut meta = [0u8; 20 + 32];
        meta[0..20].copy_from_slice(&SEAL_DOMAIN_META);
        meta[20..].copy_from_slice(&d1);
        let d2 = cgap_crypto::chatman::keyed_digest(&key, &meta)
            .expect("52 B digest input: single-chunk keyed bound cannot trip");
        let mut sig = [0u8; 64];
        sig[0..32].copy_from_slice(&d1);
        sig[32..].copy_from_slice(&d2);
        sig
    }
}

impl ProofSealer for LeaseEpochSealer {
    fn seal(&mut self, signed: &[u8; SIGNED_SPAN]) -> [u8; 64] {
        // One lease tick per sealed artifact (A16 §5: seal is the only tick
        // consumer; exhaustion traps loud).
        self.lease.tick();
        self.epoch_binding(&SEAL_DOMAIN_PROOF, signed)
    }

    fn verify(&self, signed: &[u8; SIGNED_SPAN], signature: &[u8; 64]) -> bool {
        let expected = self.epoch_binding(&SEAL_DOMAIN_PROOF, signed);
        // Constant-shape compare: fold every byte difference.
        expected.iter().zip(signature.iter()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }

    fn seal_state(&mut self, signed: &[u8; STATE_SIGNED_SPAN]) -> [u8; 64] {
        self.lease.tick();
        self.epoch_binding(&SEAL_DOMAIN_STATE, signed)
    }

    fn verify_state(
        &self,
        signed: &[u8; STATE_SIGNED_SPAN],
        signature: &[u8; 64],
    ) -> bool {
        let expected = self.epoch_binding(&SEAL_DOMAIN_STATE, signed);
        expected.iter().zip(signature.iter()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
    }
}

// ===========================================================================
// Wave 3 — the meter (§4.1-§4.4)
// ===========================================================================

/// 168-byte signed threshold-crossing proof (`#[repr(C, packed)]`, §5).
///
/// Fixed layout, little-endian, fully occupied: offsets 0-3, 4-5, 6-7, 8-15,
/// 16-23, 24-31, 32-39, 40-71, 72-103, 104-167 leave no padding holes, so
/// `packed` changes the alignment (1) but not any field offset. All
/// multi-byte integers little-endian on the wire; digest = BLAKE3 32-byte
/// (§5); seal = 64-B epoch binding per [`LeaseEpochSealer`] (§5 "signature";
/// divergence D3 records the honest MAC scope).
#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SignedProof {
    /// offset 0, 4 B — `ASC450_MAGIC`
    pub magic: [u8; 4],
    /// offset 4, 2 B — u16 LE, currently 1 (`RECORD_VERSION`)
    pub record_version: u16,
    /// offset 6, 2 B — u16 LE, MUST be 0 (`RESERVED`)
    pub reserved: u16,
    /// offset 8, 8 B — u64 LE; the meter's configured ceiling
    pub ceiling_cents: u64,
    /// offset 16, 8 B — u64 LE; first `next > ceiling` accumulator value
    pub crossing_total_cents: u64,
    /// offset 24, 8 B — u64 LE; meter's monotone event seq at the crossing
    pub crossing_seq: u64,
    /// offset 32, 8 B — u64 LE; A16 tick-lease value at the crossing
    pub crossing_tick: u64,
    /// offset 40, 32 B — BLAKE3 of the A17 P28 attestation (§5)
    pub attestation_digest: [u8; 32],
    /// offset 72, 32 B — prior proof digest; zero only at genesis (§5)
    pub prev_proof_digest: [u8; 32],
    /// offset 104, 64 B — epoch seal over bytes 0..104 (§5)
    pub signature: [u8; 64],
}

// ---- ABI freeze (§5, court C6): any widening/reorder here must fail the build ----
// Core-only const asserts, no `static_assertions` crate (zero-dependency rule).

const _: () = assert!(core::mem::size_of::<SignedProof>() == PROOF_LEN);
const _: () = assert!(PROOF_LEN == 168);
const _: () = assert!(core::mem::align_of::<SignedProof>() == 1);

// §5 byte-offset table, machine-checked at compile time (`offset_of!`,
// stable 1.77; valid on `repr(packed)` because it creates no references).
// Each pin is one table row; a field reorder that preserves total size
// still trips these.
const _: () = assert!(core::mem::offset_of!(SignedProof, magic) == 0);
const _: () = assert!(core::mem::offset_of!(SignedProof, record_version) == 4);
const _: () = assert!(core::mem::offset_of!(SignedProof, reserved) == 6);
const _: () = assert!(core::mem::offset_of!(SignedProof, ceiling_cents) == 8);
const _: () = assert!(core::mem::offset_of!(SignedProof, crossing_total_cents) == 16);
const _: () = assert!(core::mem::offset_of!(SignedProof, crossing_seq) == 24);
const _: () = assert!(core::mem::offset_of!(SignedProof, crossing_tick) == 32);
const _: () = assert!(core::mem::offset_of!(SignedProof, attestation_digest) == 40);
const _: () = assert!(core::mem::offset_of!(SignedProof, prev_proof_digest) == 72);
const _: () = assert!(core::mem::offset_of!(SignedProof, signature) == 104);

// Sealed-span pin: the signature field starts exactly where the sealed span
// ends, so "seal bytes 0..104" and the layout table cannot drift apart.
const _: () = assert!(SIGNED_SPAN == 104);
const _: () = assert!(
    core::mem::offset_of!(SignedProof, signature) == SIGNED_SPAN,
);

// Field-width pins: the digest spans (40..72, 72..104) must stay exactly
// 32 bytes each and the signature span (104..168) exactly 64, or the §5
// "fully occupied" invariant breaks.
const _: () = assert!(ASC450_MAGIC.len() == 4);
const _: () = assert!(
    core::mem::size_of::<[u8; 32]>() == 32, // digest span pins
);
const _: () = assert!(
    core::mem::size_of::<[u8; 64]>() == 64, // signature span pin
);

// Fully-occupied check: the §5 field lengths sum to the record size with
// zero padding — the packing claim of §5, asserted arithmetically.
const _: () = assert!(
    4 + 2 + 2 + 8 + 8 + 8 + 8 + 32 + 32 + 64 == PROOF_LEN,
);

// Cross-lane ceiling pin: A18's default equals A17's frozen maximum, so the
// two Wave-1 ABI freezes in this crate cannot drift apart silently.
const _: () = assert!(
    DEFAULT_SAFE_HARBOR_CEILING_CENTS
        == crate::attestation::SAFE_HARBOR_CEILING_MAX_CENTS,
);

// Const-value freeze (§5): the record shape is version 1 with a zero
// reserved field and an all-zero genesis chain digest. Array `==` is not
// const-stable (`const_cmp`), so the digest compare runs element-wise in a
// const fn — no dependency, no nightly feature.
const fn bytes32_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut i = 0;
    while i < 32 {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const _: () = assert!(RECORD_VERSION == 1);
const _: () = assert!(RESERVED == 0);
const _: () = assert!(bytes32_eq(&GENESIS_PREV_PROOF_DIGEST, &[0u8; 32]));

impl SignedProof {
    /// Canonical 168-byte encoding (§5 offset table, LE fields, no choices).
    /// Packed-struct reads are by-value copies only.
    pub fn to_bytes(&self) -> [u8; PROOF_LEN] {
        let (
            magic,
            version,
            reserved,
            ceiling,
            total,
            seq,
            tick,
            att,
            prev,
            sig,
        ) = (
            self.magic,
            self.record_version,
            self.reserved,
            self.ceiling_cents,
            self.crossing_total_cents,
            self.crossing_seq,
            self.crossing_tick,
            self.attestation_digest,
            self.prev_proof_digest,
            self.signature,
        );
        let mut b = [0u8; PROOF_LEN];
        b[0..4].copy_from_slice(&magic);
        b[4..6].copy_from_slice(&version.to_le_bytes());
        b[6..8].copy_from_slice(&reserved.to_le_bytes());
        b[8..16].copy_from_slice(&ceiling.to_le_bytes());
        b[16..24].copy_from_slice(&total.to_le_bytes());
        b[24..32].copy_from_slice(&seq.to_le_bytes());
        b[32..40].copy_from_slice(&tick.to_le_bytes());
        b[40..72].copy_from_slice(&att);
        b[72..104].copy_from_slice(&prev);
        b[104..168].copy_from_slice(&sig);
        b
    }

    /// Decodes the canonical 168-byte encoding. Structural, fail-closed:
    /// wrong length, foreign magic/version, or a nonzero reserved field is
    /// refused (§5 shape law).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Asc450Error> {
        if bytes.len() != PROOF_LEN {
            return Err(Asc450Error::MalformedProof("length"));
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if magic != ASC450_MAGIC {
            return Err(Asc450Error::MalformedProof("magic"));
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != RECORD_VERSION {
            return Err(Asc450Error::MalformedProof("version"));
        }
        if u16::from_le_bytes([bytes[6], bytes[7]]) != RESERVED {
            return Err(Asc450Error::MalformedProof("reserved"));
        }
        Ok(Self {
            magic,
            record_version: version,
            reserved: u16::from_le_bytes([bytes[6], bytes[7]]),
            ceiling_cents: u64::from_le_bytes(
                bytes[8..16].try_into().expect("8..16 is 8 bytes"),
            ),
            crossing_total_cents: u64::from_le_bytes(
                bytes[16..24].try_into().expect("16..24 is 8 bytes"),
            ),
            crossing_seq: u64::from_le_bytes(
                bytes[24..32].try_into().expect("24..32 is 8 bytes"),
            ),
            crossing_tick: u64::from_le_bytes(
                bytes[32..40].try_into().expect("32..40 is 8 bytes"),
            ),
            attestation_digest: bytes[40..72].try_into().expect("40..72 is 32 B"),
            prev_proof_digest: bytes[72..104].try_into().expect("72..104 is 32 B"),
            signature: bytes[104..168].try_into().expect("104..168 is 64 B"),
        })
    }

    /// Recomputes the epoch seal over bytes 0..104 (the signed span) and
    /// compares it to the signature field. PURE — mutates nothing, consumes
    /// no lease authority. Structural shape checks live in
    /// [`SignedProof::from_bytes`]; the sealed span itself covers every
    /// structural field, so a tampered magic/version/reserved also fails
    /// here.
    pub fn verify<S: ProofSealer>(&self, sealer: &S) -> Result<(), Asc450Error> {
        let b = self.to_bytes();
        let mut span = [0u8; SIGNED_SPAN];
        span.copy_from_slice(&b[0..SIGNED_SPAN]);
        if !sealer.verify(&span, &self.signature) {
            return Err(Asc450Error::SealVerifyFailed);
        }
        Ok(())
    }
}

/// §4.1 — per-entity usage meter: checked, overflow-rejecting accounting;
/// boundary-exact threshold semantics; one-shot signed proof emission per
/// crossing.
///
/// Generic over the [`ProofSealer`] seam (§4 "Seal bridge"). The meter NEVER
/// reads time (§4.3: ticks arrive as event arguments; court C7) and never
/// allocates in the emitter path (§4.3: fixed-layout records, fixed buffers).
pub struct Asc450Meter<S: ProofSealer> {
    /// Configured ceiling (`safe_harbor_ceiling_cents`, §4.1).
    ceiling: u64,
    /// Checked accumulator over accepted events (§4.1).
    accumulated_cents: u64,
    /// Accepted-event count (§4.1 `event_count`); wraps are REJECTED (§4.3).
    event_count: u64,
    /// Monotone event seq = ordinal of the last accepted event (§4.1
    /// `last_seq`, §4.2 "bump `last_seq`").
    last_seq: u64,
    /// One-shot latch: set on the first emission, never cleared by the meter
    /// itself (§4.2; carried across [`Asc450Meter::recover`]).
    latched: bool,
    /// BLAKE3 of the A17 attestation wire bytes — the chain anchor (§5
    /// `attestation_digest`, §4.4 countermeasure 1).
    attestation_digest: [u8; 32],
    /// BLAKE3 over the full 168 B of the last proof this meter emitted;
    /// genesis all-zero (§4.4 countermeasures 1-2).
    prev_proof_digest: [u8; 32],
    /// Last sealed state minted by this meter (§4.4-3: a sealed state exists
    /// on every emission and on every explicit `seal_state`).
    last_sealed_state: Option<SealedMeterState>,
    /// The §4 seal bridge.
    sealer: S,
}

impl<S: ProofSealer> Asc450Meter<S> {
    /// §4.1 genesis. `attestation` is the A17 P28 record anchoring the proof
    /// chain (§5 `attestation_digest` = BLAKE3 of its 80 wire bytes);
    /// fail-closed: a spoliated attestation refuses genesis
    /// ([`Asc450Error::AttestationInvalid`], divergence D7).
    pub fn new(
        safe_harbor_ceiling_cents: u64,
        attestation: &crate::attestation::P28LicenseAttestation,
        sealer: S,
    ) -> Result<Self, Asc450Error> {
        if attestation.validate_intrinsic().is_err() {
            return Err(Asc450Error::AttestationInvalid);
        }
        Ok(Self {
            ceiling: safe_harbor_ceiling_cents,
            accumulated_cents: 0,
            event_count: 0,
            last_seq: 0,
            latched: false,
            attestation_digest: cgap_blake3::hash(&attestation.to_bytes()),
            prev_proof_digest: GENESIS_PREV_PROOF_DIGEST,
            last_sealed_state: None,
            sealer,
        })
    }

    /// §4.4-3 restart path: continues a meter lineage from a sealed state.
    /// The seq/counters/latch/chain fields continue from the SEALED state
    /// (verified fail-closed — a forged state refuses recovery with
    /// [`Asc450Error::SealedStateInvalid`]); `recovered_accumulated_cents` is
    /// whatever the recovery source had (e.g. a truncated WAL yields 0). This
    /// constructor is what makes the §4.4 reset-detection law reachable and
    /// honest: a reset-with-seq-continuity produces a sealed state whose
    /// `accumulated_cents` is LOWER at HIGHER `last_seq` than its predecessor
    /// — court C2b's non-monotone triple, detectable by bytes alone.
    pub fn recover(
        safe_harbor_ceiling_cents: u64,
        attestation: &crate::attestation::P28LicenseAttestation,
        sealer: S,
        sealed: &SealedMeterState,
        recovered_accumulated_cents: u64,
    ) -> Result<Self, Asc450Error> {
        if !sealed.verify(&sealer) {
            return Err(Asc450Error::SealedStateInvalid);
        }
        let attestation_digest = cgap_blake3::hash(&attestation.to_bytes());
        if sealed.ceiling_cents != safe_harbor_ceiling_cents
            || sealed.attestation_digest != attestation_digest
        {
            return Err(Asc450Error::SealedStateInvalid);
        }
        if attestation.validate_intrinsic().is_err() {
            return Err(Asc450Error::AttestationInvalid);
        }
        Ok(Self {
            ceiling: safe_harbor_ceiling_cents,
            accumulated_cents: recovered_accumulated_cents,
            event_count: sealed.event_count,
            last_seq: sealed.last_seq,
            latched: sealed.latched == STATE_LATCHED_SET,
            attestation_digest,
            prev_proof_digest: sealed.prev_proof_digest,
            last_sealed_state: Some(*sealed),
            sealer,
        })
    }

    /// §4.1 — adds to the accumulator; NEVER emits (licensed ingestion is out
    /// of the gate's scope by definition). Checked (§4.3): overflow rejects
    /// the event with [`Asc450Error::Overflow`], meter byte-unchanged.
    ///
    /// Divergence D2: the doc shows `()` return; the §4.3 reject law needs
    /// the typed error channel — a silent drop is the exact silence this
    /// module exists to prevent.
    pub fn record_licensed(
        &mut self,
        amount_cents: u64,
        tick: u64,
    ) -> Result<(), Asc450Error> {
        // Licensed events never emit, so no crossing_tick is ever recorded.
        let _ = tick;
        self.accept_event(amount_cents).map(|_| ())
    }

    /// §4.1 — adds to the accumulator and returns `Some(proof)` exactly when
    /// the accumulator transitions from `<= ceiling` to `> ceiling` (§4.2):
    /// emit iff `prev <= ceiling && next > ceiling`; `next == ceiling` never
    /// emits; a latched or already-over meter never re-emits. Checked (§4.3):
    /// overflow rejects with [`Asc450Error::Overflow`], meter byte-unchanged,
    /// nothing emitted.
    ///
    /// Divergence D2: `Result<Option<..>, ..>` instead of the doc's
    /// `Option<..>` — same typed-rejection reasoning as `record_licensed`.
    pub fn record_unlicensed(
        &mut self,
        amount_cents: u64,
        tick: u64,
    ) -> Result<Option<SignedProof>, Asc450Error> {
        // All-or-nothing: both checked adds run before any mutation (§4.3),
        // so a rejected event leaves the meter byte-unchanged.
        let (next, seq) = self.plan_event(amount_cents)?;
        let prev = self.accumulated_cents;
        // Commit first, then emit: the §4.4-3 sealed state minted by the
        // emission must reflect the crossing event (acc = crossing_total,
        // seq = crossing_seq), not the pre-event state.
        self.commit_event(next, seq);
        let crossing = !self.latched && prev <= self.ceiling && next > self.ceiling;
        let proof = if crossing {
            // One-shot latch set before the emission act (§4.2).
            self.latched = true;
            Some(self.emit_proof(next, seq, tick))
        } else {
            None
        };
        Ok(proof)
    }

    /// §4.4-3 — seals the current meter state and stores it as the meter's
    /// last sealed state (a consumer can demand a fresh sealed state at any
    /// time). Consumes one lease tick of sealing authority per call
    /// (exhaustion traps per A16 §5).
    pub fn seal_state(&mut self) -> SealedMeterState {
        let sealed = self.build_sealed_state();
        self.last_sealed_state = Some(sealed);
        sealed
    }

    /// The last sealed state this meter minted (§4.4-3): from genesis-time
    /// recovery, from [`Self::seal_state`], or from the most recent emission
    /// (the emitter seals state on every crossing, §4.4-3).
    pub fn last_sealed_state(&self) -> Option<SealedMeterState> {
        self.last_sealed_state
    }

    /// Configured ceiling (§4.1).
    pub fn ceiling_cents(&self) -> u64 {
        self.ceiling
    }

    /// Current accumulator (§4.1).
    pub fn accumulated_cents(&self) -> u64 {
        self.accumulated_cents
    }

    /// Accepted-event count (§4.1).
    pub fn event_count(&self) -> u64 {
        self.event_count
    }

    /// Monotone event seq (§4.1/§4.2).
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// Latch state (§4.2): `true` once a crossing proof has been emitted.
    pub fn is_latched(&self) -> bool {
        self.latched
    }

    /// The chain anchor (§5 `attestation_digest`).
    pub fn attestation_digest(&self) -> [u8; 32] {
        self.attestation_digest
    }

    /// Digest of the last emitted proof (genesis all-zero; §4.4-1).
    pub fn prev_proof_digest(&self) -> [u8; 32] {
        self.prev_proof_digest
    }

    /// BLAKE3 over the full 168 B of a proof — the §4.4 chain-step function
    /// ("digest of the previous proof this meter emitted"). Pinned here so
    /// consumers and the meter share one definition.
    pub fn proof_digest(proof: &SignedProof) -> [u8; 32] {
        cgap_blake3::hash(&proof.to_bytes())
    }

    // -- internals ----------------------------------------------------------

    /// §4.3 checked acceptance, all-or-nothing: both checked adds run BEFORE
    /// any state mutation, so a rejected event leaves every field unchanged.
    fn plan_event(&self, amount_cents: u64) -> Result<(u64, u64), Asc450Error> {
        let next = self
            .accumulated_cents
            .checked_add(amount_cents)
            .ok_or(Asc450Error::Overflow)?;
        let seq = self
            .event_count
            .checked_add(1)
            .ok_or(Asc450Error::Overflow)?;
        Ok((next, seq))
    }

    fn commit_event(&mut self, next: u64, seq: u64) {
        self.accumulated_cents = next;
        self.event_count = seq;
        self.last_seq = seq;
    }

    /// Shared accept path for `record_licensed` (checked add + counters).
    fn accept_event(&mut self, amount_cents: u64) -> Result<u64, Asc450Error> {
        let (next, seq) = self.plan_event(amount_cents)?;
        self.commit_event(next, seq);
        Ok(next)
    }

    /// §4.2/§5 emission: builds the sealed span, mints the seal, latches the
    /// chain (`prev_proof_digest` = BLAKE3 of the full new record), and seals
    /// the meter state (§4.4-3: a sealed state exists on every emission).
    fn emit_proof(&mut self, crossing_total: u64, crossing_seq: u64, crossing_tick: u64) -> SignedProof {
        let mut span = [0u8; SIGNED_SPAN];
        span[0..4].copy_from_slice(&ASC450_MAGIC);
        span[4..6].copy_from_slice(&RECORD_VERSION.to_le_bytes());
        span[6..8].copy_from_slice(&RESERVED.to_le_bytes());
        span[8..16].copy_from_slice(&self.ceiling.to_le_bytes());
        span[16..24].copy_from_slice(&crossing_total.to_le_bytes());
        span[24..32].copy_from_slice(&crossing_seq.to_le_bytes());
        span[32..40].copy_from_slice(&crossing_tick.to_le_bytes());
        span[40..72].copy_from_slice(&self.attestation_digest);
        span[72..104].copy_from_slice(&self.prev_proof_digest);
        let signature = self.sealer.seal(&span);
        let proof = SignedProof {
            magic: ASC450_MAGIC,
            record_version: RECORD_VERSION,
            reserved: RESERVED,
            ceiling_cents: self.ceiling,
            crossing_total_cents: crossing_total,
            crossing_seq,
            crossing_tick,
            attestation_digest: self.attestation_digest,
            prev_proof_digest: self.prev_proof_digest,
            signature,
        };
        // Chain step (§4.4-1/2): the NEXT proof must link to THIS record's
        // full 168 bytes (sealed span + seal), so a forged sibling cannot
        // splice into the chain.
        self.prev_proof_digest = Self::proof_digest(&proof);
        // §4.4-3: a sealed meter state exists on every emission.
        let sealed = self.build_sealed_state();
        self.last_sealed_state = Some(sealed);
        proof
    }

    /// Builds the sealed-state record over the current fields (seals span
    /// 0..105 through the seal bridge).
    fn build_sealed_state(&mut self) -> SealedMeterState {
        let mut span = [0u8; STATE_SIGNED_SPAN];
        span[0..4].copy_from_slice(&STATE_MAGIC);
        span[4..6].copy_from_slice(&STATE_VERSION.to_le_bytes());
        span[6..8].copy_from_slice(&STATE_RESERVED.to_le_bytes());
        span[8..16].copy_from_slice(&self.ceiling.to_le_bytes());
        span[16..24].copy_from_slice(&self.accumulated_cents.to_le_bytes());
        span[24..32].copy_from_slice(&self.event_count.to_le_bytes());
        span[32..40].copy_from_slice(&self.last_seq.to_le_bytes());
        span[40] = if self.latched {
            STATE_LATCHED_SET
        } else {
            STATE_LATCHED_CLEAR
        };
        span[41..73].copy_from_slice(&self.attestation_digest);
        span[73..105].copy_from_slice(&self.prev_proof_digest);
        let signature = self.sealer.seal_state(&span);
        SealedMeterState {
            magic: STATE_MAGIC,
            version: STATE_VERSION,
            reserved: STATE_RESERVED,
            ceiling_cents: self.ceiling,
            accumulated_cents: self.accumulated_cents,
            event_count: self.event_count,
            last_seq: self.last_seq,
            latched: if self.latched {
                STATE_LATCHED_SET
            } else {
                STATE_LATCHED_CLEAR
            },
            attestation_digest: self.attestation_digest,
            prev_proof_digest: self.prev_proof_digest,
            signature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::{
        instrument_header_span, origin_authority_digest, ATTESTATION_LEN,
        ATTESTATION_VERSION, FLAG_ROOT, LICENSE_MAGIC,
        SAFE_HARBOR_CEILING_MAX_CENTS, P28LicenseAttestation,
    };
    use cgap_crypto::lease::Lease;

    // -- real collaborators (Chicago: no mocks) ------------------------------

    /// Real pinned instrument bytes — same source as A17's courts.
    fn real_attestation() -> P28LicenseAttestation {
        let file = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../LICENSE-P28-SIL.md"
        ))
        .expect("pinned instrument LICENSE-P28-SIL.md is readable from the repo root");
        let span = instrument_header_span(&file)
            .expect("canonical instrument has a `---` line");
        P28LicenseAttestation {
            license_magic: LICENSE_MAGIC,
            version: ATTESTATION_VERSION,
            attestation_flags: FLAG_ROOT,
            safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_MAX_CENTS,
            origin_authority_digest: origin_authority_digest(&span),
            upstream_receipt: [0u8; 32],
        }
    }

    /// A real A16 lease epoch with a deterministic key; every seal consumes
    /// one tick of this lease (exhaustion traps, A16 §5).
    fn fresh_sealer(tag: u64) -> LeaseEpochSealer {
        let mut lease_id = [0u8; 32];
        lease_id[0..8].copy_from_slice(&tag.to_le_bytes());
        let mut issued_root = [0u8; 32];
        issued_root[0..8].copy_from_slice(&tag.to_le_bytes());
        LeaseEpochSealer::new(Lease::new(lease_id, 1 << 40, issued_root))
    }

    fn meter(ceiling: u64, tag: u64) -> Asc450Meter<LeaseEpochSealer> {
        Asc450Meter::new(ceiling, &real_attestation(), fresh_sealer(tag))
            .expect("real pinned attestation is intrinsically valid")
    }

    /// Court-law re-derivation of the reset-detection signal from SEALED
    /// BYTES ALONE (§4.4): the parsed records and the seal keys are all a
    /// consumer needs; nothing else about the meter enters.
    fn reset_evidence(prev: &SealedMeterState, next: &SealedMeterState) -> bool {
        next.accumulated_cents < prev.accumulated_cents
            && next.last_seq >= prev.last_seq
            && next.attestation_digest == prev.attestation_digest
    }

    // -- Wave-1 ABI courts (unchanged) ---------------------------------------

    /// §5 offset table rendered as a runtime witness (court C6 seed — the
    /// full C6 re-derives the signed span at emission time below;
    /// the const pins above are the primary gate). Anti-vacuity: any edit
    /// that changes a field's offset, the total size, or the signed span
    /// flips this test and the build. A test asserting the table rows is
    /// not removable without deleting the table claim.
    #[test]
    fn layout_court_c6_witness() {
        assert_eq!(core::mem::size_of::<SignedProof>(), PROOF_LEN);
        assert_eq!(PROOF_LEN, 168);
        assert_eq!(core::mem::align_of::<SignedProof>(), 1);
        assert_eq!(core::mem::offset_of!(SignedProof, magic), 0);
        assert_eq!(core::mem::offset_of!(SignedProof, record_version), 4);
        assert_eq!(core::mem::offset_of!(SignedProof, reserved), 6);
        assert_eq!(core::mem::offset_of!(SignedProof, ceiling_cents), 8);
        assert_eq!(
            core::mem::offset_of!(SignedProof, crossing_total_cents),
            16
        );
        assert_eq!(core::mem::offset_of!(SignedProof, crossing_seq), 24);
        assert_eq!(core::mem::offset_of!(SignedProof, crossing_tick), 32);
        assert_eq!(
            core::mem::offset_of!(SignedProof, attestation_digest),
            40
        );
        assert_eq!(
            core::mem::offset_of!(SignedProof, prev_proof_digest),
            72
        );
        assert_eq!(core::mem::offset_of!(SignedProof, signature), 104);
        // Signed span (§5): signature covers 0..104, i.e. everything before
        // the signature field itself.
        assert_eq!(SIGNED_SPAN, 104);
        assert_eq!(core::mem::offset_of!(SignedProof, signature), SIGNED_SPAN);
    }

    /// Const-value freeze (§5 table consts + §1 default ceiling).
    /// Anti-vacuity: any magic-byte, version, reserved, or ceiling edit
    /// flips this. The ceiling pin also re-derives the A17 cross-lane
    /// equality at runtime so the const assert above is never the only
    /// witness.
    #[test]
    fn consts_freeze_court() {
        assert_eq!(ASC450_MAGIC, [0x30, 0x35, 0x34, 0x41]);
        assert_eq!(u32::from_le_bytes(ASC450_MAGIC), 0x4134_3530);
        assert_eq!(
            u32::from_le_bytes(ASC450_MAGIC).to_be_bytes(),
            *b"A450",
            "wire bytes are the ASCII magic stored little-endian"
        );
        assert_eq!(RECORD_VERSION, 1);
        assert_eq!(RESERVED, 0);
        assert_eq!(GENESIS_PREV_PROOF_DIGEST, [0u8; 32]);
        assert_eq!(DEFAULT_SAFE_HARBOR_CEILING_CENTS, 5_000_000_000);
        assert_eq!(
            DEFAULT_SAFE_HARBOR_CEILING_CENTS,
            crate::attestation::SAFE_HARBOR_CEILING_MAX_CENTS,
            "A18 default ceiling must equal A17's frozen ceiling maximum"
        );
    }

    // -- C1: boundary-exact threshold court (§6) ------------------------------
    //
    // Named anti-vacuity mutation (must FAIL the court): the `>=` mutant —
    // `prev <= c && next > c` rewritten as `next >= c` — emits at exactly
    // ceiling; the `assert!(proof.is_none())` at acc == ceiling kills it.
    // The strict-`>` half is killed by the emission assert at ceiling+1.
    //
    // Witness receipt (wave-5 defect repair, 2026-11-01): the prior in-code
    // citation pointed at an A18 "lane receipt" that does not exist on disk
    // (hearsay — removed). The mutant was re-applied in the
    // `/tmp/a18-sandbox` standalone workspace (p28 + blake3 + crypto copies,
    // minimal own manifest): baseline 30/30 PASS -> SHIM-COURT `>=` mutant
    // -> 26 passed / 4 FAILED, cargo exit 101, flipping c1 + c2b + c3 + c4
    // together -> restore sha256 byte-exact (`ba11cf08…`) -> 30/30 PASS.
    // Repo file byte-exact pre and post (same sha256).

    #[test]
    fn c1_boundary_exact_threshold_court() {
        let ceiling = 100u64;
        let mut m = meter(ceiling, 0xA1);

        // Drive to EXACTLY the ceiling in unlicensed events: 40 + 40 + 20.
        assert!(m.record_unlicensed(40, 1).expect("accept").is_none());
        assert!(m.record_unlicensed(40, 2).expect("accept").is_none());
        // The boundary event: prev=80 <= 100, next=100 == ceiling -> NO emit.
        let at_ceiling = m.record_unlicensed(20, 3).expect("accept");
        assert!(
            at_ceiling.is_none(),
            "next == ceiling must NOT emit (§4.2): got {at_ceiling:?}"
        );
        assert_eq!(m.accumulated_cents(), 100);
        assert!(
            !m.is_latched(),
            "sitting exactly at the ceiling is still inside the safe harbor"
        );

        // +1 cent crosses: prev=100 <= 100, next=101 > 100 -> EMIT.
        let proof = m
            .record_unlicensed(1, 4)
            .expect("accept")
            .expect("ceiling+1 must emit (§4.2)");
        // Packed record: fields are read by copy, never by reference
        // (E0793 — no references into a #[repr(packed)] struct).
        let p_ceiling = proof.ceiling_cents;
        let p_total = proof.crossing_total_cents;
        let p_seq = proof.crossing_seq;
        let p_tick = proof.crossing_tick;
        let p_att = proof.attestation_digest;
        let p_prev = proof.prev_proof_digest;
        assert_eq!(p_ceiling, 100);
        assert_eq!(
            p_total, 101,
            "crossing_total is the first next > ceiling (§4.2)"
        );
        assert_eq!(p_seq, 4, "the crossing event's seq");
        assert_eq!(p_tick, 4, "the event's tick (§5 offset 32)");
        assert!(m.is_latched(), "emission latches the meter (§4.2)");
        assert_eq!(
            p_att,
            cgap_blake3::hash(&real_attestation().to_bytes()),
            "proof anchors to the A17 attestation digest (§5 offset 40)"
        );
        assert_eq!(
            p_prev, GENESIS_PREV_PROOF_DIGEST,
            "first proof chains from the genesis digest (§5 offset 72)"
        );
    }

    // -- C2: signature court — exhaustive byte-flip sweep (§6) ----------------
    //
    // Named anti-vacuity mutation: a verify that compares only the first
    // byte of the seal (A16 §8-3's mutant) fails the sweep — flips in bytes
    // 105..168 of the first digest half survive such a compare; the
    // exhaustive sweep must catch every one. Positive control first: the
    // unmutated proof MUST verify, so a verify-that-always-fails cannot pass
    // the court either.

    #[test]
    fn c2_signature_exhaustive_flip_court() {
        let mut m = meter(50, 0xC2);
        let proof = m
            .record_unlicensed(51, 9)
            .expect("accept")
            .expect("crossing emits");
        let good = proof.to_bytes();

        // Positive control: the unmutated proof verifies.
        proof.verify(&fresh_sealer(0xC2)).expect("untampered proof verifies");

        // 168 bytes x 8 bits = 1344 single-bit mutations: EVERY flip anywhere
        // in 0..168 must fail verification (§5 tamper-evidence law).
        let mut flips_run = 0usize;
        for i in 0..PROOF_LEN {
            for j in 0..8u8 {
                let mut mutated = good;
                mutated[i] ^= 1 << j;
                flips_run += 1;
                let parsed = SignedProof::from_bytes(&mutated);
                let verdict = parsed
                    .map_err(|_| Asc450Error::SealVerifyFailed)
                    .and_then(|p| p.verify(&fresh_sealer(0xC2)));
                assert!(
                    verdict.is_err(),
                    "flip at byte {i} bit {j} must fail verification (§5)"
                );
            }
        }
        assert_eq!(
            flips_run, 1344,
            "sweep must run all 1344 single-bit mutations (anti-vacuity)"
        );

        // Signature-field swap: replace ONLY the 64-B signature with the
        // signature of a DIFFERENT real proof (different span, different
        // epoch key). Must fail.
        let mut m2 = meter(50, 0xC3);
        let proof2 = m2
            .record_unlicensed(51, 1)
            .expect("accept")
            .expect("crossing emits");
        let mut swapped = good;
        let sig2 = proof2.signature; // packed field: copy before referencing
        swapped[104..168].copy_from_slice(&sig2);
        let swapped_verdict = SignedProof::from_bytes(&swapped)
            .map_err(|_| Asc450Error::SealVerifyFailed)
            .and_then(|p| p.verify(&fresh_sealer(0xC2)));
        assert!(
            swapped_verdict.is_err(),
            "a foreign signature must not verify against this span"
        );

        // All-zero signature must fail.
        let mut zeroed = good;
        zeroed[104..168].fill(0);
        assert!(
            SignedProof::from_bytes(&zeroed)
                .map_err(|_| Asc450Error::SealVerifyFailed)
                .and_then(|p| p.verify(&fresh_sealer(0xC2)))
                .is_err(),
            "an all-zero signature must not verify"
        );

        // Epoch binding: the same proof under a DIFFERENT lease epoch key
        // must fail (A16 §3.3: a receipt stolen from one epoch does not
        // verify under another).
        assert!(
            proof.verify(&fresh_sealer(0xC4)).is_err(),
            "proof must be bound to its lease epoch's key"
        );
    }

    // -- C2b: signed-state monotonicity / reset-attack court (§6) -------------
    //
    // Named anti-vacuity mutation: dropping the latch or the sealed-state
    // signature check from `recover` lets a forged state launder a reset;
    // the forged-recovery section kills it. The non-monotone triple (S2,S3)
    // is the byte-level detection law; the healthy pair (S1,S2) is the
    // positive control proving healthy meters do not trip it.

    #[test]
    fn c2b_sealed_state_reset_detection_court() {
        let ceiling = 100u64;
        let att = real_attestation();
        let mut m = meter(ceiling, 0xCB);

        // Fill to the ceiling, cross (+1), then one more cent.
        for i in 0..10 {
            assert!(m.record_unlicensed(10, i).expect("accept").is_none());
        }
        assert!(m.record_unlicensed(1, 10).expect("crossing").is_some());
        let s1 = m.seal_state(); // acc=101, seq=11, latched
        assert!(m.record_unlicensed(1, 11).expect("accept").is_none());
        let s2 = m.seal_state(); // acc=102, seq=12, latched

        // Seals verify under the epoch key; the healthy pair is monotone.
        let key = fresh_sealer(0xCB);
        assert!(s1.verify(&key), "S1 must carry a valid seal");
        assert!(s2.verify(&key), "S2 must carry a valid seal");
        assert!(
            !reset_evidence(&s1, &s2),
            "healthy meters never trip the reset law (positive control)"
        );

        // THE RESET: kill-and-restart at zero with seq continuity from S2
        // (recovery source truncated to 0). One cent -> S3.
        let mut m2 = Asc450Meter::recover(
            ceiling,
            &att,
            fresh_sealer(0xCB),
            &s2,
            0, // recovered accumulator: truncated to zero
        )
        .expect("S2 is a valid sealed state");
        let s2_count = s2.event_count;
        assert_eq!(m2.event_count(), s2_count, "seq lineage continues");
        assert!(
            m2.is_latched(),
            "the latch survives recovery — a restart cannot un-cross"
        );
        assert!(m2.record_unlicensed(1, 12).expect("accept").is_none());
        let s3 = m2.seal_state(); // acc=1, seq=13, latched

        // Detection BY BYTES ALONE (§4.4): S3 verifies under the same epoch
        // key, yet its accumulated_cents is LOWER than S2's at HIGHER
        // last_seq — the non-monotone triple. A consumer holding S1..S3
        // needs nothing else to prove a reset happened.
        assert!(s3.verify(&key), "S3 must carry a valid seal");
        let s3_acc = s3.accumulated_cents;
        let s3_seq = s3.last_seq;
        assert_eq!(s3_acc, 1);
        assert_eq!(s3_seq, 13);
        assert!(
            reset_evidence(&s2, &s3),
            "acc dropped 102->1 at last_seq 12->13 under valid seals: reset detected"
        );

        // Forged-state recovery is REFUSED (§4.4-3 fail-closed): flip one bit
        // of S2's sealed span and recovery must not admit it.
        let mut forged = s2.to_bytes();
        forged[16] ^= 0x01; // accumulated_cents bit flip
        let forged_state = SealedMeterState::from_bytes(&forged)
            .expect("structural fields still parse");
        assert!(
            matches!(
                Asc450Meter::recover(ceiling, &att, fresh_sealer(0xCB), &forged_state, 0),
                Err(Asc450Error::SealedStateInvalid)
            ),
            "forged sealed state must refuse recovery"
        );

        // Epoch-bound fail-closed: a valid S2 under the WRONG epoch key
        // refuses too.
        assert!(
            Asc450Meter::recover(ceiling, &att, fresh_sealer(0xCC), &s2, 0).is_err(),
            "sealed state from another lease epoch must not recover"
        );
    }

    // -- C3: negative-control court (never-emit mutant killer) (§6) -----------
    //
    // Real over-ceiling run MUST emit. Suite-level law: the suite MUST fail
    // under the never-emit mutation (SHIM `crossing = false` planted at the
    // crossing predicate) and MUST pass unmutated.
    //
    // Witness receipt (wave-6 defect repair, 2026-11-01; fixes finding
    // D-A18-2 of `docs/jira/v26.11.1/waves/a18-wave3-audit-receipt.md`):
    // the prior text OVERCLAIMED — "0 passed-class" and "ALL TEN asc450
    // courts failing" were hearsay counts from a receipt that does not
    // exist on disk. Re-derived this session in the `/tmp/a18-w6-sb`
    // standalone workspace (p28 + blake3 + crypto copies, minimal own
    // manifest): baseline 30/30 PASS -> SHIM-COURT never-emit (`let
    // crossing = false;` at the crossing predicate) -> cargo exit 101,
    // 22 passed / 8 FAILED — exactly the 8 asc450 courts that consume an
    // emitted proof flip (c1, c2, c2b, c3, c4, c5, c6, c7b); 4 survive
    // (c7 textual gate, consts_freeze, layout_court_c6_witness,
    // genesis_spoliated_attestation_refused) -> restore sha256 byte-exact
    // (`936f2151…`) -> 30/30 PASS. Suite-level vacuity law holds: the suite
    // MUST fail under the never-emit mutation and MUST pass unmutated; the
    // mutant kills loud. Same session, same sandbox: the C1 witness's
    // `>=` mutant re-derived — 26 passed / 4 FAILED (c1 + c2b + c3 + c4),
    // exit 101, restore byte-exact — C1's witness block stands as written.
    //
    // Witness receipt (wave-8 court re-derivation, 2026-11-01): the wave-7
    // receipt file for this lane does not exist on disk (missing subject,
    // same class as D-A08-1 / D-A18-1); wave-7 evidence was /tmp shim-court
    // artifacts only, with no written findings. This wave re-derived the
    // full named-mutation battery against the current bytes in a standalone
    // sandbox (/tmp/a18-w8-sb.sE8vtL; baseline 31/31, exit 0): 9/9 mutation
    // classes flip their named courts with cargo exit 101 — M1 `>=` -> c1,
    // M2 never-emit -> c3, M3b chain-balanced saturating -> c5, M4
    // first-byte verify -> c2, M5 wall-clock needle -> c7, M6
    // drop-recover-verify -> c2b, M7 BE-seq -> c6, M8 state-verify-true ->
    // c2b — and the never-emit suite-level vacuity gate exits 101 (23
    // passed / 8 FAILED: the 8 proof-consuming asc450 courts). Every
    // restore sha256 byte-exact (70d8331a…c4985, pre == post). A first M3
    // attempt left `.ok_or(..)?` on the saturating chain and compile-
    // aborted (E0599) — recorded as a compile-abort (census law: not a
    // failure count), redone chain-balanced. A first harness run executed
    // cargo from the repo cwd instead of the sandbox (all 9 courts read
    // NO-FLIP at 0.00s against unmutated repo bytes) — discarded as a
    // harness artifact, re-run rooted in the sandbox. ZERO defects found;
    // no repair landed this wave; court receipts:
    // /tmp/a18-w8-courts.log.

    #[test]
    fn c3_positive_control_emission_court() {
        let ceiling = 1_000u64;
        let mut m = meter(ceiling, 0xC5);
        // Real events summing above the ceiling, one cent at a time.
        let mut emitted = None;
        for i in 0..1_002u64 {
            if let Some(p) = m.record_unlicensed(1, i).expect("accept") {
                emitted = Some(p);
                break;
            }
        }
        let proof = emitted.expect("a real over-ceiling run MUST emit (C3)");
        let p_total = proof.crossing_total_cents;
        assert_eq!(p_total, 1_001);
        assert!(proof.verify(&fresh_sealer(0xC5)).is_ok());
    }

    // -- C4: latch court + chain-fork detection (§6) ---------------------------
    //
    // Named anti-vacuity mutation: a latch that resets on overflow/reject, or
    // a chain that reuses the genesis digest for later proofs, is killed by
    // the exact assertions below (post-crossing still accumulates; the fork
    // proof carries genesis parentage while the original does not).

    #[test]
    fn c4_latch_and_chain_fork_court() {
        // Seq arithmetic: 10 fill events + 1 crossing + 5 post-crossing.
        let ceiling = 100u64;
        let mut m = meter(ceiling, 0xC6);

        // Crossing.
        for i in 0..10 {
            assert!(m.record_unlicensed(10, i).expect("accept").is_none());
        }
        let p1 = m
            .record_unlicensed(1, 10)
            .expect("accept")
            .expect("crossing emits");
        assert!(m.is_latched());

        // Post-crossing events emit NOTHING while exposure stays visible.
        for i in 0..5 {
            let again = m.record_unlicensed(7, 100 + i).expect("accept");
            assert!(again.is_none(), "latch holds: no re-emit (§4.2)");
        }
        assert_eq!(m.accumulated_cents(), 101 + 5 * 7);
        assert_eq!(m.last_seq(), 16, "post-crossing events still bump last_seq");
        assert_eq!(
            m.prev_proof_digest(),
            Asc450Meter::<LeaseEpochSealer>::proof_digest(&p1),
            "chain head is the emitted proof's full-record digest"
        );

        // Licensed events also never emit, before or after latch.
        assert!(m.record_licensed(1, 200).is_ok());
        assert_eq!(m.accumulated_cents(), 101 + 5 * 7 + 1);

        // Licensed-only crossing emits nothing (§4.1: licensed is out of the
        // gate's scope; §4.2 prev>ceiling branch suppresses).
        let mut ml = meter(10, 0xC7);
        assert!(ml.record_licensed(20, 1).is_ok());
        assert!(
            ml.record_unlicensed(1, 2).expect("accept").is_none(),
            "a licensed-induced over-ceiling state suppresses the unlicensed emit"
        );

        // THE CHAIN FORK: a fresh (post-kill) meter re-crosses. Its proof
        // carries GENESIS parentage — it does NOT link to the original
        // chain — so a consumer holding both proofs detects the fork by
        // bytes alone (§4.4-2).
        let mut fresh = meter(ceiling, 0xC8); // restarted process: fresh epoch
        for i in 0..10 {
            assert!(fresh.record_unlicensed(10, i).expect("accept").is_none());
        }
        let p2 = fresh
            .record_unlicensed(1, 10)
            .expect("accept")
            .expect("re-crossing after reset emits a fork proof");
        let p1_prev = p1.prev_proof_digest;
        let p2_prev = p2.prev_proof_digest;
        let p1_att = p1.attestation_digest;
        let p2_att = p2.attestation_digest;
        assert_eq!(
            p2_prev, GENESIS_PREV_PROOF_DIGEST,
            "the fork proof does not link to the original chain: it claims \
             genesis parentage (§4.4-2 / C4)"
        );
        // p1 is ALSO genesis-parented — it was the meter's first emission.
        // The reset fingerprint is therefore TWO genesis-parented proofs
        // under the SAME attestation anchor with distinct digests: an honest
        // history has exactly one.
        assert_eq!(p1_prev, GENESIS_PREV_PROOF_DIGEST);
        assert_ne!(
            Asc450Meter::<LeaseEpochSealer>::proof_digest(&p1),
            Asc450Meter::<LeaseEpochSealer>::proof_digest(&p2),
            "the two proofs are distinct records under the same attestation"
        );
        assert_eq!(
            p1_att, p2_att,
            "the fork is under the SAME attestation anchor: genesis-parented \
             proof #2 after proof #1 is the reset fingerprint"
        );
    }

    // -- C5: overflow court (§6) ----------------------------------------------
    //
    // Named anti-vacuity mutation: a saturating implementation (checked_add
    // -> saturating_add) masks the crossing near u64::MAX and CHANGES the
    // accepted total; the exact state assertions (accumulator unchanged,
    // Err(Overflow)) kill it.

    #[test]
    fn c5_overflow_rejection_court() {
        let ceiling = 100u64;
        let mut m = meter(ceiling, 0xC9);

        assert!(m.record_unlicensed(50, 1).expect("accept").is_none());
        // prev=50, amount=u64::MAX: prev + amount overflows -> REJECT.
        let verdict = m.record_unlicensed(u64::MAX, 2);
        assert_eq!(
            verdict,
            Err(Asc450Error::Overflow),
            "overflow must reject with the typed error (§4.2)"
        );
        // Meter BYTE-UNCHANGED (§4.2/§4.3): accumulator, count, seq, latch.
        assert_eq!(m.accumulated_cents(), 50);
        assert_eq!(m.event_count(), 1);
        assert_eq!(m.last_seq(), 1);
        assert!(!m.is_latched());
        assert!(m.last_sealed_state().is_none());

        // Licensed overflow rejects identically.
        assert_eq!(
            m.record_licensed(u64::MAX, 3),
            Err(Asc450Error::Overflow),
        );
        assert_eq!(m.accumulated_cents(), 50, "licensed overflow leaves no trace");

        // The overflow boundary is real: prev=1, amount=u64::MAX-1 is the
        // LAST acceptable pair; MAX is not. (ceiling=100 keeps the crossing
        // law in play while the accumulator rides to u64::MAX.)
        let mut edge = meter(100, 0xCA);
        assert!(edge.record_unlicensed(1, 1).expect("accept").is_none());
        assert_eq!(
            edge.record_unlicensed(u64::MAX - 1, 2).map(|p| p.is_some()),
            Ok(true),
            "accumulated == u64::MAX emits (crossing at the boundary is lawful)"
        );
        assert_eq!(
            edge.record_unlicensed(1, 3),
            Err(Asc450Error::Overflow),
            "one more cent past u64::MAX must REJECT, never saturate"
        );
        assert_eq!(edge.accumulated_cents(), u64::MAX);
    }

    // -- C6: layout court — wire offsets re-derived from a REAL proof (§6) ----
    //
    // Named anti-vacuity mutation: any to_bytes() field reorder or
    // endianness flip lands at the wrong offset range below. The signed-span
    // re-derivation (flip at 103 fails, flip at 104 fails) pins 0..104
    // against the wire at runtime, on top of the const pins.

    #[test]
    fn c6_wire_offsets_and_round_trip() {
        let mut m = meter(10, 0xCC);
        let proof = m
            .record_unlicensed(11, 0xABCD_EF01)
            .expect("accept")
            .expect("crossing emits");
        let b = proof.to_bytes();
        assert_eq!(b.len(), 168);
        assert_eq!(&b[0..4], &[0x30, 0x35, 0x34, 0x41], "magic LE (§5)");
        assert_eq!(&b[4..6], &[0x01, 0x00], "version 1 LE");
        assert_eq!(&b[6..8], &[0x00, 0x00], "reserved zero");
        assert_eq!(
            &b[8..16],
            &10u64.to_le_bytes(),
            "ceiling at offset 8, u64 LE"
        );
        assert_eq!(&b[16..24], &11u64.to_le_bytes(), "crossing_total at 16");
        assert_eq!(&b[24..32], &1u64.to_le_bytes(), "crossing_seq at 24");
        assert_eq!(
            &b[32..40],
            &0xABCD_EF01u64.to_le_bytes(),
            "crossing_tick at 32"
        );
        let att = proof.attestation_digest;
        assert_eq!(&b[40..72], &att[..], "att digest at 40");
        assert_eq!(
            &b[72..104],
            &[0u8; 32][..],
            "genesis prev_proof_digest at 72"
        );
        let sig = proof.signature;
        assert_eq!(&b[104..168], &sig[..], "signature at 104");

        // Round trip.
        let reparsed = SignedProof::from_bytes(&b).expect("168 B parse");
        assert_eq!(reparsed, proof);
        assert!(reparsed.verify(&fresh_sealer(0xCC)).is_ok());

        // Signed-span re-derivation at the boundary: the last signed byte
        // (103) and the first signature byte (104) BOTH fail when flipped.
        for i in [103usize, 104] {
            let mut mutated = b;
            mutated[i] ^= 0x80;
            assert!(
                SignedProof::from_bytes(&mutated)
                    .map_err(|_| Asc450Error::SealVerifyFailed)
                    .and_then(|p| p.verify(&fresh_sealer(0xCC)))
                    .is_err(),
                "flip at byte {i} must fail: signed span re-derived as 0..104"
            );
        }

        // Truncation / over-length refuses at parse (§5 total-length law).
        assert!(SignedProof::from_bytes(&b[..167]).is_err());
        assert!(SignedProof::from_bytes(&[]).is_err());

        // Sealed-state wire law (divergence D4): offsets + round trip.
        let s = m.seal_state();
        let sb = s.to_bytes();
        assert_eq!(sb.len(), 169);
        assert_eq!(&sb[0..4], b"A4MS");
        assert_eq!(&sb[8..16], &10u64.to_le_bytes());
        assert_eq!(&sb[16..24], &11u64.to_le_bytes());
        assert_eq!(&sb[24..32], &1u64.to_le_bytes());
        assert_eq!(&sb[32..40], &1u64.to_le_bytes());
        assert_eq!(sb[40], STATE_LATCHED_SET);
        let state_sig = s.signature;
        assert_eq!(&sb[105..169], &state_sig[..]);
        assert_eq!(
            SealedMeterState::from_bytes(&sb).expect("169 B parse"),
            s
        );
        assert!(s.verify(&fresh_sealer(0xCC)));
    }

    // -- C7: no-wall-clock court (§6) ------------------------------------------
    //
    // Named anti-vacuity mutation: introducing any wall-clock read into the
    // module (the needle list below) trips the textual gate; removing the
    // gate's marker assertion makes the gate vacuous and trips nothing — the
    // marker is load-bearing.
    //
    // Compile-level law, two layers: (1) the dependency closure of this
    // module is [cgap-blake3, cgap-crypto], and neither crate depends on any
    // time source (cgap-blake3 has zero dependencies; cgap-crypto depends
    // only on cgap-blake3) — inspected this session; (2) the textual gate
    // below fails the build if a wall-clock identifier enters THIS file.

    #[test]
    fn c7_no_wall_clock_textual_gate() {
        // The gate reads THIS file's real bytes (compile-time include).
        const SOURCE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/asc450.rs"
        ));
        assert!(
            SOURCE.len() > 1_000,
            "the gate must read the real module source, not an empty file"
        );
        assert!(
            SOURCE.contains("record_unlicensed"),
            "gate marker: the source under test is the emitter itself"
        );
        // Needles are assembled from fragments so this gate's own needle list
        // cannot trip itself.
        const NEEDLES: [&[u8]; 4] = [
            concat!("Sys", "temTime").as_bytes(),
            concat!("Inst", "ant").as_bytes(),
            concat!("std::", "time").as_bytes(),
            concat!("chrono", "::").as_bytes(),
        ];
        let src = SOURCE.as_bytes();
        for (k, needle) in NEEDLES.iter().enumerate() {
            assert!(
                !contains_subslice(src, needle),
                "wall-clock needle #{k} found in asc450.rs: the emitter path \
                 must never read time (§4.3, C7)"
            );
        }
    }

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|w| w == needle)
    }

    /// C7 determinism law (§6): identical tick sequences in, byte-identical
    /// proofs out. Two meters, two leases with the SAME key epoch, same
    /// events, same ticks — the proof bytes must be identical.
    #[test]
    fn c7b_determinism_court() {
        let run = |tag: u64| {
            let mut m = meter(100, tag);
            let mut out = None;
            for i in 0..12u64 {
                if let Some(p) = m.record_unlicensed(10, 1_000 + i).expect("accept") {
                    out = Some(p);
                    break;
                }
            }
            let proof = out.expect("crossing emits deterministically");
            let state = m.last_sealed_state().expect("emission seals state");
            (proof, state)
        };
        let (p1, s1) = run(0xE1);
        let (p2, s2) = run(0xE1);
        assert_eq!(
            p1, p2,
            "identical inputs must produce byte-identical proofs (§4.3/§6 C7)"
        );
        assert_eq!(
            s1, s2,
            "sealed states are deterministic too (§4.4-3)"
        );
        // ...and a different epoch key produces different bytes (the seal is
        // key-bound, not a content hash).
        let (p3, _) = run(0xE2);
        assert_ne!(
            p1.signature, p3.signature,
            "a different lease epoch must seal differently"
        );
    }

    // -- genesis fail-closed court (§4.4-1; divergence D7) ---------------------

    /// A spoliated attestation refuses meter genesis: the proof chain never
    /// anchors to a record A17's own validator rejects.
    #[test]
    fn genesis_spoliated_attestation_refused() {
        let mut bad = real_attestation();
        bad.license_magic = *b"P28X"; // §5.2 spoliation
        assert!(
            matches!(
                Asc450Meter::new(100, &bad, fresh_sealer(0xF1)),
                Err(Asc450Error::AttestationInvalid)
            ),
            "spoliated magic must refuse genesis (§4.4-1 fail-closed)"
        );
        let mut bad = real_attestation();
        bad.safe_harbor_ceiling_cents = SAFE_HARBOR_CEILING_MAX_CENTS + 1; // §5.8
        assert!(
            matches!(
                Asc450Meter::new(100, &bad, fresh_sealer(0xF1)),
                Err(Asc450Error::AttestationInvalid)
            ),
            "spoliated ceiling must refuse genesis (A17 §5.8 intrinsic; §4.4-1 \
             fail-closed)"
        );
    }

    // =========================================================================
    // Wave 9 — cross-lane integration courts (Wave 9 dispatch slice)
    // =========================================================================

    // -- W9-A04: identity-anchor court (A04 discipline: identity is the bytes)
    //
    // A04's law (cited, not redefined — lane doc A04 §8.1): identity is over
    // bytes, re-derivable from the wire, and total (every bit counts). This
    // module's identity anchor is the BLAKE3 digest of the A17 80-B record
    // (§5 offset 40). The court proves: (1) the anchor re-derives from the
    // canonical bytes; (2) the record parses back to byte-identical bytes;
    // (3) ALL 640 single-bit flips of the 80-B record either REFUSE meter
    // genesis fail-closed (A17 §5 intrinsic arm) or produce a DIFFERENT
    // anchor and byte-different proofs — an identity-blind anchor (two
    // different records yielding the same chain) is structurally impossible.
    //
    // Named anti-vacuity mutation (must FAIL this court): the identity-blind
    // anchor — `cgap_blake3::hash(&attestation.to_bytes())` replaced by a
    // constant in `Asc450Meter::new` — makes every record share one anchor.
    // Witness receipt (wave 9, 2026-11-01, /tmp/a18-w9-sb2 sandbox, baseline
    // 34/34 exit 0): SHIM-COURT constant-anchor mutant → cargo exit 101,
    // 30 passed / 4 FAILED — the anchor feeds every sealed record, so the
    // killer set is c1 (att-digest assert), c2b (attestation continuity),
    // w9_a04 (canonical-anchor positive control + distinct-anchor sweep),
    // w9_a05 (recover attestation gate); restore sha256 byte-exact
    // (76a67618…4400, pre == post). Zero repo-tree mutations.

    #[test]
    fn w9_a04_identity_anchor_rederivation_court() {
        let att = real_attestation();
        let canon = att.to_bytes();
        let canon_digest = cgap_blake3::hash(&canon);

        // Positive control: the genesis anchor IS the digest of the canonical
        // 80-B record (§5 offset 40).
        let mut m = meter(100, 0xA41);
        assert_eq!(
            m.attestation_digest(),
            canon_digest,
            "genesis anchor must re-derive from the canonical record bytes"
        );

        // A04 identity law: parse → re-encode is the byte-identity round trip.
        let reparsed = P28LicenseAttestation::from_bytes(&canon)
            .expect("canonical 80-B record parses");
        assert_eq!(reparsed.to_bytes(), canon, "parse/encode identity");

        // The canonical crossing proof, for the distinct-proof comparison.
        let mut canon_meter = meter(100, 0xA42);
        let canon_proof = canon_meter
            .record_unlicensed(101, 1)
            .expect("accept")
            .expect("crossing emits");
        let canon_proof_digest = Asc450Meter::<LeaseEpochSealer>::proof_digest(&canon_proof);
        assert!(canon_proof.verify(&fresh_sealer(0xA42)).is_ok());
        let canon_proof_bytes = canon_proof.to_bytes();
        let proof_reparsed = SignedProof::from_bytes(&canon_proof_bytes)
            .expect("168-B proof parses");
        assert_eq!(proof_reparsed.to_bytes(), canon_proof_bytes);

        // ALL 80x8 = 640 single-bit flips of the anchor record: every flip
        // lands in exactly one of the two lawful arms.
        let mut refused = 0usize;
        let mut accepted_distinct = 0usize;
        for i in 0..ATTESTATION_LEN {
            for j in 0..8u8 {
                let mut mutated = canon;
                mutated[i] ^= 1 << j;
                let mutated_digest = cgap_blake3::hash(&mutated);
                assert_ne!(
                    mutated_digest, canon_digest,
                    "any single-bit flip of the anchor record changes the digest \
                     (identity is total, byte {i} bit {j})"
                );
                let parsed = P28LicenseAttestation::from_bytes(&mutated)
                    .expect("80-B parse is total on length");
                match Asc450Meter::new(100, &parsed, fresh_sealer(0xA43)) {
                    Err(Asc450Error::AttestationInvalid) => refused += 1,
                    Err(other) => panic!("unexpected genesis error {other:?}"),
                    Ok(mut m2) => {
                        assert_ne!(
                            m2.attestation_digest(),
                            canon_digest,
                            "a valid mutated record must anchor the meter to its \
                             OWN bytes, byte {i} bit {j}"
                        );
                        let p = m2
                            .record_unlicensed(101, 1)
                            .expect("accept")
                            .expect("crossing emits under the mutated anchor");
                        assert_ne!(
                            Asc450Meter::<LeaseEpochSealer>::proof_digest(&p),
                            canon_proof_digest,
                            "a different identity must mint byte-different proofs \
                             (no silent chain reuse), byte {i} bit {j}"
                        );
                        accepted_distinct += 1;
                    }
                }
            }
        }
        assert_eq!(refused + accepted_distinct, 640, "the sweep is exhaustive");
        assert!(refused >= 1, "the fail-closed arm must be reachable");
        assert!(
            accepted_distinct >= 1,
            "the distinct-anchor arm must be reachable"
        );
    }

    // -- W9-A05: power-cut sweep court (A05 discipline, cited not redefined:
    //    lane doc A05 §"cut at ANY byte offset k in 0..N must roll to a typed,
    //    fail-closed state; the committed prefix is durable"; torn records
    //    refuse, never half-apply).
    //
    // The meter's durable boundary is the sealed state (§4.4-3). The sweep
    // cuts a 16-event run (ceiling 100, 10-cent events, crossing at event
    // 11) at EVERY point:
    //   - cut k, 1..=16: sealed state S_k survived → recovery is
    //     BYTE-TRANSPARENT: replaying events k+1..16 from S_k reproduces the
    //     uninterrupted run's bytes exactly — for k < 11 the re-crossing
    //     proof is byte-identical to the original (same epoch key, same
    //     counters, same events); for k >= 11 the latch rides the seal and
    //     nothing re-emits; the sealed lineage is monotone (S_k.last_seq <=
    //     S_16.last_seq, S_k.acc <= S_16.acc) and terminal.
    //   - cut 0: nothing survived → cold restart mints a fork proof with the
    //     SAME signed span but a DIFFERENT seal (new epoch) — the reset
    //     fingerprint is visible by bytes alone (§4.4-2).
    //   - torn state: ALL 169x8 = 1352 single-bit flips of S_16 refuse
    //     recovery (parse-structural or seal fail-closed) — the torn-write
    //     matrix over the whole record.
    //
    // Named anti-vacuity mutations (must FAIL this court):
    //   M-W9-A05a "lineage amnesia": `recover` starts the counters cold
    //     (event_count/last_seq = 0 instead of the sealed values) — the
    //     terminal `last_seq == 16` continuity assert flips.
    //   M-W9-A05b "seal-verify drop": `recover` skips `sealed.verify` — the
    //     torn-state sweep flips (a forged state recovers Ok). (Same killer
    //     as C2b's M8; the sweep re-witnesses it at matrix scale.)
    // Witness receipts (wave 9, 2026-11-01, /tmp/a18-w9-sb2 sandbox, baseline
    // 34/34 exit 0): M-W9-A05a → exit 101, 32 passed / 2 FAILED —
    // w9_a05_powercut_sweep_court (terminal `last_seq == 16` continuity) +
    // c2b (event-count lineage assert); M-W9-A05b → exit 101, 32 passed /
    // 2 FAILED — w9_a05_powercut_sweep_court (torn sweep admits forged
    // states) + c2b (forged-recovery refusal); restores sha256 byte-exact
    // both (76a67618…4400, pre == post each).

    #[test]
    fn w9_a05_powercut_sweep_court() {
        const RUN: usize = 16;
        const CEILING: u64 = 100;
        const CROSSING_EVENT: usize = 11; // 10 events x 10c = 100 == ceiling; #11 emits
        let att = real_attestation();

        // The uninterrupted run: S_k after every event, plus the crossing
        // proof and the terminal state.
        let mut m = meter(CEILING, 0xA50);
        let mut sealed: [Option<SealedMeterState>; RUN] = Default::default();
        let mut crossing_proof = None;
        for k in 0..RUN {
            let event = k + 1;
            let emit = m.record_unlicensed(10, event as u64).expect("accept");
            if event == CROSSING_EVENT {
                crossing_proof = Some(emit.expect("the crossing emits exactly here"));
            } else {
                assert!(emit.is_none(), "only the crossing emits");
            }
            sealed[k] = Some(m.seal_state());
        }
        let p1 = crossing_proof.expect("the run crosses exactly once");
        let p1_bytes = p1.to_bytes();
        let key = fresh_sealer(0xA50);
        assert!(p1.verify(&key).is_ok());
        assert!(sealed[RUN - 1].expect("terminal seal").verify(&key));

        // Sweep 1: cut at every k in 1..=16 — S_k survived, recovery is
        // byte-transparent.
        for k in 1..=RUN {
            let s_k = sealed[k - 1].expect("seal exists at every cut point");
            // Durable-prefix continuity: S_k itself is sealed and monotone
            // against the terminal seal.
            let s_k_acc = s_k.accumulated_cents;
            let s_k_seq = s_k.last_seq;
            assert_eq!(s_k_acc, 10 * k as u64, "committed prefix acc at cut {k}");
            assert_eq!(s_k_seq, k as u64, "committed prefix seq at cut {k}");
            let term = sealed[RUN - 1].expect("terminal seal");
            let term_acc = term.accumulated_cents;
            let term_seq = term.last_seq;
            assert!(s_k_acc <= term_acc && s_k_seq <= term_seq);

            // Power cut: drop the in-memory meter, recover from S_k with the
            // same epoch key (full-flush recovery source), replay the rest.
            let mut recovered = Asc450Meter::recover(
                CEILING,
                &att,
                fresh_sealer(0xA50),
                &s_k,
                s_k_acc, // recovery source yields the sealed accumulator
            )
            .expect("a valid sealed state always recovers");
            let carried_latch = k >= CROSSING_EVENT;
            assert_eq!(
                recovered.is_latched(),
                carried_latch,
                "the latch rides the seal (cut {k})"
            );
            let mut re_emitted = 0;
            for event in k + 1..=RUN {
                if let Some(_p) = recovered
                    .record_unlicensed(10, event as u64)
                    .expect("accept")
                {
                    re_emitted += 1;
                }
            }
            if k < CROSSING_EVENT {
                // The re-crossing is byte-identical to the original proof:
                // recovery is transparent, the cut never shows in bytes.
                assert_eq!(
                    re_emitted, 1,
                    "exactly the crossing re-emits on the recovered meter (cut {k})"
                );
            } else {
                assert_eq!(
                    re_emitted, 0,
                    "the latched recovery never re-emits (cut {k})"
                );
            }
            // Terminal state identical from every cut point.
            assert_eq!(recovered.accumulated_cents(), 160);
            assert_eq!(recovered.last_seq(), RUN as u64);
            assert_eq!(recovered.event_count(), RUN as u64);
            let s_end = recovered.seal_state();
            let s_end_acc = s_end.accumulated_cents;
            let s_end_seq = s_end.last_seq;
            let s_end_latched = s_end.latched;
            assert_eq!(s_end_acc, 160);
            assert_eq!(s_end_seq, RUN as u64);
            assert_eq!(s_end_latched, STATE_LATCHED_SET);
            assert!(s_end.verify(&fresh_sealer(0xA50)));
        }

        // Byte-transparency of a cut BEFORE the crossing: the recovered
        // meter's re-crossing proof is byte-identical to the uninterrupted
        // run's proof (same epoch key seals the same span).
        let mut cut3 = Asc450Meter::recover(
            CEILING,
            &att,
            fresh_sealer(0xA50),
            &sealed[2].expect("S_3"),
            30, // full-flush acc at event 3
        )
        .expect("S_3 recovers");
        for event in 4..CROSSING_EVENT {
            assert!(cut3
                .record_unlicensed(10, event as u64)
                .expect("accept")
                .is_none());
        }
        let p_cut3 = cut3
            .record_unlicensed(10, CROSSING_EVENT as u64)
            .expect("accept")
            .expect("the recovered meter re-crosses");
        assert_eq!(
            p_cut3.to_bytes(),
            p1_bytes,
            "a cut with surviving state is byte-transparent in the proof bytes"
        );

        // Sweep 2: cut 0 — nothing survived. Cold restart, same replay; the
        // fork proof carries the SAME signed span under a DIFFERENT epoch
        // seal: the reset fingerprint, visible by bytes alone.
        let mut cold = meter(CEILING, 0xC0_1D); // new lease epoch: different key
        for event in 1..CROSSING_EVENT {
            assert!(cold
                .record_unlicensed(10, event as u64)
                .expect("accept")
                .is_none());
        }
        let p_cold = cold
            .record_unlicensed(10, CROSSING_EVENT as u64)
            .expect("accept")
            .expect("the cold restart re-crosses");
        let cold_bytes = p_cold.to_bytes();
        assert_eq!(
            &cold_bytes[0..SIGNED_SPAN],
            &p1_bytes[0..SIGNED_SPAN],
            "the fork claims the same crossing facts"
        );
        assert_ne!(
            &cold_bytes[SIGNED_SPAN..PROOF_LEN],
            &p1_bytes[SIGNED_SPAN..PROOF_LEN],
            "the fork is sealed under a different epoch: distinct bytes"
        );
        assert_ne!(
            Asc450Meter::<LeaseEpochSealer>::proof_digest(&p_cold),
            Asc450Meter::<LeaseEpochSealer>::proof_digest(&p1),
            "a consumer holding both proofs detects the fork by bytes alone"
        );

        // Sweep 3: torn durable state — EVERY single-bit flip of the terminal
        // sealed state refuses recovery, fail-closed (1352 flips).
        let s_term = sealed[RUN - 1].expect("terminal seal");
        let s_bytes = s_term.to_bytes();
        for i in 0..STATE_LEN {
            for j in 0..8u8 {
                let mut torn = s_bytes;
                torn[i] ^= 1 << j;
                let verdict = SealedMeterState::from_bytes(&torn)
                    .and_then(|parsed| {
                        Asc450Meter::recover(
                            CEILING,
                            &att,
                            fresh_sealer(0xA50),
                            &parsed,
                            parsed.accumulated_cents,
                        )
                        .map(|_| ())
                    });
                assert!(
                    verdict.is_err(),
                    "torn sealed state (byte {i} bit {j}) must refuse recovery \
                     fail-closed (A05 torn-write law)"
                );
            }
        }
    }

    // -- W9-A20: scale stress court (A20 discipline: campaign-scale volume
    //    with exact final-state asserts — assert the terminal meter, never
    //    call counts; volume itself is the stress).
    //
    // 1,000,000 unlicensed 1-cent events against a 500,000-cent ceiling:
    // exactly one emission, at exactly event ceiling+1, and the terminal
    // state is exact (accumulator/count/seq all 1,000,000, latched, the
    // proof verifies, the terminal seal matches the meter fields).
    //
    // Named anti-vacuity mutation (must FAIL this court): "counter-stall" —
    // `commit_event` stops advancing `event_count` — the terminal
    // state asserts flip. Witness receipt (wave 9, 2026-11-01,
    // /tmp/a18-w9-sb2 sandbox, baseline 34/34 exit 0): SHIM-COURT
    // counter-stall mutant → cargo exit 101, 27 passed / 7 FAILED — c1
    // (p_seq), c4 (last_seq), c5 (counters byte-unchanged), c2b (lineage),
    // c6 (crossing_seq wire), w9_a05 (continuity), w9_a20 (terminal state);
    // restore sha256 byte-exact (76a67618…4400, pre == post). One earlier
    // attempt used a 12-space anchor (0 hits): the fail-closed runner
    // refused to apply and the pristine baseline re-ran 34/34 exit 0 —
    // recorded, not a flip witness.

    #[test]
    fn w9_a20_millirow_stress_court() {
        const EVENTS: u64 = 1_000_000;
        const CEILING: u64 = 500_000;
        let mut m = meter(CEILING, 0xA20);
        let mut emitted_at: Option<u64> = None;
        let mut proof: Option<SignedProof> = None;
        for i in 1..=EVENTS {
            if let Some(p) = m.record_unlicensed(1, i).expect("accept") {
                assert!(
                    emitted_at.is_none(),
                    "exactly one emission in the whole run (event {i})"
                );
                emitted_at = Some(i);
                proof = Some(p);
            }
        }
        assert_eq!(
            emitted_at,
            Some(CEILING + 1),
            "the crossing lands on the first event past the ceiling, exactly"
        );
        let proof = proof.expect("the run crossed");
        let p_total = proof.crossing_total_cents;
        let p_seq = proof.crossing_seq;
        let p_tick = proof.crossing_tick;
        assert_eq!(p_total, CEILING + 1);
        assert_eq!(p_seq, CEILING + 1);
        assert_eq!(p_tick, CEILING + 1);
        assert!(
            proof.verify(&fresh_sealer(0xA20)).is_ok(),
            "the 1M-run crossing proof verifies"
        );

        // Terminal state (Chicago: assert final state, not call counts).
        assert_eq!(m.accumulated_cents(), EVENTS);
        assert_eq!(m.event_count(), EVENTS);
        assert_eq!(m.last_seq(), EVENTS);
        assert!(m.is_latched(), "the latch holds across the whole tail");

        // Terminal seal matches the terminal meter fields and verifies.
        let s = m.seal_state();
        let s_acc = s.accumulated_cents;
        let s_seq = s.last_seq;
        let s_latched = s.latched;
        assert_eq!(s_acc, EVENTS);
        assert_eq!(s_seq, EVENTS);
        assert_eq!(s_latched, STATE_LATCHED_SET);
        assert!(s.verify(&fresh_sealer(0xA20)));
    }
}
