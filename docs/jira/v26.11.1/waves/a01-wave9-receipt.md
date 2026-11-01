# A01 Wave 9 Receipt

- **Lane**: A01-arrow-slab (`crates/cgap-arrow/src/slab.rs`,
  `crates/cgap-arrow/src/bitmask.rs`), Wave 9 — cross-lane integration,
  v26.11.1, repo `/Users/sac/cgap` (one canonical checkout, branch `main`,
  HEAD `c83619d`, uncommitted lane work on top)
- **Slice**: "A04 identity court, A14 dam over A04 frames, A20 1M-row
  stress, A05 power-cut sweep, A07 zero-string gate over all built crates"
  (lane dispatch; lane doc §7 waves schedule exhausted — this is the
  failed-lane-restart wave)
- **Verdict**: **ALIVE** — all five wave-9 courts pass in the canonical
  tree this session (`cargo test -p cgap-arrow --lib` → exit 0, 130 passed
  / 0 failed; 125 baseline + 5 wave-9), the anti-vacuity mutation matrix
  flips 5/5 in a /tmp sandbox with byte-exact restores, and the A07
  zero-string gate returns exit 0 in all three modes over all built
  crates. `bitmask.rs` is unchanged this wave (no slice law touches it;
  masks are exercised through `seal_with_mask`).

## Files touched (lease + disclosed seam edit)

| File | Change | sha256 (post) |
|---|---|---|
| `crates/cgap-arrow/src/slab.rs` | Wave-9 module doc paragraph + `mod wave9_tests` (5 courts) | `d79ba63f...984b206aac` |
| `crates/cgap-arrow/src/bitmask.rs` | **unchanged** this wave | `d491797e...f322b8aa` |
| `crates/cgap-arrow/Cargo.toml` | `[dev-dependencies]` cgap-core, cgap-gate, cgap-storage (path) | `1e5ad71a...2665b0a788` |
| `Cargo.lock` | mechanical cargo resolution of the dev-deps (already fleet-modified pre-wave) | coordinator-owned file |

## Courts landed (all in `crates/cgap-arrow/src/slab.rs`, `wave9_tests`)

| Court | Subject | Result |
|---|---|---|
| W9-C1 `w9_c1_a04_identity_court` | Sealed slab rides a real `CgapFrameHeader` (FLAG_ARROW, oabi_digest = in-crate BLAKE3): decode→validate→payload view→verify_payload→from_bytes closes on structural identity; slab-plane byte flip → `DigestMismatch` on BOTH planes; A04 §8.3 header classes (magic→`BadMagic`, ceiling→`AttestationMismatch`, length lie→`PayloadLenMismatch`, digest-field flip→`DigestMismatch`) all typed. | PASS |
| W9-C2 `w9_c2_a14_dam_over_a04_frames` | `penstock::ingress` over the framed slab: ADMIT with out-view == slab window; rows 1-5 (short header, lying length, magic, version, zero digest→OABI_MISMATCH, declared>buffer→OABI_MISMATCH, P28S flip, att version, reserved att flag, lineage 2-bits, ceiling>max, ROOT+nonzero receipt, DERIVATIVE+zero receipt → P28_SPOLIATION). | PASS |
| W9-C3 `w9_c3_a05_power_cut_sweep_framed_slab_plane` | Cut at EVERY k in 0..=4288 (4289 cases, enumerated): k<128 → dam `INVALID_MAGIC` + A04 `BufferTooSmall`; 128<=k<N → dam `OABI_MISMATCH` + `PayloadLenMismatch` + slab `BadLength{got: k-128}`; k=N → full chain commits. `refusals == N` asserted. I1: never a partial artifact. | PASS |
| W9-C3b `w9_c3b_a05_journal_plane_slab_checkpoint_sweep` | Real A05 machinery (FlashMedium + fence + begin_txn/write_record + recover), slab checkpoint whose manifest (first 8 B of the sealed BLAKE3, LE) rides `parquet_root` (opaque u64; test-plane binding disclosed, production meaning is A02's per A05 §6). Cut at every k in 0..=112: k<N → prior root, `recovered_seq == None`, per-band verdicts/reports asserted through A05's own `tear_class_for_offset` oracle; k=N → checkpoint durable, manifest matches sealed digest. | PASS |
| W9-C4 `w9_c4_a20_1m_row_stress` | Exactly 1,000,000 rows (SplitMix64, seed `0xA20C0FFEE5A7E000`, pure fn of row index) paged through 15,625 `Slab64`s; every slab `seal_with_mask` → `from_bytes` structural identity; totals: 1M rows / 15,625 slabs / 500,000 present / 65,000,000 sealed bytes; two independent passes fold to the pinned digest `521e8773...c3859f35` (determinism + pin); deterministic first slab closes the A04→A14→A01 chain. | PASS |

## Commands (this session, canonical tree)

```text
cargo test -p cgap-arrow --lib              -> exit 0, 130 passed / 0 failed, 11.27 s
cargo test -p cgap-arrow --lib wave9        -> exit 0, 5 passed / 0 failed, 11.24 s
RUSTFLAGS="-D warnings" cargo check -p cgap-arrow --lib          -> exit 0
RUSTFLAGS="-D warnings" cargo test -p cgap-arrow --lib --no-run  -> exit 0
cargo tree -p cgap-arrow -e normal          -> cgap-arrow only (runtime E=0 preserved)
cargo tree -p cgap-arrow -e dev             -> cgap-core, cgap-gate, cgap-storage (test-only)
```

## W9-C5 — A07 zero-string gate over all built crates (session runs)

```text
python3 scripts/zero_string_gate.py --self-check -> exit 0
  SELF-CHECK poison artifacts=1 hits=2 fired=True; clean artifacts=1 hits=0;
  RESULT PASS vocab=graph-derivation:ontologies/*/src/lib.rs
python3 scripts/zero_string_gate.py --courts     -> exit 0, COURTS-SUMMARY total=13 failed=0
python3 scripts/zero_string_gate.py              -> exit 0
  SUMMARY scanned=49 crates=22 hits=0 emitter_hits=2 (loud producer-class,
  emit-fibo-pack) allowed=6776 unknown=331294 vocab=9 gate=1.4.0
```

The scan ran AFTER the wave-9 rebuild, so the scanned artifacts include the
rebuilt `cgap-arrow` binaries carrying `wave9_tests`.

## Anti-vacuity mutation matrix (sandbox `/tmp/a01w9-sb`, canonical untouched)

Sandbox: 6-crate closure (cgap-arrow + cgap-core + cgap-gate + cgap-storage +
cgap-blake3 + cgap-git) under a minimal root manifest + the repo's
`rust-toolchain.toml`; sandbox baseline 5/5 exit 0; sandbox `slab.rs` sha256
`d79ba63f...984b206aac` == canonical at start; one mutation at a time via a
count==1 anchored patch, run, sha256-verified restore; canonical tree
byte-identical across the sweep. The mutations attack the courts' own
harness/fixture text (the courts' subjects are neighbor-lane code outside
this lane's lease — disclosed per the same-checkout fan-out rule; the wave-6/8
`craft()` pattern is the same class of fixture-level attack).

| # | Mutation | Court | Result |
|---|---|---|---|
| MW9-1 | digest injection → `\|_\| [0u8; 32]` (digest bypass) | W9-C1 | **FLIP** (exit 101) |
| MW9-2 | row-3a expected code `OABI_MISMATCH` → `CGAP_OK` | W9-C2 | **FLIP** |
| MW9-3 | C4-branch expectation inverted (`CGAP_OK` → `INVALID_MAGIC`) | W9-C3 | **FLIP** (fails at k=N) |
| MW9-3b | torn-offset law `out.root == base` → `== checkpoint` | W9-C3b | **FLIP** (fails at k=0) |
| MW9-4 | pinned corpus digest byte `0x52→0x53` | W9-C4 | **FLIP** (11.21 s real double-pass) |

5/5 flip, every restore sha256-verified byte-exact, MATRIX_EXIT=0.

## Standing of the lane-doc exit gate (C6, riscv32imc)

`cargo build -p cgap-arrow --target riscv32imc-unknown-none-elf --lib` →
**exit 101**, 5 errors, ALL outside the A01 lease: `chase_lev.rs:9`
(`AtomicI64` unresolved — no 64-bit atomics on RV32IMC) and
`c_abi.rs:57,87` / `c_abi_nested.rs:55,88` (`swap` unavailable on
`Atomic<*mut c_void>` under RV32IMC). **`slab.rs` and `bitmask.rs`
themselves compile target-clean** (no error points into either file).
C6 stays BLOCKED on modules this lane does not own; the blocker is now
PRECISE: 64-bit atomic availability, not the lane doc's predicted
`extern crate alloc` gate (risk 1 in the doc predicted the wrong
mechanism — recorded as a doc-yields divergence). No smoke binary /
symbol audit this wave (out of the wave-9 slice; Wave-1/2 exit item).

## Divergences (recorded, not silent)

1. **Cargo.toml dev-dependencies seam edit** — the dispatch lease names only
   `slab.rs`/`bitmask.rs`; the integration courts need the REAL A04/A14/A05
   collaborators, which is impossible without dev-deps. Landed under the
   contract's minimal-unblock seam clause with disclosure (this list), on the
   house pattern: cgap-core's `cgap-blake3` dev-dep ("cross-tier courts",
   E=0 runtime preserved) and cgap-parquet's `cgap-arrow` crate-as-dev-dep.
   `cargo tree -e normal` = `cgap-arrow` alone, so runtime E=0 stands. Note
   for the coordinator: `publish = true` on cgap-arrow + path-only
   dev-dependencies may block `cargo publish` validation — review before the
   next publication (runtime deps unchanged).
2. **Receipt file itself** — `docs/jira/v26.11.1/waves/a01-wave9-receipt.md`
   is outside the two-file lease; written as the fleet's coordination
   channel (13+ sibling lane receipts exist; A13/A20 wave-9 receipts landed
   the same way), disclosed here.
3. **C4 pin provenance** — the pinned folded digest
   (`521e8773...c3859f35`) was first observed this session (placeholder
   zeros, assertion failure printed the observed value), then pinned and
   re-proven green twice; determinism is additionally proven independently
   of the pin by the two-pass equality assert. A transcribed-byte typo in
   the pin was caught by the court itself on the first re-run (byte 21:
   0x56 vs 0x86) — the pin is load-bearing, witnessed by MW9-4.
4. **riscv C6 blocker identified precisely** — the lane doc §8.1 predicted
   `extern crate alloc` as the riscv blocker; the real (measured) blockers
   are `chase_lev.rs` AtomicI64 and `c_abi*.rs` `swap` on RV32IMC. Doc
   yields; slab/bitmask are target-clean.
5. **Test-only allocation in wave-9 courts** — `alloc::vec!`/`Vec`/`concat`
   in the `#[cfg(test)]` module. The no-alloc law governs the LIB path
   (slab/bitmask module bodies: unchanged, no alloc call site — court C2's
   structural claim intact); host tests allocate, same as every prior wave
   (`wave6_tests` already imported `alloc::vec::Vec`).

## Fence compliance

- No `Process.sleep`-style waits anywhere in the new code (cuts are
  deterministic byte truncations; generation is a pure function of the row
  index) — grep clean.
- No git write commands run; no worktrees/clones/copies of the canonical
  checkout; the sandbox is a /tmp copy for mutation attacks only.
- Zero-string gate modes all loud (skips printed, never silent);
  mutation matrix restores verified byte-exact; anchors count==1 enforced
  (no silent multi-hit patches).
