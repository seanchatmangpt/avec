# A20 Wave 3 Audit Receipt

- **Lane**: A20 (court verdict), Wave 3 full lane scope (courts, WAL, unifier,
  dam, sealers), v26.11.1
- **Auditor**: adversarial audit lane (failed-lane restart), 2026-11-01
- **Subject**: `tests/court_verdict.rs` (+1770), `scripts/boundary_gate.py`
  (+831), `crates/cgap-court/Cargo.toml` (+30) on `main` @ c83619d (working
  tree, uncommitted lane work on top); repo `/Users/sac/cgap`, one canonical
  checkout
- **Verdict**: **PARTIAL** — every attacked court and sealer flips under its
  named mutation (0 vacuous), all re-runs green except the lane's own exit
  gate #2: the verbatim DoD §9.2 mutation campaign **exits 1** this session
  (kill rate 0.7083 < 0.98, 7 untriaged survivor defects), and no lane wave
  receipt exists on disk. Repo left byte-exact (all four tracked-file hashes
  pre == post); mutations applied only in `/tmp/a20-sandbox` (left on disk:
  the session permission gate refused `rm -rf` on the sandbox paths —
  deleting `/tmp/a20-sandbox` + `/tmp/a20-sandbox-pristine` is an open
  coordinator cleanup item; contains only copies, no git metadata).

## Verified surfaces (all re-run this session)

| Surface | Evidence (command + exit, this session) |
|---|---|
| Court suite | `cargo test -p cgap-court --test court_verdict` (A20 lease `CARGO_TARGET_DIR=target-a20`, `CARGO_PROFILE_DEV_OPT_LEVEL=2`) -> exit 0, **11/11** in 0.99 s |
| Sandbox baseline | cold build in `/tmp/a20-sandbox` (minimal standalone workspace, A17-audit precedent) -> exit 0, 11/11 |
| DoD §9.1 | exit 0, 0 failed |
| DoD §9.2 `--mutate` | **exit 1**: 977 mutants generated, stride-41 sample of 24, 17 killed (12 KilledByTest + 5 KilledByBuild), kill_rate 0.7083 < 0.98, **7 SurvivorDefect rows** — gate refuses by design |
| DoD §9.3 `--deps` | exit 0, E=0 runtime externals across 15 covered crates |
| DoD §9.4 `--strings` | exit 0, 0 hits across 15 covered crates; A07-register deferral recorded loud |
| DoD §9.5 corpus receipt | pinned hash asserted in-suite; **independent Python re-derivation matches byte-exact**: `03ed29d285cec5bb381ebdfa045f0aa0a81c0aa6282b6777a20164f784f7facb` |
| Non-vacuity (§6.3) | `--shim-court-only` -> exit 0, "killed by a05_interleave_engine"; `--shim-court-only --shim-inert` -> exit 1 `VACUOUS GATE` (negative control behaves) |

## Anti-vacuity court attack (all mutations in /tmp/a20-sandbox only)

Method: per-crate `cp` of the court closure + covered crates into
`/tmp/a20-sandbox` with a hand-written minimal workspace manifest; sandbox
baseline 11/11; one mutation at a time; sha256-verified restore after every
run (fail-closed helper, `restore=OK` on all 13 runs); repo sha256
byte-exact before/after the whole session.

| Mut | File | Edit | Outcome |
|---|---|---|---|
| M1 | court | `SkipLoud = 6` -> `7` (discriminant drift, build stays clean) | **`wave1_abi_freeze_receipt` FLIPS alone** (10 pass / 1 fail, rc 101) |
| M2 | court | gate's committed SHIM (SplitMix64 mix `...E5B9` -> `...E5BA`) | **7 courts FLIP** incl. `corpus_determinism_receipt` — the receipt assert inside `corpus()` fails fail-closed before any corpus court asserts |
| M3 | court | `row_matches_tag` NORMAL arm `notional != 0` -> `== 0` (corpus digest unchanged) | **`bn3_closure_battery` + `alloc_admission_budget` FLIP** ("tag 0 does not match content") |
| M4a | court | per-row `Vec::with_capacity(8)` dropped unused in admission pass | 11/11 PASS — **optimization-equivalent**: LLVM dead-malloc elimination removes the elidable allocation; recorded, redone as M4b |
| M4b | court | same allocation escaped via `std::hint::black_box(&v)` | **`alloc_admission_budget` FLIPS alone** |
| M5 | court | black_box-escaped per-row allocation in the verify loop | **`alloc_verify_zero` FLIPS alone** |
| M6 | court | slot-plan stride divisor off-by-one (`/ N` -> `/ (N+1)`) | **`a05_interleave_engine` FLIPS alone** (endpoint invariant "sweep must cover k = N") |
| M7 | court | `K256[0]` round constant bumped | **`sha256_known_answers` FLIPS** (FIPS 180-4 vector) + 7 corpus courts (K256 feeds corpus hashing) |
| M8 | penstock.rs | row-4 attestation-magic check inverted to `return CGAP_OK` | **`a14_dam_ingress_court` FLIPS alone** (row 4 admits what taxonomy expects rejected) |
| M9 | penstock.rs | `CGAP_ERR_P28_SPOLIATION` mirror `0x5032_38FF` -> `...FE` | **`cross_lane_abi_pin` FLIPS alone**; the dam court is blind (it compares the drifted constant to itself) — exactly the cross-lane drift the pin court exists to catch |
| M10 | unify.rs | join predicate `cols.col(si)` -> `cols.col(si + 1)` | **`a12_unifier_join_court` + `a12_unifier_fixpoint_court` FLIP** |
| M11 | unify.rs | fixpoint budget `epoch >= max_rounds` -> `>` | **`a12_unifier_fixpoint_court` FLIPS alone** (typed `FixpointBudgetExceeded{rounds:2}` becomes Ok) |
| V1 | court | campaign survivor 909 cmp-swap (`>` -> `>=`) re-applied | 11/11 PASS — survivor row re-witnessed; triage: trivially true for u64, Equivalent |
| V2 | court | campaign kill 127 int-bump-up (`VERDICT_ABI_VERSION` 1 -> 2) | build abort — KilledByBuild row re-witnessed |

**11/11 courts have a witnessed flip; zero vacuous courts.** First M5/M4
attempts that failed to flip were diagnosed (anchor miss; dead-malloc
elision) and redone — recorded, not hidden.

## Sealer attacks (§6.4, sandbox)

| Attack | Outcome |
|---|---|
| S0: covered crate absent from tree | exit 1, fail-closed list of the missing crate (negative control, behaves) |
| S1: external dep injected into `[dependencies]` | exit 1, "EXTERNAL: probe-ext" named per crate — **flips** |
| S1a: same dep appended into `[dev-dependencies]` | exit 0 — correctly not runtime (first probe misplaced; gate right, probe wrong; recorded) |
| S2: cloud strings planted in a covered crate | exit 1, typed HIT rows (`amazonaws`, `blob.core`, `s3.<host>`) — **flips** |
| Clean sandbox re-runs | `--deps` exit 0 E=0 x15; `--strings` exit 0, 0 hits |

## DoD scorecard (§9, lane doc)

1. All lane courts pass: **MET** (11/11, exit 0).
2. Mutation gate at threshold with proven non-vacuity: **NOT MET as-cited**
   — plain `--mutate` exits 1 with kill_rate 0.7083 and 7 untriaged
   survivor defects (all triage-able to Equivalent; the `--equivalent`
   triage path exists and was not exercised/recorded). Non-vacuity half is
   MET (shim kills; inert control refuses).
3. Zero external dependencies: **MET** (exit 0, E=0 x15).
4. Zero cloud strings: **MET** (exit 0, 0 hits; A07 deferral loud).
5. Deterministic corpus receipt: **MET** (in-suite + independent
   cross-language re-derivation, byte-exact).

## Divergences (doc yields; recorded, not fixed)

1. **Wave receipt missing**: no A20 wave receipt existed on disk
   (`waves/` holds only the A17 audit receipt + W5 PRD). This audit
   receipt substitutes the verdict; the lane's own kill-rate claim was
   never made, so DoD-2 cannot be counted as a retracted claim — it is
   simply not met un-triaged.
2. **DoD §9.2 un-triaged survivors (7)**, all in the court's own test
   material/assert literals, each Equivalent-triageable:
   `:909` `plan.rows >= 0` (trivially true for u64); `:1076` rollback range
   hi `1` -> `2` (dag_seq 0 still refused); `:1467` distance literal
   `1` -> `0` (vacuous on the closure domain); `:1565` `48 + 6` -> `49 + 6`
   (byte 55 still trips the reserved-flag mask); `:1611` payload
   `1024` -> `1025` (content independence holds); `:1721` duplicate KAT
   sweep length; `:817` row-0 anchor `i == 0` -> `i == 1` (diverges only on
   a ~2^-64 event). None is a kernel assertion gap; the gate cannot know
   that without the recorded triage.
3. **Doc's named budget mutation is mechanically imprecise** (§5.3 module
   doc): a "per-row `Vec::push`" amortizes to ~20 allocations (< 976
   budget, would not flip), and an unused per-row `with_capacity` is
   dead-malloc-eliminated (M4a, witnessed). The invariant that actually
   holds and flips: an escaped per-row allocation (M4b/M5). Doc yields.
4. **`killed_by` is output-order nondeterministic**: under the shim, 7
   corpus courts fail together and the ledger's `killed_by` is whichever
   FAILED line the parallel runner prints first (observed
   `a05_interleave_engine` twice; the doc pins `corpus_determinism_receipt`
   semantically, not as the ledger name). Kill is real; ledger name wobbles.
5. **Gate target scope**: §6.1 says per-kernel `lib.rs` modules; the gate
   mutates only `tests/court_verdict.rs`, with the lib.rs targets deferred
   to Wave 4 (disclosed in the gate docstring and every ledger's `config`).
   Doc yields; loud deferral, not silent.
6. **A07 register absent**: `--strings` enforces the §6.4 doc-list directly
   with recorded deferral (single-list law respected — no second
   allowlist). The `s3.` pattern was tightened to the host form to avoid a
   witnessed false positive on `asc450.rs`'s `s3.verify` — tightening
   recorded in code, not an allowlist.
7. **Module-doc stale number**: court module doc cites closure counts
   "496/528"; the court pins 496 (round sizes 31+59+106+164+136 = 496).
   528 is stale prose. Doc yields.
8. **Manifest edit disclosed**: `crates/cgap-court/Cargo.toml` +30 (six
   dev-deps) carries in-file A20-sole-writer disclosure; shared
   `Cargo.lock` churn is mechanical (same class as A17's D4).
9. **Topology guard fired once** on the auditor's whole-repo rsync
   (refused); sandbox rebuilt per-crate per the A17-audit precedent — the
   guard worked, recorded here as the receipt of that refusal.

## Ownership check

`git diff --stat -- tests/court_verdict.rs scripts/boundary_gate.py
crates/cgap-court/` (read-only): exactly the 3 lane files (+2627/-4).
All other modified files in the shared checkout map to their own lanes
(A05 `atomic_commit.rs`, A12 `unify.rs`, A14 `penstock.rs`, A17 `p28`,
...). Root `Cargo.toml`, `crates/cgap-court/src/lib.rs` untouched. No git
write commands; no worktrees; canonical checkout byte-exact
pre/post-session (sha256 of `tests/court_verdict.rs`,
`scripts/boundary_gate.py`, `crates/cgap-court/Cargo.toml`, `Cargo.lock`
all unchanged; cargo runs used the pre-existing `target-a20` lane lease).

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && CARGO_PROFILE_DEV_OPT_LEVEL=2 CARGO_TARGET_DIR=$PWD/target-a20 \
  cargo test -p cgap-court --test court_verdict     # exit 0, 11 passed
python3 scripts/boundary_gate.py --deps                                   # exit 0, E=0 x15
python3 scripts/boundary_gate.py --strings                                # exit 0, 0 hits
python3 scripts/boundary_gate.py --shim-court-only --target-dir $PWD/target-a20   # exit 0
python3 scripts/boundary_gate.py --shim-court-only --shim-inert \
  --target-dir $PWD/target-a20                                            # exit 1 VACUOUS GATE
python3 scripts/boundary_gate.py --mutate --report /tmp/a20-audit/ledger_mutate.json \
  --target-dir $PWD/target-a20                                            # exit 1, 0.7083, 7 defects
# sandbox /tmp/a20-sandbox: per-crate cp, minimal workspace manifest;
# baseline 11/11; M1-M11, M4a/M4b, V1/V2 one at a time; sha256 restore OK x13;
# sealer attacks S0/S1/S1a/S2; sandbox left on disk (rm -rf refused by the
# session permission gate) — coordinator cleanup: /tmp/a20-sandbox{-pristine}
```

## See Also

- `docs/jira/v26.11.1/lanes/A20-court-verdict.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a17-wave2-audit-receipt.md` (format precedent)
- `tests/court_verdict.rs`, `scripts/boundary_gate.py`,
  `crates/cgap-court/Cargo.toml` (the audited artifacts)
