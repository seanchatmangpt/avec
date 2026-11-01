# A18 Wave 9 Audit Receipt

- **Lane**: A18 (`crates/cgap-p28/src/asc450.rs`), Wave 9 cross-lane
  integration slice, v26.11.1
- **Auditor**: adversarial audit lane (restarted lane session), 2026-11-01
- **Subject**: `crates/cgap-p28` on `main` @ c83619d (working tree, uncommitted
  lane work on top); repo `/Users/sac/cgap`, one canonical checkout
- **Verdict**: **ALIVE** — canonical gate 34/34 exit 0 re-run this session
  (canonical tree and /tmp sandbox baseline both), all three wave-9 courts
  FLIP under their named anti-vacuity mutations (4/4 mutations, exit 101
  each, killer sets matching the in-code witnesses exactly), every restore
  sha256 byte-exact, canonical `asc450.rs` byte-identical pre == post
  (`a31bdc5d…5858`). Mutations were applied only in `/tmp/a18w9-audit-sb2`
  (left on disk; every restore hash-verified). Zero vacuous courts.

## Dispatched-subject note

No standalone A18 wave-9 receipt file exists anywhere on disk (repo-wide
`*a18*` find → lane doc + wave-3 audit receipt only). The wave-9 lane's
receipts live in-code only: `asc450.rs:1747-1753` (constant-anchor),
`1866-1872` (lineage amnesia + seal-verify drop), `2066-2074`
(counter-stall). This audit re-derived every claim from the landed artifact
(recurrence of the wave-3 audit's D-A18-1).

## Verified surfaces (this session, command + exit)

| Surface | Spec | Evidence (this session) |
|---|---|---|
| Real gate | §6 | `cargo test -p cgap-p28` exit 0, 34 passed / 0 failed (canonical, `--locked`); 34/34 sandbox baseline |
| W9-A04 identity anchor | §5 offset 40, A04 law | 640-bit-flip sweep green; constant-anchor mutant flips it (M1) |
| W9-A05 power-cut sweep | A05 torn-write law | cut k=1..=16 byte-transparent + cut-0 fork + 1352 torn flips green; both named mutants flip it (M2, M3) |
| W9-A20 millirow stress | A20 discipline | 1M events, exact terminal state green; counter-stall mutant flips it (M4) |
| A07 zero-string gate | lane command (module doc) | `--courts` exit 0 (13 courts, 1 loud [SKIP]); full scan exit 0, hits=0, scanned=49 crates=22 |
| Ownership | fan-out contract | 4 files in `crates/cgap-p28/`; `asc450.rs` wholly A18-owned; `attestation.rs`/`trap.rs`/manifest = A17 |

The three wave-9 courts were green in the canonical-tree baseline run before
any mutation (exit 0, 34/34, courts named in the passing list).

## Court attack (all mutations in `/tmp/a18w9-audit-sb2` only)

Method: fresh `cp -R` of cgap-p28 + cgap-blake3 + cgap-crypto into
`/tmp/a18w9-audit-sb2` with a minimal standalone root manifest + `LICENSE.md`
+ `LICENSE-P28-SIL.md` + `rust-toolchain.toml`; sandbox baseline 34/34 exit 0
with sandbox hash == canonical hash (`a31bdc5d…5858`); one mutation at a
time via a count==1 anchored patch (runner refuses count != 1, exit 2);
`cargo test -p cgap-p28 --lib` per mutation; restore from the canonical copy
after each; sandbox sha256 checked pre/post every mutation.

| Mut | Court — named mutation | Result (observed) |
|---|---|---|
| M1 | W9-A04 constant-anchor (`new`'s anchor → `[0u8; 32]`) | exit 101, 30/4 — c1, c2b, w9_a04, w9_a05. FLIP |
| M2 | W9-A05 lineage amnesia (`recover` counters → 0) | exit 101, 32/2 — w9_a05, c2b. FLIP |
| M3 | W9-A05 seal-verify drop (`if false && !sealed.verify`) | exit 101, 32/2 — w9_a05, c2b. FLIP |
| M4 | W9-A20 counter-stall (`event_count = 0` in `commit_event`) | exit 101, 27/7 — c1, c2b, c4, c5, c6, w9_a05, w9_a20. FLIP |

All four observed killer sets match the in-code witness claims EXACTLY —
including the counts (30/4, 32/2, 32/2, 27/7). No witness overclaim found in
the wave-9 slice. Zero vacuous courts; a green suite that survived any of
these would have failed the lane; none survived.

## Witness-hash observation

The in-code witnesses cite restore hash `76a67618…4400 (pre == post)`. The
current canonical file is `a31bdc5d…5858` — the cited hash is the
pre-witness-text revision (witness text was appended after the sweep), so it
is not re-derivable from today's file. Substance fully corroborated by this
audit's own mutation runs; recorded as an as-of-date nuance, not a defect.

## Divergences (doc yields; recorded, not fixed)

1. **Missing subject file**: no `a18-wave9-receipt.md` exists; wave-9
   receipts are in-code only (`asc450.rs:1747-1753, 1866-1872, 2066-2074`).
   Recurrence of wave-3 audit D-A18-1. Cite this audit receipt + the
   artifact; do not cite an A18 lane receipt file.
2. **Lane doc wave window**: A18 lane doc §9 says "Wave 3 ... this lane
   writes only this file"; the wave-9 slice is dispatch-driven. Doc yields.
3. **Witness hash is as-of-date**: `76a67618…4400` is the pre-witness-text
   revision hash; today's file is `a31bdc5d…5858` (see above).
4. **Disclosed seam unblock still present**: `cgap-crypto` path dependency in
   `crates/cgap-p28/Cargo.toml` with in-file disclosure (recorded as D-A18-5
   at wave 3). No new unattributed manifest edits.
5. **Lane build roots left on disk**: `target-a18/` and `target-a18w9/` at
   the repo root (untracked). Coordinator cleanup-law note: lane build roots
   are leases, delete at integration.
6. **Prior finding D-A18-2 closed**: the C3 witness overclaim flagged by the
   wave-3 audit was repaired in wave-6 (`asc450.rs:1330-1345` now carries the
   re-derived 22/8 receipt citing the audit finding). Positive closure
   witness, not a defect.

## Fence compliance

- No `Process.sleep` / mock machinery in `asc450.rs` (grep clean).
- No git write commands; only read-only `git status`/`diff`/`rev-parse`.
  Canonical tree untouched by the audit: `asc450.rs` sha256 `a31bdc5d…5858`
  pre == post; repo writes limited to this receipt file.
- Skips loud: the zero-string gate's one [SKIP] is printed with its reason
  (T5 cross-extractor, `.text` placement not achievable on this target).

## Commands (exact, re-runnable)

```
CARGO_TARGET_DIR=/tmp/a18w9-audit-t cargo test --locked -p cgap-p28
    # exit 0, 34 passed / 0 failed (canonical tree)
cd /tmp/a18w9-audit-sb2 && cargo test -p cgap-p28 --lib
    # exit 0, 34/34 sandbox baseline; M1..M4 one at a time via
    # mutate.py (count==1 enforced); per-mut logs run_M*.log;
    # restore+sha256 after each; all four exit 101, restores exact
python3 scripts/zero_string_gate.py --courts   # exit 0, 13 courts, 0 failed
python3 scripts/zero_string_gate.py            # exit 0, hits=0, scanned=49
shasum -a 256 crates/cgap-p28/src/asc450.rs    # a31bdc5d…5858 pre == post
git diff --stat -- crates/cgap-p28/            # 4 files, ownership check
```

## Standing

- Wave-9 mechanism + 3 courts: **ALIVE** — canonical 34/34 exit 0 re-run
  this session; every named mutation flips its court (4/4, exit 101).
- Prior courts C1..C7 (unchanged this wave): **ALIVE** — green in baseline;
  M4 re-flips c1/c2b/c4/c5/c6, M1 re-flips c1/c2b (matrix-scale
  re-witnessing of the wave-3 attack set).
- Standalone A18 wave receipt file: **MISSING** (divergence 1). Cite this
  audit receipt plus the artifact itself.
- Perf numbers: the module doc names none; none measured here. The 1M-event
  stress is a correctness court, not a benchmark claim.

## See Also

- `docs/jira/v26.11.1/lanes/A18-asc450-proof.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a18-wave3-audit-receipt.md` (prior audit;
  D-A18-1/D-A18-2 lineage)
- `crates/cgap-p28/src/asc450.rs` (audited artifact)
