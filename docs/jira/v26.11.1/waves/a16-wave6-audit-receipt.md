# A16 Wave 6 Audit Receipt

- **Lane**: A16 (`crates/cgap-crypto`: `chatman.rs`, `lease.rs`), Wave 6
  defect-repair, v26.11.1
- **Auditor**: adversarial audit lane (failed-lane restart), 2026-11-01
- **Subject**: `crates/cgap-crypto` on `main` @ c83619d (working tree,
  uncommitted lane work on top); repo `/Users/sac/cgap`, one canonical
  checkout
- **Verdict**: **ALIVE** — real gate 24/24 exit 0 (repo and independent
  sandbox baseline), all 8 attacked mutation classes flip their named courts
  (zero vacuous), ownership clean, repo left byte-exact (sha256 pre == post
  for all six A16 files). Mutations applied only in `/tmp/a16-audit-sb`
  (independent sandbox copied from repo sources; every restore sha256-
  verified byte-exact).

## Dispatched-subject note (D-A16-1)

No A16 wave-6 defect-repair receipt file exists anywhere on disk (repo-wide
`*a16*` find → only the lane doc `lanes/A16-chatman-pqc.md`). The lane's flip
receipts live in-code (court-file doc headers citing `/tmp/a16-w6/M*.log`)
and those logs do exist on disk (M1–M8, mtimes 04:46–04:49) — but /tmp is
ephemeral. This audit re-derived every claim from the landed artifact itself
and independently reproduced the flips.

## Verified surfaces (this session, command + exit)

| Surface | Spec | Evidence |
|---|---|---|
| Real gate | §6/§8 | `cargo test -p cgap-crypto` exit 0, 18 chatman + 6 lease passed |
| Keyed BLAKE3 KATs | §3.3 | official keyed vectors 0/1/63/64/192/1024 byte-exact |
| Delta/Phi codecs | §3.1/§3.2 | 112 B / 80 B layout pins compile; roundtrip + LE offsets green |
| Byte sweep | §8.3 | 458 flips (229 offsets x 2 masks), region-typed errors |
| Trap courts | §5/§8.2 | real subprocess traps (seal-exhaust, exhaust, ceiling-0) |
| Chaining / epoch | §8.4 | epoch swap → typed `DigestMismatch`; key-ignore does NOT break it |
| Determinism | §8.5 | byte-identical encoding across builds; end-to-end 458-hex pin |
| Hostile inputs | wave-6 | all-zero / all-0xFF / cross-tier magic → typed refusals, no panic |
| Sealer seam | §4 | `Unsealed` loudly typed (`0x0000`, "CGRU"/0x00); 68-flip seal sweep |

## Court attack (all mutations in `/tmp/a16-audit-sb` only)

Method: independent `cp -r` of cgap-crypto + cgap-blake3 into
`/tmp/a16-audit-sb` with a minimal standalone workspace manifest (dev
profile `panic = "abort"` matching the root workspace) + `LICENSE.md`;
sandbox sources sha256-identical to repo before mutating; baseline 24/24
green (plus cgap-blake3's own suite green); one mutation at a time; pristine
restore from the repo copy between mutations; sandbox file sha256-checked
after every restore; repo files sha256-checked pre and post audit
(byte-identical). Logs: `/tmp/a16-audit-sb/AUDIT-M*.log`. Runs used
`--no-fail-fast` (cargo's default fail-fast stops at the first failing test
target and hides the lease-side results).

All runs `cargo test --no-fail-fast`, exit 101 each time. Isolation noted
per row; flip-set counts are failed/passed per test target.

| Mut | Named mutation | Outcome |
|---|---|---|
| M1 | tick advance dropped (rewind-capable) | chatman 15/3 (c); lease 4/2 |
| M2 | exhaustion trap → warning return | lease 5/1, `exhaustion_court` only; chatman 18/18 (a) |
| M3d | constructor zero-ceiling trap removed | lease 5/1, `constructor_court` only |
| M4 | verify digest compare → first byte only | chatman 17/1, `byte_sweep_court` only |
| M5 | keyed digest ignores the key | chatman 15/3 (d); chaining green (b) |
| M5b | verify coverage drops phi+key | chatman 13/5, chaining primary (e) |
| M6 | per-call counter XOR into receipt byte 0 | chatman 16/2 (determinism + pin) |
| M7 | from_bytes magic check dropped | chatman 17/1, hostile court only |
| M8 | verify_seal first-byte-only compare | chatman 17/1, sealer court only |

(c) Chatman flips: roundtrip, phi-monotonicity, seal-exhaust probe. Lease
flips: monotonicity, current-tick-pure.
(d) Flips: keyed KAT, digest-coverage, end-to-end pin.
(e) Flips: chaining, byte sweep, pin, phi-monotonicity, roundtrip.

(a) The `Lease::phi()` defense-in-depth trap fires before `tick()` on the
seal path — the court header's claim, corroborated.
(b) Epoch binding is structural: `issued_root` sits inside the covered
input, so a key-ignoring verify still detects the epoch swap — the lane's
robustness-positive claim, corroborated. The suite still exits 101 under
M5 (the KAT court flips).

**Zero vacuous courts**: 8/8 mutation classes flip; M2/M3d/M4/M7/M8 isolate
to exactly their own court; M5's non-flip of chaining is the disclosed
robustness positive, and the suite still exits 101 under M5 (the KAT court
flips). Lane logs (`/tmp/a16-w6/M*.log`) match this audit's runs
test-for-test (M1 15/3+4/2; M4 17/1; M5 15/3 with chaining green; M5b 13/5
with the same five named tests).

## Divergences (doc yields; recorded, not fixed)

1. **D-A16-1 (missing subject)**: the dispatched A16 wave-6 defect-repair
   receipt does not exist on disk; flip receipts are in-code plus
   `/tmp/a16-w6/M*.log` (ephemeral). Do not cite an A16 lane receipt; cite
   this audit receipt plus the artifact itself.
2. **D-A16-2 (doc vs impl, disclosed in-code)**: §3.3 "117 bytes + seal
   size" vs pinned `229 + 2420 = 2649` (the doc figure omits Phi_lease 80 +
   digest 32); provisional `algorithm_id` registry values
   (0x0001/0x0002/0x0000) the doc never names; the unsealed-tier encoding
   ("CGRU"/0x00, divergence D2); the admissions tree construction
   (leaf/parent/odd-promotion/zero-root, divergence D3); lease_id randomness
   as caller policy (divergence D1); the wave-6 honest-scope note that
   `Unsealed` is MAC-class, not a signature (A18 D-A18-4 precedent). All
   disclosed in the module docs; doc yields.
3. **D-A16-3 (mutation wording)**: lane doc §8.1 names "add a set_tick/
   rewind path"; the in-code named mutation is the no-advance tick (a
   rewind-capable path). Both flip the §8.1 courts; wording differs; doc
   yields.
4. **D-A16-4 (M1 probe collateral)**: under M1 the lease exhaustion court
   stays green only because the probe child's own `current_tick == 8` assert
   fails first (child exits nonzero for the wrong reason). The exhaustion
   trap law itself is courted by M2, which flips the court directly. Minor.
5. **D-A16-5 (fail-fast visibility in lane logs)**: the lane's single-target
   logs are consistent with per-target reruns; this audit re-ran everything
   `--no-fail-fast` and got the same flip sets. Cosmetic.
6. **D-A16-6 (shared Cargo.lock)**: cgap-crypto's path dep on cgap-blake3
   lands in the workspace `Cargo.lock` (+80 lines, shared with other lanes'
   entries) — shared seam, coordinator-owned manifest; disclosed, not a lane
   write outside scope.

## Ownership check

`git diff --stat -- crates/cgap-crypto/` (read-only): 3 files — `Cargo.toml`
(+15: license-comment + cgap-blake3 path dep, inside the lane's own crate),
`src/chatman.rs` (+642), `src/lease.rs` (+220); plus untracked `tests/`
(`chatman_courts.rs`, `lease_courts.rs`). Coordinator-owned `src/lib.rs`
OFF-LIMITS: unmodified (sha256 `471ef344…` pre == post). No git write
commands (only `git status --porcelain`, `git diff`, `git rev-parse`); no
worktrees, no clones — `/tmp/a16-audit-sb` is the dispatch-sanctioned
mutation arena. Repo hashes byte-identical pre/post audit: chatman
`ecaf8406…4b80`, lease `b94fc446…7193`, Cargo.toml `a229dc49…7acc`.

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && cargo test -p cgap-crypto   # exit 0, 18+6 passed
git diff --stat -- crates/cgap-crypto/            # 3 files, ownership check
# sandbox /tmp/a16-audit-sb: cp -r cgap-crypto + cgap-blake3, standalone
# workspace manifest (dev panic=abort) + LICENSE.md; baseline 24/24; M1 M2
# M3d M4 M5 M5b M6 M7 M8 one at a time via python3 exact-string shims
# (assert count==1); restore from repo copy after each; sha256 after every
# restore; logs /tmp/a16-audit-sb/AUDIT-M*.log
```

## Standing

- Keyed BLAKE3 engine + delta/phi codecs + seal/verify + admissions root:
  **ALIVE** — `cargo test -p cgap-crypto` exit 0, 24/24, re-run in repo and
  sandbox baseline this session.
- Courts §8.1 (monotonicity), §8.2 (exhaustion, real subprocess trap),
  §8.3 (exhaustive byte sweep + sealer sweep), §8.4 (chaining/epoch),
  §8.5 (determinism), constructor ceiling-0 trap, hostile-input, KAT:
  **ALIVE** — 8/8 named mutation classes flip, zero vacuous.
- A16 wave-6 receipt file: **MISSING** (D-A16-1). Cite this audit receipt
  plus the artifact, never a lane receipt.
- Perf numbers: none claimed by the lane; none measured here.
- The §3.4 key-rotation story and the Wave-3 PQC (ML-DSA-65) seal remain
  OPEN by design (UNSUPPORTED-clause `Unsealed` tier loudly typed) — not
  defects, recorded scope.

## See Also

- `docs/jira/v26.11.1/lanes/A16-chatman-pqc.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a18-wave3-audit-receipt.md` (same audit method,
  shared-crate precedent)
- `crates/cgap-crypto/src/chatman.rs`, `crates/cgap-crypto/src/lease.rs`,
  `crates/cgap-crypto/tests/chatman_courts.rs`,
  `crates/cgap-crypto/tests/lease_courts.rs` (audited artifacts)
