# Changelog

All notable changes to avec are documented in this file. Format based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
the project's calendar scheme (`26.10.29` = 2026-10-29). Every number below is
transcribed from the v26.10.29 receipt docs (`docs/RECEIPTS.md`,
`docs/BEAM_LATENCY.md`, `docs/BRANCH_REGISTER.md`, `docs/wasm-footprint.txt`,
`docs/BENCH_FLOORS.json`); conflicts between receipt docs are listed in the
release notes' Discrepancies section, not reconciled here.

## [Unreleased]

### Added

- `docs/CONSUMER_REQUIREMENTS.md` — PurRDF dependency pass: publication-fit
  facts, the T1-T4 requirement tickets (batch/table layer, typed zero-copy
  ingest, timestamp `Scalar`, wasm32-unknown-unknown receipt), and the
  standing table.
- Ticket T1: `batch.rs` — `Column`/`Batch`/`BatchBuilder` named column
  layer with the finish-time length law; `VarlenBuilder::len`/`is_empty`
  and `ViewVecBuilder::len`/`is_empty`.
- Ticket T2: `foreign.rs` — typed zero-copy ingest of foreign C Data
  Interface pairs (`format_of`, `primitive_view`, `is_valid`,
  `ForeignError`), bounds- and alignment-checked against caller-supplied
  buffer lengths.
- Ticket T3: `timestamps.rs` — `TimestampSecond/Milli/Micro/Nano` `Scalar`
  newtypes over i64 (format codes `tss:/tsm:/tsu:/tsn:UTC`).
- Ticket T4: `scripts/footprint.sh` accepts `WASM_TARGET`; the full
  footprint gate passes for `wasm32-unknown-unknown` (3,435 B stripped,
  CTQ-4 PASS).

## [26.10.29] - 2026-10-29

Initial entry: first release ledger for the avec columnar substrate.

### Added

- avec-arrow v26.10.29 — freestanding, branchless in-memory columnar
  substrate; `no_std` + `alloc`, zero external dependencies. Publish-enabled
  (`publish = true`). Kernels present on disk: `bitmap`, `primitive`,
  `varlen`, `views`, `c_abi` (+ `c_abi_nested`), `silicon`, `buffer`,
  `runend`, `slice`.
- avec-parquet v26.10.29 — branchless Parquet persistence kernels;
  `no_std`, zero deps. Publish-enabled (`publish = true`). Kernels present on
  disk: `rle`, `packing`, `unpack`, `encode_fast`, `dictionary`, `thrift`,
  `page`.
- Internal crates (`publish = false`): avec-ffi (C-ABI membrane, Gate 3
  surface), avec-beam (erl_nif membrane), avec-audit (branch-audit cdylib),
  bench (Gate 2 harness), integration (integration tests), wasm-footprint
  (standalone footprint / symbol-gate artifact crate).
- Gate surface (`Makefile`): `test gate0 gate1 gate2 gate3 gates lint
  publish-check`, plus auxiliary `footprint`, `branch-audit`, `symbol-gate`,
  `gate-parquet`, `bench-all`, `bench-page`, `bench-gate`.
- Ontology-driven generation: `ggen.toml` pins `avec-pack` from
  ggen-marketplace; four generated surfaces (2 headers under `include/`,
  2 Rust modules under `crates/avec-arrow/src/`); Gate 0 proves byte-stable
  replay.
- Docs of record: DFLSS charter, receipts ledger, BEAM latency receipt,
  branch register, wasm footprint doc, bench floors, mutation-court backlog.
- Measurement harnesses: `scripts/branch_audit.py`, `scripts/gate3_cdata.py`,
  `scripts/footprint.sh`, `scripts/symbol_gate.py`,
  `test/beam_latency.escript`.

### Changed

- Branch-audit claim rescoped to compiled reality
  (`docs/BRANCH_REGISTER.md`): the earlier "9/9 kernels, 0 conditional
  branches" result was vacuous-by-wrapper (Class B audit defect); the
  callee-extent rewrite lands the honest claim — **inner-loop bodies are
  branch-free; latches, dispatch, and bounds paths are itemized**. Source
  and doc headers corrected (`bitmap.rs`, `lib.rs`, `silicon.rs` scoping).
- Gate 1 (`make gate1`) = CC=1 machine-code branch audit over
  `target/audit/libavec_audit.dylib`; the mutation-court receipt lives
  behind `make gate-parquet` (requires `docs/mutation-court-backlog.txt`).
- Wording discipline: no repo claim survives as "zero conditional branches"
  without the body/latch/dispatch scoping (register R4).

### Verified

Every row was executed 2026-10-29/30 in `/Users/sac/avec` with exit codes
captured in the receipt docs.

- `cargo test --workspace` — exit 0; 26 passed / 0 failed (arrow 15,
  parquet 11). Receipt R1.
- gate0 — `ggen sync run` x2 + freestanding C99 header compile; exit 0,
  byte-identical replay, zero warnings. Receipt R5.
- gate1 — `scripts/branch_audit.py` (extent-scoped); exit 0, 9 kernels
  located, bodies branch-free. Receipt R3 + branch register.
- gate2 — `cargo run --release -p bench`; exit 0, 58.903 GiB/s single-core,
  0 allocs, 16-lane 74.8 GiB/s. Receipt R2. Machine-local.
- gate3 — pyarrow `pa.Array._import_from_c` parity; exit 0, bitwise,
  100000 rows, 14285 nulls. Receipt R4.
- ggen-marketplace check + gate-witness courts — exit 0, avec-pack
  qualify=ok, 5 cases, ALIVE standing. Receipt R6.
- footprint — `scripts/footprint.sh`; exit 101, BLOCKED, no artifact.
  Conflicting 10123 B PASS in `docs/wasm-footprint.txt` (Discrepancy 1).
- symbol-gate — zero-panic Jidoka, wasm-primary; no run receipt in docs,
  UNVERIFIED.
- gate-parquet — mutation-court receipt; ledger present, 114 survivor
  entries; court-run totals not receipted.
- BEAM scheduler-impact CTQ (<= 50 us) — FAIL as measured; impact
  7985.791 us, loaded max 11128.208 us; instrument floor disclosed.
  Receipt: BEAM_LATENCY.

Open items carried forward: 114 mutation-court survivors, BEAM latency FAIL,
symbol-gate scope, CTQ-4 reconciliation, Parquet kernels + minimal page
framing only (not the full file format). See `RELEASE_NOTES.md` (Known Open,
Next Milestone, Discrepancies).
