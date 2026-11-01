# cgap

cgap is a columnar substrate: Arrow C Data Interface kernels, a zero-dependency
BLAKE3 tree hash, Parquet persistence kernels, and a Git Packfile v2 writer,
organized as a Cargo workspace of kernel crates. The kernel crates are
`no_std`-leaning with zero external dependencies — each implements its format
from the byte layout up (branch-free inner loops, 64-byte-aligned buffers,
LSB-first validity bitmaps) rather than wrapping an existing implementation.

The workspace was renamed from `avec` on 2026-10-31; crate names, headers, and
generated surfaces use the `cgap` prefix throughout.

## Workspace map

| Crate | Publish | What it is |
|---|---|---|
| `crates/cgap` | yes | Umbrella facade re-exporting the four published kernels verbatim. |
| `crates/cgap-arrow` | yes | Arrow columnar substrate; branch-free inner loops, no_std. |
| `crates/cgap-blake3` | yes | Complete BLAKE3 tree hash, no_std; NEON/AVX2 Merkle reduction. |
| `crates/cgap-git` | yes | Packfile v2 writer: stored blobs, `.idx` v2, in-crate SHA-1/CRC32. |
| `crates/cgap-parquet` | yes | Parquet kernels: bit-unpacking, SWAR RLE, page decode, no_std. |
| `crates/cgap-ffi` | no | C Data Interface cdylib: raw `ArrowArray`/`ArrowSchema` exports. |
| `crates/cgap-beam` | no | BEAM/AtomVM NIF membrane; hand-declared `erl_nif` ABI. |
| `crates/cgap-gpu` | no | Metal zero-copy bridge for aligned Arrow buffers (macOS). |
| `crates/audit` | no | CC=1 branch-audit cdylib; forces kernels into the artifact. |
| `crates/bench` | no | Line-rate benches, alloc interception, branch audit driver. |
| `crates/cli` | no | CLI: version, mask line-rate bench, RLE comparison, alignment probe. |
| `crates/integration` | no | Cross-crate tests: C-ABI roundtrip, alignment law, varlen/views. |
| `crates/wasm-footprint` | no | no_std cdylib link wrapper measured by `scripts/footprint.sh`. |

Publish column reflects `publish` in each `Cargo.toml` at workspace version
26.10.29. `cgap`, `cgap-arrow`, `cgap-blake3`, and `cgap-git` are on crates.io;
`cgap-parquet` (publish=true) is being published.

## Quickstart

```sh
cargo add cgap
```

The umbrella re-exports the kernel crates:

```rust
pub use cgap_arrow;
pub use cgap_blake3;
pub use cgap_git;
pub use cgap_parquet;
```

Each kernel crate can also be depended on directly (`cargo add cgap-arrow`, and
so on). The umbrella exists so one dependency pulls all four.

## Development

```sh
cargo test --workspace   # default `make` target
make gates               # gate0..gate3 in order
make release-check       # full release ladder, fail-closed
```

- `make test` — `cargo test --workspace`
- `make gate0` — two consecutive `ggen sync run` passes must leave the
  generated surfaces byte-identical, then the C data header must compile
  freestanding (`-std=c99 -ffreestanding -Wall -Wextra -pedantic`).
- `make gate1` — CC=1 machine-code branch audit: builds the audit-profile
  cdylib, then `scripts/branch_audit.py` disassembles
  `target/audit/libcgap_audit.dylib` and asserts zero conditional branches
  inside each kernel symbol's extent.
- `make gate2` — release bench run (`cargo run --release -p bench`).
- `make gate3` — C Data Interface wire parity through pyarrow
  `_import_from_c` against `libcgap_ffi.dylib`.
- `make bench-gate` — benches vs `docs/BENCH_FLOORS.json`; exits 1 on any
  >25% drop below floor. `--record` reseeds.
- `make footprint` — wasm32-wasip1 footprint gate (`scripts/footprint.sh`).
- `make branch-audit`, `make symbol-gate` — individual audit gates.
- `make test-beam` — builds `cgap-beam` and runs the BEAM escript suites under
  `test/`.
- `make lint` — `cargo clippy --workspace --all-targets`; warnings tolerated,
  hard errors fail.
- `make publish-check` — `cargo publish --dry-run` for the three published
  kernel crates.

Fail-closed by construction: every recipe runs under `sh -eu`; a nonzero exit
fails the target. Gate receipts live under `docs/`.
