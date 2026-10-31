# Performance Thresholds as Receipt Contract

Status: DESIGN — thresholds are conversation-cited legacy claims, not re-measured
this session. Every value below is a non-regression *target* carried forward from
prior purrdf/avec milestone claims and MUST be re-run against real benches before
any landing claim cites it. Receipt-cited counts are stale by default.

## Scope

Target: stardf/avec v26.10.31. Predecessor lineage: purrdf (logic plane: N3
parsing, SHACL validation, W3C conformance, evaluation latency) and avec
(physical plane: literal encoding, Parquet DataPageV2 decoding). This document
pins both planes' floors as a machine-checkable receipt contract so v26.10.31
cannot silently regress a claimed capability.

## Threshold Table

All rows are design targets inherited from prior milestone claims; re-verify
before reuse.

| Metric | Bound | Target | Validation target component |
|---|---|---|---|
| n3ParsingThroughput | >= | 966000 elem/s | purrdf-lineage N3 parser (logic plane) |
| shaclPerNodeLatencyNs | <= | 340 | SHACL validation engine (per node) |
| conformanceTestFailures | == 0 | over >= 1600 tests | W3C conformance suite runner |
| evaluationLatencyP99999Ms | <= | 8.0 | evaluation path, release mode, P99.999 |
| literalEncodingGiBs | >= | 5.3 | avec literal encoder (branchless, memory-bus line rate; legacy
band 5.3-5.5) |
| dataPageV2DecodingGiBs | >= | 74.0 | Parquet DataPageV2 decoder (legacy band 74-77) |
| heapAllocationsDuringDecode | == 0 | 0 | DataPageV2 decode path, allocation-tracking allocator |
| conditionalBranchCount | == 0 | 0 | hot decode/encode loops; disassembly audit / perf counters |
| memoryAlignmentBytes | == | 64 | decode/encode buffers (cache-line alignment) |

Operator semantics: `>=`/`<=` are inclusive numeric bounds; `== 0` rows are
structural invariants (any nonzero observation is a violation, not noise);
`== 64` is an exact structural requirement.

## SHACL Receipt-Contract Design

The threshold table is pinned as SHACL shapes so a performance receipt is a
validatable RDF document, not prose.

Two NodeShapes, one per plane:

- `stardf:LogicPlanePerformanceShape` — targets the logic-plane receipt
  individual; carries `sh:minInclusive` on n3ParsingThroughput (966000),
  `sh:maxInclusive` on shaclPerNodeLatencyNs (340) and
  evaluationLatencyP99999Ms (8.0), `sh:hasValue 0` on
  conformanceTestFailures with `sh:minInclusive 1600` on conformanceTestCount.
- `stardf:PhysicalPlanePerformanceShape` — targets the physical-plane receipt
  individual; carries `sh:minInclusive` on literalEncodingGiBs (5.3) and
  dataPageV2DecodingGiBs (74.0), `sh:hasValue 0` on heapAllocationsDuringDecode
  and conditionalBranchCount, `sh:hasValue 64` on memoryAlignmentBytes.

Each constraint carries a typed violation message naming the metric, its
bound, and the measured value, so a failed validation is directly actionable
without re-deriving which row flipped.

`stardf:PerformanceReceiptContract` is the header class binding a receipt to
its lineage, with properties:

- `targetCrate` — `stardf` (the crate whose benches must satisfy the shapes).
- `predecessor` — `purrdf` (source of the logic-plane floors).
- `dataPlane` — `avec` (source of the physical-plane floors).
- `conformanceSuite` — `W3C-RDF-1.2-SHACL-RDFC-1.0` (the suite whose pass
  count backs conformanceTestFailures).

## Divan Harness Design

Explicit decision: Divan, not criterion. Rationale: Divan reports allocation
counts and byte throughput natively (ItemsCount/BytesCount counters), which
two threshold rows (`heapAllocationsDuringDecode`, `literalEncodingGiBs` /
`dataPageV2DecodingGiBs`) require; criterion would need a custom harness for
the same signal.

- Global allocator wrapper that tracks allocation count during the measured
  region; the DataPageV2 decode bench asserts the count is exactly 0 at the
  end of each iteration (structural row, not a timing row).
- Bench groups: `stardf_logic` (N3 parsing throughput, SHACL per-node
  latency, evaluation latency) and `avec_physical` (literal encoding
  throughput, DataPageV2 decode throughput, allocation assertion).
- Counters: `ItemsCount` for elements/nodes/pages, `BytesCount` for bytes
  through encode/decode so Divan emits GiB/s directly comparable to the
  threshold table.
- Evaluation latency P99.999 is not a Divan-native statistic; it is captured
  as a separate release-mode measurement ( Divan group records the run, the
  tail percentile comes from the latency-collection pass feeding the receipt).

## divan2receipt Converter Design

A zero-dependency CLI (no clap, no serde derive beyond what the standard
toolchain already vendors — parse the Divan JSON by hand or with the existing
workspace's minimal JSON reader) that closes the turnstile:

1. Parse Divan JSON output from the `stardf_logic` and `avec_physical` groups.
2. Assert each threshold-table row against the corresponding Divan metric,
   including the `== 0` structural rows.
3. Emit a `stardf:PerformanceReceiptContract` Turtle instance: one
   logic-plane individual validated against
   `stardf:LogicPlanePerformanceShape`, one physical-plane individual against
   `stardf:PhysicalPlanePerformanceShape`, both under a contract header
   naming targetCrate/predecessor/dataPlane/conformanceSuite and the exact
   bench invocation (commit SHA, command, timestamp).
4. Exit 1 on any violation, and emit no receipt Turtle on failure — receipt
   emission is blocked on validation success (turnstile semantics: a failed
   gate cannot produce a passing-looking artifact).

Non-vacuity requirement: the converter must be demonstrably able to fail —
feed it a mutated Divan JSON with one threshold breached and confirm exit 1
and zero receipt output before trusting a passing run.

## Verification Discipline

Per house rules (no-overclaiming; receipt-cited counts are stale by default):

- Every threshold in the table above is a legacy claim. None has been
  re-measured in this session. Before any landing claim cites these numbers,
  run the real Divan benches on the real stardf/avec subject and attach the
  actual output.
- A bench that cannot fail is decoration: the allocation-zero assertion and
  the divan2receipt exit-1 path must each be witnessed flipping.
- Re-measured values that miss a legacy floor are findings, not failures to
  paper over — register the divergence, do not round the threshold down to
  match the measurement.

## Amendment (wave 5, 2026-10-30)

Premise-rot audit result: every numeric threshold in the table above (966k
elem/s N3 parse, 340 ns SHACL per-node, >= 1600 conformance tests,
5.3-5.5 GiB/s literal encode, 74-77 GiB/s DataPageV2 decode) is a
conversation-cited purrdf/avec claim. None has been re-run in this repo.
All rows are marked **LEGACY-UNVERIFIED** and are retained as historical
floors only — they are not evidence of current capability and must not be
cited as such by any landing claim.

The actual v26.10.31 measurement surface lives here:

- `crates/bench/src/bin/ctq_report.rs` — CTQ pass/fail report bin.
- `crates/bench/benches/v26_10_31.rs` and
  `crates/bench/src/benches/v26_10_31.rs` — Divan bench groups.
- `crates/bench/src/bin/gage_rr.rs` — Gage R&R measurement-system gate.
- `crates/bench/src/bin/blake3_throughput.rs` — BLAKE3 throughput bench.

Current receipts pointers: `docs/jira/v26.10.31/receipts/` (gate receipts
including `gate-blake3-throughput.md`, `gate-blake3-zero-alloc.md`,
`gate-arrow-tests.md`).

Wave-5 extension (2026-10-30): the earlier "no receipt yet pins any
legacy-table value" sentence is now FALSE — two legacy values have been
tested against real benches, both unmet:

- `parquet-throughput-baseline.md` — 65.6 GiB/s measured minimum vs the
  74 GiB/s legacy claim: 11% short, PARTIAL.
- `gate-blake3-throughput.md` / `gate-cpk-study.md` — blake3 throughput
  0.43 -> 1.85 GiB/s vs the 8.5 GiB/s floor: FAIL at the legacy limit.

One ratification prerequisite for the wave-7 amendment is already
satisfied: `gate-gage-rr.md` is ALIVE (Gage R&R measurement-system gate
passed). The remaining prerequisites (gate-cpk-study vs amended LSL,
3 consecutive ctq_report PASS runs) are still open.

## Standing

PARTIAL — legacy floors tested and unmet (parquet 65.6 GiB/s vs 74, 11%
short; blake3 1.85 GiB/s vs 8.5); CTQ-06 enforcement limit stays 8.5
pending ratification. Contract shapes remain design-captured.

## See Also

- [18-dflss-charter.md](18-dflss-charter.md)
-
[../v26.10.29/AVEC-110-phase-change-benchmarks.md](../v26.10.29/AVEC-110-phase-change-benchmarks.md)

## Wave-7 amendment: CTQ-06 gate basis under re-derivation (PROPOSED)

Matched to the wave-7 amendment in `20-prd.md`. PROPOSED, not ratified.

- Re-derivation analysis: `48-lsl-rederivation.md` (LANDED; id 49 exists
  as `49-ctq04-variance-study.md` — the CTQ-04 study, a different
  subject; the re-derivation is 48).
- Proposed amended CTQ-06 limits (pending DFLSS ratification):
  LSL 1.0 / Target 1.5 / USL 2.5 GiB/s/core (per-core, NEON).
- Original CTQ-06 LSL 8.5 GiB/s/core predates the leaf-kernel
  architecture; measured trajectory 0.43 -> 1.85 GiB/s/core.
- Gate contract unchanged: `ctq_report` continues to evaluate CTQ-06
  against **8.5 GiB/s/core** and reports FAIL until ratification lands.
  No bench, receipt, or script may pre-adopt the proposed numbers.

Ratification gate (all required, in order): gate-cpk-study (N >= 30,
C_pk >= 1.33 vs amended LSL) + Gage R&R pass + 3 consecutive
`ctq_report` PASS runs. Until then this contract's enforcement limit is
8.5 and honest FAIL is the correct output.
