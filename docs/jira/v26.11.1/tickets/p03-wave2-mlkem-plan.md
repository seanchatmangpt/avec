# p03-wave2-mlkem-plan

Wave-2 ML-KEM (FIPS 203) execution plan for the v26.11.1 PQC milestone.
Agent 18 of 20; single-file lane; no git operations performed.

## 0. Provenance, Slot Reconciliation, Standing

- Written 2026-10-31 into the dispatched slot
  `docs/jira/v26.11.1/tickets/p03-wave2-mlkem-plan.md`. This lane writes
  ONLY this file.
- **Slot reconciliation.** The milestone index (`../00-index.md`,
  agent-1-owned) maps `p03` to `tickets/p03-wave2-plan.md` and labels
  Wave 2 "ML-DSA". This dispatch names `p03-wave2-mlkem-plan.md` and
  labels Wave 2 "ML-KEM", under dispatch-local wave numbering: wave-1 =
  arithmetic core, wave-2 = ML-KEM KEM layer, wave-4 = dk pool. Both
  readings cannot be the index row. Per the index's own law ("the index
  is amended, never bypassed"), the index owner must amend the p03 row
  to the dispatched filename and wave numbering. Until that amendment,
  this file is the authoritative p03 content per dispatch. Recorded
  here as an integration defect for the index owner, not silently
  reconciled.
- **t01-t03 consumption.** `tickets/` did not exist on disk at write
  time (2026-10-31); t01 (FIPS 203 spec), t02 (FIPS 204), t03 (SLH-DSA
  survey per the index) have not landed. This plan is STANDALONE. The
  "3-court stack from t03" is restated in full in section 3 so the plan
  is self-contained; if a court-corpus ticket lands (index slot t13 is
  the likely canonical home), section 3 binds to it and this doc is
  amended by its owner. Hooks: when t01 lands, its parameter tables and
  KAT-corpus pointers supersede the provisional numbers here; when
  t05/t07 land, section 3.1 corpus provenance binds to them.
- **Standing.** Everything in this document is UNVERIFIED. No kernel
  exists, no KAT has run, no receipt exists. All parameter numbers
  below enter as KAT-validation inputs only (index discipline 4) and
  become facts only when a KAT receipt says so.
- License: every file this plan lands carries the P28-SIL v2.1.0 URN
  (`urn:avec:license:p28-sil:v2.1.0-sovereign`), no SPDX.

## 1. Scope

### 1.1 Deliverable: KeyGen / Encaps / Decaps on the wave-1 core

ML-KEM per FIPS 203 (final, 2024-08-13), all three parameter sets.
Sizes below are UNVERIFIED until the KAT corpus pins them.

| set | k | ek | dk | ct | ss | dk_pke |
|---|---|---|---|---|---|---|
| ML-KEM-512 | 2 | 800 | 1632 | 768 | 32 | 768 |
| ML-KEM-768 | 3 | 1184 | 2400 | 1088 | 32 | 1152 |
| ML-KEM-1024 | 4 | 1568 | 3168 | 1568 | 32 | 1536 |

Wave-1 dependency contract (consumed, not re-specified; the wave-1
arithmetic-core lane owns this surface and its own courts):

- NTT / inverse NTT over q=3329, n=256, in place, fixed trip counts.
- Montgomery/Barrett reduction, canonical barrett rounding.
- Polynomial add / sub / multiply, matrix sampling from rho.
- CBD / PRF sampling, ByteEncode/ByteDecode for d=1..12.
- H, G, J via hand-rolled SHA3-256/512 + SHAKE-128/256 (the t08
  zero-dep substrate role). If the substrate lane has not landed an
  XOF/hash module by wave-2 start, wave-2 implements
  `src/mlkem/xof.rs` as an owned fallback and discloses it loudly;
  the blocker branches the graph, it does not stop the wave.

Wave-2 owns the KEM layer above that ABI: FO transform, key tuple,
serialization, the CT rejection machinery, and the courts.

The KEM algorithms, as implemented:

- `KeyGen(d, z)` -> `(ek, dk)`: deterministic core, FIPS 203 alg 16.
- `Encaps(ek, m)` -> `(ss, ct)`: `(K_bar, r) = G(m || H(ek))`;
  `c = PKE.Encrypt(ek, m, r)`; `ss = K_bar`.
- `Decaps(dk, c)` -> `ss`: `m' = PKE.Decrypt(dk_pke, c)`;
  `(K_bar, r') = G(m' || H(ek))`; `c' = PKE.Encrypt(ek, m', r')`;
  compare `c'` vs `c`; select `K_bar` or `J(z || c)`. FIPS 203 alg 17.

Out of scope for wave-2: ML-DSA (next wave), SLH-DSA and Falcon
(p01-gated), the BEAM NIF surface and manifest join (wave-5 / index
W5), and any higher-order masking (see 1.2).

### 1.2 The masked / constant-time rejection strategy

"Masked" here means arithmetic-mask select, NOT Boolean sharing /
d-plus-1 masking. Higher-order masking is out of scope; the M=1 law
(0 data-dependent branches on hot data paths) is the discipline.

The danger site in ML-KEM is exactly one place: the Decaps ciphertext
comparison feeding the implicit-rejection select. The naive form:

```text
if c != c_prime { return J(z || c) }   // BANNED: secret-dependent branch
```

The wave-2 form — compute both, select arithmetically:

1. **Always derive both candidate keys.** `K_bar` (honest path) and
   `J(z || c)` (rejection path) are computed unconditionally, in the
   same order, for every input. Timing is independent of the equality
   result by construction, not by discipline.
2. **Constant-time equality accumulator.** 32-byte compare:
   `diff |= a[i] ^ b[i]` over all bytes, then OR-folding to one word,
   then arithmetic zero-test producing an all-ones / all-zeros mask.
   Pure ALU ops; no branch mnemonic on the data path.
3. **Arithmetic select.** `ss = (K_bar & mask) | (J_zc & !mask)` per
   byte. On aarch64 this must compile to csel/EOR/AND-class ops; the
   branchless manifest row for `ct_select` pins the branch ceiling at
   zero branch-mnemonic sites on the data path (entry guard classes
   are adjudicated separately; see 3.5).
4. **Infallible by type.** `fn decaps(&DecapsKey, &[u8; CT_LEN])
   -> [u8; 32]` — no `Result`, no panic path on any well-formed input.
   The implicit-rejection property is structural: there is no error
   path that could bypass the select. Length mismatch is impossible
   at the typed API; byte-level corruption is the entire input space
   the court must cover.
5. **Trip counts are compile-time constants** (k, n=256, 32), so loop
   structure never depends on secret data. Sampling, compression, and
   serialization have data-independent control flow by construction;
   the ct comparison is the only secret-dependent equality in the KEM.

### 1.3 Pool integration seam (wave-4)

dk goes into the wave-4 pool. The wave-2 interface defines the tuple
type NOW, so wave-4 consumes instead of redefining (falsifier F-07).

Frozen interface (field level; layout freedom stays with wave-2):

```rust
pub struct DecapsKey {
    set: ParamSet,            // MlKem512 | MlKem768 | MlKem1024
    dk_pke: [u8; 1536],       // 768*k bytes, prefix per set
    ek: [u8; 1568],           // 384*k + 32
    h_ek: [u8; 32],           // H(ek), FIPS 203 carries it in dk
    z: [u8; 32],              // implicit-rejection seed
}

impl DecapsKey {
    /// Pool key: the pool indexes dk by H(ek).
    pub fn pool_key(&self) -> &[u8; 32] { &self.h_ek }
    /// FIPS 203 wire form: dk_pke || ek || H(ek) || z.
    pub fn to_bytes(&self) -> [u8; 3168];
    /// Validates h_ek == H(ek) on parse (free in FIPS 203 layout).
    pub fn from_bytes(&[u8]) -> Result<Self, KeyError>;
}
```

Notes pinned at interface level:

- Zero-alloc applies to the tuple itself: fixed-size buffers, no heap
  fields, so the wave-4 pool can store it inline or in a slab without
  an allocation seam of its own.
- `from_bytes` validation is the one honest error site: a dk whose
  stored `h_ek` does not match `H(ek)` is a corrupted-tuple error at
  the parse boundary, never inside decaps.
- Wave-4 consumes `DecapsKey` + `pool_key()`; it does not define a
  second tuple type, a second serialization, or a second hash of ek.

### 1.4 Exclusions (explicit, non-re-invading)

- No ML-DSA code paths, no shared-signature abstractions "while we're
  here". Wave-3 owns ML-DSA; `ct_select.rs` is the only named
  promotion candidate if ML-DSA needs the same CT compare (promote by
  move to `src/ct.rs` at that wave, disclosed).
- No external dependencies, dev-time or runtime, inside `avec-pqc`
  (discipline 2). Dev-time corpus generation uses out-of-repo tooling
  whose output is committed with a provenance header (b3sum precedent
  from `crates/avec-blake3/tests/official_vectors.rs`).
- No fixed sleeps anywhere in the courts (no-sleep testing law);
  measurement is iteration-driven with bounded polls on observables.
- No worktrees, no per-lane copies; single canonical checkout.

## 2. File Ownership Map

This lane (wave-2) owns, in the new workspace member `avec-pqc`:

| path | content |
|---|---|
| `crates/avec-pqc/Cargo.toml` | crate manifest, zero deps, dev-deps empty |
| `src/mlkem/mod.rs` | module root, re-exports, docs (URN header) |
| `src/mlkem/params.rs` | ParamSet enum; k/n/q/eta/du/dv; all sizes |
| `src/mlkem/keygen.rs` | KeyGen(d,z) core + keygen_rng wrapper |
| `src/mlkem/encaps.rs` | Encaps(ek,m) core + encaps_rng wrapper |
| `src/mlkem/decaps.rs` | Decaps; orchestration; infallible signature |
| `src/mlkem/ct_select.rs` | CT equality accumulator + masked select |
| `src/mlkem/serde.rs` | KEM-level encode/decode; DecapsKey tuple |
| `src/mlkem/xof.rs` | CONTINGENCY ONLY (see 1.1 substrate hook) |
| `tests/kat_parity.rs` | court 1: KAT I/O byte-parity (THE gate) |
| `tests/kat_intermediates.rs` | court 2: intermediate-value parity |
| `tests/kat_negative.rs` | court 3: implicit-rejection property |
| `tests/timing_dudect.rs` | dudect court (classes, t-stat, mutant) |
| `tests/zero_alloc_kem.rs` | zero-alloc court, encap/decap paths |
| `tests/vectors/mlkem/` | KAT corpus + provenance headers |
| `benches/mlkem_ctq.rs` | harness=false, hand-rolled CTQ harness |

Rules: `#[inline(always)]` is banned on `ct_select`, `decaps`,
`encaps`, `keygen` entry symbols — inlining merges the symbol into a
caller and recreates the `<ltmp0>` blind spot the manifest already
documents for `untranspose` (wave-9 note in
`scripts/branchless_manifest.toml`).

Coordination seams this lane does NOT own:

- Workspace `Cargo.toml` members list (add `avec-pqc`): integration
  lane; disclosed diff, one line.
- `scripts/branchless_manifest.toml`: this lane drafts and appends its
  own `[[kernel]]` rows (3.5) — appends are lane-owned additions,
  disclosed in the wave receipt; no edits to existing rows.
- `scripts/audit_branchless.sh` default `BRANCHLESS_SCOPE`: integration
  lane; wave-2 adjudicates under explicit scope until amended.
- `crates/bench`: wave-2 stays out; its own harness=false bench bin is
  the CTQ surface until the integration wave wires it into the
  milestone report.

## 3. Court Inventory

The 3-court stack (dispatch t03 / index t13), restated inline because
the source ticket has not landed, plus the three extended courts.

### 3.1 Court 1 — KAT I/O parity (THE gate)

- Corpus: ACVP-style ML-KEM vectors exercising the deterministic
  entry points: keyGen from (d, z); encapDecap from (d, z, m). Per
  parameter set minimum: 3 keyGen vectors + 10 encapDecap vectors;
  expand to full ACVP files when t07 lands.
- Byte-parity assert on ek, dk, ct, ss. Any single mismatch is exit 1.
  Conformance outranks throughput (discipline 1): this court is THE
  gate; every other number in this plan is advisory.
- Embedding without deps: hex-line corpus files under
  `tests/vectors/mlkem/`, parsed by a hand-rolled reader inside the
  test file. No JSON, no serde.
- Provenance header per corpus file: source implementation and
  commit, generation command, sha256 of the emitted file — the same
  discipline `official_vectors.rs` used for b3sum vectors.
- Fallback if ACVP files are unobtainable at implementation time:
  generate the corpus from a reference ML-KEM (PQClean-class) out of
  repo, commit outputs with the provenance header. Dev-time tooling
  is not a runtime dependency; discipline 2 governs runtime.

### 3.2 Court 2 — intermediate-value parity (conditional)

- Pins KEM-layer intermediates against a reference implementation:
  `m'`, `(K_bar, r')`, `c'`, and the J-domain rejection key. NTT-domain
  intermediates are wave-1's courts, not this one's.
- Source: dev-time extraction from reference C code (PQClean-class)
  if obtainable. This is a conditional court by dispatch design.
- If intermediates are NOT obtainable by wave-2 start: the court
  records `[SKIP: intermediates unobtainable]` — a loud, registered
  skip with a close condition, never a faked pass, never a silent
  omission. The court stays on this register until closed or landed.

### 3.3 Court 3 — negative KAT: implicit rejection (THE property court)

This pins THE ML-KEM implicit-rejection property as a court. Semantics
pinned:

- `decaps` NEVER errors or panics on any well-formed (correct-length)
  ct. Exercised with single-bit and multi-bit corrupted ct from the
  corpus, plus pseudo-random ct bytes.
- Corrupted ct yields `ss_prime != ss` for the honest ss (asserted on
  every corpus entry).
- The pseudorandom rejection key is pinned exactly: the corpus carries
  the expected `J(z || ct_bad)` output per negative vector, computed
  by the reference implementation, and the court asserts byte-parity
  on it. Determinism pinned too: same (dk, ct_bad) => same ss_prime.
- Positive control in the same court: decaps(dk, honest ct) == ss on
  every honest vector.
- A decaps that returns `Result`, panics, short-circuits on the
  comparison, or produces `ss_prime == ss` on any corrupted vector
  FAILs this court (exit 1), which is exit 1 for the wave.

### 3.4 Court 4 — dudect timing court

- Classes, each fix-vs-random (one fixed secret, random per trial):
  (a) decaps, dk fixed vs random, ct random;
  (b) decaps, ct fixed vs random, dk fixed;
  (c) encaps, ek fixed vs random;
  (d) keygen, (d, z) fixed vs random.
- Measurement: aarch64 `cntvct_el0` counter (house machine Mac15,9),
  per-iteration, no wall clock, no sleeps; iteration counts fixed.
- Processing per dudect (Reparaz et al., USENIX 2017): fixed-trim
  percentile cropping of both tails, then Welch t-test per class.
- Bound: `|t| < 4.5` in EVERY post-warmup battery for EVERY class.
  Minimum 3 batteries; battery 1 is cache/branch warmup, discarded.
- **Non-vacuity witness (mandatory).** A test-only branchy decaps
  mutant (an early-return on the ct compare, compiled only under
  `cfg(test)`) must trip `|t| >= 4.5`. A timing court that cannot
  detect the deliberate leak is a vacuous gate and FAILs as such.
  Same law as `audit_branchless.sh --selftest`.

### 3.5 Court 5 — branchless manifest extension

New `[[kernel]]` rows appended to `scripts/branchless_manifest.toml`
for the rejection-loop symbols (schema per the existing manifest):

```toml
[[kernel]]
name = "ml-kem ct select"
owner = "crates/avec-pqc/src/mlkem/ct_select.rs"
scope_pattern = "ct_select"
accepted_remainder = ["ENTRY_GUARDS (to be classified at first audit)"]
expected_max_count = 0      # UNVERIFIED placeholder; observed+1 law
gate_mode = "ENFORCED"
observed_mnemonics = []     # filled from first real audit-profile run
```

- Rows: `ct_select` (ENFORCED, zero branch sites on the data path),
  `decaps`, `encaps`, `keygen` (INFORMATIONAL until first audit pins
  observed counts; then per the observed+1 ceiling law used by every
  existing row).
- accepted_remainder classes, drawn from the existing named set:
  LOOP_LATCH, ZERO_TRIP_GUARD, ALLOCATOR_NULL_CHECK,
  RESULT_ERROR_GUARD. Unlisted mnemonics are FAIL even under the
  ceiling (existing gate law).
- Ordering law: first-match manifest. New scope patterns must not
  substring-collide with each other or with existing rows
  (`transpose`, `predicates`, `ladder`, `6lookup`, `6offset`,
  `chunks_4way`); verified at append time.
- Scope gap, documented not silent: the gate's default
  `BRANCHLESS_SCOPE` does not include the mlkem scopes at plan time.
  Wave-2 adjudicates under explicit
  `BRANCHLESS_SCOPE='...|ct_select|decaps|encaps|keygen'` with the
  audit profile and llvm-objdump (Apple objdump renders mach-o
  symbols as `<ltmp0>`, which defeats name scoping). Default-scope
  amendment is an integration-lane follow-up.
- Non-vacuity: the BRANCHLESS_MANIFEST override negative control
  (ceiling 2 -> 0 trick, per `gate-branchless-manifest.md`) must FAIL
  against a real mlkem artifact.

### 3.6 Court 6 — zero-alloc on encaps/decaps paths

- House idiom from `crates/avec-blake3/tests/zero_alloc.rs`: counting
  GlobalAlloc wrapper, per-thread const-init TLS counters,
  process-wide mutex taken at test start, counters reset inside the
  lock immediately before the measured body.
- Measured paths: KeyGen, Encaps, Decaps across all three parameter
  sets — plus Decaps on corrupted ct (the rejection path must also be
  zero-alloc: the select is arithmetic, not a branch, not an alloc).
- Asserts allocation delta == 0 AND byte delta == 0 in the window.

## 4. CTQ Proposals

Advisory until measured and DFLSS-ratified; ADR-19 amendment path
applies. All numeric anchors are UNVERIFIED. Ratio fences against an
independent baseline are the lawful form (absolute ms/cycle bounds
fence exactly one leaked constant and hug retry constants); the
baseline is a PQClean-class aarch64 reference measured on the same
machine in the same window.

| id | metric | lawful form | advisory anchor |
|---|---|---|---|
| CTQ-PQC-01 | KeyGen cycles/op | <= 2.0x reference, min-of-N | recorded at first run |
| CTQ-PQC-02 | Encaps cycles/op | <= 2.0x reference, min-of-N | recorded at first run |
| CTQ-PQC-03 | Decaps cycles/op | <= 2.0x reference, min-of-N | recorded at first run |
| CTQ-PQC-04 | dudect t-stat | \|t\| < 4.5 all classes/batteries | gate-class, see 3.4 |
| CTQ-PQC-05 | pool handoff latency | seam median <= 1.5x raw `to_bytes` median | N=1000; <= 1 us |
| CTQ-PQC-06 | allocations | 0 allocs / 0 bytes on all measured paths | gate-class, see 3.6 |

- CTQ-PQC-05 definition: median ns from a decaps result in hand to
  the wave-4 pool holding the serialized tuple —
  `decaps -> DecapsKey::to_bytes -> pool.insert`. Until wave-4 lands,
  the seam is measured against a wave-2-local insert stub that is a
  real function (fixed-size slab write), not a mock.
- Pin ratification follows the CTQ-04 pattern
  (`../v26.10.31/49-ctq04-variance-study.md`): spaced repeat runs,
  stdev <= 15% of mean, before any limit is enforced.
- Harness: `crates/avec-pqc/benches/mlkem_ctq.rs`, `harness = false`,
  hand-rolled timing loop, no divan in avec-pqc (zero-dep discipline);
  divan stays in `crates/bench`.

## 5. Deterministic-Seed KAT Mode and the Hedged/Randomized Question

**Pinned decision: the deterministic cores ARE the public API.**

```rust
pub fn keygen(d: &[u8; 32], z: &[u8; 32]) -> (EncapsKey, DecapsKey);
pub fn encaps(ek: &EncapsKey, m: &[u8; 32]) -> ([u8; 32], Ciphertext);
pub fn keygen_rng() -> (EncapsKey, DecapsKey);      // seeds via OS RNG
pub fn encaps_rng(ek: &EncapsKey) -> ([u8; 32], Ciphertext);
```

- Rationale: FIPS 203 specifies the algorithms deterministically —
  the randomized variants are defined as "sample d, z (or m), then
  run the deterministic algorithm". The cores are required for KATs
  and for upstream key derivation per FIPS 203; hiding them behind
  `#[doc(hidden)]` or feature gates would force courts to patch the
  API surface they test — the failure mode falsifier F-08 bans.
- Misuse risk pinned in docs, not hidden: reusing m across encaps
  calls collapses FO security; the deterministic `encaps` doc carries
  that warning verbatim. Production callers use `*_rng`; courts use
  the cores.
- `keygen_rng` / `encaps_rng` draw from the hand-rolled OS-RNG seam
  (t08 substrate role); no external RNG dependency, ever.
- ML-DSA hedged-vs-deterministic signing is a wave-3 question and is
  NOT decided here; this pin covers ML-KEM only.

## 6. Falsifiers and Definition of Done

### 6.1 Falsifiers

| id | falsifiable claim | observation that kills it |
|---|---|---|
| F-01 | KAT court is a real gate | a vector flips, suite still exits 0 |
| F-02 | decaps is infallible by type | any Result/panic path on well-formed ct |
| F-03 | implicit rejection holds | ss_prime == ss on any corrupted vector |
| F-04 | rejection is pseudorandom + pinned | J(z\|\|ct_bad) parity fails vs corpus |
| F-05 | decaps timing is input-independent | \|t\| >= 4.5 any class, post-warmup |
| F-06 | timing court is non-vacuous | branchy mutant passes with \|t\| < 4.5 |
| F-07 | wave-2/wave-4 seam is frozen | wave-4 defines a second dk tuple type |
| F-08 | cores testable as-shipped | a court must patch the API to inject (d,z,m) |
| F-09 | KEM data path is branchless | unexplained hit; gate exits 0 anyway |
| F-10 | zero-alloc holds on rejection path | injected alloc in decaps passes the court |

### 6.2 Definition of done

A wave-2 receipt exists and carries all of:

1. KAT parity receipts per parameter set: command, exit 0, corpus
   sha256s, vector counts (gate).
2. Negative-KAT receipt: corrupted-ct corpus, ss_prime parity vs
   expected J(z||ct_bad), no-error witness (gate).
3. dudect verdict receipt: per-class t-tables, batteries, mutant
   non-vacuity witness, `< 4.5` verdicts.
4. Zero-alloc receipt: 0/0 deltas on all measured paths including the
   rejection path.
5. Branchless manifest entries landed with observed_mnemonics from a
   real audit-profile run; gate exit receipts: 0 on the real tree, 1
   on an injected unexplained hit (non-vacuity).
6. Zero-runtime-external-deps receipt (E=0 audit, method of
   `../v26.10.31/82-dependency-audit.md`) for avec-pqc.
7. P28-SIL URN header on every landed file; no SPDX anywhere.
8. Intermediate-value court: landed or loud registered skip with
   close condition.
9. Standing block in this file updated from UNVERIFIED with receipt
   pointers; index owner has amended the p03 row (integration defect
   from section 0 closed).

Until 1-9 exist, the wave-2 standing is exactly: UNVERIFIED.

## See Also

- [../00-index.md](../00-index.md) — milestone charter and disciplines.
- [../../v26.10.31/gate-branchless-manifest.md](../../v26.10.31/gate-branchless-manifest.md)
  — manifest gate method and receipt this plan extends.
- [../../v26.10.31/27-branchless-kernel-abi.md](../../v26.10.31/27-branchless-kernel-abi.md)
  — M=1 ABI spec; inventory-completeness law.
- [../../v26.10.31/22-adr-register.md](../../v26.10.31/22-adr-register.md)
  — ADR-14 remainder policy; ADR-19 CTQ amendment clause.
- [../../v26.10.31/49-ctq04-variance-study.md](../../v26.10.31/49-ctq04-variance-study.md)
  — CTQ pin ratification pattern (stdev <= 15% of mean).
- [../../v26.10.31/82-dependency-audit.md](../../v26.10.31/82-dependency-audit.md)
  — E=0 external-dependency audit method.
- `/Users/sac/avec/crates/avec-blake3/tests/zero_alloc.rs` — court 6
  idiom.
- `/Users/sac/avec/crates/avec-blake3/tests/official_vectors.rs` —
  corpus provenance-header idiom.
- `/Users/sac/avec/scripts/branchless_manifest.toml` — manifest schema
  this plan extends.
- `/Users/sac/avec/LICENSE-P28-SIL.md` — governing license URN.
