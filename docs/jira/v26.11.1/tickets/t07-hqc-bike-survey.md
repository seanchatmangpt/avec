# t07: Code-Based KEM Survey — HQC, BIKE, Classic McEliece

| | |
|---|---|
| Ticket | t07 (v26.11.1 PQC wave, agent 8 of 20) |
| Type | Research — survey, no code |
| Standing | SURVEY-ONLY (verdict posture in section 4) |
| Flags | `UNVERIFIED-KAT` = claim not executed against real vectors this session;
`UNVERIFIED-NIST` = NIST-process status recalled, not re-verified against csrc.nist.gov |

## 1. Where Each Scheme Stands

| Scheme | Assumption | Round-4 outcome | Selected | Key surface |
|---|---|---|---|---|
| HQC | Quasi-cyclic syndrome decoding (QC-SD) + DFR | Adv | March 2025 | ~2-4 KiB pk, ~4-9 KiB ct |
| BIKE | QC-MDPC (QC-SD variant) | Adv | no | ~1.5 KiB pk, ~1.2 KiB ct |
| Classic McEliece | Goppa-code McEliece (1978) | Adv | no | 261 KiB - 1.3 MiB pk, 96-156 B ct |

HQC key/ct byte ranges in the table are the round-4 spec shapes (UNVERIFIED-KAT — exact
byte counts vary by level; do not cite as measured).

**HQC.** Announced by NIST March 2025 as the fifth PQC algorithm selected for
standardization and the first code-based KEM advanced since the process opened.
The selection is the anchor fact of this survey; everything downstream (draft instrument
number, comment period, final date) is process timing, and the exact status is
UNVERIFIED-NIST. NIST's announced expectation at selection time was a draft standard for
comment roughly 2026 with final publication anticipated 2027 (UNVERIFIED-NIST). Until the
draft instrument exists, there is no frozen parameter set to build against.

**BIKE.** Round-4 candidate, not selected in March 2025. It remains a live research
target (spec and reference code still maintained at bikesuite.org) but has no NIST
standardization lane. Its main residual role in this survey is negative evidence: the
QC-MDPC decoder is the best-studied constant-time surface among the three, and it is
where the timing-attack literature concentrates (see section 2).

**Classic McEliece.** Round-4 candidate, not selected. Goppa codes, no decryption
failures, public keys 261,120 B (mceliece348864) to 1,357,824 B (mceliece8192128) — the
~1 MiB class. Decapsulation is fast and has mature constant-time reference code
(UNVERIFIED-KAT), but the key sizes are disqualifying for any wire protocol avec would
plausibly feed: a ~1 MiB public key is not a columnar substrate's idea of a cell.

## 2. The avec-Relevant Mechanics

### 2.1 The linear core is bitwise AND / XOR / popcount

HQC's entire linear algebra is over GF(2). Encapsulation computes three products of
polynomials in GF(2)[x]/(x^n - 1): `r2*hat(s)`, `r1*hat(h)`, and the encrypted-message
product. Each product is a vector-matrix product where every matrix row is a cyclic shift
of one base vector. Expanding one output word from one input word:

```text
acc = 0
for j in 0..words:
    acc ^= parity(popcount(a[i] & b_shifted[j]))   # AND -> popcount -> XOR-fold
out[i] = acc
```

The per-word op is AND, popcount, XOR-fold. That is, compositionally, avec's
`avec_arrow::bitmap::mask_words` (AND) followed by `avec_arrow::bitmap::count_valid`
(popcount) with the parity bit folded into an accumulator lane. The branchless shape is
already in the branch register: `bitmap::mask_words @0xf20` compiles body-branch-free
(docs/BRANCH_REGISTER.md R2, latch 2 / body 0 / dispatch 3, LLVM auto-vectorized 4x16b
AND). `silicon::mask_words_neon` (crates/avec-arrow/src/silicon.rs:41) is the same op
under NEON.

Two multipliers on top of the plain AND/popcount loop:

1. **Carry-less multiply.** GF(2) cyclic convolution is CLMUL-expressible:
   `PCLMULQDQ` (x86) or `PMULL/PMULL2` (aarch64) multiply two 64-bit polynomials in one
   instruction; the convolution is a sequence of multiply-XOR-accumulate steps at
   O(n/64) instructions per product. This is the known-fast shape for HQC's core
   (UNVERIFIED-KAT as an avec claim — avec has no CLMUL kernel today, so this is a
   design note, not a measured result).
2. **Bit-slicing.** Process 64 (scalar u64) or 128 (NEON v128) independent instances
   of the AND/popcount fold per instruction word. For a KEM this parallelizes across
   batch encapsulation, not across one decapsulation (one decaps is one syndrome, not
   64). The avec fit is batch-shaped: many independent GF(2) dot products per column
   batch.

Weighted-vector generation (the error vectors of fixed Hamming weight) is constant-time
rejection sampling in the reference implementation (UNVERIFIED-KAT); it is control flow
of the class avec already itemizes (latch/dispatch), not secret-branch flow, provided
the rejection count is masked.

### 2.2 Decoder structure: threshold and Gao

Decryption recovers the message by decoding an error vector: compute the syndrome-like
word `v - hat(s)*u = m*G + e`, run the decoder to strip `e`, then invert the concatenated
code. The decoder is a concatenated code: a BCH inner code and a Reed-Solomon outer code
(exact per-level code parameters UNVERIFIED-KAT — structure is the load-bearing part
here, not the numbers).

- **BCH inner.** Berlekamp-Massey to get the error-locator polynomial, Chien search for
  roots, Forney for values. Standard three-stage pipeline; each stage is small-field
  arithmetic over GF(2^m).
- **RS outer via Gao's decoder.** Gao decoding avoids syndrome computation: from the
  received polynomial, form the interpolant whose roots give g0*g1, run half-extended
  Euclid to split the remainder, error evaluator falls out of the remainder split. It is
  the decoder HQC's spec references for the RS layer (UNVERIFIED-gao-specifics, fold
  under UNVERIFIED-KAT).
- **BIKE's threshold decoder, for contrast.** BIKE's BGF (Black-Gray-Flip) decoder is
  threshold bit-flipping: per pass, compute syndrome weight, derive a per-pass threshold
  from syndrome weight and column weight, flip unsatisfied-parity bits above threshold,
  then black/gray window passes refine. Every quantity that drives the threshold is
  secret-derived. That is exactly why it is the canonical constant-time hard case.

### 2.3 The constant-time decoder difficulty

DFR is public. HQC's claimed decryption failure rates are at or below 2^-lambda per
security level, justified by asymptotic/heuristic analysis of the BCH/RS concatenated
code (UNVERIFIED-KAT — the tightness of these analyses is actively debated in the
literature). Public DFR answers "how often does decoding fail"; it says nothing about
what the decoder *does* when it succeeds. The hard part is:

- Berlekamp-Massey iteration count depends on the number of errors — a secret-derived
  quantity. CT versions re-run to a fixed iteration count and mask per-iteration state.
- Chien search hit counts and Forney paths depend on error positions.
- Bit-flipping pass counts and per-pass flip counts (BIKE) are secret-derived.
- Syndrome-weight early aborts and failure classification are secret-derived branches
  unless explicitly masked.

HQC's reference implementation claims constant-time decoder paths (UNVERIFIED-KAT), and
the timing-attack literature on QC-MDPC decoders (reaction/DFR-learning attacks,
decoder-timing message recovery, fault probes on iteration counts) is the evidence that
this surface bites. The avec bar is `docs/DATA_INDEPENDENCE_AUDIT.md`: no kernel in the
repo supports a CT claim today; access-pattern constancy is the current claim class.
Under that vocabulary, the GF(2) vec-mat core is CONTENT-INDEPENDENT (fixed strides,
addresses fixed by length); the decoders are the part that must be rewritten into the
CONTENT-INDEPENDENT class before any CT claim. The honest split: **the avec-friendly
part is the linear core; the CT liability is the decoder**, and a survey ticket does not
close that gap.

## 3. Why Diversification Matters

ML-KEM (FIPS 203, final August 2024) is a structured-lattice scheme (Module-LWE). So are
ML-DSA (FIPS 204) and FN-DSA/Falcon (FIPS 206 draft pending). Three of the four first
NIST selections share one hardness family. A structural break of the SIKE class —
SIKE fell to Castryck-Decru 2022 after a decade of assumed safety, Rainbow fell the same
year (Beullens) — would hit every lattice deployment at once. The hedge is not "lattices
are weak"; it is that a second basket should have an independent failure mode.

Code-based is the oldest standing assumption in the basket: McEliece 1978, ~47 years
unbroken on the Goppa instantiation. HQC's QC-SD assumption is related-but-distinct
(quasi-cyclic syndrome decoding is not Goppa decoding; it has its own, more active,
cryptanalytic literature). Classic McEliece is the maximally conservative member of the
family — no decryption failures, oldest assumption — penalized only (but decisively) on
key size.

Honest caveats: diversification is insurance priced in key bytes and decoder risk.
HQC's DFR claims rest on heuristic analyses (UNVERIFIED-KAT), and a KEM with a nonzero
failure rate is a different risk object than one with none. The hedge argument does not
need HQC to be stronger than ML-KEM; it needs HQC to fail differently.

## 4. The avec Verdict Posture

**SURVEY-ONLY through waves 1-5.** No kernel, no crate, no KEM plumbing lands from this
ticket. Grounds:

1. HQC's standard is not final; no frozen parameter set exists to build against. Any
   code written now targets a moving spec.
2. The decoder CT gap is not closed at avec's evidentiary bar
   (docs/DATA_INDEPENDENCE_AUDIT.md). Shipping the linear core without the decoder CT
   story would mint exactly the class of overclaim the audit register exists to prevent.
3. avec is a data-plane substrate, not a KEM. If HQC lands later, avec's scope is the
   GF(2) primitives (AND/popcount folds, CLMUL convolution, fixed-weight sampling
   helpers), not encaps/decaps.

**Revisit trigger:** publication of the HQC draft FIPS (instrument number and date
UNVERIFIED-NIST). On that event: re-freeze parameters from the draft, re-run this
survey's section 2.3 against the draft's decoder requirements, and re-decide.

**The honest avec fit**, if and when it lands: the bitwise GF(2) vector ops. AND/popcount
parity folds over 64/128-bit words, CLMUL-based cyclic convolution, batch bit-sliced
encaps-side products. These are `mask_words`/`count_valid`-shaped kernels — the exact
classes already in docs/BRANCH_REGISTER.md — and nothing else. Decoders are a separate
ticket with a separate CT evidence plan, and Classic McEliece is out on key size, and
BIKE is out on decoder-CT history absent a NIST lane.

## 5. Open Items

1. **FIPS draft instrument.** Number, comment period, final timing — UNVERIFIED-NIST.
   The revisit trigger in section 4 keys on the draft's existence, not this survey's
   recollection of it.
2. **Parameter freeze risk.** Round-4 HQC parameters (n = 17669/35851/57637 for levels
   128/192/256) may drift in the draft (UNVERIFIED-KAT). No avec code should hard-code
   them.
3. **DFR claim tightness.** The 2^-lambda DFR claims are asymptotic; the debate is live
   (UNVERIFIED-KAT). If a later wave builds a decaps path, DFR epistemics become a
   security parameter, not a footnote.
4. **CLMUL/PMULL convolution probe.** One narrow probe: GF(2) cyclic conv via PMULL
   through the branch-audit CC=1 pipeline (`make branch-audit`), measured against a
   scalar AND/popcount baseline. Design note in section 2.1 becomes a claim only then.
5. **Decoder CT evidence plan.** What would satisfy DATA_INDEPENDENCE_AUDIT.md
   classification for a BCH/Gao decoder — likely fixed-iteration BM, masked Chien,
   syndrome-weight-independent control. Unscoped; needs its own ticket if the draft
   drops.
6. **KAT interop source.** If a later wave runs known-answer tests, liboqs is the
   pragmatic vector source (UNVERIFIED-KAT); with a final HQC standard, official KATs
   should come from the standard's own test vectors.

## Sources

- NIST PQC standardization announcements, csrc.nist.gov (March 2025 selection of HQC) —
  status details UNVERIFIED-NIST, not re-fetched this session.
- HQC round-4 specification, hqc.org.
- BIKE specification and reference code, bikesuite.org.
- Classic McEliece round-4 package, classic.mceliece.org.
- Repo grounding: docs/BRANCH_REGISTER.md, docs/DATA_INDEPENDENCE_AUDIT.md,
  crates/avec-arrow/src/bitmap.rs (mask_words, count_valid),
  crates/avec-arrow/src/silicon.rs (mask_words_neon).
