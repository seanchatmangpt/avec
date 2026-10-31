# Release Checklist: AVEC v26.10.31

Executable landing checklist for the v26.10.31 milestone. Every box must be
closed by a receipt (command + exit + subject SHA), never by assertion. A box
that cannot be witnessed is open, and open boxes block the release — they do
not downgrade it.

## DMADV Gate Checklist

The milestone lands only when all five gates hold simultaneously. Each gate
names its owning evidence.

- [ ] **Define** — PRD/ARD landed: 20/21 docs in `docs/jira/v26.10.31/` present
      and cross-linked; the one open doc is named and carries an owner.
- [ ] **Measure** — Gage R&R receipt: timer/alloc variance < 1% of total
      measured signal, witnessed by a Divan run on the pinned baseline host.
- [ ] **Analyze** — CFG homology proof accepted; all DFMEA RPNs under their
      documented thresholds (or each exceedance carries a registered
      mitigation row, not a waiver).
- [ ] **Design** — kernels and formats conform to specs 23-28: quad layout,
      Parquet mapping, packfile, BLAKE3 content addressing, NIF boundary.
- [ ] **Verify** — full verification ladder green plus the machine report
      committed (see [31-verification-ladder.md](31-verification-ladder.md)).

## Implementation Checklist

Ordered and dependency-true: do not start a step before its predecessor's
receipt exists. FR references are the PRD functional requirements.

- [ ] Pin 64B quad layout + sentinels per spec 23 (layout doc is the single
      source; no code before the pin lands).
- [ ] Sentinel + alignment compile asserts in `avec-arrow` (static failure on
      any layout drift; assert must fail on an injected mutation).
- [ ] Parquet zero-level decoding + isomorphism assertions per spec 24
      (round-trip identity proven on real fixtures, not summaries).
- [ ] Packfile writer + `git fsck` court per spec 25 (FR-04): writer output
      passes real fsck on real object stores.
- [ ] 16-stage ladder + banned-opcode audit gate with positive control per
      spec 27 (FR-05): gate must refuse an injected banned opcode.
- [ ] BLAKE3 known-answer vectors + throughput gate per spec 26 (FR-06):
      vectors exact; throughput fence relative to a sleep-free baseline.
- [ ] NIF zero-copy pointer identity + allocation asserts per spec 28 (FR-07):
      pointer equality witnessed, allocation counts asserted to zero on the
      hot path.
- [ ] Divan suites + `divan2receipt` with non-vacuity witness per spec 29: a
      deliberately failing bench must fail the receipt pipeline.
- [ ] SHACL suite over TTL with mutation witness per spec 30: a shape
      violation injected into the graph must be caught.
- [ ] Machine report + all receipts committed per spec 31 (ladder detail:
      [31-verification-ladder.md](31-verification-ladder.md)).

## Documentation Closure

- [ ] Index doc updated to reference the full 20-39 series.
- [ ] Traceability matrix: every row closed, or loudly open with owner and
      blocking receipt — no silent skips.
- [ ] Press release remains internal-only; the do-not-publish note is honored
      and no external-facing artifact references it.

## Landing Mechanics (Coordinator-Owned)

- [ ] Per-work-item atomic commits, message via `-F <file>`, never inline `-m`.
- [ ] No worktrees; one canonical checkout at `/Users/sac/avec`.
- [ ] Lane build roots deleted before the final commit (leases, not assets).
- [ ] Receipts under `receipts/` carrying input SHAs, replayable from a clean
      checkout.

## Abort Criteria

Stop the landing and re-baseline if any of these fires. Aborts are typed, not
silent.

- [ ] Any gate that cannot be witnessed failing (vacuous gate) — the gate is
      the defect, fix the gate before the code.
- [ ] Any FR without an owning crate after the migration audit.
- [ ] Timer Gage failure — stop and re-baseline; never publish targets as
      receipts.

Standing: UNVERIFIED for every box until its receipt exists.

## See Also

- [31-verification-ladder.md](31-verification-ladder.md)
- [35-migration-v26.10.29.md](35-migration-v26.10.29.md)
- [18-dflss-charter.md](18-dflss-charter.md)

## Wave-6 Status Margin (2026-10-30, lane 17)

Superseded by [57-checklist-sweep-v5.md](57-checklist-sweep-v5.md) — sweep
chain 42 → 44 → 46 → 54 → 57; see also
[55-final-integration-report.md](55-final-integration-report.md).

Current state (wave 8): verify ladder 10/10 PASS; CTQ-06 FAIL (LSL under
PROPOSED amendment, 48-lsl-rederivation.md); C_pk study receipted-FAILED
(gate-cpk-study-v2, breach is CTQ-06 only).

- Verify gate: 10 of 10 gates PASS (exit 0) per
  `verify-latest.md` @ HEAD ead4032.
- `cli-verify`: PASS exit 0 — the wave-6 SKIP was a probe bug, fixed and
  re-witnessed (this corrects the historical 9-of-10 + SKIP lines).
- `audit-branchless`: exit 0 this session; hits reported honestly
  (transpose/perfect_hash branch sites) under the ADR-14 remainder
  policy (INFORMATIONAL mode, manifest-bounded).
- CTQ scoreboard: CTQ-02/04/05 PASS; CTQ-06 FAIL (0.43 / 1.85 GiB/s
  vs LSL 8.5), carried by the LSL-ratification decision.
- Receipts on disk: 36 receipts — 25 ALIVE / 10 FAILED / 1 PARTIAL
  (64-receipts-inventory-final.md).
- Remaining for milestone claim: CTQ-06 LSL ratification and the
  integration commit only — cli-verify and the gage receipts are done.
- Every checkbox above stays unchecked until its owning receipt exists.
