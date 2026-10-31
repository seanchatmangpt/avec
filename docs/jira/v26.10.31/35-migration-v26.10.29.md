# Migration Plan: v26.10.29 Baseline to v26.10.31 Targets

## Standing

UNVERIFIED. This plan audits by inference from file names, git status, and
prior-milestone docs only. Per-file verification is required before any work
item is scoped as small or large.

## Current-State Inventory (as of 2026-10-30)

Crates present in the workspace (`/Users/sac/avec/crates/`): avec-arrow,
avec-parquet, avec-beam, avec-ffi, avec-blake3, avec-git, avec-gpu, audit,
bench, cli, integration, wasm-footprint — 12 crates.

Prior-milestone receipts live in `../v26.10.29/` (AVEC-101 through AVEC-111,
plus INDEX.md). Highlights cited by those docs: AVEC-111 Metal GPU + sovereign
BLAKE3 core with phase-change benchmarks; shm SPSC ring (2.2 GiB/s
cross-process, ~40us slot RTT, as committed in 30bf62c); batch rectangular
alignment wiring + offset-type fixes (20248e4); oblivious no_std fix (d80a018).

Uncommitted working-tree state at audit time (from `git status`): modified —
Cargo.lock, Cargo.toml, Makefile, README.md,
crates/avec-arrow/src/blake3.rs, crates/bench/Cargo.toml,
crates/bench/src/bin/phase_change_bench.rs, docs/CONTENT_ADDRESSING.md,
docs/DFLSS_CHARTER.md, docs/PHASE_CHANGE_RECEIPTS.md,
docs/jira/v26.10.29/AVEC-111-metal-gpu-and-blake3.md. Untracked —
crates/avec-blake3/, docs/diataxis/, docs/jira/v26.10.31/.

## Gap Analysis

| PRD FR | Current asset | Gap | Closure work item |
|---|---|---|---|
| FR-01 quad layout | avec-arrow offset types (batch fixes, 20248e4) | 64B struct layout + sentinel audit unproven | Add 64B quad struct with layout test + sentinel-value audit |
| FR-03 parquet projection | avec-parquet DataPageV2 work (v26.10.29 decode receipts) | Zero-level decode + isomorphism assertion missing | Zero-level paths + round-trip isomorphism assertion vs Arrow |
| FR-04 packfile | No packfile writer identified on disk | Loudly MISSING | Implement per 25-git-packfile-integration-spec.md |
| FR-05 branchless topology | Branchless primitives (AVEC-107/108) | 16-stage ladder + audit gate MISSING | Stage ladder + audit gate per 27-branchless-kernel-abi.md |
| FR-06 BLAKE3 engine | avec-blake3 core landed via AVEC-111 | Conformance vectors + throughput gate missing | Official vectors + throughput gate per 26-blake3-engine-spec.md |
| FR-07 beam/FFI zero-copy | avec-beam, avec-ffi exist | Pointer-identity proof missing | Zero-copy pointer-identity proof per 28-beam-nif-handoff.md |

## Sequencing

Dependency-ordered:

1. Layout (FR-01) — everything downstream assumes the 64B quad.
2. Projection (FR-03) — depends on layout; feeds packfile payload shape.
3. Packfile (FR-04) — depends on projection; CONTENT_ADDRESSING is in flight.
4. Audit gate (FR-05) — gates kernels used by projection and packfile paths.
5. BLAKE3 gates (FR-06) and FFI proof (FR-07) — independent of 1-4 once
   layout is fixed; may run as parallel lanes.

Compile-freeze discipline for parallel lanes: shared `Cargo.toml`/`Makefile`
edits are single-owner; a lane edit that breaks the shared build is fixed or
reverted by its owner within ~10 minutes; minimal unblock fixes are disclosed
in both lanes' receipts.

## Definition of Done

- Verification-ladder machine report all-green: layout, projection
  isomorphism, packfile round-trip, branchless audit gate, BLAKE3 conformance
  + throughput, FFI pointer-identity — every cell pass or registered
  divergence, no silent skips.
- Per-FR receipts committed (no receipt-only-in-stdout); working tree clean
  of the currently uncommitted AVEC-111 carryover (landed or reverted).
- Untracked crates/avec-blake3/ and milestone docs committed.

## Verification Caveat

This plan audits by inference from file names, git status facts, and
`../v26.10.29/` milestone docs. No per-file source verification was performed
in this lane. Before any work item is scoped small or large, verify the named
assets on disk (e.g. confirm avec-parquet zero-level absence, confirm no
packfile writer exists anywhere in the workspace).

## Closure (wave 7)

Wave-7 lane-11 reconciliation of the gap table above against waves 3-7.
Evidence: 32-traceability-matrix.md (W6 refresh), 47-milestone-verdict.md,
45-wave-6-receipt.md, receipts/ inventory. As-of 2026-10-30; nothing below
is committed to git.

### Closed as planned

| FR gap row | Landing | Waves | Gate + receipt evidence |
|---|---|---|---|
| FR-01 64B quad layout + sentinel audit | 64B quad layout tests + sentinel/bitmap-absence courts (crates/avec-arrow/src/layout.rs and lib tests) | W3-W4 | `cargo test -p avec-arrow --lib` 93/0 (W6 battery; 47-milestone-verdict row 2); gate-e2e-integration-v2 ALIVE |
| FR-03 zero-level decode + isomorphism | Level-free decode (level_free.rs, scan.rs), RLE court, mutation-kill court, aligned interop | W3-W6 | parquet 191/0 default-features (47 verdict rows 3/3'); rle_court 8/8, mutation_kills 17/17; gate-workspace-check-v2 ALIVE |
| FR-05 stage ladder + audit gate | audit_branchless.sh + objdump audit + positive/negative courts (scripts/audit_branchless_cases/) | W3-W6 | selftest court PASS; gate-branchless-selftest-v2 ALIVE. Partial: opcode residual open (R2) |
| FR-06 official vectors + throughput gate | Official 6-vector court + blake3_throughput bench (crates/bench/src/bin/) | W4-W6 | gate-blake3-tests-v2 ALIVE (vectors green); gate-blake3-throughput FAILED — see CTQ-06 below |
| FR-07 pointer-identity proof | bridge_contract.rs — alloc_spy delta {0,0}, dtor exactly-once 64, pointer identity, real BEAM escript | W4-W6 | gate-beam-bridge-v2 ALIVE; 47 verdict FR-07 PASS |
| FR-04 packfile writer | crates/avec-git/ full pack write/read + fsck court | W3-W4 | fsck 2/2 vs real git 2.51.2; gate-git-tests-v2 ALIVE |

### Closed differently

- **deflate (FR-04 path)**: not a plan line item. Landed W3-W4 from
  dependency-closure necessity — the fsck court (40-wave-3-receipt.md) exposed
  missing zlib deflate in the pack writer; real git 2.51.2 rejected stored
  bodies until crates/avec-git/src/deflate.rs landed (41-wave-4-receipt.md).
- **FR-04 whole crate**: the plan said "loudly MISSING" packfile writer; the
  landing is a new crate avec-git with real-git fsck + interop courts, wider
  than the plan's scoped item.
- **FR-03 shape**: plan asked for "round-trip isomorphism vs Arrow"; landing
  is level-free decode + RLE/mutation courts (green W6). Isomorphism intent
  carried by aligned_interop 4/4; not a literal Arrow-isomorphism test.
- **CTQ-04**: W6 matrix recorded a provisional pin PASS; the W6 battery
  re-measured 21.9% stdev/mean vs 15% provisional USL — recorded FAIL per
  no-rounding-up (47 verdict red-list item 2).

### Still open

- **CTQ-06 LSL ratification path**: throughput FAIL stands — best 1.85 GiB/s
  (hash_many_simd, min-of-5, 256 MiB) vs 8.5 GiB/s LSL. 48-lsl-rederivation.md
  derives a 1.1-3.1 GiB/core (no amortization) to 4.6-12.5 GiB/core (4-way
  amortization) window; 8.5 needs 4-way amortization + >=8 vops/cyc + >=4 GHz
  all at once. Decision armed, not made: ratify a reachable LSL or keep 8.5
  and stay FAIL. PRD not unilaterally amended.
- FR-05 banned-opcode census residual (R2). Wave-9 note: closed at the gate
  level — gate-branchless-manifest-v2 ALIVE over a 7-kernel manifest,
  positive control FIRED.
- C_pk / Gage R&R study never executed (R3; gage_rr.rs present, no receipt).
  Wave-9 annotation: STALE — gate-gage-rr ALIVE (exit 0, `--reps 10000`)
  and the study is executed: gate-cpk-study-v2 receipted (exit 1 by
  contract, `failed_gates: ["CTQ-06"]` only — the breach is CTQ-06's, not
  C_pk's).
- cfg_homology receipt pending (R4/R5). Wave-9 annotation: STALE —
  gate-cfg-homology-v2 ALIVE (exit 0, `--mutant` mode, mutant court
  FLIPPED); v1 FAILED normal-mode verdict registered under ADR-14.

### Definition of Done, reconciled

| Plan DoD clause | Reality |
|---|---|
| Machine report all-green | Not met — sole breach is CTQ-06. ctq_report exit 1, `failed_gates: ["CTQ-06"]` (gate-cpk-study-v2, 03:39Z; CTQ-04 closed at protocol layer, wave-8). Battery green elsewhere: 5/7 FR PASS, 2/7 PARTIAL |
| Receipts committed, tree clean of AVEC-111 carryover | NOT MET. Receipts exist on disk (36 receipts: 25 ALIVE / 10 FAILED / 1 PARTIAL per 64-receipts-inventory-final.md) but NOTHING is committed — the AVEC-111 carryover files plus all v26.10.31 work sit uncommitted/untracked |
| Untracked avec-blake3/ + milestone docs committed | NOT MET — still untracked on disk |

Wave-9 addition: the FR-07 five-op surface landed wave-8 — ops_scan.rs
(open/scan) + ops_persist.rs (flush/seal/receipt_query) registered in
crates/avec-beam/src/lib.rs:570-608; `cargo test -p avec-beam` exit 0
incl. the real-BEAM bridge contract (56 §Resolution).

Final migration step (integration commit) awaits the operator's commit
decision; this lane does not run git. Until that commit, DoD clause 2 and 3
are unmet by construction regardless of gate color.

### Standing

PARTIAL_ALIVE. Per-FR work items landed and gate-evidenced on disk; DoD
clauses 2-3 blocked solely on the operator's integration commit.

## See Also

- 32-traceability-matrix.md
- 47-milestone-verdict.md
- 48-lsl-rederivation.md
- ../v26.10.29/AVEC-111-metal-gpu-and-blake3.md
