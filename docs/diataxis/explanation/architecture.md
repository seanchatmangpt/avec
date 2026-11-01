# cgap architecture

This page explains *why* cgap is shaped the way it is — the layering, the
dependency posture, the boundary choice, and the audit substrate. For
task-oriented pages see `docs/diataxis/how-to/` and `docs/diataxis/tutorials/`;
for facts-at-a-glance see `docs/diataxis/reference/`.

## The four layers

cgap is a Cargo workspace (`Cargo.toml`, resolver 2) organized as concentric
layers. Reading from the inside out:

```
kernel crates (no_std, zero deps)
  cgap-arrow / cgap-blake3 / cgap-parquet / cgap-git
        |
umbrella facade (crates/cgap)
        |
membranes (cgap-ffi, cgap-beam, cgap-gpu)
        |
consumers (PyArrow, DuckDB, BEAM/AtomVM, Metal)
```

### 1. Kernel crates

Four crates do the actual work and depend on nothing external
(each `Cargo.toml` has an empty `[dependencies]` section):

- `crates/cgap-arrow` — the columnar substrate: buffers, bitmaps, primitive
  and varlen builders, scan kernels, and the C Data Interface types
  (`crates/cgap-arrow/src/lib.rs` enumerates ~26 modules).
- `crates/cgap-blake3` — a sovereign BLAKE3 tree hash with 4-way NEON and
  8-way AVX2 parent Merkle reduction (`crates/cgap-blake3/src/lib.rs`,
  ~1,155 lines).
- `crates/cgap-parquet` — Parquet decode kernels: bit-unpacking, SWAR RLE,
  page-to-buffer decode, thrift parsing (`crates/cgap-parquet/src/`, 20
  modules).
- `cgap-git` — a Git Packfile v2 writer with in-crate SHA-1 and CRC32,
  including its own deflate/inflate (`crates/cgap-git/src/`: `pack_write.rs`,
  `deflate.rs`, `inflate.rs`, `idx_read.rs`, `ladder.rs`).

All four declare `#![no_std]` and use `extern crate alloc` where allocation is
needed. cgap-arrow's module list is the widest surface: layout, branchless
primitives, silicon (NEON) kernels, an MMU arena, and the Arrow C ABI types
all live here.

### 2. Umbrella facade

`crates/cgap` is the one-dependency entry point for the published crates. Its
entire `lib.rs` is 10 lines: `#![no_std]`, four `pub use` re-exports, and a
doc comment stating it "carries no logic of its own — zero extra abstraction
over the kernels it names" (`crates/cgap/src/lib.rs`).

### 3. Membranes

Membranes adapt the kernels for foreign runtimes. None are published to
crates.io (`publish = false` in `crates/cgap-ffi/Cargo.toml` and
`crates/cgap-beam/Cargo.toml`); they are artifacts, not library dependencies.

- `crates/cgap-ffi` — cdylib exporting batches as raw `ArrowArray` /
  `ArrowSchema` pointers for foreign importers (PyArrow, DuckDB, Polars);
  wire parity is proven by importing through pyarrow's `_import_from_c`
  (`crates/cgap-ffi/src/lib.rs` header).
- `crates/cgap-beam` — an Erlang NIF membrane for BEAM/AtomVM. The `erl_nif`
  C ABI is hand-declared in Rust, transcribed from the OTP 28.3.1 header —
  the dylib leaves every `enif_*` symbol undefined for the BEAM to resolve at
  `dlopen` time (`crates/cgap-beam/src/lib.rs`, ~661 lines, plus shm, SPSC
  ring, resource-envelope modules).
- `crates/cgap-gpu` — a thin Metal Unified Memory zero-copy bridge (~105
  lines, `crates/cgap-gpu/src/lib.rs`). It wraps 64-byte-aligned
  `cgap_arrow::Buf`/`MmuBuf` pointers directly into shared-mode Metal buffers
  via two `extern "C"` entry points implemented in the small Objective-C shim
  (`cgap-gpu/metal_shim.m`, built by `cgap-gpu/build.rs` with the `cc` crate).
  This is intentionally minimal — a boundary wrapper, not a GPU kernel
  library; compute kernels are not in this crate.

### 4. Consumers

Foreign consumers never link Rust code. They import zero-copy through the
Arrow C Data Interface or call the cdylib exports. The stardf consumer court
was moved out of cgap entirely (see the dev-dependencies comment in
`crates/cgap-ffi/Cargo.toml`): cgap carries no consumer-side dependency, so
`cargo publish` cannot be blocked by a WIP upstream.

## The zero-dependency / no_std posture

Every kernel crate has an empty `[dependencies]` table, `#![no_std]` by
default with an opt-in `std` feature (`crates/cgap-arrow/Cargo.toml` etc.).

**What it buys:**

- The published crates cannot be broken by an upstream release; there is no
  dependency tree to audit for supply chain or MSRV drift.
- freestanding targets (wasm, AtomVM-adjacent, kernel-adjacent) are buildable
  without patching.
- The compiler is the only optimizer allowed to touch the kernel inner loops,
  which matters for the branch audit (below): the audited machine code is
  attributable to this source alone.

**What it costs:**

- Everything a crate would normally import, it reimplements: BLAKE3, SHA-1,
  CRC32, deflate/inflate, Thrift parsing. That is real, audited code to
  maintain — `crates/cgap-git/src/deflate.rs` is a hand-rolled DEFLATE.
- `std` conveniences (thread APIs, `Instant`, path handling) are absent
  unless the `std` feature is enabled.
- Spec surface must be transcribed by hand where a crate would have wrapped
  it: the erl_nif ABI in cgap-beam and the Arrow C Data Interface structs in
  cgap-arrow both live as `#[repr(C)]` declarations in this tree.

## The boundary: Arrow C Data Interface, not a Rust trait

cgap's interop boundary is the Arrow C Data Interface — the spec structs
`ArrowSchema` and `ArrowArray` declared `#[repr(C)]` in
`crates/cgap-arrow/src/c_abi.rs` — rather than a Rust trait or a Rust-native
type system boundary. The reasoning:

- **A trait boundary is a Rust boundary.** `dyn Trait` in a cdylib export
  cannot be consumed by PyArrow or DuckDB; a C ABI struct can, by design, on
  any runtime that reads the spec. The membrane stays a cdylib, and the
  consumer stays language-agnostic.
- **Zero copy is structural.** `ArrowArray` carries raw buffer pointers, so
  export is pointer handoff, not serialization. cgap-arrow's alignment
  invariant (every buffer 64-byte aligned at allocation,
  `crates/cgap-arrow/src/lib.rs`) makes those buffers directly usable by
  AVX-512/NEON kernels and by Metal shared memory (`cgap-gpu`) alike — one
  buffer, many consumers, no copies.
- **Ownership is spec'd, not invented.** Release is a callback in the struct;
  cgap makes double-release a typed no-op by atomically swapping `release` to
  NULL before the private data drops (`crates/cgap-arrow/src/c_abi.rs`
  header comment). Ownership protocol comes from the Arrow spec rather than
  cgap's own conventions.
- **Stable ABI implies stable audit surface.** The audit crate exports
  kernels through the same C ABI, so the audited machine code is exactly what
  a consumer loads.

## The audit / allocator-court substrate

`crates/audit` (`cgap-audit`, not published) is the verification substrate
that keeps performance claims honest. Two pieces:

- **Branch-audit cdylib** (`crates/audit/src/lib.rs`): `#[no_mangle] extern
  "C"` wrappers (`audit_mask_words`, `audit_mask_words_neon`,
  `audit_count_valid`, `audit_unpack_1bit` …) around real kernels from
  cgap-arrow and cgap-parquet. The premise, stated in the crate header: the
  linker cannot elide an exported function, so a branch-count audit inspects
  the REAL compiled kernel machine code, not a debug build or an inlined
  approximation. The per-kernel branch disclosure lives in
  `crates/cgap-arrow/src/silicon.rs` and is checked by
  `scripts/branch_audit.py`.
- **Allocator court** (`crates/audit/src/alloc_counter.rs`): a `CountingAlloc`
  `GlobalAlloc` wrapper installed as the process `#[global_allocator]`, with
  a const-init TLS scoped guard `with_alloc_tracking` that reports the
  allocations a closure performed. Tests and benches assert the zero-
  allocation structural invariant (decode scan, admission path) on real
  state; in the audit cdylib it is a two-atomic-add pass-through on
  untracked threads. cgap-ffi's `tests/alloc_courts.rs` links cgap-audit to
  get these assertions (`crates/cgap-ffi/Cargo.toml` dev-dependencies
  comment).

## Where beam and gpu sit, honestly

- `cgap-beam` is the most complete membrane by line count (~2,400 lines of
  non-test Rust across `lib.rs`, `shm.rs`, `ops_scan.rs`, `ops_persist.rs`,
  `resources.rs`, `resbin.rs`, `spsc_ring.rs`, `alloc_spy.rs`, `dtor_stats.rs`,
  `pointer.rs`). It is a real NIF implementation with a hand-transcribed ABI,
  an integer-handle registry (`static Mutex<Vec<Option<Batch>>>`), and a
  GC-owned resource envelope. It is not published and its OTP header was
  transcribed from a local path (`/Users/sac/.erlmcp/otp-28.3.1/...`), so the
  transcription is environment-specific in provenance.
- `cgap-gpu` is a boundary wrapper, not a GPU substrate: one struct
  (`MetalSharedBuffer`), one test, two extern entry points, ~105 lines
  (`crates/cgap-gpu/src/lib.rs`). It buys zero-copy into Metal Unified Memory
  on Apple Silicon and nothing more — there are no Metal compute kernels in
  the tree. Treat it as an early-stage bridge.

## Generated vs hand-written

Version strings and Arrow struct layout ground truth are generated from the
cgap-pack ontology (`crates/cgap-arrow/src/generated_version.rs`,
`generated_layout.rs`, per the lib.rs header). The kernels are described as
the "hand-written irreducible residue" — generation covers the spec plumbing,
humans own the inner loops. This split is why the layout is
ontology-projected (stable, regenerable) while the kernels are the audited
hand-written core.

## Summary diagram

```
ontology (cgap-pack) --ggen--> generated layout/version
                                   |
kernel crates: cgap-arrow, cgap-blake3,     zero external deps,
  cgap-parquet, cgap-git                    #![no_std] + alloc
                                   |
umbrella facade: cgap (10-line pub use)
                                   |
membranes: cgap-ffi (Arrow C ABI cdylib),
  cgap-beam (NIF), cgap-gpu (Metal wrap)    publish = false
                                   |
consumers: PyArrow / DuckDB / BEAM / Metal
```

## See Also

- `docs/diataxis/INDEX.md`
- `docs/BRANCH_REGISTER.md` (itemized branch control flow)
- `docs/DATA_INDEPENDENCE_AUDIT.md`
- `docs/RECEIPTS.md`
