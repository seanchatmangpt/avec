# Test Census: v26.10.31 (Wave-9 Lane 10)

Complete per-crate test census, wave-9 finish. Window: 2026-10-30 21:15:29–21:19:21 PDT,
host arm64, repo `/Users/sac/avec` at HEAD `ead4032`.

## Method

Canonical invocations only (Makefile / `scripts/verify_v26.10.31.sh`). Census discipline:
sum **all** `test result:` lines per crate (lib + integration + doc), never the last line —
a suite file can emit more than one result line. avec-parquet ran twice (default and
`--features std`); avec-beam ran `cargo build -p avec-beam` first (stale-dylib lesson);
wasm-footprint built as its own workspace root via `--manifest-path`
(crates/wasm-footprint is deliberately not a root-workspace member).

Full logs: `/tmp/avec-census-l10/logs/*.log` (scratch, outside the repo). Parsed totals were
cross-checked by an independent raw `grep -c "^test result:"` + field-sum pass; both methods
agree on every crate.

## Crate totals

| Crate / config | Invocation | Exit | Result lines | Passed | Failed | Ignored |
|---|---|---|---|---|---|---|
| avec-audit | `cargo test -p avec-audit` | 0 | 2 | 5 | 0 | 0 |
| avec-arrow | `cargo test -p avec-arrow` | 0 | 6 | 137 | 0 | 0 |
| avec-beam | build, then `cargo test -p avec-beam` | 0, 0 | 2 | 10 | 0 | 0 |
| avec-blake3 | `cargo test -p avec-blake3` | 0 | 7 | 25 | 0 | 1 |
| avec-ffi | `cargo test -p avec-ffi` | 0 | 4 | 13 | 0 | 0 |
| avec-git | `cargo test -p avec-git` | 0 | 5 | 48 | 0 | 1 |
| avec-gpu | `cargo test -p avec-gpu` | 0 | 2 | 1 | 0 | 0 |
| avec-integration | `cargo test -p avec-integration` | 0 | 5 | 12 | 0 | 0 |
| avec-parquet (default) | `cargo test -p avec-parquet` | 0 | 10 | 192 | 0 | 0 |
| avec-parquet (std) | `cargo test -p avec-parquet --features std` | 0 | 10 | 204 | 0 | 0 |
| bench (lib) | `cargo test -p bench --lib` | 101 | 0 | 0 | 0 | 0 |
| avec-cli | `cargo test -p avec-cli` | 0 | 3 | 6 | 0 | 0 |
| wasm-footprint | `cargo test --manifest-path crates/wasm-footprint/…` | 0 | 2 | 8 | 0 | 0 |
| **Grand total** | | | **58** | **661** | **0** | **2** |

## Per-suite rows (all 58 `test result:` lines)

The table below has 59 rows: the extra row is `bench (lib) | no lib target`,
which is not a result line — cargo refused pre-build (exit 101) and emitted
none.

Suite = one `Running ...` / `Doc-tests ...` header. The avec-arrow `tests/mutation_kills.rs`
suite emits **two** result lines: a nested filtered libtest child (1 passed, 35 filtered
out) spawned mid-suite, then the outer suite line — the doc-test parse trap, found in the
wild. Both lines are counted; attribution to `tests/mutation_kills.rs` is 37 total.

| Crate | Suite | Passed | Failed | Ignored | Notes |
|---|---|---|---|---|---|
| avec-audit | unittests src/lib.rs | 5 | 0 | 0 | |
| avec-audit | doc-tests avec_audit | 0 | 0 | 0 | |
| avec-arrow | unittests src/lib.rs | 93 | 0 | 0 | |
| avec-arrow | tests/mutation_kills.rs | 1 | 0 | 0 | nested filtered child, 35 filtered |
| avec-arrow | tests/mutation_kills.rs | 36 | 0 | 0 | outer suite line |
| avec-arrow | tests/property.rs | 1 | 0 | 0 | |
| avec-arrow | tests/validation.rs | 6 | 0 | 0 | |
| avec-arrow | doc-tests avec_arrow | 0 | 0 | 0 | |
| avec-beam | unittests src/lib.rs | 9 | 0 | 0 | prebuild exit 0 |
| avec-beam | tests/bridge_contract.rs | 1 | 0 | 0 | |
| avec-blake3 | unittests src/lib.rs | 3 | 0 | 0 | |
| avec-blake3 | tests/bench_tmp.rs | 1 | 0 | 0 | |
| avec-blake3 | tests/multi_chunk_vectors.rs | 7 | 0 | 1 | ignored by design |
| avec-blake3 | tests/official_vectors.rs | 6 | 0 | 0 | |
| avec-blake3 | tests/small_batch_vectors.rs | 3 | 0 | 0 | |
| avec-blake3 | tests/zero_alloc.rs | 5 | 0 | 0 | |
| avec-blake3 | doc-tests avec_blake3 | 0 | 0 | 0 | |
| avec-ffi | unittests src/lib.rs | 4 | 0 | 0 | |
| avec-ffi | tests/alloc_courts.rs | 4 | 0 | 0 | |
| avec-ffi | tests/zero_copy.rs | 5 | 0 | 0 | |
| avec-ffi | doc-tests avec_ffi | 0 | 0 | 0 | |
| avec-git | unittests src/lib.rs | 43 | 0 | 0 | |
| avec-git | tests/fsck_court.rs | 2 | 0 | 0 | |
| avec-git | tests/git_interop.rs | 3 | 0 | 0 | |
| avec-git | tests/perf_baseline.rs | 0 | 0 | 1 | all tests ignored |
| avec-git | doc-tests avec_git | 0 | 0 | 0 | |
| avec-gpu | unittests src/lib.rs | 1 | 0 | 0 | |
| avec-gpu | doc-tests avec_gpu | 0 | 0 | 0 | |
| avec-integration | unittests src/lib.rs | 0 | 0 | 0 | |
| avec-integration | tests/admission_e2e.rs | 2 | 0 | 0 | |
| avec-integration | tests/cross_crate.rs | 6 | 0 | 0 | |
| avec-integration | tests/membranes.rs | 4 | 0 | 0 | |
| avec-integration | doc-tests avec_integration | 0 | 0 | 0 | |
| avec-parquet (default) | unittests src/lib.rs | 157 | 0 | 0 | |
| avec-parquet (default) | tests/accessor_perf_court.rs | 0 | 0 | 0 | 0 tests under default cfg |
| avec-parquet (default) | tests/aligned_interop.rs | 4 | 0 | 0 | |
| avec-parquet (default) | tests/level_free_multi_court.rs | 0 | 0 | 0 | 0 tests under default cfg |
| avec-parquet (default) | tests/mutation_kills.rs | 17 | 0 | 0 | |
| avec-parquet (default) | tests/page_zero_alloc.rs | 2 | 0 | 0 | |
| avec-parquet (default) | tests/rle_court.rs | 8 | 0 | 0 | |
| avec-parquet (default) | tests/robustness.rs | 3 | 0 | 0 | |
| avec-parquet (default) | tests/std_feature_court.rs | 1 | 0 | 0 | |
| avec-parquet (default) | doc-tests avec_parquet | 0 | 0 | 0 | |
| avec-parquet (std) | unittests src/lib.rs | 163 | 0 | 0 | |
| avec-parquet (std) | tests/accessor_perf_court.rs | 1 | 0 | 0 | |
| avec-parquet (std) | tests/aligned_interop.rs | 4 | 0 | 0 | |
| avec-parquet (std) | tests/level_free_multi_court.rs | 4 | 0 | 0 | |
| avec-parquet (std) | tests/mutation_kills.rs | 17 | 0 | 0 | |
| avec-parquet (std) | tests/page_zero_alloc.rs | 2 | 0 | 0 | |
| avec-parquet (std) | tests/rle_court.rs | 8 | 0 | 0 | |
| avec-parquet (std) | tests/robustness.rs | 3 | 0 | 0 | |
| avec-parquet (std) | tests/std_feature_court.rs | 2 | 0 | 0 | |
| avec-parquet (std) | doc-tests avec_parquet | 0 | 0 | 0 | |
| bench (lib) | no lib target | 0 | 0 | 0 | cargo exit 101, pre-build refusal |
| avec-cli | unittests src/main.rs | 0 | 0 | 0 | |
| avec-cli | unittests src/bin/verify_v26.rs | 0 | 0 | 0 | |
| avec-cli | tests/mutation_kills.rs | 6 | 0 | 0 | |
| wasm-footprint | unittests src/lib.rs | 3 | 0 | 0 | |
| wasm-footprint | tests/no_std_core.rs | 5 | 0 | 0 | |

## Courts

| Court | Invocation | Exit | Verdict |
|---|---|---|---|
| ontology | `validate_ontology.sh` | 0 | VERDICT: PASS — conforms to own shapes |
| branchless | `audit_branchless.sh --selftest` | 0 | positive control FIRED; SELFTEST PASS |
| verify_v26.10.31.sh | SKIPPED | — | fresh: mtime Oct 30 21:06:51 2026, `ead4032` |

## BUILD-ABORT / anomalies

- **Zero compile errors across all logs** (`grep -c 'error\[E\|error: could not compile'` = 0
  everywhere). No BUILD-ABORT to record.
- `bench --lib` exit 101 is not a build abort: cargo refused pre-build ("no library targets
  found in package `bench`"), zero suites compiled or run. bench has `src/main.rs` +
  `src/bin/` + `src/benches/` but no `src/lib.rs` — per task scope ("lib tests only if any"):
  there are none.
- avec-beam prebuild `cargo build -p avec-beam` exit 0 ("Finished dev profile in 0.53s").

## Grand totals

**58 `test result:` lines · 661 passed · 0 failed · 2 ignored · 0 BUILD-ABORT.**
13 invocations (12 with tests + bench --lib with none), 2026-10-30 21:15:29–21:19:21 PDT.
Raw-grep cross-check agrees with the per-suite parse on every crate.
