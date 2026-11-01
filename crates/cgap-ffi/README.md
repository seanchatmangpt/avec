# cgap-ffi

**Not published to crates.io** (`publish = false`). This crate is the
in-repo C Data Interface cdylib membrane for cgap — consumers load the built
`.dylib`/`.so` directly; there is no registry artifact.

## Explanation: what the membrane is

`cgap-ffi` is a `cdylib` that exports cgap data as raw
[Arrow C Data Interface](https://arrow.apache.org/docs/format/CDataInterface.html)
structs (`ArrowArray` / `ArrowSchema`) plus version symbols, so a foreign
consumer (PyArrow, DuckDB, Polars, a BEAM NIF) can import cgap batches with
zero copies across the boundary. It is the Gate 3 wire-parity surface: the
proof that cgap's in-memory layout is bitwise the layout foreign Arrow
runtimes expect.

The export path: `cgap_i32_batch(n, null_every)` builds a primitive i32
batch (with a periodic null every `null_every` rows), exports it through
`cgap_arrow::c_abi::export`, and leaks the `ArrowArray`/`ArrowSchema` into
process-lifetime static slots. The importer then reads the slot addresses
(`cgap_array_addr` / `cgap_schema_addr`) and the row count
(`cgap_batch_rows`), and imports through its own C Data Interface entry
point (e.g. pyarrow's `_import_from_c`). Per the C Data Interface contract,
the CALLER (importer) takes release ownership; there is no cgap-side free
for the static slots.

The wire-parity court is `scripts/gate3_cdata.py` (see Tutorials below);
schema parity is extended by `scripts/gate3_schema.py`.

## How-to

### Build the cdylib

From the repo root:

```sh
cargo build -p cgap-ffi
# produces target/debug/libcgap_ffi.dylib (macOS) or .so (Linux)
```

### dlopen it from Python

`scripts/gate3_cdata.py` is itself a working dlopen example:

```python
import ctypes
LIB = ctypes.CDLL("target/debug/libcgap_ffi.dylib")
```

### dlopen it from C

There is no C example in the repo, so only the grounded shape: the library
exposes plain C symbols; a C consumer links against the same cdylib and
declares the Arrow C Data Interface structs and the `cgap_*` symbols
(`nm -gU target/debug/libcgap_ffi.dylib` lists them after a build).

## Reference: exported symbols

Source of truth for the list below: `#[no_mangle]` exports in
`crates/cgap-ffi/src/lib.rs` (the cdylib artifact was not built at time of
writing, so `nm -gU` was not run).

| Symbol | Signature |
|---|---|
| `cgap_i32_batch` | `fn(n: i64, null_every: i64) -> *mut ArrowArray` |
| `cgap_schema_addr` | `fn() -> usize` |
| `cgap_array_addr` | `fn() -> usize` |
| `cgap_batch_rows` | `fn() -> i64` |
| `cgap_version` | `fn() -> *const c_char` |
| `cgap_digest_of` | `fn(array_addr: usize) -> u64` |

Symbol notes:

- `cgap_i32_batch`: builds and exports an i32 batch with a null every
  `null_every` rows (`0` = all valid); returns the live array pointer.
- `cgap_schema_addr` / `cgap_array_addr`: addresses of the exported
  schema/array slots.
- `cgap_batch_rows`: length of the last exported batch.
- `cgap_version`: NUL-terminated version string (drift-guarded via
  `version_cstr_parity`).
- `cgap_digest_of`: mutation-detection digest of the live exported array.
  Rotate-xor fold, byte-for-byte parity with
  `cgap_arrow::digest::digest_of` (pinned by
  `ffi_fold_parity_matches_digest_of`); NOT a cryptographic hash. Panics on
  null address or foreign layout.

## Tutorials

### Run the Python wire-parity check

Requires [uv](https://docs.astral.sh/uv/) and pyarrow.

```sh
cargo build -p cgap-ffi
make gate3   # runs: uv run --with pyarrow python3 scripts/gate3_cdata.py
```

The script (`scripts/gate3_cdata.py`) dlopens `target/debug/
libcgap_ffi.dylib`, exports a 100,000-row i32 batch with a null every 7
rows, imports through `pa.Array._import_from_c`, and asserts bitwise
equality of BOTH buffers (validity bitmap and values) against
Python-recomputed ground truth. `GATE 3 PASS: bitwise parity through C Data
Interface` and exit 0 = wire parity. `scripts/gate3_schema.py` extends the
same cdylib to schema parity; `make gates` runs gate0..gate3 in order.

## Tests in this crate

- `tests/zero_copy.rs` — FR-07 zero-copy integration tests over the C ABI
  surface (pointer identity, alignment, release-path exact-once). Loudly
  documents the FR-07 gaps that live in `crates/cgap-beam`, not here.
- `tests/alloc_courts.rs` — hot-path allocation courts using
  `cgap_audit::alloc_counter::with_alloc_tracking`: zero-alloc asserted on
  `cgap_digest_of`, the release callbacks, and slot reads; the allocating
  export path is scoped outside the tracked window.
- In-crate `digest_ffi` tests — FFI-fold parity with the canon digest and a
  corruption court (flip one byte, digest must change; restore, digest must
  match again).

The stardf consumer court is NOT here: it moved to
`ggen-legacy/stardf/arrow crates/stardf-arrow-capi/tests/cgap_consumer_court.rs`
(path-dev-deps on cgap-ffi/cgap-audit from the stardf side) so cgap carries
no stardf dependency (see the dev-dependencies note in `Cargo.toml`).
