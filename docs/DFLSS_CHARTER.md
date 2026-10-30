# AVEC v26.10.29 DFLSS Charter

DMADV charter for avec v26.10.29, derived only from artifacts present in this repository.
Every claim carries its verification method; status vocabulary is
ALIVE (ran + passed this session, command cited), PARTIAL, BLOCKED (cause cited),
NO-RECEIPT (harness exists, no receipt in repo), NO-MECHANISM (no verification path
exists in the repo), UNVERIFIED (default, never rounded up).

| field | value |
|---|---|
| version | 26.10.29 |
| release date | 2026-10-29 |
| gate surface | `Makefile` (`make test gate0 gate1 gate2 gate3 gates lint publish-check`) |
| project graph | `ontology/avec.ttl` (av:version, av:releaseDate, av:ctq1..av:ctq5) |
| generated surfaces | 4 files: 2 under `include/`, 2 under `crates/avec-arrow/src/` (section 4) |

## 1. Define — scope

avec is a freestanding, branchless Apache Arrow columnar substrate. The attacked failure
classes, read from the code and the graph:

1. allocator jitter: unaligned buffers break vector-alignment assumptions (AVX-512/NEON);
2. branchy kernels: conditional branches bound scan throughput below line rate;
3. copy-based FFI: marshalling copies where the C Data Interface permits ownership handoff.

In scope for v26.10.29:

- `crates/avec-arrow`: in-memory columnar substrate, `no_std` + `alloc`, zero external
  dependencies (empty `[dependencies]` in `crates/avec-arrow/Cargo.toml`).
- `crates/avec-parquet`: branchless Parquet persistence kernels, `no_std`, zero deps:
  SWAR bit-unpacking (1/2/4/8-bit lanes), SWAR RLE/bit-packed hybrid codec, fail-closed
  typed errors (`crates/avec-parquet/src/rle.rs`, `crates/avec-parquet/src/unpack.rs`).
- `crates/audit` (`avec-audit`): cdylib forcing every audited kernel into the artifact so
  the branch audit inspects real compiled machine code, not inlined remnants.
- `crates/bench`: Gate 2 harness — SWAR mask throughput plus allocation interception.
- Membranes: `crates/avec-ffi` (C-ABI cdylib, Gate 3), `crates/avec-beam` (erl_nif
  membrane, in progress).
- Ontology + generated surfaces: `ontology/avec.ttl`, `ggen.toml`, `ggen.lock`, the four
  generated files, projected from `../ggen-marketplace/packs/avec-pack`.
- Gate surface: `Makefile` targets `test gate0 gate1 gate2 gate3 gates lint publish-check`.

Out of scope for v26.10.29 (declared, not merely absent):

- wasm artifact: CTQ-4 declares a target (250 KiB stripped wasm32-wasip1) but no footprint
  harness exists in the repository (`scripts/footprint.sh` is absent).
- Full Parquet file format: kernels only (RLE/bit-unpack); no page/file reader-writer.
- BEAM runtime qualification: `crates/avec-beam` compiles in the workspace test run but
  has no BEAM/AtomVM runtime test in this repo.

## 2. Measure — CTQ register

Canonical numbering is the project graph (`ontology/avec.ttl`, av:ctq1..av:ctq5). Code
citations resolve here: `crates/avec-arrow/src/bitmap.rs` cites charter CTQ-2;
`crates/avec-arrow/src/buffer.rs` cites charter M-03 (section 3).

| id | target (source) | gate | status |
|---|---|---|---|
| CTQ-1 | SWAR bitmap masking >= 9.0 GiB/s (av:ctq1) | `make gate2` | PASS |
| CTQ-2 | CC=1 branch audit gate (av:ctq2) | `make gate1` | PASS |
| CTQ-3 | 0 segfaults — mutation court (av:ctq3) | cargo-mutants court | NO-RECEIPT |
| CTQ-4 | wasm <= 250 KiB stripped wasm32-wasip1 (av:ctq4) | none in repo | NO-MECHANISM |
| CTQ-5 | 100% C Data Interface parity (av:ctq5) | `make gate3` | PASS |
| A-1 | zero post-init allocations in measured scan | `make gate2` | PASS |

Measured baseline, 2026-10-29, macOS arm64, stable Rust, exit codes captured:

- CTQ-1 / A-1 (`cargo run --release -p bench`, exit 0): three same-session runs measured
  43.127 / 57.870 / 62.026 GiB/s single-core mask over 8 rounds, 0 allocations in every
  run; 16-lane aggregate 43.1-54.3 GiB/s. Wall numbers vary with concurrent machine load;
  every run clears the 9.0 GiB/s target by at least 4.8x.
- CTQ-2 (`cargo build --profile audit -p avec-audit` exit 0; `python3
  scripts/branch_audit.py` exit 0): 9/9 audited kernels, 0 conditional branches.
- CTQ-5 (`cargo build -p avec-ffi` exit 0; `python3 scripts/gate3_cdata.py` exit 0):
  int32 batch, 100000 rows, 14285 nulls, bitwise buffer parity through
  `pa.Array._import_from_c` (pyarrow 23.0.1).
- CTQ-3: `cargo-mutants` is installed (`/Users/sac/.cargo/bin/cargo-mutants`) and the
  `Makefile` gate1 recipe points at `docs/RECEIPTS.md`, which does not exist. No court
  receipt is in the repository.
- CTQ-4: no footprint harness exists (`scripts/` contains only `branch_audit.py`,
  `branch_audit.sh`, `gate3_cdata.py`); the target is declared in `ontology/avec.ttl`.

## 3. Analyze — invariants and failure-mode controls

Invariant register (M-numbers are cited by code comments; this is the defining document;
crate-local paths below are under `crates/avec-arrow/src/` unless noted):

| id | invariant | cited in | verified by |
|---|---|---|---|
| M-01 | `no_std` + `alloc` only; zero external deps | crate docs | empty `[dependencies]` |
| M-02 | zero-copy CDI; double release = typed no-op | `c_abi.rs` | ABI tests; Gate 3 parity |
| M-03 | buffers 64-byte aligned at alloc | `buffer.rs`, `primitive.rs` | `buffers_aligned_64` |
| M-04 | scan kernels compile to CC=1 machine code | `bitmap.rs` | `scripts/branch_audit.py` |
| M-05 | fail-closed decode; no panic on bad input | `rle.rs`, `unpack.rs` | typed-error tests |

Failure-mode controls (FMEA rows realized in code):

| failure mode | control | proof |
|---|---|---|
| double release of exported array | atomic swap-to-NULL release | single-free ABI test |
| offset overread, non-monotonic offsets | monotonicity check at freeze | `offsets_monotonic` |
| malformed Parquet page | typed `ParquetError` (EOF, width, overrun) | `malformed_fails_closed` |
| hostile `n` OOM via NIF | `MAX_N = 1 << 28` row cap | `avec-beam` registry |

## 4. Design — layer map and generated-file flow

```text
ontology/avec.ttl  +  ../ggen-marketplace/packs/avec-pack
        |                              |
        +--------- ggen sync run ------+
                       |
     include/avec_c_data.h        include/avec_format_codes.h
     crates/avec-arrow/src/generated_layout.rs
     crates/avec-arrow/src/generated_version.rs
                       |
              hand-written kernels (irreducible residue)
```

- `ggen.toml` pins the pack (`avec-pack = { path = "../ggen-marketplace/packs/avec-pack" }`);
  `ggen.lock` records the pack content hash (blake3) and 2 subjects.
- The four generated files open with `do not edit`; regenerate with `ggen sync run`.
  Gate 0 proves replay determinism: two consecutive syncs leave all four files
  byte-identical, then the C header compiles freestanding under
  `cc -std=c99 -ffreestanding -Wall -Wextra -pedantic`.
- Kernel crates: `bench` and `avec-audit` depend on `avec-parquet` -> `avec-arrow`;
  membranes `avec-ffi` and `avec-beam` depend on `avec-arrow`. Path deps inside the
  workspace root resolve as workspace members.
- Profiles: `release` (thin LTO, `codegen-units = 1`) is the measured artifact; `audit`
  (LTO off, `codegen-units = 16`, strip off) keeps machine code symbol-legible for
  `scripts/branch_audit.py`.

## 5. Verify — gate list, receipts, execution tracker

Gate list (authoritative recipes in `Makefile`):

```console
make test     # cargo test --workspace
make gate0    # ggen sync run x2, byte-identical replay; freestanding C header compile
make gate1    # cargo build --profile audit -p avec-audit; python3 scripts/branch_audit.py
make gate2    # cargo run --release -p bench
make gate3    # cargo build -p avec-ffi; python3 scripts/gate3_cdata.py (needs pyarrow)
make gates    # gate0 gate1 gate2 gate3, in order
```

Receipts — every command below was executed 2026-10-29 in `/Users/sac/avec` with exit
codes captured:

| gate | command | exit | observed |
|---|---|---|---|
| tests | `cargo test --workspace` | 0 | 26 passed, 0 failed |
| gate1 build | `cargo build --profile audit -p avec-audit` | 0 | audit cdylib linked |
| gate1 audit | `python3 scripts/branch_audit.py` | 0 | 9/9 kernels, 0 branches |
| gate2 | `cargo run --release -p bench` | 0 | 43.1-62.0 GiB/s, 0 allocs |
| gate3 build | `cargo build -p avec-ffi` | 0 | cdylib linked |
| gate3 parity | `python3 scripts/gate3_cdata.py` | 0 | bitwise parity, 100000 rows |
| gate0 | `make gate0` | n/a | not run; docs lane does not regenerate shared surfaces |

Session history (recorded, not hidden): earlier in the same session, before the owning
lanes landed fixes, `cargo test --workspace` exited 101 (duplicate `panic_impl` lang item
in the avec-ffi test build; avec-beam `Sync` errors) and both cdylib builds failed at
link (`dyld_stub_binder`, `_bzero`/`_free` undefined). Their owning lanes repaired them;
the statuses above are the current state.

Execution tracker:

| workstream | status | evidence |
|---|---|---|
| avec-arrow in-memory substrate | ALIVE | tests 15/15; gates 1-3 green |
| avec-parquet persistence kernels | ALIVE | tests 11/11; fail-closed decode tests |
| branch audit gate (CTQ-2 / M-04) | ALIVE | audit exit 0, 9 kernels |
| throughput gate (CTQ-1) | ALIVE | bench exit 0, 43.1-62.0 GiB/s, 0 allocs |
| C Data Interface parity (CTQ-5 / M-02) | ALIVE | bitwise parity, 100000 rows |
| avec-ffi C membrane | PARTIAL | parity green; no runtime suite |
| avec-beam NIF membrane | PARTIAL | compiles; no BEAM runtime test in repo |
| mutation court (CTQ-3) | NO-RECEIPT | no court receipt in repository |
| wasm footprint gate (CTQ-4) | NO-MECHANISM | `scripts/footprint.sh` absent |

Testing discipline: Chicago school — real collaborators, assertions on final state, no
mocks (the C ABI tests run real export/release cycles including double release; Gate 3
imports through real pyarrow). No fixed sleeps: gates are synchronous commands.

See Also: `README.md` (developer entry), `Makefile` (gate recipes),
`ontology/avec.ttl` (CTQ facts), `scripts/branch_audit.py`, `scripts/gate3_cdata.py`.
