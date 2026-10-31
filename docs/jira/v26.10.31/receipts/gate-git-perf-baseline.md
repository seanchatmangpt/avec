# Receipt: gate-git-perf-baseline

| Field | Value |
|---|---|
| Receipt ID | gate-git-perf-baseline |
| Date | 2026-10-31T03:40:59Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-git --release --test perf_baseline -- --ignored --nocapture` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (67 entries) |
| Evidence digest (blake3 of stdout tail) | `2396fa41b64fbfd1747b6f1ae2958d421743c104dec3b5e31e1f4ce0b1a3a449` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
    Finished `release` profile [optimized] target(s) in 0.01s
     Running tests/perf_baseline.rs (target/release/deps/perf_baseline-7027dcaca3931516)

running 1 test

=== avec-git perf baseline (64 MiB corpus, 256 x 256 KiB blobs) ===
machine-context caveat: concurrent wave lanes; informational, no gate
pack write r0: 16.6 MiB/s, 66 objects/s (dt=3.851s, pack=70760510 B, idx=8240 B)
pack write r1: 16.6 MiB/s, 66 objects/s (dt=3.856s, pack=70760510 B, idx=8240 B)
pack write r2: 16.6 MiB/s, 67 objects/s (dt=3.849s, pack=70760510 B, idx=8240 B)
PACK WRITE: best 16.6 MiB/s, 66 objects/s (min-of-3)
IDX LOOKUP: 3496483 lookups/s over 65536-object idx (0.029s total, hits=50000 misses=50000)
ladder constant-16-stage: structural (probe counter compiled out in release; no data-dependent exits in ladder.rs)
GIT REFERENCE: `git pack-objects` on same 64 MiB corpus: 12.1 MiB/s (5.309s, 256 objects, includes 256 hash-object spawns)
NOTE: git reference includes process spawn overhead; not a gate, comparison point only
test perf_baseline_git_plane ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.11s
```

<!-- Tree state: dirty (67 entries) -->

## Scope note: informational-by-design (wave-8 lane 14)

This test is a measurement event, not a spec gate (`perf_baseline.rs` header:
"Informational only — no gate"). `ALIVE` here receipts that the measurement ran
and completed (exit 0, 19.11s), not conformance to a performance threshold. The
`git pack-objects` number is a reference point including process-spawn overhead
(256 `hash-object` spawns), not a pass/fail comparator.

Fresh-run measurements (2026-10-31, this receipt's mint run):

| Metric | This run | Wave-7 lane 15 (prior) |
|---|---|---|
| Pack write (min-of-3) | 16.6 MiB/s, 66 objects/s | 16.6 MiB/s, 66 objects/s |
| idx lookup ladder | 3,496,483 lookups/s (65,536-object idx, 50k hits / 50k misses) | 3.49M lookups/s |
| `git pack-objects` reference | 12.1 MiB/s (5.309s) | 12.1 MiB/s |

## Default suite evidence

Command: `cargo test -p avec-git` (default profile, run separately this lane;
exit 0). Result: 48 passed, 0 failed, 1 ignored (the perf baseline itself) —
all courts pass:

- unittests src/lib.rs: 43 passed
- tests/fsck_court.rs: 2 passed
- tests/git_interop.rs: 3 passed
- tests/perf_baseline.rs: 1 ignored (informational, `--ignored`-gated)
- doc-tests: 0 passed
