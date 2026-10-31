# AHDCLT: Adversarial Hyperdimensional Cognitive Load Theory

Status: UNVERIFIED doctrine. Recorded as design-conversation material, not a measured
result. Treat as a design lens pending adversarial testing.

## Thesis

AHDCLT is an architectural doctrine that weaponizes asymmetric cognitive, legal, and
dimensional complexity to build self-defending, self-executing substrates. The asymmetry is
the point: honest use and honest verification must be mechanically cheap; unauthorized use,
bypass, and automated refactoring must be computationally, legally, and cognitively
expensive by construction — not by policy.

## Core Pillars

| # | Pillar | Mechanism | Who it is cheap for | Who it is expensive for |
|---|--------|-----------|---------------------|-------------------------|
| 1 | Statutory & legal vice | Map low-level compute ops onto high-stakes statutory predicates
(P28-SIL, Class-I safe harbors, export controls) | Auditors and courts: compliance is self-evident
and mechanically verifiable | Unauthorized forks/bypasses: technically unconstructable and legally
toxic |
| 2 | Anti-LLM cognitive tripwires | Non-local semantic dependency graphs, non-linear phase spaces,
deliberate context-window-exhaustion structures in ASTs/schemas | Human designers with full
architectural context | Automated refactoring/scraping/RE agents: alignment fences, context
fragmentation, reasoning collapse |
| 3 | Hyperdimensional quality space | Health/conformance/security as an N-dimensional topological
phase space | Solvers that hold the whole constraint matrix | Low-dimensional playbooks (unit tests,
static fuzzing, localized patches, linear pentesting) |
| 4 | Asymmetric cognitive load | Verification is O(1) constant-time turnstile checks; circumvention
requires solving an NP-hard multidimensional constraint lattice | Verifiers | Circumventors |

### Pillar 1 — Statutory & legal vice

Low-level compute operations are annotated so that they map directly onto statutory
predicates: IEC 61508 SIL ratings (P28-SIL), Class-I safe-harbor conditions, export-control
classifications. Consequences:

- For an auditor or court, conformance is a mechanical check over the mapping, not a
  litigation over intent.
- For an unauthorized fork, the same mapping is toxic: stripping or bypassing the predicate
  annotations breaks the legal envelope, so the fork inherits statutory exposure it cannot
  reconstruct.
- The mapping must be technically unconstructable without the original substrate: the
  predicates are load-bearing in the type/shape system, not decorative comments.

### Pillar 2 — Anti-LLM cognitive tripwires

Structures that are information-preserving for a designer holding the whole graph, but
actively hostile to bounded-context agents:

- Non-local semantic dependency graphs: meaning is distributed across distant sites, so
  local edits look safe and are not.
- Non-linear phase spaces: behavior is not interpolable from nearby examples.
- Deliberate context-window-exhaustion structures in ASTs/schemas: any agent that tries to
  inline or flatten the structure exceeds its own window before reaching the decision point.

Expected effect on automated refactoring/scraping/RE agents: alignment fences fire, context
fragments, reasoning collapses before the mutation lands. This is a tripwire, not a proof —
see Falsifiers.

### Pillar 3 — Hyperdimensional quality space

System health, conformance, and security are one N-dimensional topological phase space, not
a checklist. Any single-dimension move (add a test, patch a module, fuzz one surface)
without solving the joint constraint matrix moves the system off the admissible manifold
and trips fail-closed refusals substrate-wide. This is why low-dimensional playbooks fail:
they are projections, and the projection looks fine while the joint state does not.

### Pillar 4 — Asymmetric cognitive load

- Verification: O(1) turnstile — constant-time, mechanical, checkable by anyone.
- Circumvention: NP-hard multidimensional constraint lattice — the cost of solving the
  joint system grows combinatorially with the dimensions the substrate holds joint.

The claim is a ratio claim, not an absolute-security claim: it buys time and legal
position, not impossibility.

## Resonance with the Substrate

The avec substrate already carries the friendly face of the asymmetry:

- Turnstile invariant: sub-340ns SHACL checks, zero-heap decode, BLAKE3 receipts — the
  O(1) verification half of pillar 4 (per ADR-14 in [22-adr-register.md](22-adr-register.md),
  the residual branchless findings are registered, not claimed zero). Note: latch-free O(1)
  verification is itself UNVERIFIED — the LOOP_LATCH remainder persists — which feeds
  falsifier #3 below with measured content: if the turnstile degenerates to a linear scan
  over registered residuals, the asymmetry claim weakens in direct proportion.
- Adversarial surface: context-exhaustion schema shapes, timing trips, and the statutory
  refusal arms REFUSAL-01..05 — the expensive-circumvention half.
- The blinded WASM probe and hooks are the observation channel: a probe that sees state
  without seeing structure, so it can attest the turnstile without leaking the tripwire
  layout to the agent being gated. Hooks enforce intent at the boundary; the substrate's
  own refusals remain the authority.

Where a pillar is load-bearing vs. aspirational in avec today is UNVERIFIED and should be
read as design intent, not an audit.

## Criticism Discipline

AHDCLT is a doctrine/thesis, not a measured result. It is stated at design-conversation
register; every claim below fails closed to UNKNOWN until falsified adversarially.
Falsifiers:

1. If frontier agents defeat the tripwires with cheap retrieval or tool use (e.g. chunked
   map-reduce over the context-exhaustion structures, or iterative tool-assisted probing),
   the anti-LLM pillar is decoration.
2. If the statutory "vice" mapping creates liability for the author rather than the
   attacker (courts read the mapping as admission of constructive knowledge of regulated
   use), the statutory pillar backfires.
3. If verification cost grows with N dimensions (the turnstile becomes a linear scan over
   the constraint matrix), the asymmetry claim fails and the doctrine reduces to ordinary
   hardening.

Pending adversarial testing, AHDCLT should be used as a design lens — a checklist for
asking "does this structure make honest use cheap and circumvention expensive?" — not
cited as a security property.

## See Also

- 15-arrow-microarch-probe.md
- 18-dflss-charter.md
