# Parliamentary Escrow: Robert's Rules via POWL v2, HTN FOND, and SPARQL CONSTRUCT

Standing: UNVERIFIED design thesis.

See Also: 11-ocel-entanglement.md, 12-distance-bounding-rtt.md, 07-chain-economy-8020.md

## Protocol Flow Diagram

Two counterparties express a financial transaction as declarative graph intent; a third
agent (the notary chair) adjudicates the exchange as a chaired meeting and settles it as a
git transition.

```text
Party A (seller)                    Party B (buyer)
  branch intent/party-a               branch intent/party-b
  SPARQL CONSTRUCT:                   SPARQL CONSTRUCT:
  offer + asset transfer              payment + acceptance
        |                                   |
        +----------------+------------------+
                         |
                         v
              Notary Chair (escrow agent)
                         |
  +----------------------+-----------------------+
  | (1) Robert's Rules via POWL v2               |
  |     main motion, second (counterparty        |
  |     signature match), floor/amendments       |
  |     (preconditions), call the question /     |
  |     division of assembly (cryptographic      |
  |     quorum)                                  |
  | (2) HTN FOND contingency resolution          |
  |     policy graph: timeout, insolvency,       |
  |     oracle drift                             |
  | (3) graph-law turnstile                      |
  |     both CONSTRUCTs over interned TermId     |
  |     arenas; bilateral closure check;         |
  |     Chatman admission equation               |
  | (4) OCEL v2 forensic lineage                 |
  |     lifecycle as object-centric events       |
  |     over avec Parquet                        |
  +----------------------+-----------------------+
                         |
          +--------------+---------------+
          |                              |
          v                              v
     REFUSED                        ADMITTED
  PR closed unmerged;           squash-merge to main;
  typed refusal;                merge commit hash = legal
  escrow aborts                 receipt; atomic settlement
```

The merge commit hash is the settlement instrument: it is content-addressed by the exact
union graph both parties signed, replayable by any third party from the OCEL v2 event log.

## POWL v2 Mapping Detail

Robert's Rules of Order maps onto POWL v2 (Process Ontology Workflow Language, per
37-glossary.md) operators:

- **The motion**: Party A's CONSTRUCT is the main motion — a proposed transition of the
  shared graph. It is admissible only as a well-formed POWL activity node.
- **The second**: Party B's acceptance branch is the second. An unseconded motion dies on
  the floor at the lease timeout `t_lease` — the POWL activity is marked skipped and the
  escrow aborts with a typed `UNSECONDED` refusal. This replaces open-ended lock waits
  with a bounded procedural exit.
- **Floor and amendments**: preconditions attached to the motion (price bounds, asset
  provenance, compliance predicates) are POWL sub-processes that must complete before the
  main motion is votable. An amendment is a bounded edit to the CONSTRUCT's WHERE clause;
  the amender must re-sign the amended graph.
- **Privileged motions**: POWL's concurrency (interrupt) operator models motions that
  preempt the main motion — dispute-opened, liquidity-freeze, compliance-audit. A
  privileged interrupt suspends voting until the interrupting sub-process resolves; the
  main motion's state is preserved, never dropped.
- **Call the question / division of assembly**: closing debate and verifying the vote are
  cryptographic quorum operations — signature verification under shape constraints, not
  voice votes. Quorum is expressed as a SHACL shape over the signature set: the counted
  signatures must instantiate the expected party roles exactly.

Quorum as shape-constrained signature verification means the chair cannot count a
signature twice, count a non-party signature, or proceed with a partial count — the shape
either validates over the signature multiset or the motion fails closed.

## SPARQL CONSTRUCT as Bilateral Graph Contracts

Intent is declarative, not imperative. Each party states the world they are committing to;
neither party executes code against the other's state.

This sidesteps the failure classes of imperative smart contracts: there is no reentrancy
(no call graph exists to reenter), no integer overflow (asset quantities are typed RDF
literals validated by SHACL, not unbounded machine arithmetic executed in-line), and no
ordering hazard between the two parties' submissions — the chair evaluates the union once,
atomically.

Example CONSTRUCT pair:

```sparql
# Party A (seller) — intent/party-a
CONSTRUCT {
  :escrow :holds :AssetX ;
          :beneficiary :PartyB ;
          :condition :paymentProofExists .
}
WHERE {
  :PartyA :owns :AssetX ;
          :authorizes :escrowTransfer .
}

# Party B (buyer) — intent/party-b
CONSTRUCT {
  :PartyB :pays :PartyA :Price ;
          :accepts :escrowTerms .
}
WHERE {
  :Price :withinAgreedBounds true ;
         :oracleQuoted :quotedPrice .
}
```

The notary evaluates the graph union `G_A ∪ G_B` as a bipartite match under SHACL: A's
offered conditions must be discharged by B's payment graph and B's conditions by A's asset
graph. The match is bipartite because the graphs are role-disjoint — A cannot satisfy its
own conditions, B cannot satisfy its own. A complete matching is the bilateral closure
check; any unmatched condition node is a typed refusal naming the unsatisfied predicate.

## Why This Beats Public Smart Contracts

- **No gas, no MEV.** Evaluation is local deterministic graph computation. There is no
  fee market to bid on, so no transaction-ordering extraction exists: the chair's
  evaluation is not an auction and cannot be frontrun.
- **Deterministic dispute resolution with procedural exits for every state.** POWL v2
  guarantees every reachable state has a defined exit — second, amend, interrupt, defer,
  or die at `t_lease`. Rogue-agent deadlock (one party stalling the escrow indefinitely)
  is prevented structurally, not by heuristic timeouts bolted onto an executor.
- **Audit via standard tooling (design intent).** The lifecycle would be OCEL v2
  object-centric events over avec Parquet, queryable with DuckDB SQL and readable with
  standard git tools — no proprietary VM trace semantics, no node archive required.
  No OCEL Parquet projection exists in the workspace yet; this names the target
  tooling, not shipped capability.

The legal receipt is ordinary infrastructure: `git log` on main shows the merge; the merge
commit's tree contains the adjudicated union graph; the Parquet sidecar shows every
procedural step that admitted it.

Standing: UNVERIFIED design thesis — no implementation, no falsifier run.

See Also: 11-ocel-entanglement.md, 12-distance-bounding-rtt.md, 07-chain-economy-8020.md
