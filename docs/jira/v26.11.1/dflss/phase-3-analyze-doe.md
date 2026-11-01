# phase-3-analyze-doe

- Charter source: `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` (section 4,
  Phase 3, items 1-4)
- Conversion lane: `C08-phase-3-analyze-doe`
- Artifact class: dispatch-ready phase prompt (hand verbatim to each Analyze lane)
- Standing: every predicted number in this file and its DOE specs is `TARGET` until a
  measured Divan column exists; `PENDING-LAND` marks instruments absent on disk today.
  Nothing here is `MEASURED`; nothing here rounds up to `ALIVE`.

## 1. Arbitration Law (Phase 3 in one paragraph)

Four controlled comparative experiments (DOE-1 through DOE-4) let empirical hardware
numbers arbitrate architecture instead of assumption. Faith does not vote: no variant
is "obviously better", each DOE is an A/B experiment with a named falsifier, a numeric
threshold, and a decision rule executed only on measured columns. Measurement
arbitrates; the losing arm is killed by its court, and a doctrine-losing arm that wins
on specific widths or selectivities adapts the kernel with the divergence recorded as
doctrine-learning (rule R3), never argued away.

## 2. Dispatch Contract Block (paste into every DOE lane prompt)

- Repo: `/Users/sac/cgap` — ONE canonical checkout; no worktrees, clones, or copies.
- Never run git state-mutating commands; the coordinator owns all git transitions.
- Write lease: EXACTLY the output files your dispatch names; all else read-only.
- Chicago discipline: real collaborators, assert final state, no mocks.
- Courts mutate `/tmp` sandbox copies with sha256 fail-closed restore, never the tree.
- No `Process.sleep`; poll observables with bounded deadlines.
- Exit codes: 0 pass / 1 gate fail / 2 subject missing / 3 loud skip.
- A court that cannot flip is FAIL; a skip is loud (`[SKIP] <reason>`), never silent.
- Status claims carry command + exit from this session; UNVERIFIED stays UNVERIFIED.

## 3. The Four DOEs (one-line summaries; the full spec file is authoritative)

### DOE-1 — 32-Bit Atomic Portability Tax
- **Question (one line).** Does the machine-word (32-bit-cell) Chase-Lev index carry a
  per-operation tax > 25% versus the 64-bit `AtomicI64` path — i.e., is
  `cfg(target_has_atomic)` conditional-compilation gating warranted?
- **Spec (authoritative):**
  `docs/jira/v26.11.1/dflss/doe-1-atomic-portability-tax.md` (lane C11; on disk).
- **Falsifier.** Any sweep cell with `TAX = t_mw / t_i64 > 1.25` refutes H1 and forces
  the `GATE` decision. Predicted point `TAX ~= 1.05` is `TARGET`.

### DOE-2 — RLE Decode Memory Topology
- **Question (one line).** Does direct-into-caller buffer decoding beat staged
  intermediate-vector decoding across bit widths 1-32 — or does staged win on specific
  widths, forcing a per-width kernel adaptation?
- **Spec (authoritative):**
  `docs/jira/v26.11.1/dflss/doe-2-rle-decode-memory-topology.md` (`PENDING-LAND`).
- **Falsifier.** A width regime where the doctrine-losing arm measures faster while the
  kernel stays uniform: the divergence must be recorded (R3) and the kernel adapted.

### DOE-3 — Penstock Branch Architecture
- **Question (one line).** Does branch-free bitwise predicate filtering beat
  early-exit conditional branching across varying selectivity distributions — and
  where is the selectivity crossover, if any?
- **Spec (authoritative):**
  `docs/jira/v26.11.1/dflss/doe-3-penstock-branch-architecture.md` (`PENDING-LAND`).
- **Falsifier.** A single fixed choice measured dominant across all selectivities
  while the kernel ships both arms ungated, or a crossover claimed without a
  measured regime map.

### DOE-4 — Deductive Unifier Data Structure
- **Question (one line).** Across const-generic arena capacities, do `u64`-lane
  bitwise comparisons beat bitset representations for 24-byte statement unification —
  and at which capacities does the winner flip?
- **Spec (authoritative):**
  `docs/jira/v26.11.1/dflss/doe-4-unifier-data-structure.md` (`PENDING-LAND`).
- **Falsifier.** A representation measured dominant at every swept capacity while the
  kernel hardcodes the loser, or a capacity sweep that never actually compiles both
  arms (compile-abort is not a measurement).

## 4. Binding Rules (every DOE lane, no exceptions)

- **R1 — Hardware numbers decide.** The measured Divan column is the only arbiter. A
  predicted decision (`TARGET`) carries zero authority once measurement exists; it is
  confirmed or killed, never defended.
- **R2 — `TARGET` until measured.** Every number in section 3 and in the DOE specs
  stays `TARGET` until its instrument runs on this host with a cited command + exit.
  Promoting a predicted value into kernel docs or receipts without a measurement
  witness is `REFUSED` material.
- **R3 — Divergence-recording (doctrine-learning).** When the doctrine-losing variant
  wins on specific widths (DOE-2), selectivity regimes (DOE-3), or capacities (DOE-4),
  the kernel adapts to the measured winner and the divergence is recorded in the DOE
  spec's divergence register as doctrine-learning. A recorded divergence is a permanent
  entropy sink; an argued-away one recurs every wave.
- **R4 — Generate-and-kill.** Courts kill candidates; coordinators do not pre-think.
  Each DOE names the court that kills its losing arm before results are admissible. A
  defect caught by coordinator pre-thinking instead of a court marks the wave
  decoration (fanout-first falsifier), not a success.
- **R5 — Anti-vacuity measurement (charter section 4, Phase 2, inherited).** Each DOE
  benchmark undergoes one deliberate structural degradation in a `/tmp` sandbox (e.g.,
  a heap vector injected into decoding, a branch added to the hot path). If the
  measured column fails to shift, the benchmark is `VACUOUS` and the `TARGET` to
  `MEASURED` graduation is `REFUSED`.
- **R6 — Kernel adaptation is a typed transition.** An R3 adaptation lands as a kernel
  diff whose court flips: the mutation witness must show the old arm losing or the new
  arm failing without the fix. A silent heuristic with no flip witness is not an
  adaptation; it is drift.

## 5. Kill Criteria Summary (courts, not coordinators)

- **DOE-1**: `TAX > 1.25` in any cell (`GATE` wins, `cfg` gating lands), or the
  mutation court cannot flip.
- **DOE-2**: staged loses everywhere yet a staged path ships, or a width divergence is
  adapted with no register entry.
- **DOE-3**: no selectivity regime map exists for a crossover claim, or the losing arm
  ships ungated.
- **DOE-4**: arms never compile at identical capacities, or the sweep is asserted, not
  measured.

## 6. Status Registry (this artifact, at write time)

- `docs/jira/v26.11.1/dflss/doe-1-atomic-portability-tax.md` — on disk, read this
  session; its own registry carries the instrument inventory (`PENDING-LAND` items).
- `docs/jira/v26.11.1/dflss/doe-2-rle-decode-memory-topology.md`,
  `doe-3-penstock-branch-architecture.md`, `doe-4-unifier-data-structure.md` — absent
  on disk at write time (`PENDING-LAND`); the pointers above are their canonical paths.
  Converting lanes must create exactly those files.
- Phase 2 membrane (`crates/bench`, Divan 0.1.21) — specified, not landed; see
  `docs/jira/v26.11.1/dflss/phase-2-measure-bench.md`.
