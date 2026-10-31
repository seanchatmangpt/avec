# Checklist Sweep v5: v26.10.31 Release Checklist (lane 17, wave 8)

Fifth re-walk of [36-release-checklist.md](36-release-checklist.md) against
current disk, 2026-10-30 ~20:55 local. Method: receipt Standing/Exit reads,
file-existence checks, verify-latest.md citation (mtime 254 s old — inside the
20-minute window, not re-run), plus one cheap self-run
(`cargo test -p avec-parquet --features std`, exit 1 — see Δ item 9).
Delta-only vs [54-checklist-sweep-v4.md](54-checklist-sweep-v4.md) (wave 7).

## DMADV Gates (Δ vs 54)

| Gate | State | Evidence | Δ vs 54 |
|---|---|---|---|
| Define | DONE | 20/21 docs; CTQ-06 LSL amendment still PROPOSED (20-prd.md). | unchanged |
| Measure | IN-PROGRESS | gate-gage-rr.md ALIVE. gate-perf-guard.md **re-minted exit 0** (2026-10-31T03:07:01Z, quiet window) — was FAILED exit 1 (CV 2.27% → SKIP-NOISY). Timing verdicts now admissible on this host state. | FAILED → ALIVE (quiet-window re-run) |
| Analyze | IN-PROGRESS | gate-cfg-homology.md: gate ALIVE, verdict FAILED (transpose delta +17 vs slack 4) — unchanged; court still COURT-UNPROVEN per 52 (no mutant witness). | unchanged |
| Design | IN-PROGRESS | CTQ-06 still red: gate-ctq-report-v2 exit 1 (0.43 / 1.18 GiB/s vs LSL 8.5); gate-cpk-study-v2 exit 1, `failed_gates: ["CTQ-06"]` — **CTQ-04 now PASS in the same study** (Cpu 1.863 ≥ 1.67, CV 2.68%), CTQ-05 PASS (Cpu 21.66). | CTQ-04 flipped FAIL→PASS inside cpk-study-v2; sole failed gate is CTQ-06 |
| Verify | IN-PROGRESS | verify-latest.md 03:34:34Z @ HEAD ead4032: 10/10 PASS, exit 0 (FR-07 SKIP row superseded by gate-beam-bridge-v2 ALIVE). | unchanged (10/10) |

## Implementation Checklist (Δ vs 54)

| Item | State | Evidence | Δ vs 54 |
|---|---|---|---|
| Pin 64B quad layout (spec 23) | DONE | layout.rs; FR-01 PASS. | unchanged |
| Sentinel/alignment asserts + mutation kill | DONE | predicates.rs, transpose.rs, mutation_kills.rs; FR-02 PASS. | unchanged |
| Parquet zero-level + isomorphism (spec 24) | DONE-with-regression | v4 sweep run was exit 0 / 3 passed; my run this sweep: `cargo test -p avec-parquet --features std` **exit 1 — 162 passed, 1 FAILED**: `scan::tests::typed_accessors` (scan.rs:1074 — in-bounds guard asserts `value_u64(0,0,rows-1)` or `value_u1(0,0,rows)` errors; both returned Ok). scan.rs is untracked (wave-8 lane in flight). NOT in the verify ladder (ladder runs default features, PASS 03:34Z); std-feature path only. | NEW REGRESSION witnessed this session |
| Packfile writer + git fsck court (spec 25) | DONE | FR-04 PASS; **gate-git-perf-baseline.md minted 03:40:59Z, exit 0, Standing ALIVE** (`cargo test -p avec-git --release --test perf_baseline -- --ignored`). | NEW receipt: git perf baseline |
| 16-stage ladder + banned-opcode gate (spec 27) | DONE | gate-branchless-manifest.md ALIVE (manifest-adjudicated, negative control exit 1). gate-branchless-manifest-v2 NOT on disk; manifest still covers `transpose\|predicates` only — unlisted symbols remain FAIL-loudly-missing (residual R2). | unchanged |
| BLAKE3 KATs + throughput fence (spec 26) | IN-PROGRESS | gate-blake3-tests-v2 ALIVE (vectors 6/6); CTQ-06 still FAIL, triple-witnessed (0.43/1.85/1.18). Amendment still PROPOSED. | unchanged red |
| NIF zero-copy + alloc asserts (spec 28) | DONE | gate-beam-bridge-v2 ALIVE. ops_scan.rs / ops_persist.rs **NOT on disk**; no bridge-contract markers beyond bridge_contract.rs. Five-op surface not landed. | confirmed not landed |
| Divan suites + divan2receipt (spec 29) | DONE | gate-divan-ctq FAILED (fail-closed non-vacuity witness). | unchanged |
| SHACL suite + mutation witness (spec 30) | DONE | gate-shacl-v3 ALIVE exit 0. | unchanged |
| Machine report + receipts committed (spec 31) | IN-PROGRESS | receipts/ corpus now **29 files** (was 25). New since v4: gate-cpk-study-v2 (exit 1, CTQ-06 only), gate-perf-guard re-mint (exit 0), parquet-throughput-baseline (exit 0, ≥74 GiB/s spec-24 claim), gate-git-perf-baseline (exit 0 ALIVE). Integration commit pending; 67 dirty entries. | 25 → 29 receipts |

## Documentation Closure (Δ vs 54)

| Item | State | Evidence |
|---|---|---|
| Index doc full-series refs | DONE | 00-index.md references 49/52/55 (6 hits). |
| Traceability matrix closed or loudly open | IN-PROGRESS | FR-05 PARTIAL (R2), FR-06 PARTIAL (CTQ-06), C_pk row PROVISIONAL. | unchanged |
| Press release internal-only | DONE | 19-press-release.md:3. | unchanged |

## Documentation: wave-7 gaps now closed (Δ vs 54 item 5)

| Doc | v4 state | v5 state |
|---|---|---|
| 49-ctq04-variance-study.md | MISSING | **EXISTS** — STUDY COMPLETE; ratification input for 15% pin; exit-1 contract preserved; falsifier stated. Supports CTQ-04 PASS in cpk-study-v2. |
| 52-courts-audit.md | MISSING | **EXISTS** — COURT-LIVE 21 / **COURT-UNPROVEN 3** (unchanged count): avec-git git_interop direction_b (skip-loud), avec-blake3 zero_alloc (no positive control), cfg_homology (no mutant witness). Minimal flip tests written for each; none executed yet. |
| 55-final-integration-report.md | MISSING | **EXISTS** — full battery 20:05–20:20 PDT @ ead4032. |
| 00-index.md refs | — | refreshed (6 refs). |

## Landing Mechanics (Δ vs 54)

| Item | State | Evidence | Δ vs 54 |
|---|---|---|---|
| Atomic commits via -F <file> | IN-PROGRESS | Coordinator-owned; ead4032 witnessed. | unchanged |
| No worktrees | DONE | Single canonical checkout. | unchanged |
| Lane build roots deleted pre-final-commit | IN-PROGRESS | `git status --porcelain` = 67 dirty entries (was 63). | grew by 4 |
| Receipts with input SHAs | DONE | 29 receipts, all HEAD ead40329… pinned. | 25 → 29 |

## Abort Criteria (Δ vs 54)

| Criterion | State | Evidence | Δ |
|---|---|---|---|
| Vacuous gate | NOT FIRED (9 gates proven able to fail) | Same FAILED receipts as v4. | unchanged |
| FR without owning crate | OPEN RISK (narrowed) | Only FR-06/CTQ-06 blocks full PASS. | unchanged |
| Timer Gage failure | RESOLVED-FOR-NOW | gate-perf-guard re-minted **ALIVE exit 0** in quiet window — timing verdicts admissible; but any CTQ re-run outside a quiet window re-fires it. | FAILED → quiet-window ALIVE |

## Delta Summary vs 54-checklist-sweep-v4.md

1. **gate-perf-guard ALIVE exit 0** (03:07:01Z): machine-noise gate cleared in
   a quiet window; SKIP-NOISY channel closed for now.
2. **gate-cpk-study-v2** (03:39:11Z): exit 1 with `failed_gates:
   ["CTQ-06"]` — CTQ-04 variance now **PASS** (Cpu 1.863 vs target 1.67,
   CV 2.68%) and CTQ-05 PASS; CTQ-06 is the sole failing gate, now
   clean-witnessed in a quiet window.
3. **gate-git-perf-baseline ALIVE exit 0** (03:40:59Z): git pack write/parse
   perf baseline receipted (new gate family).
4. **Docs 49, 52, 55 landed**; 00-index references them. Wave-7 doc gaps from
   v4 item 5 fully closed.
5. **Courts: COURT-UNPROVEN still 3** (52-courts-audit): zero_alloc positive
   control, cfg_homology mutant witness, git_interop direction_b — none
   executed; flip tests specified but not run.
6. **parquet-throughput-baseline receipt** (exit 0) minted for the spec-24
   ≥74 GiB/s mmap-scan claim.
7. **NEW REGRESSION (session-introduced, my run)**:
   `cargo test -p avec-parquet --features std` exit 1 —
   `scan::tests::typed_accessors` (scan.rs:1074) fails: neither
   `value_u64(0,0,rows-1)` nor `value_u64(0,0,rows)` errors on an
   out-of-bounds probe. scan.rs is untracked/in-flight (wave-8). Default-
   feature ladder unaffected (verify 10/10 PASS 03:34Z).
8. **Beam five-op surface not landed**: no ops_scan.rs / ops_persist.rs on
   disk; bridge contract unchanged.
9. gate-branchless-manifest-v2 not on disk; manifest coverage unchanged
   (`transpose|predicates`), residual R2 open.

## Remaining-to-Release

1. **CTQ-06 BLAKE3 throughput** (sole red CTQ; blocks FR-06 PASS and any full
   release claim): close 1.85 → 8.5 GiB/s, or formally admit the PRD amendment
   (20-prd.md PROPOSED, evidence 48-lsl-rederivation.md), then re-run
   ctq_report to exit 0 and re-mint gate-ctq-report-v2.
2. **New regression**: fix or revert the in-flight scan.rs typed-accessor
   bounds guard so `--features std` is green again (162/163 currently pass).
3. **COURT-UNPROVEN 3 → 0**: run the three specified flip tests (zero_alloc
   positive control, cfg_homology mutant, git_interop direction_b) and mint
   receipts.
4. **FR-05 census full scope**: manifest still `transpose|predicates`;
   unlisted symbols FAIL loudly — run over full declared scope or amend the
   manifest, closing residual R2.
5. **Beam five-op surface** (ops_scan/ops_persist) — not on disk; open or
   formally descope with an owner.
6. **Integration commit + cleanup** (coordinator): 67 dirty entries, lane
   build roots, per-gate checkbox receipts in 36, atomic -F commits.

## See Also

- [36-release-checklist.md](36-release-checklist.md)
- [54-checklist-sweep-v4.md](54-checklist-sweep-v4.md)
- [52-courts-audit.md](52-courts-audit.md)
- [49-ctq04-variance-study.md](49-ctq04-variance-study.md)
- [55-final-integration-report.md](55-final-integration-report.md)
- [verify-latest.md](verify-latest.md)
- receipts/ (append-only, 29 files)

## Resolution (wave 9, lane 4 of 20)

Verified 2026-10-30 ~21:15 PDT on the canonical checkout. This lane
re-executed the tests itself; every count below is re-read from this
session's actual output, not recalled.

1. **Scan regression RESOLVED.** Coordinator disposition: the failure this
   sweep recorded (`scan::tests::typed_accessors`, std config) was the
   wave-8 accessor court file minted ungated against the std-gated `scan`
   module. Fix: `#![cfg(feature = "std")]` at line 1 of
   `crates/avec-parquet/tests/accessor_perf_court.rs` (on disk, mtime
   20:58:30).
2. **Both parquet configs green in this lane's own runs** (exit 0 each):
   - `cargo test -p avec-parquet` (default): **192 passed / 0 failed** —
     157 lib + 17 mutation_kills + 8 rle_court + 4 aligned_interop +
     3 robustness + 2 page_zero_alloc + 1 std_feature_court;
     accessor_perf_court + level_free_multi_court + doc-tests are 0 under
     default (cfg-gated).
   - `cargo test -p avec-parquet --features std`: **204 passed / 0
     failed** — 163 lib (incl. `scan::tests::typed_accessors ... ok`) +
     accessor_perf_court 1 + level_free_multi_court 4 + std_feature_court 2
     + remainder as default.
   - Sum of test-result lines across both configs: **396 passed / 0
     failed** (vs the wave-8 battery where default was a compile-abort
     contributing 0 tests — 58 item 3').
3. **Remaining-to-Release item 2 CLOSED.** Items 4 and 5 also close on
   disk: gate-branchless-manifest-v2.md ALIVE exit 0 (03:46:09Z; manifest
   now declares 7 kernels — inventory closure per 27 §5), and the beam
   five-op surface landed (see 56/58 resolutions). Item 3 narrows:
   gate-cfg-homology-v2.md carries the mutant witness (MUTANT COURT
   FLIPPED), so COURT-UNPROVEN 3 → 2. Item 1 (CTQ-06) is unchanged red —
   sole failing gate in gate-cpk-study-v2 (03:39:11Z, exit 1 by contract).
4. receipts/ corpus 29 → **32 files** (new: gate-branchless-manifest-v2,
   gate-cfg-homology-v2, gate-git-perf-baseline). Integration commit still
   pending with the coordinator (Remaining-to-Release 6).

## Erratum (wave 9, final-fix lane X2)

Appended; the sweep body and Resolution above stand as written.

- Existence claims stale-at-write: `crates/avec-beam/src/ops_scan.rs`
  (mtime 20:51:53), `ops_persist.rs` (20:45:52), and
  `receipts/gate-branchless-manifest-v2.md` (20:46:37) all predate this
  sweep's ~20:55 walk — the body's "NOT on disk" claims for those three
  were stale at write time. The landings are confirmed by
  56 §Resolution (wave 9).
- Receipt count: `gate-git-perf-baseline` was already inside the body's
  "29 files" and is counted again among the Resolution's three new
  files, so the resolution-time total was 29 + 2 = 31, not 32.
  64-receipts-inventory-final.md records 35 receipts in `receipts/` at
  its 04:18:16Z as-of stamp (after the 04:11-04:15Z live-churn mints).
