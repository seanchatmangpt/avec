# stardf-hooks: Declarative Reflex Engine for Git Turnstiles

## Provenance

`purrdf-hooks` already exists upstream as a declarative reflex engine: RDF-described
condition-action rules evaluated to a fixpoint over SHACL shapes, SPARQL CONSTRUCT
bodies, and SQL-in-RDF predicates. `stardf-hooks` is its reconstitution on the stardf
substrate: the deterministic reflex layer that binds Git lifecycle events to the
branchless turnstile.

Standing: **UNVERIFIED** design. Upstream exists; this document specifies the
reconstitution target, not a receipted implementation.

## Pipeline

```
LLM/agent push intent
        |
        v
git-receive-pack
        |
        v
stardf-git hook trigger   (pre-receive / update / post-receive)
        |
        v
payload frame buffer      (argv + stdin framed into zero-copy stardf buffers)
        |
        v
stardf-hooks evaluator
  - topological dependency DAG of reflex rules
  - SHACL shape preconditions as admissibility gates
  - fixpoint delta evaluation over the zero-copy TermId arena
  - resource-bounded reflex budget (max iterations, max quads interned)
        |
   +----+-----+
   |          |
   v          v
rejection   admission
exit != 0   firing quad interned (content-addressed) into the event log;
machine-    state admitted into the avec columnar log
readable
receipt
```

Two exit shapes only. Rejection is a non-zero exit plus a machine-readable receipt
(`hook_id`, violated shape, evidence term ids). Admission interns a firing quad whose
name is content-addressed (BLAKE3 over subject/predicate/object/graph) and hands the
admitted state to the avec columnar log.

## Invariant Mapping (purrdf-hooks spec -> stardf-hooks execution)

| purrdf-hooks invariant | stardf-hooks execution |
| --- | --- |
| Rule specification: Turtle-declared condition-action pairs | Stored in `.stardf/hooks.ttl` inside
the target repo; versioned with the code the rules govern |
| Trigger points: graph deltas, metric thresholds, time windows | Standard git hook boundaries:
`pre-receive`, `pre-commit`, `post-merge` |
| Evaluation engine: SHACL validation + SPARQL fixpoint loop | Branchless CMOV state gate over avec
buffers; <= 340 ns/node target on the compiled DAG
(unmeasured: no hooks DAG bench exists) |
| Provenance quad | Immutable Quad64 frame (crates/avec-arrow/src/layout.rs, spec 23) carrying the BLAKE3 commit receipt |
| Safety: resource-bounded recursion + topological rule DAG | Fail-closed: an unresolved dependency
or any constraint violation aborts the push with a typed receipt |

The mapping is structural, not aspirational: each upstream invariant lands on exactly
one stardf execution surface (file, git boundary, arena gate, frame, or abort path).

## Example Declarative Hook

Sketch of `.stardf/hooks.ttl`:

```turtle
@prefix hook: <https://stardf.dev/hooks#> .
@prefix sh:   <http://www.w3.org/ns/shacl#> .
@prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .
@prefix ex:   <https://example.org/txn#> .

hook:GitPreReceiveHook a hook:ReflexHook ;
    hook:trigger     hook:PreReceive ;
    hook:evaluator   hook:ShaclFixpointEngine ;
    hook:shape       [ a sh:NodeShape ;
                       sh:targetClass ex:AgentTransaction ;
                       sh:property [ sh:path     ex:amountUsd ;
                                     sh:maxInclusive "10000.0"^^xsd:decimal ] ] ;
    hook:onFailure   hook:AbortGitPush ;
    hook:onSuccess   hook:EmitFiringReceipt .
```

Semantics: every `pre-receive` invocation evaluates the inline `sh:NodeShape` against
the push's framed payload. Any `AgentTransaction` with `amountUsd > 10000.0` fails the
shape, the fixpoint does not admit, and the hook aborts the push with a receipt naming
the offending node.

## Implementation Plan

1. **C ABI surface.** Map the stardf-hooks C ABI that connects git argv/stdin streams
   directly to the evaluator — no subshells, no intermediate scripts. Git invokes the
   compiled hook binary; the binary parses argv and stdin into payload frames and calls
   the evaluator in-process.

2. **Compile the fixpoint DAG.** Via ggen, compile the `.stardf/hooks.ttl` rule set into
   static, constant-time verification binaries embedded in `.git/hooks`. Each rule DAG
   becomes a topologically ordered, branchless (CMOV) check sequence; the ggen manifest
   is the source of truth and the binaries are projections, never hand-edited.

## Standing

- `purrdf-hooks`: exists upstream (declarative reflex engine over SHACL + SPARQL
  fixpoint + SQL-in-RDF predicates).
- `stardf-hooks`: UNVERIFIED reconstitution target. This document is design capture;
  no ABI, no compiled DAG, no hook binary has been executed against a real
  `git-receive-pack` under this design.

## See Also

- `08-pr-turnstile-llm-safety.md`
- `03-subsystem-mapping.md`
