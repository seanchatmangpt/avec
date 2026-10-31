# Receipt: gate-blake3-zero-alloc

| Field | Value |
|---|---|
| Receipt ID | gate-blake3-zero-alloc |
| Date | 2026-10-31T01:57:04Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-blake3 --test zero_alloc -- --test-threads=1` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (49 entries) |
| Evidence digest (blake3 of stdout tail) | `34d7f9f0781716b032694b929d2accae28fb94711c263bb02a41b348419889a6` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
warning: profiles for the non root package will be ignored, specify profiles at the workspace root:
package:   /Users/sac/avec/crates/cli/Cargo.toml
workspace: /Users/sac/avec/Cargo.toml
   Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
warning: unnecessary `unsafe` block
  --> crates/avec-blake3/src/multi_chunk.rs:86:9
   |
86 |         unsafe {
   |         ^^^^^^ unnecessary `unsafe` block
   |
   = note: `#[warn(unused_unsafe)]` (part of `#[warn(unused)]`) on by default

warning: `avec-blake3` (lib) generated 1 warning
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
     Running tests/zero_alloc.rs (target/debug/deps/zero_alloc-27b9599ccf6f256f)

running 4 tests
test chunked_1kib_pass_is_zero_alloc ... ok
test compress_is_zero_alloc ... ok
test hash_scalar_64mib_is_zero_alloc ... ok
test hash_wide_simd_64mib_is_zero_alloc ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.78s
```

<!-- Tree state: dirty (49 entries) -->
