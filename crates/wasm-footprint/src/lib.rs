//! wasm-footprint: the no_std cdylib link wrapper for the avec wasm
//! footprint gate (charter CTQ-4).
//!
//! `avec-arrow` and `avec-parquet` are `#![no_std]` + `alloc` libs that
//! deliberately omit `#[panic_handler]` and `#[global_allocator]` — those are
//! consumer-supplied link glue. Building either crate directly as a wasm
//! cdylib therefore fails to link. This crate is that consumer: it supplies
//! the glue and exports a real C ABI surface over three kernels, so every
//! retained byte after `wasm-opt -Oz` is code a real wasm consumer linking
//! these kernels would carry (exports force codegen references; nothing is
//! whole-archive'd in).
//!
//! Glue supplied here:
//! - bump allocator over a static 1 MiB, 64-byte-aligned array (the avec
//!   buffer alignment invariant), alignment honored by construction;
//! - `#[panic_handler]` = halt loop;
//! - `rust_eh_personality` stable symbol stub (`#[no_mangle] pub extern "C"`,
//!   NOT `#[lang]` — the `#[lang]` form is nightly-only).
//!
//! Exports: `wasm_version` — C ABI returning a NUL-terminated version string
//! (single-sourced from this crate's manifest via `CARGO_PKG_VERSION`); plus
//! `wasm_mask_words`, `wasm_unpack_1bit`, `wasm_rle_decode` — thin C ABI
//! wrappers over `avec_arrow::bitmap::mask_words`,
//! `avec_parquet::unpack::unpack_1bit`, `avec_parquet::rle_decode`.
//!
//! Build (standalone manifest; not a root-workspace member):
//! `cd crates/wasm-footprint && cargo build --release --target wasm32-wasip1`

#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

/// Heap capacity: 1 MiB.
const HEAP_SIZE: usize = 1024 * 1024;

/// 64-byte alignment is the avec buffer invariant (AVX-512 / NEON); putting
/// it on the type makes the heap base aligned by construction, not by luck.
#[repr(C, align(64))]
struct Heap {
    _bytes: [u8; HEAP_SIZE],
}

static HEAP: Heap = Heap {
    _bytes: [0; HEAP_SIZE],
};

/// Bump offset within `HEAP`, in bytes.
static BUMP: AtomicUsize = AtomicUsize::new(0);

/// Bump allocator: allocation is an align-up plus a size check against the
/// fixed 1 MiB window; deallocation is a no-op (bump allocators never free —
/// memory is reclaimed only by discarding the whole artifact instance).
///
/// Alignment is honored by construction: the heap base is 64-byte aligned
/// (`#[repr(align(64))]`) and every allocation start is rounded up to
/// `layout.align()` (a power of two, guaranteed by `Layout`'s validity rule),
/// so any layout with align <= 64 — and in fact any power-of-two align that
/// still fits the window — is satisfied.
struct BumpAlloc;

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_impl(layout)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump discipline: never free.
    }
}

impl BumpAlloc {
    fn alloc_impl(&self, layout: Layout) -> *mut u8 {
        let base = HEAP._bytes.as_ptr() as usize;
        let align = layout.align();
        let size = layout.size();
        let mut cur = BUMP.load(Ordering::Relaxed);
        loop {
            // Some(null) on any overflow; wasm32 usize is 32-bit so the
            // checked adds are belt-and-braces, not load-bearing.
            let Some(unaligned) = base.checked_add(cur) else {
                return core::ptr::null_mut();
            };
            let Some(rounded) = unaligned.checked_add(align - 1) else {
                return core::ptr::null_mut();
            };
            let aligned = rounded & !(align - 1);
            let Some(next_off) = aligned
                .checked_add(size)
                .and_then(|a| a.checked_sub(base))
            else {
                return core::ptr::null_mut();
            };
            if next_off > HEAP_SIZE {
                return core::ptr::null_mut();
            }
            match BUMP.compare_exchange_weak(
                cur,
                next_off,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                // Aligned by construction; within the heap window by the
                // `next_off <= HEAP_SIZE` check above.
                Ok(_) => return aligned as *mut u8,
                Err(retry) => cur = retry,
            }
        }
    }
}

#[global_allocator]
static BUMP_ALLOC: BumpAlloc = BumpAlloc;

/// Halt loop: a wasm footprint artifact has no unwinding target and no host
/// diagnostic channel to report to; a stuck CPU is the honest terminal state.
#[panic_handler]
fn footprint_panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Stable-form EH personality stub. `#[no_mangle] pub extern "C"` provides
/// the `rust_eh_personality` symbol the compiler may reference; the
/// `#[lang = "eh_personality"]` form requires nightly, this form does not.
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

/// NUL-terminated artifact version, C-string form so any ABI consumer can
/// read it without a length parameter.
///
/// Single-sourced from this crate's manifest (`CARGO_PKG_VERSION`) at compile
/// time: there is no second hand-written copy of the version here to drift
/// from `Cargo.toml`. The node execution court in `scripts/footprint.sh`
/// asserts the artifact's bytes equal the manifest version (26.10.29 at the
/// time of writing) — the court is what makes that binding visible, not this
/// source line.
static VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

/// C ABI: returns a pointer to the NUL-terminated artifact version string
/// (e.g. `b"26.10.29\0"`) living in the module's read-only data. The pointer
/// is valid for the lifetime of the instance; the string is never mutated.
#[no_mangle]
pub extern "C" fn wasm_version() -> *const u8 {
    VERSION.as_ptr()
}

/// Build a shared `&[u64]` over caller memory. `len == 0` yields the empty
/// slice without dereferencing `ptr` (which may legitimately dangle).
#[inline]
fn u64_slice<'a>(ptr: *const u64, len: usize) -> &'a [u64] {
    if len == 0 {
        &[]
    } else {
        // Safety: caller-contract — `ptr` points to `len` readable u64s.
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }
}

/// Mutable form of [`u64_slice`].
#[inline]
fn u64_slice_mut<'a>(ptr: *mut u64, len: usize) -> &'a mut [u64] {
    if len == 0 {
        &mut []
    } else {
        // Safety: caller-contract — `ptr` points to `len` writable u64s.
        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }
}

/// Shared `&[u8]` over caller memory.
#[inline]
fn u8_slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        // Safety: caller-contract — `ptr` points to `len` readable bytes.
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }
}

/// Mutable `&[u32]` over caller memory.
#[inline]
fn u32_slice_mut<'a>(ptr: *mut u32, len: usize) -> &'a mut [u32] {
    if len == 0 {
        &mut []
    } else {
        // Safety: caller-contract — `ptr` points to `len` writable u32s.
        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }
}

/// C ABI over [`avec_arrow::bitmap::mask_words`]: `out[i] = valid[i] & pred[i]`
/// for `n` lanes (kernel clamps to the shortest input internally).
#[no_mangle]
pub extern "C" fn wasm_mask_words(
    valid: *const u64,
    pred: *const u64,
    out: *mut u64,
    n: usize,
) {
    avec_arrow::bitmap::mask_words(
        u64_slice(valid, n),
        u64_slice(pred, n),
        u64_slice_mut(out, n),
    );
}

/// C ABI over [`avec_parquet::unpack::unpack_1bit`]: expand `src_len` packed
/// u64 words into `out_len` one-bit-per-lane u64 values.
#[no_mangle]
pub extern "C" fn wasm_unpack_1bit(
    src: *const u64,
    src_len: usize,
    out: *mut u64,
    out_len: usize,
) {
    avec_parquet::unpack::unpack_1bit(
        u64_slice(src, src_len),
        u64_slice_mut(out, out_len),
    );
}

/// C ABI over [`avec_parquet::rle_decode`]. Returns 0 on success, 1 on a
/// typed decode error (fail-closed status code; no panics cross the ABI).
#[no_mangle]
pub extern "C" fn wasm_rle_decode(
    src: *const u8,
    src_len: usize,
    out: *mut u32,
    out_len: usize,
) -> i32 {
    match avec_parquet::rle_decode(u8_slice(src, src_len), u32_slice_mut(out, out_len)) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
