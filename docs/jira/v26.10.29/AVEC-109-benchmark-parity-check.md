# AVEC-109: Benchmark Parity & Performance Regression Audit Post-Phase-Change & Branchless Innovations

**Status**: RESOLVED  
**Type**: Task / Performance Audit  
**Assignee**: Sean / Agent  
**Date**: 2026-10-30  
**Resolution**: Completed / Receipts Attached  

---

## 1. Context & Objective
Following the completion of:
- `AVEC-101`: Steam Power (Unified `Batch` / `BatchBuilder` with 64-byte alignment)
- `AVEC-102`: Transistor (Fused Frame-of-Reference + ByteSlice + BitSlice)
- `AVEC-103`: Alternating Current (Normal-Scheduler Twin + Zero-Copy Resource Binaries)
- `AVEC-106`: Shared Memory & Oblivious Manifest Resource Binary
- `AVEC-107`: Branchless Primitives Library & Cache-Separated Topologies
- `AVEC-108`: In-Loop Branch Elimination in Bitmap, RLE, and BitSlice

Execute the full suite of release benchmarks and verify:
1. No regressions against `docs/BENCH_FLOORS.json` and `docs/DFLSS_CHARTER.md`.
2. CTQ-5 zero-allocation invariant across all scan paths.
3. Branch audit preservation (`make branch-audit`).
4. C-Data interface wire parity (`make gate3`).
5. BEAM normal-scheduler and resource binary parity (`make test-beam`).

---

## 2. Benchmark Evidence & Receipts

### 2.1 Automated Floor Gate (`python3 scripts/bench_gate.py`)
- **Status**: **PASS (32/32 metrics within tolerance)**
- Key Throughputs:
  - `mask/0x0000000000000000`: 77.604 GiB/s (+2.1% vs floor)
  - `mask/0x1111111111111111`: 76.122 GiB/s (-2.3% vs floor)
  - `mask/0x5555555555555555`: 74.100 GiB/s (-4.5% vs floor)
  - `mask/0xffffffffffffffff`: 74.662 GiB/s (-3.3% vs floor)
  - `page/decode/repeated`: 76.519 GiB/s (+2.3% vs floor)
  - `page/decode_body/repeated`: 82.336 GiB/s (+6.4% vs floor)
  - `parquet/unpack/random64/w4`: 24.799 GiB/s (+6.6% vs floor)
  - `parquet/rle_decode/repeated/w4`: 81.646 GiB/s (+1.2% vs floor)

### 2.2 Secondary Benchmarks
- **ByteSlice (`byteslice_bench`)**:
  - Point lookup: 0.910 ns/lookup across $k \in \{2, 4, 8\}$; 1.346 ns/lookup for $k=12$.
  - Scan rate: 2.656–2.789 GiB/s (1,482–2,994 Mrows/s). 0 allocations.
- **Dictionary Bench (`dict_bench`)**:
  - Decode rate: 3.577–3.640 GiB/s with 0 allocations.
- **Packed Filter Bench (`packed_filter_bench`)**:
  - $w=1$: 19,716.4 Mrows/s logical (32.60x speedup over materialize-then-mask).
  - $w=4$: 1,993.4 Mrows/s logical (3.00x speedup over materialize-then-mask).

### 2.3 Gates Verification
- `make gates`: **ALL GATES PASS (Gate 0, 1, 2, 3)**
- `make branch-audit`: **PASS** (3/9 inner loops completely branchless, 71 loop-latch/dispatch/bounds itemized, 0 unaccounted).
- `make test-beam`: **PASS** (BEAM NIF tests pass, resource binaries persist through GC, fast normal-scheduler wins up to $n=65,536$).
