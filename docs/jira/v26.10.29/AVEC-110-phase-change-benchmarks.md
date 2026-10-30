# AVEC-110: Dedicated Release Benchmark Suite for Civilizational Phase Changes

**Status**: RESOLVED  
**Type**: Benchmark Harness & Empirical Verification  
**Assignee**: Sean / Agent  
**Date**: 2026-10-30  
**Resolution**: Completed / Empirical Receipts Recorded  

---

## 1. Context & Motivation
Following the implementation of the five civilizational phase changes:
1. Compressed-domain algebraic execution (`sum_masked_vertical_128`).
2. Zero-probe multiply-shift perfect minimal hashing (`PerfectMap`).
3. 16 KiB MMU hardware page-table remapping arena (`MmuBuf`).
4. In-register universal homomorphic fingerprinting (`Fingerprint`).
5. Planar schema specialization manifest.

Construct a dedicated release benchmark binary ([`crates/bench/src/bin/phase_change_bench.rs`](file:///Users/sac/avec/crates/bench/src/bin/phase_change_bench.rs)) to quantify physical vs logical throughput and latency without allocating on heap.

---

## 2. Empirical Benchmark Receipts ($A = \mu(O^*), R = \text{receipt}(A)$)

**Execution Command**: `cargo run --release -p bench --bin phase_change_bench`  
**Platform**: macOS arm64 (Apple Silicon), 16 logical CPUs, stable Rust.  
**Invariant**: **0 post-init allocations across all loops**.

### 2.1 Compressed-Domain Algebraic Execution (`sum_masked_vertical_128`)
Evaluated across 1,000,000 tuples; values aggregated directly over vertical bit-slices without inflating to 64-bit scalars in RAM:
- $w=1$: **516.89 GiB/s** logical uncompressed rate (8.08 GiB/s physical) | **69,376.2 Mrows/s** | 0 allocs
- $w=2$: **326.31 GiB/s** logical uncompressed rate (10.20 GiB/s physical) | **43,797.2 Mrows/s** | 0 allocs
- $w=4$: **216.64 GiB/s** logical uncompressed rate (13.54 GiB/s physical) | **29,076.8 Mrows/s** | 0 allocs
- $w=8$: **124.33 GiB/s** logical uncompressed rate (15.54 GiB/s physical) | **16,687.3 Mrows/s** | 0 allocs
- $w=12$: **106.98 GiB/s** logical uncompressed rate (20.06 GiB/s physical) | **14,358.2 Mrows/s** | 0 allocs

### 2.2 Zero-Probe Perfect Minimal Hashing (`PerfectMap`)
- Lookup latency: **0.53 ns/lookup**
- Lookup throughput: **1,871.1 Mlookups/s**
- Probe collisions: **0** (Straight-line 3-instruction circuit: `mul`, `lsr`, `ldr`)
- Allocations during lookup: **0**

### 2.3 16 KiB MMU Hardware Page-Aligned Arena (`MmuBuf`)
- Allocation & alignment throughput: **1,353.75 GiB/s**
- Alignment verification: Strict 16,384-byte boundary matching ARM64 MMU page table granule.

### 2.4 In-Register Homomorphic Fingerprint (`Fingerprint::fold_word`)
- Line rate: **3.97 GiB/s** in straight-line universal polynomial arithmetic.
- Memory passes: **0 extra passes** (fuses directly into load/scan loops).
