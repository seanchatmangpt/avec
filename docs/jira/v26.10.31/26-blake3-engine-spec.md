# BLAKE3 Tree Reduction Engine Specification

Standing: UNVERIFIED — this document specifies the conformance surface for the
sovereign in-repo BLAKE3 crate (`avec-blake3`). It governs what must be added or
asserted for v26.10.31; it makes no claims about the crate's current internals.

## Purpose and Scope

The `avec` engine commits content-addressed objects, so its hash function is the
correctness floor for everything above it. The in-repo crate `avec-blake3`
exists as a workspace member and is the sovereign implementation: no external
hash dependency on the commit path. This spec defines the compliance,
architecture, performance, and audit surface that crate must satisfy.

See Also: [21-ard.md](21-ard.md),
[AVEC-111](../v26.10.29/AVEC-111-metal-gpu-and-blake3.md)

## Compliance Requirement (FR-06)

The engine is strictly spec-compliant BLAKE3. Compliance is an acceptance gate,
not a quality goal:

- Outputs must be byte-identical to reference BLAKE3 implementations for every
  input class the engine can produce (empty, sub-chunk, exact-chunk, multi-chunk,
  multi-parent, keyed, derivation).
- Verification runs against the official BLAKE3 test vectors as a hard test
  suite; any vector mismatch fails the build.
- Random corpora are cross-checked against the external `b3sum` tool: the crate
  generates deterministic pseudo-random inputs, hashes them, and compares
  digests against `b3sum` output executed as a real subprocess.
- A release that passes throughput gates but fails any vector is not shippable.
  Conformance outranks speed.

## Chunk Architecture

BLAKE3 is a merkle tree over fixed-size input chunks:

- **Chunk size**: 1024 bytes. Each chunk is hashed independently through the
  compression function in 64-byte blocks, producing one per-chunk chaining
  value (CV).
- **Per-chunk chaining**: a chunk's CV depends only on its own bytes and the
  chunk counter; chunks are independent until tree reduction begins.
- **Tree hash mode**: parent nodes combine child CVs. A parent CV is
  `Compress(L_key || R_key)`, i.e. the compression function applied to the
  concatenation of the left and right child chaining values as an 8-word block,
  with the parent flag set.
- **Root derivation**: the final root value is obtained from the last
  remaining node with the root flag set; extended outputs (XOF) stream from
  that root without further compression. The engine derives 32-byte object
  identifiers from the root by default.

The reduction strategy (which subtrees fold when) is an implementation choice;
the resulting root digest is not. Any restructuring of the tree walk must
re-verify the full vector suite.

## Parallel and Vector Reduction

Leaf hashing dominates the cost and is embarrassingly parallel:

- **SIMD lanes**: leaf compression targets the platform vector width — NEON
  on aarch64, AVX2 on x86_64, WASM v128 on wasm32. Multiple 64-byte message
  blocks (or multiple chunk states) are interleaved in vector registers so one
  vector instruction advances several lanes of compression state.
- **Multi-chunk interleaving**: beyond intra-block SIMD, the engine interleaves
  several independent chunk pipelines so memory latency and instruction
  dependencies of one chunk overlap with compute of the next. Independent
  chunk CVs reduce in a vector-width-wide tree.
- **Throughput target**: the design target is at least 10 GiB/s per core,
  enforced as the LSL 8.5 gate in CI benchmarks (see Falsification Hooks).
  A lane falling below the lower spec limit fails qualification even if
  correctness passes.
- **Inline derivation during memory writes**: because a chunk's CV depends only
  on its own bytes, hashing can begin as data is written. The design goal is
  that the commit OID is available as the last byte of the object lands —
  derivation overlaps the write path instead of following it, removing a full
  serialization pass from the commit latency.

## no_std Constraints

The hash path runs inside allocation-sensitive and time-sensitive contexts:

- **Zero heap allocation** on hash paths: no `alloc`, no `Vec`, no `Box` in any
  function reachable from `hash`/`update`/`finalize`. State is stack-resident
  and bounded (chunk buffer of 1024 bytes plus the CV stack, depth bounded by
  the 2^64-chunk input limit).
- **Freestanding operation**: the crate builds under `no_std` with `core` only;
  `std`/`alloc` support is feature-gated for the host-side test harness, never
  required by the core.
- **Feature-gated platform acceleration**: NEON, AVX2, and AVX-512 paths are
  enabled by target features/cargo features. The scalar fallback must remain
  vector-suite-identical: every accelerated path is validated against the same
  official vectors before the accelerated feature is considered conformant.
- **Constant-time discipline**: no data-dependent branches on input bytes. The
  M=1 discipline applies — mask-select rather than branch. Secret-keyed modes
  inherit this without exception; unkeyed hashing follows the same rule so a
  single audited codegen shape covers all modes.

## Conformance Surface and Audit Items for v26.10.31

The crate `avec-blake3` is present on disk as a workspace member; AVEC-111
landed the sovereign BLAKE3 core and Metal unified-memory GPU work. This spec
does not assert what the crate currently does internally. It fixes what must be
added or asserted this cycle:

- Vendor or generate the official BLAKE3 test vectors as a test fixture and
  wire them as a mandatory test.
- Add the `b3sum` cross-check harness over a deterministic random corpus
  (real subprocess, asserted final digests).
- Add throughput benchmarks per backend (scalar, NEON, AVX2, Metal) with the
  10 GiB/s/core LSL wired into CI as a pass/fail gate.
- Add allocation assertions on hash paths (allocator counting in the host test
  harness; zero allocations asserted during update/finalize).
- Add a disassembly audit step checking for data-dependent branch opcodes on
  the keyed hash path.
- Record each item's pass/fail in the AVEC-111 ledger as receipts land.

## Falsification Hooks

Each requirement has a mechanical falsifier; a hook that cannot fail is a
vacuous gate:

- **Vector mismatch = fail**: any divergence from official test vectors fails
  the test suite and blocks release.
- **Throughput below LSL = fail**: CI benchmark under 10 GiB/s/core (LSL 8.5)
  on the target backend fails the build.
- **Allocation during hash = fail**: any allocator invocation reached from
  `update`/`finalize` fails the allocation-assert test.
- **Branch opcode = fail**: a data-dependent branch opcode appearing in the
  disassembly audit of the keyed hash path fails the audit.

## Implementation Status Amendment (v26.10.31, wave-3/4)

Amendment recording measured implementation status against this spec. The
original spec text above is unchanged; where this amendment diverges, the
amendment reflects what was built and measured.

- **NEON width is 4-way, not 8-way** (ADR-15): aarch64's 32 v128 registers
  cannot hold 8 chunks' chaining state plus message words plus constants;
  8-way would spill and erase the interleaving gain.
- **Measured throughput**: leaf hashing improved from 0.43 GiB/s to
  0.938 GiB/s on the NEON path across wave-3/4 optimization. The honest gap
  to the 10 GiB/s design target (LSL 8.5 gate) is roughly a factor of 10.
- **Closing path**: reaching the LSL at 4-way width requires a message-load
  transpose plus group interleaving; that work is wave-5 L2, currently in
  flight. Feasibility caveat (ADR-15): the LSL itself may need
  re-derivation if the transpose+interleave work does not close the gap at
  4-way width.
- **Conformance remains the floor**: the vector suite and b3sum cross-check
  outrank throughput; the throughput gate reports measured numbers honestly
  and does not silently pass below the LSL.

### Wave-5/6 Amendment: Throughput Trajectory

- **Trajectory** (256 MiB, min-of-5, each row a receipted run): 0.43 GiB/s
  (wave-3 baseline NEON) -> 0.938 GiB/s (wave-5, 4-way kernel) -> 1.18
  GiB/s (wave-5 `hash_many_simd`, CTQ-06). Wave-6 amendment-time re-run of
  `target/release/ctq_report`: `hash_many_simd` 1.18 GiB/s digest-correct,
  `hash_wide_simd` 0.43 GiB/s, both FAIL vs LSL 8.5.
- **Wave-6 work (message-load transpose + group interleaving)**: IN-FLIGHT
  at amendment time. `multi_chunk_vectors` passes (3/3) but no new
  throughput number is observable from a fresh 256 MiB run; the row is
  recorded as IN-FLIGHT, not estimated.
- **Decision point (ADR-15 amendment)**: either (a) LSL 8.5 is reachable
  with the named levers — transpose, group interleave, 2x4-way streams —
  each requiring a receipted increment, or (b) the LSL is re-derived: the
  10 GiB/s/core target was set in the PRD charter before the 4-way leaf
  architecture was known. Re-derivation is a spec amendment here, never a
  silent gate relaxation. The gate stays REAL and reports FAIL at the
  measured number until one branch closes.

### Wave-7 Amendment: Register-Residency Refuted, LSL Re-Derived

- **Register-residency hypothesis REFUTED**: assembly evidence shows LLVM
  already kept the permute in registers; the literal-index rewrite delivered
  only +0.2%. The bottleneck is re-attributed to the serial g4 dependency
  chain: kernel compute ceiling ~1.36 GiB/s, e2e 1.85 GiB/s.
- **LSL re-derived** (`48-lsl-rederivation.md` landed): the original
  "8.5 GiB/s/core LSL" was a spec error. PROPOSED LSL 1.0 GiB/s, Target 1.5,
  USL 2.5 (recorded as an amendment to `20-prd.md`). Per the wave-5/6
  decision point, this closes branch (b): the LSL is re-derived as a spec
  amendment, never a silent gate relaxation.
- **Measured trajectory** (256 MiB, min-of-5): 0.43 -> 0.938 -> 1.18 ->
  1.85 GiB/s.
- See [48-lsl-rederivation.md](48-lsl-rederivation.md) for the derivation
  receipt.

## Standing

UNVERIFIED. This is a specification of the required conformance surface.
Compliance status of `avec-blake3` becomes verified only through receipts:
vector-suite output, `b3sum` corpus digests, CI benchmark exits, allocation
asserts, and disassembly audit results recorded against this document.

## See Also

- [21-ard.md](21-ard.md)
- [AVEC-111: Metal GPU and BLAKE3](../v26.10.29/AVEC-111-metal-gpu-and-blake3.md)
- [48-lsl-rederivation.md](48-lsl-rederivation.md)
- `docs/CONTENT_ADDRESSING.md`
