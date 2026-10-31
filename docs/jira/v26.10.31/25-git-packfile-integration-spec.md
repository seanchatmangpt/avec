# Git Packfile v2 Integration Specification

Status: UNVERIFIED design. Owner: lane 6, wave v26.10.31. Byte-level integration
contract for the avec sequencing plane: how avec-authored state snapshots ride in
standard Git Packfile v2 so that unmodified `git` verifies them (FR-04).

Companions: [06-stardf-git-odb.md](06-stardf-git-odb.md) defines the ODB-as-columnar
positioning; this document pins the exact bytes. [18-m1-branchless-algebra.md]
(18-m1-branchless-algebra.md) supplies the ladder formalism reused in Section 3.

## 1. `.idx` v2 layout

The `.idx` file is the lookup index over one `.pack`. All multi-byte integers are
big-endian. Section order is fixed; a reader must not reorder them.

| offset | size | field |
|--------|------|-------|
| 0      | 4    | magic `\377tOc` (0xFF 0x74 0x4F 0x63) |
| 4      | 4    | version = 2 |
| 8      | 1024 | fan-out table: 256 × u32 BE, cumulative object count for OIDs whose first byte is ≤ the index |
| 1032   | 20N  | sorted OID table: N × 20-byte SHA-1, ascending (standard v2; 32-byte only under the sha256 object-format extension) |
| 1032+20N | 4N  | CRC32 table: one u32 BE per object, CRC of the packed object's compressed bytes |
| 1032+24N | 4N  | small-offset table: N × u32 BE; bit 31 set ⇒ real offset lives in the large-offset table at index `off & 0x7FFF_FFFF` |
| 1032+28N | 4L  | large-offset table: L × u64 BE (present only if L > 0; omitted entirely when no offsets exceed 2^31−1) |
| tail−40  | 20   | pack checksum: SHA-1 of the entire `.pack` file |
| tail−20  | 20   | idx checksum: SHA-1 of all preceding idx bytes |

Note on hash width: avec targets BLAKE3 identity internally but the v2 on-disk
format is SHA-1 keyed; see Section 2's compatibility constraint. The 256-entry
fan-out row i holds the cumulative count of objects with first byte ≤ i, so the
partition for first byte b spans `[fanout[b−1], fanout[b])` (with
`fanout[−1] = 0`).

## 2. `.pack` v2 layout

| offset | size | field |
|--------|------|-------|
| 0      | 4    | magic `PACK` |
| 4      | 4    | version = 2, big-endian |
| 8      | 4    | object count N, big-endian |
| 12     | var  | N object entries, back to back |
| end−20 | 20   | trailing SHA-1 checksum over all preceding pack bytes |

Each object entry begins with a varint header encoding type and size:

    bit 7      MSB: 1 = another header byte follows
    bits 6..4  type (3 bits)
    bits 3..0  low 4 bits of size
    continuation bytes: 7 size bits each, little-endian order within the size

Type values: 1 = commit, 2 = tree, 3 = blob, 4 = tag, 6 = ofs-delta,
7 = ref-delta. Types 0 and 5 are reserved and invalid.

Entry bodies:

- **Standard objects** (types 1–4): zlib (RFC 1950) stream of `size` inflated
  bytes. Git reads these directly.
- **Delta objects** (types 6–7): zlib stream of a delta payload applied to a
  base object; avec does not author deltas (Section 6) but must parse them from
  packs received over transport.
- **avec-authored chunks**: the design target is aligned columnar chunks
  (avec `DataPageV2` payloads, BLAKE3-identified) stored in the pack. The
  compatibility constraint is absolute: **Git Packfile v2 has no extension
  mechanism for new object types.** Types 0, 5, and ≥8 are invalid to unmodified
  git; writing them makes the pack unreadable by `git fsck`.

Permitted placement of avec payloads:

- **Permitted**: an avec columnar chunk serialized as a complete, self-contained
  blob (type 3). The blob content is the chunk's canonical byte serialization;
  its OID is the SHA-1 of those bytes per Git CAS. This is the only conforming
  home for avec data inside a v2 pack.
- **NOT permitted**: new type codes, non-standard trailer extensions, or
  sidecar data interleaved into the pack body. Anything non-standard must ride
  as blob content so unmodified git verifies the pack end to end.
- avec-side metadata that must not pollute the pack (chunk alignment, column
  schema, BLAKE3 ↔ SHA-1 bindings) lives in the tree structure and commit
  message of the snapshot (Section 4), not in pack-adjacent bytes.

## 3. Lookup algorithm: branchless ladder over the fan-out

Given target OID `t`, read `b = t[0]` (first byte). The fan-out row gives the
partition `[fanout[b−1], fanout[b])` containing at most 65,536 candidate
objects under the working assumption below.

Within the partition, the 16-stage unrolled ladder from
[18-m1-branchless-algebra.md](18-m1-branchless-algebra.md) §4 applies:

    p_k     = b_k + 2^k                                  k = 15, 14, ..., 0
    λ_k     = Π[ p_k < N ] · Π[ α_{p_k} < t ]
    b_{k-1} = b_k + λ_k · 2^k

Displacements run 32768 → 16384 → ... → 1 (2^15 down to 2^0), sixteen stages
total. The predicate `Π[…]` is a full-width mask; the base update `b_{k-1}` is
predicated (multiply-by-mask + add), never a conditional jump. Every stage
executes unconditionally: exactly 16 probes, 16 compares, 16 adds for every
input.

Match resolution, also branchless:

- Constant-time 32-byte (or 20-byte, SHA-1 keyed) equality on the candidate OID:
  accumulate `diff |= a[i] ^ t[i]`, reduce with word-wide OR, `eq = (diff == 0)`
  as a mask; no early exit.
- Offset selection: `sel = eq & small_off` gives the u32 entry; if its MSB
  (bit 31) is set, the true pack offset is `large[sel & 0x7FFF_FFFF]`. The MSB
  mask selects between small and large with an arithmetic mask, not a branch.

Invariants:

- Exactly 16 stages; zero conditional jumps in the emitted hot path
  (`cmov`/`csel` permitted; see 18-m1 §5 for the CI disassembly audit).
- `Var(T_lookup) = 0`: identical instruction count and trajectory for hit, miss,
  and every key value.
- **Partition-size assumption**: 16 stages address a window of at most
  2^16 = 65,536 entries. The fan-out guarantees a first-byte partition is at
  most the full table, so the assumption holds while the pack holds ≤ 2^16
  objects total. **Beyond 65,536 objects in a single first-byte partition**
  (e.g. adversarial or heavily skewed OID distributions), one or more
  partitions exceed the ladder window and the ladder depth must be re-derived
  for that partition. **Open item L6-1**: derive the general K = ⌈log₂
  (max partition size)⌉ ladder generation rule, or cap per-partition load by
  splitting packs. Until closed, avec packs MUST stay ≤ 2^16 objects.
  **CLOSED-by-implementation (wave 7)**: MAX_OBJECTS = 65,536 is enforced as
  `GitError::TooManyObjects` in `crates/avec-git/src/pack_write.rs:234` and
  `:261` and `idx_read.rs:127`, with boundary courts exercising exactly 65,536
  objects. The cap option was taken; the K-ladder generalization remains
  design-only. Note: the reader DOES implement the bit-31 large-table
  escape — the small/large tables are read at 1032+24N / 1032+28N
  (`idx_read.rs:86`/`:98`), so a received pack carrying large offsets reads
  correctly; only the writer refuses offsets ≥ 2^31 (`OffsetTooLarge`),
  so avec-authored packs never produce a large table.

## 4. Write path

Pack growth is append-only; the index is a derived artifact regenerated per
append batch, never mutated in place.

1. **Append**: serialize each object entry and write it at the current pack
   tail. Record `(oid, offset, crc32)` in the in-memory pending table. No byte
   of an existing entry is ever rewritten; growth is a single tail write plus
   fsync.
2. **Idx regeneration**: on commit of a batch, rebuild the `.idx` from the full
   `(oid, offset, crc32)` set: sort by OID, derive fan-out, emit tables per
   Section 1, compute both trailing checksums. Write to a temp file and rename
   atomically. Regeneration is O(N log N) per batch but readers mmap the
   previous idx until the rename lands.
3. **Snapshot mapping**:
   - **blobs** = avec Parquet columnar chunks, stored whole (Section 2). One
     chunk = one blob; the blob OID is the CAS identity of the chunk bytes.
   - **trees** = the directory mapping: path components → (mode, OID). Chunk
     grouping and column schema live in tree structure plus a schema descriptor
     blob referenced from the tree.
   - **commits** = one commit per state snapshot: parent-linked to the previous
     snapshot commit, tree pointing at the chunk set. The commit message
     carries the BLAKE3 root and chunk-manifest reference; the commit OID
     itself is the receipt — it transitively hashes the entire snapshot via
     the Merkle tree, so `R = receipt(A)` reduces to the commit OID.
4. **Receipt**: publishing a snapshot = advancing a ref to the new commit OID.
   The OID is 160 bits of transitive hash over all chunk bytes: identity and
   integrity in one value, matching the B-primitive receipt shape in
   [06-stardf-git-odb.md](06-stardf-git-odb.md).

## 5. Interop verification (FR-04 acceptance)

Every avec-generated repository MUST verify with unmodified git. This is the
FR-04 acceptance gate and is executed, not asserted:

    git fsck --full <repo>          # object graph integrity, zero errors
    git verify-pack -v <pack>.idx   # per-object offsets, types, sizes, CRC
    git cat-file -p <commit>        # snapshot commit parses
    git log --oneline               # parent DAG walks

Plus cross-checks against avec's own identity layer:

    b3sum <chunk-file> == manifest blake3 root entry   # chunk identity
    git hash-object <chunk-file> == tree entry OID     # CAS agreement

Acceptance: `git fsck` exits 0, `git verify-pack` reports no errors, and every
chunk's BLAKE3 and Git-SHA-1 identities both match their respective recorded
values. A pack that git cannot read is a failed integration regardless of any
avec-internal check passing.

## 6. Non-goals

- **No pack protocol in avec.** Transport (`git fetch`/`push` wire protocol,
  negotiation, thin packs over the wire) is handled by standard git or higher
  orchestration layers. avec produces and consumes pack *files* only.
- **No delta authoring.** avec-authored chunks are stored non-delta (full
  zlib-enclosed blob content). Delta resolution is required only on the *read*
  path for packs received from external git sources (types 6/7 must be
  resolvable to serve interop), never on avec's write path.
- **No loose-object writing.** stardf-git appends pack blocks directly; the
  loose-object store and its repack loop are outside scope (see 06, §Value
  Analysis).
- **No new object types or pack extensions.** Section 2's compatibility
  constraint is a permanent non-goal boundary, not a deferred feature.

## Standing

UNVERIFIED. Byte layouts restated from the public Git pack-format
specification; the ladder reuse, blob-only chunk placement rule, and the
≤ 2^16 object cap are design claims with no executed receipt in this wave.
Amendment (wave 7): open item L6-1 is CLOSED-by-implementation (see §3) —
the 65,536-object cap is enforced in src with boundary courts. Verification
path is Section 5 against a real avec-generated repository.

## See Also

- [06-stardf-git-odb.md](06-stardf-git-odb.md) — ODB-as-columnar-ODB framing;
  write-path economics this spec pins to bytes
- [18-m1-branchless-algebra.md](18-m1-branchless-algebra.md) — ladder
  formalism, mask algebra, CI disassembly audit (§4/§5)
