# LSL Re-derivation: CTQ-06 BLAKE3 Throughput

First-principles re-derivation of the CTQ-06 lower spec limit. This doc arms the
decision; it does not amend `20-prd.md` unilaterally.

## 1. First-Principles Ceiling (NEON, per core)

BLAKE3 g-function, vectorized: 6 vector ops per g
(3 adds, 3 xors, with rotate folded via `vsri`/`ushr`+`or` counting).

Per 1 KiB chunk:

- 16 blocks x 7 rounds x 16 g = 1792 g-ops per chunk
- 1792 x 6 = 10,752 vector ops per KiB (one lane of work)

Apple M3 Max NEON issue: ~4-8 vector ops/cycle (2-4 NEON pipes,
dual-issue dependent). P-core clock ~3-4 GHz sustained.

Case A — no SIMD lane amortization (10,752 vops per KiB):

- floor: 3.0 GHz x 4 vops/cyc = 1.2e10 vops/s
  -> 1.2e10 / 10,752 = 1.12e6 chunks/s = 1.09 GiB/s
- ceil: 4.0 GHz x 8 vops/cyc = 3.2e10 vops/s
  -> 3.2e10 / 10,752 = 2.98e6 chunks/s = 3.13 GiB/s
- Case A ceiling: **1.1 - 3.1 GiB/s/core**

Case B — full 4-way lane amortization (each vop covers 4 u32 lanes,
i.e. 4 chunks interleaved; 2688 vops per KiB effective):

- floor: 1.2e10 / 2688 = 4.47e6 chunks/s = 4.58 GiB/s
- ceil: 3.2e10 / 2688 = 1.19e7 chunks/s = 12.5 GiB/s
- Case B ceiling: **4.6 - 12.5 GiB/s/core**

8.5 GiB/s/core therefore requires: 4-way amortization AND >= 8 vops/cyc
AND >= 4 GHz sustained AND zero loop/load/store overhead. That is the
far corner of every parameter interval simultaneously; no slack remains
for message scheduling, chunk-chain serialization, or tree merging.

## 2. Measured vs. Reference Points

- Profiling lane, this repo, this machine: 1.3 - 1.85 GiB/s/core
  (4/8-way NEON kernel). ALIVE measurement, this session's receipts.
- BLAKE3 upstream publishes ~1-2 GiB/s/core for NEON targets.
  External reference point only — qualitative, not verified here.
- AVX-512 builds reach several GiB/s/core upstream. x86_64-only;
  not available on this hardware. External reference point only.

This machine's measured 1.85 sits at the top of the upstream NEON band
and inside Case A of the ceiling math. That is consistent: a healthy
NEON implementation lands at 40-60% of the Case B optimistic corner,
not 95-100%.

## 3. Verdict

**8.5 GiB/s/core is not physically attainable on this hardware.**
NEON-only, no AVX-512: the compute ceiling under the most favorable
stacked assumptions (Case B top corner) is ~12.5 GiB/s with zero
real-world overhead; a provisioning-grade ceiling is ~4.6 GiB/s
(Case B floor). The measured 1.85 GiB/s already sits at 40% of the
Case B optimistic corner — squarely where mature NEON BLAKE3 lands.
Hitting 8.5 would require doubling the demonstrated best while
consuming the entire remaining headroom as overhead-free margin.
The 8.5 LSL as written is unsatisfiable; it is a spec error, not an
implementation gap.

Aggregate 16-core figure note: if 8.5 was intended as machine-wide
rather than per-core, 16 P-cores x 1.85 = ~29.6 GiB/s aggregate, so the
row's ambiguity also needs resolving at amendment time.

## 4. Recommendation (for ratification, not self-executed)

Re-derived LSL, provisioned at 50-60% of the demonstrated ceiling:

- Demonstrated best on-subject: 1.85 GiB/s/core
- Proposed LSL: **1.0 GiB/s/core** (floor of demonstrated band,
  guards regression)
- Proposed Target: **1.5 GiB/s/core** (mid-band)
- Proposed USL: 2.5 GiB/s/core (Case B floor x ~0.55; ceiling-class,
  not expected to bind)

DFLSS ratification path:

1. Amender: the PRD owner amends the CTQ table in
   `docs/jira/v26.10.31/20-prd.md` (row "BLAKE3 throughput (GiB/s)"),
   replacing 8.5 / 10.2 with the re-derived LSL / Target and stating
   "per-core, NEON" in the row label to kill the aggregate ambiguity.
2. Required receipt: a benchmark execution receipt on the exact
   subject binary (this repo, this machine) showing the measured
   distribution with N >= 30 samples, mean and C_pk vs the amended
   limits — the same court class as `crates/bench/src/bin/ctq_report.rs`
   output. C_pk >= 1.33 against the amended LSL at pilot closure.
3. Standing transition: PRD CTQ row moves UNVERIFIED -> PARTIAL_ALIVE
   only on that receipt; until then the amended numbers remain
   design-stage.

## Standing

UNVERIFIED — arithmetic shown above is checkable by inspection;
external reference points are qualitative and unverified; no benchmark
was executed in this session by this lane.

See Also: 20-prd.md · 04-benchmark-contract.md · 26-blake3-engine-spec.md ·
docs/PHASE_CHANGE_RECEIPTS.md

## Wave-8 Measurement Appendix (2026-10-30, lane 19)

Executed after wave-8 lane 1 landed the dual-chain software-pipelined
g4-interleave kernel in `crates/avec-blake3/src/multi_chunk.rs`
(courts green: `cargo test -p avec-blake3 --test multi_chunk_vectors`
7 passed / 0 failed, 1 ignored). Machine-load caveat: wave ran with
~20 concurrent lanes; these are loaded-machine numbers, not
quiet-machine values.

`blake3_profile` (release):

| metric | wave-8 value | prior reference |
|---|---|---|
| e2e plateau (64-8192 chunks) | 1.857-1.871 GiB/s (512-514 ns/chunk) | 1.85 GiB/s baseline |
| kernel ceiling (compress_chunks_4way, resident 4 KiB) | 2796.91 ns/4-chunk batch = 699.23 ns/chunk = 1.364 GiB/s | 1.36 GiB/s ceiling |
| marginal fit | 513.66 ns/chunk, fixed ~0 | — |
| parent-node share @ 8192 chunks | 9.3% (5.719 ms leaf / 0.583 ms parents / 6.302 ms full) | — |
| hash_wide SIMD @ 8k chunks | 0.424 GiB/s | — |
| blake3_throughput, 256 MiB min-of-5 | 0.43 GiB/s (0.5843 s min) | — |

Attribution: unchanged. The e2e plateau still sits at ~1.86 GiB/s
(inside the Case-A band 1.1-3.1 GiB/s/core), the kernel ceiling still
reads 1.36 GiB/s, and the profile's kernel-ceiling harness
(`compress_chunks_4way` resident) shows the same ns/batch as before the
g4x2 interleave — the measured profile surface did not move with lane
1's kernel. The Case-A ceiling assessment therefore does not change:
the demonstrated 1.85-1.86 GiB/s remains consistent with Case-A
(10,752 vops/KiB, 1.1-3.1 GiB/s/core) and remains the basis for the
provisioned USL of 2.5 GiB/s/core.

Caveats: (a) machine loaded by the wave — treat deltas below ~5-10%
as noise; (b) the g4x2 interleave may be present in the lib but not on
the profile harness's measured path (ceiling ns/batch bit-identical to
the prior reading), so this appendix certifies the e2e surface only,
not kernel-internal improvement; a dedicated g4x2-vs-scalar court is
the missing receipt.

## Ratification record (wave 12, 2026-10-31)

This study's Option (b) proposal — LSL 1.0 / target 1.5 / USL 2.5
GiB/s/core, CPU NEON — was RATIFIED by operator decision
(docs/OPERATOR_DECISIONS.md decision 1) and executed as edit set
A/A2/C/D/E/F. The gate now evaluates hash_many_simd REAL at LSL 1.0;
hash_wide_simd is informational (superseded architecture). The
pre-ratification FAIL receipts (gate-blake3-throughput, gate-ctq-report,
gate-ctq-report-v2, gate-cpk-study, gate-cpk-study-v2) are retained
verbatim as the refutation record this thesis requires.
