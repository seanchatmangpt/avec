# Tutorial: consuming cgap through the C Data Interface

This tutorial walks you through consuming the `cgap-ffi` cdylib from a
foreign language (Python via ctypes) and importing an Arrow batch through
the Arrow C Data Interface — the same path the repo's own wire-parity
court exercises.

What you will do:

1. Build the dylib.
2. dlopen it from Python and call the version symbol.
3. Export a batch and read it through the C Data Interface structs.

Everything here is transcribed from a run verified on this checkout.

## Prerequisites

- A checkout of `cgap` at the repo root (all paths below are relative to
  it).
- Python 3 with `ctypes` in the standard library. Steps 1-2 need nothing
  beyond the standard library; step 3 needs `pyarrow`.

## Step 1: build the dylib

```console
$ cargo build -p cgap-ffi
   Compiling cgap-ffi v26.10.29 (/Users/sac/cgap/crates/cgap-ffi)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.15s
```

Exit 0. The artifact lands at `target/debug/libcgap_ffi.dylib` on macOS
(`libcgap_ffi.so` on Linux).

## Step 2: dlopen and call the version symbol

The library exports `cgap_version()`, a NUL-terminated C string. A
minimal ctypes consumer:

```python
# version_probe.py — run from the repo root
import ctypes

lib = ctypes.CDLL("target/debug/libcgap_ffi.dylib")
lib.cgap_version.restype = ctypes.c_char_p
print(lib.cgap_version().decode())
```

Run it:

```console
$ python3 version_probe.py
26.10.29
```

Exit 0. This is the smallest possible foreign-language consumer: dlopen,
declare one restype, call one symbol. Every later step is the same shape,
just with more structs.

## Step 3: export a batch and import it

The full court is `scripts/gate3_cdata.py` — this tutorial's consumer is
modeled on that file (and the schema-parity sibling,
`scripts/gate3_schema.py`). The pattern:

```python
import ctypes

import pyarrow as pa

LIB = ctypes.CDLL("target/debug/libcgap_ffi.dylib")
for name, restype in [
    ("cgap_array_addr", ctypes.c_size_t),
    ("cgap_schema_addr", ctypes.c_size_t),
    ("cgap_batch_rows", ctypes.c_int64),
]:
    getattr(LIB, name).restype = restype

N = 100_000
NULL_EVERY = 7

LIB.cgap_i32_batch(N, NULL_EVERY)
array_addr = LIB.cgap_array_addr()
schema_addr = LIB.cgap_schema_addr()
rows = LIB.cgap_batch_rows()
assert rows == N

arr = pa.Array._import_from_c(array_addr, schema_addr)
print("imported:", arr.type, len(arr), "nulls:", arr.null_count)
```

Expected output:

```console
imported: int32 100000 nulls: 14285
```

## The export lifecycle

There is no `get_schema`/`get_next`/`release` symbol triple in cgap-ffi.
The lifecycle is a slot-based variant of the same Arrow C Data Interface
contract, implemented in `crates/cgap-ffi/src/lib.rs` and driven by the
court `scripts/gate3_cdata.py`:

- **Export (`get_schema` analogue)**: `cgap_i32_batch(n, null_every)`
  builds the batch, then `export()` produces an `ArrowSchema`/`ArrowArray`
  pair. Both are `Box::leak`-stored into static slots
  (`ARRAY_SLOT`/`SCHEMA_SLOT`), and the caller reads their addresses via
  `cgap_schema_addr()` / `cgap_array_addr()`. `cgap_batch_rows()` returns
  the length of the last exported batch. The schema is exported once per
  batch, not per column — a single `i\0` format string for the primitive
  i32 type.
- **Consume (`get_next` analogue)**: the foreign consumer reads the two
  addresses and imports one batch at a time. In `gate3_cdata.py` the
  import is `pa.Array._import_from_c(array_addr, schema_addr)`, followed
  by asserts on type, length, null count, and bitwise buffer parity
  against Python-recomputed ground truth. There is one batch per export
  call, so a stream is N exports rather than a cursor.
- **Release**: per the C Data Interface contract, release ownership
  passes to the importer. `crates/cgap-ffi/src/lib.rs` documents this on
  `cgap_i32_batch`: "Returns the live ArrowArray pointer; the CALLER
  (importer) takes release ownership per the C Data Interface contract."
  In the gate3 court, pyarrow's `_import_from_c` performs the release
  through the array's release callback when the imported array is
  dropped; the leaked slot boxes are intentionally not freed by the
  Rust side (the export is one-shot per process run, matching the
  court's lifetime).

For a streaming producer the symbol triple would be replaced by an
`ArrowArrayStream`; cgap-ffi deliberately exports one batch per call and
lets the court prove wire parity, not stream throughput.

## Where to go next

- The full wire-parity court: `scripts/gate3_cdata.py` (bitwise buffer
  assertions).
- Schema parity: `scripts/gate3_schema.py` (extends gate3 from wire
  parity to schema parity through pyarrow).
- The FFI surface: `crates/cgap-ffi/src/lib.rs` (export slots, version,
  digest of the live export).
- Run the whole gate ladder: `make gates` (gate0 through gate3).
