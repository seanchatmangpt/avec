# Wave 11 Receipt: Convergence (Lanes C1-C5)

Wave 11 (2026-10-30/31) is the v26.10.31 convergence wave: five lanes over
the single canonical checkout at /Users/sac/avec. No git transitions — the
coordinator owns them. Every status below was re-verified on disk at this
edit (2026-10-30 22:50 PDT = 2026-10-31T05:50Z).

## Convergence Lanes

| Lane | Scope | Status |
|---|---|---|
| C1 | dod exec-bit + verdict | CLOSED — GATES GREEN 3 PENDING, exit 0 |
| C2 | fresh ladder + census v2 | LANDED — 10/10 PASS; census 661/0/2 reproduced |
| C3 | slack adjudication + OQ | LANDED — ADJUDICATED, 19/19 sites per ADR-14 |
| C4 | cites + attestation map | LANDED — mapping PINNED; OQ-06 settled |
| C5 | index + receipt | THIS DOC — 00-index Wave 10/11 sections + this receipt |

## Lane Evidence (disk-verified at this edit)

- **C1** — scripts/dod.sh is 0755 (`-rwxr-xr-x`, `ls -la` witnessed).
  This lane re-ran `bash scripts/dod.sh` in this window (21-entry
  manifest): `== DoD verdict: GATES GREEN -- 3 PENDING (green=18)`,
  DOD EXIT 0. Pending rows witnessed: P-CTQ06-lsl-ratification,
  P-CTQ04-protocol-declaration, P-INTEGRATION-commit. Log:
  /tmp/dod-wave11-l5.log (scratch, non-durable; verdict quoted verbatim).
  Prior run + selftest court: 66-dod-receipt.md (GATES GREEN 3 PENDING;
  `--selftest` SELFTEST PASS at 05:32:06Z).
- **C2** — 72-census-v2.md on disk: fresh ladder
  `bash scripts/verify_v26.10.31.sh` → 10/10 gates PASS, exit 0; census
  60 result lines · 661 passed / 0 failed / 2 ignored (the 63-baseline
  661/0/2 reproduced exactly; +2 lines = two new empty avec-cli bin
  suites, +0 tests); verify-latest.md regenerated 2026-10-31T05:46:41Z
  (10/10 PASS, HEAD ead4032); G1 git stage witnessed live under
  `--nocapture` (admission_e2e.rs:259 `[stage: git]` LANDED, 2/2).
- **C3** — 73-slack-adjudication.md on disk, Status: ADJUDICATED: the
  live +17 transpose row in scripts/cfg_homology.sh fully adjudicated
  per ADR-14 — all 19 emitted sites (12 transpose + 7 untranspose) map
  to declared remainder classes in scripts/branchless_manifest.toml;
  the proxy FAIL is a proxy/slack mismatch, not an unadjudicated
  defect; slack 4 → 20 proposed as a reviewed diff (script NOT edited
  by the lane); 4-item falsifier list present.
- **C4** — 74-attestation-mapping.md on disk, mapping PINNED: SHA-1
  (git object name) vs BLAKE3 (content receipt) dual map; in-band
  carrier = quad inline_digest column; commit OID as transitive Merkle
  witness; settles OQ-06; halves ALIVE (gate-git-tests-v3 fsck 2/2 +
  interop 3/3; gate-blake3-tests-v3); §4 per-chunk cross-check court
  named, open.

## Closed Items (wave-11 plan items 1-16)

Provenance disclosure: the wave-11 plan text is not on disk; this mapping
is reconstructed from the coordinator dispatch and the wave-10 receipt's
lane table (68-wave-10-receipt.md). Every pointer verified on disk at
this edit. Most items landed in wave-10 or its G1-G3 gap lanes; wave 11
verified rather than re-did them.

| # | Item | Landed via | Pointer (verified) |
|---|---|---|---|
| 1 | e2e git stage (fsck in-loop) | G1 | admission_e2e.rs · gate-e2e-integration-v3.md |
| 2 | receipts v3 minted | G2 | gate-git-tests-v3.md · gate-cpk-study-v3.md |
| 3 | untranspose manifest entry | G3 | gate-branchless-manifest-v4.md |
| 4 | ADR-17 NEON-transpose refutation | G3 | 22-adr-register.md |
| 5 | 69-validation-receipt | wave-10 | 69-validation-receipt.md |
| 6 | dod.sh + manifest + selftest | wave-10 L1 | scripts/dod.sh · dod-manifest.toml |
| 7 | doctor bin (--json) | wave-10 L2 | crates/cli/src/bin/doctor.rs |
| 8 | justfile (8 recipes) | wave-10 L3 | justfile |
| 9 | GETTING_STARTED | wave-10 L4 | docs/GETTING_STARTED.md |
| 10 | TROUBLESHOOTING | wave-10 L9 | docs/TROUBLESHOOTING.md |
| 11 | diataxis router | wave-10 L10 | docs/diataxis/ |
| 12 | FEATURE_MATRIX | wave-10 L13 | docs/FEATURE_MATRIX.md |
| 13 | OPERATOR_DECISIONS | wave-10 L16 | docs/OPERATOR_DECISIONS.md |
| 14 | README refresh | wave-10 L19 | README.md |
| 15 | CI mirror (3 jobs) | wave-10 L6 | .github/workflows/ci.yml |
| 16 | e2e demo + examples + hygiene | wave-10 L12/L17 | examples/sovereign-e2e · 71-hygiene |

## Items 17-18: C2's Run (fresh ladder + census v2)

- **17 (fresh ladder)**: `bash scripts/verify_v26.10.31.sh` → 10/10
  gates PASS, exit 0; verify-latest.md regenerated 2026-10-31T05:46:41Z,
  HEAD ead4032. v3/v4 receipt family on disk, standings extracted from
  the files: workspace-check-v3, arrow-tests-v3, blake3-tests-v3,
  e2e-integration-v3, shacl-v3, branchless-selftest-v3,
  branchless-manifest-v4, git-tests-v3 all ALIVE exit 0;
  gate-cpk-study-v3 FAILED exit 1 — CTQ-06 at 1.85 GiB/s vs the 8.5
  LSL, the standing honest red, kept unrounded.
- **18 (census v2)**: 72-census-v2.md — 60 result lines · 661 passed /
  0 failed / 2 ignored; the 63-baseline 661/0/2 reproduced exactly;
  +2 lines = two new empty avec-cli bin suites (+0 tests); 0
  BUILD-ABORT; bench --lib pre-build refusal (exit 101) disclosed
  in the doc.

## PENDING-OPERATOR

- **Wave-12 ratification** (operator only, per 65-coordinator-handoff.md):
  D1 CTQ-06 LSL ratification (ctq_report.rs LSL_RATIFIED flip; docs
  amendment in 20-prd.md), D2 CTQ-04 p99/p50 protocol declaration.
- **Wave-13 commit** (operator only): D3 integration commit — atomic
  per-crate + one docs commit, `git commit -F <file>`, no push.
- dod.sh carries all three as PENDING rows (non-failing by design):
  the re-run verdict GATES GREEN 3 PENDING is the designed terminal
  class, not DONE.

## Corpus Re-Read at Use Time

- receipts/ holds 40 receipt files (README.md + template.md excluded);
  with gate-branchless-manifest.md (top level) the corpus is 41,
  matching 68-wave-10-receipt.md's 41 (29 ALIVE / 11 FAILED / 1 PARTIAL).
- Fresh DoD re-run this window: 21 manifest entries, green=18, 3
  PENDING, 0 RED, exit 0.

## Standing

v26.10.31 PARTIAL_ALIVE — engineering converged; three operator decisions
open (ratification wave-12, commit wave-13). Nothing committed.

## Verification Performed by This Lane

- `bash scripts/dod.sh` → exit 0, GATES GREEN 3 PENDING (fresh witness).
- `ls -la` / `grep` / `sed` disk checks for every pointer in both owned
  files.
- v3 receipt standings extracted from the receipt files
  (Exit/Standing rows), not copied from prose.
- NOT re-run by this lane: the 10-gate ladder and per-crate tests
  (C2's fresh run accepted at 05:46:41Z), cfg_homology (73
  re-witnessed it twice this session), per-crate benches.

## See Also

- 68-wave-10-receipt.md · 69-validation-receipt.md · 72-census-v2.md ·
  73-slack-adjudication.md · 74-attestation-mapping.md
- 66-dod-receipt.md · 67-dx-audit.md · 65-coordinator-handoff.md ·
  OPERATOR_DECISIONS.md (repo docs/ dir)
