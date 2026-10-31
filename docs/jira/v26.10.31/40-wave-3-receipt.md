# Wave 3 Receipt: Implementation Fan-Out (I1-I20)

20 implementation lanes, 2026-10-30, canonical checkout `/Users/sac/avec`,
one disjoint crate/script/docs surface per lane, coordinator verified all
gates personally at integration. All numbers below are re-run by the
coordinator, not trusted from lane reports.

## Final Gate Sweep (coordinator-executed)

| Gate | Result | Evidence |
|---|---|---|
| `cargo check --workspace` | PASS | exit 0 |
| avec-arrow lib tests | PASS | 93 passed / 0 failed |
| avec-parquet lib (`--features std`) | PASS | 162 passed / 0 failed |
| avec-git lib tests | PASS | 26 passed / 0 failed |
| avec-blake3 (`--test-threads=1`) | PASS | 9 passed / 0 failed |
| avec-audit | PASS | 5 passed / 0 failed |
| avec-beam | PASS | 1 passed (real BEAM escript, load_nif) / 0 failed |
| avec-integration e2e | PASS | 2 passed (release timing probe: 38 us vs 35 us USL) |
| verify_v26 machine report | PASS | 6 PASS / 1 SKIP (FR-07 BEAM ops), JSON valid |
| audit_branchless selftest | PASS | positive control FIRED (4 banned ops) |
| audit_branchless full gate | **FAIL (true baseline)** | 182 banned opcodes in transpose (114), c_abi release paths (42), filter_quads_eq (14) |
| avec-git fsck court | **FAIL (real, unhidden)** | git 2.51.2 rejects stored bodies; zlib deflate missing in pack writer (I6 surface) |

## Known Open Defects (owner, not hidden)

1. **FR-04 fsck court RED**: pack bodies are stored, git requires zlib
   deflate. Fix owner: avec-git pack_write. Court flips green on that fix.
2. **CTQ-06 LSL unreachable as architected**: avec-blake3 measures
   0.43 GiB/s (hash_wide_simd) — leaf-level scalar 7-round compression;
   only the parent Merkle reduction is SIMD. The 8.5 GiB/s LSL requires an
   8-way leaf kernel, which does not exist. Spec/impl owner decision
   needed (26-blake3-engine-spec.md).
3. **zero_alloc tests require `--test-threads=1`**: parallel libtest
   siblings allocate inside the measured window (witnessed false failure).
   CI must pin the flag.
4. **Branchless audit gate**: hot-scope symbols currently contain 182
   branches. True baseline, gate working as designed (selftest non-vacuous).
5. **Pre-existing defects found, unfixed (other surfaces)**:
   `rle_encode` bits=12 shift-overflow panic (avec-parquet rle.rs:129);
   duplicate `#[inline(always)]` (avec-arrow predicates.rs:59-60, future
   hard error); pre-existing warnings in layout/level_free.

## First CTQ Datapoints (baseline, no gates wired)

Apple M3 Max (Mac15,9), release: blake3 0.43-0.46 GiB/s (LSL 8.5: FAIL);
quad transpose ~14.2 GB/s input; predicate filter ~628M quads/s;
parquet level-free roundtrip ~98 GB/s aggregate; e2e admission 38 us
release (USL 35: just above).

## Lane Ownership Summary

I1 layout.rs (Quad64, sentinels, offsets) / I2 transpose.rs / I3
predicates.rs / I4 level_free.rs / I5 scan.rs (mmap, zero-level courts) /
I6+I7 avec-git (pack writer, idx ladder; I6 unblocked bench stub) / I8
fsck_court.rs (RED by design) / I9 official_vectors.rs (25-length corpus,
b3sum cross-verified) / I10 zero_alloc + blake3_throughput bins / I11 ffi
zero_copy courts / I12 beam bridge contract in real BEAM / I13
alloc_counter.rs (crates/audit) / I14 audit_branchless.sh + positive
control / I15 admission_e2e / I16 wasm build receipts (both wasm targets
green; gated scan under std) / I17 cli verify_v26 JSON machine report /
I18 Divan benches (v26_10_31) / I19 mint_receipt.sh + receipts/ / I20
verify orchestrator + baseline.

Standing: implementation wave PARTIAL_ALIVE — code exists and its courts
run; FR-04 court is honestly RED; CTQ-06 floor unreachable on the current
architecture. Nothing committed; 41 dirty paths in the working tree.

## Wave-4 Outcomes (closure of this receipt's open defects, 2026-10-30)

Status of every defect named above, after the wave-4 fan-out (final statuses
in 41-wave-4-receipt.md):

| # | Wave-3 open defect | Outcome | Owner lane |
|---|---|---|---|
| 1 | FR-04 fsck court RED (zlib deflate missing in pack writer) | **FIXED** — deflate lane landed real zlib deflate; fsck court PASS 2/2 against real git 2.51.2 | L1 (deflate) |
| 2 | CTQ-06 LSL unreachable as architected (0.43 GiB/s leaf-scalar) | **IMPROVED, still FAIL** — 4-way NEON leaf kernel: 0.43 -> 0.938 GiB/s vs 8.5 LSL; 8-way impractical on current leaf design, disclosed | L5 (blake3-leaf-simd) |
| 3 | zero_alloc requires `--test-threads=1` | **FIXED** — tests restructured to be structurally parallel-safe; pin no longer load-bearing | L9 (zero-alloc-serial) |
| 4 | Branchless audit gate: 182 banned opcodes | **REDUCED, gate still RED by design** — transpose 114 -> 24 (allocator null-checks + loop latch inherent), predicates 7 -> 2; residual documented | L3 (transpose), L4 (predicates) |
| 5a | rle_encode bits=12 shift-overflow panic (rle.rs:129) | **FIXED** — domain refusal at encode boundary; 8-court regression file `tests/rle_court.rs` added | L8 (rle-fix) |
| 5b | duplicate `#[inline(always)]` (predicates.rs:59-60) | **FIXED** — removed in the predicates rewrite | L4 (predicates) |
| 5c | pre-existing warnings in layout/level_free | **OPEN (low priority)** — not addressed this wave; no owner lane assigned | unassigned |
| — | e2e 38 us vs 35 us USL (wave-3 datapoint) | **STILL MARGINAL** — 38 us re-measured; 94% blake3-bound, admission not the constraint | L6 (e2e-latency) |

Carried into wave 5: git-interop direction B, SHACL 3 closed-shape
violations (NON-CONFORMANT), CTQ-05/06 margin, level_free warnings.
