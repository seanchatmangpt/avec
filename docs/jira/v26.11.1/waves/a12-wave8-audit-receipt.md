# A12 Wave 8 Audit Receipt

- **Lane**: A12 (`crates/cgap-bn3`: `src/unify.rs`, `src/project.rs`), Wave 8
  defect-repair / failed-lane restart wave, v26.11.1
- **Mode**: adversarial audit + failed-lane restart receipt (A15-wave8 /
  A08-wave7 method): no A12 wave receipt exists anywhere on disk
  (`docs/jira/v26.11.1/waves/` holds no a12 file), so every lane claim was
  re-derived from the landed artifact — the real gate was re-run, and every
  named anti-vacuity mutation was executed in a /tmp sandbox against the
  courts, with fail-closed sha256 restores
- **Subject**: repo `/Users/sac/cgap`, branch `main`, working tree on top of
  coordinator scaffold c83619d; ONE canonical checkout, no worktrees/clones;
  no git write commands run (read-only `git status`/`git diff --stat` only)
- **Verdict**: **ALIVE, zero vacuous courts** — real gate
  `CARGO_TARGET_DIR=target-a12 cargo test -p cgap-bn3` exit 0, **87/87
  passed** (run three times this session: in-repo pre-audit, sandbox
  baseline, in-repo post-audit re-run), and **11/11 mutation courts FLIP
  (0 VACUOUS)** in the sandbox. Repo left byte-identical: `unify.rs`
  `e2373305…0c17f`, `project.rs` `855cdcae…a022` before == after the whole
  audit.

## Court attack (all mutations in `/tmp/a12-w8-sb` only)

Method: standalone workspace at `/tmp/a12-w8-sb` (`crates/{cgap-bn3,
cgap-blake3}` + `[workspace.package]` v26.10.29 edition 2021 + dev
`panic = "abort"` matching the root + `LICENSE.md`; sandbox sources
sha256-identical to repo pre-mutation: `unify.rs e2373305…`,
`project.rs 855cdcae…`). Baseline `cargo test -p cgap-bn3 --lib` 87/87
exit 0. One mutation at a time via python3 exact-string replacement (anchor
count asserted == 1 before every write; two aborts on miscounted anchors —
M12-6, W5-M9 — fired BEFORE any write, sandbox verified unchanged); restore
from `.pristine` after every mutation with sha256 fail-closed compare
(11/11 restores byte-exact); sandbox == repo byte-exact at the end.
Runner: `cargo test -p cgap-bn3 --lib`; verdict FLIP = exit non-zero with
the named court in the failed set. Two panic-class mutations abort the
lib test binary early (profile.dev `panic = "abort"`), so their flip-sets
are partial by construction — recorded as-is, not normalized.

| Mutation (exact anchor replaced) | Named court | Flip-set (exit 101 each) |
|---|---|---|
| M12-1: `if w != const_pool[…]` → `if false && …` (mask AND→OR class) | differential court | differential, kernel single-atom, kernel cross-row, kernel no-match, projector empty-match (5) |
| M12-3: `cols.col(si)` → `cols.col(si ^ 1)` (probe column swap; si=2 hits `unreachable!` → aborts binary early) | differential court | differential, kernel cross-row, zero-alloc scan court (3, partial — abort) |
| M12-5: `for rule in &compiled` → `compiled.iter().skip(1)` | differential court | differential, driver self-derivation, driver round-budget (3) |
| M12-8: oracle `Some(&v) if v == have => {}` → `Some(_) => {}` | differential court (positive control) | **differential only** (isolated) |
| M12-6: `for i in start..end` → `(start..end).rev()` | determinism court | determinism, stress chunk-stability, 3 kernel courts, projector head-instantiation, digest court (7) |
| M12-7: `rows.push(…)` → consecutive-dedupe guard | M12-7 popcount court | M12-7 court, digest court (2) |
| W5-M7: `binds.clear(); seen.clear();` → fresh `Vec::new()` shadows | zero-alloc court | zero-alloc + 12 collateral (13 — abort class) |
| W5-M9: `matches.atom_rows.first().map_or(0, Vec::len)` → `atom_rows[0]` | default-empty-matchset court | that court **only** (isolated) |
| W5-M10: `other => unreachable!(…)` → `{ let _ = other; self.o }` | out-of-range-slot should_panic court | that court **only** (isolated) |
| DIGEST-REVERSE: `for i in 0..self.len()` → `(0..self.len()).rev()` in `write_canonical_bytes` | digest court | digest court **only** (isolated) |
| ALLOC-COUNTER-BYPASS: `ALLOCS.with(|c| c.set(c.get() + 1));` → comment | zero-alloc positive control | positive control **only** (isolated) |

Every named court flips under its named mutation. **A green court that
survives its mutation = VACUOUS: none observed.**

## Verified surfaces (this session, command + exit)

| Surface | Evidence |
|---|---|
| Real gate | `CARGO_TARGET_DIR=target-a12 cargo test -p cgap-bn3` exit 0, 87/87 (pre-audit, post-audit re-run identical) |
| Differential court (spec 7.2) | flips under M12-1, M12-3, M12-5, M12-8 — kills mask, join, rule-drop, and oracle classes |
| Determinism court (M12-6) | flips under chunk-order reversal; stress court co-flips |
| M12-7 popcount court | flips under projector dedupe |
| Zero-alloc court (spec 7.4) | flips under W5-M7 per-call alloc AND its own positive control flips under counter bypass |
| Digest court | flips under reverse-order canonical bytes (isolated) |
| Loud-slot court (W5-M10) | flips when the silent `_ => self.o` fallback is restored (isolated) |
| Alignment court (W5-M9) | flips when the guard reverts to `atom_rows[0]` (isolated) |
| Hygiene | `grep sleep(` / mock greps exit 1 (zero hits); unwrap/expect/panic confined to `#[cfg(test)]` + one courted `unreachable!` at `unify.rs:318` |

## Scope court

`git diff --numstat -- crates/cgap-bn3/` = exactly the six files of the
three bn3 lanes' leases: A11 (`arena.rs`, `sigil.rs`), A12 (`unify.rs`,
`project.rs`), A13 (`builtins.rs`, `Cargo.toml`). A12's own diff is
`unify.rs` + `project.rs` only — inside its lease. No A12-authored file
outside the lease. The crate's `Cargo.toml` edit is A13's disclosed unblock
(path dep `cgap-blake3`, disclosure comment in-file), not A12's.

## Divergences (doc yields; recorded, not silent)

1. **D-A12-1 (missing subject — the restart premise)**: no A12 wave receipt
   exists on disk (the in-file headers cite a "Wave-4 lane report" and a
   Wave-5 ledger that are not in the repo). This audit re-derives from the
   artifact; cite THIS file, not the absent reports.
2. **D-A12-2 (M12-2 BM0 gate)**: the BM0 branch-mispredict court is
   **BLOCKED**, registered honestly in `unify.rs:81-84` (divergence note 8):
   `benches/` was never created and macOS exposes no `perf` counter here.
   Lane doc Section 6 says a counter-less host degrades the gate to PARTIAL
   with the failure receipted; the artifact registers BLOCKED scope instead.
   Doc yields; the perf claims remain TARGETs, never claimed as measured.
3. **D-A12-3 (M12-4 N/A)**: pad-corruption mutation has no subject — the
   authoritative A11 statement layout has no pad field (recorded divergence
   2, `unify.rs:28-32`). Typed N/A with citation, not a silent skip.
4. **D-A12-4 (doc status stale)**: lane doc `A12-simd-unifier.md` Section
   Status still reads PROPOSED / TARGET while the artifact is at Wave-4/5
   hardening state with courted kernel + projector + driver. DoD 8 ("this
   document's status section updated … receipts land here as courts run")
   is unmet by the doc, met by the artifact.
5. **D-A12-5 (corpus shape)**: lane doc 7.1 specifies generated corpora of
   "1..=64 rules, bodies of 1..=4 atoms, up to 8 variables"; the implemented
   generator varies rows per seed over a FIXED 5-rule corpus covering every
   Section 7.1 rule class (single-match, join, self-join, constant-bound,
   head-swap; no-match covered by its own kernel court). Differential
   coverage = 100 seeds x 2 chunkings = 200 (corpus, batch) pairs, meeting
   DoD 2's >= 100.
6. **D-A12-6 (packaging)**: `naive.rs` lives inside `unify.rs`'s `#[cfg(test)]`
   module, not a feature-gated file (in-file divergence 7: Cargo.toml/lib.rs
   coordinator-owned); `benches/` and `tests/` directories absent (BLOCKED
   scope, divergence 8). Recorded in-file; listed here for the register.
7. **D-A12-7 (seam-mate edit)**: `crates/cgap-bn3/Cargo.toml` carries A13's
   disclosed minimal unblock (path dep `cgap-blake3`), disclosed in-file
   per the compile-freeze SLA. Not an A12 edit; no undisclosed seam edits
   found in A12's files.
8. **D-A12-8 (unreaped sandbox lease)**: `/tmp/a12-w8-sb` could not be
   deleted (rm denied by the permission system). It is outside the repo
   tree, holds no repo state, is byte-verified == repo, and is ephemeral.
   Coordinator may ignore it (D-A15-5 precedent). Repo tree untouched:
   the audit's only repo write is this receipt file.

## Standing

- Register unifier + projector (`unify.rs` + `project.rs`): **ALIVE** —
  gate exit 0, 87/87, three runs this session, repo byte-identical after.
- All spec 7.3 court classes with real subjects (M12-1/3/5/6/7/8 + 7.4 +
  W5-M9/M10 + digest + positive control): **ALIVE, flips witnessed** —
  11/11 FLIP, 0 VACUOUS.
- M12-2 BM0 perf gate: **BLOCKED** (`unify.rs:81-84`), honestly registered;
  all perf numbers remain TARGETs.
- M12-4: N/A by authoritative layout (typed, cited).
- Lane standing overall: **ALIVE** on every claim with a court subject;
  BLOCKED residue named and bounded (M12-2), no overclaim.

## See Also

- `docs/jira/v26.11.1/lanes/A12-simd-unifier.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a15-wave8-audit-receipt.md` (same restart
  method, D-A15-5 sandbox precedent)
- `crates/cgap-bn3/src/unify.rs`, `crates/cgap-bn3/src/project.rs`
  (audited artifacts)
