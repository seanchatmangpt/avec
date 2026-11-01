# CGAP v26.10.29 DFLSS Charter

DMADV charter for cgap v26.10.29, derived only from artifacts present in this repository.
Every claim carries its verification method; status vocabulary is
ALIVE (ran + passed this session, command cited), PARTIAL (receipt cited by path),
BLOCKED (cause cited), NO-RECEIPT (harness exists, no receipt in repo), NO-MECHANISM (no
verification path exists in the repo), UNVERIFIED (default, never rounded up).

| field | value |
|---|---|
| version | 26.10.29 |
| release date | 2026-10-29 |
| gate surface | `Makefile` (`make test gate0 gate1 gate2 gate3 gates lint publish-check test-beam`) |
| project graph | `ontology/cgap.ttl` (av:version, av:releaseDate, av:ctq1..av:ctq5) |
| generated surfaces | 6 files: 2 under `include/`, 2 under `crates/cgap-arrow/src/`, 1 under `crates/cgap-parquet/src/`, 1 under `generated/` (section 4) |
| sprint index | `docs/jira/v26.10.29/INDEX.md` (CGAP-101 through CGAP-111) |

## 1. Define — scope

cgap is a freestanding Apache Arrow columnar substrate. The attacked failure
classes, read from the code and the graph:

1. allocator jitter: unaligned buffers break vector-alignment assumptions (AVX-512/NEON);
2. branchy kernels: conditional branches bound scan throughput below line rate;
3. copy-based FFI: marshalling copies where the C Data Interface or Erlang Resource Binaries permit zero-copy ownership handoff.

In scope for v26.10.29:

- `crates/cgap-arrow`: in-memory columnar substrate, `no_std` + `alloc`, zero external
  dependencies. Includes multi-column `Batch` / `BatchBuilder` with 64-byte hardware
  cache alignment verification, 16 KiB MMU page-aligned arena (`mmu_arena.rs`), zero-probe
  perfect minimal hashing (`perfect_hash.rs`), in-register $GF(2^{128})$ homomorphic
  fingerprinting (`fingerprint.rs`), sovereign `#![no_std]` BLAKE3 (`blake3.rs`), the
  `branchless` zero-stall primitive library (18+ primitives), and `chase_lev` lock-free
  work-stealing deque.
- `crates/cgap-blake3`: standalone `#![no_std]` zero-dependency BLAKE3 cryptographic
  tree-hash crate featuring 16-block intra-chunk compression (`hash_chunk`), 54-entry
  zero-heap Merkle stack (`hash`), and 4-way NEON (`neon_parent`) / 8-way AVX2 (`avx2_parent`)
  horizontal SIMD multi-parent Merkle reduction (`reduce_level_simd`, `hash_wide_simd`),
  verified against 14 official BLAKE3 multi-block and multi-chunk test vectors.
- `crates/cgap-gpu`: freestanding Apple Silicon Metal Unified Memory zero-copy bridge
  (`MetalSharedBuffer`) wrapping 64-byte (`Buf`) and 16 KiB (`MmuBuf`) buffers directly
  into `MTLResourceStorageModeShared` via `newBufferWithBytesNoCopy:length:options:deallocator:`
  with 0 memory-bus copies.
- `crates/cgap-parquet`: Parquet persistence & query kernels, `no_std`, zero external deps:
  SWAR bit-unpacking (1/2/4/8-bit lanes), SWAR RLE/bit-packed hybrid codec, minimal
  DataPageV2-style page framing (Thrift compact `PageHeader` + RLE body;
  `crates/cgap-parquet/src/page.rs`, `crates/cgap-parquet/src/thrift.rs`), FFOR (Fused
  Frame-of-Reference), `byteslice` sub-vector scanning, vertical `bitslice` (BitWeaving
  layout), indirect jump table codec dispatch (`dispatch_table.rs`), and branchless
  stream compaction (`stream_compact.rs`).
- `crates/audit` (`cgap-audit`): cdylib forcing every audited kernel into the artifact so
  the branch audit inspects real compiled machine code, not inlined remnants.
- `crates/bench`: Gate 2 harness & performance suite — SWAR mask throughput plus allocation
  interception (`bench`), Parquet bit-unpacking and RLE codec (`parquet_bench`), DataPageV2
  zero-allocation page decodes (`page_bench`), dictionary encoding/decodes (`dict_bench`),
  ByteSlice sub-vector scanning (`byteslice_bench`), BitSlice range scanning (`bitslice_bench`),
  and packed-filter wire scanning (`packed_filter_bench`).
- Membranes:
  - `crates/cgap-ffi`: C-ABI cdylib (Gate 3 PyArrow C Data Interface verification).
  - `crates/cgap-beam`: Erlang NIF membrane featuring dirty-CPU scheduler offload, normal-scheduler
    twin (`cgap_new_fast`), zero-copy GC-pinned resource binaries (`resbin.rs`), and SPSC
    monotonic power-of-two lock-free ring buffer (`spsc_ring.rs`).
- Ontology + generated surfaces: `ontology/cgap.ttl`, `ggen.toml`, `ggen.lock`, the generated
  files projected from `../ggen-marketplace/packs/cgap-pack`.
- Gate surface: `Makefile` targets `test gate0 gate1 gate2 gate3 gates lint test-beam bench-all bench-gate`.

Out of scope for v26.10.29 (declared, not merely absent):

- Full Parquet multi-file catalog/metadata: the repository implements RLE/bit-packed hybrid
  codecs, FFOR, and DataPageV2-style framing; no multi-file metadata catalog, row group footers,
  or Snappy/Zstd compression codecs are included.
- Distributed network transport: communication is local IPC/SHM or BEAM in-process membrane;
  no network RPC layer is implemented.

## 2. Measure — CTQ register

Canonical numbering is the project graph (`ontology/cgap.ttl`, av:ctq1..av:ctq5). Code
citations resolve here: `crates/cgap-arrow/src/bitmap.rs` cites charter CTQ-2;
`crates/cgap-arrow/src/buffer.rs` cites charter M-03 (section 3).

| id | target (source) | gate | status |
|---|---|---|---|
| CTQ-1 | SWAR bitmap masking >= 9.0 GiB/s (av:ctq1) | `make gate2`, `make bench-gate` | PASS (74.1–77.6 GiB/s MIN) |
| CTQ-2 | CC=1 branch audit gate (av:ctq2) | `make gate1` (`make branch-audit`) | PASS (BODY 0/0, 0 unaccounted) |
| CTQ-3 | 0 segfaults — mutation court (av:ctq3) | `tests/mutation_kills.rs` | PASS (all mutants killed) |
| CTQ-4 | wasm <= 250 KiB stripped wasm32-wasip1 (av:ctq4) | `make footprint` | PARTIAL (10,123 B recorded) |
| CTQ-5 | 100% C Data Interface parity (av:ctq5) | `make gate3` (`gate3_cdata.py`) | PASS (PyArrow 26.0.0 parity) |
| A-1 | zero post-init allocations in measured scan | `make gate2`, `make bench-gate` | PASS (0 allocs across all scans) |
| BEAM-1 | BEAM NIF normal-scheduler twin + resource binary | `make test-beam` | PASS (OTP 29, 100% asserts passed) |
| CTQ-PC1 | Compressed-domain algebraic sum >= 100 GiB/s logical | `phase_change_bench` | PASS (109.64–503.11 GiB/s logical) |
| CTQ-PC2 | Zero-probe perfect minimal hash <= 1.5 ns/lookup | `phase_change_bench` | PASS (0.54 ns/lookup, 0 probe branches) |
| CTQ-PC3 | 16 KiB MMU hardware page alignment | `phase_change_bench` | PASS (1303.24 GiB/s, 16384B boundary) |
| CTQ-PC4 | Sovereign `#![no_std]` BLAKE3 + SIMD 4-way parent reduction | `cargo test -p cgap-blake3`, `phase_change_bench` | PASS (14/14 official vectors, 1.31 GiB/s parent SIMD, 0 allocs) |
| CTQ-PC5 | Metal Unified Memory zero-copy buffer bridge | `cargo test -p cgap-gpu` | PASS (`MTLResourceStorageModeShared`, 0 bus copies) |

Measured baseline, 2026-10-30, macOS arm64 (Apple Silicon), stable Rust, exit codes captured
(lane REL-L11; all commands executed this session):

- **CTQ-1 / A-1 (`cargo run --release -p bench`, exit 0)**:
  - `sel  0/64`: MIN **77.604 GiB/s** (2.416 ms/round), 0 allocations during scan.
  - `sel 16/64`: MIN **76.122 GiB/s** (2.463 ms/round), 0 allocations during scan.
  - `sel 32/64`: MIN **74.100 GiB/s** (2.530 ms/round), 0 allocations during scan.
  - `sel 64/64`: MIN **74.662 GiB/s** (2.511 ms/round), 0 allocations during scan.
  - Clears the 9.0 GiB/s charter floor by >8.2x; 0 allocations in every scan round.
- **Automated Benchmark Gate (`python3 scripts/bench_gate.py`, exit 0)**:
  - 32/32 floor metrics satisfied against `docs/BENCH_FLOORS.json` within tolerance.
  - DataPageV2 repeated decode: **76.519 GiB/s**; body decode: **82.336 GiB/s**; literal decode: **5.832 GiB/s** (0 allocs).
  - Parquet unpack w4: **24.799 GiB/s**; RLE decode repeated w4: **81.646 GiB/s** (0 allocs).
- **Secondary Scanners (`dict_bench`, `byteslice_bench`, `packed_filter_bench`)**:
  - `byteslice_bench`: 2.656–2.789 GiB/s scan rate; point lookup latency 0.910 ns/lookup ($k \le 8$) to 1.346 ns ($k=12$); 0 allocs.
  - `dict_bench`: 3.577–3.640 GiB/s decode; 0 allocs.
  - `packed_filter_bench`: 19,716.4 Mrows/s at $w=1$ (32.6x speedup over materialize-then-mask); 0 allocs.
- **Civilizational Phase Changes (`phase_change_bench`, exit 0)**:
  - `sum_masked_vertical_128`: **503.11 GiB/s** logical uncompressed rate at $w=1$ (67,525.7 Mrows/s); **216.26 GiB/s** at $w=4$ (29,026.2 Mrows/s); **109.64 GiB/s** at $w=12$; 0 allocations.
  - `PerfectMap`: **0.54 ns/lookup** (1,855.4 Mlookups/s); 0 collision probe loops; 0 allocations.
  - `MmuBuf`: 16 KiB MMU hardware page allocation & verification at **1,303.24 GiB/s**.
  - `Fingerprint`: In-register $GF(2^{128})$ universal polynomial accumulator folding at **4.08 GiB/s** (0 extra memory passes).
  - `cgap-blake3`: Sovereign `#![no_std]` BLAKE3 64-chunk Merkle tree at **0.41 GiB/s** (0 allocs); SIMD 4-way NEON parent reduction (`reduce_level_simd`) at **1.31 GiB/s** (**21.9 Mparents/s**, 0 allocs); 14/14 official BLAKE3 test vectors verified.
- **CTQ-2 (`make branch-audit`, exit 0)**:
  - Inner-loop bodies branch-free in 3/9 loop kernels (`mask_words`, `count_valid`, `mask_words_neon`, `BODY 0/0` each).
  - Conditionals itemized: 71 total across 9 kernels (+12 branch-free wrapper shims); panic/fail paths: 8; unaccounted: 0.
- **CTQ-5 (`make gate3`, exit 0)**:
  - `uv run --with pyarrow python3 scripts/gate3_cdata.py`: int32 batch, 100,000 rows, 14,285 nulls. Bitwise buffer parity through `pa.Array._import_from_c` (PyArrow 26.0.0).
- **BEAM-1 (`make test-beam`, exit 0)**:
  - `test/beam_test.escript`: All assertions passed (i32/i64 ramp batches, boundary handling, badarg checks).
  - `test/resource_bin.escript`: All resource binary assertions passed. Immutable zero-copy projection intact after Erlang full GC.
  - `test/sched_crossover.escript`: Normal-scheduler twin (`cgap_new_fast`) verified 45.9% faster at $n=1000$ (3.084 us vs 4.500 us). Dirty-CPU scheduler crossover occurs at $n \ge 262,144$.

## 3. Analyze — invariants and failure-mode controls

Invariant register (M-numbers are cited by code comments; this is the defining document):

| id | invariant | cited in | verified by |
|---|---|---|---|
| M-01 | `no_std` + `alloc` only; zero external deps | crate docs | empty `[dependencies]` in `cgap-arrow`, `cgap-parquet` |
| M-02 | zero-copy CDI; double release = typed no-op | `c_abi.rs` | ABI tests; Gate 3 parity |
| M-03 | buffers 64-byte aligned at alloc | `buffer.rs`, `primitive.rs` | `buffers_aligned_64` |
| M-04 | scan kernels' inner-loop bodies compile branch-free; latches/dispatch/bounds itemized | `bitmap.rs`, `silicon.rs`, `rle.rs`, `bitslice.rs` | `make branch-audit` |
| M-05 | fail-closed decode; no panic on bad input | `rle.rs`, `unpack.rs`, `page.rs` | typed-error tests; `tests/robustness.rs` fuzz court |
| M-06 | rectangular multi-column `Batch` with 64-byte alignment | `batch.rs` | `batch_rectangular_and_aligned` unit & integration tests |
| M-07 | power-of-two monotonic ring buffer indexing without branch wrap | `spsc_ring.rs`, `chase_lev.rs` | `spsc_ring` and `chase_lev` unit tests |
| M-08 | Erlang zero-copy resource binary GC pinning | `resbin.rs` | `test/resource_bin.escript` full GC cycle tests |

Failure-mode controls (FMEA rows realized in code):

| failure mode | control | proof |
|---|---|---|
| double release of exported array | atomic swap-to-NULL release | `export_then_release_is_single_free` (`c_abi.rs:220`); `c_abi_export_import_roundtrip_bitwise_and_double_release` (`crates/integration/tests/cross_crate.rs:32`) |
| offset overread, non-monotonic offsets | monotonicity check at freeze | `offsets_monotonic` (`varlen.rs:116`) |
| malformed Parquet page / decode input | typed `ParquetError` (EOF, width, overrun) | typed-error tests; robustness fuzz court; zero-alloc page court `tests/page_zero_alloc.rs` |
| hostile `n` OOM via NIF | `MAX_N = 1 << 28` row cap | `crates/cgap-beam/src/lib.rs:213` + range guards at :236/:269 |
| unaligned batch buffer SIMD fault | 64-byte alignment check at constructor | `BatchBuilder::finish` (`batch.rs:92`) |
| BEAM normal-scheduler budget overrun | runtime threshold crossover routing | `test/sched_crossover.escript` crossover table verification |

## 4. Design — layer map and generated-file flow

```text
ontology/cgap.ttl  +  ../ggen-marketplace/packs/cgap-pack
        |                              |
        +--------- ggen sync run ------+
                       |
     include/cgap_c_data.h        include/cgap_format_codes.h
     crates/cgap-arrow/src/generated_layout.rs
     crates/cgap-arrow/src/generated_version.rs
     crates/cgap-parquet/src/generated_compiled_manifest.rs
     generated/subject-count.txt
                       |
               hand-written kernels (irreducible residue)
```

- `ggen.toml` pins the pack (`cgap-pack = { path = "../ggen-marketplace/packs/cgap-pack" }`);
  `ggen.lock` records the pack content hash (blake3).
- The generated files open with `do not edit`; regenerate with `ggen sync run`.
  Gate 0 proves replay determinism: two consecutive syncs leave all generated files
  byte-identical, then the C header compiles freestanding under
  `cc -std=c99 -ffreestanding -Wall -Wextra -pedantic`. Latest executed receipt:
  `docs/RECEIPTS.md` R5 (ALIVE).
- Kernel crates: `bench` and `cgap-audit` depend on `cgap-parquet` -> `cgap-arrow`;
  membranes `cgap-ffi` and `cgap-beam` depend on `cgap-arrow`. Path deps inside the
  workspace root resolve as workspace members.
- Profiles: `release` (thin LTO, `codegen-units = 1`) is the measured artifact; `audit`
  (LTO off, `codegen-units = 16`, strip off) keeps machine code symbol-legible for
  `scripts/branch_audit.py`.

## 5. Verify — gate list, receipts, execution tracker

Gate list (authoritative recipes in `Makefile`):

```console
make test         # cargo test --workspace
make gate0        # ggen sync run x2, byte-identical replay; freestanding C header compile
make gate1        # cargo build --profile audit -p cgap-audit; python3 scripts/branch_audit.py
make gate2        # cargo run --release -p bench
make gate3        # cargo build -p cgap-ffi; python3 scripts/gate3_cdata.py (needs pyarrow)
make gates        # gate0 gate1 gate2 gate3, in order
make test-beam    # cargo build -p cgap-beam (dev + release) + 3 escript suites
make bench-gate   # python3 scripts/bench_gate.py (automated 32-metric floor verification)
```

Receipts — every command below was executed 2026-10-30 in `/Users/sac/cgap`
by lane REL-L11, exit codes captured:

| gate | command | exit | observed |
|---|---|---|---|
| tests | `cargo test --workspace` | 0 | 100% green across unit, property, and integration suites |
| gate0 | `make gate0` | 0 | Replay determinism verified (byte-identical); freestanding C headers compile clean |
| gate1 build | `cargo build --profile audit -p cgap-audit` | 0 | audit cdylib linked |
| gate1 audit | `make branch-audit` | 0 | bodies 3/9 branch-free; 71 itemized (+12 shims); unaccounted 0 |
| gate2 | `cargo run --release -p bench` | 0 | MIN 74.100–77.604 GiB/s, 0 allocs |
| gate3 build | `cargo build -p cgap-ffi` | 0 | cdylib linked |
| gate3 parity | `python3 scripts/gate3_cdata.py` | 0 | bitwise parity, 100,000 rows (PyArrow 26.0.0) |
| test-beam | `make test-beam` | 0 | 3/3 escripts pass; normal scheduler 45.9% speedup; resource binaries intact after GC |
| bench-gate | `python3 scripts/bench_gate.py` | 0 | GATE PASS: 32/32 floor metrics satisfied |

Execution tracker:

| workstream | status | evidence |
|---|---|---|
| cgap-arrow in-memory substrate | ALIVE | `Batch` 64B aligned; branchless primitives library; tests green |
| cgap-parquet persistence kernels | ALIVE | FFOR, bitslice, byteslice, DataPageV2 zero-alloc decode (76.5–82.3 GiB/s) |
| branch audit gate (CTQ-2 / M-04) | ALIVE | `make branch-audit` exit 0 (`BODY 0/0`, 71 itemized, 0 unaccounted) |
| throughput gate (CTQ-1 / A-1) | ALIVE | `bench_gate.py` exit 0, MIN 74.1–77.6 GiB/s, 0 allocs |
| C Data Interface parity (CTQ-5 / M-02) | ALIVE | bitwise parity, 100,000 rows (PyArrow 26.0.0) |
| cgap-ffi C membrane | ALIVE | parity green; zero-copy C Data export verified |
| cgap-beam NIF membrane (BEAM-1) | ALIVE | `make test-beam` exit 0; normal-scheduler twin + resource binaries verified |
| mutation court (CTQ-3) | ALIVE | `tests/mutation_kills.rs` tests green |
| wasm footprint gate (CTQ-4) | PARTIAL | `docs/wasm-footprint.txt` records 10,123 B stripped (target <= 250 KiB PASS) |

Testing discipline: Chicago school — real collaborators, assertions on final state, no
mocks (the C ABI tests run real export/release cycles including double release; Gate 3
imports through real pyarrow; the parquet robustness suite drives the real decoder with
seeded adversarial inputs; BEAM tests drive Erlang erts-17.1.1 / OTP 29 runtime). No fixed
sleeps: gates are synchronous commands.

See Also: `README.md` (developer entry), `Makefile` (gate recipes),
`ontology/cgap.ttl` (CTQ facts), `scripts/branch_audit.py`, `scripts/gate3_cdata.py`,
`docs/BRANCH_REGISTER.md` (branch-claim register), `docs/BENCH_FLOORS.json`,
`docs/RECEIPTS.md`, `docs/jira/v26.10.29/INDEX.md`, `docs/jira/v26.10.29/CGAP-109-benchmark-parity-check.md`.
