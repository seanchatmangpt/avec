# AVEC-102: Eliminate unused imports and compiler warnings across workspace

**Component**: Workspace (`avec-parquet`, `avec-ffi`, `avec-arrow`, `avec-beam`)  
**Priority**: Normal  
**Reporter**: Antigravity Scan Pass  
**Status**: OPEN  

## Description
A scan of `cargo check --all-targets --workspace` yields four distinct compiler warnings:
1. `crates/avec-parquet/tests/aligned_interop.rs:27:25`: unused import `write_bool_field`.
2. `crates/avec-ffi/src/lib.rs:9:47`: unused import `Scalar`.
3. `crates/avec-arrow/src/foreign.rs:18:17`: unused import `c_char`.
4. `crates/avec-arrow/src/batch.rs:14:5`: unused import `alloc::vec`.

In addition, `crates/avec-beam/src/resources.rs:128` triggers unused field warnings for `ptr` and `bit_width`.

## Acceptance Criteria
- [ ] Remove all unused imports across tests and lib roots.
- [ ] Incorporate or prefix dead-code fields in `resources.rs`.
- [ ] `cargo check --all-targets --workspace` exits 0 with 0 warnings.
