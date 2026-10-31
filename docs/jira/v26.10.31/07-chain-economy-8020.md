# The Chain Economy 80/20 and Its Substrate Equivalent

## The 95/5 Split

Roughly 95% of public-chain compute is coordination overhead: BFT voting rounds,
gossip floods, mempool thrashing, state bloat, and VM interpretation. Only ~5% does
real work — state-transition verification, signature checks, and transaction
ordering. The remaining value in the chain economy concentrates in three places:

- **Ordering rights** (MEV, sequencer priority)
- **Batch settlement**
- **State-proof turnstiles**

The thesis: a local-first substrate can capture the 5% directly and discard the
95% entirely, because it does not need to coordinate distrusting parties.

## The Equivalence Table

Each row: element | the 80% mechanical waste | the 20% real primitive |
stardf-git + avec equivalent.

- **Consensus / block production** | PoW/PoS voting, gossip floods, slashing
  games | Monotonic total ordering of events | Monotonic nanosecond ordering +
  BLAKE3 content-addressed turnstile
- **World state** | Patricia / Merkle-Mountain trees, disk amplification |
  Verifiable state commitment | Git packfile v2 / .idx mmap + avec columnar
  Parquet
- **Execution** | EVM stack machine, gas metering, runtime branching |
  Deterministic state transition | Branchless TermId bitmasking + SHACL
  pre-admitted turnstile
- **Data availability** | Blob auctions, erasure coding, sampling networks |
  Content-addressed data publication | Git wire protocol (`git fetch` /
  `git-upload-pack`)
- **Settlement / rollups** | ZK proving clusters, >10^6 proving overhead | Final
  commitment to a root of trust | The receipt turnstile: ΔS_admitted +
  Φ_lease ≡ R_receipt (mod Σ_PQC)

Reading the table: every row replaces a mechanism whose cost exists to arbitrate
distrust with a mechanism whose cost exists to commit content. The turnstile moves
from "prove you paid and reached quorum" to "your content address resolves and your
receipt verifies."

## The Sovereign Sequencer Model

Local-first, zero-gas, line-rate clearing. There is no mempool and no auction:
ordering is a hardware timestamp, not a market.

Pipeline:

```text
inbound agent intent
        |
        v
4D hardware gate
  - nanosecond monotonic timestamp
  - vector causality check
  - TermId bitmasking (SHACL pre-admission)
  - zero alloc, zero gas
        |
        v
avec columnar write
  (>= 5.3 GiB/s encode, direct DuckDB ASOF)
        |
        v
Git packfile v2 CAS turnstile
  (BLAKE3 / SHA-256 Merkle ancestry)
        |
        v
settlement / DA: git push --mirror
  (wire-speed sync to upstream ledger)
```

The gate is the only admission surface. An intent that passes the 4D gate is
already ordered, causally justified, schema-admitted, and allocation-free; the
rest of the pipeline is mechanical content addressing and transport.

## 80/20 Leverage Points

- **Zero gas / absolute cost predictability.** A graph update costs ~nanoseconds
  of cache compute, not a volatile gas price. Cost becomes a hardware constant,
  not a market variable — the economic precondition for agent-scale transaction
  volumes.
- **Standard Git as the ultimate DA layer + block explorer.** `git log`,
  `git checkout`, and GitHub already solve node sync, history inspection, and
  explorer UX for free. Data availability and observability are not products to
  build; they are inherited infrastructure.
- **Instant analytical settlement.** DuckDB ASOF joins over mmap'd Parquet
  replace The Graph / subgraph indexer fleets. Query latency collapses from
  indexer-replication lag to local mmap reads.

## Status

This is thesis/design material. Economic claims are qualitative; performance
comparisons (GiB/s encode, nanosecond gate cost, >10^6 ZK overhead ratio) are
design targets. UNVERIFIED as to any measured numbers — see
04-benchmark-contract.md for the measurement gates that would verify them.

## See Also

- 06-stardf-git-odb.md
- 12-distance-bounding-rtt.md
- 19-press-release.md
