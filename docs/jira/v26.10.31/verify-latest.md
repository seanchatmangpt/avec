# Verify Latest: v26.10.31 (lane I20)

Generated: 2026-10-31T15:03:07Z · HEAD: `85cdb75f82bf1a4e830a69c2b2e4bf9d3a93b217` · host: arm64

## Gate Summary

| Gate | Status | Exit |
|------|--------|------|
| `check-workspace` | PASS | 0 |
| `test-avec-arrow` | PASS | 0 |
| `test-avec-parquet` | PASS | 0 |
| `test-avec-git` | PASS | 0 |
| `test-avec-blake3` | PASS | 0 |
| `test-integration` | PASS | 0 |
| `test-avec-beam` | PASS | 0 |
| `audit-branchless` | PASS | 0 |
| `cli-verify` | PASS | 0 |
| `test-audit` | PASS | 0 |

## Evidence Tails (last 12 lines per gate)

### check-workspace

```
    Checking avec-git v26.10.29 (/Users/sac/avec/crates/avec-git)
   Compiling find-msvc-tools v0.1.15
   Compiling shlex v2.0.1
   Compiling avec-beam v26.10.29 (/Users/sac/avec/crates/avec-beam)
    Checking avec-ffi v26.10.29 (/Users/sac/avec/crates/avec-ffi)
    Checking avec-audit v26.10.29 (/Users/sac/avec/crates/audit)
    Checking avec-integration v26.10.29 (/Users/sac/avec/crates/integration)
   Compiling cc v1.10.0
    Checking avec-cli v26.10.29 (/Users/sac/avec/crates/cli)
   Compiling avec-gpu v26.10.29 (/Users/sac/avec/crates/avec-gpu)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.75s
```

### test-avec-arrow

```
test null_count_matches_pushed_none_count ... ok
test null_slots_carry_zero_bytes ... ok
test viewvec_descriptor_law_inline_and_foreign ... ok
test varlen_offset_ladder_u32_and_u64 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests avec_arrow

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### test-avec-parquet

```

running 2 tests
test lib_links_and_core_api_runs_under_both_configs ... ok
test std_only::std_scan_surface_runs_end_to_end_under_std_feature ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests avec_parquet

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### test-avec-git

```
     Running tests/perf_baseline.rs (target/debug/deps/perf_baseline-0a8ceea606a2dc97)

running 1 test
test perf_baseline_git_plane ... ignored

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests avec_git

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### test-avec-blake3

```
test compress_is_zero_alloc ... ok
test hash_scalar_64mib_is_zero_alloc ... ok
test hash_wide_simd_64mib_is_zero_alloc ... ok
test positive_control_counter_is_live ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.25s

   Doc-tests avec_blake3

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### test-integration

```
test varlen_u64_offsets_family_roundtrip ... ok
test nested_export_struct_abi_walk_two_children ... ok
test c_abi_roundtrip_i64_u64_f32_f64_bitwise_and_double_release ... ok
test neon_vs_portable_mask_bitwise_equality ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

   Doc-tests avec_integration

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### test-avec-beam

```

running 1 test
   Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
   Compiling avec-git v26.10.29 (/Users/sac/avec/crates/avec-git)
   Compiling avec-parquet v26.10.29 (/Users/sac/avec/crates/avec-parquet)
   Compiling avec-arrow v26.10.29 (/Users/sac/avec/crates/avec-arrow)
   Compiling avec-beam v26.10.29 (/Users/sac/avec/crates/avec-beam)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.09s
test beam_bridge_contract_end_to_end ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.85s

```

### audit-branchless

```
== disassembler: objdump (Apple LLVM version 17.0.0)
== SELFTEST
== positive control: FIRED (4 banned ops in branchy.o) — gate is not vacuous
== scope demo: clean branchless fixture -> 0 hits
== SELFTEST PASS
```

### cli-verify

```
[verify_v26] FR-04 court using git version 2.51.2
[verify_v26] battery complete: 7 rows, 0 FAIL, skips=[FR-07]
{"tool":"verify_v26","rows":[{"id":"FR-01","gate":"size/align (Quad64 64B/64B; Buf 64B-aligned heap)","status":"PASS","evidence":"size_of(Quad64)=64 align_of(Quad64)=64 Buf 143/143 allocs 64B-aligned (sizes 1..=1000 step 7)"},{"id":"FR-02","gate":"layout law (derived from FR-01 + Quad64 wire round-trip ladder spot check)","status":"PASS","evidence":"Quad64 new->to_le_bytes(64B)->from_le_bytes round-trip equal; offsets 0x00/0x04/0x08/0x0C/0x10/0x18/0x1C/0x20 held by build-time asserts in layout.rs"},{"id":"FR-03","gate":"parquet RLE round-trip (rle_encode -> rle_decode)","status":"PASS","evidence":"values=10000 bits=8 encoded=10003B enc_fast=10003B decode_ok=true values_equal=true"},{"id":"FR-04","gate":"git pack write + in-crate parser read + real git fsck/verify-pack/cat-file court","status":"PASS","evidence":"blobs=8 pack_bytes=147 idx_bytes=1296 in_crate_parse=true git=git version 2.51.2 fsck_exit_ok=true verify-pack_exit_ok=true cat-file_exit_ok=true n_objects=8  writer_us=2199"},{"id":"FR-06","gate":"blake3 known-answer spot check (empty + abc + 2KiB)","status":"PASS","evidence":"hash(\"\")=af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262 match=true hash(\"abc\")=6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85 match=true hash(<2048B>)=634f590a498b3e29165cd8bd32f30a99a2b0d8949a3d7ce35779b6253d8ca0d5 expected=634f590a498b3e29165cd8bd32f30a99a2b0d8949a3d7ce35779b6253d8ca0d5 match=true oracle=[b3sum(/opt/homebrew/bin/b3sum)=634f590a498b3e29165cd8bd32f30a99a2b0d8949a3d7ce35779b6253d8ca0d5 match=true]"},{"id":"FR-05","gate":"replay/determinism spot check (Quad64 wire re-encode stability)","status":"PASS","evidence":"to_le_bytes->from_le_bytes->to_le_bytes byte-identical (deterministic re-encode)"},{"id":"FR-07","gate":"BEAM interop","status":"SKIP","evidence":"requires a live BEAM (avec-beam NIF harness); owned by the integration lane, not runnable in this in-process battery"}],"summary":{"total":7,"fail":0,"skips":["FR-07"],"timing_us":{"FR-01":477,"FR-02":16,"FR-03":2133,"FR-04":128126,"FR-06":3273,"FR-05":4}},"exit_ok":true}
```

### test-audit

```
test alloc_counter::tests::nested_inner_scope_excludes_outer_allocations ... ok
test alloc_counter::tests::tracking_is_reset_after_scope ... ok
test alloc_counter::tests::vec_push_reports_nonzero_count_and_bytes ... ok
test alloc_counter::tests::multithreaded_totals_add_up ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests avec_audit

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Baseline

Workspace-check baseline: `scripts/verify_baseline.txt` (0 lines).
PRE-EXISTING-FAIL = failure output matches baseline; FAIL = output differs (introduced).
Regenerate: `scripts/verify_v26.10.31.sh --regen-baseline` (see verify-README.md).
