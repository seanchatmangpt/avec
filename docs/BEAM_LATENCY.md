# BEAM_LATENCY

Measurement receipt for the avec-beam NIF membrane latency and the M4 CTQ
(scheduler impact <= 50 us), lane OPT-L8, 2026-10-29.

## CTQ and register note

CTQ under test: **scheduler impact <= 50 us** for the avec-beam membrane under
heavy NIF churn. Register note: the repo charter (`docs/DFLSS_CHARTER.md`,
v26.10.29) carries no such row — its register is CTQ-1..CTQ-5 (and M-04 is the
CC=1 branch-audit invariant, not a scheduler gate). The CTQ is defined by the
OPT-L8 work order and measured here as specified: a 1 ms timer-sensitive
process (`receive after 1`) sampled under idle and under saturating
`avec_new`/`avec_free` churn.

## Method

1. `avec_get/2` round-trip latency: 10000 samples against a 1024-row i32
   batch, each sample bracketed by two `erlang:monotonic_time/0` reads.
   A no-NIF calibration row (monotonic-time pair only) measures the
   bracketing overhead so the NIF cost is legible.
2. `avec_new(16384, 0)` wall time, 500 iterations, free untimed. 16384 rows is
   the dirty-CPU flag threshold in `NIF_FUNCS` (`crates/avec-beam/src/lib.rs`).
3. Scheduler impact: a timer process loops `receive after 1` and records per
   iteration the overshoot beyond the requested 1 ms ("timer drift").
   Churn: one `avec_new(262144, 0)` + `avec_free` loop per dirty CPU
   scheduler (16 churners). Identical sensor, two 3000 ms phases: idle, then
   loaded. Both maxima are reported; the CTQ is gated on the differential
   impact (loaded max - idle max), and the absolute loaded max vs the gate is
   printed beside it as a second verdict. Impact-gate FAIL exits 1.

## Environment

- OTP 28, ERTS 16.2, macOS arm64 (Darwin 25.2.0), 16 logical CPUs,
  16 normal schedulers, 16 dirty CPU schedulers, 10 dirty IO.
- NIF: debug build (`target/debug/libavec_beam.dylib`,
  `avec_version = <<"26.10.29">>`).
- Run class: single run, machine-local. Host load average at run time:
  **14.14** (1 min) on 16 logical CPUs — concurrent lanes were running
  CPU-heavy benchmark work on the same host before and during this run.

## Results (verbatim, receipt run)

```text
nif loaded: /Users/sac/avec/target/debug/libavec_beam.so
[env] erts=16.2 otp_release=28
[env] logical_cpus=16 schedulers_online=16 dirty_cpu_schedulers=16 dirty_io_schedulers=10
[env] nif_build=debug dylib=target/debug/libavec_beam.dylib run_class=single-run machine-local
[env] host_load: 18:41  up 3 days,  2:11, 6 users, load averages: 14.14 13.20 13.88
ok   sanity: ramp + stats + i64 path verified; avec_version=<<"26.10.29">>
[get-latency]      n=10000 min_us=0.083 median_us=0.084 p99_us=0.167 max_us=39.417  | avec_get/2 round-trip us (incl. 2x monotonic_time)
[get-calibration]  n=10000 min_us=0.000 median_us=0.042 p99_us=0.042 max_us=43.292  | monotonic_time pair only, no NIF (subtractable overhead)
[new-dirty]        n=500 min_us=686.541 median_us=742.333 p99_us=1513.208 max_us=2240.041  | avec_new(16384,0) wall us, dirty CPU, n=500 (free untimed)
[timer-idle]   window_ms=3000 churners=0 rows_per_new=262144 n=1500 min_us=561.083 median_us=999.917 p99_us=1207.084 max_us=3142.417  | receive-after-1ms overshoot
[timer-loaded] window_ms=3000 churners=16 rows_per_new=262144 n=1220 min_us=105.791 median_us=1038.875 p99_us=9057.833 max_us=11128.208  | receive-after-1ms overshoot
[CTQ] max timer drift under load = 11128.208 us | idle baseline max = 3142.417 us | scheduler impact (loaded - idle) = 7985.791 us | gate <= 50.0 us => impact FAIL, absolute-loaded-max FAIL
```

Harness: `escript test/beam_latency.escript` — exit code 1 (impact gate FAIL).

## CTQ verdict

**FAIL** — receipt run, both readings: differential impact 7985.791 us and
absolute loaded max 11128.208 us against the 50 us gate. No threshold was
adjusted.

### Run-to-run stability (disclosed)

A second full harness run minutes later on the same host (same load class):

```text
[timer-idle]   window_ms=3000 churners=0 rows_per_new=262144 n=1473 min_us=139.042 median_us=1000.375 p99_us=2611.625 max_us=10829.042  | receive-after-1ms overshoot
[timer-loaded] window_ms=3000 churners=16 rows_per_new=262144 n=1153 min_us=27.459 median_us=1159.208 p99_us=6660.541 max_us=9122.084  | receive-after-1ms overshoot
[CTQ] max timer drift under load = 9122.084 us | idle baseline max = 10829.042 us | scheduler impact (loaded - idle) = -1706.958 us | gate <= 50.0 us => impact PASS, absolute-loaded-max FAIL
```

Run 2 exit 0 (impact gate passed on a negative differential); run 1 exit 1.
The max-timer-drift statistic swings 3.1-11.1 ms across runs and the
differential flips sign — at this host load the differential is
noise-bound and the harness exit code is noise-determined. The one stable
finding across both runs: the idle timer floor alone (3.1 ms / 10.8 ms max)
exceeds the 50 us gate by 60x-220x before any NIF churn, so this
instrument on this host cannot resolve the CTQ either way. The absolute
gate reads FAIL in both runs.

Attribution (diagnostic single runs, same sensor and host, loadavg 12-14):

| phase | churners | rows | median_us | p99_us | max_us |
|---|---|---|---|---|---|
| idle | 0 | 0 | 999.917 | 1878.250 | 11182.459 |
| churn1 | 1 | 16384 | 976.209 | 1747.250 | 6231.709 |
| churn1 | 1 | 262144 | 999.958 | 3015.125 | 10688.292 |
| churn4 | 4 | 262144 | 251.625 (min) | 4202.875 | 11011.250 |

Reading: one 262144-row churner issues ~11 ms dirty NIF calls continuously,
yet the timer distribution stays in the same band as idle (max 10.7 ms vs
idle's 11.2 ms in the same probe; medians pinned at the ~1.0 ms floor). The
dirty-CPU flag is honored — dirty work does not migrate onto normal
schedulers. The multi-ms drift tails are ambient host noise present at idle
(loadavg 12-14 from concurrent lanes); they are not churn-proportional, and
the run-to-run sign flip above shows the receipt-run differential is
noise-dominated rather than NIF-caused.

Floor disclosure: on this host, this sensor, this session, the idle 1 ms
timer floor alone is ~+1.0 ms median and 3.1-11.2 ms max — 60x-220x the
50 us gate before any NIF churn. The instrument cannot resolve a 50 us
scheduler-impact gate under this load; the FAIL verdict is the honest
reading of the mandated instrument, and the attribution table separates
NIF-caused drift (none observed at 1-churner scale) from ambient noise.

## Other measurements

- `avec_get/2` median 0.084 us, p99 0.167 us; the monotonic-time bracketing
  floor is 0.042 us median, so the NIF round-trip itself is ~0.04 us class.
  Single caller — the registry mutex (`crates/avec-beam/src/lib.rs`,
  `lock_registry/0`) is uncontended in this row; contended behavior is not
  measured here.
- `avec_new(16384, 0)` median 742.333 us on the dirty path in the debug
  build. This is a debug-build number; a release build is not measured in
  this receipt.

## Reproduce

```sh
escript test/beam_latency.escript   # exit 0 on impact PASS, 1 on FAIL
```

Numbers are machine-local and single-run; concurrent host load is printed by
the harness (`[env] host_load:` line) and must be read together with them.

## Sub-us sensor and scheduler-isolation probe (lane BEAM-L7, 2026-10-29)

The OPT-L8 receipt above failed the <=50 us CTQ on a 1 ms-timer sensor whose
idle floor alone is 60x-220x the gate. Lane BEAM-L7 adds the sensor class the
BEAM itself supports and states plainly what that class can and cannot prove
about the CTQ. Harness: `test/beam_latency2.escript` (new; the escript module
name must be `avec_beam` because the NIF registers under the module name baked
in at build time). The OPT-L8 sections above are unchanged and their FAIL
verdict stands as the reading of the mandated 1 ms-timer instrument.

### Method

1. `avec_get/2` round-trip on the RELEASE dylib
   (`target/release/libavec_beam.dylib`, sha256
   `2a45b21dbfaaeb9001e314019fb603d3f038e3cfcc0b0f7c7c1bf47f860ebd22`,
   version law `<<"26.10.29">>` verified in-run): 10000 samples, each
   bracketed by explicit-unit `erlang:monotonic_time(nanosecond)` reads
   (native unit receipted at 1.0 ns), plus a no-NIF calibration row so the
   bracketing overhead is subtractable.
2. Scheduler isolation, benchmark-to-degradation: a pure-Erlang
   tail-recursive integer burst, calibrated per run into [8, 14] ms and then
   FROZEN for all phases (run 1: 4940320 iterations / 9.468 ms measured;
   run 2: 4581901 / 10.040 ms), timed burst-by-burst across five 2.5 s
   windows: idle-A, churn-1, churn-4, churn-16, idle-B. Churn = continuous
   `avec_new(262144, 0)`/`avec_free` loops; 262144 >= 16384 rows is the
   dirty-CPU flag threshold (`crates/avec-beam/src/lib.rs:418`). One churner
   issues calls of ~0.5 ms wall on the release build (measured below). A
   dirty call occupies a dirty CPU scheduler, so a normal-scheduler Erlang
   burst can slow only if dirty work leaks onto normal schedulers or through
   OS core contention. The sweep separates the two: churn-1 (one dirty
   thread on 16 logical CPUs) cannot oversubscribe the host, while
   churn-16 saturates it. Idle-B brackets ambient drift. The verdict rule is
   printed with the output.

### Receipt run (verbatim)

```text
nif loaded: /Users/sac/avec/target/release/libavec_beam.so
[env] erts=16.2 otp_release=28
[env] logical_cpus=16 schedulers_online=16 dirty_cpu_schedulers=16 dirty_io_schedulers=10 native_ns_per_unit=1.0
[env] nif_build=release dylib=target/release/libavec_beam.dylib run_class=single-run machine-local
[env] dylib_sha256: 2a45b21dbfaaeb9001e314019fb603d3f038e3cfcc0b0f7c7c1bf47f860ebd22  /Users/sac/avec/target/release/libavec_beam.dylib
[env] host_load: 20:51  up 3 days,  4:22, 4 users, load averages: 10.73 12.00 12.43
ok   sanity: ramp + stats + i64 path verified; version law 26.10.29 verified (26.10.29)
[get-latency]       n=10000 min_us=0.000 median_us=0.042 p99_us=0.084 max_us=0.916  | avec_get/2 round-trip us (incl. 2x monotonic_time(nanosecond)), release dylib
[get-calibration]   n=10000 min_us=0.000 median_us=0.042 p99_us=0.042 max_us=18.167  | monotonic_time pair only, no NIF (subtractable bracketing overhead)
[churn-char]        n=50 min_us=452.500 median_us=508.708 p99_us=939.625 max_us=939.625  | avec_new(262144,0) wall us, dirty CPU, release, n=50 (free untimed); one churner loop = repeated calls of this size
[burst-cal] iterations=4940320 measured_ms=9.468 (target 10.0 ms, fixed for all phases)
[iso-idle-A ]  n=263 min_us=9053.375 median_us=9519.375 p99_us=10201.584 max_us=10450.542  | fixed pure-Erlang ~10ms burst wall us; churners=0
[iso-churn-1]  n=259 min_us=9252.625 median_us=9637.417 p99_us=10536.958 max_us=14037.000  | fixed pure-Erlang ~10ms burst wall us; churners=1
[iso-churn-4]  n=250 min_us=9250.084 median_us=9995.875 p99_us=11124.500 max_us=11525.500  | fixed pure-Erlang ~10ms burst wall us; churners=4
[iso-churn-F ]  n=180 min_us=9897.459 median_us=12202.083 p99_us=92821.708 max_us=144623.583  | fixed pure-Erlang ~10ms burst wall us; churners=16
[iso-idle-B ]  n=261 min_us=9103.541 median_us=9408.000 p99_us=14133.166 max_us=26649.292  | fixed pure-Erlang ~10ms burst wall us; churners=0
[iso-ratio] churn-1 median_ratio=0.991 p99_ratio=1.054 (vs tighter idle bracket)
[iso-ratio] churn-4 median_ratio=1.092 p99_ratio=1.060 (vs tighter idle bracket)
[iso-ratio] churn-16 median_ratio=1.464 p99_ratio=1.191 (vs tighter idle bracket)
[iso-verdict] ISOLATION-CONSISTENT; degradation at higher churn attributed to OS core contention near saturation (churn-1 clean)
```

Exit code 0. Host load at run time 10.73/12.00/12.43 (1/5/15 min) on 16
logical CPUs: concurrent lanes were running CPU-heavy work on the same host
throughout, and that load is part of every number below.

### Run-to-run stability (disclosed)

A second full run minutes later, same host, loadavg 9.31/11.36/12.16, same
dylib sha256, version law verified, exit 0:

```text
[get-latency]       n=10000 min_us=0.000 median_us=0.042 p99_us=0.084 max_us=13.375  | avec_get/2 round-trip us (incl. 2x monotonic_time(nanosecond)), release dylib
[get-calibration]   n=10000 min_us=0.000 median_us=0.042 p99_us=0.042 max_us=8.666  | monotonic_time pair only, no NIF (subtractable bracketing overhead)
[churn-char]        n=50 min_us=419.584 median_us=492.083 p99_us=514.958 max_us=514.958  | avec_new(262144,0) wall us, dirty CPU, release, n=50 (free untimed); one churner loop = repeated calls of this size
[burst-cal] iterations=4581901 measured_ms=10.040 (target 10.0 ms, fixed for all phases)
[iso-idle-A ]  n=238 min_us=10007.750 median_us=10070.250 p99_us=15314.416 max_us=49018.916  | fixed pure-Erlang ~10ms burst wall us; churners=0
[iso-churn-1]  n=247 min_us=10017.166 median_us=10088.833 p99_us=10969.750 max_us=11429.375  | fixed pure-Erlang ~10ms burst wall us; churners=1
[iso-churn-4]  n=208 min_us=10039.417 median_us=10442.292 p99_us=26225.458 max_us=60517.834  | fixed pure-Erlang ~10ms burst wall us; churners=4
[iso-churn-F ]  n=201 min_us=10081.042 median_us=12313.083 p99_us=16841.792 max_us=17402.292  | fixed pure-Erlang ~10ms burst wall us; churners=16
[iso-idle-B ]  n=242 min_us=9965.709 median_us=10012.709 p99_us=16053.042 max_us=22594.959  | fixed pure-Erlang ~10ms burst wall us; churners=0
[iso-ratio] churn-1 median_ratio=0.999 p99_ratio=0.999 (vs tighter idle bracket)
[iso-ratio] churn-4 median_ratio=1.410 p99_ratio=1.015 (vs tighter idle bracket)
[iso-ratio] churn-16 median_ratio=1.371 p99_ratio=1.266 (vs tighter idle bracket)
[iso-verdict] ISOLATION-CONSISTENT; degradation at higher churn attributed to OS core contention near saturation (churn-1 clean)
```

### Reading

- **get-latency (release)**: median 0.042 us and p99 0.084 us in both runs.
  The median equals the no-NIF bracketing floor (0.042 us in both runs), so
  the `avec_get/2` round-trip including both timer reads is indistinguishable
  from the timer pair alone at median; the NIF cost sits below the ~42 ns
  bracketing overhead of this sensor, and the p99 increment over the
  calibration row (+0.042 us) is one timer-resolution step. Sub-us
  single-call latency on the release dylib: receipted. Max is ambient-noisy
  (0.916 us run 1; 13.375 us run 2, matched by the 8.666 us calibration-row
  max in the same run — the tail is the bracketing sensor, not the NIF).
- **churn characterization (release)**: `avec_new(262144, 0)` median
  492-509 us (debug build was 742 us at 16384 rows in the OPT-L8 receipt).
- **isolation**: churn-1 median ratio 0.991 (run 1) and 0.999 (run 2) — one
  continuously-churning dirty worker leaves the normal-scheduler Erlang
  burst unchanged to within 1%. The architecture's actual claim (dirty-CPU
  work pinned to dirty schedulers, not migrating onto normal schedulers)
  holds at this instrument's resolution. churn-16 median ratios
  1.464/1.371: with 16 dirty + 16 normal scheduler threads plus ambient load
  on 16 logical CPUs, the burst slows ~40-46% at median — OS core contention
  at saturation, absent from every churn-1 observation. churn-4 is the noisy
  level (1.092 run 1; 1.410 median with 1.015 p99 run 2): mid-level ratios
  are not stable enough at this ambient load to gate on, and the verdict
  rule deliberately keys the isolation claim on churn-1. Idle phases show
  multi-ms tails with zero churn (idle-B max 22.6-26.6 ms) — ambient,
  matching the OPT-L8 finding.
- **verdict (both runs)**: ISOLATION-CONSISTENT — dirty-NIF churn does not
  leak onto normal schedulers at this instrument's resolution; degradation
  appears only near saturation and is quantified with the OS-contention
  confound disclosed rather than resolved.

### What this sensor class can and cannot prove about the 50 us CTQ

CAN (receipted above):

- Sub-us single-call NIF latency, with a subtractable bracketing floor.
- Scheduler-class isolation at ms-class workload granularity: normal-
  scheduler Erlang compute is unaffected by continuous dirty-NIF churn below
  scheduler saturation.

CANNOT:

- Resolve the <=50 us scheduler-impact gate, for three independent reasons.
  (1) The CTQ names scheduling/wakeup impact; every in-ERTS timer floors
  near 1 ms (OPT-L8 receipt) and `statistics/1` counters are ms-granular, so
  no in-ERTS sensor can timestamp an external wakeup with sub-us truth.
  (2) This probe's unit of measurement is a ~10 ms burst; a persistent
  50 us-class per-event perturbation is 0.5% of one burst, below the
  observed run-to-run ambient spread (idle medians 9.4 -> 10.1 ms across
  runs 15 minutes apart on the same host). (3) The churn phases run on a
  shared host (loadavg 9-12) where idle phases already show multi-ms tails,
  so any sub-ms gate would be noise-dominated here.

Status of the 50 us CTQ after this lane: **UNSUPPORTED (instrument class)** —
not PASS, not FAIL. The OPT-L8 FAIL verdict above stands as the honest
reading of its mandated instrument; neither 1 ms timers, nor ms-granular
counters, nor 10 ms burst degradation can adjudicate 50 us. Resolving the
CTQ requires hardware-timer-class instrumentation outside the BEAM: an
external timestamping probe around real scheduled wakeups (DTrace/ktrace-
class tracer, or a sensor pinned to a dedicated core), which this repo does
not have today. No threshold was adjusted anywhere in this lane.

### Reproduce

```sh
escript test/beam_latency2.escript   # exit 0 = completed run, any verdict
```

Exit codes: 0 completed run (verdicts are printed, not gated); 1 version-law
mismatch; 2 NIF load failure; 3 phase timeout.
