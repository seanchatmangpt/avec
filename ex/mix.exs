defmodule CgapEx.MixProject do
  @moduledoc "Mix project for the cgap-beam Elixir wrapper (lane REL-L9)."

  use Mix.Project

  @version "26.10.29"

  def project do
    [
      app: :cgap_ex,
      version: @version,
      elixir: "~> 1.19",
      elixirc_paths: elixirc_paths(Mix.env()),
      start_permanent: Mix.env() == :prod,
      deps: []
    ]
  end

  # Run in the test env only; the test suite needs no extra compile-time paths.
  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  def application do
    [extra_applications: [:logger]]
  end
end
