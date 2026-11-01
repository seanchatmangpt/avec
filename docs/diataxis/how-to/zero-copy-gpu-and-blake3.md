# How-To Guide: Zero-Copy Metal Unified Memory, `cgap-blake3`, and Gate Verification

Task-oriented recipes for integrating `cgap-blake3`, `cgap-gpu`, and running the fail-closed verification gates.

---

## 1. How to Wrap an Arrow Buffer into Apple Silicon Metal Unified Memory (`cgap-gpu`)

On Apple Silicon, CPU and GPU share LPDDR5X DRAM. Use `cgap_gpu::MetalSharedBuffer` to wrap a 64-byte aligned `Buf` or 16 KiB page-aligned `MmuBuf` into a Metal `id<MTLBuffer>` (`MTLResourceStorageModeShared`) with **zero bus copies**:

```rust
use cgap_arrow::mmu_arena::MmuBuf;
use cgap_gpu::MetalSharedBuffer;

pub fn map_to_metal_zero_copy() {
    // Allocate a 16 KiB hardware page-aligned buffer
    let mut mmu_buf = MmuBuf::with_capacity(16384);
    assert!(mmu_buf.is_mmu_page_aligned());
    mmu_buf.as_mut_slice().fill(0xAA);

    // Wrap directly via newBufferWithBytesNoCopy:length:options:deallocator:
    let metal_buf = MetalSharedBuffer::from_mmu_buf(&mmu_buf)
        .expect("Metal device available");
    assert_eq!(metal_buf.len(), 16384);
    let _raw_mtl_buffer = metal_buf.raw_handle();
    // Ensure `mmu_buf` outlives `metal_buf`
}
```

---

## 2. How to Run Horizontal SIMD BLAKE3 Parent Reduction (`cgap-blake3`)

When reducing a batch of 32-byte chunk chaining values (CVs) at the same depth of a BLAKE3 Merkle tree, call `cgap_blake3::reduce_level_simd` to compress 4 parent pairs simultaneously in ARM64 NEON registers (or 8 parent pairs in x86_64 AVX2 registers):

```rust
use cgap_blake3::{hash_chunk, reduce_level_simd};

pub fn reduce_64_chunks_simd(data_64k: &[u8; 65536]) -> [u32; 8] {
    let mut cvs = [[0u32; 8]; 64];
    for chunk_idx in 0..64 {
        let slice = &data_64k[chunk_idx * 1024..(chunk_idx + 1) * 1024];
        let out = hash_chunk(slice, chunk_idx as u64, false);
        cvs[chunk_idx].copy_from_slice(&out[0..8]);
    }

    let mut count = 64usize;
    while count > 1 {
        reduce_level_simd(&mut cvs, &mut count);
    }
    cvs[0]
}
```

---

## 3. How to Choose Between CPU SIMD and GPU Dispatch for BLAKE3

Use `cgap_blake3::should_dispatch_gpu(payload_len)` to route payloads across the 16 MiB crossover threshold:

```rust
use cgap_blake3::{hash_wide_simd, should_dispatch_gpu, GPU_CROSSOVER_BYTES};

pub fn adaptive_hash(payload: &[u8]) -> [u8; 32] {
    if !should_dispatch_gpu(payload.len()) {
        // < 16 MiB: CPU caches + NEON/AVX2 beat GPU kernel launch overhead (< 100 ns dispatch)
        hash_wide_simd(payload)
    } else {
        // >= 16 MiB (16,384+ chunks): eligible for zero-copy Metal Unified Memory SIMT leaf saturation
        hash_wide_simd(payload)
    }
}
```

---

## 4. How to Run the Machine-Code Branch Audit (`make gate1`)

To verify that inner-loop bodies contain zero conditional branches (`BODY 0/0`) and that all loop latches and dispatch checks are accounted for:

```bash
PATH="/opt/homebrew/opt/llvm/bin:$PATH" make gate1
```

---

## 5. How to Run PyArrow C Data Interface Wire Parity (`make gate3`)

To verify zero-copy C Data Interface export/import bitwise parity against PyArrow:

```bash
cargo build -p cgap-ffi
/Users/sac/.local/bin/uv run --with pyarrow python3 scripts/gate3_cdata.py
```
