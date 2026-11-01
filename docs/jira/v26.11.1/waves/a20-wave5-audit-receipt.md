# A20 Wave 5 Audit Receipt

- **Lane**: A20 (court verdict), Wave 5 failed-lane-restart audit of
  `a20-wave3-audit-receipt.md`, v26.11.1
- **Auditor**: adversarial audit lane (Wave 5 restart), 2026-11-01
- **Subject**: `docs/jira/v26.11.1/waves/a20-wave3-audit-receipt.md` and the
  tree it describes: `tests/court_verdict.rs` (1783 lines, +1784 in
  `git diff --stat`), `scripts/boundary_gate.py` (1142 lines, +1142),
  `crates/cgap-court/Cargo.toml` (+30) on `main` @ c83619d (uncommitted lane
  work on top); repo `/Users/sac/cgap`, one canonical checkout
- **Working-tree identity (sha256, unchanged pre/post this session)**:
  `tests/court_verdict.rs` `cab5c2f52ca46cd9...6b86`,
  `scripts/boundary_gate.py` `0330e186902539d9...9ac6`,
  `crates/cgap-court/Cargo.toml` `1128bb06e6608547...36c2b`,
  `Cargo.lock` `47f5a125e2309317...d9b07`
- **Verdict**: **ALIVE** — every re-runnable claim in the audited receipt
  reproduced; every attacked court flips under its named mutation (zero
  vacuous); the receipt's one PARTIAL gating item (DoD-2, mutation campaign
  exit 1 at kill_rate 0.7083) is **stale, not false**: the current tree
  PASSES DoD-2 (exit 0, kill_rate 1.0000, 0 survivor defects) because a
  post-receipt repair landed (Wave-4 row-0 anchor hardening + Wave-5
  `rule_id` pin + a 4-entry `EQUIVALENT_TRIAGE` register whose entries I
  attack-validated as honest). Lane standing on this session's re-runs:
  all five DoD gates return their expected exits.

## Re-runs (all commands executed this session, /Users/sac/cgap)

| Receipt claim | This session | Outcome |
|---|---|---|
| Court suite 11/11, exit 0 (§9.1) | `CARGO_PROFILE_DEV_OPT_LEVEL=2 CARGO_TARGET_DIR=$PWD/target-a20 cargo test -p cgap-court --test court_verdict` -> exit 0, 11/11 in 0.91 s | REPRODUCED |
| `--deps` exit 0, E=0 x15 (§9.3) | exit 0, "E=0 runtime externals across 15 covered crates" | REPRODUCED |
| `--strings` exit 0, 0 hits (§9.4) | exit 0, "0 hits across 15 covered crates" | REPRODUCED |
| Shim-court exit 0 (§6.3) | exit 0, "killed by a05_interleave_engine" | REPRODUCED |
| `--shim-inert` -> exit 1 VACUOUS GATE | exit 1, VACUOUS GATE refusal printed | REPRODUCED |
| §9.5 corpus receipt `03ed29d2...` | independent Python re-derivation (SplitMix64 + overrides + 54-byte LE encoding) -> byte-exact match | REPRODUCED |
| `--mutate` exit 1, 0.7083, 7 survivors (§9.2) | **exit 0, 20/20 non-equivalent killed (1.0000), 4 Equivalent, 0 survivor defects, 995 generated / 24 sampled** | **DIVERGES — repair landed post-receipt (see D1)** |

## Anti-vacuity re-attack (all mutations in /tmp/a20adv-sandbox only)

Method: my own sandbox — `cp -r` of the 10-crate closure (9 runtime crates +
cgap-git dev-dep) plus `tests/court_verdict.rs`, same relative layout so the
`[[test]] path = "../../tests/court_verdict.rs"` stanza needed no rewrite;
hand-written minimal workspace manifest; sandbox baseline exit 0, 11/11;
one mutation at a time via a base64-anchored fail-closed helper (unique-anchor
check, sha256 before/after, restore mismatch aborts everything); every attack
below reports `restore=OK`. Canonical checkout never written.

| Attack | File | Edit | Outcome (this session) |
|---|---|---|---|
| M1 | court | `SkipLoud = 6` -> `7` | **FLIP** — `wave1_abi_freeze_receipt` alone |
| M3 | court | NORMAL arm `notional != 0` -> `== 0` | **FLIP** — `bn3_closure_battery` + `alloc_admission_budget` |
| M6 | court | stride divisor `/ N` -> `/ (N+1)` | **FLIP** — `a05_interleave_engine` alone |
| M7 | court | `K256[0]` `0x428a2f98` -> `0x428a2f99` | **FLIP** — `sha256_known_answers` + 7 corpus courts |
| M4b | court | per-row `Vec::<u64>` escaped via `black_box` in admission loop | **FLIP** — `alloc_admission_budget` alone |
| M5 | court | escaped allocation in verify loop scope | **FLIP** — `alloc_verify_zero` alone |
| M9 | penstock.rs | `CGAP_ERR_P28_SPOLIATION` `0x5032_38FF` -> `0x5032_38FE` | **FLIP** — `cross_lane_abi_pin` alone |
| M10 | unify.rs | `cols.col(si)` -> `cols.col(si + 1)` | **FLIP** — `a12_unifier_join_court` + `a12_unifier_fixpoint_court` |
| M11 | unify.rs | fixpoint `epoch >= max_rounds` -> `>` | **FLIP** — `a12_unifier_fixpoint_court` alone |

**All 9 re-attacked mutations flip with the exact failed-court sets the
receipt recorded; union of witnessed failures covers all 11 courts — zero
vacuous courts, confirmed independently.** Not re-run (recorded, not hidden):
M2 (the committed shim — equivalent to the gate's own shim-court, re-run
above), M4a (receipted as a non-flip / dead-malloc elision), V1/V2 (campaign
survivor re-witnesses; moot — the current campaign has no survivors).

## Triage-register anti-padding validation

The repaired gate carries `EQUIVALENT_TRIAGE` (scripts/boundary_gate.py:213-240)
with 4 keys. I re-applied each registered mutant in the sandbox; a true
Equivalent must stay 11/11 green:

| Register key | Re-applied edit | Outcome |
|---|---|---|
| `:918:cmp-swap:0` | `plan.rows > 0` -> `>= 0` | 11/11 PASS — honest Equivalent (u64 tautology) |
| `:1085:int-bump-down:0` | rollback `range_root(rows, 0, 1, 0)` -> `0, 0, 0` | 11/11 PASS — honest Equivalent |
| `:1626:hex-bump:0` | filler `0xA5` -> `0xA6` | 11/11 PASS — honest Equivalent |
| `:1735:int-bump-down:9` | KAT length `200` -> `199` | 11/11 PASS — honest Equivalent |

All four keys point at real current-file sites (read at :918/:1085/:1626/:1735
this session). No denominator padding detected.

## DoD scorecard (this session)

1. All lane courts pass: **MET** (exit 0, 11/11).
2. Mutation gate >=0.98 with proven non-vacuity: **MET NOW** — exit 0,
   kill_rate 1.0000, ledger first rows Baseline + ShimCourt-kill, inert
   control refuses VACUOUS GATE. (The audited receipt scored this NOT MET
   as-cited; see D1.)
3. Zero external dependencies: **MET** (exit 0, E=0 x15).
4. Zero cloud strings: **MET** (exit 0, 0 hits, A07 deferral loud).
5. Deterministic corpus receipt: **MET** (in-suite assert + my independent
   cross-language re-derivation, byte-exact).

## Divergences (receipt/doc yields; recorded, not fixed)

1. **DoD-2 finding is stale, not false**: the receipt's exit-1 / 0.7083 /
   7-survivor campaign was real when written; the tree has since been
   repaired — `scripts/boundary_gate.py:205-212` documents the Wave-4
   row-0 anchor hardening + Wave-5 `compiled.rule_id` pin (both converted
   receipt survivors into kills, witnessed as 10/1 flips), and
   `tests/court_verdict.rs:821-827` carries the hardening doc; the module
   doc's "528" staleness (receipt divergence 7) is also self-corrected at
   `tests/court_verdict.rs:109-110`. Receipt yields; current standing wins.
2. **Subject drift**: receipt cites court file "+1770"; current diff is
   +1784 — same repair, consistent lineage (pre-receipt vs post-repair).
3. **Register coverage is sample-relative**: the 24-mutant sample covers
   24 of 995 generated mutants; sites outside the sample (e.g. the
   receipt's :1467/:1565 survivor classes) are neither kill-witnessed nor
   triaged this session — UNVERIFIED, not counted. Sampling is recorded in
   the ledger (lane doc §8), so this is recorded, not silent.
4. **Sandbox cleanup refused**: the session permission gate refused
   `rm -rf` on /tmp sandbox paths (same class the audited receipt
   witnessed). Open coordinator cleanup: `/tmp/a20-sandbox`,
   `/tmp/a20-sandbox-pristine` (prior auditor), `/tmp/a20adv-sandbox`
   (this audit; contains only copies, no git metadata).
5. **Auditor self-defects, recorded**: my mutate.sh initially hashed the
   pre-image after applying (two `restore=MISMATCH` lines that were script
   artifacts — tree verified restored byte-exact); the first M4b/M5
   attempts BUILD-ABORTed on an untyped `Vec::with_capacity` (E0282) and
   were redone typed; one discarded driver draft contained placeholder
   corruption and was never executed (every executed attack used the
   base64-anchored helper with unique-anchor checks).
6. **killed_by wobble**: both my shim runs printed
   `a05_interleave_engine`; no contradiction with the receipt's divergence
   4 (ledger name nondeterminism) — untested this session, stays
   UNVERIFIED as a claim.

## Ownership check

`git diff --stat -- tests/court_verdict.rs scripts/boundary_gate.py
crates/cgap-court/` (read-only): exactly the 3 lane files, +2952/-4.
Root `Cargo.toml` and `crates/cgap-court/src/lib.rs`: untouched (empty
diff). Other modified files in the shared checkout map to their own lanes.
All mutations this session were applied under `/tmp/a20adv-sandbox` only;
the four tracked-file hashes above are byte-identical pre/post session.
No git write commands; no worktrees; no repo copies.

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && CARGO_PROFILE_DEV_OPT_LEVEL=2 CARGO_TARGET_DIR=$PWD/target-a20 \
  cargo test -p cgap-court --test court_verdict     # exit 0, 11 passed
python3 scripts/boundary_gate.py --deps                                   # exit 0, E=0 x15
python3 scripts/boundary_gate.py --strings                                # exit 0, 0 hits
python3 scripts/boundary_gate.py --shim-court-only --target-dir $PWD/target-a20   # exit 0
python3 scripts/boundary_gate.py --shim-court-only --shim-inert \
  --target-dir $PWD/target-a20                                            # exit 1 VACUOUS GATE
python3 scripts/boundary_gate.py --mutate --report /tmp/a20adv/ledger_mutate.json \
  --target-dir $PWD/target-a20                                            # exit 0, 1.0000, 0 defects
# sandbox /tmp/a20adv-sandbox: cp -r of 9 runtime crates + cgap-git dev-dep
# + tests/court_verdict.rs under a minimal workspace manifest; baseline
# 11/11; attacks M1,M3,M6,M7,M4b,M5,M9,M10,M11 (all FLIP, restore OK x9);
# triage validations :918/:1085/:1626/:1735 (all PASS); sha256-verified
# restore after every run. Python corpus re-derivation -> 03ed29d2... match.
```

## See Also

- `docs/jira/v26.11.1/waves/a20-wave3-audit-receipt.md` (audited receipt)
- `docs/jira/v26.11.1/lanes/A20-court-verdict.md` (normative lane doc)
- `tests/court_verdict.rs`, `scripts/boundary_gate.py`,
  `crates/cgap-court/Cargo.toml` (the audited artifacts)
