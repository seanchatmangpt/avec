# Falsifier Register — v26.10.31

Standing register of every refuted, retired, or superseded performance/design
target in the v26.10.31 milestone, each with its disproof and disposition. This
is the retained refutation record the wave-12/13 receipt points at ("the
falsification record is retained in full", 87-wave-12-13-receipt.md). Method
law: every design number is a hypothesis with a falsifier; lane receipts, not
conversation, set the verdict.

Status vocabulary follows the house register: CONFIRMED / PARTIAL / REFUTED /
NOT-CONFIRMED / RETIRED / LANDED. Refutation evidence is cited at file:line so
the disproof is replayable. Entries F-1..F-8 match the wave-14 lane-18 brief.

## F-1: CTQ-06 LSL 8.5 GiB/s/core — REFUTED as unsatisfiable; ratified 1.0/1.5/2.5

- **Claim.** BLAKE3 throughput LSL 8.5 GiB/s/core (target 10.2), set in the
  PRD charter before the leaf architecture was known.
- **Refutation.** `48-lsl-rederivation.md:25-63`: first-principles NEON
  ceiling — Case A (no lane amortization) 1.1-3.1 GiB/s/core, Case B (4-way
  amortization) 4.6-12.5 GiB/s/core; "The 8.5 LSL as written is unsatisfiable;
  it is a spec error, not an implementation gap" (:63). Five FAIL receipts
  retained verbatim as the refutation record:
  `receipts/gate-blake3-throughput.md:33` (0.43 GiB/s < 8.50, exit 1),
  `receipts/gate-ctq-report.md:44,50` (0.43 / 1.18 vs 8.5 -> FAIL),
  `receipts/gate-ctq-report-v2.md` (same, reproduces),
  `receipts/gate-cpk-study.md:35` (n=100 mean 1.85, sigma 0.01, Cpl -254.8),
  `receipts/gate-cpk-study-v2.md` (n=100 mean 1.846241, sigma_overall
  0.010571, CV 0.57% — process stable, limit wrong).
- **Disposition.** RATIFIED wave-12 by operator decision 1
  (`docs/OPERATOR_DECISIONS.md:24-44`): LSL 1.0 / target 1.5 / USL 2.5
  GiB/s/core, per-core NEON, executed as edit set A/A2/C/D/E/F
  (`LSL_RATIFIED = Some(1.0)` at `crates/bench/src/bin/ctq_report.rs:45`;
  PRD row ratified at `20-prd.md:75`). Post-flip receipts:
  `receipts/gate-ctq06-ratified-pass.md` (4 consecutive ctq_report runs, all
  verdict=PASS, exit 0) and `receipts/gate-cpk-study-v4.md:35` (CTQ-06
  C_pl 12.245 PASS at LSL 1.0; the loaded-machine value still clears the 1.67
  target 2.9x). The old-LSL FAIL receipts stay in the corpus, never deleted.
- **Doctrine lesson.** A limit set before the architecture that must achieve
  it is a hypothesis, not a requirement — and a gate that keeps FAILing
  honestly at an unsatisfiable limit is what makes re-derivation lawful.

## F-2: >= 74 GiB/s sustained decode — NOT CONFIRMED (65.6 measured, 11% short)

- **Claim.** Sustained mmap-scan decode >= 74 GiB/s (charter
  `18-dflss-charter.md:22,38`; spec `24-parquet-projection-spec.md:63,102`).
- **Refutation.** `receipts/parquet-throughput-baseline.md:34` — PAYLOAD-SUM
  7 rounds, min 65.6 GiB/s, mean 64.8 (checksum ffffffffe4000000) on a
  load-shared M3 Max under concurrent wave lanes (:48-49); against the 74
  bar that is 11% short. Later windows under load re-read 65.6 -> 63.7
  (`perf/perf-20261030-2236.md:31`, `perf/perf-20261030-2238.md:48`).
- **Disposition.** NOT confirmed. The 74 target text is kept intact and the
  measured status attached by amendment (`24-parquet-projection-spec.md:132,
  140, 151`); registered PARTIAL in `04-benchmark-contract.md:152-153,164`,
  marked "NOT confirmed — 65.6 measured, 11% short" in
  `19-press-release.md:100`, carried as open item 2 in
  `60-release-readiness.md:190`, and stated as bus-bound at the measured
  figure in `21-ard.md:69-70,192`. No threshold was rounded down.
- **Doctrine lesson.** Register the divergence; never round the threshold to
  the measurement — and a baseline taken on a load-shared host bounds what it
  can confirm.

## F-3: 8-wide g4x2 interleave — REFUTED on assembly evidence (spill-bound)

- **Claim.** Doubling NEON interleave width to 8 chunks (g4x2) lifts the
  kernel ceiling toward the LSL.
- **Refutation.** Assembly: the candidate's prologue shows `sub sp, #1552` —
  a 1,552-byte stack frame, spill-bound by construction
  (`59-ratification-package.md:62-77`; `22-adr-register.md:540-549`). State
  arithmetic: 8-wide holds 16 state + 16 message v128 vectors — the whole
  32-register NEON file for state+message alone, 0 left for constants and
  temporaries (`59-ratification-package.md:71-75`). Profile: the
  `compress_chunks_4way` resident-kernel ceiling is bit-identical ns/batch
  before and after the interleave landed (`48-lsl-rederivation.md:125-140`).
- **Disposition.** REFUTED and reverted; kernel ceiling unmoved at
  1.36 GiB/s; 4-way remains the shipped width
  (`00-index.md:232-233`; ADR-15 wave-8 amendment).
- **Doctrine lesson.** Doubling SIMD width is not a lawful lever past a
  serial dependency chain — the disassembly is the court, not the intention.

## F-4: 8-way register-budget premise — RETIRED (superseded by state-spill)

- **Claim.** ADR-15's original Context arithmetic: 8 chunks need 8x chaining
  state plus 8x message words plus round constants — "the register budget
  cannot hold it, and spills would erase the interleaving gain"
  (`22-adr-register.md:444-451, 466-469`).
- **Refutation.** Two measured findings replaced the a-priori argument.
  Wave-7: LLVM had already folded the const-table permute; the 4-way round
  loop shows 0 spill-relative loads after the literal-index rewrite, at
  +0.2% — register pressure was never binding at 4-way
  (`22-adr-register.md:521-527`); the 4-way bottleneck is the serial g4
  dependency chain under shared NEON ALU throughput. Wave-8: the real 8-wide
  constraint is measured state spill (`sub sp, #1552`, F-3 above), not the
  register-count arithmetic.
- **Disposition.** Premise retired; the 4-way decision stands but on
  re-derived grounds — "the register-budget premise is dead for 4-way,
  confirmed live for 8-wide" (`22-adr-register.md:540-549`).
- **Doctrine lesson.** An a-priori feasibility argument can survive with the
  right conclusion for the wrong reason; keep it only after re-deriving it
  from measured evidence.

## F-5: NEON ZIP/UZP transpose — REFUTED-AS-UNBUILT (ADR-17)

- **Claim.** Spec 23 §3.1 named per-architecture transpose kernels (aarch64
  `ZIP1/ZIP2`/`UZP1/UZP2`); spec 27 §1 tabled `VZIP1/2Q`/`VUZP1/2Q`/`VTRN`
  with per-instruction cost estimates (~2 instructions per field column).
- **Refutation.** ADR-17 (`22-adr-register.md:583-621`): the V2 validation
  lane found none of it was ever built — `crates/avec-arrow/src/transpose.rs`
  carries no `cfg(target_arch)` NEON/AVX path (NEON lives only in
  `silicon.rs` and avec-blake3). What shipped is a fixed-stride scalar
  transpose, 19 branch sites, every one an ADR-14 remainder class. The cost
  table described a kernel with no receipt.
- **Disposition.** REFUTED-as-unbuilt. Scalar transpose kept (round-trip
  courts 5/5 green, adequate for the milestone); the NEON transpose is
  re-classed as an open optimization item; any future work re-derives from
  ADR-17 plus the branchless manifest, not the spec cost table; specs 23 §3.1
  and 27 §1 carry one-line pointer notes to ADR-17.
- **Doctrine lesson.** A spec that names an unbuilt kernel states a
  hypothesis about the codebase; validation refutes it by reading, and
  "not built" is a finding to register, not a gap to paper over.

## F-6: thrift slot-swap "writer pending" — LANDED

- **Claim (the refuted claim behind the pending item).** "crate::thrift
  frames interoperable pages" — REFUTED: DataPageHeaderV2 slots 4-7 swapped
  vs parquet.thrift, internally self-consistent, externally rejected.
- **Refutation of the defect, and the pending item.** Writer/reader/fixtures
  corrected to 4=encoding, 5=def_len, 6=rep_len, 7=is_compressed; thrift
  16/16 + page 31/31 green; the row carried "Fresh pyarrow receipt through
  the FIXED writer pending"
  (`docs/PHASE_CHANGE_RECEIPTS.md:16`, restated at :76).
- **Disposition.** LANDED — witnessed this session (lane 18, 2026-10-31) by
  re-execution on the integrated HEAD:

  ```text
  cargo test -p avec-parquet --features std --test aligned_interop -- --nocapture
  4 passed; 0 failed
  PYARROW-RECEIPT pyarrow=23.0.1 rows=256 type=fixed_size_binary[1] fidelity=EXACT
  PYARROW-RECEIPT pyarrow=23.0.1 rows=256 type=fixed_size_binary[1] fidelity=MISMATCH
  ```

  The EXACT receipt is minted through the current (fixed) writer; the
  corruption test still flips to MISMATCH, so the court is non-vacuous.
  The "pending" wording in `PHASE_CHANGE_RECEIPTS.md:16,76` is stale-by-
  default and is superseded by this observation (that file is outside this
  lane's write scope; owners should flip the wording at next touch).
- **Doctrine lesson.** Receipt-cited status is stale by default — re-execute
  the surface before carrying "pending" forward.

## F-7: parquet `--features std` collects-0-tests anomaly — RESOLVED (parse artifact + cfg gate)

- **Claim (the anomaly).** Census rows showed the parquet std-feature test
  surface collecting 0 tests, reading as a dead test surface (red-list item
  in the wave-6/7 verdict; `47-milestone-verdict.md:131-132`).
- **Refutation (of the anomaly as a real gap).** Two compounding causes, both
  benign: (1) the census parser attributed the "0 passed" doc-test result
  line to the std suite — a doc-test parse artifact
  (`47-milestone-verdict.md:131`); (2) the tests are cfg-gated on the `std`
  feature, so default-config runs legitimately collect 0
  (`72-census-v2.md:90-92`, "0 tests under default cfg").
- **Disposition.** RESOLVED. Lane I16 fix landed the feature gate
  (`crates/avec-parquet/tests/level_free_multi_court.rs:9-18`), the
  `std_feature_court` landed (2 tests under std, 1 under default, both
  configs exit 0 — `51-release-notes.md:165-168`), and census v2 counts the
  std surface at 204 passed across 10 suites
  (`72-census-v2.md:99-108`); `verify-latest.md` re-reads the std scan
  surface PASS on the integrated HEAD.
- **Doctrine lesson.** A census that can mis-parse a suite reads as a dead
  surface; attribute the artifact to the tool before filing it against the
  code.

## F-8: LEGACY-UNVERIFIED design-conversation numbers — RETIRED (never receipted)

- **Claim.** Conversation-cited purrdf/avec figures: 966k elem/s N3 parse,
  340 ns SHACL per-node, >= 1600 conformance tests, 5.3-5.5 GiB/s literal
  encode, 74-77 GiB/s DataPageV2 decode.
- **Refutation.** Premise-rot audit (wave 5): none has ever been re-run in
  this repo — all rows marked **LEGACY-UNVERIFIED**, retained as historical
  floors only, and barred from citation by any landing claim
  (`04-benchmark-contract.md:126-134`); the wave-5 extension records the two
  values tested against real benches, both unmet (parquet 65.6 vs 74;
  blake3 1.85 vs 8.5) (`04-benchmark-contract.md:148-155`).
- **Disposition.** RETIRED as evidence. Where a value was later re-measured,
  the divergence is registered, not papered over: literal encode page-write
  path rewired to the fast encoder and measured 5.472 GiB/s (throughput bar
  >= 5.0 MET; alloc bar <= 2 not fully met at 6)
  (`docs/PHASE_CHANGE_RECEIPTS.md:96-100`); the 74/74.8 decode figure stays
  NOT confirmed (F-2). The blake3 8.5 floor's refutation is F-1.
- **Doctrine lesson.** A design-conversation number is O (candidate), never
  O* — it earns standing only by being re-measured on the exact subject.

## Coverage law

Every falsified/retired target known to this lane is registered above; the
register is append-only hereafter — new refutations enter as F-9+, each with
claim, refutation evidence at file:line, disposition, and a one-line doctrine
lesson. A target missing from this register is UNKNOWN, not absent.

## Standing

PARTIAL_ALIVE — entries F-1..F-5 and F-7/F-8 are grounded entirely in
on-disk receipts and specs re-read this session; F-6's disposition carries a
same-session executed witness (aligned_interop 4/4, exit 0). No benchmark was
executed for F-1..F-5 by this lane; those cite prior receipts and are marked
as such.

See Also: 48-lsl-rederivation.md · 22-adr-register.md (ADR-15, ADR-17) ·
59-ratification-package.md · docs/OPERATOR_DECISIONS.md ·
receipts/parquet-throughput-baseline.md · docs/PHASE_CHANGE_RECEIPTS.md ·
04-benchmark-contract.md · 87-wave-12-13-receipt.md
