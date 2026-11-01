//! Consequent-projector ABI (Wave 1 freeze) + Wave 2 emission engine: the
//! `DeltaBatch` envelope the projector emits for lane A15's audit slab.
//! Spec: `docs/jira/v26.11.1/lanes/A12-simd-unifier.md` Section 4.4.
//!
//! Wave 1 froze the envelope ABI: struct + const definitions plus
//! compile-time asserts. Wave 2 (this file now) adds the emission engine:
//! head-template instantiation over a [`crate::unify::MatchSet`], the
//! digest computation, and the concrete row storage ([`DeltaRows`]).
//!
//! Determinism contract (spec 4.4): for a fixed (input batch, rule set),
//! `DeltaBatch` bytes are byte-identical across runs — the Wave-2
//! determinism court (M12-6, in `unify.rs` tests) pins that; A15 consumes
//! `digest` as its slab anchor.
//!
//! Layout ground truth is pinned by compile-time asserts — a regression
//! is a build failure, not a runtime surprise (house convention,
//! `crates/cgap-arrow/src/layout.rs`).
//!
//! Recorded divergences (spec yields to authoritative lanes / reality):
//!
//! 1. **Row storage** (amends the Wave-1 note): spec 4.4 fixes `rows` as
//!    the Arrow batch (`cgap_arrow::batch::Batch`). `cgap-bn3` holds no
//!    `cgap-arrow` dependency and this lane may not edit
//!    `crates/cgap-bn3/Cargo.toml`, so Wave 2 instantiates
//!    `B = DeltaRows` — an in-crate SoA store with the frozen 4-column
//!    schema. The Arrow binding remains the Wave-3 integration seam.
//! 2. **Object column width**: spec 4.1/4.4 say `object: u32`, but the
//!    authoritative statement ABI (dispatch: "A11 BN3 layout — cite, don't
//!    redefine") is A11's `BN3Statement` with three 64-bit term words
//!    (`crate::sigil`). The object column is therefore `u64` term words.
//!    `DELTA_COLUMNS` stays 4 as frozen in Wave 1.
//! 3. **Flags column**: spec 4.1 defines flags bit 0 "object is literal",
//!    bit 1 "deleted". The A11 sigil ABI carries literal-ness in-band
//!    (`SlotConst` covers both IRI and literal interned ids), so the bit
//!    is not derivable from the term word. Wave 2 emits `flags = 0`
//!    (reserved; derived statements are never deletions) — asserted by the
//!    flags court, not silently assumed.
//! 4. **Digest algorithm**: spec 4.4 says SHA-256. [`sha256`] is an
//!    in-file FIPS 180-2 implementation pinned by known-answer tests.
//!    (Wave-1 note: the workspace hash substrate is `cgap-blake3`, now a
//!    declared dependency as of A13's disclosed Cargo.toml unblock — but
//!    it is Blake3, not SHA-256; the spec names SHA-256, so SHA-256
//!    stands and the in-file implementation stays.)
//!
//! Wave 4 (hardening + DoD closure): the projector gained
//! [`ConsequentProjector::emit_into`] — emission into caller-owned
//! [`DeltaRows`] + canonical-byte buffers, the zero-allocation half of
//! the scan+project hot path (spec 7.4, DoD 6). [`ConsequentProjector::project`]
//! is unchanged in signature and delegates to it.
//!
//! Wave 9 (cross-lane integration, v26.11.1 wave 9): the emission side
//! gains the two seams the wave-9 slice names, both byte-exact mirrors of
//! AUTHORITATIVE specs cited, not redefined (dispatch law):
//!
//! 1. **A04 wire frame** (`docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md`
//!    §6; authoritative artifact `crates/cgap-core/src/frame.rs`
//!    `CgapFrameHeader`): [`bn3_frame_header_encode`] writes the 128-byte
//!    `FLAG_BN3` header over a [`DeltaRows`] canonical-byte payload;
//!    [`frame_oabi_digest`] is the §5.1 digest binding (`cgap_blake3::hash`,
//!    the same injected kernel A04's `verify_payload` names). cgap-bn3
//!    holds no `cgap-core` dependency and the manifest is coordinator-owned
//!    (Wave-2 divergence 1 stands), so the layout is mirrored byte-for-byte
//!    and pinned against the authoritative values by
//!    `frame_court_flag_bn3_header_layout_pin` — the dependency-free
//!    consumer-lane pattern already used by `cgap-gate/src/penstock.rs`
//!    (its `HEADER_LEN`/`MAGIC`/`OFF_*` constants) with the cross-crate
//!    drift tripwire living in A20's `cross_lane_abi_pin`.
//! 2. **A05 power-cut WAL** (`docs/jira/v26.11.1/lanes/A05-powercut-wal.md`
//!    §3.3-3.4; authoritative artifact
//!    `crates/cgap-storage/src/atomic_commit.rs`): [`Bn3WalDescriptor`] is
//!    the 48-byte BN3 root descriptor — rule id, epoch, the A15 digest
//!    anchor, and the audit popcount — carried in the authoritative
//!    48+48+16 journal-record framing; [`bn3_wal_classify`] is the typed
//!    per-offset truncation classifier so the wave-9 power-cut sweep runs
//!    over A12's own records: every cut `k in 0..=112` resolves to a typed
//!    verdict (courted by
//!    `wal_court_power_cut_sweep_every_offset_is_typed`). The A12 payload
//!    domain is A12's choice (recorded divergence 5 below); the framing,
//!    CRC-32, and split BLAKE3 binding are A05's, unmodified.
//!
//! Recorded divergences (wave 9, doc yields to authoritative lanes):
//!
//! 5. **BN3 WAL payload domain**: A05 §3.4's `NewRootDescriptor`
//!    (git oid / parquet root / dag seq / ts / flags / pad) is the
//!    STORAGE-root payload the storage lane owns. A12's descriptor is a
//!    reasoning-layer root: it names the deriving rule, the input epoch,
//!    the A15 digest anchor, and the audit popcount, so a recovered
//!    journal names exactly which fixpoint round committed. The 48-byte
//!    payload SIZE and the whole §3.3 record framing are A05's, cited.
//! 6. **Torn-vs-refused boundary**: A05 §2 classifies a cut strictly by
//!    byte offset (`atomic_commit.rs::tear_class_for_offset`: k=0
//!    PreWrite, k<48 TornHeader, k<96 TornPayload, k<112 TornTrailer) —
//!    content never turns a prefix into a refusal. [`bn3_wal_classify`]
//!    mirrors that: prefix length alone selects the Torn variant; the
//!    strict field chain (A05 `decode_slot` order) runs only at full
//!    record length. The A05-owned erased-slot state (`Empty`, all-0xFF,
//!    flash erase semantics) is storage's medium concern, not the A12
//!    record codec's, and is out of this seam's subject.

/// Digest width of [`DeltaBatch::digest`]: SHA-256 (spec 4.4: "SHA-256
/// over canonical row order"). Wave 1 froze the field *width* only; the
/// digest algorithm is Wave-2 logic (and note: the workspace hash
/// substrate is `cgap-blake3` — the algorithm decision is recorded as an
/// open spec note, not silently re-decided here).
pub const DELTA_DIGEST_LEN: usize = 32;

/// Column count of the delta-row payload (spec 4.4: "columns: subject
/// u64, predicate u64, object u32, flags u16 — same layout as 4.1,
/// column-major, no padding"). The concrete Arrow column types land in
/// Wave 2/3 with the crate's `cgap-arrow` dependency; Wave 1 freezes the
/// schema shape.
pub const DELTA_COLUMNS: usize = 4;

/// Delta-batch envelope handed to lane A15 (spec 4.4). Field set, field
/// order, and scalar types are the frozen ABI; A15 consumes `digest` as
/// its slab anchor and dedup is A15's concern, never the projector's
/// (spec 4.4; M12-7 guards the division).
///
/// `B` is the row-storage type. Wave 2 instantiates `B = DeltaRows`
/// (recorded divergence 1: the Arrow batch type is the Wave-3 seam).
///
/// Determinism contract: for a fixed (input batch, rule set), the emitted
/// `DeltaBatch` is byte-identical across runs (spec 4.4; Wave-2 court
/// M12-6). Dedup is A15's concern, not the projector's (spec 4.4).
#[derive(Clone, Debug, PartialEq)]
pub struct DeltaBatch<B> {
    /// Emitting rule. (Spec 4.4.)
    pub rule_id: u32,
    /// Monotonically increasing input epoch. (Spec 4.4.)
    pub epoch: u64,
    /// Derived rows, column-major (schema: [`DELTA_COLUMNS`], spec 4.4).
    pub rows: B,
    /// SHA-256 over canonical row order; A15's slab anchor. (Spec 4.4.)
    pub digest: [u8; DELTA_DIGEST_LEN],
    /// Audit counter: rows selected pre-dedup. The M12-7 invariant
    /// (`popcount` vs emitted-row-count) guards the projector-emits-
    /// derivations / A15-collapses-sets division. (Spec 4.4, 7.3.)
    pub source_mask_popcount: u32,
}

// --- ABI freeze: compile-time layout laws (spec 4.4) --------------------
// Any digest-width, schema-shape, or field-set change must fail here, not
// in A15's slab six months from now. Anti-vacuity mutations (build MUST
// fail):
//   - widen `DELTA_DIGEST_LEN` 32 -> 64 (digest-width law flips);
//   - delete a `DeltaBatch` field (struct itself stops compiling).
const _: () = assert!(DELTA_DIGEST_LEN == 32);
const _: () = assert!(core::mem::size_of::<[u8; DELTA_DIGEST_LEN]>() == 32);
const _: () = assert!(DELTA_COLUMNS == 4);

// ========================================================================
// Wave 2: SHA-256 (FIPS 180-2), the digest substrate for spec 4.4
// ========================================================================

/// SHA-256 (FIPS 180-2). In-file implementation pinned by FIPS
/// known-answer tests below; a mutation to any rotation, constant, or the
/// message schedule MUST flip a KAT.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = Vec::with_capacity(data.len() + 72);
    msg.extend_from_slice(data);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    sha256_padded(&msg)
}

/// SHA-256 over an already-padded message (`len % 64 == 0`, 0x80 +
/// zero pad + big-endian bit length already appended; asserted). The
/// allocation-free core: no heap use beyond the caller's buffer.
///
/// Wave 4: [`ConsequentProjector::emit_into`] pads its caller-owned
/// canonical-byte buffer in place and calls this, so the digest step
/// performs zero allocations once warm.
fn sha256_padded(msg: &[u8]) -> [u8; 32] {
    debug_assert!(msg.len() % 64 == 0, "padded SHA-256 message must be block-aligned");
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, wi) in w.iter_mut().enumerate().take(16) {
            let b = &block[4 * i..4 * i + 4];
            *wi = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (hi, vi) in h.iter_mut().zip(v) {
            *hi = hi.wrapping_add(vi);
        }
    }

    let mut out = [0u8; 32];
    for (i, hi) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&hi.to_be_bytes());
    }
    out
}

// ========================================================================
// Wave 2: concrete row storage (B = DeltaRows; recorded divergence 1)
// ========================================================================

/// Derived-row store, column-major: the frozen 4-column schema (subject
/// u64 term word, predicate u64 term word, object u64 term word, flags
/// u16 — recorded divergence 2 for the object width, divergence 3 for the
/// flags value). 26 canonical bytes per row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeltaRows {
    /// Subject term word (A11 sigil ABI).
    pub s: Vec<u64>,
    /// Predicate term word (A11 sigil ABI).
    pub p: Vec<u64>,
    /// Object term word (A11 sigil ABI).
    pub o: Vec<u64>,
    /// Reserved flags (recorded divergence 3: Wave 2 emits 0).
    pub flags: Vec<u16>,
}

impl DeltaRows {
    /// Append one derived row.
    pub fn push(&mut self, s: u64, p: u64, o: u64, flags: u16) {
        self.s.push(s);
        self.p.push(p);
        self.o.push(o);
        self.flags.push(flags);
    }

    /// Drop all rows, retaining column capacity (the zero-allocation
    /// reuse seam for [`ConsequentProjector::emit_into`]).
    pub fn clear(&mut self) {
        self.s.clear();
        self.p.clear();
        self.o.clear();
        self.flags.clear();
    }

    /// Row count.
    pub fn len(&self) -> usize {
        self.s.len()
    }

    /// True when no rows are held.
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    /// Row `i` as a tuple (final-state readback for courts).
    pub fn row(&self, i: usize) -> (u64, u64, u64, u16) {
        (self.s[i], self.p[i], self.o[i], self.flags[i])
    }

    /// Canonical byte form fed to the digest: for each row in order,
    /// `s` LE, `p` LE, `o` LE, `flags` LE — 26 bytes per row. Row order is
    /// the input batch's row order filtered by the selection vector, so
    /// the digest is stable without sorting (spec 4.4).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.len() * 26);
        self.write_canonical_bytes(&mut out);
        out
    }

    /// [`DeltaRows::canonical_bytes`] into a caller-owned buffer
    /// (cleared first, capacity retained) — the zero-allocation form.
    pub fn write_canonical_bytes(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.len() * 26); // no-op when capacity suffices
        for i in 0..self.len() {
            out.extend_from_slice(&self.s[i].to_le_bytes());
            out.extend_from_slice(&self.p[i].to_le_bytes());
            out.extend_from_slice(&self.o[i].to_le_bytes());
            out.extend_from_slice(&self.flags[i].to_le_bytes());
        }
    }
}

// ========================================================================
// Wave 2: the projector engine (spec 4.3 step 5, Section 5.2 C3)
// ========================================================================

/// Consequent projector: instantiates a compiled rule's head template per
/// selected match through the `R0..R255` register file and emits the
/// [`DeltaBatch`] for lane A15 (spec Sections 4.4, 5.2).
///
/// The projector emits *derivations*; dedup is A15's concern (spec 4.4).
/// The M12-7 invariant — `source_mask_popcount == rows.len()` for this
/// projector — is courted in the tests below: a projector that collapsed
/// sets would emit fewer rows than it counted and flip the court.
#[derive(Clone, Copy, Debug, Default)]
pub struct ConsequentProjector;

impl ConsequentProjector {
    /// A projector (unit struct; constructor kept for call-site clarity).
    pub fn new() -> Self {
        ConsequentProjector
    }

    /// Instantiate the head template for every match in `matches` and emit
    /// one batch. Row order is match order (input row order filtered by the
    /// selection vector — spec 4.4 determinism contract).
    pub fn project(
        &self,
        rule: &crate::unify::CompiledRule,
        epoch: u64,
        matches: &crate::unify::MatchSet,
    ) -> Result<DeltaBatch<DeltaRows>, crate::unify::UnifyError> {
        let mut rows = DeltaRows::default();
        let mut canon = Vec::new();
        let (digest, source_mask_popcount) =
            self.emit_into(rule, epoch, matches, &mut rows, &mut canon)?;
        Ok(DeltaBatch {
            rule_id: rule.rule_id,
            epoch,
            rows,
            digest,
            source_mask_popcount,
        })
    }

    /// [`ConsequentProjector::project`] into caller-owned buffers (Wave 4,
    /// spec 7.4): `rows` is cleared and refilled (capacity retained) and
    /// `canon` is the canonical-byte scratch; returns the digest and the
    /// pre-dedup popcount without touching the allocator once warm. The
    /// courted zero-allocation half of the scan+project path.
    pub fn emit_into(
        &self,
        rule: &crate::unify::CompiledRule,
        _epoch: u64,
        matches: &crate::unify::MatchSet,
        rows: &mut DeltaRows,
        canon: &mut Vec<u8>,
    ) -> Result<([u8; DELTA_DIGEST_LEN], u32), crate::unify::UnifyError> {
        use crate::unify::{CompiledTerm, RegisterFile, VAR_BAND_FIRST};

        // Alignment invariants (debug): a kernel-produced MatchSet is
        // column-aligned, and a default (empty) MatchSet is also
        // well-formed -- zero matches, zero columns -- and must fall
        // through to the well-formed empty batch, not an index panic
        // (Wave-5 defect D1: `atom_rows[0]` here panicked on
        // `MatchSet::default()` through this public entry; the kernel
        // path yields the same logical no-match case as a courted empty
        // batch, so the direct path must too).
        debug_assert_eq!(
            matches.len,
            matches.atom_rows.first().map_or(0, Vec::len)
        );
        for col in matches.var_values.iter().take(rule.var_count) {
            debug_assert_eq!(col.len(), matches.len);
        }

        // Stage the rule's constant pool once per batch (spec 4.3 step:
        // "load rule constants (once per chunk)"). Band-checked stores —
        // a band violation here is a compiler bug, surfaced as a typed
        // error, never a silent wrap.
        let mut rf = RegisterFile::zeroed();
        for (i, &word) in rule.const_pool.iter().enumerate() {
            rf.load_const(i, word)?;
        }

        rows.clear();
        for m in 0..matches.len {
            // Bind the variable band for this match, then read the head
            // template through the register file (spec 4.2: scratch/output
            // staging runs through the file, not around it).
            for idx in 0..rule.var_count {
                rf.bind_var(idx, matches.var_values[idx][m])?;
            }
            let mut words = [0u64; 3];
            for (si, term) in rule.head.iter().enumerate() {
                words[si] = match *term {
                    CompiledTerm::Const(reg) => rf.load(reg as usize)?,
                    CompiledTerm::Var(idx) => rf.load(VAR_BAND_FIRST + idx as usize)?,
                };
            }
            // Flags: reserved 0 (recorded divergence 3).
            rows.push(words[0], words[1], words[2], 0);
        }

        rows.write_canonical_bytes(canon);
        // Pad the canonical bytes IN PLACE (FIPS 180-2: 0x80, zeros to
        // 56 mod 64, big-endian bit length), hash, then restore the
        // buffer to its canonical length — zero allocations once warm,
        // and `canon` still holds exactly the canonical bytes on return.
        let canon_len = canon.len();
        let bit_len = (canon_len as u64).wrapping_mul(8);
        canon.push(0x80);
        while canon.len() % 64 != 56 {
            canon.push(0);
        }
        canon.extend_from_slice(&bit_len.to_be_bytes());
        let digest = sha256_padded(canon);
        canon.truncate(canon_len);
        Ok((digest, matches.len as u32))
    }
}

// ========================================================================
// Wave 9: cross-lane emission seams (v26.11.1 wave 9) — A04 wire frame +
// A05 power-cut WAL. Every constant below is CITED from the authoritative
// artifact (file:line in the doc comment), not redefined: this crate has
// no cgap-core / cgap-storage dependency and its manifest is
// coordinator-owned, so the byte layouts are mirrored and pinned against
// the recorded authoritative values by the courts below (dependency-free
// consumer-lane pattern; `cgap-gate/src/penstock.rs` precedent). The
// cross-crate drift tripwire is A20's `cross_lane_abi_pin`.
// ========================================================================

/// A04 §6 flags bit 2 — the BN3 payload flag. (Cited:
/// `crates/cgap-core/src/frame.rs:32` `pub const FLAG_BN3: u8 = 0x04`;
/// A20's `cross_lane_abi_pin` holds the cross-crate equality.)
pub const BN3_FRAME_FLAG: u8 = 0x04;

/// A04 §6 canonical frame-header length. (Cited:
/// `crates/cgap-core/src/frame.rs:41` `pub const FRAME_HEADER_LEN: usize = 128`.)
pub const BN3_FRAME_HEADER_LEN: usize = 128;

/// A04 §6 frame magic. (Cited: `crates/cgap-core/src/frame.rs:20`
/// `pub const FRAME_MAGIC: [u8; 4] = *b"CGAP"`.)
pub const BN3_FRAME_MAGIC: [u8; 4] = *b"CGAP";

/// A04 §6 frame format version. (Cited: `crates/cgap-core/src/frame.rs:23`
/// `pub const FRAME_VERSION: u8 = 1`.)
pub const BN3_FRAME_VERSION: u8 = 1;

/// A05 §3.3 journal-record header size. (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:55`
/// `pub const HEADER_SIZE: usize = 48`.)
pub const BN3_WAL_HEADER_LEN: usize = 48;

/// A05 §3.4 payload size — the new-root descriptor width. (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:61`
/// `pub const PAYLOAD_SIZE: usize = 48`.)
pub const BN3_WAL_PAYLOAD_LEN: usize = 48;

/// A05 §3.3 trailer size. (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:58`
/// `pub const TRAILER_SIZE: usize = 16`.)
pub const BN3_WAL_TRAILER_LEN: usize = 16;

/// A05 §3.4: "a full record is 112 bytes". (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:65`
/// `pub const RECORD_SIZE: usize = HEADER_SIZE + PAYLOAD_SIZE + TRAILER_SIZE`.)
pub const BN3_WAL_RECORD_LEN: usize = BN3_WAL_HEADER_LEN + BN3_WAL_PAYLOAD_LEN + BN3_WAL_TRAILER_LEN;

/// A05 §3.3 record magic 0x4A414353 ("SCAJ" as u32 LE). (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:45`
/// `pub const RECORD_MAGIC: u32 = 0x4A41_4353`.)
pub const BN3_WAL_MAGIC: u32 = 0x4A41_4353;

/// A05 §3.3 trailer magic 0x524A4353. (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:48`
/// `pub const TRAILER_MAGIC: u32 = 0x524A_4353`.)
pub const BN3_WAL_TRAILER_MAGIC: u32 = 0x524A_4353;

/// A05 §3.3 record version. (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:51`
/// `pub const RECORD_VERSION: u16 = 0x0001`.)
pub const BN3_WAL_VERSION: u16 = 1;

/// A05 §3.3 RESERVED header field, must be zero. (Cited:
/// `crates/cgap-storage/src/atomic_commit.rs:93`
/// `pub const RESERVED_MUST_BE_ZERO: u32 = 0x0000_0000`.)
pub const BN3_WAL_RESERVED: u32 = 0;

/// The 48-byte BN3 new-root descriptor (A05 §3.4 payload SIZE, A12 payload
/// DOMAIN — recorded divergence 5): the identity of one committed fixpoint
/// delta. Field map (little-endian): `rule_id` u32 @0, `epoch` u64 @4,
/// `digest` [u8; 32] @12 (the A15 slab anchor, byte-exact),
/// `source_mask_popcount` u32 @44. 4+8+32+4 = 48.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bn3WalDescriptor {
    /// Emitting rule (`DeltaBatch::rule_id`).
    pub rule_id: u32,
    /// Fixpoint round that emitted the batch (`DeltaBatch::epoch`).
    pub epoch: u64,
    /// The A15 digest anchor (`DeltaBatch::digest`), byte-exact.
    pub descriptor_digest: [u8; 32],
    /// Audit popcount (`DeltaBatch::source_mask_popcount`).
    pub source_mask_popcount: u32,
}

impl Bn3WalDescriptor {
    /// Bind a descriptor to a real emitted batch (final-state read).
    pub fn of_batch<B>(batch: &DeltaBatch<B>) -> Self {
        Bn3WalDescriptor {
            rule_id: batch.rule_id,
            epoch: batch.epoch,
            descriptor_digest: batch.digest,
            source_mask_popcount: batch.source_mask_popcount,
        }
    }

    /// Encode the byte-exact 48-byte wire form (little-endian, field map
    /// per the struct docs; round-trip courted below).
    pub fn to_bytes(&self) -> [u8; BN3_WAL_PAYLOAD_LEN] {
        let mut b = [0u8; BN3_WAL_PAYLOAD_LEN];
        b[0..4].copy_from_slice(&self.rule_id.to_le_bytes());
        b[4..12].copy_from_slice(&self.epoch.to_le_bytes());
        b[12..44].copy_from_slice(&self.descriptor_digest);
        b[44..48].copy_from_slice(&self.source_mask_popcount.to_le_bytes());
        b
    }

    /// Decode the byte-exact 48-byte wire form. Panic-free reads (the
    /// penstock `read_u64_le` pattern); the 48-byte array makes every
    /// offset statically in-bounds.
    pub fn from_bytes(b: &[u8; BN3_WAL_PAYLOAD_LEN]) -> Self {
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&b[12..44]);
        Bn3WalDescriptor {
            rule_id: read_u32_le_at(b, 0),
            epoch: read_u64_le_at(b, 4),
            descriptor_digest: digest,
            source_mask_popcount: read_u32_le_at(b, 44),
        }
    }
}

/// Panic-free little-endian u32 read at a fixed offset. The caller
/// guarantees `off + 4 <= bytes.len()` (the record codec's offsets are
/// statically in-bounds); the byte-loop shape follows
/// `cgap-gate/src/penstock.rs::read_u64_le` for the same codegen reason
/// recorded there.
fn read_u32_le_at(bytes: &[u8], off: usize) -> u32 {
    debug_assert!(off + 4 <= bytes.len());
    let mut v = [0u8; 4];
    let mut i = 0;
    while i < 4 {
        v[i] = bytes[off + i];
        i += 1;
    }
    u32::from_le_bytes(v)
}

/// Panic-free little-endian u64 read (same proof duty as
/// [`read_u32_le_at`]).
fn read_u64_le_at(bytes: &[u8], off: usize) -> u64 {
    debug_assert!(off + 8 <= bytes.len());
    let mut v = [0u8; 8];
    let mut i = 0;
    while i < 8 {
        v[i] = bytes[off + i];
        i += 1;
    }
    u64::from_le_bytes(v)
}

/// IEEE CRC-32 (reflected, poly 0xEDB8_8320), bitwise table-free — the
/// same algorithm as the authoritative `atomic_commit.rs::crc32`
/// (cgap-storage). Private: the record codec's checksum substrate, pinned
/// by the check-value court (`wal_court_crc32_ieee_check_value`) and
/// exercised through the record codec everywhere else.
fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// One committed A12 WAL record, decoded: the journal sequence and the
/// delta identity it committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bn3WalCommit {
    /// Journal sequence number (A05 §3.3 SEQ, starts at 1).
    pub seq: u64,
    /// The delta identity committed by this record.
    pub descriptor: Bn3WalDescriptor,
}

/// Torn-write class of a truncated A12 record (A05 §5 crash matrix C0-C3
/// via `atomic_commit.rs::tear_class_for_offset`; C4 `CompleteRecord` is
/// the [`Bn3WalVerdict::Commit`] arm, not a torn class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bn3WalTearClass {
    /// C0: `k = 0` — nothing written; prior record stands.
    PreWrite,
    /// C1: `1..=47` — header torn.
    TornHeader,
    /// C2: `48..=95` — payload torn.
    TornPayload,
    /// C3: `96..=111` — trailer torn.
    TornTrailer,
}

/// Typed rejection of a FULL-LENGTH A12 WAL record. Variant set and names
/// mirror the authoritative `atomic_commit.rs::DecodeRejection` (the
/// storage-owned erased-slot state `Empty`, all-0xFF flash semantics, is
/// the medium's concern and has no A12 subject — recorded divergence 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bn3WalRefusal {
    /// MAGIC mismatch (record offset 0..4).
    BadMagic,
    /// VERSION mismatch (offset 4..6).
    BadVersion,
    /// HEADER_SIZE field mismatch (offset 6..8).
    BadHeaderSize,
    /// PAYLOAD_LEN field mismatch (offset 16..24; this framing fixes 48).
    BadPayloadLen,
    /// RESERVED field nonzero (offset 32..36).
    BadReserved,
    /// HEADER_CRC mismatch (offset 28..32; zero-fill rule).
    BadHeaderCrc,
    /// PAYLOAD_CRC mismatch (offset 24..28, over the 48 payload bytes).
    BadPayloadCrc,
    /// BLAKE3 header half mismatch (offset 36..48 vs BLAKE3(payload)[0..12]).
    BadBlake3Head,
    /// TRAILER_MAGIC mismatch (offset 96..100).
    BadTrailerMagic,
    /// BLAKE3 trailer half mismatch (offset 100..112 vs BLAKE3(payload)[12..24]).
    BadBlake3Tail,
}

/// Typed verdict for a (possibly truncated) A12 WAL record byte string —
/// the per-offset power-cut classification of A05 §2 ("the court
/// enumerates every `k in 0..=N`"). A prefix strictly shorter than a full
/// record is TORN by length alone (`atomic_commit.rs::tear_class_for_offset`
/// semantics: content never converts a prefix into a refusal; the prior
/// record stands). Only a full-length record reaches the strict field
/// chain, in the authoritative `decode_slot` order: magic, version,
/// header_size, payload_len, reserved, header_crc (zero-fill rule:
/// recomputed over `h[0..24] ++ zeros(24..32) ++ h[32..48]`, the recorded
/// cgap-storage divergence D1), payload_crc, BLAKE3 head half, trailer
/// magic, BLAKE3 tail half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bn3WalVerdict {
    /// Full-length record, every law passed: the delta committed.
    Commit(Bn3WalCommit),
    /// Truncated prefix of a commit in flight; `have` bytes survived.
    Torn {
        /// Bytes that survived the cut.
        have: usize,
        /// Crash-matrix class of the cut.
        class: Bn3WalTearClass,
    },
    /// Full-length record failing a framing/digest law.
    Refused(Bn3WalRefusal),
}

/// Typed verdict resolution: a (possibly truncated) A12 WAL record. See
/// [`Bn3WalVerdict`] for the classification law.
pub fn bn3_wal_classify(bytes: &[u8]) -> Bn3WalVerdict {
    if bytes.len() < BN3_WAL_RECORD_LEN {
        let class = if bytes.is_empty() {
            Bn3WalTearClass::PreWrite
        } else if bytes.len() < BN3_WAL_HEADER_LEN {
            Bn3WalTearClass::TornHeader
        } else if bytes.len() < BN3_WAL_HEADER_LEN + BN3_WAL_PAYLOAD_LEN {
            Bn3WalTearClass::TornPayload
        } else {
            Bn3WalTearClass::TornTrailer
        };
        return Bn3WalVerdict::Torn { have: bytes.len(), class };
    }
    let le32 = |o: usize| read_u32_le_at(bytes, o);
    let le64 = |o: usize| read_u64_le_at(bytes, o);
    let le16 = |o: usize| {
        debug_assert!(o + 2 <= bytes.len());
        u16::from_le_bytes([bytes[o], bytes[o + 1]])
    };

    if le32(0) != BN3_WAL_MAGIC {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadMagic);
    }
    if le16(4) != BN3_WAL_VERSION {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadVersion);
    }
    if le16(6) != BN3_WAL_HEADER_LEN as u16 {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadHeaderSize);
    }
    if le64(16) != BN3_WAL_PAYLOAD_LEN as u64 {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadPayloadLen);
    }
    if le32(32) != BN3_WAL_RESERVED {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadReserved);
    }
    // Zero-fill rule (authoritative divergence D1): HEADER_CRC is
    // recomputed over `h[0..24] ++ zeros(24..32) ++ h[32..48]`.
    let mut tmpl = [0u8; BN3_WAL_HEADER_LEN];
    tmpl[0..24].copy_from_slice(&bytes[0..24]);
    tmpl[32..48].copy_from_slice(&bytes[32..48]);
    if crc32_ieee(&tmpl) != le32(28) {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadHeaderCrc);
    }
    let mut pb = [0u8; BN3_WAL_PAYLOAD_LEN];
    pb.copy_from_slice(&bytes[BN3_WAL_HEADER_LEN..BN3_WAL_HEADER_LEN + BN3_WAL_PAYLOAD_LEN]);
    if crc32_ieee(&pb) != le32(24) {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadPayloadCrc);
    }
    let digest = cgap_blake3::hash(&pb);
    if bytes[36..48] != digest[0..12] {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadBlake3Head);
    }
    if le32(BN3_WAL_HEADER_LEN + BN3_WAL_PAYLOAD_LEN) != BN3_WAL_TRAILER_MAGIC {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadTrailerMagic);
    }
    if bytes[100..112] != digest[12..24] {
        return Bn3WalVerdict::Refused(Bn3WalRefusal::BadBlake3Tail);
    }
    Bn3WalVerdict::Commit(Bn3WalCommit {
        seq: le64(8),
        descriptor: Bn3WalDescriptor::from_bytes(&pb),
    })
}

/// Encoder side of [`bn3_wal_classify`]: the byte-exact 112-byte journal
/// record for `seq` carrying `desc` (A05 §3.3 framing: CRC-32 IEEE payload
/// checksum, zero-fill HEADER_CRC, 96-bit BLAKE3 split 12 header + 12
/// trailer so a torn write cannot leave a payload whose only digest bytes
/// were the ones destroyed).
pub fn bn3_wal_encode(desc: &Bn3WalDescriptor, seq: u64, out: &mut [u8; BN3_WAL_RECORD_LEN]) {
    let pb = desc.to_bytes();
    let payload_crc = crc32_ieee(&pb);
    let digest = cgap_blake3::hash(&pb);
    *out = [0u8; BN3_WAL_RECORD_LEN];
    out[BN3_WAL_HEADER_LEN..BN3_WAL_HEADER_LEN + BN3_WAL_PAYLOAD_LEN].copy_from_slice(&pb);
    out[0..4].copy_from_slice(&BN3_WAL_MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&BN3_WAL_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&(BN3_WAL_HEADER_LEN as u16).to_le_bytes());
    out[8..16].copy_from_slice(&seq.to_le_bytes());
    out[16..24].copy_from_slice(&(BN3_WAL_PAYLOAD_LEN as u64).to_le_bytes());
    out[24..28].copy_from_slice(&payload_crc.to_le_bytes());
    // HEADER_CRC needs the BLAKE3 half already placed; compute over the
    // zero-filled template, then store (authoritative order,
    // atomic_commit.rs::encode_record).
    out[36..48].copy_from_slice(&digest[0..12]);
    let mut head = [0u8; BN3_WAL_HEADER_LEN];
    head.copy_from_slice(&out[0..BN3_WAL_HEADER_LEN]);
    // Zero-fill rule: h[0..24] ++ zeros(24..32) ++ h[32..48].
    let mut tmpl = [0u8; BN3_WAL_HEADER_LEN];
    tmpl[0..24].copy_from_slice(&head[0..24]);
    tmpl[32..48].copy_from_slice(&head[32..48]);
    let hcrc = crc32_ieee(&tmpl);
    out[28..32].copy_from_slice(&hcrc.to_le_bytes());
    out[96..100].copy_from_slice(&BN3_WAL_TRAILER_MAGIC.to_le_bytes());
    out[100..112].copy_from_slice(&digest[12..24]);
}

/// A04 §6: the 128-byte `FLAG_BN3` frame header over a BN3 delta payload.
/// All multi-byte integers little-endian (A04 §6.1); `reserved` zeroed on
/// write (A04 §6 "zero on write"); the 80-byte attestation slot is carried
/// VERBATIM — attestations are A17's domain and A12 mints none.
pub fn bn3_frame_header_encode(
    payload_byte_len: u64,
    oabi_digest: &[u8; 32],
    attestation: &[u8; 80],
    out: &mut [u8; BN3_FRAME_HEADER_LEN],
) {
    *out = [0u8; BN3_FRAME_HEADER_LEN];
    out[0..4].copy_from_slice(&BN3_FRAME_MAGIC);
    out[4] = BN3_FRAME_VERSION;
    out[5] = BN3_FRAME_FLAG;
    out[6..8].copy_from_slice(&[0u8; 2]); // reserved: zero on write (A04 §6)
    out[8..16].copy_from_slice(&payload_byte_len.to_le_bytes());
    out[16..48].copy_from_slice(oabi_digest);
    out[48..128].copy_from_slice(attestation);
}

/// A04 §5.1 oabi digest binding for BN3 delta payloads: BLAKE3 over the
/// exact canonical payload bytes — the same injected kernel A04's
/// `CgapFrameHeader::verify_payload` names (`cgap_blake3::hash`), so the
/// frame's embedded digest and the payload's memory-tier bytes stay bound.
pub fn frame_oabi_digest(payload: &[u8]) -> [u8; 32] {
    cgap_blake3::hash(payload)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::unify::{
        unify_batch, BodyAtom, Rule, RuleCompiler, StatementColumns, UnifyError,
    };

    /// Test-local row-storage stand-in: proves the envelope ABI without
    /// the Wave-2 `cgap-arrow` dependency. Not a mock — a real concrete
    /// value of the parameter.
    #[derive(Clone, Debug, PartialEq)]
    struct NoRows;

    /// Real envelope value for the courts (Chicago: assert final state on
    /// a real constructed value, no interaction assertions).
    fn sample() -> DeltaBatch<NoRows> {
        DeltaBatch {
            rule_id: 7,
            epoch: 42,
            rows: NoRows,
            digest: [0xA5; DELTA_DIGEST_LEN],
            source_mask_popcount: 3,
        }
    }

    /// Test-only term-word builders (A11 sigil ABI: sigil in bits 63..56,
    /// payload in bits 55..0).
    fn cw(id: u64) -> u64 {
        (0x00u64 << 56) | id
    }
    fn vw(reg: u8) -> u64 {
        (0x01u64 << 56) | reg as u64
    }

    /// Compile a one-atom rule and run it over columns; shared helper for
    /// the projector courts (real collaborators end to end).
    fn project_rule(body: BodyAtom, head: [u64; 3], cols: (&[u64], &[u64], &[u64])) ->
        Result<DeltaBatch<DeltaRows>, UnifyError>
    {
        let rule = Rule { rule_id: 9, body: vec![body], head };
        let compiled = RuleCompiler::compile(&rule)?;
        let columns = StatementColumns::new(cols.0, cols.1, cols.2)?;
        let matches = unify_batch(&compiled, &columns, 4096)?;
        ConsequentProjector::new().project(&compiled, 1, &matches)
    }

    // --- Wave-1 envelope courts (kept verbatim) --------------------------

    /// Digest-width court (spec 4.4): the digest field is exactly 32
    /// bytes and the field type is tied to `DELTA_DIGEST_LEN` (widening
    /// the const re-types the field, so the const-law build failure and
    /// this court flip together).
    /// Mutation that MUST flip this court: `DELTA_DIGEST_LEN` 32 -> 64.
    #[test]
    fn digest_width_court_digest_is_32_bytes() {
        let b = sample();
        assert_eq!(b.digest.len(), 32);
        assert_eq!(b.digest, [0xA5u8; DELTA_DIGEST_LEN]);
    }

    /// Envelope-field court (spec 4.4): the frozen field set, types, and
    /// read-back of a real envelope value.
    /// Mutation that MUST flip this court: delete or rename any field
    /// (build failure), or change a scalar type (e.g. `rule_id` u32 ->
    /// u64 — the court's literal types stop compiling).
    #[test]
    fn envelope_field_court_frozen_field_set_reads_back() {
        let b = sample();
        assert_eq!(b.rule_id, 7_u32);
        assert_eq!(b.epoch, 42_u64);
        assert_eq!(b.rows, NoRows);
        assert_eq!(b.source_mask_popcount, 3_u32);
        // Determinism-contract anchor fields compile at their frozen
        // widths: rule_id 32-bit, epoch 64-bit, popcount 32-bit.
        let _width_witness: (u32, u64, u32) = (b.rule_id, b.epoch, b.source_mask_popcount);
    }

    // --- Wave 2: SHA-256 known-answer courts (FIPS 180-2 examples) -------

    /// KAT 1: the empty message. Mutation to any hash constant, rotation,
    /// or the message schedule MUST flip this vector.
    #[test]
    fn sha256_kat_empty_message() {
        let expect = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(sha256(b""), expect);
    }

    /// KAT 2: "abc".
    #[test]
    fn sha256_kat_abc() {
        let expect = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(sha256(b"abc"), expect);
    }

    /// KAT 3: the 56-byte FIPS example — two compression blocks (message
    /// + padding cross the 64-byte boundary), exercising multi-block mode.
    #[test]
    fn sha256_kat_two_block_boundary() {
        let expect = [
            0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8, 0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e,
            0x60, 0x39, 0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67, 0xf6, 0xec, 0xed, 0xd4,
            0x19, 0xdb, 0x06, 0xc1,
        ];
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            expect
        );
    }

    // --- Wave 2: projector courts ----------------------------------------

    /// Head-instantiation court: per-match register staging produces the
    /// hand-computed derived rows in match order. Also the M12-7 court:
    /// two matches here derive the IDENTICAL head row — the projector
    /// must emit both (derivations, not collapsed sets), so
    /// `source_mask_popcount == rows.len()` even across duplicates.
    /// Mutation that MUST flip this court: dedupe derivations inside the
    /// projector (spec 7.3 M12-7) — `rows.len()` drops to 2, the
    /// popcount invariant fails, and the duplicate disappears.
    #[test]
    fn projector_court_head_instantiation_and_m12_7_popcount_invariant() {
        // Store: (A,P1,B) (A,P1,C) (D,P1,B); rule ?x P1 ?y => (K, P1, ?x)
        // with K a constant. Matches: rows 0,1,2. Derived:
        // (K,P1,A) (K,P1,A) (K,P1,D) — rows 0 and 1 collide as rows.
        let s = [cw(1), cw(1), cw(4)];
        let p = [cw(100), cw(100), cw(100)];
        let o = [cw(2), cw(3), cw(2)];
        let head = [cw(50), cw(100), vw(10)];
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            head,
            (&s, &p, &o),
        )
        .unwrap();

        assert_eq!(batch.rule_id, 9);
        assert_eq!(batch.source_mask_popcount, 3, "three matches selected");
        assert_eq!(batch.rows.len(), 3, "derivations emitted, not collapsed");
        assert_eq!(
            batch.rows.row(0),
            (cw(50), cw(100), cw(1), 0),
            "x bound to A"
        );
        assert_eq!(
            batch.rows.row(1),
            (cw(50), cw(100), cw(1), 0),
            "duplicate derivation retained (M12-7)"
        );
        assert_eq!(batch.rows.row(2), (cw(50), cw(100), cw(4), 0));
    }

    /// Digest court: the batch digest is SHA-256 over the canonical bytes
    /// of the emitted rows, verified here against an independently built
    /// byte string of the hand-computed rows (not a re-call of
    /// `canonical_bytes` on the batch).
    /// Mutation that MUST flip this court: hashing rows in reverse order,
    /// or dropping the flags bytes, changes the digest.
    #[test]
    fn digest_court_sha256_over_canonical_row_bytes() {
        let s = [cw(1), cw(1), cw(4)];
        let p = [cw(100), cw(100), cw(100)];
        let o = [cw(2), cw(3), cw(2)];
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [cw(50), cw(100), vw(10)],
            (&s, &p, &o),
        )
        .unwrap();

        let mut expect = Vec::new();
        for (s_w, p_w, o_w, f_w) in [
            (cw(50), cw(100), cw(1), 0u16),
            (cw(50), cw(100), cw(1), 0u16),
            (cw(50), cw(100), cw(4), 0u16),
        ] {
            expect.extend_from_slice(&s_w.to_le_bytes());
            expect.extend_from_slice(&p_w.to_le_bytes());
            expect.extend_from_slice(&o_w.to_le_bytes());
            expect.extend_from_slice(&f_w.to_le_bytes());
        }
        assert_eq!(expect.len(), 3 * 26, "26 canonical bytes per row");
        assert_eq!(batch.digest, sha256(&expect));
    }

    /// Empty-emission court: a rule that matches nothing emits a
    /// well-formed empty batch whose digest is SHA-256 of the empty byte
    /// string (tied to the FIPS KAT above).
    /// Mutation that MUST flip this court: emitting a placeholder row or a
    /// nonzero popcount for an empty match set.
    #[test]
    fn projector_court_empty_match_emits_well_formed_empty_batch() {
        // Rule const-bound to an absent subject (999): zero matches.
        let s = [cw(1), cw(2)];
        let p = [cw(100), cw(100)];
        let o = [cw(3), cw(4)];
        let batch = project_rule(
            BodyAtom::new(cw(999), cw(100), vw(10)),
            [vw(10), cw(200), cw(1)],
            (&s, &p, &o),
        )
        .unwrap();
        assert_eq!(batch.rows.len(), 0);
        assert!(batch.rows.is_empty());
        assert_eq!(batch.source_mask_popcount, 0);
        let empty_kat = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(batch.digest, empty_kat);
    }

    /// Flags court (recorded divergence 3): the projector emits the
    /// reserved flags value 0 for every row — asserted, not assumed.
    /// Mutation that MUST flip this court: emitting a nonzero flags value.
    #[test]
    fn flags_court_reserved_zero_is_asserted() {
        let s = [cw(1)];
        let p = [cw(100)];
        let o = [cw(2)];
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&s, &p, &o),
        )
        .unwrap();
        assert_eq!(batch.rows.len(), 1);
        assert!(batch.rows.flags.iter().all(|&f| f == 0));
    }

    /// Direct-entry empty court (Wave-5 defect-D1 repair): a well-formed
    /// empty `MatchSet::default()` through the public `project` entry
    /// yields the well-formed empty batch — same digest (SHA-256 of the
    /// empty byte string, tied to the FIPS KAT), zero popcount, no panic.
    /// The kernel-path empty batch is courted by
    /// `projector_court_empty_match_emits_well_formed_empty_batch`; this
    /// court pins the same semantics for the direct entry.
    /// Mutation that MUST flip this court: reverting the alignment guard
    /// to `matches.atom_rows[0].len()` — the court FAILS with
    /// "index out of bounds" (witnessed 2026-11-01: the pre-repair probe
    /// `probe_d1_default_empty_matchset_no_panic` panicked at
    /// project.rs:322 before this repair; shim-court W5-M9 re-witnesses
    /// the flip on the repaired file).
    #[test]
    fn projector_court_default_empty_matchset_is_well_formed_empty_batch() {
        let rule = Rule {
            rule_id: 1,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
            head: [vw(11), cw(200), vw(10)],
        };
        let compiled = RuleCompiler::compile(&rule).unwrap();
        let ms = crate::unify::MatchSet::default();
        let batch = ConsequentProjector::new()
            .project(&compiled, 1, &ms)
            .unwrap();
        assert_eq!(batch.rule_id, 1);
        assert_eq!(batch.epoch, 1);
        assert_eq!(batch.rows.len(), 0);
        assert_eq!(batch.source_mask_popcount, 0);
        let empty_kat = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(batch.digest, empty_kat);
    }

    /// Emission-determinism court (projector-local half of M12-6):
    /// projecting the same (rule, match set) twice yields byte-identical
    /// batches. The full chunk-partition court lives in `unify.rs` tests.
    /// Mutation that MUST flip this court: any nondeterministic ordering
    /// or time-seeded content in emission.
    #[test]
    fn projector_court_repeat_emission_is_byte_identical() {
        let s = [cw(1), cw(2)];
        let p = [cw(100), cw(100)];
        let o = [cw(3), cw(4)];
        let a = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&s, &p, &o),
        )
        .unwrap();
        let b = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&s, &p, &o),
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.digest, b.digest);
    }
    // ====================================================================
    // Wave 9: cross-lane integration courts (A04 frame identity + A05
    // power-cut WAL). Spec: wave-9 slice "A04 identity court, A14 dam over
    // A04 frames, A05 power-cut sweep" — A12's producer side, in-lease.
    // ====================================================================

    /// CRC-32 check-value court: `crc32_ieee` is the FIPS-style known
    /// answer of the IEEE polynomial (the standard check value
    /// CRC-32("123456789") = 0xCBF43926), pinning the reflected poly +
    /// init/final-xor against the authoritative `atomic_commit.rs::crc32`
    /// (same algorithm, independent substrate).
    /// Mutation that MUST flip this court: any init/constant/xor change in
    /// `crc32_ieee` (the check value moves).
    #[test]
    fn wal_court_crc32_ieee_check_value() {
        assert_eq!(super::crc32_ieee(b"123456789"), 0xCBF4_3926);
        assert_eq!(super::crc32_ieee(b""), 0x0000_0000);
    }

    /// Descriptor court (A05 §3.4 payload domain, A12 divergence 5):
    /// a descriptor built from a REAL projected batch reads back
    /// field-for-field, the 48-byte wire form is pinned byte-exactly
    /// against a hand-built expectation, and the byte form round-trips.
    /// Mutation that MUST flip this court: any field-map change (offsets,
    /// widths, endianness) in `to_bytes`/`from_bytes`.
    #[test]
    fn wal_court_descriptor_binds_batch_identity() {
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&[cw(1), cw(2)], &[cw(100), cw(100)], &[cw(3), cw(4)]),
        )
        .unwrap();
        let d = Bn3WalDescriptor::of_batch(&batch);
        assert_eq!(d.rule_id, batch.rule_id);
        assert_eq!(d.epoch, batch.epoch);
        assert_eq!(d.descriptor_digest, batch.digest);
        assert_eq!(d.source_mask_popcount, batch.source_mask_popcount);

        let b = d.to_bytes();
        // Hand-built expectation (independent of to_bytes' field map).
        let mut expect = [0u8; 48];
        expect[0..4].copy_from_slice(&batch.rule_id.to_le_bytes());
        expect[4..12].copy_from_slice(&batch.epoch.to_le_bytes());
        expect[12..44].copy_from_slice(&batch.digest);
        expect[44..48].copy_from_slice(&batch.source_mask_popcount.to_le_bytes());
        assert_eq!(b, expect);

        assert_eq!(Bn3WalDescriptor::from_bytes(&b), d);
    }

    /// Encode→classify round-trip court: the byte-exact 112-byte record
    /// classifies back to Commit with the SAME sequence and SAME delta
    /// identity; the framing geometry (112 = 48+48+16) and both magics are
    /// pinned.
    /// Mutation that MUST flip this court: any encoder/decoder field-map
    /// or checksum disagreement (classify stops returning Commit).
    #[test]
    fn wal_court_encode_classify_roundtrip_is_commit() {
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&[cw(1)], &[cw(100)], &[cw(3)]),
        )
        .unwrap();
        let desc = Bn3WalDescriptor::of_batch(&batch);
        let mut rec = [0u8; BN3_WAL_RECORD_LEN];
        bn3_wal_encode(&desc, 7, &mut rec);
        assert_eq!(rec.len(), 112);
        assert_eq!(u32::from_le_bytes(rec[0..4].try_into().unwrap()), BN3_WAL_MAGIC);
        assert_eq!(
            u32::from_le_bytes(rec[96..100].try_into().unwrap()),
            BN3_WAL_TRAILER_MAGIC
        );
        assert_eq!(
            bn3_wal_classify(&rec),
            Bn3WalVerdict::Commit(Bn3WalCommit { seq: 7, descriptor: desc })
        );
    }

    /// POWER-CUT SWEEP court (A05 §2 method over A12's own records; the
    /// wave-9 slice's "A05 power-cut sweep" item): for EVERY cut
    /// `k in 0..=112`, the truncated record resolves to a typed verdict —
    /// `Torn { have: k }` with the A05 crash-matrix class strictly by
    /// offset (content never converts a prefix into a refusal), and only
    /// the complete record commits. 113 offsets, all typed, zero waits:
    /// deterministic truncation replaces wall-clock guessing.
    /// Mutation that MUST flip this court: an off-by-one in the tear
    /// boundary (e.g. classifying `have == 111` as Commit, or shifting a
    /// class edge by one byte) — the sweep observes every boundary.
    #[test]
    fn wal_court_power_cut_sweep_every_offset_is_typed() {
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&[cw(1), cw(2)], &[cw(100), cw(100)], &[cw(3), cw(4)]),
        )
        .unwrap();
        let desc = Bn3WalDescriptor::of_batch(&batch);
        let mut rec = [0u8; BN3_WAL_RECORD_LEN];
        bn3_wal_encode(&desc, 42, &mut rec);

        for k in 0..=BN3_WAL_RECORD_LEN {
            if k < BN3_WAL_RECORD_LEN {
                let expect_class = match k {
                    0 => Bn3WalTearClass::PreWrite,
                    1..=47 => Bn3WalTearClass::TornHeader,
                    48..=95 => Bn3WalTearClass::TornPayload,
                    _ => Bn3WalTearClass::TornTrailer,
                };
                match bn3_wal_classify(&rec[..k]) {
                    Bn3WalVerdict::Torn { have, class } => {
                        assert_eq!(have, k, "cut at {k}");
                        assert_eq!(class, expect_class, "cut at {k}");
                    }
                    other => panic!("cut at {k}: unexpected verdict {other:?}"),
                }
            } else {
                match bn3_wal_classify(&rec) {
                    Bn3WalVerdict::Commit(c) => {
                        assert_eq!(k, BN3_WAL_RECORD_LEN);
                        assert_eq!(c.seq, 42);
                        assert_eq!(c.descriptor, desc);
                    }
                    other => panic!("full record: unexpected verdict {other:?}"),
                }
            }
        }
    }

    /// Corruption-typing court: each framing law produces EXACTLY its named
    /// refusal on a full-length record. Because the field chain is ordered
    /// (authoritative `decode_slot` order) and each corruption sits in
    /// exactly one law's field, the exact-variant assertions also PIN THE
    /// ORDER (e.g. the payload_len byte would also break HEADER_CRC; the
    /// observed refusal must be the earlier law's).
    /// Mutation that MUST flip this court: removing or reordering a law in
    /// `bn3_wal_classify` (the corrupted record then resolves to a later
    /// variant or Commit).
    #[test]
    fn wal_court_corruption_is_typed_refusal() {
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&[cw(1)], &[cw(100)], &[cw(3)]),
        )
        .unwrap();
        let desc = Bn3WalDescriptor::of_batch(&batch);
        let mut rec = [0u8; BN3_WAL_RECORD_LEN];
        bn3_wal_encode(&desc, 3, &mut rec);

        // Each corruption names the refusal its field's law produces under
        // the ordered chain. BLAKE3-head bytes sit inside HEADER_CRC's
        // zero-fill coverage (32..48), so that one case re-fixes HEADER_CRC
        // after the flip — the A05 M-class form (corrupt, then make the
        // EARLIER laws consistent, so the LATER law is what fires).
        let cases: [(usize, Bn3WalRefusal, bool); 10] = [
            (1, Bn3WalRefusal::BadMagic, false),         // magic
            (5, Bn3WalRefusal::BadVersion, false),       // version
            (7, Bn3WalRefusal::BadHeaderSize, false),    // header_size
            (20, Bn3WalRefusal::BadPayloadLen, false),   // payload_len
            (33, Bn3WalRefusal::BadReserved, false),     // reserved
            (28, Bn3WalRefusal::BadHeaderCrc, false),    // header_crc field
            (26, Bn3WalRefusal::BadPayloadCrc, false),   // payload_crc field
            (40, Bn3WalRefusal::BadBlake3Head, true),    // blake3 head half
            (98, Bn3WalRefusal::BadTrailerMagic, false), // trailer magic
            (105, Bn3WalRefusal::BadBlake3Tail, false),  // blake3 tail half
        ];
        for (off, expect, refix_header_crc) in cases {
            let mut corrupt = rec;
            corrupt[off] ^= 0x01;
            if refix_header_crc {
                let mut tmpl = [0u8; BN3_WAL_HEADER_LEN];
                tmpl[0..24].copy_from_slice(&corrupt[0..24]);
                tmpl[32..48].copy_from_slice(&corrupt[32..48]);
                let hcrc = super::crc32_ieee(&tmpl);
                corrupt[28..32].copy_from_slice(&hcrc.to_le_bytes());
            }
            assert_eq!(
                bn3_wal_classify(&corrupt),
                Bn3WalVerdict::Refused(expect),
                "byte {off}"
            );
            // The corrupted record must NOT read back as the original
            // descriptor: damage is never laundered into a commit.
            assert_ne!(
                bn3_wal_classify(&corrupt),
                Bn3WalVerdict::Commit(Bn3WalCommit { seq: 3, descriptor: desc })
            );
        }
    }

    /// A04 §6 layout pin court: the FLAG_BN3 header this lane writes is
    /// byte-exact against an independently hand-built 128-byte
    /// expectation — magic/version/flag/reserved/payload_len/oabi
    /// digest/attestation-verbatim at the authoritative offsets. The
    /// attestation bytes are caller-supplied (A17's domain; A12 mints
    /// none) and must survive untouched.
    /// Mutation that MUST flip this court: any offset, flag-bit, or
    /// zero-fill change in `bn3_frame_header_encode` (e.g. FLAG_BN3
    /// 0x04 -> 0x05, or attestation zeroing).
    #[test]
    fn frame_court_flag_bn3_header_layout_pin() {
        let mut att = [0u8; 80];
        for (i, b) in att.iter_mut().enumerate() {
            *b = (i as u8) ^ 0xA5;
        }
        let payload_len = 26u64 * 3;
        let digest: [u8; 32] = core::array::from_fn(|i| (i as u8) << 4 | 0x07);
        let mut hdr = [0u8; BN3_FRAME_HEADER_LEN];
        bn3_frame_header_encode(payload_len, &digest, &att, &mut hdr);

        // Independent hand-built expectation (court oracle).
        let mut expect = [0u8; 128];
        expect[0..4].copy_from_slice(b"CGAP");
        expect[4] = 1;
        expect[5] = 0x04; // FLAG_BN3 (A04 §6 flags bit 2)
        // reserved 6..8 stays zero
        expect[8..16].copy_from_slice(&payload_len.to_le_bytes());
        expect[16..48].copy_from_slice(&digest);
        expect[48..128].copy_from_slice(&att);
        assert_eq!(hdr, expect);
    }

    /// A04 identity court at the A12 seam (wave-9 slice item): a REAL
    /// delta batch's canonical bytes are the memory-tier projection; the
    /// packed frame carries them byte-identical as its payload; the
    /// embedded oabi digest re-derives over the payload bytes
    /// (`frame_oabi_digest` == `cgap_blake3::hash`); and a one-byte
    /// payload tamper is DETECTED by digest re-derivation (A04 §8.3
    /// "corrupt payload byte -> DigestMismatch" class).
    /// Mutation that MUST flip this court: any canonical-byte or digest
    /// drift between the projector's emission and the frame payload.
    #[test]
    fn frame_court_bn3_delta_payload_identity() {
        let batch = project_rule(
            BodyAtom::new(vw(10), cw(100), vw(11)),
            [vw(11), cw(200), vw(10)],
            (&[cw(1), cw(2), cw(3)], &[cw(100); 3], &[cw(4), cw(5), cw(6)]),
        )
        .unwrap();

        // Memory tier: the projector's canonical bytes.
        let canonical = batch.rows.canonical_bytes();
        assert_eq!(canonical.len(), 26 * batch.rows.len());

        // Wire tier: header + payload, digest bound by BLAKE3.
        let digest = frame_oabi_digest(&canonical);
        assert_eq!(digest, cgap_blake3::hash(&canonical));
        let mut hdr = [0u8; BN3_FRAME_HEADER_LEN];
        bn3_frame_header_encode(canonical.len() as u64, &digest, &[0u8; 80], &mut hdr);
        let mut frame = Vec::with_capacity(BN3_FRAME_HEADER_LEN + canonical.len());
        frame.extend_from_slice(&hdr);
        frame.extend_from_slice(&canonical);
        assert_eq!(frame.len(), 128 + canonical.len());

        // Identity: the payload region of the frame IS the canonical
        // bytes, and the header's embedded digest matches the re-derivation.
        assert_eq!(&frame[128..], canonical.as_slice());
        let embedded = <[u8; 32]>::try_from(&frame[16..48]).unwrap();
        assert_eq!(embedded, frame_oabi_digest(&frame[128..]));

        // Tamper detection: one flipped payload byte breaks the binding.
        let mut tampered = frame.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert_ne!(
            frame_oabi_digest(&tampered[128..]),
            <[u8; 32]>::try_from(&tampered[16..48]).unwrap(),
            "one-byte payload tamper must break the oabi binding"
        );
    }
}
