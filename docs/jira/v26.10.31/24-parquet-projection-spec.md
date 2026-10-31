# Level-Free Parquet DataPageV2 Projection Specification

Persistence-plane spec for the avec level-free Parquet projection: flat non-nullable
schemas mapped 1:1 onto Arrow record-batch buffers, scanned via mmap at memory-bandwidth.

Standing: UNVERIFIED. (Stale 2026-10-31: projection landed and verified — test-avec-parquet PASS,
gate-e2e-integration-v2 ALIVE; parquet-throughput-baseline receipt cites this spec; see
46-checklist-sweep-v3.md.)

## Constraint Set

The projection admits one schema shape only: a flat, non-nullable, fixed-width column set.

- Allowed physical types: `u32`, `u64`, `fixed_len_byte_array(32)` (256-bit values).
- Max definition level = 0 for every column. Max repetition level = 0 for every column.
- Consequence: rep/def level buffers are zero-length in every DataPageV2 payload.
- No nested groups, no optional fields, no maps, no lists, no repeated elements.
- No logical-type layers that change physical width; the physical width is the contract.

Rationale: the entire level machinery (RLE/DLE hybrid-encoded rep/def streams, bit-unpacking
of level nibbles, validity-map synthesis) exists to serve optionality and nesting. A
level-free writer that never admits optionality removes that decode cost at the schema
boundary instead of paying it per page. Writers MUST refuse schemas violating this set with
a typed refusal, never silently flatten.

## Page Mapping

Each DataPageV2 payload column is a contiguous fixed-width slice mapping 1:1 onto an Arrow
record-batch buffer.

- Column `i` of a page occupies `rows * width_i` bytes at a computable offset inside the
  page payload; the Arrow buffer for column `i` has identical length and byte order.
- Arrow offset arrays are unnecessary: no variable-length data exists, so offsets are
  implicit (`row_index * width_i`).
- Encoding per column, in preference order:
  - `PLAIN` (uncompressed): payload bytes ARE the Arrow buffer bytes. Decode is a no-op.
  - SIMD bit-packing: candidates from the SIMD-BP128 / FastPFOR class — fixed-bit
    bit-packed blocks with a per-block bit-width header, decodable with vector registers
    and a branch-free shift/blend body. The unpacked block size MUST equal a multiple of
    the column width's natural vector width so unpacking lands directly in Arrow-buffer
    layout.
- Rejected alternative: LZ/ZSTD-family block compression. Rejected on hot decode paths —
  LZ decoders are branch-heavy (literal/match state machines, per-token control flow),
  which serializes on the scalar front end and destroys vector-register streaming. Bit-
  packing keeps the decode body branch-free and width-predictable. Record here so the
  rejection is auditable, not folklore.

Page headers: the DataPageV2 header carries `num_rows`, `null_count` (always 0), and
per-column encodings. No statistics blob is required on the hot path.

## mmap Scan Architecture

- Files are opened read-only and `mmap`'d; no read() syscall on the scan path.
- The footer (FileMetaData) is parsed exactly once at open; row-group and page byte
  offsets are cached. Thrift metadata parsing MUST NOT appear on the hot scan path.
- Page access is byte-offset arithmetic against the mapping base: `base + page_offset +
  header_len + column_offset_i`. No copy, no descriptor juggling.
- Decoding streams directly through vector registers from the mapping into the Arrow
  buffer (or, for PLAIN, aliases the mapping with zero copies).
- Zero heap allocation during scan: the scan loop allocates nothing; Arrow buffers are
  obtained from a pool or the mapping itself. Allocation during scan is a spec violation
  (see Falsification Hooks).
- Throughput target: sustained decode >= 74 GiB/s on the reference machine, bounded by the
  memory controller (LSL 70). The floor is a memory-bandwidth floor, not a compute floor:
  a compliant decoder is one whose steady-state loop is load/stream instructions, not
  branches or scalar byte shuffling.

## File Layout

- Footer: standard Parquet `PAR1` magic, FileMetaData (Thrift compact), 4-byte length,
  trailing `PAR1`. Parsed once, then cached in a decoded offset index.
- Schema in footer: flat struct, each field `REQUIRED` with fixed width. The schema
  doubles as the admission check — any OPTIONAL/REPEATED field invalidates the file for
  this projection.
- Page index: ColumnIndex/OffsetIndex are optional but, when present, MUST be consistent
  with the cached offsets; the projection prefers its own cached offsets derived from
  page-header lengths.
- Row-group sizing: align row-group payload spans to the 256B (4-quad) block grid used by
  the avec storage plane. Guidance: choose `rows_per_rowgroup` so each column's uncompressed
  span is a multiple of 256 bytes and the total row-group payload is a multiple of the
  device page size (16 KiB on Apple Silicon). This keeps page payload boundaries and block
  boundaries from interleaving at unaligned offsets.
- Pages: target page payload sizes that keep per-page header overhead < 0.1% of payload;
  column-chunk page boundaries land on column-width boundaries by construction.

## Falsification Hooks

Each load-bearing claim gets a mechanical test; a claim with no hook is prose.

1. **Zero-level claim**: for every written page, assert the DataPageV2 header reports
   `max_rep_level == 0` and `max_def_level == 0`, and that serialized rep/def level
   buffer lengths are exactly zero. Falsified by any page carrying level bytes.
2. **1:1 mapping claim**: byte-offset assertions — for a random sample of pages, read the
   page payload slice and the corresponding Arrow record-batch buffer and require
   byte-equality (PLAIN) or deterministic round-trip equality (bit-packed). The offset
   formula `base + page_offset + header_len + column_offset_i` must locate the payload
   without any side table beyond the cached footer.
3. **Zero-allocation claim**: run a full scan under an allocation counter (global
   allocator hook or runtime tracking); require zero allocations attributable to the scan
   loop after warm-up. Falsified by any count > 0.
4. **Throughput floor**: `crates/bench`-style throughput bench over a row-group exceeding
   LLC capacity; sustained decode must clear >= 74 GiB/s. Falsified by any run below
   floor. Bench methodology: fixed sleeps banned; bounded poll on completed-bytes
   observable; ratio fence against a memcpy baseline if the absolute floor is
   environment-dependent.
5. **Admission refusal**: a writer offered an optional, repeated, nested, map, or
   variable-width field must return a typed refusal, never a flattened file. Falsified by
   any accepted invalid schema.

## Repo Cross-Check

Audit items for `crates/avec-parquet`. Audit items only — no claims about current state;
each item is a verification task, not a status report.

- [ ] Writer refuses non-flat / nullable / repeated schemas with a typed refusal (Hook 5).
- [ ] DataPageV2 headers written with max rep/def levels 0; level buffers absent (Hook 1).
- [ ] Page payload layout documented and test-asserted against Arrow buffer layout (Hook 2).
- [ ] Any bit-packed encoding branch-free and vector-width-aligned per the Page Mapping
      section; PLAIN path aliases rather than copies.
- [ ] No Thrift parsing on the scan hot path; footer parsed once and offsets cached.
- [ ] Scan loop allocation-clean under an allocator hook (Hook 3).
- [ ] Throughput bench exists and its floor is stated and measured (Hook 4).
- [ ] Row-group sizing guidance matches the 256B block alignment rule; a test or doc
      receipt shows the alignment arithmetic.

## Standing

PARTIAL — the projection landed and verified (see the header note:
test-avec-parquet PASS, gate-e2e-integration-v2 ALIVE). The typed-accessor
fix landed without a post-fix throughput receipt:
parquet-throughput-baseline is STALE-UNREMINTED (see the wave-7 amendment).
The ≥74 GiB/s floor stays NOT confirmed (65.6 measured); falsification-hook
items above remain verification tasks.

## Amendment (wave 7)

The ≥74 GiB/s throughput claims (Constraint context above and the Hook 4
floor) were tested and are NOT confirmed:

- receipts/parquet-throughput-baseline.md measured 65.6 GiB/s min (mean
  64.8) PAYLOAD-SUM on a shared M3 Max — 11% short of the floor. Standing on
  the throughput claim: PARTIAL. A quiet-machine re-run is pending.
- Typed-accessor finding — FIXED (wave 8): `scan.rs resolve()` now caches
  per-(row_group, column) slices; header parse happens at most once per
  (row_group, column). Measured 0.595 -> 8.1 GiB/s (~13.7x); accessor court
  landed as `crates/avec-parquet/tests/accessor_perf_court.rs` (cfg-gated).
- Receipt state: no post-fix throughput receipt yet —
  receipts/parquet-throughput-baseline.md is STALE-UNREMINTED for the fixed
  subject; quiet-machine re-run pending.

The 74 GiB/s target text above is kept intact; this amendment attaches the
measured state to it rather than relaxing the floor.

## See Also

- `docs/jira/v26.10.31/23-64b-quad-spec.md`
- `docs/jira/v26.10.31/21-ard.md`
