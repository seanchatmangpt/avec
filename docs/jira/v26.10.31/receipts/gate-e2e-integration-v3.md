# Receipt: gate-e2e-integration-v3

| Field | Value |
|---|---|
| Receipt ID | gate-e2e-integration-v3 |
| Date | 2026-10-31T04:14:42Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-integration --release --test admission_e2e` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `dc3f56d8109c17058f1d6d66fcf226f44d32912ab53bfccea33f5b807ad9b7e6` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
   Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
   Compiling avec-integration v26.10.29 (/Users/sac/avec/crates/integration)
    Finished `release` profile [optimized] target(s) in 3.76s
     Running tests/admission_e2e.rs (target/release/deps/admission_e2e-b3ff473fa36fbba7)

running 2 tests
test admission_e2e_roundtrip_and_receipt ... ok
test admission_e2e_timing_probe_min_of_100 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

<!-- Tree state: dirty (70 entries) -->
