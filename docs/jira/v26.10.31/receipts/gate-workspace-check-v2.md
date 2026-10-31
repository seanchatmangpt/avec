# Receipt: gate-workspace-check-v2

| Field | Value |
|---|---|
| Receipt ID | gate-workspace-check-v2 |
| Date | 2026-10-31T02:17:52Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo check --workspace` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (57 entries) |
| Evidence digest (blake3 of stdout tail) | `8940eb83a7a5f690355c09a839db250c4a4d96f43f5010099a9d726c81485176` |
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
    Checking avec-gpu v26.10.29 (/Users/sac/avec/crates/avec-gpu)
    Checking avec-beam v26.10.29 (/Users/sac/avec/crates/avec-beam)
    Checking avec-cli v26.10.29 (/Users/sac/avec/crates/cli)
    Checking bench v26.10.29 (/Users/sac/avec/crates/bench)
    Checking avec-audit v26.10.29 (/Users/sac/avec/crates/audit)
    Checking avec-integration v26.10.29 (/Users/sac/avec/crates/integration)
    Checking avec-ffi v26.10.29 (/Users/sac/avec/crates/avec-ffi)
warning: function `avec_alloc_stats` is never used
  --> crates/avec-beam/src/alloc_spy.rs:56:26
   |
56 | pub(crate) extern "C" fn avec_alloc_stats(
   |                          ^^^^^^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `avec-beam` (lib) generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s
```

<!-- Tree state: dirty (57 entries) -->
