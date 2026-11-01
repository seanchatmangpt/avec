# A03 Wave 5 Audit Receipt

- **Lane**: A03 (crates/cgap-git — Git Packfile + Content-Addressed Merkle
  DAG), Wave 5 defect repair, v26.11.1
- **Auditor**: adversarial audit lane (failed-lane restart), 2026-11-01
- **Subject**: `crates/cgap-git` working tree on `main` @ c83619d (lane work
  uncommitted on top); repo `/Users/sac/cgap`, one canonical checkout
- **Verdict**: **PARTIAL** — the landed defect-repair substance is real and
  non-vacuous (all 14 in-file dag courts green this session; 5/5 attacked
  code-level shim courts FLIP; 1 compile-abort witness), but DoD 2's
  `pack.rs` facade does not exist, the wave-5 lane report the code cites as
  its receipt locus is missing on disk, and the audit found one new
  robustness defect (dag.rs:1113). Repo left byte-exact: `dag.rs`
  sha256 `d06c9ad5…38e` and `pack_write.rs` sha256 `7750d2af…42b`
  identical pre/post audit; all mutations applied only in
  `/tmp/a03-sandbox` (deleted after receipt).

## Verified surfaces (re-run this session, exact commands)

| Surface | Evidence (command + exit, this session) |
|---|---|
| Build + zero warnings | `cargo build -p cgap-git` exit 0; post-`touch` rebuild grep -ci warning = **0** |
| Full suite | `cargo test -p cgap-git` exit 0 — **85 passed / 0 failed** across 10 binaries (2 loud `ignored`: perf/timing), no compile-abort |
| dag courts | `--lib` 57/57 green incl. all 14 `dag::tests` (d1..d9, dod4, r3, commit_validation, abi_frozen, value_types_are_inert) |
| D-6 KAT provenance | official BLAKE3 vectors (empty `af1349b9…`, 1/64/1024/1025/2048/2500/4096), pinned at dag.rs:1367-1406 |
| DoD-4 exit gate | `dod4_proof_budget_17_hashes` green: 16 sibling levels, **17** BLAKE3 hashes/proof, 618-byte §5.2 wire pin at L=65,536 — independent of N |
| D-10 sweep | grep `mock\|sleep` over src/: only `idx_read.rs:219` comment "no mocks" (comment-only hit); zero real mocks, zero sleeps |
| Deps | `crates/cgap-git/Cargo.toml` `[dependencies]` EMPTY — zero new deps (DoD 9); cgap-git stanza in Cargo.lock unchanged |
| Ownership | `git diff --stat -- crates/cgap-git/` = **1 file**: `dag.rs` +1686; `lib.rs` diff empty; no untracked files in the crate; `git worktree list` = canonical only |

## Anti-vacuity court attack (all mutations in /tmp/a03-sandbox only)

Method: `cp -r` of `crates/cgap-git` + LICENSE.md into `/tmp/a03-sandbox`
with a minimal standalone workspace manifest; baseline 57/57 green; one
mutation at a time; restore from pristine repo copy between mutations;
sha256-verified byte-exact restore after every mutation (dag.rs
`d06c9ad5…`, pack_write.rs `7750d2af…`); post-restore re-run 57/57.

| Shim | Edit (sandbox only) | Court outcome (`cargo test -p cgap-git --lib`) |
|---|---|---|
| SHIM-ORD | dag.rs:851 seq sort → descending | **5 courts FLIP** (52/57): d2 (pinned-root KAT), d4, d8, r3, dod4 — D-2 target flips loudly; collateral = proofs desynced from the reordered tree |
| SHIM-PARENT | dag.rs:1022 parent check `if X {` → `if false && X {` | **exactly d3 + d8 FLIP** (55/57) — isolated, attributable |
| SHIM-B3 | dag.rs:749 final chunk-stack fold `.take(0)` | **exactly d6 FLIP** (56/57): fails at "BLAKE3 KAT at input length 1025" (first multi-chunk vector); single-chunk vectors keep passing |
| SHIM-CAP | dag.rs:178 `MAX_BLOCKS … + 1` | **build ABORTS, 5 errors**: static asserts fire at dag.rs:472/473/474/478 — cap cannot be raised silently (compile-abort recorded as such, not a failure count) |
| SHIM-TRAILER | dag.rs:1122 trailer check `&& false` | **exactly d9 FLIP** (56/57) |
| SHIM-SHA1 | pack_write.rs Sha1 IV zeroed | **d5 FLIPS** (54/57; + d2 pinned-root collateral + `sha1_known_vectors`) |

Zero vacuous courts: every attacked court flips under its named mutation
class, and SHIM-PARENT / SHIM-B3 / SHIM-TRAILER flips are isolated to
their own courts.

## Audit finding (verdicts only; NOT fixed)

**F-1 (new defect, advisory): hostile `.cgap-dag` bytes panic in debug
builds.** `decode_dag_file` evaluates `2 * block_count - 1` at
`dag.rs:1113` before any length guard; `block_count = 0x80000000`,
`node_count = 0xFFFFFFFF` (valid magic/version, 92-byte file) →
`panic: attempt to multiply with overflow` (witnessed via sandbox probe
`tests/hostile_probe.rs`, exit FAIL). Release builds wrap, the 2L-1 law
passes, and the later length check returns typed `Truncated` — so the
defect is debug-only in effect but violates the typed-refusal contract
for a public parser of untrusted bytes (D-9's court never feeds hostile
counts). Fix belongs to the lane, not the auditor.

## Divergences (doc yields; recorded, not fixed)

1. **Wave-5 lane report MISSING.** dag.rs:8-10 cites "receipts in the
   wave-5 lane report" as the shim-court receipt locus; no such file
   exists anywhere on disk (only a11/a17/a20 audit receipts under
   docs/jira/v26.11.1/waves/). DoD-10's "mutation receipts recorded"
   therefore rests on this audit's own flips, not the lane's claim.
   The lane doc §10 itself still says "Status: UNVERIFIED — no code
   landed this session", stale against +1686 lines of landed dag.rs.
2. **DoD 2 unmet: `pack.rs` facade does not exist** (listed lane target
   file, never scaffolded, never created). dag types are reachable only
   as `cgap_git::dag::*`; lib.rs root re-exports unchanged (the
   back-compat half of DoD 2 holds trivially).
3. **Internal payload (divergence note 1, disclosed in-code)**: §3.2.3's
   `left || right || seq_parent || depth` is implemented as
   `left || right` (64 B), bound by the frozen 618-byte §5.2 wire pin —
   arithmetically correct; doc yields.
4. **Node table postorder (note 2, disclosed)**: §5.1's "preorder" is
   implemented as postorder so the decoder can structurally reduce with
   a 2-stack while re-verifying every internal digest; count law and
   field sizes unchanged. Doc yields.
5. **D-2 mutation wording**: doc says "shuffle node ordering internally";
   implementation is a pinned root VALUE KAT (`PINNED_ROOT_HEX`,
   dag.rs:1207) + the `SHIM-ORD` target line — strictly stronger than
   the doc (self-consistent reordering cannot pass).
6. **D-10 literal-grep false positive**: the court's own grep matches the
   "no mocks" comment at `idx_read.rs:219`; comment-only, no real mock.
7. **Auditor session mark**: this audit `touch`ed dag.rs once (mtime
   only, to surface build warnings); content sha256 identical
   pre/post (`d06c9ad5…`).

## Commands (exact, re-runnable)

```
cargo build -p cgap-git                        # exit 0, 0 warnings
cargo test -p cgap-git                         # exit 0, 85 passed / 0 failed
cargo test -p cgap-git --lib                   # exit 0, 57 passed (14 dag courts)
git diff --stat -- crates/cgap-git/            # 1 file: dag.rs +1686
git worktree list                              # canonical checkout only
# sandbox /tmp/a03-sandbox (deleted): standalone workspace, baseline 57/57,
# SHIM-ORD/SHIM-PARENT/SHIM-B3/SHIM-CAP/SHIM-TRAILER/SHIM-SHA1 one at a time,
# sha256 byte-exact restore between all mutations, hostile_probe.rs (F-1)
```

## See Also

- `docs/jira/v26.11.1/lanes/A03-git-merkle.md` (normative lane doc)
- `crates/cgap-git/src/dag.rs` (sole modified file, +1686)
- `docs/jira/v26.11.1/waves/a17-wave2-audit-receipt.md` (audit protocol)
