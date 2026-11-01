defmodule CgapPagesTest do
  @moduledoc """
  Page-pipeline language-surface tests for the cgap-beam NIF surface.

  Lane OPT20-L11 (version law 26.10.29). The page pipeline (write page ->
  RLE-encode -> decode -> read back) has NO servable NIF surface: the
  cgap-beam NIF (crates/cgap-beam/src/lib.rs) exposes only the integer
  batch surface (cgap_new/cgap_get/cgap_len/cgap_free/cgap_version and the
  i64 twins plus cgap_stats). This module tests exactly what IS servable
  over that integer surface and pins the typed gap loudly instead of
  silently skipping it.

  ## GAPS (typed; ids shared with bindings/python/cgap/tests/test_pages.py)

  - **GAPS-1** — no `cgap_rle_decode_into` NIF export. The RLE codec exists
    in the core crate (`RunEndEncoded::decode_to_dense`,
    crates/cgap-arrow/src/runend.rs) but is not reachable from the beam.
    Live probe: "page/RLE NIF exports are absent".
  - **GAPS-2** — no `cgap_write_page` / `cgap_read_page` NIF export (no
    `cgap_page*` symbol anywhere under crates/ or include/). Same probe.

  Close condition for both gaps: when an export lands, the probe test FAILS
  (loud) and demands a GAPS rewrite plus the real page test. The static
  `@tag skip` below never un-skips by itself — the probe is the live
  enforcement; the skip only surfaces the missing coverage in the ExUnit
  report with its typed reason. A green run here certifies the integer
  batch surface ONLY, never the page path.
  """

  use ExUnit.Case, async: true

  @version "26.10.29"

  @banned_exports [:cgap_rle_decode_into, :cgap_write_page, :cgap_read_page]

  @known_exports [
    :cgap_new,
    :cgap_len,
    :cgap_get,
    :cgap_free,
    :cgap_version,
    :cgap_new_i64,
    :cgap_get_i64,
    :cgap_stats
  ]

  # Fresh configs, disjoint from ex/test/cgap_ex_test.exs.
  @fresh_i32_n 4096
  @fresh_i32_fill 100
  @fresh_i64_n 1024
  @fresh_i64_fill 1_099_511_627_776

  setup do
    # Real load path: the first CgapEx call fires :cgap_beam's @on_load NIF
    # load, which is a precondition for the export-list probes below.
    ver = CgapEx.version()
    assert is_binary(ver)
    {:ok, ver: ver}
  end

  test "version is the cgap-arrow version law" do
    assert CgapEx.version() == @version
  end

  test "known NIF exports present (probe positive control)" do
    exports = :cgap_beam.module_info(:functions)

    for name <- @known_exports do
      assert Enum.any?(exports, fn {n, _arity} -> n == name end),
             "expected NIF export missing: #{name}"
    end
  end

  test "page/RLE NIF exports are absent (GAPS-1/GAPS-2 probe)" do
    exports = :cgap_beam.module_info(:functions)

    for name <- @banned_exports do
      refute Enum.any?(exports, fn {n, _arity} -> n == name end),
             """
             GAPS stale: #{name} is now exported by cgap-beam — update the \
             GAPS section in ex/test/cgap_pages_test.exs and \
             bindings/python/cgap/tests/test_pages.py, then write the real \
             page-pipeline test this probe was holding the place of\
             """
    end
  end

  @tag skip: "typed gap GAPS-1/GAPS-2: cgap-beam exports no cgap_rle_decode_into / cgap_write_page / cgap_read_page, so no page pipeline is drivable from the beam; see the GAPS section in @moduledoc"
  test "page pipeline through the NIF" do
    # Runs only if someone removes the @tag without closing the gap — keep
    # it loud. The live probe above is the real close condition.
    flunk("GAPS stale or tag removed: write the real page-pipeline NIF test")
  end

  test "fresh i32 batch roundtrip reads the ramp exactly" do
    id = CgapEx.new(@fresh_i32_n, @fresh_i32_fill)
    assert is_integer(id)
    assert CgapEx.len(id) == @fresh_i32_n
    assert CgapEx.get(id, 0) == @fresh_i32_fill
    assert CgapEx.get(id, 2047) == @fresh_i32_fill + 2047
    assert CgapEx.get(id, @fresh_i32_n - 1) == @fresh_i32_fill + @fresh_i32_n - 1
    assert CgapEx.get(id, @fresh_i32_n) == {:error, :error}
    assert CgapEx.stats(id) == {@fresh_i32_n, 32}
    assert CgapEx.free(id) == :ok
    assert CgapEx.free(id) == {:error, :error}
  end

  test "fresh i64 batch roundtrip beyond the i32 range" do
    id = CgapEx.new_i64(@fresh_i64_n, @fresh_i64_fill)
    assert is_integer(id)

    for i <- [0, 513, @fresh_i64_n - 1] do
      assert CgapEx.get_i64(id, i) == @fresh_i64_fill + i
    end

    assert CgapEx.stats(id) == {@fresh_i64_n, 64}
    assert CgapEx.free(id) == :ok
  end
end
