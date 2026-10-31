# Cyclomatic Complexity M=1: Algebraic and Topological Formulation

Formal foundation for the branchless methodology: why M=1 is the correctness-and-performance
fixed point, how decisions become algebra, and what the compiler will and will not preserve.

Standing: formalization of design doctrine. The theorem proofs are standard-derived in
conversation; treat as internal design math, not peer-reviewed.

## 1. The mechanical wall as stochastic process

An execution is a trajectory through instruction phase space. At each branch the predictor
commits to one successor edge; the committed trajectory is either realized or flushed.

Let `p` = branch predictor accuracy, `D` = penalty depth (pipeline stages between fetch and
resolve), `τ_flush` = per-stage cost, `τ_retire` = steady-state retire cost. Expected cost per
dynamic branch:

    E[T] = τ_retire + (1 − p) · D · τ_flush

Entropy collapse makes the second term dominate. Let `H(σ)` be the entropy of branch-outcome
distribution σ at a site. Data-dependent branches on well-mixed inputs have `H(σ) → 1`, which
is maximum-uncertainty, so `p → 1/2`: the predictor is a coin flip and the speculative
penalty term `(1 − p)·D·τ_flush` is maximal. A branch is cheapest exactly when its outcome is
predictable; an M=1 program has no branches, so `E[T] = τ_retire` exactly, with zero
variance contribution from prediction.

Memory is the second wall, and it is non-Euclidean. Define latency distance `d(z1, z2)` not
by address difference but by cache topology:

- same 64-byte cache line: `d ≈ 0` (already loaded, overlapped)
- same prefetch stream, adjacent line: `d ≈ τ_L1 ≈ 1 ns`
- same 4 KB page, different stream: `d ≈ τ_L2 ≈ 3 ns`
- page/TLB boundary or cold region: `d ≈ τ_DRAM ≈ 60–100 ns`

Two addresses 8 bytes apart can be 100 ns apart if a page boundary separates them; two
addresses 4 KB apart can be 1 ns apart if the prefetcher covers both. Distance is a metric on
hardware topology, not on the address line — layout IS the algorithm's memory cost.

## 2. Homological McCabe

Cyclomatic complexity is a homology invariant, not a counting rule. Model a program as a
1-dimensional CW-complex `K = (V, E)`: vertices are basic blocks, edges are control transfers.

The chain complex over Z with the standard simplicial boundary:

    C₁ = Z^E ,  C₀ = Z^V ,  ∂₁(e) = v − u   (e: u → v)

Then `H₀` measures connectivity (one component per function) and `H₁` measures the cycle
rank — the independent loops in the flow graph. McCabe's M is exactly the first Betti number
of the *augmented* complex `K*`, where the augmented graph adds one virtual edge `(t, s)`
from the exit vertex `t` back to the entry vertex `s`:

    M = b₁(K*) = dim H₁(K*, Z)

Euler–Poincaré for the connected augmented graph gives the familiar counting form:

    M = |E*| − |V| + 1 = (|E| + 1) − |V| + 1 = |E| − |V| + 2

This is a theorem, not a definition: any control-flow structure's complexity is the dimension
of its cycle space, and the cycle space is what forces the hardware to speculate.

**The Unit Invariant Theorem.** `M = 1` if and only if the CFG is a single point or a
homeomorphic line segment (up to the cases `|V| = 1, |E| = 0` and the isolated-edge segment).

*Proof sketch.* (`⇐`) A point has `H₁ = 0`, so `b₁(K*) = 1` from the single virtual edge. A
line segment `s → v₁ → … → t` has `|E| = |V| − 1`, so
`M = (|V| − 1) − |V| + 2 = 1`. (`⇒`)
Use the out-degree sum. Every vertex except the exit has out-degree ≥ 1 in the augmented
graph, and each extra out-edge creates one independent cycle, so:

    Σᵥ (deg⁺(v) − 1) = |E*| − (|V| − 1) = M − 1

If `M = 1` the sum is 0, so `deg⁺(v) = 1` for every non-exit vertex: the graph is a directed
path from `s` to `t`. A directed path is homeomorphic to a closed interval (or collapses to a
point when `|V| = 1, |E| = 0`). Conversely any branch vertex has `deg⁺ ≥ 2`, contributing ≥ 1
to the sum, forcing `M ≥ 2`. ∎

So M=1 is precisely the class of programs with no decision geometry at all — the trajectory
through phase space is fixed before execution begins.

## 3. Algebraic linearization of decisions

A conditional assignment is a control-flow decision; an orthogonal projection is the same
decision as arithmetic. Over `Z/2^k Z`, for a predicate mask `Π_P` (all-ones when P holds,
all-zeros otherwise):

    x = Π_P · a + (I − Π_P) · b

This is a projection onto the subspaces spanned by `a` and `b` and is exactly `select(P, a, b)`
with zero control flow.

The mask itself is built from the two's-complement equality characteristic function. For
`u, v ∈ Z/2^k Z`, let:

    δ = u ⊕ v            (zero iff u = v)
    μ(δ) = (δ | −δ) ≫ (k−1)

Because `δ | −δ` has its sign bit set iff `δ ≠ 0` (any nonzero value or its negation is
negative), the arithmetic right shift smears the sign bit to a full-width mask: `μ` is
all-ones when `u ≠ v`, all-zeros when `u = v`. Invert or rearrange to get the equality mask.
Full-width all-ones / all-zeros masks compose with `&`, `|`, `^` as a Boolean algebra
isomorphic to the predicate algebra — decisions become ring operations, and ring operations
schedule without branching.

## 4. The straight-line search ladder

Classical binary search over `N` sorted keys is a decision tree of height `h = ⌈log₂ N⌉`.
Its CFG has one root, two edges per internal node, and one exit, giving:

    M = h + 2

and — the part the counting rule hides — `Var(T_search) > 0`: each lookup's latency depends
on the data, because each comparison is a data-dependent branch. Under §1, `H(σ) ≈ 1` on
random keys, so the predictor pays maximally and the cache trajectory is input-dependent
(timing side channel).

The straight-line ladder fixes `K` stages (here `K = 16` for a Git `.idx` v2 fanout-scale
table) and unrolls them into a recurrence with no branch vertices:

    p_k     = b_k + 2^k                                  (probe index at stage k)
    λ_k     = Π[ p_k < N ] · Π[ α_{p_k} < target ]       (stage advance condition, as masks)
    b_{k−1} = b_k + λ_k · 2^k

Each `Π[…]` is a full-width mask product (§3), `λ_k ∈ {0, 1}` realized as multiply-by-mask,
and the index update is an add. Every stage executes unconditionally; the data selects the
accumulated offset, not the program counter.

Topological audit: `|V| = 1`, `|E| = 0`, therefore `M = 0 − 1 + 2 = 1` — the Unit Invariant
case. Timing audit: exactly `K` probes, `K` compares, `K` adds regardless of input, so:

    Var(T_search) ≡ 0

Constant-time Git `.idx` lookup: no speculation penalty (`E[T] = τ_retire` exactly) and a
cache trajectory determined by the probe sequence, not the key.

## 5. Compiler honesty

M=1 is a property of the emitted machine code, not the source. LLVM's InstCombine and
SimplifyCFG passes legitimately re-branch certain `select` idioms — a source-level `if` with
arithmetic bodies is NOT M=1, and neither is a `select` the backend decides to lower as a
branch. The compiler optimizes for average throughput, not for the invariant; the invariant
must be enforced below the optimizer.

Enforcement: write the hot kernel as naked asm (`core::arch` / `global_asm`) using the
compare-and-select primitives, then audit in CI with a disassembly check asserting zero
conditional-jump opcodes in the hot `.text` range:

- x86-64: no `j*` family (`je`, `jne`, `jb`, `ja`, `cmov` is fine — it is not a jump)
- AArch64: no `b.cond`

Instruction-level primitives (the only decision primitive needed):

| Target  | Sequence                 | Instructions | Branches |
|---------|--------------------------|--------------|----------|
| x86-64  | `cmp` + `cmovb`          | 4            | 0        |
| AArch64 | `cmp` + `csel`           | 3            | 0        |
| WASM    | `i32.select` (lowering)  | 1 (packed)   | 0        |

The CI audit is the theorem's mechanical witness: the sum `Σ(deg⁺(v) − 1)` computed on the
disassembled CFG must be 0, i.e. no conditional control transfers exist.

## 6. Relevance to avec today

This document is the formal foundation for the branchless topology already in flight:

- [AVEC-107: branchless primitives
topology](../v26.10.29/AVEC-107-branchless-primitives-topology.md)
  — the mask algebra and primitive table operationalized.
- [AVEC-108: in-loop branch elimination](../v26.10.29/AVEC-108-in-loop-branch-elimination.md)
  — the loop-body application of §3/§4.

The ladder of §4 is the design for constant-time `.idx` lookups; §5's CI disassembly audit
is the enforcement mechanism those lanes should adopt so the M=1 claim is checked, not
asserted.

## Standing

PARTIAL — the §5 enforcement now exists in-repo: scripts/audit_branchless.sh
consumes scripts/branchless_manifest.toml (ADR-14 remainder policy). The gate
receipt is ALIVE with a negative control (gate-branchless-manifest.md).
Scope note: hosted-CI wiring is absent — this is an in-repo gate script only,
not a CI-enforced check. The math (§1–§4) remains design formalization.

## See Also

- [15-arrow-microarch-probe.md](15-arrow-microarch-probe.md)
- [AVEC-107: branchless primitives
topology](../v26.10.29/AVEC-107-branchless-primitives-topology.md)
