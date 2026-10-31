# t01-ml-kem-params

ML-KEM parameter-set ground truth for the v26.11.1 PQC wave. Read-only research
ticket: no code, no git, one file. Every numeric claim carries its source —
a FIPS 203 cite tag pinned to the extraction, a local arithmetic receipt, or
an UNVERIFIED-KAT flag. Nothing here is a receipted engineering fact; per the
milestone index (ADR-19 clause), parameter claims enter as KAT-validation
inputs only.

- Standing: UNVERIFIED (all numeric claims pending KAT receipts, see
  [Standing](#standing)).
- Owner: wave v26.11.1 PQC, agent 2 of 20.
- Scope: parameter ground truth, avec geometry mapping, open items. KAT
  acquisition mechanics belong to t05/t07; this ticket only names targets.

## Provenance

The primary source was fetched and text-extracted on 2026-10-31; the recovery
session re-executed the whole chain the same day, so the cites below are
pinned to the document, not to memory.

- Document: FIPS 203, Module-Lattice-Based Key-Encapsulation Mechanism
  Standard (final, 2024-08-13).
- URL: https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.203.pdf
- SHA-256:
  `fe1f12f32a7e44ec9fdebbf400cda843a40b506dee676725234dc6f7923b6cac`
- Session-2 replay: fresh `curl` of the URL above to `/tmp/fips203-fresh.pdf`
  hashed to the same SHA-256 — byte-identical document.
- Extraction: `pdftotext` to `/tmp/fips203.txt` (3601 lines, exit 0), then a
  second extraction to `/tmp/fips203-fresh.txt` (3601 lines); `diff` reports
  the two byte-identical, so every cite line below is stable.
- Cite tags:
  - `[T1]` = Table 1, p. 15 (decapsulation failure rates)
  - `[T2]` = Table 2, Section 8, p. 39 (approved parameter sets)
  - `[T3]` = Table 3, Section 8, p. 39 (key/ciphertext byte sizes)
  - `[T4]` = Table 4, Appendix B, p. 46 (SampleNTT loop bounds)
  - `[Alg n]` = Algorithm n (numbering read from the extracted document)
  - `[Sec 4.3]` = NTT discussion (q, zeta, root-of-unity structure)
  - `[C.1]`/`[C.2]` = Appendix C (differences vs Kyber r3 / vs the IPD)
- UNVERIFIED-INDEX marks an internal index not pinned during extraction; the
  formula or name itself is from the extraction. Only one item remains under
  this flag: H's equation number (Section 6). Index slips affect no numeric
  claim.
- Page numbers follow the pdftotext footer model (digit between page bodies).

## 1. The Three Parameter Sets

From [T2] and [T3], extracted from the fetched document:

| Parameter | ML-KEM-512 | ML-KEM-768 | ML-KEM-1024 | Source |
|---|---|---|---|---|
| n | 256 | 256 | 256 | [T2] |
| q | 3329 | 3329 | 3329 | [T2] |
| k | 2 | 3 | 4 | [T2] |
| eta1 | 3 | 2 | 2 | [T2] |
| eta2 | 2 | 2 | 2 | [T2] |
| du | 10 | 10 | 11 | [T2] |
| dv | 4 | 4 | 5 | [T2] |
| required RBG strength (bits) | 128 | 192 | 256 | [T2] |
| ek (bytes) | 800 | 1184 | 1568 | [T3] |
| dk (bytes) | 1632 | 2400 | 3168 | [T3] |
| ct (bytes) | 768 | 1088 | 1568 | [T3] |
| ss (bytes) | 32 | 32 | 32 | [T3], [C.1] |

Arithmetic re-derivation (cross-check receipt, all rows pass; python3 run in
the recovery session):

- ek = 384k + 32: 800 / 1184 / 1568 — matches [T3] for all three sets.
- dk = 768k + 96: 1632 / 2400 / 3168 — matches [T3] for all three sets.
  Layout: dk = dkPKE || ekPKE || H(ekPKE) || z = 384k + 384k + 32 + 32, with
  dkPKE ∈ B^384k read from the extraction (K-PKE.KeyGen output type).
- ct = 32*(du*k + dv): 32*(10*2+4) = 768; 32*(10*3+4) = 1088;
  32*(11*4+5) = 1568 — matches [T3] for all three sets.
- ss fixed at 256 bits for all sets [C.1]; Kyber r3 allowed variable length.
- Security strength: "ML-KEM-512 is claimed to be in security category 1,
  ML-KEM-768 ... category 3, and ML-KEM-1024 ... category 5" (extraction,
  Section 8); SP 800-57 framing per the same section.
- Planning default: "NIST recommends using ML-KEM-768 as the default
  parameter set" (extraction, Section 8) — input to p02's default-set choice.

## 2. Corrections Ledger (Dispatch Prompt vs Primary Source)

The dispatch prompt's parameter block disagrees with the fetched final FIPS
203 on four points. Per the index discipline (unverified parameter claims are
KAT-validation inputs, never receipted facts), the primary source wins every
row, and each row is independently receipted. The dispatch's dk, ct and ss
values matched [T3] as dispatched.

| # | Dispatch claim | Receipted value | Receipt |
|---|---|---|---|
| 1 | dv(768) = 8 | dv(768) = 4 | [T2]; ct(768) = 1088 = 32*(10*3 + 4) |
| 2 | ek(768) = 1188 | ek(768) = 1184 | [T3]; falsifier in note below |
| 3 | eta1(1024) = 3 | eta1(1024) = 2 | [T2] column read; note below |
| 4 | "256th root of unity exists" (unnamed) | zeta = 17 is that root | [Sec 4.3]; check below |

- Row 2 falsifier (independent of the PDF): 384*3 + 32 = 1184, and
  1188 - 32 = 1156 is not a multiple of 384, so 1188 B is not a possible ek
  size under the ek = 384k + 32 law.
- Row 3 receipts: eta1 has no size implication, so there is no arithmetic
  cross-check. The receipts are the [T2] column read — twice, from two
  byte-identical extractions of the SHA-verified document — and a live fetch
  of the CRYSTALS-Kyber reference params.h (KYBER_ETA1 = 2 in the
  KYBER_K == 4 block), which agrees with the standard.
- Row 4 receipt detail: the dispatch row "q = 3329 = 2^8 * 13 + 1, 256th root
  of unity exists" names no root; a plausible misreading takes 2 for the
  root. Hand check: 2^256 = 1492 (mod 3329), so 2 is not a 256th root of
  unity; the standard names zeta = 17.

## 3. Modulus and NTT Arithmetic

- Ring: Rq = Zq[x]/(x^256 + 1), negacyclic; q = 3329, bit length 12
  (extraction: "Recall that q = 3329, and that the bit length of q is 12").
- [Sec 4.3], quoting the extracted text: "q is the prime
  3329 = 2^8 * 13 + 1, and n = 256. There are 128 primitive 256-th roots of
  unity and no primitive 512-th roots of unity in Zq. Note that zeta = 17
  is a primitive 256-th root of unity modulo q. Thus, zeta^128 = -1."
- NTT-friendly status: 3329 is NTT-friendly because 2^8 divides q - 1, so a
  full 256-point NTT exists. q - 1 = 2^8 * 13 carries an odd factor, so 3329
  is not of Fermat form 2^m + 1; there is no 2^k +- 1 reduction shortcut, and
  Barrett/Montgomery-class reductions are the implementation space.
- Correction row 4, hand check that 2 is not a 256th root of unity
  (python-verified chain): 2^8 = 256, 2^16 = 2285, 2^32 = 1353,
  2^64 = 2988, 2^128 = 3095, 2^256 = 1492 (all mod 3329). Since
  1492 != 1, 2 is not a 256th root.
- Corroborating hand chain for zeta = 17 (the standard's own [Sec 4.3]
  statement is the load-bearing receipt; chain python-verified):
  17^2 = 289; 17^4 = 296; 17^8 = 1062; 17^16 = 2642; 17^32 = 2580;
  17^64 = 1729; 17^128 = 3328 = -1 (mod 3329). Hence ord(17) = 256.
- Twiddle storage: NTT uses zeta^BitRev7(i) for i = 1..127 and MultiplyNTTs
  uses zeta^(2*BitRev7(i)+1) [Sec 4.3, Alg 9, Alg 11]; precomputed values
  are tabulated in Appendix A of the standard.

### Barrett/Montgomery

- Grep receipt: zero occurrences of "Montgomery" or "Barrett" in the
  extracted FIPS 203 text. The standard mandates results in Zq; the
  reduction technique is implementation-class, not spec-class.
- Montgomery requirements: q odd; R = 2^16 > q works; R mod q = 2285;
  q^(-1) mod 2^16 = 62209 (python receipt: 3329 * 62209 == 1 mod 2^16) and
  -q^(-1) mod 2^16 = 3327. The attribution of 62209 to the Kyber reference
  code's QINV constant is UNVERIFIED-KAT — it is not a FIPS number.
- Barrett requirements: v = ((1<<26) + q/2)/q = 20159 (rounded; floor is
  20158) — UNVERIFIED-KAT as a reference-code constant (formula from the
  Kyber reference barrett_reduce pattern); requires intermediate products to
  stay inside the lane width. UNVERIFIED-KAT until exercised on real vectors.
- Headroom: 3329 < 2^12 = 4096, so coefficients fit 12 bits; u16 lanes hold
  Zq values with 4 spare bits for lazy reduction; u32 lanes hold
  pre-reduction accumulators for late-reduction strategies.

## 4. Rejection Sampling and Dense Coefficient Arenas

- SampleNTT [Alg 7]: the XOF (SHAKE128 wrapper, Section 4) produces 12-bit
  candidates; accept iff j < q. Acceptance ratio 3329/4096 = 0.812744140625;
  expected draws per accepted coefficient 4096/3329 ≈ 1.230.
- [T4]: if the loop is bounded, the limit shall not be lower than 280
  iterations; exceedance probability at the limit is 2^-261. The standard
  says implementations should not bound the loop; if bounded and exceeded,
  the algorithm shall destroy all intermediate results and any returned
  value shall be identical for every such execution (deterministic failure).
- SamplePolyCBD_eta [Alg 8]: consumes PRF_eta(s, b) = SHAKE256(s || b,
  8*64*eta) (extraction eq. 4.3) — 64*eta bytes = 512*eta bits = 2 eta-bits
  per coefficient across 256 coefficients.
- Why dense arenas map: SampleNTT always emits exactly n = 256 accepted
  coefficients regardless of how many rejections occur; rejection consumes
  more XOF input, never changes output length. Output shape is static, so
  the sampler writes straight into a fixed-capacity dense arena — no growth
  path, no per-coefficient indirection. This is the Quad64 lesson applied
  to sampling: static shape, dense layout, cache-line arithmetic.
- M=1 note: the accept predicate (j < q) is a branchless predicate; rejection
  under branchless discipline becomes mask compaction. The interaction of
  mask compaction with the [T4] iteration-count semantics is an open item
  for t06/t12, not solved here.

## 5. Wire Sizes and the Quad64 Columnar Geometry

- avec ground truth: `Quad64` in crates/avec-arrow/src/layout.rs:36 —
  64 bytes, 64-byte aligned (`#[repr(C, align(64))]`), compile-time
  size/align/offset asserts (layout.rs:59-69), inline_digest `[u8; 32]` at
  offset 0x20 (layout.rs:54, assert at :68). Spec:
  docs/jira/v26.10.31/23-64b-quad-spec.md. One record = one cache line, no
  split-line records, no implicit padding.
- Cache-line arithmetic (line = 64 B), from the [T3] sizes (all counts
  python-verified):
  - ek: 800 = 12.5 lines; 1184 = 18.5 lines; 1568 = 24.5 lines. Since
    ek = 384k + 32 and 384 = 6*64, ek mod 64 = 32 for every set.
  - dk: 1632 = 25.5; 2400 = 37.5; 3168 = 49.5 lines. Since dk = 768k + 96,
    768 = 12*64, 96 = 1.5*64, dk mod 64 = 32 — always half-tailed.
  - ct: 768 = 12 lines exactly; 1088 = 17 lines exactly; 1568 = 24.5 lines.
  - ss: 32 B — exactly half a line.
- Structural law: every ML-KEM wire object is congruent to 0 or 32 (mod 64)
  because every component (poly encode 384 B, digest 32 B, seed 32 B) is a
  32-byte multiple. Half-line tails are universal on ek/dk; ct is line-exact
  at 512/768 and half-tailed at 1024.
- The dispatch's true statement, corrected: "768 B ct = 12 cache lines
  exactly" holds ([T3]); the "1188 B ek" it contrasts is not a real size —
  1184 B = 18.5 lines. The arena question is unchanged: exact-size wire
  records with explicit half-row tails vs pad-to-line records. Seven of
  nine wire objects land on half-line tails (all ek, all dk, ct-1024), so
  exact-size + explicit tails is the lower-overhead default; pad-to-line
  buys alignment-free loads. This is a p02 decision, not a t01 conclusion.
- 32-byte ss matches the 32-byte `inline_digest` slot exactly (size match
  only). The slot is a BLAKE3 receipt today with its own validity law ("an
  unset digest is invalid and rejected at decode", layout.rs:52-53); an ss
  slot has different validity semantics, so reuse is a semantic decision
  for p02, flagged not decided.
- Working (non-wire) formats are line-exact, unlike the wire: dense u16
  poly = 256 x 2 B = 512 B = 8 lines; dense u32 poly = 256 x 4 B =
  1024 B = 16 lines; ByteEncode12 output = 384 B = 6 lines. Existing arena
  substrate: crates/avec-arrow/src/mmu_arena.rs.
- Negative receipt (recovery session, true exit code): `grep -rln` over
  crates/ for ml_kem / ml-kem / fips203 / fips-203 / MlKem exits 1 with zero
  files — the PQC code surface is fresh (2026-10-31).

## 6. KAT / Conformance Targets

- Conformance surface (extraction, Appendix C intro): "a conforming
  implementation need only match the input-output behavior of these three
  algorithms (i.e., KeyGen, Encaps, Decaps)". Byte-parity at those three
  functions is the gate.
- Algorithm inventory (numbering read from the extraction's algorithm
  index; Alg 7 SampleNTT and Alg 16 KeyGen_internal independently
  spot-checked this session): Alg 5 ByteEncode_d; Alg 6 ByteDecode_d;
  Alg 7 SampleNTT; Alg 8 SamplePolyCBD; Alg 9 NTT; Alg 10 NTT^-1;
  Alg 11 MultiplyNTTs; Alg 12 BaseCaseMultiply; Alg 13 K-PKE.KeyGen;
  Alg 14 K-PKE.Encrypt; Alg 15 K-PKE.Decrypt; Alg 16 KeyGen_internal;
  Alg 17 Encaps_internal; Alg 18 Decaps_internal; Alg 19 KeyGen;
  Alg 20 Encaps; Alg 21 Decaps.
- Hash/PRF/XOF (Section 4), equation numbers pinned this session:
  H = SHA3-256 (display equation; its number is UNVERIFIED-INDEX — the
  extraction shows it untagged immediately before eq. 4.4);
  J = SHAKE256(s, 8*32) (eq. 4.4);
  G = SHA3-512 split as a || b, so K = a and r = b (eq. 4.5);
  PRF_eta = SHAKE256(s || b, 8*64*eta) (eq. 4.3);
  XOF = SHAKE128 wrapper over the SP 800-185 incremental API.
- Decapsulation failure rates [T1], exact table values: 2^-138.8 (512),
  2^-164.8 (768), 2^-174.8 (1024), conditioned on no RBG failures and on
  hash/XOF random-function heuristics — input to W1 negative-court design;
  acquisition mechanics hand off to t07.
- Randomized/deterministic split: Alg 16 KeyGen_internal(d, z) is
  deterministic; Alg 19 KeyGen supplies fresh random d and z. [C.2]:
  "domain separation was added to K-PKE.KeyGen" (cross-level seed-reuse
  misuse), and the IPD's swapped A-matrix indices were "changed back in the
  final version of ML-KEM to match CRYSTALS-KYBER" (both verbatim in the
  extraction).
- Encaps input-check requirement [C.1], verbatim: "ML-KEM.Encaps requires
  that the byte array containing the encapsulation key correctly decodes to
  an array of integers modulo q without any modular reductions" — a
  modulus-checking court is spec-mandated, not gold-plating.
- Other [C.1] deltas from Kyber r3, all verbatim in the extraction: ss fixed
  at 256 bits; ML-KEM.Encaps "no longer includes a hash of the ciphertext in
  the derivation of the shared secret" (Decaps adjusted to match); the
  IPD-era m <- H(m) pre-hash of the Encaps randomness was dropped ("this
  step is unnecessary and is not performed in ML-KEM").
- KAT source class: NIST ACVP ML-KEM vectors; acquisition and provenance
  chain belong to t05/t07. Every numeric claim here promotes to fact only
  via a KAT byte-parity receipt.

## 7. Open Items for the Plan Lane (p02)

1. Filename contract collision: the milestone index assigns t01 =
   `t01-fips-203-ml-kem-spec.md` (agent 1's contract), while this file was
   landed at the dispatched name `t01-ml-kem-params.md`. The same divergence
   pattern is visible on disk for at least t04, t07, t09, t13, t14, p03 and
   p06 (each landed under a name other than its index-contract slot).
   Agent 1/p02 reconciles: rename into the contracted slots or amend the
   index rows.
2. Arena layout: exact-size wire records with explicit half-row tails vs
   pad-to-line records (Section 5). Decision needed from p02.
3. inline_digest slot reuse semantics for ss (Section 5): size fit is
   exact; validity law is not. Decide slot semantics or a dedicated slot.
4. SampleNTT loop policy under M=1: unbounded (spec-preferred) vs the [T4]
   bound (280 iterations, 2^-261, deterministic failure + destroy
   intermediates). Mask-compaction interaction is a t06/t12 question.
5. Reduction strategy: i16 lanes (Barrett-class) vs i32/u32 lanes
   (Montgomery-class, lazy reduction) over dense u16/u32 arenas. Reference
   constants (QINV = 62209, Barrett v = 20159) are UNVERIFIED-KAT.
6. eta1(1024) = 2 is double-receipted (canonical text twice + Kyber ref
   params.h); an optional ACVP cross-check during t07 acquisition closes it
   permanently. The corrections ledger reopens only if a receipt disagrees.
7. Modulus-checking court for the Encaps input validation [C.1]:
   spec-mandated check needs a negative-court design in W1/t13 scope.
8. KeyGen seed policy: randomized Alg 19 surface with a KeyGen_internal(d, z)
   test hook; the [C.2] domain-separation behavior is KAT-relevant.
9. Wave-1 court corpus: [T1] failure-rate bounds plus the [T4] loop-limit
   law (destroy intermediates, deterministic failure value) as court inputs.

## Standing

- This ticket: UNVERIFIED end to end. It is a Phase-0 exploration receipt;
  it receipts the exploration, not the engineering (index, Standing).
- Every numeric claim is a KAT-validation input until a KAT byte-parity
  receipt (t07 harness, ACVP vectors) lands against it. UNVERIFIED-KAT items
  are additionally not FIPS-pinned: they are reference-implementation-class
  constants pending exercise.
- Document identity is pinned by the SHA-256 above, re-verified by a fresh
  download this session; the cite lines are pinned by a byte-identical
  double extraction. If t07's independent copy disagrees, the corrections
  ledger reopens.
- Process note (recovery session, 2026-10-31): the first on-disk draft of
  this file predated a mid-session interruption. This session re-executed
  the full verification chain — fresh NIST download (SHA match),
  byte-identical second extraction, python re-derivation of every arithmetic
  claim, live Kyber params.h cross-check. It fixed three draft slips
  (acceptance-ratio decimal, a half-line-tail count, Fermat-form phrasing)
  and refuted the hypothesis that the draft's eta1(1024) = 2 was a corrupted
  table read: the canonical text says 2. Only this on-disk version counts.

## Sources

- FIPS 203 final PDF: URL + SHA-256 in Provenance; extractions at
  `/tmp/fips203.txt` and `/tmp/fips203-fresh.txt` (working scratch, outside
  the repo; fresh PDF at `/tmp/fips203-fresh.pdf`).
- Kyber reference params.h (corroboration only, not governor):
  https://raw.githubusercontent.com/pq-crystals/kyber/master/ref/params.h
- avec geometry: crates/avec-arrow/src/layout.rs (Quad64, inline_digest,
  layout-law asserts); crates/avec-arrow/src/mmu_arena.rs (arena
  substrate); docs/jira/v26.10.31/23-64b-quad-spec.md (quad spec).
- Milestone: docs/jira/v26.11.1/00-index.md (filename contract, standing
  discipline, ADR-19 clause).
- Companion tickets: t05/t07 (KAT acquisition + provenance), t06 (M=1
  arithmetic), t12 (CT audit), t13 (court corpus), p02 (wave-1 plan).

## See Also

- docs/jira/v26.11.1/00-index.md
- docs/jira/v26.10.31/23-64b-quad-spec.md
- docs/jira/v26.10.31/22-adr-register.md (ADR-14, ADR-19)
