# Hygiene Audit: v26.10.31 (Wave-10 Lane 17)

Repository `/Users/sac/avec` at HEAD `ead4032`, 2026-10-30 22:24 PDT, wave-10 DX/QoL
lane 17 of 20. Scope: classify every untracked path, extend `.gitignore` with
evidence-backed patterns only, simulate clean-clone residue. Lane-owned files:
`.gitignore` (extend-only) and this document. Read-only git only; no worktrees.

## Method

Real git output only, this session, this tree:

- `git status --porcelain | grep '^??'` — 51 untracked entries pre-change, 50
  post-change (the delta is `err.log`, now ignored).
- `git check-ignore -v` on every candidate pattern and scratch path.
- `git clean -nd` dry run (read-only) — cross-check that status and clean agree
  on what `git clean -fd` would hit.
- Size/type sweep: `du`, `find -size +512k`, `file` over all untracked paths.
  No file over 512 KB; no binaries; no `*.rlib`/core/nohup strays; largest
  aggregate is `docs/jira/v26.10.31/` at 952 KB of text.

## Classification (50 post-change entries; 51 at 22:26 re-read, see drift note)

**All entries remaining in status are intentional-milestone: they SHOULD be
committed, never ignored.** Zero build-artifact or scratch entries remain in
status. Counts are as-of first census (22:24 PDT); one new lane output
(`scripts/receipt_doctor.sh`) landed during the audit — classified on the same
intentional-milestone terms, added to the scripts group (12 → 13).

| Group | Count | Where |
|---|---|---|
| New crate sources + tests | 30 | `crates/**` (breakdown below) |
| Milestone docs + receipts | 4 | `docs/**`, wasm-footprint `NOTES-v26.10.31.md` |
| Ops scripts + fixtures | 13 | `scripts/*`, `justfile` |
| In-flight, other lanes | 4 | `.github/`, `examples/`, `scripts/dod.sh`, docs pair |
| Untrackable empty dir | 1 | `benches/` (repo root) |

Crate-group breakdown (30 entries):

- Full new crates: `crates/avec-blake3/`, `crates/avec-git/`.
- avec-arrow src: `layout.rs`, `predicates.rs`, `transpose.rs`.
- avec-beam src: `alloc_spy.rs`, `dtor_stats.rs`, `ops_persist.rs`,
  `ops_scan.rs`, `pointer.rs`; plus `crates/avec-beam/tests/`.
- avec-ffi: `tests/alloc_courts.rs`, `tests/zero_copy.rs`.
- avec-parquet src: `level_free.rs`, `scan.rs`; plus 4 court tests under
  `crates/avec-parquet/tests/`.
- bench: `benches/v26_10_31.rs`, `src/benches/v26_10_31.rs`, and 6 new
  `src/bin/` programs (`blake3_profile`, `blake3_throughput`, `cpk_study`,
  `ctq_report`, `gage_rr`, `parquet_throughput`).
- cli: `src/bin/verify_v26.rs`; integration: `tests/admission_e2e.rs`.

Doc-group detail (4 entries):

- `docs/jira/v26.10.31/**` — 82 files incl. `receipts/` (minted gate receipts).
- `docs/diataxis/**` — 5 files (tutorials/how-to/reference/explanation/INDEX).
- `docs/GETTING_STARTED.md`, `docs/TROUBLESHOOTING.md` (docs lane).
- `crates/wasm-footprint/NOTES-v26.10.31.md`.

Scripts-group detail (13 entries): `audit_branchless.sh`,
`audit_branchless_cases/*.c` (2 fixtures), `branchless_manifest.toml`,
`cfg_homology.sh`, `cfg_homology_cases/*.c` (2 fixtures), `divan2receipt.py`,
`mint_receipt.sh`, `perf_guard.sh`, `receipt_doctor.sh` (landed during audit),
`validate_ontology.sh`, `verify_baseline.txt`, `verify_v26.10.31.sh`, plus
`justfile` at root. `scripts/dod.sh` counted with the in-flight group.

In-flight detail (4 entries, other lanes, classified untouched):
`.github/workflows/ci.yml`, `examples/sovereign-e2e/` (with own `Cargo.lock` —
intentional pin), `scripts/dod.sh`, and one of the two docs files above.

Empty-dir note: root `benches/` is empty; git cannot track empty dirs, status
never shows it, `git clean -nd` lists it, and a fresh clone will not have it.

Notes on individually-verified paths:

- `err.log` (repo root, 1068 bytes ASCII): cargo/rustc warning spillover from a
  build redirect. Scratch. Now ignored (see diff below). Content was pure
  warnings, nothing to preserve.
- `scripts/verify_baseline.txt` (0 bytes): **empty by design, commit it.**
  `scripts/verify_v26.10.31.sh:88-89` writes the empty file when the workspace
  check PASSES ("empty baseline written"); line 16 reads it at runtime. The
  empty state encodes "zero known failure signatures" — the healthy state. Do
  not ignore; do not treat as lost data.
- `examples/sovereign-e2e/Cargo.lock`: example crate with its own lockfile —
  intentional pin (examples run as standalone workspaces). Commit.
- `.claude/` (session state): contains only `.claude/scheduled_tasks.lock`,
  which is matched by `.git/info/exclude`
  (`**/.claude/scheduled_tasks.lock` — harness-managed, per-checkout file).
  The repo tracks zero `.claude` files; no repo `.gitignore` entry needed.
  Out of repo scope: a fresh clone re-derives info/exclude when the harness
  first runs there.

## .gitignore diff

```diff
--- a/.gitignore
+++ b/.gitignore
@@ -8,3 +8,6 @@ dist/
 *.pyc
 __pycache__/
+
+# Scratch: transient build/test spillover (hygiene audit v26.10.31, lane 17)
+/err.log
```

Patterns considered and REJECTED (no observed referent — an ignore pattern with
no referent carries no bits and risks masking intentional files):

- `**/target/` — redundant: existing `target/` (no leading slash) already
  matches at any depth. Verified: `git check-ignore -v crates/wasm-footprint/target`
  reports `.gitignore:1:target/`.
- `*.wasm` — zero `.wasm` files exist outside `target/` dirs (find sweep).
  Generated wasm lives under `crates/wasm-footprint/target/`, already covered.
- Perf-output patterns (`perf.data`, `*.out`) — every script writes only to
  `mktemp` dirs and `/tmp` (grep over `scripts/`, `justfile`, `Makefile`);
  nothing lands in-repo.
- `.claude/` — covered by harness-managed `.git/info/exclude`, not repo scope.
- `/tmp` artifact patterns — no in-repo referent.

## Clean-clone simulation

No clone was performed (read-only git constraint). Simulation: filter the
untracked set through `git check-ignore -v` with the updated `.gitignore`.

Verified matches (command exited 0, six match lines):

```
.gitignore:13:/err.log	err.log
.gitignore:1:target/	target
.gitignore:1:target/	crates/wasm-footprint/target
.gitignore:5:.DS_Store	.DS_Store
.gitignore:5:.DS_Store	crates/avec-parquet/.DS_Store
.gitignore:10:__pycache__/	scripts/__pycache__/x.pyc
```

Projection: once the coordinator commits the intentional entries (51 as of
22:26 PDT, still moving while the wave runs), a fresh
clone + full build regenerates only artifacts that every rule already matches:
`target/`, `crates/wasm-footprint/target/`, `.DS_Store`, `__pycache__/`, and
`err.log`-class spillover.

**Simulated-clone residue list (untracked ∧ unignored after integration): empty.**

Known variance: `git status` during the wave differs from the post-integration
tree — 4 entries are other lanes' in-flight outputs (listed above); they are
classified but intentionally untouched (disjoint file ownership). `git clean
-nd` additionally lists `benches/` (empty, untrackable) and would hit other
lanes' in-flight files; not run for real.

## Verification

Commands and exits, this session, this tree:

- `git check-ignore -v err.log target crates/wasm-footprint/target .DS_Store
  crates/avec-parquet/.DS_Store scripts/__pycache__/x.pyc` — exit 0, 6 matches.
- `git status --porcelain | grep '^??' | wc -l` — 50 (post-change).
- `git clean -nd` — lists only intentional entries + `benches/`.

Standing: ALIVE for every command above (ran this session, exits cited).
