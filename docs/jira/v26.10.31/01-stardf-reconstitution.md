# stardf Reconstitution: purrdf to ggen

Standing: PARTIAL — the TermId-arena SHAPE has landed in avec
(`crates/avec-arrow/src/layout.rs`: TermId = u32, Quad64
`repr(C, align(64))`, receipted via gate-arrow-tests and spec
23-64b-quad-spec.md) but under avec, not stardf; the ggen contract-graph
pipeline remains unexecuted. No contract artifact, template, or generated
target exists yet. Design record plus landed-shape pointer.

## Motivation

purrdf cannot be published: it is pinned to a rev-pinned git dependency and is sovereign
material. It has also been made branchless. Publishing is not the only route to reuse, so
the existing source is treated as transient intermediate material rather than an asset to
ship. What carries value is its contracts — API signatures, ABI shapes, memory discipline —
not its line-by-line text.

The proposal: reconstitute purrdf's API structures into ggen as machine-readable authority
(Turtle/SHACL), so clean-room replacements can be manufactured from the contract graph
without ever publishing raw source. Extract contracts, not ports.

## The 5-Link Invariant Chain

1. **No-read ingestion.** Dump purrdf's public API signatures, C ABI structs, memory
   boundaries, and turnstile preconditions into Turtle/SHACL — `purrdf_contract.ttl`
   feeding `stardf_contract.ttl`. Model the branchless invariants explicitly:
   constant-time operations, zero heap allocation during admission, and flat
   64-byte-aligned buffer layouts compatible with the avec Arrow substrate.
2. **ggen metamodel encoding.** SPARQL CONSTRUCT rules bind SHACL shapes to templates;
   parameterize TermId arenas, RDFC-1.0 canonicalization turnstiles, and avec Arrow
   ingestion pointers.
3. **Deterministic compilation** to freestanding targets — no_std Rust, C99, WASM —
   preserving branchless control flow end to end.
4. **Independent falsification.** Differential test vectors against the legacy purrdf
   harness produce receipts (`R_receipt`). A generated kernel is admitted only when its
   vectors match the legacy harness byte-for-byte.
5. **Sunset authorization** for the unpublishable source. Once the contract graph plus
   falsification receipts stand on their own, purrdf itself can be retired.

## Naming Decision

The name is **stardf** — `*rdf`: the Klein star / wildcard closure operator (`∗`) bound to
the admitted ontology `O*` of the operating doctrine (A = μ(O*), R = receipt(A)).

Rejected candidates, with etymologies:

- **bcrdf** — branchless C RDF, following the `bcinr` pattern. Descriptive but carries no
  closure semantics; reads as a build flavor, not an authority.
- **radix / radx** — root/namespace connotation; collides with the existing radix trie
  vocabulary and dates badly in search.
- **semc** — semantic compression shorthand; opaque, no binding to the doctrine.
- **sturn** — semantic turnstile; names one mechanism (the turnstile), not the whole
  reconstitution surface.

`stardf` wins because the star is the doctrine symbol for admitted observation: the
ontology is the `O*`, and the generated kernels are its projection.

## Skeleton Authority Schema

Sketch of the core authority class (Turtle, abbreviated):

```turtle
stardf:MechanicalAuthority a owl:Class ;
    rdfs:label "stardf mechanical authority" ;
    stardf:predecessor "purrdf" ;
    stardf:executionModel stardf:BranchlessConstantTime ;
    stardf:memoryModel stardf:ZeroAllocationArena ;
    stardf:downstreamSubstrate "avec" .
```

SHACL shapes constrain every `stardf:KernelOperation`:

- `stardf:hasConditionalBranches` — must be `false`.
- `stardf:dynamicAllocations` — must be `0`.

The TermId arena is defined as a first-class shape:

- 64-byte alignment on all arena buffers (avec Arrow compatibility).
- `u32` index width for TermId references.
- BLAKE3 fingerprint over arena contents (identity anchor).
- `ArrowArrayStream` interop surface for ingestion into avec.

Any generated kernel violating the branch or allocation constraints fails SHACL
validation and is refused before compilation — the graph is the gate, not the review.

## Standing

The design remains **UNVERIFIED** at the chain level: no `purrdf_contract.ttl`,
`stardf_contract.ttl`, template set, or differential-vector receipt exists as of this
document. The TermId-arena shape alone has landed under avec (see Standing at top).
It composes with composition primitive **G** (ontology -> code generation via
ggen), whose standing is ALIVE in the composition catalog — the substrate mechanism is
proven; this particular application is not.

Falsifier when executed: a generated stardf kernel whose differential vectors diverge from
the legacy purrdf harness, or a kernel that passes vectors while violating the SHACL
branch/allocation shapes — either falsifies the chain.

## See Also

- `02-architecture-planes.md`
- `19-ontology-ttl.md`
