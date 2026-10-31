# 52 Courts Audit: Non-Vacuity Sweep (Wave-7 Lane 13)

Wave-7 anti-tautology sweep over every test court family on the wave surfaces.
Question per court: does it assert real final state, and has it been witnessed
FAILING or designed to flip (in-code mutation/corrupt/negative mechanism)?
Run receipt: all suites green this session — `cargo test -p avec-arrow`,
`-p avec-parquet --features std`, `-p avec-git`, `-p avec-blake3`,
`-p avec-ffi`, `-p avec-beam` (1 passed), `-p avec-integration
--test admission_e2e` (2 passed), plus `scripts/audit_branchless.sh --selftest`
rc=0, `scripts/validate_ontology.sh` rc=0, and a divan2receipt both-direction
probe (good rc=0 receipt minted; mutated rc=1, no receipt). Prior full battery:
`docs/jira/v26.10.31/verify-latest.md` (2026-10-31T02:58:40Z, HEAD ead40329, all PASS).

## Verdict Summary

- COURT-LIVE: 21 court families
- COURT-UNPROVEN: 3 court families (each with a minimal flip test below)

## 1. avec-arrow — COURT-LIVE (4 families)

### mutation_kills.rs (36 tests) — LIVE

- Path: `crates/avec-arrow/tests/mutation_kills.rs`.
- Mutant kills cited as `file:line:col` rows in test bodies
  (e.g. L116-128 `<`->`<=` mutant; L167-171 `!`-deletion, `&`->`|`,
  `i>>3`->`i<<3` kills; L236 `if tail == 64` -> `!=`; L302 loop-skip mutant).
- Asserts final state (bitmap words, tails, counts) on real buffers.

### transpose round-trip (src/transpose.rs, 5 tests) — LIVE

- `round_trip_random_corpora` (L204): transpose -> untranspose equality over
  lens 0..257 incl. odd remainder (L227). Any transpose/remainder mutant
  diverges from `aos` — the property flips by construction.
- Layout/size laws asserted (`size_and_align_are_64`).

### predicates exhaustive/reference courts (src/predicates.rs, 6 tests) — LIVE

- Exhaustive ct_eq/lt/select over small domains (L170, L235).
- Differential vs scalar reference filter on 64 seeded corpora (L246),
  all-match/no-match (L294), sentinel-heavy near-needle corpus (L308).
- Final-state asserts (`assert_eq!` on filtered index vectors).

### layout/validation/property inline tests (22 tests) — LIVE

- `src/layout.rs` (11), `tests/validation.rs` (6), `tests/property.rs` (1)
  plus src inline courts; final-state asserts on real builders/arrays.

## 2. avec-parquet — COURT-LIVE (5 families)

### rle_court (8 tests) — LIVE

- `crates/avec-parquet/tests/rle_court.rs`.
- Witnessed-defect regression: bits=12 shift-overflow class
  (L126 `bits12_witness_class_regression`; header L1-7 "witnessed at bits=12";
  `crates/avec-parquet/src/rle.rs:85` "witnessed").
- Loud refusals asserted via panic-message match (L95-119), typed decoder
  refusal (L147-155), fast/baseline byte parity + refusal delegation
  (L161-178). Golden-byte wire asserts in mutation_kills kill tag/shift
  mutants (`tests/mutation_kills.rs` L29-88, 17 tests).

### level_free_multi_court (4 tests) — LIVE

- `crates/avec-parquet/tests/level_free_multi_court.rs`.
- Positive isomorphism court: writer -> file -> mmap scanner -> byte equality
  per column (L71-126), header carries all columns (L131-155).
- Negative courts with typed exact errors: `optional_field_refused`
  (L161-177), `wrong_payload_length_refused` (L182-222, incl. refusal then
  lawful-write sanity L209 — the court can flip).

### scan courts (src/scan.rs, 6 tests) — LIVE

- Typed refusals: `nonzero_def_level_refused` (L994),
  `optional_column_refused` (L1022), `corrupt_offset_refused` (L1038).
- Byte-equality round-trips vs level-free writer (L896, L918).

### page_zero_alloc (2 tests) — LIVE

- `crates/avec-parquet/tests/page_zero_alloc.rs`.
- Positive control in-code: `positive_control_counter_is_live` (L133-142)
  asserts the counting allocator counted >= 1 for a fresh Vec
  ("court is vacuous" otherwise). The anti-vacuity pattern to copy.

## 3. avec-git — 2 LIVE, 1 UNPROVEN

### pack roundtrip + corrupt-trailer mutation tests (src/pack_write.rs, 7 tests) — LIVE

- `corrupted_pack_trailer_fails_checksum` (L612): flips last byte, asserts
  typed `PackChecksumMismatch` (explicit mutation mechanism).
- `corrupted_idx_trailer_fails_checksum` (L624): same for idx
  (`IdxChecksumMismatch`).
- `bad_magic_rejected` (L656), `object_count_cap_enforced` (L637),
  independent-parser roundtrip (L556), builder/direct byte equality (L597).

### fsck_court negative control (2 tests) — LIVE

- `crates/avec-git/tests/fsck_court.rs`.
- `negative_control_corrupted_git_built_pack_is_rejected` (L209): real git
  builds a pack, one byte corrupted (L287 `corrupt_bytes[16] ^= 0x01`), git
  must reject; if git accepts, the test fails with an explicit "VACUOUS
  COURT" verdict (L311). Designed to flip, real subprocess + disk.

### git_interop direction_b (skip-loud) — COURT-UNPROVEN

- `crates/avec-git/tests/git_interop.rs` `direction_b_git_accepts_avec_pack`
  (L304-358).
- The writer emits STORED bodies; real git rejects the pack; the test prints
  `[COURT-B SKIP-LOUD]` and exits 0 (L349-358). The skip itself is the
  recorded FAIL — never witnessed passing.
- Minimal flip test: emit zlib (deflate::zlib_compress) bodies in
  `write_pack_v2` so `git fsck --full` + `verify-pack` accept; the existing
  cat-file content asserts (L334-342) then bind for real. Alternative: a
  court asserting the skip branch is taken only when git's stderr matches
  the zlib prerequisite (pin the gap, forbid gap-drift).

## 4. avec-blake3 — 3 LIVE, 1 UNPROVEN

### official vectors (6 tests) — LIVE

- `crates/avec-blake3/tests/official_vectors.rs`: known-answer vectors
  (external grounding), wide-SIMD vs scalar differential (L76), chunk and
  subtree boundaries (L91-118).

### b3sum differential + x2 parity (multi_chunk_vectors.rs, 7 tests) — LIVE

- `differential_vs_b3sum_corpus_25_and_odd_lengths` (L176),
  `x2_driver_matches_b3sum` (L225), `x2_kernel_equals_two_4way_kernels`
  (L149), `x2_kernel_matches_scalar_and_4way` (L274). External reference
  (b3sum binary) + cross-kernel parity; final-state (hex digest) asserts.

### small-batch court (small_batch_vectors.rs, 3 tests) — LIVE

- b3sum window differential (L43), eligibility predicate (L77), exhaustive
  small parity sequential vs 4-way (L90).

### zero_alloc (4 tests) — COURT-UNPROVEN

- `crates/avec-blake3/tests/zero_alloc.rs`.
- A hand-rolled counting `GlobalAlloc` + thread-local counters; all four
  tests assert delta == 0. No positive control: if the counters silently
  stopped counting, every test passes vacuously.
- Minimal flip test: add `positive_control_counter_is_live` mirroring
  `crates/avec-parquet/tests/page_zero_alloc.rs:133` — measure
  `Vec::<u8>::with_capacity(64)` under the same counters, assert
  `delta >= 1` ("counting allocator counted 0 allocs — court is vacuous").

## 5. avec-ffi — COURT-LIVE (2 families)

### zero_copy (5 tests) — LIVE

- `crates/avec-ffi/tests/zero_copy.rs`: mutation-through-view visibility
  (L71), exact-once release with disarm assert (L153), pointer identity
  (L51). Real FFI views, final-state asserts; double-release negative.

### alloc_courts (4 tests) — LIVE

- `crates/avec-ffi/tests/alloc_courts.rs`: uses
  `avec_audit::alloc_counter::with_alloc_tracking`, whose own tests include
  a live counter probe (`crates/avec-audit/src/alloc_counter.rs:174`
  `vec_push_reports_nonzero_count_and_bytes`). Counter liveness grounded.

## 6. avec-beam bridge contract — COURT-LIVE (1 family)

- `crates/avec-beam/tests/bridge_contract.rs` (1 test, real escript).
- Real BEAM VM loads the NIF; stdout must contain `ok fail_closed` — the
  Erlang side asserts the NIF's error path (negative control inside
  `ESCRIPT_SRC`, L49+). Exit 0 + 10 contract proof strings asserted
  (L471-483). Build failure loud (L39-41).

## 7. integration — COURT-LIVE (admission_e2e), no mutation kills

- `crates/integration/tests/admission_e2e.rs` (2 tests): lossless layout
  codec sanity, deterministic BLAKE3 receipt, byte-identical readback
  (L193), receipt-survival (L195). Witnessed green this session.
- Disclosed skip: git packfile wrap stage (L155) — same zlib prerequisite
  as avec-git court B; counted once under avec-git UNPROVEN above.
- No mutation-kill tests exist in integration (checked
  `membranes.rs`/`cross_crate.rs` — negative paths asserted: double
  release, alignment laws, format table).

## 8. scripts gates — 3 LIVE (witnessed), 1 UNPROVEN

### audit_branchless positive control — LIVE (witnessed this session)

- `scripts/audit_branchless.sh --selftest` rc=0:
  "== positive control: FIRED (4 banned ops in branchy.o) — gate is not
  vacuous"; exit 2 = VACUOUS GATE branch in-code (L148).

### validate_ontology mutant — LIVE (witnessed this session)

- `scripts/validate_ontology.sh` rc=0: mutant TTL
  (settlementLatencyNs 14900 -> 50000) rejected by pyshacl
  ("== ANTI-VACUITY: PASS (mutant rejected: REFUSAL-13 violation)").
  Mutated gate exits 1 (L48).

### divan2receipt both-direction probes — LIVE (witnessed this session)

- Good table (12.9 GiB/s >= LSL 8.5): rc=0, receipt minted (pg.ttl, 1558 B).
- Mutated table (7.9 GiB/s): rc=1, "TURNSTILE: CTQ-06 breach — receipt NOT
  emitted"; mut.ttl ABSENT. Both directions witnessed on disk.

### cfg_homology — COURT-UNPROVEN

- `scripts/cfg_homology.sh`: compares source branch constructs against
  emitted branch opcodes with declared slack; no anti-vacuity control, no
  witnessed flip anywhere in the script (grep: only semantics notes).
- Minimal flip test: compile a branchy constant-time-kernel fixture (e.g.
  the `scripts/audit_branchless_cases/branchy.c` corpus) and require the
  gate to exit 1; also a slack=0 positive control on a clean kernel fixture
  (must pass) — both directions, then record the receipts.

## Notes

- `crates/avec-blake3/tests/bench_tmp.rs` (1 test) is a scratch bench stub,
  not a correctness court.
- `crates/avec-git/tests/perf_baseline.rs` (1 `#[ignore]`d perf test) is not
  a correctness court; opt-in via `--ignored`.
- `fsck in-repo parser` maps to `fsck_court.rs`'s negative control (above,
  LIVE); no separate in-repo parser gate exists.

## Receipt

- Subject: wave-7 lane-13 anti-tautology audit, repo /Users/sac/avec (main,
  HEAD ead40329 + working tree), doc
  docs/jira/v26.10.31/52-courts-audit.md.
- Commands/exits: cargo test per crate (all exit 0; counts: arrow
  mutation_kills 36, validation 6, property 1; parquet rle_court 8,
  level_free_multi_court 4, mutation_kills 17, page_zero_alloc 2;
  avec-git fsck_court 2 + git_interop 3; avec-blake3 4+0 targets green;
  avec-ffi 4+5; avec-beam 1; admission_e2e 2); audit_branchless --selftest
  rc=0; validate_ontology rc=0; divan2receipt good rc=0 / mutated rc=1.
- Verdicts: COURT-LIVE 21, COURT-UNPROVEN 3
  (git direction_b zlib flip, blake3 zero_alloc positive control,
  cfg_homology both-direction control).

## Addendum (wave 8/9): COURT-UNPROVEN 3 -> 0

All three wave-7 COURT-UNPROVEN entries are stale as of wave 8/9. Each closure
was re-verified on disk and re-run this session (2026-10-30).

### 1. avec-blake3 zero_alloc positive control — LANDED, now COURT-LIVE (5 tests)

- `crates/avec-blake3/tests/zero_alloc.rs:169`
  `positive_control_counter_is_live`: allocates a fresh `Vec` inside a tracked
  window and asserts delta >= 1 (L182-185, "court is vacuous" on zero), then
  resets counters and re-asserts `avec_blake3::hash` == 0 allocations under
  the same held lock (L188-194).
- Witnessed this session: 5 passed / 0 failed, rc=0, 3 consecutive runs
  (13.35s / 13.41s / 13.34s) via `cargo test -p avec-blake3 --test zero_alloc`.
- The wave-7 "no positive control" premise is obsolete; the zero-alloc
  assertions are grounded by an in-file anti-vacuity test.

### 2. cfg_homology both-direction control — LANDED, court witnessed FLIPPED

- `scripts/cfg_homology.sh --mutant` with `scripts/cfg_homology_cases/`
  (clean.c, branchy.c) runs the SAME source_constructs + count_symbols
  pipeline over -O0 fixtures at slack=4: clean (2 constructs, 6 emitted,
  delta 4) sits exactly on the slack floor and PASSES; branchy (4 constructs,
  17 emitted, delta 13) FAILS; any other combination is VACUOUS (exit 2).
- Witnessed this session: "== MUTANT COURT FLIPPED", exit 0 — identical to
  receipt gate-cfg-homology-v2.md (2026-10-31T03:41Z, Standing ALIVE).
- The wave-7 "no witnessed flip anywhere in the script" premise is obsolete.

### 3. avec-git git_interop direction_b — LANDED (wave 4/5), now COURT-LIVE

- `crates/avec-git/src/pack_write.rs` L4-8: FR-04 flip — object bodies are
  zlib streams (RFC 1950/1951, in-crate `crate::deflate::zlib_compress`,
  call sites L245/L314). The stored-bodies premise of the wave-7 skip is
  obsolete.
- Witnessed this session: `cargo test -p avec-git` exit 0 — 43 lib tests +
  fsck_court 2/2 + git_interop 3/3; `direction_b_git_accepts_avec_pack` ok
  with no skip-loud branch taken (git 2.51.2; `git fsck --full` and
  `git verify-pack -v` both exit-ok=true).
- Receipt gate-git-tests-v2.md (2026-10-31T02:17Z) Standing ALIVE.

### Net + remaining caveat

- COURT-UNPROVEN 3 -> 0; COURT-LIVE 21 -> 24 court families.
- Remaining caveat (adjudication, not vacuity): cfg_homology NORMAL mode
  still FAILs on transpose — delta +17 (2 source constructs, 19 emitted
  branches: transpose 12 + untranspose 7) vs slack 4, gate exit 1, witnessed
  this session. That is the ADR-14 question (fix the kernel vs reviewed
  slack amend; tracked in docs/jira/v26.10.31/54-checklist-sweep-v4.md).
  Not a court-vacuity defect: the mutant court proves the delta verdict
  flips, so the FAIL is a live signal, not a dead gate.

### Also stale: section 7 integration skip (observation only; test unedited)

- `admission_e2e.rs` L159-165 skip text says crates/avec-git is absent from
  workspace members and pack_write.rs lacks the wrap API. Both premises are
  now false: `Cargo.toml` L10 lists crates/avec-git; `lib.rs` L22 re-exports
  write_idx_v2 / write_pack_v2 / IdxEntry / PackBuilder. Test file is not
  owned by this lane; flagged for the owning lane.

### Addendum receipt

- Subject: wave-9 lane-3 addendum, appended only, to
  docs/jira/v26.10.31/52-courts-audit.md, repo /Users/sac/avec (main,
  HEAD ead4032 + dirty tree).
- Commands/exits this session: `cargo test -p avec-blake3 --test zero_alloc`
  x3 (rc=0, 5 passed each); `cargo test -p avec-git` (exit 0; 43 lib +
  fsck 2 + interop 3); `cargo test -p avec-git --test git_interop --
  direction_b --nocapture` (exit 0, fsck/verify-pack exit-ok=true);
  `bash scripts/cfg_homology.sh --mutant` (exit 0, court FLIPPED);
  `bash scripts/cfg_homology.sh` (exit 1, transpose +17 vs slack 4 — the
  expected FAIL, i.e. the ADR-14 question).
- Receipts cited: gate-cfg-homology-v2.md, gate-git-tests-v2.md (ALIVE).
- Verdicts: COURT-UNPROVEN 3 -> 0; COURT-LIVE 21 -> 24.
