# Operator Decisions

The standing page of what only the operator (Sean) can decide. Everything else in
this repo is lane-dispatchable: a lane edits, gates kill or pass, receipts record.
These three decisions are withheld from lanes by construction — each flips a
spec-level acceptance limit, a standing transition, or tree-integration authority.

Verified on disk 2026-10-30 at wave-10 relaunch. Line numbers were re-checked
against the working tree before this page was written; where a cited reference in
the source package disagrees with disk, disk wins and the drift is named.

## Why these are operator-only

- Amendment authority lives with the operator by written law in the gate itself:
  `crates/bench/src/bin/ctq_report.rs:43` — "Do not flip from a lane; this is
  operator-owned."
- A ratified LSL is a spec-level acceptance limit (20-prd.md CTQ table). Lanes
  measure; the operator decides what the limit means. The wave-7 no-silent-
  relaxation law keeps the gate REAL and FAILing until the flip lands.
- Commit authority over the shared tree is coordinator/operator territory:
  117 dirty paths (re-read 2026-10-30) spanning 16+ crates/scripts/docs cannot
  be integrated by concurrent lanes without serializing ownership.

## Decision 1 — RATIFY CTQ-06: BLAKE3 throughput LSL re-derivation

### What

Approve the amendment and flip the CTQ-06 limit from 8.5 GiB/s/core to
LSL 1.0, target 1.5, ceiling-class USL 2.5 (all per-core, NEON).

- Refutation is receipted: 48-lsl-rederivation.md §3 verdict — "The 8.5 LSL as
  written is unsatisfiable; it is a spec error, not an implementation gap."
- The process is stable at the ceiling the hardware actually offers: 1.85
  GiB/s/core, gate-cpk-study-v2 (n=100, mean 1.846241, sigma_overall 0.010571,
  CV 0.57%) — sigma ~= 0.01, so the failure is the limit, not the process.
- Post-flip prediction (arithmetic on the receipted distribution, §2.4 of the
  package): C_pl = (1.846241 - 1.0) / (3 x 0.010571) = 26.7 vs required
  C_pk >= 1.33.

### Why operator-only

The flip constant is operator-gated in source (`ctq_report.rs:43` verbatim, cited
above). While `LSL_RATIFIED` is `None`, the gate honestly FAILs at 8.5; flipping
it is the act of ratification itself, not a code change a lane may ship.

### Exact mechanics (edit set A/A2/C/D/E/F, 59 §6 — verified on disk)

- **A — the flip** at `crates/bench/src/bin/ctq_report.rs:45`:
  `const LSL_RATIFIED: Option<f64> = None;` -> `Some(1.0)`.
  Drift note: package §6 and the file's own doc-comment at `:13` both say
  line 44; on disk the const is at line 45. Fix both stale refs in the same pass.
- **A2 — row scoping, REQUIRED.** Required because the flip alone cannot PASS:
  `all_pass` folds both CTQ-06 rows (loop at `ctq_report.rs:571`), and the legacy
  `hash_wide_simd` row measures 0.43 GiB/s (receipted: gate-blake3-throughput;
  58-final-verdict-v2 §3) — it fails any LSL above 0.43.
  Proof: `/tmp/ctq_dryrun/dryrun_stdout_run2.json` is the post-flip dry run —
  LSL 1.0, AMENDMENT-RATIFIED markers present, `hash_wide_simd` still
  `"gate":"REAL","verdict":"FAIL"` at 0.43, overall `verdict: FAIL`,
  `failed_gates: ["CTQ-06"]`.
  Option 1 (recommended): keep the production row (`hash_many_simd`) gated
  against `LSL_RATIFIED`; demote `hash_wide_simd` to
  `"gate":"informational","verdict":"REPORT"`, excluded from `all_pass`,
  mirroring the CTQ-03 row pattern. Production is what CTQ-05 rides.
  Option 2: keep both rows gated — CTQ-06 stays FAIL (exit 1) forever; no PASS
  receipt obtainable.
- **C — `crates/bench/src/bin/cpk_study.rs`** (all sites verified on disk):
  `:303` `const LSL_GIB_S: f64 = 8.5;` -> `1.0`; `:300` log string; `:297` doc
  comment; `:341` `emit(..., Some(8.5), ...)`. Without C, ladder step 4 cannot
  pass — the study would keep evaluating C_pl against 8.5.
  Drift note: package §6 cites 300/297/338 for these four sites; disk has all
  three anchors shifted +3 (303/300/341, with 297 the doc comment, not the log).
- **D — `crates/bench/src/bin/blake3_throughput.rs`**: `:10` const 8.5 -> `1.0`;
  `:4` doc comment. The `--gate` surface must ride the same amendment or the
  repo carries two contradictory CTQ-06 gates.
- **E — `docs/jira/v26.10.31/20-prd.md:73`** CTQ table row: label becomes
  "BLAKE3 throughput (GiB/s/core, NEON)", LSL 1.0, target 1.5, USL 2.5
  (USL moves off "Memory-controller max" to the ceiling-class 2.5).
- **F — `docs/jira/v26.10.31/20-prd.md:101-102`** amendment Status:
  PROPOSED -> RATIFIED `<date>` by operator on the §5 receipt ladder
  (gate-ctq-report-v3/v4/v5, gate-cpk-study-v3, gate-gage-rr).
  Drift note: package §6 cites prd:98 for this line; on disk :98 is blank —
  the Status line is at :101 (wrapping to :102).
- Not in scope (flagged in §6): FR-06 row at `20-prd.md:59` — separate follow-up
  amendment.
- Scope boundary (59 §7): correctness surfaces, CTQ-02 zero-alloc, and CTQ-05
  are independent of this flip; the gate stays REAL either way; the Metal/GPU
  path stays open.

### Post-decision verification (59 §5 ladder — order is load-bearing)

1. Quiet-gage precondition: `cargo build --release -p bench`, run
   `target/release/gage_rr`; cv_trimmed < 1% required. Mint the receipt.
2. Apply edit A (+A2), rebuild `-p bench`.
3. 3 consecutive `ctq_report` PASS runs, each minting
   `docs/jira/v26.10.31/receipts/gate-ctq-report-vN.md` — verdict PASS, exit 0,
   `"amendment":"AMENDMENT-RATIFIED"` present, mini-gage admits each run.
4. 1 `cpk_study` run (n >= 30; bin runs n=100), C_pk >= 1.33 vs amended LSL;
   predicted C_pl ~= 26.7.
5. Only then the docs move: edits E + F, PROPOSED -> RATIFIED.

If any ladder run fails: amendment stays PROPOSED, flip reverts to `None`,
failure receipted.

## Decision 2 — RATIFY CTQ-04: declare p99/p50 <= 1.30

### What

Promote the CTQ-04 latency-variance pin from PROVISIONAL to RATIFIED: the gated
statistic is p99/p50 <= 1.30 (nearest-rank percentiles over 100 timed runs), and
the 15% stdev/mean pin stays retired with provenance
(49-ctq04-variance-study.md).

### Why operator-only

Same amendment-authority law as Decision 1: a pin retirement plus a protocol
declaration is a spec-level acceptance statement. The pin_status string is the
receipt surface — lanes must not author RATIFIED text into gates.

### Exact mechanics (edit B, plus §6 companion doc stamps)

- **B — `crates/bench/src/bin/ctq_report.rs:273,314`** (both verified on disk):
  the two `format!` sites that emit `pin_status` in the CTQ-04 JSON. Replace
  `PROVISIONAL p99/p50 <= 1.30 (49-ctq04-variance-study.md Option b)` with
  `RATIFIED p99/p50 <= 1.30 (... ratified <date> by operator)` — 2 occurrences,
  one per branch (SKIP-NOISY and main verdict).
- Companion doc stamps from 59 §6, bundle in the same pass: **G** — header stamp
  in `docs/jira/v26.10.31/49-ctq04-variance-study.md` under Status; **H** —
  append to `docs/jira/v26.10.31/29-benchmark-harness-plan.md` §"CTQ-04
  superseded": protocol ratified `<date>`; 49 remains the live metric source.
- Scope boundary: ratifying now does not close the designated successor (ratio
  fence vs sleep-free baseline, 29 §CTQ-04 primary).

### Post-decision verification

1. Quiet machine, re-run `ctq_report` once: CTQ-04 row verdict PASS,
   `pin_status` carries RATIFIED, JSON `"retired_pin":0.15` provenance intact.
2. Mint the receipt (gate-ctq-report-vN+1) — this is the declaration's receipt.
3. Confirm the SKIP-NOISY branch string also reads RATIFIED (dry run or grep).

## Decision 3 — APPROVE COMMIT: wave integration of the dirty tree

### What

Authorize the integration commit sequence for the current wave: 117 dirty paths
(count re-read 2026-10-30; dispatch said ~70+, disk says 117) across
crates/scripts/docs, landed as atomic per-crate commits plus scripts and docs.

### Why operator-only

Tree-integration authority over the shared canonical checkout. Lanes own files;
only the operator/coordinator owns git transitions (same-checkout fan-out law).
No push, no rebase: project law is NEVER REBASE, NEVER FORCE PUSH, ONLY MERGE.

### Exact mechanics

- Stage and commit in atomic units, largest ownership first:
  crates/audit -> avec-arrow -> avec-parquet -> avec-beam -> bench ->
  remaining crates (cli, avec-ffi, avec-git, avec-blake3, integration,
  wasm-footprint) -> scripts/ -> docs/ -> root manifests (Cargo.toml,
  Cargo.lock, Makefile, README, .gitignore) last so manifest and consumer
  commits stay coherent.
- Commit messages via `git commit -F <file>`, never inline `-m` (multi-line
  corruption precedent, global tools rule).
- Per commit: build or test the committed unit before moving on; a red unit
  blocks its commit, not the sequence.
- No push. No rebase. Merge-only forward fixes.

### Post-decision verification

1. `git status` clean after the sequence; one atomic commit per unit above.
2. Rebuild + `ctq_report` on the integrated head: ladder green.
3. Receipts re-minted against the post-commit head
   (docs/jira/v26.10.31/receipts/).
4. `scripts/dod.sh` exit 0 — no red (PENDING/ADV-SKIP yellow allowed);
   `scripts/dod.sh --selftest` proves the checker still flips (anti-vacuity).

## Post-decision state (all three decisions landed)

- CTQ-06: gate REAL at LSL 1.0, hash_many_simd PASS, hash_wide_simd
  informational; 3 PASS receipts + cpk_study receipt minted.
- CTQ-04: pin_status RATIFIED in both branches; quiet re-run receipt minted.
- Tree: integrated as atomic commits; working tree clean; no push, no rebase.
- dod.sh green; receipts re-minted on the integrated head.

## See Also

- docs/jira/v26.10.31/59-ratification-package.md (§5 ladder, §6 edits, §7 scope)
- docs/jira/v26.10.31/20-prd.md (CTQ table, Amendment section)
- docs/jira/v26.10.31/48-lsl-rederivation.md (ceiling math, refutation)
- docs/jira/v26.10.31/49-ctq04-variance-study.md (p99/p50 protocol, pin retirement)
- docs/jira/v26.10.31/receipts/ (gate receipts)
- scripts/dod.sh (Definition-of-Done checker)
