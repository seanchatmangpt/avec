# Error Ergonomics Audit — Typed-Error Display Strings

Wave v26.10.31, lane 5 (DX/QoL). Scope: Display strings only — same variants, same
matches, zero behavior change. Files:
`crates/avec-git/src/{lib,inflate,deflate}.rs`, `crates/avec-parquet/src/{scan,level_free}.rs`.

Standing: ALIVE. Every command below was run this session, exit 0, and post-edit
counts matched the pre-edit baseline exactly (see Verification).

## Bar and rubric

An error message passes when it says WHAT failed, WHY (the invariant), and the FIX.
Grades: PASS = all three legs present. WEAK = WHAT present, one leg missing or thin.
FAIL = bare label or Debug passthrough — no invariant, no fix.

## Summary

| Crate | Error enum | Variants | Before (PASS/WEAK/FAIL) | After |
|---|---|---|---|---|
| avec-git | GitError | 8 | 0/7/3 | 8 PASS |
| avec-git | InflateError | 9 | 0/7/2 | 9 PASS |
| avec-parquet | ScanError | 11 | 0/0/11 (Debug passthrough) | 11 PASS |
| avec-parquet | LevelFreeError | 8 | 0/0/8 (Debug passthrough) | 8 PASS |
| total | 4 enums | 36 | 0/14/24 | 36 PASS |

`crates/avec-git/src/deflate.rs` has no error enum of its own; it raises `GitError`
(BadVarint, Truncated, BadMagic, PackChecksumMismatch) and so is graded under those
rows — no Display code lives in deflate.rs ("error Display if any" → none).

## GitError — crates/avec-git/src/lib.rs

| Variant | Before (verbatim) | Grade |
|---|---|---|
| TooManyObjects | too many objects in pack: {n} (cap 2^16, L6-1) | WEAK |
| PackChecksumMismatch | pack SHA-1 checksum mismatch | FAIL |
| IdxChecksumMismatch | idx SHA-1 checksum mismatch | FAIL |
| BadMagic | bad magic/version in pack or idx | WEAK |
| Truncated | truncated pack or idx | FAIL |
| BadVarint | malformed varint object header | WEAK |
| SizeOverflow | object size exceeds u32 | WEAK |
| OffsetTooLarge | offset exceeds 2^31-1 (large-offset table not implemented) | WEAK |

After (verbatim, message text after `write!(f, ...)` unwrapping; `{n}` = the payload):

```text
TooManyObjects(n)      "too many objects in pack: {n} exceeds the pack-v2 ladder-window
                        cap of 2^16 (open item L6-1) — split the snapshot across multiple packs"
PackChecksumMismatch   "pack trailer SHA-1 does not match the pack bytes preceding it: content was
                        corrupted or altered after the pack was written — re-generate or re-fetch
                        the pack; do not retry against the same bytes"
IdxChecksumMismatch    "idx trailer SHA-1 does not match the idx bytes preceding it: the index was
                        corrupted after writing — re-generate the idx from the pack (write_idx_v2)
                        rather than patching bytes"
BadMagic               "bad magic/version in pack or idx: leading bytes are not 'PACK' + version 2
                        (pack) or 0xff 't' 'O' 'c' + version 2 (idx) — confirm the file is a git
                        packfile-v2/idx-v2 artifact, not a loose object or other format"
Truncated              "pack or idx bytes ended mid-structure: the byte stream is shorter than the
                        header/count fields declare — re-check the file was fully written (atomic
                        temp+rename) and not truncated in transfer"
BadVarint              "malformed varint object header: type code or MSB size continuation is not
                        valid pack-v2 encoding — the pack was not written by a conforming writer
                        or is corrupt; re-generate it"
SizeOverflow           "object size exceeds u32: the declared size does not fit the u32 the idx v2
                        small-offset table admits — shrink the object or add the large-offset
                        path; the entry cannot be encoded as-is"
OffsetTooLarge         "offset exceeds 2^31-1: pack offsets past 2^31-1 need the v2 large-offset
                        table, not implemented — keep individual packs under 2 GiB or add the
                        large-offset table"
```

## InflateError — crates/avec-git/src/inflate.rs

| Variant | Before (verbatim) | Grade |
|---|---|---|
| Truncated | truncated deflate stream | FAIL |
| BadZlibHeader | bad zlib header (CM != 8 or CMF*256+FLG % 31 != 0) | WEAK |
| BadBlockType(t) | reserved deflate block type {t} | WEAK |
| BadCodeLengths | over-subscribed or incomplete huffman code lengths | WEAK |
| BadCodeLengthRepeat | code-length repeat with no prior length | WEAK |
| MissingEndOfBlock | dynamic block missing end-of-block code | WEAK |
| OverlongBackreference | backreference distance {d} exceeds {n} bytes of output | WEAK |
| BadDistanceCode | invalid distance code | FAIL |
| AdlerMismatch | adler32 mismatch: expected {e}, computed {a} | WEAK |

After (verbatim, message text after `write!(f, ...)` unwrapping):

```text
Truncated              "deflate stream ended mid-structure: input ran out of bits/bytes
                        before the final block finished or before the zlib adler32
                        trailer — the object was truncated in storage or transfer;
                        re-read the object from its source of truth"
BadZlibHeader          "zlib header failed the RFC 1950 check (CM != 8, or
                        CMF*256+FLG is not a multiple of 31, or FDICT set): the
                        bytes are not a zlib stream — confirm you are decoding an
                        object body, not raw content or a truncated file"
BadBlockType(t)        "reserved deflate block type {t}: RFC 1951 defines only block
                        types 0-2 — the stream is corrupt or is not DEFLATE;
                        re-read the object"
BadCodeLengths         "huffman code lengths are over-subscribed or incomplete: no
                        valid canonical-code assignment exists (Kraft inequality
                        violated) — the stream is corrupt or was not produced by a
                        conforming deflate compressor; re-read the object"
BadCodeLengthRepeat    "code-length repeat (symbol 16) with no prior length to repeat:
                        a dynamic header's length list cannot begin with a repeat —
                        the stream is corrupt; re-read the object"
MissingEndOfBlock      "dynamic block declares no end-of-block code (symbol 256 has
                        length 0): the block cannot terminate — the stream is
                        corrupt; re-read the object"
OverlongBackreference  "backreference distance {distance} exceeds the {written} bytes
                        of output produced so far: an LZ77 distance may not reach
                        before the start of the output — the stream is corrupt;
                        re-read the object"
BadDistanceCode        "distance symbol 30/31 decoded: reserved by RFC 1951 and never
                        valid in any stream — the stream is corrupt; re-read the object"
AdlerMismatch          "zlib adler32 mismatch: trailer stores {expected:#010x} but
                        the inflated bytes hash to {actual:#010x} — content changed
                        after compression; re-read the object"
```

## ScanError — crates/avec-parquet/src/scan.rs

Before: every variant rendered as its Debug shape (`write!(f, "{self:?}")` — e.g.
`UnsupportedSchema(7)`) — no invariant, no fix: FAIL for all 11.

| Variant | Before (Debug shape) | Grade |
|---|---|---|
| Io | `ScanError::Io` | FAIL |
| EmptyFile | `ScanError::EmptyFile` | FAIL |
| MalformedFooter | `ScanError::MalformedFooter` | FAIL |
| FooterOverflow | `ScanError::FooterOverflow` | FAIL |
| UnsupportedSchema(id) | `ScanError::UnsupportedSchema(7)` | FAIL |
| SchemaOverflow | `ScanError::SchemaOverflow` | FAIL |
| LevelsPresent(def_len) | `ScanError::LevelsPresent(1)` | FAIL |
| UnsupportedPage(rejected) | `ScanError::UnsupportedPage(0)` | FAIL |
| OffsetMismatch | `ScanError::OffsetMismatch` | FAIL |
| Thrift(err) | `ScanError::Thrift(...)` | FAIL |
| Page(err) | `ScanError::Page(...)` | FAIL |

After (verbatim, from `impl core::fmt::Display for ScanError`):

```text
Io                     "file open or mmap failed: the scanner reads pages through a
                        read-only mmap, so an unreadable or unmappable file is
                        fatal — check the path exists, is a regular file, and is
                        readable by this process"
EmptyFile              "zero-length parquet file: there is no footer to parse — the
                        file was never written or was truncated to zero; inspect
                        the writer that produced it"
MalformedFooter        "parquet footer not decodable: magic, declared metadata length,
                        or thrift structure does not parse — the file is corrupt or
                        was not written to completion; re-check the writer and the
                        file's transfer"
FooterOverflow         "footer arithmetic overflowed: declared metadata lengths are
                        large enough to overflow usize while resolving footer
                        offsets — corrupt or hostile file; refuse it, do not clamp"
UnsupportedSchema(id)  "schema outside the level-free projection (rejected type/id
                        {id}): only flat REQUIRED fixed-width INT32/INT64/
                        FIXED_LEN_BYTE_ARRAY columns are admitted — rewrite the
                        file flat or scan it with a general parquet reader"
SchemaOverflow         "schema arithmetic overflowed: column count or total row
                        width overflows usize — corrupt or hostile schema; refuse
                        the file, do not clamp"
LevelsPresent(len)     "data page carries definition/repetition levels ({def_len}
                        def-level bytes): the level-free projection requires
                        zero-length level buffers — this page was written by a
                        general parquet writer; scan it with a general reader"
UnsupportedPage(rej)   "page shape outside the level-free projection (rejected
                        page-type/compression/encoding id {rejected}): only
                        DataPageV2, uncompressed, PLAIN-encoded pages are admitted
                        — rewrite the file or use a general parquet reader"
OffsetMismatch         "cached offset does not resolve inside the file mapping, or
                        the page payload does not line up with the declared schema
                        widths — rebuild the offset cache/manifest against the
                        current file, or re-scan the footer"
Thrift(err)            "thrift protocol failure inside footer or page header: {err}
                        — the metadata bytes are corrupt; re-check the file"
Page(err)              "page-header failure mapped through the lib's typed error:
                        {err} — the page header is corrupt; re-check the file"
```

## LevelFreeError — crates/avec-parquet/src/level_free.rs

Before: Debug passthrough (`{self:?}`), FAIL for all 8.

| Variant | Before (Debug shape) | Grade |
|---|---|---|
| OptionalField{index,name} | `OptionalField { index: 0, name: .. }` | FAIL |
| RepeatedField{index,name} | `RepeatedField { index: 0, name: .. }` | FAIL |
| NestedField{index,name} | `NestedField { index: 0, name: .. }` | FAIL |
| UnsupportedType{..} | `UnsupportedType { index: 0, name: .., code: 5 }` | FAIL |
| UnsupportedWidth{..} | `UnsupportedWidth { index: 0, name: .., length: 16 }` | FAIL |
| UnknownRepetition{..} | `UnknownRepetition { index: 0, name: .., code: 9 }` | FAIL |
| UndecodablePage | `UndecodablePage` | FAIL |
| PayloadMismatch{..} | `PayloadMismatch { expected: 36, actual: 40 }` | FAIL |

After (verbatim, from `impl fmt::Display for LevelFreeError`; '{name}' =
`String::from_utf8_lossy(name)`):

```text
OptionalField{i,n}     "field {index} ('{name}'): repetition_type OPTIONAL — a
                        nullable column cannot map onto a fixed-width level-free
                        record batch; make the field REQUIRED in the schema or
                        scan the file with a general parquet reader"
RepeatedField{i,n}     "field {index} ('{name}'): repetition_type REPEATED — a
                        repeated column cannot map onto a fixed-width level-free
                        record batch; flatten the repetition away in the schema or
                        scan the file with a general parquet reader"
NestedField{i,n}       "field {index} ('{name}'): a group with no physical type
                        (num_children > 0) — flatten the schema; nested groups are
                        outside the level-free projection"
UnsupportedType{i,n,c} "field {index} ('{name}'): physical type id {code} is
                        outside the admitted set {INT32(1), INT64(2),
                        FIXED_LEN_BYTE_ARRAY(7)} — rewrite the column to an
                        admitted type or extend the projection"
UnsupportedWidth{i,n,l} "field {index} ('{name}'): FIXED_LEN_BYTE_ARRAY length
                        {length} is not the admitted width 32 — change the schema
                        width to 32 or extend the projection"
UnknownRepetition{i,n,c} "field {index} ('{name}'): repetition code {code} is
                        outside the parquet.thrift FieldRepetitionType range
                        {0=REQUIRED, 1=OPTIONAL, 2=REPEATED} — the schema metadata
                        is corrupt; re-check the writer"
UndecodablePage        "written page could not be parsed back: page header corrupt
                        or payload truncated — re-check the page writer
                        (write_page_header) and the buffer construction; a page
                        this crate wrote must round-trip"
PayloadMismatch{e,a}   "page payload is {actual} bytes but rows*width declares
                        {expected}: a PLAIN V2 payload must be exactly
                        rows*sum(width) bytes — re-check the writer's row-count
                        and column-width arithmetic"
```

## Verification

Baseline (pre-edit) and post-edit runs, same commands, byte-identical counts:

```text
cargo test -p avec-git
  before: 43 lib + 2 fsck_court + 3 git_interop = 48 passed, 1 ignored
  after:  43 lib + 2 fsck_court + 3 git_interop = 48 passed, 1 ignored  (identical)

cargo test -p avec-parquet --features std
  before: 163 lib + 1 accessor + 4 aligned + 4 level_free_multi + 17 mutation_kills
          + 2 page_zero_alloc + 8 rle_court + 3 robustness + 2 std_feature = 204 passed
  after:  identical, 204 passed

cargo test -p avec-parquet            (default = no-std lib configuration)
  before: 157 lib + 4 aligned + 17 mutation_kills + 2 page_zero_alloc + 8 rle_court
          + 3 robustness + 1 std_feature = 192 passed
  after:  identical, 192 passed
```

Zero test failures, zero new warnings in the two edited crates. The only string
assertion pinning a Display message (avec-git `error_display`, TooManyObjects) was
updated in lockstep — message-only change, no behavior delta.

no_std compile check:

```text
cargo build -p avec-parquet --no-default-features   → Finished (exit 0)
cargo build -p avec-git --no-default-features       → FAILS (pre-existing, not this lane)
```

The avec-git no_std failure is `std::thread_local!` + `probe_count::PROBES` used
ungated in `crates/avec-git/src/ladder.rs:31,41,54,143` (E0433/E0425). None of the
four files this lane owns appear in the error list; the failing symbols live in
ladder.rs, untouched by this lane. Flagged for the owning lane / coordinator; not
fixed here (lane boundaries).

## Method note

Every "after" message was written against the variant's actual raise sites (grep
receipts in session transcript), so the WHY clause names the real invariant and the
FIX clause names an action the caller can take. The no_std constraint is honored
without `alloc::format!`: all messages are `write!` into the `core::fmt::Formatter`
(plus `alloc::string::String::from_utf8_lossy` for field names in level_free.rs,
where alloc is already imported).