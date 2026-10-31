# DX Audit: v26.10.31 (Wave-10 Lane 18)

MEASURED developer-experience scorecard for the v26.10.31 wave-10 DX/QoL cohort.
Every number below was produced by timing a real run in this tree — no planning
numbers except where marked. Window: 2026-10-30 21:55-22:40 PDT, host arm64
(Darwin 25.2.0), repo `/Users/sac/avec` at HEAD `ead40329` with wave-10 work
uncommitted in the tree.

In-window note: lanes 1/2/11 (dod.sh, doctor bin, status bin) landed *during* this
audit (absent 22:00, present 22:32) — start-state vs end-state is disclosed per row.
`FEATURE_MATRIX.md` (lane 13 remainder) was still absent at write time.

## Scorecard

| # | Metric | Measured | Target | Score |
|---|---|---|---|---|
| 1 | Time-to-first-green-test | <1s warm, 1.6s incremental | <60s | 2/2 |
| 2 | Time-to-full-verify | 170s warm (exit 0) | <15 min | 2/2 |
| 3 | Commands-to-know | 9/9 exist, 6 timed | full set | 2/2 |
| 4 | Docs coverage vs census | 12/12 via census; FEATURE_MATRIX missing | all discoverable | 1/2 |
| 5 | Doctor check count | 10 measured (9 GREEN/1 YELLOW/0 RED) | 10 (lane-2 spec) | 2/2 |
| 6 | Friction coverage (TROUBLESHOOTING) | 5/5 named irritants covered | 5/5 | 2/2 |
| | **Total** | | | **11/12** |

Rubric: 2 = measured and meets target; 1 = present but partial; 0 = pending/absent.

## 1. Time-to-first-green-test

State: tree already warm from prior waves; `cargo check --workspace
--all-targets` re-verified warm at audit start: 2.37s, exit 0.

| Path | Wall time | Result |
|---|---|---|
| `cargo test -p avec-audit` from check-warm (fully cached) | <1s | 2 result lines ok, 5 tests |
| Same, with `crates/audit/src/lib.rs` touched (incremental) | 1.6s | 2 result lines ok, 5 tests |
| Cold build (`cargo clean`) | not measured | destructive to shared `target/`; refused (lane rule) |

Command: `cargo test -p avec-audit`. The 5-test audit suite plus the verify ladder's
`test-audit` gate are the fastest real signal a new developer can buy.

## 2. Time-to-full-verify

`time bash scripts/verify_v26.10.31.sh` — full 10-gate ladder, warm cache:

| Run | Wall | Exit | Outcome |
|---|---|---|---|
| 1 (22:22 PDT) | 174s | 1 | `check-workspace` FAIL 101 — concurrent-lane edit; green again 22:26 |
| 2 (22:29 PDT) | 170s | 0 | RESULT: OK — 10/10 gates PASS |

The measured cost of full verification is ~3 minutes warm — 3.5-5x under the 10-15
minute planning number quoted in `docs/GETTING_STARTED.md:124` (operator-stated,
not a measured receipt, by its own wording). Both runs overwrote
`docs/jira/v26.10.31/verify-latest.md` (the script's designed output); the prior
receipted report (2026-10-31T04:06:51Z @ `ead40329`, 10/10 PASS) is quoted verbatim
in GETTING_STARTED, so the record survives.

## 3. Commands-to-know (9-command minimal set)

| Command | Purpose | State | Measured this session |
|---|---|---|---|
| `cargo build --workspace` | build | EXISTS | exercised via every gate |
| `cargo test -p <crate>` | test | EXISTS | yes (sec. 1) |
| `bash scripts/verify_v26.10.31.sh` | verify | EXISTS | 170s, exit 0 |
| `cargo run … --bin doctor` | doctor | EXISTS (landed in-window) | 1s, exit 1 (YELLOW, by design) |
| `just courts` | courts | EXISTS | 1s, exit 0, 3/3 PASS |
| `just bench` | bench | EXISTS | no (receipted gate-ctq-report-v2.md; CTQ seen via status) |
| `just receipts` | receipts | EXISTS | exit 0 (20 newest + standing tally) |
| `bash scripts/dod.sh` | dod | EXISTS (landed in-window) | 162s full run + <1s selftest, exit 0 |
| `cargo run -q -p avec-cli --bin status` | status | EXISTS (landed in-window) | 8s, exit 0 |

At audit start the set was 6/9 (doctor/dod/status pending); final state 9/9.
`just --list` shows all 8 recipes wired (lane 3). Recipe-level detail:

- `just doctor` exit 1 is fail-closed non-green (c09 lsl-ratified YELLOW: CTQ-06
  ratification pending), not a recipe defect.
- `just bench` re-run skipped this session: release build under a concurrently
  verifying tree adds load to a shared machine; receipt
  `docs/jira/v26.10.31/receipts/gate-ctq-report-v2.md` stands.
- `just status` runs the live CTQ battery (CTQ-02/03/04/05 PASS; CTQ-06 rows FAIL
  vs the un-ratified 8.5 GiB/s LSL, disclosed "[original, not ratified]") —
  consistent with doctor c09 and dod `P-CTQ06-lsl-ratification`.

## 4. Docs coverage vs 63-census

Census (`63-test-census.md`): 58 result lines, 661 passed, 0 failed, 2 ignored
across 13 invocations, 12 crates + bench no-lib row.

| Surface | Covers counts? | State |
|---|---|---|
| `63-test-census.md` | 13/13 invocations | landed (wave 9) |
| `docs/GETTING_STARTED.md` | totals + sum-all-lines law + routes to 63 | landed (lane 13) |
| `docs/diataxis/` (lane 10) | no per-crate counts (4 quadrants, routing only) | landed |
| `FEATURE_MATRIX.md` | absent | pending lane 13 remainder |

Every crate with tests has its counts discoverable from docs (via 63, routed from
GETTING_STARTED): 12/12. Score 1/2 only because lane 13's `FEATURE_MATRIX.md` is
absent — "pending lane 13".

## 5. Doctor check count

Bin landed mid-audit; measured live (1s wall incl. compile):

```text
summary: 9 GREEN, 1 YELLOW, 0 RED of 10 checks
overall: YELLOW (exit 1)
```

10 checks (c01-c10) — matches the lane-2 spec count of 10. The single YELLOW is
c09 lsl-ratified (CTQ-06 ratification pending), cross-confirmed by the status bin
and the dod manifest's `P-CTQ06-lsl-ratification` — three independent surfaces
agree on the same open item. Exit 1 on YELLOW is fail-closed, by design.

## 6. Friction inventory (top 5 wave irritants vs TROUBLESHOOTING.md)

| Irritant | TROUBLESHOOTING | Coverage |
|---|---|---|
| Stale dylib (avec-beam `nif_not_loaded`) | 1 | COVERED (build-before-test + census receipt) |
| Doc-test parse trap ("passes with 0 tests") | 2 | COVERED (sum-ALL-result-lines law) |
| std-feature (bare parquet run collects 0 courts) | 2 | COVERED (explicit `--features std`) |
| Standalone manifests (wasm-footprint) | 7 | COVERED (in-place / `--manifest-path`) |
| profile-audit build requirement (objdump/bitcode) | 4 | COVERED (`--profile audit`, lto=false) |

5/5 covered, each with Symptom/Cause/Fix and a receipt pointer. One newly witnessed
gap, proposed as TROUBLESHOOTING candidate 10:

- **Transient verify FAIL under concurrent lanes.** Run 1's `check-workspace` FAILed
  on another lane's in-flight edit; the baseline classifier could not absorb it
  (`scripts/verify_baseline.txt` is 0 lines — an empty baseline matches only a clean
  check, so any in-flight edit signature classifies as introduced FAIL). Self-healed
  in ~4 min (compile-freeze SLA honored). Fix direction: rerun `check-workspace`
  once on a green re-check before scoring FAIL, or scope the baseline to
  known-pre-existing signatures with a stale-baseline warning.

## Findings

1. **GETTING_STARTED drift (F1, owner lane 13).** Two stale claims from its
   landed-before-dependencies write time: `docs/GETTING_STARTED.md:30-32` ("there
   is no justfile in the tree yet" — `justfile` landed via lane 3) and
   `:176-178` (TROUBLESHOOTING "pending, not yet on disk" — landed via lane 9, 131
   lines, 9 entries). Suggested fix is two one-line edits, filed here for the
   owning lane.
2. **Empty-baseline FAIL absorption (F2)** — the section-6 gap above.
3. **verify-latest.md regenerated by this audit (F3).** Disclosed in section 2;
   prior receipt quoted in GETTING_STARTED remains the durable record.

## See Also

- `docs/jira/v26.10.31/63-test-census.md` — census the docs-coverage row grades against
- `docs/jira/v26.10.31/verify-latest.md` — regenerated green report (run 2, 170s)
- `docs/TROUBLESHOOTING.md` — friction coverage graded in section 6
- `docs/GETTING_STARTED.md` — onboarding doc with the two stale claims in F1
- `justfile` — the 8 wired recipes (lane 3)
