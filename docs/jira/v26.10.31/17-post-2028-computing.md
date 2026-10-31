# Post-2028 Computing: Git-Packfile State as the Network

Standing: UNVERIFIED thesis. Dated 2026-10-30.

## The Transport Mechanic: Wire Payload IS the Storage Payload

Classic distributed state moves through a decode-heavy chain: serialize the
database image, wrap it in gRPC or REST, push it through a socket, deserialize
on the far side, re-index into a local store. Each hop is a full copy plus a
format translation, and every translation is a correctness and latency
liability.

Avec collapses the chain. State lives natively as Git packfiles — packfile v2
containing 64-byte aligned columnar chunks. The bytes on the wire are the
bytes on disk. A receiving node appends the payload to its `.pack`, `mmap`s
the file, and hands raw pointers to the vector engine. There is no decode
step, no re-index, no row reconstruction. The moment the bytes land:

- DuckDB queries the columnar chunks in place.
- Turnstiles evaluate the same buffers without materialization.

Transport and storage are one format. `git fetch`/`git push` is the replication
protocol, and it already runs over SSH, HTTPS, Unix sockets, and removable
media — the network is wherever Git speaks.

## Compliant BLAKE3 Invariants

The packfile sealing engine targets three invariants:

1. **Memory-bus-saturating throughput.** Design target >10 GiB/s per core,
   versus the ~300–600 MiB/s class of SHA-256 and Keccak implementations.
   Hashing stops being the bottleneck of the commit path.
2. **Parallel 1 KiB chunk-tree reduction.** BLAKE3's chunk tree lets commit
   OIDs be derived *during* the write pass, not in a second pass afterward.
   Content addressing becomes a side effect of writing state.
3. **Strict spec compliance.** Because the engine is a compliant BLAKE3, any
   external `b3sum` or forensic tool independently verifies every packfile.
   Verification does not require trusting avec's own code paths.

Together these make the content address cheap enough to stamp on every state
transition, and trustworthy enough to be checked by parties who run no avec
software at all.

## Post-2028 Implications

### Collapse of the database-vs-blockchain schism

A Git repository of columnar Arrow/Parquet buffers, sealed with BLAKE3 DAG
lineage, is simultaneously:

- a queryable analytical database (local DuckDB over zero-copy chunks), and
- a ledger: append-only, content-addressed, DAG-ordered, sovereign, zero-gas,
  and limited only by wire and disk speed.

The schism existed because ledgers paid for tamper-evidence with throughput
and databases paid for throughput with mutability. Content-addressed
columnar packfiles remove both halves of the trade.

### Sovereign agent clearinghouses

Every avec node is self-contained: storage, verification, and query are all
local. Sync is `git fetch`/`git push` against any peer. Two agents that
disagree about history resolve the dispute by comparing BLAKE3 DAG lineage —
the divergence point is a hash, and the hash is checkable by both sides and
by any third party with `b3sum`. A forged action breaks the content address
at the network edge: the receiving node refuses the pack before it is ever
queried. Trust is established per-pack at ingress, not per-platform at
account creation.

### EU AI Act compliance by construction

The regulation's logging articles (12 and 62: continuous, tamper-evident
record-keeping of system behavior; Article 14: human oversight) are usually
served by bespoke telemetry pipelines. A avec node's state *is* a continuous
tamper-evident log with cryptographic lineage. Compliance becomes handing a
regulator a read-only Git remote, or an offline bundle, and letting them
verify lineage with open forensic tools. The audit artifact is the same
object the system runs on — no parallel compliance stack, no pipeline to
keep honest.

### Death of API churn and vendor lock-in

Git's wire protocol is the most universally deployed state-transfer protocol
in existence, running over SSH, HTTPS, Unix sockets, and air-gapped media.
Because avec state is native to it, no vendor owns the serialization, no API
version gates the reads, and no migration is needed to move between edge
device, laptop, and cloud. State is liquid. An API vendor can only charge for
what they exclusively provide; avec state provides itself.

## The Cloud-Cartel Threat Analysis

This thesis is hostile to several existing profit centers. The mechanics:

### Egress tollbooth annihilation

When the storage format is the network packet, egress stops being a
billable transformation and becomes byte conduit work. There is no
provider-side "export" step to meter at a premium — the export is the
storage format itself.

### Managed-database margin collapse

Two warehouse revenue lines dissolve:

- **Consistency**: Merkle-DAG ordering gives it for free, at commit time,
  with no coordinator service.
- **Analytics**: DuckDB runs against the local mmap'd chunks — the same
  engine path targets >74 GiB/s — with no warehouse credits consumed.

The customer's laptop is the warehouse.

### Proprietary governance and telemetry layer collapse

Auditor and regulator features (immutable logs, lineage, access attestation)
are exactly what the BLAKE3 DAG already provides. Verification happens
offline with open tools. A governance layer that sells what the substrate
gives away cannot hold its price.

### Commoditization of hyperscalers

What remains billable: electricity, bandwidth, silicon rental. Real and
large markets — but commodity ones, priced against each other rather than
against a locked-in data gravity.

### Profit center vs substrate reality

| Cloud profit center | Substrate reality under packfile-native state |
| --- | --- |
| Egress / export fees | Storage format IS the packet; conduit-only pricing |
| Managed warehouse credits | Local DuckDB over mmap'd chunks; consistency via DAG order |
| Lock-in via proprietary serialization | Git wire protocol; universal, versionless |
| Governance / audit add-ons | Offline verification with open BLAKE3 tools |
| Compliance pipelines (AI Act, SOC 2) | Hand over a read-only remote or bundle |
| Premium data gravity | State liquid across edge, laptop, cloud |

## Discipline Note

This document is a strategic thesis, standing UNVERIFIED as of 2026-10-30.
Market and regulatory claims are qualitative forecasts, not measurements.

Update (post-wave-5): two of its targets now have receipts that partially
address them, both short of target. CTQ-06 measured compliant BLAKE3
throughput at 1.85 GiB/s/core against the >10 GiB/s/core invariant — FAIL
(the compliant no_std engine exists; the memory-bus-saturating claim does
not). The parquet decode baseline measured 65.6 GiB/s against the >74 GiB/s
query-path target (receipts/parquet-throughput-baseline.md) — PARTIAL.

## See Also

- `06-stardf-git-odb.md`
- `07-chain-economy-8020.md`
- `18-dflss-charter.md`
- `24-parquet-projection-spec.md`
- `51-release-notes.md`
