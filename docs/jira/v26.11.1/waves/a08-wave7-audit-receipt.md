# A08 Wave 7 Audit Receipt

- **Lane**: A08 (`crates/cgap-bont`: `src/shacl_compiler.rs`), Wave 7
  defect-repair driven by wave-6 court receipts, v26.11.1
- **Auditor**: adversarial audit lane (failed-lane restart), 2026-11-01
- **Subject**: `crates/cgap-bont` on `main` @ c83619d (working tree,
  uncommitted lane work on top); repo `/Users/sac/cgap`, one canonical
  checkout
- **Verdict**: **ALIVE** — real gate 30/30 exit 0 (repo and independent
  sandbox baseline), all 11 attacked mutation classes flip their named
  courts (zero vacuous; the two wave-7 named courts M12 and M13 flip
  in isolation), ownership clean, repo left byte-exact (sha256 pre ==
  post for all five crate files). Mutations applied only in
  `/tmp/a08-audit-sb` (independent sandbox, `rsync --exclude 'target*'`
  from repo sources; every restore sha256-verified byte-exact).

## Dispatched-subject note (D-A08-1)

No A08 wave-6 or wave-7 defect-repair receipt file exists anywhere on
disk (repo-wide `grep -rli a08 docs/` → only the lane doc
`lanes/A08-shacl-bitmask.md` and `lanes/A20-court-verdict.md`). The
lane's court map and flip receipts live in-code (module doc header,
`shacl_compiler.rs:18-37`, citing the T9/T10 wave-6 and T11/M13 wave-7
repairs). This audit re-derived every claim from the landed artifact
and independently reproduced all flips. Do not cite an A08 lane
receipt file; cite this audit receipt plus the artifact itself.

## Verified surfaces (this session, command + exit)

| Surface | Spec | Evidence |
|---|---|---|
| Real gate | doc §7 T1, §8 DoD-1 | `CARGO_TARGET_DIR=target-a08 cargo test -p cgap-bont` exit 0, 30/30 passed (repo) |
| Sandbox baseline | audit method | same suite on `/tmp/a08-audit-sb` (sources sha256-identical), exit 0, 30/30 |
| T2 constraint court | §7 T2 | masks 0b1111/0b0011/0b0000, bit table sorted-IRI; in-test M1a/M1b flips |
| T3 XOR court | §7 T3 | m_group 0b0101; in-test M2 flip |
| T4 closed-world court | §7 T4 | undeclared bit 6 → `IllegalProperty`; in-test M3 flip |
| T5 asm witness | §7 T5, §6 | real `--emit asm` scratch-crate subprocess ran `ok` this session; witness artifact `/Users/sac/cgap/target-a08/a08_t5_witness.s` (1466 B; `t5_validate` body, `popcnt`, call-bearing control `bl _t5_marker`) |
| T6 round-trip | §7 T6, DoD-7 | render/parse/re-render byte-equal, file round-trip lossless |
| T7 refusal corpus | §7 T7, DoD-3 | one fixture per registry row incl. both NotClosedShape readings |
| T8 report invariants | §7/§Wave-5/§Wave-7 | 9 tamper classes refuse incl. M13 sorted-BIT-rows |
| T9 IRIREF law | wave-6 | forbidden bytes typed at source line, fail-closed parse side |
| T10/T11 sh:closed law | wave-6/7 | duplicate and wrong-typed `sh:closed` → `MalformedShape` rows |
| Boundary / namespaces / parse errors | DoD-8, §10 | width-64 saturation, per-shape bits, typed parse errors |

## Court attack (all mutations in `/tmp/a08-audit-sb` only)

Method: `rsync -a --exclude 'target*'` of `cgap-bont` + `cgap-blake3`
into `/tmp/a08-audit-sb/repo` with a standalone workspace manifest
(`[workspace.package]` v26.10.29 edition 2021, dev `panic = "abort"`
matching the root workspace) + `LICENSE.md`; sandbox sources
sha256-identical to repo before mutating; sandbox baseline 30/30
green; one mutation at a time via python3 exact-string replacement
(anchor count asserted == 1 before every write); pristine restore from
`/tmp/a08-audit-sb/pristine.rs` after every mutation with sha256
fail-closed check; repo files sha256-checked pre and post audit
(byte-identical). Logs: `/tmp/a08-audit-sb/AUDIT-M*.log`. All runs
`cargo test -p cgap-bont --lib --no-fail-fast` (full lib suite per
mutation; flip-set = full failed list, exit 101 every time).

The compiler-level mutations below are **stronger** than the lane's
in-test mutations (M1a/M1b/M2/M3 mutate the emitted `CompiledShape`
inside the test): these attack the compiler itself, so a flip proves
the court catches real miscompiles, not just test-internal edits.

| Mut | Named mutation (compiler-level) | Outcome |
|---|---|---|
| M1c | mandatory-bit assignment disabled (`if false && prop.min_count == 1`) | t2+t3+t7e flip (11 green remain) |
| M2c | group-bit assignment dropped (`m_group \|= ... → ;`) | t3+t7e flip |
| M3c | `m_legal` forced open-world (`u64::MAX`) | t2+t3+t4+t6+t8 flip |
| M6 | malformed known-local detection disabled | **t7c only** |
| M7 | `"or"` dropped from `LOGICAL_CONSTRUCTS` | **t7b only** |
| M8 | property-level deny-by-default branch emptied | **t7d only** |
| M9 | mandatory-group contradiction threshold 2→3 | **t7e only** |
| M10 | duplicate `sh:closed` check disabled | t10+t11 flip |
| M11 | IRIREF forbidden-byte check disabled | **t9 only** |
| M12 | **wave-7 named**: `malformed:sh:closed` row emission disabled | **t11 only** — isolated flip |
| M13 | **wave-7 named**: parse-side sorted-BIT-rows law disabled | **t8 only** — isolated flip |

**Zero vacuous courts**: 11/11 mutation classes flip; M6/M7/M8/M9/M11/
M12/M13 isolate to exactly their own court. M10's collateral on t11 is
structural (t11 case (c) builds on the duplicate-`sh:closed` fixture),
not a detection gap. M1c/M2c/M3c collateral is exactly the set of
courts that assert the mutated mask word.

## Divergences (doc yields; recorded, not fixed)

1. **D-A08-1 (missing subject)**: the dispatched A08 wave-7
   defect-repair receipt (and any wave-6 court receipt file) does not
   exist on disk; court receipts are in-code only. Cite this audit
   receipt plus `shacl_compiler.rs`, never a lane receipt file.
2. **D-A08-2 (doc standing stale)**: `lanes/A08-shacl-bitmask.md` §11
   and the header status line still say crate `PROPOSED`/`NOT
   BUILT`/`PARTIAL_ALIVE`; reality: the crate exists, builds, 30/30
   green exit 0. The doc yields; standing is now ALIVE per this
   receipt.
3. **D-A08-3 (registry extension, disclosed in-code)**: doc §1 names 6
   refusal rows; the implementation carries a 7th (`MalformedShape`,
   `shacl_compiler.rs:93-104`) covering wrong-typed/duplicate known
   locals, unsatisfiable mandatory groups, and deny-by-default
   unclassified constructs. Extension is disclosed in the module docs;
   doc yields.
4. **D-A08-4 (court inventory grown)**: doc §7 names T2/T3/T4/T5 with
   in-test mutations M1/M2/M3; the implementation carries 18 tests
   incl. compiler-level classes M6-M13 and the T5 call-bearing
   control. Superset of the doc; no doc conflict on the named courts
   themselves.
5. **D-A08-5 (shared-crate seam)**: the `cgap-bont` working-tree diff
   also carries `oabi.rs` (+636) and `primitives.rs` (+935) and
   `Cargo.toml` (+4, comment cites A06 §9.1) — A06's surfaces in the
   shared crate; plus `crates/cgap-bont/target-a06/` (52 MB A06 build
   root nested **inside** the crate dir — cleanup-law violation to
   surface to the coordinator for integration deletion; not an A08
   write). A08's subject file is fully self-contained (std-only
   imports, `shacl_compiler.rs:40-41`; no `cgap_blake3` use), so this
   audit's court evidence stands independent of the seam-mates.
6. **D-A08-6 (lane build-root lease)**: `target-a08/` at repo root
   holds the lane's T5 witness artifact (`a08_t5_witness.s`) and build
   cache; it is a lease to delete at integration per the cleanup law,
   after the witness is preserved elsewhere if wanted.

## Ownership check

`git diff --stat -- crates/cgap-bont/` (read-only): 4 files —
`Cargo.toml` (+4), `src/oabi.rs` (+636), `src/primitives.rs` (+935),
`src/shacl_compiler.rs` (+2803). Coordinator-owned `src/lib.rs` and
root `Cargo.toml` OFF-LIMITS: unmodified (git status; sha256 pre ==
post). A08's subject file `shacl_compiler.rs` is the lane's own
surface. No git write commands (only `git status --porcelain`,
`git diff --stat`, `git rev-parse`, `git log`); no worktrees, no
clones — `/tmp/a08-audit-sb` is the dispatch-sanctioned mutation
arena. Repo hashes byte-identical pre/post audit (all five files,
incl. `shacl_compiler.rs` `1f37e2f0…e0d4c`).

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && CARGO_TARGET_DIR=target-a08 cargo test -p cgap-bont
    # exit 0, 30/30 passed
git diff --stat -- crates/cgap-bont/          # 4 files, ownership check
# sandbox /tmp/a08-audit-sb: rsync -a --exclude 'target*' cgap-bont +
# cgap-blake3, standalone workspace manifest (dev panic=abort) +
# LICENSE.md; baseline 30/30; M1c M2c M3c M6 M7 M8 M9 M10 M11 M12 M13
# one at a time via python3 exact-string shims (anchor count==1);
# restore from pristine.rs after each; sha256 fail-closed after every
# restore; logs /tmp/a08-audit-sb/AUDIT-M*.log
```

## Standing

- SHACL closed-world compiler (TTL subset parser, eligibility court,
  BitmaskLayout, mask evaluator, canonical report, parse-side
  invariants): **ALIVE** — `cargo test -p cgap-bont` exit 0, 30/30,
  re-run in repo and sandbox baseline this session.
- Courts: T2/T3/T4 (in-test M1/M2/M3), T5 (asm + control), T6, T7,
  T7b-T7e, T8 (incl. M13), T9, T10, T11: **ALIVE** — 11/11
  compiler-level mutation classes flip, zero vacuous.
- A08 wave-7 lane receipt file: **MISSING** (D-A08-1). Cite this audit
  receipt plus the artifact, never a lane receipt.
- Perf numbers: the doc's "2/3 instructions" claim is witnessed by T5's
  real asm (bounded ≤48 instruction assert, no calls, no backward
  edges) — asm-level claim only; no wall-clock numbers claimed or
  measured.
- Wave-2 items (W2-1..W2-5) remain OPEN by design (stage-2 value
  validation, nested shapes, two-level masks, ggen pack, migration) —
  recorded scope, not defects.

## See Also

- `docs/jira/v26.11.1/lanes/A08-shacl-bitmask.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a16-wave6-audit-receipt.md` (same audit
  method, failed-lane-restart precedent)
- `crates/cgap-bont/src/shacl_compiler.rs` (audited artifact)
- `target-a08/a08_t5_witness.s` (T5 asm witness artifact)
