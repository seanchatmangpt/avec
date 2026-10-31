# Flake Register — Wave 14 Lane 13 (Recovery)

First clean-tree flake measurement, v26.10.31. Subject: on-disk tree at
/Users/sac/avec, coordinator-stated committed/clean at dispatch (HEAD
85cdb85 per dispatch context; no git commands run, per lane contract).
Method: 10 batches x 6 suites = 60 runs, 2026-10-31 15:20:03Z through
15:24:44Z (4m41s). Truth = process exit code. No sleeps; suites ran
back-to-back. Raw logs: /tmp/wave14-lane13/ (results.tsv, failures.txt,
uptime.log, 60 per-run logs). Toolchain: cargo/rustc 1.97.0, macOS
Darwin 25.2.0, 16 cores, 48 GB.

## Verdict

**Zero flakes in 60 runs.** No run failed intermittently. Every failure
observed repeated identically from batch to batch until the owning lane
changed the code mid-sweep. All failures are deterministic and live in
other lanes' in-flight wave-14 test files (handoff items, not flakes).

## Suite register (the table)

Exact commands, one per suite:

```text
cargo test -p avec-arrow --lib
cargo test -p avec-git
cargo test -p avec-blake3 --test zero_alloc
cargo test -p avec-ffi --test alloc_courts
cargo test -p avec-beam
cargo test -p avec-parquet --features std --lib scan
```

| Suite | Command | Rate | Class | Failing surface |
|---|---|---|---|---|
| avec-arrow lib | arrow | 10/10 | STABLE | — |
| avec-git all | git | 0/10 | DETERMINISTIC-FAIL | ladder_mutations b1-9; large_offset b10 |
| avec-blake3 zero_alloc | blake3 | 10/10 | STABLE | — |
| avec-ffi alloc_courts | ffi | 10/10 | STABLE | — |
| avec-beam all | beam | 0/10 | DETERMINISTIC-FAIL | kill_stress all 10 |
| avec-parquet scan | parquet | 10/10 | STABLE | — |

Pass-rate totals: 40/60 suite-runs PASS. Class vocabulary: STABLE =
10/10 with identical test counts; DETERMINISTIC-FAIL = 0/10 with an
identical signature every run. FLAKE (intermittent pass/fail) did not
occur; if it had, the row would carry run#, error, class per cell.

## 60-run matrix (P = rc 0, F = rc 101)

| Batch | arrow | git | blake3 | ffi | beam | parquet | Load 1m |
|---|---|---|---|---|---|---|---|
| 1 | P | F | P | P | F | P | 3.35 |
| 2 | P | F | P | P | F | P | 3.35 |
| 3 | P | F | P | P | F | P | 4.26 |
| 4 | P | F | P | P | F | P | 4.26 |
| 5 | P | F | P | P | F | P | 4.99 |
| 6 | P | F | P | P | F | P | 4.54 |
| 7 | P | F | P | P | F | P | 4.61 |
| 8 | P | F | P | P | F | P | 3.95 |
| 9 | P | F | P | P | F | P | 3.82 |
| 10 | P | F | P | P | F | P | 5.53 |

## Failure signatures (run#, error, class)

1. **avec-git, batches 1-9** — `ladder_mutations.rs`
   `fanout_inflation_crossing_window_is_typed_too_many_objects`
   (ladder_mutations.rs:350): expected `Err(TooManyObjects(4294901765))`,
   got `Err(Truncated)`; class: wrapped-u32 fanout partition classifies
   as Truncated, not typed TooManyObjects. `oid_byte_corruption_never_false_hits`
   (ladder_mutations.rs:444): expected `Ok(())`, got
   `Err(IdxChecksumMismatch)`; class: opt-in checksum fires on a flip
   the test held must not false-hit. Identical both failures, all 9
   batches. **Handoff: ladder-mutation lane** — the lane fixed both
   between batch 9 and 10 (b10: `12 passed; 0 failed`).
2. **avec-git, batch 10 only** — new target `tests/large_offset.rs`,
   landed mid-sweep:
   `case4_corrupt_msb_on_small_entry_is_typed_error_never_panic`
   (large_offset.rs:283) and `case6_offsets_above_2p32_resolve_as_full_64bit`
   (large_offset.rs:155), `4 passed; 2 failed` in 0.00s.
   Class: new-test-vs-implementation mismatch, deterministic on its one
   observed run. **Handoff: large-offset lane.**
3. **avec-beam, all 10 batches** — `tests/kill_stress.rs`
   `beam_kill_stress_100_iterations`, rc 101 each run: escript
   `{badmatch,200}` in `avec_beam storm_iter` (line 136),
   `KILL_STRESS_FAIL error:{storm_failed,1,...}`, panic site moved
   :419 -> :418 between b9 and b10 (file edited mid-sweep).
   Class: kill-storm iteration-1 badmatch, deterministic across all 10.
   **Handoff: kill-stress lane.**

## Moving-tree disclosure

Another wave-14 lane mutated the measured tree during the sweep.
Witnessed in-run: ladder_mutations 11 -> 12 tests at batch 6, fixed at
batch 10; large_offset.rs target appears at batch 10; kill_stress.rs
edited between b9 and b10; `compiled=y` rebuilds observed for avec-ffi
(b1,3,4,7,8), avec-git (b9,10), avec-beam (b1,7,10). No intermittent
outcome can be attributed to the tree alone — but a strictly clean-tree
claim covers suites b1-b9 signatures only. The four STABLE suites held
identical test counts and rc 0 across all 10 batches including all
rebuild windows.

## Load context (uptime per batch)

| Batch | Time | Uptime | Load 1m/5m/15m |
|---|---|---|---|
| 1 | 8:20 | up 23:29 | 3.35 / 4.50 / 4.30 |
| 2 | 8:20 | up 23:30 | 3.35 / 4.41 / 4.27 |
| 3 | 8:20 | up 23:30 | 4.26 / 4.57 / 4.33 |
| 4 | 8:21 | up 23:31 | 4.26 / 4.52 / 4.32 |
| 5 | 8:21 | up 23:31 | 4.99 / 4.67 / 4.38 |
| 6 | 8:22 | up 23:31 | 4.54 / 4.58 / 4.36 |
| 7 | 8:22 | up 23:32 | 4.61 / 4.57 / 4.36 |
| 8 | 8:23 | up 23:32 | 3.95 / 4.41 / 4.31 |
| 9 | 8:23 | up 23:33 | 3.82 / 4.34 / 4.28 |
| 10 | 8:24 | up 23:33 | 5.53 / 4.77 / 4.44 |

Load ranged 3.35-5.53 (1m) on 16 cores with concurrent wave-14 lanes
active. Failure incidence showed zero load correlation: both failing
suites failed identically at the minimum and maximum observed load.

## Falsifier / replay

- Replay: `/bin/zsh /tmp/wave14-lane13/sweep.zsh` on a quiescent tree;
  any rc that flips pass/fail between batches with an unchanged
  signature is a flake and amends this register with run#, error, class.
- The three DETERMINISTIC-FAIL signatures are falsified by their owning
  lanes' fixes landing: rerun the suite post-fix; register row flips to
  STABLE only on a 10/10 witness.

Standing: STABLE x4 witnessed, DETERMINISTIC-FAIL x2 witnessed, FLAKE
x0 witnessed. Sweep receipt: exit 0, 60/60 runs logged, logs on disk at
/tmp/wave14-lane13/.
