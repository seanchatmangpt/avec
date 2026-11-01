# A18 Wave 3 Audit Receipt

- **Lane**: A18 (`crates/cgap-p28/src/asc450.rs`), Wave 3, v26.11.1
- **Auditor**: adversarial audit lane (failed-lane restart), 2026-11-01
- **Subject**: `crates/cgap-p28` on `main` @ c83619d (working tree, uncommitted
  lane work on top); repo `/Users/sac/cgap`, one canonical checkout
- **Verdict**: **ALIVE** — real gate 30/30 exit 0 (repo and sandbox baseline),
  all 9 attacked courts flip under their named mutations (zero vacuous),
  ownership clean, repo left byte-exact (sha256 `936f2151…` pre == post).
  Mutations were applied only in `/tmp/a18-sb` (left in place — sandbox
  `rm` was denied by the permission system; every restore sha256-verified).

## Dispatched-subject note

No A18 wave receipt file exists anywhere on disk (repo-wide `*a18*` find →
only the lane doc). The prior in-code citations pointed at that missing
receipt and were removed as hearsay by the wave-5 repair
(`asc450.rs:1083-1093`, `1318-1329`). This audit therefore re-derived every
claim from the landed artifact itself (divergence D-A18-1).

## Verified surfaces (this session, command + exit)

| Surface | Spec | Evidence |
|---|---|---|
| Real gate | §6 | `cargo test -p cgap-p28` exit 0, 30 passed / 0 failed |
| Boundary law | §4.2 | C1 green; `>=` mutant flips it (M1) |
| Tamper evidence | §5/C2 | 1344-flip sweep (asc450.rs:1166-1180); weak-verify mutant flips (M4) |
| Reset detection | §4.4/C2b | non-monotone triple; verify-drop mutant flips it (M8) |
| Never-emit vacuity gate | C3 | suite exits 101 under the mutant (M2) |
| Latch + chain fork | §4.2/4.4/C4 | genesis-chain mutant flips it (M5) |
| Overflow rejection | §4.3/C5 | saturating mutant flips it (M3b) |
| Wire layout | §5/C6 | BE-seq mutant flips it (M6) |
| No wall clock + determinism | §4.3/C7 | needle insert flips c7 (M7); c7b green |

Edits applied per mutation (exact, sandbox file only):

- M1: `next > ceiling` → `next >= ceiling` in the crossing predicate.
- M2: predicate line → `let crossing = false;`
- M3: first attempt left `.ok_or` in the chain — compile-abort E0599,
  recorded as such, not a flip witness; M3b redid it chain-balanced:
  `checked_add(..).ok_or(..)?` → `saturating_add(..)`.
- M4: proof `verify` fold-compare → `expected[0] == signature[0]`.
- M5: `prev_proof_digest := GENESIS_PREV_PROOF_DIGEST` in `emit_proof`.
- M6: `crossing_seq` LE → BE in `SignedProof::to_bytes`.
- M7: appended `SystemTime` text line to the module.
- M8: `SealedMeterState::verify` → always `true`.

## Court attack (all mutations in `/tmp/a18-sb` only)

Method: `cp -r` of cgap-p28 + cgap-blake3 + cgap-crypto into `/tmp/a18-sb`
with a minimal standalone workspace manifest + `LICENSE.md` +
`LICENSE-P28-SIL.md`; baseline 30/30 green; one mutation at a time; pristine
restore from the repo copy between mutations; sandbox file sha256-checked
after every restore; repo file sha256-checked pre and post audit.

| Mut | Court — named mutation | Outcome (`cargo test -p cgap-p28`) |
|---|---|---|
| M1 | C1 emit-at-ceiling `>=` mutant | exit 101, 26/4 — FLIP c1+c2b+c3+c4 (matches witness) |
| M2 | C3 never-emit mutant | exit 101, 22/8 — FLIP 8 asc450 courts (D-A18-2) |
| M3 | C5 saturating add | compile-abort E0599 (recorded); M3b exit 101 — FLIP c5 only |
| M4 | C2 weak verify | exit 101 — FLIP c2 only |
| M5 | C4 genesis chain | exit 101 — FLIP c4 only |
| M6 | C6 BE-seq wire flip | exit 101 — FLIP c6 (+c2, c3) |
| M7 | C7 wall-clock needle | exit 101 — FLIP c7 only |
| M8 | C2b state-verify drop | exit 101 — FLIP c2b only |

**Zero vacuous courts**: every attacked court flips under its named mutation
class, exit 101 each time, with M4/M5/M7/M8 isolating to exactly their own
court.

## Baseline corroboration of in-code witness claims

- M1 witness (`asc450.rs:1089-1093`) **corroborated exactly**: 26 passed /
  4 failed, flipping c1 + c2b + c3 + c4, exit 101, restore byte-exact.
- M2 witness (`asc450.rs:1322-1329`) **overclaims**: says the never-emit
  mutant fails "ALL TEN asc450 courts"; observed 8 of 12 asc450 tests failing
  (c7 textual gate, consts_freeze, layout_court_c6_witness,
  genesis_spoliated_attestation_refused survive). The suite-level vacuity law
  holds (exit 101); the count claim is wrong.

## Divergences (doc yields; recorded, not fixed)

1. **D-A18-1 (missing subject)**: the dispatched A18 wave receipt does not
   exist on disk; the lane's witness receipts live only in-code, and the two
   surviving in-code witness blocks were partially hearsay-corrected by the
   wave-5 repair. Audit re-derived everything from the artifact.
2. **D-A18-2 (witness overclaim)**: C3's in-code witness count "ALL TEN
   asc450 courts failing" is wrong (8 observed). Safe direction (suite still
   exits 101), but the claim is inaccurate on disk today.
3. **D-A18-3 (API shape)**: doc §4.1/§4.2 `record_* -> Option<SignedProof>`;
   implemented `Result<Option<SignedProof>, Asc450Error>` — typed overflow
   rejection, disclosed in-code as divergence D2. Doc yields.
4. **D-A18-4 (seal primitive)**: doc §5 "signature ... Ed25519-class";
   implemented keyed-BLAKE3 lease-epoch MAC (`LeaseEpochSealer`), disclosed
   in-code as divergence D3. Honest scope: verification requires the epoch
   key — a MAC, not asymmetric non-repudiation. C2's tamper-evidence law
   holds regardless.
5. **D-A18-5 (seam unblock, disclosed)**: `cgap-crypto` path dependency added
   to `crates/cgap-p28/Cargo.toml` (A17's authority file) with in-file
   disclosure citing the A17 precedent; recorded here per the audit contract.
6. **Genesis superset**: doc §4.1 `new(ceiling)`; implemented
   `new(ceiling, attestation, sealer)` fail-closed + `recover(...)` (§4.4-3),
   disclosed in-code (D7). Stricter than doc; doc yields.
7. **Sandbox cleanup blocked**: `rm -rf /tmp/a18-sb` denied by the permission
   system; the sandbox and its per-mutation logs (`M*.log`) remain on disk.
   The repo tree was never touched by the audit.

## Ownership check

`git diff --stat -- crates/cgap-p28/` (read-only): 4 files — `Cargo.toml`
(+12: the `cgap-crypto` block is A18's disclosed seam unblock, the
`cgap-blake3` block is A17's), `asc450.rs` (+1687, A18-owned),
`attestation.rs` (+1280) and `trap.rs` (+744) are A17's. No writes outside
lane scope; no git write commands (only `git status --porcelain` and
`git diff --stat`); no worktrees, clones, or repo copies — `/tmp/a18-sb` is
the dispatch-sanctioned mutation arena. Repo `asc450.rs` sha256
`936f21514b2a5d2d73a8432457fadd57b304273c2120305a2114b097dc8babb7`
byte-identical pre and post audit.

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && cargo test -p cgap-p28   # exit 0, 30 passed / 0 failed
shasum -a 256 crates/cgap-p28/src/asc450.rs    # 936f2151…babb7, pre == post
git diff --stat -- crates/cgap-p28/            # 4 files, ownership check
# sandbox /tmp/a18-sb: cp -r cgap-p28 + cgap-blake3 + cgap-crypto, own
# workspace manifest; baseline 30/30; M1..M8 one at a time; sha256 restore
# check after each; per-mutation logs /tmp/a18-sb/M*.log; M3 compile-abort
# recorded, redone as M3b
```

## Standing

- Emitter mechanism + 12 asc450 tests: **ALIVE** — `cargo test -p cgap-p28`
  exit 0, 30/30, re-run in repo and sandbox baseline this session.
- Courts C1, C2, C2b, C3, C4, C5, C6, C7, C7b: **ALIVE** — 9/9 flip under
  their named mutation classes, exit 101, zero vacuous.
- Suite-level vacuity gate (never-emit mutant): **ALIVE** — exit 101.
- A18 wave receipt file: **MISSING** (D-A18-1). Do not cite an A18 lane
  receipt; cite this audit receipt plus the artifact itself.
- Perf numbers: none claimed by the lane; none measured here.

## See Also

- `docs/jira/v26.11.1/lanes/A18-asc450-proof.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a17-wave2-audit-receipt.md` (shared-crate audit)
- `crates/cgap-p28/src/asc450.rs` (audited artifact)
