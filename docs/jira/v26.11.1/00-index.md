# AVEC v26.11.1 Milestone Index

Hub document and charter for the v26.11.1 milestone: **AVEC PQC —
Post-Quantum Cryptography for the Sovereign Substrate**. This file is the
canonical document index for the milestone. All lanes write under
`docs/jira/v26.11.1/` only; the filenames in the tables below are the
canonical slots every lane fills. The index is amended, never bypassed.

## Milestone Provenance

- 20-agent fan-out over the single canonical checkout at `/Users/sac/avec`,
  2026-10-31. No worktrees, no per-lane copies, no git commands from lanes.
- Agent 1 of 20 owns this file and thereby the filename contract; every
  other lane writes into the slots defined here.
- Every claim in this charter is UNVERIFIED (see Standing). Nothing in this
  milestone has executed; no KAT has been run; no receipt exists yet.

## Mission

Hand-rolled, zero-external-dependency FIPS 203 (ML-KEM) and FIPS 204
(ML-DSA) inside the avec substrate, extending the branchless M=1 discipline
to cryptographic arithmetic.

- Conformance is the deliverable: KAT byte-parity is the gate, benchmarks
  are advisory.
- SLH-DSA (FIPS 205) and Falcon (FN-DSA, FIPS 206 draft) are admitted only
  if the Phase-1 Go/No-Go adjudication (p01) does not defer them.
- Every crypto kernel joins the branchless manifest with
  `observed_mnemonics`, same as the v26.10.31 kernels did.
- The P28-SIL v2.1.0 license governs all of it (see Disciplines).

## Governing Disciplines (Inherited from v26.10.31)

These five disciplines carry over from the parent milestone unchanged.
They are the admission criteria for every artifact this milestone lands.

1. **Conformance outranks throughput.** NIST Known-Answer-Test byte-parity
   is the gate; benchmark numbers are advisory context, never gates. A
   kernel that is fast and off-by-one is dead.
2. **Zero external dependencies.** Hand-rolled zeroize, hand-rolled
   constant-time primitives (the `ring` role), hand-rolled threading (the
   crossbeam/rayon roles) — the dependency audit checks
   (`../v26.10.31/82-dependency-audit.md` method) must show E=0 runtime
   externals for every PQC crate it covers.
3. **Branchless manifest.** Every crypto kernel joins the branchless
   manifest with `observed_mnemonics`, adjudicated per the ADR-14 remainder
   policy. The gate is the
   `../v26.10.31/gate-branchless-manifest.md` method, extended to the new
   kernels.
4. **ADR-19 applies** (`../v26.10.31/22-adr-register.md`): CTQ limits are
   amendable when proven impossible, via the doc-48 re-derivation pattern.
   Unverified parameter claims (e.g. Gemini-sourced parameter numbers)
   enter as KAT-validation inputs only — never as receipted facts. A
   parameter claim becomes a fact when a KAT receipt says so.
5. **P28-SIL v2.1.0 governs.** `urn:avec:license:p28-sil:v2.1.0-sovereign`,
   inherited from the parent repo (`LICENSE-P28-SIL.md`). No SPDX
   identifiers; manifests and headers cite the URN. Compliance mechanics:
   `../v26.10.31/34-p28-sil-compliance.md`.

## Phase Structure

| Phase | Shape | Output surface |
|---|---|---|
| 0 | 20 read-only explore lanes | `tickets/` t01-t14 — findings are first-artifact receipts |
| 1 | Plan lanes | `tickets/` p01 Go/No-Go; p02-p06 wave plans |
| 2 | Wave 1 | FIPS 203 ML-KEM: spec + implementation + courts together |
| 3 | Wave 2 | FIPS 204 ML-DSA: spec + implementation + courts together |
| 4 | Wave 3 | FIPS 205 SLH-DSA (Go/No-Go gated): spec + impl + courts together |
| 5 | Wave 4 | FN-DSA Falcon (Go/No-Go gated): spec + impl + courts together |
| 6 | Wave 5 | Substrate integration: NIF surface, manifest join, audits, release |

Waves carry spec + implementation + courts together, so each wave doc is a
PRD/ARD hybrid: requirements, kernel design, and its court corpus in one
document.

## Document Index

Standing of every row: UNVERIFIED until the file exists on disk and, for
implementation artifacts, until KAT receipts land against it.

### tickets/ — Phase 0 explore tickets (t01-t14)

Filenames below are relative to `tickets/`.

| Lane | File | Hook |
|---|---|---|
| t01 | `t01-fips-203-ml-kem-spec.md` | FIPS 203 ML-KEM parameter sets, KAT corpus, encap/decap |
| t02 | `t02-fips-204-ml-dsa-spec.md` | FIPS 204 ML-DSA parameter sets, signing/verify surface |
| t03 | `t03-fips-205-slh-dsa-survey.md` | SLH-DSA stateless hash-based signatures; Go/No-Go input |
| t04 | `t04-falcon-fn-dsa-feasibility.md` | Falcon FP FFT under branchless M=1; Go/No-Go input |
| t05 | `t05-pqc-prior-art-survey.md` | Reference impls, PQClean-class corpora, KAT formats |
| t06 | `t06-branchless-crypto-arithmetic.md` | M=1 on NTT, poly arithmetic, rejection sampling |
| t07 | `t07-kat-corpus-provenance.md` | KAT acquisition, byte-parity harness, provenance chain |
| t08 | `t08-zero-dep-crypto-substrate.md` | Hand-rolled zeroize, CT prims, threading choices |
| t09 | `t09-adr19-ctq-amendment.md` | CTQ amendment policy for crypto limits; ADR-19 inputs |
| t10 | `t10-p28-sil-license-mapping.md` | P28-SIL v2.1.0 obligations mapped onto crypto kernels |
| t11 | `t11-beam-nif-surface.md` | BEAM NIF surface for PQC ops; zero-copy + alloc discipline |
| t12 | `t12-constant-time-audit-method.md` | CT audit; observed_mnemonics capture per kernel |
| t13 | `t13-court-corpus-design.md` | Court design: KAT courts, negative controls, mutations |
| t14 | `t14-risk-open-questions.md` | Risk register + open questions carried from v26.10.31 |

### tickets/ — Phase 1 plan tickets (p01-p06)

Filenames below are relative to `tickets/`.

| Lane | File | Hook |
|---|---|---|
| p01 | `p01-go-no-go-adjudication.md` | Adjudicates W3 (SLH-DSA) and W4 (Falcon) admission |
| p02 | `p02-wave1-plan.md` | Wave 1 (ML-KEM) execution plan |
| p03 | `p03-wave2-plan.md` | Wave 2 (ML-DSA) execution plan |
| p04 | `p04-wave3-plan.md` | Wave 3 (SLH-DSA) plan — void unless p01 admits |
| p05 | `p05-wave4-plan.md` | Wave 4 (Falcon) plan — void unless p01 admits |
| p06 | `p06-wave5-plan.md` | Wave 5 (integration/release) plan |

### waves/ — Phase 2-6 PRD/ARD docs (W1-W5)

Filenames below are relative to `waves/`.

| Wave | File | Hook |
|---|---|---|
| W1 | `w1-ml-kem-fips203.md` | ML-KEM PRD/ARD: spec, kernel design, KAT courts |
| W2 | `w2-ml-dsa-fips204.md` | ML-DSA PRD/ARD: spec, kernel design, KAT courts |
| W3 | `w3-slh-dsa-fips205.md` | SLH-DSA PRD/ARD — CONDITIONAL on p01 Go |
| W4 | `w4-falcon-fips206.md` | FN-DSA PRD/ARD — CONDITIONAL on p01 Go |
| W5 | `w5-substrate-integration.md` | Integration PRD/ARD: NIF surface, manifest join, release |

Total surface: this index + 14 + 6 + 5 = 26 documents.

## Standing

- **Everything is UNVERIFIED** — the milestone starts with the same
  evidence discipline as v26.10.31: UNVERIFIED is the default; ALIVE
  requires execution against the exact admitted subject with a command +
  exit receipt.
- Phase-0 explore findings are receipts from the first artifact of the
  milestone — they receipt the exploration, not the engineering. They do
  not promote any engineering claim.
- Nothing is VERIFIED, ALIVE, or PASS here until a KAT byte-parity receipt
  lands. Benchmark numbers, where they eventually appear, are advisory.
- Wave-3 (SLH-DSA) and Wave-4 (Falcon) docs are CONDITIONAL: if p01 defers
  them, the rows above become recorded deferrals, not silent gaps.

## See Also

- [../v26.10.31/00-index.md](../v26.10.31/00-index.md) — parent milestone
  index (disciplines inherited from here).
- [../v26.10.31/22-adr-register.md](../v26.10.31/22-adr-register.md) —
  ADR-14 (remainder policy) and ADR-19 (CTQ amendment clause).
- [../v26.10.31/82-dependency-audit.md](../v26.10.31/82-dependency-audit.md)
  — the zero-external-dep audit method this milestone re-runs.
- [../v26.10.31/34-p28-sil-compliance.md](../v26.10.31/34-p28-sil-compliance.md)
  — P28-SIL operational compliance mechanics.
- [../../../LICENSE-P28-SIL.md](../../../LICENSE-P28-SIL.md) — the governing
  license instrument (URN, no SPDX).
- [../../../README.md](../../../README.md) — repo overview.
