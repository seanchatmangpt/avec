# Receipt: gate-perf-guard

| Field | Value |
|---|---|
| Receipt ID | gate-perf-guard |
| Date | 2026-10-31T03:07:01Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `scripts/perf_guard.sh` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (64 entries) |
| Evidence digest (blake3 of stdout tail) | `b10c7d5c3d1c260396ced099a4d48571f3ac2cf7e1fe27f3f637d6312ca63a2e` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
[perf_guard] building release (avec-blake3, avec-integration, bench)...
   Compiling bench v26.10.29 (/Users/sac/avec/crates/bench)
    Finished `release` profile [optimized] target(s) in 6.83s
[perf_guard] gage R&R probe (reps=2000, noise CV threshold 2.0%)...
[perf_guard] gage cv_trimmed=0.8833% (gage_pass_exit=0)
[perf_guard] running ctq_report...
[perf_guard] ctq_report exit=1 CTQ-05=8.208 us (canary ceiling 30.0 us; spec USL 35 us)
[perf_guard] WARNING: ctq_report exit=1 from non-CTQ-05 gates [CTQ-06] — out of this guard's scope; ctq_report remains the authority for those gates
PERF-GUARD-OTHER-GATES-FAILED [CTQ-06] (informational; not this guard's verdict)
[perf_guard] PASS: CTQ-05=8.208 us <= 30.0 us (spec USL 35 us unchanged)
PERF-GUARD-PASS ctq05_us=8.208 ceiling_us=30.0 gage_cv_pct=0.8833
```

<!-- Tree state: dirty (64 entries) -->
