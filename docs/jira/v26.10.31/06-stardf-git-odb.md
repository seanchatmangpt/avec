# stardf-git: Packfile v2 as a Columnar Object Database

Status: PARTIAL_ALIVE — design plus landed avec-git code; see Standing.
Owner: lane 7, wave v26.10.31.

## Value Analysis: Strip the Porcelain, Keep the Mechanical Core

Git's durable value is small and mechanical:

- **Content-addressed immutability (CAS)**: an object's OID is the hash of its
  bytes; identity and integrity are the same number.
- **DAG ancestry**: commits form an append-only parent-linked DAG; ancestry is
  structure, not text.
- **Merkle verification**: trees hash subtrees; a corrupt byte breaks the root.
- **Universal wire interop**: `git fetch`/`git push` speak packfiles; any host
  that reads Packfile v2 can read us.

Everything else is porcelain and discardable:

- **zlib repack loops** — periodic whole-store rewrite to control loose-object
  sprawl. stardf-git never writes loose objects; it appends blocks.
- **Per-object POSIX file thrashing** — one `open/stat/read/close` per lookup
  against loose stores; replaced by two `mmap` regions held for process life.
- **String-parsed commits** — `git log` is regex over `author <t> ` lines;
  replaced by a fixed-layout binary commit descriptor.

The reduction: Git is a Merkle DAG of compressed byte spans with a sorted
OID index. That is exactly a columnar object database with a hash index.

## Contrast: Standard Git / isomorphic-git vs stardf-git

| Dimension | standard / isomorphic-git | stardf-git |
|---|---|---|
| Engine core | JS `FS` calls per object; libgit2 FFI per object | `mmap` ODB; two mappings, zero
syscalls per lookup |
| Object repr | loose zlib blobs, inflate per read | raw Packfile v2 spans as mmap pointers, no
inflate on hot path |
| Index search | linear directory scans / re-read `.idx` | branchless binary search over 256-entry
`.idx` v2 fan-out |
| Snapshot write | Turtle serialize whole state, ~650 ms | direct packfile block append via avec
`DataPageV2`, <=3 ms target |
| Hashing | SHA-1, unaccelerated | hardware-accelerated SHA-256 + BLAKE3 dual attestation |

Notes on the write column: the ~650 ms figure is the measured isomorphic-git
Turtle snapshot path in prior avec benchmarking; the <=3 ms target is the
stardf-git budget for an append-only pack block plus `.idx` v2 rebuild, not a
measured receipt. Both numbers are claims to be falsified by
`crates/bench` phase-change benchmarks before this document leaves
UNVERIFIED.

## The .idx v2 Layout as Hardware-Friendly Structure

`.idx` v2 is already a columnar index; stardf-git treats it as the canonical
schema rather than a compatibility artifact.

- **Fan-out table**: 256 `u32` entries, 1 KB, at offset 8 (after the
  `\377tOc` magic and version). Entry `i` is the cumulative count of OIDs
  whose first byte is `<= i`. First-byte bucketing is O(1), and the table
  fits in L1 — every lookup starts with one L1 load.
- **OID table**: N x 32 bytes, lexicographically sorted. With the fan-out
  bounding the search to one bucket, the lookup is a bounded branchless
  binary search: O(log 256) = 8 predicated steps worst case, not O(log N).
- **CRC32 table**: N x 4 bytes, parallel to the OID table; corruption check
  per object without touching the pack.
- **Offset table**: N x 4 bytes. If bit 31 (MSB) is set, the low 31 bits
  index the trailing 64-bit large-offset table (needed for packs > 2 GiB).
  Dereference is an arithmetic mask plus a second indexed load — no branch.

The whole index for 100k objects is ~4.2 MB; the hot 1 KB fan-out plus the
one probed bucket stays resident.

## Data Path Diagram

```
mmap'd .idx                mmap'd .pack
+----------------+         +----------------------+
| magic/version  |         | "PACK" ver=2 count=N |
| fanout[256]----+---(1)   | header: type+size    |
| oids[N]        |         | zlib/span data       |
| crc32[N]       |  binary | ...object streams... |
| offsets[N]--+  |  search |                      |
+-------------|--+         +----------^-----------+
              | (2) offset            |
              v                       | (3) slice ptr
      large-offset tbl --> pack span -+
                                      |
                                      v
                        avec ingestion turnstile
                                      |
                                      v
                    Arrow / Parquet frame buffer
```

1. First byte of the queried OID selects the fan-out bucket.
2. Bounded search resolves the 32-bit (or 64-bit escaped) pack offset.
3. The offset indexes directly into the mapped pack; the object stream is
   handed to the avec ingestion turnstile without copies, and lands in the
   Arrow/Parquet frame buffer as typed columns.

## ABI Surface Design

C-ABI, freestanding, no allocator on the read path.

```c
typedef struct stardf_git_pack_view {
    const uint8_t* idx_map;   // mmap'd .idx v2
    size_t         idx_len;
    const uint8_t* pack_map;  // mmap'd .pack
    size_t         pack_len;
    uint32_t       object_count;
} stardf_git_pack_view_t;

// Branchless lookup; returns pack offset or SIZE_MAX on miss.
size_t stardf_git_find_offset(const stardf_git_pack_view_t* v,
                              const uint8_t oid[32]);

// Ingest one commit span into avec; the commit OID is the returned receipt.
int stardf_git_commit_turnstile(stardf_git_pack_view_t* v,
                                size_t offset,
                                uint8_t out_oid[32]);

typedef struct stardf_git_commit {
    uint8_t  oid[32];              // SHA-256 OID, the receipt hash
    int64_t  commit_time_ns;       // fixed-width, no string parse
    uint64_t tree_offset;          // pack offset of the tree object
    uint32_t parent_count;         // DAG arity
    uint64_t parent_offsets[/* parent_count */]; // DAG edges as offsets
    void*    avec_datapage_id;     // owning avec DataPageV2
} stardf_git_commit_t;
```

`stardf_git_commit_t` is an aligned fixed-layout descriptor: fixed-size
trailing array, `parent_offsets` immediately after `parent_count`, so the
struct is a single pointer-dereferenceable span (safe to overlay on an mmap
region after alignment validation).

`stardf_git_commit_turnstile` recomputes the OID over the ingested bytes and
returns it as the receipt: the caller gets back the content address, not a
status code pretending to be one.

## Search Mechanics

```
b      = fanout[oid[0] - 1]        // bucket start (cumulative)
e      = fanout[oid[0]]            // bucket end
lo = b; hi = e;
while (lo < hi) {
    mid = (lo + hi) >> 1;
    p   = &oids[mid * 32];
    lt  = memcmp_lt_32(p, oid);   // predicated 32-byte compare
    lo  = lt ? mid + 1 : lo;      // CSEL/CMOV form
    hi  = lt ? hi : mid;
}
hit = (lo < e) & memcmp_eq_32(&oids[lo * 32], oid);
```

- The bucket bound converts an O(log N) problem into O(log 256) = 8
  predicated iterations; each iteration is two loads and two CSEL-class
  updates.
- **Large-offset dereference is arithmetic, not branchy**:
  `off64_base = off & 0x7FFFFFFF; is_large = off >> 31; addr =
  large_table + (off64_base << 3); result = is_large ? *addr : off` —
  the MSB acts as a mask-selecting multiplexer, keeping the path branch-free.
- **Open issue (flagged)**: source-level `if` is not Cyclomatic Complexity 1
  in the sense this design requires — a branchy `if` in the offset
  dereference or the search updates is a control-dependence LLVM may exploit,
  de-optimizing select idioms back into branches under inlining and
  profile-guided layout. The contract is machine-checked disassembly: every
  lookup path must show zero conditional branches between fan-out load and
  return. Cross-ref `16-ahdclt` disassembly-audit discipline; the audit is
  the gate, the Rust source is only intent.

## Interop Guarantee

- **Wire format**: all output strictly complies with Packfile v2. Object
  headers, zlib spans, and trailer checksum are byte-compatible.
- **Host compatibility**: standard git and GitHub can `clone`, `log`, and
  `fsck` a stardf-git store with no stardf-git tooling present.
- **Receipt identity**: a commit OID *is* the receipt hash — the content
  address doubles as the admission receipt identifier, replayable by any
  conformant pack reader.
- **State representation**: tree objects reference avec Parquet chunks
  carrying the (O, t, V, G) state tuple — objects, timestamps, values,
  graph edges — as typed columns rather than serialized text.
- **Time travel**: `.idx` lookup + mmap, nothing else. No checkout, no work
  tree, no text parsing; historical state is a pack offset away, at mmap
  page-fault cost.

## Standing

PARTIAL_ALIVE — code exists and is receipted, but under avec, not stardf,
and diverges from this design in three pinned respects.

Landed (receipted):

- `crates/avec-git` is real: fsck court 2/2 against real git,
  `git_interop` 3/3, gate `gate-git-tests-v2.md` ALIVE (FR-04 PASS).
- Interop guarantee holds at the wire level: a store standard git fscks
  clean without any stardf-git tooling.

Divergences from this design (these sections remain design-only):

- Hashing: shipped code is SHA-1 / 20-byte OID, not the SHA-256 /
  32-byte / dual-BLAKE3 design above.
- Index mechanics: a 16-stage fixed ladder over 2^16 partitions, not the
  256-fanout / 8-predicated-step search.
- Large-offset table: deliberately omitted in src (`OffsetTooLarge`).

Unverified / unreceipted:

- The `~650 ms` vs `<= 3 ms` write claims remain unverified on both
  sides.
- Perf numbers exist only in `crates/avec-git/tests/perf_baseline.rs`
  (16.6 MiB/s pack write, 3.49M lookups/s, vs git pack-objects
  12.1 MiB/s) — informational, not a gate receipt.

Original promotion requirements, now partially met: (1) packfile
round-trip against real `git fsck` — DONE; (2) phase-change timing
receipt for the append path — unrun; (3) zero-branch disassembly audit —
unrun.
