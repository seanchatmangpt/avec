# Receipts Inventory Final (Wave 9, Lane 15)

As-of: 2026-10-31T04:18:16Z. HEAD `ead4032` throughout. Supersedes the corpus
tables in [50-receipts-audit.md](50-receipts-audit.md) (wave-7, 22 receipts).
No receipt was edited; no git operations performed; only this file was created.

Corpus is live: 5 receipts (arrow-v3, workspace-v3, blake3-tests-v3,
parquet-tests, e2e-integration-v3) landed during this inventory (04:11-04:15Z).
Counts are a snapshot at the as-of stamp. Classification boundary = last
observed code edit, 2026-10-31T04:15:09Z (`crates/avec-arrow/src/layout.rs`).

Corpus: 36 receipts — 35 in `receipts/` (33 script-minted by
`scripts/mint_receipt.sh`, 2 hand-written: `gate-cfg-homology.md` and
`parquet-throughput-baseline.md`, which self-declares hand-minted) plus 1
hand-written stray at this directory's top level (see Stray below).
`README.md`/`template.md` are non-receipts and excluded.

## 1. Totals by Standing

| Standing | Count | Notes |
|---|---|---|
| ALIVE | 25 | incl. gate-cfg-homology (gate ALIVE / verdict FAILED, hand-written) and the gate-branchless-manifest stray |
| FAILED | 10 | 1 deliberate demo (demo-fail); 9 real gate FAILs |
| PARTIAL | 1 | parquet-throughput-baseline (real number, load-shared host, 74 GiB/s not confirmed) |
| **Total** | **36** | |

## 2. Per-receipt inventory

| Receipt | Standing | Minted (UTC) | Command subject | Chain terminal / superseded-by |
|---|---|---|---|---|
| demo-cargo-version | ALIVE | 10-31 00:34Z | `cargo --version` | terminal, CURRENT |
| demo-fail | FAILED | 10-31 00:34Z | `false` | terminal, CURRENT (deliberate demo) |
| gate-arrow-tests | ALIVE | 10-31 01:55Z | `cargo test -p avec-arrow --lib` | -> arrow-tests-v3 |
| gate-arrow-tests-v3 | ALIVE | 10-31 04:11Z | `cargo test -p avec-arrow --lib` | terminal, STALE-UNREMINTED |
| gate-beam-bridge-v2 | ALIVE | 10-31 02:28Z | `cargo test -p avec-beam` | terminal, STALE-UNREMINTED |
| gate-blake3-tests-v2 | ALIVE | 10-31 02:21Z | `cargo test -p avec-blake3 -- --test-threads=1` | -> blake3-tests-v3 |
| gate-blake3-tests-v3 | ALIVE | 10-31 04:13Z | `cargo test -p avec-blake3` | terminal, CURRENT |
| gate-blake3-throughput | FAILED | 10-31 01:57Z | `target/release/blake3_throughput --gate` | terminal, STALE-UNREMINTED |
| gate-blake3-zero-alloc | ALIVE | 10-31 01:57Z | `cargo test -p avec-blake3 --test zero_alloc -- --test-threads=1` | -> blake3-tests-v3 |
| gate-branchless-manifest | ALIVE | 10-31 03:04Z | audit_branchless.sh (prose) | -> branchless-manifest-v2 |
| gate-branchless-manifest-v2 | ALIVE | 10-31 03:46Z | `/Users/sac/avec/scripts/audit_branchless.sh` | terminal, STALE-UNREMINTED |
| gate-branchless-selftest-v2 | ALIVE | 10-31 02:21Z | `bash scripts/audit_branchless.sh --selftest` | terminal, STALE-UNREMINTED |
| gate-cfg-homology | ALIVE* | 10-30 ~02:31Z | `./scripts/cfg_homology.sh` | -> cfg-homology-v2 |
| gate-cfg-homology-v2 | ALIVE | 10-31 03:41Z | `bash scripts/cfg_homology.sh --mutant` | terminal, CURRENT |
| gate-cli-machine-report | ALIVE | 10-31 01:57Z | `cargo run -q -p avec-cli --bin verify_v26` | terminal, STALE-UNREMINTED |
| gate-cpk-study | FAILED | 10-31 03:03Z | `./target/release/cpk_study` | -> cpk-study-v2 |
| gate-cpk-study-v2 | FAILED | 10-31 03:39Z | `./target/release/cpk_study` | terminal, CURRENT (open defect) |
| gate-ctq-report | FAILED | 10-31 02:22Z | `target/release/ctq_report` | -> ctq-report-v2 |
| gate-ctq-report-v2 | FAILED | 10-31 02:36Z | `./target/release/ctq_report` | terminal, STALE-UNREMINTED |
| gate-divan-ctq | FAILED | 10-31 02:21Z | `python3 scripts/divan2receipt.py /tmp/divan_clean.txt` | terminal, STALE-UNREMINTED |
| gate-e2e-ctq05 | ALIVE | 10-31 02:29Z | `cargo test -p avec-integration --release --test admission_e2e -- --nocapture` | -> e2e-integration-v3 |
| gate-e2e-integration-v2 | ALIVE | 10-31 02:21Z | `cargo test -p avec-integration --release --test admission_e2e` | -> e2e-integration-v3 |
| gate-e2e-integration-v3 | ALIVE | 10-31 04:14Z | `cargo test -p avec-integration --release --test admission_e2e` | terminal, CURRENT |
| gate-gage-rr | ALIVE | 10-31 02:33Z | `./target/release/gage_rr --reps 10000` | terminal, CURRENT |
| gate-git-perf-baseline | ALIVE | 10-31 03:40Z | `cargo test -p avec-git --release --test perf_baseline -- --ignored --nocapture` | terminal, STALE-UNREMINTED |
| gate-git-tests | FAILED | 10-31 01:55Z | `cargo test -p avec-git --lib` | -> git-tests-v2 |
| gate-git-tests-v2 | ALIVE | 10-31 02:17Z | `cargo test -p avec-git` | terminal, STALE-UNREMINTED |
| gate-parquet-tests | ALIVE | 10-31 04:14Z | `cargo test -p avec-parquet --features std` | terminal, STALE-UNREMINTED |
| gate-perf-guard | ALIVE | 10-31 03:07Z | `scripts/perf_guard.sh` | terminal, CURRENT |
| gate-shacl | FAILED | 10-31 01:55Z | `scripts/validate_ontology.sh` | -> shacl-v2 |
| gate-shacl-v2 | ALIVE | 10-31 02:15Z | `bash /Users/sac/avec/scripts/validate_ontology.sh` | -> shacl-v3 |
| gate-shacl-v3 | ALIVE | 10-31 03:03Z | `bash /Users/sac/avec/scripts/validate_ontology.sh` | terminal, CURRENT |
| gate-workspace-check | FAILED | 10-31 01:55Z | `cargo check --workspace` | -> workspace-check-v2 |
| gate-workspace-check-v2 | ALIVE | 10-31 02:17Z | `cargo check --workspace` | -> workspace-check-v3 |
| gate-workspace-check-v3 | ALIVE | 10-31 04:11Z | `cargo check --workspace` | terminal, STALE-UNREMINTED |
| parquet-throughput-baseline | PARTIAL | 10-30 03:07Z | `cargo run --release -p bench --bin parquet_throughput` | terminal, STALE-UNREMINTED |

\* gate-cfg-homology declares gate ALIVE / verdict FAILED (exit 1, transpose
delta +17 vs slack 4). Counted ALIVE by gate standing per 50's convention.

## 3. Superseded chains

10 supersession chains (24 receipts); 12 singletons (12 receipts);
22 terminals total (10 chain terminals + 12 singletons-as-terminals).

| # | Chain | Terminal | Terminal standing / state |
|---|---|---|---|
| 1 | gate-shacl -> shacl-v2 -> shacl-v3 | gate-shacl-v3 | ALIVE, CURRENT |
| 2 | gate-workspace-check -> v2 -> v3 | gate-workspace-check-v3 | ALIVE, STALE-UNREMINTED |
| 3 | gate-arrow-tests -> v3 (no v2 ever minted) | gate-arrow-tests-v3 | ALIVE, STALE-UNREMINTED |
| 4 | gate-blake3-zero-alloc + gate-blake3-tests-v2 -> v3 | gate-blake3-tests-v3 | ALIVE, CURRENT |
| 5 | gate-ctq-report -> v2 | gate-ctq-report-v2 | FAILED, STALE-UNREMINTED |
| 6 | gate-cpk-study -> v2 | gate-cpk-study-v2 | FAILED, open defect, premise unchanged |
| 7 | gate-cfg-homology (hand) -> v2 | gate-cfg-homology-v2 | ALIVE, CURRENT (--mutant mode; plain-gate FAIL at v1 unreminted) |
| 8 | gate-branchless-manifest (stray) -> v2 | gate-branchless-manifest-v2 | ALIVE, STALE-UNREMINTED |
| 9 | gate-e2e-integration-v2 -> gate-e2e-ctq05 -> v3 | gate-e2e-integration-v3 | ALIVE, CURRENT |
| 10 | gate-git-tests -> v2 | gate-git-tests-v2 | ALIVE, STALE-UNREMINTED |

Cross-family coverage edges (same-surface, later mint): zero-alloc ->
blake3-tests-v3; e2e-ctq05 -> e2e-integration-v3. Both folded into chains 4
and 9. Terminal summary: 4 ALIVE-CURRENT, 1 FAILED-open (cpk-study-v2),
5 STALE-UNREMINTED (ws-v3, arrow-v3, ctq-v2, branchless-manifest-v2,
git-tests-v2).

## 4. Stale-class list (premise changed post-mint)

14 receipts (1 REPLACED-BY + 13 STALE-UNREMINTED). REPLACED-BY = a newer
receipt covers the same surface; STALE-UNREMINTED = premise moved, no newer
receipt.

| Receipt | Class | Premise change (evidence) |
|---|---|---|
| gate-ctq-report | REPLACED-BY v2 | measured the retired stdev/mean pin (49-ctq04-variance-study) |
| gate-ctq-report-v2 | STALE-UNREMINTED | same retired pin; `ctq_report.rs` edited 03:41:39Z post-mint — wave-8 implemented the p99/p50 protocol (PASS 1.134 witnessed in 56/00-index, NO receipt minted post-swap) |
| gate-blake3-throughput | STALE-UNREMINTED | FAILED 01:57Z predates the avec-blake3 kernel it gates (multi_chunk.rs 03:43:42Z); no successor of the `blake3_throughput --gate` surface |
| gate-divan-ctq | STALE-UNREMINTED | transient `/tmp/divan_clean.txt` input — unreplayable as written; no successor |
| gate-cli-machine-report | STALE-UNREMINTED | `verify_v26.rs` edited 02:05:43Z, 8 min post-mint; re-witnessed ALIVE by verify-latest 04:06Z (not a receipt) |
| gate-beam-bridge-v2 | STALE-UNREMINTED | beam lib.rs/ops_scan/ops_persist/resources moved 03:43-03:51Z post-mint; no v3 |
| gate-git-tests-v2 | STALE-UNREMINTED | avec-git grew post-mint (perf_baseline.rs 03:05Z, pack_write/inflate/deflate 04:15Z); no v3 |
| gate-git-perf-baseline | STALE-UNREMINTED | avec-git pack_write/inflate/deflate edited 04:15:05-08Z post-mint |
| gate-branchless-selftest-v2 | STALE-UNREMINTED | `audit_branchless.sh` edited post-mint (wave-7 manifest adjudication, then 04:10:15Z); no --selftest re-mint |
| gate-branchless-manifest-v2 | STALE-UNREMINTED | `audit_branchless.sh` edited 04:10:15Z post-mint |
| gate-arrow-tests-v3 | STALE-UNREMINTED | `layout.rs` edited 04:15:09Z, 3.5 min post-mint (live churn) |
| gate-workspace-check-v3 | STALE-UNREMINTED | workspace crates edited 04:15:0xZ, 4 min post-mint (live churn) |
| gate-parquet-tests | STALE-UNREMINTED | `level_free_multi_court.rs` edited 04:15:04Z, 54 s post-mint (live churn) |
| parquet-throughput-baseline | STALE-UNREMINTED | `parquet_throughput.rs` edited 04:08:29Z post-mint; wave-8 scan.rs slice cache (~13.7x) moved the kernel; standing PARTIAL as-minted |

The stale-class list is 14 receipts: 13 STALE-UNREMINTED plus gate-ctq-report
(which is REPLACED-BY in the chain table and premise-dead here via the retired
pin — the one overlap between the two classes). The remaining 13 REPLACED-BY
receipts are clean chain supersession (section 3), not premise-change:
shacl, shacl-v2, workspace-check, workspace-check-v2, arrow-tests,
blake3-tests-v2, blake3-zero-alloc, cfg-homology, cpk-study,
e2e-integration-v2, e2e-ctq05, branchless-manifest (stray), git-tests.

## 5. Stray gate-*.md at top level

`gate-branchless-manifest.md` sits at `docs/jira/v26.10.31/` top level (mtime
2026-10-31T03:04Z), not in `receipts/`. Hand-written wave-7 lane-5 receipt in
gate form; the minter's convention (receipts/README.md rule 1: receipts live
in `receipts/`, minted by the script only) puts receipts in `receipts/`, so
this is recorded for relocation-by-operator. Not moved by this lane. Its
surface is superseded by `receipts/gate-branchless-manifest-v2.md`
(ALIVE, 03:46Z).

## 6. Live-churn caveat

Receipts minted 04:11-04:15Z (arrow-v3, ws-v3, blake3-tests-v3, parquet-tests,
e2e-v3) straddle the 04:15:09Z churn boundary: three of them are already
premise-drifted. The corpus needs one fresh verify-ladder pass at wave close
to re-terminate every chain at a current-tree terminal.

## 7. Counts

| Metric | Value |
|---|---|
| Total receipts | 36 (35 in receipts/ + 1 stray top-level) |
| Script-minted | 33 |
| Hand-written | 3 (gate-cfg-homology.md; stray gate-branchless-manifest.md; parquet-throughput-baseline.md — self-declares hand-minted) |
| Standing ALIVE / FAILED / PARTIAL | 25 / 10 / 1 |
| Supersession chains | 10 (24 receipts) |
| Singletons | 12 |
| Terminals | 22 (10 chain terminals + 12 singletons) |
| CURRENT | 9 |
| REPLACED-BY | 14 |
| STALE-UNREMINTED | 13 |
| Stale-class list (4) | 14 (13 STALE-UNREMINTED + gate-ctq-report) |
| CURRENT+REPLACED-BY+STALE | 36 |

## See Also

- [50-receipts-audit.md](50-receipts-audit.md) (wave-7 audit; corpus tables
  superseded by this file)
- [49-ctq04-variance-study.md](49-ctq04-variance-study.md) (CTQ-04 protocol
  swap grounding)
- [receipts/README.md](receipts/README.md) (minter convention, standing rule)

## 8. Wave-10 additions (gap-closure lane G2, 2026-10-31)

Appends only — sections 1-7 stand as the wave-9 snapshot (36 receipts,
25/10/1). Five receipts landed post-snapshot (three by fix lane X3, two by
this lane); totals recounted from disk after this wave's mints:
**41 receipts — 40 in `receipts/` + 1 stray top-level —
29 ALIVE / 11 FAILED / 1 PARTIAL** (29+11+1 = 41).

| Receipt | Standing | Minted (UTC) | Command | Closes / supersedes |
|---|---|---|---|---|
| gate-git-tests-v3 | ALIVE | 10-31 05:33Z | `cargo test -p avec-git` | §4 gate-git-tests-v2 STALE-UNREMINTED flag closed; chain 10 now terminates at v3 (48 passed / 0 failed / 1 ignored across 5 binaries). Minted during a concurrent gap lane's edit of integration/avec-git test texts; result recorded as-run; this was the last mint of the wave. |
| gate-cpk-study-v3 | FAILED (exit 1) | 10-31 05:32Z | `./target/release/cpk_study` | Chain 6 now terminates at v3 (open defect: CTQ-06 mean 1.84 GiB/s vs LSL 8.5, Cpl −58.49 vs target 1.67; CTQ-05 Cpu 9.80 PASS, CTQ-04 Cpu 4.13 PASS — FAIL on CTQ-06 only). Supersedes the receipt-less fresh-cpk rows in 58 §3 and 61 §3; one-line errata appended to both docs this wave. |
| gate-branchless-manifest-v3 | ALIVE | 10-31 05:00Z | `bash scripts/audit_branchless.sh` | Landed by fix lane X3, not G2. Closes §4 gate-branchless-manifest-v2 STALE-UNREMINTED flag; chain 8 continues. |
| gate-branchless-manifest-v4 | ALIVE | 10-31 05:28Z | `audit_branchless.sh` (BRANCHLESS_SCOPE=transpose\|untranspose) | Landed by fix lane X3. Chain 8 terminal moves v3 -> v4 (ALIVE). |
| gate-branchless-selftest-v3 | ALIVE | 10-31 04:58Z | `bash scripts/audit_branchless.sh --selftest` | Landed by fix lane X3. Closes §4 gate-branchless-selftest-v2 STALE-UNREMINTED flag. |

Not re-audited by this lane: the remaining §4 STALE-UNREMINTED flags
(ctq-report-v2, blake3-throughput, divan-ctq, cli-machine-report,
beam-bridge-v2, git-perf-baseline, arrow-v3, workspace-check-v3,
parquet-tests, parquet-throughput-baseline) stand as recorded at wave-9
close. gate-cfg-homology keeps its ALIVE* (gate ALIVE / verdict FAILED)
convention count; parquet-throughput-baseline keeps PARTIAL. Both G2 mints
verified on disk with their recorded Standing at wave close.
