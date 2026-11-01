# cgap-parquet

Parquet persistence kernels with branch-free inner-loop bodies: bit-unpacking,
SWAR RLE, page-to-buffer decode. `no_std`, zero dependencies.

- Crates.io: [`cgap-parquet` 26.10.29](https://crates.io/crates/cgap-parquet)
- Docs: [docs.rs/cgap-parquet](https://docs.rs/cgap-parquet)

```toml
[dependencies]
cgap-parquet = "26.10.29"
```

## Explanation: the decode pipeline

cgap-parquet implements the inner loops of Parquet column decoding as
branch-free SWAR kernels. The pipeline, as implemented, is:

1. **Page header** — `thrift::read_page_header` parses a Thrift Compact
   Protocol `PageHeader` (real parquet.thrift field ids) by borrowing from
   the page slice. No allocation; any protocol-level failure (truncation,
   unknown type, bad field id, missing required field, nesting bomb)
   collapses to `ParquetError::UnexpectedEof` at page granularity.
2. **Body** — the RLE/bit-packed hybrid body is decoded by
   `rle::decode_into`, which writes lanes directly into caller-owned memory.
   Runs take the SWAR path; literal runs pack `w`-bit lanes LSB-first
   (lane `i` occupies bit range `[i*w, (i+1)*w)`), with a clamped tail:
   missing source words read as zero, source words beyond the output budget
   are never read.
3. **Output** — the caller's `&mut [u32]` is the destination. The fast path
   contains no `Vec`, no copy-out, no intermediate buffer; a
   counting-global-allocator court (`tests/page_zero_alloc.rs`) asserts the
   decode allocates nothing.

Fail-closed is the design rule, not a slogan: malformed pages produce typed
errors (`ParquetError::UnexpectedEof`, `UnsupportedBitWidth`,
`RunOverrun`, `LiteralOverrun`) — never a panic on external input. A page
whose header claims more values than the caller budgeted is refused with
`RunOverrun`; a body that yields fewer lanes than claimed is
`UnexpectedEof`.

Scope, stated honestly: the page framer is a minimal DataPageV2-style
framing — no row groups in the page layer, no compression
(`is_compressed` must be false), zero repetition/definition level byte
lengths. Full-file reading (footer, row groups, mmap scan) lives in
`scan` (std-gated); schema and file assembly in `level_free`.

## Tutorial: decode a page

`write_data_page_v2` emits a complete page (Thrift header + RLE hybrid
body); `page::decode_page_into` decodes it straight into your buffer.

```rust
use cgap_parquet::page::{decode_page_into, write_data_page_v2};

let values = [7u32, 7, 7, 3, 1, 1, 4, 4];
let page = write_data_page_v2(&values, 8, values.len() as u32);

let mut out = [0u32; 8];
let n = decode_page_into(&page, &mut out).unwrap();
assert_eq!(n, 8);
assert_eq!(out, values);
```

Every `bits` outside the SWAR kernel set {1, 2, 4, 8} is not silently
rewritten: the page is emitted as written and decode refuses it with
`UnsupportedBitWidth`. Values are truncated to the declared width, same as
the hybrid codec.

### Dictionary round-trip

`dictionary` provides first-seen-order interning over `u32` values. A
dictionary may hold at most 256 entries (`required_bits` fails with
`UnsupportedBitWidth` above that).

```rust
use cgap_parquet::dictionary::{decode_page, encode_page, DictionaryBuilder};

let values = [5u32, 5, 9, 5, 9, 2];
let mut b = DictionaryBuilder::new();
for v in values {
    b.push(v);
}
let (dict, indices) = b.finish();
let bits = cgap_parquet::dictionary::required_bits(dict.len()).unwrap();

let page = encode_page(&dict, &indices, bits).unwrap();
let mut out = [0u32; 6];
decode_page(&page, &dict, &mut out).unwrap();
assert_eq!(out, values);
```

## How-to

### Feature flags

One optional feature:

- `std` (off by default) — links std for the hosted reader `scan` (mmap,
  `ParquetFile`, footer parsing) and for the `std::error::Error` impls.

Without `std` the crate is pure `no_std` + `alloc`. Note the flag is not
decorative: it actually links std (an earlier revision's gated
`impl std::error::Error` failed to resolve under `cfg`).

### no_std constraints

- `#![no_std]` + `extern crate alloc`; all heap use goes through `alloc`.
- `#![forbid(unsafe_op_in_unsafe_fn)]`.
- `scan` is `#[cfg(feature = "std")]` — the `no_std` build (wasm32
  footprint targets) stays std-free.
- Typed errors everywhere on external input; bounded slice arithmetic;
  no panics on malformed pages.

### Dependency surface

The crate itself has **zero dependencies**. `cgap-arrow` appears only as a
dev-dependency (the wire round-trip property test reaches the real
cgap-arrow layout); it is not a runtime dependency.

### Relationship to cgap-arrow

`cgap-arrow` (same workspace, published separately) owns the Arrow-layout
surface; `cgap-parquet` owns the persistence kernels. The property tests
assert the two agree on wire layout.

## Reference: public surface

Top-level re-exports: `rle_encode`, `rle_decode`, `ParquetError`.

- `bitslice` — `pack_vertical`, `scan_range_vertical(_counted)`:
  vertical bit-slicing and range scans
- `byteslice` — byte-plane kernels
- `compiled_manifest` / `generated_compiled_manifest` — dispatch manifests
- `dictionary` — `required_bits`, `DictionaryBuilder`, `encode_page`,
  `decode_page` (dictionaries capped at 256 entries)
- `dispatch_table` — dispatch tables for kernel selection
- `encode_fast` — `rle_encode_fast`: pre-sized encoder, byte-identical
  output to `rle_encode`
- `ffor` — `ffor_unpack`, `ffor_pack`: frame-of-reference fused with the
  SWAR lane math; widths {1, 2, 4, 8}
- `level_free` — schema/file assembly: `admit_schema`, `write_page`,
  `write_row_group_page`, `write_footer`, `write_file`,
  `read_page_payload`, `LevelFreeType`, `LevelFreeColumn`
- `oblivious` — data-independent access-pattern kernels
- `packed_filter` / `unpack_filter` — predicate filters over packed lanes
- `packing` — `pack_bits`, `unpack_bits_tight`: tight bit packing
- `page` — `write_data_page_v2`, `decode_page_into`: the page framer
- `page_aligned` — aligned page layout (`align_rows_256b` itself lives in
  `level_free`)
- `rle` — `rle_encode`, `rle_decode`, `decode_into`, `write_varint`,
  `ParquetError`: the RLE/bit-packed hybrid
- `scan` (std) — `ParquetFile::open`, `parse_footer`, `Mmap`,
  `value_u32` / `value_u64` / `value_flba`, `write_fixture_file`
- `stream_compact` — streaming compaction
- `thrift` — `read_page_header`, `write_page_header`, `PageHeader`,
  `DataPageHeaderV2`, varint, zigzag
- `unpack` — `unpack_1bit` / `2` / `4` / `8`, `unpack_bits`:
  SWAR bit-unpacking

Error enum `ParquetError`:
`UnexpectedEof` | `UnsupportedBitWidth(u8)` | `RunOverrun { run_len,
budget }` | `LiteralOverrun { claimed, available }`.

## Testing

```sh
cargo test -p cgap-parquet              # no_std core tests
cargo test -p cgap-parquet --features std   # adds scan tests
```

Includes property tests (`codec_properties.rs`), fuzz seeds
(`offset_fuzz.rs`), robustness courts, and a zero-allocation court.

## License

See `LICENSE.md` in the repository root.
