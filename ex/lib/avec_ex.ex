defmodule :avec_beam do
  @moduledoc false

  # The NIF's ErlNifEntry.name is "avec_beam" (crates/avec-beam/src/lib.rs),
  # and erlang:load_nif/2 requires the calling module's name to equal it, so
  # this module is deliberately the atom :avec_beam (not Elixir.AvecEx).
  # Loading happens in @on_load, the canonical load_nif context.

  @on_load :init

  @dylib "libavec_beam.dylib"
  @so "libavec_beam.so"

  def init do
    dir = nif_dir()
    dylib = Path.join(dir, @dylib)
    so = Path.join(dir, @so)

    if not File.exists?(dylib) do
      raise """
      avec_beam NIF not found at #{dylib}.

      Pre-step (dev): build the cdylib from the repo root:

          cargo build -p avec-beam

      cargo emits libavec_beam.dylib; erlang:load_nif/2 only ever appends
      ".so", so this init symlinks libavec_beam.so -> libavec_beam.dylib
      (refreshed on every load, mirroring test/beam_test.escript). Or point
      AVEC_NIF_DIR at a directory holding libavec_beam.dylib.
      """
    end

    # Refresh the .so symlink every load so it always targets the current
    # build. Deleting a symlink never touches the dylib.
    case File.rm(so) do
      :ok -> :ok
      {:error, :enoent} -> :ok
      {:error, reason} -> raise "cannot remove stale #{so}: #{inspect(reason)}"
    end

    case File.ln_s(dylib, so) do
      :ok -> :ok
      {:error, :eexist} -> :ok
      {:error, reason} -> raise "cannot symlink #{so} -> #{dylib}: #{inspect(reason)}"
    end

    path = String.to_charlist(Path.join(dir, "libavec_beam"))
    case :erlang.load_nif(path, 0) do
      :ok ->
        :ok

      # Re-loading the same NIF over itself is not an error (hot-reload path).
      {:error, {:reload, _}} ->
        :ok

      {:error, reason} ->
        raise "erlang:load_nif/2 failed for #{path}: #{inspect(reason)}"
    end
  end

  # Dev path resolution: $AVEC_NIF_DIR wins; default is the repo's
  # target/debug — i.e. ../target/debug relative to the ex/ dir, which from
  # this file's ex/lib home is two levels up (baked at compile time).
  defp nif_dir do
    case System.get_env("AVEC_NIF_DIR") do
      nil -> Path.expand("../../target/debug", __DIR__)
      "" -> Path.expand("../../target/debug", __DIR__)
      dir -> Path.expand(dir)
    end
  end

  # --- NIF stubs (replaced at load_nif time) --------------------------------

  def avec_new(_n, _fill), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_len(_id), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_get(_id, _i), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_free(_id), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_version(), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_new_i64(_n, _fill), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_get_i64(_id, _i), do: :erlang.nif_error(:nif_library_not_loaded)
  def avec_stats(_id), do: :erlang.nif_error(:nif_library_not_loaded)
end

defmodule AvecEx do
  @moduledoc """
  Elixir wrapper for the avec-beam NIF membrane (Rust: `crates/avec-beam`).

  Every function calls into the raw NIF module `:avec_beam`. The NIF answers
  raw values on success (ramp integers, `{len, bits}` stats tuples, `:ok` on
  free) and the atom `:error` on every failure — unknown handle, freed handle,
  out-of-range index, wrong type-tag dispatch, refused n/fill — plus a badarg
  exit for non-integer arguments to get/2 / get_i64/2. This wrapper normalizes
  all of those failures to `{:error, :error}`.

  ## Build pre-step (dev)

  The NIF is a cargo cdylib. Build it from the repo root before use:

      cargo build -p avec-beam

  cargo emits `target/debug/libavec_beam.dylib`; `erlang:load_nif/2` only ever
  appends `.so`, so at load time the wrapper refreshes the symlink
  `target/debug/libavec_beam.so -> libavec_beam.dylib` and loads from there.

  NIF directory resolution: `$AVEC_NIF_DIR` if set (non-empty), else the
  repo's `target/debug` (`../target/debug` relative to the ex/ dir).
  """

  @version "26.10.29"

  @doc "NIF module version (avec-arrow VERSION; version law #{@version})."
  def version, do: nif(fn -> :avec_beam.avec_version() end)

  @doc """
  i32 batch of n rows, row i holding fill + i (ramp). Handles are integer
  slot ids. Refusals (non-integer args, n out of 0..2^28, ramp overflow past
  i32::MAX, non-integer fill) come back `{:error, :error}`.
  """
  def new(n, fill), do: nif(fn -> :avec_beam.avec_new(n, fill) end)

  @doc """
  i64 twin of new/2: full i64 fill range, ramp overflow refused rather than
  wrapped. Cross-dispatch (get/2 on an i64 batch) is `{:error, :error}`.
  """
  def new_i64(n, fill), do: nif(fn -> :avec_beam.avec_new_i64(n, fill) end)

  @doc "Ramp value at index i of an i32 batch."
  def get(id, i), do: nif(fn -> :avec_beam.avec_get(id, i) end)

  @doc "Ramp value at index i of an i64 batch."
  def get_i64(id, i), do: nif(fn -> :avec_beam.avec_get_i64(id, i) end)

  @doc "Row count of the batch at id."
  def len(id), do: nif(fn -> :avec_beam.avec_len(id) end)

  @doc "{len, bits} for the batch at id — bits is 32 or 64."
  def stats(id), do: nif(fn -> :avec_beam.avec_stats(id) end)

  @doc """
  Frees the batch at id: `:ok` on first free, `{:error, :error}` on a second
  free (the slot is None after the first; no double-drop).
  """
  def free(id), do: nif(fn -> :avec_beam.avec_free(id) end)

  # NIF failure normalizer: the atom :error and the badarg exit (non-integer
  # args to get/get_i64) both fold to {:error, :error}; successes pass raw.
  defp nif(fun) do
    try do
      case fun.() do
        :error -> {:error, :error}
        other -> other
      end
    catch
      :error, :badarg -> {:error, :error}
    end
  end
end
