"""ctypes binding for the avec-ffi Arrow C Data Interface cdylib.

Runtime dependency-free. pyarrow is a test-only extra: the Arrow C Data
Interface import (``avec_new_i32_batch`` -> ``pa.Array``) imports it lazily
at call time.

Library resolution: ``$AVEC_LIB`` if set, else ``libavec_ffi.dylib``/``.so``
under ``<repo>/target/debug`` resolved relative to this file
(``bindings/python/avec`` -> three levels up is the repo root).
"""

from __future__ import annotations

import ctypes
import os
import sys

__version__ = "26.10.29"

__all__ = [
    "__version__",
    "version",
    "avec_new_i32_batch",
    "load_library",
]

_LIB = None


def _default_library_path() -> str:
    name = (
        "libavec_ffi.dylib"
        if sys.platform == "darwin"
        else "libavec_ffi.so"
    )
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.normpath(
        os.path.join(here, os.pardir, os.pardir, os.pardir, "target", "debug", name)
    )


def load_library():
    """Load (once, cached) and return the avec-ffi cdylib ctypes handle.

    Signatures are configured here so pointer returns are never truncated:
    ``avec_array_addr``/``avec_schema_addr`` return ``usize`` (c_size_t), and
    ``avec_i32_batch`` returns a pointer (c_void_p), which would truncate on
    64-bit under ctypes' default ``c_int`` restype.
    """
    global _LIB
    if _LIB is None:
        path = os.environ.get("AVEC_LIB") or _default_library_path()
        try:
            lib = ctypes.CDLL(path)
        except OSError as exc:
            raise RuntimeError(
                f"avec: cannot load FFI library {path!r}: {exc} "
                "(build the cdylib or set AVEC_LIB)"
            ) from exc
        lib.avec_i32_batch.restype = ctypes.c_void_p
        lib.avec_i32_batch.argtypes = [ctypes.c_int64, ctypes.c_int64]
        lib.avec_array_addr.restype = ctypes.c_size_t
        lib.avec_array_addr.argtypes = []
        lib.avec_schema_addr.restype = ctypes.c_size_t
        lib.avec_schema_addr.argtypes = []
        lib.avec_batch_rows.restype = ctypes.c_int64
        lib.avec_batch_rows.argtypes = []
        lib.avec_version.restype = ctypes.c_char_p
        lib.avec_version.argtypes = []
        _LIB = lib
    return _LIB


def version() -> str:
    """Version string reported by the loaded avec-ffi cdylib."""
    return load_library().avec_version().decode("utf-8")


def avec_new_i32_batch(n: int, null_every: int) -> "pa.Array":
    """Export an i32 batch and import it through pyarrow's C Data Interface.

    Values are ``0..n-1`` with a null wherever ``i % null_every ==
    null_every - 1`` (no nulls when ``null_every == 0``). Raises if the
    export slots are unset or the batch length disagrees with ``n``.
    pyarrow is imported lazily; it is a test-only dependency.
    """
    lib = load_library()
    lib.avec_i32_batch(n, null_every)
    array_addr = lib.avec_array_addr()
    schema_addr = lib.avec_schema_addr()
    rows = lib.avec_batch_rows()
    if rows != n:
        raise RuntimeError(f"avec: batch rows {rows} != requested {n}")
    if not array_addr or not schema_addr:
        raise RuntimeError("avec: null export slots")
    pa = _pyarrow()
    return pa.Array._import_from_c(array_addr, schema_addr)


def _pyarrow():
    try:
        import pyarrow as pa
    except ImportError as exc:  # pragma: no cover - exercised via extra-less env
        raise RuntimeError(
            "avec: pyarrow is required for avec_new_i32_batch "
            "(test extra): pip install 'avec[test]'"
        ) from exc
    return pa
