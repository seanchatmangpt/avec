# p06-wave5-baselines-plan

| | |
|---|---|
| Ticket | p06 (v26.11.1 PQC wave, agent 20 of 20) |
| Type | Phase-1 plan — Wave 5: baselines + hardening |
| Standing | UNVERIFIED (plan only; nothing in this file has executed) |
| Inputs | Waves 1-4 crates/avec-pqc artifacts, t09 if landed, inherited infra |
| Owner | Wave-5 execution lane(s), post waves 1-4 |

## 1. Mission

Wave 5 closes the v26.11.1 PQC milestone with baselines and hardening over
whatever waves 1-4 landed: the advisory comparison bench against a
liboqs-linked reference, SUPERCOP-shape cycle benches, the final
statistical-power dudect verdicts, an error-path coverage sweep over every
crypto error enum, fuzz courts (KAT parser, decaps-on-corrupted-ct), the
final branchless-manifest extension audit, docs integration, and the
wave-5 receipt. Conformance stays the deliverable: every KAT court from
waves 1-4 must remain green at wave-5 HEAD. Benches are advisory context,
never gates.

## 2. Slot and Shape Notes

Recorded so the index owner (agent 1) can amend, not bypass:

- The 00-index.md slots name `tickets/p06-wave5-plan.md` and
  `waves/w5-substrate-integration.md`. This lane's dispatch assigns
  `tickets/p06-wave5-baselines-plan.md` and
  `waves/W5-baselines-hardening-prd-ard.md`. The dispatch slots are the
  operative contract for this lane; the index amendment is the owner's
  transition.
- Wave-5 shape evolved from the index's "substrate integration" hook to
  baselines + hardening. The BEAM NIF surface stays a design input (t11);
  NIF implementation is a recorded non-goal (PRD section 2, non-goals).
- `85-wave-11-receipt.md` at the milestone root is wrong-numbered and is
  skipped by this lane. The wave-5 receipt slot defined here is
  `docs/jira/v26.11.1/90-wave-5-receipt.md`.

## 3. Inputs

- **Waves 1-4** (prerequisite): `crates/avec-pqc` with the admitted
  parameter sets, KAT courts, and error enums. Wave 5 consumes them; it
  does not re-open them. If a wave-5 court exposes a wave-1/2 defect, the
  fix lands as a named repair in both waves' receipts, never silently.
- **t09 (liboqs landscape)**: LANDED 2026-10-31 as
  `tickets/t09-baseline-landscape-liboqs.md` (SURVEY-ONLY; its filename
  also deviates from the index slot — flagged there as its open item 1).
  Consumed by this plan, survey findings are orientation inputs, never
  receipted facts:
  - W5-B home and measurement contract per t09 section 4: bench bin in
    `crates/bench/src/bin` (sibling of `ctq_report.rs` / `gage_rr.rs`),
    gage admission before timing (mini-gage trimmed CV <= 2%, else
    SKIP-NOISY), KAT-correctness screen disqualifying rows before any
    number is reported, min-of-N `Instant` loops, `black_box` on every
    output, buffers allocated outside the timed window, counting-allocator
    pass asserting zero allocation events inside it.
  - W5-A oracle mechanics per t09 sections 1/5: liboqs invoked as a
    `/tmp` subprocess oracle (b3sum/pyshacl precedent), never linked;
    SHA captured at build time; a rebuilt oracle is a new oracle —
    numbers are not comparable across SHAs.
  - W5-A result sanity check: t09 section 3 performance classes
    (UNVERIFIED-EXTERNAL) are order-of-magnitude orientation only —
    never targets, floors, or CTQ limits.
  - Advisory artifact class: the merged comparison JSON is a new
    artifact; `docs/BENCH_FLOORS.json` stays floors-only (t09 open
    item 6 agrees with AC-2).
  - Units: ns-first reporting; cycles as a derived field with the
    frequency recorded at run time (t09 open item 2).
- **p01 (Go/No-Go)**: wave 5 covers exactly the parameter sets p01
  admits. A p01 deferral shrinks W5-B/W5-C scope to the surviving
  algorithms; the deferred algorithm's rows become recorded deferrals,
  never silent gaps.
- **Inherited infrastructure**: section 5.

## 4. Work Items

### W5-0 — Infrastructure (first, before any court)

1. `scripts/mint_receipt.sh`: add env overrides so receipts mint into
   the v26.11.1 receipt dir without touching the default path:
   `RECEIPT_DIR` env (default stays `docs/jira/v26.10.31/receipts`) and
   a template fallback (if `$RECEIPT_DIR/template.md` is missing, fall
   back to the canonical v26.10.31 template). Default behavior
   byte-identical when the env vars are unset. Receipts are append-only
   (exit 73 on existing name) — family names below are one-time.
2. Receipt families for wave 5 (all `[A-Za-z0-9._-]` per the minter's
   charset rule), minted into `docs/jira/v26.11.1/receipts/`:
   `w5gate-*`, `w5kat-*`, `w5fuzz-*`, `w5dudect-*`, `w5bench-*`,
   `w5errpath-*`.
3. `docs/jira/v26.11.1/dod-manifest.toml`: seed entries for every wave-5
   court (section 7 table). Run via the checker's existing
   alternate-manifest mode — `scripts/dod.sh` itself is NOT modified; it
   already accepts `scripts/dod.sh <manifest>`, already fails closed
   (exit 3 on missing or zero-entry manifest), and already carries the
   `--selftest` anti-vacuity contract.
4. Re-run `scripts/dod.sh --selftest` once after the manifest lands
   (exit 0 expected) — the selftest is per-checker, not per-manifest,
   but the re-run receipts the checker version actually used.

### W5-A — liboqs-linked advisory comparison bench

- Build liboqs in scratch (`/tmp/avec-w5-liboqs`), pinned to an exact
  release tag with the commit SHA captured at build time. NEVER a
  workspace dependency: no Cargo.toml reference, no path dep, no include
  path into the repo. The oracle is invoked as a subprocess whose stdout
  is merged as data (the b3sum/pyshacl precedent t09 cites); the avec
  side is the W5-B bench bin. Nothing about the avec build changes.
- Oracle rows over the same (alg, op) grid as W5-B. Absent oracle
  binary at run time => a loud `SKIP-ORACLE-ABSENT` row, advisory bench
  still exits 0 (the oracle is context, not a gate — pyshacl-class loud
  refusal, never a faked comparison). Build-time failure with no oracle
  => mint a BLOCKED receipt naming the failing step: a recorded
  deferral, never a silent skip.
- Merge JSON carries both sides' rows plus the liboqs build provenance
  (version, SHA, flags). A rebuilt oracle is a new oracle: numbers are
  never compared across oracle SHAs, and each receipt cites the SHA it
  ran against.
- Interleave A/B/A/B (avec, oracle) runs under one load profile;
  min-of-N per side. Machine-local, advisory only. Result rows are
  sanity-checked by order of magnitude against t09 section 3 classes
  (UNVERIFIED-EXTERNAL) — orientation, never targets.
- Output: `docs/jira/v26.11.1/pqc-advisory-bench.json` plus receipt
  `w5bench-liboqs-compare` (SHA, flags, host, both sides' stdout
  digests). `docs/BENCH_FLOORS.json` stays floors-only.

### W5-B — SUPERCOP-shape cycle benches

- Per admitted parameter set, the SUPERCOP op shape: `keygen`, `encap`,
  `decap` for KEMs; `keygen`, `sign`, `verify` for signatures.
- Home per t09 section 4: `crates/bench/src/bin/pqc_cycles.rs` (sibling
  of `ctq_report.rs` / `gage_rr.rs`), consuming avec-pqc — the repo's
  gage-validated timing infrastructure is reused, not re-implemented
  (one clock law with W5-C). crates/bench gains one path dep
  (avec-pqc, workspace member); avec-pqc itself gains zero deps.
- Measurement contract (t09 section 4, the CTQ courts' pattern): gage
  admission first (mini-gage trimmed CV <= 2%, else SKIP-NOISY); KAT
  screen before timing — a row whose output fails the KAT/reference
  digest is disqualified from the advisory table; min-of-N `Instant`
  loops, `black_box` on every output; all buffers allocated outside the
  timed window; counting-allocator pass asserts zero allocation events
  inside it.
- Output lines machine-parseable, ns-first:
  `pqc-cycles: <algo>/<set> <op> min_ns=... rounds=... freq_mhz=...`
  with cycles as a derived field only when the run-time frequency record
  makes it computable (t09 open item 2). No fabricated cycle counts.
- Advisory by construction: these numbers NEVER enter
  `docs/BENCH_FLOORS.json` (that file is the bench_gate.py regression
  gate, exit 1 on a 25% drop — the wrong semantics for advisory
  baselines; t09 open item 6: floors-only stays). The advisory artifact
  is `docs/jira/v26.11.1/pqc-advisory-bench.json`.

### W5-C — dudect statistical verdicts (final statistical-power runs)

- Method: dudect ("Dude, is my code constant time?", Reparaz/Balasch/
  Tunstall, ePrint 2016/1123) — fixed-secret vs random-secret input
  classes, Welch t-test on timing distributions.
- Home: the crates/bench bin family (one timing core with W5-B — the
  gage-validated clock, not a second timer). Verdict bin per admitted
  kernel prints `PASS|FAIL|INCONCLUSIVE` with `n` per class and the
  t-statistic. A PASS line without `n` and `t` in the same output is a
  defective bin (falsifier F-3).
- Final statistical-power runs: this is the high-n campaign, distinct
  from any wave-1/2 smoke-scale constant-time probes. Iteration floor
  proposed as CTQ-PQC-03 (wave doc section 5); if the machine budget
  cannot reach the floor, the ADR-19 amendment path applies (48-pattern
  re-derivation), never silent floor reduction.
- Verdict semantics: PASS per kernel is the acceptance criterion (AC-3).
  A FAIL on a kernel the branchless manifest shows clean is a
  methodology conflict, not a pass/fail tie: it gets an ADR-19-class
  adjudication receipt (measurement artifact vs real leakage) before
  wave 5 closes. INCONCLUSIVE runs extend, never downgrade.

### W5-D — error-path coverage sweep (all crypto enums)

- Method: the v26.10.31 wave-14 sweep (81-error-path-coverage.md)
  extended to every error enum in `crates/avec-pqc`: every variant
  constructed and asserted from OUTSIDE the owning crate, in
  `crates/integration/tests/pqc_error_paths.rs` (adds `avec-pqc` as a
  path dev-dep of avec-integration — a workspace member, zero
  externals).
- Matrix: COVERED / NEWLY-COVERED / UNCOVERED per variant, citing exact
  test names, recorded in the wave-5 receipt.
- Exit rule: zero UNCOVERED rows, or each UNCOVERED row carries a
  fix-forward deletion of the dead variant (deletion outranks
  generation). An UNCOVERED row left standing with no deletion is
  wave-5 FAIL.

### W5-E — fuzz courts

- `crates/avec-pqc/tests/fuzz_kat_parser.rs`: the wave-1 KAT harness's
  file parser fed mutated corpora — bit flips, truncations, hex-digit
  substitution, duplicated/oversized fields, non-hex bytes. Seeded,
  hand-rolled PRNG (no `rand`, no `arbitrary`), fixed seed list recorded
  in the receipt so every run replays. Lawful verdicts on a mutated
  file: parse Err, or loud court FAIL of the mutated file. Unlawful:
  panic, hang, or a mutated file passing as if canonical.
- `crates/avec-pqc/tests/fuzz_decaps_corrupt.rs`: decapsulation under
  corrupted ciphertext. Full single-bit enumeration over ct where the
  parameter set makes it feasible (ML-KEM-768: 1088 B ct = 8704
  single-bit decaps — cheap), plus seeded multi-bit flips. Lawful
  verdicts: `Ok` with a shared secret different from the honest one
  (FIPS 203 implicit rejection), or `Err` (malformed). Unlawful
  verdicts: panic, hang, and length-acceptance (a length-truncated ct
  parsed as canonical is a defect regardless of the key outcome).
- Wave-1 KAT courts stay authoritative: a fuzz court never mutates the
  canonical corpus files in place — mutations live in temp files.
- Optional extension (not an AC, executor's discretion): a corrupted
  signature verify court for the signature side, same shape.

### W5-F — branchless-manifest final extension audit

- Every PQC kernel joins `scripts/branchless_manifest.toml` with the
  full schema: `scope_pattern`, `accepted_remainder` (ADR-14 classes),
  `expected_max_count`, `gate_mode`, and `observed_mnemonics` from the
  audit-profile disassembly. Incremental joins during waves 1-4 are
  expected; wave 5 owns the FINAL sweep.
- No-gap law for new kernels: the audit's default `BRANCHLESS_SCOPE`
  must cover every new PQC `scope_pattern`. The wave-9 documented gap
  (`6lookup|6offset|chunks_4way`, blake3) is grandfathered and stays
  documented; no new kernel enters behind a gap. Any new kernel whose
  adjudication requires an explicit non-default scope is a plan
  violation, not a documented gap.
- Zero-match detection (wave-5 gate extension): a `scope_pattern`
  matching zero symbols in the audited artifact set is a manifest
  defect — the gate exits 3 (fail-closed class), so a stale or mistyped
  pattern can never adjudicate as a vacuous pass. Grandfathered kernels
  run under an explicit scope that covers them, so they match and pass
  this check.
- Negative control re-run: `BRANCHLESS_MANIFEST` override with a zeroed
  ceiling must flip the gate to FAIL (existing mechanism, re-receipted).
- Court command: `scripts/audit_branchless.sh` with a scope expression
  covering `transpose|predicates|<all PQC patterns>`, exit 0, plus
  selftest exit 0.

### W5-G — FEATURE_MATRIX / docs integration

- `docs/FEATURE_MATRIX.md`: a PQC section — crate x feature rows with
  the build-proof command per cell, in the file's existing grammar
  (every YES proven by running the command; `—` cells carry the proving
  command).
- `docs/jira/v26.11.1/pqc-advisory-bench.json`: the advisory numbers
  artifact (from W5-A/W5-B) — explicitly NOT `docs/BENCH_FLOORS.json`.
- KAT court commands enter FEATURE_MATRIX so each conformance claim is
  reproducible from the matrix alone.
- `CHANGELOG.md` entry for the PQC milestone (URN-cited, no SPDX).

### W5-H — Wave-5 receipt

- `docs/jira/v26.11.1/90-wave-5-receipt.md`: the manufacturing receipt —
  subject (HEAD at execution), per-court receipts linked (section 4
  families), command + exit table, standing per court, advisory numbers
  table (advisory-labeled), deviations recorded (t09 standalone; index
  slot names; p01-deferred algorithms), and the final KAT-green
  assertion with the exact court commands.
- The receipt re-reads, never recalls: every count cited in it is
  re-read from the receipt file at use time (no stale receipts).

## 5. Inherited Infrastructure

The four inherited instruments govern every new crate and court; none is
re-invented by wave 5.

1. **dod.sh manifest entries.** Wave 5 seeds
   `docs/jira/v26.11.1/dod-manifest.toml` and runs the existing checker
   in alternate-manifest mode. The v26.10.31 manifest (21 entries; 20
   green + 1 advisory pending as of the parent wave-12 ratification
   commit `0f17b7e`) is the entry-grammar precedent: `id`, `class`,
   `expect`, `description`, `check`, `evidence` — expect semantics
   `pass|fail|pending|advisory`. Entry classes proposed for wave 5:
   `ladder` (KAT courts), `gate` (branchless, fuzz non-vacuity),
   `bench` (advisory — expect `advisory`, never `pass`), `coverage`
   (error paths), `stat` (dudect), `receipt` (90-wave-5-receipt.md
   exists and cites commands). The checker's exit codes (0 no red /
   1 red / 2 usage / 3 fail-closed) are inherited unchanged.
2. **mint_receipt.sh families.** The minter is fail-closed (nonzero
   command exit still writes a FAILED-marked receipt and exits nonzero),
   append-only (exit 73 on name reuse), digests stdout (blake3, sha256
   fallback), and captures HEAD + dirty flag. W5-0 adds the
   `RECEIPT_DIR` env override and template fallback (default
   byte-identical); everything else — families `w5gate-`, `w5kat-`,
   `w5fuzz-`, `w5dudect-`, `w5bench-`, `w5errpath-` — rides the
   existing machinery.
3. **ADR-19 CTQ amendment.** ADR-19 (22-adr-register.md, RATIFIED
   2026-10-31): any CTQ limit may be re-derived when proven impossible
   via the doc-48 pattern (ceiling math + measured distribution +
   architecture attribution + retained refutation record). Wave 5's
   numeric/statistical limits (dudect n-floor CTQ-PQC-03, bench
   iteration floors CTQ-PQC-04) are ADR-19-amendable with a receipt.
   Boundary: ADR-19 amends numeric limits only — conformance booleans
   (KAT byte-parity) and enumerable counts (error-path matrix) are spec
   gates, not limits, and are never amended by this route.
4. **P28-SIL v2.1.0.** `urn:avec:license:p28-sil:v2.1.0-sovereign`
   (LICENSE-P28-SIL.md line 3) is the sole governor; no SPDX identifiers
   in any new manifest, header, or doc (the parent repo's SPDX-strip
   commit `67c0ec1` is the standing precedent). The avec parent-repo
   license covers `crates/avec-pqc` as a workspace member — no per-crate
   license files, no workspace Cargo.toml license-field divergence. The
   liboqs scratch build in W5-A never enters the workspace, so its own
   license terms never touch avec distribution; the comparison receipt
   records liboqs's license for attribution only. Compliance mechanics:
   34-p28-sil-compliance.md (parent milestone).

## 6. Execution Order

W5-0 (infra) → waves 1-4 land → W5-D, W5-E (courts over real kernels) →
W5-B, W5-C (timing campaigns) → W5-F (final manifest sweep) → W5-A
(liboqs comparison, needs W5-B's harness) → W5-G (docs) → W5-H
(receipt). W5-F per-kernel joins may land earlier during waves 1-4; the
final sweep and the no-gap law are wave-5 owned. Every ordering step
ends in a minted receipt before the next begins.

## 7. Acceptance Criteria

| AC | Criterion | Verdict evidence |
|---|---|---|
| AC-1 | KAT courts green at wave-5 HEAD | court commands exit 0, receipt; non-negotiable |
| AC-2 | Benches advisory, gating nothing | FLOORS unchanged; no pass-expect bench dod row |
| AC-3 | dudect t-stat PASS per admitted kernel | verdict bin prints n and t; minted w5dudect-* |
| AC-4 | Fuzz courts pass, proven non-vacuous | courts exit 0; seeded-mutation receipt |
| AC-5 | Error paths: zero UNCOVERED (or deletion) | pqc_error_paths exit 0; matrix in receipt |
| AC-6 | Branchless sweep green, no-gap, zero-match | audit exit 0 full scope; control flips |
| AC-7 | FEATURE_MATRIX rows carry passing commands | matrix commands re-run to exit 0 |
| AC-8 | 90-wave-5-receipt.md landed, complete | file exists, cites every AC |

## 8. Falsifiers

- F-1 (AC-1): any KAT court exit != 0 at wave-5 HEAD — wave 5 FAILs, no
  advisory offset applies.
- F-2 (AC-2): a bench number appears as a dod manifest
  `expect = "pass"` row, or inside BENCH_FLOORS.json — advisory
  discipline broken.
- F-3 (AC-3): a PASS line lacking `n`/`t`, or a verdict bin that cannot
  print INCONCLUSIVE — the bin is defective, rerun with a fixed bin.
- F-4 (AC-4): a fuzz court still passing with its own seed list mutated
  into no-ops — the court is vacuous.
- F-5 (AC-6): a manifest row whose scope_pattern matches zero symbols —
  exit 3 under the wave-5 zero-match check.
- F-6 (AC-8): a receipt citing a count that differs from the cited
  receipt file on disk — stale receipt, wave-5 receipt re-issued.
- F-7 (W5-A): liboqs entering Cargo.toml, a path dep, or an include
  path — the scratch-only boundary is broken.

## 9. Standing

UNVERIFIED end to end. Nothing in this plan has executed; no bench has
run; no dudect verdict exists; no receipt is minted. The plan becomes
PARTIAL_ALIVE only when W5-0 receipts land, and ALIVE per court only on
executed commands with recorded exits. Bench numbers, once they exist,
stay advisory and machine-local forever.

## See Also

- [../waves/W5-baselines-hardening-prd-ard.md](../waves/W5-baselines-hardening-prd-ard.md)
  — the wave-5 PRD/ARD this plan executes.
- [t09-baseline-landscape-liboqs.md](t09-baseline-landscape-liboqs.md)
  — the landed survey this plan consumes (oracle + bench design).
- [../00-index.md](../00-index.md) — milestone charter and disciplines.
- [../../v26.10.31/22-adr-register.md](../../v26.10.31/22-adr-register.md)
  — ADR-14 remainder policy, ADR-19 CTQ amendment.
- [../../v26.10.31/81-error-path-coverage.md](../../v26.10.31/81-error-path-coverage.md)
  — the error-path sweep method.
- [../../v26.10.31/gate-branchless-manifest.md](../../v26.10.31/gate-branchless-manifest.md)
  — manifest-adjudicated gate method.
- [../../v26.10.31/34-p28-sil-compliance.md](../../v26.10.31/34-p28-sil-compliance.md)
  — license compliance mechanics.
