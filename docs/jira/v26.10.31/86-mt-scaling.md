# Multi-Core BLAKE3 Scaling Note (operator directive, wave 12/13)

Operator analysis 2026-10-31. Design basis for the multi-threaded bulk
hash path — this REPLACES the GPU row as the primary bulk-throughput
direction (GPU deferred to a background-compaction study; see
76-gpu-row-design.md for the dispatch-latency verdict that keeps the
admission turnstile on CPU).

## The scaling math (M3 Max, 16 cores = 12P + 4E)

- Receipted single-core baseline (P-core): 1.85 GiB/s (hash_many_simd).
- E-core estimate: 1.85 x (2.75/4.05) x ~0.50 (reduced NEON dispatch)
  ~= 0.63 GiB/s.
- Raw linear sum: 12x1.85 + 4x0.63 = 22.20 + 2.52 = 24.72 GiB/s.
- Degradation factors: all-core thermal freq drop ~0.901; Merkle parent
  reduction ~0.985 (<0.1% of cycles — 4 log-passes over 768 bytes);
  L2 crossbar arbitration ~0.950. Aggregate efficiency ~0.843.
- Realized: 12P-only ~18.7 GiB/s; 16-core ~20.8 GiB/s.
- DRAM bus is NOT a constraint: required ~20-24 GiB/s vs 300-400 GiB/s
  LPDDR5X (<7% of bus).
- Workload constraint: buffers >= 2 MiB (>= 2,048 chunks; >= 128
  chunks/core to saturate the x2 driver).

## Architectural verdict (operator)

Pure multi-threaded CPU eliminates Metal driver setup, kernel-launch
latency, and UMA sync logic while staying in no_std/POSIX thread pools.
The GPU is not involved in the bulk path.

## CTQ consequences

- FUTURE CTQ-08 (multi-core bulk BLAKE3): candidate LSL 15 GiB/s
  (below the 12P realized estimate, above single-core x8), target
  18.7, measured on >= 64 MiB buffers, quiet machine, gage-preconditioned.
- The 2 MiB dispatch knee joins the GPU 1 MiB knee in the dispatch
  law: < 64 KiB small-batch CPU single-thread; 64 KiB - 2 MiB
  single-core SIMD; >= 2 MiB multi-core thread pool.
- Implementation surface: a std-threaded driver over hash_many_simd
  chunk-range partitioning + the existing Merkle merge (the tree
  algebra already supports it — wave-5's merge_into_stack).

Standing: DESIGN RECORD + directive. Implementation is a wave-15 lane
(performance depth); this note is its design basis. The 0.843 efficiency
factor is a PREDICTION — the wave-15 lane must measure it and record the
realized factor honestly (the CTQ-06 history shows why predictions are
not receipts).

See Also: 76-gpu-row-design.md, 48-lsl-rederivation.md, 20-prd.md.
