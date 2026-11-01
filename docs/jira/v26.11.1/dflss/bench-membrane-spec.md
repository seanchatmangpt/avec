# bench-membrane-spec

Construction spec for `crates/bench` as the v26.11.1 Phase 2 (Measure) Divan
benchmark membrane. Audience: lane-runner agents, one dispatch per lane. This
file is the construction contract; every performance number the membrane
exists to graduate is carried at TARGET standing until a `cargo bench`
receipt witnesses it. No number in this spec is MEASURED.

## Sources (read and verified this session)

- `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` sections 4 (Phase 2)
  and 5 (scope/boundaries): membrane as dev-only target, canonical SplitMix64
  seeding, black_box + checksum realism, anti-vacuity column-shift court.
- `docs/jira/v26.11.1/bench-campaign-charter.md` (whole doc): crates/bench as
  membrane, exact divan pin, `[[bench]] kernels harness=false`, bench map,
  receipt shape, anti-vacuity flip protocol. Agreements noted inline; the one
  manifest-state conflict is recorded under Divergences.
- Verified paths: `crates/bench/` (pre-scaffold, exists),
  `docs/jira/v26.11.1/waves/` (receipt home), `scripts/boundary_gate.py`,
  `scripts/zero_string_gate.py`.

## Ground state (verified this session, command + exit)

- `grep -rn "\[\[bench\]\]" crates/*/Cargo.toml Cargo.toml` -> exit 0, ONE
  hit: `crates/bench/Cargo.toml:17` (`v26_10_31`, prior campaign). No bench
  target exists in any kernel crate today; the no-contamination invariant
  holds at authoring time and construction must not break it.
- `grep -rn "divan" crates/cgap-*/Cargo.toml` -> exit 1 (zero hits). std and
  divan have no edge into any kernel crate manifest today.
- `cargo search divan` -> exit 0, `divan = "0.1.21"` is current on
  crates.io; the exact-at-authoring-time pin is therefore `=0.1.21`.

## Cargo.toml shape

`crates/bench/Cargo.toml` target shape (path deps only; the sole registry
dependency is the dev-dep divan itself):

```toml
[package]
default-run = "bench"
name = "bench"
version.workspace = true
edition.workspace = true
description = "Divan membrane: v26.11.1 perf-target graduation harness."
publish = false

[dependencies]
cgap-arrow = { path = "../cgap-arrow" }
cgap-blake3 = { path = "../cgap-blake3" }
cgap-parquet = { path = "../cgap-parquet", features = ["std"] }

[dev-dependencies]
# Exact-at-authoring-time pin (cargo search 2026-11-01 -> 0.1.21).
# A caret pin drifts silently across 0.1.x; exact only.
divan = "=0.1.21"

[[bench]]
name = "kernels"
harness = false
```

- Kernel deps are path deps (`path = "../cgap-*"`), never registry versions.
- The existing `[[bench]] v26_10_31` target is retained prior-campaign
  surface; `kernels` is additive. Nothing is deleted by this spec.
- The bench target file is `crates/bench/benches/kernels.rs` (Cargo default
  path for a `[[bench]]` target named `kernels`).

## Entry point shape (`crates/bench/benches/kernels.rs`)

```rust
fn main() {
    divan::Divan::default().run();
}
```

`Divan::default().run()` executes every `#[divan::bench]` function linked
into the `kernels` binary. Per-bench selection at run time uses the standard
cargo filter: `cargo bench -p bench --bench kernels -- <name substring>`.

## No-contamination rule (hard boundary)

`std` must never enter a `no_std` kernel's dependency graph, and divan is a
std-bound harness. Therefore:

1. NO `[[bench]]` targets inside kernel crates
   (`cgap-arrow`, `cgap-blake3`, `cgap-git`, `cgap-parquet`, `cgap-core`,
   `cgap-storage`, `cgap-bont`, `cgap-bosym`, `cgap-bn3`, `cgap-gate`,
   `cgap-crypto`, `cgap-p28`, `cgap-ffi`, `cgap-beam`, `cgap-gpu`).
2. NO divan edge into any kernel crate manifest, not even under
   `[dev-dependencies]` — a dev-dep edge still contaminates
   `cargo test --workspace` builds of the kernel crate.
3. All measurement machinery lives only under `crates/bench`.
4. Enforcement check (must stay exit 1 / zero hits):
   `grep -rn "\[\[bench\]\]" crates/cgap-*/Cargo.toml`
   `grep -rn "divan" crates/cgap-*/Cargo.toml`
5. Deeper proof required at membrane-build time (UNVERIFIED here):
   `cargo tree -p cgap-arrow -e dev` must not list `divan` or `std` as a dev
   edge; run it and cite it in the lane receipt.
6. `cargo test --workspace` (Gate 1) compiles bench targets but does not run
   them, so the membrane's build is witnessed by Gate 1 for free.
7. Bare-metal cross builds of kernel crates stay clean: no bench target, no
   divan, no std — the riscv32imc ELF surface is untouched by the membrane.

## SplitMix64 canonical seeding

The canonical constant, verbatim, both as the SplitMix64 second-round
multiplier and as the pinned corpus seed:

- `0xBF58_476D_1CE4_E5B9`

Any other value (e.g. `0xBF58_476D_1CE4_E5BA`) is the known write-corruption
class: the bench is REFUSED and the lane receipt names the corruption.

```rust
/// Canonical SplitMix64 corpus generator. The constant `0xBF58_476D_1CE4_E5B9`
/// is both the algorithm's second-round multiplier and the pinned corpus seed
/// (`CANONICAL_SEED`). Determinism: same seed + same corpus shape = same
/// bytes, every machine, every run.
const CANONICAL_SEED: u64 = 0xBF58_476D_1CE4_E5B9;

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
```

Corpus rules:

- Every seeded corpus is a pure function of `CANONICAL_SEED` and shape
  parameters: same seed + same shape = same bytes, every machine, every run.
- Seed lines never use arithmetic that could silently remap the constant
  (`seed ^ 1`, `seed + 1` are corruption-shaped).

## black_box + checksum assertions (Chicago realism)

A bench that the optimizer can delete reports fake numbers. Pattern:

```rust
use divan::{black_box, Bencher};

#[divan::bench(consts = [64, 512, 4096, 1_048_576])]
fn bitmask_popcount_validity<const N: usize>(b: Bencher) {
    // Setup, outside the timed region: seeded corpus + output buffer.
    let corpus = seeded_bitmask_corpus::<N>(CANONICAL_SEED);
    let mut out = vec![0u8; N / 8];
    b.bench(|| {
        cgap_arrow::bitmask::popcount_valid(
            black_box(&corpus),
            black_box(&mut out),
        );
        black_box(&out)
    });
    // Final-state assert AFTER the timed region: the optimized kernel must
    // match a straightforward scalar reference on the same seeded corpus.
    let reference = scalar_reference::<N>(&corpus);
    assert_eq!(out, reference, "kernel output drifted from scalar reference");
}
```

Rules:

- `black_box` every consumed input and every produced output. No bare
  passthrough of kernel results.
- Assert final state after the timed loop: optimized-kernel output equals a
  straightforward scalar reference computed at setup (reference-vs-optimized
  cross-check; non-tautological). After the first MEASURED receipt pins a
  golden checksum literal, the literal may replace the scalar reference.
- Benches call real kernel entrypoints on real memory (Chicago): no mocks,
  no stubs, no fake allocators unless the lane charter names the exception.
- min-of-N across whole `cargo bench` runs; no `Process.sleep` anywhere:
  runs are synchronous cargo invocations; the exit code is the observable.

## Anti-vacuity validation protocol

A bench whose number is mutation-invariant is decoration. Each bench names
ONE structural mutation and the Divan column that must move when it lands.

Protocol (court-auditor, one flip per lane):

1. Copy the tree to a `/tmp` sandbox (never mutate the shared tree).
2. sha256 the bench file + `crates/bench/Cargo.toml` before any edit.
3. Apply the named mutation to the sandbox copy.
4. Re-run `cargo bench -p bench --bench kernels -- <name>` in the sandbox.
5. The named column must shift: ns column moves beyond the min-of-N baseline
   spread, or the alloc column leaves `0 B`.
6. Restore byte-exact; sha256 must match step 2 (fail-closed on mismatch).
7. Verdict: column shifted = bench non-vacuous. Column invariant = bench is
   `VACUOUS` and that lane's TARGET -> MEASURED graduation is `REFUSED`.

Named-mutation examples (bench family -> mutation -> column that must move):

| Bench family | Mutation | Column that must shift |
|---|---|---|
| RLE decode (A02) | inject a staged `Vec` before the caller-buffer write | alloc: 0 B -> >0 B |
| Bit-unpack (A02) | widen the width loop to a heap-held stride table | alloc: 0 B -> >0 B |
| Ingress (A04/A14) | insert a data-dependent branch on the payload | ns/frame moves beyond spread |
| Scan/unify (A11-A13) | const size -> runtime loop bound | GB/s drops beyond spread |
| Attestation (A17) | recompute verify with a per-call heap buffer | alloc: 0 B -> >0 B |
| FFI loop (A19) | route the round-trip through a boxing shim | ns/call moves beyond spread |
| Popcount (A01) | swap the const-generic bitmask for a heap Vec | alloc: 0 B -> >0 B |

Exit-code contract (repo-wide gate convention): 0 = non-vacuous, 1 = VACUOUS
(gate fail), 2 = subject missing, 3 = loud skip. A skip is loud, never a
fake pass.

## Receipt and standing discipline

- Receipt = `cargo bench` command + exit code + full Divan table text, per
  lane, into `docs/jira/v26.11.1/waves/bench-<lane>-receipt.md`.
- TARGET -> MEASURED only on that receipt. Until then every perf number
  stays TARGET (including the charter's `~4.0 ns/frame`, `> 1.0 GB/s`,
  sub-us 640-bit verify, 0 B alloc claims).
- Divan numbers are host-machine numbers. Bare-metal flash/SRAM fit stays
  the domain of `llvm-size` over the cross-compiled riscv32imc ELF (Gate 5);
  Divan never speaks for Gate 5.
- Divan allocation counting covers the profiled closures only; Gate 2
  zero-alloc evidence remains `nm`/`zero_string_gate.py` based.
- Flat-tail (p99) claims cite Divan's per-iteration distribution, not a
  percentile the table does not print.

## Divergences

1. The campaign charter says the divan pin is "exact 0.1.x — run cargo
   search at authoring time". The pre-scaffold `crates/bench/Cargo.toml:15`
   carries a caret pin (`divan = "0.1.21"`), not an exact pin. `cargo
   search` this session returned 0.1.21, so the membrane construction must
   exact the pin to `divan = "=0.1.21"` (or re-run cargo search at build
   time and exact whatever it prints, recorded in the receipt).
2. The pre-scaffold crate also carries prior-campaign surface
   (`benches/v26_10_31.rs`, `src/bin/*` measurement binaries). This spec
   adds `[[bench]] kernels`; it deletes nothing — the retained surface
   already builds under Gate 1 today.
3. No conflict between the two charters on the canonical seed constant:
   both pin `0xBF58_476D_1CE4_E5B9` identically.

## Non-goals

- No kernel-crate source changes (membrane construction only).
- No gate rewiring: Gate 3/5 cross-links happen at gate-runner aggregation,
  not here.
- No registry publish, no git transitions (coordinator-only).
