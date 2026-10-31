# Error Path Coverage (81)

Wave-14 lane 16: error-path coverage sweep over the storage plane's typed
error enums. Every variant is constructed and asserted at least once from
OUTSIDE the owning crates, in `crates/integration/tests/error_paths.rs`
(8 courts, one per family). Pre-existing per-variant coverage was found by
grep before writing anything; the matrix cites the existing test names.

Court command + exit (this session): `cargo test -p avec-integration
--test error_paths` -> `8 passed; 0 failed; 0 ignored` (exit 0).
Whole package: `cargo test -p avec-integration` -> 20 passed / 0 failed
across admission_e2e (2), cross_crate (6), error_paths (8), membranes (4).
`cargo clippy -p avec-integration --all-targets` -> zero warnings from
error_paths.rs (pre-existing avec-arrow lib warnings unchanged).

## Coverage Matrix

Status vocabulary: COVERED = an existing test constructed the variant
before this lane; NEWLY-COVERED = this lane's court is the first
constructing test; UNCOVERED = no constructing test exists and none can
(dead variant).

Witness abbreviations (all `#[cfg(test)]` modules or tests/ files;
exact test names are quoted as written):

- pw = crates/avec-git/src/pack_write.rs (tests)
- ir = crates/avec-git/src/idx_read.rs (tests)
- df = crates/avec-git/src/deflate.rs
- sc = crates/avec-parquet/src/scan.rs (tests)
- lf = crates/avec-parquet/src/level_free.rs (tests)
- inf = crates/avec-git/src/inflate.rs (tests)
- mc = crates/avec-parquet/tests/level_free_multi_court.rs
- fz = crates/avec-git/tests/fuzz_codec.rs
- ac = crates/audit/src/alloc_counter.rs (tests)
- ep = crates/integration/tests/error_paths.rs (this lane)

### GitError (8 variants; avec-git lib.rs)

| Variant | Existing witness | Status |
|---|---|---|
| TooManyObjects | pw `object_count_cap_enforced`; ir `oversized_partition...` | COVERED |
| PackChecksumMismatch | pw `corrupted_pack_trailer_fails_checksum` | COVERED |
| IdxChecksumMismatch | pw `corrupted_idx_trailer_fails_checksum` | COVERED |
| BadMagic | pw `bad_magic_rejected`; ir `parse_rejects_bad_magic...` | COVERED |
| Truncated | ir `parse_rejects_bad_magic_and_truncation` | COVERED |
| BadVarint | none | NEWLY-COVERED |
| SizeOverflow | none | NEWLY-COVERED |
| OffsetTooLarge | none | NEWLY-COVERED |

Newly constructed by lane 16 (tests `git_pack_reader_error_paths`,
`git_idx_and_writer_error_paths`):

- BadVarint: reserved varint type code 5 (0x50) behind a valid pack
  SHA-1; plus the FDICT zlib refusal (df:547).
- SizeOverflow: declared size 2^32 via encode_object_header behind a
  valid SHA-1.
- OffsetTooLarge: offset 2^31 through write_idx_v2.

### ScanError (11 variants; scan.rs)

| Variant | Existing witness | Status |
|---|---|---|
| Io | none | NEWLY-COVERED |
| EmptyFile | none | NEWLY-COVERED |
| MalformedFooter | sc `corrupt_offset_refused` | COVERED |
| FooterOverflow | none | NEWLY-COVERED |
| UnsupportedSchema | sc `optional_column_refused` | COVERED |
| SchemaOverflow | none | NEWLY-COVERED |
| LevelsPresent | sc `nonzero_def_level_refused` | COVERED |
| UnsupportedPage | none | NEWLY-COVERED |
| OffsetMismatch | sc `corrupt_offset_refused` (private page_slice) | COVERED |
| Thrift | none | NEWLY-COVERED |
| Page | none | UNCOVERED |

Newly constructed by lane 16 (tests `scan_open_error_paths`,
`scan_page_error_paths`):

- Io: nonexistent path; EmptyFile: zero-byte file.
- FooterOverflow: zeroed footer length prefix.
- SchemaOverflow: FLBA type_length = -1.
- UnsupportedPage: is_compressed flag poison.
- Thrift: garbage page header -> Thrift(MissingRequiredField(1)).
- Page: DEAD VARIANT - zero construction sites in avec-parquet (only its
  Display arm, scan.rs:228). Constructed directly; Display + equality
  pinned.

### LevelFreeError (8 variants; level_free.rs)

| Variant | Existing witness | Status |
|---|---|---|
| OptionalField | lf `refuses_optional_field`; mc `optional_field_refused` | COVERED |
| RepeatedField | lf `refuses_repeated_field` | COVERED |
| NestedField | lf `refuses_nested_field` | COVERED |
| UnsupportedType | lf `refuses_unsupported_type_and_width` | COVERED |
| UnsupportedWidth | lf `refuses_unsupported_type_and_width` | COVERED |
| UnknownRepetition | none | NEWLY-COVERED |
| UndecodablePage | none | NEWLY-COVERED |
| PayloadMismatch | lf `payload_length_mismatch...`; mc `wrong_payload_length_refused` | COVERED |

Newly constructed by lane 16 (test `level_free_error_paths`):

- UnknownRepetition: repetition code 3.
- UndecodablePage: garbage page fails the header parse-back.

### InflateError (9 variants; inflate.rs)

| Variant | Existing witness | Status |
|---|---|---|
| Truncated | inf `truncated_stream_typed_error` | COVERED |
| BadZlibHeader | inf `bad_zlib_header_typed_error`; fz `court_nonvacuity...` | COVERED |
| BadBlockType | inf `bad_block_type_typed_error` | COVERED |
| BadCodeLengths | inf `corrupt_stored_block_len_check` | COVERED |
| BadCodeLengthRepeat | none | NEWLY-COVERED |
| MissingEndOfBlock | none | NEWLY-COVERED |
| OverlongBackreference | inf `overlong_backreference_typed_error` | COVERED |
| BadDistanceCode | none | UNCOVERED |
| AdlerMismatch | inf `bad_adler_typed_error` | COVERED |

Newly constructed by lane 16 (test `inflate_dynamic_header_error_paths`):

- BadCodeLengthRepeat: bit-built dynamic header whose only code-length
  symbol is 16 (repeat with no prior length).
- MissingEndOfBlock: bit-built dynamic header with an all-zero length
  list (symbol 256 gets length 0).
- BadDistanceCode: DEAD BRANCH - HDIST is capped at 30 by dynamic_trees
  and the fixed distance tree emits 30 5-bit codes (symbols 0..=29), so
  `dsym >= 30` never holds. Constructed directly; Display pinned.

### AllocReport (alloc_counter.rs)

| Surface | Existing witness | Status |
|---|---|---|
| with_alloc_tracking + report fields | ac inline tests (5); avec-ffi alloc_courts.rs | COVERED |
| is_zero() | ac `stack_only_closure_allocates_nothing` | COVERED |
| Error-path tie-in (zero-alloc typed refusal) | none | NEWLY-COVERED |

Newly constructed by lane 16 (test `alloc_report_scopes_and_error_path`):

- Error-path tie-in: a parse_pack_v2 checksum refusal under
  with_alloc_tracking performs zero allocations (the refusal fires before
  the object table is built; SHA-1 stays on the stack).

## Matrix Totals

- 36 variant rows: 21 COVERED (pre-existing), 13 NEWLY-COVERED,
  2 UNCOVERED (dead variants: ScanError::Page and
  InflateError::BadDistanceCode).
- AllocReport: covered pre-lane (5 inline tests + avec-ffi alloc_courts);
  this lane added the zero-alloc-error-path tie-in court.

## Findings (for the coordinator)

1. Two dead error variants. ScanError::Page(ParquetError) has zero
   construction sites in avec-parquet (only its Display arm, scan.rs:228).
   InflateError::BadDistanceCode is unreachable: dynamic_trees caps HDIST
   at 30 and the fixed distance tree emits 30 5-bit codes (symbols
   0..=29), so the dsym >= 30 guard never holds. Both are constructed
   directly in the court (Display/equality pinned); candidates for
   deletion (ERRC: delete outranks generate) or for explicit wiring.
2. crates/avec-git/tests/fuzz_codec.rs buckets GitError/InflateError
   variants but deterministically constructs only BadZlibHeader (its
   nonvacuity court). The BadVarint / SizeOverflow / OffsetTooLarge /
   BadCodeLengthRepeat / MissingEndOfBlock / BadDistanceCode bucket arms
   exist without any witnessed firing. Bucketing is not construction.
3. crates/avec-parquet/tests/level_free_multi_court.rs is feature-gated:
   bare `cargo test -p avec-parquet` runs 0 of its tests; the verify gate
   (verify_v26.10.31.sh:96) runs `--features std`, where all 4 are live.
   Not dead in CI, but a bare run silently drops the court.
4. ScanError::Io and EmptyFile - the scanner's two simplest refusals -
   had zero coverage before this lane.

## Receipt

- New files: crates/integration/tests/error_paths.rs (8 courts) and this
  doc. Edited: crates/integration/Cargo.toml (the allowed file): added
  the avec-audit dev-dep (was missing) and features = ["std"] on
  avec-parquet (required: the scan module is feature-gated; without it no
  ScanError court compiles). Both changes disclosed per the compile-freeze
  disclosure law.
- Full package run receipt: `cargo test -p avec-integration` -> 20 passed
  / 0 failed (admission_e2e 2, cross_crate 6, error_paths 8, membranes 4);
  clippy clean for the new file.
