# Getting Started

10-minute onboarding for `avec`, a freestanding Apache Arrow columnar substrate.
Every command below was executed on the reference host (arm64, Rust 1.97.0) before
this document was written; expected outputs are quoted from those runs. Build times
assume a warm `target/` cache; a cold build is slower.

## Prerequisites

Hard requirements (the build and tests fail without them):

| Tool | Why | Check | Reference (this host) |
|---|---|---|---|
| Rust stable | builds everything | `rustc --version` | `rustc 1.97.0` |
| rustfmt | format gate | `cargo fmt --version` | `rustfmt 1.9.0-stable` |
| git | gate `cli-verify` shells out to real git | `git --version` | `git version 2.51.2` |

Optional tools (individual courts degrade without them):

| Tool | Why | Check | Install |
|---|---|---|---|
| `b3sum` | independent FR-06 hash oracle in the `cli-verify` gate | `b3sum --version` | `brew install b3sum` |
| `python3` + `pyshacl` + `rdflib` | the SHACL ontology court | `python3 -c "import pyshacl, rdflib"` | `pip3 install --user pyshacl rdflib` |
| `just` | recipe runner; recipes are Makefile targets today | `just --version` | `brew install just` |
| wasm32 targets | portability checks (see Build below) | `rustup target list --installed` | `rustup target add wasm32-unknown-unknown wasm32-wasip1` |

Reference versions on this host: `b3sum 1.8.5`, `just 1.56.0`, Python 3.14.3,
pyshacl 0.31.0, targets `wasm32-unknown-unknown` + `wasm32-wasip1` + `wasm32-wasip2`.

Note on `just`: a justfile now exists at the repo root (`just doctor`,
`just verify`, `just bench`, `just courts`, `just receipts`, `just dod`,
`just status`). The Makefile remains as the legacy entry point.

## Build

```bash
cargo build --workspace
```

Expected (warm cache; the seconds vary):

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.16s
```

Zero warnings is the standing bar: the ladder's `check-workspace` gate diffs every
`error`/`warning` line against `scripts/verify_baseline.txt` (0 lines today), and its
latest run is PASS. If you see a `warning:` line, treat it as introduced and fix it
before proceeding.

Optional portability check (needs the wasm32 target above):

```bash
cargo build -p avec-arrow --target wasm32-unknown-unknown
```

Expected: `Finished ... target(s) in <time>` with exit 0. Also receipted in
`crates/wasm-footprint/NOTES-v26.10.31.md` for `wasm32-wasip1` and release mode.

## Run the tests

The fast signal — the avec-arrow unit suite:

```bash
cargo test -p avec-arrow --lib
```

Expected final line:

```text
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

### Reading counts without the doc-test parse trap

Do not read a crate's health off a single `test result:` line. A crate emits one
result line per suite: `unittests`, each `tests/*.rs` file, and a `Doc-tests` line.
Worse, `avec-arrow`'s `tests/mutation_kills.rs` emits **two** result lines (a nested
filtered libtest child, then the outer suite) — the trap found in the wild.

Law: per crate, sum **all** `test result:` lines. The full census, with the
canonical invocations and per-suite rows, is
[docs/jira/v26.10.31/63-test-census.md](jira/v26.10.31/63-test-census.md):
58 result lines, 661 passed, 0 failed, 2 ignored across 13 invocations.

## Run the doctor

Status: **PENDING** — not yet on disk. `ls crates/cli/src/bin/` shows only
`verify_v26.rs` at the time of writing. When the doctor lane lands, run:

```bash
cargo run -q -p avec-cli --bin doctor
```

No output block is quoted here: no receipt for it exists yet, and this document
does not quote outputs it has not seen. Re-check availability with:

```bash
ls crates/cli/src/bin/
```

Interim health probe, verified live (the ladder's `cli-verify` gate payload):

```bash
cargo run -q -p avec-cli --bin verify_v26
```

Expected (exit 0; timings vary):

```text
[verify_v26] FR-04 court using git version 2.51.2
[verify_v26] battery complete: 7 rows, 0 FAIL, skips=[FR-07]
{"tool":"verify_v26","rows":[...]}
```

## Run the verify ladder

Full 10-gate battery, dependency order, writes the machine report:

```bash
./scripts/verify_v26.10.31.sh
```

Budget 10-15 minutes on a warm cache (operator-stated planning number, not a
measured receipt). On success the last lines are:

```text
SUMMARY: report -> docs/jira/v26.10.31/verify-latest.md
RESULT: OK — no FAIL; skips/pre-existing disclosed in docs/jira/v26.10.31/verify-latest.md
```

Exit codes: `0` = no FAIL (skips and pre-existing failures disclosed but green);
`1` = at least one FAIL. The receipted latest run: 2026-10-31T04:06:51Z at HEAD
`ead40329` — 10/10 gates PASS, exit 0, in
[docs/jira/v26.10.31/verify-latest.md](jira/v26.10.31/verify-latest.md).

## Interpret verify-latest.md

The report has three parts: a gate table (10 rows), evidence tails (last 12 output
lines per gate), and the baseline block.

Gate table from the receipted run — all PASS:

| Gate | Status | Exit |
|------|--------|------|
| `check-workspace` | PASS | 0 |
| `test-avec-arrow` | PASS | 0 |
| `test-avec-parquet` | PASS | 0 |
| `test-avec-git` | PASS | 0 |
| `test-avec-blake3` | PASS | 0 |
| `test-integration` | PASS | 0 |
| `test-avec-beam` | PASS | 0 |
| `audit-branchless` | PASS | 0 |
| `cli-verify` | PASS | 0 |
| `test-audit` | PASS | 0 |

Status semantics (from
[verify-README.md](jira/v26.10.31/verify-README.md)):

| Status | Meaning | Fails run? |
|--------|---------|------------|
| `PASS` | exit 0, output tail captured | no |
| `FAIL` | non-zero exit and (gate 1 only) output differs from baseline — introduced | yes (exit 1) |
| `PRE-EXISTING-FAIL` | gate 1 failed but output matches `scripts/verify_baseline.txt` | no |
| `SKIP` | owner artifact absent, or a timeout-guarded gate hit its limit (exit 124) | no, surfaced loudly |

Baseline classification is content-based, not exit-code-based: the script sorts
`error`/`warning` signature lines from `cargo check --workspace` and compares them
against `scripts/verify_baseline.txt`. Matching output is disclosed as
PRE-EXISTING-FAIL; differing output is FAIL (introduced). Partially-fixed states
read as FAIL. Regenerate only from a known-green tree:
`./scripts/verify_v26.10.31.sh --regen-baseline`.

## What next

- A gate is red? [docs/TROUBLESHOOTING.md](TROUBLESHOOTING.md) is the first
  stop (landed wave-10 lane 9): nine Symptom -> Cause -> Fix sections covering
  the known failure classes. The gate's own evidence tail in
  `verify-latest.md` plus the owning lane's receipt under
  [docs/jira/v26.10.31/receipts/](jira/v26.10.31/receipts/) remain the
  primary evidence.
- Guided tour by task:
  [docs/diataxis/INDEX.md](diataxis/INDEX.md) routes to tutorials, how-tos,
  API reference, and architecture explanations (all four quadrant docs on disk).
- The milestone corpus: [docs/jira/v26.10.31/00-index.md](jira/v26.10.31/00-index.md)
  — specs, receipts, wave reports, the verification ladder spec
  ([31-verification-ladder.md](jira/v26.10.31/31-verification-ladder.md)), and the
  release checklist ([36-release-checklist.md](jira/v26.10.31/36-release-checklist.md)).

# See Also

- [docs/jira/v26.10.31/verify-README.md](jira/v26.10.31/verify-README.md) — gate
  battery runbook and baseline policy
- [docs/jira/v26.10.31/63-test-census.md](jira/v26.10.31/63-test-census.md) —
  full test census and the result-line counting method
- [docs/RECEIPTS.md](RECEIPTS.md) — receipt discipline for claims
- [README.md](../README.md) — project overview
