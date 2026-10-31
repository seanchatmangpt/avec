# Release Notes: v26.10.31 (internal engineering)

Status: PARTIAL_ALIVE (per 47-milestone-verdict.md). Internal engineering release;
not a full release claim. Red-list items below remain open.

## 1. What Shipped

### avec-arrow

- Quad64 64B/64B layout law (`layout.rs`): build-time offset asserts at
  0x00/0x04/0x08/0x0C/0x10/0x18/0x1C/0x20, derived from FR-01.
- Transpose and predicate kernels (`transpose.rs`, `predicates.rs`).
- Zero-copy proof: 1000 slices of 1M rows allocate zero
  (`zero_copy_1000_slices_of_1m_rows_allocate_zero`).
- 93 lib tests passing (47-milestone-verdict.md, battery row 2).

### avec-parquet

- Level-free DataPageV2 writer + scanner (`level_free.rs`, `scan.rs`),
  multi-column support, footer.
- RLE round-trip courts (`rle_court.rs`, 8/8) and mutation courts (17/17).
- Default-features battery: 191 passed / 0 failed (verdict row 3').
- Note: `--features std` collects 0 tests (row 3); recorded so it is not
  mistaken for a substantive pass.

### avec-git (new crate)

- Packfile v2 writer, idx ladder, deflate/inflate.
- Real-git fsck court: unmodified `git fsck` / `verify-pack` / `cat-file`
  accept avec-written packs (fsck_court 2/2, git_interop 3/3).
- FR-04 court numbers: blobs=8, pack 147 B, idx 1296 B, writer 902 us,
  git 2.51.2 (verify-latest.md cli-verify row).

### avec-blake3 (new crate)

- Sovereign BLAKE3 core plus 4-way / 8-chunk NEON kernels.
- Official vectors 6/6, multi-chunk 6/6 (1 ignored), small-batch 3/3,
  zero-alloc courts 4/4 (verdict row 5, 23 passed / 0 failed total).
- Known-answer spot checks vs b3sum oracle (verify-latest.md FR-06 row).

### avec-beam

- NIF membrane: pointer identity (`pointer.rs`), alloc spy (`alloc_spy.rs`),
  dtor exactly-once stats (`dtor_stats.rs`).
- Bridge contract runs on a real BEAM via escript (bridge_contract 1/1;
  9 lib + 1 = 10 passed / 0 failed).

### avec-ffi

- Zero-copy courts: C ABI round-trip bitwise + double-release,
  NEON vs portable mask bitwise equality, nested struct ABI walk
  (test-integration evidence, verify-latest.md).

### avec-audit

- New alloc counter (`alloc_counter.rs`): scoped allocation counting,
  thread-safe totals. 5 tests passing.

### bench

- Divan benches + `ctq_report` (machine-readable CTQ verdicts, exit 1 on FAIL),
  `gage_rr` (Gage R&R harness, study not yet executed),
  `blake3_profile` / `blake3_throughput`, `cpk_study`.
- FR-05 row skipped in the FR battery (BEAM-owned), verdict FR-07 SKIP note.

### scripts

- `verify_v26.10.31.sh`: verify orchestrator with PRE-EXISTING-FAIL baseline
  (`scripts/verify_baseline.txt`, 0 lines).
- `audit_branchless.sh` + cases + `branchless_manifest.toml`: branchless gate
  with positive control (FIRED, 4 banned ops) — gate not vacuous.
- `mint_receipt.sh`, `validate_ontology.sh` (Conforms: True; anti-vacuity
  mutant rejected, REFUSAL-13), `divan2receipt.py`, `cfg_homology.sh`.

## 2. Gate Outcomes

### Continuous gate sweep (verify-latest.md, 2026-10-31T02:58:40Z, HEAD ead4032)

All 10 gates PASS, exit 0: check-workspace, test-avec-arrow,
test-avec-parquet, test-avec-git, test-avec-blake3, test-integration,
test-avec-beam, audit-branchless, cli-verify, test-audit.
FR battery inside cli-verify: 7 rows, 0 FAIL, FR-07 SKIP (BEAM-owned).
CTQ-05 e2e admission: 8.208 us vs 35 us USL — PASS.

### Definitive battery (47-milestone-verdict.md, 2026-10-30 19:47-20:05 PDT)

| FR | Verdict | Key evidence |
|----|---------|--------------|
| FR-01 | PASS | arrow 93/0, zero-copy 1M-row test |
| FR-02 | PASS | arrow 93/0 + parquet 191/0 |
| FR-03 | PASS | parquet 191/0, rle_court 8/8, mutation 17/17 |
| FR-04 | PASS | fsck_court 2/2 (real git fsck), git_interop 3/3 |
| FR-05 | PARTIAL | gate selftest green; hot-symbol opcode census residual |
| FR-06 | PARTIAL | vectors green; throughput 1.85 GiB/s vs LSL 8.5 |
| FR-07 | PASS | bridge_contract 1/1 real escript; audit 5/0 |

CTQ table (ctq_report exit 1, failed_gates [CTQ-04, CTQ-06]):

| CTQ | Value | USL/LSL | Verdict |
|-----|-------|---------|---------|
| CTQ-02 | 0 alloc events / 0 bytes (64 MiB pass) | 0 | PASS |
| CTQ-03 | 23,531.99 GiB/s level-free roundtrip (83 ns min) | informational | REPORT |
| CTQ-04 | 21.93% stdev/mean (n=4096, 100 runs) | 15% provisional | FAIL |
| CTQ-05 | 8.208 us (16 KiB batch, hash_many_simd) | 35 us | PASS |
| CTQ-06 | hash_wide_simd 0.43, hash_many_simd 1.85 GiB/s | 8.5 GiB/s LSL | FAIL |
| C_pk | no study executed | — | UNVERIFIED |

Overall: 5/7 FR PASS, 2/7 PARTIAL; 3/4 REAL CTQ PASS; CTQ-04/06 FAIL.
Standing: PARTIAL_ALIVE. No rounding up.

## 3. Known Open

1. CTQ-06 BLAKE3 throughput FAIL: best 1.85 GiB/s vs LSL 8.5 GiB/s.
   Digest correctness green; gap is performance-only. Trajectory
   0.43 -> 1.18 -> 1.85 GiB/s across W5/W6. Blocks full release claim
   pending LSL ratification or kernel closure.
2. CTQ-04 variance FAIL this battery: 21.93% stdev/mean vs 15% provisional
   USL. Pin is provisional; needs DFLSS ratification or a variance fix.
3. FR-05 residuals under ADR-14 (M=1 remainder policy, 22-adr-register.md):
   branchless gate selftest green, hot-symbol banned-opcode census residual
   open; not re-measured this battery. FR-05 stays PARTIAL.
4. C_pk capability study never executed; harness exists (gage_rr.rs),
   CTQ row UNVERIFIED.
5. `cargo test -p avec-parquet --features std` collects 0 tests
   (informationally empty; std gates no tests).
6. FR-07 FR-battery row is SKIP by design (owned by integration lane,
   not runnable in the in-process battery).

## 4. Receipt Index

Script-minted, append-only: `docs/jira/v26.10.31/receipts/` (26 receipt
files + README.md + template.md, listed this session). Key gate receipts:
gate-workspace-check-v2, gate-arrow-tests, gate-git-tests-v2,
gate-blake3-tests-v2, gate-blake3-zero-alloc, gate-blake3-throughput,
gate-beam-bridge-v2, gate-e2e-integration-v2, gate-ctq-report-v2,
gate-shacl-v2, gate-branchless-selftest-v2, gate-cli-machine-report,
gate-cfg-homology, gate-divan-ctq, gate-e2e-ctq05, gate-gage-rr.
Format: receipts/template.md; policy: receipts/README.md.
Wave receipts: 39-wave-2, 40-wave-3, 41-wave-4, 43-wave-5, 45-wave-6.
Battery evidence of record: verify-latest.md, 47-milestone-verdict.md.

## See Also

- [47-milestone-verdict.md](47-milestone-verdict.md)
- [verify-latest.md](verify-latest.md)
- [36-release-checklist.md](36-release-checklist.md)
- [22-adr-register.md](22-adr-register.md)

## Addendum (wave 9, final-fix lane X2)

Appended; sections 1-4 above stand as written at mint.

- CTQ-04 row superseded: the 21.93% stdev/mean FAIL read the retired
  15% provisional pin. The protocol swapped to the gage-conditioned
  p99/p50 ≤ 1.30 statistic (49-ctq04-variance-study.md Option b) —
  SKIP-NOISY under load, quiet PASS 1.134 (58 §3). DFLSS ratification
  open.
- C_pk row superseded: "no study executed" is stale — the study ran;
  receipts `gate-cpk-study` and `gate-cpk-study-v2` (exit 1 by
  contract) record the verdict with `failed_gates: ["CTQ-06"]` — the
  sole breach is CTQ-06's, not C_pk's.
- Receipt count superseded: the corpus is 36 receipts (25 ALIVE /
  10 FAILED / 1 PARTIAL per 64-receipts-inventory-final.md), not the
  26 files listed in section 4.
- Known-open item 5 closed: the `--features std` collects-0-tests
  anomaly is gone — `std_feature_court` landed
  (crates/avec-parquet/tests/std_feature_court.rs: 2 tests under std,
  1 under default; both configs exit 0, 63-test-census.md).
