# Glossary: AVEC v26.10.31

Terms of art used across the 00–19 context series and 20–39 spec series. Design-stage
terms are marked **[design-stage]** — they are proposed, not witnessed; unmarked terms are
standard industry or academic terms.

## Formats & Substrate

**Apache Arrow (flat core)**
: Standard columnar in-memory format (Arrow Columnar Specification). AVEC uses a
  non-nullable, 64-byte-aligned flat profile; no offset or bitmap buffers in the core path.

**Parquet DataPageV2**
: Standard Parquet page encoding carrying levels and values in separated sections.
  AVEC projects Arrow records isomorphically onto DataPageV2 with zero
  definition/repetition levels.

**Definition/repetition levels**
: Dremel-style counters that reconstruct optional/repeated nesting from a flattened
  column. Eliminated in the AVEC projection: the core layout is flat and non-nullable.

**Git Packfile v2**
: Standard Git ODB wire/storage container (version 2): header, object records with
  variable-length size prefixes, zlib-deflated payloads, trailing pack checksum.
  Verified by unmodified `git`.

**.idx v2 fan-out table**
: The 256-entry prefix index at the head of a pack index file; entry *i* holds the count
  of objects whose first byte ≤ *i*, enabling binary search by first-byte fan-out.

**Large-offset escape (MSB bit 31)**
: Packfile .idx mechanism for offsets ≥ 2 GiB: a large-offset slot stores the MSB set and
  points to an 8-byte offset in a secondary table.

**mmap scan**
: Execution model where the query engine reads the mapped file image directly as its
  working set — no ETL, no decompression, no copy into heap.

**Bottom-type sentinel**
: Type-theoretic bottom (⊥) encoded as a bit pattern occupying the slot of a null. Replaces
  validity bitmaps; null-checks become value comparisons, removing bitmap read loops.

**TermId (dense interned u32)**
: Dense 32-bit handle for an interned RDF/ontology term in the term arena; identity is the
  index, comparison is u32 equality.

**Policy mask**
: A SIMD-width vector of per-lane admission bits applied branchlessly during the gate;
  lanes with mask 0 are dead but still consume cycles (constant time).

**Vector clock**
: Standard logical-clock structure tracking causal order across replicas. Used in lane 11
  (OCEL entanglement) for bilateral micro-window ordering.

**Inline digest**
: The 64-byte record field carrying a BLAKE3-derived digest, computed during the write
  pass itself so commitment is not a separate phase.

## Complexity & Execution

**McCabe Cyclomatic Complexity M**
: Standard metric: M = E − N + 2P, the number of linearly independent paths through a
  control-flow graph. Per the ADR-14 scoped policy, absolute M=1 is declared
  unreachable for allocation-performing kernels; the manifested remainder is
  accepted and adjudicated (see ADR-14 remainder policy below).

**Basic block**
: Maximal straight-line instruction sequence with one entry and one exit; the unit of the
  branchless kernels' single-block structure.

**Augmented CFG**
: Control-flow graph instrumented with dominator and loop information for the complexity
  audit that certifies the ADR-14 scoped M policy per kernel (absolute M=1 for
  pure kernels; manifested remainder adjudicated for allocation-performing ones).

**Unrolled search ladder**
: The fully unrolled 16-round fixed-iteration search (2^16 = 65,536 objects per partition);
  zero conditional jumps, one uniform latency for every query and miss. Hard cap
  of 2^16 objects, enforced with the typed `TooManyObjects` refusal above it.

**Quad64**
: The 64-byte-aligned, non-nullable quad record — the atomic columnar unit of the
  AVEC layout (23-64b-quad-spec.md): timestamp, id, policy mask, digest fields
  in one 64 B granule, no validity bitmap.

**branchless_manifest**
: Wave-7 adjudication input for the branchless gate
  (`scripts/branchless_manifest.toml`): maps manifest-covered emitted branches
  in hot symbols to explained sources; exit 0 on covered remainder, 1 on an
  unexplained hit, 2 on vacuous negative control. Receipt:
  gate-branchless-manifest.md.

**LSL_RATIFIED**
: A lower spec limit that has passed ratification (gaged harness + pinned
  profile) and is therefore admissible as a milestone-admitting fence, vs a
  provisional LSL that fences nothing yet.

**SKIP-NOISY**
: Typed skip class for a perf gate whose host is measurably noisy at mint time
  (perf-guard SKIP-NOISY history): the run is not a pass — it must be re-minted
  on a quiet machine before any throughput claim cites it.

**ADR-14 remainder policy**
: The scoped M policy (ADR-14): absolute M=1 is declared unreachable for
  allocation-performing kernels; the manifested remainder is accepted only when
  every emitted branch is adjudicated by branchless_manifest — unexplained
  hits fail the gate.

**Predication**
: Executing both sides of a branch and selecting by mask, converting control dependence
  into data dependence.

**CSEL/CSINC/CSETM**
: AArch64 conditional-select/conditional-increment/conditional-set-minus instructions —
  the branch-free primitives the gate compiles to on ARM.

**CMOV family**
: x86 conditional-move instructions serving the same role as CSEL on ARM.

**select / v128.bitselect**
: SIMD lane-wise selection ops (portable SIMD `select`; WASM `v128.bitselect`) — the
  portable form of predication inside sealed kernels.

**Branch misprediction penalty**
: The 15–20 cycle front-end flush incurred per x86/ARM mispredict; the cost the M=1
  invariant eliminates, and the timing channel it closes.

**Timing variance**
: Input-dependent execution-time spread. Target is zero observable variance across the
  admission gate for any input class.

**Gage R&R**
: Measurement-system study (repeatability and reproducibility) qualifying the benchmark
  harness itself before its numbers are admitted as receipts.

**Ratio fence**
: A performance bound expressed as a ratio against an independent, sleep-free baseline
  (paired rounds, min-of-N) instead of an absolute millisecond constant.

**g4x2**
: Refuted wave-8 interleave-kernel candidate for the BLAKE3 tree path: a 4-way
  lane-group interleave proposed to lift throughput. Refuted with assembly
  evidence as spill-bound — the interleaving pushed register pressure over the
  limit and the kernel was reverted (00-index wave-8 block; measurement
  appendix in 48-lsl-rederivation.md).

**State-spill ceiling**
: The measured BLAKE3 throughput limiter on this NEON hardware: register-file
  pressure spills kernel state to stack, capping the kernel below the memory
  controller ceiling regardless of interleave width — the real ceiling behind
  the refuted g4x2 candidate.

**p99/p50 protocol**
: The gage-conditioned CTQ-04 variance statistic (49-ctq04-variance-study.md
  Option b): gate on p99/p50 ≤ 1.30 with a mini-gage CV ≤ 2% admission branch
  — SKIP-NOISY under load, evaluate only on a quiet machine; the retired 15%
  stdev/mean pin is recorded as provenance, not verdict. Quiet-machine run
  PASS 1.134 vs USL 1.30. DFLSS ratification of the swap is open.

## Crypto & Receipts

**BLAKE3**
: Standard ARX hash function: 1 KiB chunks, 8-way SIMD leaf hashing, binary parent
  reduction to one root.

**Chunk tree / tree-hash mode**
: BLAKE3's Merkle reduction of chunk hashes into a single root; enables parallel hashing
  and verified streaming.

**Merkle-DAG**
: Directed acyclic graph of digest-linked nodes; each node's identity covers its children.

**Content-addressed storage (CAS)**
: Storage where the digest of content is its address; a stored object verifies itself.

**Commit OID as receipt**
: The Git commit object ID over the packfile state, computed inline at append time,
  serving as the transition's only admissible evidence.

**b3sum cross-verification**
: Independent verification of BLAKE3 roots using the reference `b3sum` tool, decoupling
  the claim from AVEC's own implementation.

**Chatman Equation** **[design-stage]**
: The settlement invariant: `ΔS_admitted + Φ_lease ≡ R_receipt (mod Σ_BLAKE3)`. The
  admitted batch digest plus the monotonic lease counter equals the receipt root under
  BLAKE3 digest equality; digest equality is the only channel of truth.

**Fail-closed admission**
: Rule that any invariant breach (alignment, digest mismatch, lease non-monotonicity)
  rejects the transition leaving persistent memory unmodified — no torn record, no
  receipt.

## BEAM Boundary

**NIF**
: Native Implemented Function — the Erlang VM's C-ABI extension point; here the
  zero-copy seam into the Rust substrate.

**Resource binary**
: A NIF resource object wrapping substrate memory so the BEAM references bytes it does
  not own or copy.

**Dirty scheduler**
: BEAM scheduler class for NIFs that block or run long; keeps the normal schedulers'
  reduction accounting clean.

**Zero-copy handoff**
: Crossing the BEAM/Rust boundary by passing references to the same physical bytes —
  no serialization, no GC pause attributable to the handoff.

## Process & Economics

**DMADV / DFLSS**
: Design For Lean Six Sigma: Define, Measure, Explore, Develop, Implement — the standing
  framework for substantive work (charter in doc 18).

**CTQ**
: Critical To Quality — a measurable product characteristic derived from customer or
  regulatory requirement (e.g., ≤ 35 µs settlement).

**LSL/USL**
: Lower/Upper Specification Limit — the bounds a CTQ must fall within.

**C_pk**
: Process capability index measuring how well a centered process fits inside its spec
  limits; used to qualify timing-distribution receipts.

**DFMEA**
: Design Failure Mode and Effects Analysis — systematic enumeration of design failure
  modes with severity/occurrence/detection scoring.

**RPN**
: Risk Priority Number — the DFMEA product score (severity × occurrence × detection)
  ranking failure modes for action.

**Poka-yoke**
: Mistake-proofing: a mechanism that makes the error class structurally impossible
  (e.g., fail-closed admission, alignment-enforced layouts).

**P28-SIL**
: The regulatory/licensing governance framework of doc 07: an IEC 61508-style SIL-style
  severity ladder mapping deployment scale to legal obligations. **[design-stage]**

**Safe harbor**
: The $50M deployment threshold under P28-SIL below which use is automatically
  royalty-free and perpetual.

**Commercial Regularization Notice**
: P28-SIL instrument offered above the threshold: formal notice converting unlicensed
  use into a regularization path. **[design-stage]**

**Covenant Not to Sue**
: Irrevocable covenant attached to the regularization path; converts infringement
  exposure into an obligation without litigation risk to either party. **[design-stage]**

**FASB ASC 450**
: US GAAP guidance on loss contingencies — the accounting treatment for disclosed
  regularization exposure.

**SOX 302/404**
: Sarbanes-Oxley sections requiring officer certification (302) and internal-control
  attestation (404) over financial reporting; the substrate's tamper-evidence supports
  both.

**EU AI Act Articles 12/14/62**
: Reg (EU) 2024/1689 articles: 12 (record-keeping/logs), 14 (human oversight), 62
  (SME/support provisions) — the compliance surface the notary substrate targets.

## Protocol Layer (context docs 00–19)

**POWL 2.0**
: Process Ontology Workflow Language — partially-ordered workflow language used for the
  escrow notary flows of doc 10.

**OCEL 2.0**
: Object-Centric Event Log standard (IEEE); events related to multiple objects, used in
  doc 11 for bilateral micro-window unforgeability.

**HTN FOND**
: Hierarchical Task Networks over Fully Observable Nondeterministic domains — planning
  formalism behind the FOND-HDDL task decomposition used in the design flow.

**ASOF join**
: Temporal join matching each left row to the nearest-in-time right row; used for
  aligning event streams with clock evidence.

**Distance bounding**
: Cryptographic protocol (Brands–Chaum lineage) proving proximity by measuring round-trip
  time of challenge-response; the anti-relay basis of doc 12.

**RTT envelope**
: The admissible round-trip-time window separating genuine proximity from
  relay/processing-time attacks (the processing-time trap).

**Blinded WASM**
: Stripped, zero-import WASM challenge kernels (doc 14) whose internals are not
  inspectable by the prover side; execution timing carries the evidence.

**AHDCLT** **[design-stage]**
: Adversarial Hyperdimensional Cognitive Load Theory (doc 16) — doctrine that stacks
  asymmetric cognitive, legal, and cryptographic load against adversaries; a design
  lens, not a measured result.

**stardf** **[design-stage]**
: The RDF 1.2 triples/quads substrate layer with SHACL turnstiles and term arena
  (docs 01, 05, 06, 09); owns ontology reasoning, not memory scheduling.

**graphlaw** **[design-stage]**
: The graph-law layer paired with stardf: executable rules/admission laws over the RDF
  substrate (WASM graphlaw kernel lineage), distinct from the BEAM layer above it.

## Standing Note

Updated: a receipt corpus now exists — 14 ALIVE / 8 FAILED per
50-receipts-audit.md (all pinned to HEAD ead4032). Superseded total: the
final corpus is 36 receipts — 25 ALIVE / 10 FAILED / 1 PARTIAL per
64-receipts-inventory-final.md, which supersedes 50's counts. Partial
witnesses include settlement latency (CTQ-05, gate-e2e-ctq05 ALIVE) and
the harness gage (gate-gage-rr ALIVE); CTQ-06 BLAKE3 throughput remains
FAILED at 1.85 GiB/s vs the unratified 8.5 LSL. Terms marked
**[design-stage]** carry lower standing than standard terms: they are
proposals subject to the falsifiers in `31-verification-ladder.md`, not
established results.

## See Also

- [20-prd.md](20-prd.md) — product requirements these terms operationalize.
- [21-ard.md](21-ard.md) — architectural decisions and the Chatman Equation definition.
- [31-verification-ladder.md](31-verification-ladder.md) — falsifiers that govern standing.
