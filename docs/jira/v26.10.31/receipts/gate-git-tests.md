# Receipt: gate-git-tests

| Field | Value |
|---|---|
| Receipt ID | gate-git-tests |
| Date | 2026-10-31T01:55:50Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-git --lib` |
| Exit | 101 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (48 entries) |
| Evidence digest (blake3 of stdout tail) | `b99b5e6422ab239d165d1696d2b71489e85245b16f9fe013ea66266443a67f75` |
| Standing | FAILED (exit 101) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
    |
594 |             String::from_utf8_lossy(&out.stderr)
    |             ^^^^^^ use of undeclared type `String`
    |
help: consider importing one of these structs
    |
571 +     use std::string::String;
    |
571 +     use alloc::string::String;
    |

warning: unused imports: `String` and `ToString`
   --> crates/avec-git/src/pack_write.rs:514:25
    |
514 |     use alloc::string::{String, ToString};
    |                         ^^^^^^  ^^^^^^^^
    |
    = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: variable does not need to be mutable
   --> crates/avec-git/src/deflate.rs:602:17
    |
602 |             let mut v = vec![
    |                 ----^
    |                 |
    |                 help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `i`
   --> crates/avec-git/src/pack_write.rs:598:47
    |
598 |         let blobs: Vec<Vec<u8>> = (0..5).map(|i| blob_bytes(100, 7)).collect();
    |                                               ^ help: if this is intentional, prefix it with an underscore: `_i`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

For more information about this error, try `rustc --explain E0433`.
warning: `avec-git` (lib test) generated 3 warnings
error: could not compile `avec-git` (lib test) due to 1 previous error; 3 warnings emitted
```

<!-- Tree state: dirty (48 entries) -->
