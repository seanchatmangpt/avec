# Receipt: gate-gage-rr

| Field | Value |
|---|---|
| Receipt ID | gate-gage-rr |
| Date | 2026-10-31T02:33:40Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `./target/release/gage_rr --reps 10000` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (61 entries) |
| Evidence digest (blake3 of stdout tail) | `9449b793e8657e20d8b8aef7c935f184a9a9941136306f1f1341b574e3a49731` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
config: scale=1 workload_bytes=4096 n_reps=10000
warmup: 500 reps, sink=80
measured: 10000 reps
{"timer":"Instant (monotonic, CNTVCT-class on Apple Silicon)","workload":"avec_blake3::hash over fixed 4096-byte buffer (scale 1)","scale":1,"n_samples":10000,"warmup_reps":500,"trim_rule":"drop bottom 1% and top 1% of sorted samples (100 per side at N=10000)","trimmed_n":9800,"stats":{"mean_us":3.2175,"median_us":3.2080,"p1_us":3.1660,"p99_us":3.3750,"stddev_us":0.3491,"cv_raw_pct":10.8488,"mean_trimmed_us":3.2038,"median_trimmed_us":3.2080,"stddev_trimmed_us":0.0278},"cv_raw_pct":10.8488,"cv_trimmed_pct":0.8678,"gage_threshold_pct":1,"gage_pass":true}
gage: cv_raw=10.8488% cv_trimmed=0.8678% threshold=1% pass=true
```

<!-- Tree state: dirty (61 entries) -->

## Wave-6 lane-5 config table (each config run 3x, cv_trimmed % / mean trimmed us)

| config | run1 | run2 | run3 | pass (<1%) |
|---|---|---|---|---|
| A: scale=1, default QoS (receipted) | 0.8028 / 3.2080 | 0.7993 / 3.2023 | 0.8180 / 3.2082 | PASS 3/3 |
| B: `--scale 10` (40 KiB, ~32us mean), default QoS | 0.8425 / 32.1980 | 0.8701 / 32.1956 | 0.9583 / 32.2079 | PASS 3/3 |
| C: scale=1, `taskpolicy -b` (background QoS) | 15.6539 / 9.3563 | 20.5114 / 9.0686 | 30.1407 / 8.2361 | FAIL 0/3 |
| D: `--scale 10`, `taskpolicy -b` | 20.2256 / 92.0817 | 19.2563 / 92.8257 | 15.0075 / 95.2888 | FAIL 0/3 |

Notes:
- Wave-5 failures (CV 1.4-5.5%) did not reproduce: under 4-lane load the baseline
  passes. Root cause of wave-5 failures was 19-lane contention, not the harness.
- `--scale`/`--reps` flags added to `crates/bench/src/bin/gage_rr.rs` (parse_args).
- Scale-10 repair not needed on this machine; it passes but does not improve CV
  over baseline (jitter dilution offset by larger-workload variance).
- Background QoS (`taskpolicy -b`) is an anti-repair: it throttles the process
  and inflates CV 15-30%. Do not use for timing gates.
- Process-affinity note: macOS has no process-level CPU pinning CLI
  (`taskpolicy` sets QoS/priority only); affinity would require
  `thread_policy_set` in-process — not pursued, unnecessary given baseline PASS.
