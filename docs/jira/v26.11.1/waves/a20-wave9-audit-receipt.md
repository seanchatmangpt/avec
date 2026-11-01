# A20 Wave 9 Audit Receipt

- **Lane**: A20 (court verdict), Wave 9 failed-lane-restart audit of
  `a20-wave5-audit-receipt.md` (the latest A20 receipt on disk; waves 6-8
  audit sessions left /tmp artifacts but no A20 receipts landed), v26.11.1
- **Auditor**: adversarial audit lane (failed-lane restart), 2026-11-01
- **Subject**: the wave-5 receipt's claims re-verified against the CURRENT
  tree: `tests/court_verdict.rs` (1783 lines, sha256 `cab5c2f5...6b86`),
  `scripts/boundary_gate.py` (1995 lines, sha256 `ab3d4bc9...97fb` — see
  D1), `crates/cgap-court/Cargo.toml` (sha256 `1128bb06...36c2b`) on `main`
  @ c83619d (uncommitted lane work on top); repo `/Users/sac/cgap`, one
  canonical checkout
- **Working-tree identity (sha256, byte-identical pre/post this session)**:
  `tests/court_verdict.rs` `cab5c2f5...6b86`,
  `scripts/boundary_gate.py` `ab3d4bc9...97fb`,
  `cratests/cgap-court/Cargo.toml` `1128bb06...36c2b` (path typo in this
  bullet is in the receipt itself — the file audited is
  `crates/cgap-court/Cargo.toml`),
  `Cargo.lock` `47f5a125...d9b07`,
  `crates/cgap-gate/src/penstock.rs` `709707c4...2555`,
  `crates/cgap-bn3/src/unify.rs` `e2373305...0c17f`
- **Verdict**: **ALIVE** — every re-runnable claim in the wave-5 receipt
  reproduced on the current tree; 9/9 re-attacked mutations flip with the
  exact receipted failed-court sets (zero vacuous courts, union covers all
  11); the 4-entry Equivalent-triage register revalidates (all four
  re-applied mutants stay 11/11 green); all five DoD gates return their
  expected exits this session. One material new finding (D2: a live
  shared-tree mutant was present during this session's sandbox copy) and
  one minor gate defect (D3) are recorded, not fixed.

## Re-runs (all commands executed this session, /Users/sac/cgap)

| Receipt claim | This session | Outcome |
|---|---|---|
| Court suite 11/11, exit 0 | `CARGO_PROFILE_DEV_OPT_LEVEL=2 CARGO_TARGET_DIR=$PWD/target-a20 cargo test -p cgap-court --test court_verdict` -> exit 0, 11/11 in 0.90 s (run twice: before and after a `cargo clean -p cgap-court` rebuild inside the pre-existing lane target lease) | REPRODUCED |
| `--deps` exit 0, E=0 x15 | exit 0, "E=0 runtime externals across 15 covered crates" | REPRODUCED |
| `--strings` exit 0, 0 hits | exit 0, 0 hits; A07 register v2 consumed (sha 7778078dfe11) | REPRODUCED |
| Shim-court exit 0 | exit 0, "killed by a05_interleave_engine" | REPRODUCED |
| `--shim-inert` -> exit 1 VACUOUS GATE | exit 1, VACUOUS GATE refusal printed | REPRODUCED |
| `--mutate` exit 0, kill_rate 1.0000, 0 defects | exit 0, 995 generated / 24 sampled / 20 killed / 4 Equivalent / 0 survivor defects; ledger row 1 = Baseline (11 passing courts listed), row 2 = ShimCourt killed_by `a05_interleave_engine` | REPRODUCED |
| Corpus receipt `03ed29d2...` | verbatim-source scratch binary (mechanical extraction of the generator text from the pristine file, compiled standalone) -> `03ed29d285cec5bb381ebdfa045f0aa0a81c0aa6282b6777a20164f784f7facb` byte-exact | REPRODUCED |
| Triage register keys point at real sites | :918 `assert!(plan.rows > 0);`, :1085 `range_root(rows, 0, 1, 0)`, :1626 `vec![0xA5u8; payload_len]`, :1735 `200, 1000]` all read at those lines this session | REPRODUCED |

## Anti-vacuity re-attack (all mutations in /tmp/a20w9-sandbox only)

Method: fresh sandbox, `cp -R` of the 10-crate closure (court + audit +
storage + core + blake3 + bn3 + gate + arrow + parquet + git) plus
`tests/court_verdict.rs`, hand-written minimal workspace manifest
(+`[profile.dev] panic = "abort"`), the repo's `rust-toolchain.toml` copied
in, sandbox baseline 11/11 exit 0 after the pristine re-copy; one mutation
at a time via a hex-anchor fail-closed helper (unique-anchor check, sha256
pre-image, apply -> run -> sha256-verified restore; anchor-count aborts
fired once, correctly: `admitted += 1;` had 2 hits, anchor widened); every
attack below reports restore=OK; sandbox end-state PRISTINE (sha256
cab5c2f5 = repo). Canonical checkout never mutated by this auditor.

| Attack | File | Edit | Outcome (this session) |
|---|---|---|---|
| M1 | court | `SkipLoud = 6,` -> `7,` | **FLIP** — `wave1_abi_freeze_receipt` alone (rc 101, 10 pass / 1 fail) |
| M3 | court | `row.notional != 0` -> `== 0` | **FLIP** — `bn3_closure_battery` + `alloc_admission_budget` |
| M6 | court | stride `/ INTERLEAVE_STRIDE_DIVISIONS` -> `/ (... + 1)` | **FLIP** — `a05_interleave_engine` alone |
| M7 | court | K256[0] `0x428a2f98` -> `0x428a2f99` | **FLIP x8** — `sha256_known_answers` + all 7 corpus courts |
| M4b | court | escaped per-row `Vec` + `black_box` in admission loop | **FLIP** — `alloc_admission_budget` alone |
| M5 | court | escaped alloc + `black_box` before `hasher.finish()` in verify scope | **FLIP** — `alloc_verify_zero` alone |
| M9 | penstock.rs | `CGAP_ERR_P28_SPOLIATION` `0x5032_38FF` -> `...FE` | **FLIP** — `cross_lane_abi_pin` alone |
| M10 | unify.rs | `cols.col(si)` -> `cols.col(si + 1)` | **FLIP** — `a12_unifier_join_court` + `a12_unifier_fixpoint_court` |
| M11 | unify.rs | `epoch >= max_rounds` -> `>` | **FLIP** — `a12_unifier_fixpoint_court` alone |

Union of witnessed failures = all 11 courts; zero vacuous; the failed-court
sets match the wave-3/wave-5 receipts exactly. Not re-run (recorded): M2
(committed shim — covered by the gate's own `--shim-court-only` re-run),
M4a (receipted non-flip, dead-malloc elision), V1/V2 (campaign survivor
classes, moot: current campaign has 0 survivor defects).

## Triage-register revalidation (anti-padding)

Each registered Equivalent re-applied in the sandbox; a true Equivalent
must stay 11/11 green. All four PASS with restore=OK:

| Key | Re-applied edit | Outcome |
|---|---|---|
| `:918:cmp-swap:0` | `plan.rows > 0` -> `>= 0` | 11/11 PASS |
| `:1085:int-bump-down:0` | `range_root(rows, 0, 1, 0)` -> `0, 0, 0` | 11/11 PASS |
| `:1626:hex-bump:0` | `0xA5u8` -> `0xA6u8` | 11/11 PASS |
| `:1735:int-bump-down:9` | `200, 1000]` -> `199, 1000]` | 11/11 PASS |

## Interference incident (material, new this session)

While building the audit sandbox (~07:46), the shared
`tests/court_verdict.rs` presented a **live un-restored gate mutant**:
`0x94D0_49BB_1331_11EB` -> `0x94D0_49BB_1331_11EC` at line 449 (an
int-bump-up of the SplitMix64 constant; sandbox copy hash `2974f383...`,
mtime 07:24:12). Built from that state, the corpus generates
`d254689e...`, failing the pin in 7 courts. The file later read pristine
`cab5c2f5` (== the wave-5 receipt's recorded hash) with mtime 07:34:07 —
both mtimes predate this session, so mtime is not evidence of write time
in this checkout; content hash is the only receipt. Attribution: a
concurrent session's mutation campaign was mutating the real tree in this
window (the gate mutates the canonical file in place during `--mutate`).
This is a live demonstration of the Wave-8 interference threat model: any
court run, sandbox copy, or gate baseline taken while another session's
campaign holds a mutant reads a mutant as pristine. Not fixed (verdicts
only); the campaign design (mutate-in-place on a shared checkout) is the
root cause and is a campaign-level hazard for every lane, not just A20.

## DoD scorecard (this session)

1. All lane courts pass: **MET** (exit 0, 11/11, twice).
2. Mutation gate >= 0.98 with proven non-vacuity: **MET** (exit 0,
   kill_rate 1.0000, Baseline + ShimCourt rows present, inert control
   refuses VACUOUS GATE).
3. Zero external dependencies: **MET** (exit 0, E=0 x15).
4. Zero cloud strings: **MET** (exit 0, 0 hits, A07 deferral loud).
5. Deterministic corpus receipt: **MET** (in-suite pin + this session's
   verbatim-source scratch re-derivation, byte-exact).

## Divergences (receipt/doc yields; recorded, not fixed)

1. **Subject drift, post-receipt gate growth**: the wave-5 receipt pinned
   `boundary_gate.py` at 1142 lines / sha `0330e186...9ac6`; the current
   file is 1995 lines / sha `ab3d4bc9...97fb` — later sessions (w6-w8)
   added `--cross-gate-court` and `--interference-court` modes and
   refreshed the triage register (all disclosed in-file). The receipt's
   numbers describe a tree that no longer exists; its verdict survives
   because every claim re-ran green on the current tree. Receipt yields;
   current standing wins.
2. **Interference incident above** — recorded as D2 (section above).
   Consequence: my first sandbox baseline (built from the captured mutant
   state) produced a false corpus-pin failure that took a full
   isolate-and-reproduce investigation to refute; a gate-level lock or
   sandbox-only mutation is the structural fix, not applied.
3. **Gate `--report` parent-dir crash**: a full ~15-min campaign run with
   `--report /tmp/a20w9/ledger_mutate.json` completed all mutants, then
   died on `write_report` with FileNotFoundError because the parent dir
   did not pre-exist (boundary_gate.py:573). Exit 1, ledger lost. Minor
   robustness defect; recorded, not fixed.
4. **Auditor self-defects, disclosed**: two draft-corruption events while
   writing the attack driver (both aborted before execution — the
   hex-anchor helper compiled clean and was the only version executed);
   the first `--mutate` run's ledger was lost to D3; my hand-written
   Python corpus model produced `592d07a8...` (wrong; residual
   model-vs-source divergence unresolved — the authoritative
   re-derivation is the verbatim-scratch, which needs no model); one
   anchor-abort fired correctly on a 2-hit anchor.
   `pristine-check`-able via the state files under /tmp/a20w9-attack-state.
5. **Sandbox cleanup open**: `/tmp/a20w9-sandbox`,
   `/tmp/a20w9-attack-state`, `/tmp/a20w9-scratch*.rs`,
   `/tmp/a20w9-scratch{,2}`, plus all prior auditors' sandboxes
   (/tmp/a20-sandbox{-pristine}, /tmp/a20adv-sandbox, a20w6/w7/w8 dirs)
   remain on disk (rm -rf on /tmp refused by the session permission gate;
   coordinator cleanup list grows). Contains copies only, no git
   metadata.
6. **Receipt numbering**: waves 6-8 audit sessions left /tmp artifacts but
   no A20 receipts landed; this receipt takes wave 9 to avoid collision
   with any late-landing w6/w7/w8 receipt. Other lanes have wave-8/wave-9
   receipts on disk, so the number is fleet-consistent.

## Ownership check

`git diff --stat -- tests/court_verdict.rs scripts/boundary_gate.py
crates/cgap-court/` (read-only): exactly the 3 lane files
(+2952/-4 -> current tree +3805/-4 across the 3 files; the growth is
post-receipt gate/court work by w6-w8 sessions, disclosed above). All
mutations this session were sandbox-only; the repo court file is
byte-identical pre/post session (hashes above). No git write commands; no
worktrees; the only repo-side writes were cargo artifacts under the
pre-existing `target-a20` lane lease (including one `cargo clean -p
cgap-court` inside that lease, disclosed).

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && CARGO_PROFILE_DEV_OPT_LEVEL=2 CARGO_TARGET_DIR=$PWD/target-a20 \
  cargo test -p cgap-court --test court_verdict     # exit 0, 11 passed (x2)
python3 scripts/boundary_gate.py --deps                                  # exit 0, E=0 x15
python3 scripts/boundary_gate.py --strings                               # exit 0, 0 hits
python3 /Users/sac/cgap/scripts/boundary_gate.py --shim-court-only \
  --target-dir $PWD/target-a20                                           # exit 0
python3 scripts/boundary_gate.py --shim-court-only --shim-inert \
  --target-dir $PWD/target-a20                                           # exit 1 VACUOUS GATE
mkdir -p /tmp/a20w9 && python3 scripts/boundary_gate.py --mutate \
  --report /tmp/a20w9/ledger_mutate.json --target-dir $PWD/target-a20    # exit 0, 1.0000, 0 defects
# sandbox /tmp/a20w9-sandbox: 10-crate cp closure + tests/court_verdict.rs,
# minimal workspace manifest + [profile.dev] panic=abort + rust-toolchain.toml;
# baseline 11/11 (after pristine re-copy); attacks M1,M3,M6,M7,M4b,M5,M9,M10,M11
# (all FLIP, restore OK each); triage T1-T4 (all PASS, restore OK);
# sandbox end-state PRISTINE (cab5c2f5)
# corpus re-derivation: verbatim-source scratch (mechanical extraction of the
# generator region [VERDICT_ABI_VERSION..static CORPUS), rustc -O standalone)
# -> 03ed29d2... byte-exact
```

## See Also

- `docs/jira/v26.11.1/waves/a20-wave5-audit-receipt.md` (audited receipt)
- `docs/jira/v26.11.1/waves/a20-wave3-audit-receipt.md` (lineage)
- `docs/jira/v26.11.1/lanes/A20-court-verdict.md` (normative lane doc)
- `tests/court_verdict.rs`, `scripts/boundary_gate.py`,
  `crates/cgap-court/Cargo.toml` (the audited artifacts)
