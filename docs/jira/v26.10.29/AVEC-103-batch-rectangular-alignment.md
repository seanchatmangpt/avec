# AVEC-103: Steam Power: Named Multi-Column RecordBatch with 64-Byte Alignment Invariant

**Component**: `crates/avec-arrow`  
**Priority**: High  
**Reporter**: Antigravity Scan Pass  
**Status**: IN PROGRESS  

## Description
Upstream consumers (PurRDF columnar OCEL projection, EAV shape with 7 tables) require a named, row-count-synchronized batch container (`Batch` and `BatchBuilder`).
To guarantee deterministic execution and hardware vector efficiency, every buffer within the batch must satisfy the 64-byte alignment invariant.

## Invariants
1. **Physical Alignment**: Every column buffer pointer must satisfy `(ptr as usize) & 63 == 0`.
2. **Rectangular Shape**: All columns must have identical lengths. Mismatches return typed `BatchError::LengthMismatch`.
3. **Empty Sentinel**: An empty vector has buffer address 64 ($O(1)$ sentinel, zero allocation).

## Acceptance Criteria
- [ ] `Batch::try_new` validates row counts across all columns.
- [ ] Verify buffer alignment via non-allocating helper `(ptr as usize) & 63 == 0`.
- [ ] Implement manual `core::fmt::Debug` for all contained types without procedural derive bloat.
- [ ] Unit tests covering empty batch, multi-column rectangular batch, and mismatch refusal.
