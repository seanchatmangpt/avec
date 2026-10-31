# Benchmark Harness Plan (Divan) & Receipt Pipeline

Status: DESIGN — plan only, no bench executed this session. Every threshold
restated here is inherited from the contract (`04-benchmark-contract.md`) and
remains UNVERIFIED until a Divan run on the pinned profile produces a receipt.
Conversation-cited historical numbers are stale by default.

## Harness Decision: Divan, Not Criterion

Divan is selected over criterion for all runtime benches.

Rationale:

- Zero statistical-bloat overhead: criterion spends per-iteration bookkeeping
  and warmup machinery that contaminates the microsecond-scale measurements the
  CTQ table cares about (CTQ-05 admission is a <=35us USL). Divan times with
  minimal harness intrusion.
- Native counters: `ItemsCount` and `BytesCount` map directly onto the CTQ
  units (GiB/s decode throughput needs bytes; OID lookups need items). No
  custom per-bench unit plumbing.
- Allocation tracking: Divan's allocation counting pairs with a counting
  global allocator to make CTQ-02 a first-class measured quantity rather than
  an out-of-band check.

Criterion is not adopted anywhere in this plan; no dual-harness maintenance
burden is accepted.

## Bench Inventory Mapped to CTQs

| CTQ | Method | Divan role |
|---|---|---|
| CTQ-01 branch rate | static disassembly audit; perf-counter spot check | none |
| CTQ-02 allocations | counting allocator asserted 0 in decode scan and admission | alloc counter |
| CTQ-03 decode GiB/s | mmap Parquet scan, `BytesCount`, floor 70.0 / target 74.8 | primary |
| CTQ-04 OID lookup variance | min-of-N paired rounds vs sleep-free baseline (ratio) | primary |
| CTQ-05 settlement latency | end-to-end admission bench, <=35us USL | primary bench |
| CTQ-06 BLAKE3 | per-core throughput bench, LSL 8.5 / target 10.2 GiB/s | primary bench |

CTQ-01 detail: branch-freedom is a structural property of the compiled hot
loops, so the gate is an audit of disassembly (`objdump`/`llvm-objdump` of the
release build) asserting zero conditional branches in the marked loop bodies.
A perf-counter spot check (branch-miss events) is optional corroboration, never
the primary gate.

CTQ-02 detail: the counting allocator wraps `System`, records a delta around
the measured call, and the bench asserts the delta is exactly zero. Any
nonzero count is a violation, not noise (structural invariant, per the
contract's operator semantics).

CTQ-03 detail: input is a memory-mapped Parquet fixture so page-copy costs are
excluded from the decode measurement; bytes processed are reported via
`BytesCount`. Floor (LSL) 70.0 GiB/s, target 74.8.

CTQ-04 detail: per the no-sleep doctrine, absolute-ms bounds are banned. The
fence is a ratio: min-of-N paired rounds of the lookup path against an
independent, sleep-free baseline measured in the same process. The claim under
test is variance relative to the baseline within a paired budget, not
convergence before a clock expires.

CTQ-05 detail: end-to-end admission (the settlement path) benched as one
Divan measurement; USL 35us. Allocation counting is active here too (feeds
CTQ-02).

CTQ-06 detail: BLAKE3 core throughput, LSL 8.5 GiB/s, target 10.2, measured
per-core so multi-core scaling is never silently folded into the floor.

## Gage R&R: Validate the Timer Before Any Throughput Claim

DMADV Measure gate: the timing infrastructure itself is gage-validated before
any throughput number is admitted.

- The CNTVCT_EL0 (Apple Silicon) / RDTSC harness is repeat-measured: same
  subject, same core, N repeated reads; variance across repeats must be <1%.
- If the gage fails the <1% gate, no throughput claim from that harness is
  admissible — repair or replace the timer first.
- Gage results are recorded in the receipt (timer kind, repeat count,
  observed variance) so a receipt consumer can audit the measurement
  instrument, not just the measurement.

## divan2receipt Pipeline

Pipeline: parse Divan JSON output -> assert every CTQ row -> emit a Turtle
receipt instance -> exit 1 on any violation (receipt emission is blocked on
violation; a failing CTQ never produces a passing-looking receipt).

Properties:

- One converter, deterministic output (no timestamps in the graph body; wall
  clock lives in receipt metadata fields only, if at all).
- Exit code is the admission boundary: 0 only when all CTQ assertions pass and
  the Turtle receipt was written; 1 otherwise.
- Non-vacuity: the converter must be witnessed failing — a mutated
  (sub-threshold) input must produce exit 1 with no receipt emitted. A court
  that cannot flip is FAIL; the mutation witness is stored alongside the
  converter.

## CI Wiring

- Benches run on a pinned hardware profile: Apple M3 Max baseline. Receipts
  from other hardware are labeled with their profile and are not comparable to
  M3 Max floors.
- Receipts land under a `receipts/` directory carrying the input SHA (fixture
  + source revision) and tool versions (rustc, Divan, converter version).
- Historical thresholds from prior milestones (e.g. 74-77 GiB/s DataPageV2
  decode, 5.3-5.5 GiB/s literal encode, cited in v26.10.29 lanes) are
  conversation-cited only. They may not be restated as standing floors until a
  re-run on this plan's harness produces a receipt. Until then they are
  targets, exactly as `04-benchmark-contract.md` pins them.

## Prior Art In-Repo

`crates/bench` (including
`crates/bench/src/bin/phase_change_bench.rs`) and the AVEC-110 phase-change
benches already exist in-repo. Consolidation audit items:

- Inventory which phase-change bench subjects overlap CTQ-03/CTQ-06 and fold
  them into the Divan harness rather than maintaining parallel harnesses.
- Preserve any existing fixture generators; reuse them as the Divan bench
  inputs so numbers stay comparable across the migration.
- Retire criterion-era scaffolding after migration; a retired harness is a
  permanent entropy sink, not a fallback.

Standing: UNVERIFIED — no bench, converter, or gage run has been executed
under this plan.

## Amendment 2026-10-30 (wave-8 lane 9, applying audit lane L)

Status and Standing above are stale. Receipts now exist:

- `gate-gage-rr.md` — ALIVE: gage validated, cv_trimmed 0.87% (<1% gate pass),
  3x config table; `--scale`/`--reps` flags landed.
- `gate-divan-ctq.md` — FAILED receipt = the witnessed mutation (converter
  non-vacuity proven; a mutated sub-threshold input produces exit 1).
- `ctq_report` load-gating + `LSL_RATIFIED` hook implemented. Wave-8 lane 2 is
  additionally implementing 49's recommended p99/p50 metric — reference as
  wave-8, not landed here.

### Trimmed-CV semantics (pin)

Raw CV reads as failing; the gate passes on `cv_trimmed < 1%`. Receipts and
gate output must state which statistic is asserted — the <1% gage law binds
cv_trimmed, not raw CV. See gate-gage-rr's config table for the 3x config
under which 0.87% was measured.

### CTQ-04 superseded

This plan's §CTQ-04 ratio-fence prescription is superseded by 49's
gage-conditioned p99/p50 recommendation. The ratio fence text above is
historical; the live metric is 49's.

### Standing

PARTIAL — gage R&R and divan2receipt converter executed with receipts;
Divan CI wiring still absent.

## See Also

- [04-benchmark-contract.md](04-benchmark-contract.md)
- [AVEC-110 phase-change benchmarks](../v26.10.29/AVEC-110-phase-change-benchmarks.md)

## Ratification stamp (wave 12, 2026-10-31)

CTQ-04: the gage-conditioned p99/p50 protocol is RATIFIED (decision 2,
docs/OPERATOR_DECISIONS.md); the retired-pin provenance lives in the
ctq_report JSON (retired_pin field). CTQ-06: the LSL amendment is RATIFIED
at 1.0 / 1.5 / 2.5 GiB/s/core (decision 1); this doc's CTQ-06 table rows
(line ~37, ~64) are superseded by 20-prd's ratified row. General clause:
CTQ limits are amendable when proven impossible (20-prd, wave-12
amendment section).
