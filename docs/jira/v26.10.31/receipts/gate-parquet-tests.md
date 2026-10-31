# Receipt: gate-parquet-tests

| Field | Value |
|---|---|
| Receipt ID | gate-parquet-tests |
| Date | 2026-10-31T04:14:10Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-parquet --features std` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `fb2457db223d86adb06fc89ee608411e55a01a7eb4b37418b7172d05300d3722` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/rle_court.rs (target/debug/deps/rle_court-afc0ea55496262ab)

running 8 tests
test decoder_still_refuses_out_of_kernel_headers ... ok
test decode_into_contract_unaffected ... ok
test bits12_witness_class_regression ... ok
test parquet_error_reexport_is_same_type ... ok
test exhaustive_lane_values_round_trip ... ok
test fast_encoder_parity_with_baseline ... ok
test out_of_domain_widths_refused_loudly_not_overflow_panics ... ok
test every_kernel_width_round_trips_random_and_adversarial ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/robustness.rs (target/debug/deps/robustness-644e7f16d17aab3d)

running 3 tests
test partial_write_contract_observed ... ok
test fixtures_decode_to_hand_written_values ... ok
test fail_closed_adversarial_corpus ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/std_feature_court.rs (target/debug/deps/std_feature_court-4791906ec40d5990)

running 2 tests
test lib_links_and_core_api_runs_under_both_configs ... ok
test std_only::std_scan_surface_runs_end_to_end_under_std_feature ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests avec_parquet

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

<!-- Tree state: dirty (70 entries) -->
