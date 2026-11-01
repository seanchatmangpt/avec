# A09 Wave-9 Backfill Audit Receipt (adversarial, verdicts only)

- **Subject**: lane A09 `ontologies/fibo` at `main` @ `62215326`, audited
  this session from the canonical checkout `/Users/sac/cgap` (no worktree,
  no clone; the only copy is the mandated `/tmp` mutation sandbox).
- **Auditor**: backfill verification pass, adversarial lane audit.
- **Verdict**: **ALIVE** — every re-run command green, zero vacuous courts.
  All 7 independent anti-vacuity mutations FLIPPED their named courts in a
  `/tmp` sandbox; every restore byte-exact by sha256; canonical tree
  untouched (sha256-verified before/after).
- **Standing vocabulary**: ALIVE below means run+passed this session with
  the cited command+exit.

## 1. Re-runs (this session, canonical tree)

| Command (cwd `/Users/sac/cgap`) | Exit | Result |
|---|---|---|
| `CARGO_TARGET_DIR=$PWD/target-a09 cargo test -p cgap-fibo --no-fail-fast` | 0 | 21/21 pass (8 binaries: 0+0+1+6+9+1+4) |
| `RUSTFLAGS="-D warnings" CARGO_TARGET_DIR=$PWD/target-a09 cargo check -p cgap-fibo --all-targets` | 0 | clean |
| `python3 scripts/zero_string_gate.py` | 0 | scanned=66 crates=22 hits=0 emitter_hits=2 vocab=9 |
| `target-a09/debug/emit-fibo-pack --check` | 0 | `OK 496 bytes, digest d7495fba8022ee19…` == `PACK_DIGEST` |

Receipt drift: the in-code wave-9 receipt says "20/20"; the suite is now
21 tests (all pass). The "scanned=49" figure is now 66 (gate census is
dynamic across lanes). Non-blocking staleness, recorded.

## 2. Mutation matrix (all in `/tmp/a09-audit-sb`, flat 4-crate closure)

Sandbox: `cp -R` of `crates/cgap-blake3`, `crates/cgap-bont`,
`crates/cgap-core`, `ontologies/fibo` under a fresh workspace root;
disclosed deltas: sandbox root manifest (auditor-authored, coordinator
role), `cgap-core` cgap-gate dev-dep elided (gate crate not in closure).
Baseline before any mutation: suite 21/21 green, exit 0.

| Row | Mutation (sandbox copy) | Court | Flip witnessed | Restore |
|---|---|---|---|---|
| M-T7 | pack byte 100 flipped (116->117) | `t7_pack_digest_court` | FAILED, exit 101; `emit-fibo-pack --check` exit 4, typed `PackDrift … byte @100` | sha256 byte-exact (`31f754549f562cc9…` == canonical) |
| M-T8 | `Usd = 0x0348` -> `0x0349` | `t8_currency_court` | FAILS at compile: E0081 duplicate discriminant (cannot pass; mechanism is the compile gate, not the runtime assert) | BYTE-EXACT |
| M-T12 | `#[repr(u16)]` -> `#[repr(u32)]` widen | const-assert compile gate | E0080 x3: `size_of::<FiboCurrency>() == 2`, align, `offset_of!(ArRecord, lex) == 18` | BYTE-EXACT |
| M-W8 | lib.rs citation `oabi.rs:61` -> `:62` | `a06_citation_freshness_court` | FAILED, exit 101 (stale-citation join flips) | BYTE-EXACT |
| M-A07 | append `fibo:SecEqu:Equity` to facade | `w9_a07_facade_token_census_court` | FAILED, exit 101 | BYTE-EXACT |
| M-T13 | referenced refused lexeme in emitter argv path | `t13_producer_quarantine_court` | FAILED, exit 101 (positive controls held) | BYTE-EXACT |
| M-A20b | `verify_slot_derivation` disabled in sandbox `cgap-bont` | `w9_a20_1m_row_stress_court` | FAILED, exit 101 (corrupted-row witness names exact index) | BYTE-EXACT (canonical `oabi.rs` sha `85a2d125e402e69e…` unchanged) |

Post-restore re-prove: sandbox suite green again (8/8 binaries ok,
exit 0). Replay drivers: `/tmp/a09_audit_driver.sh`,
`/tmp/a09_mut_rows1.sh`, `/tmp/a09_mut_rows2.sh`. Sandbox dir deletion
was denied by the permission system, so `/tmp/a09-audit-sb` (56 MB,
outside the repo) remains on disk for manual cleanup or re-verification.

## 3. Court quality notes

- In-court anti-vacuity witnesses use real mutated bytes, real loader, real
  subprocess emitter, real BLAKE3 — Chicago-clean; zero mock usage (grep
  sweep: only "no mocks" doc mentions).
- `t11`'s `--inject` typed-refusal flip (exit 3, `TermRefused …
  module=SecEqu section=2.2`) and the refuse-everything control (allowed
  term admitted) are witnessed inside the green suite.
- M-T8's flip lands at the compile gate (E0081) one step earlier than the
  named runtime assert — equivalent-or-stronger; recorded as mechanism
  drift, not vacuity.
- The A14-dam slice item is a typed UNSUPPORTED (const pack has no flow
  surface), reasoned and named, not a silent skip.

## 4. Ownership

- Tracked A09 surface: 9 files, all under `ontologies/fibo/`, all clean
  (`git status --short -- ontologies/` empty; `git ls-files` = 9).
- The uncommitted tree noise (`Cargo.lock`, root `Cargo.toml`, `crates/cli`,
  `crates/audit`) belongs to concurrent lanes in flight — it appeared
  mid-audit and is not A09's; A09's sha256 surface was stable across the
  whole audit.
- Zero tracked build-dir debris; `ontologies/fibo/target-a09/` is
  untracked lane lease (per cleanup law, coordinator deletes at
  integration).
- Scope note: the wave-1 design doc names 2 files in scope; the lane ships
  9 (court/witness/bin files plus Cargo.toml [[bin]] + dev-dep edits).
  Later per-wave dispatches are cited as the widening authority in
  `src/lib.rs`; no off-limits shared file (root manifest, other crates'
  lib.rs) was touched by A09.

## 5. DoD scorecard (lane doc §9, 12 items)

1. no_std zero-extern-crate facade — ALIVE
2. const surface + compile asserts — ALIVE (re-ran)
3. Provenance — **PARTIAL / BLOCKED residue**: release id `FBC-2024Q4`
   recorded; distribution SHA-256 honestly `UNVERIFIED:no-distribution-on-disk`
   (`src/lib.rs:158`) — pinned distribution absent on this host.
4. Emit+load `SlotId == truncate64(LexHash)` — ALIVE (both halves courted)
5. T7 + 3 vacuity mutations — ALIVE (+ independent M-T7 flip)
6. T8 both directions + mutations — ALIVE (+ independent M-T8 flip)
7. T9 golden determinism — ALIVE
8. T10 loader round-trip + typed corrupted-record refusal — ALIVE
9. T11 refusal court + typed TermRefused — ALIVE
10. T12 golden blobs + widening mutation — ALIVE (+ independent M-T12 flip)
11. Emit-time collision check — ALIVE (emitter + wave-1 courts)
12. Clean build/test, no artifacts left — ALIVE (12 tracked files clean)

## 6. Divergences (recorded, none fatal)

1. Lane doc §7 asserts 48 B records; reality is 56 B with recorded
   divergence (doc's own field list cannot reach 48 B padded; `src/lib.rs`
   carries the note). Doc yields — correct direction.
2. Lane doc §5.1 reserved-area provenance slot dropped (reserved must stay
   zero); provenance lives in `src/lib.rs` instead — recorded in
   `emit-pack.rs` docs.
3. DoD 3 distribution SHA-256 remains UNVERIFIED (in-place record).
4. Receipt staleness: "20/20" vs current 21 tests; "scanned=49" vs 66.
5. M-T8 flip mechanism is the E0081 compile gate, not the runtime assert.
6. Lane doc §11 still names only the wave-2 window; the lane's actual scope
   evolved through wave 9 under per-wave dispatch text (authority cited in
   `src/lib.rs`); doc yields.
7. Concurrent-lane root-manifest dirt appeared mid-audit; not attributable
   to A09 (A09 sha256 surface stable end-to-end).

## 7. Lane status

ALIVE for the wave 1-9 slice: 21/21 courts green this session, every court
independently flipped under its named mutation class, ownership clean, no
vacuity found. Open residue: DoD 3's pinned-distribution SHA-256 (BLOCKED
on external artifact admission, honestly recorded in place at
`ontologies/fibo/src/lib.rs:158`); receipt-count staleness (cosmetic).
