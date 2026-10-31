# wasm-footprint v26.10.31 — wasm portability notes (lane I16)

# Status

- `avec-arrow` and `avec-blake3` both compile clean for `wasm32-unknown-unknown`
  and `wasm32-wasip1` — receipts below. No source changes were needed for
  wasm compatibility in this lane.
- `wasm-footprint` cdylib builds for both targets, release, 4,309 bytes each
  (pre-existing gate, re-witnessed this session).
- Host gates re-witnessed green: `cargo test` (w4_courts, 3 passed) and
  `cargo test --test no_std_core` (3 passed, the lane deliverable).
- **Cross-lane unblock (disclosed, compile-freeze SLA rule 2/6):** the scan
  lane added `pub mod scan;` (module-level `extern crate std;`, `std::fs`)
  to avec-parquet ungated, which broke the no_std wasm gate
  (E0152 duplicate `panic_impl` on host; std::fs uncompilable on wasm32).
  Minimal fix applied by I16 in `crates/avec-parquet/src/lib.rs`:
  `#[cfg(feature = "std")] pub mod scan;` + `#[cfg(feature = "std")]
  extern crate std;` (the `std` feature previously existed but was
  decorative). Scan lane's own tests verified green after the fix:
  `cargo test --features std` in crates/avec-parquet = 188 passed across
  5 suites. Scan lane must now run its tests with `--features std`.
- Remaining other-lane warnings, recorded not fixed: duplicate
  `#[inline(always)]` on `ct_lt_u64_mask`
  (`crates/avec-arrow/src/predicates.rs:59-60`), unused import
  `alloc::vec` (`crates/avec-parquet/src/level_free.rs:22`).

# Building the core crates for wasm32

## Prerequisite

    rustup target add wasm32-unknown-unknown wasm32-wasip1

This machine already has both installed (`rustup target list --installed`:
wasm32-unknown-unknown, wasm32-wasip1, wasm32-wasip2, aarch64-apple-darwin).
No toolchain installation was needed; none was performed.

## Commands + exits (all run 2026-10-30, rustc 1.97.0)

    # from crates/wasm-footprint (standalone workspace manifest)
    cd crates/wasm-footprint
    cargo build -p avec-arrow --target wasm32-unknown-unknown   # exit 0
    cargo build -p avec-arrow --target wasm32-wasip1            # exit 0
    cargo build --release -p avec-arrow --target wasm32-unknown-unknown  # exit 0

    # avec-blake3 is not a wasm-footprint dependency; standalone build
    cd crates/avec-blake3
    cargo build --release --target wasm32-unknown-unknown       # exit 0

    # footprint cdylib, both targets (pre-existing gate)
    cargo build --release --target wasm32-unknown-unknown       # exit 0
    cargo build --release --target wasm32-wasip1                # exit 0

## Artifact sizes (release)

    target/wasm32-unknown-unknown/release/libavec_arrow.rlib    797,576 B
    target/wasm32-unknown-unknown/release/libavec_blake3.rlib   135,100 B
    target/wasm32-unknown-unknown/release/wasm_footprint.wasm     4,309 B
    target/wasm32-wasip1/release/wasm_footprint.wasm              4,309 B

rlib size is not footprint. The footprint CTQ is decided by the
wasm-footprint cdylib + `wasm-opt -Oz` pipeline (`scripts/footprint.sh`,
scripts lane I14); these sizes are build receipts, not CTQ claims.

## Expected blockers for a fresh clone

1. Missing rustup targets (fix: the `rustup target add` above).
2. The crate root Cargo.toml is a **standalone manifest, deliberately not a
   root-workspace member** — every command must run from
   `crates/wasm-footprint`, not the repo root; `-p wasm-footprint` from the
   repo root fails with "package ID specification did not match".
3. `wasm_footprint.wasm` links only with `panic = "abort"` (set in this
   manifest's `[profile.release]`). A dev-profile cdylib build of this crate
   for wasm fails with "unwinding panics are not supported without std" —
   same message visible in the host-target note below. Build with
   `--release`, or add per-profile panic settings if a debug wasm build is
   ever needed.
4. `wasm-opt` is not needed to build, only for the footprint number.

## Host-target gate status (precise, not rounded up)

- `cargo check --release` (host): ALIVE, exit 0.
- `cargo test` (host, includes the new `tests/no_std_core.rs`): ALIVE —
  w4_courts 3 passed + no_std_core 3 passed.
- `cargo check` (host, **dev profile**): FAILS, pre-existing — the no_std
  cdylib with the test-cfg-glue design carries `panic = "abort"` only in
  `[profile.release]`, so a dev-profile host compile of the lib hits
  "unwinding panics are not supported without std". This predates lane I16
  (wasm-footprint sources are unmodified this session; the same failure
  occurs with `cargo check` on the untouched tree). The green host gates for
  this crate are therefore `cargo check --release` and `cargo test`.

# no_std core test (lane deliverable)

`tests/no_std_core.rs` — portability court over the no_std core surface, two
registers of assertion:

- Const (compiler-decided): `Quad64` size/align 64/64, sentinel constants
  (`BOTTOM_TERM_ID == 0`, `WILDCARD_TERM_ID == u32::MAX`).
- Runtime `#[test]`s: the `avec_arrow::predicates` masks behave
  (ct_eq_u32 / ct_lt_u32 / ct_select_u32 / ct_select_u64 / ct_eq_u64_mask),
  and the `layout`/`transpose` surfaces are nameable from a consumer crate.
  The predicates are not `const fn` (upstream's choice, not this test's), so
  those facts are asserted at runtime, not in consts.

Run:

    cd crates/wasm-footprint && cargo test --test no_std_core   # 3 passed

# Wave-4 re-verification (lane 17, 2026-10-30 ~19:00, post concurrent-lane landings)

Re-run AFTER the wave-4 concurrent lanes landed into avec-arrow (transpose
rework, predicates rework) and avec-blake3 (`multi_chunk`). All commands
re-run this session; exits recorded inline.

## Wasm builds (post-landing), all exit 0

    cd crates/wasm-footprint
    cargo build [--release] -p avec-arrow --target {wasm32-unknown-unknown, wasm32-wasip1}
      -> 8/8 exit 0 (dev+release, both targets)

    cd crates/avec-blake3   # resolves inside the ROOT workspace; artifacts in repo-root target/
    cargo build [--release] --target {wasm32-unknown-unknown, wasm32-wasip1}
      -> 8/8 exit 0 (dev+release, both targets)

    # footprint cdylib, release, both targets: exit 0 / exit 0

## Sizes (release rlibs / cdylib, re-read from disk 19:00)

    target/wasm32-unknown-unknown/release/libavec_arrow.rlib    836,048 B  (was 797,576 B pre-rework; this is the wasm-footprint crate-local target, post predicates/transpose rework)
    target/wasm32-unknown-unknown/release/libavec_blake3.rlib   180,520 B  (was 135,100 B pre-multi_chunk; root workspace target)
    target/wasm32-wasip1/release/libavec_blake3.rlib            180,508 B
    target/wasm32-unknown-unknown/release/wasm_footprint.wasm     4,309 B  (unchanged)
    target/wasm32-wasip1/release/wasm_footprint.wasm              4,309 B  (unchanged)

The 4,309 B cdylib is RE-WITNESSED unchanged: the concurrent lanes' additions
(transpose/predicates rework, multi_chunk) do not touch the linked cdylib
footprint subject.

## no_std gate: still green

The scan std-gating fix (`#[cfg(feature = "std")] pub mod scan;` +
`extern crate std` in avec-parquet) HOLDS: the wasm-footprint cdylib (which
depends on avec-parquet) builds clean for both wasm targets at 4,309 B, and
`cargo test` in avec-parquet is unaffected. No regression this session.

## New-surface court (additive, tests/no_std_core.rs)

- Added `wave4_new_surfaces_nameable_no_std` — asserts the post-landing
  public surfaces stay nameable from a `#![no_std]` consumer:
  `avec_arrow::predicates::{ct_quad_eq_u32, ct_select_usize}` (quad-WIDE
  mask semantics: all-ones iff all 4 lanes match — first draft asserted
  per-lane packing and was corrected after a real failure) and
  `avec_blake3::multi_chunk::compress_chunks_4way` (requires a full
  4096-byte input — asserts internally; first draft used 64 B and hit that
  assert, then corrected).
- `hash_parallel`: NOT landed — grep over avec-blake3/src finds no such pub
  symbol this session. Recorded, not asserted.
- Disclosure (compile-freeze SLA): `avec-blake3` added as a
  `[dev-dependencies]` entry of wasm-footprint (it was not a dependency;
  the additive test requires it). Manifest diff is one table + one line;
  production deps unchanged.
- Mid-run the predicates lane broke the shared avec-arrow compile for ~1 min
  (E0425 `zero_mask`, predicates.rs in-flight); I waited (SLA) rather than
  touched their file — clean at first 60 s poll.

## Host gates (re-witnessed 19:00)

    cd crates/wasm-footprint
    cargo check --release -p wasm-footprint   # exit 0
    cargo test                                 # w4_courts 3 passed + no_std_core 4 passed, exit 0

# Wave-6 re-verification (lane 10, 2026-10-30 ~19:32, concurrent lanes 1-2 in multi_chunk)

Re-run AFTER wave-6 lanes 1-2 landed `compress_chunks_4way_x2` (NEON-gated on
aarch64, scalar elsewhere) into `crates/avec-blake3/src/multi_chunk.rs`.
All commands run this session; exits recorded inline. Real-findings-only
policy honored: no fixes to other lanes' crates.

## Wasm matrix (dev+release, both targets), all exit 0

    cd crates/avec-blake3   # root workspace; artifacts in repo-root target/
    cargo build [--release] --target {wasm32-unknown-unknown, wasm32-wasip1}
      -> 4/4 exit 0

    cd crates/wasm-footprint
    cargo build [--release] -p avec-arrow --target {wasm32-unknown-unknown, wasm32-wasip1}
      -> 4/4 exit 0

    # footprint cdylib, release, both targets: exit 0 / exit 0

## Sizes (release, re-read from disk 19:32)

    target/wasm32-unknown-unknown/release/libavec_blake3.rlib   222,764 B  (root workspace target; was 186,430 B mid-session, 180,520 B at wave-4 — lanes 1-2 still landing during this lane, rlib grew twice under us)
    target/wasm32-wasip1/release/libavec_blake3.rlib            222,746 B
    target/wasm32-unknown-unknown/release/libavec_arrow.rlib    836,048 B  (unchanged from wave-4)
    target/wasm32-unknown-unknown/release/wasm_footprint.wasm     4,309 B  (unchanged — RE-WITNESSED third consecutive wave)
    target/wasm32-wasip1/release/wasm_footprint.wasm              4,309 B  (unchanged)

## NEON gating verdict: no wasm effect

`compress_chunks_4way_x2` gates the NEON body behind
`#[cfg(target_arch = "aarch64")]`; the wasm builds take the scalar
`#[cfg(not(target_arch = "aarch64"))]` path. Matrix above is 10/10 exit 0
and the cdylib footprint subject is byte-size-identical at 4,309 B.

## no_std gate: green, extended additively

- Added `wave6_multi_chunk_x2_nameable_no_std` to `tests/no_std_core.rs`
  (additive only): asserts the new x2 surface is nameable from a
  `#![no_std]` consumer AND consistent with two flattened
  `compress_chunks_4way` calls (outs[0]==A, outs[1]==B on zero input).
- First draft forgot the import — E0425 caught by compile, fixed in this
  file only (import line extended).
- `cargo test --test no_std_core`: 5 passed, exit 0.

## Host gates (re-witnessed 19:32)

    cd crates/wasm-footprint
    cargo check --release -p wasm-footprint   # exit 0
    cargo test                                 # w4_courts 3 passed + no_std_core 5 passed, exit 0

# Wave-7/8 note (lane X3, 2026-10-31)

- No wave-7/8 wasm re-verification was performed in this lane; no wasm gate
  receipt exists — open item per docs/jira/v26.10.31/45-wave-6-receipt.md
  (45, L10).
- The sizes recorded in the wave-6 section above remain the last witnesses
  of the wasm footprint subject (cdylib 4,309 B, both targets).

# Related files

- `tests/no_std_core.rs` — the court (new this lane).
- `scripts/footprint.sh`, `scripts/wasm_bench.mjs` — footprint + JS courts
  (other lanes).
- Spec rows: docs/jira/v26.10.31/21-ard.md §3 (WASM row),
  docs/jira/v26.10.31/27-branchless-kernel-abi.md (WASM table).
