# AVEC v26.10.31 Milestone Index

Hub document for the v26.10.31 design-capture wave. This wave converts a
design conversation into milestone documentation; it is design work, not a
benchmarked release.

## Summary

AVEC v26.10.31 targets a branchless unification of three physical data
formats as a single substrate:

- Apache Arrow (columnar buffers, zero-copy layout)
- Apache Parquet DataPageV2 (encoded page structure)
- Git Packfile v2 (content-addressed object storage)

The unification is specified as: sub-BEAM, freestanding, zero-allocation,
Cyclomatic Complexity M=1, with compliant BLAKE3 tree hashing over the
content-addressed object graph.

Two companion theses ride the same milestone:

- **stardf reconstitution** — purrdf's semantic layer renamed and
  reconstituted via ggen from its RDF/ontology capital, mapping
  purrdf-process/-arrow/-sql onto stardf-* crates.
- **Chain economy thesis** — chain overhead analyzed against substrate
  equivalents (80/20), with Git PR mechanics as a universal agent
  boundary, POWL-v2 escrow notary flows, and time-cryptographic
  unforgeability (RTT distance bounding, blinded WASM challenge kernels).

## Wave Provenance

- 20-lane fan-out over the single canonical checkout at `/Users/sac/avec`,
  2026-10-30, one file per lane, coordinator owns integration.
- Source material: a Gemini-architected design session (design-capture,
  not implementation).
- **All numbers in this wave are design targets, NOT measured receipts.**
  No benchmark was run for this wave; any performance figure in lane files
  states an aim, not an observation.

## Document Index

| Lane | File | Hook |
|---|---|---|
| 00 | 00-index.md | This hub document |
| 01 | 01-stardf-reconstitution.md | purrdf to stardf naming + ggen-legacy reconstitution pipeline |
| 02 | 02-architecture-planes.md | purrdf logic plane over avec physical plane |
| 03 | 03-subsystem-mapping.md | purrdf-process/-arrow/-sql mapped onto stardf-* crates |
| 04 | 04-benchmark-contract.md | Performance thresholds as SHACL receipt contract |
| 05 | 05-kgc4d-stardf.md | stardf-4d (O, t, V, G) frame design |
| 06 | 06-stardf-git-odb.md | Packfile v2 as columnar object database |
| 07 | 07-chain-economy-8020.md | Chain overhead vs substrate equivalents |
| 08 | 08-pr-turnstile-llm-safety.md | Git PR as universal agent boundary |
| 09 | 09-stardf-hooks.md | purrdf-hooks to stardf-hooks reflex engine |
| 10 | 10-notary-escrow-powl.md | Robert's Rules via POWL v2, escrow notary |
| 11 | 11-ocel-entanglement.md | Bilateral OCEL v2 micro-window unforgeability |
| 12 | 12-distance-bounding-rtt.md | Notary RTT + processing-time trap |
| 13 | 13-time-crypto-prior-art.md | Brands-Chaum, PoET, VDF, Roughtime |
| 14 | 14-blinded-wasm.md | Stripped zero-import WASM challenge kernels |
| 15 | 15-arrow-microarch-probe.md | Arrow buffer as covert hardware assay surface |
| 16 | 16-ahdclt.md | Adversarial hyperdimensional cognitive load theory |
| 17 | 17-post-2028-computing.md | Working-backwards post-2028 computing thesis |
| 18 | 18-dflss-charter.md | DMADV charter + P28-SIL DFLSS audit |
| 18b | 18-m1-branchless-algebra.md | M=1 branchless algebra note (same-lane companion) |
| 19 | 19-ontology-ttl.md | Full v26.10.31 RDF/SHACL/ODRL ontology artifact |
| 19b | 19-press-release.md | Working-backwards press release for v26.10.31 |

## Wave 2: PRD/ARD & Engineering Formalization (20 docs)

Wave 2 (2026-10-30, 20-lane fan-out, same checkout) formalizes wave 1 into
requirements and engineering artifacts, then an implementation wave began
against the crates. Index:

| File | Hook |
|---|---|
| 20-prd.md | Product Requirements Document (PRD, FR-01..07, CTQ table) |
| 21-ard.md | Architectural Requirements Document (ARD, triad, M=1, Chatman Eq) |
| 22-adr-register.md | ADR-01..12 expanded register |
| 23-64b-quad-spec.md | 64-byte quad physical layout specification |
| 24-parquet-projection-spec.md | Level-free DataPageV2 projection specification |
| 25-git-packfile-integration-spec.md | Byte-level packfile/idx v2 integration spec |
| 26-blake3-engine-spec.md | BLAKE3 conformance + throughput engine spec |
| 27-branchless-kernel-abi.md | Predication primitives + disassembly audit gate |
| 28-beam-nif-handoff.md | Zero-copy NIF resource specification |
| 29-benchmark-harness-plan.md | Divan harness + receipt pipeline plan |
| 30-shacl-validation-suite.md | Negative/boundary SHACL corpus plan |
| 31-verification-ladder.md | Verification ladder + CI gates |
| 32-traceability-matrix.md | FR ↔ gate ↔ crate traceability |
| 33-risk-register.md | DFMEA + adversarial/risk triage |
| 34-p28-sil-compliance.md | P28-SIL operational compliance mechanics |
| 35-migration-v26.10.29.md | Gap-closure migration plan from current tree |
| 36-release-checklist.md | DMADV gate + landing checklist |
| 37-glossary.md | Terms of art across the series |
| 38-open-questions.md | OQ-01..22 UNKNOWN register |
| 39-wave-2-receipt.md | Wave-2 provenance receipt |

Implementation wave (I1-I20): same-day 20-lane fan-out owning crates/ and
scripts/ — quad layout pin, transpose + predicate kernels, level-free
Parquet writer/scan, new crates/avec-git (pack writer, idx ladder, git-fsck
court), BLAKE3 official vectors + zero-alloc + throughput, FFI/BEAM
contract tests, allocation counter in crates/audit, branchless audit gate
(script), e2e admission test, WASM notes, CLI machine report, Divan bench,
receipts pipeline, verify orchestrator. Statuses recorded in
verify-latest.md and receipts/ as those gates actually run.

Known reconciliation items: TTL measures `assetUnderManagement` while the
charter/PRD speak of annual gross revenue (34-p28-sil-compliance.md,
OPEN); 25 §3 pins a 65,536-objects-per-partition cap pending ladder-depth
re-derivation (OQ register); the plane shapes' bare `sh:hasValue` needs an
explicit `sh:minCount 1` presence decision (30-shacl-validation-suite.md).

Resolved since: the VALIDATOR chain is closed — gate-shacl → gate-shacl-v2
→ gate-shacl-v3 all ALIVE (`receipts/gate-shacl-v3.md`). The
AUM-vs-annual-gross-revenue measure reconciliation remains OPEN
(34-p28-sil-compliance.md).

Wave 7 docs landed after this table's files (48-55): see the Wave 7 Docs
table below.

## Wave 3: Implementation Receipt

See [40-wave-3-receipt.md](40-wave-3-receipt.md) — 20 implementation lanes
landed in crates/ and scripts/; coordinator re-ran every gate: workspace
check green, per-crate suites green (arrow 93, parquet 162, avec-git 26,
blake3 9, audit 5, beam 1-in-real-BEAM, integration e2e 2), machine report
6 PASS / 1 SKIP. Honest REDs: FR-04 git fsck court (stored bodies, zlib
missing) and the branchless audit gate (182 banned opcodes — true
baseline). Wave standing: PARTIAL_ALIVE. Nothing committed.

## Waves 4-5

Fix and ledger waves: see [41-wave-4-receipt.md](41-wave-4-receipt.md),
[43-wave-5-receipt.md](43-wave-5-receipt.md), and
[44-checklist-sweep-v2.md](44-checklist-sweep-v2.md) (chain continues via
46-checklist-sweep-v3.md to 54-checklist-sweep-v4.md).

## Wave 6: Closure Wave (20 lanes, one-per-message)

Wave 6 (2026-10-30) closed the remaining reds. Coordinator re-verified the
final state; full verdict in [47-milestone-verdict.md](47-milestone-verdict.md).

**Verify ladder: 10/10 gates PASS, run exit 0** (verify-latest.md) —
including the real-git fsck court, SHACL validator (conforms + non-vacuous
mutant), and the cli machine report.

**Defects closed this wave**: gage R&R ALIVE on quiet machine (CV ~0.8%
vs 1% gate); cfg-homology receipted (transpose over-slack FAIL recorded
honestly); divan2receipt lint-clean with both non-vacuity probes passing;
cli-verify probe fixed (10/10); beam bridge ALIVE (alloc-spy {0,0},
dtor exactly-once 64, pointer identity); branchless manifest codifies
ADR-14 remainder policy; workspace 0 warnings.

**CTQ-06 trajectory**: 0.43 → 1.18 → **1.85 GiB/s** (4-way→x2 NEON kernel,
message-load transpose, CV chaining; the 0.938 midpoint was a wave-3/4-only
reading, attributed to 41-wave-4-receipt.md, not the wave-6 close). Still
FAIL vs the 8.5 GiB/s LSL. **CTQ-05 PASS** (8.208µs vs 35µs USL per
47-milestone-verdict.md); CTQ-02 PASS (0 allocs); CTQ-04 FAILed one run at
21.9% vs provisional 15% pin (noise-sensitive, DFLSS ratification open);
CTQ-06 FAIL.

**Standing: PARTIAL_ALIVE.** FR-01..04, FR-07 PASS; FR-05/FR-06 PARTIAL at
wave-6 close (residual opcodes under ADR-14 policy; throughput below LSL).
Receipts at wave-6 close: 19+ gate receipts on disk (10+ ALIVE, 6+ FAILED
kept as non-vacuity corpus); the corpus has since grown to 27 (see Wave 7).
Nothing committed — working tree carries all four waves.

**Wave standing: UNVERIFIED.** This is design documentation only. No
benchmark, conformance run, or receipt-producing execution backs any claim
in lanes 01-19. Per the standing vocabulary:

- UNVERIFIED is the default and remains so until a lane's falsifier runs
  against an exact subject.
- ALIVE requires execution against the exact admitted subject with a
  command + exit receipt; none exists for this wave.
- Numbers labeled "design target" are aspirations; they must not be cited
  as measured results.

## Wave 7: Gate Closure Wave (2026-10-30/31)

See [53-wave-7-receipt.md](53-wave-7-receipt.md) and the closing integration
report [55-final-integration-report.md](55-final-integration-report.md).

- **FR-05 closed** via the manifest-aware branchless gate:
  [gate-branchless-manifest.md](gate-branchless-manifest.md) ALIVE with a
  negative control.
- **C_pk study receipted FAILED** (`receipts/gate-cpk-study.md`, exit 1):
  CTQ-06 Cpl -254.8 (breach); CTQ-05 Cpu(overall) 9.63 / Cpu(within) 95.5
  PASS; CTQ-04 Cpu 4.33 vs target 1.67 PASS.
- **Perf guard landed**: `receipts/gate-perf-guard.md` re-minted ALIVE at
  20:07 PDT (exit 0; gage cv 0.88%; CTQ-05 8.208µs under the 30µs canary).
- **LSL re-derivation landed** ([48-lsl-rederivation.md](48-lsl-rederivation.md)):
  8.5 GiB/s/core not physically attainable; proposed LSL 1.0 / Target 1.5 /
  USL 2.5 GiB/s; PRD amendment PROPOSED in 20-prd.md.
- **Parquet decode baseline**: measured 65.6 GiB/s min vs the legacy 74
  claim (`receipts/parquet-throughput-baseline.md`, standing PARTIAL).
- **Audits**: release notes (51-release-notes.md), courts audit
  (52-courts-audit.md — 21 LIVE / 3 UNPROVEN), receipts audit
  (50-receipts-audit.md — 22 seen: 14 ALIVE / 8 FAILED), CTQ-04 variance
  study (49-ctq04-variance-study.md — excursions attributed to environment;
  recommends gage-conditioned p99/p50).
- **Receipts corpus now 36** — 25 ALIVE / 10 FAILED / 1 PARTIAL per
  64-receipts-inventory-final.md (supersedes the wave-7 count of 27 and
  50's 14/8-of-22 snapshot).

## Wave 7 Docs

| File | Hook |
|---|---|
| 48-lsl-rederivation.md | LSL physically unattainable; LSL 1.0 / Target 1.5 / USL 2.5 proposed |
| 49-ctq04-variance-study.md | CTQ-04 excursions are environment; gage-conditioned p99/p50 |
| 50-receipts-audit.md | 22 receipts audited: 14 ALIVE / 8 FAILED |
| 51-release-notes.md | v26.10.31 release notes |
| 52-courts-audit.md | Court inventory: 21 LIVE / 3 UNPROVEN |
| 53-wave-7-receipt.md | Wave-7 provenance receipt |
| 54-checklist-sweep-v4.md | Fourth checklist sweep |
| 55-final-integration-report.md | Seven-wave closing integration report |

## Wave 8: Closure Completion (20 lanes)

Wave 8 (2026-10-30) closed the last finish-list items. Receipt:
[56-wave-8-receipt.md](56-wave-8-receipt.md); verdict:
[58-final-verdict-v2.md](58-final-verdict-v2.md); sweep:
[57-checklist-sweep-v5.md](57-checklist-sweep-v5.md).

**Landed**: beam five-op surface (ops_scan open/scan zero-copy with
pointer identity + 1000-scan alloc-delta {0,0}; ops_persist
flush/seal/receipt - FR-07 gap closed); CTQ-04 p99/p50 protocol
implemented per study 49 (PASS at 1.134 vs 1.30, three runs); FR-05
manifest extended 2 to 7 kernels (gate-branchless-manifest-v2 ALIVE);
cfg_homology --mutant court (ALIVE, flips); zero_alloc positive control;
scan.rs slice cache (accessor scan 0.595 to 8.1 GiB/s, ~13.7x); quiet
C_pk re-run (CTQ-06 sole red); git perf receipt (16.6 MiB/s pack write,
3.49M lookups/s); kernel g4x2 interleave REFUTED with assembly evidence
(spill-bound - reverted; state-spill is the real ceiling); PRD FR status
column; docs 29/30/05/09/10/11/13/20/21/45/47/36/39 drift applied.

**Final state**: verify ladder 10/10 PASS exit 0 (coordinator re-run);
battery 397 passed / 0 failed / 3 ignored; CTQ-02/04/05 PASS, CTQ-06
FAIL (1.85 GiB/s vs unratified 8.5 LSL - amendment PROPOSED, doc 48);
C_pk: CTQ-06 sole breach. Standing: PARTIAL_ALIVE. Open ledger: CTQ-06
ratification, CTQ-04 protocol ratification, commit decision (operator).

**Final gate table** — verify ladder 10/10 PASS, exit 0
([verify-latest.md](verify-latest.md), generated 2026-10-31T04:06:51Z =
21:06 PDT 2026-10-30, HEAD `ead4032`). Receipt re-read fresh at this
edit (<30 min old); the orchestrator was not re-run.

| Gate | Status | Exit |
|---|---|---|
| check-workspace | PASS | 0 |
| test-avec-arrow | PASS | 0 |
| test-avec-parquet | PASS | 0 |
| test-avec-git | PASS | 0 |
| test-avec-blake3 | PASS | 0 |
| test-integration | PASS | 0 |
| test-avec-beam | PASS | 0 |
| audit-branchless | PASS | 0 |
| cli-verify | PASS | 0 |
| test-audit | PASS | 0 |

**Wave-8 gate receipts** (each verified on disk at this edit):

- [gate-branchless-manifest-v2.md](receipts/gate-branchless-manifest-v2.md)
  — ALIVE: every hit adjudicated MANIFEST-OK, zero unexplained hits;
  positive control fired.
- [gate-cfg-homology-v2.md](receipts/gate-cfg-homology-v2.md) — ALIVE:
  `--mutant` court flips (clean PASS, branchy FAIL); non-vacuous.
- [gate-cpk-study-v2.md](receipts/gate-cpk-study-v2.md) — **FAILED**
  (exit 1): CTQ-06 the sole breach; CTQ-04 and CTQ-05 PASS.
- [gate-git-perf-baseline.md](receipts/gate-git-perf-baseline.md) —
  ALIVE (informational-by-design): 16.6 MiB/s pack write (min-of-3),
  3.49M idx lookups/s.

**FR-07 closed**: ops_scan + ops_persist landed and registered in
`crates/avec-beam/src/lib.rs` — the five-op BEAM surface is complete:
avec_open, avec_scan (zero-copy), avec_flush_to_parquet,
avec_seal_to_packfile, avec_receipt_query.

**CTQ-04 protocol landed**: the gage-conditioned p99/p50 statistic from
[49-ctq04-variance-study.md](49-ctq04-variance-study.md) is implemented
and evaluates PASS at 1.134 vs the provisional 1.30 USL (per
[58-final-verdict-v2.md](58-final-verdict-v2.md)); ratification is
still open.

**Operator decision list**: owned by the wave-9 artifacts
[59-ratification-package.md](59-ratification-package.md) (on disk,
assembled for decision — nothing ratified) and 62-wave-9-receipt.md
(named, still pending at this edit).

## Wave 9 Docs

| File | Hook |
|---|---|
| 60-release-readiness.md | Terminal readiness checklist: engineering boxes closed, D1-D3 operator-open |
| 63-test-census.md | Per-crate test census: 58 result lines, 661 passed / 0 failed / 2 ignored |
| 64-receipts-inventory-final.md | Receipts corpus final inventory: 36 receipts (25 ALIVE / 10 FAILED / 1 PARTIAL) |
| 65-coordinator-handoff.md | One-page coordinator handoff for the integration commit |

## Wave 10: DX/QoL (20 lanes + 3 gap lanes)

Wave 10 (2026-10-30) turned the finished engineering into a self-diagnosing
system. Receipt: [68-wave-10-receipt.md](68-wave-10-receipt.md); DoD run and
verdict: [66-dod-receipt.md](66-dod-receipt.md) (GATES GREEN, 3 PENDING);
measured DX scorecard: [67-dx-audit.md](67-dx-audit.md) (11/12).

| Item | Landing pointer (verified on disk at this edit) |
|---|---|
| dod.sh + 21-entry manifest + selftest court | scripts/dod.sh · dod-manifest.toml |
| doctor / status bins (--json) | crates/cli/src/bin/doctor.rs · status.rs |
| justfile (8 recipes) | justfile |
| onboarding docs chain | GETTING_STARTED · TROUBLESHOOTING · diataxis/ |
| feature / operator surfaces | FEATURE_MATRIX · OPERATOR_DECISIONS |
| README refresh (honest CTQ-06 red) | README.md |
| CI mirror (3 local-first jobs) | .github/workflows/ci.yml |
| e2e demo + C99 example | examples/sovereign-e2e · examples/c99_consumer |
| hygiene + error ergonomics | 71-hygiene-audit.md · 70-error-ergonomics-audit.md |
| nightly perf relaunch | perf/ (two logs) |

## Wave 10 Docs

| File | Hook |
|---|---|
| [66-dod-receipt.md](66-dod-receipt.md) | DoD run + verdict: GATES GREEN, 3 PENDING (green=18) |
| [67-dx-audit.md](67-dx-audit.md) | Measured DX scorecard: 11/12 |
| [68-wave-10-receipt.md](68-wave-10-receipt.md) | Wave-10 provenance receipt (20 lanes + G1-G3) |
| [69-validation-receipt.md](69-validation-receipt.md) | 17-lane thesis-court sweep |
| [70-error-ergonomics-audit.md](70-error-ergonomics-audit.md) | 36 error variants → what/why/fix |
| [71-hygiene-audit.md](71-hygiene-audit.md) | Clean-clone residue: empty |

## Wave 11: Convergence (5 lanes C1-C5)

Wave 11 (2026-10-30/31) closed the wave-11 plan's gap items and converged
the receipt surface. Receipt:
[75-wave-11-receipt.md](75-wave-11-receipt.md).

- **G1-G3 gap closures** (lane rows in the wave-10 receipt): e2e git
  stage wired with real-git fsck in-loop
  ([gate-e2e-integration-v3](receipts/gate-e2e-integration-v3.md),
  admission_e2e 2/2); receipts v3 minted
  ([gate-git-tests-v3](receipts/gate-git-tests-v3.md),
  [gate-cpk-study-v3](receipts/gate-cpk-study-v3.md) FAILED exit 1 —
  the standing CTQ-06 red, kept); untranspose manifest entry + ADR-17
  NEON-transpose refutation
  ([gate-branchless-manifest-v4](receipts/gate-branchless-manifest-v4.md),
  [ADR-17](22-adr-register.md)).
- **Independent validation**:
  [69-validation-receipt.md](69-validation-receipt.md) — 15 VALIDATED /
  1 PARTIAL / 1 REFUTED-recorded.
- **Convergence docs**: [72-census-v2.md](72-census-v2.md) (fresh ladder
  10/10 PASS; census 60 lines · 661 passed · 0 failed · 2 ignored),
  [73-slack-adjudication.md](73-slack-adjudication.md) (19/19 branch
  sites adjudicated per ADR-14; slack 4→20 proposed),
  [74-attestation-mapping.md](74-attestation-mapping.md) (SHA-1↔BLAKE3
  map PINNED; settles OQ-06; per-chunk court open).
- **Terminal state**: `bash scripts/dod.sh` → GATES GREEN, 3 PENDING,
  exit 0 (re-run this window). Operator items remain PENDING-OPERATOR:
  ratification (wave-12), commit (wave-13).

## Open Questions

- Does the Arrow/Parquet-DataPageV2/Packfile-v2 unification admit a single
  buffer layout, or do Parquet page encodings force a second representation?
- Can Cyclomatic Complexity M=1 hold across the BLAKE3 tree hashing paths,
  or does branchless hashing break down at the boundary-check seams?
- What is the minimal ggen reconstitution path for stardf that reuses
  purrdf capital without forking the ontology graph?
- Which chain-economy claims (80/20 split) survive contact with the
  substrate-equivalent measurements in lane 07 once benchmarks run?
- Is the POWL-v2 escrow notary expressible without the OCEL
  micro-window assumptions in lane 11, or are they load-bearing?
- What falsifier corpus qualifies the blinded WASM challenge kernels
  (lane 14) as actually stripped and zero-import?

See Also: [../v26.10.29/](../v26.10.29/) (prior milestone) and the repo
[README](../../../README.md).
