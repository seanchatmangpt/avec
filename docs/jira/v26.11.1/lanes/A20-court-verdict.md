# A20: Court Verdict Lane — 1M-Row Stress Suite, Anti-Vacuity Mutation Gate, Campaign Merge Plan

Lane: A20 of 20, v26.11.1 overnight campaign. Repo: `/Users/sac/cgap` (single
canonical checkout; no worktrees, no per-lane copies, no git commands from
this lane). Wave 4 window: 06:30-08:00 (merge-and-seal). File ownership: this
document only; the artifacts it specifies (`tests/court_verdict.rs`,
`scripts/boundary_gate.py`) are PROPOSED — no code is delivered by this lane.

## 1. Status and Standing

- Lane doc: WRITTEN (this file).
- `tests/court_verdict.rs`: PROPOSED. `scripts/boundary_gate.py`: PROPOSED.
- Exit gate (design-level, enforced by the artifacts themselves once
  implemented):
  1. 100% pass of the `court_verdict` stress suite.
  2. >=98% mutation kill rate over the non-equivalent mutant corpus.
  3. `boundary_gate.py` confirms 0 external dependencies and 0 cloud
     strings across the covered crates.
- Every claim below is a design claim (UNVERIFIED until executed). Nothing
  in this file asserts executed verification, consistent with the milestone
  charter standing rule (`../00-index.md`, "Standing").

## 2. ADR: Why a Verdict Lane Exists

### 2.1 Decision

A20 is deliberately the last lane of the campaign and produces no feature
code. Its deliverable is the machinery that decides whether the other 19
lanes' claims survive adversarial pressure: a Chicago-school stress suite
(real collaborators, real data, asserted final state) plus a mutation gate
that proves the suite can fail.

### 2.2 Why (forces)

- Correctness must be decoupled from generator reliability. Every lane
  above A20 generated artifacts and self-reported. A fleet whose generation
  rate exceeds its execution rate accumulates entropy; the countermeasure is
  a court that kills, not a coordinator who pre-thinks. Any campaign defect
  caught by human/coordinator inspection rather than a court marks the wave
  as decoration.
- Tautological-test hazard: a suite that only re-asserts what its own setup
  constructed carries zero bits. The mutation gate is the structural answer:
  a court that cannot be made to fail is vacuous, and a vacuous court must
  fail the gate loudly rather than pass silently.
- Saturation definition adopted by this campaign: a full wave returning zero
  defects. A20's exit is the operational form: all courts pass at 100%,
  the mutant kill rate is >=98%, and the boundary gates hold — i.e., one
  full adversarial vibration over all 19 lane surfaces returns nothing.

### 2.3 Rejected alternatives

- Fuzzing-only (`cargo-fuzz` class): rejected as the primary court because
  it explores input space but does not prove the assertions detect
  behavioral change; mutation testing does the latter directly. Fuzzing
  remains a Wave-5 candidate complement.
- Coverage-percentage gates: rejected as the headline metric — line coverage
  can be 100% while assertions are vacuous. Kill rate measures assertion
  strength, coverage measures reachability; only the former is the gate.
- Coordinator manual review as the verdict: rejected per fan-out-first
  falsifier (defects found by pre-thinking, not courts, decorate the wave).

## 3. C4 Context: The Verdict Lane Consumes All 19 Lane Surfaces

```mermaid
C4Context
  title A20 verdict lane — context (Wave 4, merge-and-seal)
  Person coordinator as "Campaign coordinator"
  System verdict as "A20 verdict lane
  tests/court_verdict.rs
  scripts/boundary_gate.py"
  System_Ext w1 as "W1 ML-KEM surface (KAT courts)"
  System_Ext w2 as "W2 ML-DSA surface (KAT courts)"
  System_Ext w3 as "W3 SLH-DSA surface (conditional on p01)"
  System_Ext w4 as "W4 FN-DSA surface (conditional on p01)"
  System_Ext a02 as "A02 parquet-mmap"
  System_Ext a03 as "A03 git-merkle"
  System_Ext a04 as "A04 triumvirate isomorphism"
  System_Ext a05 as "A05 power-cut WAL (cgap-storage)"
  System_Ext a06 as "A06 BoNT scalars"
  System_Ext a07 as "A07 dependency/string gates"
  System_Ext rest as "A08-A19 lane surfaces"
  coordinator -> verdict: runs courts + gate at 06:30
  verdict -> w1: consumes KAT corpus + public APIs
  verdict -> w2: consumes KAT corpus + public APIs
  verdict -> w3: consumes iff admitted
  verdict -> w4: consumes iff admitted
  verdict -> a02: consumes read path over real Parquet pages
  verdict -> a03: consumes Merkle proof surface
  verdict -> a04: consumes isomorphism property surface
  verdict -> a05: consumes recovery classifier per truncated offset
  verdict -> a06: consumes scalar kernels
  verdict -> a07: its output is A20's gate #3 substrate
  verdict -> rest: consumes public APIs only
```

## 4. C4 Container: The Mutation-Testing Loop

```mermaid
C4Container
  title Mutation loop — mutate, run courts, record kills
  Container boundary_gate as "boundary_gate.py
  orchestrator"
  Container suite as "tests/court_verdict.rs
  1M-row stress courts"
  ContainerDb mutants as "mutant corpus
  per-kernel mutations, sha256'd"
  Container ledger as "kill ledger (JSON)
  mutant id, killed-by, verdict"
  boundary_gate -> mutants: baseline sha256 -> enumerate
  boundary_gate -> suite: cargo test --test court_verdict (per mutant)
  suite --> boundary_gate: pass/fail per court
  boundary_gate -> ledger: killed = suite FAILED under mutant
  boundary_gate -> mutants: restore exact bytes (sha256 fail-closed)
```

## 5. The 1M-Row FIBO/AR Settlement + BN3 Closure Stress Scenario

The headline court is a deterministic, end-to-end, 1,000,000-row settlement
corpus exercising the full read/verify/admit path across lane surfaces, with
a BN3 (boundary-negative class 3) closure battery appended.

### 5.1 Data generation (determinism is load-bearing)

- Generator: a hand-rolled SplitMix64/xoshiro-style PRNG (zero external
  deps — same discipline as the substrate) seeded with a fixed constant,
  `0xA20C0FFEE5A7E000`, committed in the test source. No entropy, no clocks,
  no filesystem order dependence.
- Schema: FIBO-aligned settlement rows (instrument id, notional, rate,
  settlement date, book, counterparty) expressed as the repo's generated
  arrow/parquet types (see A02 surface), plus AR (accounts-receivable)
  aging buckets derived per row; BN3 closure rows are degenerate/edge rows
  (zero notional, extreme dates, duplicate keys) that must be *admitted and
  correctly classified*, not crashed on.
- Scale: exactly 1,000,000 rows, generated in a single deterministic pass
  into an in-memory Arrow table and (for the A02 court) a real Parquet file
  under a per-run temp dir, deleted `on_exit` in the same test file (lane
  build-root cleanup law: a test that mints a unique-per-run artifact must
  delete it itself).
- Determinism receipts: the generator emits the corpus sha256 before the
  courts run and asserts it against a pinned constant. If the pinned hash
  and regenerated hash diverge, the court fails before any assertion runs —
  the corpus itself is a receipted subject.

### 5.2 Power-cut interleave with lane A05

The stress suite consumes A05's `cgap-storage::atomic_commit` recovery
classifier (see `A05-powercut-wal.md` §3: checksummed, sequence-numbered,
double-buffered journal; per-offset deterministic truncation). The
interleave protocol:

- For the 1M-row commit transaction, the court enumerates truncation
  offsets `k` on a deterministic stride (full sweep over the journal region;
  stride chosen so total runtime stays inside the Wave-4 budget — the
  stride is recorded in the kill ledger, never hidden).
- At each `k`, the court: (1) commits rows `0..m1` into journal slot A,
  (2) begins a second transaction for rows `m1..1M` interleaved into slot
  B, (3) truncates both slots at the adversarial offsets, (4) invokes the
  public recovery function, and (5) asserts: the prior root always
  classifies cleanly, ZERO corruption is observable through the public read
  path, and every admitted row under the recovered root re-verifies.
- No sleeps anywhere. Deterministic truncation IS the observation (A05 §2:
  time is not a proxy for state).

### 5.3 Allocation assertions — real CountingAlloc API (read this session)

Allocation discipline is asserted with the existing shared primitive in
`crates/audit/src/alloc_counter.rs` (read in full for this lane doc). The
real API surface A20 uses:

- `cgap_audit::CountingAlloc` — a `GlobalAlloc` pass-through over `System`
  already installed as `#[global_allocator]` in that crate's test binary
  (crates/audit/src/alloc_counter.rs:39-40).
- `cgap_audit::with_alloc_tracking(f) -> (T, AllocReport)` — scoped,
  per-thread tracking; nested scopes fold into the enclosing scope so an
  outer report covers all its allocations while an inner report excludes
  the outer's baseline (alloc_counter.rs:127-153).
- `cgap_audit::AllocReport { count: usize, bytes: usize }` with
  `AllocReport::is_zero()` (alloc_counter.rs:107-121).
- Process-wide `TOTAL_ALLOCS` / `TOTAL_ALLOC_BYTES` atomics for cross-thread
  totals (alloc_counter.rs:19-21), per the multithreaded test pattern at
  alloc_counter.rs:202-226.

Asserted invariants over the 1M-row pipeline:

- The admission path over the settlement rows allocates a bounded,
  pinned budget: `report.count <= ROWS / 1024` (tunable constant, recorded
  in the ledger), asserted via `with_alloc_tracking` around the hot segment.
- The per-row verify loop over the recovered root is zero-alloc:
  `with_alloc_tracking(|| verify_all(&root)).1.is_zero()`.
- Note on the `#[global_allocator]` coupling: `CountingAlloc` is installed
  per-crate-test-binary. If `court_verdict.rs` needs the counting allocator
  process-wide, A20 proposes the test binary declare
  `#[global_allocator] static COUNTING: cgap_audit::CountingAlloc` (the
  struct is `pub`, the type is reusable) — the exact integration is an
  implementation detail marked UNVERIFIED.

## 6. The Mutation-Gate Protocol (scripts/boundary_gate.py, PROPOSED)

### 6.1 Loop, in full

1. **Baseline**: sha256 every mutable source file (per-kernel `lib.rs`
   modules across the lane-covered crates). Record the manifest.
2. **Before**: run `cargo test --test court_verdict` unmutated. Record PASS.
   A baseline failure aborts the gate (no mutation campaign over a red
   baseline).
3. **Mutate**: apply one mutation per candidate — operator set: constant
   tweak (`+1/-1`, boundary `<`/`<=` swap, literal zero/one flip), boolean
   negation, comparison-operator swap, early-return removal, mask/shift
   constant corruption in the branchless kernels. Each mutant is identified
   as `(file, line, operator, index)` and its post-mutation sha256 is
   recorded.
4. **Run courts**: full `cargo test --test court_verdict` under the mutant.
   Compilation failure counts as killed (still a detected behavioral
   difference for gate purposes, but recorded in its own class).
5. **Kill recording**: `killed = (test binary FAILED) OR (build FAILED)`.
   Killed-by is the first failing court name. Survivors are recorded with
   their passing court list.
6. **Restore**: rewrite the file from the baseline copy; verify sha256
   byte-exact. A restore mismatch aborts the gate with a non-zero exit
   (fail-closed) — no further mutants run on a corrupted tree.
7. **Report**: JSON ledger of every mutant, verdict, and killed-by, plus
   the kill rate.

### 6.2 Kill-rate definition (exact)

- Numerator: the count of mutants with verdict `KILLED` (build-fail or
  test-fail classes both included).
- Denominator: the count of mutants classified `NON_EQUIVALENT` — i.e.
  total generated mutants minus `EQUIVALENT`-classified survivors.
- `kill_rate = |KILLED| / |NON_EQUIVALENT|`; gate passes iff
  `kill_rate >= 0.98`.
- Survivors are triaged by hand into: `EQUIVALENT` (provably
  observation-identical; excluded from denominator with a one-line
  justification in the ledger) or `SURVIVOR_DEFECT` (a real assertion gap;
  each one is a defect row against the owning lane, and a wave with any
  untriaged survivor has not saturated).

### 6.3 The gate's own anti-vacuity (shim-court pattern)

A mutation-free run must FAIL. Mechanism, exactly the smalt shim-court
protocol (baseline -> before -> shim -> after -> restore, sha256
fail-closed):

- **Baseline**: sha256 the target kernel file.
- **Before**: courts pass on the clean tree (expected outcome recorded).
- **Shim**: plant a known-killable mutant (a fixed, committed mutation —
  e.g. flip a comparison in a scalar kernel). Mark the ledger row
  `SHIM-COURT`.
- **After**: run the courts under the shim; require >=1 kill.
- **Restore**: sha256 must match baseline byte-exact or the gate exits 1.
- Verdict: if the kill count is 0, the gate REFUSES with exit 1 and the
  typed reason `VACUOUS GATE` — the court cannot detect a deliberate
  behavioral change, so every pass it ever produced is void. The
  non-vacuity check runs first, every invocation, before any real mutant
  campaign.

### 6.4 Dependency and string gates (tie to lane A07)

Gate #3 reuses A07's dependency/string audit outputs rather than
re-deriving them (single kernel, N bindings). A20's contribution is the
enforcement wiring:

- **0 external dependencies**: the gate fails unless the A07 audit (method:
  `../v26.10.31/82-dependency-audit.md`, E=0 runtime externals per covered
  crate) reports zero runtime externals for every crate in its coverage
  set, including the PROPOSED `cgap-storage` (A05) and any PQC crates
  admitted by p01.
- **0 cloud strings**: the gate greps the covered sources and manifests for
  cloud-vendor strings (`aws`, `azure`, `gcp`, `s3.`, `blob.core`,
  `amazonaws`, plus the A07 register's list as canonical) and fails on any
  hit outside the A07 allowlist.
- Division of labor: A07 owns the register and allowlist; A20 owns the
  fail-closed enforcement at campaign seal time. A20 must not maintain a
  second allowlist (O(N^2) drift law).

## 7. Merge Plan — All 19 Lanes (Wave 4, 06:30-08:00)

Conflict-ownership matrix by crate; "owner" is the only lane whose diff may
touch that path during the merge window. Everyone else rebases their
consumption onto the owner's resolution. Coordinator executes git
transitions; lanes never do.

| Path / crate | Owner | Conflict class | Notes |
|---|---|---|---|
| `crates/cgap-arrow`, parquet read path | A02 | data-format | mmap flags are A02-only |
| `crates/cgap-git` | A03 | packfile format | merkle proofs |
| isomorphism property tests | A04 | test files | consumes A02+A03 APIs read-only |
| `cgap-storage` (PROPOSED) journal | A05 | new crate | owns recovery classifier |
| scalar kernels | A06 | branchless manifest | join via observed_mnemonics |
| dep audit register + allowlist | A07 | gate inputs | A20 consumes, never edits |
| W1/W2 ML-KEM/ML-DSA surfaces | W1/W2 lanes | KAT corpora | byte-parity receipts |
| W3/W4 surfaces | p01-gated | conditional | deferred rows recorded, not silent |
| benches/, alloc budgets | A20 | constants | budget constants live with the court |
| `tests/court_verdict.rs`, `scripts/boundary_gate.py` | A20 | gate files | sole writer this lane |
| `docs/jira/v26.11.1/**` | per-lane files | docs | no cross-lane doc edits |

Sequence: (1) owners resolve their crates; (2) A20 runs the non-vacuity
shim-court; (3) full stress suite; (4) mutant campaign; (5) boundary gates;
(6) coordinator commits per-lane atomically; (7) Morning DoD sweep (§9).

## 8. Risks

- **Merge-day risk (highest)**: 19 lanes landing on one checkout in a
  90-minute window; the conflict matrix above is the mitigation, and the
  compile-freeze SLA (~10 min, owner fixes or reverts) applies to every
  lane edit that breaks the shared build. Compile-abort is reported as its
  own class, never counted as a test failure (census discipline).
- **Court runtime risk**: the 1M-row suite plus a per-mutant full-court run
  multiplies runtime by mutant count. Mitigations: stride-limited power-cut
  sweep (§5.2), mutant sampling recorded in the ledger (never silent), and
  per-mutant `cargo test` scoping to the affected crate's courts where the
  mutant's blast radius permits — recorded per row so the ledger stays
  honest.
- **Equivalent-mutant flood**: branchless arithmetic attracts semantically
  redundant mutants (wrapping adds, mask equivalences). The triage duty in
  §6.2 is the mitigation; an unbounded equivalent class means the operator
  set is too coarse for that kernel — record and refine, do not pad the
  denominator.
- **Allocator coupling**: `CountingAlloc` is a test-binary-level
  `#[global_allocator]`; multi-crate measurement may need one consolidated
  test harness crate (marked UNVERIFIED, §5.3).

## 9. DoD — Campaign Morning Definition of Done

The campaign charter's five items, expanded to objectively checkable
commands with expected exits. All run from `/Users/sac/cgap` at seal time.

1. **All lane courts pass.**
   `cargo test --test court_verdict` -> exit 0, 0 failed.
2. **Mutation gate at threshold with proven non-vacuity.**
   `python3 scripts/boundary_gate.py --mutate --report ledger.json` ->
   exit 0, `kill_rate >= 0.98` in the report; the report's first row is a
   `SHIM-COURT` kill (non-vacuity), and a run of
   `python3 scripts/boundary_gate.py --shim-court-only` on a court corpus
   with kills disabled -> exit 1 `VACUOUS GATE`.
3. **Zero external dependencies.**
   `python3 scripts/boundary_gate.py --deps` -> exit 0, E=0 runtime
   externals for every covered crate (A07 register as input).
4. **Zero cloud strings.**
   `python3 scripts/boundary_gate.py --strings` -> exit 0, no hits outside
   the A07 allowlist.
5. **Deterministic corpus receipt.**
   The 1M-row generator's pinned sha256 matches the regenerated corpus
   hash inside court run #1's output (asserted in-test; failure fails the
   suite before any other court).

A wave is sealed only when all five commands return their expected exits in
one continuous session against the merged head — saturation is a full wave
returning zero defects, not a summary claiming it.

## 10. Wave 4 Disposition

This lane executes in Wave 4 (06:30-08:00, merge-and-seal): the artifacts
`tests/court_verdict.rs` and `scripts/boundary_gate.py` are specified here
as PROPOSED and land with the merge, owned by A20; the verdict run itself
consumes every other lane's sealed surface. If any lane's surface is absent
at seal time, its courts are recorded as loudly missing (`[SKIP] <reason>`
in the ledger) — never silently skipped, never faked as a pass.

## See Also

- `../00-index.md` — campaign charter, standing rules, wave structure.
- `A05-powercut-wal.md` — power-cut WAL design consumed in §5.2.
- `A03-git-merkle.md`, `A02-parquet-mmap.md`, `A04-triumvirate-isomorphism.md`
  — consumed read-path surfaces.
- `../../v26.10.31/82-dependency-audit.md` — dependency-audit method (A07).
- `crates/audit/src/alloc_counter.rs` — real CountingAlloc API cited in
  §5.3.
