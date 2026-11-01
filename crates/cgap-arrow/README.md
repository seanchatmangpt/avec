# cgap-arrow

Freestanding Apache Arrow columnar substrate: branch-free kernel inner-loop
bodies with itemized latches/dispatch, `no_std` + `alloc` only, zero external
dependencies (`crates/cgap-arrow/Cargo.toml` — empty `[dependencies]`).
Crate version tracks the workspace release (`26.10.29`,
`crates/cgap-arrow/src/generated_version.rs`, projected from
`ontology/cgap.ttl` via `ggen sync run`).

## Explanation: what "freestanding" means here

`cgap-arrow` is freestanding in the strict sense: it compiles with
`#![no_std]` (`crates/cgap-arrow/src/lib.rs`) and depends on no external
crates, so it can be built for any target the Rust compiler supports.

- **Value structs are the spec's structs.** `ArrowArray` and `ArrowSchema`
  (`crates/cgap-arrow/src/c_abi.rs`) are `#[repr(C)]` definitions exactly as
  the Arrow C Data Interface spec defines them — `format`/`name`/`metadata`
  as `*const c_char`, `flags: i64`, children and dictionary pointers, and the
  `Option<unsafe fn>` release callback.
- **Flags come from the spec.** `CGAP_FLAG_NULLABLE = 2`,
  `CGAP_FLAG_DICTIONARY_ORDERED = 1`, `CGAP_FLAG_MAP_KEYS_SORTED = 4`
  (`c_abi.rs`), matching the Arrow spec flag values.
- **Layout ground truth is generated.** `generated_layout.rs` and
  `generated_version.rs` are produced by `ggen sync run` from
  `ontology/cgap.ttl` ("do not edit"); the kernels are the hand-written
  irreducible residue (crate root docs).
- **Branch discipline is itemized.** Kernel inner loops are branch-free;
  latches and dispatch are itemized control flow audited by
  `docs/BRANCH_REGISTER.md` and `scripts/branch_audit.py` over true callee
  extents.
- **Release safety is a control proof.** The export path boxes an owned
  record (validity bitmap, value buffer, buffer pointer table) and installs a
  release callback that atomically swaps the release slot to NULL before
  dropping, so a double release is a typed no-op — never a double free
  (`c_abi.rs`, `release_array`/`release_schema`).
- **Buffers are 64-byte aligned at allocation** (AVX-512 / NEON invariant,
  crate root docs; alignment is asserted by the `columnar_walk` example).

## Tutorial: build, export, release (end to end)

Construct a primitive vector, export it as an owned C Data Interface pair,
release it, and observe that a double release is a no-op. This doctest is
verified with `cargo test -p cgap-arrow --doc`.

```rust
use core::ffi::c_char;

use cgap_arrow::c_abi::{export, ForeignArray};
use cgap_arrow::{Primitive, PrimitiveBuilder};

// 1. Construct: builder API, nulls via `None`.
let mut b = PrimitiveBuilder::<i32>::new();
b.push(Some(10));
b.push(None);
b.push(Some(-7));
let arr: Primitive<i32> = b.finish();

// 2. Export: ownership moves into the ArrowArray/ArrowSchema pair.
//    "i" is the Arrow C-schema format code for i32
//    (Primitive::<i32>::format_code()).
let format = c"i".as_ptr();
let (array, schema) = export(arr, format);

// 3. Import: heap the ArrowArray and hand it to ForeignArray. Its Drop
//    runs the spec's importer duty — release is called exactly once.
//    Release must run on a heap allocation: `release_schema` frees the
//    struct it is given via Box::from_raw, so never release a by-value
//    (stack) copy.
let ptr = Box::into_raw(Box::new(array));
let foreign = unsafe { ForeignArray::take(ptr) };
assert_eq!(foreign.len(), 3);
assert_eq!(schema.flags, cgap_arrow::c_abi::CGAP_FLAG_NULLABLE);

// 4. Release #1: drop runs the release callback, freeing the owned
//    validity/values buffers.
drop(foreign);

// 5. Double release is a typed no-op: release #1 CAS-swapped the in-struct
//    slot to NULL, so release #2 observes `release == NULL` and returns
//    without touching memory — never a double free.
match unsafe { (*ptr).release } {
    Some(f) => unsafe { f(ptr) },
    None => {} // expected: slot is NULL after release #1
}

// 6. Free the struct shell itself (release only freed the owned buffers).
drop(unsafe { Box::from_raw(ptr) });
```

The `columnar_walk` example (Section 5) walks this exact path and prints
alignment proofs, format codes, SWAR mask counts, and the double-release
control proof: `cargo run -p cgap-arrow --example columnar_walk`.

## How-to

### Build with or without `std`

The crate is `no_std` by default. The single feature flag is `std`
(`crates/cgap-arrow/Cargo.toml`), which links the `std` crate on top of
`alloc` for hosts that have it. Both builds compile the same kernels.

```sh
cargo build -p cgap-arrow --no-default-features
cargo build -p cgap-arrow --features std
```

### MSRV

No `rust-version` is declared in the workspace or crate `Cargo.toml`, so no
MSRV is formally pinned. The crate uses `edition 2021` and `core::ffi`
(stable since Rust 1.64); treat 1.64+ as the practical floor, unverified
below it.

## Reference: module map

Every entry below is verified against the module doc comment on the file in
`crates/cgap-arrow/src/`. Items private to the crate (`generated_layout`,
`generated_version`) are named for completeness.

| Module | Role (from module docs) |
|---|---|
| `batch` | Named column batches: the record-batch layer over the vector builders |
| `bitmap` | LSB-first validity bitmap kernels — SWAR scans, branch-free inner-loop bodies |
| `blake3` | Sovereign, standalone, zero-dependency `no_std` BLAKE3 |
| `branchless` | Branchless arithmetic, comparison, alignment, SWAR, memory primitives |
| `buffer` | 64-byte aligned buffer — the memory foundation layer |
| `c_abi` | C Data Interface ABI: zero-copy export/import (`ArrowArray`/`ArrowSchema`) |
| `c_abi_nested` | Nested/child C-ABI export support |
| `chase_lev` | Branchless Chase-Lev work-stealing deque |
| `digest` | Mutation-detection digest kernels |
| `fingerprint` | In-register homomorphic universal fingerprinting |
| `foreign` | Typed zero-copy ingest of foreign C Data Interface pairs |
| `layout` | Canonical 64-byte quad physical layout |
| `mmu_arena` | 16 KiB hardware page-aligned MMU remap arena |
| `perfect_hash` | Branchless, zero-probe multiply-shift perfect minimal hashing |
| `predicates` | Constant-time predication primitives; branchless quad-equality filter |
| `primitive` | Fixed-width primitive vectors: [validity bitmap, value buffer] |
| `runend` | Run-end encoded array support |
| `silicon` | NEON silicon kernels — compile-time dispatch over aarch64 |
| `slice` | Zero-copy array slicing |
| `timestamps` | Timestamp element types over the i64 layout (`tss:`/`tsm:`/`tsu:`/`tsn:`) |
| `transpose` | AoS -> SoA transpose for the 64-byte quad record |
| `varlen` | Variable-length binary/string vectors: [validity, offsets, bytes] |
| `views` | String/binary view vectors (Arrow 2.0 layout: "vu"/"vz") |
| `generated_layout` | Generated by ggen from cgap-pack ontology.ttl (private; do not edit) |
| `generated_version` | Release version projected from `ontology/cgap.ttl` (private) |

## Re-exports at the crate root

Verified from the `pub use` list in `lib.rs`: `Batch`, `BatchBuilder`,
`BatchError`, `Column`, `ColumnBuilder`, `Buf`,
`export`, `ArrowArray`, `ArrowSchema`, `CGAP_FLAG_NULLABLE`,
`format_of`, `is_valid`, `primitive_view`, `ForeignError`, `Primitive`,
`PrimitiveBuilder`, `Varlen`, `VarlenBuilder`, `ViewVec`, `ViewVecBuilder`,
`TimestampMicro/Milli/Nano/Second`, `VERSION`, `VERSION_C`.
