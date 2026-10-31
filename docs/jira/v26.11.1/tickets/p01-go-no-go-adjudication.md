# P01 Go/No-Go Adjudication — PQC Scheme Portfolio

Phase-1 plan ticket. Adjudicates which PQC schemes the v26.11.1 milestone
admits to a wave, on the evidence available at write time. Decides W3
(SLH-DSA) and W4 (Falcon) admission per the index contract; ratifies or
amends the charter's implicit W1/W2 shape.

## Standing

UNVERIFIED throughout. This ticket receipts a decision, not an engineering
result. Nothing here executes a KAT, compiles a kernel, or measures
anything. A GO verdict means "proceeds to wave planning"; it does not admit
an implementation — admission happens at the wave's own PRD/ARD plus KAT
byte-parity receipts (discipline 1).

## Inputs Consumed

- `00-index.md` — the milestone contract this ticket executes (p01 slot,
  W3/W4 CONDITIONAL rows, disciplines 1-5).
- Parent-milestone disciplines, read from disk 2026-10-31:
  `../v26.10.31/18-m1-branchless-algebra.md` (M=1, mask algebra §3,
  disassembly enforcement §5), `../v26.10.31/22-adr-register.md` (ADR-14
  remainder policy; ADR-19 CTQ amendment clause),
  `../v26.10.31/82-dependency-audit.md` (E=0 proof method),
  `../v26.10.31/33-risk-register.md` (falsifier discipline).
- Standards status, public record as of 2026-10-31 (web-verified this
  session): FIPS 203/204/205 final 2024-08-13; FIPS 206 (FN-DSA/Falcon)
  internally approved draft 2025-08-28 but initial public draft not yet
  published; HQC selected 2025-03-11, draft standard pending, final
  expected 2027.
- Phase-0 explore tickets: NOT CONSUMED — `tickets/` was absent on disk at
  write time (t01-t14 unlanded). This adjudication is therefore STANDALONE.
  Amendment clause: if t03 (SLH-DSA), t04 (Falcon), or t06 (branchless
  arithmetic) lands with findings that contradict any evidence chain below,
  this ticket is amended by its owner lane; until then the chains below are
  the adjudication of record.

## Adjudication Summary

| Scheme | Standard | Verdict | One-line reason |
|---|---|---|---|
| ML-KEM | FIPS 203 (final) | GO | Straight-line NTT arithmetic; zero secret-indexed memory; FO select solved by existing mask algebra |
| ML-DSA | FIPS 204 (final) | GO | Shares the NTT core; two named CT risk items, both with in-discipline escapes |
| SLH-DSA | FIPS 205 (final) | DEFER | Sponge-dominant — no arithmetic kernel for the M=1 discipline to win |
| FN-DSA | FIPS 206 (draft) | DEFER | Standard not final; CT Gaussian sampling unsolved ecosystem-wide |
| HQC | pre-standard | DEFER | Standard not final; genuinely avec-shaped, revisit on the draft |

W1/W2 proceed. W3/W4 become recorded deferrals — the index's own rule
("deferrals, not silent gaps"). p04/p05 are void by their own hooks unless
this ticket is amended.

## ML-KEM (FIPS 203) — GO

### Evidence chain

1. **Final standard, frozen KAT.** FIPS 203 final since 2024-08-13; the
   KAT corpus is public and frozen. Conformance is checkable, which is the
   milestone's only gate (discipline 1).
2. **The arithmetic is the M=1 shape exactly.** ML-KEM is an NTT over
   q=3329, n=256, with fixed butterfly indices. Every memory access in the
   hot path is structurally indexed — no secret-dependent addressing
   anywhere in the scheme. This is the same fixed-probe geometry as the
   receipted branchless `.idx` ladder (18-m1 §4), applied to arithmetic.
3. **Correction to the charter shape: there is no rejection sampling in
   ML-KEM.** The sampler is CBD — a branch-free bit-sum of PRF output by
   construction. The expected-shape claim that "rejection sampling is the
   one hard CT item" is wrong for this scheme. The actual hard CT item is
   the Fujisaki-Okamoto transform's implicit rejection in decapsulation:
   a ciphertext equality test whose result selects between K' and
   (rho || z). That is a conditional select — and 18-m1 §3's mask algebra
   (delta = u xor v, sign-smear to full-width mask, ring-ops select) is
   precisely the primitive for it, already receipted in the substrate.
   The GO stands on this stronger footing: the scheme's one CT hazard is
   an instance of a solved discipline.
4. **Proposed CTQ CT-KEM-1** (owner: p02): the decapsulation FO select is
   mask-realized end to end; gate is the branchless-manifest join with
   `observed_mnemonics` plus a negative-control court (discipline 3).
5. **Scope note.** All three parameter sets ride the same kernel family.
   Which sets ship, and in what order, is p02's decision. Per discipline 4,
   no parameter numbers are claimed as facts here — they enter as
   KAT-validation inputs.

### Dependency-zero proof strategy

- One sovereign FIPS 202 Keccak core serves the whole scheme: SHA3-256,
  SHA3-512, SHAKE-128, SHAKE-256 are modes over the same Keccak-f[1600]
  permutation, which is a straight-line round function — M=1 by
  construction, same hand-rolled pattern as the receipted avec-blake3.
- E=0 proof per the `../v26.10.31/82-dependency-audit.md` method: empty
  `[dependencies]` in the new crate manifest, Cargo.lock sweep showing no
  foreign hash/RNG entries (getrandom-absent is an established precedent
  from the wave-14 audit), offline vendored rebuild.
- **RNG policy: randomness is injected, never generated.** Encapsulation
  seed and keygen entropy enter the kernel as byte inputs at the API seam.
  The kernel stays pure and deterministic — KAT-checkable by construction,
  and no RNG dependency exists to audit. Entropy sourcing is a host/BEAM
  concern (t11's surface), out of the kernel's receipt scope.
- Zeroize is hand-rolled (volatile write + compiler fence; t08's role),
  never the `zeroize` crate; p02 names the erasure points per key object.

## ML-DSA (FIPS 204) — GO

### Evidence chain

1. **Final standard, frozen KAT** (2024-08-13), same conformance footing
   as ML-KEM.
2. **Kernel reuse is real.** ML-DSA rides the same q=3329, n=256 NTT as
   ML-KEM — one sovereign NTT kernel serves both GO schemes. The delta is
   shift-and-round decompression, hint arithmetic, and norm checks: more
   straight-line arithmetic in the same discipline.
3. **The two real CT risk items, named honestly:**
   - **CT-DSA-1 (proposed), the signing rejection loop.** Signing retries
     a fresh y until the z norm bound holds. The loop's trip count is
     secret-dependent — this is the one genuinely secret-dependent
     control-flow item in the entire GO portfolio. In-discipline escapes,
     for p03 to adjudicate: (a) fixed-attempt-batch execution with the
     accepted attempt selected by masks — expressible in M=1; (b) a
     documented leak analysis under ADR-19, if the batch approach is
     refuted by measured cost; no third option — an unbounded
     secret-dependent loop cannot join the branchless manifest.
   - **CT-DSA-2 (proposed), sparse-multiply secret-index gather.** The
     sparse c times s product gathers s at c's nonzero positions, and
     those positions are secret. Escape: the dense c*s multiply is CT and
     always available; and per discipline 1 (conformance outranks
     throughput) the dense form is fully admissible with the sparse form
     as an advisory optimization, gated on its own CT proof. This escape
     hatch is why GO survives adjudication rather than being deferred:
     neither risk item is capability-blocking.
4. **Both risk items are wave-2 work by construction** — they are planning
   and gate decisions inside W2 (p03), not scope questions. The milestone
   charter already assigns ML-DSA to W2; this ticket does not move it.

### Dependency-zero proof strategy

- Shares ML-KEM's Keccak core. Pure ML-DSA (the default mode) needs only
  SHAKE-256 over the same permutation; pre-hash variants would extend the
  surface to SHA-2 — p03 scopes this; default is pure mode only.
- Same RNG-injection policy (signing nonce enters as bytes), same E=0
  proof method, same hand-rolled zeroize, same manifest join.

## SLH-DSA (FIPS 205) — DEFER

### Evidence chain

1. **The standard is final — the fit is not.** SLH-DSA is sponge-dominant:
   keygen, sign, and verify decompose into thousands of SHA-2/SHAKE
   invocations plus Merkle-tree bookkeeping. A hash function is already
   straight-line; there is no arithmetic kernel for the M=1 discipline to
   accelerate. The avec win (branch-free arithmetic over 64-byte-aligned
   buffers) has no surface here.
2. **Signature geometry is hostile to the quad plane.** SLH-DSA
   signatures are kilobyte-scale — on the order of 8-50 KB across the
   parameter sets (public record; to be pinned by t03's receipt, not
   asserted here, per discipline 4). The quad plane is 64-byte-aligned
   field columns; such signatures ride as undifferentiated byte spans,
   gaining nothing from the substrate's one layout discipline.
3. **The software win is already claimed by hardware.** ARMv8 crypto
   extensions and x86 SHA-NI make the fast generic implementation the one
   that calls a hardware instruction — permitted under zero-deps (an
   instruction is not a dependency) but equally available to every other
   implementation; a sovereign hand-rolled sponge has no differentiation
   to earn its audit surface.
4. **Cost is a full wave for zero discipline fit.** Hand-rolling SHA-256
   (not currently in-tree; avec has BLAKE3) plus the WOTS+/FORS/hypertree
   machinery at KAT parity occupies a W3-sized slot that produces no new
   branchless kernel class.
5. **The honest virtue, recorded:** SLH-DSA is the portfolio's most
   conservative security assumption (hash-only). That is a real property
   for long-lived receipts. It is a demand argument, not a substrate-fit
   argument — see the revisit trigger.

## FN-DSA / Falcon (FIPS 206) — DEFER

### Evidence chain

1. **The standard is not final.** As of 2026-10-31 the initial public
   draft of FIPS 206 is unpublished (internal draft approval 2025-08-28;
   finalization expected late 2026 or 2027). There is no frozen KAT
   corpus to gate against — conformance, the milestone's only gate, is
   not yet checkable (discipline 1 fails structurally).
2. **The CT core problem is the hardest in PQC and unsolved
   ecosystem-wide.** FN-DSA signing requires a discrete Gaussian sampler
   over an FP FFT lattice path. Floating-point hardware carries
   secret-data-dependent timing hazards (denormal penalties, flag-driven
   paths, divergent x86/NEON semantics) that have no clean M=1 mapping.
   The public record attributes FIPS 206's own delay to exactly this
   implementation difficulty — the ecosystem has not solved it; a
   sovereign substrate will not lead that solution.
3. **The FP path collides with the dependency wall.** Reference Falcon
   implementations lean on double-precision FP behavior; pulling an FP
   library violates zero-deps (discipline 2), and hand-rolling a CT
   FP/fixed-point engine that also hits the standard's normative FFT
   semantics is a research program, not a port. Published fixed-point
   work (PQShield, 2026) exists as research but is not the standard's
   FP semantics and is not a validated conformance target.
4. **Verdict shape:** DEFER with a hard external gate — this is not a
   scheduling deferral but a capability-unavailable-at-standard deferral.

## HQC (pre-standard) — DEFER

### Evidence chain

1. **Standard not final.** Selected by NIST 2025-03-11 as the fifth
   algorithm; draft standard pending as of 2026-10-31, final expected
   2027. Parameters and the KAT corpus are not frozen — same structural
   conformance gap as FN-DSA (discipline 1).
2. **The honest positive: this is the most avec-shaped deferred scheme.**
   HQC is code-based: GF(2)[x] bitwise polynomial arithmetic — u64 word
   vectors, shifts, popcount, masked reduction — which is precisely the
   SWAR + mask-algebra shape avec-arrow already receipts. Its FO-class
   implicit-rejection select has the same shape ML-KEM's solved CT-KEM-1
   case covers, so the CT method transfers. The DEFER is about standard
   immaturity, not substrate fit.
3. **Revisit cost is wave-class, not research-class** — see the table
   below. When the draft lands, HQC should win any available slot on
   merit among the deferred set.

## Revisit Triggers + Costs

Costs are UNVERIFIED estimates in wave-slot units; each trigger is a typed
observation, not a date. A trigger firing opens an amendment ticket to
this file, not a silent wave start.

| Scheme | Trigger (typed observation) | Estimated cost when triggered | First artifact |
|---|---|---|---|
| SLH-DSA | A consuming wave files a hash-based-signature demand (insurance/regulatory class — e.g. a notary or escrow consumer rejects lattice assumptions for long-lived receipts) | ~1 wave slot + 1 new sovereign SHA-2 core; CT risk low (already straight-line) | p04 re-issue + W3 PRD/ARD |
| SLH-DSA (alt) | Measured probe showing a sovereign multi-buffer sponge beating the hardware-SHA path by a margin justifying the audit surface | ~1 spike (cheap, pre-wave) before any wave commitment | probe receipt |
| FN-DSA | FIPS 206 final publication AND a published CT reference implementation (or final-admitted fixed-point variant) portable under zero-deps. Both required: final alone leaves CT unsolved; a CT reference without a final is not a conformance target | 1+ wave slot; unbounded until the CT Gaussian path is proven | p05 re-issue on the final's actual content |
| HQC | Draft standard published with frozen parameters + public KAT corpus | ~1 wave slot (ML-KEM-class); reuses Keccak core + FO-select mask pattern; new GF(2) kernels (carry-less mult, popcount, cyclic-mod reduce) | amendment ticket + W-slot plan |

## Explicit Falsifiers

What evidence would flip a verdict. A DEFER flips on its named
observation; a GO flips on any contradicting KAT or CT evidence at its
wave gate.

- **SLH-DSA DEFER -> GO:** the demand trigger or the probe trigger in the
  table above fires. Expected flipper: the demand trigger — a fit
  argument will not do it alone, by this ticket's own reasoning.
- **FN-DSA DEFER -> GO:** both table conditions fire. Partial firing
  (final standard, no portable CT reference) is recorded as DEFER-REVISED,
  not GO.
- **HQC DEFER -> GO:** the draft trigger fires. If the draft's parameters
  or transform diverge from the assumptions here, the amendment ticket
  re-adjudicates on the draft's actual content — the GO shape is
  provisional on draft fidelity.
- **ML-KEM/ML-DSA GO -> DEFER:** any of (a) t06 lands evidence that the
  NTT/CBD path cannot meet the ADR-14 remainder policy without a
  capability-blocking remainder growth; (b) the CT-DSA-1 batch approach
  is refuted by p03's planning AND the ADR-19 leak-analysis route is
  refused; (c) a KAT-corpus provenance failure (t07) breaks the
  conformance gate for the scheme. (a) or (b) would be genuinely
  surprising — they are named because a verdict without a falsifier is
  prose, per the risk-register discipline.
- **Whole-ticket:** any landed phase-0 ticket (t03, t04, t06 named)
  contradicting an evidence chain above amends this adjudication before
  its verdicts are consumed by p02/p03.

## Consequences

- W1 (ML-KEM) and W2 (ML-DSA) proceed to p02/p03 planning. This ticket
  adds two proposed CTQs (CT-KEM-1, CT-DSA-1, CT-DSA-2) and the
  RNG-injection and hand-rolled-zeroize policies to their inputs.
- W3/W4 are recorded deferrals per the index rule. p04/p05 are void by
  their own hooks; the milestone surface becomes 24 documents with the
  two void slots recorded here rather than silently absent.
- W5 integration scopes to ML-KEM + ML-DSA over the shared Keccak + NTT
  kernel base.
- The Keccak core and the RNG-injection policy are wave-1 prerequisites;
  p02 must sequence them ahead of any ML-KEM kernel work.

## Standing (closing)

Every claim above is a decision or a cited public record — no executed
verification exists for any engineering assertion. The verdicts of record
as of 2026-10-31: ML-KEM GO, ML-DSA GO, SLH-DSA DEFER, FN-DSA DEFER, HQC
DEFER. Amendment authority: this ticket's owner lane (16 of 20), or any
lane whose landed phase-0 findings contradict an evidence chain, filing
against this file.

## See Also

- [../00-index.md](../00-index.md) — milestone contract; p01 slot and
  CONDITIONAL wave rows.
- [../../v26.10.31/18-m1-branchless-algebra.md](../../v26.10.31/18-m1-branchless-algebra.md)
  — mask algebra (§3) and disassembly enforcement (§5) cited by the CT
  reasoning.
- [../../v26.10.31/22-adr-register.md](../../v26.10.31/22-adr-register.md)
  — ADR-14 remainder policy; ADR-19 CTQ amendment clause.
- [../../v26.10.31/82-dependency-audit.md](../../v26.10.31/82-dependency-audit.md)
  — the E=0 proof method each GO strategy inherits.
- [t01-fips-203-ml-kem-spec.md](t01-fips-203-ml-kem-spec.md),
  [t02-fips-204-ml-dsa-spec.md](t02-fips-204-ml-dsa-spec.md),
  [t03-fips-205-slh-dsa-survey.md](t03-fips-205-slh-dsa-survey.md),
  [t04-falcon-fn-dsa-feasibility.md](t04-falcon-fn-dsa-feasibility.md),
  [t06-branchless-crypto-arithmetic.md](t06-branchless-crypto-arithmetic.md)
  — phase-0 inputs named by the amendment clause (unlanded at write time).
- [p02-wave1-plan.md](p02-wave1-plan.md),
  [p03-wave2-plan.md](p03-wave2-plan.md) — GO consumers.
- [p04-wave3-plan.md](p04-wave3-plan.md),
  [p05-wave4-plan.md](p05-wave4-plan.md) — void by their hooks under this
  adjudication.
