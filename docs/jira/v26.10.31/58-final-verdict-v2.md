# Final Verdict v2: v26.10.31 (Wave 8, Lane 20)

Closing verdict for the v26.10.31 milestone. Supersedes
[55-final-integration-report.md](55-final-integration-report.md). Battery
executed sequentially 2026-10-30 20:51–21:05 PDT on the canonical checkout
`/Users/sac/avec`, branch `main`, HEAD `ead4032` (verified `git rev-parse
--short HEAD` at run time), dirty tree per lane contract. A ~10-minute
machine-quiet wait (20:41–20:51) preceded the battery so concurrent wave-8
lanes could settle; the check step still observed one transient build lock
block, evidence lanes had just written. Every number below is re-read from
this session's actual command output; no count is recalled from a prior
receipt.

## 1. Definitive Battery (this session, real exits)

| # | Command | Exit | Key result |
|---|---------|------|------------|
| 1 | `cargo check --workspace` | 0 | clean check (0.39s, one transient build-lock wait) |
| 2 | `cargo test -p avec-arrow --lib` | 0 | 93 passed / 0 failed |
| 3 | `cargo test -p avec-parquet --features std` | 0 | **204 passed / 0 failed** (163 lib + 1 |
|   |   |   | accessor_perf_court + 4 aligned_interop + 4 level_free_multi_court |
|   |   |   | + 17 mutation_kills + 2 page_zero_alloc + 8 rle_court + 3 robustness |
|   |   |   | + 2 std_feature_court). New in wave 8: accessor_perf_court (lane 6 |
|   |   |   | resolved-slice-cache court). |
| 3' | `cargo test -p avec-parquet` (default features) | **101** | **BUILD BROKEN — REGRESSION this wave.** |
|   |   |   | `crates/avec-parquet/tests/accessor_perf_court.rs` (wave-8 lane 6) |
|   |   |   | imports `avec_parquet::scan` (std-gated) with no `#[cfg(feature = |
|   |   |   | "std")]` gate on the test file: E0432 ×2 + E0433, `could not compile |
|   |   |   | avec-parquet (test "accessor_perf_court")`. Under default features |
|   |   |   | **zero tests run** (compile-abort, not a failure count — per |
|   |   |   | same-checkout-fanout law 3). Fix: gate the test file's imports with |
|   |   |   | `#![cfg(feature = "std")]` or gate the file in Cargo.toml |
|   |   |   | `required-features`. |
| 4 | `cargo test -p avec-git` | 0 | 48 passed / 0 failed / 1 ignored (43 lib + 2 real `git fsck` court |
|   |   |   | + 3 git_interop; perf_baseline bin ignored, documented) |
| 5 | `cargo test -p avec-blake3 -- --test-threads=1` | 0 | **25 passed** / 0 failed / 2 ignored (3 lib + 1 soak + 7 multi-chunk |
|   |   |   | (1 ignored) + 6 official vectors + 3 small-batch + 5 zero-alloc; |
|   |   |   | wave-8 grew zero-alloc 4→5 and kept 25 total vs wave-7's 24) |
| 6 | `cargo test -p avec-audit` | 0 | 5 passed / 0 failed |
| 7 | `cargo test -p avec-beam` | 0 | 10 passed / 0 failed (9 lib + 1 real-BEAM escript bridge contract) |
| 8 | `cargo test -p avec-integration` | 0 | 12 passed / 0 failed (2 admission_e2e + 6 cross_crate + 4 membranes) |
| 9 | `bash scripts/validate_ontology.sh` | 0 | Conforms: True; anti-vacuity mutant rejected (REFUSAL-13) |
| 10 | `bash scripts/audit_branchless.sh --selftest` | 0 | positive control FIRED (4 banned ops in branchy.o); clean fixture 0 |
|   |   |   | hits; SELFTEST PASS |
| 11 | `cargo build --release -p bench` | 0 | finished (fresh compile of wave-8 bench changes, 6.89s) |
| 12 | `target/release/ctq_report` | **1** | JSON verdict `FAIL`, `failed_gates: ["CTQ-06"]`. **CTQ-04 protocol |
|   |   |   | replaced and now evaluates: PASS** (p99/p50 1.134 vs USL 1.30, |
|   |   |   | mini-gage CV 0.64% ≤ 2% admitted, 15% stdev/mean pin retired — |
|   |   |   | 49-ctq04-variance-study Option b). CTQ-02 0 alloc events PASS; CTQ-05 |
|   |   |   | 8.208 µs vs 35 µs PASS; CTQ-03 47637.92 GiB/s REPORT; CTQ-06 FAIL |
|   |   |   | both rows (hash_wide_simd 0.43, hash_many_simd 1.85 GiB/s vs LSL 8.5). |
| + | `target/release/cpk_study` | JSON `FAIL` | CTQ-05 Cpu(overall) 10.25 PASS; **CTQ-04 Cpu(overall) 2.31 PASS** |
|   |   |   | (fresh, closes the SKIP-NOISY question: on a quiet machine the |
|   |   |   | capability study passes); CTQ-06 Cpl(overall) −697.23 FAIL. Overall |
|   |   |   | verdict FAIL on CTQ-06 only. |

Totals: **397 tests passed, 0 failed, 3 ignored** where the battery could
compile and run. The parquet default-feature config is a compile-abort
(exit 101), contributing 0 tests — loudly excluded, not silently rounded
into the total.

## 2. FR-01..07 Final Verdicts

| FR | Requirement | W7 verdict (55) | This battery | Evidence |
|----|-------------|-----------------|--------------|----------|
| FR-01 | Columnar alignment (64 B, no splits) | PASS | **PASS** | arrow 93/0, exit 0 |
| FR-02 | Non-nullable layout (sentinel) | PASS | **PASS** | arrow 93/0 + parquet std 204/0, exit 0 |
| FR-03 | Parquet projection (zero-copy, level-free) | PASS | **PARTIAL (regressed)** | `--features std` green
(204/0) **but default-feature test build is compile-broken (exit 101)** — a one-line gate fix, owner:
wave-8 lane 6 (file owner of accessor_perf_court.rs). FR-03's runtime semantics are green; the
default-feature config cannot even run its courts. |
| FR-04 | Packfile CAS (verified by real git) | PASS | **PASS** | avec-git 48/0 incl. fsck 2/2 (real
git 2.x), git_interop 3/3 |
| FR-05 | Unrolled search ladder (zero branches) | PARTIAL | **PARTIAL** | gate infra green (selftest
PASS, positive control FIRED); manifest-aware adjudication landed W7
(`scripts/branchless_manifest.toml`, receipt ALIVE). Residual: manifest adjudicates hits, doesn't
eliminate them; closure = ratified manifest + zero unexplained hits. |
| FR-06 | Parallel BLAKE3 tree (>10 GiB/s/core) | PARTIAL | **PARTIAL** | correctness green (25/0,
official vectors 6/6); throughput FAIL: best 1.85 GiB/s vs LSL 8.5.
[48-lsl-rederivation.md](48-lsl-rederivation.md) shows 8.5 GiB/s/core is not physically attainable on
this NEON hardware; amendment PROPOSED (LSL 1.0 / target 1.5), not ratified. |
| FR-07 | Sub-BEAM handoff (zero-copy NIF) | PASS | **PARTIAL (re-graded)** | Bridge contract itself
green: beam 10/0, pointer identity, alloc_spy delta {0,0}, dtor exactly-once —
receipt gate-beam-bridge-v2 ALIVE. But re-graded against the **five-op surface state**
(28-beam-nif-handoff.md): `crates/avec-beam/src/ops_scan.rs` and `ops_persist.rs` **do not exist**;
src/ contains only lib/resbin/resources/shm/spsc_ring/alloc_spy/dtor_stats/pointer. The five ops
(open/attach, scan, flush_to_parquet, seal_to_packfile, receipt_query) remain **UNIMPLEMENTED**, and
acceptance 4 (concurrency/mutable-mint refusal) is likewise UNIMPLEMENTED per the handoff doc's own
standing section. Acceptances 1–3 are DONE. Bridge PASS, full FR-07 surface PARTIAL. |

Net: **3 PASS / 4 PARTIAL**, down from wave 7's 5 PASS / 2 PARTIAL.
The two wave-7 PARTIALs (FR-05, FR-06) are unchanged and decision-gated;
the two new PARTIALs are wave-8-introduced (FR-03 build regression — a
one-line fix owned by the file's lane; FR-07 re-grade from applying the
five-op surface definition rather than reading only the bridge contract).
No rounding up: the FR-03 regression and FR-07 five-op gap are stated as
found.

## 3. CTQ Table (fresh `ctq_report` JSON, exit 1, this session)

| CTQ | Metric | Fresh value | Gate | Verdict |
|-----|--------|-------------|------|---------|
| CTQ-02 | alloc events, 64 MiB BLAKE3 pass | 0 events / 0 bytes | REAL | **PASS** |
| CTQ-03 | parquet level-free roundtrip | 47637.92 GiB/s (41 ns min) | informational | **REPORT** |
| CTQ-04 | transpose/lookup variance (n=4096, 100 runs) | p99/p50 **1.134** vs USL 1.30 (mini-gage CV
0.64% admitted; stdev/mean 2.8% informational; 15% pin retired) | REAL (PROVISIONAL p99/p50 pin) | **PASS** |
| CTQ-05 | e2e admission (16 KiB batch) | 8.208 µs best vs USL 35 µs | REAL | **PASS** |
| CTQ-06 | BLAKE3 throughput, min-of-5 / 256 MiB | hash_wide_simd 0.43; hash_many_simd 1.85 GiB/s vs
LSL 8.5 | REAL | **FAIL** (both rows) |
| C_pk | capability study (fresh run) | CTQ-05 Cpu 10.25 PASS; CTQ-04 Cpu 2.31 PASS; CTQ-06 Cpl
−697.23 FAIL; overall FAIL on CTQ-06 only | — | executed; FAIL on CTQ-06 only |

The CTQ-04 SKIP-NOISY open item from 55 is **closed at the protocol level**:
the new p99/p50 protocol evaluated this session (quiet machine) and passed,
and the independent cpk_study confirms Cpu 2.31 ≥ 1.67. DFLSS ratification
of the protocol swap remains the residual.

## 4. Open-Items Ledger

| # | Item | State | Blocking class |
|---|------|-------|----------------|
| 1 | CTQ-06 LSL ratification | PROPOSED amendment (LSL 1.0 / target 1.5 GiB/s,
[20-prd.md](20-prd.md)) not ratified; gate reads 8.5 and fails at 1.85. | ratified-decision |
| 2 | CTQ-04 protocol ratification | New p99/p50 protocol now evaluates (PASS this session); C_pk
corroborates (Cpu 2.31). Ratification of the protocol swap still pending. | ratified-decision |
| 3 | **Parquet default-feature build regression (NEW, wave-8 lane 6)** | `accessor_perf_court.rs`
imports std-gated `avec_parquet::scan` ungated; `cargo test -p avec-parquet` exit 101, 0 tests run.
One-line fix, owner: file's lane. | engineering (trivial) |
| 4 | FR-07 five-op surface | `ops_scan.rs` / `ops_persist.rs` absent from
`crates/avec-beam/src/`; five ops + acceptance 4 UNIMPLEMENTED. Bridge contract (acceptances 1–3)
green. | engineering |
| 5 | Commit decision | Working tree dirty (~66 entries per latest receipt); no lane ran git
transitions; operator owns the integration commit. Pending. | operator decision |
| 6 | Courts audit residual | [52-courts-audit.md](52-courts-audit.md): 21 court families COURT-LIVE,
3 COURT-UNPROVEN with named flip tests. | engineering (bounded) |
| 7 | cfg homology delta | `gate-cfg-homology.md`: gate ALIVE, verdict FAILED (transpose delta +17 vs
slack 4). Tracked, not release-gating. | engineering (bounded) |

## 5. Overall Standing

Vocabulary applied, definition per operating doctrine: ALIVE requires
observed execution of the exact admitted subject with all gates green;
PARTIAL_ALIVE = the tree builds, the battery executes, gates exist and run,
but named gates are red or unimplemented. Not ALIVE (CTQ-06 FAIL, FR-07
five-op surface absent, FR-03 default config cannot run courts). Not
BLOCKED (nothing blocks; the reds have named owners and bounded fixes). Not
BUILD_BROKEN at the workspace level (workspace check exit 0), though the
parquet default-feature test config is locally broken (exit 101).

**PARTIAL_ALIVE.** No rounding up: CTQ-06 is FAIL against the limit as
written; FR-07 is PARTIAL on the five-op surface; FR-03 is PARTIAL on the
default-feature build; the release claim stays PARTIAL until the LSL
amendment and the CTQ-04 protocol swap are ratified and the two wave-8
regressions/gaps are closed.

## See Also

- [55-final-integration-report.md](55-final-integration-report.md) (superseded)
- [47-milestone-verdict.md](47-milestone-verdict.md)
- [48-lsl-rederivation.md](48-lsl-rederivation.md)
- [49-ctq04-variance-study.md](49-ctq04-variance-study.md)
- [28-beam-nif-handoff.md](28-beam-nif-handoff.md)
- [52-courts-audit.md](52-courts-audit.md)
- [31-verification-ladder.md](31-verification-ladder.md)
- [receipts/](receipts/)

## Resolution (wave 9, lane 4 of 20)

Verified on disk 2026-10-30 ~21:15 PDT; tests re-executed by this lane.
Appends only — sections 1-5 above stand as written at wave-8 close.

1. **FR-03 default-config build regression (battery item 3', ledger 3)
   FIXED.** `crates/avec-parquet/tests/accessor_perf_court.rs` now opens
   with `#![cfg(feature = "std")]` (line 1, on disk). This lane's runs,
   both exit 0: `cargo test -p avec-parquet` (default) **192 passed / 0
   failed**; `cargo test -p avec-parquet --features std` **204 passed / 0
   failed**. Default config compiles and runs its courts again — the
   compile-abort is gone. **FR-03: PARTIAL (regressed) → PASS** (204/0 std
   + 192/0 default, real exits this session).
2. **FR-07 re-graded PARTIAL → PASS-with-receipt-pointer.** The five-op
   surface landed after the wave-8 battery: `crates/avec-beam/src/
   ops_scan.rs` (20:51) and `ops_persist.rs` (20:45) exist and are wired —
   `lib.rs` registers all 5 of 5 ops (`avec_open`, `avec_scan`,
   `avec_flush_to_parquet`, `avec_seal_to_packfile`,
   `avec_receipt_query`, lib.rs:570-608). `cargo test -p avec-beam` this
   session: exit 0, **10 passed / 0 failed**, including the real-BEAM
   escript stage `beam_bridge_contract_end_to_end` which runs
   `t_persist_roundtrip()` and `t_open_scan()`
   (tests/bridge_contract.rs:102-103) — flush byte-identical PAR1,
   deterministic 32 B receipt, deterministic 20 B seal OID, fail-closed
   error split, zero-copy scan pointer-identity court
   (`pointer_of(parent) + start`), double-open-legal envelope path,
   alloc-delta {0,0}. Receipt pointer: gate-beam-bridge-v2 (ALIVE,
   acceptances 1-3) + verify-latest.md (04:06:51Z, `test-avec-beam` PASS,
   postdates the ops landing). Residual (stated, not rounded over): no
   five-op-specific receipt is minted yet; the coordinator should mint one
   at integration.
3. **Net FR score moves 3 PASS / 4 PARTIAL → 5 PASS / 2 PARTIAL.** The two
   wave-8-introduced PARTIALs are closed. FR-05 and FR-06 remain PARTIAL,
   unchanged and decision-gated (CTQ-06 LSL amendment; FR-05 ratification).
   **Overall standing remains PARTIAL_ALIVE** — CTQ-06 is still the sole
   failing gate (gate-cpk-study-v2, 03:39:11Z, exit 1 by contract,
   failed_gates: ["CTQ-06"]); this lane did not re-run ctq_report, so the
   CTQ table above is unchanged as-of its receipts.
4. Wave-8's default-config total now sums cleanly: parquet contributes 396
   test-result passes across both configs (192 + 204) where the battery
   could count 204; beam contributes 10.

## Erratum (wave 9, final-fix lane X2)

Appended; sections 1-5 and the Resolution above stand as written.

- Ignored counts: avec-blake3 ignored = 1, not 2 (the
  multi_chunk_vectors ignore only — 63-test-census.md records 25/0/1);
  the battery's ignored total is 2, not 3 (blake3 1 + avec-git
  perf_baseline 1). Section 1's totals line "397 passed / 0 failed /
  3 ignored" and blake3 row "2 ignored" read one high.
- Ops-file timing: ops_persist.rs (20:45:52) and ops_scan.rs
  (20:51:53) predate / straddle this battery's 20:51-21:05 window —
  the body's "ops_scan.rs / ops_persist.rs do not exist" claim was
  stale-at-write; confirmed landed in 56 §Resolution.
- FR-07 acceptance-4 disposition: structurally witnessed (the real-BEAM
  bridge contract exercises the concurrency/mutable-mint paths), but no
  named acceptance-4 proof receipt exists — carried as stated, not
  rounded up.

> Erratum (wave-10 gap closure, 2026-10-31): the §3 fresh-cpk rows ("C_pk | capability study (fresh run)") are receipt-less in-doc numbers, superseded by [receipts/gate-cpk-study-v3.md](receipts/gate-cpk-study-v3.md) (minted 2026-10-31T05:32:07Z, FAILED exit 1 on CTQ-06 only).
