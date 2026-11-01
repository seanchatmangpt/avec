# A11 Wave 4 Audit Receipt

- **Lane**: A11-bn3-arena (`crates/cgap-bn3`: `arena.rs`, `sigil.rs`), Wave 4
  (harden + close DoD), v26.11.1
- **Auditor**: adversarial audit lane, 2026-11-01, repo `/Users/sac/cgap`
  (one canonical checkout, branch `main`, HEAD `c83619d`)
- **Subject audited**: the Wave 4 lane work in the working tree; the wave
  receipt file the lane's module doc cites
- **Verdict**: **PARTIAL** — every materially testable DoD claim is ALIVE
  under independent adversarial re-run, but the Wave 4 report the module
  doc cites is absent from the repo and DoD 8 is unmet.

## 1. The receipt under audit does not exist on disk

- `docs/jira/v26.11.1/waves/` contains only `a17-wave2-audit-receipt.md`,
  `a20-wave3-audit-receipt.md`, and `W5-baselines-hardening-prd-ard.md`.
- No `a11-wave4*` receipt exists anywhere in the repo (`find` + `grep -r`
  over `docs/`, repo-wide `.md` sweep: zero hits).
- `crates/cgap-bn3/src/arena.rs:30-41` (module doc) claims five mutation
  receipts "ran this session ... receipts in the Wave 4 report" — the
  cited artifact is missing; that claim is UNVERIFIED as documented, and
  is re-proven below from my own runs instead.
- The lane's working artifacts exist only in `/tmp` (`a11-mut-M*.out`
  00:44, `a11-audit/*` 01:53-01:57, `a11-shim-courts.sh` 01:01). The v1
  logs predate the final `arena.rs` edit (mtime 02:16) — stale relative
  to the audited code. The v2 rerun (scratch restore mtimes 02:20:06 /
  02:20:53; `a11-M4-refuse-run.log`, `a11-M5-scope-swap-run.log` showing
  real test flips) postdates the final owned-file edits; no captured
  output exists for its M1/M2/M3 cases. My runs close that gap.

## 2. Baseline re-run (this session, canonical tree)

- `cargo test -p cgap-bn3 --lib` → exit 0, **78 passed / 0 failed**
  (includes all 19 A11-owned courts in `sigil::tests` + `arena::tests`).
- A11-owned files contain zero `std::` paths (core/alloc only). The crate
  lacks `#![no_std]` in `lib.rs` — coordinator scaffold file, lanes
  barred; recorded as a scaffold divergence, not a lane defect.

## 3. Anti-vacuity mutation matrix (my independent /tmp sandbox)

Sandbox: `/tmp/a11-adv` = copied `cgap-bn3` + zero-dep `cgap-blake3` under
a minimal root workspace. Sandbox `arena.rs`/`sigil.rs` hashes equaled the
canonical hashes (`ae6b2fbf…`, `d946a186…`) at start. BEFORE run green
(78/78). Canonical tree verified byte-unchanged before/after the whole
audit (`/tmp/a11_canonical_before.sha` == after; the four hashes identical).

| # | Mutation (spec Section 8) | Flip mode | Result (this session) |
|---|---|---|---|
| M1 | widen `s: u64` → `u128` (sigil.rs) | build | E0080 const-assert + E0308, rc=101 |
| M2 | `SlotConst = 0x00` → `0x01` (sigil.rs) | build | E0081 duplicate discriminant, rc=101 |
| M3 | `close()` `end` → `end + 1` (arena.rs) | test | 6 courts FAILED; 72/6, rc=101 |
| M4 | `decode_term` Err arm → const fallback | test | 3 courts FAILED; 75/3, rc=101 |
| M5c | full `VarUni`/`VarExi` swap (spec's named M5) | test | 26 courts FAILED; 52/26, rc=101 |

Courts flipped per case (all observed this session):

- M3: `flatten_court_nested_quotation_intervals`,
  `flatten_court_overlap_mutation_flips_verifier`,
  `depth_boundary_court_64_ok_65_refuses`,
  `scope_court_innermost_containing_interval`,
  `arena_court_second_program_appends_disjoint`,
  `refusal_court_all_seven_spec_rows`.
- M4: `refusal_court_sigil_rows_typed`,
  `statement_court_encode_decode_validate`, plus one further test.
- M5c: `scope_court_innermost_containing_interval` and
  `abi_court_sigil_codes_match_spec` among the 26.

- 5/5 spec-named mutations flip; zero vacuous courts. The spec's M5 is the
  full swap — half-swaps flip only via E0081 build error, so the
  test-level flip was proven with the full swap (M5c).
- Every restore sha256 byte-exact; the runner fail-closed on any mismatch.
- Audit-method note: my first M3 shim produced a parse error (shorthand
  `end,` cannot take `end + 1`), which flips the BUILD, not the court.
  The lane's own v1 script (`/tmp/a11-shim-courts.sh`, 01:01) had the
  same defect; its v2 (`a11-shim-M3-close.py`, 02:19, `end: end + 1`) is
  the valid form. Any receipt citing the v1 M3 is a build-flip
  masquerading as a court flip; the v2 run and my redo are the valid
  evidence.

## 4. Ownership check (read-only)

- A11-owned files: `arena.rs`, `sigil.rs`. Content is in-scope (arena,
  sigil ABI, courts); no evidence of A11 edits outside its ownership.
- In-file disclosures: `[A13 unblock 2026-11-01]` in `sigil.rs:314-318`
  (const-assert fix, restated by A11) and `Cargo.toml:9-16` (blake3 path
  dep, coordinator review requested) — seam-mate edits, disclosed per
  contract. `git diff --stat` is aggregate across the lanes sharing
  `cgap-bn3` (A12 `unify.rs`/`project.rs`, A13 `builtins.rs`), so
  per-lane attribution comes from the in-file disclosure record,
  not git.

## 5. DoD scorecard (lane doc Section 9)

| DoD | Status | Evidence |
|---|---|---|
| 1 crate builds, no_std intact | PARTIAL | green this session; `lib.rs` lacks `#![no_std]` |
| 2 size courts green | ALIVE | exit 0 this session |
| 3 round-trip court green | ALIVE | exit 0 this session |
| 4 flatten court green | ALIVE | exit 0 this session |
| 5 refusal court green (7 rows) | ALIVE | `refusal_court_all_seven_spec_rows` green |
| 6 scope court green; A12 cited | ALIVE | scope court green; A12 cited (sigil.rs:44-52) |
| 7 five mutation receipts, restores | PARTIAL | re-proven (5/5 flips); report MISSING |
| 8 Wave-1 freeze co-signed A01/A06/A17 | NOT MET | no freeze ticket exists anywhere under `docs/` |

## open items (coordinator)

1. Land the missing Wave 4 report (the lane's /tmp v2 evidence is the raw
   material; M1/M2/M3 need re-capture or citation of my re-run).
2. DoD 8: mint the Wave-1 freeze ticket with A01/A06/A17 co-signature, or
   downgrade DoD 8 in the lane doc.
3. Scaffold gap: add `#![no_std]` to `crates/cgap-bn3/src/lib.rs`
   (coordinator-owned file) or amend DoD 1.
4. Stray untracked `atomic_commit.rs` at repo root (belongs to the
   cgap-storage lane family, not A11).

## Divergences (doc yields; recorded)

- Spec Section 6 pseudocode closes the parent header pre-order, which
  contradicts its own `verify_intervals_disjoint_and_nested` + the
  Section 4.4 child ⊆ parent law; implementation closes post-order.
  Recorded in `arena.rs:15-24` — implementation is the lawful reading.
- Spec 4.1 "bit 55 always 0" contradicts the 56-bit payload bound; the
  MAY-assert is not taken (`sigil.rs:210-213`).
- `BN3Error::StatementIndexOverflow` is a named extension beyond the
  seven-row Section 7 table (recorded, sigil.rs:148-153).
- Lane doc scope = `arena.rs` + `sigil.rs` only; reality is a shared
  crate (A12/A13 files present) — expected under the shared-crate seam
  rule, disclosed in-file.
