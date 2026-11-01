# doe-4-unifier-data-structure

- Charter source: `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md`
  (Section 4, Phase 3, item 4 — charter line 67).
- Conversion lane: `C14-doe-4-unifier-ds` (charter conversion engineer).
- Status: SPECIFICATION — instruments not yet landed. Every predicted number in
  this file carries standing `TARGET`, including the `> 1.0 GB/s` scan and the
  sub-microsecond pair-unify directionals. Nothing has been measured by this
  lane; execution standing is `UNVERIFIED` until a Divan receipt exists.
- Standing vocabulary: `ALIVE` (ran + passed this session, command + exit
  cited) / `TARGET` (directional nominal, unmeasured) / `PENDING-LAND`
  (instrument absent on disk; command specified, not run) / `UNVERIFIED`
  (default; never rounded up).
- Membrane law: `phase-2-measure-bench.md` (seed, black_box, flip court,
  receipt protocol). DOE shape precedent: `doe-1-atomic-portability-tax.md`.

## 1. Charter Quote (Verbatim Source of Truth)

> **DOE-4 (Deductive Unifier Data Structure):** Sweep const-generic arena
> capacities comparing `u64`-lane bitwise comparisons against bitset
> representations.

CTQ row feeding this DOE (charter Section 3, line 39): "Line-Speed Deductive
Reasoning ... `> 1.0 GB/s` sequential scan; sub-microsecond pair unify ...
Divan throughput table across const-generic sizes ($N \in [64, 1M]$)".
Both directionals stay `TARGET` here; the GB/s scan column itself is owned by
the A06/A11 scan bench lanes (Divergence D3 below).

## 2. Subjects Under Test (On-Disk; Read This Session)

Kernel subject: `crates/cgap-bn3/src/unify.rs` (read in full this session; no
bitset selection structure exists anywhere in the crate today). The comparator
sites that constitute the u64-lane arm:

| Site | Location | Operation |
| --- | --- | --- |
| Constant equality | unify.rs:600 | `w != const_pool[creg]` — full 64-bit lane compare |
| Self-join repetition | unify.rs:611 | `w != cols.col(fs)[row]` — lane compare, two slot columns |
| Join probe (hot loop) | unify.rs:825 | bound VALUE compare, early-exit `break` |

All three live in `atom_local_match_into` (unify.rs:585-624) and the
nested-loop join body (unify.rs:820-848). Current selection data structure —
the thing the bitset arm replaces — is `EvalScratch` (unify.rs:556-576):
`bound_flags: Vec<bool>` (byte per variable, unify.rs:559), per-candidate
bind spans `cand_spans`/`cand_binds` (unify.rs:562-564), cur/nxt double
buffers. Register ABI: `REGISTER_COUNT = 256` u64 lanes (unify.rs:103), a
2 KiB L1-resident file, band partition const-pinned (unify.rs:159-180). The
projector stages the same register file per match (`crates/cgap-bn3/src/
project.rs`, `emit_into`, lines 361-430).

Court witnesses on disk (present in the tree; NOT executed this session —
their run standing is `UNVERIFIED`): differential court vs the naive
recursive oracle over 100 witnessed corpora (unify.rs:1613; mutation classes
M12-1/3/5/8), M12-6 chunk-partition determinism court (unify.rs:1664),
1M-row closed-form stress court (unify.rs:1947), M12-7 popcount-invariant
projector court (project.rs:936).

## 3. Divergence Register (Charter vs Tree — read before measuring)

- **D1 — No const-generic arena exists.** The A11 arena slab
  (`crates/cgap-bn3/src/arena.rs`) is a Vec-backed flat statement store with
  no const-generic capacity parameter (read this session). The charter's
  "const-generic arena capacities" is realized BENCH-SIDE: a const-generic
  store-capacity `N` plus a Divan `consts` sweep. The kernel gains no new
  const generic; the charter wording yields to tree reality and the decision
  rule (Section 9) is unchanged in structure.
- **D2 — The landed arm IS u64-lane.** The charter names an A/B between two
  representations. Arm A (u64-lane value equality over byte-per-flag
  selection state) is the landed kernel; Arm B (u64 bitset selection masks)
  does not exist in the tree and lands ONLY inside the bench membrane as a
  real alternative implementation — no kernel edit, per the Phase 2 M-3
  kernel-purity law.
- **D3 — Metric ownership split.** The CTQ row couples scan throughput
  (`> 1.0 GB/s`) with unify latency. DOE-4's primary metric per dispatch is
  ns/pair; the GB/s scan column stays owned by the A06/A11 bench lanes and
  is carried here only as secondary context, never redefined.
- **D4 — Occurs-check model.** The A12 kernel binds flat first-occurrence
  variables; occurs-check semantics live in the differential court's naive
  oracle (M12-8 is that court's positive control, unify.rs:1610-1612). The
  invariant this DOE pins is the COURTED semantics (Section 7), not a new
  occurs-check implementation.

## 4. Hypotheses (All Magnitudes `TARGET`)

- **H1 (u64-lane dominance at sparse selectivity).** At selectivity < 1%
  (few matches per 64-row word), the u64-lane arm's early-exit lane compare
  (unify.rs:825 `break`) beats bitset mask construction: predicted ns/pair
  ratio BITSET/U64 of 1.1-2.0. Falsifier: a measured ratio <= 1.0 at sparse
  cells refutes H1.
- **H2 (bitset dominance at dense selectivity / large N).** At selectivity
  >= 1/64 with N >= 65536, 64-decisions-per-word masks plus
  `trailing_zeros` enumeration and `popcount` compaction win: predicted
  ratio 0.5-0.85. Falsifier: a measured ratio >= 1.0 at dense cells
  refutes H2.
- **H3 (capacity cliff location).** Both arms stay sub-microsecond ns/pair
  at every swept N (the CTQ directional), with the inflection where the
  store leaves L2: 65536 rows x 24 B ~= 1.5 MiB (L2-resident) vs 1048576
  rows x 24 B = 24 MiB (DRAM-bound). Predicted (TARGET): Arm A ns/pair
  ~2-6 at N=64, ~20-80 at N=1048576. Falsifier: any cell >= 1000 ns/pair,
  or a cliff not correlated with the L2 boundary, refutes H3.
- Point estimates are directional nominals only. The experiment exists
  because representation superiority is an empirical question (charter
  Phase 3 preamble); no hypothesis survives on argument alone.

## 5. Instruments (Witnessed Inventory)

| Instrument | Path / command | Status |
| --- | --- | --- |
| Kernel subject + courts | `cgap-bn3/src/{unify,project}.rs` | on disk; run `UNVERIFIED` |
| Divan membrane | `crates/bench/`, divan `0.1.21` dev-dep | exists; DOE-4 target `PENDING-LAND` |
| DOE-4 bench file | `crates/bench/benches/doe4_unifier_data_structure.rs` | `PENDING-LAND` |
| Bitset arm (Arm B) | bench-local module, generated per 6.2 | `PENDING-LAND` |
| Flip court | SHIM-DOE4 over `/tmp` sandbox copies (Section 8) | `PENDING-LAND` |
| Receipt | `docs/jira/v26.11.1/waves/bench-bn3-unifier-receipt.md` | `PENDING-LAND` |

## 6. Bench Construction

### 6.1 File, wiring, corpus, metric (`PENDING-LAND`)

`crates/bench/benches/doe4_unifier_data_structure.rs`, registered as
`[[bench]] name = "doe4_unifier_data_structure", harness = false` in
`crates/bench/Cargo.toml` (sole manifest edit; kernel manifests untouched).
Entry: `fn main() { divan::Divan::default().run(); }`. Consumed API is public
surface only: `cgap_bn3::unify::{RuleCompiler, StatementColumns,
unify_batch_into, EvalScratch, CompiledRule, BodyAtom, Rule}`.

Capacity sweep: `#[divan::bench(consts = [64, 1024, 65536, 1048576])]` over a
`const N: usize` store-capacity parameter (divan pin `0.1.21`, recorded in the
receipt). Corpus: canonical seed `0xBF58_476D_1CE4_E5B9`, golden-ratio
increment + `mix64` (phase-2 Section 4, character-exact constant), rule
classes mirroring `rule_set()` (unify.rs:1037-1073), with the
witnessed-corpus property (every rule fires at least once — the
`gen_witnessed_corpus` law, unify.rs:1217-1227) checked once per capacity
BEFORE timing; a violation refuses the bench (exit 1), never times a dead
corpus. Selectivity is a second factor at two pinned levels, sparse (~0.5%)
and dense (~20%), set by corpus generation constants and checksum-asserted
post-run.

**Metric definition (primary).** One PAIR = one (parent match, candidate
row) probe of the hot loop (unify.rs:820-832). Pair count per
(N, selectivity) is computed once from the corpus (`sum over join atoms of
cur_len x cand_len`), `black_box`-wrapped, checksum-asserted; ns/pair =
measured `unify_batch_into` wall time / pair count. Secondary columns:
derivations/s, the Divan allocation column (TARGET `0 B` after warm
`EvalScratch`, mirroring the zero-alloc court), and the Section 7 invariant
asserts (a gate on admissibility, not a column). Wrap every consumed output
in `std::hint::black_box`; assert final state after the loop.

### 6.2 Arm B: bitset twin (real alternative implementation, never a mock)

Arm B is a bench-local implementation of the SAME matching semantics with a
different selection data structure: per-atom candidate decisions packed one
bit per row into `u64` words; per-slot predicate masks AND-combined; match
enumeration by `trailing_zeros` walk; match count by `popcount`;
bound-variable consistency by AND-ing the candidate mask with per-variable
value masks before enumeration. Derivation law (doe-1 Section 6.2
precedent): generate Arm B from the canonical comparator semantics by
script into the bench crate — identical match ORDER (atom-0 row ascending,
candidate ascending; the unify.rs:626-632 contract) — never hand-written.
Drift check: the generation script re-run into `/tmp` MUST produce an empty
diff against the checked-in Arm B.

## 7. Invariants Named In Advance (admissibility gate BEFORE timing)

- **INV-1 Binding consistency.** One variable binds exactly one value per
  match; every bound-slot probe compares the bound VALUE
  (register-dereferenced), never the register index or slot position — the
  exact aliasing class Section 8 mutates. Witness: the `MatchSet` column
  alignment assert (unify.rs:683-687) plus INV-4.
- **INV-2 Occurs/self-join behavior preserved.** Within-atom repetition
  (`?x p ?x`) compares the row's own slot values (unify.rs:609-619); an
  unbound head variable stays a typed compile refusal
  (`UnifyError::UnboundHeadVariable`). The differential oracle's
  occurs-check semantics (M12-8) are the preserved reference.
- **INV-3 Capacity independence.** For a fixed corpus, the derivation set is
  IDENTICAL across all swept N (the M12-6 partition-independence law at
  bench scale), and match order is the 6.2 order at every N.
- **INV-4 Cross-arm equivalence.** Arm A match set == Arm B match set,
  row-for-row, at every (N, selectivity) cell — asserted on final state
  (checksum plus spot row readback) inside the bench before any timing
  column is admitted.

A timing table produced while any INV assert fails, or is skipped silently,
is `VACUOUS`; graduation is `REFUSED` (exit 1). A loud `[SKIP] <reason>` per
capacity is the only lawful non-measurement (exit 3).

## 8. Anti-Vacuity Mutation (Named In Advance)

**SHIM-DOE4 — binding-alias comparator.** In a `/tmp` sandbox copy of the
bench crate (never the shared tree), replace Arm A's bound-slot probe
(`cols.col(si)[j] != cur_vars[vidx][m]`) with an aliasing comparator that
compares the variable's REGISTER INDEX against the candidate lane value —
bind slots alias whatever sits in the compared word, so distinct bindings
collapse into false joins. Marked `SHIM-COURT` in the diff.

Required flips (BOTH must hold, else the bench is `VACUOUS`):

1. The closure/deduction court fails with rc=101: INV-4 cross-arm equality
   (and/or the witnessed-corpus closure equality re-run in the sandbox)
   breaks — a value-blind comparator cannot reproduce the oracle's
   derivations; the join and self-join rule classes both witness it.
2. The Divan ns/pair column MOVES (the comparator is on the hot path; an
   invariant column with a broken court means the column measures nothing).

Protocol (phase-2 Section 6, verbatim): BASELINE sha256 -> BEFORE run ->
SHIM -> AFTER run -> RESTORE sha256 byte-exact vs BASELINE; a restore
mismatch is a fail-closed abort and the court run is VOID. Exit contract:
0 pass / 1 gate fail (including VACUOUS) / 2 subject missing / 3 loud skip.
Kernel-side positive control already on disk: the M12-3 join-probe-swap
class (unify.rs:1443-1447).

## 9. Decision Rule

- Admissible run: all Section 7 invariants green at every cell, SHIM-DOE4
  flipped, min-of-5 process runs per cell; flat-tail claims cite Divan's
  per-iteration distribution; no `Process.sleep` anywhere.
- **ADOPT BITSET** iff Arm B ns/pair < Arm A ns/pair by >= 10% at BOTH
  N=65536 and N=1048576, with no cell regressing > 10% at any smaller N.
- **KEEP U64-LANE (status quo)** otherwise; the landed form stands and the
  bitset hypothesis is recorded refuted-at-host with the full table.
- **CAPACITY-GATED SPLIT** iff the sign of the delta flips across N:
  record the crossover capacity and gate the bitset form behind it
  (const-generic dispatch) — the same structure as DOE-1's GATE outcome.
- The 10% threshold is the noise band (TARGET; recalibrate from the spread
  across the first measured 5 runs — if spread exceeds 10%, widen the band
  to 3x spread and re-run before deciding; never decide inside noise).
- Any adoption or keep lands as a divergence entry in the executing lane's
  receipt; the kernel changes only through a named follow-up lane, never
  inside the bench membrane.

## 10. Receipt Template

```text
# bench-bn3-unifier-receipt (DOE-4)
lane: C14-doe-4-unifier-ds (executor: <agent>)
command: cargo bench -p bench doe4_unifier_data_structure
exit: <code>
divan_pin: 0.1.21
corpus_seed: 0xBF58_476D_1CE4_E5B9 (byte-verified in bench source)
table: <full Divan table text; all N x selectivity x arm cells>
invariants: INV-1..INV-4 per-cell verdicts (pass/fail cell list)
pair_counts: <per-cell black_box'd counts>
flip_court: SHIM-DOE4 <FLIP rc=101 | VACUOUS | [SKIP] reason>
sha256: baseline=<...> restore=<...> (byte-exact: yes/no)
decision: ADOPT BITSET | KEEP U64-LANE | CAPACITY-GATED SPLIT (rule cited)
standing_changes: TARGET -> MEASURED only for cells with table + flip pass
divergences: <D1-D4 status + any new>
```

Graduation discipline (phase-2 Section 7): a number graduates TARGET ->
MEASURED only inside this receipt, only with BOTH the table and the passed
flip court; code comments and docs keep TARGET standing.

## 11. Provenance

- Source: `00-dflss-charter-source.md` Section 4 Phase 3 item 4 (line 67);
  CTQ row (line 39); Phase 2 membrane law (lines 54-58); risk-matrix vacuity
  control (lines 101-105).
- Context read this session: `crates/cgap-bn3/src/unify.rs`,
  `crates/cgap-bn3/src/project.rs`, `crates/cgap-bn3/src/arena.rs`,
  `phase-2-measure-bench.md`, `doe-1-atomic-portability-tax.md`.
- No measurement was executed by this conversion lane; nothing here upgrades
  any claim past `TARGET`.

