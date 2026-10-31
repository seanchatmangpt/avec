# Verification Ladder & CI Gates: v26.10.31

Ordered verification plan for the v26.10.31 FRs, cheapest-highest-information first.
Standing: UNVERIFIED. No rung below has been executed for this document; each receipt
lands only with command + exit evidence. (Stale 2026-10-31: rungs now executed — verify-latest.md
10/10 PASS; receipts in receipts/.)

## Ladder Definition

House verify discipline, eight rungs, cheapest first:

1. **Narrow unit** — compile-time asserts, pure-function tests (microseconds).
2. **Property tests** — generated inputs, round-trip and invariant laws (milliseconds).
3. **Integration** — two or more real collaborators, in-process (milliseconds to seconds).
4. **E2E** — full admission loop and real artifacts on disk (seconds).
5. **Chaos** — corrupted inputs, truncated files, concurrent writers (seconds).
6. **Stress** — sustained load, oversized corpora, resource ceilings (minutes).
7. **Benchmark** — measured against CTQ fences, ratio-based not absolute-ms (minutes).
8. **Machine report** — single command emitting milestone verdict JSON (exit 1 on any miss).

A failed rung repairs at that rung before climbing; re-run the failed rung plus the
boundary after fix, never the whole ladder blind.

### FR-to-Rung Map

| FR | Unit | Property | Integration | E2E | Chaos | Stress | Bench | Report |
|----|------|----------|-------------|-----|-------|--------|-------|--------|
| FR-01 (quad page layout) | R1 | R2 | R3 | - | R5 | - | - | R8 |
| FR-02 (sentinels/offsets) | R1 | R2 | R3 | R4 | R5 | R6 | - | R8 |
| FR-03 (Arrow/Parquet) | R1 | R2 | R3 | R4 | R5 | R6 | - | R8 |
| FR-04 (packfile/git) | R1 | - | R3 | R4 | R5 | R6 | - | R8 |
| FR-05 (branchless ladder) | R1 | - | R3 | - | - | R6 | R7 | R8 |
| FR-06 (analytical sieve) | R1 | - | R3 | R4 | - | R6 | R7 | R8 |
| FR-07 (BEAM zero-copy NIF) | R1 | - | R3 | R4 | R5 | - | - | R8 |

## Unit and Property Rungs (R1-R2)

Cheapest, run on every compile:

- **Quad struct layout (FR-01)**: `const _: () = assert!(size_of::<Quad>() == 64);`
  and `assert!(align_of::<Quad>() == 64);` as compile-time asserts — layout errors
  fail the build, not the test suite.
- **Sentinel round-trips (FR-02)**: every sentinel encodes and decodes to identity;
  no sentinel value collides with a legitimate pointer range. Table-driven over the
  full sentinel enumeration.
- **Def/rep levels zero on written pages (FR-01/02)**: a freshly written page has
  def == 0 and rep == 0 in its header bytes; property test writes N pages and asserts
  on the raw bytes.
- **Fan-out partition bounds (FR-02)**: on synthetic index fixtures, every partition's
  [lo, hi) bound is non-overlapping, contiguous, and total coverage equals the input
  range; property test over generated index shapes including empty and single-element.
- **BLAKE3 official vectors (FR-04/06)**: the BLAKE3 reference test-vector JSON runs
  under the sovereign core; byte-identical digests across all vector classes.

## Integration Rungs (R3)

- **Arrow-Parquet isomorphism (FR-03)**: encode Arrow record batch to page stream,
  write Parquet, read back, re-encode — byte-offset mapping is an isomorphism.
  Assert on final decoded equality plus offset-table round-trip, not call counts.
- **Packfile vs git (FR-04)**: after packfile write, `git fsck` and
  `git verify-pack -v` pass unmodified on the produced pack. Real git binary, real
  pack file on disk — no pack-format fake.
- **NIF zero-copy pointer identity (FR-07)**: the binary handed across the NIF
  boundary is the same allocation — assert pointer equality of the sub-binary
  reference into the mmap region across the boundary, verified from the Erlang side.
- **16-stage branchless ladder (FR-05)**: shuffled OID corpora hit all 16 stages;
  disassembly of the hot function contains zero conditional jumps on the hot path
  (ties to the kernel audit gate in 27-branchless-kernel-abi.md). Fail = any stage
  unhit or any `jcc` on the measured path.

## E2E Rungs (R4)

- **Continuous state admission loop (FR-02/07)**: delta in, receipt out, frame
  appended; end-to-end latency < 35 microseconds measured under the Gage-validated
  timer. No absolute-ms fences elsewhere; this CTQ is the one contracted bound.
- **Analytical sieve throughput (FR-06)**: over an mmap'd file, sustained >= 70 GiB/s
  aggregate scan on the bench corpus.
- **Replication via packfile transfer (FR-04)**: two local processes; process A packs,
  transfers the packfile, process B unpacks and resumes; state converges and both
  sides verify.

## Gates Table

Each gate must be witnessed failing on a deliberate mutation (shim-court discipline:
baseline, before, mutation, after, byte-exact restore). A gate that cannot flip is
FAIL — vacuous. Skips are loud (`[SKIP] <reason>`), never silent passes.

| Gate | Command | Fail condition | Non-vacuity proof |
|------|---------|----------------|-------------------|
| layout | `cargo build -p avec-core` | compile-time assert trips | corrupt a size assert; build must fail |
| unit-prop | `cargo test -p avec-core --lib` | any unit/property failure | flip one sentinel encode bit; suite must fail |
| vectors | `cargo test -p avec-blake3 --test vectors` | official-vector mismatch | perturb one digest constant; must fail |
| iso | `cargo test -p avec-core --test arrow_parquet_iso` | isomorphism assertion fails | drop an offset entry in the writer; must fail |
| git-pack | `just gate git-pack` | `git fsck` or `git verify-pack` nonzero | truncate pack tail byte; must fail |
| nif-zero-copy | `mix test test/avec_nif_zero_copy_test.exs` | pointer-identity assert fails | force a copy path in the NIF; must fail |
| ladder-abi | `just gate ladder-abi` | any `jcc` on hot path or stage unhit | introduce one branch; must fail |
| ctq-loop | `just gate ctq-loop` | p99 >= 35us on Gage timer | inject a sleep in the admission path; must fail |
| sieve | `just gate sieve` | < 70 GiB/s sustained | add a per-chunk copy; must fail |
| replicate | `just gate replicate` | state divergence or verify failure | corrupt one frame byte in transit; must fail |

## Machine Report

Single command produces the milestone verdict JSON — the DMADV Verify exit artifact:

```
just verify-report        # writes docs/jira/v26.10.31/verify-report.json
```

Output schema: one entry per CTQ with `{ctq, gate, status: PASS|FAIL|SKIP, exit,
receipt_sha, rung}`, plus `wave_class` per failure (see below) and an overall
`milestone: VERIFIED|BLOCKED`. Exit 1 on any FAIL or any SKIP without a reason.
A missing court file degrades to FAIL, never a silent pass.

## Pre-existing vs Wave-Introduced Failures

House rule: every run report classifies each failure.

- **Pre-existing**: reproduces on the wave-base commit (`git stash`-free check via a
  worktree-free `git show HEAD` checkout into scratch, or CI history on the base SHA).
  Not charged to this wave; still gets a work order.
- **Wave-introduced**: absent at base, present after a wave lane's diff. Blocks
  integration; owning lane fixes within the compile-freeze SLA.
- **Classification evidence**: the report records the base SHA, the failing gate, and
  the first wave commit exhibiting the failure. A failure claimed pre-existing without
  a base-SHA reproduction receipt is classified wave-introduced by default.

## Reality divergence (wave 7)

The implemented gate battery diverges from this document's plan:

- **Implemented battery**: `scripts/verify_v26.10.31.sh` — 10 gates:
  check-workspace, test-avec-arrow, test-avec-parquet --features std,
  test-avec-git, test-avec-blake3, test-integration, test-avec-beam,
  audit-branchless --selftest, cli-verify build-and-run, test-audit.
  These names/commands diverge from the Gates Table above (0/10
  name-match; the `just gate <name>` targets this doc specifies do not
  exist).
- **Machine report**: the actual artifact is `verify-latest.md` (a
  gate/status/exit table), not the `verify-report.json` with this doc's
  `{ctq, gate, status, exit, receipt_sha, rung}` schema.
- **Baseline**: pre-existing vs wave-introduced is decided by error-
  signature diff against `scripts/verify_baseline.txt`, not base-SHA
  reproduction as this doc specifies.
- **SKIP semantics**: skips do not fail the run (contrary to "Exit 1 on
  any FAIL or any SKIP without a reason" above).
- **Standing update**: receipts exist for executed gates
  (gate-workspace-check-v2, gate-cli-machine-report, gate-beam-bridge-v2,
  gate-git-tests-v2, gate-blake3-tests-v2, gate-e2e-integration-v2,
  gate-branchless-selftest-v2) — rung statuses partially backfilled.

## Standing

UNVERIFIED. This document specifies; the wave-7 divergence section above
records what was actually executed. Each gate earns
ALIVE only with a witnessed run (command, exit, receipt) and a witnessed mutation flip.

## See Also

- [29-benchmark-harness-plan.md](29-benchmark-harness-plan.md)
- [30-shacl-validation-suite.md](30-shacl-validation-suite.md)
- [27-branchless-kernel-abi.md](27-branchless-kernel-abi.md)
