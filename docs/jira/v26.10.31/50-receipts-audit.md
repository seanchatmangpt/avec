# Receipts Corpus Audit (Wave 7, Lane 9)

Audit of `docs/jira/v26.10.31/receipts/` for integrity and staleness.
Method: parsed every receipt's Standing/Command/Exit/HEAD/Date fields, classified
CURRENT / SUPERSEDED / STALE, then re-ran 3 ALIVE receipts' commands on the live
tree. No receipt was edited.

Corpus: 22 receipt files; 20 minted by `scripts/mint_receipt.sh` with the standard
table, 1 hand-written gate receipt (`gate-cfg-homology.md`), plus README/template
(excluded from counts).

## Integrity

- All 22 receipts pin `Input SHA (git HEAD) = ead40329a699878a91b070378066b5e50541ba33`,
  which is still repo HEAD (verified `git rev-parse HEAD`, 2026-10-31). No receipt
  is detached from the current head. Staleness is dirty-tree drift only: tree grew
  27 → 61 entries across the mint window 2026-10-31T00:34Z – 02:36Z.
- `gate-cfg-homology.md` is the only hand-written receipt (inline Standing block,
  gate ALIVE / verdict FAILED, exit 1, no minted table). Integrity acceptable;
  format exception noted.

## Standing counts

| Standing | Count | Receipts |
|---|---|---|
| ALIVE | 14 | demo-cargo-version, gate-arrow-tests, gate-beam-bridge-v2, gate-blake3-tests-v2, gate-blake3-zero-alloc, gate-branchless-selftest-v2, gate-cfg-homology (gate ALIVE, verdict FAILED), gate-cli-machine-report, gate-e2e-ctq05, gate-e2e-integration-v2, gate-gage-rr, gate-git-tests-v2, gate-shacl-v2, gate-workspace-check-v2 |
| FAILED | 8 | demo-fail (exit 1, deliberate), gate-blake3-throughput (1), gate-ctq-report (1), gate-ctq-report-v2 (1), gate-divan-ctq (1), gate-git-tests (101), gate-shacl (1), gate-workspace-check (101) |

## Superseded chains

| v1 (SUPERSEDED) | v2+ successor | Note |
|---|---|---|
| gate-shacl (exit 1) | gate-shacl-v2 (exit 0) | fixed; v2 re-verified below |
| gate-workspace-check (exit 101) | gate-workspace-check-v2 (exit 0) | fixed |
| gate-git-tests (exit 101) | gate-git-tests-v2 (exit 0) | scope widened --lib → crate |
| gate-ctq-report (exit 1) | gate-ctq-report-v2 (exit 1) | both FAILED; v2 not yet green |
| gate-e2e-integration-v2 (exit 0) | gate-e2e-ctq05 (exit 0) | same test + `--nocapture`; later mint |

Every v1 in a chain is FAILED except gate-e2e-integration-v2 (superseded by a
later, richer mint of the same passing command — a formatting superset, not a fix).

## Classification

CURRENT = command would still reflect tree state. SUPERSEDED = successor exists.
STALE = tree moved materially after mint (kernel churn, untracked wave-6/7 files,
transient inputs) with no re-mint.

| Receipt | Classification | Rationale |
|---|---|---|
| demo-cargo-version | CURRENT | trivial, environment-level |
| demo-fail | CURRENT | deliberate FAILED demo, trivially reproducible |
| gate-arrow-tests | STALE | minted 01:55 @ tree 48; avec-arrow kernel files (layout.rs, predicates.rs, transpose.rs) landed after — no re-mint |
| gate-beam-bridge-v2 | CURRENT | 02:28 @ tree 59, latest beam surface mint |
| gate-blake3-tests-v2 | CURRENT | 02:21 @ tree 59 |
| gate-blake3-throughput | STALE | FAILED 01:57 @ tree 49, predates avec-blake3 kernel landing; unreminted |
| gate-blake3-zero-alloc | CURRENT | 02:21 @ tree 59 |
| gate-branchless-selftest-v2 | CURRENT | re-verified below |
| gate-cfg-homology | STALE | hand-written; disassembly numbers marked "as of this run" 2026-10-30; kernel churn since |
| gate-cli-machine-report | CURRENT | verify_v26 reflects scripts/verify_v26.10.31.sh, unchanged since |
| gate-ctq-report | SUPERSEDED | by gate-ctq-report-v2 |
| gate-ctq-report-v2 | STALE | FAILED 02:36, latest mint, but post-02:36 kernel churn unreminted; open defect |
| gate-divan-ctq | STALE | FAILED; input `/tmp/divan_clean.txt` is transient (likely gone), command not re-runnable as written |
| gate-e2e-integration-v2 | SUPERSEDED | by gate-e2e-ctq05 |
| gate-e2e-ctq05 | CURRENT | 02:29, latest integration mint |
| gate-gage-rr | CURRENT | re-verified below |
| gate-git-tests | SUPERSEDED | by gate-git-tests-v2 |
| gate-git-tests-v2 | CURRENT | 02:17 @ tree 57; avec-git untouched since |
| gate-shacl | SUPERSEDED | by gate-shacl-v2 |
| gate-shacl-v2 | CURRENT | re-verified below |
| gate-workspace-check | SUPERSEDED | by gate-workspace-check-v2 |
| gate-workspace-check-v2 | CURRENT | 02:17 @ tree 57 |

Totals: CURRENT 12, SUPERSEDED 5, STALE 5.

## Re-verification (2026-10-31, this audit)

Three ALIVE receipts re-run on the live tree (HEAD unchanged, ead4032):

| Receipt | Command re-run | Receipted exit | Actual exit | Result |
|---|---|---|---|---|
| gate-shacl-v2 | `bash scripts/validate_ontology.sh` | 0 | 0 | MATCH — "VERDICT: PASS — ontology conforms to its own shapes; anti-vacuity court live." |
| gate-branchless-selftest-v2 | `bash scripts/audit_branchless.sh --selftest` | 0 | 0 | MATCH — "positive control: FIRED … SELFTEST PASS" (gate non-vacuous) |
| gate-gage-rr | `./target/release/gage_rr --reps 2000` | 0 (at --reps 10000) | 0 | MATCH — gage_pass=true, cv_trimmed 0.7618% < 1% threshold (short-reps variant per audit plan) |

## Mismatch list

None. All three recomputed ALIVE receipts exit exactly as receipted; no receipt's
command flips outcome on the current tree. The open risks are staleness-class,
not integrity-class:

1. `gate-ctq-report-v2` remains the latest ctq mint and still FAILED (exit 1) —
   an open defect, not yet superseded by a passing mint.
2. `gate-blake3-throughput` FAILED exit 1 predates the avec-blake3 kernel work it
   was gating; needs a re-mint against current artifacts.
3. `gate-divan-ctq` is unreplayable as written (transient `/tmp` input captured
   in the command line).
4. `gate-arrow-tests` ALIVE predates the wave-6/7 avec-arrow kernel files; its
   ALIVE standing is as-of-mint only.

## See Also

- `docs/jira/v26.10.31/receipts/README.md` (standing rule: ALIVE = exit 0 +
  non-empty evidence; anything else FAILED)
- `docs/jira/v26.10.31/47-milestone-verdict.md`
