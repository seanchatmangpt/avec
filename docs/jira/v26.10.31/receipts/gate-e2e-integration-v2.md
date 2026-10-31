# Receipt: gate-e2e-integration-v2

| Field | Value |
|---|---|
| Receipt ID | gate-e2e-integration-v2 |
| Date | 2026-10-31T02:21:32Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-integration --release --test admission_e2e` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (59 entries) |
| Evidence digest (blake3 of stdout tail) | `152cb5beac55070f586d3b81586ad2fc2439f22ceb0f6c5f2592fe9daef6d8d8` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
   Compiling avec-arrow v26.10.29 (/Users/sac/avec/crates/avec-arrow)
   Compiling avec-parquet v26.10.29 (/Users/sac/avec/crates/avec-parquet)
warning: unnecessary `unsafe` block
   --> crates/avec-blake3/src/multi_chunk.rs:130:5
    |
100 |     unsafe {
    |     ------ because it's nested under this `unsafe` block
...
130 |     unsafe {
    |     ^^^^^^ unnecessary `unsafe` block
    |
    = note: `#[warn(unused_unsafe)]` (part of `#[warn(unused)]`) on by default

warning: unnecessary `unsafe` block
  --> crates/avec-blake3/src/multi_chunk.rs:90:9
   |
90 |         unsafe {
   |         ^^^^^^ unnecessary `unsafe` block

warning: `avec-blake3` (lib) generated 2 warnings
   Compiling avec-integration v26.10.29 (/Users/sac/avec/crates/integration)
    Finished `release` profile [optimized] target(s) in 3.68s
     Running tests/admission_e2e.rs (target/release/deps/admission_e2e-b3ff473fa36fbba7)

running 2 tests
test admission_e2e_roundtrip_and_receipt ... ok
test admission_e2e_timing_probe_min_of_100 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

<!-- Tree state: dirty (59 entries) -->
