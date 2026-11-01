# Project posture: publishing, licensing, and naming

This page explains *why* cgap is structured the way it is for outside
consumers: which crates are published, why the license has no SPDX
identifier, and why parts of the project still carry the name `avec`.

## Publishing model

The workspace publishes five crates to crates.io, all at version
`26.10.29` (`crates/*/Cargo.toml`, `version.workspace = true`):

| Crate | Role | Status |
|---|---|---|
| `cgap` | umbrella facade re-exporting the four kernel crates | live |
| `cgap-arrow` | columnar substrate (Arrow C Data Interface) | live |
| `cgap-blake3` | BLAKE3 hashing | live |
| `cgap-git` | Git packfile v2 writing | live |
| `cgap-parquet` | Parquet persistence | publishing 2026-10-31 |

Everything else in the workspace is internal: `publish = false` on
`cgap-ffi`, `cgap-beam`, `cgap-gpu`, `cli`, `audit`, `bench`,
`integration`, `wasm-footprint` (their `Cargo.toml` files).

The `cgap` facade exists so a consumer declares one dependency and gets
the kernel crates as a matched set (`crates/cgap/Cargo.toml` pins all
four at `26.10.29`). `cgap-ffi` is unpublishable partly because a
dev-dependency on `stardf` would drag every workspace member's
dev-deps into `cargo publish` — the comment at
`crates/cgap-ffi/Cargo.toml:24` records this packaging constraint.

## Licensing posture

Two files at the repo root form the license:

- `LICENSE.md` — the summary; it states there is **no SPDX expression**
  for the instrument and that it is identified by URN.
- `LICENSE-P28-SIL.md` — the instrument itself: the P28 Sovereign
  Invariant License v2.1.0, canonical identifier
  `urn:avec:license:p28-sil:v2.1.0-sovereign`.

Published manifests wire the license by file, not by SPDX id:
`license-file = "../../LICENSE.md"` in each of the five published
`Cargo.toml` files. So crates.io shows a license file rather than a
machine-readable SPDX expression — that is deliberate, because the
instrument's identity is the URN, not an SPDX code. If you need to
reference the license in your own metadata, use
`LicenseRef-P28-SIL-2.1.0` or the URN, per `LICENSE.md`.

## Frozen namespace: why `avec` still appears

The project was renamed `avec` → `cgap` on 2026-10-31 (receipt:
`docs/jira/v26.11.1/03-rename-receipt.md`). The rename was executed as
a forward commit chain with explicit per-decision verdicts (D1–D12).
Two categories of `avec` references are frozen on purpose:

1. **The license URN (D2: KEEP).** The URN
   `urn:avec:license:p28-sil:v2.1.0-sovereign` is the identity of a
   signed legal instrument. Renaming it would be equivalent to
   re-issuing the instrument, so `LICENSE*.md` were left untouched and
   the URN is cited as a historical namespace exception.
2. **Historical receipts (D7: DO NOT EDIT).** `docs/jira/v26.10.*`
   records are untouched; do not expect old receipts to use the new
   name.

By contrast, the *project identity namespace* was not a signed
instrument, so it moved: D10 replaced the `urn:avec:` prefix with the
dereferenceable https IRI space `https://cgap.chatmangpt.com/`
(operator decision recorded in commit `8b86157`). The distinction is:
de-reference targets rename; signed instruments do not.

A third pin exists for machine-checkable reasons: SHACL gates
byte-pin the v26.10.31 canonical TTL's `avec:` prefix bound to
`https://avec.chatman.dev/` (see the rename receipt's "Namespace
decision" section). Sweeping the needles breaks the gates.

## Frozen ABI surface

There is no standalone `docs/ABI-FREEZE.md`. The frozen ABI commitments
live in two places:

- `docs/jira/v26.11.1/03-rename-receipt.md`, decision **D4**: Arrow C
  Data Interface names — `ArrowArray`, `ArrowSchema`, `ARROW_FLAG_*` —
  NEVER RENAME; they stay verbatim in generated headers.
- `docs/DFLSS_CHARTER.md` (M-02): zero-copy C Data Interface with
  double-release as a typed no-op, verified by C-ABI tests in
  `crates/cgap-ffi` (`c_abi.rs`), exercised against the PyArrow
  C Data Interface (Gate 3 parity).

Practical consequence: if you consume the FFI surface, the Arrow C Data
Interface symbol and flag names are stable identifiers; the rename to
cgap did not and will not touch them.

## See Also

- `LICENSE.md`, `LICENSE-P28-SIL.md`
- `docs/jira/v26.11.1/03-rename-receipt.md` (D1–D12 decisions)
- `docs/DFLSS_CHARTER.md`
- `docs/diataxis/INDEX.md`
