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
