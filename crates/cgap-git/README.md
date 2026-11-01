# cgap-git

Git Packfile v2 writer for cgap snapshot chunks: stored blob objects,
`.idx` v2 generation, in-crate SHA-1 and CRC32. Zero external dependencies
— `Cargo.toml` declares an empty `[dependencies]` table — and the crate is
`no_std` with `#![forbid(unsafe_code)]`.

- Crates.io: `cgap-git` v26.10.29
- Objects are **stored** (type 3, blob), never deltified.
- Bodies are zlib streams: in-crate fixed-Huffman DEFLATE compressor,
  RFC 1951/1950 inflater.

## Tutorials

### Write a pack from blobs and generate the idx

The primary entry point is `build_pack_and_idx`: pack bytes, `.idx` bytes,
and the per-object `(oid, offset, crc)` table in one pass.

```rust
use cgap_git::pack_write::build_pack_and_idx;

let blobs: &[&[u8]] = &[b"hello", b"world", b"hello world"];
let (pack, idx, entries) = build_pack_and_idx(blobs).unwrap();

assert_eq!(&pack[0..4], b"PACK");
assert_eq!(&pack[4..8], &2u32.to_be_bytes());
assert_eq!(entries.len(), 3);
```

`pack` ends with the SHA-1 trailer over all preceding bytes; `idx` is a
conformant v2 index (fan-out, OID table, CRC table, small-offset table,
pack checksum, idx checksum). Verified conformant for real `git fsck` /
`verify-pack` / `cat-file` (crate doc comment).

### Incremental building with `PackBuilder`

```rust
use cgap_git::PackBuilder;

let mut b = PackBuilder::new();
b.push(b"alpha");
b.push(b"beta");
let (pack, idx) = b.finish().unwrap();
assert_eq!(&pack[0..4], b"PACK");
```

`finish` regenerates pack + idx from the full pending set on each call
(append-then-regenerate flow); growth is tail-only via `push`.

### Verify the round trip through the independent parser

The read side (`parse_pack_v2` / `parse_idx_v2`) is a separate code path
from the writer, so a round trip through it is a real-collaborator check.

```rust
use cgap_git::pack_write::{build_pack_and_idx, parse_idx_v2, parse_pack_v2};

let blobs: &[&[u8]] = &[b"round", b"trip"];
let (pack, idx, entries) = build_pack_and_idx(blobs).unwrap();

let objs = parse_pack_v2(&pack).unwrap();
assert_eq!(objs.len(), 2);
assert_eq!(objs[0].obj_type, 3); // stored blob
assert_eq!(objs[0].size, 5);

let (oids, crcs, offsets) = parse_idx_v2(&idx).unwrap();
assert_eq!(oids.len(), 2);
assert!(oids.windows(2).all(|w| w[1] > w[0])); // sorted, unique
assert_eq!(
    entries.iter().map(|e| e.oid).collect::<Vec<_>>(),
    oids
);
```

## How-to

### Verify a pack's trailing checksum

`pack_write::parse_pack_v2` verifies the SHA-1 trailer over all preceding pack bytes
before parsing objects; a mismatch is `GitError::PackChecksumMismatch`.

```rust
use cgap_git::pack_write::{build_pack_and_idx, parse_pack_v2};

let blobs: &[&[u8]] = &[b"data"];
let mut pack = build_pack_and_idx(blobs).unwrap().0;

let last_body_byte = pack.len() - 21; // one byte before the trailer
pack[last_body_byte] ^= 0xFF;
let err = parse_pack_v2(&pack).unwrap_err();
assert!(matches!(err, cgap_git::GitError::PackChecksumMismatch));
```

### Verify an idx's trailing checksum

`IdxV2::parse` validates structure only; `verify_idx_checksum` checks the
SHA-1 trailer.

```rust
use cgap_git::idx_read::IdxV2;
use cgap_git::pack_write::build_pack_and_idx;

let (_, idx, _) = build_pack_and_idx(&[b"data"]).unwrap();
let parsed = IdxV2::parse(&idx).unwrap();
assert!(parsed.verify_idx_checksum().is_ok());
```

### Compute checksums directly

SHA-1 and CRC-32 are exposed for standalone use.

```rust
use cgap_git::pack_write::{crc32, sha1, Crc32, Sha1};

assert_eq!(sha1(b"abc").len(), 20);

// CRC-32/IEEE check value for "123456789":
assert_eq!(crc32(b"123456789"), 0xCBF43926);

// Streaming forms:
let mut c = Crc32::new();
c.update(b"1234");
c.update(b"56789");
assert_eq!(c.finalize(), 0xCBF43926);

let mut s = Sha1::new();
s.update(b"a");
s.update(b"bc");
assert_eq!(s.finalize(), sha1(b"abc"));
```

### Look up an OID in a parsed idx

`IdxV2::lookup` binary-searches the fan-out partition with the 16-stage
constant-time ladder and returns `(entry index, pack offset)`.

```rust
use cgap_git::idx_read::IdxV2;
use cgap_git::pack_write::build_pack_and_idx;

let (_, idx, entries) = build_pack_and_idx(&[b"target", b"other"]).unwrap();
let parsed = IdxV2::parse(&idx).unwrap();

let (i, off) = parsed.lookup(&entries[0].oid).unwrap().unwrap();
assert_eq!(parsed.oid(i), entries[0].oid);
assert_eq!(parsed.offset(i).unwrap(), off as u64);
assert_eq!(parsed.crc(i), entries[0].crc);
```

### Compress and decompress zlib streams

Write side (`deflate` module, returns `GitError` on read):

```rust
use cgap_git::deflate::{deflate_fixed, zlib_compress, zlib_decompress};

let compressed = zlib_compress(b"some snapshot chunk");
assert_eq!(zlib_decompress(&compressed).unwrap(), b"some snapshot chunk");
let _ = deflate_fixed(b"raw DEFLATE, no zlib framing");
```

Read side (`inflate` module, returns `InflateError`):

```rust
use cgap_git::inflate::zlib_decode;

let compressed = cgap_git::deflate::zlib_compress(b"payload");
assert_eq!(zlib_decode(&compressed).unwrap(), b"payload");
```

### Parse an idx into tables

For full-table access without the borrowed `IdxV2` view:

```rust
use cgap_git::pack_write::{build_pack_and_idx, parse_idx_v2};

let (_, idx, _) = build_pack_and_idx(&[b"x"]).unwrap();
let (oids, crcs, offsets) = parse_idx_v2(&idx).unwrap();
assert_eq!(oids.len(), crcs.len());
assert_eq!(crcs.len(), offsets.len());
```

## Reference

### Crate root

| Item | Kind | Notes |
|---|---|---|
| `Oid` | type alias | `[u8; 20]` — SHA-1 object name. |
| `GitError` | enum | All writer/reader failures; see below. |
| `write_pack_v2`, `write_idx_v2` | fns | Re-exported from `pack_write`. |
| `PackBuilder`, `IdxEntry` | structs | Re-exported from `pack_write`. |
| `deflate`, `inflate`, `idx_read`, `ladder`, `pack_write` | modules | See below. |

Everything else lives in `pack_write`:

| Item | Kind | Notes |
|---|---|---|
| `build_pack_and_idx` | fn | Primary entry point: pack + idx + entries. |
| `parse_pack_v2` | fn | Independent pack parser + trailer SHA-1 verify. |
| `parse_idx_v2` | fn | Independent idx parser + both trailing checksums. |
| `ParsedObject` | struct | `{ obj_type, size, offset, crc }` from parsing. |
| `MAX_OBJECTS` | const | 65,536; above this: `TooManyObjects`. |
| `Sha1`, `sha1` | struct, fn | In-crate SHA-1 (streaming, one-shot). |
| `Crc32`, `crc32` | struct, fn | In-crate CRC-32/IEEE (streaming, one-shot). |
| `encode_object_header` | fn | Pack varint header (type + size). |
| `git_object_oid` | fn | Git object OID: `sha1("blob <len>\0" + content)`. |

### Modules

**`deflate`** — `Adler32`, `adler32`, `deflate_fixed`, `zlib_compress`,
`zlib_decompress`, and `inflate`, which returns `(Vec<u8>, usize)`: the
decompressed data plus the number of compressed bytes consumed.

**`inflate`** — `InflateError`, `inflate`, `adler32`, `zlib_decode`.

**`idx_read`** — `IdxV2`, the zero-copy borrowed idx view: `parse`, `len`,
`is_empty`, `fanout`, `oid`, `crc`, `offset`, `lookup`,
`verify_idx_checksum`.

**`ladder`** — `LADDER_STAGES`, `LADDER_WINDOW`, `debug_probes`,
`debug_tick`, `oid_lt_eq`, `oid_lt_mask`, `ct_eq_oid`, `ladder_search`.

### GitError variants

| Variant | Meaning |
|---|---|
| `TooManyObjects(n)` | count exceeds 65,536 (pack-v2 ladder-window cap, open item L6-1) |
| `PackChecksumMismatch` | pack trailer SHA-1 does not match preceding bytes |
| `IdxChecksumMismatch` | idx trailer SHA-1 does not match preceding bytes |
| `BadMagic` | bad magic/version in pack or idx |
| `Truncated` | stream ended mid-structure |
| `BadVarint` | malformed varint header (bad type code or size encoding) |
| `SizeOverflow` | object size exceeds u32 (idx small-offset table domain) |
| `OffsetTooLarge` | offset ≥ 2^31; large-offset table not implemented |

## Explanation

### Why in-crate SHA-1 and CRC32 instead of ring/sha1/crc32fast

`ring`, `sha1`, and `crc32fast` all pull transitive dependency trees, and
several contain `unsafe` and cc/asm build steps — incompatible with this
crate's posture: `no_std`, `#![forbid(unsafe_code)]`, and an empty
`[dependencies]` table. SHA-1 here hashes only pack/idx trailers and
`git_object_oid` framing; CRC32 covers only per-entry integrity words.
Both algorithms are small, fully specified, and stable, so carrying them
in-crate costs little and keeps the crate buildable with no registry
access and no supply-chain surface, auditable in one sitting.

### How the .idx v2 is laid out (as implemented)

`write_idx_v2` writes, big-endian throughout:

1. magic `FF 74 4F 63` (`\377tOc`) + version `2` — 8 bytes
2. 256-row cumulative fan-out table: row *i* = count of OIDs with first
   byte ≤ *i* (built as per-first-byte counts, then one cumulative sum)
3. OID table, sorted ascending — 20 bytes per entry
4. CRC32 table — 4 bytes per entry (CRC over the header+body bytes at
   that object's offset)
5. small-offset table — u32 per entry
6. pack checksum — 20 bytes
7. idx checksum — SHA-1 over all of the above, 20-byte trailer

Entries are sorted in place (`write_idx_v2` takes `&mut [IdxEntry]`).
Offsets ≥ 2^31 are refused with `OffsetTooLarge` before any bytes are
written; the v2 large-offset table is not implemented. Total idx size is
`1032 + 44·n + 40` bytes for n entries.

### The 65,536-object cap and the constant-time ladder

The 16-stage unrolled search ladder in `ladder.rs` addresses exactly 2^16
entries: 16 probes, 16 compares, 16 predicated adds for every input — no
data-dependent branches. A fan-out partition above that window cannot be
searched in one pass, so the writer refuses counts above
`MAX_OBJECTS = 65,536` up front (`TooManyObjects`); `IdxV2::lookup`
returns the same error rather than running the ladder out of window.
Recurrence (k = 15 down to 0): `p_k = b_k + 2^k`,
`λ_k = [p_k ≤ N]·[α_{p_k} < t]` (α 1-indexed),
`b_{k-1} = b_k + λ_k·2^k`. Debug builds expose a probe counter
(`debug_probes` / `debug_tick`), compiled out in release.

## License

`LICENSE.md` at the workspace root (packaged via `license-file`).
