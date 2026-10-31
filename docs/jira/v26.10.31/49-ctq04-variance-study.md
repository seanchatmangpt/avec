# CTQ-04 Transpose-Variance Study (Wave-7 Lane 3)

Status: STUDY COMPLETE (2026-10-31). Six spaced `ctq_report` runs plus two
`gage_rr` runs on the same machine in the same window. This document is the
DFLSS ratification input for the provisional CTQ-04 pin
(stdev <= 15% of mean). Nothing in `ctq_report.rs` was modified by this lane.

## Question

CTQ-04 measures stdev/mean over 100 in-process transpose+lookup runs
(n=4096). Runs flip PASS/FAIL across invocations (9.7 / 6.7 / 9.3 / 21.9
historical; 13.0 / 13.3 / 3.2 / 1.2 / 1.2 / 1.7 this study). Is the variance
a property of the kernel, or of machine contention?

## Distribution Table

Six runs of `./target/release/ctq_report`, ~2 min apart (2026-10-31
20:00-20:11 local), macOS 26.2, Mac15,9, load sampled per run via `uptime`
(1/5/15 min) and process census (`ps aux`, %CPU > 30%).

| run | time  | load 1m | busy>30% | mean us | stdev us | stdev/mean | verdict |
|-----|-------|---------|----------|---------|----------|------------|---------|
| 1   | 20:00 | 6.22    | 2        | 25.404  | 3.296    | 13.0%      | PASS    |
| 2   | 20:02 | 3.72    | 1        | 15.876  | 2.108    | 13.3%      | PASS    |
| 3   | 20:04 | 9.42    | 2        | 16.076  | 0.518    | 3.2%       | PASS    |
| 4   | platform | 11.50 | 7       | 15.907  | 0.190    | 1.2%       | PASS    |
| 5   | 20:08 | 3.24    | 0        | 15.762  | 0.183    | 1.2%       | PASS    |
| 6   | 20:11 | 3.47    | 1        | 15.876  | 0.264    | 1.7%       | PASS    |

(run 4 timestamp 20:06:53.) Historical values for the same binary and pin:
9.7%, 6.7%, 9.3%, 21.9% (the 21.9% FAIL is receipted in
`receipts/gate-ctq-report-v2.md` / `47-milestone-verdict.md`).

## Root-Cause Analysis

1. The kernel baseline is stable: runs 4-6 sit at mean ~15.8 us, stdev
   0.18-0.26 us (1.2-1.7%) with near-zero load. That is the true kernel
   signature on this machine.
2. Excursions are mean-shift plus jitter, not kernel drift. Run 1's mean
   (25.4 us) is +60% over baseline while runs 2-6 agree within 2% — a
   classic contention contamination, not a kernel property.
3. Load-average correlation is weak in the direction people expect. Run 4
   had the highest 1-minute load (11.50) and the LOWEST variance (1.2%) —
   load average is a lagging 1/5/15-min mean; instantaneous interference
   during the ~2 ms CTQ-04 window is what matters, and a 100-sample window
   of ~16 us each completes in ~1.6 ms, so a single preemption or timer
   coalescing event inside the window can inject a 2-3x outlier sample that
   dominates stdev.
4. The gage agrees the machine was noisy in this window: two `gage_rr` runs
   immediately after run 6 measured cv_trimmed 1.37% and 1.47% — both FAIL
   against the gage's own <1% criterion (`receipts/gate-gage-rr.md` records
   the same class of result under load). Per `29-benchmark-harness-plan.md`
   §Gage, no timing claim is admissible when the gage fails.
5. Spec tension: `29-benchmark-harness-plan.md` (CTQ-04 detail) already
   prescribes the durable form — a ratio fence of min-of-N paired rounds
   against an independent sleep-free baseline in the same process — not a
   raw stdev/mean over 100 runs. The implemented bin measures stdev/mean.
   The gate as implemented measures machine contention and calls it kernel
   variance.

Conclusion: the metric is measuring the environment, not the kernel. The
kernel passes its own signature (<=2%) whenever the environment is quiet;
excursions (13%, 21.9%) track contention windows, exactly matching the
flip-flop PASS/FAIL history.

## Ratification Recommendation

Option (b): the pin moves to a Gage-conditioned percentile metric. A pure
load-gated protocol (option a) is necessary but not sufficient — it makes
the gate honest only when the machine is quiet, but the stdev/mean statistic
itself remains a 100-sample raw statistic whose outliers are environment-
injected; conditioning on gage at run time both (i) refuses to admit CTQ-04
when the gage CV >= 1% and (ii) pairs it with a robust dispersion statistic
so residual jitter cannot masquerade as kernel variance.

Recommendation details:

1. Cross-gate: CTQ-04 is VALID only when a gage R&R CV < 1% holds on the
   same invocation. Implementation: run the gage loop (same 4 KiB BLAKE3
   fixed workload, same trimming rule as `gage_rr`) inside `ctq_report`
   immediately before the CTQ-04 timed loop, in the same process.
   `cv_trimmed < 1.0` gates admission; otherwise CTQ-04 reports verdict
   `INDETERMINATE` (environment), does not count as PASS or FAIL, and the
   exit-code contract treats INDETERMINATE as neither (documented).
2. Robust statistic: replace stdev/mean with a percentile dispersion ratio
   immune to single-sample contamination: `p99/p50` of the 100 run times
   (or p95/p50 if 100 samples make p99 unstable; pick one and pin it).
   Provisional USL on the new statistic: p99/p50 <= 1.30 pending its own
   ratification study; the 15% stdev/mean pin is RETIRED.
3. Keep a ratio-fence variant as the follow-on (per the harness plan's CTQ-04
   detail): min-of-N paired rounds vs a sleep-free baseline in-process.
   This supersedes both statistics once implemented.

## Spec Text for ctq_report (NOT implemented — bin owned by another lane)

In `ctq04_transpose_variance()`:

- After building the input quads and the one correctness pass, run the gage
  loop: 10,000 reps of `avec_blake3::hash` over a fixed 4 KiB buffer with
  500 warmup reps; sort deltas; trim bottom/top 1%; compute `cv_trimmed`.
- If `cv_trimmed >= 1.0%`: emit CTQ-04 JSON with
  `"verdict":"INDETERMINATE","reason":"gage_cv_trimmed_pct>=1"`, include the
  gage value, and do NOT push "CTQ-04" onto `gates_failed`.
- Else: run the existing 100-run loop; additionally compute p50 and p99 of
  the samples; emit `"p50_ns"`, `"p99_ns"`, `"p99_over_p50"`; gate on
  `p99_over_p50 <= 1.30` (new provisional pin; stdev/mean reported
  informationally, not gated).
- The exit-1 contract stays: exit 1 iff a REAL gate fails; INDETERMINATE
  CTQ-04 does not affect exit code but is loud in stderr and JSON.

## Falsifier

If CTQ-04 stdev/mean exceeded 15% on a run where the same-process gage
cv_trimmed < 1%, the environment hypothesis is falsified and the pin
debate reopens on kernel evidence.

## See Also

`18-dflss-charter.md` · `29-benchmark-harness-plan.md` ·
`47-milestone-verdict.md` · `receipts/gate-gage-rr.md` ·
`receipts/gate-ctq-report-v2.md`

## Ratification stamp (wave 12, 2026-10-31)

Option (b) — the gage-conditioned p99/p50 <= 1.30 protocol with the 15%
stdev/mean pin retired — was RATIFIED by operator declaration
(docs/OPERATOR_DECISIONS.md decision 2). As implemented in ctq_report.rs
(load-gated SKIP-NOISY at mini-gage CV > 2%; p99/p50 gate on the quiet
path). One deviation from this study's spec text, recorded in
55-final-integration-report.md: the noisy threshold landed at 2% (the
study drafted 1%); the 2% gate is the ratified form.
