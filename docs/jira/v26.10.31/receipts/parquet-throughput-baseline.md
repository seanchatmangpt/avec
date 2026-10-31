# Receipt: parquet-throughput-baseline

| Field | Value |
|---|---|
| Receipt ID | parquet-throughput-baseline |
| Date | 2026-10-30 |
| Subject | Level-free Parquet mmap-scan decode throughput (spec 24, lines 61/100 `>= 74 GiB/s` claim) |
| Command | `cargo run --release -p bench --bin parquet_throughput` |
| Exit | 0 |
| Machine | macOS 26.2 (25C56), Apple M3 Max |
| Input SHA (git HEAD) | `ead40329a699878a91b070378066b5e50541ba33` |
| Tree state | dirty (wave-7 fan-out in flight) |
| Evidence | bin stdout (verbatim below); hand-minted — see Standing rule |
| Standing | PARTIAL — real measured number, load-shared host, claim NOT confirmed at 74 |

**Hand-mint reason**: `scripts/mint_receipt.sh` renders only the field table from
`template.md`; this receipt's substance is the comparison of three differently-scoped
numbers (below), which the template cannot hold. Command, exit, machine, HEAD, and
stdout are captured verbatim above/below; the run itself is real, not synthesized.

## What ran

New bin `crates/bench/src/bin/parquet_throughput.rs` (this lane): writes a 1 GiB
level-free Parquet file with the crate's own `level_free::write_file` — 4 x
FLBA(32) columns, 2^23 rows, 256 MiB per column, payload exactly 1,073,741,824
bytes — then mmap-scans it via `scan::ParquetFile` and measures two modes.

Bin stdout (verbatim, 2026-10-30, `cargo run --release -p bench --bin parquet_throughput`, exit 0):

```
== parquet level-free scan throughput baseline ==
fixture: 4 x FLBA(32) cols, 8388608 rows, 256 MiB/col, payload 1073741824 bytes
open: ok (footer cached, 1073742079 bytes on disk)
PAYLOAD-SUM: 7 rounds, min 65.6 GiB/s, mean 64.8 GiB/s (checksum ffffffffe4000000)
ACCESSOR-SCAN: 2 rounds (33.5M per-row value_flba calls, each re-parsing the page header), min 0.595 GiB/s, mean 0.593 GiB/s — measures header-parse overhead, NOT decode throughput
done (fixture removed)
```

## The three numbers, honestly labeled

1. **65.6 GiB/s (min) / 64.8 GiB/s (mean) — PAYLOAD-SUM** (this receipt): one
   `column_bytes` call per column, then a black_boxed byte-sum fold over the
   full zero-copy slice. This is the memcpy-equivalent ceiling comparator —
   page-cache-hot mmap read at memory speed with no accessor work. This is the
   number the spec's "sustained decode" claim should be judged against.
2. **74-77 GiB/s (spec 24 lines 61/100)**: LEGACY-UNVERIFIED — no prior bin or
   receipt ever measured the level-free scan path end to end. This receipt
   does NOT confirm it: measured 65.6 GiB/s min on a shared M3 Max under
   concurrent wave lanes. Against a 74 GiB/s bar the current figure is 11%
   short; the gap may be machine load (mean/min spread was small, so load
   alone may not close it) and needs a quiet-machine re-run to separate.
3. **~98 GB/s aggregate (wave-3 receipt, doc 40 line 48)**: a different
   measurement — in-memory write+read roundtrip aggregate, decimal GB/s, small
   buffers. Not comparable to either number above (different metric, unit
   convention, and working-set size); cited only to prevent conflation.

## Accessor-path finding (actionable, separate from the headline)

`ACCESSOR-SCAN` at 0.595 GiB/s is ~110x below the payload ceiling. Cause is
structural: `scan::ParquetFile::value_flba` -> `column_bytes` re-parses the
Thrift page header on EVERY row access (`crates/avec-parquet/src/scan.rs:535`
`column_bytes` called from the accessors at lines 566-596). Any row-at-a-time
consumer path pays a full header parse per value. Fix shape: cache the
per-(row-group, column) resolved slice at open (the footer is already parsed
once there), making the accessor path O(1) per row. Registered as a follow-up;
not fixed in this lane.

## Caveats

- Machine under concurrent fan-out load; min-of-7 is the defensible figure.
- Page-cache-hot: the fixture was written by the same process immediately
  before the scan; cold-cache numbers will be lower and are disk-bound, not
  the claim under test.
- Fixture uses one row group / one page per file; multi-row-group files were
  not measured.
