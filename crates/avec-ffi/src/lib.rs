//! avec-ffi: C-ABI membrane (cdylib) for Gate 3 wire-parity.
//!
//! Exports a primitive i32 batch as raw ArrowArray/ArrowSchema pointers that
//! a foreign consumer (PyArrow, DuckDB, Polars) imports via the C Data
//! Interface. Wire parity is proven by `scripts/gate3_cdata.py` importing
//! through pyarrow's `_import_from_c`.

use avec_arrow::c_abi::{export, ArrowArray, ArrowSchema};
use avec_arrow::primitive::PrimitiveBuilder;
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
    // Rust &str is NOT NUL-terminated; serve the generated C form
    // (drift-guarded: version_cstr_parity asserts the prefix equals VERSION).
    avec_arrow::VERSION_C.as_ptr() as *const core::ffi::c_char
}

/// Rotate-xor fold word step. MUST stay byte-for-byte identical to the
/// private `fold_word` in `avec_arrow::digest` — drift is caught by the
/// `digest_ffi::ffi_fold_parity_matches_digest_of` test, not by review.
#[inline]
fn fold_word(acc: u64, w: u64) -> u64 {
    acc.rotate_left(1) ^ w
}

/// Mutation-detection digest of the live exported array at `array_addr`
/// (the address [`avec_array_addr`] serves).
///
/// Reads the C Data Interface struct at that address (length, offset,
/// n_buffers, buffers) and folds the physical buffers exactly as
/// `avec_arrow::digest::digest_of` folds a `Primitive<i32>`:
///
/// 1. value lanes — i32 lanes paired low-half/high-half into one u64 word
///    each, folded in order; an odd tail lane zero-extends into the low
///    half;
/// 2. validity words — when the validity buffer is present (not NULL),
///    each 8-byte word starting at buffer offset `w*8`, reading only the
///    `ceil(length/8)` bitmap bytes and zero-filling the tail word beyond
///    them (byte-for-byte `Bitmap::word`). An all-valid batch carries no
///    validity buffer (Arrow law) and folds no validity words.
///
/// Assumptions (loudly asserted, not silent): the array is an avec-exported
/// primitive i32 batch (`n_buffers == 2`, value lanes at `buffers[1]`,
/// validity at `buffers[0]`). A null `array_addr` or a foreign layout
/// panics — the digest is only defined for the export this crate produces.
///
/// Digest law (pinned by tests on both sides of the membrane): empty array
/// folds to `0`; null slots are stored zero at build time, so the digest is
/// a deterministic function of the physical buffers.
///
/// THIS IS NOT A CRYPTOGRAPHIC HASH — rotate-xor, no avalanche, no keying,
/// no collision resistance. Its only claim is ACCIDENTAL-mutation detection;
/// see `docs/CONTENT_ADDRESSING.md` for the claim boundary and the
/// tree-hash upgrade path.
#[no_mangle]
pub extern "C" fn avec_digest_of(array_addr: usize) -> u64 {
    assert!(array_addr != 0, "avec_digest_of: null array address");
    // SAFETY: array_addr is the caller-asserted address of a live exported
    // ArrowArray (the caller read it from avec_array_addr while the export
    // slots are unreleased); we only READ through it here.
    unsafe { digest_of_raw(array_addr as *const ArrowArray) }
}

/// Raw-buffer fold over a live ArrowArray. Byte-for-byte the
/// `avec_arrow::digest::digest_of` fold (canon: `crates/avec-arrow/src/
/// digest.rs`); parity against the canon is test-pinned, since
/// `digest_of` takes a `&Primitive<i32>` and no public constructor exists
/// from raw C Data Interface parts.
///
/// # Safety
/// `array` must point at a live, avec-exported ArrowArray for the duration
/// of the call (read-only).
unsafe fn digest_of_raw(array: *const ArrowArray) -> u64 {
    let length = (*array).length;
    assert!(length >= 0, "avec_digest_of: negative length {length}");
    let n_buffers = (*array).n_buffers;
    assert!(
        n_buffers == 2,
        "avec_digest_of: expected a primitive i32 array (n_buffers == 2), got {n_buffers}"
    );
    let n = length as usize;
    let offset = (*array).offset as usize;
    let buffers = (*array).buffers;

    let mut acc: u64 = 0;

    // 1. Value lanes: i32 lanes paired low/high into u64 words, in order;
    //    odd tail lane zero-extends into the low half. digest.rs indexes
    //    lanes (never re-slices bytes), so this is platform-independent.
    if n > 0 {
        let vals_ptr = *buffers.add(1);
        assert!(!vals_ptr.is_null(), "avec_digest_of: null values buffer");
        // SAFETY: live export with `n` i32 lanes at `buffers[1] + offset`
        // (the builder allocates exactly ceil-64-aligned length*T bytes).
        let vals = unsafe { core::slice::from_raw_parts(vals_ptr.add(offset) as *const i32, n) };
        let mut i = 0;
        while i + 1 < n {
            let w = (vals[i] as u32 as u64) | ((vals[i + 1] as u32 as u64) << 32);
            acc = fold_word(acc, w);
            i += 2;
        }
        if i < n {
            acc = fold_word(acc, vals[i] as u32 as u64);
        }
    }

    // 2. Validity words: present buffer only (all-valid folds nothing);
    //    tail word reads only available bytes, zero-fills the rest — the
    //    Bitmap::word law — so builder padding bits never reach the fold.
    let validity_ptr = *buffers.add(0);
    if !validity_ptr.is_null() {
        let bytes_len = n.div_ceil(8); // bitmap_bytes(n_bits), canon: bitmap.rs
        // SAFETY: live export; the validity buffer carries exactly
        // ceil(length/8) bytes (Buf over Bitmap::from_bools).
        let bytes = unsafe { core::slice::from_raw_parts(validity_ptr as *const u8, bytes_len) };
        let words = n.div_ceil(64); // bitmap_words(n_bits), canon: bitmap.rs
        let mut w = 0;
        while w < words {
            let off = w * 8;
            let take = bytes_len.saturating_sub(off).min(8);
            let mut buf = [0u8; 8];
            buf[..take].copy_from_slice(&bytes[off..off + take]);
            acc = fold_word(acc, u64::from_ne_bytes(buf));
            w += 1;
        }
    }

    acc
}

#[cfg(test)]
mod version_cstr_parity {
    /// Drift guard: the C-string form must equal VERSION + NUL.
    #[test]
    fn c_form_matches_version() {
        let v = avec_arrow::VERSION.as_bytes();
        let c = avec_arrow::VERSION_C;
        assert_eq!(c.len(), v.len() + 1);
        assert_eq!(&c[..v.len()], v);
        assert_eq!(c[v.len()], 0);
    }
}

/// FFI-side digest court (lane HWL6). Canon fold: `avec_arrow::digest`.
/// Chicago discipline: the collaborators are real — the cdylib export path
/// (`avec_i32_batch` -> static slots), the raw heap ArrowArray, and raw
/// pointer writes. No fakes.
#[cfg(test)]
mod digest_ffi {
    use avec_arrow::c_abi::export;
    use avec_arrow::digest::digest_of;
    use avec_arrow::primitive::PrimitiveBuilder;

    const FMT_I: *const core::ffi::c_char = b"i\0".as_ptr() as *const _;

    /// Heap-export `rows` like the static slots do (Box::leak) and return
    /// the ArrowArray address, mirroring foreign-consumer consumption.
    fn export_i32(rows: &[Option<i32>]) -> usize {
        let mut b = PrimitiveBuilder::<i32>::new();
        for v in rows {
            b.push(*v);
        }
        let (array, _schema) = export(b.finish(), FMT_I);
        Box::into_raw(Box::new(array)) as usize
    }

    /// FFI fold must agree with the canon `digest_of` on identically built
    /// batches — this is the drift guard that makes the raw-buffer fold in
    /// `avec_digest_of` lawful. Covers: empty, odd tail lane, single
    /// validity word with nulls, multi-word validity, all-valid (no
    /// validity buffer), and 64-row word boundary.
    #[test]
    fn ffi_fold_parity_matches_digest_of() {
        let cases: &[&[Option<i32>]] = &[
            &[],
            &[Some(7)],
            &[Some(1), None, Some(3), Some(i32::MAX)],
            &[Some(-1); 65],
            &[None; 64],
            &[Some(0); 64],
            &(0..130)
            .map(|i| if i % 3 == 2 { None } else { Some(i) })
            .collect::<Vec<_>>(),
        ];
        for rows in cases {
            let canon = {
                let mut b = PrimitiveBuilder::<i32>::new();
                for v in rows.iter() {
                    b.push(*v);
                }
                digest_of(&b.finish())
            };
            let addr = export_i32(rows);
            let ffi = super::avec_digest_of(addr);
            assert_eq!(ffi, canon, "FFI fold diverged from digest_of");
        }
    }

    /// Corruption court over the real export path: digest -> flip ONE value
    /// byte through a raw pointer -> digest MUST differ -> restore the
    /// captured byte -> digest MUST match again.
    #[test]
    fn corruption_court_value_byte_flip_is_detected_and_restored() {
        // SAFETY: avec_i32_batch is the crate's own export; slots are
        // process-lifetime leaked by design.
        let ptr = unsafe { super::avec_i32_batch(7, 3) };
        assert!(!ptr.is_null());
        let addr = super::avec_array_addr();

        let d1 = super::avec_digest_of(addr);
        let d1_again = super::avec_digest_of(addr);
        assert_eq!(d1, d1_again, "digest must be stable across calls");

        // Row 0 (valid, value 0): byte 0 of the values buffer. Capture,
        // flip, digest, restore.
        // SAFETY: buffers[1] is the live values buffer of our own export.
        let values_ptr = unsafe { *(*ptr).buffers.add(1) } as *mut u8;
        let original = unsafe { *values_ptr };
        unsafe { core::ptr::write_volatile(values_ptr, !original) };

        let d2 = super::avec_digest_of(addr);
        assert_ne!(d2, d1, "single value-byte flip must change the digest");

        unsafe { core::ptr::write_volatile(values_ptr, original) };
        let d3 = super::avec_digest_of(addr);
        assert_eq!(d3, d1, "restored buffer must fold to the original digest");
    }

    /// Digest law across the membrane: empty export folds to 0; a validity
    /// state change with identical value bytes changes the digest (validity
    /// words are load-bearing through the FFI fold too).
    #[test]
    fn empty_folds_to_zero_and_validity_is_load_bearing() {
        let empty = export_i32(&[]);
        assert_eq!(super::avec_digest_of(empty), 0, "empty -> 0");

        // Same value bytes (zeros), differing only in validity state:
        // 64 valid-zero rows vs 63 valid + 1 null-zero row. The null-bearing
        // batch folds one validity word; the all-valid batch folds none.
        let all_valid = export_i32(&[Some(0); 64]);
        let with_null = export_i32(
            &(0..64)
                .map(|i| if i == 63 { None } else { Some(0) })
                .collect::<Vec<_>>(),
        );
        let (dv, dn) = (super::avec_digest_of(all_valid), super::avec_digest_of(with_null));
        assert_ne!(dv, dn, "validity state must be load-bearing in the digest");
    }
}
