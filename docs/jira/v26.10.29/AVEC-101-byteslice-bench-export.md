# AVEC-101: Unresolved import `avec_parquet::byteslice` in bench target

**Component**: `crates/avec-parquet`, `crates/bench`  
**Priority**: Blocker  
**Reporter**: Antigravity Scan Pass  
**Status**: OPEN  

## Description
Running `cargo check --all-targets --workspace` or `make lint` fails with error code 101:

```
error[E0432]: unresolved import `avec_parquet::byteslice`
  --> crates/bench/src/bin/byteslice_bench.rs:16:19
   |
16 | use avec_parquet::byteslice::{pack_byteslice, point_get, scan_eq_byteslice};
   |                   ^^^^^^^^^ could not find `byteslice` in `avec_parquet`
```

The file `crates/avec-parquet/src/byteslice.rs` exists on disk but is not declared in `crates/avec-parquet/src/lib.rs`.

## Acceptance Criteria
- [ ] Declare `pub mod byteslice;` in `crates/avec-parquet/src/lib.rs`.
- [ ] Verify `cargo check --bin byteslice_bench -p bench` compiles with 0 errors.
- [ ] Verify `make lint` proceeds past this failure point.
