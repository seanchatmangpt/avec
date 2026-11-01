# How-to: cgap-arrow recipes

Task-oriented recipes for `cgap-arrow` users who know the basics of Apache
Arrow columnar layouts. Every snippet below was compiled and run against
`cgap-arrow` (workspace `26.10.29`); build commands and exits are recorded
per recipe.

## Build a validated named batch

**Problem.** You have row-synchronized data in several shapes — integers,
floats with nulls, byte strings — and you need one named, validated table
object that refuses ragged or duplicated columns at construction.

**Code.**

```rust
use cgap_arrow::{BatchBuilder, Column, ColumnBuilder, PrimitiveBuilder, VarlenBuilder};

fn main() {
    let mut b = BatchBuilder::new()
        .with_column("id", ColumnBuilder::Int32(PrimitiveBuilder::new()))
        .with_column("score", ColumnBuilder::Float64(PrimitiveBuilder::new()))
        .with_column("name", ColumnBuilder::Utf8(VarlenBuilder::new()));

    for i in 0..3u32 {
        if let Some(ColumnBuilder::Int32(ids)) = b.column_mut("id") {
            ids.push(Some(i as i32));
        }
        if let Some(ColumnBuilder::Float64(scores)) = b.column_mut("score") {
            scores.push(if i == 1 { None } else { Some(f64::from(i)) });
        }
        if let Some(ColumnBuilder::Utf8(names)) = b.column_mut("name") {
            names.push(Some(b"row"));
        }
    }

    let batch = b.finish().expect("rectangular batch");
    assert_eq!(batch.row_count(), 3);
    match batch.get("id") {
        Some(Column::Int32(ids)) => assert_eq!(ids.values(), &[0, 1, 2]),
        other => panic!("expected i32 column, got {other:?}"),
    }
}
```

**Why it works.** `BatchBuilder` mirrors `Column`'s closed family of array
types; `with_column` registers a named `ColumnBuilder`, and `column_mut`
hands back the typed builder so each push keeps every column in lockstep.
`finish()` freezes each builder and runs `Batch::try_new`, which enforces
the length law (every column must equal the first column's row count) and
refuses duplicate names — so a `Batch` you hold always denotes a
rectangular table, and `get`/`columns` expose typed access in insertion
order. Errors are typed (`BatchError::DuplicateColumn`,
`BatchError::LengthMismatch`), never silent repair.

Verified: `cargo build --bin r1_batch` (exit 0);
`./target/debug/r1_batch` printed `recipe 1 ok: 3 rows` (exit 0).

## Export an array through the C Data Interface and release it safely

**Problem.** You need to hand a primitive array to a foreign consumer over
the Arrow C Data Interface, and guarantee the buffers are freed exactly
once even if the other side releases twice.

**Code.**

```rust
use cgap_arrow::c_abi::{export, ForeignArray};
use cgap_arrow::PrimitiveBuilder;

fn main() {
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(42));
    b.push(None);
    b.push(Some(-1));
    let arr = b.finish();
    let format = c"i".as_ptr();

    let (array, schema) = export(arr, format);
    assert_eq!(array.length, 3);
    assert_eq!(array.null_count, 1);

    // Importer side: own the last reference; Drop runs the release once.
    let ptr = Box::into_raw(Box::new(array));
    let foreign = unsafe { ForeignArray::take(ptr) };
    assert_eq!(foreign.len(), 3);
    drop(foreign);

    // Double release is a typed no-op (CAS swap-to-NULL), never a double
    // free; after Drop the release slot observes NULL.
    unsafe {
        debug_assert!((*ptr).release.is_none());
        drop(Box::from_raw(ptr)); // free the struct shell itself
    }
    let _ = schema;
}
```

**Why it works.** `export` boxes the validity bitmap and value buffer into
an `OwnedPrimitive<T>` that lives behind `private_data`; the array's
`release` callback is `release_array::<T>`, whose first instruction is an
atomic swap of the release slot to NULL. The first caller wins and frees
the box; any later call observes a NULL previous value and returns without
touching memory. `ForeignArray` is the importer-side wrapper: `Drop` calls
the release exactly once, so ownership transfers cleanly across the ABI
boundary and the FMEA "double release" hazard degrades to a typed no-op.

Verified: `cargo build --bin r2_export` (exit 0); the binary ran clean
(exit 0).

## Ingest a foreign pair with typed, bounds-checked views

**Problem.** You received an `ArrowArray`/`ArrowSchema` pair from an
external producer and want typed access to its values and validity —
without misreading bytes when the format string lies.

**Code.**

```rust
use cgap_arrow::foreign::{format_of, is_valid, primitive_view, ForeignError};
use cgap_arrow::{export, PrimitiveBuilder};

fn main() {
    let mut b = PrimitiveBuilder::<i64>::new();
    b.push(Some(10));
    b.push(None);
    let arr = b.finish();
    let format = c"l".as_ptr();
    let (array, schema) = export(arr, format);

    let found = unsafe { format_of(&schema) }.expect("utf-8 format");
    assert_eq!(found, "l");

    // Buffer byte lengths are NOT carried by the interface; you supply
    // them from out-of-band knowledge (the spec's documented gap).
    let view = unsafe {
        primitive_view::<i64>(&schema, &array, 2 * core::mem::size_of::<i64>())
    }
    .expect("typed view");
    assert_eq!(view[0], 10);

    let validity_bytes = 1; // ceil(2 rows / 8) bytes of bitmap
    assert!(!unsafe { is_valid(&array, validity_bytes, 1) }.unwrap());

    // A format mismatch is refused as a typed error, not misread.
    let err = unsafe { primitive_view::<i32>(&schema, &array, 16) }.unwrap_err();
    assert!(matches!(err, ForeignError::FormatMismatch { .. }));
}
```

**Why it works.** `format_of` reads the schema's NUL-terminated format
string as UTF-8, and `primitive_view::<T>` refuses to proceed unless it
equals `T::FORMAT_CODE`. It then checks `n_buffers`, rejects negative
lengths, verifies the caller-supplied byte length covers `len * T::WIDTH`,
and alignment-checks the slot pointer against `align_of::<T>` — every
failure mode maps to a `ForeignError` variant. `is_valid` walks the
validity bitmap with Arrow's absent-bitmap-means-all-valid law. The views
are lifetime-free; the liveness contract (no release while a view is
alive) stays with you, the pair's owner, exactly as the spec's importer
duty assigns it.

Verified: `cargo build --bin r3_view` (exit 0); the binary ran clean
(exit 0).

## Select feature flags for no_std vs std builds

**Problem.** You target a firmware or kernel environment with no `std`,
and need to know exactly which crate features to enable and what your own
crate must still provide.

**Code.** `Cargo.toml` for a `no_std` consumer:

```toml
[dependencies]
cgap-arrow = { path = ".../cgap-arrow", default-features = false }
```

Minimal `no_std` library consumer (staticlib, `panic = "abort"`):

```rust
#![no_std]
extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
struct Bump;
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, _: Layout) -> *mut u8 { core::ptr::null_mut() }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
#[global_allocator]
static A: Bump = Bump;

use cgap_arrow::PrimitiveBuilder;

#[unsafe(no_mangle)]
pub extern "C" fn cgap_howto_check() -> i64 {
    let mut b = PrimitiveBuilder::<i64>::new();
    b.push(Some(10));
    let arr = b.finish();
    i64::try_from(arr.len()).unwrap_or(-1)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
```

For hosted (std) use, enable the opt-in feature:

```toml
[dependencies]
cgap-arrow = { path = ".../cgap-arrow", features = ["std"] }
```

**Why it works.** `cgap-arrow` is `#![no_std]` with `extern crate alloc`
unconditionally and `default = []` features — `std` is strictly opt-in and
only adds `extern crate std`. Building the crate with
`default-features = false` therefore never pulls `std` into your dependency
graph. Because the crate allocates, a `no_std` consumer must still supply a
`#[global_allocator]` and a `#[panic_handler]`; with those, the crate's
builders and the 64-byte alignment guarantee work unchanged on bare metal.
On hosted targets, `features = ["std"]` merely unlocks the `std`-gated
code paths; the columnar API surface is identical.

Verified: `cargo build` in a `no_std` staticlib consumer with
`default-features = false` (exit 0, `libnostd_check.a` produced, symbol
`cgap_howto_check` present); `cargo build` in a std consumer with
`features = ["std"]` (exit 0), binary printed `std feature ok`.
