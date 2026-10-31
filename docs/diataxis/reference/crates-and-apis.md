# Reference: Crates, Modules, and APIs (`avec` v26.10.29)

Information-oriented reference covering every workspace crate, public module, core invariant, and measured CTQ baseline.

---

## 1. Workspace Crates

| Crate | Path | Attributes | External Deps | Description |
|---|---|---|---|---|
| `avec-arrow` | `crates/avec-arrow` | `#![no_std]` (`alloc`) | `0` | Freestanding Apache Arrow columnar substrate, 64B `Buf`, 16 KiB `MmuBuf`, `PerfectMap`, `Fingerprint`, `blake3`, C Data Interface |
| `avec-blake3` | `crates/avec-blake3` | `#![no_std]` (no `alloc`) | `0` | Sovereign BLAKE3 cryptographic tree hash with 16-block intra-chunk chaining, 54-depth stack Merkle tree, and 4-way NEON / 8-way AVX2 parent reduction |
| `avec-parquet` | `crates/avec-parquet` | `#![no_std]` (`alloc`) | `0` | Parquet codecs & compressed-domain query kernels: RLE/bit-packed hybrid, FFOR, `bitslice`, `byteslice`, Thrift compact header, DataPageV2 |
| `avec-gpu` | `crates/avec-gpu` | `std` + Metal FFI | `cc` (build) | Freestanding Apple Silicon Metal Unified Memory zero-copy bridge (`MetalSharedBuffer`, `MTLResourceStorageModeShared`) |
| `avec-ffi` | `crates/avec-ffi` | `cdylib` | `0` | C Data Interface shared library membrane for PyArrow / C consumers |
| `avec-beam` | `crates/avec-beam` | `cdylib` (`erl_nif`) | `0` | Erlang/OTP & AtomVM NIF membrane: dirty/normal scheduler twins, GC-pinned resource binaries (`resbin`), POSIX SHM SPSC ring |
| `avec-audit` | `crates/audit` | `cdylib` | `0` | Machine-code disassembly target for `scripts/branch_audit.py` |
| `bench` | `crates/bench` | `bin` suite | `0` | Release benchmark harnesses with global allocation interception |

---

## 2. `avec-blake3` Public API Reference

| Symbol | Signature | Description |
|---|---|---|
| `IV` | `[u32; 8]` | BLAKE3 initialization vector words |
| `MSG_PERMUTATION` | `[usize; 16]` | Round message word permutation schedule |
| `CHUNK_START` | `u32 = 1 << 0` | Flag set on the first 64-byte block of a 1024-byte chunk |
| `CHUNK_END` | `u32 = 1 << 1` | Flag set on the final 64-byte block of a 1024-byte chunk |
| `PARENT` | `u32 = 1 << 2` | Flag set when compressing two 32-byte child chaining values |
| `ROOT` | `u32 = 1 << 3` | Flag set on the single root node of the Merkle tree |
| `GPU_CROSSOVER_BYTES` | `usize = 16 * 1024 * 1024` | 16 MiB payload threshold where Unified Memory GPU SIMT saturates leaf chunks |
| `should_dispatch_gpu` | `const fn(usize) -> bool` | Returns `true` when `payload_len >= GPU_CROSSOVER_BYTES` |
| `compress_scalar` / `compress` | `fn(&[u32; 8], &[u32; 16], u64, u32, u32) -> [u32; 16]` | 7-round ChaCha-based BLAKE3 block compression function |
| `hash_chunk` | `fn(&[u8], u64, bool) -> [u32; 16]` | Compresses a single chunk (`0..=1024` bytes) across up to 16 sequential 64B blocks |
| `hash` | `fn(&[u8]) -> [u8; 32]` | Computes the 32-byte BLAKE3 Merkle digest using a zero-heap 54-entry stack |
| `hash_wide_simd` | `fn(&[u8]) -> [u8; 32]` | Computes the 32-byte BLAKE3 digest using 64-chunk level-batched SIMD parent reduction |
| `reduce_level_simd` | `fn(&mut [[u32; 8]; 64], &mut usize)` | Reduces a horizontal queue of CVs using 4-way NEON (`aarch64`) or 8-way AVX2 (`x86_64`) |
| `neon_parent::compress_parents_4way_neon` | `unsafe fn(&[[u32; 8]; 4], &[[u32; 8]; 4], [u32; 4], &mut [[u32; 8]; 4])` | 4-way parallel parent compression in `uint32x4_t` NEON registers |
| `avx2_parent::compress_parents_8way_avx2` | `unsafe fn(&[[u32; 8]; 8], &[[u32; 8]; 8], [u32; 8], &mut [[u32; 8]; 8])` | 8-way parallel parent compression in `__m256i` AVX2 registers |

---

## 3. `avec-arrow` Phase-Change Modules

| Module | Key Types / Functions | Invariant / Performance Receipt |
|---|---|---|
| `buffer` | `Buf::new`, `Buf::new_zeroed` | 64-byte hardware cache-line alignment (`M-03`) |
| `mmu_arena` | `MmuBuf::with_capacity`, `MmuBuf::is_mmu_page_aligned` | 16 KiB (`16,384` B) hardware MMU page alignment (**1,303.24 GiB/s**) |
| `perfect_hash` | `PerfectMap::try_build`, `PerfectMap::get` | Zero-probe $O(1)$ multiply-shift lookup (`((key * M) >> S) ^ seed`, **0.54 ns/lookup**, 0 allocs) |
| `fingerprint` | `Fingerprint::fold_word`, `Fingerprint::combine` | In-register $GF(2^{128})$ polynomial folding (**4.08 GiB/s**, 0 extra memory passes) |
| `blake3` | `hash`, `hash_wide_simd`, `reduce_level_simd` | Sovereign `#![no_std]` BLAKE3 (**1.31 GiB/s** 4-way NEON parent reduction, 0 allocs) |
| `branchless` | 18+ branch-free arithmetic/SWAR/memory primitives | `BODY 0/0` inner-loop foundation |

---

## 4. `avec-parquet` Phase-Change Modules

| Module | Key Functions | Invariant / Performance Receipt |
|---|---|---|
| `bitslice` | `pack_vertical`, `sum_masked_vertical_128`, `range_le_64` | Algebraic compressed-domain sum: **503.11 GiB/s** logical uncompressed ($w=1$), **216.26 GiB/s** ($w=4$), 0 allocs |
| `byteslice` | `ByteSlices`, `scan_eq`, `point_get` | Sub-vector byte-sliced scanning (**2.789 GiB/s**, **0.91 ns/get**) |
| `ffor` | `encode_ffor`, `decode_ffor` | Fused Frame-of-Reference (**9.782 GiB/s**, 1.54x over two-pass) |
| `unpack_filter` | `unpack_and_filter_gt` | Fused single-pass unpack + predicate mask (**13.94 GiB/s** at $w=4$, 2.35x over two-pass) |
| `oblivious` | `oblivious_gather`, `oblivious_compact` | Data-independent constant-trace primitives |
