# A17 Wave 2 Audit Receipt

- **Lane**: A17 (crates/cgap-p28), Wave 2 core engines, v26.11.1
- **Auditor**: adversarial audit lane (re-run of failed-lane restart), 2026-11-01
- **Subject**: `crates/cgap-p28` on `main` @ c83619d (working tree, uncommitted
  lane work on top); repo `/Users/sac/cgap`, one canonical checkout
- **Verdict**: **ALIVE** — Wave 2 scope fully landed, every attacked court
  flips, zero production defects found. Repo left byte-exact
  (sha256 `a33cf1ff…`, `dc04cbaa…` pre/post), mutations applied only in
  `/tmp/a17-sandbox` (deleted after receipt).

## Verified surfaces (Wave 2 scope, all ALIVE this session)

| Surface | Spec | Evidence (command + exit, this session) |
|---|---|---|
| Wire codec | §2.1 | `cargo test -p cgap-p28` exit 0, 17/17 incl. `codec_exact_le_bytes_and_round_trip` |
| origin_authority_digest | §3 | `origin_digest_determinism_and_pin` green over the real instrument; instrument SHA-256 re-verified on disk |
| Transpile | §4.2 | `transpile_transform_pure_equality` green; M4 flips it |
| Ordered validation | §5 | `validate_rejects_each_s5_step_violation` green; M1/M3 flip validators |
| Halt + C-ABI | §6.1/6.2 | `halt_court_subprocess_sigabrt` + `check_then_halt_court` green; M2 flips the C-ABI court |
| Core courts | §7 | 5 anti-vacuity mutations applied in /tmp sandbox; 5/5 courts flip; 0 vacuous |

## Baseline (re-run this session)

- `cargo test -p cgap-p28` → exit 0, **17 passed / 0 failed** (14 A17 +
  3 seam-mate A18 `asc450` tests). Same result re-witnessed in the /tmp
  sandbox before mutating (17/17) and after full restore (17/17).
- Instrument pin: `shasum -a 256 LICENSE-P28-SIL.md` =
  `79107c7ade61eb747098f93f1a5641ba665460b76daa1f235bd7894ea4e2484f`
  — matches the pin comment at `attestation.rs:734`.
- Independent span re-derivation: python byte-exact extraction vs
  `instrument_header_span` (sandbox `examples/span_dump.rs`) — **413 bytes,
  byte-identical** (`cmp` clean). Span = file start through the first
  `---\n` line inclusive.
- Cross-crate authority claim (`attestation.rs:23-26`): cgap-core
  `frame.rs` embedded copy verified pin-for-pin this session (offsets
  0/4/6/8/16/48, size 80, align 8, magic `P28S`).
- §6.1 export chain: `ERR_P28_SPOLIATION = 0x5032_38FF` owned at
  `trap.rs:27`; exported in `include/cgap.h:63`
  (`CGAP_ERR_P28_SPOLIATION`); acknowledged by A19 at
  `crates/cgap-ffi/src/sovereign.rs:71`; staged at halt `trap.rs:53`.

## Anti-vacuity court attack (all mutations in /tmp/a17-sandbox only)

Method: `cp -r` of cgap-p28 + cgap-blake3 (+ LICENSE.md,
LICENSE-P28-SIL.md) into `/tmp/a17-sandbox` with a minimal standalone
workspace manifest; baseline 17/17 green; one mutation at a time;
pristine restore from repo copies between mutations; repo sha256
byte-exact before/after.

| Mutation | Edit | Court outcome (cargo test -p cgap-p28) |
|---|---|---|
| M1 | §5.3 version check removed from `validate_intrinsic` + `validate` | **3 courts FLIP**: `validate_rejects_each_s5_step_violation`, `bit_flip_sweep_court`, `check_then_halt_court` (spoliated child exits 0: `unix_wait_status(0)`) |
| M2 | C-ABI `Err(()) => spoliation_halt()` → `Err(()) => 0` (halt path removed — the §7.1 corollary mutation) | **`check_then_halt_court` FLIPS** ("spoliated record must halt, got unix_wait_status(0)"); all other courts stay green — flip attributable |
| M3 | ceiling check `>` → `>=` (§5.8 boundary; validator-rejects-everything attack on positive controls) | **FLIPS maximally loud**: in-process positive control `check_returns_zero_iff_intrinsic_valid` aborts the harness (SIGABRT); `cargo test` exit **101** |
| M4 | `transpile` emits `FLAG_DERIVATIVE` instead of `FLAG_TRANSPILED` | **`transpile_transform_pure_equality` FLIPS** (`left: 2, right: 4`) |
| M5 | sweep loop truncated to 10 of 80 bytes (the sweep court's own named mutation-skipping shim) | **`bit_flip_sweep_court` FLIPS** with the named message "sweep must run all 640 single-bit mutations (anti-vacuity)" |

A first M5 attempt (unbalanced braces) produced a **compile-abort** —
recorded as such, not a flip witness; redone brace-balanced.

No vacuous court: every attacked court flipped under its named mutation
class, and M2's flip was isolated to its own court.

## Divergences (doc yields; recorded, not fixed)

1. **D-A17-1 (lane re-derivation, arithmetically correct)**: doc §7.1
   claims pinning the ceiling at MAX makes all 640 bits load-bearing —
   false for CLEAR-flips of the 11 set bits of 0x12A05F200 (clearing
   stays <= MAX). The lane's re-derived law (survivors == exactly the
   set-bit class, hand-count pinned at 11) is correct; doc yields.
2. **D-A17-2 (stricter than doc)**: `validate` fails closed when a
   Derivative/Transpiled record has no parent, or a Root record is
   presented with one. Stricter than doc §5 step 7's literal "only
   when"; fail-closed direction; doc yields.
3. **Descope, disclosed** (`attestation.rs:13-16`): §4.1 ELF emission +
   `find_in_elf`, §4.2 WASM custom-section round-trip, §7.1's
   subprocess-per-mutation sweep over real ELF/WASM artifacts, and
   §7.2's real section-strip are later waves. DoD 5/6 and the full
   forms of DoD 7/8 are therefore PARTIAL at whole-DoD scope; the Wave
   2 dispatch scope (core engines + core courts) is fully met.
4. **Manifest edit (seam disclosure)**: `crates/cgap-p28/Cargo.toml` is
   coordinator-owned per the lane doc, but the lane added the
   `cgap-blake3` path dependency (in-file disclosure present). Shared
   `Cargo.lock` churn also touches other lanes' dep entries (mechanical).
5. **§3 span prose**: doc says "first 5 lines"; implemented span is
   first line through the first `---\n` inclusive (413 bytes, verified
   byte-identical to an independent extraction). Deterministic and
   pinned; doc yields.
6. **DoD 4 "SHA-256 in the lane receipt"**: recorded in-code
   (`attestation.rs:732-736`, verified on disk) and in this receipt; no
   standalone A17 lane receipt file exists on disk.
7. **Wave 1 freeze preservation**: HEAD@c83619d scaffold files are
   empty, so "preserved unchanged" is not diffable against a commit;
   the Wave 1 block is present, const-pinned, and test-witnessed.
   Cross-lane freeze sign-off (DoD 11) UNVERIFIED this session.
8. **wasm32 lowering** (`trap.rs:50-51` claims abort → WASM
   `unreachable`): UNVERIFIED — no wasm32 target run (WASM carriers
   descoped, see 3).

## Ownership check

`git diff --stat -- crates/cgap-p28/` (read-only): 4 files —
`Cargo.toml` (+7, disclosed dev 4), `attestation.rs` (+744, A17),
`trap.rs` (+263, A17), `asc450.rs` (+219, seam-mate A18). No writes
outside lane scope; root `Cargo.toml`, `lib.rs` untouched; no git write
commands run; no worktrees/clones (the /tmp court sandbox is the
dispatch-sanctioned mutation arena, deleted after receipt).

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && cargo test -p cgap-p28        # exit 0, 17 passed
shasum -a 256 LICENSE-P28-SIL.md                     # 79107c7a…84f
git diff --stat -- crates/cgap-p28/                  # 4 files, ownership
# sandbox: /tmp/a17-sandbox (cp -r cgap-p28 + cgap-blake3, own workspace
# manifest; baseline 17/17; M1..M5 one at a time; sha256 restore check)
# M3: cd /tmp/a17-sandbox && cargo test -p cgap-p28   # exit 101 (SIGABRT harness)
```

## See Also

- `docs/jira/v26.11.1/lanes/A17-p28-derivative.md` (normative lane doc)
- `docs/jira/v26.11.1/03-rename-receipt.md` (D2 instrument freeze)
- `crates/cgap-p28/src/attestation.rs`, `crates/cgap-p28/src/trap.rs`
