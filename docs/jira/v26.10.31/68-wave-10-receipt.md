# Wave 10 Receipt: DX/QoL Completion (20 lanes + 3 gap lanes)

Wave 10 (2026-10-30) turned the finished engineering into a
self-diagnosing system. Every lane's claims below were verified against
disk; the coordinator re-ran the terminal gates.

## Lanes

| Lane | Deliverable | State |
|---|---|---|
| L1 | scripts/dod.sh + dod-manifest.toml (21 entries, selftest court) | LANDED — selftest 10/10, real run: 18 GREEN / 3 PENDING / 0 RED |
| L2 | crates/cli/src/bin/doctor.rs (10 checks, --json) | LANDED — real run 7 GREEN / 2 YELLOW / 1 RED (dod.sh exec bit, since fixed) |
| L3 | justfile (8 recipes, no logic duplication) | LANDED — just --list verified, receipts+courts run green |
| L4 | docs/GETTING_STARTED.md | LANDED — every command executed; 2 drift lines fixed by coordinator post-audit |
| L5 | Error ergonomics (36 variants → what/why/fix) | LANDED — 36 PASS, zero behavior change, counts unchanged |
| L6 | .github/workflows/ci.yml (3 jobs, local-first mirror) | LANDED — yaml-validated; fmt-clippy non-blocking (7,390-line drift measured) |
| L7 | clippy+fmt mechanical pass | LANDED — clippy exit 0 (uninit_vec allowlisted), scoped fmt 0 hunks, baselines exact |
| L8 | scripts/receipt_doctor.sh (living freshness scanner) | LANDED — 40 receipts classified, 15-item refresh worklist |
| L9 | docs/TROUBLESHOOTING.md (9 failure classes) | LANDED — all file:line cites re-verified; +class 10 added by coordinator |
| L10 | docs/diataxis/README.md router | LANDED — 39 links checked, 0 broken |
| L11 | crates/cli/src/bin/status.rs (--json) | LANDED — dashboard honest incl. CTQ-06 FAIL |
| L12 | examples/sovereign-e2e demo | LANDED — run exits 0 with narrated plane crossings; shim court flips |
| L13 | docs/FEATURE_MATRIX.md | LANDED — wasm re-proofs fresh (6 builds incl. new wasip1 blake3 receipt) |
| L14 | scripts/nightly_perf.sh + first perf log | RELAUNCHED — pending at receipt time |
| L15 | 66-dod-receipt.md | LANDED — GATES GREEN, 3 PENDING, embedded verdict byte-verified |
| L16 | docs/OPERATOR_DECISIONS.md | LANDED — full edit set with corrected file:line cites |
| L17 | .gitignore + 71-hygiene-audit.md | LANDED — clean-clone residue: empty |
| L18 | 67-dx-audit.md (measured scorecard) | LANDED — 11/12; verify full-ladder 170s measured |
| L19 | README.md refresh | LANDED — shipped reality, honest CTQ-06 red |
| L20 | this receipt | coordinator-authored |
| G1 | e2e git stage wired (was SKIP) | LANDED — admission_e2e 2/2 with real git fsck in-loop; stale texts fixed |
| G2 | gate-git-tests-v3 + gate-cpk-study-v3 minted; 58/61 errata | LANDED — corpus now 41 receipts (29/11/1) |
| G3 | untranspose manifest entry + ADR-17 + mint_receipt.sh sed-escape fix | LANDED — gate green, blind spot documented-and-armed |

## Terminal State

- Definition of Done: `bash scripts/dod.sh` → GATES GREEN, 3 PENDING
  (operator decisions), 0 RED. Selftest court proves the checker flips.
- `avec doctor` / `avec status`: one-command health + dashboard.
- Everything a new developer needs: GETTING_STARTED → TROUBLESHOOTING →
  diataxis router → FEATURE_MATRIX → OPERATOR_DECISIONS.
- Independent validation (69-validation-receipt.md): 15 VALIDATED, 1
  PARTIAL, 1 REFUTED-recorded (NEON transpose, ADR-17).

## The operator decision list (unchanged, now fully mechanical)

1. RATIFY CTQ-06 — edit set A/A2/C/D/E/F per docs/OPERATOR_DECISIONS.md
   (mechanics verified; dry-run proof at /tmp/ctq_dryrun/).
2. RATIFY CTQ-04 — declare the p99/p50 protocol; quiet re-run.
3. APPROVE COMMIT — atomic per-crate + docs commits, -F file, no push.

Standing: v26.10.31 PARTIAL_ALIVE pending exactly those three decisions.
Nothing committed.

See Also: 66-dod-receipt.md, 69-validation-receipt.md,
docs/OPERATOR_DECISIONS.md, 65-coordinator-handoff.md.
