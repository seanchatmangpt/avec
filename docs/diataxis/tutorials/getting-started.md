# Tutorial: Getting Started with `avec`

This tutorial walks through building a 64-byte cache-aligned Arrow column, running a compressed-domain algebraic aggregation without unpacking, and computing a cryptographic Merkle root using `avec-blake3`.

---

## Step 1: Verify the Substrate Gates

Run the fast core tests for `avec-arrow`, `avec-blake3`, and `avec-gpu`:

```bash
cargo test -p avec-arrow --lib
cargo test -p avec-blake3
cargo test -p avec-gpu
```

Run the civilizational phase-change benchmark suite:

```bash
cargo run --release -p bench --bin phase_change_bench
```

Expected output:
```text
=== 1. Compressed-Domain Algebraic Execution (sum_masked_vertical_128) ===
  w= 1 | physical:    7.86 GiB/s | LOGICAL UNCOMPRESSED:   503.11 GiB/s |  67525.7 Mrows/s | 0 allocs
...
=== 5. Sovereign #![no_std] BLAKE3 (avec-blake3) ===
  64 KiB (64-chunk Merkle tree)      | throughput:    0.41 GiB/s | 0 allocs
  SIMD 4-way NEON parent reduction   | throughput:    1.31 GiB/s |    21.9 Mparents/s | 0 allocs
```

---

## Step 2: Build a 64-Byte Aligned Arrow Buffer & Compute its BLAKE3 Merkle Root

Every `avec_arrow::Buf` is allocated on a 64-byte hardware cache-line boundary (invariant `M-03`), and `avec_arrow::mmu_arena::MmuBuf` aligns to a 16 KiB hardware MMU page boundary (`16,384` bytes).

```rust
use avec_arrow::Buf;
use avec_blake3::{hash, hash_wide_simd, should_dispatch_gpu};

fn main() {
    // 1. Allocate a 64 KiB buffer (64 chunks of 1024 bytes)
    let mut buf = Buf::new(64 * 1024);
    for (i, b) in buf.as_mut_slice().iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }

    // 2. Check the CPU-SIMD vs. GPU crossover law (16 MiB threshold)
    assert!(!should_dispatch_gpu(buf.len()));

    // 3. Compute the 32-byte BLAKE3 Merkle digest with 0 heap allocations
    let d_stack = hash(buf.as_slice());
    let d_simd = hash_wide_simd(buf.as_slice());
    assert_eq!(d_stack, d_simd);
}
```

---

## Step 3: Compressed-Domain Algebraic Sum (`avec-parquet`)

Instead of inflating 4-bit integers into 64-bit scalars in RAM before summing, `sum_masked_vertical_128` computes the filtered sum directly on vertical bit-slices:

$$\sum_{i \in M} v_i = (k \cdot b) + \sum_{j=0}^{w-1} 2^j \cdot \text{popcnt}(B_j \land M)$$

```rust
use avec_parquet::bitslice::{pack_vertical, sum_masked_vertical_128};

fn example_compressed_sum() {
    let values: Vec<u32> = (0..128).map(|i| (i % 16) as u32).collect();
    let w = 4u8;
    let packed = pack_vertical(&values, w);

    let base = 1000i64;
    let mask_lo = 0xFFFF_FFFF_FFFF_FFFFu64;
    let mask_hi = 0xFFFF_FFFF_FFFF_FFFFu64;

    let total = sum_masked_vertical_128(&packed, w, base, mask_lo, mask_hi);
    assert_eq!(total, 128 * base + values.iter().map(|&v| v as i64).sum::<i64>());
}
```
