# Tutorial: your first columnar batch

In this tutorial you will build your first **columnar batch** with cgap: a
small, typed, named table held in contiguous, 64-byte-aligned buffers, then
read the values back out.

You will learn how the three core pieces fit together:

- `BatchBuilder` — attach named columns and push rows.
- `Batch` — the frozen, rectangular result of a build.
- `Column` — one typed array inside a batch, read by format code.

Everything here runs against the published `cgap` umbrella crate (or
`cgap-arrow` 26.10.29 directly — the API is identical; the umbrella simply
re-exports it).

## Why a batch, not a `Vec<Row>`

Row-oriented storage scatters one logical row across many allocations. A
columnar batch stores each field's values contiguously: scanning one field
over a million rows walks one buffer. cgap enforces the columnar invariants
for you — every buffer is 64-byte aligned (an AVX-512 / NEON requirement),
and a `Batch` can only exist if all its columns have the same row count.

## What you need

- Rust (any recent stable toolchain; the crate is edition 2021 compatible).
- The `cgap` crate. Two equivalent ways to depend on it:

  ```toml
  # From crates.io (registry form):
  [dependencies]
  cgap = "26.10.29"
  ```

  ```toml
  # From a local checkout (path form, works offline):
  [dependencies]
  cgap = { path = "/Users/sac/cgap/crates/cgap", version = "26.10.29" }
  ```

  Both give the same `cgap::cgap_arrow` module. If you prefer the kernel
  crate directly, substitute `cgap-arrow = "26.10.29"` (registry) or
  `cgap-arrow = { path = ".../crates/cgap-arrow" }` (path) and drop the
  `cgap_arrow::` segment from the import paths below.

## Step 1: create a scratch project

```console
$ cargo new cgap-tut01 --bin
$ cd cgap-tut01
```

Add the dependency to `Cargo.toml`, then replace `src/main.rs` with the
listing in the next step.

## Step 2: build, freeze, and read the batch

```rust
use cgap::cgap_arrow::batch::{Batch, BatchBuilder, ColumnBuilder};
use cgap::cgap_arrow::primitive::PrimitiveBuilder;
use cgap::cgap_arrow::varlen::VarlenBuilder;

fn main() {
    // 1. Assemble the batch: three named columns.
    let mut builder = BatchBuilder::new();
    builder = builder
        .with_column("id", ColumnBuilder::Int32(PrimitiveBuilder::new()))
        .with_column("score", ColumnBuilder::Float64(PrimitiveBuilder::new()))
        .with_column("name", ColumnBuilder::Utf8(VarlenBuilder::new()));

    // 2. Push rows: id and score are fixed-width, name is variable-length.
    if let Some(ColumnBuilder::Int32(ids)) = builder.column_mut("id") {
        ids.push(Some(1));
        ids.push(Some(2));
        ids.push(Some(3));
    }
    if let Some(ColumnBuilder::Float64(scores)) = builder.column_mut("score") {
        scores.push(Some(97.5));
        scores.push(None); // null: the validity bitmap records it
        scores.push(Some(88.0));
    }
    if let Some(ColumnBuilder::Utf8(names)) = builder.column_mut("name") {
        names.push(Some(b"ada"));
        names.push(Some(b"grace"));
        names.push(Some(b"linus"));
    }

    // 3. Freeze: length law + duplicate-name law checked here.
    let batch: Batch = builder.finish().expect("columns are rectangular");

    // 4. Inspect what you built.
    println!("row_count = {}", batch.row_count());
    for (name, column) in batch.columns() {
        println!(
            "column {name:?}: format = {}, len = {}",
            column.format_code(),
            column.len()
        );
    }

    // 5. Read one fixed-width column back by values.
    match batch.get("id") {
        Some(cgap::cgap_arrow::batch::Column::Int32(v)) => {
            println!("id values = {:?}", v.values());
        }
        _ => unreachable!(),
    }

    // 6. Read the variable-length column back row by row.
    match batch.get("name") {
        Some(cgap::cgap_arrow::batch::Column::Utf8(v)) => {
            for i in 0..v.len() {
                println!(
                    "name[{i}] = {}",
                    String::from_utf8(v.row(i).to_vec()).unwrap()
                );
            }
        }
        _ => unreachable!(),
    }
}
```

## Step 3: run it

```console
$ cargo run
```

Expected output (from the verified run of exactly this program):

```text
row_count = 3
column "id": format = i, len = 3
column "score": format = g, len = 3
column "name": format = u, len = 3
id values = [1, 2, 3]
name[0] = ada
name[1] = grace
name[2] = linus
```

## What happened here

- `with_column(name, ColumnBuilder::...)` attaches a typed column builder
  under a `&'static str` name, in insertion order.
- `column_mut(name)` borrows the builder back out so you can `push` rows.
  Each `ColumnBuilder` variant wraps the plain array builders
  (`PrimitiveBuilder`, `VarlenBuilder`, ...), so pushes go through the
  same per-type APIs.
- `finish()` freezes every column and enforces the two batch laws:
  duplicate column names are refused, and ragged columns are refused.
  A `Batch` in hand always denotes a rectangular table.
- Fixed-width columns push `Option<T>`; `None` records a null in the
  validity bitmap. Variable-length columns push `Option<&[u8]>` bytes.
- Format codes (`i`, `g`, `u`) are the Arrow format strings for the
  column's element type — the same codes the C Data Interface uses.

## Next steps

- Tutorial 02: hash a batch's payload and write it into a Git packfile
  (`02-hash-and-packfile.md`).
- Tutorial 03: cross the FFI boundary with the Arrow C Data Interface
  (`03-ffi-c-data-interface.md`).
