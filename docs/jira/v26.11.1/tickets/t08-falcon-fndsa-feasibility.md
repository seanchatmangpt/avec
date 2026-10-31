# t08: Falcon / FN-DSA Feasibility — the DEFER Evidence

| | |
|---|---|
| Ticket | t08 (v26.11.1 PQC wave, agent 9 of 20) |
| Type | Research — feasibility evidence input to the Go/No-Go adjudication; no code |
| Standing | RESEARCH-ONLY; verdict posture is DEFER (section 4) |
| Flags | `UNVERIFIED-NIST` = NIST-process status recalled, not re-verified against csrc.nist.gov;
`UNVERIFIED-SPEC` = Falcon/FN-DSA spec parameter recalled from the round-3 submission,
not re-fetched; `UNVERIFIED-EXTERNAL` = published attack/benchmark recalled, never
re-measured here |

Slot note: `00-index.md`'s Phase-0 table lists t08 as
`t08-zero-dep-crypto-substrate.md`, and its t04 row hooks "Falcon FP FFT"
while the landed t04 is ML-DSA rejection mechanics (t04's own slot note
records the same divergence pattern, as do t07/t14). Meanwhile p01 cites a
Falcon feasibility ticket at `t04-falcon-fn-dsa-feasibility.md` — a file
that does not exist on disk (verified 2026-10-31). This ticket lands the
missing feasibility record under its dispatched filename; index amendments
(t08 slot name, t04 hook, the p01 link) belong to the index owner
(agent 1 of 20).

## 1. The Scheme and Its Two Algebras

Falcon (Fouque et al.) was selected by NIST in July 2022 for
standardization as FN-DSA alongside ML-DSA and SLH-DSA
(UNVERIFIED-NIST). It is an NTRU-lattice signature: the public key is
`h = g * f^-1 (mod q)` in `R_q = Z_q[x]/(x^n + 1)`, with secret short
vectors `(f, g)` and `(F, G)` satisfying the NTRU equation
`f*G - g*F = q`. Sizes (UNVERIFIED-SPEC):

| Parameter set | n | pk | avg sig | Note |
|---|---|---|---|---|
| Falcon-512 | 512 | 897 B | ~666 B | smallest signature of the NIST selections |
| Falcon-1024 | 1024 | 1793 B | ~1280 B | NIST level 5 |

The ~666 B average signature is the scheme's headline: smallest of all
NIST PQC signature selections, and roughly 3.6x smaller than ML-DSA-44's
2420 B. The dispatch's "smallest PQ" claim holds for the NIST selection
set; schemes outside the process are out of scope here.

q = 12289 = 3 * 2^12 + 1, so q - 1 = 2^12 * 3 and v2(q-1) = 12. Both
parameter sets admit full negacyclic-NTT splits (needs 2n | q-1:
1024 | 12288 and 2048 | 12288 both hold), so `x^n + 1` splits completely
and the `Z_q`-side arithmetic is an 8-9 stage butterfly network of the
same class as the ML-KEM/ML-DSA NTTs (t04 section 4.2 contrast method).

The scheme splits into two algebras, and the split is the whole
feasibility story:

- **The `Z_q` algebra (avec-friendly).** Verification is: hash the
  message to `c` (SHAKE256-class XOF, domain-separation per spec),
  compute `s1 = c - h*s2 (mod q)`, then check the norm of `(s1, s2)`
  against a fixed bound derived from sigma and n (exact constant
  UNVERIFIED-SPEC). Every op is fixed-stride modular arithmetic over
  `Z_q[x]/(x^n + 1)` — the NTT/butterfly/Montgomery class the wave plan
  already builds. Everything the verifier touches is public.
- **The real algebra (the hard part).** Signing must sample from
  discrete Gaussians over the reals, in the FFT domain, down an LDL
  tree of the Gram matrix of the secret lattice basis. This path runs
  on floating-point (the reference `fpr` is double-precision,
  UNVERIFIED-SPEC) with secret-dependent data in the FP pipeline.

Verification has no secret data and no sampling: it is the avec-shaped
half. Signing carries every CT liability the scheme has.

## 2. The Gaussian Sampler Is the Hard Part

### 2.1 What fast-Fourier sampling does

Falcon signs by fast-Fourier sampling (Ducas-Prest): build the LDL^T
decomposition of the Gram matrix of the secret basis, recursively, as a
binary tree whose per-node sigmas shrink with depth (exact per-level
sigma tables UNVERIFIED-SPEC). At each node, sample one coefficient of
`z2` from `D_{Z, sigma_i, c_i}` — a discrete Gaussian centered at a
real-valued center over `Z`, evaluated in the FFT (real/imaginary)
domain. The output is the secret-dependent sample stream plus the
floating-point state that produced it.

### 2.2 Sampler families vs the CT bar

Every published sampler family fails the avec bar today:

- **CDT samplers (Peikert-class).** Cumulative distribution tables are
  probed at secret-derived indices — the classic cache-timing leak
  class. Fixing it means padding to a fixed worst-case access pattern
  over a table whose size grows with target precision.
  (UNVERIFIED-EXTERNAL on the cited families.)
- **Lyu12 rejection sampling (uniform-in-disk).** Accept if
  `z^2/(2 sigma^2) - ln(u) ln...` — requires FP `exp`/`ln` plus a loop
  whose acceptance is secret-correlated; per-iteration cost is
  secret-dependent, which is the TL class by construction. This is the
  family the Falcon reference sampler approximates.
  (UNVERIFIED-EXTERNAL.)
- **Fast-Fourier sampling (Falcon's choice).** Correctness is defined
  by double-precision FP semantics; the recursion depth is fixed but
  the per-node real-algebra state is secret-derived. The sampler is
  only as CT as the FP pipeline underneath it.
- **Integer samplers (DDY-class, integral lattice Gaussian sampling).**
  Removes FP but is research-grade, not the standard's normative
  semantics, and not a conformance target. (UNVERIFIED-EXTERNAL.)

CT Gaussian sampling is widely characterized in the literature as the
hardest constant-time problem in PQC (UNVERIFIED-EXTERNAL), and the
timing-attack record backs the characterization: working attacks exist
against non-CT Falcon samplers (the Leaky Falcon line, 2020-2021,
UNVERIFIED-EXTERNAL). The public record attributes FIPS 206's own delay
to exactly this implementation difficulty (UNVERIFIED-NIST).

### 2.3 The FP hazard class

Even with a fixed-trip sampler loop, the FP pipeline itself is the
hazard: denormal penalties on x86, FP flag-driven paths, x87 80-bit vs
SSE double vs NEON divergence, and FMA contraction differences mean
byte-level output divergence across platforms unless the standard pins
the semantics. Transcendental approximations (the `exp` inside the
rejection sampler) are the worst case: platform-libm `exp` is not
byte-stable across platforms, so KAT byte-parity — the milestone's only
gate — is not platform-robust against a libm-dependent sampler. A
conforming CT implementation needs the FP (or fixed-point) semantics
pinned by the standard itself, not by the host libm.

### 2.4 The zero-deps collision

Discipline 2 is E=0 runtime externals (00-index.md). The reference
Falcon code leans on double-precision FP behavior with libm `exp` in
the sampler. Pulling an FP library (or linking libm's transcendentals)
violates E=0; hand-rolling a CT FP/fixed-point engine that also hits
the standard's normative FFT/Gaussian semantics is a research program,
not a port. Published fixed-point Falcon work exists (PQShield-class,
2026, UNVERIFIED-EXTERNAL) as research but not as the standard's
normative semantics nor a validated conformance target.

### 2.5 In avec vocabulary

Per `docs/DATA_INDEPENDENCE_AUDIT.md` classes and the M=1 branchless
discipline (CC=1 kernel bodies, `make branch-audit`):

- **Verify path: CI by construction.** Fixed strides, public data, no
  sampling; butterflies join the `mask_words`/`count_valid`-shaped
  manifest row class already registered.
- **Sign path: TL-class as referenced.** Secret-derived acceptance
  loops plus secret-dependent FP state. Under M=1, butterflies are
  fine; the sampler's rejection loop and FP transcomputes are not.
  The CT rewrite (fixed-trip loop, masked FP/fixed-point sampler) is
  the research residue, and no kernel in the repo supports a CT claim
  today (the audit's own verdict).

## 3. FIPS 206 Status (UNVERIFIED-NIST)

- FN-DSA announced as the third selection (with ML-DSA, SLH-DSA) in
  July 2022.
- The instrument is FIPS 206. Internal draft approval 2025-08-28
  (p01's record); the initial public draft is UNPUBLISHED as of
  2026-10-31; finalization expected late 2026 or 2027.
- Consequence for the milestone: no public draft means no frozen
  parameter sets, no KAT corpus, no ACVP surface. Conformance — the
  milestone's only gate (discipline 1, KAT byte-parity) — is
  structurally uncheckable for FN-DSA today.

## 4. The avec Verdict: DEFER

The evidence chain is three legs; the DEFER stands if all three hold,
and flips on any single leg collapsing:

1. **CT sampling difficulty x zero-deps.** CT Gaussian sampling is the
   hardest CT problem in PQC, unsolved ecosystem-wide; the avec bar
   (M=1 branchless manifest, DATA_INDEPENDENCE_AUDIT classification)
   cannot court a secret-dependent rejection loop or an unpinned FP
   pipeline today. Fixing it needs a fixed-point/FP-semantics research
   program that also requires the standard's normative semantics.
2. **No final standard.** No public FIPS 206 draft exists; no frozen
   parameters, no KAT corpus; the conformance gate is structurally
   uncheckable. Any code written now targets a moving spec — the exact
   class of overclaim the milestone's standing vocabulary exists to
   prevent.
3. **The FFT/FF arithmetic itself is wave-class, not the blocker.** The
   `Z_q`-side NTT over q=12289 is the butterfly class the wave plan
   already builds; the FP FFT/LDL machinery has no in-tree precedent
   but is ordinary numerics. The blocker is leg 1+2 jointly: hardest
   CT problem with no frozen target.

Verdict shape: **DEFER** — a recorded deferral per the index rule
("deferrals, not silent gaps"), consistent with p01's 2026-10-31
adjudication (FN-DSA DEFER). p05 (wave-4 plan) stays void by its own
hook; the W4 index row becomes a recorded deferral row, not a silent
gap. This is a capability-unavailable-at-standard deferral, not a
scheduling one.

**Revisit trigger** (both conditions, matching p01's table): FIPS 206
final publication AND a published CT reference implementation (or
final-admitted fixed-point variant) portable under zero-deps. Final
alone leaves CT unsolved; a CT reference without a final is not a
conformance target. On the trigger: re-freeze parameters from the
final, re-run section 2.2 against the final's sampler requirements, and
re-issue p05 against the final's actual content.

**Expected cost if triggered**: 1+ wave slot, unbounded until the CT
Gaussian path is proven. The split: `Z_q`-side NTT verify arithmetic is
wave-class work (ML-KEM/ML-DSA butterfly class, port-shaped); the FP
FFT/LDL/Gaussian path is wave-class too (ordinary numerics at KAT
parity) EXCEPT the CT sampler, which is research-class and the
schedule-unbounded part.

**What transfers anyway**: (a) the NTT over q=12289 shares the
butterfly scaffold class with the wave-1/2 NTTs — quantify reuse only
if FN-DSA re-opens (open item 5); (b) the fixed-trip-loop and masked-
check CT disciplines built in waves 1-2 are the prerequisite skills a
future FN-DSA wave would court against; (c) the ML-DSA-44/65/87 sizes
(t04) cover the small-signature demand: ML-DSA-44 at 2420 B is the
smallest FIPS-standardized signature, so the wire-size pressure that
makes Falcon attractive is partly absorbed by the GO schemes.

**What does not transfer**: the FP FFT/LDL/Gaussian machinery — no
in-tree precedent, no zero-deps FP substrate in the repo, and the
audit register has no CT-class vocabulary for FP pipelines at all.

## 5. Open Items

1. **FIPS 206 public draft.** Timing, instrument number, comment
   period — UNVERIFIED-NIST. The revisit trigger keys on the
   publication event, not this survey's recollection of timing.
2. **Parameter drift.** Round-3 Falcon parameters (n = 512/1024,
   q = 12289, sigma tables, the verification bound constant) may drift
   in the FN-DSA draft (UNVERIFIED-SPEC). No avec code exists to
   hard-code them; keep it that way until the final freezes.
3. **CT sampler evidence plan.** What would satisfy
   DATA_INDEPENDENCE_AUDIT classification for a Gaussian sampler —
   fixed-trip sampling, masked FP/fixed-point state, platform-pinned
   FP semantics from the standard itself. Unscoped; needs its own
   ticket if the trigger fires.
4. **Verify-only carve-out.** Verification is public-grade (CI, no
   sampler, no secrets), so a verify-only FN-DSA has no CT liability —
   but it also has no standalone conformance corpus (KATs bind sign
   and verify; ACVP verify-only surface UNVERIFIED-NIST), and the p01
   DEFER covers the scheme, not a carve-out. Landing one needs an
   amendment ticket, not this survey. Recorded, not recommended.
5. **NTT(12289) reuse quantification.** If FN-DSA re-opens, measure
   butterfly-scaffold overlap against the wave-1/2 kernels before
   deciding shared code; t04 section 4.2 warns that shared-NTT review
   across schemes must assume nothing transfers except scaffolding.
6. **Index amendments owed.** t08 slot name (this file vs
   `t08-zero-dep-crypto-substrate.md`), the t04 row hook (landed t04
   is ML-DSA mechanics, not Falcon), and p01's dangling link to the
   nonexistent `t04-falcon-fn-dsa-feasibility.md` — all owned by the
   index owner (agent 1).

## Sources

- FIPS 206 status: NIST CSRC announcements and p01's 2026-10-31 record
  — all process claims UNVERIFIED-NIST, not re-fetched this session.
- Falcon specification (Fouque et al.), falcon-sign.info — parameters,
  sizes, sampler structure; UNVERIFIED-SPEC, not re-fetched.
- Ducas-Prest fast-Fourier sampling / FFT orthogonalization (Eurocrypt
  2016); Lyu12 rejection sampler; Peikert CDT sampler; DDY integral
  lattice Gaussian sampling — sampler-family provenance,
  UNVERIFIED-EXTERNAL.
- Leaky Falcon timing attacks (2020-2021) — UNVERIFIED-EXTERNAL.
- Fixed-point Falcon research (PQShield-class, 2026) —
  UNVERIFIED-EXTERNAL, per p01's record.
- Repo grounding: `docs/jira/v26.11.1/00-index.md` (disciplines,
  deferral rule, slot contract), `tickets/p01-go-no-go-adjudication.md`
  (the adjudication this ticket feeds), `tickets/t04-ml-dsa-rejection-hints.md`
  (NTT contrast method, slot-note precedent, ML-DSA size coverage),
  `tickets/t07-hqc-bike-survey.md` (format + CT precedent),
  `docs/DATA_INDEPENDENCE_AUDIT.md` (CI/PD/TL classes, no-CT-claim
  verdict), `docs/BRANCH_REGISTER.md` (M=1 / CC=1 branch-audit method).
