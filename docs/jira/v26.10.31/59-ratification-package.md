# CTQ-06 + CTQ-04 Ratification Decision Package

Status: ASSEMBLED for operator decision (wave-9 lane 7, 2026-10-31).
Nothing in this package is flipped, ratified, or executed. `LSL_RATIFIED`
still reads `None` on disk at assemble time; the `20-prd.md` CTQ table is
untouched. This document assembles the decision; the operator executes it.

## 1. The Decision Being Requested

Four operator-only actions, all in one decision:

1. Flip `LSL_RATIFIED` from `None` to `Some(1.0)` in
   `crates/bench/src/bin/ctq_report.rs:44`. The const's own doc-comment
   (lines 38-43) names it operator-owned: "Do not flip from a lane; this is
   operator-owned."
2. Amend the `20-prd.md` CTQ-06 row (line 73):
   `8.5 / 10.2 / Memory-controller max` -> `1.0 / 1.5 / 2.5`, with the row
   label changed to `BLAKE3 throughput (GiB/s/core, NEON)` per the PROPOSED
   amendment already filed in `20-prd.md` §Amendment (lines 96-131).
3. Move that amendment section's `Status: PROPOSED` to `RATIFIED`, citing
   the receipts from the ladder in §5.
4. Ratify the CTQ-04 p99/p50 protocol: the pin at
   `crates/bench/src/bin/ctq_report.rs:211` (`USL_P99_OVER_P50 = 1.30`)
   moves PROVISIONAL -> RATIFIED, and the 15% stdev/mean pin stays retired
   (provenance via the `retired_pin` JSON field, `ctq_report.rs:212`).

The flip is operator-only. No lane may perform it.

## 2. The Engineering Evidence For

### 2.1 Ceiling math (48-lsl-rederivation.md §1, checkable by inspection)

- Case A — no SIMD lane amortization (10,752 vops/KiB):
  **1.1 - 3.1 GiB/s/core** (3.0 GHz x 4 vops/cyc floor to
  4.0 GHz x 8 vops/cyc ceiling).
- Case B — full 4-way lane amortization (2,688 vops/KiB):
  **4.6 - 12.5 GiB/s/core**.
- 8.5 requires "4-way amortization AND >= 8 vops/cyc AND >= 4 GHz sustained
  AND zero loop/load/store overhead. That is the far corner of every
  parameter interval simultaneously" (48 §1, verbatim). No slack remains
  for message scheduling, chunk-chain serialization, or tree merging.
- 48 §3 verdict, verbatim: "The 8.5 LSL as written is unsatisfiable; it is
  a spec error, not an implementation gap."

### 2.2 Measured trajectory (each row receipted)

Per ADR-15 wave-6 amendment (22-adr-register.md) and wave receipts:

| wave | measurement | GiB/s |
|---|---|---|
| 3 | baseline NEON leaf hash | 0.43 |
| 5 | 4-way kernel | 0.938 |
| 5 | `hash_many_simd` (ctq_report row) | 1.18 |
| 7/8 | `hash_many_simd`, 256 MiB min-of-5 | 1.85 |

Stability at the top: gate-cpk-study-v2
(receipts/gate-cpk-study-v2.md, exit 1 only because the LSL under test was
the unratified 8.5) records n=100, mean 1.846241, sigma_overall 0.010571 —
CV 0.57%. The process is stable and tight; the failure is the spec limit,
not the process.

### 2.3 Wave-8 assembly evidence (why no kernel lever closes the gap)

- g4x2 interleave REFUTED: 48 §Wave-8 appendix records the
  `compress_chunks_4way` resident-kernel ceiling at bit-identical ns/batch
  before and after the g4x2 interleave kernel landed — the 4-way kernel was
  already overlapping its latency chains; doubling width adds only register
  pressure. On-disk record: 00-index.md:231-232 "kernel g4x2 interleave
  REFUTED with assembly evidence (spill-bound - reverted; state-spill is
  the real ceiling)".
- State-spill arithmetic: an 8-wide candidate holds 16 state + 16 message
  v128 vectors — 32 registers for state+message alone, leaving 0 of the
  ARM64 NEON register file's 32 registers for round constants and
  temporaries. The candidate's prologue shows `sub sp, #1552` — a 1,552-byte
  stack frame, i.e. spill-bound by construction. (The `#1552` figure is
  wave-9 dispatch evidence from the disassembly lane; the on-disk
  corroboration is the 00-index refutation line above.)
- Consistency check: ADR-15 wave-7 amendment (22-adr-register.md:517-525)
  found the 4-way round loop at 0 spill-relative loads. The register file
  holds at 4-way and cannot hold at 8-wide. The two findings agree: the
  real ceiling at 4-way is the serial g4 chain under shared NEON ALU
  throughput, and width is not a lawful lever past it.

### 2.4 Position of the measurement against the proposed limits

- 1.85 clears the proposed LSL 1.0 by +85%.
- 1.85 sits at the 1.5 target band edge (1.23x target) — above target,
  below the ceiling-class USL 2.5 (which is Case-B-floor-derived and not
  expected to bind).
- Predicted capability after flip, from cpk-study-v2's own numbers:
  C_pl = (1.846241 - 1.0) / (3 x 0.010571) = **26.7** vs the required
  C_pk >= 1.33 (48 §4) and 1.67 target. Not a prediction of a new
  measurement — arithmetic on a receipted distribution.

## 3. The Counter-Case (stated fairly)

- **8.5 was the design-conversation aspiration.** 20-prd §Amendment says so
  itself: "set from the design conversation before the leaf-kernel
  architecture existed." But it is recorded in a normative CTQ table, and
  ratifying replaces a normative number — that is precisely why the flip is
  operator-only and receipt-gated, and why the counter-case is entitled to
  a hearing: a ratified 1.0 forecloses pushing the CPU kernel further.
- **The GPU path is unexplored and could change the ceiling.**
  `crates/avec-gpu` exists (AVEC-111, Metal unified-memory bridge,
  status CLOSED — but CLOSED covers the bridge, not a CTQ-grade throughput
  receipt on it). `avec-blake3` carries the dispatch hook
  (`GPU_CROSSOVER_BYTES = 16 MiB`, `should_dispatch_gpu`,
  lib.rs:33,38), and no ctq_report row measures the GPU path today — the
  two rows are `hash_wide_simd` and `hash_many_simd`, both CPU. A Metal
  SIMT pass over >= 16 MiB inputs plausibly exceeds any NEON ceiling; the
  Case-A/B math is a CPU constraint.
- **Consequent scoping requirement**: the ratified LSL must be scoped as
  "LSL for the CPU/NEON kernel path" (the row-label fix `GiB/s/core, NEON`
  in §6 edit E does this), and the GPU path must be carried as a separate
  future CTQ row (e.g. "BLAKE3 throughput, Metal path (GiB/s/core)") — not
  silently abandoned. Ratifying 1.0 for NEON is a decision about the CPU
  kernel; it must not read as a decision that 1.85 is fast enough forever.

## 4. CTQ-04: p99/p50 Protocol Implemented, Pin Retirement

- Root cause (49-ctq04-variance-study.md): the old stdev/mean-over-100-runs
  statistic measured machine contention, not kernel variance — excursions
  (13.0%, 21.9%) tracked load windows while quiet-machine runs sat at
  1.2-1.7%; the gage agreed the machine was noisy (cv_trimmed 1.37%,
  1.47%).
- Implemented per 49 Option b (`ctq_report.rs:210-317`): inline mini-gage
  (500 reps, trimmed CV <= 2%) gates admission; SKIP-NOISY when noisy
  (`ctq_report.rs:264-289`); when admitted, gate statistic is p99/p50 of
  the 100 runs, USL 1.30 (`ctq_report.rs:211`); stdev/mean reported
  informationally; `RETIRED_PIN_STDEV_FRAC = 0.15` kept at
  `ctq_report.rs:212` and emitted as `retired_pin` provenance in both JSON
  branches (`ctq_report.rs:273,314`).
- Results: PASS at p99/p50 1.134 (receipted, 58-final-verdict-v2 §1/§3,
  mini-gage CV 0.64% admitted) and 1.134-1.176 across wave-9 runs (the
  1.176 upper value is wave-9 dispatch observation; only 1.134 is
  receipted on disk at assemble time). cpk_study corroborates:
  Cpu(overall) 2.31 >= 1.67 on a quiet machine (58 §1).
- 29-benchmark-harness-plan.md §"CTQ-04 superseded" already records 49's
  protocol as the live metric and its own ratio-fence text as historical.
  The residual is exactly the PROVISIONAL -> RATIFIED status move.

## 5. Ratification Receipt Ladder

Order is load-bearing: flip the gate, then run the ladder, then move the
docs. All runs on the exact subject binary, quiet machine (the gage
precondition is what makes "quiet" a check, not a hope).

1. **Precondition — gage.** `cargo build --release -p bench`, then run
   `target/release/gage_rr`; cv_trimmed < 1% required (49 rec 1;
   receipts/gate-gage-rr.md is the receipt class). Mint the receipt.
2. **Flip.** Apply edit A (§6). Rebuild `-p bench`.
3. **3 consecutive `ctq_report` PASS runs** (20-prd §Amendment receipt 3).
   Each run mints `gate-ctq-report-vN.md`; verdict PASS, exit 0, and the
   JSON carries `"amendment":"AMENDMENT-RATIFIED"` on the CTQ-06 object.
   The mini-gage must admit each run (verdict PASS, not SKIP-NOISY).
4. **1 `cpk_study` run** (20-prd §Amendment receipt 1): N >= 30 samples
   (the bin runs n=100), C_pk >= 1.33 vs the amended LSL. After edit C the
   C_pl evaluates against 1.0; predicted ~26.7 (§2.4).
5. **Then, and only then, the doc moves**: 20-prd amendment PROPOSED ->
   RATIFIED (edit F) + CTQ table row (edit E). Standing transition of the
   CTQ-06 row per 48 §4: UNVERIFIED -> PARTIAL_ALIVE on the receipt set.

If any ladder run fails, the amendment stays PROPOSED, the flip is
reverted to `None`, and the failure is receipted — same no-silent-
relaxation law that has governed this gate since wave 7.

## 6. The Exact Edits (mechanical list)

Code (all in `crates/bench/src/bin/`):

- **A. `ctq_report.rs:44`** — the flip:
  `const LSL_RATIFIED: Option<f64> = None;`
  -> `const LSL_RATIFIED: Option<f64> = Some(1.0);`
- **A2. `ctq_report.rs` row scoping — OPERATOR DECISION POINT.** The flip
  alone does not produce a PASS gate: `all_pass` requires BOTH rows to
  clear the LSL, and the legacy `hash_wide_simd` row measures 0.43
  GiB/s (receipted, gate-blake3-throughput + 58 §3) — it fails any LSL
  above 0.43. Post-flip outcomes:
  - Option 1 (recommended): gate the production row (`hash_many_simd`)
    against `LSL_RATIFIED`; demote `hash_wide_simd` to informational
    (`"gate":"informational","verdict":"REPORT"`), mirroring the CTQ-03
    row's pattern. The production path is what CTQ-05 rides
    (ctq_report.rs ctq05 candidates; fastest-correct selection).
  - Option 2: keep both rows gated. CTQ-06 stays FAIL (exit 1) at
    hash_wide_simd forever; the flip changes the gate's meaning but not
    the observable exit code, and no PASS receipt is obtainable.
- **B. `ctq_report.rs:273,314`** — pin status strings: replace
  `PROVISIONAL p99/p50 <= 1.30 (49-ctq04-variance-study.md Option b)`
  with `RATIFIED p99/p50 <= 1.30 (49-ctq04-variance-study.md Option b,
  ratified <date> by operator)` in both `pin_status` fields (2
  occurrences).
- **C. `cpk_study.rs:300`** — `const LSL_GIB_S: f64 = 8.5;` -> `1.0`;
  also the log string at `:297` and the `emit(...Some(8.5)...)` literal at
  `:338`. Without C, the ladder's step-4 cpk receipt cannot pass — the
  study would keep evaluating C_pl against 8.5 and fail at ~-697.
- **D. `blake3_throughput.rs:10`** — `const LSL_GIB_S: f64 = 8.5;` ->
  `1.0` (plus its doc-comment line 4). The `--gate` surface must ride the
  same amendment or the repo carries two contradictory CTQ-06 gates.

Docs:

- **E. `20-prd.md:73`** —
  `| BLAKE3 throughput (GiB/s)    | 8.5             | 10.2            | Memory-controller max |`
  ->
  `| BLAKE3 throughput (GiB/s/core, NEON) | 1.0 | 1.5 | 2.5 |`
  (row-label fix per the amendment's own text; USL moves from
  "Memory-controller max" to the ceiling-class 2.5).
- **F. `20-prd.md:98`** — `Status: PROPOSED — armed for DFLSS
  ratification, NOT decided.` -> `Status: RATIFIED <date> by operator on
  the §5 receipt ladder (gate-ctq-report-v3/v4/v5, gate-cpk-study-v3,
  gate-gage-rr).`
- **G. `49-ctq04-variance-study.md`** — header stamp under Status:
  `Protocol RATIFIED <date> by operator (p99/p50 <= 1.30; 15% pin retired
  with provenance).`
- **H. `29-benchmark-harness-plan.md` §"CTQ-04 superseded"** — append:
  `Protocol ratified <date>; 49 remains the live metric source.`
- **Not in scope (flagged)**: the FR-06 row at `20-prd.md:59` still reads
  "Over 10 GiB/s per core, spec-compliant". Same spec-error family, but it
  is a functional-requirement line, not the CTQ table — a separate
  follow-up amendment, not bundled into this one.

## 7. What Ratification Does NOT Change

- **Correctness surfaces are untouched**: official BLAKE3 vectors (6/6 in
  the wave-8 battery, 58 §1), b3sum conformance tests
  (official_vectors.rs / small_batch_vectors.rs / multi_chunk_vectors.rs
  all assert b3sum digests), the digest-equality screens inside CTQ-05/06.
  A throughput limit says nothing about digests.
- **CTQ-02 zero-alloc is independent**: the counting-allocator gate
  (0 allocation events over a 64 MiB pass) does not read the LSL.
- **CTQ-05 is independent**: the 35 us USL does not read the LSL; its
  candidate selection is correctness-first.
- **The gate stays REAL either way.** Ratification changes the limit the
  REAL gate evaluates, not the gate's class. The wave-7 "no silent
  relaxation" law (20-prd §Amendment) is honored by construction: the
  gate FAILs honestly at 8.5 until the operator flips it.
- **The p99/p50 pin stays provisional-class in one respect**: 49 rec 3's
  ratio-fence successor (min-of-N paired rounds vs a sleep-free in-process
  baseline, 29 §CTQ-04 primary) remains the designated future
  supersession. Ratifying p99/p50 now does not close that follow-on.
- **The GPU question stays open** (§3): ratification scopes the NEON LSL;
  it does not adjudicate the Metal path.

## Standing

- This lane ran: no gates, no builds, no flips. Read-only assembly plus
  the greps cited above; exit 0 on every command shown.
- On-disk receipted claims: trajectory rows, cpk-study-v2 distribution,
  1.134 CTQ-04 PASS, assembly refutation record (00-index:231-232), all
  line numbers cited in §6.
- Dispatch-reported, not yet receipted on disk: the 1.176 CTQ-04 upper
  value, the `sub sp, #1552` disassembly figure. Both are named as such
  where used; the §5 ladder re-derives the load-bearing ones.
- UNVERIFIED (default): whether the operator ratifies. Nothing here
  changes any gate, table, or constant.

## See Also

- 20-prd.md · 48-lsl-rederivation.md · 49-ctq04-variance-study.md
- 22-adr-register.md (ADR-15 amendments) · 58-final-verdict-v2.md
- 00-index.md · 29-benchmark-harness-plan.md
- receipts/gate-cpk-study-v2.md · receipts/gate-ctq-report-v2.md
- receipts/gate-gage-rr.md · receipts/gate-blake3-throughput.md
- ../v26.10.29/AVEC-111-metal-gpu-and-blake3.md
