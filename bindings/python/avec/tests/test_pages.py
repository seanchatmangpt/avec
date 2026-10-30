"""Page-pipeline language-surface tests for the avec Python binding.

Lane OPT20-L11 (version law 26.10.29). The page pipeline (write page ->
RLE-encode -> decode -> read back) has NO servable surface in any language
binding today: crates/avec-ffi/src/lib.rs exports only the i32-batch and
version symbols (avec_i32_batch, avec_schema_addr, avec_array_addr,
avec_batch_rows, avec_version). This module tests exactly what IS servable
over the ctypes surface and pins the typed gap loudly instead of silently
skipping it.

GAPS (typed; ids shared with ex/test/avec_pages_test.exs)
---------------------------------------------------------
GAPS-1  avec-ffi does not export ``avec_rle_decode_into``. The RLE codec
        exists in the core crate (``RunEndEncoded::decode_to_dense``,
        crates/avec-arrow/src/runend.rs) but has no C-ABI membrane export.
        Live probe: test_page_exports_absent.
GAPS-2  avec-ffi does not export ``avec_write_page`` / ``avec_read_page``
        (no ``avec_page*`` symbol anywhere under crates/ or include/).
        Live probe: test_page_exports_absent.

Close condition for both gaps: when an export lands, test_page_exports_absent
FAILS (loud) and the skipif gate on test_page_pipeline opens into a loud
stub-failure demanding the real page test — a green run here certifies the
batch path ONLY, never the page path.
"""

from __future__ import annotations

import pytest

import avec
from avec.tests.test_cdata import _assert_bitmap_bitwise, _expected

VERSION_LAW = "26.10.29"

# Fresh configs: disjoint from test_cdata.CONFIGS [(100_000, 7), (1000, 0),
# (65537, 1000)]; 257 and 33 exercise the partial-byte validity mask path.
FRESH_CONFIGS = [(4096, 3), (257, 2), (33, 0)]

_LIB = avec.load_library()

_PAGE_SYMBOLS = ("avec_rle_decode_into", "avec_write_page", "avec_read_page")
_HAS_PAGE_EXPORT = any(hasattr(_LIB, s) for s in _PAGE_SYMBOLS)

_KNOWN_EXPORTS = (
    "avec_i32_batch",
    "avec_schema_addr",
    "avec_array_addr",
    "avec_batch_rows",
    "avec_version",
)


def test_known_exports_present():
    """Positive control for the gap probe: the probe instrument reads the real
    cdylib symbol table, so the five shipped exports must resolve."""
    for name in _KNOWN_EXPORTS:
        assert hasattr(_LIB, name), f"expected export missing: {name}"


def test_page_exports_absent():
    """GAPS-1/GAPS-2 probe: asserts the documented absence of every page/RLE
    symbol in the loaded cdylib. When an export lands this test FAILS by
    design — that failure is the close condition demanding a GAPS rewrite
    plus a real page test."""
    for name in _PAGE_SYMBOLS:
        assert not hasattr(_LIB, name), (
            f"GAPS stale: {name} is now exported by avec-ffi — update the "
            "GAPS section in bindings/python/avec/tests/test_pages.py and "
            "ex/test/avec_pages_test.exs, then write the real page-pipeline "
            "test this probe was holding the place of"
        )


@pytest.mark.skipif(
    not _HAS_PAGE_EXPORT,
    reason=(
        "typed gap GAPS-1/GAPS-2: avec-ffi exports no "
        "avec_rle_decode_into / avec_write_page / avec_read_page, so no "
        "page pipeline is drivable from python; see the GAPS section above"
    ),
)
def test_page_pipeline():
    """Un-skips only when the gap closes — and then fails loud on purpose:
    a stub must never silently pass in place of a page test that was never
    written."""
    pytest.fail(
        "GAPS closed: a page/RLE export landed in avec-ffi — replace this "
        "stub with the real page-pipeline test and delete the GAPS section"
    )


def test_raw_ctypes_batch_surface():
    """Drive the exported batch path directly through ctypes (no pyarrow):
    the pointer return, the row counter, and both export slots must agree."""
    ret = _LIB.avec_i32_batch(97, 4)
    assert ret is not None, "avec_i32_batch returned a null ArrowArray pointer"
    assert _LIB.avec_batch_rows() == 97
    assert _LIB.avec_array_addr() != 0, "null array export slot"
    assert _LIB.avec_schema_addr() != 0, "null schema export slot"
    assert avec.version() == VERSION_LAW


@pytest.mark.parametrize("n, null_every", FRESH_CONFIGS)
def test_batch_import_parity_fresh_config(n, null_every):
    """Page-adjacent regression: re-assert batch import parity for fresh
    configs (disjoint from test_cdata.CONFIGS) against recomputed ground
    truth — bitwise values, null count, and in-range validity bits."""
    import pyarrow as pa

    arr = avec.avec_new_i32_batch(n, null_every)
    assert arr.type == pa.int32(), f"type: {arr.type}"
    assert len(arr) == n

    expected_nulls, expected_validity, expected_values = _expected(n, null_every)
    assert arr.null_count == expected_nulls

    vbuf, dbuf = arr.buffers()[0], arr.buffers()[1]
    assert dbuf is not None, "values buffer missing"
    assert bytes(dbuf)[: 4 * n] == expected_values, "values bitwise mismatch"

    if vbuf is not None:
        _assert_bitmap_bitwise(vbuf, expected_validity, n)
    else:
        assert null_every == 0, "validity buffer missing on a null-bearing batch"
