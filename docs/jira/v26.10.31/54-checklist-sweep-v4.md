# Checklist Sweep v4: v26.10.31 Release Checklist (lane 19, wave 7)

Fourth re-walk of [36-release-checklist.md](36-release-checklist.md) against
current disk, 2026-10-30 ~20:05 local. Method: `ls`, file reads, receipt
Standing-line reads, plus two cheap self-runs (parquet `--features std` tests;
receipt-dir grep). No heavy builds run. Delta-only vs
[46-checklist-sweep-v3.md](46-checklist-sweep-v3.md) (wave 6).

## DMADV Gates (Δ vs 46)

| Gate | State | Evidence | Δ vs 46 |
|---|---|---|---|
| Define | DONE | 20-prd.md, 21-ard.md; wave-7 PRD amendment section "CTQ-06 LSL re-derivation (PROPOSED)" present (20-prd.md). | amendment PROPOSED noted |
| Measure | IN-PROGRESS | gate-gage-rr.md ALIVE (exit 0, `--reps 10000`); new gate-perf-guard.md **FAILED** exit 1 (`gage cv_trimmed=2.2661% > 2.0%` → SKIP-NOISY — machine too noisy for a timing verdict); gate-cpk-study.md **FAILED** exit 1 (CTQ-06 breach during study). | strengthened: Gage receipt ALIVE confirmed; new fail-closed perf-guard + C_pk receipts (study executed, gates refuse) |
| Analyze | IN-PROGRESS | gate-cfg-homology.md: gate ALIVE, verdict FAILED (transpose delta +17 vs slack 4). | unchanged |
| Design | IN-PROGRESS | Sole open gap remains CTQ-06: gate-ctq-report-v2 FAILED exit 1 (hash_wide_simd 0.43 / hash_many_simd 1.85 GiB/s vs LSL 8.5). LSL re-derivation doc 48-lsl-rederivation.md landed (arms amendment; does not amend alone). | LSL re-derivation + PRD amendment PROPOSED arrived; gate still red |
| Verify | IN-PROGRESS | verify-latest.md 02:58:40Z @ HEAD ead4032: **10/10 PASS** (cli-verify SKIP → PASS exit 0, 7 rows 0 FAIL, FR-07 skip superseded by gate-beam-bridge-v2 ALIVE). | 9 PASS/1 SKIP → 10/10 PASS |

## Implementation Checklist (Δ vs 46)

| Item | State | Evidence | Δ vs 46 |
|---|---|---|---|
| Pin 64B quad layout (spec 23) | DONE | crates/avec-arrow/src/layout.rs; FR-01 PASS (47 verdict). | unchanged |
| Sentinel/alignment asserts + mutation kill | DONE | predicates.rs, transpose.rs, mutation_kills.rs; FR-02 PASS. | unchanged |
| Parquet zero-level + isomorphism (spec 24) | DONE | 47 verdict rows 3/3': 191/0 default features; my sweep run: `cargo test -p avec-parquet --features std` exit 0, **3 passed** — red-list item 4 (0 tests under std) closed by lane I16 fix (level_free_multi_court.rs:9-18 gate). | std-feature anomaly CLOSED |
| Packfile writer + git fsck court (spec 25) | DONE | FR-04 PASS (fsck_court 2/2 real git); gate-git-tests-v2 ALIVE. | unchanged |
| 16-stage ladder + banned-opcode gate (spec 27) | DONE | gate-branchless-manifest.md (new, wave-7 L5) Standing **ALIVE** at docs/jira/v26.10.31/gate-branchless-manifest.md — manifest adjudication over `transpose\|predicates`, exit 0 on covered remainder, 1 on unexplained hit, 2 on vacuous control (expected_max_count 2→0 negative control). verify-latest audit-branchless PASS exit 0. | strengthened: FR-05 gate now manifest-aware with receipt |
| BLAKE3 KATs + throughput fence (spec 26) | IN-PROGRESS | gate-blake3-tests-v2 ALIVE (vectors 6/6 official); CTQ-06 still FAIL (0.43/1.85 vs 8.5, triple-witnessed: throughput, divan-ctq, ctq-report-v2). Trajectory 0.43→1.18→1.85. | unchanged red; LSL re-derivation + amendment PROPOSED |
| NIF zero-copy + alloc asserts (spec 28) | DONE | gate-beam-bridge-v2 ALIVE; FR-07 PASS (bridge_contract real escript). | unchanged |
| Divan suites + divan2receipt (spec 29) | DONE | gate-divan-ctq FAILED (fail-closed refusal = non-vacuity witness). | unchanged |
| SHACL suite + mutation witness (spec 30) | DONE | gate-shacl-v3.md **ALIVE** exit 0 (new, wave-7 L12) over v2 ALIVE over FAILED chain. | strengthened: v3 green on refreshed TTL |
| Machine report + receipts committed (spec 31) | IN-PROGRESS | Receipts corpus now 25 files in receipts/ (23 substantive; gate-branchless-manifest.md sits at the jira dir top level, not receipts/); 50-receipts-audit.md (wave-7 L9) classifies 14 ALIVE / 8 FAILED, all pinned to HEAD ead4032. Integration commit pending. | strengthened: corpus audited |

## Documentation Closure (Δ vs 46)

| Item | State | Evidence | Δ vs 46 |
|---|---|---|---|
| Index doc full-series refs | DONE | 00-index.md (mtime 19:56, refreshed). | refreshed |
| Traceability matrix closed or loudly open | IN-PROGRESS | 32-traceability-matrix.md; FR-05 PARTIAL (residual R2), FR-06 PARTIAL (CTQ-06), C_pk row PROVISIONAL. | unchanged this sweep |
| Press release internal-only | DONE | 19-press-release.md:3 note. | unchanged |

## Landing Mechanics (Δ vs 46)

| Item | State | Evidence | Δ vs 46 |
|---|---|---|---|
| Atomic commits via -F <file> | IN-PROGRESS | Coordinator-owned; ead4032 witnessed. | unchanged |
| No worktrees | DONE | Single canonical checkout. | unchanged |
| Lane build roots deleted pre-final-commit | IN-PROGRESS | `git status --porcelain` = 63 dirty entries at sweep time. | unchanged |
| Receipts with input SHAs | DONE | 50-receipts-audit.md: all 22 receipts audited pin HEAD ead40329…; re-ran 3 ALIVE commands live. | corpus audit receipt landed |

## Abort Criteria (Δ vs 46)

| Criterion | State | Evidence |
|---|---|---|
| Vacuous gate | NOT FIRED (now 9 gates proven able to fail) | FAILED receipts: gate-shacl, gate-shacl-v2→v3 chain root, gate-workspace-check, gate-git-tests, gate-divan-ctq, gate-ctq-report(-v2), gate-blake3-throughput, gate-cpk-study, gate-perf-guard. |
| FR without owning crate | OPEN RISK (narrowed) | All FRs have owning crates + receipts; only FR-06 throughput fence (CTQ-06) blocks full PASS. FR-05 now gated by receipt (manifest ALIVE); census residual tracked under ADR-14. |
| Timer Gage failure | OPEN | No clean Gage study: gate-perf-guard FAILED (machine CV 2.27% > 2.0% → SKIP-NOISY); gate-gage-rr ALIVE proves harness, not quiet-machine study. |

## Wave-7 docs landing check (task-mandated)

| Doc | State | Evidence |
|---|---|---|
| 47-milestone-verdict.md | EXISTS | PARTIAL_ALIVE verdict; red-list 1-6. |
| 48-lsl-rederivation.md | EXISTS | CTQ-06 LSL re-derived 1.1-3.1 (Case A) / higher (Case B) GiB/s/core; arms amendment. |
| 49 (CTQ-04 variance study, L3) | MISSING | No `49-*.md` on disk. |
| 50-receipts-audit.md | EXISTS | 22 receipts, 14 ALIVE / 8 FAILED, all HEAD-pinned. |
| 51-release-notes.md | EXISTS | Opens "Status: PARTIAL_ALIVE (per 47-milestone-verdict.md)". |
| 52 (courts audit, L13) | MISSING | No `52-*.md` on disk. |
| 53-wave-7-receipt.md | EXISTS | 20-lane table; 11 ALIVE / 9 UNKNOWN-AT-MINT at 20:04. |
| 35-migration closure append (L11) | EXISTS | `35-migration-v26.10.29.md:74` "## Closure (wave 7)". |
| PRD CTQ-06 amendment (L8) | EXISTS, PROPOSED | 20-prd.md amendment section; needs admission, not self-adopted. |

## Delta Summary vs 46-checklist-sweep-v3.md

1. Verify gate 10/10 PASS (was 9/1 SKIP): cli-verify now exit 0 (verify-latest.md 02:58:40Z).
2. FR-05: manifest-aware branchless gate landed with **ALIVE** receipt
   (gate-branchless-manifest.md, wave-7 L5) — includes negative control
   (max_count 2→0 → exit 1, non-vacuous).
3. SHACL v3 ALIVE exit 0 over refreshed TTL (wave-7 L12).
4. C_pk study executed: gate-cpk-study.md **FAILED** exit 1 (CTQ-06 breach) —
   C_pk moves UNVERIFIED → receipted-FAILED; still blocks, non-vacuously.
5. Perf guard receipt minted: **FAILED** exit 1 — machine noise CV 2.27% > 2.0%
   (SKIP-NOISY). Timing verdicts currently unadmissible on this host state.
6. Parquet `--features std` anomaly closed: my run exit 0, 3 passed
   (red-list item 4 from 47 verdict resolved; level_free_multi_court.rs:9-18).
7. Gage R&R receipt ALIVE confirmed unchanged (`--reps 10000`, exit 0).
8. New docs on disk: 47, 48, 50, 51, 53; 35 closure append. Still missing:
   49 (CTQ-04 variance study), 52 (courts audit), 55 (final integration).
9. CTQ-06 remains the sole Design/FR-06 blocker; CTQ-04 provisional-USL FAIL is
   the second red gate (47 verdict red-list item 2).

## Milestone-Critical Remaining Items (each with unblocking action)

1. **CTQ-06 BLAKE3 throughput** (blocks FR-06 PASS, any full release claim).
   Unblock: kernel lanes close 1.85 → 8.5 GiB/s, **or** the PRD amendment
   (20-prd.md, PROPOSED) is formally admitted with 48-lsl-rederivation.md as
   its evidence; then re-run ctq_report to exit 0 and mint the receipt.
2. **CTQ-04 variance** (21.9% vs provisional 15% USL). Unblock: either ratify
   the provisional limit (49-variance study, lane L3) or fix the variance;
   ctq_report must exit 0.
3. **Machine-noise gate (perf guard)**. Unblock: run gage/CTQ batteries in a
   machine-quiet window with cv_trimmed ≤ 2.0%, re-mint gate-perf-guard ALIVE;
   this also unblocks a quiet-machine C_pk study (gate-cpk-study re-run).
4. **FR-05 census full-scope closure**. Unblock: run audit_branchless.sh over
   the full declared scope (manifest currently covers `transpose|predicates`;
   unlisted symbols still FAIL) and mint the receipt; closes red-list item 3.
5. **Missing docs**: 49 (CTQ-04 variance study), 52 (courts audit), 55 (final
   integration report) — owners L3/L13/L20; 00-index.md must reference them
   once landed.
6. **Integration commit + cleanup** (coordinator): delete lane build roots /
   scratch (63 dirty entries), per-gate checkbox receipts in 36, atomic -F
   commits. Last mechanical step before any release claim.

## See Also

- [36-release-checklist.md](36-release-checklist.md)
- [46-checklist-sweep-v3.md](46-checklist-sweep-v3.md)
- [47-milestone-verdict.md](47-milestone-verdict.md)
- [50-receipts-audit.md](50-receipts-audit.md)
- [53-wave-7-receipt.md](53-wave-7-receipt.md)
- [verify-latest.md](verify-latest.md)
- receipts/ (append-only)

Superseded by [57-checklist-sweep-v5.md](57-checklist-sweep-v5.md).

Resolution (wave 9): this sweep's "Missing docs: 49 (CTQ-04 variance
study), 52 (courts audit), 55 (final integration)" row is resolved — all
three landed and are referenced by 00-index.md.
