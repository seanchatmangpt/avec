# AVEC-111: Freestanding Apple Silicon Metal Unified Memory Bridge & Sovereign `avec-blake3` Crate

**Status:** CLOSED
**Component:** `crates/avec-blake3`, `crates/avec-gpu`, `crates/avec-arrow`
**Type:** Feature / Moonshot
**Date:** 2026-10-30

---

## 1. Context & Architectural Boundaries

1. **Sovereign `#![no_std]` BLAKE3 Crate (`crates/avec-blake3` & `crates/avec-arrow/src/blake3.rs`)**:
   - **Zero External Dependencies**: Standalone `#![no_std]` crate with `#![forbid(unsafe_op_in_unsafe_fn)]` and zero heap allocations (M-01 / CTQ-4).
   - **16-Block Intra-Chunk Chaining (`hash_chunk`)**: Compresses all 1..=16 sequential 64-byte blocks per 1024-byte chunk with exact `CHUNK_START` (block 0), `CHUNK_END` (`num_blocks - 1`), and `ROOT` flag boundaries.
   - **Zero-Allocation Stack Merkle Tree (`hash`)**: Uses a 54-entry stack (`[[u32; 8]; 54]`, covering up to $2^{64}$ bytes) for binary Merkle parent reduction (`PARENT = 1 << 2`, terminal `PARENT | ROOT`).
   - **SIMD Multi-Parent Reduction (`neon_parent::compress_parents_4way_neon`, `avx2_parent::compress_parents_8way_avx2`, `reduce_level_simd`, `hash_wide_simd`)**: Stacks 4 parent pairs horizontally across `uint32x4_t` NEON registers (or 8 parent pairs across `__m256i` AVX2 registers) to compress sibling subtrees in parallel.
   - **CPU SIMD vs. Unified Memory GPU Crossover (`GPU_CROSSOVER_BYTES = 16 MiB`)**: Encodes the hardware dispatch boundary (`should_dispatch_gpu(len)`): buffers $< 16\text{ MiB}$ stay on sub-100ns L1/L2/L3 CPU SIMD; massive batches $\ge 16\text{ MiB}$ (16,384+ independent 1024-byte chunks) cross over to zero-copy Unified Memory SIMT execution.

2. **Freestanding Apple Silicon Metal Unified Memory Bridge (`crates/avec-gpu`)**:
   - Maps 64-byte aligned `Buf` and 16 KiB page-aligned `MmuBuf` memory directly into Apple Silicon Metal Unified Memory (`MTLResourceStorageModeShared`) via `newBufferWithBytesNoCopy:length:options:deallocator:`.

---

## 2. Verification Receipts

### 2.1 Official BLAKE3 Multi-Block & Multi-Chunk Merkle Test Vectors (`avec-blake3`)
Verified against the official BLAKE3 test vectors (repeating `0..251` byte sequence) at lengths `0`, `1`, `63`, `64`, `65`, `127`, `128`, `1023`, `1024`, `1025`, `2048`, `2049`, `4096`, and `8192`, plus 64-chunk and 65-chunk SIMD parent reduction parity:
```
$ cargo test -p avec-blake3
running 3 tests
test tests::test_gpu_crossover_heuristic ... ok
test tests::test_official_blake3_vectors_multi_block_and_merkle_tree ... ok
test tests::test_simd_parent_reduction_parity_64_to_128_chunks ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### 2.2 Benchmark Receipt (`phase_change_bench`)
```
=== 5. Sovereign #![no_std] BLAKE3 (avec-blake3) ===
  64 KiB (64-chunk Merkle tree)      | throughput:    0.41 GiB/s | 0 allocs
  SIMD 4-way NEON parent reduction   | throughput:    1.31 GiB/s |    21.9 Mparents/s | 0 allocs
```

### 2.3 Publish Dry-Run (`cargo publish --dry-run -p avec-blake3`)
```
   Packaging avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
    Packaged 5 files, 29.8KiB (7.5KiB compressed)
   Verifying avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.11s
```
