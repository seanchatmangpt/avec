# Verify README: v26.10.31 Gate Battery (lane I20)

How to run and read the top-level verification orchestrator for the
v26.10.31 milestone. Spec parents:
[31-verification-ladder.md](31-verification-ladder.md) (R8 machine report) and
[36-release-checklist.md](36-release-checklist.md) (DMADV Verify gate).

## Run

```bash
./scripts/verify_v26.10.31.sh                  # standard run
./scripts/verify_v26.10.31.sh --regen-baseline # discard + rewrite the baseline from this run
```

The script runs the gate battery in dependency order and writes
[verify-latest.md](verify-latest.md) with per-gate status and evidence tails.
Exit code: `0` = no FAIL (skips and pre-existing failures disclosed, run still
green); `1` = at least one FAIL.

## Gate List (dependency order)

| # | Gate | Command | Owner |
|---|------|---------|-------|
| 1 | `check-workspace` | `cargo check --workspace` | I20 |
| 2 | `test-avec-arrow` | `cargo test -p avec-arrow` | I20 |
| 3 | `test-avec-parquet` | `cargo test -p avec-parquet --features std` | lanes I01+ (tests need `--features std`) |
| 4 | `test-avec-git` | `cargo test -p avec-git` | packfile lane |
| 5 | `test-avec-blake3` | `cargo test -p avec-blake3` | BLAKE3 lane (plain invocation; zero_alloc structural fix landed, `--test-threads=1` not needed) |
| 6 | `test-integration` | `cargo test -p avec-integration` | integration lane |
| 7 | `test-avec-beam` | `timeout 600 cargo test -p avec-beam` | beam lane (real BEAM VM; SKIP-with-reason on 600s timeout, exit 124) |
| 8 | `audit-branchless` | `scripts/audit_branchless.sh --selftest` | lane I14 (SKIP loudly if absent). Manifest-aware (wave-7): checks `scripts/branchless_manifest.toml` per-kernel caps; override path with `BRANCHLESS_MANIFEST=` |
| 9 | `cli-verify` | `timeout 300 cargo run -q -p avec-cli --bin verify_v26` | lane I17 — build-and-run under 300s timeout; runs FR-01..07 battery, prints JSON (SKIP on 300s timeout, exit 124) |
| 10 | `test-audit` | `cargo test -p avec-audit` | audit lane |

## Status Semantics

| Status | Meaning | Fails run? |
|--------|---------|------------|
| `PASS` | Exit 0, tail captured | no |
| `FAIL` | Non-zero exit and (for check-workspace) output differs from baseline — introduced | **yes (exit 1)** |
| `PRE-EXISTING-FAIL` | check-workspace failed but failure output matches `scripts/verify_baseline.txt` — disclosed, not introduced by this wave | no |
| `SKIP` | Gate's owner artifact absent (e.g. `audit_branchless.sh` from lane I14), or a timeout-guarded gate hit its limit — `test-avec-beam` at 600s or `cli-verify` at 300s (exit 124) | no, but surfaced loudly in output and report |

## Latest Result

Run of 2026-10-31T04:06:51Z at HEAD `ead40329a699878a91b070378066b5e50541ba33`
(arm64): all 10 gates PASS, exit 0. See [verify-latest.md](verify-latest.md).

## Baseline Regeneration Policy

`scripts/verify_baseline.txt` is the disclosed-failure baseline for gate 1
only (`cargo check --workspace`). It is:

- **Auto-written on first run** if missing (from that run's check output — a
  failing state at first capture is recorded as pre-existing by construction;
  only regenerate from a known-good tree).
- **Checked into the tree state under docs/jira/v26.10.31 (uncommitted
  working tree)** so other lanes/machines compare against the same baseline.
  Delete it and re-run `--regen-baseline` only when the introduced failures
  are fixed and the workspace checks green, or when the baseline itself is
  stale (e.g. pre-existing diagnostics legitimately changed upstream).
- **Diffed as sorted error/warning SIGNATURE lines** — the script's `sig()` is
  `grep -E '^(error|warning)' | sort -u` over the check output, not "sorted
  tails". Classification is content-based, not exit-code-based, so
  partially-fixed states read as FAIL.

# See Also

- [31-verification-ladder.md](31-verification-ladder.md)
- [36-release-checklist.md](36-release-checklist.md)
- [verify-latest.md](verify-latest.md)
