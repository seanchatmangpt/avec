# Final Verdict v3: v26.10.31 (Wave 9, Lane 16)

Closing verdict for the v26.10.31 milestone. Supersedes
[58-final-verdict-v2.md](58-final-verdict-v2.md) (which superseded
[55-final-integration-report.md](55-final-integration-report.md)). Battery
executed sequentially 2026-10-30 21:24–21:29 PDT on the canonical checkout
`/Users/sac/avec`, branch `main`, HEAD `ead4032` (re-verified at run time;
70 dirty porcelain entries read at 21:13, before the settle window
closed). A ~12.3-minute settle wait (21:12–21:24) preceded the battery so
concurrent wave-9 lanes could finish writing; no build-lock block was
observed this run. Every count below is summed from this session's actual
command logs (all `test result:` lines per crate, including integration
binaries and doc-test lines — the doc-test parse trap is handled by
summing every line); no count is recalled from a prior receipt.

## 1. Definitive Battery (this session, real exits)

| # | Command | Exit | Passed / failed / ignored |
|---|---------|------|---------------------------|
| 1 | `cargo check --workspace` | 0 | clean check, 0.4 s |
| 2 | `cargo test -p avec-arrow --lib` | 0 | **93 / 0 / 0** |
| 3 | `cargo test -p avec-parquet` (default) | 0 | **192 / 0 / 0** |
| 4 | `cargo test -p avec-parquet --features std` | 0 | **204 / 0 / 0** |
| 5 | `cargo test -p avec-git` | 0 | **48 / 0 / 1** |
| 6 | `cargo test -p avec-blake3` | 0 | **25 / 0 / 1** |
| 7 | `cargo test -p avec-audit` | 0 | **5 / 0 / 0** |
| 8 | `cargo test -p avec-beam` | 0 | **10 / 0 / 0** |
| 9 | `cargo test -p avec-integration` | 0 | **12 / 0 / 0** |
| 10 | `bash scripts/validate_ontology.sh` | 0 | Conforms True; anti-vacuity mutant rejected |
| 11 | `bash scripts/audit_branchless.sh --selftest` | 0 | positive control FIRED; SELFTEST PASS |
| 12 | `cargo build --release -p bench` | 0 | finished 8.0 s |
| 13 | `target/release/ctq_report` | **1** | JSON `FAIL`, failed_gates `["CTQ-06"]` |
| + | `target/release/cpk_study` (supplementary) | **1** | JSON `FAIL`; CTQ-06 + CTQ-04 |

Per-crate decomposition (all `test result:` lines, this session):

- **avec-parquet default (192)**: 157 lib + 4 aligned_interop +
  17 mutation_kills + 8 rle_court + 3 robustness + 2 page_zero_alloc +
  1 std_feature_court. `accessor_perf_court` and
  `level_free_multi_court` compile and correctly report 0 tests under
  default features — the feature gate landed. **The wave-8 exit-101
  compile-abort is FIXED: this is the first battery of the milestone
  with zero compile-aborts.**
- **avec-parquet std (204)**: 163 lib + 1 accessor_perf_court +
  4 aligned_interop + 4 level_free_multi_court + 17 mutation_kills +
  2 page_zero_alloc + 8 rle_court + 3 robustness + 2 std_feature_court.
- **avec-git (48)**: 43 lib + 2 fsck_court (real `git fsck`) +
  3 git_interop; perf_baseline bin ignored across its 2 result lines,
  documented.
- **avec-blake3 (25)**: 3 lib + 1 soak (bin `bench_tmp`, 68.7 s) +
  7 multi_chunk_vectors (1 ignored) + 6 official_vectors +
  3 small_batch_vectors + 5 zero_alloc. Plain invocation, no thread
  pinning needed.
- **avec-beam (10)**: 9 lib + 1 bridge_contract — real escript BEAM
  subprocess; all 12 asserted contract proofs held: version,
  fail_closed, i64_typed_dispatch, zero_copy_projection,
  envelope_lifecycle, concurrency_and_kill, pointer_identity,
  dtor_exactly_once, alloc_spy, alloc_spy_creation, persist_roundtrip,
  open_scan.
- **avec-integration (12)**: 2 admission_e2e + 6 cross_crate +
  4 membranes.

Totals: **589 tests passed, 0 failed, 2 ignored** across the eight
`cargo test` steps (93+192+204+48+25+5+10+12). Steps 13 and + exit 1 by
design: REAL gates that can fail are failing on exactly the unratified
CTQ-06 LSL (and the load-noised CTQ-04 capability row), not on product
defects.

## 2. FR-01..07 Final Verdicts

| FR | Requirement | W8 (58) | Final |
|----|-------------|---------|-------|
| FR-01 | Columnar alignment (64 B, no splits) | PASS | **PASS** |
| FR-02 | Non-nullable layout (sentinel) | PASS | **PASS** |
| FR-03 | Parquet projection (level-free) | PARTIAL (regressed) | **PASS** |
| FR-04 | Packfile CAS (real git) | PASS | **PASS** |
| FR-05 | Unrolled search ladder | PARTIAL | **PARTIAL** |
| FR-06 | Parallel BLAKE3 tree | PARTIAL | **PARTIAL** |
| FR-07 | Sub-BEAM handoff (zero-copy NIF) | PARTIAL (re-graded) | **PASS** |

- **FR-01 PASS** — arrow 93/0, exit 0.
- **FR-02 PASS** — arrow 93/0 + parquet std 204/0.
- **FR-03 PASS** — both configs green: std 204/0 **and** default 192/0.
  The wave-8 regression (ungated std import in `accessor_perf_court.rs`)
  is closed: the file compiles under default features, runs 0 tests
  there, 1 under std.
- **FR-04 PASS** — avec-git 48/0 incl. fsck_court 2/2 against real
  `git`, git_interop 3/3.
- **FR-05 PARTIAL** — gate green: selftest PASS, positive control
  FIRED, exit 0; manifest ALIVE. Residual = the **ADR-14 adjudication
  items**: every accepted-remainder site claimed by a manifest kernel
  (`expected_max_count` + observed mnemonics), growth as a reviewed
  diff; closure = zero unexplained hits.
- **FR-06 PARTIAL** — correctness green (25/0, vectors 6/6, both CTQ-06
  rows digest-correct); throughput FAIL at the limit as written:
  0.43 / 1.85 GiB/s vs LSL 8.5. [48-lsl-rederivation.md](48-lsl-rederivation.md):
  8.5 is a spec error, not an implementation gap;
  [59-ratification-package.md](59-ratification-package.md) is ASSEMBLED,
  awaiting the operator flip. Gate stays REAL at 8.5 until then.
- **FR-07 PASS** — Precondition 1 on disk: `crates/avec-beam/src/ops_scan.rs`
  (ops 1–2: `avec_open`, `avec_scan`) and `ops_persist.rs` (ops 3–5:
  `avec_flush_to_parquet`, `avec_seal_to_packfile`, `avec_receipt_query`)
  landed and wired in `lib.rs`. Precondition 2 in this run: beam 10/0,
  exit 0, bridge contract green incl. five-op proofs `persist_roundtrip`,
  `open_scan`, plus `concurrency_and_kill` (16 readers + owner kill +
  post-kill liveness) on a real BEAM. Scope note: the acceptance-4
  mutable-mint-refusal clause is witnessed structurally (typed `error`
  refusals: open-after-consume, bounds, arity; binaries immutable), not
  as a separately named contract proof.

Net: **5 PASS / 2 PARTIAL**, up from wave 8's 3 / 4, equal to wave 7's
best. Both PARTIALs are decision-gated, not engineering-gated: FR-05
closes on ADR-14 adjudication discipline (gate and manifest already run
green), FR-06 closes on the operator ratification flip. No rounding up:
FR-05's remainder adjudication and FR-06's unratified LSL are stated as
found, and FR-07's PASS carries its scope note.

## 3. CTQ Table (fresh `ctq_report` JSON, exit 1, this session)

| CTQ | Metric | Fresh value (this run) | Gate | Verdict |
|-----|--------|------------------------|------|---------|
| CTQ-02 | alloc events, 64 MiB BLAKE3 pass | 0 events / 0 bytes | REAL | **PASS** |
| CTQ-03 | parquet level-free roundtrip | 47637.92 GiB/s (41 ns min) | informational | **REPORT** |
| CTQ-04 | transpose/lookup variance | CV 6.87% > 2% gate | REAL (PROV.) | **SKIP-NOISY** |
| CTQ-05 | e2e admission (16 KiB batch) | 8.208 µs vs USL 35 µs | REAL | **PASS** |
| CTQ-06 | BLAKE3 thr. (min-of-5) | wide 0.43; many 1.85 vs LSL 8.5 | REAL | **FAIL** (both) |

CTQ-04 honest note: this run degraded to SKIP-NOISY (gage_reps 500,
PASS/FAIL not evaluated) where wave 8's quiet-machine run evaluated PASS
(p99/p50 1.134 vs USL 1.30, CV 0.64%, receipt gate-ctq-report-v2). That
is the load gate working as designed, not a product regression — the
battery ran while residual wave-9 lane load was still settling. It is
also live evidence for the ratification package's evidence condition:
CTQ-04 numbers are valid only from a quiet machine.

### C_pk summary (`cpk_study`, fresh run, exit 1)

| CTQ | n | Overall Cpu/Cpl | Target | Verdict |
|-----|---|-----------------|--------|---------|
| CTQ-05 admission latency | 200 | Cpu 5.845 | >= 1.67 | **PASS** |
| CTQ-06 throughput | 100 | Cpl −869.67 vs LSL 8.5 | >= 1.67 | **FAIL** (spec limit; CV 0.14%) |
| CTQ-04 per-run time | 100 | Cpu 1.106 | >= 1.67 | **FAIL** (load; W8 quiet: 2.31 PASS) |

CTQ-06 row detail: n=100, mean 1.8465 GiB/s, sigma_overall 0.00255 —
the process is tight and centered far above the PROPOSED LSL 1.0; the
FAIL is the unratified spec limit, not the process, per
[59-ratification-package.md](59-ratification-package.md).

Overall cpk verdict FAIL on CTQ-06 (as written) and the load-noised
CTQ-04 row. The capability study corroborates that the throughput
process is tight and centered far above the PROPOSED LSL 1.0 — the
failure is the spec limit, per
[59-ratification-package.md](59-ratification-package.md).

## 4. Open-Items Ledger (reduced to operator items)

Wave-8 ledger items 3 (parquet default build regression) and 4 (FR-07
five-op surface) are **closed this wave** by observed execution, and
item 6's court families closed per the [52-courts-audit.md](52-courts-audit.md)
addendum (COURT-UNPROVEN 3 -> 0; COURT-LIVE 24). The ledger reduces to:

| # | Item | State | Owner |
|---|------|-------|-------|
| 1 | Execute the ratification package (§1) | ASSEMBLED (59) | **operator** |
| 2 | Quiet-machine re-run: ctq_report + cpk_study | pending | **operator** |
| 3 | Integration commit (tree dirty, 70 entries) | pending | **operator** |
| 4 | FR-05/ADR-14 remainder adjudication | discipline | engineering (bounded) |
| 5 | cfg-homology delta (+17 vs slack 4) | tracked | engineering (bounded) |

Item 1 detail — the four actions, one decision, per package §1: flip
`LSL_RATIFIED` to `Some(1.0)` in `crates/bench/src/bin/ctq_report.rs`;
amend the PRD CTQ-06 row to 1.0/1.5/2.5 with the NEON label; mark the
PRD amendment section RATIFIED; ratify the CTQ-04 p99/p50 protocol
(PROVISIONAL -> RATIFIED, 15% pin stays retired). The const's own
doc-comment names the flip operator-only; no lane may perform it.
Item 2 detail — this run's CTQ-04 degraded to SKIP-NOISY under residual
lane load; the wave-8 quiet run evaluated PASS (1.134). The evidence
ladder needs the quiet condition.

Nothing else is open: no compile-abort, no failing test, no
COURT-UNPROVEN family.

## 5. Overall Standing

Vocabulary applied, definition per operating doctrine: ALIVE requires
observed execution of the exact admitted subject with all gates green;
PARTIAL_ALIVE = the tree builds, the full battery executes, gates exist
and run, but named gates are red or ratification is unexecuted. This
run: **not ALIVE** — CTQ-06 is FAIL against the limit as written and the
ratification package is unexecuted (operator-only flip). **Not
BUILD_BROKEN** — workspace check exit 0 and, for the first time, every
feature configuration of every crate compiles and runs its courts. **Not
BLOCKED** — every remaining item has a named owner and a bounded action.

**PARTIAL_ALIVE.** No rounding up: the release claim stays PARTIAL until
the operator executes the ratification package (items 1–2) and lands the
integration commit (item 3). On this evidence, that closes the tree:
589/0/2 across the battery, 5/7 FRs PASS, and the two remaining
PARTIALs are decision-gated with their decision package assembled and
their gates running honestly.

## See Also

- [58-final-verdict-v2.md](58-final-verdict-v2.md) (superseded)
- [59-ratification-package.md](59-ratification-package.md) (decision package, ASSEMBLED)
- [55-final-integration-report.md](55-final-integration-report.md) (superseded)
- [48-lsl-rederivation.md](48-lsl-rederivation.md)
- [49-ctq04-variance-study.md](49-ctq04-variance-study.md)
- [28-beam-nif-handoff.md](28-beam-nif-handoff.md)
- [52-courts-audit.md](52-courts-audit.md)
- [31-verification-ladder.md](31-verification-ladder.md)
- [receipts/](receipts/)

> Erratum (wave-10 gap closure, 2026-10-31): the §3 "C_pk summary (`cpk_study`, fresh run)" table is a receipt-less in-doc table, superseded by [receipts/gate-cpk-study-v3.md](receipts/gate-cpk-study-v3.md) (minted 2026-10-31T05:32:07Z, FAILED exit 1 on CTQ-06 only).
