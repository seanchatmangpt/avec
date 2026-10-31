# Receipt: gate-git-tests-v3

| Field | Value |
|---|---|
| Receipt ID | gate-git-tests-v3 |
| Date | 2026-10-31T05:33:29Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-git` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (117 entries) |
| Evidence digest (blake3 of stdout tail) | `cbf669417b544d982bd37fd29275e889d5f2bcce2a5979a541fca42b23a8127c` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
test inflate::tests::git_fixture_inflate_round_trip ... ok
test idx_read::tests::oversized_partition_returns_typed_error ... ok
test idx_read::tests::boundary_n65536_uniform_partitions_lookup_all ... ok
test deflate::tests::adler32_matches_python ... ok
test deflate::tests::we_inflate_python_zlib_compress ... ok
test deflate::tests::python_inflates_our_deflate ... ok
test inflate::tests::round_trip_python_zlib_multiple_levels ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s

     Running tests/fsck_court.rs (target/debug/deps/fsck_court-4734e87f3520379c)

running 2 tests
test fsck_court_accepts_avec_git_pack ... ok
test negative_control_corrupted_git_built_pack_is_rejected ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

     Running tests/git_interop.rs (target/debug/deps/git_interop-22b95c13dc04ba45)

running 3 tests
test direction_b_git_accepts_avec_pack ... ok
test direction_a_fanout_total_matches_git_census ... ok
test direction_a_git_idx_parsed_by_crate_reader ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s

     Running tests/perf_baseline.rs (target/debug/deps/perf_baseline-0a8ceea606a2dc97)

running 1 test
test perf_baseline_git_plane ... ignored

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests avec_git

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

<!-- Tree state: dirty (117 entries) -->
