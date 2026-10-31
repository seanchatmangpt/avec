# Milestone Verdict: v26.10.31 (wave-6, lane 20)

Definitive battery, executed sequentially 2026-10-30 19:47–20:05 PDT (after 15 min
machine-quiet wait), on the canonical checkout `/Users/sac/avec`, branch `main`
(recent head `ead4032` at run start; working tree dirty per lane-20 task contract).
Every number below is re-read from this session's actual command output — no counts
recalled from prior receipts.

## Battery Receipts

| # | Command | Exit | Key result |
|---|---------|------|------------|
| 1 | `cargo check --workspace` | 0 | clean check, 0.68s (cached) |
| 2 | `cargo test -p avec-arrow --lib` | 0 | 93 passed / 0 failed |
| 3 | `cargo test -p avec-parquet --features std` | 0 | 0 tests collected under `std` feature (see note) |
| 3' | `cargo test -p avec-parquet` (default features) | 0 | 157 lib + 4 + 17 + 2 + 8 + 3 integration = 191 passed / 0 failed |
| 4 | `cargo test -p avec-git` | 0 | 43 lib + fsck_court 2/2 + git_interop 3/3 = 48 passed / 0 failed |
| 5 | `cargo test -p avec-blake3 -- --test-threads=1` | 0 | 3 lib + 1 + 6 (1 ignored) + 6 official vectors + 3 small-batch + 4 zero-alloc = 23 passed / 0 failed |
| 6 | `cargo test -p avec-audit` | 0 | 5 passed / 0 failed |
| 7 | `cargo test -p avec-beam` | 0 | 9 lib + 1 bridge_contract (real BEAM escript) = 10 passed / 0 failed |
| 8 | `cargo test -p avec-integration` | 0 | 2 admission_e2e + 6 cross_crate + 4 membranes = 12 passed / 0 failed |
| 9 | `bash scripts/validate_ontology.sh` | 0 | Conforms: True; anti-vacuity mutant rejected (REFUSAL-13) |
| 10 | `cargo build --release -p bench && target/release/ctq_report` | 1 | JSON verdict `FAIL`, failed_gates `[CTQ-04, CTQ-06]` |

Note on row 3: `--features std` collects 0 tests (the parquet test files are gated on
default features, not `std`). Row 3' is the substantive run and is the evidence of
record for FR-02/FR-03.

## Per-FR Verdicts (FR-01..07)

| FR | Requirement | Verdict | Evidence (command + exit + key number) |
|----|-------------|---------|----------------------------------------|
| FR-01 | Columnar Alignment (64 B, no cache-line splits) | **PASS** | `cargo test -p avec-arrow --lib` exit 0, 93/0 incl. `zero_copy_1000_slices_of_1m_rows_allocate_zero` |
| FR-02 | Non-Nullable Layout (sentinel, no bitmap loops) | **PASS** | arrow 93/0 (exit 0) + parquet default-features run exit 0, 191/0 incl. page_zero_alloc 2/2 |
| FR-03 | Parquet Projection (zero-copy, level-free, isomorphic) | **PASS** | parquet exit 0, 191/0 incl. rle_court 8/8, mutation_kills 17/17, aligned_interop 4/4 |
| FR-04 | Packfile CAS (verified by unmodified git) | **PASS** | avec-git exit 0: fsck_court 2/2 (real `git fsck`), git_interop 3/3 |
| FR-05 | Unrolled Search Ladder (zero conditional jumps) | **PARTIAL** | gate infra green (branchless selftest PASS per W6 matrix); banned-opcode census residual in hot symbols — not re-audited in this battery (owner lanes: transpose/predicates-branchless) |
| FR-06 | Parallel BLAKE3 Tree (>10 GiB/s per core, spec-compliant) | **PARTIAL** | vectors green (official 6/6, multi-chunk 6/6, small-batch 3/3, exit 0); throughput FAIL: CTQ-06 best 1.85 GiB/s vs LSL 8.5 GiB/s |
| FR-07 | Sub-BEAM Handoff (zero-copy NIF, no serialization) | **PASS** | avec-beam exit 0: bridge_contract 1/1 (alloc_spy, dtor_stats, real escript); avec-audit 5/0 |

## Per-CTQ Verdicts (from `ctq_report` JSON, exit 1)

| CTQ | Metric | Value | Gate | Verdict |
|-----|--------|-------|------|---------|
| CTQ-02 | alloc_events (64 MiB BLAKE3 pass) | 0 events / 0 bytes | REAL | **PASS** |
| CTQ-03 | parquet level-free roundtrip | 23,531.99 GiB/s (83 ns min) | informational | REPORT |
| CTQ-04 | transpose/lookup variance (stdev/mean, n=4096, 100 runs) | 21.93% vs USL 15% | REAL (PROVISIONAL pin) | **FAIL** |
| CTQ-05 | e2e admission (16 KiB batch, path hash_many_simd) | 8.208 µs vs USL 35 µs | REAL | **PASS** |
| CTQ-06 | BLAKE3 throughput, min-of-5 / 256 MiB | hash_wide_simd 0.43 GiB/s; hash_many_simd 1.85 GiB/s vs LSL 8.5 | REAL | **FAIL** (both rows) |
| — | C_pk capability study | no study executed this battery | — | UNVERIFIED (Gage R&R lane not landed) |

CTQ-04 note (honest regression signal): the W6 traceability refresh recorded CTQ-04 as
"provisional pin PASS"; this battery measured 21.9% stdev/mean vs the 15% USL and the
bin itself prints `pin_status: PROVISIONAL pending DFLSS ratification`. Recorded as
FAIL per no-rounding-up. The pin is provisional, so this is a FAIL against a
not-yet-ratified limit — carried on the red-list, not silently dropped.

## Red-List (explicit, fail-closed)

1. **CTQ-06 BLAKE3 throughput — FAIL.** Best 1.85 GiB/s (`hash_many_simd`, min-of-5
   over 256 MiB) vs LSL 8.5 GiB/s. Digest correctness green (all vector courts pass),
   so the gap is performance, not correctness. Trajectory 0.43 → 1.18 → 1.85 GiB/s
   across W5/W6; kernel lanes have not closed it. Blocks FR-06 full PASS and blocks
   any full v26.10.31 release claim.
2. **CTQ-04 variance — FAIL this battery.** 21.9% stdev/mean vs 15% provisional USL.
   New fail signal this run (W6 matrix recorded a provisional PASS); the measure is
   a provisional pin, so the gap is provisional-limit exceedance, but it stands as
   FAIL. Needs either a ratified limit or a variance fix before release claim.
3. **FR-05 banned-opcode residual.** Branchless audit selftest court green, but the
   hot-symbol opcode census residual from W6 remains open; not re-measured here.
   Keeps FR-05 at PARTIAL.
4. **`cargo test -p avec-parquet --features std` collects 0 tests.** Exit 0 but
   informationally empty; the std feature does not gate any tests. Recorded so the
   battery row is not mistaken for a substantive pass.
5. **C_pk capability study** never executed; Gage R&R harness exists
   (`crates/bench/src/bin/gage_rr.rs`) but no study ran and no receipt minted.
   CTQ table stays UNVERIFIED on that row.
6. **Branchless gate remainder is informational** per W6 (selftest court green;
   census remainder tracked, not a red gate). Included here so the release claim
   math is explicit.

## Overall Standing

**PARTIAL_ALIVE.**

Definition applied (per 31-verification-ladder.md vocabulary): ALIVE = every FR gate
and every REAL CTQ gate observed passing this session on the exact subject with
receipts; PARTIAL_ALIVE = the tree builds, the full test battery is green, and the
majority of FR/CTQ gates pass on fresh runs, but at least one REAL gate fails
measurably. This battery: 5/7 FR PASS, 2/7 PARTIAL (FR-05, FR-06); 3/4 REAL CTQ
gates PASS, CTQ-04 and CTQ-06 FAIL. A FAIL is a FAIL: no rounding up.

## Gap List to a Full v26.10.31 Release Claim

1. Close CTQ-06: BLAKE3 throughput to ≥ 8.5 GiB/s on the pinned bench (both rows or
   the pinned row per 04-benchmark-contract.md), with a minted receipt.
2. Resolve CTQ-04: either DFLSS-ratify a variance USL consistent with observed
   variance, or fix transpose/lookup variance to ≤ 15% stdev/mean.
3. Close FR-05: drive the banned-opcode census on hot symbols to zero (or to the
   ratified budget) via `scripts/audit_branchless.sh` and mint the receipt.
4. Execute the C_pk capability study (gage_rr.rs) and ratify C_pk ≥ 1.67, minting a
   receipt (CTQ table row UNVERIFIED today).
5. Re-run `verify_v26` full battery (`cargo build -p avec-cli --bin verify_v26 &&
   target/debug/verify_v26`) after 1–4 land, mint the integration receipt, and
   refresh 32-traceability-matrix.md and 36-release-checklist.md from that run.

## See Also

- [32-traceability-matrix.md](32-traceability-matrix.md)
- [04-benchmark-contract.md](04-benchmark-contract.md)
- [31-verification-ladder.md](31-verification-ladder.md)
- [43-wave-5-receipt.md](43-wave-5-receipt.md)
- [36-release-checklist.md](36-release-checklist.md)
- receipts/ (append-only, script-minted only)

## Wave-7/8 addenda (2026-10-30, lane 12)

Appended block; the battery tables above are historical and not edited in place.

- CTQ-04 FAIL row superseded. Root cause was a contention artifact, not kernel
  variance; SKIP-NOISY load-gating landed, `49-ctq04-variance-study.md` documents
  the mechanism, and quiet runs PASS (cpk_study CTQ-04: CV 1.16% vs USL 15%).
- CTQ-06 rows still FAIL (hash_wide 0.43 / hash_many 1.85 GiB/s vs LSL 8.5), but
  the LSL itself is under PROPOSED amendment in `48-lsl-rederivation.md` — the
  FAIL verdict stands against the unamended limit.
- C_pk row no longer UNVERIFIED. `receipts/gate-cpk-study.md` minted FAILED
  (exit 1, 2026-10-31T03:03:46Z): CTQ-05 Cpu 9.63 PASS, CTQ-04 Cpu 4.33 PASS,
  CTQ-06 Cpl -254.8 FAIL — the C_pk breach is CTQ-06's, recorded on its row.
- FR-05 PARTIAL → closed-by-manifest: `receipts/gate-branchless-manifest.md`
  ALIVE; census remainder is manifest-bounded INFORMATIONAL per ADR-14.
- Parquet `--features std` collects-0-tests anomaly resolved as a doc-test parse
  artifact; `std_feature_court` landed (`crates/avec-parquet/tests/`).
- Perf-guard standing: `receipts/gate-perf-guard.md` ALIVE (exit 0,
  2026-10-31T03:07:01Z) — PERF-GUARD-PASS CTQ-05=8.208 us <= 30.0 us, gage
  cv_trimmed 0.8833% < 2.0%; the ctq_report exit=1 inside its evidence is
  CTQ-06, explicitly out of that guard's scope (guard remains CTQ-05/gage only).
- CTQ-04 addendum superseded (wave 9): the 15%-pin framing above is
  retired — the protocol is now the gage-conditioned p99/p50 ≤ 1.30
  statistic (49-ctq04-variance-study.md Option b): SKIP-NOISY under load,
  evaluates only on a quiet machine, quiet PASS 1.134 vs USL 1.30.
  DFLSS ratification of the swap remains open.
- FR-07 (wave 9): the five-op surface landed wave-8 (ops_scan.rs +
  ops_persist.rs, registered lib.rs:570-608); FR-07 is restored PASS —
  battery rows above that read PARTIAL/SKIP for the five-op gap are
  superseded by 56 §Resolution.
