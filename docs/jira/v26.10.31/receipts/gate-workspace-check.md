# Receipt: gate-workspace-check

| Field | Value |
|---|---|
| Receipt ID | gate-workspace-check |
| Date | 2026-10-31T01:55:28Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo check --workspace` |
| Exit | 101 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (48 entries) |
| Evidence digest (blake3 of stdout tail) | `ec47626c8c7eb3f96ea9b7dd38dbef90ad90c21c916c5541b11bf368ccfc2c5d` |
| Standing | FAILED (exit 101) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

error[E0308]: mismatched types
     --> crates/avec-blake3/src/multi_chunk.rs:144:35
      |
  144 |             cv_vec[i] = vld1q_u32(col.as_ptr());
      |                         --------- ^^^^^^^^^^^^ expected `*const u32`, found `*const [u32; 8]`
      |                         |
      |                         arguments to this function are incorrect
      |
      = note: expected raw pointer `*const u32`
                 found raw pointer `*const [u32; 8]`
note: function defined here
     --> /Users/sac/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/src/rust/library/core/src/../../stdarch/crates/core_arch/src/aarch64/neon/generated.rs:10141:15
      |
10141 | pub unsafe fn vld1q_u32(ptr: *const u32) -> uint32x4_t {
      |               ^^^^^^^^^

error[E0308]: mismatched types
   --> crates/avec-blake3/src/multi_chunk.rs:183:28
    |
183 |                 cv[l][i] = lane[l];
    |                 --------   ^^^^^^^ expected `[u32; 8]`, found `u32`
    |                 |
    |                 expected due to the type of this binding

warning: unnecessary `unsafe` block
  --> crates/avec-blake3/src/multi_chunk.rs:86:9
   |
86 |         unsafe {
   |         ^^^^^^ unnecessary `unsafe` block
   |
   = note: `#[warn(unused_unsafe)]` (part of `#[warn(unused)]`) on by default

For more information about this error, try `rustc --explain E0308`.
warning: `avec-blake3` (lib) generated 1 warning
error: could not compile `avec-blake3` (lib) due to 2 previous errors; 1 warning emitted
warning: build failed, waiting for other jobs to finish...
warning: `avec-beam` (lib) generated 1 warning (run `cargo fix --lib -p avec-beam` to apply 1 suggestion)
```

<!-- Tree state: dirty (48 entries) -->
