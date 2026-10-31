# P28-SIL Compliance Mechanics: v26.10.31

Operational reading of the P28 Sovereign Invariant License for avec v26.10.31.
The machine-readable policy is the ODRL `SovereignInvariantPolicy_v1` in
19-ontology-ttl.md; this doc is the human-operational surface. Standing UNVERIFIED.

## License Surface

P28-SIL applies to avec v26.10.31 and its shipped artifacts. It rests on three
mechanisms.

1. **Safe harbor (below threshold).** An entity below the $50,000,000 threshold
   holds automatic, royalty-free, perpetual use. No notice, registration, or
   reporting duty. The comparison is pinned exactly under
   "Boundary-Value Pinning" below.
2. **Commercial Regularization Notice (at or above threshold).** An entity at
   or above the threshold that has used avec without a negotiated grant
   issues a Commercial Regularization Notice. Issuing one produces a
   cryptographic receipt (BLAKE3, native packfile) and simultaneously
   activates the Irrevocable Covenant Not to Sue: upon regularization the
   licensor surrenders the injunction and damages action for the regularized
   usage period.
3. **Enforcement surface.** P28-SIL does not litigate first. It routes through
   financial-reporting mechanics: FASB ASC Topic 450 (loss contingencies — a
   probable and reasonably estimable unasserted claim must be accrued and
   disclosed) and SOX 302/404 (officers certify internal control over that
   accrual). Unregularized at-threshold usage becomes a balance-sheet line
   and an internal-control finding, not a courtroom docket entry.

Design intent: the license makes compliance the cheapest path and litigation
the expensive one, without ever requiring a courtroom.

## Evidentiary Chain

The compliance mechanism depends on usage being non-deniable. BLAKE3 receipts
in native packfiles (25-git-packfile-integration-spec.md,
26-blake3-engine-spec.md) provide that. Usage of avec writes a content-addressed
receipt; the receipt chain makes "we never used it" untenable after the fact.

A receipt serves as evidence only if it fixes subject identity. Minimum fields:

| Field | Content | Evidentiary role |
| --- | --- | --- |
| `subject_identity` | Product id + checkout identity of the avec build used | Fixes which licensed artifact |
| `timestamp` | ns-precision, monotonic-ordered within an append-only packfile | Orders usage in time |
| `tool_version` | Exact avec version + kernel ABI revision | Pins capability and license surface |
| `input_sha` | BLAKE3 digest of the inputs consumed | Binds the receipt to real work performed |

Additional fields MAY be added; none of the four MAY be omitted. A receipt
missing any of the four is not evidence of usage under P28-SIL and is a defect
(DFMEA class: evidentiary gap).

## ODRL Encoding Cross-Reference

The normative policy is `avec:SovereignInvariantPolicy_v1` in
19-ontology-ttl.md. This document is the human-operational reading; where the
two disagree, the TTL wins and this doc gets a reconcile entry. Two
must-reconcile items exist today.

### Must-Reconcile: Threshold Measure Mismatch

The TTL constrains `avec:assetUnderManagement` (AUM). The charter table in
18-dflss-charter.md and the milestone brief speak of **annual gross revenue**.
AUM and annual gross revenue are different measures with different reporting
rules; the policy must pin one. Until reconciled, the effective measure is
undefined and the safe harbor is ambiguous for entities where the two diverge
(holding companies with small AUM and large revenue, and the reverse).

**Resolution rule proposed**: pin **annual gross revenue** (audited figure,
prior fiscal year) as the `odrl:leftOperand`, because it is uniformly reported
and auditable, and remap the TTL operand accordingly.

### Must-Reconcile: Boundary-Value Pinning

Per the TTL: the safe-harbor branch is `odrl:lt` 50,000,000 USD; the
regularization-duty branch is `odrl:gte` 50,000,000 USD. Pinned exactly:

- $49,999,999.99 → safe harbor: royalty-free perpetual use, no duty.
- $50,000,000.00 → NOT safe harbor; regularization duty applies.

The comparison is **≥, not >**: the duty branch is `gte`, so an entity at
exactly $50,000,000 is inside the duty branch, not the harbor. Any prose or
code saying "below $50M" means strictly-below. This doc and the TTL agree on
the boundary today; any future change to one must be mirrored in the other,
and the SHACL negative test suite (19-ontology-ttl.md, next-step candidates)
must include a `$50,000,000.00 exactly` boundary case that trips REFUSAL on
the permission branch. This item stays open only until the measure mismatch
above is resolved.

## EU AI Act Mapping Summary

Mapping of the P28-SIL substrate to the EU AI Act obligations this milestone
targets. All items are **self-assessed design intent**, not conformance: no
external conformity assessment exists for avec v26.10.31.

| Article | Obligation | avec substrate |
| --- | --- | --- |
| Art 12 | Logging requirement | Append-only ns-timestamped packfile logging |
| Art 14 | Human oversight | Policy/duty masks in the quad struct as oversight envelopes |
| Art 62 | Pre-deployment audits | Audits via open tools over read-only Git endpoints |

Caveat, stated plainly: until an external conformity assessment exists, this
mapping is a conformance-ready design claim, not conformance. It must not be
quoted as certified compliance in any customer-facing material.

## Open Items

| Item | Status | Note |
| --- | --- | --- |
| Canonical P28-SIL license text | OPEN | No canonical license file location is pinned in-repo. Candidate: `LICENSES/P28-SIL.md` at repo root. |
| Regularization receipt format | OPEN | Only the minimum schema above is pinned; full format (signing key, issuer identity, covenant scope) unspecified. |
| Dispute-test status | NONE | No dispute test exists; none has been run. |
| Threshold measure (AUM vs annual gross revenue) | OPEN | Must-reconcile with 19-ontology-ttl.md. |
| Risk-register linkage | OPEN | Superseded note removed: 33-risk-register.md exists and is the register of record; this table is a summary cross-referencing it. |

## DFMEA Summary

Failure modes and their P28-SIL controls, consistent with the charter table in
18-dflss-charter.md. Cross-reference: 33-risk-register.md (the register of
record — its DFMEA rows supersede this summary).

| Traditional Failure Mode | P28-SIL Control | Severity/cause note |
| --- | --- | --- |
| Willful infringement | Receipted usage makes intent arithmetic, not argument | Receipt chain converts "didn't know" into a fixed cost |
| Injunction risk | Covenant not to sue upon regularization removes the remedy | Regularization retroactively immunizes the regularized period |
| License phobia (adoption chilling) | $50M safe harbor: zero friction below threshold | Automatic; no registration step that could chill adoption |
| Evidentiary disputes | BLAKE3 content-addressed receipts; non-deniable | Minimum schema above is the admissibility floor |

---

Standing: PARTIAL. No receipt has been issued under P28-SIL; no dispute test
exists. Updated: the ODRL policy HAS been SHACL-validated —
receipts/gate-shacl-v3.md ALIVE (exit 0; validate_ontology.sh, anti-vacuity
mutant rejected). Still open: the threshold measure (AUM vs annual gross
revenue) remains unreconciled — 19-ontology-ttl.md still uses
`avec:assetUnderManagement` as `odrl:leftOperand` at lines 209 and 220.

## See Also

- 18-dflss-charter.md
- 19-ontology-ttl.md

## Instrument of Record (wave 13, 2026-10-31)

The P28-SIL v2.1.0 canonical text now exists in this repository:
[LICENSE-P28-SIL.md](../../../LICENSE-P28-SIL.md) (adapted from the
graphlaw canonical; URN `urn:avec:license:p28-sil:v2.1.0-sovereign`).
LICENSE.md carries the MIT code base + P28-SIL execution overlay
(graphlaw dual structure); workspace SPDX is MIT. The AUM-vs-revenue
measure reconciliation and the N-14 ODRL boundary-corpus items remain
the open items on this doc.
