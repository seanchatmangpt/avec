# Requirements Traceability Matrix: v26.10.31

Bidirectional mapping: PRD functional requirements (20-prd.md) → ARD sections (21-ard.md) →
design specs → verification gates → owning crates. Status is UNVERIFIED until the named gate
runs with a receipt on the exact subject.

Wave-6 refresh (2026-10-30, lane 12 of 20). Fresh gate runs this session:
avec-arrow lib 93/0; fsck court 2/0 (real git); validate_ontology.sh PASS (anti-vacuity
mutant rejected). Verify orchestrator (W6): 9 PASS / 1 SKIP; the SKIP (cli-verify) was a
probe bug, now fixed — see cli-verify note below. FR-04 fully green (fsck 2/2 +
git_interop 3/3 + real-git cli row). CTQ-05 flipped PASS (12.875 µs vs 35 µs USL,
small-batch hash path). CTQ-02 zero-alloc PASS (0 events). CTQ-04 provisional pin PASS.
FR-07 beam bridge green again: alloc_spy delta {0,0}, dtor exactly-once 64, pointer
identity — receipt gate-beam-bridge-v2 ALIVE. SHACL ALIVE (gate-shacl-v2). CTQ-06 still
FAIL (1.18 GiB/s vs 8.5 LSL; wave-6 kernel lanes in flight). gage + homology receipts
pending.

### cli-verify SKIP mystery — RESOLVED (wave-6, lane 12)

The orchestrator probed `target/debug/{avec-cli,avec,cli}` for a `verify --help`
subcommand. The real artifact is the standalone bin `verify_v26`
(crates/cli/src/bin/verify_v26.rs -> target/debug/verify_v26), which runs the
FR-01..07 battery itself and prints its own JSON — it takes no subcommand. Probe
fixed in scripts/verify_v26.10.31.sh: now checks target/debug/verify_v26 first and
runs it bare. Correct invocation:
`cargo build -p avec-cli --bin verify_v26 && target/debug/verify_v26`
(emits JSON, exit 0 on all-pass / 1 on any FAIL).

## Forward Mapping: FR → ARD → Design → Gate → Crate

```
| FR    | Requirement            | ARD Section      | Design Doc                     | Verification Gate                  | Crate(s)                        | Status (2026-10-30 W6) |
|-------|------------------------|------------------|--------------------------------|------------------------------------|---------------------------------|------------|
| FR-01 | Columnar Alignment     | 21-ard.md §2.1   | 23-64b-quad-spec.md            | Compile-time asserts + bench       | crates/avec-arrow               | PASS-gate-exists (lib 93/0 re-run W6; gate-e2e-integration-v2 ALIVE) |
| FR-02 | Non-Nullable Layout    | 21-ard.md §2.1/2.2 | 23-64b-quad-spec.md + 24-parquet-projection-spec.md | Sentinel + bitmap-absence tests | crates/avec-arrow, crates/avec-parquet | PASS-gate-exists (arrow 93/0 W6; parquet green in W6 verify, gate-e2e-integration-v2 ALIVE) |
| FR-03 | Parquet Projection     | 21-ard.md §2.2   | 24-parquet-projection-spec.md     | Isomorphism test + decode bench    | crates/avec-parquet             | PASS-gate-exists (rle fix W5; test-avec-parquet PASS W6 verify) |
| FR-04 | Packfile CAS           | 21-ard.md §2.3   | 25-git-packfile-integration-spec.md | git fsck court (crates/avec-git/tests/fsck_court.rs) | crates/avec-git | PASS (FULLY GREEN W6: fsck 2/2 re-run W6; git_interop 3/3; gate-git-tests-v2 ALIVE; real-git cli row in report) |
| FR-05 | Unrolled Search Ladder | 21-ard.md §3.3   | 27-branchless-kernel-abi.md    | audit_branchless.sh (objdump branch audit + positive control) | scripts/audit_branchless.sh over avec-arrow hot symbols | PARTIAL (audit-branchless selftest PASS W6 verify + gate-branchless-selftest-v2 ALIVE; residual banned-opcode census in hot symbols — owner: transpose/predicates-branchless lanes) |
| FR-05 | Unrolled Search Ladder | (cont.)          | —                              | —                                  | —                               | (gate infra green; op-code reduction residual tracked in red-list R2) |
| FR-06 | Parallel BLAKE3 Tree   | 21-ard.md §4.1   | 26-blake3-engine-spec.md       | Official vectors + throughput bench | crates/avec-blake3              | PARTIAL (lib green: gate-blake3-tests-v2 ALIVE; CTQ-06 FAIL 1.18 GiB/s vs 8.5 LSL; wave-6 kernel lanes in flight) |
| FR-07 | Sub-BEAM Handoff       | 21-ard.md §1.1/§28 | 28-beam-nif-handoff.md       | Pointer-identity test (crates/avec-beam/tests/bridge_contract.rs, real BEAM escript) | crates/avec-beam, crates/avec-ffi | PASS (GREEN AGAIN W6: alloc_spy delta {0,0}, dtor exactly-once 64, pointer identity — gate-beam-bridge-v2 ALIVE) |
| FR-07 | Sub-BEAM: alloc courts | (cont.)          | 28-beam-nif-handoff.md         | ffi alloc courts + zero_alloc court | crates/avec-ffi                | PASS (zero_alloc ALIVE receipt gate-blake3-zero-alloc; CTQ-02 W6: 0 alloc events) |
```

Wave-6 diffs vs wave-5: FR-04 GREEN→FULLY GREEN (fsck 2/2 + git_interop 3/3 + real-git
cli row); FR-07 RED→PASS (beam bridge green again, gate-beam-bridge-v2); FR-05 GATE-RED→
PARTIAL (gate infra green via selftest court, opcode residual open); FR-06 vectors green
(CTQ-06 throughput still FAIL, 0.43→1.18 GiB/s improvement); CTQ-05 38→12.875 µs PASS;
CTQ-02 0 alloc events; CTQ-04 provisional pin PASS. cli-verify SKIP was an orchestrator
probe bug (fixed W6, see note above), not a missing artifact.

## Reverse Mapping: Gate → FR

```
| Gate                               | Traces To  | Status (2026-10-30 W6) |
|------------------------------------|------------|------------|
| Compile-time asserts + bench       | FR-01      | PASS-gate-exists (lib 93/0 re-run W6) |
| Sentinel + bitmap-absence tests    | FR-02      | PASS-gate-exists (arrow 93/0 W6; parquet green in W6 verify) |
| Isomorphism test + decode bench    | FR-03      | PASS-gate-exists (test-avec-parquet PASS, W6 verify) |
| git fsck court                     | against real git | PASS (2/2 W6 re-run + git_interop 3/3 + real-git cli row; gate-git-tests-v2 ALIVE) |
| objdump branch audit               | FR-05      | PARTIAL (selftest court PASS W6; banned-opcode residual open, R2) |
| Official vectors + throughput bench | FR-06     | PARTIAL (vectors green gate-blake3-tests-v2; CTQ-06 1.18 GiB/s vs 8.5 LSL) |
| Pointer-identity test              | FR-07      | PASS (gate-beam-bridge-v2 ALIVE: spy {0,0}, dtor 64 exactly-once, pointer identity) |
```

## CTQ → Bench Mapping

```
| CTQ                          | LSL   | Target | USL            | Bench (crates/bench)          | Owning Lane Doc              | Status (2026-10-30 W6) |
|------------------------------|-------|--------|----------------|-------------------------------|------------------------------|------------|
| M                            | 1.0   | 1.0    | 1.0            | branch/misprediction census   | 27-branchless-kernel-abi.md  | PARTIAL (selftest court PASS W6; opcode residual R2) |
| Hot-path heap bytes          | 0     | 0      | 0              | alloc_counter (crates/audit) + zero_alloc court | 28-beam-nif-handoff.md | PASS (CTQ-02 W6: 0 alloc events; gate-blake3-zero-alloc ALIVE) |
| mmap decode (GiB/s)          | 70.0  | 74.8   | Saturated bus  | decode bench (parquet_bench, v26_10_31 Divan) | 23-64b-quad-spec.md    | PASS-gate-exists (~98 GB/s aggregate roundtrip, W3, as-of-date) |
| Branch misprediction rate    | 0.000 | 0.000  | 0.000          | perf counters / census        | 27-branchless-kernel-abi.md  | PARTIAL (same residual as M, R2) |
| Settlement latency (µs)      | —     | 31.4   | 35.0           | admission_e2e (crates/integration/tests/admission_e2e.rs) | 28-beam-nif-handoff.md | PASS (CTQ-05 W6: 12.875 µs vs 35 µs USL via small-batch hash path; gate-e2e-integration-v2 ALIVE) |
| BLAKE3 throughput (GiB/s)    | 8.5   | 10.2   | Mem-ctrl max   | blake3_throughput bench       | 26-blake3-engine-spec.md     | FAIL (CTQ-06: 1.18 GiB/s vs 8.5 LSL; 0.43→1.18 W6; kernel lanes in flight) |
| C_pk                         | 1.67  | 1.84   | —              | ctq_report v2 + gage_rr       | 04-benchmark-contract.md     | PROVISIONAL PIN PASS (CTQ-04); superseded by p99/p50 protocol, wave-8; full study awaits gage quiet-machine receipt (R3) |
```

Bench names resolve to on-disk artifacts: bins in crates/bench/src/bin/ plus Divan suite
`benches/v26_10_31.rs`. A CTQ whose gate is RED is stated RED, not rounded up.

## Coverage v3 (wave-6, lane 12)

### Gate inventory v3 (scripts/ + bench bins, one-line status)

```
| Artifact                          | Status (W6) |
|-----------------------------------|-------------|
| scripts/validate_ontology.sh      | PASS — SHACL Conforms: True + anti-vacuity mutant rejected (re-run W6) |
| scripts/verify_v26.10.31.sh       | ALIVE — W6 run 9 PASS / 1 SKIP; cli-verify probe bug fixed W6 (next run: 10/0) |
| scripts/audit_branchless.sh       | PARTIAL — selftest court PASS (gate-branchless-selftest-v2); opcode residual R2 |
| scripts/branch_audit.sh/.py       | SUPPORT — underlying census for audit_branchless |
| scripts/bench_gate.py             | SUPPORT — bench admission helper; no standalone receipt |
| scripts/timing_gate.py            | SUPPORT — latency fence helper; feeds admission_e2e court |
| scripts/symbol_gate.py            | SUPPORT — symbol-level audit input |
| scripts/cfg_homology.sh           | PENDING — homology receipt not yet minted (R4) |
| scripts/footprint.sh              | SUPPORT — wasm-footprint driver; wasm 16/16 per W4 |
| scripts/c99_smoke.sh              | SUPPORT — C99 ABI smoke; no receipt this wave |
| scripts/mint_receipt.sh           | ALIVE — receipt minter backing docs/.../receipts/ |
| scripts/divan2receipt.py          | ALIVE — Divan output -> receipt converter, landed W5/W6 |
| scripts/audit_branchless_cases/   | SUPPORT — positive/negative courts for audit_branchless |
| scripts/release_check.py          | SUPPORT — release checklist driver |
| gate3_cdata.py / gate3_schema.py  | SUPPORT — gate-3 schema/cdata checks |
| crates/bench bins (17)            | PRESENT — parquet/page/packing/dict/bitslice/byteslice/
|                                   |   arrow_accessors/const_time/neon_ab/packed_filter/digest/
|                                   |   blake3_profile/blake3_throughput/phase_change/
|                                   |   ctq_report/gage_rr + Divan suite benches/ |
| ctq_report.rs                     | ALIVE — v2 landed; receipt gate-ctq-report FAILED (CTQ-06-driven) |
| gage_rr.rs                        | PENDING — quiet-machine receipt awaited (R3) |
| blake3_throughput.rs              | FAIL — 1.18 GiB/s vs 8.5 LSL (gate-blake3-throughput FAILED) |
| phase_change_bench.rs             | ALIVE — receipts in docs/PHASE_CHANGE_RECEIPTS.md |
| crates/cli/src/bin/verify_v26.rs  | ALIVE — standalone verify bin; probe fixed in orchestrator W6 |
```

### Receipt inventory v3 (docs/jira/v26.10.31/receipts/, by Standing)

```
| Standing | Receipts |
|----------|----------|
| ALIVE (10)  | gate-arrow-tests, gate-beam-bridge-v2, gate-blake3-tests-v2,
|             | gate-blake3-zero-alloc, gate-branchless-selftest-v2,
|             | gate-cli-machine-report, gate-e2e-integration-v2, gate-git-tests-v2,
|             | gate-shacl-v2, demo-cargo-version |
| FAILED (7)  | demo-fail (intentional demo), gate-shacl (superseded by -v2),
|             | gate-git-tests (superseded by -v2), gate-workspace-check (superseded
|             | by -v2), gate-blake3-throughput (CTQ-06 1.18 vs 8.5 LSL),
|             | gate-ctq-report (CTQ-06-driven FAIL), gate-divan-ctq (CTQ-06-driven) |
| Template    | template.md, README.md (meta, not receipts) |
```

The three v2 re-mints landed W6 (git-tests-v2, workspace-check-v2, beam-bridge-v2),
clearing the W5 stale-FAILED note. Remaining FAILED receipts are all CTQ-06-driven;
they flip when the wave-6 kernel lane closes throughput.

### Red-list v3 (still red + owner)

```
| #  | Red item                                        | Owner                          |
|----|-------------------------------------------------|--------------------------------|
| R1 | CTQ-06 BLAKE3 1.18 GiB/s vs 8.5 LSL             | wave-6 blake3 kernel lanes     |
| R2 | FR-05 banned-opcode residual in hot symbols     | transpose/predicates-branchless lanes |
| R3 | Gage R&R: quiet-machine receipt pending         | gage_rr lane                   |
| R4 | cfg_homology receipt pending                    | homology-proxy lane            |
| R5 | gate-ctq-report / gate-divan-ctq FAILED (CTQ-06)| flips with R1                  |
| R6 | verify rerun for cli-verify un-SKIP (probe fixed,| verify orchestrator owner     |
|    | needs one more run to record 10/0)              |                                |
```

Closed since W5: settlement latency (CTQ-05 PASS), FR-07 beam bridge, FR-04 fully
green, SHACL (re-minted -v2), cli-verify probe bug (fixed this lane).

### Red-list v4 deltas (wave 7)

Red-list v3 above is a wave-6 snapshot. Wave-7 deltas:

Closed in wave 7:

- FR-05 branchless gate: manifest-aware adjudication landed
  (`scripts/branchless_manifest.toml`); receipt gate-branchless-manifest.md
  ALIVE with negative control (expected_max_count 2→0 must exit 2). R2 closes
  at the gate level; full closure still wants the ratified manifest with zero
  unexplained hits.
- SHACL: gate-shacl-v3 ALIVE (exit 0), superseding -v2.
- cli-verify: probe fix verified — the verify orchestrator run is 10/10 (see
  54-checklist-sweep-v4.md "Verify 10/10 PASS"); R6 closed.

Still open, corrected:

- C_pk: receipted FAILED, not green — gate-cpk-study exit 1 (CTQ-06 breach);
  the W6 "PROVISIONAL PIN PASS" row is superseded by this receipt.
- perf-guard: first mint was SKIP-NOISY (noisy host); re-minted — current
  receipt gate-perf-guard.md is ALIVE (exit 0, 2026-10-31T03:07Z).
- cfg_homology: receipted FAILED — gate ALIVE, verdict FAILED (exit 1);
  transpose +17 emitted branches vs slack 4 (receipts/gate-cfg-homology.md).

New reds (wave 7):

- parquet typed-accessor gap: ~110x vs baseline; per-row Thrift re-parse at
  scan.rs:535; fix = slice caching in scan.rs. Owner: parquet scan lane.
- wave-7 kernel premise refutation: see 48 (kernel-lane premise doc); R1's
  "kernel lanes will close CTQ-06" premise is itself refuted — see 48.

Receipt inventory additions (all under docs/jira/v26.10.31/receipts/):
gate-cpk-study (FAILED exit 1), gate-shacl-v3 (ALIVE), gate-perf-guard
(ALIVE), gate-branchless-manifest (ALIVE, jira dir top level), and
parquet-throughput-baseline (PARTIAL — measured under load sharing; claim
NOT confirmed at 74 GiB/s).

### Red-list v5 (wave 8, 2026-10-31, lane 18 of 20)

Fresh gates this session (all exit 0): cargo test -p avec-arrow --lib 93/0;
cargo test -p avec-git --test fsck_court 2/2; validate_ontology.sh PASS
(anti-vacuity mutant rejected, exit 0). Timestamped 2026-10-31.

Resolved since v4 (wave-7 reds re-graded):

- FR-05 census: gate-branchless-manifest-v2.md DOES NOT EXIST. The only
  manifest receipt is wave-7 v1 (docs/jira/v26.10.31/gate-branchless-manifest.md,
  ALIVE). FR-05 census stays OPEN at v1 scope (transpose|predicates); default
  SCOPE_RE over-captures (`release_struct_schema`, `perfect_hash`) and is
  documented as out-of-scope, not closed. Re-grade: NOT RESOLVED in wave 8.
- Courts UNPROVEN red: zero_alloc positive control for cfg_homology DOES NOT
  EXIST (cfg_homology_cases/ has only branchy.c + clean.c, no alloc case);
  gate-cfg-homology-v2 receipt DOES NOT EXIST (v1 FAILED, exit 1, transpose
  +17 branches vs slack 4). Courts red stays OPEN, unchanged from v4.
- Parquet accessor gap: slice-cache MECHANISM IS LANDED in scan.rs (Thrift
  page header parsed at most once per (row_group, column); cache hit = no
  Thrift, stored bounds only; scan.rs:535 region now cached resolve path).
  But accessor_perf_court.rs DOES NOT EXIST and no post-fix re-measure
  receipt refutes the ~110x claim. Re-grade: PARTIALLY RESOLVED — fix
  landed, proof missing. Owner: parquet scan lane.

Carried opens (state re-checked):

- CTQ-06 + PROPOSED amendment: unchanged. No new blake3 throughput receipt;
  gate-blake3-throughput FAILED (1.18 vs 8.5 GiB/s LSL) still current.
- CTQ-04: p99 protocol IS LANDED in ctq_report.rs. Current logic: mini-gage
  CV > 2% -> SKIP-NOISY (load gating); quiet path gates on p99/p50 vs USL
  1.30 with stdev/mean informational (retired 15% pin). SKIP-NOISY is now
  the conditioned first branch, not the verdict; quiet-machine p99 run
  still awaited. Re-grade: protocol closed, quiet-machine receipt OPEN.
- Beam five-op surface: ops_scan.rs and ops_persist.rs DO NOT EXIST in
  crates/avec-beam/src/ (contents: alloc_spy, dtor_stats, lib, pointer,
  resbin, resources, shm, spsc_ring). FR-07's gap list unchanged — the
  five-op surface is unbuilt; bridge (gate-beam-bridge-v2) covers scan/
  persist in NIF terms only. FR-07 gap list re-graded UNCHANGED.
- Commit decision: still open, above lane authority.

Gate inventory v5 additions (all docs/jira/v26.10.31/, receipts/ unless
noted; statuses re-read from receipts this session):

```
| Receipt                          | Status   | Note                             |
|----------------------------------|----------|----------------------------------|
| gate-branchless-manifest-v2      | MISSING  | does not exist; v1 ALIVE (jira/) |
| gate-cfg-homology-v2             | MISSING  | v1 FAILED (exit 1, +17 branches) |
| gate-cpk-study-v2                | FAILED   | exit 1, 2026-10-31T03:39Z (CTQ-06)|
| gate-git-perf-baseline           | MISSING  | does not exist                   |
| gate-e2e-ctq05                   | ALIVE    | exit 0, admission_e2e, 02:29Z    |
| parquet-throughput-baseline      | PARTIAL  | load-shared host; NOT 74 GiB/s   |
| gate-shacl-v3                    | ALIVE    | exit 0 (superseded -v2)          |
| gate-perf-guard                  | ALIVE    | exit 0, 2026-10-31T03:07Z        |
```

Standing: PARTIAL_ALIVE (wave-8 refresh, 2026-10-31, lane 18 of 20).
Grounded this session only in the three cheap gates above; all receipt
statuses are as-read-from-disk 2026-10-31.

### Red-list v6 (wave 9, 2026-10-31, lane 5 of 20)

Fresh runs this session (all exit 0): cargo test -p avec-git 48/0/1
ignored (43 lib + 2 fsck + 3 git_interop); cargo test -p avec-parquet
192/0 default AND 204/0 --features std (both configs); cargo test -p
avec-beam exit 0 (9 lib + 1 real-BEAM bridge contract); cargo test -p
avec-blake3 --test zero_alloc 5/0.

Dropped since v5 (fixed, verified on disk):

- Parquet accessor gap: CLOSED. Resolved-slice cache landed in scan.rs
  (Thrift page header parsed once per (row_group, column), scan.rs:504
  region); accessor_perf_court.rs on disk; measured 0.595 -> 8.1 GiB/s,
  ~13.7x (00-index wave-8 block). Court green this session (std config).
- Scan regression: CLOSED. accessor_perf_court.rs line 1 is now
  `#![cfg(feature = "std")]` — the wave-8 row-3' default-config
  E0432/E0433 compile-abort is gone; default 192/0 and std 204/0 both
  exit 0 this session. Both configs collect and pass.
- FR-05 census: CLOSED at gate level. gate-branchless-manifest-v2 ALIVE
  (exit 0, 2026-10-31T03:46Z) over a 7-kernel manifest; positive control
  FIRED. Engineering residuals carried below (regex transport, collision
  reclaim).
- Courts-unproven 3 -> 0. zero_alloc control: PROVEN —
  positive_control_counter_is_live in crates/avec-blake3/tests/
  zero_alloc.rs (5/5 this session; counter proven live, not vacuous).
  Homology mutant: PROVEN — gate-cfg-homology-v2 ALIVE (exit 0, 03:41Z),
  "MUTANT COURT FLIPPED: clean PASS && branchy FAIL". git_interop B:
  PROVEN — court B hard PASS this session (git 2.51.2 fsck --full +
  verify-pack accept the avec-git pack+idx; git_interop 3/3).
- FR-07 five-op surface: CLOSED. ops_scan.rs (ops 1-2, avec_open/
  avec_scan zero-copy views) + ops_persist.rs (ops 3-5, flush/seal/
  receipt_query) on disk; avec-beam exit 0 this session incl. the
  real-BEAM bridge contract.
- CTQ-04 protocol: CLOSED at protocol layer. p99/p50 per 49 Option b
  implemented; retired_pin recorded in ctq_report.rs JSON provenance;
  PASS 1.134 vs USL 1.30 ("three runs", 00-index; 58-final-verdict-v2
  section 1). Correction: the wave-9 brief's 1.176 upper endpoint has no
  disk grounding (no hit in any jira md or receipt); 1.134 is the only
  grounded value and is what stands.

Carried OPERATOR-ONLY (no lane can close):

- CTQ-06 LSL ratification: LSL_RATIFIED = None (ctq_report.rs:44).
  Proposed LSL 1.0 / Target 1.5 / USL 2.5 GiB/s/core per 48 (lines
  75-78, Case-A basis); measured 1.85-1.86 stable (48 wave-8 appendix:
  e2e plateau 1.857-1.871 GiB/s, kernel ceiling 1.36 GiB/s). g4x2
  interleave refuted with assembly evidence; state-spill is the real
  ceiling (00-index wave-8 block; measurement appendix in 48). FAILED
  throughput receipts stand: gate-blake3-throughput (1.18 vs unratified
  8.5 LSL); gate-ctq-report-v2 + gate-cpk-study-v2 (exit 1, CTQ-06-
  driven; they flip with ratification).
- CTQ-04 protocol DFLSS ratification (PRD owner amends 20-prd.md per
  the ratification path in 48/49).
- Integration commit: ~70 dirty paths (receipts minted 03:39-03:46Z
  record "Tree state | dirty (66-70 entries)"; no git run by this lane).

Also carried (engineering, low priority):

- Manifest default-scope collisions: REGISTERED, regex fix still OPEN.
  The 7-kernel manifest registers the struct_/perfect_hash substring
  collisions as guard-class INFORMATIONAL kernels (scope_pattern
  release_struct_(array|schema) + 9try_build; accepted_remainder with
  expected_max_count = 12 ceilings). The underlying awk -v regex
  transport bug (REGEX_TRANSPORT_FP: `b\.[a-z]+` decodes to `b.[a-z]+`,
  matching bic/blr) is unfixed in audit_branchless.sh; the manifest-v2
  receipt names it "a one-line follow-up for the script's owner"
  (lane-1 outcome pending).
- Ladder E0433-class residual: NONE. avec-git 48/0/1 ignored exit 0
  this session (43 lib + 2 fsck + 3 git_interop, court B hard PASS);
  the wave-8 E0433-class compile-abort was avec-parquet's accessor
  court, fixed by the cfg gate (both configs green, above).

Gate inventory v6 additions (docs/jira/v26.10.31/receipts/, statuses
re-read from receipts this session):

```
| Receipt                      | Status  | Note                                   |
|------------------------------|---------|----------------------------------------|
| gate-branchless-manifest-v2  | ALIVE   | exit 0, 03:46Z, 7 kernels, ctrl FIRED  |
| gate-cfg-homology-v2         | ALIVE   | exit 0, 03:41Z, mutant court FLIPPED   |
| gate-git-perf-baseline       | ALIVE   | exit 0, 03:40Z, perf_baseline ignored  |
| gate-gage-rr                 | ALIVE   | exit 0, 02:33Z, --reps 10000           |
| gate-ctq-report-v2           | FAILED  | exit 1, 02:36Z, CTQ-06-driven          |
| gate-cpk-study-v2            | FAILED  | exit 1, 03:39Z, CTQ-06-driven;         |
|                              |         | CTQ-05 Cpu(overall)=21.657 PASS        |
```

Standing: PARTIAL_ALIVE (wave-9 refresh, 2026-10-31, lane 5 of 20).
Grounded this session in the four live runs above plus receipts re-read
from disk (manifest-v2, cfg-homology-v2, cpk-study-v2, ctq-report-v2,
gage-rr, git-perf-baseline). Carried as-of-date: 0.595 -> 8.1 GiB/s and
CTQ-04 1.134 (wave-8 measured); the CTQ-06 1.18-receipt / 1.85-
demonstrated split is recorded, not rounded up.

## Coverage Assertions

- Every FR (FR-01..07) names at least one verification gate — PASS (structural).
- Every gate traces to at least one FR — PASS (structural).
- No FR without a gate row — PASS (7/7 rows name a gate; 0 MISSING).
- Doc coverage, full 00–42 series re-inventoried:

```
| Doc                             | Referenced By                          | Orphan? |
|---------------------------------|----------------------------------------|---------|
| 20-prd.md                       | All FR/CTQ rows                        | no      |
| 21-ard.md                       | All FR rows                            | no      |
| 22-adr-register.md              | 00-index, 21-ard §ADRs, 38, 39         | no      |
| 23-64b-quad-spec.md             | FR-01, FR-02, mmap-decode CTQ          | no      |
| 24-parquet projection spec      | FR-02, FR-03                           | no      |
| 25-git-packfile-integration-spec.md | FR-04 (landed W4)                  | no      |
| 26-blake3-engine-spec.md        | FR-06, BLAKE3 CTQ                      | no      |
| 27-branchless-kernel-abi.md     | FR-05, M/branch CTQs                   | no      |
| 28-beam-nif-handoff.md          | FR-07, latency/heap CTQs               | no      |
| 29–31 series                    | on disk; 31-verification-ladder cited  | no      |
| 33–39 series, 42-checklist-sweep | milestone admin/receipt docs          | no      |
| 40-wave-3-receipt.md            | FR-04/05/06 gate evidence              | no      |
| 41-wave-4-receipt.md            | wave-4 refresh evidence                | no      |
```

Gaps, loudly: no orphan docs; no FR without a gate row. Residual gaps are gate-color
gaps: CTQ-06 throughput FAIL (R1) and the FR-05 opcode residual (R2). Everything else
from the W5 red-list closed W6 (FR-04 fully green, FR-07 beam bridge, CTQ-05, CTQ-02,
CTQ-04, SHACL re-mint, cli-verify probe bug).

No unverifiable requirements: all seven FRs name an executable gate — none is prose-only.

## Standing

PARTIAL_ALIVE (wave-9 refresh, 2026-10-31, lane 5 of 20) — supersedes the
wave-6 standing text that stood here. Verify ladder 10/10 PASS exit 0
(verify-latest.md 2026-10-31T04:06:51Z @ ead40329); CTQ-04 closed at the
protocol layer (gage-conditioned p99/p50, quiet PASS 1.134 vs USL 1.30,
15% pin retired — DFLSS ratification open); CTQ-06 remains FAIL at
1.85 GiB/s against the unratified 8.5 GiB/s LSL (gate-blake3-throughput,
gate-ctq-report-v2, gate-cpk-study-v2 receipts stand).

## See Also

- 20-prd.md
- 21-ard.md
- 31-verification-ladder.md
