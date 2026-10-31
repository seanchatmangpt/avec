# ADR Register: AVEC v26.10.31

Register of architecture decision records for the v26.10.31 milestone. ADR-01..04 are
core ADRs from the PRD/ARD; ADR-05..12 are derived from the same design session. Split:
ADR-01..12 carry Standing `proposed`/UNVERIFIED (design-only, no implementation receipts);
ADR-13..16 and ADR-18 are `Accepted-for-v26.10.31` with implementation receipts; ADR-17
is `Accepted-as-recorded` (see each Status section).

## ADR-01: Reject Validity Bitmaps; Non-Nullable Sentinel Bottom-Type

### Context

Arrow's canonical nullability model is a separate validity bitmap per column. For AVEC's
quad-oriented, zero-copy, data-independent kernels this forces a second metadata stream
to load, branch on, and keep coherent with the value stream — a per-row divergence that
breaks the M=1 branchless algebra and doubles cache-line touch surface on decode.

### Decision

Nullability is expressed inside the value domain itself: a non-nullable column of a
bottom-typed sentinel value (sentinel `TermId`) represents "absent". There is no validity
bitmap anywhere in the physical record.

### Consequences

Kernels stay branchless and data-independent (Var(T)=0 friendly); decode touches one
stream, not two; the sentinel is a typed value and flows through hashing and receipts
like any other term. Consumers who need "was it absent?" compare against the sentinel.
Sentinel values must be excluded from legitimate term-id spaces by construction.

### Alternatives Rejected

Arrow-native bitmap validity (interoperability win, but reintroduces per-row branching
and a second stream); nullable Rust `Option<T>` columns (niche/layout cost, not
zero-copy stable).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-02: Eliminate Parquet Definition/Repetition Levels

### Context

Parquet's definition and repetition levels exist to reconstruct nested nullability and
list structure after column-wise shredding. AVEC's records are flat fixed-shape quad
rows; the levels are pure overhead on encode/decode and a source of data-dependent
control flow in reconstruction.

### Decision

AVEC does not use Parquet definition/repetition levels. The physical record is flat;
nesting, where it exists semantically, lives above the physical layer in stardf, not in
the column encoding.

### Consequences

Encoders and decoders drop the level machinery entirely; the on-disk reader is simpler
and constant-time per record. Full Parquet file compatibility is surrendered by design —
AVEC adopts Parquet's compression/encoding vocabulary where useful but not its nested
shredding semantics.

### Alternatives Rejected

Parquet-faithful nesting with levels (interop with the Parquet ecosystem, but overhead
and data-dependence contradicts ADR-04); an ad hoc "levels light" mode (two encodings,
two code paths, drift risk).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-03: Native Git Packfile v2 as Sequencer and Wire Format

### Context

AVEC needs an ordered, content-addressed, delta-friendly container that already has
battle-tested tooling, protocol, and replication semantics. Inventing a bespoke sequence
format re-purchases fifty years of solved problems (object model, deltitization, fetch
negotiation) with new attack surface.

### Decision

Git Packfile v2 is adopted natively as both the sequencer (ordered commit structure over
record batches) and the wire format. No translation layer between internal storage and
transport.

### Consequences

Storage and transport share one byte-level identity (see ADR-09); existing pack
tooling — index-pack, repack, delta resolution — applies directly; the git protocol
becomes the DA layer without a gateway. Constraint: the record layout must fit packfile
object granularity, and the sequencer must be expressed through git's commit/tree DAG
rather than a custom ordering block.

### Alternatives Rejected

A bespoke append-only sequence log (full control, but new format, new tooling, new
protocol); Parquet-frames-over-HTTP (loses content addressing and delta transport);
IPLD/CAR files (content-addressed, but thinner sequencing and tooling than git's).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-04: Mandate M=1 via asm Predication and Disassembly CI Gate

### Context

The M=1 law (all kernels execute exactly one instruction stream regardless of data) is
the project's data-independence guarantee. Rust source review cannot prove it: the
optimizer silently introduces branches, and a passing test suite proves only the inputs
it saw.

### Decision

M=1 is mandated at the machine-code level: hot kernels are written with asm-level
predication (cmov/select classes), and CI carries a disassembly gate that fails the
build if a banned branch pattern appears in the compiled kernel objects.

### Consequences

The guarantee is checked against the artifact that executes, not the source that
suggests; regressions are caught mechanically at the gate (a gate that can exit 1 is the
point). Cost: kernels are constrained in expressible control flow, and the gate must be
maintained against toolchain drift (optimizer upgrades can change codegen).

### Alternatives Rejected

Property-based fuzzing over inputs only (Var(T)=0 unprovable from samples); interpreter-
level branch counting at runtime (instrumentation perturbs the kernel, still input-
sampled); `#[inline(never)]` + source lint (source-level, optimizer-blind).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-05: Divan over criterion for Benchmarking

### Context

The benchmark contract (04-benchmark-contract.md) requires allocation and byte counters
alongside timing. criterion's harness adds its own overhead into the measured window and
counts allocations only via external tooling.

### Decision

Use Divan as the benchmark harness: zero-overhead measurement, native allocation and
bytes counters. A global allocator with allocation tracking backs an assertion that
decode paths perform zero allocations.

### Consequences

Allocation-count assertions become first-class CI checks, not side reports; timing
numbers are not inflated by harness overhead. Coupling: benchmark layer tracks Divan's
API; the zero-alloc assertion is only as good as the tracked allocator's coverage.

### Alternatives Rejected

criterion + separate alloc counters (mature, but harness overhead and non-integrated
counters); iai-callgrind (precise, but heavyweight toolchain dependency per CI run).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-06: Sovereign In-Repo BLAKE3 Core (crate avec-blake3)

### Context

AVEC's content addressing rests on BLAKE3 everywhere (per-record, sub-tree, tree hash).
Third-party dependency brings no_std gaps and no control over tree-hash parameterization,
while the project already carries AVEC-111 lineage (sovereign crypto core, Metal
unified-memory paths) that a vendored dependency would duplicate.

### Decision

Maintain a sovereign in-repo BLAKE3 core (`crates/avec-blake3`) rather than depending on
an external implementation. Spec compliance is verified against external b3sum
cross-vectors, not by self-attestation.

### Consequences

Full no_std control, tree-hash parameterization owned locally, existing AVEC-111 GPU
lineage reusable. Cost: the repo owns hash correctness; the cross-vector suite is the
load-bearing safety net and must run as a gate, and upstream BLAKE3 spec changes must be
tracked manually.

### Alternatives Rejected

`blake3` crate dependency (correctness inherited, but no_std/tree-hash control lost and
AVEC-111 lineage duplicated); SHA-2/SHA-3 family (no built-in tree hashing, slower
wide-input profile).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-07: Sub-BEAM Scoping — Semantic Law to stardf/graphlaw, Clocks to BEAM

### Context

AVEC sits beside a BEAM estate (stardf, graphlaw). Without a boundary, scheduling,
supervision, and semantic evaluation blur across runtimes and each surface reinvents the
others' concerns.

### Decision

Scope split: semantic law (admission, graph evaluation, receipts) lives in
stardf/graphlaw; clocks, supervision, and process reliability live in the BEAM. AVEC
adopts exactly three external specifications — Arrow, Parquet (encoding/compression
vocabulary per ADR-02), and Git Packfile v2 — and nothing else.

### Consequences

Each plane stays replaceable on its own receipt; AVEC's conformance surface is three
named specs, mechanically checkable. Cost: cross-boundary features (e.g. timed admission)
must negotiate an explicit interface instead of reaching across.

### Alternatives Rejected

AVEC as a self-contained runtime (duplicates supervision and clocks); full BEAM
assimilation of semantic law (couples semantics to scheduler internals).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-08: Non-Nullable Sentinel Encoding over Bitmap Validity

### Context

Derived from the same design session as ADR-01, from the encoding side: the question was
not only whether bitmaps are dropped, but what physically replaces them per quad.

### Decision

Per-quad absent-ness is encoded as a bottom-type `TermId` sentinel stored inline in the
non-nullable value stream. The sentinel is a legal machine value and an illegal semantic
term.

### Consequences

Zero secondary metadata; sentinel membership is a single predicated compare, M=1
compatible (see ADR-04). The bottom-type value must be reserved out of the allocated
TermId space and asserted at mint time.

### Alternatives Rejected

Validity bitmap (ADR-01's rejected case); NaN-style tagged words in the value itself
(couples absence to value representation, breaks typed hashing).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-09: Storage Format == Wire Format

### Context

A serialization split (internal format ↔ transport format) creates a translation layer:
two encoders, two decoders, a drift surface, and double-hashing at every boundary.

### Decision

Git packfiles (ADR-03) are both the on-disk storage format and the wire format; the git
protocol serves as the DA layer. What is stored is byte-for-byte what is transmitted.

### Consequences

Content identity is transport-independent — a receipt computed on disk replays on the
wire and vice versa; no gateway translation; differential testing collapses to one
format. Constraint: any future transport (e.g. non-git DA) must carry packfiles verbatim
or it breaks the identity.

### Alternatives Rejected

Internal format + gRPC/HTTP framing (flexibility, but a second encoder and hash
recomputation); Arrow IPC as wire format (columnar-friendly, but not content-addressed
or delta-transported).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-10: Inline Digest Column over Post-Hoc Hashing Passes

### Context

Integrity verification over batches can be computed after the fact by walking records,
or carried at write time. Post-hoc passes re-read data, serialize the hashing cost at
verify time, and drift from the write path's byte identity.

### Decision

Each record carries an inline 32-byte digest column: a per-quad sub-tree receipt,
computed at write time and stored at the byte level.

### Consequences

Verification is a read of an already-computed column; partial/sub-tree verification is
addressable without full-batch walks; the write path absorbs the hashing cost once.
Cost: 32 bytes per quad of storage and mandatory write-time hashing; the digest column
must itself be covered by the packfile's object hashing (ADR-03) or trust splits.

### Alternatives Rejected

Post-hoc hashing passes (no storage cost, verify-time latency and drift risk); Merkle
root only per batch (cheap, but no sub-batch verification granularity).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-11: Policy Mask and Vector Clock Columns in the Physical Record

### Context

Admission metadata (policy evaluation results, causal ordering) is conventionally
re-derived at read time from side tables. Re-derivation couples the read path to the
policy engine and reintroduces data-dependent control flow.

### Decision

The physical record carries policy mask and vector clock columns inline: pre-evaluated
admission metadata stored at the byte level, not re-derived at read time.

### Consequences

Reads are admission-complete from bytes alone; the hot path never calls the policy
engine; vector clocks ride the same zero-copy layout. Cost: record width grows, and
policy re-evaluation is a rewrite (mask is a snapshot, not a live view) — late policy
changes require record migration, not just re-reads.

### Alternatives Rejected

Side-table policy state joined at read (no write-time cost, but read-path coupling and
join latency); embedding an interpreter reference instead of a mask (defers evaluation,
keeps data-dependence).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-12: Fixed 16-Stage Ladder over Data-Dependent Loop Binary Search

### Context

Search-shaped kernels (e.g. index lookup inside a batch) invite binary search: O(log n)
but data-dependent — iteration count varies per input, which is exactly the variance M=1
forbids.

### Decision

Fixed 16-stage ladder: a constant, fully unrolled 16-step search over the record space.
Every input executes exactly 16 stages; Var(T)=0 by construction.

### Consequences

Constant time and constant instruction stream, M=1 compatible (ADR-04); the unrolled
ladder vectorizes and pipelines without branch prediction. Cost: 16 stages cap the
searchable range (2^16) and over-scan small inputs — the cap must be a spec constant,
enforced at encode time like any other fixed-shape law.

### Alternatives Rejected

Data-dependent loop binary search (fewer steps on average, Var(T)>0, gate-hostile);
linear scan (Var(T)=0 but O(n), unusable at batch scale); hardware BSF/CLZ tricks
(platform-dependent instruction streams, breaks cross-target M=1 proofs).

### Status

Proposed. Standing UNVERIFIED — no implementation receipt.

## ADR-13: In-Crate DEFLATE (Fixed-Huffman LZ77 + zlib Wrapper) for Pack Bodies

### Context

Git packfile object bodies are zlib-deflated; a conformance-faithful pack writer must
emit deflated bodies or `git fsck` and index-pack reject the pack. The dependency
closure for v26.10.31 carries no zlib crate, and closure discipline forbids adding one
late in the milestone. Wave-3/4 fsck courts exposed this on the first real pack write.

### Decision

Implement DEFLATE in-crate: fixed-Huffman LZ77 with the zlib wrapper, as
compressor and inflate, living inside the avec-git crate (~600 lines in-tree).

### Consequences

The fsck court passes against real git tooling; the repo owns compression
correctness. Fixed-Huffman trades ratio for code simplicity — acceptable for
pack bodies. A dynamic-Huffman upgrade is a direction to consider later; the
wrapper/parser surface is stable under that swap.

### Alternatives Rejected

Stored (uncompressed) bodies — git rejects them as malformed; external zlib
dependency — closure discipline.

### Status

Accepted-for-v26.10.31 (wave-3/4: fsck court pass receipt; in-tree
compressor+inflate).

## ADR-14: M=1 Remainder Policy — 0 Data-Dependent Branches on Hot Data Paths

### Context

The original 0-banned-opcodes reading of ADR-04 proved unreachable for any
allocation-performing kernel: allocator null-checks and runtime trip-count latches
are inherent to the machine code the toolchain emits, not kernel-author choices.
Treating them as gate failures would either falsify the gate or freeze the build.

### Decision

Refine the policy to a scoped remainder: hot data paths carry 0 data-dependent
branches (the ADR-04 guarantee, unchanged where it bites); loop latches and
assert-guard branches are documented as accepted remainder. The gate stays
0-tolerance and reports honestly — remainder sites are listed, never silenced.

### Consequences

The gate remains able to exit 1 and remains honest about what it sees; the M=1
claim is stated at the granularity it can actually be proven (data paths), not
as an unreachable absolute. Cost: the accepted-remainder list is a reviewed
surface — growth is a diff, not a default.

### Alternatives Rejected

Absolute 0-banned-opcodes (unreachable for allocating kernels; forces gate
vacuum or lies); relaxing to source-level review only (optimizer-blind, the
ADR-04 rejected case).

### Status

Accepted-for-v26.10.31 (wave-3/4: disassembly audit + gate output receipts).

Wave-8/9 closure: the manifest-v2 disassembly inventory
(receipts/gate-branchless-manifest-v2, 7 kernels, GATE PASS, exit 0) closes
the inventory step — every manifested kernel site carries a declared
INFORMATIONAL gate mode; none is ENFORCED at v26.10.31.

## ADR-15: BLAKE3 NEON 4-Way, Not 8-Way

### Context

The spec's 10 GiB/s/core target (LSL 8.5) suggested 8-chunk-wide NEON
interleaving. aarch64 offers 32 v128 registers; 8 simultaneous chunks need 8x
chaining state plus 8x message words plus round constants — the register budget
cannot hold it, and spills would erase the interleaving gain.

### Decision

NEON leaf compression runs 4-way (4 chunks in flight), the width the register
file actually supports. Reaching the 8.5 GiB/s LSL at 4-way requires a
message-load transpose plus group interleaving; that work is wave-5 L2,
currently in flight.

### Consequences

Feasibility caveat recorded: the LSL at 4-way may need re-derivation if the
transpose+interleave work does not close the gap; spec amendment in
26-blake3-engine-spec.md records the measured numbers and the honest gap.

### Alternatives Rejected

8-way NEON (register budget exceeded — spills dominate); scalar-only
(conformance-fine, throughput-gate-fatal).

### Status

Accepted-for-v26.10.31 (wave-3/4 NEON receipts; wave-5 L2 open).

### Amendment (wave-6): Throughput Trajectory and the LSL Decision Point

Measured trajectory of the NEON leaf-hash throughput, each row a receipted
measurement (256 MiB, min-of-5 rounds unless noted):

| wave | measurement | GiB/s |
|---|---|---|
| 3 | baseline NEON leaf hash | 0.43 |
| 5 | 4-way kernel | 0.938 |
| 5 | `hash_many_simd` (ctq_report CTQ-06) | 1.18 |
| 6 | message-load transpose + group interleave | IN-FLIGHT |

Wave-6 poll at amendment time: `cargo test -p avec-blake3 --test
multi_chunk_vectors` passes (3/3); a fresh 256 MiB run of
`target/release/ctq_report` re-observes `hash_many_simd` at 1.18 GiB/s
(digest-correct) and `hash_wide_simd` at 0.43 GiB/s, both FAIL against the
LSL 8.5 gate. No wave-6 transpose/interleave number is observable yet.

Decision point. The trajectory is 2.7x from baseline and the remaining gap
to the LSL is roughly 7x. Exactly one of:

(a) **The LSL 8.5 is reachable with more kernel work.** Named levers:
message-load transpose (removes per-lane load shuffles), group interleaving
(overlaps dependency stalls across chunk groups), and 2x4-way streams
(two independent 4-way kernels resident, bounded by the same 32-register
file ADR-15 was derived from). Each lever must show a receipted increment;
if the levers saturate below the LSL, option (b) is forced.

(b) **Re-derive the LSL.** The 10 GiB/s/core design target (LSL 8.5 gate)
was set in the PRD charter before the leaf architecture was known — before
the 4-way NEON width constraint (this ADR) and the measured cost structure
of the sovereign core. A target set before the architecture that must
achieve it is a hypothesis, not a requirement; re-derivation from measured
per-round costs is lawful and must be recorded as a spec amendment in
26-blake3-engine-spec.md, never as a silent gate relaxation.

Position at amendment time: option (a) stays open until the wave-6
transpose/interleave receipts land. The gate continues to report FAIL
honestly at 1.18; it is not relaxed and not silenced.

### Status (amended)

Accepted-for-v26.10.31 (wave-3/4 NEON receipts; wave-5 L2 landed at 1.18;
RATIFIED wave-12: LSL 1.0/1.5/2.5 executed as edit set A/A2/C/D/E/F; the refuted targets are retained as the falsification record; see Amendment
(wave-7)).

### Amendment (wave-7)

The wave-6 transpose/interleave work landed and the assembly evidence REFUTES
the register-residency premise of this ADR: LLVM had already folded the
const-table permute; the round loop shows 0 spill-relative loads after the
literal-index rewrite, at a cost of only +0.2%. Register pressure was never
the binding constraint.

Re-attributed bottleneck: the serial g4 dependency chain under shared NEON
ALU throughput — the rounds serialize on their own chaining state, not on
spills.

Decision point (b) is therefore armed. 48-lsl-rederivation.md derives the
Case-A ceiling at 1.1–3.1 GiB/s/core (Case-B 4.6–12.5) and concludes 8.5 LSL
is a spec error. A PRD amendment is PROPOSED (LSL 1.0 / Target 1.5 / USL 2.5,
filed against 20-prd.md). The LSL_RATIFIED hook still sits None in
crates/bench/src/bin/ctq_report.rs — the gate keeps reporting FAIL at the old
LSL until the amendment is ratified.

### Amendment (wave-8)

The 8-wide g4x2 interleave candidate was built and REFUTED: spill-bound in
the prologue (`sub sp, #1552`), reverted. Kernel ceiling unmoved at
1.36 GiB/s. This also settles the attribution split with the wave-7
amendment above: the serial g4 dependency chain binds the 4-way kernel
(the re-attributed bottleneck); state spill binds the 8-wide candidate
(the rejected 8-way direction of this ADR's original Context). The
register-budget premise is dead for 4-way, confirmed live for 8-wide.
Decision point remains (b), armed.

## ADR-16: SHACL Closed-Shape Whitelisting via ignoredProperties

### Context

The SHACL engine, once run against the repo's own fixtures, found the fixture
itself violating closed-shape constraints: `rdf:type` and `prov:generated`
triples are structural/lineage metadata present on every node, not shape-domain
properties. A closed shape without whitelisting rejects valid graphs.

### Decision

Closed shapes carry an explicit `sh:ignoredProperties` whitelist covering
`rdf:type` and `prov:generated`. All other properties remain inside the closed
shape's validation surface.

### Consequences

Fixtures validate against their own shapes; the whitelist is explicit and
reviewable, so closure semantics stay strict for domain properties. Any new
cross-vocabulary metadata property requires a reviewed whitelist entry — the
same reviewed-surface discipline as ADR-14's remainder list.

### Alternatives Rejected

Open shapes (defeats the closed-shape guarantee); stripping
rdf:type/prov:generated from fixtures (loses provenance and typing in the
graph itself).

### Status

Accepted-for-v26.10.31 (wave-3/4: engine fixture self-violation finding).

## ADR-17: Scalar Fixed-Stride Transpose Shipped; NEON ZIP/UZP Transpose REFUTED-as-Unbuilt

### Context

Spec 23 §3.1 named per-architecture transpose kernels (aarch64 `ZIP1/ZIP2`/
`UZP1/UZP2`, x86_64 `vpunpck*`/`vperm2i128`, wasm32 `v128.shuffle`) and spec
27 §1 tabled `VZIP1/2Q`/`VUZP1/2Q`/`VTRN` as the quad-transpose primitives.
The V2 validation lane (69-validation-receipt.md) found none of it was ever
built: `crates/avec-arrow/src/transpose.rs` carries no `cfg(target_arch)`
NEON/AVX path — NEON exists only in `silicon.rs` and avec-blake3. What
shipped is a fixed-stride scalar transpose with 19 total branch sites
(12 in `transpose::transpose` + 7 in `QuadColumns::untranspose`), every site
an ADR-14 remainder class (allocator null-checks, zero-trip guards,
capacity guards, loop latch); the in-loop 32-byte digest copies are fixed
wide loads/stores, straight-line — 0 data-dependent branches on the data
path. The 7 untranspose sites were additionally invisible to both gates
(ltmp0 symbol rendering; see the `quad untranspose` manifest entry) until
registered.

### Decision

Record the refutation and keep the scalar transpose: the round-trip courts
are green (5/5) and performance is adequate for the milestone; the NEON
transpose becomes an open optimization item, not an unimplemented claim.
The spec text named a kernel that does not exist; this ADR is the
correction, and 23 §3.1 / 27 §1 carry one-line notes pointing here.

### Consequences

The spec's per-instruction transpose cost estimates (~2 instructions per
field column) describe a kernel with no receipt — any future NEON transpose
work re-derives from this ADR plus the branchless manifest, not from the
spec's cost table. The untranspose kernel enters
`scripts/branchless_manifest.toml` as a documented blind spot (the body
renders `<ltmp0>` under both Apple objdump and llvm-objdump 21.1.7, so the
gate attributes 0 in-scope instructions to it; the authoritative 7-site
count comes from direct disassembly of the nm-bounded region). If the NEON
item is taken, it lands as a new kernel with its own manifest entry and
gate receipt — the scalar remainder rows are then re-adjudicated, not
silently replaced.

### Alternatives Rejected

Building the NEON transpose late in the milestone (a SIMD kernel landing
with no performance receipt to justify the audit surface); re-labeling the
scalar kernel as NEON in the spec text (false claim).

### Status

Accepted-as-recorded (2026-10-30: V2 lane finding; manifest `quad
untranspose` entry; receipt gate-branchless-manifest-v4 GATE PASS with the
blind spot disclosed).

## ADR-18: Per-Plane Byte Order — Quad Plane LE, Packfile Plane BE-by-Git-Law

### Context

OQ-02 flagged that the two container specs state opposite byte orders: 23 §5
pins little-endian for the quad plane, while 25 §1/§2 pins big-endian integers
for the packfile plane. The packfile side is not a repo choice: git packfile
v2 / idx v2 format law fixes its integer fields BE, and that law is what
ADR-03's interop (index-pack, fsck, real-git acceptance) rides on. The quad
plane carries no external byte-order constraint.

### Decision

Per-plane byte order, both pinned. Quad plane: little-endian everywhere (23
§5); big-endian targets byte-swap explicitly at the encode/decode seams
(`layout.rs` `to_le_bytes`/`from_le_bytes`). Packfile plane: big-endian
exactly as git format law requires (25 §1-2), read/written via
`from_be_bytes`/`to_be_bytes` (`idx_read.rs`, `deflate.rs` adler32). Each
plane's order is fixed at its own seam; no normalizing translation layer.

### Consequences

Two byte orders coexist deliberately, one per plane. Cross-plane moves stay
byte-verbatim: a quad chunk's bytes are its own LE wire bytes; only packfile
framing integers (version, counts, fanout/offset/CRC tables) are BE — BE never
touches quad field bytes, so mixed-endian hazards are confined to packfile
framing parsers. LE side receipted: layout tests assert explicit LE byte
sequences (gate-arrow-tests-v3; live re-run 2026-10-31, 14 passed). BE side
receipted via real-git acceptance courts (V7, 48/0). Residual, named: the
byte-swapped-fixture cross-plane property test (OQ-02) does not exist yet.

### Alternatives Rejected

Single-endian (LE) everywhere — forces re-swapping packfile framing against
git format law and forfeits the ADR-03 interop that motivates BE; runtime-
native endianness — layout instability across targets, violates the pinned
wire law; a normalizing translation layer — a second encoder/decoder, the
ADR-09 rejected case.

### Status

Accepted-for-v26.10.31 (wave-11 lane 3, 2026-10-31: LE seams + tests in
layout.rs; BE reads in idx_read.rs/deflate.rs; OQ-02 closed ADMITTED-DESIGN
with the swapped-fixture test residual named).

## See Also

- [21-ard.md](21-ard.md) — Architecture Requirements Document
- [04-benchmark-contract.md](04-benchmark-contract.md) — benchmark contract (Divan
  counters, zero-alloc gates)
- [20-prd.md](20-prd.md) — PRD for this milestone
- [18-m1-branchless-algebra.md](18-m1-branchless-algebra.md) — M=1 algebra background

## ADR-19: CTQ limits amendable when proven impossible (wave 12)

Context: the CTQ-06 case proved a spec limit can be physically
unattainable on the target hardware class while the engineering is sound.
Decision: the operator ratifies a general amendment clause — any CTQ limit
may be re-derived via the 48 pattern (ceiling math + measured distribution
+ architecture attribution + retained refutation record). Consequences:
limits are hypotheses with falsifiers, not constants; every re-derivation
leaves the old receipts standing. Alternatives rejected: immutable limits
(would have kept a proven-impossible 8.5 gate FAILing forever), silent
relaxation (breaks the receipt discipline). Status: RATIFIED 2026-10-31.
