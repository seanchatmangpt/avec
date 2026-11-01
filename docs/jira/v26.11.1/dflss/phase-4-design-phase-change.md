# phase-4-design-phase-change

Status: PHASE PROMPT, converted 2026-11-01 from
docs/jira/v26.11.1/dflss/00-dflss-charter-source.md Section 4, Phase 4
("The Three Phase-Change Discontinuities & Retirement"). The metric
panel lives in docs/jira/v26.11.1/phase-change-charter.md ("Metric
panel (picked BEFORE the run)"); this file CITES the panel and does not
duplicate it. Executing agents read that panel for the primary witness
and direction per transition; this prompt encodes the wave mechanics
that produce the curves.

Standing law: metrics WITNESS, never promise. Every number in this file
is TARGET or UNVERIFIED until a phase receipt under
docs/jira/v26.11.1/waves/ graduates it. Flat curves are reported as
witnessed flatness (PARTIAL / NOT-YET), never forced.

## 1. Agent contract (binds every executing agent)

- Repo: /Users/sac/cgap, the ONE canonical checkout. No worktrees, no
  clones, no copies.
- Never run git add/commit/push/checkout/stash; the coordinator owns
  all git transitions. Read-only git (status/diff) allowed.
- Lease = exactly the files your dispatch names. Write nothing else.
- No Process.sleep anywhere; poll observables with bounded deadlines.
- Exit-code contract for gates: 0 pass / 1 gate fail / 2 subject
  missing / 3 loud skip. A skip is loud (`[SKIP] <reason>`), never a
  fake pass.
- Courts mutate /tmp sandbox copies with sha256 fail-closed restore,
  never the shared tree. A court that cannot flip is FAIL; a
  mutation-free green gate run is VACUOUS.
- Standing vocabulary only: ALIVE / PARTIAL / BLOCKED / REFUSED /
  UNSUPPORTED / UNVERIFIED.

## 2. Inputs (existence witnessed 2026-11-01 via ls)

- docs/jira/v26.11.1/dflss/00-dflss-charter-source.md - EXISTS;
  Section 4 Phase 4 is the source converted here.
- docs/jira/v26.11.1/phase-change-charter.md - EXISTS; the three
  transitions, the metric panel, the retirement protocol, and the
  enactment order. CITE it; do not duplicate it.
- scripts/zero_string_gate.py - EXISTS (ls witness 2026-11-01: 109891
  bytes on disk). The AM->FM diode.
- scripts/projection_coverage.py - PENDING-LAND (ls witness
  2026-11-01: not on disk). Rules in section 4 govern until it lands.
- docs/jira/v26.11.1/waves/ - phase receipt home.

## 3. D-1: AM->FM diode lock (representation change)

Charter text: lock the diode ensuring zero runtime strings re-enter
read-only data segments, `strings_admitted: 0` per receipt.

- [ ] D1-1 Receipt field: every Phase 4 receipt ENDS with the line
  `strings_admitted: <int>`, counting runtime string paths added by
  that receipt. MUST be 0. Nonzero = the falsifier fired = the
  transition is not happening; land the receipt anyway (honest) and
  record the wave REFUSED for AM->FM.
- [ ] D1-2 Diode: scripts/zero_string_gate.py (EXISTS, ls-witnessed
  2026-11-01). It scans COMPILED ARTIFACTS, never sources, refuses
  (exit 1) on any vocabulary token in a fail-set section (.rodata /
  .text family on ELF/PE; __TEXT on Mach-O), and is fail-closed: no
  error path exits 0.
- [ ] D1-3 Non-rubber-valve proof: run
  `python3 scripts/zero_string_gate.py --self-check`. It really builds
  a poisoned fixture crate plus a clean twin with the real toolchain
  in a temp dir, scans both with the production predicate, and exits 0
  only if the poisoned arm HITs (naming the planted token) and the
  clean arm does not. A predicate that does not fire on poison exits
  nonzero as VACUOUS GATE. Cite command + exit in the phase receipt.
- [ ] D1-4 Witness curve: domain strings in .rodata per execution
  crate, monotonic -> 0 (Gate 2, nm-derived); already gated - cite the
  existing gate, do not rebuild it.

## 4. D-2: AC->DC projection (baseline BEFORE conversions, one class per wave)

Charter text: capture baseline byte-identical regeneration percentages
from the semantic graph and expand projected artifact classes.

- [ ] D2-1 Order is load-bearing: the baseline percentage is captured
  BEFORE any conversion. A baseline taken after the first converted
  class is void; the transition stands BLOCKED until a pre-conversion
  baseline exists.
- [ ] D2-2 Instrument: scripts/projection_coverage.py is PENDING-LAND
  (not on disk, ls-witnessed 2026-11-01). Phase 4's first construct
  item is landing it, mechanically per the phase-change charter:
  classify tracked files as generated (declared regen path in a ggen
  manifest or patch script) vs hand-authored; replay every regen path;
  report % of tracked bytes regenerable byte-identically; exit
  contract 0/1/2/3.
- [ ] D2-3 Honesty rule: while the script is PENDING-LAND the AC->DC
  baseline is UNVERIFIED. Never synthesize it from grep counts and
  never narrate it into existence; the item reports
  `[SKIP] projection_coverage.py not landed` until D2-2 lands, then
  runs for real.

## 5. D-3: Muscle->Steam retirement (energy-source change)

Charter text: audit the workflow ledger for transformations repeated
>= 2x within known problem classes; externalize these steps into
deterministic generators or static gates paired with differential
courts that flip on mutation; report flat curves honestly as witnessed
flatness rather than forced success.

- [ ] D3-1 Retirement audit (court-auditor): scan every landed lane
  receipt for transforms that are (a) repeated >= 2x across lanes or
  waves, (b) of a known class (derivable software, deterministic
  verification, recurring observation), (c) currently LLM-only.
  Output: a ranked retirement ledger with a target form per item
  (SPARQL CONSTRUCT rule, ggen template, gate script, court corpus,
  generator).
- [ ] D3-2 Retirement dispatch (lane-runner): externalize each ledger
  item WITH its differential court:
  - template/rule: regenerate the artifact class from the graph and
    byte-compare against the LLM-authored original (or a typed delta
    list).
  - gate: replay the last wave's recorded inputs; verdicts must match
    the LLM auditor's.
- [ ] D3-3 Anti-vacuity (mandatory per item): mutate the machinery
  (one wrong token, one flipped verdict) in a /tmp sandbox with sha256
  fail-closed restore; the differential MUST catch it. A retirement
  whose differential cannot flip is decoration: the item returns to
  LLM-only and the ledger records REFUSED.
- [ ] D3-4 Receipt schema: every transform named in a Phase 4 receipt
  gains `executor: agent | gate | generator | court`.
- [ ] D3-5 Steam witness: machinery share = 1 - (agent-hops /
  contract-hops) per phase shape; agent-hops read from journal.jsonl;
  contract-hops fixed by the dispatch contract.
- [ ] D3-6 Structural honesty: flat machinery share across campaigns =
  a sprint ran, not a phase change. Report PARTIAL on the steam
  transition; never narrate flatness away.

## 6. Exit criteria (mechanical; all mandatory)

- [ ] E-1 `strings_admitted: 0` is the final line of every Phase 4
  receipt.
- [ ] E-2 The --self-check non-rubber-valve run is cited in the phase
  receipt with command + exit.
- [ ] E-3 A pre-conversion AC->DC baseline number is landed, OR the
  item carries a loud `[SKIP] projection_coverage.py not landed` plus
  BLOCKED standing - never a synthesized number.
- [ ] E-4 The retirement ledger is landed with ranked items; each
  externalized item carries a differential court flip witness, or a
  REFUSED ledger entry.
- [ ] E-5 Post-phase recompute of machinery share and projection
  coverage lands before/after values in the phase receipt. No
  improvement = the phase still ran (the ledger is capital) but the
  steam witness says NOT-YET - honest.
- [ ] E-6 Metric verdicts are read from the metric panel of
  docs/jira/v26.11.1/phase-change-charter.md ("Metric panel (picked
  BEFORE the run)"); this file does not restate that panel.

## 7. Standing and falsifiers

- AM->FM falsifier: `strings_admitted > 0` in any Phase 4 receipt.
- AC/DC falsifier: a regen replay drifts from disk without a graph
  change, or conversions started before the baseline was captured.
- Steam falsifier: flat machinery share across same-shape campaign
  replays.
- Enactment order (charter): Baseline-Metrics -> bench campaign ->
  retirement phase -> Re-Seal. Phase 4 does not claim the
  discontinuity in advance: it arms the diode, takes the baseline, and
  runs the retirement court. If the curves stay flat, the honest
  output is witnessed flatness.
- [ ] D2-4 Wave encoding: each wave converts exactly ONE artifact
  class from hand-edited to graph-projected (docs, receipts, benches,
  court corpora - same discipline as the CLI six-pack byte-identical
  regen replay).
- [ ] D2-5 Falsifier: replay drifts from disk without a graph change =
  not projected (same falsifier as CLI regen byte-stability).
