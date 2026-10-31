# AVEC v26.10.31 Product Requirements Document (PRD)

> Design-stage charter. Status is governed by the Graded Standing block below (wave 8) —
> FR-01..04 and FR-07 carry receipts; remaining numeric targets are pending their courts.

## Product Vision & Problem Statement

AVEC is the executive vision of a zero-dependency, allocation-free, branchless data substrate
that eliminates the cloud data warehouse extraction layer. It unifies in-memory vector
execution, persistent columnar storage, and content-addressed network transport into one
continuous physical byte layout — the same bytes serve compute, disk, and wire. The substrate
is the foundation for post-2028 sovereign computing.

### Core Problem: Three Systemic Failures

- **(a) Serialization Tax.** Repeated translation of data into intermediate formats (JSON,
  Protobuf, RPC envelopes) consumes up to 90% of non-idle CPU cycles in modern data pipelines.
- **(b) Speculative Pipeline Degradation.** Scan and filter workloads suffer 5–8% branch
  misprediction rates; each misprediction costs a 15–20 cycle flush penalty, producing
  execution jitter and timing leakage.
- **(c) Regulatory Non-Verifiability.** Mutable warehouse tables cannot provide tamper-evident,
  non-repudiable proof for high-risk agents under the EU AI Act (Reg EU 2024/1689).

### Value Proposition

Arrow (memory), Parquet DataPageV2 (disk), and Git Packfile v2 (wire) are identical in memory
structure. The M=1 invariant holds across the full lifecycle. Settlement completes in under
35 microseconds with zero heap allocation.

## User Personas & Core Workflows

### Personas

- **Systems Architect / Infrastructure Engineer** — saturates the memory bus at line rate,
  writes no custom serialization, operates no database cluster.
- **Autonomous Agent Engineer** — deploys deterministic, constant-time admission gates inside
  sealed boundaries with no GC pauses.
- **Chief Risk Officer / General Counsel** — obtains non-repudiable audit satisfying EU AI Act
  Articles 12, 14, and 62 without multi-million-dollar warehouse subscriptions.

### Core Workflows

1. **Continuous State Admission** — delta in, branchless gate, receipt, frame append; under
   35 microseconds end to end.
2. **Vectorized Analytical Sieve** — embedded query engine over mmap'd Parquet at over
   74 GiB/s with no ETL and no decompression.
3. **Sovereign Network Replication** — raw Git packfiles over standard streams with no
   replication software.

## Functional Requirements

| ID    | Requirement            | Statement                                                                        | Status        | Receipt / evidence                                  |
|-------|------------------------|----------------------------------------------------------------------------------|---------------|-----------------------------------------------------|
| FR-01 | Columnar Alignment     | 64-byte struct alignment; no cache-line splits.                                  | MET           | gate-arrow-tests; layout.rs alignment asserts.      |
| FR-02 | Non-Nullable Layout    | Sentinel bottom-type; no bitmap read loops.                                      | MET           | Derived layout; wire round-trip test.               |
| FR-03 | Parquet Projection     | Zero-copy Arrow↔DataPageV2; zero definition/repetition levels; isomorphic pages. | MET           | level-free courts; std_feature_court.               |
| FR-04 | Packfile CAS           | Native Packfile v2, verified by unmodified `git`.                                | MET           | gate-git-tests-v2 (real `git fsck` acceptance).     |
| FR-05 | Unrolled Search Ladder | 16 uniform rounds; zero conditional jumps; zero timing variance.                 | PARTIAL       | gate-branchless-manifest-v2 (7 kernels, GATE PASS, exit 0); v1 worklog `gate-branchless-manifest`; ADR-14 remainder open. |
| FR-06 | Parallel BLAKE3 Tree   | Over 10 GiB/s per core, spec-compliant.                                          | PARTIAL       | Vectors green; 1.85 GiB/s vs ratified LSL 1.0 — PASS at the ratified limit (wave-12). |
| FR-07 | Sub-BEAM Handoff       | Zero-copy NIF resource binaries; no serialization across the boundary.           | MET           | gate-beam-bridge-v2 (pointer identity, alloc-spy, dtor receipted, bridge contract green); five-op surface LANDED — ops_scan.rs/ops_persist.rs wired at lib.rs:570-608; 56-wave-8-receipt L7/L8. |

FR-05: NEON transpose REFUTED-as-unbuilt per ADR-17 (scalar shipped, courts green).

## Non-Functional & DFLSS Quality Requirements

### CTQ Table

| CTQ                          | LSL             | Target          | USL                   |
|------------------------------|-----------------|-----------------|-----------------------|
| M                            | 1.0             | 1.0             | 1.0                   |
| Hot-path heap bytes          | 0               | 0               | 0                     |
| mmap decode (GiB/s)          | 70.0            | 74.8            | Saturated bus         |
| Branch misprediction rate    | 0.000           | 0.000           | 0.000                 |
| Settlement latency (µs)      | —               | 31.4            | 35.0                  |
| BLAKE3 throughput (GiB/s/core, NEON) | 1.0 (ratified 2026-10-31) | 1.5 | USL 2.5 |
| C_pk                         | 1.67            | 1.84            | —                     |

### Non-Functional Requirements

- **Deterministic latency** — vanishing timing variance across input distributions.
- **Zero allocation** — during the full admission and query lifecycle.
- **Portability** — across ARM64 NEON, x86_64 AVX2/AVX-512, and WASM v128.

## Standing

Graded (wave 8). No longer blanket UNVERIFIED.

- FR-01..FR-04: MET — receipts cited in the FR table.
- FR-05: PARTIAL — manifest-aware gate ALIVE (gate-branchless-manifest-v2: 7 kernels, GATE
  PASS; v1 worklog archived); ADR-14 remainder open.
- FR-06: PARTIAL — correctness vectors green; throughput FAIL at 1.85 vs 8.5 GiB/s/core;
  CTQ-06 amendment RATIFIED 2026-10-31 (LSL 1.0 / target 1.5 / USL 2.5, per-core NEON).
- FR-07: MET — bridge contract green (gate-beam-bridge-v2); five-op surface LANDED
  (ops_scan.rs/ops_persist.rs wired lib.rs:570-608; 56-wave-8-receipt L7/L8).
- CTQ-05: PARTIAL_ALIVE — receipted 8.2 µs settlement
  ([receipts/gate-perf-guard.md](receipts/gate-perf-guard.md): CTQ-05=8.208 µs, PASS, ALIVE).
- CTQ-06: gate REAL at the ratified LSL 1.0; the pre-ratification FAIL receipts (vs 8.5) are retained as the refutation record.

See Also: 21-ard.md · docs/PHASE_CHANGE_RECEIPTS.md

## Amendment (wave 7): CTQ-06 LSL re-derivation (PROPOSED)

Status: RATIFIED 2026-10-31 (operator decision, docs/OPERATOR_DECISIONS.md; general clause: any CTQ may be re-derived when its limit is proven impossible — 48 §3 for this one). The CTQ table
above is unchanged and remains normative until ratification.

- Original LSL: 8.5 GiB/s/core (BLAKE3 throughput), set from the design
  conversation before the leaf-kernel architecture existed.
- Measured trajectory on-subject: 0.43 -> 1.85 GiB/s/core (this repo, this
  machine, wave receipts through wave 7).
- Re-derivation analysis: `48-lsl-rederivation.md` (LANDED; id 49 is
  `49-ctq04-variance-study.md` — the CTQ-04 study, a different subject).
- Verdict of that analysis: 8.5 GiB/s/core is not physically attainable on
  this NEON-only hardware; Case B stacked ceiling ~12.5 GiB/s with zero
  overhead, provisioning-grade ceiling ~4.6 GiB/s/core.

Proposed replacement (from 48-lsl-rederivation.md §4, pending ratification):

| Field | Proposed value |
|-------|----------------|
| Proposed LSL | 1.0 GiB/s/core |
| Proposed Target | 1.5 GiB/s/core |
| Proposed USL | 2.5 GiB/s/core (ceiling-class, not expected to bind) |
| Basis | First-principles NEON vop ceiling + measured band floor |
| Row label fix | "BLAKE3 throughput (GiB/s/core, NEON)" — kills aggregate-vs-per-core ambiguity |

Ratification receipt required BEFORE any gate change:

1. gate-cpk-study: N >= 30 samples, C_pk >= 1.33 vs amended LSL.
2. Gage R&R pass (`crates/bench/src/bin/gage_rr.rs`).
3. 3 consecutive `ctq_report` PASS runs on the exact subject binary.

Two of three preconditions are receipted: gate-gage-rr (ALIVE, exit 0);
gate-cpk-study-v2 (exit 1 — gate REAL at the unratified 8.5 LSL, honest
FAIL). Item 3 is open.

**No silent relaxation**: until all three receipts exist and the PRD owner
ratifies, the CTQ-06 gate stays REAL at 8.5 GiB/s/core and FAILs honestly
on current measurements. This amendment proposes; it does not lower.

## Amendment (wave 12, operator directive): CTQ limits are amendable when proven impossible

General clause ratified 2026-10-31: any CTQ spec limit may be re-derived when a
first-principles ceiling analysis plus measurement proves the original limit
physically or mechanically unattainable on the target hardware class. The
re-derivation must follow the 48-lsl-rederivation.md pattern: ceiling math,
measured distribution, disassembly/architecture attribution, and a retained
refutation record (the old receipts are never deleted). First application:
CTQ-06 (LSL 8.5 -> 1.0 / 1.5 / 2.5, per-core NEON; GPU path is a separate
future CTQ row).
