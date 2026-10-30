# AVEC-104: The Transistor: Fused Frame-of-Reference (FFOR) Kernel & Branch Audit

**Component**: `crates/avec-parquet`, `crates/audit`  
**Priority**: High  
**Reporter**: Antigravity Scan Pass  
**Status**: IN PROGRESS  

## Description
Current bit-unpacking and baseline addition require a two-pass decode: materialize intermediate integers and add base. Fused Frame-of-Reference (`ffor.rs`) fuses `wrapping_add(base)` into the tight 1/2/4/8-bit bit-unpack loops, writing base-relative values in a single cache-friendly pass.

## Measured Constraints & Invariants
- **Tame the Headline**: Bus bandwidth is bounded at 0.9–2.3 packed GiB/s (speedup 3.0–40.6× over materialization). The value proposition is eliminating the 0.99–5.5 GiB/s decode bottleneck and avoiding cache thrashing.
- **Extent-Scoped Branch Audit Law**:
  - Inner loop bodies must compile to **zero conditional branches** (`BODY 0/0`).
  - Loop latches and bounds clamps are itemized and budgeted in the branch audit manifest.

## Acceptance Criteria
- [ ] Export `ffor` in `crates/avec-parquet/src/lib.rs`.
- [ ] Implement `ffor_unpack` and `ffor_pack` across SWAR bit-widths {1, 2, 4, 8}.
- [ ] Add `ffor_bench` or audit wrappers in `crates/audit` and verify `BODY 0/0` under `make branch-audit`.
- [ ] Fail-closed tests for out-of-range deltas and corrupt packed words.
