# A13 Wave 9 Audit Receipt

- **Lane**: A13-bn3-builtins (`crates/cgap-bn3/src/builtins.rs`), adversarial
  audit of the wave 2-8 lane work, v26.11.1
- **Auditor**: adversarial audit lane, 2026-11-01, repo `/Users/sac/cgap`
  (one canonical checkout, branch `main`, HEAD `c83619d`)
- **Subject audited**: no `a13-wave*` receipt file exists in
  `docs/jira/v26.11.1/waves/` — the receipts under audit are the module-doc
  receipt blocks in `crates/cgap-bn3/src/builtins.rs:29-143` (waves 2-8) plus
  the lane's /tmp evidence (`/tmp/a13w8/`). Every materially testable claim
  was re-proven this session against the current canonical bytes.
- **Verdict**: **PARTIAL** — every materially testable DoD claim is ALIVE
  under independent adversarial re-run (8/8 named anti-vacuity mutations
  flip, zero vacuous courts), but the wave-receipt FILE the module doc cites
  is absent from the repo (coordinator open item, unchanged since wave 6).

## 1. Baseline re-run (this session, canonical tree)

- `cargo test -p cgap-bn3 --lib` → exit 0, **87 passed / 0 failed**
  (all 26 `builtins::tests` courts among them).
- `RUSTFLAGS="-D warnings" cargo check -p cgap-bn3 --lib` → exit 0.
- `cargo rustc --release -p cgap-bn3 --lib -- --emit=asm` (fresh compile;
  artifact `target/release/build/cgap-bn3/f68f3465…/out/cgap_bn3-*.s`):
  dispatch region `_RNvNtCsilLg8LBkrzA_8cgap_bn38builtins8dispatch`, 236
  non-empty body lines, bounded by the adjacent `BuiltinError`
  `Display::fmt`:
  - 0 `blr`, 0 `br xN`, 0 `brk`;
  - 1 `adrp` → `l_anon.…@PAGE` + `add …@PAGEOFF` (anonymous constant-pool
    data load, not a function-pointer table);
  - exactly 3 direct `bl`: `cgap_blake3::hash`,
    `cgap_blake3::hash_many_simd`, `alloc::raw_vec::grow_one`.
  - whole-object census: 6 `br x3` sites, all inside
    `cgap_bn3::sigil::BN3Error Display::fmt` (A11's function) — matches the
    wave-8 claim. The §2.1/§5.3 zero-indirection exit gate is ALIVE as an
    emitted-code fact, re-verified this session (lane doc §5.3 UNVERIFIED
    blocking fact is resolved).
- `b3sum` of `abc` = `6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85`
  — matches the `known_answer_crypto_blake3_abc` literal byte-for-byte
  (DoD 4's "real BLAKE3" fence re-proven with the official tool).
- Zero `unsafe` (only a doc comment mentions the word); zero `dyn`/`Box<dyn`
  in `builtins.rs`; no `std::` path in the module body (the
  `core::error::Error` impl is the wave-5 repair).

## 2. Anti-vacuity mutation matrix (independent /tmp sandbox)

Sandbox `/tmp/a13-adv-sb`: copied `cgap-bn3` + `cgap-blake3` under a minimal
root workspace; sandbox `builtins.rs` sha256 `5060b702…` == canonical at
start; BEFORE run green (**87/87**, exit 0). Runner fail-closed: count==1
anchor assert, byte-exact restore (sha256), pre-condition sandbox==canonical
before every mutation.

| # | Mutation (spec §5.2 + wave-6/7/8 named set) | Flip mode | Result (this session) |
|---|---|---|---|
| M1 | `OP_MATH_SUM` 0x11 → 0x10 | build | E0080 (`OP_MATH_SUM == 0x11`), rc=101 |
| M2 | delete the `OP_MATH_MIN` dispatch arm | test | 84/3: `known_answer_math_min`, `all_registered_opcodes_dispatch`, `registry_exhaustively_closed_sweep`; refusal is typed `UnknownOpcode(19)` |
| M3 | swap `OP_MATH_GREATER_THAN` ↔ `OP_CRYPTO_BLAKE3` | build | E0080 x4 (pinning + namespace lattice), rc=101 |
| M3c | M3 + pinning asserts made consistent | build | E0080 x2 (`MATH_NS_FIRST == OP_MATH_GREATER_THAN`, `CRYPTO_NS_FIRST == OP_CRYPTO_BLAKE3`) — the wave-7/8 "no buildable consistent form" finding reproduces |
| M4 | `core::error::Error` → `std::error::Error` | test | 86/1: `nostd_hygiene_court_module_body_has_no_std_paths` |
| M5 | insert `.expect(` panic site in `math:sum` | test | 85/2: `panic_hygiene_court…` + `hostile_domain_sweep_court` (real panic) |
| M6 | `checked_add` → `a + b` in `math:sum` | test | 86/1: `hostile_domain_sweep_court` (debug overflow panic at builtins.rs:530); 56-bit KATs stay green as claimed |
| M7 | decode masks dirty reserved bits off | test | 84/3: `hostile_decode_word_court`, `payload_encode_decode_roundtrip`, `refusal_write_isolation_court` |

8/8 rows flip; zero vacuous courts; every restore sha256 byte-exact;
injected-corruption self-check caught a deliberate corruption (restore
detection is not vacuous). Canonical tree verified byte-unchanged across the
whole audit (8-file sha256 snapshot before == after).

## 3. Ownership check (read-only)

- A13-owned file: `crates/cgap-bn3/src/builtins.rs` (+1399 lines). Content
  in-scope; no evidence of A13 edits outside ownership.
- Disclosed seam edits (present on disk, restated here per contract):
  `crates/cgap-bn3/Cargo.toml:9-16` (blake3 path dep, coordinator review
  requested) and `crates/cgap-bn3/src/sigil.rs:327` (const-assert unblock).
- Root `Cargo.toml` untouched. `Cargo.lock` +80 lines is a mechanical cargo
  artifact of the fleet's builds, not a lane manifest edit.

## 4. DoD scorecard (lane doc Section 6)

| DoD | Status | Evidence (this session) |
|---|---|---|
| 1 builtins.rs exists; workspace build green | ALIVE | exit 0 |
| 2 closed registry, 9 opcodes, dense, gapped | ALIVE | opcode/registry courts + 256-opcode sweep green |
| 3 u8-match dispatch, no dyn/fn/registry | ALIVE | source read + asm court (no fn tables) |
| 4 §5.1 KATs green incl. real digest | ALIVE | 87/87 + b3sum cross-check |
| 5 dispatch court green; §5.2 mutations killed | ALIVE | my M1/M2/M3/M3c flips |
| 6 zero-indirection court | ALIVE | asm re-run (0 blr/0 br xN/0 brk) |
| 7 typed refusals, no panic path | ALIVE | refusal-isolation + hostile-sweep + panic-hygiene courts green |
| 8 `-D warnings` green; no unsafe | ALIVE | exit 0; grep clean |
| 9 real cgap_blake3 calls, no re-impl | ALIVE | asm `bl` targets are the real surface |

## 5. Divergences (doc yields; recorded)

1. **Receipt FILE missing** (the one PARTIAL axis): waves 6/7/8 receipts
   exist only as module-doc comments + /tmp artifacts
   (`/tmp/a13w8/M-M*.log`); nothing under `docs/jira/v26.11.1/waves/`.
   The module doc itself has flagged this as a coordinator open item since
   wave 6; unchanged this audit.
2. **Wave-8 receipts certify `c615bcf5…`; current canonical is `5060b702…`.**
   The delta is comment-only (the wave-8 receipt block itself,
   builtins.rs:110-143), so the certification transfers; my runs re-prove
   the current bytes regardless.
3. **Lane doc §5.1 digest literal is wrong in the doc**: `ba7816bf…` is
   SHA-256("abc"), not BLAKE3("abc") — my b3sum run confirms. The lane
   caught this on first run and recorded the divergence in-module
   (builtins.rs:869-873); the lane doc §5.1 text still carries the bad
   literal and should be corrected by the coordinator.
4. **Lane doc is stale**: Status "PROPOSED / `crates/cgap-bn3` does not
   exist yet" — the crate exists, 87/87 green, waves 2-8 landed.
5. **asm line count variance**: lane claimed 238 dispatch-region lines; my
   count of the same region is 236 (counting-boundary variance; all
   flip-relevant properties identical: 0/0/0/1 adrp/3 bl).

## 6. Open items (coordinator)

1. Land the missing A13 wave-receipt FILE (the /tmp/a13w8 logs + module-doc
   blocks are the raw material; my §2 matrix re-proves every row this
   session).
2. Correct the lane doc §5.1 digest literal (SHA-256 vs BLAKE3) and the
   stale "does not exist yet" status.
3. `Cargo.lock` is working-tree-modified fleet-wide; fold into the
   coordinator's integration commit review.

## Commands run this session (identity + exit)

- `cargo test -p cgap-bn3 --lib` → exit 0 (87/87) — canonical
- `RUSTFLAGS="-D warnings" cargo check -p cgap-bn3 --lib` → exit 0
- `cargo rustc --release -p cgap-bn3 --lib -- --emit=asm` → exit 0
- `b3sum` (abc) → `6437b3ac…d85`, matches KAT literal
- sandbox: `cargo test -p cgap-bn3 --lib` BEFORE → exit 0 (87/87); M1-M7 +
  M3c → exit 101 each, flips per §2; restores byte-exact
- canonical sha256 snapshot before/after → identical (8 files)
