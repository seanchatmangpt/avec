# T14: GPU/Multi-Core Dispatch Applicability for PQC Batch Operations

Phase-0 explore ticket, v26.11.1 PQC wave, agent 15 of 20, 2026-10-31.
Design basis: `../v26.10.31/76-gpu-row-design.md` (threshold-gated
dispatch) and `../v26.10.31/86-mt-scaling.md` (multi-core scaling
algebra). This ticket transfers the established dispatch-knee +
ceiling-algebra pattern from BLAKE3 to the PQC batch surface.

Index note: the current `00-index.md` slot table assigns `t14` to
`t14-risk-open-questions.md`; this file lands OUTSIDE that table and
requires an index amendment by the index owner (agent 1) — the index is
amended, never bypassed.

## Standing

- DESIGN RECORD — no implementation claim. Nothing here has executed.
- Everything is UNVERIFIED; every number marked PREDICTION stays a
  prediction until a wave-15 measurement receipt lands (the CTQ-06
  history shows why predictions are not receipts).
- External reference-implementation cycle/throughput figures for ML-KEM
  and ML-DSA are UNVERIFIED-EXTERNAL: they enter as KAT-validation and
  wave-15 measurement inputs only, never as receipted facts (discipline
  4, ADR-19; `../v26.10.31/22-adr-register.md`).

## 1. The Pattern to Apply

### 1.1 From 76: threshold-gated dispatch

- Two regimes, decided by a branchless threshold at ingestion, never by
  per-op branching inside the kernel:
  - Small-batch / latency-bound: dispatch latency dominates. On Apple
    UMA the command-buffer encode + commit + semaphore wait is ~4-8 us
    (~15-30 us over PCIe) — against a measured BLAKE3 admission budget
    of 8.2-10.3 us (USL 35 us), the budget is spent before one hash
    round executes. CPU SIMD is mandatory for the turnstile.
  - Bulk: above a knee (76 proposed 1 MiB theory knee; the existing
    `GPU_CROSSOVER_BYTES = 16 MiB` hook is the conservative form), the
    workload is embarrassingly parallel and the fixed dispatch cost
    amortizes. GPU wins only here.
- The exact knee constant is a measurement task, not a hand-picked
  constant.

### 1.2 From 86: multi-core scaling algebra

M3 Max, 16 cores = 12P + 4E, with the receipted single-core baseline:

- P-core 1.85 GiB/s (hash_many_simd, receipted); E-core estimate 0.63
  GiB/s (freq ratio 2.75/4.05 x ~0.50 reduced-NEON dispatch derate).
- Raw linear sum 24.72 GiB/s; degradation factors: all-core thermal
  ~0.901, Merkle parent reduction ~0.985, L2 crossbar arbitration
  ~0.950; aggregate efficiency ~0.843 (PREDICTION).
- Realized predictions: 12P-only ~18.7 GiB/s; 16-core ~20.8 GiB/s.
- DRAM not a constraint (<7% of LPDDR5X bus).
- Knee: buffers >= 2 MiB (>= 2,048 chunks, >= 128 chunks/core).
- Dispatch law (86): < 64 KiB single-thread; 64 KiB - 2 MiB single-core
  SIMD; >= 2 MiB multi-core thread pool.
- The 0.843 factor is a PREDICTION; the wave-15 lane must measure and
  record the realized factor honestly.

## 2. PQC Batch-Workload Taxonomy

Three classes, split by parallelism shape x latency sensitivity:

- T (turnstile): a single ML-KEM decaps or ML-DSA verify at the
  admission gate. One op, latency-bound.
- B1 (enrollment): ML-KEM bulk encapsulation for archival key
  enrollment — thousands of independent encapsulations, one DEK per
  archive/epoch. Batchable.
- B2 (OCEL sign): ML-DSA bulk signing of OCEL event streams —
  per-event signatures over append-only logs. Batchable.

| Class | Parallelism shape | Dispatch verdict |
|---|---|---|
| T | One op, latency-bound | CPU single-core (76 verdict inherited) |
| B1 | Parallel ACROSS ops; results concatenate | Batch path (s4) |
| B2 | Parallel ACROSS ops; results concatenate | Batch path (s4) |

Structural note: 86's parallelism was INTRA-buffer (chunk-range
partitioning + Merkle merge over one tree). PQC batch parallelism is
INTER-op — a pure map with no cross-op merge step at all. Each
encapsulation and each signature is independent; the only serial
component is the per-op tail (XOF seeding, encode, compress), which is
the Amdahl term and is expected to be far smaller than 86's 0.985
Merkle term. UNVERIFIED until the per-op serial-tail census lands
(open item O5).

## 3. The Math Transfer

### 3.1 Kernel shape (UNVERIFIED-EXTERNAL)

- ML-KEM-768: k = 3, n = 256, q = 3329. Encapsulation is NTT-dominant:
  in shape, ~k·(NTT + basemul) — keygen/encaps sample the matrix A-hat
  (k^2 = 9 SampleNTT-class ops), noise (2k = 6 CBD+NTT), one forward
  NTT of the message row, k basemuls, inverse NTTs for u/v, compress.
  Exact op census is a wave-2 kernel-design input (t01/t06 lanes), not
  a t14 deliverable.
- ML-DSA: same NTT family over q = 8380417, complete NTT (8 layers)
  plus rejection-sampling loops — the rejection loops are the one
  branchless-manifest-sensitive structure class that BLAKE3 had no
  analogue for; t06/t12 own that audit.
- The exact NTT layer counts, basemul conventions, and cycle costs
  from reference implementations are UNVERIFIED-EXTERNAL. Nothing in
  this section is a performance receipt.

### 3.2 Multi-core scaling prediction (the 0.843 transfer)

The 0.843 aggregate decomposes term-by-term; the terms do NOT all
transfer:

| 86 term | Value | Transfer verdict |
|---|---|---|
| All-core thermal freq drop | ~0.901 | Yes, identically |
| Merkle parent reduction | ~0.985 | No analogue |
| L2 crossbar arbitration | ~0.950 | Conservative floor |
| Aggregate | ~0.843 | PREDICTION, NTT-dominant portion |

- Thermal ~0.901: transfers identically — a hardware property of a
  compute-dense all-core load; batch NTT is compute-dense.
- Merkle ~0.985: no analogue — PQC batch has no tree merge (s2
  structural note); replaced by a per-op serial-tail Amdahl term,
  expected smaller. Measure, don't assume (O5).
- L2 ~0.950: transfers as a CONSERVATIVE floor — per-op working sets
  are KiB-class (one poly = 512 B; zeta tables KiB-class, L1-resident,
  read-only shared) vs 86's MiB-class streaming buffers, so crossbar
  pressure should be LOWER; 0.950 is pessimistic, not optimistic.
- Aggregate ~0.843: applies to the NTT-dominant portion identically AS
  A PREDICTION; the realized factor is a wave-15 measurement per
  scheme, ADR-19-amendable.

Prediction: throughput(S) ~= 0.843 x (12 x r_P(S) + 4 x r_E(S)) ops/s
for scheme S over the NTT-dominant portion, where r_P, r_E are
single-core P/E rates measured, not assumed. The E-core rate needs
re-derivation for NTT-class arithmetic (open item O3).

### 3.3 The GPU knee question (wave-15 study)

Batch NTT on GPU is a different kernel shape than BLAKE3 chunking:
the NTT butterfly is shuffle-heavy — its permutation stage is
cross-lane/cross-thread data movement, the classic GPU story of warp
shuffles and shared-memory bank conflicts. The 76 ceiling algebra
(dispatch latency vs compute time vs transfer cost) is the METHOD and
transfers; its constants do not:

- Dispatch-latency floor: same 4-8 us UMA class as 76. The knee is
  where amortized (dispatch + transfer) per op < CPU-core time per op.
  With thousands of independent ops in flight, the amortization case
  is far stronger than BLAKE3's single-buffer case — this is exactly
  why the question stays open instead of being dismissed by analogy.
- Transfer cost: batch state is KiB-class per op; UMA zero-copy
  aliasing (76's `MTLResourceStorageModeShared`) applies to a batch
  buffer identically.
- New algebra term: a shuffle/bank-conflict degradation factor for
  butterfly permutations, with no BLAKE3 analogue — wave-15 derives it
  before any GPU CTQ row exists.
- Standing: DESIGN RECORD only. The avec-gpu crate is buffer-wrapping
  only per the wave-13 audit; nothing in this ticket changes that.

## 4. Verdict Posture

1. CPU multi-core is the PRIMARY batch path for B1/B2 — the 86 pattern,
   std/POSIX thread pool (hand-rolled per discipline 2, t08 lane), no
   Metal driver setup, no kernel-launch latency, no UMA sync logic.
2. GPU remains a wave-15 study for the bulk-NTT case only, with the 76
   algebra as the method. No GPU dispatch in any wave-1/wave-2 plan.
3. The turnstile (class T) stays CPU single-core permanently — the 76
   verdict transfers by regime logic (dispatch latency dominates small
   latency-bound work; single-op GPU occupancy is a fraction of a
   wave). The MAGNITUDES do not transfer: a PQC op is one-to-two
   orders of magnitude more cycles than a BLAKE3 chunk, so the PQC
   turnstile needs its own latency ceiling derivation, not an inherited
   8.2-10.3 us number (open item O6).
4. The dispatch law extends: the PQC batch knee is measured in
   OPERATIONS (N >= N_knee ops per pool submission), not bytes.
   N_knee falls out of the same amortization argument as 86's 2 MiB
   knee and is a wave-15 calibration, not a hand-picked constant.

## 5. CTQ Candidates

Candidate rows only — numbering coordinates with the reserved FUTURE
CTQ-07 (76, bulk GPU) and CTQ-08 (86, multi-core BLAKE3) slots; the
register of record is the 20-prd.md ratified CTQ table via the t09
lane (ADR-19 amendment policy).

| Candidate | Definition sketch | Amendment status |
|---|---|---|
| PQC-CTQ-A: cycles/op per scheme | encaps/sign/verify cycles | Amendable (ADR-19) |
| PQC-CTQ-B: batch throughput | Ops/s at and above N_knee, thread pool | Amendable (ADR-19) |
| PQC-CTQ-C: dudect t-stat bound | CT audit under the batch driver | Amendable (ADR-19) |
| PQC-CTQ-D: pool latency | Per-op pool handoff cost (queue + wake) | Amendable (ADR-19) |

- PQC-CTQ-A: measured on the target core class; amendable per ADR-19
  via the doc-48 re-derivation pattern (ceiling math + measured
  distribution + architecture attribution + retained refutation).
- PQC-CTQ-B: quiet machine, gage-preconditioned, 86-method.
- PQC-CTQ-C: no secret-dependent indexing into the batch,
  secret-independent scheduling — the dispatch threshold branches on
  PUBLIC batch geometry only; dudect-class fixed-vs-random t-statistic
  bound; method owned by t12.
- PQC-CTQ-D: bounded and measured; the PQC analogue of 86's
  no-dispatch-latency claim and an input to N_knee.

Every candidate is UNVERIFIED and unratified: these are inputs to the
t09 amendment lane and the CTQ table, not rows in it.

## 6. Open Items

- O1: N_knee calibration — ops per pool submission at the amortization
  crossover, per scheme (wave-15).
- O2: realized multi-core efficiency factor per scheme vs the 0.843
  prediction; record honestly (86's own law).
- O3: E-core derate for NTT-class arithmetic — 86's ~0.50 NEON-dispatch
  derate was hash-class; the (2.75/4.05) freq ratio transfers, the
  dispatch derate does not automatically.
- O4: GPU bulk-NTT ceiling algebra — constants and the new
  shuffle/bank-conflict term, before any FUTURE GPU CTQ row (76 method,
  wave-15).
- O5: per-op serial-tail census (XOF seeding, encode, compress) — the
  Amdahl term replacing 86's 0.985 Merkle term.
- O6: PQC turnstile latency ceiling — own derivation for single-op
  decaps/verify budgets; do not inherit the BLAKE3 numbers.
- O7: CTQ numbering coordination with the reserved FUTURE CTQ-07/08
  slots and the 20-prd.md table (t09 lane).
- O8: rejection-sampling loop classification for the branchless
  manifest (t06/t12 interface) — batch driver must not turn a
  secret-independent public branch into a manifest exception.

## See Also

- [../v26.10.31/76-gpu-row-design.md](../v26.10.31/76-gpu-row-design.md)
  — dispatch-latency verdict, branchless threshold law.
- [../v26.10.31/86-mt-scaling.md](../v26.10.31/86-mt-scaling.md) —
  12P+4E scaling math, 0.843 prediction, 2 MiB knee, dispatch law.
- [../v26.10.31/48-lsl-rederivation.md](../v26.10.31/48-lsl-rederivation.md)
  — the re-derivation pattern ADR-19 names for CTQ amendment.
- [../v26.10.31/22-adr-register.md](../v26.10.31/22-adr-register.md) —
  ADR-19 (CTQ limits amendable when proven impossible).
- [00-index.md](../00-index.md) — milestone charter and disciplines.
