# A17 Wave 2 Audit Re-Audit

- **Lane**: adversarial re-audit of `a17-wave2-audit-receipt.md` (restart-wave
  audit lane), 2026-11-01
- **Subject**: `crates/cgap-p28` on `main` @ c83619d + working tree (uncommitted
  lane work), repo `/Users/sac/cgap`, one canonical checkout
- **Verdict**: **ALIVE (receipt CONFIRMED on the current tree)** — every
  re-runnable claim in the audited receipt held this session; every attacked
  court flips under its named mutation; zero vacuous courts; canonical checkout
  byte-exact through the whole battery
  (`attestation.rs` `e750ed43…`, `trap.rs` `835d9d50…` pre/post sha256).
- **Standing detail**: the audited receipt's counts are STALE (it witnessed
  17/17 tests, +1126 lines; the tree has since grown to 31/31 tests, +3849
  lines — A18 wave 3 and the Wave-3 ELF/WASM carrier courts landed after it
  was written). Every claim that could be re-run still holds on the new tree;
  the stale-count divergence is recorded below, not treated as a defect.

## Re-ran evidence (this session, exact commands)

- `cargo test -p cgap-p28` → **exit 0, 31 passed / 0 failed** (15
  `attestation::` + 4 `trap::` + 12 A18 `asc450::` tests). The audited receipt
  cites 17/17 — D-RA1 below.
- `shasum -a 256 LICENSE-P28-SIL.md` →
  `79107c7ade61eb747098f93f1a5641ba665460b76daa1f235bd7894ea4e2484f` — matches
  the audited receipt and the in-code pin at `attestation.rs:1362-1370`
  (`PINNED_ORIGIN_AUTHORITY_DIGEST`, receipt cited line 734 — line drift only;
  the instrument SHA-256 and the digest pin test
  `origin_digest_determinism_and_pin` are green over the real on-disk file).
- `git diff --stat -- crates/cgap-p28/` → 4 files: `Cargo.toml` (+12),
  `attestation.rs` (+1372), `trap.rs` (+750), `asc450.rs` (+1718, seam-mate
  A18 — disclosed in both crates' manifests). Off-limits files clean:
  `git diff --stat -- Cargo.toml crates/cgap-p28/src/lib.rs` → 0 lines.
- Cross-crate authority copy (`cgap-core/src/frame.rs`): re-read pin-for-pin —
  offsets 0/4/6/8/16/48, size 80, align 8, magic `P28S`, flag bits
  0x01/0x02/0x04, lineage mask 0x07, reserved mask 0xFFF8, ceiling 5e9 all
  match the A17 §2.1 table.
- `ERR_P28_SPOLIATION = 0x5032_38FF` chain: owned `trap.rs:27`; exported
  `include/cgap.h` (`CGAP_ERR_P28_SPOLIATION`); acknowledged A19
  `crates/cgap-ffi/src/sovereign.rs` (~line 80; receipt cited 71 — file
  drifted); staged at halt `trap.rs:53-55` (`STAGED_DISCRIMINATOR` +
  `spoliation_halt`).
- §3 span re-derivation (independent, this session): python3 byte-exact
  extraction vs `instrument_header_span` via a sandbox
  `examples/span_dump.rs` — **both 413 bytes, `cmp` byte-identical**
  (`/tmp/span_rust.bin` vs `/tmp/span_py.bin`).
- wasm32 lowering (audited receipt's divergence 8 was UNVERIFIED): this
  session `cargo check -p cgap-p28 --target wasm32-unknown-unknown` → exit 0.
  Check-level witness only (no trap execution under wasm32); the
  `trap.rs:49-51` claim is check-verified, not run-verified.

## Court attack (7 mutations, all in `/tmp/a17-reaudit-sandbox` only)

Method: `cp -r` of `cgap-p28` + `cgap-blake3` + `cgap-crypto` (current
manifest deps) + both LICENSE files into a standalone-workspace sandbox;
baseline 31/31 exit 0; one mutation at a time; pristine restore from canonical
bytes between mutations; `shasum -a 256 -c` restore gate before, after each
mutation's run, and at the end (byte-exact). Script
`/tmp/a17_mutation_battery.sh`; full logs `/tmp/a17-reaudit-mutations.log`,
`/tmp/a17-reaudit-M{1..7}.out`.

| Mut | Edit (subject, not the court) | Outcome on current tree (cargo test exit 101 unless noted) |
|---|---|---|
| M1 | §5.3 version check removed from BOTH `validate_intrinsic` and `validate` | **4 courts FLIP**: `validate_rejects_each_s5_step_violation`, `bit_flip_sweep_court`, `check_then_halt_court` ("spoliated record must halt, got unix_wait_status(0)"), `spoliation_loader_court` (trap.rs:679) |
| M2 | C-ABI `Err(()) => spoliation_halt()` → `Err(()) => 0` | **exactly `check_then_halt_court` FLIPS** ("spoliated record must halt, got unix_wait_status(0)"); all 30 other tests stay green — flip attributable |
| M3 | ceiling `>` → `>=` in `validate_intrinsic` (§5.8 boundary) | **flips maximally loud**: in-process positive control `cgap_p28_invariant_check` aborts the harness — `signal: 6, SIGABRT`, exit **101**; asc450 A18 courts also fail (collateral, real) |
| M4 | `transpile` emits `FLAG_DERIVATIVE` instead of `FLAG_TRANSPILED` | **exactly `transpile_transform_pure_equality` FLIPS** (attestation.rs:1066, `left: 2, right: 4`) |
| M5 | sweep truncated `0..ATTESTATION_LEN` → `0..10` | **`bit_flip_sweep_court` FLIPS** at the anti-vacuity count assert (attestation.rs:1039: "sweep must run all 640 single-bit mutations (anti-vacuity)") |
| M6 | `loader_die` halt → `std::process::exit(0)` (the §7.1 corollary against the Wave-3 full-form loader court — a mutation the audited receipt never ran; its court postdates it) | **`spoliation_loader_court` FLIPS** (trap.rs:417, `assert_death` "must halt, child exited cleanly") |
| M7 | `validate` always-`Ok` (validator-admits-everything class; positive controls must catch) | **6 courts FLIP**: `validate_rejects_each_s5_step_violation`, `bit_flip_sweep_court`, `stripping_court_record_level`, `validate_lineage_context_is_fail_closed`, `transpile_transform_pure_equality`, `spoliation_loader_court` |

Zero vacuous courts: all receipt-named mutations (M1-M5) still flip on the
current tree, and the two courts the receipt never attacked (the Wave-3
full-form `spoliation_loader_court`, plus the always-Ok positive-control
class M7) also flip. A first M5 attempt with an unbalanced pattern failed the
pattern-count assert loudly (recorded as an apply failure, not a flip).

## Divergences (doc/receipt yields; recorded, not fixed)

1. **D-RA1 (stale counts, as-of-date receipt)**: audited receipt witnessed
   17/17 tests and +1126 lines; current tree is 31/31 and +3849 lines. The
   tree moved after the receipt (A18 wave 3 + A17 Wave-3 ELF/WASM carrier
   courts landed). No re-runnable claim was invalidated by the move.
2. **D-RA2 (line-cite drift)**: pin comment `attestation.rs:734` → now
   :1362-1370; `sovereign.rs:71` → ~:80. Artifacts verified present; only the
   line numbers moved.
3. **D-RA3 (test-census delta)**: A17 tests 14 → 19 (15 attestation + 4 trap);
   A18 3 → 12. Receipt's 14+3 = 17 decomposition no longer matches.
4. **D-RA4 (false cleanup claim)**: audited receipt says the prior sandbox was
   "deleted after receipt"; `/tmp/a17-sandbox` (232K, source-only, no target
   dir) still exists on disk. Cleanup-lane item, not a lane defect. My own
   sandbox `/tmp/a17-reaudit-sandbox` I leave for replay; the coordinator may
   delete both.
5. **D-RA5 (scope note)**: the audited receipt's descope (divergence 3) listed
   §4/§7.1/§7.2 full forms as later waves; the current tree now HAS them
   (`elf_carrier_round_trip_court`, `wasm_carrier_round_trip_court`,
   `spoliation_loader_court` — 640×2×2 subprocess-per-mutation sweep over real
   cc/wat2wasm artifacts, green this session). The descope divergence is
   overtaken by events; DoD 5/6/7/8 now witness at full form on the current
   tree.
6. **D-RA6 (wasm32, upgraded)**: receipt divergence 8 (UNVERIFIED) is now
   check-level witnessed this session (`cargo check --target
   wasm32-unknown-unknown` exit 0); run-level wasm32 trap execution remains
   unwitnessed.

## Ownership check

`git diff --stat -- crates/cgap-p28/` → 4 files only (Cargo.toml disclosed
seam deviation in-file; `asc450.rs` is seam-mate A18's, disclosed). Root
`Cargo.toml` and `crates/cgap-p28/src/lib.rs` show zero diff. No git write
commands; no worktrees; no writes into the canonical tree — canonical sha256
byte-exact before/after the battery. Sandbox-only mutation arena
`/tmp/a17-reaudit-sandbox` retained for replay (leasable scratch, not a repo
copy of record).

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && cargo test -p cgap-p28        # exit 0, 31 passed
shasum -a 256 LICENSE-P28-SIL.md                     # 79107c7a…84f
git diff --stat -- crates/cgap-p28/                  # 4 files
cargo check -p cgap-p28 --target wasm32-unknown-unknown   # exit 0
bash /tmp/a17_mutation_battery.sh                    # M1..M7, logs in /tmp/a17-reaudit-*
```

## See Also

- `docs/jira/v26.11.1/waves/a17-wave2-audit-receipt.md` (the receipt audited)
- `docs/jira/v26.11.1/lanes/A17-p28-derivative.md` (normative lane doc)
- `crates/cgap-p28/src/attestation.rs`, `crates/cgap-p28/src/trap.rs`
