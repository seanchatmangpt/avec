# 64-Byte Quad Physical Layout Specification

Status: LANDED for the physical layout — `Quad64` in crates/avec-arrow/src/layout.rs
conforms (const size/align/offset asserts; wave-7 court; see Conformance above Standing).
Section 6 remains the audit-item list. Scope: canonical physical layout of the 64-byte
aligned quad record. Companions: 21-ard.md (architecture),
19-ontology-ttl.md (semantic layer).

## 1. Field Table

The quad record is exactly 64 bytes: one cache line on every supported
target (x86_64, aarch64, wasm32). Every offset is both a field start and the
next field's natural alignment boundary. There is no implicit padding.

```
Offset  Width  Name           Type     Semantics
0x00    4      s_id           u32      subject TermId
0x04    4      p_id           u32      predicate TermId
0x08    4      o_id           u32      object TermId
0x0C    4      g_id           u32      graph/context TermId (named graph)
0x10    8      t_ns           u64      monotonic timestamp, nanoseconds
0x18    4      vector_clock   u32      causal counter for the record's causality
0x1C    4      policy_mask    u32      pre-evaluated permission/duty bitmask
0x20    32     inline_digest  [u8;32]  inline sub-tree BLAKE3 receipt
```

Field semantics:

- **TermId** (`s_id`, `p_id`, `o_id`, `g_id`): u32 index into the dictionary
  of interned terms. Interning protocol and dictionary growth live in
  21-ard.md. This document pins width, slot, and sentinels (Section 1.1).
- **t_ns**: monotonic nanoseconds, not wall-clock. This document pins the
  encoding (u64, little-endian) and slot; the epoch contract lives in
  21-ard.md.
- **vector_clock**: a single u32 causal register. The name is contract, not
  implementation: any upgrade to a multi-register causal structure is a
  layout-version change per Section 5.
- **policy_mask**: evaluated at write time, stored inline, so read-path
  permission checks are mask tests, not re-evaluations. Bit assignment is
  owned by the policy layer (21-ard.md); this document pins slot and width.
- **inline_digest**: 32-byte BLAKE3 content address of the sub-tree rooted
  at this quad. Always present, never sparse. A record with an unset digest
  is invalid and must be rejected at decode.

## 1.1 Sentinel Bottom Type

Two TermId values are reserved and pinned here:

```
0x0000_0000  BOTTOM    "null"/unbound — deterministic zero, consistent with
                       the avec-arrow convention (timestamps.rs:79, "null
                       slot is the deterministic zero")
0xFFFF_FFFF  WILDCARD  match-anything pattern sentinel
```

Neither is a valid interned term index; interning must refuse to mint either
as a real term. Pattern semantics: `BOTTOM` matches only itself; `WILDCARD`
matches any TermId including `BOTTOM` unless the pattern engine states
otherwise (pattern-engine contract, not layout: 21-ard.md).

## 2. Alignment Rules

1. `Quad` is `#[repr(align(64))]`; `size_of == 64` and `align_of == 64` are
   conformance assertions, not conventions.
2. Every quad buffer is allocated on a 64-byte boundary. The existing
   `Primitive::buffers_aligned_64` law (primitive.rs:103-107, charter M-03)
   is the precedent and the reuse: quad storage reuses this allocator path.
3. A block of 4 consecutive records is exactly 256 bytes — four cache lines,
   no straddling. This is the transpose unit for Sections 3.1 and 4.
4. Split-line records are forbidden: a record never spans a cache-line
   boundary. This follows from (1)-(3) but is stated as a law because it is
   what makes the Section 3.1 zero-copy path legal.
5. No implicit padding: any future field addition keeps explicit offsets or
   bumps the layout version (Section 5).

## 3. Arrow Projection

Canonical Arrow schema, all columns non-nullable — nullability lives in the
sentinel convention (Section 1.1), not a validity bitmap. This mirrors the
existing avec-arrow law that all-valid inputs drop the validity buffer
(primitive.rs:173-176).

```
s_id           UInt32
p_id           UInt32
o_id           UInt32
g_id           UInt32
t_ns           Timestamp(ns)
vector_clock   UInt32
policy_mask    UInt32
inline_digest  FixedSizeBinary(32)
```

Arrow C data interface format strings:

```
UInt32               "I"
Timestamp(ns)        "ttn"
FixedSizeBinary(32)  "w:32"
```

Note: format strings follow the Arrow C data interface `format` descriptor
grammar. Implementations must verify against the Arrow C data interface spec
as the authority, not against this document.

DuckDB types:

```
s_id, p_id, o_id, g_id     UINTEGER
t_ns                       TIMESTAMP_NS
vector_clock, policy_mask  UINTEGER
inline_digest              BLOB
```

### 3.1 Zero-Copy Path

The record stream is interleaved (AoS); Arrow consumption is columnar (SoA).
The transpose is the mandatory seam: 4 records per iteration, 256 bytes in,
256 bytes out, with loads/stores aligned because the 4-record block is
256-byte aligned (Section 2, rule 3). Per-architecture kernel expectations
are specified in docs 25/27 — this document references, not duplicates.

- aarch64 (NEON): `ZIP1/ZIP2` and `UZP1/UZP2` perform the 4x4 u32 transpose
  in-register (~2 instructions per field column per block).
- x86_64 (AVX2): `vpunpckldq/vpunpckhdq` intra-register pass plus
  `vperm2i128` (or `vinserti128`/`vextracti128`) for the 128-bit lane
  crossing, classic 4x4 transpose (~2-3 instructions per field column).
- wasm32: `v128` shuffles (`i8x16.shuffle` composition) implement the same
  4x4 transpose within the 128-bit SIMD unit.

> Correction (2026-10-30, ADR-17): the NEON/AVX2/wasm transpose kernel named
> in this section was specified, never built — the shipped transpose is
> fixed-stride scalar (see 22-adr-register.md ADR-17).

## 4. Interleaved vs Columnar

The record stream is AoS because the write path is record-at-a-time; Arrow
consumption is SoA. The transpose (Section 3.1) is the required seam.

- Write path: append one 64-byte record; the 4-record block (256 B) is the
  transpose unit.
- Read path: transpose one 4-record block per iteration.
- Kernel specifics per architecture: 25/27 (reference, do not duplicate).

Why 4-record blocks: 4 x 64 B = 4 cache lines, the largest power-of-two
block that fits the 4x4 u32 transpose idiom on 128-bit SIMD units without
spill/fill. Larger blocks add no transpose parallelism on 128-bit lanes;
smaller blocks leave lanes idle.

## 5. Versioning & Compatibility

- **Layout version tag**: u16 stored in the buffer header next to the record
  count, not per-record (per-record storage would burn 2 bytes per record).
  Layout version 1 = this document.
- **Endianness**: little-endian, pinned. Every multi-byte field is LE on the
  wire and in storage buffers; big-endian targets byte-swap explicitly.
  Reference reads: `u32::from_le_bytes`/`u64::from_le_bytes` — the existing
  views.rs decoder already reads all fields via `from_le_bytes`
  (views.rs:63-67).
- **Forward compat**: the top 4 bits of `policy_mask` (`0xF000_0000` region)
  are reserved. Writers must clear them; readers must ignore unknown bits.
- **No implicit padding**: changes within a version are additive-only; any
  structural change bumps the version tag. Adding fields beyond the current
  64-byte budget is a new version with a new record size, never silent
  growth.

## 6. Cross-Check Against crates/avec-arrow

Audit items — where existing code must be checked against this spec. These
are audit items, not claims about current behavior.

- **A1 primitive.rs** — `buffers_aligned_64` (~103-107) covers buffers;
  quad storage must satisfy the 64-byte law at record granularity too.
  Audit allocator reuse for a 64-byte-aligned record arena.
- **A2 batch.rs** — `Column::UInt32` with format `"I"` (batch.rs:36-37,
  103): audit projection against Section 3. No variant exists for
  `Timestamp(ns)` or `FixedSizeBinary(32)`; if absent, both are missing.
- **A3 timestamps.rs** — confirm null-slot-zero (line 79) matches
  `BOTTOM = 0x0000_0000`, and the `t_ns` epoch matches the 21-ard.md
  contract.
- **A4 views.rs** — descriptor reads are `from_le_bytes` (63-67); audit
  that no crate decoder reads u32/u64 native-endian.
- **A5 generated_layout.rs** — slot tables are 64-bit-centric; audit that
  32-bit and 32-byte fixed-width slots do not conflict with the tables.
- **A6 (this spec)** — `inline_digest` semantics must stay single-sourced
  in 21-ard.md/blake3.rs; audit on next edit pass.

## Conformance (wave 7)

Verified against `crates/avec-arrow/src/layout.rs`: `Quad64` with const
size/align/offset asserts (layout.rs:59-69); offsets 0x00-0x20 and the
BOTTOM/WILDCARD sentinels match this spec exactly.

## Standing

LANDED for the physical layout: a `Quad`-shaped record type with
`#[repr(align(64))]`-equivalent const size/align assertions exists and
conforms. The Section 6 audit items remain open as code-level verification
tasks for the projection/decoder surface; the transpose and Arrow-projection
courts are receipted separately (52-courts-audit.md).

## See Also

- 21-ard.md
- 19-ontology-ttl.md

Conformance (wave 7): moved adjacent to Standing (above); verified against
crates/avec-arrow/src/layout.rs, offsets 0x00–0x20 and sentinels match
exactly.
