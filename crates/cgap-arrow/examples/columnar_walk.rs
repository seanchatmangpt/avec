//! End-to-end printed walk of the cgap-arrow substrate.
//!
//! Run: `cargo run -p cgap-arrow --example columnar_walk`
//!
//! Every section prints evidence lines: alignment proofs, format codes, SWAR
//! mask counts with a scalar cross-check, varlen offsets, view-vector
//! inline/foreign classification, and the C Data Interface export path down
//! to raw `ArrowArray` fields and the release-once / double-release-noop
//! control proof.

use cgap_arrow::bitmap::{self, Bitmap};
use cgap_arrow::c_abi::{export, ArrowArray, ForeignArray};
use cgap_arrow::varlen::Offset;
use cgap_arrow::{Primitive, PrimitiveBuilder, VarlenBuilder, ViewVecBuilder};

fn main() {
    println!("cgap-arrow v{} — columnar_walk", cgap_arrow::VERSION);

    primitive_walk();
    swar_mask_walk();
    varlen_walk();
    view_walk();
    c_abi_walk();

    println!("columnar_walk: all sections printed");
}

/// Section 1: i32 primitive with nulls — builder, alignment, format code.
fn primitive_walk() {
    println!("\n[1] primitive: i32 with nulls");
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(10));
    b.push(None);
    b.push(Some(i32::MAX));
    b.push(Some(-7));
    let arr = b.finish();

    println!("     len            = {}", arr.len());
    println!("     format code    = {:?}", Primitive::<i32>::format_code());
    let nulls = arr.validity().map_or(0, Bitmap::null_count);
    println!("     null_count     = {}", nulls);
    println!("     values[0..4]   = {:?}", &arr.values()[..arr.len()]);

    let v = arr.validity_ptr() as usize;
    let x = arr.values_ptr() as usize;
    println!(
        "     values_ptr % 64    = {:3}   validity_ptr % 64 = {:3}",
        x % 64,
        v % 64
    );
    println!(
        "     buffers_aligned_64 = {}",
        arr.buffers_aligned_64()
    );
}

/// Section 2: SWAR validity+predicate mask over 64 rows (one word).
fn swar_mask_walk() {
    println!("\n[2] SWAR mask: 64 rows, one word");
    let mut b = PrimitiveBuilder::<i32>::new();
    for i in 0..64i32 {
        // nulls at i % 7 == 0; values otherwise i * 3
        b.push(if i % 7 == 0 { None } else { Some(i * 3) });
    }
    let arr = b.finish();
    let bm = arr.validity().expect("nulls present by construction");

    let valid = [bm.word(0)];
    println!("     valid word      = {:#018x}", valid[0]);

    // Predicate: value >= 100, built as a word from the typed value slice.
    let vals = arr.values();
    let mut pred = [0u64; 1];
    for (i, &v) in vals[..64].iter().enumerate() {
        if v >= 100 {
            pred[0] |= 1 << i;
        }
    }
    println!("     pred word       = {:#018x}", pred[0]);

    let mut out = [0u64; 1];
    bitmap::mask_words(&valid, &pred, &mut out);
    let admitted = u64::from(out[0].count_ones());
    println!("     masked word     = {:#018x}", out[0]);
    println!("     admitted rows (SWAR popcount) = {}", admitted);
    println!(
        "     count_valid(valid, pred)      = {}",
        bitmap::count_valid(&valid, &pred)
    );

    // Scalar cross-check: real state, no mocks — both must agree.
    let mut scalar = 0u64;
    for i in 0..64usize {
        if valid[0] >> i & 1 == 1 && pred[0] >> i & 1 == 1 {
            scalar += 1;
        }
    }
    println!("     scalar cross-check             = {}", scalar);
    debug_assert_eq!(admitted, scalar);
    debug_assert_eq!(admitted, bitmap::count_valid(&valid, &pred));

    // Null rows are deterministic ZERO slots — show one.
    println!(
        "     null slot values[0] (i=0)     = {} (deterministic ZERO)",
        vals[0]
    );
}

/// Section 3: Varlen u32 offsets — unicode rows + a null row.
fn varlen_walk() {
    println!("\n[3] varlen: u32 offsets, unicode + null");
    let mut b = VarlenBuilder::<u32>::new();
    b.push(Some("héllo".as_bytes())); // 6 bytes utf8
    b.push(Some("日本語".as_bytes())); // 9 bytes utf8
    b.push(Some("naïve".as_bytes())); // 6 bytes utf8
    b.push(None);
    let v = b.finish();

    println!("     len                = {}", v.len());
    println!(
        "     offset family      = {:?} (format \"u\" = utf8)",
        <u32 as Offset>::format_family()
    );
    println!("     offsets            = {:?}", v.offsets());
    for i in 0..v.len() {
        let row = v.row(i);
        let s = core::str::from_utf8(row).map_or("<invalid utf8>", |s| s);
        let valid = v.validity().is_none_or(|bm| bm.get(i));
        println!(
            "     row {} ({} bytes, valid={}) = {:?}",
            i,
            row.len(),
            valid,
            s
        );
    }
    println!("     offsets_monotonic  = {}", v.offsets_monotonic());
}

/// Section 4: ViewVec — inline (<=12 B) and foreign (>12 B) rows.
fn view_walk() {
    println!("\n[4] views: inline + foreign rows");
    let mut b = ViewVecBuilder::new();
    b.push(Some(b"short")); // 5 B  -> inline
    b.push(Some(b"this row lives in the data block")); // 31 B -> foreign
    b.push(None); // null row
    b.push(Some(b"")); // 0 B -> inline
    let v = b.finish();

    println!("     len              = {}", v.len());
    for i in 0..v.len() {
        let valid = v.validity().is_none_or(|bm| bm.get(i));
        let class = if !valid {
            "null"
        } else if cgap_arrow::views::is_inline(v.row(i).len() as u32) == 1 {
            "inline"
        } else {
            "foreign"
        };
        let s = core::str::from_utf8(v.row(i)).map_or("<invalid utf8>", |s| s);
        println!(
            "     row {} ({} B, {:7}) = {:?}",
            i,
            v.row(i).len(),
            class,
            s
        );
    }
    println!(
        "     descriptors      = {} x 16 B = {} B",
        v.len(),
        v.views_bytes().len()
    );
    println!("     data block       = {} B", v.data().len());
    println!(
        "     MAX_INLINE       = {} B",
        cgap_arrow::views::MAX_INLINE
    );
}

/// Section 5: C Data Interface export — raw field walk, take, release-once,
/// double-release typed no-op.
fn c_abi_walk() {
    println!("\n[5] c_abi: export -> heap ArrowArray -> release control");
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(42));
    b.push(None);
    b.push(Some(-1));
    let arr = b.finish();

    let format = c"i".as_ptr();
    let (array, schema) = export(arr, format);

    println!(
        "     schema: format={:?} flags={} (ARROW_FLAG_NULLABLE={})",
        unsafe { core::ffi::CStr::from_ptr(schema.format) }.to_string_lossy(),
        schema.flags,
        cgap_arrow::c_abi::CGAP_FLAG_NULLABLE
    );

    // Heap the array so the release callback frees exactly this shell.
    let ptr = Box::into_raw(Box::new(array));

    // SAFETY: we are the sole owner of the heap ArrowArray just created.
    let foreign = unsafe { ForeignArray::take(ptr) };
    println!("     foreign.len()    = {}", foreign.len());
    // SAFETY: two-buffer primitive layout, both slots live.
    unsafe {
        println!("     buffer[0] (validity) = {:p}", foreign.buffer(0));
        println!("     buffer[1] (values)   = {:p}", foreign.buffer(1));
    }

    // Raw field walk before release.
    // SAFETY: sole owner; read-only field walk.
    unsafe {
        let a: &ArrowArray = &*ptr;
        println!("     raw fields: length={} null_count={} offset={} n_buffers={} n_children={}",
            a.length, a.null_count, a.offset, a.n_buffers, a.n_children);
    }

    println!("     drop ForeignArray -> release #1 (frees owned buffers)");
    drop(foreign);

    // Second release attempt: run the exact `ForeignArray::drop` code path
    // again. The CAS swap-to-NULL made release #1 the sole winner, so the
    // second attempt observes `release == NULL` and is a typed no-op.
    // SAFETY: shell struct itself is still allocated (freed below).
    unsafe {
        let slot = (*ptr).release;
        match slot {
            Some(f) => {
                f(ptr);
                println!("     release #2: callback WAS set and ran (unexpected after drop #1)");
            }
            None => {
                println!(
                    "     release #2: slot NULL (CAS-swapped) -> typed no-op, no double free"
                );
            }
        }
        drop(Box::from_raw(ptr)); // free the struct shell itself
        println!("     shell freed — export path fully reclaimed");
    }
}
