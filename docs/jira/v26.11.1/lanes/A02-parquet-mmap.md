# A02: Zero-Copy Memory-Mapped Parquet Block Writer + Footer Sealer

Lane: A02 of 20, Squadron 1, overnight campaign v26.11.1.
Target files: `crates/cgap-parquet/src/mmap_writer.rs` (new),
`crates/cgap-parquet/src/footer.rs` (new).
Companions: A01 (Arrow slab producer), A03 (git DAG consumer), A05 (atomic
commit consumer), A17 (`P28LicenseAttestation` struct owner).

## 1. Context

`cgap-parquet` is a `no_std`, zero-dependency crate (see
`crates/cgap-parquet/src/lib.rs`: `#![no_std]`, `extern crate alloc`, and a
`std` feature that gates the hosted reader `scan`). Its Parquet surface today
is a deliberately minimal `DataPageV2` framer, documented honestly in
`crates/cgap-parquet/src/page.rs` as "a minimal DataPageV2-style framing,
NOT the full Parquet format": no row groups, no file metadata, no footer
magic, no compression. The building blocks that DO exist and are real:

- `page.rs::write_data_page_v2(values: &[u32], bits: u8, num_rows: u32) ->
  Vec<u8>` — Thrift Compact Protocol `PageHeader` (real parquet.thrift
  field ids; the PCL2 external-reader receipt in
  `docs/PHASE_CHANGE_RECEIPTS.md` fixed the swapped V2 subheader field ids
  4/5/6/7) + RLE/bit-packed hybrid body via `rle_encode_fast`.
- `page.rs::decode_page_into(page: &[u8], out: &mut [u32]) ->
  Result<usize, ParquetError>` — zero-allocation, borrow-only decode into
  caller memory, courted by the counting-global-allocator test
  `tests/page_zero_alloc.rs`.
- `thrift.rs::read_page_header` / `write_page_header`, `PageHeader`,
  `DataPageHeaderV2` — compact protocol reader/writer, lenient
  forward-compat skip of unknown fields, `MissingRequiredField` fail-closed.
- `rle.rs::rle_encode / rle_decode / decode_into` (SWAR hybrid),
  `encode_fast.rs::rle_encode_fast` (pre-sized, byte-identical, 240k-case
  differential receipt).
- `dictionary.rs::DictionaryBuilder` (build/reorder + `encode_page`/
  `decode_page`, `required_bits`).
- `ParquetError` (`rle.rs`) variants in use: `UnexpectedEof`,
  `UnsupportedBitWidth`, `RunOverrun`.

The gap this lane closes: there is no file-level container. No row-group
framing above the page, no footer, no magic, no checksum, no way for A03
(git DAG) or A05 (atomic commit) to consume a sealed, attested,
content-addressable block. And there is no mmap story at all — which on a
`no_std` bare-metal substrate is a genuine design problem, addressed in
Section 3 (ADR) with platform tiers rather than hand-waved.

Exit gate (TARGET, not measured): a 100,000-row Arrow slab (as delivered by
A01) flushes to a valid Parquet v2 row group through a static page buffer in
< 2 ms. This is labeled as a gate target throughout; no benchmark number in
this document is a measurement.

## 2. Deliverables

1. `mmap_writer.rs` — `StaticPageWriter`: a fixed-capacity, page-aligned
   static buffer block writer that emits Parquet-v2 row groups from A01
   Arrow slabs without a heap path in the flush loop.
2. `footer.rs` — `FooterSealer`: file-level footer framing (magic, metadata
   block, P28 attestation slot, checksum) producing a sealed,
   replay-verifiable block that A03/A05 can address by digest.
3. Platform-tier mmap seam (Section 3.4): a trait boundary where a hosted
   `mmap` backend can be attached without contaminating the `no_std` core.
4. Mermaid C4 diagrams (Section 5), footer byte spec (Section 6),
   interface contracts (Section 7), Chicago test plan with named
   anti-vacuity mutations (Section 8), DoD (Section 10).

## 3. ADR

### Decision

Adopt a two-layer writer:

- **Layer 1 (`mmap_writer.rs`)**: `StaticPageWriter` owns a
  compile-time-capacity, page-aligned static buffer (`page_aligned.rs`
  already provides the alignment kernel — read its real API before
  implementing). Writes are: A01 slab in -> `write_data_page_v2` per column
  chunk -> row-group framing appended into the static buffer at a
  byte-accurate offset cursor. No `Vec` growth in the flush path; capacity
  overflow is a typed `ParquetError` (new variant `CapacityExceeded`, added
  to `rle.rs` or re-mapped onto `RunOverrun` — decide at implementation;
  prefer a new variant so the court can distinguish the mutation).
- **Layer 2 (`footer.rs`)**: `FooterSealer` appends, over the static
  buffer's contents: the metadata block (Section 6.2), the P28 attestation
  slot (Section 6.3), the footer checksum, the 4-byte magic + 4-byte
  footer-length trailer (Parquet convention, fixed little-endian).

The pair is "zero-copy" in the same sense the crate already claims it
(`page.rs` module docs): the encode path borrows the caller's slab, writes
pages into a pre-sized buffer, and never copies through an intermediate
owned container. "mmap" is a *target-backend seam*, not a dependency: on
hosted targets the sealed buffer is written to a file or mapped; on
bare metal the buffer IS the device window. Honest scoping, not a claim
that we implemented an OS facility.

### Alternatives Rejected

1. **Real `mmap(2)` dependency (`memmap2`)**: std/OS-boundary dependency in
   a zero-dep `no_std` crate; violates the crate charter visible in
   `lib.rs`. Rejected for the core; permitted behind the hosted seam.
2. **Heap-backed `Vec<u8>` writer** (what `write_data_page_v2` returns
   today): the flush loop would allocate per page, breaking the zero-alloc
   law courted in `tests/page_zero_alloc.rs` and unpredictable on bare
   metal. Rejected as the flush path; retained for one-shot page encoding
   (which is its existing, receipted contract).
3. **Full Parquet thrift `FileMetaData`** (schema, row-group, column-chunk
   thrift structs): large surface, mostly metadata we do not need (ADR-02
   already eliminated levels), and the existing framer deliberately does
   not claim full-format compatibility. Rejected; we define a minimal
   metadata block (Section 6.2) that stays wire-shape-honest about not
   being full Parquet.
4. **Footerless (streaming) format**: no single digest to hand A03/A05, no
   seal/verify asymmetry, no attestation anchor. Contradicts the
   content-addressing need. Rejected.
5. **CRC in the header per page** (parquet.thrift field 4): per-page CRC
   cost in the < 2 ms target path; block-level checksum in the footer
   covers the same fault class at file granularity. Rejected for v1;
   documented as a forward option.

### 3.4 The no_std mmap story, stated honestly

Real `mmap(2)` — demand-paged file-backed virtual memory managed by a
kernel page table — does not exist on a `no_std` bare-metal target. The
honest framing is platform tiers, and the design must not pretend
otherwise:

- **Tier H (hosted, std feature)**: a real file-backed `mmap` via the OS
  (`memmap2` or libc direct) is possible and belongs behind a feature-gated
  backend implementing the `BlockSink` trait (Section 7.3). The `std`
  feature already exists as the gate pattern (`lib.rs` gates `scan` this
  way).
- **Tier M (RTOS / MMU bare metal)**: no general mmap; a statically
  reserved, page-aligned region (linker section or linker symbol) is
  handed to `StaticPageWriter` as its buffer. This is "static buffers,
  not mmap" and is the default tier.
- **Tier B (MMU-less bare metal / wasm32)**: no virtual memory at all; the
  static buffer is ordinary RAM, page-aligned only as an optimization
  (`page_aligned.rs`). The wasm footprint link glue referenced in `lib.rs`
  is the existing witness that this tier is already supported by the crate.

Consequence: the crate's "mmap writer" is a *writer that can front an mmap
window where one exists*, not an mmap implementation. The ADR text and the
module docs must say this; marketing the static buffer as mmap would be an
overclaim (see `no-overclaiming` discipline). The < 2 ms gate is defined on
the Tier M static-buffer path, which is the path every tier shares.

## 4. Wave 2 plan (02:00-04:30)

- 02:00-02:20 — read `page_aligned.rs`, `encode_fast.rs`, `thrift.rs`
  full APIs; decide `CapacityExceeded` variant placement.
- 02:20-03:10 — `mmap_writer.rs`: `StaticPageWriter::new`,
  `write_slab`, offset cursor, row-group framing, `flush` returning the
  byte length + running digest input.
- 03:10-03:50 — `footer.rs`: metadata block writer, attestation slot
  framing (consumes A17's `P28LicenseAttestation` as an opaque
  80-byte `&[u8; 80]` — this lane defines the FRAMING, not the struct),
  checksum (XXH64-class or FNV-1a; pick by no-dep constraint, document
  choice), magic + length trailer, `seal()` and `verify_seal()`.
- 03:50-04:15 — courts (Section 8), including anti-vacuity mutations.
- 04:15-04:30 — < 2 ms gate measurement on Tier M path; write the lane
  receipt with the actual measured number next to the target.

## 5. C4 Diagrams

### 5.1 Context (C4 Level 1)

```mermaid
C4Context
  title Block sealing in the cgap overnight campaign
  Person operator "Operator"
  System cgap "cgap portfolio substrate" {
    System_Ext a01 "A01 Arrow slab producer"
    System a02 "A02 block writer + footer sealer"
    System_Ext a03 "A03 git DAG writer"
    System_Ext a05 "A05 atomic commit"
    System_Ext a17 "A17 P28 attestation"
  }
  Rel(a01, a02, "Arrow slab (u32 lanes)")
  Rel(a17, a02, "80-byte attestation")
  Rel(a02, a03, "sealed block digest")
  Rel(a02, a05, "sealed block bytes + digest")
  Rel(operator, cgap, "overnight campaign orders")
```

### 5.2 Container (C4 Level 2)

```mermaid
C4Container
  title cgap-parquet container view
  ContainerDb slab "A01 Arrow slab" "fixed-shape u32 lanes"
  Container writer "mmap_writer.rs / StaticPageWriter" "no_std static buffer"
  Container footer "footer.rs / FooterSealer" "no_std footer framing"
  ContainerDb sink "BlockSink backend" "Tier H mmap / Tier M static / Tier B ram"
  ContainerDb dag "A03 git DAG" "content-addressed objects"
  Rel(slab, writer, "write_slab(slab)")
  Rel(writer, footer, "sealed candidate bytes")
  Rel(a17_link, footer, "attestation bytes")
  Rel(footer, sink, "flush sealed block")
  Rel(sink, dag, "digest -> object")
```

Note: `a17_link` in 5.2 is the attestation byte source supplied through the
same process boundary as A01; Mermaid C4 has no import statement, so the
declaration is omitted deliberately and the relationship label carries the
meaning.

### 5.3 Component (C4 Level 3)

```mermaid
C4Component
  title StaticPageWriter + FooterSealer internals
  Component cursor "OffsetCursor" "monotonic u64, saturating i32 envelope"
  Component rg "RowGroupFramer" "page seq + row-group header"
  Component meta "MetaBlock" "Section 6.2 layout"
  Component attest "AttestSlot" "80-byte P28 framing"
  Component cksum "FooterChecksum" "block digest"
  Component magic "Trailer" "PAR2 magic + u32 len"
  Rel(rg, cursor, "advances")
  Rel(meta, cursor, "reads span offsets")
  Rel(attest, meta, "slot bytes")
  Rel(cksum, meta, "digest of body+meta")
  Rel(magic, cksum, "trailer order")
```

## 6. Footer Format Specification

All integers little-endian. Offsets are relative to block start unless
stated. The block layout:

### 6.1 Byte layout

| region | offset | size | content |
|---|---|---|---|
| body | 0 | `B` | row groups (pages from `write_data_page_v2`) |
| metadata | `B` | `M` | meta block, Section 6.2 |
| attestation | `B+M` | 80 | `P28LicenseAttestation` (A17), verbatim |
| checksum | `B+M+80` | 8 | digest over `[0, B+M+80)` |
| magic | `B+M+88` | 4 | `b"PAR2"` |
| footer len | `B+M+92` | 4 | `u32 = 96 + M` (self-describing) |

Total trailer = 96 + M bytes. Minimum valid block = body(0 allowed for an
empty row-group set? No: at least one row group is required) — an empty
body is `Err(ParquetError::RunOverrun)`-class refusal, decided as: seal
with zero row groups is refused (`MissingRequiredField`-spirit; map to
`RunOverrun`'s sibling or the new variant).

### 6.2 Metadata block (M bytes)

Framed with the crate's own compact-protocol writer (`thrift.rs::
write_field_header` family) so the footer shares the wire discipline of the
pages. Field ids (footer-local, NOT parquet.thrift ids; the footer is a
cgap-private extension and says so):

| field | id | type | content |
|---|---|---|---|
| format version | 1 | i16 zigzag | 1 |
| row-group count | 2 | i32 zigzag | >= 1 |
| row count | 3 | i64 zigzag | total rows (= 100,000 for the gate slab) |
| per-row-group spans | 4 | list<i32> len | each i32 = body bytes of one row group |
| body digest prefix | 5 | binary | digest over body region only |
| flags | 6 | i32 zigzag | bit0: attestation present |

Field 5 lets `verify_seal` localize a corruption to body-vs-trailer before
the whole-block checksum verdict, which sharpens the mutation courts in
Section 8.

### 6.3 Attestation slot framing

Exactly 80 bytes, verbatim `P28LicenseAttestation` bytes as defined by A17
(see `docs/LICENSE-P28-SIL.md` at repo root for the license surface; the
struct owner is lane A17 — this lane consumes it as an opaque
`&[u8; 80]` and defines only the framing: fixed offset `B+M`, fixed size,
included in the checksum span, and gated by metadata `flags` bit0). A
zeroed attestation slot with bit0 set is a `verify_seal` failure (typed:
checksum passes, attestation check fails — distinguishable verdicts).

## 7. Interface Contracts

### 7.1 Consumes: A01 Arrow slab

- Input: `&[u32]` lanes per column, fixed row count, bit widths from the
  kernel set {1, 2, 4, 8} (`page.rs` contract; outside widths are refused
  by `UnsupportedBitWidth` on decode — the writer must not "truncate and
  hope", it refuses up front).
- Contract: `StaticPageWriter::write_slab(&mut self, slab: &SlabView) ->
  Result<(), ParquetError>` where `SlabView` is the A01-owned view type;
  until A01 lands, the lane defines `SlabView` locally as `{ lanes:
  &[&[u32]], rows: u32, bits: u8 }` and A01 adoption is a Wave-2
  integration item, not a blocker.

### 7.2 Feeds: A03 git DAG + A05 atomic commit

- `FooterSealer::seal(&self) -> SealedBlock` where `SealedBlock { bytes:
  &[u8], digest: [u8; 32] }` (digest = the same function as checksum,
  widened or prefixed; exact choice documented at implementation).
- A03 contract: the digest is the content address; `verify_seal(bytes,
  digest) -> Result<Verified, SealError>` must succeed on the stored object
  bytes for any A03 consumer.
- A05 contract: the sealed block is an immutable payload suitable for
  atomic rename/commit; `verify_seal` is the post-commit read-back check
  (Chicago: assert on the read-back state, not on the write call).

### 7.3 BlockSink seam

```rust
pub trait BlockSink {
    fn window(&mut self, len: usize) -> &mut [u8];
    fn commit(self, len: usize) -> Result<(), ParquetError>;
}
```

Tier H (std): file-backed mmap backend. Tier M: static buffer backend
(default, in-crate). Tier B: RAM backend. The core never mentions an OS.

## 8. Chicago Test Plan (real collaborators, no mocks)

All courts assert on final on-disk/on-buffer state. Where a court needs a
mutated artifact, the mutation is applied to a real copy of real writer
output and the restore is byte-verified (shim-court discipline: a court
that cannot flip is FAIL).

| # | court | real collaborator | anti-vacuity mutation (named) | must-fail assertion |
|---|---|---|---|---|
- **C1 footer checksum court** — collaborator: real `seal()` output.
  Mutation MUT-FOOT-1: flip exactly one byte in the body region; verdict
  must change from `Ok` to typed `SealError::ChecksumMismatch`. Non-vacuity:
  flipping zero bytes still passes.
- **C2 footer-byte court** — collaborator: same sealed block. Mutation
  MUT-FOOT-2: flip one byte INSIDE the footer (metadata field 3, row
  count). Must fail as checksum mismatch, localized to trailer via the
  field-5 body-digest comparison.
- **C3 attestation court** — collaborator: real A17 struct bytes (until A17
  lands, a real 80-byte fixture derived from `LICENSE-P28-SIL.md`
  constants). Mutation MUT-FOOT-3: zero the attestation slot, keep flags
  bit0 set. Must fail with the attestation verdict while the checksum
  verdict still passes.
- **C4 zero-alloc flush court** — collaborator: counting global allocator
  (existing pattern: `tests/page_zero_alloc.rs`). Mutation MUT-FOOT-4:
  switch the flush to a `Vec` push path. Assert: alloc count in flush is 0
  on the real path; the mutated path must show > 0 (the court can flip).
- **C5 < 2 ms gate court** — collaborator: Tier M static path, 100k-row
  slab, min-of-N vs a sleep-free ratio baseline. No mutation (timing
  court); non-vacuity = the gate is labeled TARGET and the lane receipt
  carries the measured number, or records TARGET-MISSED with it.
- **C6 refusal courts** — collaborators: oversize slab, zero row groups,
  bad bit width. Mutation MUT-FOOT-6: bypass the up-front width refusal by
  writing then decoding. Decode must still refuse
  (`UnsupportedBitWidth`); refusal is not bypassable by writing first.
- **C7 external-reader court** — collaborator: pyarrow/arrow-rs reading the
  row-group pages (Tier H only). Mutation MUT-FOOT-7: swap the V2
  subheader field ids (the historical PCL2 defect). External reader fails
  on mutated, succeeds on real output.

### 8.1 Court non-vacuity protocol

Each mutation court runs BASELINE (sha256 of sealed output) -> BEFORE
(verdict on clean block) -> MUTATION -> AFTER (verdict must differ) ->
RESTORE (sha256 byte-exact). A court whose AFTER verdict equals its BEFORE
verdict exits 1 as VACUOUS. Skips are loud (`[SKIP] <reason>`), never
silent passes.

## 9. Risks

- **Schedule — < 2 ms target missed on hosted CI (noisy neighbors).**
  Mitigation: min-of-N, ratio fence vs sleep-free baseline
  (`no-sleep-testing` discipline); the receipt carries the number, target
  labeled TARGET throughout.
- **Dependency — A17 struct not landed when this lane runs.** Mitigation:
  opaque `[u8; 80]` fixture; C3 upgrades to the real struct when A17 lands.
- **Shared-file — `CapacityExceeded` variant churn in `rle.rs`.**
  Mitigation: disclose in both lanes' receipts if `rle.rs` is contended
  (compile-freeze SLA).
- **Infra — external-reader court flaky on CI (pyarrow install).**
  Mitigation: C7 is Tier H optional, loud-skip with reason; C1-C4 are the
  load-bearing courts.
- **Design — static buffer capacity too small for the 100k slab.**
  Mitigation: capacity is a `const` parameter, documented with the gate;
  oversize is a typed refusal, never silent truncation.
- **Doc — "mmap" naming read as overclaim.** Mitigation: Section 3.4
  platform tiers; module docs repeat the honest scoping.

## 10. Definition of Done

1. `mmap_writer.rs` and `footer.rs` exist in `crates/cgap-parquet/src/`,
   `#![no_std]`-clean, zero new dependencies (verify: `Cargo.toml` diff
   empty in `[dependencies]`).
2. `lib.rs` gains `pub mod mmap_writer; pub mod footer;` with module docs
   repeating the Tier M/H/B honest scoping.
3. `StaticPageWriter` flush path is allocation-free, proven by C4 with the
   MUT-FOOT-4 flip witness.
4. Footer spec (Section 6) implemented byte-for-byte; a byte-offset table
   in the module docs matches the code (C1/C2 prove the offsets by
   mutation).
5. P28 attestation slot framed at exactly 80 bytes, checksum-spanned,
   flags-gated (C3).
6. `verify_seal` returns typed, distinguishable verdicts
   (checksum vs attestation vs shape).
7. C1-C4, C6 courts green; C5 measured and recorded in the lane receipt
   with target-vs-actual; C7 run or loud-skipped.
8. 100,000-row slab seals end-to-end on the Tier M path (the gate
   scenario), with the sealed block re-verified by `verify_seal` from the
   bytes (read-back, not in-memory handoff).
9. All mutation courts demonstrate at least one flip (AFTER differs from
   BEFORE) or the court is recorded FAIL, not PASS.
10. Lane receipt written with: exact commands, exits, measured gate number,
   and any cross-lane disclosures (A01 `SlabView` adoption, A17 struct
   handoff).

## See Also

`docs/jira/v26.11.1/00-index.md` (campaign index) — sibling lanes A01, A03,
A05, A17; `docs/jira/v26.10.31/22-adr-register.md` (ADR-01..04);
`docs/PHASE_CHANGE_RECEIPTS.md` (PCL2 field-id receipt);
`docs/BRANCH_REGISTER.md`; `LICENSE-P28-SIL.md`.
