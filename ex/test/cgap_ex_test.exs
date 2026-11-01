defmodule CgapExTest do
  # The NIF registry is process-global, but every test allocates its own
  # slots and asserts only on its own handles, so parallel test processes
  # cannot collide (ids are monotonic slot indexes, never assumed absolute).
  use ExUnit.Case, async: true

  @version "26.10.29"

  test "version is the cgap-arrow version law" do
    ver = CgapEx.version()
    assert is_binary(ver)
    assert ver == @version
  end

  test "i32 ramp contract reads" do
    id = CgapEx.new(1000, 5)
    assert is_integer(id)
    assert CgapEx.len(id) == 1000
    assert CgapEx.get(id, 0) == 5
    assert CgapEx.get(id, 999) == 1004
    assert CgapEx.free(id) == :ok
  end

  test "typed errors: freed handle, unknown handle, out-of-range index" do
    id = CgapEx.new(3, -1)

    assert CgapEx.get(id, 3) == {:error, :error}

    assert CgapEx.get(id, 2) == 1
    assert CgapEx.free(id) == :ok

    assert CgapEx.free(id) == {:error, :error}
    assert CgapEx.len(id) == {:error, :error}
    assert CgapEx.get(id, 0) == {:error, :error}
    assert CgapEx.get(999_999, 0) == {:error, :error}
    assert CgapEx.stats(id) == {:error, :error}
  end

  test "non-integer args fold badarg exits to typed errors" do
    assert CgapEx.get(:not_an_integer, 0) == {:error, :error}
    assert CgapEx.get_i64(:not_an_integer, 0) == {:error, :error}
  end

  test "dirty path: n = 20000 builds and reads through" do
    id = CgapEx.new(20_000, 7)
    assert CgapEx.len(id) == 20_000
    assert CgapEx.get(id, 16_383) == 16_390
    assert CgapEx.free(id) == :ok
  end

  test "i64 ramp exact reads, stats, double free" do
    id = CgapEx.new_i64(3, 4_000_000_000)
    assert is_integer(id)
    assert CgapEx.len(id) == 3
    assert CgapEx.get_i64(id, 0) == 4_000_000_000
    assert CgapEx.get_i64(id, 1) == 4_000_000_001
    assert CgapEx.get_i64(id, 2) == 4_000_000_002
    assert CgapEx.get(id, 0) == {:error, :error}
    assert CgapEx.stats(id) == {3, 64}

    assert CgapEx.free(id) == :ok
    assert CgapEx.free(id) == {:error, :error}
    assert CgapEx.get_i64(id, 0) == {:error, :error}
  end

  test "i64 negative fill beyond the i32 range" do
    id = CgapEx.new_i64(2, -4_000_000_000)
    assert CgapEx.get_i64(id, 1) == -3_999_999_999
    assert CgapEx.stats(id) == {2, 64}
    assert CgapEx.free(id) == :ok
  end

  test "i64 ramp overflow is refused, not wrapped" do
    assert CgapEx.new_i64(2, 9_223_372_036_854_775_807) == {:error, :error}
  end

  test "mixed workload: i32 and i64 alive simultaneously, independent" do
    id_a = CgapEx.new(2, 10)
    id_b = CgapEx.new_i64(2, 4_000_000_000)
    assert is_integer(id_a) and is_integer(id_b)
    assert id_a != id_b

    assert CgapEx.len(id_a) == 2
    assert CgapEx.len(id_b) == 2
    assert CgapEx.get(id_a, 1) == 11
    assert CgapEx.get_i64(id_b, 1) == 4_000_000_001
    assert CgapEx.get_i64(id_a, 0) == {:error, :error}
    assert CgapEx.stats(id_a) == {2, 32}
    assert CgapEx.stats(id_b) == {2, 64}

    assert CgapEx.free(id_a) == :ok
    assert CgapEx.free(id_b) == :ok
    assert CgapEx.len(id_a) == {:error, :error}
    assert CgapEx.len(id_b) == {:error, :error}
  end
end
