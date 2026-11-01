#!/usr/bin/env python3
"""Gate 3: C Data Interface wire-parity harness (pyarrow _import_from_c).

Loads the cgap-ffi cdylib, exports an i32 batch with nulls, imports through
pyarrow's C Data Interface entry point, and asserts BITWISE equality of both
buffers against python-recomputed ground truth. Exit 0 = wire parity.
"""
import ctypes
import sys

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
assert rows == N, f"rows mismatch: {rows}"
assert array_addr and schema_addr, "null export slots"

arr = pa.Array._import_from_c(array_addr, schema_addr)
assert arr.type == pa.int32(), f"type: {arr.type}"
assert len(arr) == N
assert arr.null_count == N // NULL_EVERY
print("imported:", arr.type, len(arr), "nulls:", arr.null_count)

truth_valid = [i % 7 != 6 for i in range(N)]
bm = bytearray((N + 7) // 8)
for i, v in enumerate(truth_valid):
    if v:
        bm[i // 8] = bm[i // 8] | (1 << (i % 8))
expected_validity = bytes(bm)
expected_values = b"".join(
    ((i if i % 7 != 6 else 0) & 0xFFFFFFFF).to_bytes(4, "little") for i in range(N)
)
vbuf = arr.buffers()[0]
assert vbuf is not None, "validity buffer missing"
gv = bytes(vbuf)
gvv = bytes(arr.buffers()[1])
assert gv[: len(expected_validity)] == expected_validity, "validity bitmap mismatch"
assert gvv[: 4 * N] == expected_values, "values bitwise mismatch"
print("GATE 3 PASS: bitwise parity through C Data Interface")
sys.exit(0)
