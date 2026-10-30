#!/usr/bin/env python3
"""Gate 3 (schema level): C Data Interface schema-parity harness.

Extends gate3_cdata.py from wire parity to schema parity. Same cdylib
(target/debug/libavec_ffi.dylib) and same ctypes surface — NO new avec-ffi
exports; all schema-level facts are proven through pyarrow's public API on
the imported array:

  1. type identity:      arr.type == pa.int32() and pa.types.is_integer
  2. null facts:         nullability flag + null counts match ground truth
                         for 3 different (n, null_every) configs
  3. roundtrip:          pa.Array -> pa.Table('avec_col', nullable) ->
                         to_batches -> .equals equivalence, plus a full
                         C-Data re-export -> re-import -> .equals
  4. bitwise buffers:    buffers() byte-equality vs recomputed ground truth,
                         re-run per config
  5. zero-copy slice:    arr.slice(100, 1000) null count + shared buffers +
                         overlapping-window bytes match the pattern

Exit 0 iff every check passes; one PASS line per (check, config).
"""
import ctypes
import sys

import pyarrow as pa

REPO = "/Users/sac/avec"
LIB_PATH = f"{REPO}/target/debug/libavec_ffi.dylib"
LIB = ctypes.CDLL(LIB_PATH)
for name, restype in [
    ("avec_i32_batch", ctypes.c_size_t),  # returns the live ArrowArray pointer
    ("avec_array_addr", ctypes.c_size_t),
    ("avec_schema_addr", ctypes.c_size_t),
    ("avec_batch_rows", ctypes.c_int64),
]:
    getattr(LIB, name).restype = restype

# include/avec_c_data.h struct layouts (spec ABI).
SCHEMA_FLAGS_OFFSET = 24  # format(0) name(8) metadata(16) flags(24)
SCHEMA_FORMAT_OFFSET = 0
SCHEMA_NAME_OFFSET = 8
ARRAY_LENGTH_OFFSET = 0  # length(0) null_count(8) offset(16) n_buffers(24)
ARRAY_NULL_COUNT_OFFSET = 8
ARRAY_OFFSET_OFF = 16
ARRAY_N_BUFFERS_OFFSET = 24
ARROW_FLAG_NULLABLE = 2  # include/avec_c_data.h

# Configs: (n, null_every). Config B exercises the all-valid wire shape
# (null_every <= 0 pushes no nulls; avec-arrow drops the validity buffer).
CONFIGS = [
    (100_000, 7),
    (1_000, 0),
    (65_537, 1_000),
]
SLICE_OFF, SLICE_LEN = 100, 1000


def ground_truth(n, null_every):
    """Pattern from crates/avec-ffi/src/lib.rs:25: null iff i %% ne == ne-1,
    else value i (as i32); null slots carry Scalar::ZERO (deterministic 0)."""
    validity = [null_every <= 0 or i % null_every != null_every - 1 for i in range(n)]
    values = [(i if v else 0) & 0xFFFFFFFF for i, v in enumerate(validity)]
    bm = bytearray((n + 7) // 8)
    for i, v in enumerate(validity):
        if v:
            bm[i // 8] |= 1 << (i % 8)
    return validity, bytes(bm), b"".join(v.to_bytes(4, "little") for v in values)


def struct_i64(addr, off):
    return ctypes.c_int64.from_address(addr + off).value


def c_string(addr, off):
    ptr = ctypes.c_size_t.from_address(addr + off).value
    return None if not ptr else ctypes.string_at(ptr)


def check(name, cond, detail=""):
    if not cond:
        print(f"FAIL {name}: {detail}")
        sys.exit(1)
    print(f"PASS {name}{(' :: ' + detail) if detail else ''}")


def run_config(tag, n, null_every):
    validity, exp_validity, exp_values = ground_truth(n, null_every)
    exp_nulls = validity.count(False)

    LIB.avec_i32_batch(n, null_every)
    array_addr = LIB.avec_array_addr()
    schema_addr = LIB.avec_schema_addr()
    rows = LIB.avec_batch_rows()
    assert rows == n, f"rows mismatch: {rows}"
    assert array_addr and schema_addr, "null export slots"

    # --- schema-level ground truth via the public header layout, read BEFORE
    # import (the C-Data import consumes/releases both structs).
    flags = struct_i64(schema_addr, SCHEMA_FLAGS_OFFSET)
    fmt = c_string(schema_addr, SCHEMA_FORMAT_OFFSET)
    name_ptr = c_string(schema_addr, SCHEMA_NAME_OFFSET)
    w_length = struct_i64(array_addr, ARRAY_LENGTH_OFFSET)
    w_nulls = struct_i64(array_addr, ARRAY_NULL_COUNT_OFFSET)
    w_offset = struct_i64(array_addr, ARRAY_OFFSET_OFF)
    w_nbuf = struct_i64(array_addr, ARRAY_N_BUFFERS_OFFSET)

    arr = pa.Array._import_from_c(array_addr, schema_addr)
    keepalive.append(arr)

    # (1) type identity
    check(
        f"[{tag}] 1 type",
        arr.type == pa.int32() and pa.types.is_integer(arr.type),
        f"wire format {fmt!r} -> {arr.type}; is_integer={pa.types.is_integer(arr.type)}",
    )

    # (2) nullability + null counts vs ground truth
    check(
        f"[{tag}] 2 nulls",
        flags & ARROW_FLAG_NULLABLE
        and w_nulls == exp_nulls
        and arr.null_count == exp_nulls
        and w_length == n
        and w_offset == 0
        and w_nbuf == 2,
        f"flags={flags} (nullable bit set), wire null_count={w_nulls}, "
        f"pyarrow null_count={arr.null_count}, expected={exp_nulls}, "
        f"length={w_length}, offset={w_offset}, n_buffers={w_nbuf}",
    )

    # (3) roundtrip: Array -> Table('avec_col', nullable) -> to_batches ->
    # .equals, then full C-Data re-export -> re-import -> .equals
    schema = pa.schema([pa.field("avec_col", pa.int32(), nullable=True)])
    table = pa.Table.from_arrays([arr], schema=schema)
    f = table.schema.field("avec_col")
    assert f.name == "avec_col" and f.nullable, f"table field: {f}"
    batches = table.to_batches()
    assert sum(len(b) for b in batches) == n and len(batches) >= 1
    col = batches[0].column(0)
    rt_ok = col.equals(arr) and arr.equals(col)
    out_arr = ctypes.c_size_t(0)
    out_schema = ctypes.c_size_t(0)
    col._export_to_c(
        ctypes.addressof(out_arr), ctypes.addressof(out_schema)
    )
    reimported = pa.Array._import_from_c(
        ctypes.addressof(out_arr), ctypes.addressof(out_schema)
    )
    keepalive.append(reimported)
    ri_ok = (
        reimported.equals(arr)
        and arr.equals(reimported)
        and reimported.type == pa.int32()
        and reimported.null_count == exp_nulls
    )
    check(
        f"[{tag}] 3 roundtrip",
        rt_ok and ri_ok,
        f"batches={len(batches)}, to_batches col .equals(arr)={rt_ok}, "
        f"C-Data re-export/re-import .equals={ri_ok} "
        f"(nulls={reimported.null_count})",
    )

    # (4) bitwise buffers per config
    vbuf, dbuf = arr.buffers()
    vbytes = bytes(vbuf) if vbuf is not None else None
    if vbytes is None:
        v_ok = exp_nulls == 0  # all-valid wire shape: validity buffer dropped
    else:
        v_ok = vbytes[: (n + 7) // 8] == exp_validity
    d_ok = dbuf is not None and bytes(dbuf)[: 4 * n] == exp_values
    check(
        f"[{tag}] 4 buffers",
        v_ok and d_ok,
        f"validity={'<dropped>' if vbytes is None else f'{len(vbytes)}B'} "
        f"bitwise-{'eq' if v_ok else 'NE'}, values {4 * n}B bitwise-"
        f"{'eq' if d_ok else 'NE'}",
    )

    # (5) zero-copy slice: shared buffers + window ground truth
    assert n > SLICE_OFF, f"config too small to slice: n={n}"
    w_len = min(SLICE_LEN, n - SLICE_OFF)  # pyarrow clamps the window to n
    sliced = arr.slice(SLICE_OFF, SLICE_LEN)
    w_validity = validity[SLICE_OFF : SLICE_OFF + w_len]
    w_nulls_expected = w_validity.count(False)
    w_exp_values = b"".join(
        ((i if v else 0) & 0xFFFFFFFF).to_bytes(4, "little")
        for i, v in enumerate(w_validity, start=SLICE_OFF)
    )
    sv, sd = sliced.buffers()
    pv, pd = arr.buffers()
    # What pyarrow exposes: slices share the UNTRIMMED parent buffers (same
    # base address + size; different Python Buffer objects); the window is
    # carried as Array.offset, not by trimming the buffers.
    def shared(a, b):
        if a is None or b is None:
            return a is None and b is None
        return a.address == b.address and a.size == b.size

    shared_ok = shared(sv, pv) and shared(sd, pd)
    # Overlapping-region byte comparison (BufferView semantics): the slice's
    # logical window is parent bytes [offset*w, (offset+len)*w); verify the
    # window content equals the recomputed pattern so sharing is not vacuous.
    region_ok = bytes(sd)[
        sliced.offset * 4 : (sliced.offset + w_len) * 4
    ] == w_exp_values
    bits_ok = all(
        (bytes(sv)[(SLICE_OFF + j) >> 3] >> ((SLICE_OFF + j) & 7)) & 1 == (1 if v else 0)
        for j, v in enumerate(w_validity)
    ) if sv is not None else (w_nulls_expected == 0)
    slice_nulls_ok = sliced.null_count == w_nulls_expected
    check(
        f"[{tag}] 5 slice",
        len(sliced) == w_len
        and slice_nulls_ok
        and shared_ok
        and region_ok
        and bits_ok,
        f"slice({SLICE_OFF},{min(SLICE_LEN, n - SLICE_OFF)}): "
        f"offset={sliced.offset}, nulls={sliced.null_count}"
        f"=={w_nulls_expected}, buffers shared (addr+size)={shared_ok}, "
        f"window bytes bitwise-eq={region_ok}, window bits eq={bits_ok}",
    )


keepalive = []
print(f"gate3_schema: lib={LIB_PATH} pyarrow={pa.__version__}")
for idx, (n, ne) in enumerate(CONFIGS):
    run_config(f"cfg{idx} n={n} null_every={ne}", n, ne)
print("GATE 3 SCHEMA PASS: all checks green across 3 configs")
sys.exit(0)
