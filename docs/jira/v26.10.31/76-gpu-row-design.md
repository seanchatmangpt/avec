# GPU-Row CTQ Design Note (operator directive, wave 12/13)

Operator analysis 2026-10-31, recorded verbatim-in-substance. This is the
design basis for the future GPU CTQ row that the CTQ-06 ratification
scoped OUT of the CPU/NEON limit.

## The two regimes

**Small-batch admission (16 KiB turnstile batches): GPU loses.**
Dispatch latency dominates: command-buffer encode + commit + semaphore
wait is ~4-8 us on Apple UMA (15-30 us over PCIe) — the whole admission
budget (measured 8.2-10.3 us, USL 35 us) is spent before a single hash
round executes. At 16 KiB there are only 16 chunks of parallel work:
>98% of GPU silicon idles. Verdict: CPU SIMD (hash_many_simd, the
ratified 12.75 us path) is mandatory for the admission turnstile. CTQ-05
is a CPU-plane CTQ permanently.

**Bulk ingest (>= 1-4 MiB knee): GPU wins.** The chunk tree is
embarrassingly parallel — a 4 MiB buffer is 4,096 independent 1 KiB leaf
chunks. Apple UMA (MTLResourceStorageModeShared) gives zero-copy
aliasing; a persistent compute queue hides submission overhead. Expected
bulk class: 20-40 GiB/s shared-bus-bound (Apple GPU) to 40-80+ GiB/s
(discrete, compute-bound).

## Dispatch law (branchless threshold at ingestion)

Payload < 1 MiB -> CPU hash_many_simd (the 12.75 us path).
Payload >= 1 MiB -> Metal compute pipeline (bulk throughput class).

The existing GPU_CROSSOVER_BYTES = 16 MiB hook (avec-blake3 lib.rs) is
the current conservative form; the operator analysis proposes 1 MiB as
the theory knee — the exact constant is a wave-15 (performance depth)
measurement task, not a hand-picked constant.

## CTQ consequences

- CTQ-05 (admission): stays CPU-plane; no GPU row.
- CTQ-06 (CPU NEON): ratified at 1.0/1.5/2.5 GiB/s/core — unchanged.
- FUTURE CTQ-07 (bulk GPU throughput): to be defined when the Metal
  kernel exists; candidate LSL classes 20 GiB/s (shared bus floor).
- FUTURE work: threshold calibration study; Metal kernel + ceiling
  algebra; BEAM integration via shared-pointer command buffers.

Standing: DESIGN RECORD — no implementation claim. The avec-gpu crate is
buffer-wrapping only (no tests, no kernel) per the wave-13 audit.

See Also: 48-lsl-rederivation.md (CPU ceiling), 59-ratification-package.md
(s scoping), 20-prd.md (ratified CTQ table + general amendment clause).
