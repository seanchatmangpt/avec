# A11: BN3 Arena — crate `cgap-bn3` (PROPOSED)

- **Lane**: A11 of 20, Squadron 3, overnight-campaign doc fan-out, v26.11.1
- **Status**: PROPOSED (design lane; no `crates/cgap-bn3` exists yet)
- **Files in scope (PROPOSED)**: `crates/cgap-bn3/src/arena.rs`,
  `crates/cgap-bn3/src/sigil.rs`
- **Coordinator owns**: `Cargo.toml` workspace membership, any edit outside
  this lane.
- **Read-only receipt basis**: `/Users/sac/cgap/Cargo.toml` (workspace,
  edition 2021, `#![no_std]` convention); `crates/cgap-blake3/src/lib.rs`
  (hash substrate); `docs/jira/v26.11.1/lanes/A06-bont-scalars.md` (the
  scalar vocabulary BN3 payloads index into).

## 1. Context (ADR-003: index-interval arena vs pointer-chasing ASTs)

BN3 ("Bitsized N3") is the v26.11.1 execution representation for
Notation3-shaped logic. W3C N3 toolchains (EYE, cwm, N3.js) model a formula
as a tree: every triple holds pointers to subject/predicate/object nodes,
every `{ ... }` quotation is a child subtree, and a unifier walks the tree
through per-node indirection. Costs of that model:

- ~4 heap nodes per triple; no `Copy`; `Send` requires `Arc`; structural
  snapshot of "what is in this formula" is a full tree walk.
- Unification kernels are memory-latency-bound, not arithmetic-bound — the
  pointer chase IS the budget.

ADR-003 replaces the tree with a **flat arena**: a `BN3Statement` is exactly
24 bytes (`[u64; 3]`, one word per s/p/o position), all statements live in
one contiguous slab, and a quoted graph `{ ... }` is a `BN3FormulaHeader`
recording a **start/end index interval** into the same slab. No `Box`, no
`Vec`, no pointer anywhere in a term payload.

Squadron context:

- **A12 owns the unifier.** The `R0..R255` register file is defined here but
  *adjudicated* there: register usage is well-formed only after A12's unifier
  accepts it. This lane ships the data layout; A12 ships the binding
  discipline.
- **A06 owns the scalar vocabulary.** BN3 payloads are indices into A06's
  `SlotId`/`ClassId` space, not a re-derivation of it. BN3 is a consumer of
  the A06 ABI frozen in its Wave 1.

## 2. ADR + rejected alternatives

**Decision**: 24-byte `BN3Statement = [u64; 3]`; 8-bit sigil + 56-bit payload
per term; contiguous slab; quoted graphs as pointer-free index intervals.

1. **Rejected — W3C N3 pointer AST (`Rc`/`Box` graph nodes).** 4 allocations
   per triple, no `Copy`, cache-miss-dominated unification, structural
   equality costs a tree walk. Lane falsifier: if interval flattening cannot
   represent a real N3 program without semantic loss (Section 8), the arena
   is the wrong substrate and this ADR dies.
2. **Rejected — boxed dynamic terms (`Box<dyn Term>`).** Vtable indirection
   on the unifier hot path; non-`Copy`; the 24-byte invariant becomes
   unreachable by construction.
3. **Rejected — tag+payload fused in one `u64` with no explicit `Sigil`
   type.** Every touch pays a mask/shift before a `match`; the sign/semantics
   split of Section 4 hides inside bit twiddling at each call site. The
   explicit `#[repr(u8)] Sigil` decodes in one `match`.
4. **Rejected — `FORMULA_REF` into a side table of formula IDs with
   on-demand materialization.** Two representations of the same construct;
   the unifier must handle both term kinds; breaks "flat is the only
   representation".
5. **Rejected — 32-byte statement (4th word for metadata).** Breaks the
   24-byte exit gate and the 3-words-per-triple cache line arithmetic
   (2.67 statements/line; 2 with the 32-byte variant); metadata (formula
   intervals, arities) lives in headers, not per-statement.

## 3. Mermaid C4

**C4 context (level 1)**

```mermaid
C4_Context
  title BN3 lane context — v26.11.1
  Person(operator, "Operator", "Owns the portfolio; consumes court receipts")
  System(cgap_bn3, "cgap-bn3", "PROPOSED crate: 24-byte statements, sigil terms, arena")
  System(cgap_bont, "cgap-bont", "A06 crate: SlotId/ClassId scalar vocabulary")
  System(cgap_unify, "cgap-unify", "A12 crate: register-file unifier (authority on R0..R255)")
  System_Ext(w3c_n3, "W3C N3 toolchains", "EYE / cwm / N3.js — the rejected pointer-AST baseline")
  Rel(cgap_bn3, cgap_bont, "indexes payloads into")
  Rel(cgap_unify, cgap_bn3, "reads statements + intervals")
  Rel(operator, cgap_unify, "demands court receipts")
```

**C4 container (level 2)**

```mermaid
C4_Container
  title cgap-bn3 containers (proposed)
  ContainerDb(arena, "Statement arena", "Rust, [u64;3] slab",
    "Contiguous backing store; u32 statement indices")
  ContainerDb(ftab, "Formula table", "Rust, Vec<BN3FormulaHeader>",
    "Index intervals [start,end) into the arena; no pointers")
  Container(sigil, "sigil.rs", "Rust module",
    "Sigil enum + BN3Statement + encode/decode")
  Container(arenamod, "arena.rs", "Rust module",
    "Slab, BN3FormulaHeader, flatten + interval checks")
  ContainerDb_Boundary(b, "cgap-bn3 boundary") {
    Container(sigil, "sigil.rs", "Rust module", "Sigil + BN3Statement")
    Container(arenamod, "arena.rs", "Rust module", "Slab + formula intervals")
  }
  Rel(arenamod, arena, "owns")
  Rel(sigil, arena, "encodes terms into")
  Rel(ftab, arena, "addresses intervals of")
```

## 4. Bit-level layout

### 4.1 Term word (one of three in a `BN3Statement`)

Little-endian `u64`; bit 0 = least significant.

```text
  63            56 55                                        0
 +----------------+-------------------------------------------+
 |  Sigil (8 b)   |        payload (56 b)                     |
 +----------------+-------------------------------------------+
```

| Field | Bits | Type | Meaning |
|---|---|---|---|
| sigil | 63..56 | `Sigil` (`#[repr(u8)]`) | term kind tag |
| payload | 55..0 | `u56` | register/slot/index |

- Payload is stored as the **low 56 bits of a `u64`**; encode asserts
  `payload <= 0x00FF_FFFF_FFFF_FFFF` and ORs `sigil << 56`. Decode is
  `(word >> 56) as u8` and `word & 0x00FF_FFFF_FFFF_FFFF`.
- **Sign handling**: payloads are unsigned indices. There is no signed
  payload class; any future signed immediate gets its own sigil rather than
  stealing bit 55. Bit 55 is therefore always 0 in v1 and a decoder MAY
  assert it as a redundancy check.
- Endianness: in-memory layout is host-endian `u64`; the (future) wire/pack
  encoding is little-endian, matching A06's `.oabi` decision.

### 4.2 Sigil inventory

| Code | Name | Payload semantics |
|---|---|---|
| `0x00` | `SLOT_CONST` | A06 `SlotId` truncated to 56 bits; full id in A06 pack header |
| `0x01` | `VAR_UNI` | universally quantified variable register `R_n` |
| `0x02` | `VAR_EXI` | existentially quantified variable register `R_n` |
| `0x03` | `FORMULA_REF` | formula table index (the quoted graph `{ ... }`) |
| `0x04` | `BUILTIN_OP` | dense builtin opcode (dense index, not a `SlotId`) |
| `0x05..0xFF` | RESERVED | decoder MUST refuse; no forward-compatible silent widening |

Reserved codes are a **typed refusal**, not a fallback: an unknown sigil is
`BN3Error::UnknownSigil(u8)`, never "treat as const".

### 4.3 `BN3Statement`

```rust
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BN3Statement {
    pub s: u64, // term word: sigil<<56 | payload
    pub p: u64,
    pub o: u64,
}
static_assert!(size_of::<BN3Statement>() == 24);
```

`[u64; 3]` newtype; `repr(C)` so the FFI surface (cgap-ffi) can reinterpret
without a codec.

### 4.4 `BN3FormulaHeader` (quoted graphs)

```rust
#[repr(C)]
pub struct BN3FormulaHeader {
    pub start: u32, // inclusive statement index
    pub end: u32,   // exclusive statement index
    pub term_words: u32, // redundant size witness for the round-trip court
    pub _pad: u32,
}
static_assert!(size_of::<BN3FormulaHeader>() == 16);
```

A `{ ... }` quotation flattens to: its statements appended contiguously to
the slab in source order, plus one header. **Nesting flattens recursively**:
a formula inside a formula is a sub-interval of its parent's interval;
`FORMULA_REF` payloads are the *child* header indices. The invariant court
(Section 9) fails on any overlapping or mis-nested intervals.

## 5. Register file `R0..R255`

- 8-bit payload gives 256 registers. `VAR_UNI`/`VAR_EXI` payloads are the
  register number `n` in `R_n`; `R0..R255` is the full legal range.
- **A12 is the unifier authority** (lane A12): this lane guarantees the
  layout transports a register number; it does not own binding, occurs-check,
  or trails. Cite: `docs/jira/v26.11.1/lanes/A12-*.md` (A12's lane doc, when
  it lands) — until then this doc is the only register-layout authority and
  A12's doc supersedes it on arrival.
- Encoding note: a register payload must additionally carry quantifier
  scope. Scope is **not** in the 56 bits; it is an interval property: a
  variable register is scoped by the innermost formula interval containing
  its statement. This is what makes flattening lossless for N3 scoping.

## 6. Encoding/decoding pseudocode

```text
encode_term(sigil, payload) -> u64:
    assert payload <= 0x00FF_FFFF_FFFF_FFFF
    assert sigil_is_known(sigil)            # reserved codes refused here
    return (u64(sigil) << 56) | payload

decode_term(word) -> Result<(Sigil, u56), BN3Error>:
    s = (word >> 56) as u8
    if s > 0x04: return Err(UnknownSigil(s))   # typed refusal, no fallback
    p = word & 0x00FF_FFFF_FFFF_FFFF
    return Ok((Sigil(s), p))

flatten(graph, slab, ftab) -> Result<(), BN3Error>:
    base = slab.len()
    for stmt in graph.statements:            # in source order
        slab.push(encode_all_three_terms(stmt))
    push_formula_header(ftab, start=base, end=slab.len())
    for child in graph.quoted_subgraphs:     # recursion = nested flatten
        assert slab.len() >= child_base      # children always after parent
        flatten(child, slab, ftab)
    verify_intervals_disjoint_and_nested(ftab)  # or Err(IntervalOverlap)

to_u56(a06_slot: u64) -> u56:
    return a06_slot >> 8        # high 56 of the 64-bit SlotId
```

Decode is total over reserved codes (returns `Err`), never panics — the
`#![no_std]` crate has no panic path on the decode boundary.

## 7. Expressibility boundary (typed refusals)

What does **not** fit in v1, each with its refusal type:

| N3 construct | Why it does not fit | Refusal |
|---|---|---|
| Literals with payloads > 56 bits | payload budget | `BN3Error::PayloadOverflow` |
| > 256 live registers in one scope | 8-bit register number | `RegisterExhausted` |
| Quoted graph in predicate slot | `p` bans `FORMULA_REF` (A12) | `FormulaInPredicate` |
| Formula ref to a non-ancestor scope | interval nesting check | `BN3Error::IntervalOverlap` |
| `@forSome`/`@forAll` with > 256 names in a formula | register file width | `RegisterExhausted` |
| Quotation nested > 64 levels | depth cap, `no_std` stack safety | `FlattenDepthExceeded` |
| Unknown/reserved sigil codes | forward-compat silence banned | `BN3Error::UnknownSigil(u8)` |

Every boundary is a typed error, never a lossy coercion. This table is the
falsifier surface for the refusal court (Section 9).

## 8. Chicago test plan (real slab, real round-trips; no mocks)

Nothing here is infeasible in-process — every court runs against a real
encoded arena. No doubles needed; the mock-ban exception never triggers.

**Size court** — collaborator: a real `BN3Statement` value.

- Asserts: `size_of::<BN3Statement>() == 24` (the exit gate) and
  `size_of::<BN3FormulaHeader>() == 16`.
- Mutation (MUST flip to fail): widen `s: u64` to `u128`, or add a 4th
  field.

**Round-trip court** — collaborator: encode/decode over all 5 sigils.

- Asserts: decoded `(sigil, payload)` equals input at payload bounds 0 and
  `0x00FF_FFFF_FFFF_FFFF` for every sigil.
- Mutation: renumber `SLOT_CONST` from `0x00` to `0x01`, colliding with
  `VAR_UNI` — MUST fail.

**Flatten court** — collaborator: a real nested `{ { ... } }` program
flattened into a real slab.

- Asserts: child interval ⊆ parent interval; siblings disjoint; every
  payload ≤ the 56-bit max (no pointer payload class exists).
- Mutation: bump one header's `end` by +1 so it overlaps a sibling — MUST
  fail with `IntervalOverlap`.

**Refusal court** — collaborator: the real decoder fed reserved-code words.

- Asserts: every Section 7 row returns its exact typed error.
- Mutation: replace one `Err` arm with a fallback-const coercion — MUST
  fail.

**Scope court** — collaborator: a real flattened `@forAll`-inside-`{ }`
program.

- Asserts: variable scope resolves to the innermost containing interval.
- Mutation: swap the `VAR_UNI`/`VAR_EXI` codes — MUST fail.

Non-vacuity discipline: each mutation is a one-line shim applied to a real
temp copy, run, and restored byte-exact (sha256 before/after); a court that
cannot flip under its mutation is a VACUOUS GATE and fails the lane.

## 9. Definition of done

1. `crates/cgap-bn3` exists with `arena.rs` and `sigil.rs`; workspace builds
   with the crate's `#![no_std]` convention intact.
2. Size court green: `size_of::<BN3Statement>() == 24` (exit gate),
   `size_of::<BN3FormulaHeader>() == 16`, both `repr(C)`.
3. Round-trip court green over all 5 sigils at payload bounds 0 and
   `0x00FF_FFFF_FFFF_FFFF`.
4. Flatten court green: nested quotation flattens to disjoint, properly
   nested index intervals; zero pointers in payloads.
5. Refusal court green: all 7 Section 7 rows return their exact typed error.
6. Scope court green; A12 cited as the unifier authority for `R0..R255`
   semantics.
7. All five anti-vacuity mutations of Section 8 demonstrably flip their
   courts (mutation run receipts recorded, restores sha256-verified).
8. This doc's ABI table (Sections 4.2-4.4) marked **Wave 1 ABI freeze** and
   co-signed by A01, A06, and A17 representatives in the v26.11.1 Wave 1
   freeze ticket.

## 10. Risks

- **56-bit payload ceiling.** Slot truncation (`SlotId >> 8`) drops 8 bits of
  A06's 64-bit `SlotId`; collision vetting must move to the truncated space
  or full IDs must live in the A06 pack header. Decision needed before Wave 1
  freeze; until then `SLOT_CONST` payloads are PROVISIONAL.
- **Formula nesting depth.** Recursive flatten is capped at 64; deep
  machine-generated N3 could hit it. Mitigation: explicit typed refusal plus
  a depth counter in the flatten court, not a stack overflow.
- **Register pressure.** 256 registers per scope; deeply nested `@forAll`
  lists exhaust. Typed refusal today; widening is a Wave 2 ABI break, so the
  exhaustion court must exist from day one.
- **ABI freeze coupling.** Sections 4.2-4.4 freeze in Wave 1 with A01/A06/
  A17; any post-freeze sigil renumber is a cross-lane break. Mitigation:
  reserved codes refuse loudly so renumbering cannot happen silently.
- **Truncated-SlotId unification semantics** depend on A06's collision
  vetting being defined over the truncated space; if A06 vetts over 64 bits
  only, `SLOT_CONST` is unsound. Cross-lane dependency, tracked as a Wave 1
  freeze precondition.

## 11. See Also

- `docs/jira/v26.11.1/lanes/A06-bont-scalars.md` — scalar vocabulary
- `docs/jira/v26.11.1/lanes/A12-*.md` — unifier (register authority)
- ADR-003 (this doc, Sections 1-2) — arena vs pointer ASTs
