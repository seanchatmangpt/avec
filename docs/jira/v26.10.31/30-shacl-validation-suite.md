# SHACL Validation Suite: Negative & Boundary Corpus Plan

## Scope

This is the conformance court for the v26.10.31 canonical ontology artifact
(`19-ontology-ttl.md`). The TTL has never been executed by any engine in this repo;
this suite is the fix.

One command loads the artifact's classes, properties, kernel individuals, ODRL policy,
SHACL shapes, and Genesis fixture into a real SHACL engine and emits a machine-readable
verdict (JSON or Turtle report per SHACL `sh:conforms` / results graph). The court
runs in two directions:

- Positive: the Genesis release fixture conforms (conforms = true, zero results).
- Negative: each mutation case in the corpus below produces non-conformance with the
  exactly one expected REFUSAL code in the results graph.

The command exits 0 only when every positive case conforms AND every negative case
fails with its pinned code. Any other outcome exits 1.

One command entry point (implementation decides the wrapper shape):

```bash
make shacl-court   # or: just shacl-court
```

## Positive Corpus

The Genesis individuals as written in the artifact must conform:

- `avec:Genesis_Arrow_Batch` (Arrow execution plane membership): alignment 64,
  `nullBitmapPresent false`, kernel `cyclomaticComplexity 1`, `basicBlockCount 1`,
  `internalEdgeCount 0`.
- `avec:Genesis_Parquet_Page`: `decodeThroughputGiBs 74.8` (>= 70 floor), zero
  definition/repetition levels on any typed plane node.
- `avec:Genesis_Git_Packfile`: packfile v2, 256-entry fanout table, 16-stage search
  ladder, `searchTimeVarianceNs` <= 1.5.
- `avec:Genesis_Turnstile_Event`: 0 heap bytes, 0.000 branch miss rate, 31400 ns
  settlement (<= 35000), Cpk >= 1.67.

Kernel individuals `Kernel_ARM64_NEON`, `Kernel_x86_64_AVX2`,
`Kernel_WASM32_SIMD128` must satisfy the kernel shape: M=1 (`cyclomaticComplexity`),
`basicBlockCount 1`, `internalEdgeCount 0`.

Note for implementers: the current plane shapes use `sh:hasValue` on property paths.
`sh:hasValue` is satisfied vacuously when the property is absent, so a typed node
missing the property entirely conforms. If the court must enforce presence, the shapes
need an explicit `sh:minCount 1` companion constraint — decide and record during
implementation, do not silently accept absence.

## Negative / Boundary Corpus

Each case is a typed violation: build a minimal instance graph (a typed node of the
targeted class with the single perturbed value), run the engine, and assert
non-conformance whose result message carries exactly the pinned REFUSAL code.

| Case | Perturbation | Expected code |
|---|---|---|
| N-01 | cache-line alignment 32 (≠ 64) | REFUSAL-01 |
| N-02 | `nullBitmapPresent true` | REFUSAL-02 |
| N-03 | kernel M=2 (`cyclomaticComplexity 2`) | REFUSAL-03 |
| N-04 | definition-level byte length 4 (≠ 0) | REFUSAL-04 |
| N-05 | repetition-level byte length nonzero (≠ 0) | REFUSAL-05 |
| N-06 | decode throughput 60 GiB/s (< LSL 70) | REFUSAL-06 |
| N-07 | search ladder stages 32 (≠ 16) | REFUSAL-07 |
| N-08 | fanout table 128 entries (≠ 256) | REFUSAL-08 |
| N-09 | searchTimeVariance 4 ns (> USL 1.5) | REFUSAL-10 |
| N-10 | `heapAllocationBytes 4096` (≠ 0) | REFUSAL-11 |
| N-11 | `branchMissRate 0.003` (≠ 0) | REFUSAL-12 |
| N-12 | settlement latency 40000 ns (> USL 35000) | REFUSAL-13 |
| N-13 | Cpk 1.4 (< 1.67) | REFUSAL-14 |
| N-14 | ODRL AUM exactly 50000000 (boundary) | see below |

Note on numbering: the task brief listed "ladder stages ≠ 16" before fan-out ≠ 256;
the artifact pins ladder stages as REFUSAL-09 and fanout as REFUSAL-08. The table
above uses the artifact's codes as the register of truth; if the artifact's numbering
differs at implementation time, re-read the TTL and pin to it, not to this table.

### ODRL boundary pinning (N-14)

The `SovereignInvariantPolicy_v1` permissions split at $50M AUM: below is
`odrl:lt` (safe harbor, no duty), at-or-above is `odrl:gte` (regularization duty).
The two operators must tile the real line with no gap and no overlap. The court must
pin this explicitly:

- AUM = 49999999.99 → first permission satisfied, duty-free (positive case).
- AUM = 50000000.00 exactly → the `odrl:lt` permission is NOT satisfied; only the
  `odrl:gte` permission applies, carrying the regularization duty.
- AUM = 50000000.01 → duty applies.

The pinning test asserts the exact boundary value routes to the `gte` branch, not the
`lt` branch — an implementation using `>` instead of `>=` at the boundary is a
statutory defect this case exists to catch. ODRL constraint evaluation semantics for
numeric comparison must come from the engine or an explicit ODRL evaluator, never
hand-rolled inside the court.

## Anti-Vacuity: The Suite Must Be Able to Fail

A conformance suite that cannot fail verifies nothing (shim-court law: a court that
cannot flip is FAIL). The suite carries a built-in mutation harness:

1. Take the conforming Genesis fixture.
2. Apply one mechanical perturbation (e.g. rewrite `decodeThroughputGiBs 74.8` to
   `60.0`, or `fanoutTableEntries 256` to `128`).
3. Re-run the engine on the mutated graph.
4. Assert the verdict is non-conformant AND the result message carries the expected
   REFUSAL code.

If any mutation case still reports `sh:conforms true`, the suite is vacuous: the
shape under test never had teeth. The court exits 1 with `VACUOUS COURT` in that
case. At least one mutation flip must be witnessed per shape family (Arrow, Parquet,
Git, Turnstile, ODRL) — a shape family with no witnessed flip is unproven, not
passing.

The mutation harness itself gets a positive control: corrupting the expected-code
assertion (accepting any code instead of the pinned one) must make the court report
the miss. This is the self-check that the code pinning is not decorative.

## Receipt

Every court run emits a machine-readable verdict (SHACL report in JSON or Turtle)
containing `sh:conforms`, per-result `sh:focusNode`, `sh:resultMessage` (the
REFUSAL codes), and `sh:sourceShape`. The receipt is committed alongside the input
graph SHA (the TTL artifact is content-addressed before the run; the receipt records
that SHA so the verdict is replayable against the exact subject).

- Receipt fields: artifact SHA, engine name + version, conforms boolean, result
  count, per-case expected-code vs observed-code table, mutation-flip witnesses.
- A run whose input SHA does not match the committed artifact is not a receipt for
  this artifact — replay refuses it.

## SHACL Engine Choice

Prior-art first: prefer a mature, maintained engine over any hand-rolled evaluator.
Decision is open at plan time; the pick and its rationale are recorded here when
made.

- **pySHACL** — mature, full SHACL spec coverage, JSON/Turtle report output,
  `--force-advanced`/SPARQL-backed features; pip-installable; the natural default.
- **Rust engines** (e.g. `sr/shacl` lineage in Oxigraph-adjacent ecosystems) —
  keeps the court inside the existing Rust toolchain; check spec coverage and report
  format before committing.

Standing: UNVERIFIED. (Stale 2026-10-30: engine run happened — see the Amendment below;
gate-shacl-v2/v3 receipts.)

## Standing

UNVERIFIED — the TTL artifact has never been loaded into any SHACL engine in this
repo. No court run, no mutation flip, and no ODRL boundary pin has been executed.
This suite is the qualification gate that flips the ontology artifact's standing;
it stays UNVERIFIED until a real engine run with a committed receipt exists.
(Stale 2026-10-30: the engine run happened — gate-shacl FAIL → gate-shacl-v2 ALIVE chain and
gate-shacl-v3 exist in receipts/; superseded by the Amendment below, Standing PARTIAL.)

## Amendment 2026-10-30 (wave-8 lane 9, applying audit lane L — reality divergence)

The plan above diverges from what exists. Current reality:

- **Entry point**: `bash scripts/validate_ontology.sh`. No `make shacl-court`
  / `just shacl-court` target exists.
- **Engine decision record: pySHACL** picked (witnessed in gate-shacl-v2 /
  gate-shacl-v3 receipts). Rationale: mature spec coverage, JSON report output,
  as predicted at plan time.
- **Implemented**: exactly 1 of the 14 negative-corpus cases — the
  settlementLatencyNs mutant (N-12 / REFUSAL-13), witnessed ALIVE in
  gate-shacl-v3.
- **Still plan-only**: N-01..N-11 and N-13 (REFUSAL-01..12, 14); the N-14 ODRL
  boundary tiling; per-family flip witnesses (only the Turnstile family is
  proven); the positive-control self-check on expected-code pinning; the
  receipt spec fields (artifact SHA, per-case expected-vs-observed table,
  replay-refusal on SHA mismatch).

### Standing

PARTIAL — engine validated (pySHACL, gate-shacl-v2/v3) and 1 witnessed mutant
(N-12); 13 negative cases unimplemented. This suite still does not qualify the
ontology artifact; the remaining corpus is the gate.

## See Also

- [19-ontology-ttl.md](19-ontology-ttl.md)
- [04-benchmark-contract.md](04-benchmark-contract.md)
