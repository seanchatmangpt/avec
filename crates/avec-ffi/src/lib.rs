//! avec-ffi: C-ABI membrane (cdylib) for Gate 3 wire-parity.
//!
//! Exports a primitive i32 batch as raw ArrowArray/ArrowSchema pointers that
//! a foreign consumer (PyArrow, DuckDB, Polars) imports via the C Data
//! Interface. Wire parity is proven by `scripts/gate3_cdata.py` importing
//! through pyarrow's `_import_from_c`.

use avec_arrow::c_abi::{export, ArrowArray, ArrowSchema};
use avec_arrow::primitive::{PrimitiveBuilder, Scalar};
use core::sync::atomic::{AtomicPtr, Ordering};

/// Live export slots read by the foreign importer.
static ARRAY_SLOT: AtomicPtr<ArrowArray> = AtomicPtr::new(core::ptr::null_mut());
static SCHEMA_SLOT: AtomicPtr<ArrowSchema> = AtomicPtr::new(core::ptr::null_mut());
static ROWS: core::sync::atomic::AtomicI64 = core::sync::atomic::AtomicI64::new(0);

/// # Safety
/// Returns the live ArrowArray pointer; the CALLER (importer) takes release
/// ownership per the C Data Interface contract.
#[no_mangle]
pub unsafe extern "C" fn avec_i32_batch(n: i64, null_every: i64) -> *mut ArrowArray {
    let mut b = PrimitiveBuilder::<i32>::new();
    let mut i = 0;
    while i < n {
        if null_every > 0 && i % null_every == null_every - 1 {
            b.push(None);
        } else {
            b.push(Some(i as i32));
        }
        i += 1;
    }
    let (array, schema) = export(b.finish(), b"i\0".as_ptr() as *const _);
    ROWS.store(n, Ordering::Relaxed);
    ARRAY_SLOT.store(Box::leak(Box::new(array)), Ordering::Relaxed);
    SCHEMA_SLOT.store(Box::leak(Box::new(schema)), Ordering::Relaxed);
    ARRAY_SLOT.load(Ordering::Relaxed)
}

/// Address of the exported schema slot.
#[no_mangle]
pub extern "C" fn avec_schema_addr() -> usize {
    SCHEMA_SLOT.load(Ordering::Relaxed) as usize
}

/// Address of the exported array slot.
#[no_mangle]
pub extern "C" fn avec_array_addr() -> usize {
    ARRAY_SLOT.load(Ordering::Relaxed) as usize
}

/// Length of the last exported batch.
#[no_mangle]
pub extern "C" fn avec_batch_rows() -> i64 {
    ROWS.load(Ordering::Relaxed)
}

/// avec version string pointer (NUL-terminated).
#[no_mangle]
pub extern "C" fn avec_version() -> *const core::ffi::c_char {
    avec_arrow::VERSION.as_ptr() as *const core::ffi::c_char
}
