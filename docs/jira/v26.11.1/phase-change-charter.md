# Phase-Change Charter — Three Admitted Transitions (v26.11.1)

Status: CHARTERED 2026-11-01. Enacted by the NEXT mega workflow (after
`wf_f175cd34-71f` completes + backfill + bench campaign). Companion to
`bench-campaign-charter.md` and the campaign charter (five gates unchanged).

Law: metrics WITNESS, never promise. A phase change is claimed only
retrospectively from an armed cost curve going vertical. Maximize search
space before selection; the falsifiers stay armed whether or not the
discontinuity comes.

## The three transitions

### 1. AM->FM: strings -> binary slots (representation change)

Information moves from a dimension coupled to the noise (allocator, cache,
parse — amplitude domain) into one orthogonal to it (fixed-width slots,
constant time, no panic path). Phase change = the transition becomes
irreversible: strings never re-enter the runtime.

- Diode: `scripts/zero_string_gate.py` (+ its self-check as non-rubber-
  valve proof). Already armed.
- Wave encoding: every receipt ends with `strings_admitted: int` — runtime
  string paths added by that receipt. Must stay 0; nonzero = falsifier
  fired = transition not happening.
- Witness curve: domain strings in `.rodata` per execution crate,
  monotonic -> 0 (nm/zero-string, already gated; Gate 2).

### 2. AC/DC: authoring -> projection (transformation change)

Trust transmits as the graph and rectifies locally. Phase change = every
artifact class in the repo is a byte-identical projection of the graph
(docs, receipts, benches, court corpora — same discipline as the CLI
six-pack). Catalog composition C11.

- Instrument (new, mechanical): `scripts/projection_coverage.py` —
  classify tracked files generated (declared regen path in a ggen manifest
  / patch script) vs hand-authored; replay every regen path; report %
  tracked bytes regenerable byte-identically. Exit contract 0/1/2/3.
- Wave encoding: each wave converts ONE artifact class from hand-edited to
  graph-projected.
- Falsifier: replay drifts from disk without a graph change = not
  projected (same falsifier as CLI regen byte-stability).

### 3. Steam: LLM effort -> compiled machinery (energy-source change)

Next-time intelligence needed ↓. Catalog composition C10: machinery share
of hops per cycle. Phase change = work-cost leaves the LLM budget
permanently.

- Instrument: machinery share = 1 − (agent-hops / contract-hops) per phase
  shape. agent-hops are read directly from journal.jsonl; contract-hops
  are fixed per phase shape (known from the dispatch contract). Receipt
  schema gains `executor: agent | gate | generator | court` for every
  transform named in the receipt.
- Wave encoding: a **Retirement** phase (see below) externalizes repeated
  transforms; the NEXT campaign replays the same phase shape with fewer
  agent() calls for the same contract.
- Falsifier: flat machinery share across campaigns = a sprint ran, not a
  phase change. Report PARTIAL on the steam transition; never narrate it
  away.

## Metric panel (picked BEFORE the run)

| Transition | Primary witness | Direction |
|---|---|---|
| AM->FM | .rodata domain strings + alloc symbols per execution crate; strings_admitted per receipt | -> 0, stay 0 |
| AC/DC | % tracked bytes regenerable byte-identically (projection_coverage.py) | rises per converted class |
| Steam | machinery share of hops (journal-derived, same-shape replay) | rises per campaign |
| (secondary) | tokens / admitted artifact | campaign-granularity ONLY — wave-to-wave is confounded by lane heterogeneity; demoted honestly |

## Retirement phase (between Saturation and Seal)

ERRC applied to cognition: delete/tighten outranks generate.

1. **Retirement audit** (court-auditor): scan every landed lane receipt for
   transforms that are (a) repeated >= 2x across lanes/waves, (b) of a
   known class (derivable software, deterministic verification, recurring
   observation), (c) currently LLM-only. Output: ranked retirement ledger
   with target form per item (SPARQL CONSTRUCT rule, ggen template, gate
   script, court corpus, generator).
2. **Retirement dispatch** (lane-runner): externalize each ledger item.
   Every retirement lands WITH its differential court:
   - template/rule: regenerate the artifact class from the graph,
     byte-compare vs the LLM-authored original (or typed delta list).
   - gate: replay the last wave's recorded inputs; verdicts must match the
     LLM auditor's.
   - anti-vacuity: mutate the machinery (one wrong token / one flipped
     verdict) -> the differential MUST catch it. A retirement whose
     differential cannot flip is decoration; the item returns to LLM-only
     and the ledger records REFUSED.
3. **Exit criterion (mechanical)**: recompute machinery share and
   projection coverage post-phase; both numbers land in the phase receipt
   with before/after values. No improvement = the phase still ran (the
   ledger is capital) but the steam witness says NOT-YET — honest.

## Enactment order (next mega workflow)

1. Baseline-Metrics: run projection_coverage.py (before conversions),
   read machinery share from `wf_f175cd34-71f` + prior journals, witness
   zero-string counts. All three baselines witnessed with exits, not
   narrated.
2. Bench campaign (per bench-campaign-charter.md).
3. Retirement phase (above).
4. Re-Seal: all five gates re-run after new capital lands (benches +
   retirements can break them; the re-run is the proof they did not).

## Structural honesty

The named historical phase changes were not planned as such; they were
the tail of a search process that kept the falsifier in view. This
charter arms the metrics and the retirement court; it does not claim the
discontinuity in advance. If the curves stay flat, the honest output is
witnessed flatness.
