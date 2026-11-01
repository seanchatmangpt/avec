# CGAP Consumer Requirements — PurRDF dependency pass (2026-10-29)

Requirements a PurRDF consumption of cgap must see satisfied before wiring.
Status vocabulary follows `docs/DFLSS_CHARTER.md` (ALIVE / PARTIAL / BLOCKED /
NO-RECEIPT / NO-MECHANISM / UNVERIFIED). Every claim cites its evidence.
PurRDF-side requirement sources: `docs/jira/v26.10.26/optimization/OPT-17-arrow-egress-wiring.md`
and `crates/process/src/log/{model,write}.rs` at purrdf `c0059bf5`.

## 0. Publication fit (NO TICKET — fits today)

| fact | evidence |
|---|---|
| both crates zero-dep, `no_std` | both crates' `Cargo.toml` (empty `[dependencies]`) |
| `publish = true` + dry-run gate | `scripts/release_check.py`, README Receipts |
| version 26.10.29 == purrdf | `crates/cgap-arrow/src/generated_version.rs` |
| no wasm target cfgs | grep over `crates/*/src` |
| Apache-2.0 as external | purrdf `dependency-ledger.toml`, `structured-zstd` form |
| publication bridge | purrdf `[patch.crates-io]`, frozen-duckdb precedent |

The version is ontology-driven: `ontology/cgap.ttl` → `ggen sync run` →
`crates/cgap-arrow/src/generated_version.rs`, with the pin enforced by
`scripts/release_check.py`. A consumer pins `cgap-arrow = "26.10.29"` and
bumps with cgap's releases; that is the release contract consumers honor.
(cgap moves; the consumer never patches cgap's version pins.)

## T1 (high): named batch/table layer over the vector builders

**Consumer ask**: purrdf-process's columnar OCEL projection (EAV shape, 7
tables: events / event_attrs / event_object_refs / objects / object_attrs /
o2o / types — attributes are schema-free `BTreeMap`s, so a fixed-column
projection does not fit) needs a named, row-count-synchronized batch
container over the existing builders. Today the only multi-column construct
is the wire-format `export_struct` (`crates/cgap-arrow/src/c_abi_nested.rs:128`),
with caller-supplied lengths and no named columns; the closest producer is
the cgap-ffi single-column `cgap_i32_batch` (`crates/cgap-ffi/src/lib.rs:22`).
**No RecordBatch-shaped type exists.**

**Acceptance shape** (minimum): a `Batch`/`Table` type holding named columns
(`&'static str` names, boxed `Column` enum over `Primitive/Varlen/ViewVec/
RunEndEncoded`), row-count synchronization (every column's len asserted at
construction; mismatch = typed refusal), deterministic column order
(insertion order, `Vec<(name, column)>`), plus a builder mirror
(`BatchBuilder` with per-column builders + finish-time length check). All
`no_std` + zero-dep; the 64-byte alignment invariant holds because every
column is builder-produced.

**Priority rationale**: T1 is the gate for purrdf-process adoption; T2 is the
gate for OPT-17's slab handoff. The wiring recipe for both lives in purrdf's
`docs/jira/v26.10.26/cgap-consumption.md`.

## T2 (high): typed zero-copy ingest of foreign Arrow buffers

**Consumer ask**: OPT-17's acceptance invariant says slabs are "written from
existing Arrow buffers" (OPT-17 §3 point 3). Today every typed representation
is builder-owned with copies (`Buf` owned-only with unconditional `Drop`
dealloc, `buffer.rs:113-126`; builders copy into fresh aligned `Buf`s;
`Sliced` borrows only cgap-owned arrays, `slice.rs:50`; and the only ingress
for foreign data, `c_abi.rs:164 ForeignArray::take`, returns raw void
pointers with no schema interpretation and no typed bridge). Acceptance:
a typed foreign-array bridge — schema-format parse → typed views, non-owning
(lifetime-bound views over caller memory, or an externally-owned `Buf`
variant with a drop-policy enum), validated against cgap's existing pyarrow
parity courts (`scripts/gate3_cdata.py`).

## T3 (medium): timestamp `Scalar` types

`Scalar` covers ten primitives (`crates/cgap-arrow/src/primitive.rs:38-47`:
c/C/s/S/i/I/l/L/f/g). Timestamps and decimals exist only as inert layout
metadata (`crates/cgap-arrow/src/generated_layout.rs:70-84, 326-357`), with
no backing `Scalar` impl. The primary consumer keeps lexical Utf8 as wire
truth (sorts and digests compare lexical xsd:dateTime strings), so this is a
typed-access convenience, not a blocker: minimum acceptance is a timestamp
`Scalar` (at least one unit, e.g. microsecond) as a newtype over i64 with the
spec format code (`tsu:...` family from the layout metadata), so OCEL time
columns can be typed rather than Utf8.

## T4 (medium): wasm32-unknown-unknown receipt

The footprint receipt measures `wasm32-wasip1` only
(`scripts/footprint.sh:41`, `crates/wasm-footprint/Cargo.toml:5`). No target
cfg blocks `wasm32-unknown-unknown`, but no build, gate, or number exists for
it, and the link glue (panic handler, global allocator, EH stub) is
consumer-supplied (`crates/wasm-footprint/src/lib.rs:4-7`). Acceptance: the
primary consumer builds `wasm32-unknown-unknown --lib` over both crates in
its own CI (its `make wasm` allowlist), and cgap adds an unknown-unknown
footprint row to `scripts/footprint.sh` + `docs/wasm-footprint.txt` so the
claim transfers by receipt.

## Related existing capability (not a gap)

`crates/cgap-arrow/src/digest.rs` (landed 26fcb35) is a rotate-xor
mutation-detection digest over masked bitmap words — explicitly non-
cryptographic, accidental-rot class only. The consumers' digest needs
(BLAKE3 over slab bytes, OPT-17 `columnar_digest`) are met consumer-side by
`purrdf-hash`; the zero-dependency law keeps BLAKE3 out of cgap.

## Standing

| element | standing |
|---|---|
| publication fit | DECIDED (patch bridge; crates.io is destination) |
| T1 batch/table layer | LANDED (ALIVE) |
| T2 typed zero-copy ingest | LANDED (ALIVE) |
| T3 timestamp Scalar | LANDED (ALIVE) |
| T4 unknown-unknown wasm receipt | LANDED (ALIVE) |
| dictionary vector layer | DEFERRED (later want for EAV name/type columns) |
| purrdf wiring | TRIGGER MET (T1+T2 landed) — recipe in purrdf's
`docs/jira/v26.10.26/cgap-consumption.md` §2 |

## Receipts (2026-10-29, tickets landed)

- **T1** — `crates/cgap-arrow/src/batch.rs`: `Column` (14 families),
  `Batch::try_new` length law (ragged + duplicate-name refusals),
  `BatchBuilder`/`ColumnBuilder` mirror; `VarlenBuilder::len`/`ViewVecBuilder::len`
  added (swept into co-lane commit `18ece61`/`177cec4` window). Receipt:
  `cargo test -p cgap-arrow --lib` → **54 passed; 0 failed**; full test
  targets green (property 1, validation 6, mutation_kills 36/36, 1+35
  filtered).
- **T2** — `crates/cgap-arrow/src/foreign.rs`: `format_of`,
  `primitive_view::<T>` (format-match, bounds vs caller-supplied buffer
  length, alignment check), `is_valid` (absent bitmap = all-valid Arrow
  law). Receipt: export→view round-trip, format-mismatch refusal, and
  short-buffer refusal all pass in the 54; `gate3_cdata.py` (pyarrow
  bitwise parity) remains the external court, not re-run here.
- **T3** — `crates/cgap-arrow/src/timestamps.rs`: `TimestampSecond/Milli/
  Micro/Nano` newtypes over i64, format codes `tss:/tsm:/tsu:/tsn:UTC`
  matching the layout prefixes; byte-identity with i64 columns pinned by
  test. Included in the 54.
- **T4** — `scripts/footprint.sh` `WASM_TARGET` override; run receipt:
  `WASM_TARGET=wasm32-unknown-unknown bash scripts/footprint.sh` → GATE
  PASS (raw 4,309 B, stripped 3,435 B, CTQ-4 PASS, version court byte-exact
  "26.10.29", 5-export surface + 15 node checks); plus release rlib builds
  for both crates (arrow 399,168 B / parquet 250,526 B rlib).
