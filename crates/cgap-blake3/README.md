# cgap-blake3

Sovereign, zero-dependency `#![no_std]` BLAKE3 cryptographic tree hash with 4-way
NEON and 8-way AVX2 parent Merkle reduction (crates.io `cgap-blake3` v26.10.29).

The crate implements the complete BLAKE3 specification: 16-block intra-chunk
sequential compression, a zero-allocation stack-based Merkle tree reduction, and
horizontal SIMD multi-parent reduction kernels. All hashing paths produce
byte-identical output (verified in-repo against the official BLAKE3 test
vectors, `crates/cgap-blake3/tests/official_vectors.rs`).

## Tutorials

### Hash a byte slice

The entry point is `cgap_blake3::hash`, which takes any byte slice and returns
the 32-byte digest:

```rust
let digest = cgap_blake3::hash(b"hello cgap");
assert_eq!(digest.len(), 32);
```

The output is the standard BLAKE3 hash of the input — the in-repo test suite
asserts it against the official BLAKE3 vectors, including the empty input
`af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262`.

```rust
let empty = cgap_blake3::hash(b"");
assert_eq!(
    empty,
    [
        0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6, 0xa0, 0x40, 0x4d, 0xea,
        0x36, 0xdc, 0xc9, 0x49, 0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7,
        0xcc, 0x9a, 0x93, 0xca, 0xe4, 0x1f, 0x32, 0x62,
    ]
);
```

### Incremental hashing

There is no streaming/`ChunkState` incremental API in this crate. The only
per-chunk building block that is public is `hash_chunk`, which hashes one
1 KiB chunk (0..=1024 bytes) at a fixed chunk counter. You can use it to hash
chunk-by-chunk yourself, but the returned value is the 16-word compression
output of the chunk, not a rolling hash object:

```rust
// Hash a single whole chunk as a non-root node.
let chunk = [7u8; 1024];
let out = cgap_blake3::hash_chunk(&chunk, 0, false);
assert_eq!(out.len(), 16);
```

For multi-chunk input, call `hash` (or one of the `hash_*_simd` variants) on
the whole slice; they chunk internally.

## How-to

### SIMD paths: which target gets what

Dispatch is by compile-time target, not runtime detection:

- **`aarch64`** — NEON is assumed (NEON is architectural on ARM64). The
  4-way parent kernel (`neon_parent::compress_parents_4way_neon`) and the
  4-way leaf kernel (`multi_chunk::compress_chunks_4way`,
  `compress_chunks_4way_x2`) compile in unconditionally on `aarch64`.
- **`x86_64`** — the 8-way AVX2 parent kernel
  (`avx2_parent::compress_parents_8way_avx2`) is compiled for `x86_64` but
  only *used* from `reduce_level_simd` when `target_feature = "avx2"` is
  enabled at compile time (e.g. `RUSTFLAGS="-C target-cpu=native"` or
  `-C target-feature=+avx2`). Without it, `reduce_level_simd` falls through
  to the scalar tail. The leaf `multi_chunk` kernels are aarch64-only; on
  `x86_64` they fall back to per-chunk scalar `hash_chunk` calls.
- **Any other target** — pure scalar: `hash` and `hash_chunk` alone, with the
  same byte-identical output.

There is no `simd` cargo feature to enable; the SIMD modules in
`src/lib.rs` are gated purely by `cfg(target_arch)` / `cfg(target_feature)`.

### Choose a hashing function

- `hash(data)` — the reference path: scalar chunk compression plus the stack
  Merkle reduction, with a 4..=64-whole-chunk input window routed through the
  4-way SIMD leaf kernel when eligible.
- `hash_wide_simd(data)` — level-batched horizontal SIMD parent reduction over
  64-chunk subtrees, byte-identical to `hash`.
- `hash_many_simd(data)` / `hash_many_simd_x2(data)` — dual-group 8-chunk
  leaf kernel driver (with 4-way and scalar fallbacks for remainders),
  byte-identical to `hash`.

### Skip GPU dispatch for small payloads

The crate exposes a single architectural dispatch boundary constant for
callers that also route large payloads elsewhere (e.g. Unified Memory GPU
kernels):

```rust
assert!(!cgap_blake3::should_dispatch_gpu(1024));
assert!(cgap_blake3::should_dispatch_gpu(cgap_blake3::GPU_CROSSOVER_BYTES));
```

## Reference

Public API of `cgap_blake3` (all from `src/lib.rs` unless noted):

- `hash(data: &[u8]) -> [u8; 32]` — compute the 32-byte BLAKE3 digest with
  zero-allocation stack Merkle reduction.
- `hash_wide_simd(data: &[u8]) -> [u8; 32]` — same digest via level-batched
  64-chunk SIMD parent reduction; byte-identical to `hash`.
- `hash_many_simd(data: &[u8]) -> [u8; 32]` — leaf chunks via the dual-group
  8-chunk kernel; byte-identical to `hash`.
- `hash_many_simd_x2(data: &[u8]) -> [u8; 32]` — the dual-group driver
  `hash_many_simd` delegates to.
- `hash_chunk(chunk: &[u8], chunk_counter: u64, is_root: bool) -> [u32; 16]` —
  hash one 0..=1024-byte chunk across up to 16 blocks.
- `compress_scalar(cv, block_words, counter, block_len, flags) -> [u32; 16]` —
  the 7-round BLAKE3 compression function (also aliased as `compress`).
- `reduce_level_simd(cvs: &mut [[u32; 8]; 64], cv_count: &mut usize)` —
  horizontal SIMD (4-way NEON / 8-way AVX2) parent reduction of one tree
  level, with a scalar tail.
- `is_small_batch_eligible(len: usize) -> bool` — true for inputs of 4..=64
  whole 1 KiB chunks (the 4-way SIMD leaf-kernel window inside `hash`).
- `should_dispatch_gpu(payload_len: usize) -> bool` — true when
  `payload_len >= GPU_CROSSOVER_BYTES`.
- `GPU_CROSSOVER_BYTES: usize` — the 16 MiB CPU/GPU dispatch threshold.
- `IV: [u32; 8]`, `MSG_PERMUTATION: [usize; 16]` — BLAKE3 constants.
- `CHUNK_START`, `CHUNK_END`, `PARENT`, `ROOT: u32` — domain-separation flags.
- `multi_chunk::compress_chunks_4way(data: &[u8], base_chunk_idx: u64)
  -> [[u32; 16]; 4]` — leaf CVs of 4 consecutive full 1 KiB chunks.
- `multi_chunk::compress_chunks_4way_x2(data: &[u8], base_chunk_idx: u64)
  -> [[[u32; 16]; 4]; 2]` — the 8-chunk dual-group variant.
- `neon_parent::compress_parents_4way_neon(...)` (`aarch64` only, `unsafe`) —
  4 parent compressions in NEON registers.
- `avx2_parent::compress_parents_8way_avx2(...)` (`x86_64` only, `unsafe`) —
  8 parent compressions in AVX2 registers.

## Explanation

### Why a hand-rolled BLAKE3

The design constraint is the zero-dependency `#![no_std]` posture: the crate
has an empty `[dependencies]` table, so it cannot lean on an external BLAKE3
implementation (which would itself pull in `std`-adjacent machinery or its own
dependency tree). Everything — the compression function, the Merkle stack, the
SIMD kernels — is implemented in-crate against the BLAKE3 specification, with
parity to the official vectors as the admission gate. The single opt-in
`std` feature exists for downstream consumers that want it; the crate core
never requires it.

### The parent Merkle reduction, as implemented

`hash` keeps a fixed 54-entry stack of 8-word chaining values on the Rust
stack (depth 54 covers 2^64 bytes) — no heap allocation. Each 1 KiB chunk is
compressed to a CV, then the stack merges: while the running chunk count is
even, two CVs at the top are concatenated into a 16-word parent block and
re-compressed with the `PARENT` flag, deferring the final merge so the last
node can carry `PARENT | ROOT`. `hash_wide_simd` accelerates the same
reduction: aligned runs of 64 chunks are hashed to CVs, then each tree level
is reduced horizontally — pairs gathered into lane arrays, up to 8 parents at
once through the AVX2 kernel (or 4 through NEON), remainder pairs through the
scalar compression — before the per-subtree CV rejoins the same outer stack.
Every path funnels through the identical `compress_scalar`, which is why the
variants are byte-identical rather than merely equivalent.

### Benchmarks

There is no benchmark target inside this crate. The in-repo throughput
harness is `crates/bench/src/bin/blake3_throughput.rs` (min-of-5 rounds over
256 MiB via `hash_wide_simd`, report-only by default; `--gate` exits non-zero
below the ratified 1.0 GiB/s lower service level). No performance number is
claimed here beyond what that harness reports on your machine.

## License

See `LICENSE.md` at the repository root (`license-file` in `Cargo.toml`).
