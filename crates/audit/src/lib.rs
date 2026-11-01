//! cgap-audit: forces every audited kernel into a cdylib artifact.
//!
//! Each export wraps one kernel; the linker cannot elide what is exported,
//! so the branch audit inspects the REAL compiled kernel machine code.

pub mod alloc_counter;

use cgap_arrow::bitmap;
use cgap_arrow::silicon;
use cgap_arrow::views::is_inline;
use cgap_parquet::unpack;

/// # Safety
/// n must bound the readable words at each pointer.
#[no_mangle]
pub unsafe extern "C" fn audit_mask_words(
    valid: *const u64,
    pred: *const u64,
    out: *mut u64,
    n: usize,
) {
    let (v, p, o) = unsafe {
        (
            core::slice::from_raw_parts(valid, n),
            core::slice::from_raw_parts(pred, n),
            core::slice::from_raw_parts_mut(out, n),
        )
    };
    bitmap::mask_words(v, p, o);
}

/// NEON silicon kernel export: `out[i] = valid[i] & pred[i]` through
/// `cgap_arrow::silicon::mask_words_neon`. On aarch64 this dispatches to the
/// 4-word-stride NEON kernel (branch-free inner body; loop control and the
/// scalar tail carry the extent's conditionals — see the disclosure on
/// `mask_words_neon` in crates/cgap-arrow/src/silicon.rs, audited by the
/// extent-scoped rule in scripts/branch_audit.py). On other targets the same
/// export dispatches to the portable SWAR kernel with the identical output
/// contract, so the audit surface exists on every host.
///
/// # Safety
/// n must bound the readable words at each pointer.
#[no_mangle]
pub unsafe extern "C" fn audit_mask_words_neon(
    valid: *const u64,
    pred: *const u64,
    out: *mut u64,
    n: usize,
) {
    let (v, p, o) = unsafe {
        (
            core::slice::from_raw_parts(valid, n),
            core::slice::from_raw_parts(pred, n),
            core::slice::from_raw_parts_mut(out, n),
        )
    };
    silicon::mask_words_neon(v, p, o);
}

/// # Safety
/// n must bound the readable words at each pointer.
#[no_mangle]
pub unsafe extern "C" fn audit_count_valid(valid: *const u64, pred: *const u64, n: usize) -> u64 {
    unsafe {
        bitmap::count_valid(
            core::slice::from_raw_parts(valid, n),
            core::slice::from_raw_parts(pred, n),
        )
    }
}

/// # Safety
/// i/8+1 must bound the readable bytes.
#[no_mangle]
pub unsafe extern "C" fn audit_get_bit(bits: *const u8, i: usize) -> u8 {
    unsafe { bitmap::get_bit(core::slice::from_raw_parts(bits, i / 8 + 1), i) }
}

/// # Safety
/// i/8+1 must bound the writable bytes.
#[no_mangle]
pub unsafe extern "C" fn audit_set_bit(bits: *mut u8, i: usize) {
    unsafe { bitmap::set_bit(core::slice::from_raw_parts_mut(bits, i / 8 + 1), i) }
}

/// # Safety
/// n must bound buffers.
#[no_mangle]
pub unsafe extern "C" fn audit_unpack_1bit(src: *const u64, out: *mut u64, n: usize) {
    unsafe {
        unpack::unpack_1bit(
            core::slice::from_raw_parts(src, n / 64 + 1),
            core::slice::from_raw_parts_mut(out, n),
        )
    }
}

/// # Safety
/// n must bound buffers.
#[no_mangle]
pub unsafe extern "C" fn audit_unpack_2bit(src: *const u64, out: *mut u64, n: usize) {
    unsafe {
        unpack::unpack_2bit(
            core::slice::from_raw_parts(src, n / 32 + 1),
            core::slice::from_raw_parts_mut(out, n),
        )
    }
}

/// # Safety
/// n must bound buffers.
#[no_mangle]
pub unsafe extern "C" fn audit_unpack_4bit(src: *const u64, out: *mut u64, n: usize) {
    unsafe {
        unpack::unpack_4bit(
            core::slice::from_raw_parts(src, n / 16 + 1),
            core::slice::from_raw_parts_mut(out, n),
        )
    }
}

/// # Safety
/// n must bound buffers.
#[no_mangle]
pub unsafe extern "C" fn audit_unpack_8bit(src: *const u64, out: *mut u64, n: usize) {
    unsafe {
        unpack::unpack_8bit(
            core::slice::from_raw_parts(src, n / 8 + 1),
            core::slice::from_raw_parts_mut(out, n),
        )
    }
}

/// # Safety
/// Reads the u32 descriptor length at `views`.
#[no_mangle]
pub unsafe extern "C" fn audit_is_inline(len: u32) -> u64 {
    is_inline(len)
}

/// Whole-bitmap validity scan kernel export (`Bitmap::is_all_valid`).
///
/// The pointer is opaque: the audit cdylib never constructs a `Bitmap`, it
/// only forces the kernel into the artifact so scripts/branch_audit.py can
/// inspect the REAL compiled machine code. (Audit-vacuity receipt 2026-10-29:
/// this kernel previously had no audit-crate caller and was GC'd out of the
/// audit dylib entirely — unaudited and unnamed. The wrapper closes that.)
///
/// # Safety
/// bm must point to a valid Bitmap.
#[no_mangle]
pub unsafe extern "C" fn audit_is_all_valid(bm: *const bitmap::Bitmap) -> u64 {
    unsafe { u64::from((*bm).is_all_valid()) }
}

/// Popcount kernel export (`bitmap::count_one_bits`). Opaque-pointer export
/// for the same reason as `audit_is_all_valid` — see that receipt.
///
/// # Safety
/// bm must point to a valid Bitmap.
#[no_mangle]
pub unsafe extern "C" fn audit_count_one_bits(bm: *const bitmap::Bitmap) -> u64 {
    unsafe { bitmap::count_one_bits(&*bm) }
}
