# Feature Matrix

Crate x feature support matrix with the build-proof command per cell. Every cell marked
YES was proven by actually running the command in this file; every cell marked `—` is
unproven and carries the command that WOULD prove it.

Wave-10 lane 13, 2026-10-30, host `aarch64-apple-darwin`, rustc 1.97.0, repo `/Users/sac/avec`.

## Scope and method

Test counts are cited, not re-run: `docs/jira/v26.10.31/63-test-census.md` (wave-9 lane 10,
661 passed / 0 failed / 2 ignored across 58 `test result:` lines, HEAD `ead4032`). The one
exception: wasm-footprint `cargo test` was re-run fresh this session (8 passed / 0 failed).
Wasm cells reuse the wave-6/8 validated matrix (`crates/wasm-footprint/NOTES-v26.10.31.md`,
lane I16) where it applies and re-ran all six of the V13 builds fresh
(`69-validation-receipt.md` lane V13: 6/6 builds exit 0).

Host checks ran today, warm, exit codes recorded verbatim. One transient: the first
`cargo check -p avec-cli` of the window exited 101 on parse errors in
`crates/cli/src/bin/status.rs:493/650/660` (another lane's in-flight edit); the owning lane
landed within the window and the re-check exited 0. Recorded as final state.

Two standalone manifests are not root-workspace members; their invocations differ:

    cargo check --manifest-path crates/integration/Cargo.toml -p avec-integration
    cd crates/wasm-footprint          # its own workspace root; crate-local target/

`wasm-footprint` bare `cargo check` (dev profile) exits 101 BY DESIGN of its manifest: the
lib is `#![no_std]` cdylib with no `[profile.dev]` panic override, so the host dev-profile
check hits `unwinding panics are not supported without std`. The release profile pins
`panic = "abort"` and checks clean. Host `cargo test` also passes (the test harness links
std). Use `--release` for its host check cell.

## Crate x feature matrix

| Crate | Host build | Tests (63 census) | no_std core (wasm32-unknown-unknown) | wasm32-wasip1 | NEON path (aarch64 cfg) | Scalar fallback | std feature |
|---|---|---|---|---|---|---|---|
| avec-audit | YES `cargo check -p avec-audit` 0 | 5 passed | — unproven | — unproven | none | n/a | none declared (host std) |
| avec-arrow | YES `cargo check -p avec-arrow` 0 | 137 passed | YES `cd crates/wasm-footprint && cargo build -p avec-arrow --target wasm32-unknown-unknown` 0 | YES `... --target wasm32-wasip1` 0 | YES: silicon.rs + blake3.rs `neon_parent` (aarch64 cfg) | YES: silicon.rs:75 `mask_words_neon` -> SWAR bitmap; blake3.rs portable loop bodies | optional, default off (no_std default) |
| avec-beam | YES `cargo check -p avec-beam` 0 | 10 passed (prebuild law: `cargo build -p avec-beam` first) | N/A host-only (BEAM NIF cdylib) | N/A host-only | none | n/a | none declared (host std) |
| avec-blake3 | YES `cargo check -p avec-blake3` 0 | 25 passed, 1 ignored (by design) | YES `cd crates/avec-blake3 && cargo build --release --target wasm32-unknown-unknown` 0 | YES `... --target wasm32-wasip1` 0 (fresh this session) | YES: lib.rs + multi_chunk.rs 4-way NEON sites | YES: multi_chunk.rs:29/:259 portable chunk loop | optional, default off (no_std default) |
| avec-ffi | YES `cargo check -p avec-ffi` 0 | 13 passed | — unproven | — unproven | none | n/a | none declared (host std) |
| avec-git | YES `cargo check -p avec-git` 0 | 48 passed, 1 ignored (perf_baseline all-ignored) | — unproven | — unproven | none | n/a | `default = ["std"]` |
| avec-gpu | YES `cargo check -p avec-gpu` 0 (macOS host; build.rs is macOS-only) | 1 passed | — non-macOS unproven | — unproven | none (GPU path, not CPU NEON) | n/a | none declared (host std) |
| avec-integration | YES `cargo check --manifest-path crates/integration/Cargo.toml -p avec-integration` 0 | 12 passed | — unproven | — unproven | none | n/a | none declared (dev-test crate) |
| avec-parquet | YES `cargo check -p avec-parquet` 0 | 192 default / 204 std (run tests with `--features std`) | YES via wasm-footprint cdylib dep (transitive proof, exit 0 both targets) | YES (same transitive proof) | YES additive: stream_compact.rs:47 `neon_table_compact_8` | YES: `compact_lanes_branchless` portable primary body | optional, default off; `scan` module is `#[cfg(feature = "std")]` |
| bench | YES `cargo check -p bench` 0 | 0 lib (no lib target; bins + benches only) | N/A host tool | N/A host tool | driver only: `src/bin/neon_ab_bench.rs` (no own NEON) | the portable kernels it drives | n/a (host tool; pulls `avec-parquet features = ["std"]`) |
| avec-cli | YES `cargo check -p avec-cli` 0 | 6 passed | N/A host tool | N/A host tool | none | n/a | none declared (host std CLI) |
| wasm-footprint | YES `cd crates/wasm-footprint && cargo check --release` 0 (bare dev check 101, see method) | 8 passed (re-run fresh today: 8/0) | YES `cargo build --release --target wasm32-unknown-unknown` 0 | YES `... --target wasm32-wasip1` 0 | none | glue only: panic handler + global allocator | no_std by design (supplies the link glue) |

## Column proofs, as run (2026-10-30)

Host checks (all exit 0, warm):

    cargo check -p avec-audit        # 0.32s
    cargo check -p avec-arrow        # 0.01s
    cargo check -p avec-beam         # 0.44s
    cargo check -p avec-blake3       # 0.01s
    cargo check -p avec-ffi          # 0.09s
    cargo check -p avec-git          # 0.01s
    cargo check -p avec-gpu          # 0.09s
    cargo check -p avec-parquet      # 0.01s
    cargo check -p bench             # 0.98s
    cargo check -p avec-cli          # 0.35s (after owning lane landed status.rs)
    cargo check --manifest-path crates/integration/Cargo.toml -p avec-integration  # 0.08s
    cd crates/wasm-footprint && cargo check --release                              # 0.22s

Wasm (all exit 0, fresh this session; superset of the V13 6/6 set — blake3 wasip1 is new):

    # from crates/wasm-footprint (standalone manifest, crate-local target/)
    cd crates/wasm-footprint
    cargo build -p avec-arrow --target wasm32-unknown-unknown   # exit 0, 0.55s
    cargo build -p avec-arrow --target wasm32-wasip1            # exit 0, 0.57s
    cargo build --release -p wasm-footprint --target wasm32-unknown-unknown  # exit 0, 1.06s
    cargo build --release -p wasm-footprint --target wasm32-wasip1           # exit 0, 1.07s
    # from crates/avec-blake3
    cd crates/avec-blake3
    cargo build --release --target wasm32-unknown-unknown       # exit 0, 0.64s
    cargo build --release --target wasm32-wasip1                # exit 0, 0.64s (new receipt)

Tests re-run fresh this session (one crate only; all others cited from 63-test-census.md):

    cd crates/wasm-footprint && cargo test   # exit 0: 3 passed + 5 passed = 8/0

## Refuted and absent entries

- **NEON transpose REFUTED** (V2, `69-validation-receipt.md`): docs 23/27 specify ZIP/UZP
  transpose on NEON; never implemented. `crates/avec-arrow/src/transpose.rs` carries no
  `cfg(target_arch = "aarch64")`. Do not read the NEON column's avec-arrow cell as covering
  transpose — it does not.
- **avec-gpu is macOS-only by construction**: `build.rs` gates on `cfg(target_os = "macos")`,
  compiles `metal_shim.m`, links Metal + Foundation frameworks. Non-macOS host build is an
  unproven cell, command: `cargo check -p avec-gpu --target x86_64-unknown-linux-gnu`
  (target not installed on this host).
- **avec-beam is host-only by design** (BEAM NIF cdylib): no wasm/no_std claim exists to
  prove.
- **Bare `cargo test -p avec-parquet` collects 0 refusal tests** (V9 caveat): the
  std-gated suites compile away under the default config; run `--features std` (204 passed).

## Open cells (unproven, with the command that WOULD prove them)

| Crate | Cell | Proving command |
|---|---|---|
| avec-audit | wasm x2 | `cargo check -p avec-audit --target wasm32-unknown-unknown` and `... --target wasm32-wasip1` |
| avec-ffi | wasm x2 | `cargo check -p avec-ffi --target wasm32-unknown-unknown` and `... --target wasm32-wasip1` |
| avec-git | wasm x2 | `cargo check -p avec-git --target wasm32-unknown-unknown` and `... --target wasm32-wasip1` |
| avec-integration | wasm x2 | `cargo check --manifest-path crates/integration/Cargo.toml -p avec-integration --target wasm32-unknown-unknown` (and wasip1) |
| avec-gpu | non-macOS host | `cargo check -p avec-gpu --target x86_64-unknown-linux-gnu` (target not installed here) |

`—` cells are UNKNOWN until one of these commands runs with a recorded exit code.

## Standing

Host build: 12/12 crates exit 0 today. Wasm: 6 fresh builds exit 0 (arrow x2 targets,
blake3 x2 targets, wasm-footprint cdylib x2 targets), extending V13's 6/6 with the first
avec-blake3 wasm32-wasip1 receipt. Tests: 661/0/2 standing per 63-test-census.md; the only
fresh test run this session re-witnessed wasm-footprint at 8/0.

ALIVE cells are exactly those with a command + exit code in this file. `—` cells are
UNKNOWN; the open-cells table is their proving path.

See Also: `docs/jira/v26.10.31/63-test-census.md` (test counts),
`crates/wasm-footprint/NOTES-v26.10.31.md` (wasm wave-6/8 matrix),
`docs/jira/v26.10.31/69-validation-receipt.md` (V2 NEON-transpose refutation, V13 wasm),
`docs/jira/v26.10.31/65-coordinator-handoff.md`.
