# TROUBLESHOOTING

Known failures witnessed in the v26.10.31 milestone, as Symptom -> Cause -> Fix. Every
entry below was hit in this tree during waves 1-10 — nothing here is hypothetical. Run
gates the way the census did (entry 2) before diagnosing further; canonical invocations
live in the `Makefile`, `justfile`, and `scripts/verify_v26.10.31.sh`.

## Contents

1. [avec-beam tests: nif_not_loaded / stale dylib](#1-avec-beam-tests-nif_not_loaded--stale-dylib)
2. [Gate passes with 0 tests: doc-test parse trap](#2-gate-passes-with-0-tests-doc-test-parse-trap)
3. [Branchless audit false-matches bic/blr](#3-branchless-audit-false-matches-bicblr)
4. [objdump refuses rlibs / `<ltmp0>` symbols](#4-objdump-refuses-rlibs--ltmp0-symbols)
5. [Timing gates flap under load: SKIP-NOISY](#5-timing-gates-flap-under-load-skip-noisy)
6. [cargo test -p bench --lib exits 101](#6-cargo-test--p-bench---lib-exits-101)
7. [cargo build -p wasm-footprint: no match](#7-cargo-build--p-wasm-footprint-no-match)
8. [fsck_court inflate errors: stored body](#8-fsck_court-inflate-errors-stored-body)
9. [pyshacl missing: validate_ontology.sh refuses](#9-pyshacl-missing-validate_ontologysh-refuses)

## 1. avec-beam tests: nif_not_loaded / stale dylib

**Symptom.** `cargo test -p avec-beam` dies with `nif_not_loaded`, or bridge tests fail
against symbols the current source no longer has.
**Cause.** `cargo test` does not rebuild the cdylib; the tests dlopen a stale `.dylib`
left by an earlier build.
**Fix.** Build before test:

```sh
cargo build -p avec-beam
cargo test -p avec-beam
```

`make test-beam` already orders it this way (Makefile:131-136). Census receipt:
`docs/jira/v26.10.31/63-test-census.md` ("stale-dylib lesson"); NIF surface spec:
`docs/jira/v26.10.31/28-beam-nif-handoff.md`.

## 2. Gate "passes with 0 tests": doc-test parse trap

**Symptom.** A test gate exits 0 but reports "0 passed; 0 failed" — it tested nothing.
**Cause.** Doc-test parse trap: the LAST `test result:` line can be a 0-test doc-test
section, so last-line parsing hides the real totals. Separately, bare
`cargo test -p avec-parquet` (default features) collects the std-gated courts as 0.
**Fix.** Sum ALL `test result:` lines per crate, never the last (method:
`docs/jira/v26.10.31/63-test-census.md`). Run the std courts explicitly:

```sh
cargo test -p avec-parquet --features std
```

## 3. Branchless audit false-matches bic/blr

**Symptom.** The branchless audit reports banned-opcode hits on legal `bic`/`blr`
instructions, so gate1 fails spuriously on clean kernels.
**Cause.** The `awk -v` assignment escape-decodes its value: `b\.[a-z]+` arrived as
`b.[a-z]+`, and the unescaped dot matched any mnemonic starting with `b`.
FIXED via ENVIRON transport (scripts/audit_branchless.sh:62-79; retired as
`REGEX_TRANSPORT_FP` in `scripts/branchless_manifest.toml`).
**Fix.** Nothing to do — fixed. If you add a new gate script, pass backslashed regexes
through ENVIRON, never `awk -v`. Receipt:
`docs/jira/v26.10.31/gate-branchless-manifest.md`.

## 4. objdump refuses rlibs / `<ltmp0>` symbols

**Symptom.** `objdump -d` refuses an rlib or returns no text sections for
workspace-built artifacts. With Apple objdump, kernel symbols render as `<ltmp0>` and
symbol-scoped matching sees nothing to scope.
**Cause.** The workspace release profile emits bitcode (`lto = "thin"`,
`codegen-units = 1`; Cargo.toml:24-27). Apple objdump's symbolization is also not
useful for scoped kernel sites.
**Fix.** Build the disassembly subject with `--profile audit` (`lto = false`,
`codegen-units = 16`, `strip = false`; Cargo.toml [profile.audit] at :32-37) —
`make gate1` does this (Makefile:60-64). For symbol scoping, use the Homebrew LLVM
disassembler instead of Apple objdump:
`/opt/homebrew/opt/llvm/bin/llvm-objdump` (witnessed: real symbol names, no `<ltmp0>`).
Kernel-site ledger: `docs/jira/v26.10.31/27-branchless-kernel-abi.md`.

## 5. Timing gates flap under load: SKIP-NOISY

**Symptom.** Gage/ctq timing gates exit 2 SKIP-NOISY, or CV swings between identical runs.
**Cause.** 20-lane contention plus ordinary machine noise; timing verdicts are
load-sensitive by construction.
**Fix.** None needed — perf_guard's SKIP-NOISY (exit 2 when gage CV > 2%) is the
designed behavior (scripts/perf_guard.sh:86). Run timing gates when the machine is
quiet. `taskpolicy -b` makes it WORSE (anti-repair: background batching adds
contention, it does not remove it). Study:
`docs/jira/v26.10.31/49-ctq04-variance-study.md`.

## 6. cargo test -p bench --lib exits 101

**Symptom.** `cargo test -p bench --lib` exits 101 with "no library targets".
**Cause.** The bench crate has no `src/lib.rs` — bins only (`ctq_report`, `gage_rr`, ...).
**Fix.** Not an error; bins-only is by design. Run the binary directly
(`cargo run --release -p bench --bin ctq_report`) or use `just bench` (justfile:27).
Census row: `docs/jira/v26.10.31/63-test-census.md` ("bench (lib) | no lib target").

## 7. cargo build -p wasm-footprint: no match

**Symptom.** `cargo build -p wasm-footprint` from the repo root errors with
"package ID specification `wasm-footprint` did not match any packages".
**Cause.** `crates/wasm-footprint` is a standalone manifest, deliberately not a
root-workspace member.
**Fix.** Build in place — `cd crates/wasm-footprint && cargo build` — or pass
`--manifest-path crates/wasm-footprint/Cargo.toml` from anywhere. Witness:
`docs/jira/v26.10.31/69-validation-receipt.md` (V13 note); census method:
`docs/jira/v26.10.31/63-test-census.md`.

## 8. fsck_court inflate errors: stored body

**Symptom.** The avec-git `fsck_court` fails with inflate errors on pack bodies.
**Cause.** The pack writer emits zlib (RFC 1950) bodies as of wave 4; a stored-body
expectation predates the deflate landing.
**Fix.** Stored-body errors mean you are on a pre-deflate tree — update the tree; do
not weaken the court. Wave-4 landing: `docs/jira/v26.10.31/41-wave-4-receipt.md`
(deflate/inflate rows ALIVE); pack spec:
`docs/jira/v26.10.31/25-git-packfile-integration-spec.md`.

## 9. pyshacl missing: validate_ontology.sh refuses

**Symptom.** `scripts/validate_ontology.sh` exits 1 with "pyshacl/rdflib unavailable".
**Cause.** The SHACL engine is absent from the environment — no more than that; the
court refuses rather than skipping (fail-closed, scripts/validate_ontology.sh:17-18).
**Fix.**

```sh
pip3 install --user pyshacl rdflib
```

Ontology court: `docs/jira/v26.10.31/19-ontology-ttl.md`; witness of pyshacl rejecting
a real mutant: `docs/jira/v26.10.31/52-courts-audit.md`.

## See Also

- `docs/jira/v26.10.31/63-test-census.md` — census method: sum ALL `test result:` lines
- `Makefile`, `scripts/verify_v26.10.31.sh` — canonical gate invocations
- `justfile` — one-command recipes (`just verify`, `just courts`, `just doctor`)

## 10. Verify transiently FAILs under concurrent lanes

**Symptom**: `scripts/verify_v26.10.31.sh` reports FAIL on a gate that is
green minutes later; `git status` shows other work in flight.

**Cause**: `scripts/verify_baseline.txt` is 0 lines by design on a green
tree, so it cannot absorb another lane's in-flight edit signatures — a
mid-write compile error classifies as FAIL (introduced), not
PRE-EXISTING-FAIL.

**Fix**: re-run after the concurrent lane lands (the wave-18 DX audit
measured a transient FAIL self-healing in ~4 minutes). For adjudication
runs, wait for a quiet tree; the gage/CTQ-04 SKIP-NOISY gate is the
timing-side analogue of the same discipline.

## 11. Verify ladder transient FAIL on test-avec-beam

**Symptom**: the ladder's beam gate fails (exit 101, bridge-contract test
FAIL) while `cargo test -p avec-beam` passes in isolation minutes later.

**Cause**: the bridge-contract test stages the just-built dylib into the
escript temp dir; under concurrent cargo activity on the shared tree the
staging window can race, and one witness run reads a stale binary.

**Fix**: re-run the ladder once the tree is quiet — the failure is
self-healing (witnessed twice: fail → pass on the next run, same HEAD).
For adjudication runs, hold the tree quiet; the gage precondition is the
timing-side analogue.
