# cgap

Umbrella facade for the cgap kernel crates: one dependency, no logic of its
own. `cgap` is `#![no_std]` and re-exports the published surfaces of
`cgap-arrow`, `cgap-blake3`, `cgap-git`, and `cgap-parquet` verbatim
(`version = 26.10.29` for all four).

## Install

```sh
cargo add cgap
```

## Tutorial

Every kernel is reachable through the facade root, e.g.
`cgap::cgap_blake3::hash`. The snippets below compile and run against the
workspace at v26.10.29 (see Receipts at the bottom).

### Arrow bitmaps (cgap-arrow)

```rust
use cgap::cgap_arrow::bitmap;

let valid = [bitmap::bcast8(0b1111_1111)];
let pred = [bitmap::bcast8(0b1010_1010)];
// bcast8 replicates each bit into all 8 byte lanes of a u64 word:
// 4 set bits x 8 lanes = 32 matching bits in one word.
assert_eq!(bitmap::count_valid(&valid, &pred), 32);
```

### Hashing (cgap-blake3)

```rust
let digest = cgap::cgap_blake3::hash(b"cgap");
assert_eq!(digest.len(), 32);
```

### Git packfiles (cgap-git)

```rust
use cgap::cgap_git::PackBuilder;

let mut builder = PackBuilder::new();
builder.push(b"hello\n");
let (pack, idx) = builder.finish().unwrap();
assert_eq!(&pack[0..4], b"PACK");
assert!(!idx.is_empty());
```

### Parquet RLE (cgap-parquet)

```rust
use cgap::cgap_parquet::rle;

let values: [u32; 5] = [7, 7, 7, 9, 9];
let encoded = rle::rle_encode(&values, 8);
let mut decoded = [0u32; 5];
rle::rle_decode(&encoded, &mut decoded).unwrap();
assert_eq!(decoded, values);
```

## Reference

### `cgap::cgap_arrow`

Branch-free Apache Arrow columnar substrate: `no_std` + `alloc`, zero
external dependencies, every buffer 64-byte aligned at allocation, C Data
Interface (`ArrowArray` / `ArrowSchema`) zero-copy export/import.

### `cgap::cgap_blake3`

Sovereign, standalone, zero-dependency BLAKE3: complete spec including
zero-allocation Merkle tree reduction (`hash` -> `[u8; 32]`), 4-way NEON and
8-way AVX2 multi-parent SIMD, 16 MiB GPU dispatch crossover.

### `cgap::cgap_git`

Git Packfile v2 writer (FR-04): `PackBuilder`, `write_pack_v2`,
`write_idx_v2`; object bodies are zlib DEFLATE streams, with SHA-1, CRC32,
and DEFLATE all in-crate — zero external dependencies.

### `cgap::cgap_parquet`

Parquet persistence kernels: vectorized bit-unpacking, SWAR RLE/bit-packed
hybrid decode, direct page-to-buffer decode; fail-closed on malformed pages
with typed errors, never a panic. The `std` feature gates the hosted `scan`
reader.

## Explanation: why an umbrella crate

Each kernel is published and usable standalone; `cgap` exists so consumers
who want the whole columnar substrate (Arrow + BLAKE3 + Parquet + Git
packfiles) take one dependency instead of four, and so the set moves as one
version. The facade carries zero logic: `crates/cgap/src/lib.rs` is four
`pub use` lines, and its version tracks the workspace (26.10.29), matching
the pins in its `Cargo.toml` exactly. Pick a single kernel crate directly if
you need only one surface.

## Receipts

Examples verified 2026-10-31 in a scratch crate against the workspace path
dependencies (`cargo run`, exit 0, all asserts passed). `cargo test -p cgap
--doc` exits 0 with 0 doc tests — the facade's `lib.rs` contains no doc
examples of its own; the snippets above were executed externally as shown.
