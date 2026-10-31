# Wave 14 Receipt: Adversarial Hardening (Lanes 1-19 + Receipt Lane 20)

Wave 14 (2026-10-31) is the adversarial-hardening wave: 20 lanes over the
one canonical checkout at /Users/sac/avec (87-wave-12-13-receipt.md names
the wave: fuzz courts, mutation courts, SHACL negative corpus,
panic/air-tightness audits, flake register, falsifier register,
determinism court). Lane 20 = this receipt. Every existence claim below
was re-verified on disk at this edit (2026-10-31T15:39Z). The aggregate
gate was run by this lane at 2026-10-31T15:36:58Z — see the aggregate
section for commands and exits.

## Artifacts On Disk (per-lane table)

Every path was `ls`-verified at 15:37-15:39Z. Statuses come from disk
evidence only (this lane's own runs, or the lane's own doc/receipt).
"witnessed" = this lane executed the command and quotes its own exit.
Details in the bullets below the table.

| Lane | Artifact (mtime UTC) | Status |
|---|---|---|
| L1 | crates/avec-git/tests/fuzz_codec.rs (15:09Z) | LANDED + ALIVE (witnessed) |
| L2 | crates/avec-parquet/tests/offset_fuzz.rs (15:24Z) | LANDED + ALIVE (witnessed) |
| L3 | crates/avec-git/tests/ladder_mutations.rs (15:31Z) | LANDED + ALIVE (witnessed) |
| L4 | crates/avec-beam/tests/kill_stress.rs (15:28Z) | LANDED + BLOCKED (escalated) |
| L5 | crates/avec-blake3/tests/thread_differential.rs (14:59Z) | LANDED, verdict PENDING-READ |
| L6 | crates/avec-git/tests/packfile_fuzz.rs (15:07Z) | LANDED + ALIVE (witnessed) |
| L7 | crates/avec-git/tests/ladder_timing_stress.rs (15:10Z) | LANDED, verdict PENDING-READ |
| L11 (recovery) | crates/avec-git/tests/large_offset.rs (15:30Z) | LANDED + ALIVE (witnessed) |
| L16 | crates/integration/tests/error_paths.rs (15:23Z) | LANDED (lane-reported) |
| L9 | docs/jira/v26.10.31/78-panic-freedom-audit.md | LANDED |
| L12 | docs/jira/v26.10.31/79-doctest-coverage.md | LANDED |
| L13 (recovery) | docs/jira/v26.10.31/80-flake-register.md | LANDED |
| L16 | docs/jira/v26.10.31/81-error-path-coverage.md | LANDED |
| L17 | docs/jira/v26.10.31/82-dependency-audit.md | LANDED |
| L18 | docs/jira/v26.10.31/83-falsifier-register.md | LANDED |
| L14 | receipts/gate-shacl-negative-a.md (15:17Z) | LANDED + ALIVE |
| L15 (relaunch) | receipts/gate-shacl-negative-b.md (15:19Z) | LANDED + ALIVE |
| — | docs/jira/v26.10.31/77-*.md | NOT LANDED (absent 15:38Z) |
| — | docs/jira/v26.10.31/84-*.md | NOT LANDED (absent 15:38Z) |
| — | crates/avec-parquet/tests/codec_properties.rs | NOT LANDED (absent 15:39Z) |

Per-lane disk evidence:

- **L1 fuzz_codec** — witnessed: 4/4 pass, exit 0 (5.09s). 200
  round-trip corpora + 500 bit-flips + truncation sweep + non-vacuity
  court, deterministic fixed-seed LCG.
- **L2 offset_fuzz** — witnessed: 3/3 pass, exit 0 (0.04s).
  Footer/page-offset corruption sweep, 64-bit truncation boundaries,
  LCG corruption corpus on the mmap scanner.
- **L3 ladder_mutations** — witnessed: 11/11 pass, exit 0 (2.15s) at
  15:36Z. Systematic single-byte mutation court over idx/ladder; the
  earlier in-flight state was 10/12 (see Findings F1/F1b).
- **L4 kill_stress** — file present; NOT re-run by this lane.
  78-panic-freedom-audit.md witnesses it FAILING at in-flight HEAD:
  BEAM refuses the NIF (`bad_lib: Function not found
  avec_beam:avec_new_fast/2`). Owner avec-beam / lane 4 (escalation B1).
- **L5 thread_differential** — file present; suite not re-run by this
  lane; no lane doc/receipt found on disk → verdict PENDING-READ.
- **L6 packfile_fuzz** — witnessed: 2/2 pass, exit 0 (47.29s). 40-case
  accept matrix + 20 single-byte mutants, all vs real git subprocesses.
- **L7 ladder_timing_stress** — file present; suite not re-run by this
  lane; no lane doc/receipt found on disk → verdict PENDING-READ.
- **L11 large_offset (recovery)** — witnessed: 6/6 pass, exit 0 at
  15:36Z. Bit-31 escape court; earlier in-flight state was 4/6
  (Findings F2a/F2b).
- **L16 error_paths** — lane-reported: 8/8 pass, exit 0
  (81-error-path-coverage.md). Not re-run by this lane. Landed under
  crates/integration, not avec-parquet (dispatch's `crates/*/tests/`
  glob admits this).
- **L9 78-panic-freedom-audit** — 287 keyword sites / 40 files swept;
  2 production fixes + 1 disclosed minimal unblock; 4 LISTED findings;
  pre-existing beam failure escalated.
- **L12 79-doctest-coverage** — verdict: zero doctests exist anywhere
  in the workspace (8 crates).
- **L13 80-flake-register (recovery)** — first clean-tree flake
  measurement: 60/60 runs, 0 flakes witnessed, exit 0
  (15:20:03Z-15:24:44Z), truth = process exit code, no sleeps.
- **L16 81-error-path-coverage** — typed-error coverage sweep:
  error_paths 8/8, `cargo test -p avec-integration` 20/0,
  COVERED/NEWLY-COVERED register.
- **L17 82-dependency-audit** — zero vulnerabilities, zero runtime
  external dependencies; gap: no `deny.toml` (cargo deny licenses fails
  on an empty allowlist).
- **L18 83-falsifier-register** — standing register of every
  refuted/retired/superseded performance-design claim.
- **L14 gate-shacl-negative-a** + scripts/shacl_negative_corpus.py
  (15:16Z) — receipt exit 0, 7/7 cases FLIPPED, self-check PASS.
- **L15 gate-shacl-negative-b (relaunch)** +
  scripts/shacl_negative_corpus_b.py (15:18Z) — receipt exit 0, ODRL
  tiling negative corpus, replay command in the receipt.

Landing timeline (poll witnessed, 15s/20s cadence, no git commands): the
dispatch named 10 test files, docs 77-84, and receipts gate-shacl-negative-a/b.
At receipt time 9/10 named test files are on disk (codec_properties
absent; error_paths landed at crates/integration instead of avec-parquet),
6/8 named docs (77 and 84 absent), 2/2 receipts.

## Aggregate Run (receipt gate, this lane, 2026-10-31T15:36:58Z)

```text
cargo test -p avec-git --test fuzz_codec --test ladder_mutations \
  --test large_offset --test packfile_fuzz
  -> fuzz_codec 4/4, ladder_mutations 11/11, large_offset 6/6,
     packfile_fuzz 2/2 = 23 passed / 0 failed, exit 0
cargo test -p avec-parquet --features std --test offset_fuzz
  -> 3 passed / 0 failed, exit 0
```

Aggregate total: 26/26 PASS, both crates exit 0.

Loud skip: `--test codec_properties` was NOT run — the file does not
exist (crates/avec-parquet/tests/codec_properties.rs absent at
15:36:58Z); running it would have been a compile error, not a pass.

## Findings Summary (wave-15 blocker ledger)

This lane ran the aggregate twice. The first pass (~15:29-15:31Z, against
in-flight lane edits) was red; the second pass at 15:36:58Z, after the
owning lanes landed fixes, is green. Both are recorded because the red
runs are the courts proving non-vacuity.

- **F1** (typed-refusal divergence) — ladder_mutations.rs:350: wrapped
  u32 partition returned `Err(Truncated)` where the court wanted
  `Err(TooManyObjects)`. RESOLVED in-flight: lane 9's idx_read.rs Fix 1
  (`checked_sub` -> `Truncated`) and lane 3's merged test
  `fanout_inflation_window_and_inversion_typed_refusals` now pass. The
  adjudicated contract: typed `Truncated` on non-monotonic fanout.
- **F1b** (silent-accept of corruption) — ladder_mutations.rs:444: an
  OID byte flip under the opt-in checksum returned `Ok(())` where the
  court wanted `IdxChecksumMismatch`. RESOLVED in-flight:
  `oid_byte_corruption_never_false_hits` passes (11/11 at 15:36Z).
- **F2a** (silent wrong value) — large_offset.rs:283: corrupt MSB on a
  small offset entry returned `offset()` = `Ok(0)` where the court
  wanted a typed error. RESOLVED in-flight: case4 passes at 15:36Z.
- **F2b** (silent wrong value) — large_offset.rs:155: escape offset
  >= 2^32 resolved as 0 where the court wanted 4294967296. RESOLVED
  in-flight: case6 passes at 15:36:58Z.

Final aggregate state: ZERO open PANIC or silent-wrong-bytes findings
from this lane's runs.

### Escalations from lanes' own reports (disk-visible)

- **B1 (BLOCKED, pre-existing)** — crates/avec-beam/tests/kill_stress.rs
  fails at in-flight HEAD: BEAM refuses the NIF
  (`bad_lib: Function not found avec_beam:avec_new_fast/2`;
  `make test-beam` fails the same way on `pointer_of/1`). The compiled
  export table in the dylib is correct (21/21 entries), source/escript
  stub surface matches 21/21, NIF version constants match — the failure
  survives fresh rebuilds and is spawn-context-dependent (cargo-spawned
  erl gets past dlopen; direct-shell erl fails at dlopen). Owner:
  avec-beam / lane 4. Escalated in 78-panic-freedom-audit.md; wave-15
  blocker candidate.
- **B2** — 78's 4 LISTED (not fixed) findings: avec-arrow varlen.rs:28
  `Offset::from_usize` panics on >4 GiB u32 offset; avec-arrow views.rs
  `row(i)` index-panic contract; avec-parquet scan.rs:877,883
  `write_fixture_file` asserts on caller shapes; avec-arrow transpose.rs
  `untranspose` cross-column invariants debug_assert-only with pub fields
  (release-UB class). Recommend work orders.
- **B3** — 82-dependency-audit.md: no `deny.toml` (cargo deny check
  licenses fails on an empty allowlist). Config gap, not a vuln.
- **B4** — 79-doctest-coverage.md: zero doctests workspace-wide; lane
  proposed 5 doctests for highest-value public APIs.
- **B5** — gate-shacl-negative-a.md: N-07 code-mapping divergence
  (suite-table REFUSAL-07 -> artifact REFUSAL-09) disclosed; and the
  pyshacl 0.31.0 probe CONTRADICTS the 30-shacl-validation-suite.md
  vacuity note (sh:hasValue DOES violate on absent property — the
  mutation nodes are fully populated). 30-shacl note needs correction in
  wave 15.

## What Wave 14 Adds to the Milestone

1. **Mutation-survival proof for the codecs/index/scanner.** The git
   codec now carries a deterministic 500-bit-flip + truncation + round-trip
   court (L1) and a real-git-verified 40-accept/20-mutant packfile court
   (L6); the .idx ladder/index carries a systematic single-byte mutation
   court (L3) and a bit-31 escape court (L11); the parquet scanner carries
   an mmap-scan offset fuzz (L2); the beam bridge a 100-round
   create/kill/verify court (L4, currently BLOCKED at NIF load — court
   exists, runtime edge BLOCKED); blake3 a multi-thread differential
   court (L5); the ladder a timing-stress court (L7). Every court is
   deterministic by construction (fixed-seed LCG, no wall clock, no
   external fuzzer) so any future finding replays byte-for-byte.
   Aggregate 26/26 PASS at 15:36:58Z.
2. **Panic-freedom + air-tightness audits.** 78 swept 287 panic-surface
   sites across 40 files, classed every site (OK / OK-WITNESSED /
   OK-CONTRACT / FINDING), landed 2 production fixes (idx_read.rs typed
   refusal of non-monotonic fanout; pack_write.rs typed Truncated on
   malformed varint) plus 1 disclosed minimal unblock to another lane's
   file, and listed 4 non-trivial findings for work orders. 82 proved the
   supply chain air-tight: zero vulnerabilities, zero runtime external
   dependencies (24 external crates trace to two non-runtime roots:
   divan dev-dep, cc build-dep).
3. **Flake register.** First clean-tree flake measurement (80): 60 runs /
   60 suites in 4m41s, 0 flakes, truth = process exit code, no sleeps.
4. **Falsifier register (83)**: every refuted/retired/superseded
   performance-design claim now has a standing register entry so dead
   claims stop circulating as live.
5. **SHACL negative corpus.** Two minted receipts (a: 7/7 refusal-code
   mutations FLIPPED + self-check; b: ODRL tiling corpus) make the SHACL
   layer's refusal surface adversarially tested — the anti-vacuity court
   now has a negative corpus, not just a positive-control court.
6. **Net milestone effect**: the storage plane's codecs/index/scanner
   move from "tests pass" to "adversarial courts survived"; two new
   typed-refusal contracts are adjudicated and encoded in tests (F1,
   F2a); and the wave surfaced exactly one BLOCKED runtime edge (beam
   NIF load) plus a short, receipted escalation ledger (B1-B5) for
   wave 15.

## Wave 14 Standing

v26.10.31 adversarial-hardening row: PARTIAL_ALIVE — 9/10 named test
files landed (codec_properties NOT LANDED), 6/8 named docs landed (77,
84 NOT LANDED), 2/2 SHACL receipts ALIVE; aggregate 26/26 PASS exit 0 at
15:36:58Z; zero open fuzz findings at receipt time; 1 BLOCKED edge
(beam NIF) escalated; 3 artifacts pending (77, 84, codec_properties).
Nothing committed by this lane (NEW FILES ONLY, no git commands).

## Verification Performed by This Lane

- Ran (exit codes real, `set -o pipefail`):
  `cargo test -p avec-git --test fuzz_codec` 4/4 exit 0;
  `cargo test -p avec-git --test packfile_fuzz` 2/2 exit 0;
  `cargo test -p avec-git --test ladder_mutations` (10/12 exit 101,
  re-run 11/11 exit 0); `cargo test -p avec-git --test large_offset`
  (4/6 exit 101, re-run 6/6 exit 0); `cargo test -p avec-parquet
  --features std --test offset_fuzz` 3/3 exit 0; canonical aggregate
  23/23 + 3/3 exit 0 at 15:36:58Z.
- Not re-run by this lane (lane-reported or file-presence only):
  kill_stress, thread_differential, ladder_timing_stress, error_paths.
- `ls`-verified every path in the artifact table at 15:37-15:39Z; loud
  absences: codec_properties.rs, 77-*.md, 84-*.md.
- Poll timeline kept (15s/20s cadence ~45 min) as landing-timeline
  evidence; two background polls hit their time caps and were
  restarted/dropped (disclosed).
- NOT done by this lane: no git commands (lane contract), no edits to
  any existing file, no reruns of lanes' benches/audits.

## See Also

- 87-wave-12-13-receipt.md (dispatches wave 14; names this receipt
  "in flight") · 78-panic-freedom-audit.md (B1/B2 source) ·
  80-flake-register.md · 81-error-path-coverage.md ·
  82-dependency-audit.md (B3) · 79-doctest-coverage.md (B4) ·
  83-falsifier-register.md · receipts/gate-shacl-negative-a.md ·
  receipts/gate-shacl-negative-b.md
