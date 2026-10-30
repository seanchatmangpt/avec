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
//! `avec_parquet::unpack::unpack_1bit`, `avec_parquet::rle_decode`; plus
//! `wasm_decode_page_body_w4` — the W=4 constant-parameter page-body decode
//! (record loop mirrored from `avec_parquet::rle::decode_into` with the
//! width dispatch removed; differential-courted against the real decoder in
//! `w4_courts` and from the JS side via the minted hex fixture in
//! `scripts/wasm_bench.mjs`).
//!
//! Build (standalone manifest; not a root-workspace member):
//! `cd crates/wasm-footprint && cargo build --release --target wasm32-wasip1`

#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

use avec_parquet::ParquetError;

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

// `cfg(not(test))`: the contract-mandated host unit tests (module
// `w4_courts` below) link the std test harness, which supplies its own panic
// handler and global allocator — this crate's would be a duplicate lang item
// (E0152). The wasm cdylib build (test cfg off) carries both, unchanged.
#[cfg(not(test))]
#[global_allocator]
static BUMP_ALLOC: BumpAlloc = BumpAlloc;

/// Halt loop: a wasm footprint artifact has no unwinding target and no host
/// diagnostic channel to report to; a stuck CPU is the honest terminal state.
#[cfg(not(test))]
#[panic_handler]
fn footprint_panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Stable-form EH personality stub. `#[no_mangle] pub extern "C"` provides
/// the `rust_eh_personality` symbol the compiler may reference; the
/// `#[lang = "eh_personality"]` form requires nightly, this form does not.
/// (`cfg(not(test))`: std's test harness exports the same symbol, so the
/// host test link would warn on a duplicate.)
#[cfg(not(test))]
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

/// W=4 constant-parameter specialization: mask and run-value width are
/// compile-time constants derived from the width itself, so the general
/// decoder's width dispatch is gone — the only width work left is one
/// `== 4` admission compare on the body's leading width byte.
const W4_BITS: u64 = 4;
const W4_MASK: u64 = (1u64 << W4_BITS) - 1;
const W4_VALUE_BYTES: usize = (W4_BITS as usize).div_ceil(8);

/// Decode an RLE/bit-packed hybrid page BODY whose bit width is the
/// constant `W4_BITS` (4), writing `out_len` u32 lanes.
///
/// Wire form is exactly [`avec_parquet::rle::decode_into`]'s: a leading
/// bit-width byte, then records —
/// - repeated run: varint header `(count << 1) | 0`, one LE value of
///   `ceil(w/8)` bytes (1 byte at w=4),
/// - literal run: varint header `(groups << 1) | 1`, then `groups` u64
///   words LE, each word 8 lanes of `w` bits LSB-first (at w=4 one lane
///   occupies one whole byte), zero-padded final group.
///
/// This is the [`avec_parquet::rle::decode_into`] record loop replicated
/// with the width dispatch removed: `W4_MASK`/`W4_VALUE_BYTES` fold to
/// constants, and the arm bodies mirror the general decoder's math exactly
/// (differential court `w4_courts::differential_vs_decode_into` proves the
/// two agree lane-exact on encoder-produced bodies; the JS court in
/// `scripts/wasm_bench.mjs` carries a fixture generated by that court and
/// re-proves the equality through the compiled wasm artifact).
///
/// Contract mirrors [`avec_parquet::rle::decode_into`]: `Ok(n)` is the lane
/// count written (the page may end at a record boundary before `out`
/// fills); typed error on malformed input, never a panic.
fn decode_page_body_w4(src: &[u8], out: &mut [u32]) -> Result<usize, ParquetError> {
    let Some(&bits) = src.first() else {
        return Err(ParquetError::UnexpectedEof);
    };
    // Constant-parameter admission: anything that is not exactly 4 is a
    // typed refusal, never a fallback to the general dispatch.
    if bits != W4_BITS as u8 {
        return Err(ParquetError::UnsupportedBitWidth(bits));
    }
    let mut pos = 1usize; // past the bit-width byte
    let mut oi = 0usize;
    while oi < out.len() {
        if pos == src.len() {
            // Page exhausted at a record boundary: streaming contract.
            return Ok(oi);
        }
        let header = read_varint(src, &mut pos)?;
        if header & 1 == 0 {
            // repeated run
            let run_len = (header >> 1) as usize;
            if run_len > out.len() - oi {
                return Err(ParquetError::RunOverrun { run_len, budget: out.len() - oi });
            }
            let vb = W4_VALUE_BYTES; // const 1
            let Some(&v) = src.get(pos..pos + vb).and_then(|s| s.first()) else {
                return Err(ParquetError::UnexpectedEof);
            };
            // The general decoder widens the vb LE bytes via
            // u32::from_le_bytes and fills unmasked — at w=4 that is the raw
            // byte widened, so the fill value is unmasked here too (exact).
            let value = u32::from(v);
            pos += vb;
            out[oi..oi + run_len].fill(value);
            oi += run_len;
        } else {
            // literal run: groups of 8 byte-per-lane words; same fail-closed
            // arithmetic as the general decoder (giant group counts and
            // wrapping `pos + byte_len` are typed refusals, never overflow
            // panics).
            let groups = (header >> 1) as usize;
            let available = src.len() - pos;
            let Some(byte_len) = groups.checked_mul(8) else {
                return Err(ParquetError::LiteralOverrun { claimed: usize::MAX, available });
            };
            let count = byte_len.min(out.len() - oi);
            let Some(end) = pos.checked_add(byte_len) else {
                return Err(ParquetError::LiteralOverrun { claimed: byte_len, available });
            };
            let Some(packed) = src.get(pos..end) else {
                return Err(ParquetError::LiteralOverrun { claimed: byte_len, available });
            };
            let mut k = 0;
            while k < count {
                // In-bounds: `packed.len() = byte_len >= 8` for any
                // non-empty literal record (`groups >= 1`), `k & !7` is a
                // multiple of 8 <= k, so `(k & !7) + 8 <= byte_len` — the
                // same proof that makes the general decoder's
                // `try_into().unwrap()` unreachable.
                let word = u64::from_le_bytes(
                    packed[(k & !7)..(k & !7) + 8].try_into().unwrap(),
                );
                // w=4: lane k sits in byte k%8 of the LE word, so the
                // shift/mask pair folds to `byte & 0x0F` at codegen.
                out[oi + k] = ((word >> ((k % 8) * 8)) & W4_MASK) as u32;
                k += 1;
            }
            oi += count;
            pos += byte_len;
        }
    }
    Ok(oi)
}

/// Bounded varint read — byte-for-byte mirror of the crate's private
/// `read_varint` (crates/avec-parquet/src/rle.rs), which is not `pub` and so
/// cannot be called from this wrapper crate.
fn read_varint(src: &[u8], pos: &mut usize) -> Result<u64, ParquetError> {
    let mut value: u64 = 0;
    let mut shift = 0;
    loop {
        let Some(&b) = src.get(*pos) else {
            return Err(ParquetError::UnexpectedEof);
        };
        *pos += 1;
        value |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(ParquetError::UnexpectedEof);
        }
    }
}

/// C ABI over [`decode_page_body_w4`]: the W=4 constant-parameter page-body
/// decode, exported for the wasm sandbox court (`scripts/wasm_bench.mjs`).
/// Full-buffer like [`wasm_rle_decode`]: 0 = the page exactly filled
/// `out_len` lanes; 1 = typed decode error (including a body whose width
/// byte is not 4, or a partial page). Fail-closed; no panics cross the ABI.
#[no_mangle]
pub extern "C" fn wasm_decode_page_body_w4(
    src: *const u8,
    src_len: usize,
    out: *mut u32,
    out_len: usize,
) -> i32 {
    match decode_page_body_w4(u8_slice(src, src_len), u32_slice_mut(out, out_len)) {
        Ok(n) if n == out_len => 0,
        _ => 1,
    }
}

/// W=4 differential courts: the hand-specialized record loop must agree
/// lane-exact with the real [`avec_parquet::rle::decode_into`] on
/// encoder-produced bodies, and refuse every other width. Also mints the
/// hex fixture the JS court in `scripts/wasm_bench.mjs` embeds (provenance
/// comment there names this test).
#[cfg(test)]
mod w4_courts {
    extern crate std;

    use super::decode_page_body_w4;
    use std::{format, println, string::String, vec};
    use avec_parquet::rle::decode_into;
    use avec_parquet::rle_encode;
    use std::vec::Vec;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Per-position distinct 4-bit pattern: no 3-run, so rle_encode emits
    /// literal batches (plus group-boundary absorption).
    fn literal_pattern(lanes: usize) -> Vec<u32> {
        (0..lanes).map(|i| ((i * 7 + 1) & 0xF) as u32).collect()
    }

    /// Mixed body: literal stretch, then a long repeated run, then a short
    /// literal tail that is not a multiple of 8 (zero-padded partial final
    /// group — the only partial form the wire admits).
    fn mixed_body_values() -> Vec<u32> {
        let mut values = literal_pattern(40);
        values.extend(std::iter::repeat_n(7u32, 20));
        values.extend_from_slice(&[1, 2, 4, 8]);
        values
    }

    #[test]
    fn differential_vs_decode_into() {
        // (name, values) cases: pure literal, run-heavy, mixed with partial
        // final group, and an exact multiple of the 8-lane group.
        let cases: Vec<(&str, Vec<u32>)> = vec![
            ("literal-257", literal_pattern(257)),
            (
                "run-heavy",
                std::iter::repeat_n(9u32, 100).collect(),
            ),
            ("mixed-partial-final-group", mixed_body_values()),
            ("exact-groups-64", literal_pattern(64)),
        ];
        for (name, values) in &cases {
            let body = rle_encode(values, 4);
            let mut want = vec![0xEEu32; values.len()];
            let n = decode_into(&body, &mut want).expect("decode_into on encoder output");
            assert_eq!(n, values.len(), "{name}: page must exactly fill");
            let mut got = vec![0x5Au32; values.len()];
            let m = decode_page_body_w4(&body, &mut got).expect("w4 decode");
            assert_eq!(m, values.len(), "{name}: lane count");
            assert_eq!(
                got, want,
                "{name}: specialized w=4 loop diverged from decode_into"
            );
        }
    }

    #[test]
    fn w4_refuses_every_other_width() {
        let values: Vec<u32> = (0..16u32).collect();
        for bits in [1u8, 2, 8] {
            let body = rle_encode(&values, bits);
            let mut out = vec![0xEEu32; values.len()];
            let err = decode_page_body_w4(&body, &mut out)
                .expect_err("non-4 width must be refused");
            assert_eq!(
                err,
                avec_parquet::ParquetError::UnsupportedBitWidth(bits),
                "width {bits}"
            );
            // Fail-closed: no lane written on refusal.
            assert!(out.iter().all(|&l| l == 0xEE), "width {bits}: lanes untouched");
        }
        // Empty body: typed EOF, not a panic.
        assert_eq!(
            decode_page_body_w4(&[], &mut []),
            Err(avec_parquet::ParquetError::UnexpectedEof)
        );
    }

    #[test]
    fn w4_mints_js_fixture() {
        // Fixture mint: body from the real encoder, lanes from the real
        // decoder (decode_into). Pasted into scripts/wasm_bench.mjs as
        // RLE4_FIXTURE_BODY_HEX / RLE4_FIXTURE_LANES_HEX; regenerate with
        // `cargo test w4_mints_js_fixture -- --nocapture` and re-paste.
        let values = mixed_body_values();
        let body = rle_encode(&values, 4);
        let mut want = vec![0xEEu32; values.len()];
        let n = decode_into(&body, &mut want).expect("decode_into");
        assert_eq!(n, values.len());
        println!("W4_FIXTURE_LANES={} BODY_HEX={} LANES_HEX={}", values.len(), hex(&body), {
            let mut lanes_hex = String::new();
            for lane in &want {
                lanes_hex.push_str(&format!("{:08x}", lane));
            }
            lanes_hex
        });
    }
}
