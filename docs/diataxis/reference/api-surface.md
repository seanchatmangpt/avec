# API surface reference (26.10.29)

Reference for the public API surface of the five published crates (`cgap`,
`cgap-arrow`, `cgap-blake3`, `cgap-git`, `cgap-parquet`) at workspace version
`26.10.29`. All five are `no_std`-first (`std` behind a feature where
present); SHA-1, CRC32, DEFLATE and BLAKE3 are implemented in-crate with zero
external dependencies. Cross-references are plain code spans (relative links
would not resolve on crates.io). Descriptions derive from signatures and
module docs in `src/` at `26.10.29`.

## `cgap` (umbrella facade)

`#![no_std]`, no logic of its own: re-exports the four kernel crates verbatim.

- `cgap::cgap_arrow` — re-export of `cgap-arrow`.
- `cgap::cgap_blake3` — re-export of `cgap-blake3`.
- `cgap::cgap_git` — re-export of `cgap-git`.
- `cgap::cgap_parquet` — re-export of `cgap-parquet`.

## `cgap-arrow`

Freestanding Apache Arrow columnar substrate: branch-free kernel inner-loop
bodies with latches, dispatch and bounds checks itemized as control flow
(`docs/BRANCH_REGISTER.md`); every buffer 64-byte aligned at allocation
(AVX-512 / NEON invariant); C Data Interface zero-copy export/import with
atomic release-swap double-release control. Layout ground truth is generated
(`generated_layout.rs`) from the cgap-pack ontology; the kernels are the
hand-written irreducible residue.

### Modules

`batch`, `bitmap`, `blake3`, `branchless`, `buffer`, `c_abi`,
`c_abi_nested`, `chase_lev`, `digest`, `fingerprint`, `foreign`, `layout`,
`mmu_arena`, `perfect_hash`, `predicates`, `primitive`, `runend`, `silicon`
(platform-gated), `slice`, `timestamps`, `transpose`, `varlen`, `views`;
private `generated_layout`, `generated_version`.

### Root re-exports

- `VERSION`, `VERSION_C` — version (26.10.29) projected from the project
  graph (`ontology/cgap.ttl`); `VERSION_C` is the NUL-terminated C form.
- `Batch`, `BatchBuilder`, `BatchError`, `Column`, `ColumnBuilder` — batch
  container and builders from `batch`.
- `Buf` — aligned buffer from `buffer`.
- `ArrowArray`, `ArrowSchema`, `CGAP_FLAG_NULLABLE`, `export` — C ABI
  surface from `c_abi`.
- `ForeignError`, `format_of`, `is_valid`, `primitive_view` — foreign-array
  accessors from `foreign`.
- `Primitive`, `PrimitiveBuilder` — primitive arrays from `primitive`.
- `TimestampMicro`, `TimestampMilli`, `TimestampNano`, `TimestampSecond` —
  timestamp newtypes from `timestamps`.
- `Varlen`, `VarlenBuilder` — variable-length arrays from `varlen`.
- `ViewVec`, `ViewVecBuilder` — binary/string view arrays from `views`.

### `batch`

- `enum Column` — typed column value, one variant per physical layout;
  `len`, `is_empty`, `format_code`.
- `enum BatchError` — batch construction errors.
- `struct Batch` — named columns with a uniform row count; `try_new`,
  `row_count`, `columns`, `get`.
- `struct BatchBuilder` — `new`, `with_column`, `column_mut`, `finish`.
- `enum ColumnBuilder` — per-variant builder; `len`, `is_empty`, `finish`.

### `bitmap`

- `bitmap_bytes(n_bits)`, `bitmap_words(n_bits)` — storage size for n bits.
- `bcast8(b)` — broadcast a byte into a `u64` word.
- `mask_words(valid, pred, out)` — word-wise combine of validity and
  predicate masks into `out`.
- `count_valid(valid, pred)` — popcount of the combined mask.
- `get_bit`, `set_bit`, `clear_bit` — LSB-order bitmap element access.
- `word_valid(bits, word_idx)`, `word_at(bytes, word_idx)` — per-word
  extraction.
- `struct Bitmap` — validity bitmap over a 64-byte-aligned `Buf`;
  `from_bools`, `from_bytes`, `n_bits`, `bytes`, `as_ptr`, `get`, `set`,
  `word`, `words_len`, `is_all_valid`, `null_count`.
- `count_one_bits(bm)` — popcount of a whole bitmap.

### `blake3` (module)

In-crate BLAKE3 for the columnar substrate; mirrors the `cgap-blake3` crate
surface.

- Constants: `IV`, `MSG_PERMUTATION`, `CHUNK_START`, `CHUNK_END`, `PARENT`,
  `ROOT`.
- `GPU_CROSSOVER_BYTES` — payload size above which GPU dispatch is
  indicated (16 MiB).
- `should_dispatch_gpu(payload_len)` — const crossover predicate.
- `compress_scalar(...)` — scalar BLAKE3 compression function.
- `compress(...)` — dispatching compress (scalar vs platform path).
- `hash_chunk(chunk, chunk_counter, is_root)` — hash one chunk to a 16-word
  state.
- `hash(data)` — BLAKE3 digest of a byte slice (`[u8; 32]`).
- Modules `neon_parent`, `avx2_parent` — platform SIMD parent-block
  compression.
- `reduce_level_simd(cvs, cv_count)` — fold a CV level with SIMD parent
  compression.
- `hash_wide_simd(data)` — SIMD-wide tree hashing to a 32-byte digest.

### `branchless`

Branch-free scalar primitives (all pure functions over words):

- Masks/selects: `branchless_mask_bool`, `branchless_mask_sign`,
  `branchless_select`, `branchless_eq`, `is_nonzero`, `branchless_signum`.
- Arithmetic: `branchless_min`, `branchless_max`, `branchless_abs`,
  `branchless_clamp`.
- Alignment/integral: `is_p2`, `round_up_p2`, `align_64`,
  `is_aligned_64`, `ring_modulo`.
- Bitwise: `get_bit`, `set_bit`, `lowest_set_bit`,
  `clear_lowest_set_bit`, `has_zero_byte`, `match_byte_needle`,
  `reverse_bits_swar`.
- Data movement: `branchless_compact_step` (compaction step writing through
  a write head), `oblivious_gather_u32` (16-entry table gather).

### `buffer`

- `ALIGNMENT` — 64.
- `struct Buf` — 64-byte-aligned byte buffer; `new`, `new_zeroed`, `len`,
  `is_empty`, `as_ptr`, `as_mut_ptr`, `as_slice`, `as_mut_slice`,
  `as_typed<T>`.

### `c_abi`

C Data Interface export/import (`ArrowArray` / `ArrowSchema`).

- `CGAP_FLAG_NULLABLE`, `CGAP_FLAG_DICTIONARY_ORDERED`,
  `CGAP_FLAG_MAP_KEYS_SORTED` — schema flag bits.
- `struct ArrowSchema`, `struct ArrowArray` — C ABI structs.
- `export<T: Scalar>(arr, format)` — zero-copy export of a `Primitive` to
  `(ArrowArray, ArrowSchema)` with atomic release-swap.
- `struct ForeignArray` — owned handle over an imported foreign array;
  `len`, `is_empty`.

### `c_abi_nested`

- `export_struct(...)` — export of a nested struct array across the C Data
  Interface.

### `chase_lev`

- `struct WorkStealingDeque<T: Copy>` — lock-free Chase–Lev work-stealing
  deque; `new`, `push`, `pop`, `steal`.

### `digest`

- `mask_words_hashed(valid, pred, out)` — combine validity and predicate
  masks, returning a rolling digest word.
- `digest_of(arr: &Primitive<i32>)` — digest of a primitive array.

### `fingerprint`

- `struct Fingerprint` — 128-bit structural fingerprint; `ZERO`, `new`,
  `fold_word`, `combine`.

### `foreign`

Safe/unsafe accessors over imported C Data Interface arrays.

- `enum ForeignError` — typed errors for foreign schema/array access.
- `unsafe format_of(schema)` — format string of an imported `ArrowSchema`.
- `unsafe primitive_view<T: Scalar>(...)` — typed view over an imported
  primitive array.
- `unsafe is_valid(...)` — validity check against an imported array.

### `layout` (RDF quad layout)

- `type TermId = u32` with `BOTTOM_TERM_ID` (0x0000_0000) and
  `WILDCARD_TERM_ID` (0xFFFF_FFFF).
- `QUAD_ALIGN` (64), `QUAD_SIZE` (64).
- `struct Quad64` — one 64-byte RDF quad with wildcard/bottom term
  matching; `ZERO`, `new`, `is_bottom`, `is_wildcard`,
  `term_matches(pattern, term)`, `matches`, `to_le_bytes`, `from_le_bytes`.

### `mmu_arena`

- `MMU_PAGE_SIZE` — 16384.
- `struct MmuBuf` — page-backed growable buffer; `with_capacity`,
  `extend_from_slice`, `as_ptr`, `as_mut_ptr`, `is_empty`, `len`,
  `capacity`, `is_mmu_page_aligned`.

### `perfect_hash`

- `struct PerfectMap` — minimal perfect hash over `u32` keys; `try_build`,
  `get`.

### `predicates`

Constant-time predicates and mask ops over `u32`/`u64`/`usize`:

- `ct_eq_u32`, `ct_lt_u32`, `ct_select_u32`, `ct_select_u64`,
  `ct_select_usize`, `ct_eq_u64_mask`, `ct_lt_u64_mask`,
  `ct_quad_eq_u32` — constant-time compare/select returning masks.
- `filter_quads_eq(data, n_quads, needle, out)` — branch-free filter of
  quad rows matching a needle, returning hit count.

### `primitive`

- `trait Scalar` — copy-able physical type with format code.
- `struct Primitive<T: Scalar>` — values buffer plus optional validity
  bitmap; `len`, `is_empty`, `format_code`, `values`, `validity`,
  `values_ptr`, `validity_ptr`, `buffers_aligned_64`.
- `struct PrimitiveBuilder<T: Scalar>` — `new`, `push(Option<T>)`, `len`,
  `is_empty`, `finish`.

### `runend`

- `RUN_END_FORMAT_CODE` — `"+r"`.
- `struct RunEndEncoded<I: Scalar>` — run-end encoded array; `format_code`,
  `logical_len`, `is_empty`, `num_runs`, `run_ends`, `values`, `get`,
  `decode_to_dense`, `buffers_aligned_64`.
- `struct RunEndEncodedBuilder<I: Scalar>` — `new`, `push(i32)`, `len`,
  `is_empty`, `finish`.

### `silicon` (platform-gated)

- `mask_words_neon(valid, pred, out)` — NEON path of the mask-combine
  kernel (aarch64 builds; fallback definition on other targets).

### `slice`

- `enum SliceError` — typed zero-copy slicing errors.
- `struct Sliced<'a, T: Scalar>` — zero-copy window over a `Primitive`;
  `len`, `is_empty`, `get`, `iter`.
- `slice(arr, offset, len)` — checked zero-copy slice of a primitive array.

### `timestamps`

Macro-generated timestamp newtypes over `i64`, one per unit:
`TimestampSecond`, `TimestampMilli`, `TimestampMicro`, `TimestampNano`.

### `transpose`

- `struct Quad64` (transpose view), `struct QuadColumns` — columnar
  (struct-of-arrays) form of quads; `untranspose`.
- `transpose(aos) -> QuadColumns` — AoS-to-SoA quad transpose.

### `varlen`

- `trait Offset` — offset word type for variable-length data.
- `struct Varlen<O: Offset>` — offsets+data variable-length array; `len`,
  `is_empty`, `offsets`, `data`, `row`, `validity`, `offsets_monotonic`.
- `struct VarlenBuilder<O: Offset>` — `new`, `push(Option<&[u8]>)`, `len`,
  `is_empty`, `finish`.

### `views`

- `MAX_INLINE` — 12-byte inline view threshold.
- `is_inline(len)` — whether a view length is inline.
- `struct ViewVec` — Arrow string/binary view layout; `len`, `is_empty`,
  `views_bytes`, `data`, `row`, `validity`.
- `struct ViewVecBuilder` — `new`, `len`, `is_empty`,
  `push(Option<&[u8]>)`, `finish`.

## `cgap-blake3`

Standalone in-crate BLAKE3 (zero external dependencies). Public surface is
the crate root (`src/lib.rs`); module `multi_chunk` carries the multi-chunk
SIMD paths.

- Constants: `IV`, `MSG_PERMUTATION`, `CHUNK_START`, `CHUNK_END`, `PARENT`,
  `ROOT`.
- `GPU_CROSSOVER_BYTES` — 16 MiB crossover above which GPU dispatch is
  indicated.
- `should_dispatch_gpu(payload_len)` — const crossover predicate.
- `compress_scalar(...)` — scalar compression function.
- `compress(...)` — dispatching compress (scalar vs platform path).
- `hash_chunk(chunk, chunk_counter, is_root)` — hash one chunk to a
  16-word state.
- `hash(data)` — BLAKE3 digest (`[u8; 32]`) of a byte slice.
- `is_small_batch_eligible(len)` — const predicate for the small-batch SIMD
  path.
- Modules `neon_parent`, `avx2_parent` — platform SIMD parent-block
  compression.
- `reduce_level_simd(cvs, cv_count)` — fold one CV level via SIMD parent
  compression.
- `hash_wide_simd(data)`, `hash_many_simd(data)`, `hash_many_simd_x2(data)`
  — SIMD tree/batch hashing entry points to a 32-byte digest.
- `multi_chunk::compress_chunks_4way(data, base_chunk_idx)` — compress four
  chunks in parallel (SIMD 4-way).
- `multi_chunk::compress_chunks_4way_x2(...)` — two independent 4-way
  batches.

## `cgap-git`

Git Packfile v2 writer/reader for snapshot chunks (FR-04); object bodies are
zlib streams (in-crate DEFLATE), no deltas, no new type codes; SHA-1, CRC32
and DEFLATE are in-crate, zero external dependencies.

### Modules

`deflate`, `pack_write`, `idx_read`, `ladder`, `inflate`.

### Root items

- `type Oid = [u8; 20]` — 20-byte SHA-1 object name.
- `enum GitError` — pack/idx writing and parsing errors:
  `TooManyObjects(usize)` (exceeds u32 field or 2^16 ladder-window cap),
  `PackChecksumMismatch`, `IdxChecksumMismatch`, `BadMagic`, `Truncated`,
  `BadVarint`, `SizeOverflow`, `OffsetTooLarge`; each variant carries a
  diagnostic `Display` with remediation text.
- Re-exports: `write_idx_v2`, `write_pack_v2`, `IdxEntry`, `PackBuilder`
  from `pack_write`.

### `deflate`

- `struct Adler32` — streaming Adler-32; `new`, `update`, `finalize`.
- `adler32(data)` — one-shot Adler-32.
- `deflate_fixed(data)` — DEFLATE (fixed-Huffman) stream for a byte slice.
- `zlib_compress(data)` — zlib-wrapped compression.
- `inflate(data)` — DEFLATE inflate, returns bytes plus consumed-byte
  count.
- `zlib_decompress(data)` — zlib-wrapped decompression.

### `pack_write`

- `struct Sha1` — streaming SHA-1; `new`, `update`, `finalize`.
- `sha1(data)` — one-shot SHA-1.
- `git_object_oid(kind, content)` — OID of a git object (`"<type> <len>\0"`
  header).
- `struct Crc32` — streaming CRC32; `new`, `update`, `finalize`.
- `crc32(data)` — one-shot CRC32.
- `encode_object_header(obj_type, size)` — pack-v2 varint object header.
- `struct IdxEntry` — one idx-v2 record (OID, CRC, offset).
- `MAX_OBJECTS` — 65,536 (2^16 ladder-window cap).
- `write_pack_v2(blobs)` — pack-v2 bytes for the given object bodies.
- `write_idx_v2(...)` — idx-v2 bytes for OID/CRC/offset tuples.
- `build_pack_and_idx(...)` — combined pack+idx generation.
- `struct PackBuilder` — `new`, `push(blob)`, `len`, `is_empty`,
  `finish` (pack + idx bytes).
- `struct ParsedObject` — parsed pack record (type, offset, body).
- `parse_pack_v2(pack)` — parse a pack-v2 into `ParsedObject`s.
- `parse_idx_v2(...)` — parse idx-v2 contents.

### `idx_read`

- `struct IdxV2<'a>` — zero-copy view over idx-v2 bytes; `parse`, `len`,
  `is_empty`, `fanout`, `oid`, `crc`, `offset`, `lookup` (binary search,
  ladder compare), `verify_idx_checksum`.

### `ladder`

Constant-time OID ordering/ladder search over SHA-1 names.

- `LADDER_STAGES` — 16; `LADDER_WINDOW` — 2^16.
- `debug_probes()`, `debug_tick()` — probe counters (debug instrumentation).
- `oid_lt_eq(a, t)` — constant-time OID ≤ compare, returns
  (lt, eq) mask pair.
- `oid_lt_mask(a, t)`, `ct_eq_oid(a, t)` — constant-time less-than and
  equality masks.
- `ladder_search(oids, t)` — constant-time binary search over a sorted OID
  table; returns (index, eq-mask).

### `inflate`

Read-side RFC 1951/1950 with its own typed error surface.

- `enum InflateError` — typed inflate/zlib decode errors.
- `inflate(data)` — RFC 1951 inflate.
- `adler32(data)` — one-shot Adler-32 (read-side copy).
- `zlib_decode(data)` — RFC 1950 zlib decode.

## `cgap-parquet`

Parquet persistence kernels (charter layer 3): vectorized bit-unpacking,
SWAR RLE/bit-packed hybrid decode, direct page-to-buffer decode; fail-closed
on malformed pages (bounded slice arithmetic + typed error, never a panic on
external input).

### Modules

`bitslice`, `byteslice`, `compiled_manifest`, `dictionary`,
`dispatch_table`, `encode_fast`, `ffor`, `level_free`, `oblivious`,
`packed_filter`, `packing`, `page`, `page_aligned`, `rle`, `scan`
(`std` feature), `stream_compact`, `thrift`, `unpack`, `unpack_filter`.

### Root re-exports

`rle_encode`, `rle_decode`, `ParquetError` from `rle`.

### `bitslice`

Vertical (bit-sliced) layout kernels over `u32` values packed into `u64`
words.

- `pack_vertical(values, w)` — pack into bit-sliced vertical layout.
- `scan_range_vertical(src, w, lo, hi, out_valid)` — range predicate
  producing a validity mask.
- `scan_range_vertical_counted(...)` — range scan returning mask + count.
- `scan_range_vertical_128(...)` — 128-value-per-iteration variant.
- `sum_masked_vertical_128(...)` — masked vertical sum.

### `byteslice`

Struct-of-array byte-slice layout for fixed-width `u32`.

- `byteslice_columns(k)` — column count for byte width k.
- `pack_byteslice(values, k)` — pack into k byte-slice columns.
- `scan_eq_byteslice(packed, k, needle, out_valid)` — equality scan
  producing a mask.
- `point_get(packed, k, i)` — branch-free point read.

### `compiled_manifest`

- `enum Enc` — encoding selector.
- `type Kernel = fn(&[u8], &mut [u32]) -> Result<usize, ParquetError>` —
  decode-kernel function pointer.
- `struct Spec` — one manifest entry (width, encoding, kernel).
- `rle_w::<W>`, `plain_w::<W>` — const-width RLE and plain decode kernels.
- `plain_encode::<W>(lanes)` — const-width plain encode.
- `dispatch(width, encoding)` — kernel lookup by (width, encoding).
- `SPECIALIZATION_MANIFEST` — static table of generated `Spec`s (mirrored
  by generated `generated_compiled_manifest`).

### `dictionary`

- `required_bits(dict_len)` — bits needed for a dictionary length.
- `struct DictionaryBuilder` — `new`, `push(u32)`, `len`, `is_empty`,
  `dict_len`, `finish` (dict + indices).
- `encode_page(dict, indices, bits)` — RLE/bit-packed index page bytes.
- `decode_page(src, dict, out)` — decode an index page through the
  dictionary.

### `dispatch_table`

- `type KernelDecodeFn = unsafe fn(...)` — raw decode kernel signature.
- `struct DispatchTable` — table of kernel function pointers.
- `dispatch_decode(type_token, src, out)` — token-dispatched raw decode.

### `encode_fast`

- `rle_encode_fast(values, bits)` — fast-path RLE/bit-packed hybrid
  encoder.

### `ffor`

Frame-of-reference (FOR) packing/unpacking.

- `ffor_unpack(src, w, base, out)` — unpack FOR-packed `u32` values.
- `ffor_pack(values, w)` — pack with the frame base; returns base + bytes.

### `level_free`

Level-free (flat, REQUIRED-only) Parquet schema/page IO.

- Modules `ptype` (Parquet physical type codes) and `repetition` (REQUIRED,
  OPTIONAL, REPEATED).
- `enum LevelFreeType` — admitted physical types; `width`, `code`.
- `enum LevelFreeError` — typed schema/page errors.
- `struct LevelFreeColumn` — column descriptor (type, name, etc.).
- `struct WireSchemaElement` — raw wire schema element.
- `write_schema_elements`, `write_root_element`, `write_schema_list` —
  ThriftCompact schema emission.
- `parse_schema_elements(src)` — parse wire schema elements.
- `admit_schema(elements)` — admit (or typed-refuse) a schema into
  `LevelFreeColumn`s.
- `align_rows_256b(rows, width)` — row count padded to 256-byte alignment.
- `write_page(...)`, `write_row_group_page(...)`, `write_footer(...)`,
  `write_file(...)` — page/row-group/footer/file emission.
- `read_page_payload(page, ty)` — page payload extraction with bounds
  checking.

### `oblivious`

- `OBLIVIOUS_GATHER_MAX_DICT` — 64-entry dictionary gather cap.
- `oblivious_gather_small(...)` — oblivious (access-pattern-independent)
  dictionary gather.
- `oblivious_masked_compact(values, valid, out)` — oblivious masked
  compaction, returns output length.

### `packed_filter`

- `packed_rows(src_len, w)`, `packed_filter_words(src_len, w)` — layout
  arithmetic for packed filtering.
- `filter_eq_packed(src, w, needle, out_valid)` — equality filter over
  bit-packed data producing a mask.

### `page`

- `write_data_page_v2(values, bits, num_rows)` — data-page-v2 bytes.
- `decode_page_into(page, out)` — decode a data page into a `u32` buffer,
  returning row count.

### `page_aligned`

- `write_plain_page(values, num_rows)` — plain-encoded page bytes.
- `aligned_decode(page, out)` — aligned page decode, returns row count.

### `packing`

- `pack_bits(values, w)` — bit-pack `u32` values at width w.
- `unpack_bits_tight(bytes, w, out)` — tight bit-unpack, fail-closed.

### `rle`

- `enum ParquetError` — typed errors for RLE/page decode.
- `write_varint(dst, value)` — u64 varint emission.
- `rle_encode(values, bits)` — RLE/bit-packed hybrid encode.
- `rle_decode(src, out)` — hybrid decode into a `u32` buffer.
- `decode_into(src, out)` — decode, returning values written.

### `scan` (`std` feature)

Hosted-target reader: mmap + footer parse + point access.

- `struct Mmap` — read-only file mapping; `open`, `as_slice`.
- `enum ScanError` — typed reader errors.
- `enum PhysType` — physical types; `width`.
- `struct ColumnDesc`, `struct RowGroupEntry`, `struct FileMeta` — footer
  metadata records.
- `parse_footer(meta)` — footer metadata parse.
- `struct ParquetFile` — opened file; `open`, `meta`, `num_rows`,
  `row_groups`, `column_bytes`, `value_u32`, `value_u64`, `value_flba`.
- `struct FixtureColumn`, `write_fixture_file(columns, num_rows)` — test
  fixture emission.
- Field-ID constants (`FMD_*`, `SE_*`, `RG_*`, `CMD_*`) — ThriftCompact
  field IDs for footer structures.

### `stream_compact`

- `compact_lanes_branchless(...)` — branch-free multi-lane compaction.

### `thrift`

ThriftCompact protocol primitives.

- `enum ThriftError` — typed Thrift decode errors.
- `ctype` module — compact protocol type codes (STOP, BOOLEAN_TRUE, ...,
  STRUCT).
- `read_varint`, zigzag encode/decode pairs for i16/i32/i64, field-header
  writers (`write_field_header`, `write_bool_field`, `write_i32_field`,
  `write_i64_field`, `write_binary_field`), readers (`read_field_header`,
  `read_i32`, `read_i64`, `read_byte`, `read_binary`).

### `unpack`

- `unpack_1bit`, `unpack_2bit`, `unpack_4bit`, `unpack_8bit` — fixed-width
  bit-unpack kernels into `u64` words.
- `unpack_bits(src, w, out)` — width-parameterized bit-unpack.

### `unpack_filter`

- `unpack_4bit_filtered(src, needle, out, valid)` — 4-bit-wide unpack with
  equality predication.
- `unpack_8bit_filtered(src, needle, out, valid)` — 8-bit-wide unpack with
  equality predication.
