# Receipt: gate-blake3-tests-v3

| Field | Value |
|---|---|
| Receipt ID | gate-blake3-tests-v3 |
| Date | 2026-10-31T04:13:05Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-blake3` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `6e214e089953e4921e1f9c7dbfb9a0e4b754ebcb4e4daa965ae95aa5ab7fae7b` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
test result: ok. 7 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running tests/official_vectors.rs (target/debug/deps/official_vectors-f91de024106d08ea)

running 6 tests
test chunk_boundary_1024_single_chunk_root ... ok
test chunk_boundary_2048_two_chunk_parent_root ... ok
test chunk_boundary_3072_three_chunk_unbalanced_tree ... ok
test official_vectors_hash_known_answer ... ok
test differential_hash_wide_simd_matches_hash_on_full_corpus ... ok
test simd_subtree_boundary_65536_and_66560 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/small_batch_vectors.rs (target/debug/deps/small_batch_vectors-9184f8b8cf02141d)

running 3 tests
test court_eligibility_predicate ... ok
test court_hash_vs_b3sum_small_batch_window ... ok
test court_exhaustive_small_parity_sequential_vs_4way ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s

     Running tests/zero_alloc.rs (target/debug/deps/zero_alloc-27b9599ccf6f256f)

running 5 tests
test positive_control_counter_is_live ... ok
test chunked_1kib_pass_is_zero_alloc ... ok
test hash_scalar_64mib_is_zero_alloc ... ok
test compress_is_zero_alloc ... ok
test hash_wide_simd_64mib_is_zero_alloc ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.37s

   Doc-tests avec_blake3

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

<!-- Tree state: dirty (70 entries) -->
