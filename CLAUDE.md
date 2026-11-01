# cgap — Project Instructions

Sovereign `#![no_std]` columnar substrate (Arrow C Data Interface kernels,
BLAKE3, Parquet, Git packfiles) + binary ontology/N3 lanes. Renamed
avec -> cgap 2026-10-31; all five crates published on crates.io at 26.10.29
(cgap umbrella + cgap-arrow/blake3/git/parquet). Frozen byte-pins: license
URN `urn:avec:license:p28-sil:v2.1.0-sovereign` and the v26.10.31 SHACL
corpus IRIs — never sweep them.

## CLI (ggen six-pack generated)

`crates/cli` is a STANDALONE ggen crate (own `[workspace]`, NOT a root
workspace member). Binary: `cgap` (noun-verb: `cgap self version`,
`cgap bench mask`, `cgap bench parquet`, `cgap align test`; JSON output,
`--format` supported).

- Regenerate: `cd crates/cli && rm Cargo.toml && ggen sync run && python3
  manifest_patch.py` — proven byte-identical replay. Graph:
  `crates/cli/ontology/cgap-cli.ttl`; packs pinned in `crates/cli/ggen.lock`.
- Engine laws that shaped this: FM-PACK-018 (six-pack capability closure —
  crate-pack is mandatory), FM-WRITE-008 (single writer per target — the
  consumer manifest is patch-injected post-sync, never co-authored),
  FM-WRITE-005 (no silent clobber).
- Side-bins (`doctor`, `status`, `verify_v26`) are hand-maintained and stay
  in the crate; `just doctor` runs them via `--manifest-path`.
- `[patch.crates-io]` points clap-noun-verb at the operator checkout
  `~/clap-noun-verb` (26.9.1) over the generated =26.8.22 pin.

## Gates

- `cargo build --workspace` (root) must stay green; the CLI crate gates
  separately: `cd crates/cli && cargo build && cargo test` (includes the
  generated route proofs + the ported A20 `mutation_kills` RSS courts).
- `just doctor`: 10 checks; YELLOW = informational (exit 1 by design).
- Lane receipts live in `docs/jira/v26.11.1/lanes/` (A01-A20 specs) and
  `docs/jira/v26.11.1/waves/` (execution receipts).

## Discipline (inherits global rules)

- Chicago courts: every gate names its anti-vacuity mutation; a court that
  cannot flip is FAIL; skips are loud. No Process.sleep.
- Same-checkout fan-out: disjoint file ownership, coordinator owns git,
  lane build roots (`target-*/`) are leases — never commit them
  (`.gitignore` covers it; untrack, don't reset).
- `git commit -F <file>`, never inline `-m`; no rebase, no force-push.
- Generated outputs are never hand-edited: regenerate via the graph.
  Consumer seams: `custom_handlers.rs` (once), `manifest_patch.py` (post-sync).
