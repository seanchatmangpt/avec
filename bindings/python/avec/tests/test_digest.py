"""Consumer-side digest tests: avec_digest_of through the ctypes loader.

Drives the avec-ffi mutation-detection digest exactly as a foreign consumer
would: export via the static slots (avec_i32_batch / avec_array_addr), read
the ArrowArray struct fields through a ctypes mirror, fold via
avec_digest_of, flip one value byte with a real memory write, and require
the digest to move and to restore. Real collaborators only — the cdylib is
loaded for real; no fakes.

The signature is configured TEST-LOCALLY off the shared LIB object (the
loader module is untouched): avec_digest_of returns u64 and takes usize.
"""

import ctypes

import pytest

import avec

VERSION_LAW = "26.10.29"

LIB = avec.load_library()
LIB.avec_digest_of.restype = ctypes.c_uint64
LIB.avec_digest_of.argtypes = [ctypes.c_size_t]


class _ArrowArray(ctypes.Structure):
    """C Data Interface ArrowArray mirror, through the `buffers` field."""

    _fields_ = [
        ("length", ctypes.c_int64),
        ("null_count", ctypes.c_int64),
        ("offset", ctypes.c_int64),
        ("n_buffers", ctypes.c_int64),
        ("n_children", ctypes.c_int64),
        ("buffers", ctypes.POINTER(ctypes.c_void_p)),
    ]


def _export(n: int, null_every: int) -> tuple[int, _ArrowArray]:
    """Export a batch through the real slots; return (array_addr, mirror)."""
    LIB.avec_i32_batch(n, null_every)
    addr = LIB.avec_array_addr()
    assert addr, "avec: null export slot"
    rows = LIB.avec_batch_rows()
    assert rows == n, f"avec: batch rows {rows} != requested {n}"
    return addr, _ArrowArray.from_address(addr)


def _values_ptr(mirror: _ArrowArray) -> int:
    assert mirror.n_buffers == 2, f"expected n_buffers == 2, got {mirror.n_buffers}"
    ptr = mirror.buffers[1]
    assert ptr, "values buffer missing"
    return ptr


def test_version_law():
    assert avec.__version__ == VERSION_LAW
    assert avec.version() == VERSION_LAW


def test_digest_stable_across_two_calls():
    addr, mirror = _export(1000, 7)
    assert mirror.length == 1000
    assert mirror.null_count == 1000 // 7
    d1 = LIB.avec_digest_of(addr)
    d2 = LIB.avec_digest_of(addr)
    assert d1 == d2, "digest must be deterministic across calls"
    assert d1 != 0, "a 1000-row batch must not fold to the empty digest"


def test_digest_changes_for_different_batch():
    addr_a, _ = _export(512, 8)
    d_a = LIB.avec_digest_of(addr_a)
    addr_b, _ = _export(512, 5)  # same length, different content
    d_b = LIB.avec_digest_of(addr_b)
    assert d_a != d_b, "different batch content must change the digest"

    # Re-export the SAME content: the digest must match the first export
    # (deterministic function of the physical buffers, not of the address).
    addr_a2, _ = _export(512, 8)
    assert LIB.avec_digest_of(addr_a2) == d_a


def test_corruption_court_one_value_byte_flip():
    """Digest -> flip ONE value byte (real memory write) -> digest MUST
    differ -> restore the captured byte -> digest MUST match again."""
    addr, mirror = _export(64, 3)  # row 0 valid, value 0
    d1 = LIB.avec_digest_of(addr)

    raw = ctypes.cast(_values_ptr(mirror), ctypes.POINTER(ctypes.c_uint8))
    original = raw[0]
    raw[0] = original ^ 0xFF

    d2 = LIB.avec_digest_of(addr)
    assert d2 != d1, "single value-byte flip must change the digest"

    raw[0] = original
    d3 = LIB.avec_digest_of(addr)
    assert d3 == d1, "restored buffer must fold to the original digest"


def test_empty_batch_folds_to_zero():
    addr, mirror = _export(0, 0)
    assert mirror.length == 0
    assert LIB.avec_digest_of(addr) == 0, "empty array folds to zero"
