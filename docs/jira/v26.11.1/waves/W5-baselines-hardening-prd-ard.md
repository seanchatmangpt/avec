# W5-baselines-hardening-prd-ard

| | |
|---|---|
| Doc | Wave-5 PRD/ARD — baselines + hardening (v26.11.1 PQC milestone) |
| Plan | [../tickets/p06-wave5-baselines-plan.md](../tickets/p06-wave5-baselines-plan.md) |
| Inputs | t09-baseline-landscape-liboqs.md (consumed; survey posture) |
| Standing | UNVERIFIED until wave-5 execution |
| Wave shape | PRD/ARD hybrid per 00-index.md: requirements + court corpus in one doc |
| License | P28-SIL v2.1.0, `urn:avec:license:p28-sil:v2.1.0-sovereign` — no SPDX |

## 1. Overview

Wave 5 hardens and baselines whatever waves 1-4 landed in
`crates/avec-pqc`. It adds no cryptographic algorithm. Its outputs are
measurement and adversarial courts over the landed kernels: an advisory
comparison against a scratch-built liboqs, SUPERCOP-shape cycle benches,
final statistical-power dudect verdicts, an error-path coverage sweep
across all crypto error enums, fuzz courts over the KAT parser and
decapsulation, the final branchless-manifest extension audit, docs
integration, and the wave-5 receipt.

Conformance is the deliverable and the non-negotiable: every KAT court
from waves 1-4 remains green at wave-5 HEAD. Benches are advisory. The
only statistical gate wave 5 adds is dudect.

### Goals

- G-1 Advisory external baseline: avec PQC ops vs liboqs, same host,
  same load profile, numbers labeled advisory and machine-local.
- G-2 SUPERCOP-shape cycle benches per admitted parameter set and op.
- G-3 dudect verdicts at statistical power: PASS per admitted kernel.
- G-4 Every crypto error enum variant constructed from outside its
  crate.
- G-5 Fuzz courts over KAT parsing and corrupted-ct decapsulation,
  proven non-vacuous.
- G-6 Every PQC kernel manifest-adjudicated in the branchless gate with
  observed mnemonics, no scope gaps.
- G-7 Docs carry reproducible commands: FEATURE_MATRIX rows, advisory
  artifact, receipt.

### Non-goals

- N-1 No new algorithm, parameter set, or kernel math. Wave 5 measures
  and attacks; it does not add crypto.
- N-2 BEAM NIF implementation — non-goal (t11 remains the design
  surface; a later wave or milestone owns implementation).
- N-3 No benchmark floors, no CI perf gates on PQC numbers. Advisory
  means advisory: no entry into `docs/BENCH_FLOORS.json`.
- N-4 No fuzzer dependency (`libfuzzer`, `arbitrary`, `rand`). Fuzz
  courts are seeded, deterministic, in-tree.
- N-5 No workspace changes to accommodate liboqs — scratch build in
  `/tmp`, comparison by stdout, workspace dep count stays at zero.

## 2. PRD — Functional Requirements

### FR-1: Advisory comparison bench vs liboqs (G-1, W5-A)

- FR-1.1 Build liboqs in `/tmp/avec-w5-liboqs` at a pinned release tag;
  capture the commit SHA, build flags, and host at build time
  (provenance is the point of the oracle).
- FR-1.2 Compare like ops only: KEM keygen/encap/decap and signature
  keygen/sign/verify for parameter sets both sides implement
  (expected: the ML-KEM and ML-DSA families, under liboqs's own names).
- FR-1.3 Interleaved A/B runs under one load profile; min-of-N per
  side; both sides' raw stdout digested in the receipt. A rebuilt
  oracle is a new oracle — numbers are never compared across oracle
  SHAs.
- FR-1.4 Output: `docs/jira/v26.11.1/pqc-advisory-bench.json` plus
  `w5bench-liboqs-compare` receipt. Advisory-labeled in every artifact.
- FR-1.5 Run-time oracle absence => a loud `SKIP-ORACLE-ABSENT` row,
  advisory bench still exits 0 (pyshacl-class refusal-to-fake; the
  oracle is context, not a gate). Build-time failure with no oracle =>
  BLOCKED receipt naming the failing step: recorded deferral, never a
  silent skip.
- FR-1.6 Results are sanity-checked by order of magnitude against the
  t09 section 3 performance classes (UNVERIFIED-EXTERNAL). Those
  classes are orientation only — never targets, floors, or CTQ limits
  (importing them as limits would re-mint the CTQ-06 defect ADR-19
  closed).

### FR-2: SUPERCOP-shape cycle benches (G-2, W5-B)

- FR-2.1 Ops: `keygen|encap|decap` per KEM; `keygen|sign|verify` per
  signature scheme; every admitted parameter set.
- FR-2.2 Home per t09 section 4: `crates/bench/src/bin/pqc_cycles.rs`
  (sibling of `ctq_report.rs` / `gage_rr.rs`), consuming avec-pqc —
  the repo's gage-validated timing infrastructure is reused, not
  re-implemented. avec-pqc itself gains zero deps.
- FR-2.3 Measurement contract (the CTQ courts' pattern): gage admission
  first (mini-gage trimmed CV <= 2%, else SKIP-NOISY); KAT screen
  before timing — a row failing the KAT/reference digest is
  disqualified from the advisory table; min-of-N `Instant` loops with
  `black_box` on every output; all buffers allocated outside the timed
  window; counting-allocator pass asserts zero allocation events
  inside it.
- FR-2.4 Output rows machine-parseable, ns-first:
  `pqc-cycles: <algo>/<set> <op> min_ns=... rounds=... freq_mhz=...`;
  cycles as a derived field only when the run-time frequency record
  makes it computable. No fabricated cycle counts.
- FR-2.5 Advisory by construction: never a gate, never in
  BENCH_FLOORS.json (bench_gate.py would enforce a 25%-drop floor —
  wrong semantics for advisory baselines; t09 open item 6: floors-only
  stays).

### FR-3: dudect verdict bin (G-3, W5-C)

- FR-3.1 Method: fixed-secret vs random-secret classes, Welch t-test on
  timing distributions (dudect, ePrint 2016/1123).
- FR-3.2 One verdict bin, one home, named in the receipt, sharing the
  FR-2 timing core (one clock law); per admitted kernel it prints
  `PASS|FAIL|INCONCLUSIVE` with `n` per class and `t`.
- FR-3.3 Final statistical-power runs: the high-n campaign, above any
  wave-1/2 smoke-scale probe. Floor = CTQ-PQC-03; ADR-19 is the only
  amendment route.
- FR-3.4 Verdict semantics: PASS = AC. FAIL on a manifest-clean kernel
  = methodology conflict -> ADR-19-class adjudication receipt
  (measurement artifact vs real leakage) before wave 5 closes.
  INCONCLUSIVE extends the campaign; it never downgrades to PASS.

### FR-4: Error-path matrix (G-4, W5-D)

- FR-4.1 Scope: every error enum in `crates/avec-pqc` (KAT, KEM, DSA,
  encoding families as waves 1-2 landed them).
- FR-4.2 Method: 81-error-path-coverage.md — every variant constructed
  and asserted from OUTSIDE the owning crate, in
  `crates/integration/tests/pqc_error_paths.rs` (avec-pqc path dev-dep).
- FR-4.3 Matrix in the wave-5 receipt: COVERED / NEWLY-COVERED /
  UNCOVERED per variant, exact test names cited.
- FR-4.4 Exit rule: zero UNCOVERED rows, or each UNCOVERED row paired
  with a fix-forward deletion of the dead variant.

### FR-5: Fuzz courts (G-5, W5-E)

- FR-5.1 KAT parser fuzz (`crates/avec-pqc/tests/fuzz_kat_parser.rs`):
  seeded hand-rolled PRNG; mutations over copies of the real KAT corpus
  (temp files, canonical corpus never mutated in place): bit flips,
  truncation, hex-digit substitution, duplicated/oversized fields,
  non-hex bytes; fixed seed list recorded in the receipt for replay.
  Lawful: parse Err or loud FAIL of the mutated file. Unlawful: panic,
  hang, or a mutated file passing as canonical.
- FR-5.2 Decaps-on-corrupted-ct
  (`crates/avec-pqc/tests/fuzz_decaps_corrupt.rs`): full single-bit
  enumeration over ct where feasible (ML-KEM-768: 1088 B ct = 8704
  single-bit decaps), plus seeded multi-bit flips. Lawful: `Ok` with a
  secret differing from honest (FIPS 203 implicit rejection), or `Err`
  (malformed). Unlawful: panic, hang, length-acceptance.
- FR-5.3 Non-vacuity: each court re-runs with its own seed list mutated
  into no-ops and must then behave differently (shim-court pattern) — a
  court indifferent to its own mutation source is FAIL.
- FR-5.4 Optional: corrupted-signature verify court, same shape,
  executor discretion, not an AC.

### FR-6: Branchless-manifest final extension audit (G-6, W5-F)

- FR-6.1 Every PQC kernel in `scripts/branchless_manifest.toml` with
  the full schema: scope_pattern, accepted_remainder (ADR-14 classes),
  expected_max_count, gate_mode, observed_mnemonics.
- FR-6.2 No-gap law (new kernels): default `BRANCHLESS_SCOPE` covers
  every new PQC scope_pattern. The wave-9 gap (`6lookup|6offset|
  chunks_4way`, blake3) is grandfathered; no new kernel enters behind
  one.
- FR-6.3 Zero-match detection (gate extension): a scope_pattern
  matching zero symbols in the audited artifact set exits 3
  (fail-closed) — a stale pattern can never adjudicate a vacuous pass.
- FR-6.4 Negative control re-receipted: zeroed-ceiling manifest
  override flips the gate to FAIL.

### FR-7: Docs integration (G-7, W5-G)

- FR-7.1 FEATURE_MATRIX PQC section: build-proof command per cell, in
  the file's grammar; KAT court commands included.
- FR-7.2 `docs/jira/v26.11.1/pqc-advisory-bench.json` as the advisory
  artifact; BENCH_FLOORS.json untouched.
- FR-7.3 CHANGELOG entry, URN-cited, no SPDX.

### FR-8: Wave-5 receipt (W5-H)

- FR-8.1 `docs/jira/v26.11.1/90-wave-5-receipt.md`: subject (HEAD at
  execution), per-court receipts, command + exit table, standing per
  court, advisory numbers (labeled), deviations (t09 standalone, index
  slots, p01 deferrals), final KAT-green assertion with exact commands.
- FR-8.2 Counts cited from other receipts are re-read from disk at use
  time; a mismatch re-issues the receipt (falsifier F-6).

## 3. Acceptance Criteria

| AC | Statement | Evidence |
|---|---|---|
| AC-1 | KAT courts green at wave-5 HEAD (non-negotiable) | court commands exit 0, in the receipt |
| AC-2 | Benches advisory | no pass-expect bench row; FLOORS unchanged |
| AC-3 | dudect t-stat PASS per admitted kernel | bin prints n and t; minted w5dudect-* |
| AC-4 | Fuzz courts green and non-vacuous | courts exit 0; FR-5.3 self-mutation receipt |
| AC-5 | Error paths: zero UNCOVERED or paired deletion | pqc_error_paths exit 0; matrix receipt |
| AC-6 | Branchless sweep green, no-gap, zero-match | audit exit 0, full scope; control flips |
| AC-7 | FEATURE_MATRIX rows re-run to exit 0 | commands from the matrix, executed |
| AC-8 | 90-wave-5-receipt.md complete | file cites every AC with commands |

Priority: AC-1 dominates. Wave 5 with AC-1 green and any subset of the
rest incomplete is a partial wave, never a failed milestone; wave 5 with
AC-1 red is FAIL regardless of anything else.

## 4. ARD — Architecture and Asset Reuse

Each court composes existing avec primitives; wave 5 writes glue and
courts, not new infrastructure.

| Court | Reuses (avec asset) | New surface |
|---|---|---|
| KAT-green (AC-1) | wave-1/2 KAT courts; corpus + provenance receipts | none — re-run only |
| Advisory bench (FR-1) | W5-B bench bin; oracle pattern | A/B driver; advisory JSON |
| Cycle benches (FR-2) | ctq_report/gage_rr timing courts; mini-gage admission | pqc_cycles.rs bin |
| dudect verdict (FR-3) | same FR-2 clock; branchless gate as CT cross-check | dudect verdict bin |
| Error paths (FR-4) | error_paths.rs method (doc 81); dev-dep grammar | pqc_error_paths.rs |
| KAT fuzz (FR-5.1) | fuzz_codec.rs shape; seeded PRNG; real KAT seeds | fuzz_kat_parser.rs |
| Corrupt-ct (FR-5.2) | avec-pqc encap/decap API; KAT-corpus fixtures | fuzz_decaps_corrupt.rs |
| Branchless (FR-6) | audit_branchless.sh; manifest schema; ADR-14 | manifest rows; zero-match |
| DoD checker (W5-0) | dod.sh alternate-manifest mode + selftest | v26.11.1 manifest entries |
| Receipts (W5-0) | mint_receipt.sh minter + template | RECEIPT_DIR override; w5* names |
| Docs (FR-7) | FEATURE_MATRIX grammar; CHANGELOG | PQC section rows |

One timing kernel, two consumers (FR-2, FR-3) is a law, not a
preference: N implementations of one calculus drift O(N^2); the dudect
campaign and the cycle benches must measure with the same clock.

## 5. CTQ Proposals (through ADR-19)

Limits are hypotheses with falsifiers, per ADR-19. Boundary: ADR-19
amends numeric/statistical limits only. Conformance booleans (AC-1) and
enumerable counts (AC-5) are spec gates — never amended by this route.

| CTQ | Limit | Type | Amendment |
|---|---|---|---|
| CTQ-PQC-01 | KAT byte-parity 100%, every court | spec gate | never amendable |
| CTQ-PQC-02 | corrupt-ct: zero panics on 1-bit sweep | spec gate + advisory | panics never |
| CTQ-PQC-03 | dudect: n >= 1e6/class; \|t\| < 4.5 band | statistical limit | ADR-19 receipt |
| CTQ-PQC-04 | benches: min-of-N, gage-admitted, >= 1e3 iters/op | advisory floor | ADR-19 receipt |
| CTQ-PQC-05 | error paths: zero UNCOVERED | enumerable count | deletion only |
| CTQ-PQC-06 | branchless: all kernels adjudicated | ADR-14-governed | remainder policy |

The `\|t\| < 4.5` PASS band and the 1e6 floor are proposed, adopted at
wave-5 execution, and falsifiable: a kernel at n = 1e6 with \|t\| >= 4.5
is FAIL regardless of any prior smoke probe. If 1e6 per class proves
impossible within a bounded machine budget, the ADR-19 receipt
re-derives the floor from ceiling math and the measured distribution —
the old receipts stand; the floor is not silently lowered.

## 6. Falsifiers

| F | Breaks | Observation |
|---|---|---|
| F-1 | AC-1 | any KAT court exit != 0 at wave-5 HEAD — wave FAIL |
| F-2 | AC-2 | bench number in BENCH_FLOORS.json or a pass-expect dod row |
| F-3 | AC-3 | PASS line lacking n/t; bin cannot print INCONCLUSIVE |
| F-4 | AC-4 | court unchanged under self-mutation of its seed list |
| F-5 | AC-6 | scope_pattern matching zero symbols — gate exit 3 |
| F-6 | AC-8 | receipt count differing from the cited file on disk |
| F-7 | FR-1 | liboqs in Cargo.toml, a path dep, or an include path |
| F-8 | FR-5 | canonical KAT corpus files modified by a fuzz run |

## 7. Definition of Done

Wave 5 is done when all of the following hold, with commands executed
and exits recorded in 90-wave-5-receipt.md:

1. `cargo test -p avec-pqc` — exit 0 (KAT courts, unit, fuzz courts).
2. `cargo test -p avec-integration --test pqc_error_paths` — exit 0.
3. `scripts/audit_branchless.sh` under a scope covering all PQC patterns
   plus `transpose|predicates` — exit 0; selftest exit 0; zeroed-ceiling
   negative control exit 1; zero-match check live.
4. dudect verdict bin — exit 0, per-kernel PASS with `n` and `t`.
5. Advisory artifacts minted: `w5bench-*` receipts +
   `pqc-advisory-bench.json` (or a BLOCKED receipt for FR-1).
6. `scripts/dod.sh docs/jira/v26.11.1/dod-manifest.toml` — exit 0, no
   red rows, advisory rows listed yellow, `--selftest` exit 0.
7. FEATURE_MATRIX PQC rows re-run to exit 0.
8. `CHANGELOG.md` PQC entry present, URN-cited, zero SPDX strings
   (`grep -r "SPDX" crates/avec-pqc docs/jira/v26.11.1` empty).
9. 90-wave-5-receipt.md lands citing 1-8 with the subject HEAD.

## 8. Standing

Everything in this document is UNVERIFIED until wave-5 execution against
the landed waves 1-4 subject. Courts become ALIVE per executed command
with a recorded exit; benches and dudect numbers, once they exist,
remain advisory and machine-local regardless of standing. AC-1 receipts
are the only promotion path for conformance claims; nothing else in
wave 5 promotes any wave-1/2 engineering claim.

## See Also

- [../tickets/p06-wave5-baselines-plan.md](../tickets/p06-wave5-baselines-plan.md)
  — execution plan.
- [../tickets/t09-baseline-landscape-liboqs.md](../tickets/t09-baseline-landscape-liboqs.md)
  — consumed survey: oracle + bench design inputs.
- [../00-index.md](../00-index.md) — milestone charter and disciplines.
- [../../v26.10.31/22-adr-register.md](../../v26.10.31/22-adr-register.md)
  — ADR-14, ADR-19.
- [../../v26.10.31/27-branchless-kernel-abi.md](../../v26.10.31/27-branchless-kernel-abi.md)
  — branchless ABI spec.
- [../../v26.10.31/81-error-path-coverage.md](../../v26.10.31/81-error-path-coverage.md)
  — error-path method.
- [../../v26.10.31/82-dependency-audit.md](../../v26.10.31/82-dependency-audit.md)
  — zero-external-dep audit method.
- [../../v26.10.31/34-p28-sil-compliance.md](../../v26.10.31/34-p28-sil-compliance.md)
  — compliance mechanics.
- [../../../FEATURE_MATRIX.md](../../../FEATURE_MATRIX.md) — matrix
  grammar.
- [../../../../LICENSE-P28-SIL.md](../../../../LICENSE-P28-SIL.md) —
  governing license (URN).
