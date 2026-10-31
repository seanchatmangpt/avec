# AVEC v26.10.31 Architectural Requirements Document (ARD)

## Architectural Invariants & Scope Boundaries

AVEC is a freestanding physical substrate. It deliberately excludes everything that requires a
semantic or scheduler authority above it. Out of scope: ontology management, W3C RDF reasoning,
and SHACL evaluation — those belong to stardf/graphlaw. Also out of scope: concurrency
scheduling, mailboxes, supervisor trees, and logical monotonic clocks — those belong to the
BEAM/OTP layer above.

In scope: the memory geometry, SIMD arithmetic, level-free storage projection, binary index
lookups, and parallel tree hashing that make the upper layers fast and deterministic. AVEC owns
bytes, alignment, and cycles; it owns no meaning and no control flow policy.

```
+----------------------------------------------------------------------+
| stardf / graphlaw                                                    |
|   RDF 1.2 triples/quads, SHACL turnstiles, term arena                |
+----------------------------------------------------------------------+
| BEAM / OTP                                                           |
|   supervision trees, hybrid logical clocks, mailboxes                |
+----------------------------------------------------------------------+
| AVEC core                        <-- this document's subject         |
|   Arrow 64B non-nullable record matrix                               |
|   Parquet DataPageV2 level-free mmap projection                      |
|   Packfile v2 16-stage ladder (storage == wire)                      |
|   BLAKE3 tree engine (parallel Merkle commitment)                    |
|   M=1 predication kernel (single basic block)                        |
+----------------------------------------------------------------------+
| commodity hardware                                                   |
|   ARM64 NEON, x86_64 AVX2/AVX-512, WASM simd128                      |
|   hardware prefetchers, memory controllers, NVMe                     |
+----------------------------------------------------------------------+
```

Invariant I1: every record is exactly 64 bytes and 64-byte aligned. Invariant I2: no nullable
encoding anywhere in the hot path (no def/rep levels, no validity bitmaps). Invariant I3: the
on-disk packfile format and the wire format are the same bytes. Invariant I4: all control flow in
search kernels is predicated, not branched — M (basic block count) stays 1.

## The Physical Storage Triad

### 2.1 Arrow Matrix: the 64-Byte Record

Each quad record is one 64-byte slot in a columnar Arrow matrix:

| field          | width | encoding                          |
|----------------|-------|-----------------------------------|
| s (subject)    | 4 B   | TermId                            |
| p (predicate)  | 4 B   | TermId                            |
| o (object)     | 4 B   | TermId                            |
| g (graph)      | 4 B   | TermId                            |
| t_ns           | 8 B   | monotonic u64 nanosecond clock    |
| vector clock   | 4 B   | causality tag                     |
| policy mask    | 4 B   | duty/policy envelope bits         |
| inline digest  | 32 B  | BLAKE3 truncated commitment       |
| **total**      | 64 B  | one full cache line / one vreg×4  |

Buffers are 64-byte aligned end to end, so a record loads directly into four 128-bit vector
registers (or two 256-bit, or one 512-bit) with no unaligned fault path and no gathering. The
record is simultaneously the memory unit, the Arrow unit, and the Parquet row unit.

### 2.2 Parquet DataPageV2: Level-Free Projection

Because the schema is flat and non-nullable, definition and repetition levels are zero-length by
construction — ADRV does not store them at all. DataPageV2 headers are emitted with
level_byte_length = 0; pages map 1:1 onto Arrow record buffers, so an mmap of a page file is an
mmap of the matrix. Scans are pure sequential loads with no level reconstruction, no validity
decode, and no per-value branching: measured sustained throughput on commodity NVMe is 65.6
GiB/s min (PAYLOAD-SUM; the >74 GiB/s design figure is NOT confirmed — 24 amendment, wave 7);
the bus, not the decoder, is the ceiling.

### 2.3 Git Packfile v2: Storage and Wire Equivalence

The same Packfile v2 byte stream serves as the durable storage projection and the wire protocol
payload. The 256-entry fan-out table gives O(1) first-byte partition bounds — one table lookup
names the [start, end) window of the index for any object id. Commit lineage is an append-only
Merkle DAG: each commit hashes its tree and its parent, so history is verifiable in parallel and
no rewrite path exists. Zero serialization boundary: what is committed is what is sent.

## The M=1 Execution Engine

Graph-theoretic statement: the search kernel compiles to an augmented control-flow graph
G = (V, E, entry, exit) with |V| = 1 — a single basic block. There are no back edges, no branch
arcs, no loop headers. The M invariant (basic block count = 1) is structural, enforced at the
machine-instruction level, not by compiler hope.

ADR-14 note (wave 8): absolute |V| = 1 is declared unreachable for any allocation-performing
kernel — the prologue/epilogue allocation path is itself a basic block. The manifested,
enforced form of M=1 is therefore allocation-freedom of the hot path plus predication within
it; the register records this as the remainder policy (see 22-adr-register.md ADR-14 and
FR-05 PARTIAL in 20-prd.md).

Microarchitectural predication primitives per target:

| target  | branchless select        | lane shuffle / packing   |
|---------|--------------------------|--------------------------|
| ARM64   | CSEL, CSINC, CSETM       | VZIP, VUZP               |
| x86_64  | CMOVB, CMOVNZ            | VPUNPCKL, VPUNPCKH       |
| WASM    | select, v128.bitselect   | v128 lane ops            |

The 16-stage unrolled search ladder:

```
displacement: 32768 16384 8192 4096 2048 1024 512 256 128 64 32 16 8 4 2 1
stage i:        probe base[i]  ->  compare  ->  bounds check
                ->  ordered select (predicated)
                ->  predicated base update (CSEL/CMOV/select)
```

Mechanics: each stage probes the candidate slot at its displacement, compares, reorders the
lo/hi bounds, and updates the base register under a predicated move. Every stage emits the
identical instruction sequence for hits, misses, and boundary conditions — the only difference
between outcomes is register contents, never instructions executed. Zero timing variance by
construction; no branch predictor state is touched. With 16 stages of halving displacement the
ladder addresses up to 2^16 = 65,536 objects per partition.

## Cryptographic Invariants & the Chatman Equation

BLAKE3 commitment structure: input is split into 1 KiB chunks hashed as vectorized ARX leaf
nodes (4-way SIMD rounds per target — ADR-15, refutation record: the wave-7 assembly audit
refuted the register-budget premise for 4-way at 0 spill loads; the wave-8 8-wide g4x2
candidate was spill-bound and reverted; see 22-adr-register.md ADR-15 amendment); parents
reduce pairwise in parallel —
Parent = Compress(L ∥ R) — until a single root remains. The inline digest field of each 64-byte
record is derived during the write pass itself, so commitment is not a separate phase; the
commit-OID is computed inline as records are appended to the packfile.

The Chatman Equation:

```
Delta S_admitted + Phi_lease  ==  R_receipt   (mod Sigma_BLAKE3)
```

Four terms:

- Delta S_admitted — the admitted state delta: the 64B-aligned batch of records accepted in
  this transition, hashed through the BLAKE3 chunk tree.
- Phi_lease — the monotonic lease counter drawn from the host (ns clock + counter), binding
  the receipt to exactly one point in the causal order.
- R_receipt — the 32-byte receipt: the BLAKE3 root of (batch digest ∥ lease), the only
  admissible evidence that the transition executed.
- Sigma_BLAKE3 — the verification-root modulus: all equality is checked as digest equality
  under the BLAKE3 root; there is no side-channel of truth.

Notation note (wave 8): `Sigma_BLAKE3` here is informal prose notation, NOT a minted TTL term.
The ontology (19-ontology-ttl.md) carries only `Blake3MerkleReceipt/sec:blake3`. Recorded as
informal by preference; minting is deferred until a court consumes the term.

Fail-closed rule: on any invariant breach (alignment, digest mismatch, lease non-monotonicity),
the transition is rejected and persistent memory is left unmodified — a rejected write produces
no partial page, no torn record, no receipt.

## Regulatory & Licensing Governance (P28-SIL)

Threshold: $50M safe harbor. Deployments under the threshold receive automatic royalty-free,
perpetual licensing. Above the threshold, a Commercial Regularization Notice plus Irrevocable
Covenant Not to Sue governs continued use — the covenant converts infringement exposure into a
regularization obligation without litigation risk to either party.

Accounting posture: unasserted liability is recognized under FASB ASC 450 when probable and
estimable; SOX 302/404 enforcement is mechanical via non-deniable BLAKE3 receipts embedded in
packfiles — the receipt stream is the audit evidence, and digest equality denies nothing.

EU AI Act conformity:

- Art 12 (continuous logging): append-only packfiles with ns timestamps are the automatic
  event log; the Merkle DAG makes retroactive edit detectable.
- Art 14 (human oversight): oversight envelopes live in the policy/duty mask fields of the
  64-byte quad, so every record carries its own oversight binding.
- Art 62 (post-market surveillance): satisfied by open-source tooling operating over read-only
  Git endpoints — surveillance needs no special access beyond the public packfile stream.

## ADR Summary

### ADR-01: Reject validity bitmaps; use non-nullable sentinels

- Context: Arrow's default validity bitmap forces a mask load and branch per column per
  batch, and breaks 64-byte record alignment.
- Decision: no validity bitmaps. Nulls are represented by sentinel TermIds inside the fixed
  64-byte record.
- Consequence: branch mispredictions from null checks are eliminated; the 64-byte record is
  alignment-guaranteed; one dead-bit class of bug (bitmap/row desync) is structurally
  impossible.

### ADR-02: Eliminate Parquet def/rep levels

- Context: DataPageV1/level-full V2 pages require level reconstruction before values are
  usable, capping scan throughput below bus speed.
- Decision: zero-length def/rep levels (flat non-nullable schema); pages map 1:1 to Arrow
  buffers.
- Consequence: mmap scans are bus-bound (65.6 GiB/s measured min; the >74 GiB/s design figure
  is NOT confirmed — 24-parquet-projection-spec.md amendment); decode cost per value is
  zero; nested schemas are out of scope by construction.

### ADR-03: Native Packfile v2 as sequencer and wire

- Context: a separate storage format plus wire protocol plus consensus layer is three
  serializers to keep in agreement.
- Decision: use Git Packfile v2 natively as both durable sequencer and wire format; no
  consensus daemons, no blockchains.
- Consequence: zero serialization boundary (commit bytes == wire bytes); O(1) fan-out index
  partitioning; append-only Merkle lineage gives free verifiability.

### ADR-04: Mandate M=1 at the machine-instruction level

- Context: C compilers re-introduce branches under register pressure, silently destroying
  constant-time search guarantees.
- Decision: M=1 is enforced in assembly via predication (CSEL/CMOV/select), verified by
  automated disassembly checks in CI.
- Consequence: zero branch mispredicts; deterministic latency for hits, misses, and edges;
  compiler evolution cannot regress the invariant without a loud CI failure — at data-path
  granularity per ADR-14 (allocation paths excluded from the |V|=1 claim).

The expanded register with status and receipt linkage lives in 22-adr-register.md.

## Standing

UNVERIFIED for the authored design body — with landed in-body notes (the ADR-14 remainder
note, wave 8, and the ADR-15 refutation record are receipts-backed; see 22-adr-register.md).
Performance figures (65.6 GiB/s measured min; the >74 GiB/s figure is NOT confirmed per
24-parquet-projection-spec.md; 65,536 objects/partition) remain subject to the qualification
courts in 20-prd.md.

## See Also

- 20-prd.md — product requirements this ARD elaborates
- 02-architecture-planes.md — plane decomposition
