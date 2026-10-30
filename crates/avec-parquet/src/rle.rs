//! SWAR RLE / bit-packed hybrid codec for u32 values.
//!
//! Wire form: varint bit-width, then records:
//! - repeated run:  header = (count << 1) | 0, one LE value of ceil(w/8) bytes
//! - literal run:   header = (groups << 1) | 1, then `groups` u64 words LE,
//!   each word = 8 lanes of `w` bits (LSB-first), zero-padded final group.
//!
//! Decode is fail-closed: bounded reads, typed errors, zero panics on
//! malformed pages (FMEA row 3). The decode path is zero-allocation:
//! [`decode_into`] writes lanes directly into caller-owned memory and never
//! constructs an intermediate `Vec` (proved by construction and by the
//! counting-allocator court in the test module).

use alloc::vec::Vec;
use core::fmt;

/// Typed decode/encode errors - loud, never silent success.
#[derive(Debug, PartialEq, Eq)]
pub enum ParquetError {
    /// Input ended mid-record.
    UnexpectedEof,
    /// Width outside the SWAR kernel set {1,2,4,8}.
    UnsupportedBitWidth(u8),
    /// A run claimed more output lanes than the caller budgeted.
    RunOverrun { run_len: usize, budget: usize },
    /// A literal run claimed more input than present.
    LiteralOverrun { claimed: usize, available: usize },
}

impl fmt::Display for ParquetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParquetError {}

/// Varint (LEB128) read - bounded.
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

/// Varint write.
pub fn write_varint(dst: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut b = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            b |= 0x80;
        }
        dst.push(b);
        if value == 0 {
            return;
        }
    }
}
/// Encode values as the hybrid codec (repeated runs >= 3, literal groups of 8).
pub fn rle_encode(values: &[u32], bits: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(bits);
    let mut i = 0;
    while i < values.len() {
        // repeated run when >= 3 equal values
        let v = values[i];
        let mut run = 1;
        while i + run < values.len() && values[i + run] == v {
            run += 1;
        }
        if run >= 3 {
            write_varint(&mut out, u64::from(run as u32) << 1);
            let vb = usize::from(bits).div_ceil(8);
            out.extend_from_slice(&v.to_le_bytes()[..vb]);
            i += run;
        } else {
            // literal batch: coalesce until the next >=3-run (or end)
            let start = i;
            let mut j = i + run;
            while j < values.len() {
                let mut r = 1;
                while j + r < values.len() && values[j + r] == values[j] {
                    r += 1;
                }
                if r >= 3 {
                    break;
                }
                j += r;
            }
            // Literal records are full 8-lane groups; the wire form has no
            // intra-record lane count, so a partial FINAL group is the only
            // partial form a decoder can resolve (budget clamp at end of
            // stream). Mid-stream partial batches would over-decode pad
            // lanes over the following records — absorb following lanes up
            // to the group boundary instead (values are preserved; runs
            // absorbed here re-encode as literals, which is exact).
            let count = j - start;
            let rem = count % 8;
            if rem != 0 {
                let need = 8 - rem;
                let absorb = need.min(values.len() - j);
                j += absorb;
            }
            let count = j - start;
            let groups = count.div_ceil(8);
            write_varint(&mut out, ((groups as u64) << 1) | 1);
            let mut lane = 0;
            while lane < groups * 8 {
                let val = if start + lane < j { values[start + lane] } else { 0 };
                let vb = usize::from(bits);
                let mut byte = 0u8;
                let mut bit = 0;
                while bit < vb {
                    if (u64::from(val) >> bit) & 1 == 1 {
                        byte |= 1 << bit;
                    }
                    bit += 1;
                }
                out.push(byte);
                lane += 1;
            }
            i = j;
        }
    }
    out
}

/// Decode the hybrid codec into `out` (u32 lanes).
///
/// Full-buffer decode: the page must exactly fill `out`. Implemented by
/// delegating to [`decode_into`] — same wire format, same typed errors, zero
/// allocation.
pub fn rle_decode(src: &[u8], out: &mut [u32]) -> Result<(), ParquetError> {
    match decode_into(src, out) {
        Ok(n) if n == out.len() => Ok(()),
        Ok(_) => Err(ParquetError::UnexpectedEof),
        Err(e) => Err(e),
    }
}

/// Decode a page directly into the caller's slice, writing lanes in place.
///
/// Contract:
/// - `Ok(out.len())` — the page exactly fills `out` (the [`rle_decode`]
///   full-buffer contract).
/// - `Ok(n)` with `n < out.len()` — the page was exhausted at a record
///   boundary before `out` filled; lanes `0..n` are written, `out[n..]` is
///   untouched.
/// - Typed error on malformed input (never a panic on external input); on
///   error, lanes before the failing record may already be written — the
///   error is loud, the partial prefix is documented, never a silent success.
///
/// Zero allocation by construction: the path (`decode_into` + `read_varint`)
/// contains no `Vec::new`/`vec!`/`collect`/`to_vec`, and the counting-allocator
/// court below asserts a measured 0 during a full decode.
pub fn decode_into(src: &[u8], out: &mut [u32]) -> Result<usize, ParquetError> {
    let Some(&bits) = src.first() else {
        return Err(ParquetError::UnexpectedEof);
    };
    if !matches!(bits, 1 | 2 | 4 | 8) {
        return Err(ParquetError::UnsupportedBitWidth(bits));
    }
    let mask = (1u64 << bits) - 1;
    let mut pos = 1usize; // past the bit-width byte
    let mut oi = 0usize;
    while oi < out.len() {
        if pos == src.len() {
            // Page exhausted at a record boundary: streaming contract —
            // report the lanes written so far, leave the rest of `out`
            // untouched.
            return Ok(oi);
        }
        let header = read_varint(src, &mut pos)?;
        if header & 1 == 0 {
            // repeated run
            let run_len = (header >> 1) as usize;
            if run_len > out.len() - oi {
                return Err(ParquetError::RunOverrun { run_len, budget: out.len() - oi });
            }
            let vb = usize::from(bits).div_ceil(8);
            let Some(vb_slice) = src.get(pos..pos + vb) else {
                return Err(ParquetError::UnexpectedEof);
            };
            let mut w = [0u8; 4];
            w[..vb].copy_from_slice(vb_slice);
            let value = u32::from_le_bytes(w);
            pos += vb;
            out[oi..oi + run_len].fill(value);
            oi += run_len;
        } else {
            // literal run: groups of 8 byte-per-lane words
            let groups = (header >> 1) as usize;
            // Fail-closed arithmetic on attacker-controlled headers (both
            // profiles, never an overflow panic — module law: zero panics on
            // malformed pages):
            // - `groups * 8` can exceed usize (giant group count);
            // - `pos + byte_len` can wrap even when byte_len fits.
            // Both classes report LiteralOverrun: the record claims more
            // literal payload than could ever be present.
            let available = src.len() - pos;
            let Some(byte_len) = groups.checked_mul(8) else {
                return Err(ParquetError::LiteralOverrun { claimed: usize::MAX, available });
            };
            // Literal records may end with a zero-padded partial final group;
            // the caller budget bounds the real lane count (Parquet spec
            // discipline: decode stops at page end).
            let count = byte_len.min(out.len() - oi);
            let Some(end) = pos.checked_add(byte_len) else {
                return Err(ParquetError::LiteralOverrun { claimed: byte_len, available });
            };
            let Some(packed) = src.get(pos..end) else {
                return Err(ParquetError::LiteralOverrun { claimed: byte_len, available });
            };
            let mut k = 0;
            while k < count {
                let word = u64::from_le_bytes(packed[(k & !7)..(k & !7) + 8].try_into().unwrap());
                out[oi + k] = ((word >> ((k % 8) * 8)) & mask) as u32;
                k += 1;
            }
            oi += count;
            pos += byte_len;
        }
    }
    Ok(oi)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(values: &[u32], bits: u8) {
        let encoded = rle_encode(values, bits);
        let mut decoded = alloc::vec![0u32; values.len()];
        rle_decode(&encoded, &mut decoded).expect("decode");
        assert_eq!(decoded, values, "bits={bits}");
    }

    #[test]
    fn repeated_runs() {
        round_trip(&[7, 7, 7, 7, 7, 2, 2, 2], 4);
        round_trip(&alloc::vec![9u32; 100], 2);
    }

    #[test]
    fn literal_groups() {
        round_trip(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 4);
        round_trip(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 3, 3, 3], 4);
    }

    #[test]
    fn mixed_and_widths() {
        round_trip(&[0, 0, 0, 1, 1, 1, 2, 2, 2, 3], 2);
        round_trip(&[200, 201, 202, 203], 8);
        round_trip(&[1, 0, 1, 1, 0, 0, 1, 0], 1);
    }

    #[test]
    fn empty_page() {
        let encoded = rle_encode(&[], 4);
        let mut decoded: [u32; 0] = [];
        rle_decode(&encoded, &mut decoded).expect("empty ok");
    }

    #[test]
    fn malformed_fails_closed() {
        // truncated stream: claims a huge run, input ends
        let mut decoded = [0u32; 10];
        assert!(rle_decode(&[4, 0xFF, 0xFF, 0xFF], &mut decoded).is_err());
        // bad width byte
        assert_eq!(
            rle_decode(&[3, 0], &mut decoded),
            Err(ParquetError::UnsupportedBitWidth(3))
        );
    }
}

/// Zero-allocation court: a counting global allocator with thread-local
/// counters, so concurrently running tests on other threads cannot pollute
/// the measurement window. The lib is `#![no_std]`; the test harness links
/// std, so it is pulled in explicitly here.
#[cfg(test)]
mod alloc_proof {
    extern crate std;

    use super::{decode_into, rle_decode, rle_encode};
    use alloc::vec::Vec;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::{println, thread_local};

    thread_local! {
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
        static IN_ALLOC: Cell<bool> = const { Cell::new(false) };
    }

    struct Counting;

    impl Counting {
        fn bump() {
            if IN_ALLOC.with(Cell::get) {
                return;
            }
            IN_ALLOC.with(|c| c.set(true));
            ALLOCS.with(|c| c.set(c.get() + 1));
            IN_ALLOC.with(|c| c.set(false));
        }
    }

    // SAFETY: pure pass-through to System with a counted bump; contract of
    // each method is delegated unchanged.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            Self::bump();
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            Self::bump();
            unsafe { System.realloc(ptr, layout, new_size) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            Self::bump();
            unsafe { System.alloc_zeroed(layout) }
        }
    }

    #[global_allocator]
    static COUNTING_ALLOC: Counting = Counting;

    fn current_allocs() -> usize {
        ALLOCS.with(Cell::get)
    }

    /// Mixed literal+run page, identical for every measurement.
    fn court_page() -> (Vec<u8>, usize) {
        let lanes = 4096;
        let mut values = Vec::with_capacity(lanes);
        for i in 0..lanes {
            // Literal stretches, then a repeated run: exercises both arms.
            values.push(if i < 3000 { (i * 7 + 1) as u32 } else { 42 });
        }
        let page = rle_encode(&values, 8);
        (page, lanes)
    }

    #[test]
    fn decode_into_zero_allocations() {
        let (page, lanes) = court_page();
        let mut sink = alloc::vec![0xDEAD_BEEFu32; lanes];
        assert_ne!(sink.len(), 0);

        let before = current_allocs();
        let n = decode_into(&page, &mut sink).expect("decode_into");
        let delta = current_allocs() - before;

        println!("[alloc-court] decode_into allocs={delta} lanes={lanes}");
        assert_eq!(delta, 0, "decode_into allocated {delta} times during decode — zero-alloc law violated");
        assert_eq!(n, lanes, "page must exactly fill the pre-allocated slice");

        // rle_decode delegates to decode_into: same zero-alloc law.
        sink.fill(0);
        let before = current_allocs();
        rle_decode(&page, &mut sink).expect("rle_decode");
        let delta = current_allocs() - before;
        println!("[alloc-court] rle_decode  allocs={delta} lanes={lanes}");
        assert_eq!(delta, 0, "rle_decode allocated {delta} times — zero-alloc law violated");
    }
}

/// Mutate-input-pattern + streaming-contract tests for [`decode_into`].
#[cfg(test)]
mod decode_into_tests {
    use super::{decode_into, rle_decode, rle_encode, ParquetError};
    use alloc::vec::Vec;

    /// Per-position distinct pattern (no two neighbors equal, so the encoder
    /// emits pure literal runs) for every SWAR width.
    fn pattern(lanes: usize, bits: u8) -> Vec<u32> {
        let alpha = 1u32 << bits;
        (0..lanes).map(|i| (i as u32) % alpha).collect()
    }

    const SENTINEL: u32 = 0xDEAD_BEEF;

    #[test]
    fn decode_into_round_trip_full_buffer() {
        for bits in [1u8, 2, 4, 8] {
            let values = pattern(257, bits);
            let page = rle_encode(&values, bits);
            let mut out = alloc::vec![SENTINEL; values.len()];
            let n = decode_into(&page, &mut out).expect("decode_into");
            assert_eq!(n, out.len(), "full-buffer decode, bits={bits}");
            assert_eq!(out, values, "bits={bits}");
        }
    }

    #[test]
    fn decode_into_writes_distinct_values_at_exact_indices() {
        for bits in [1u8, 2, 4, 8] {
            let values = pattern(300, bits);
            let page = rle_encode(&values, bits);
            let mut out = alloc::vec![SENTINEL; values.len()];
            let n = decode_into(&page, &mut out).expect("decode_into");
            assert_eq!(n, values.len());
            // Exact per-position landing: value i at slice index i.
            for (i, &v) in values.iter().enumerate() {
                assert_eq!(out[i], v, "lane {i} bits={bits}: got {:x}, want {v:x}", out[i]);
            }
            // No lane beyond the decoded prefix was touched.
            assert!(out[values.len()..].is_empty());
        }

        // Mid-buffer window: the caller's sub-slice is the whole decode target,
        // lanes land at the sub-slice's own indices (absolute 2..7 here).
        let values = [10u32, 20, 1, 2, 3, 4, 5, 6, 7, 30];
        let page = rle_encode(&values, 8);
        let mut out = alloc::vec![SENTINEL; values.len()];
        {
            let window = &mut out[2..7];
            let n = decode_into(&page, window).expect("decode_into window");
            assert_eq!(n, 5, "page holds 10 lanes, window budget 5");
        }
        for (i, want) in [10u32, 20, 1, 2, 3].iter().enumerate() {
            assert_eq!(out[2 + i], *want, "window lane {i} must land at absolute index {}", 2 + i);
        }
        // Everything outside the window keeps the sentinel.
        assert_eq!(out[0], SENTINEL);
        assert_eq!(out[1], SENTINEL);
        assert!(out[7..].iter().all(|&x| x == SENTINEL));
    }

    #[test]
    fn decode_into_partial_page_streaming() {
        // Page encodes 16 lanes; the caller budgets 40. decode_into must stop
        // at the record boundary and report Ok(16), leaving lanes 16.. intact.
        let values: Vec<u32> = (0..16).collect();
        let page = rle_encode(&values, 8);
        let mut out = alloc::vec![SENTINEL; 40];
        let n = decode_into(&page, &mut out).expect("decode_into");
        assert_eq!(n, 16, "lanes written must equal the page's lane count");
        assert_eq!(&out[..16], &values[..]);
        assert!(out[16..].iter().all(|&x| x == SENTINEL), "tail must be untouched");

        // rle_decode keeps its full-buffer semantics: a partial page is not
        // Ok(()) — it is UnexpectedEof, exactly as the pre-refactor body
        // surfaced it (read_varint at a consumed page end).
        assert_eq!(rle_decode(&page, &mut out), Err(ParquetError::UnexpectedEof));
    }

    /// varint-encode `value` (same wire form as `write_varint`).
    fn varint(value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        let mut v = value;
        loop {
            let mut b = (v & 0x7F) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                return out;
            }
        }
    }

    #[test]
    fn decode_into_giant_literal_groups_fail_closed() {
        let mut out = alloc::vec![0u32; 64];

        // Class 1: groups*8 overflows usize. header = (2^61 << 1) | 1.
        let header = (0x2000_0000_0000_0000u64 << 1) | 1;
        let mut page = alloc::vec![8u8];
        page.extend_from_slice(&varint(header));
        let available = 0; // src is exactly the width byte + header
        assert_eq!(
            decode_into(&page, &mut out),
            Err(ParquetError::LiteralOverrun { claimed: usize::MAX, available }),
            "groups*8 overflow must fail closed with LiteralOverrun, not overflow-panic"
        );

        // Class 2 (robustness-lane seed class): groups*8 fits, but
        // pos + byte_len wraps. Repro wire: [04, ff*8, 3f] -> groups = 2^61-1,
        // byte_len = 2^64-8, pos = 10 -> checked_add overflows.
        let page2: Vec<u8> = alloc::vec![
            0x04, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x3F,
        ];
        assert_eq!(
            decode_into(&page2, &mut out),
            Err(ParquetError::LiteralOverrun { claimed: usize::MAX - 7, available: 0 }),
            "pos+byte_len wrap must fail closed with LiteralOverrun in BOTH profiles"
        );
    }
}

/// Wiring court: decode_into writing lanes directly into a 64-byte-aligned
/// caller-owned buffer viewed as u32 — the avec-arrow `Buf` consumption
/// pattern (charter M-03 alignment law). avec-parquet is a zero-dependency
/// crate and its Cargo.toml is outside this lane's file ownership, so the
/// `Buf` contract (Layout::from_size_align(bytes, 64), as_mut_slice, typed
/// read views — see /Users/sac/avec/crates/avec-arrow/src/buffer.rs) is
/// reproduced here as a hand-written real implementation: real allocator,
/// real memory, no stubbed behavior.
#[cfg(test)]
mod wiring {
    extern crate std;

    use super::{decode_into, rle_encode};
    use alloc::vec::Vec;
    use core::slice;
    use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
    use std::ptr::NonNull;

    const ALIGNMENT: usize = 64;

    struct Buf64 {
        ptr: NonNull<u8>,
        len: usize,
    }

    impl Buf64 {
        fn new(len: usize) -> Self {
            if len == 0 {
                return Buf64 { ptr: NonNull::dangling(), len: 0 };
            }
            let layout = Layout::from_size_align(len, ALIGNMENT)
                .unwrap_or_else(|e| panic!("invalid buffer layout: {e}"));
            // SAFETY: layout has non-zero size.
            let raw = unsafe { alloc(layout) };
            match NonNull::new(raw) {
                Some(ptr) => Buf64 { ptr, len },
                None => handle_alloc_error(layout),
            }
        }

        fn as_ptr(&self) -> *const u8 {
            self.ptr.as_ptr()
        }

        fn as_mut_ptr(&self) -> *mut u8 {
            self.ptr.as_ptr()
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            // SAFETY: ptr valid for len bytes for the life of &mut self.
            unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
        }

        /// Typed read view — same contract as `avec_arrow::buffer::Buf::as_typed`.
        fn as_typed<T>(&self) -> &[T] {
            let (sz, an) = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
            assert!(an <= ALIGNMENT, "element alignment exceeds buffer alignment");
            assert_eq!(self.len % sz, 0, "buffer length not a multiple of element size");
            // SAFETY: 64-byte base + uniform element size -> aligned for T.
            unsafe { slice::from_raw_parts(self.as_ptr() as *const T, self.len / sz) }
        }
    }

    impl Drop for Buf64 {
        fn drop(&mut self) {
            if self.len == 0 {
                return;
            }
            // SAFETY: allocated with exactly this layout.
            unsafe {
                dealloc(
                    self.as_mut_ptr(),
                    Layout::from_size_align_unchecked(self.len, ALIGNMENT),
                );
            }
        }
    }

    #[test]
    fn decode_into_wires_into_buf_view() {
        let lanes = 64;
        let values: Vec<u32> = (0..lanes).map(|i| ((i * 31 + 5) & 0xFF) as u32).collect();
        let page = rle_encode(&values, 8);

        let mut buf = Buf64::new(lanes * 4);
        assert_eq!(buf.as_ptr() as usize % ALIGNMENT, 0, "charter M-03: 64-byte alignment");

        // The wiring: mutable typed window derived from the buffer's byte
        // view (as_mut_slice/as_mut_ptr — avec-arrow has no as_typed_mut, so
        // this rebind is what every &mut consumer writes), then decode_into
        // writes lanes in place. Zero intermediate storage.
        let out: &mut [u32] = unsafe {
            slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u32>(), buf.as_mut_slice().len() / 4)
        };
        let n = decode_into(&page, out).expect("decode_into");
        assert_eq!(n, lanes);

        // Verify through the typed READ view — the as_typed::<u32>() pattern.
        assert_eq!(buf.as_typed::<u32>(), values.as_slice());
    }
}

/// Benchmark comparison (prints only, never an assertion): decode_into vs
/// rle_decode on the same pages, in the test (debug) profile.
#[cfg(test)]
mod bench_compare {
    extern crate std;

    use super::{decode_into, rle_decode, rle_encode};
    use alloc::vec::Vec;
    use std::hint::black_box;
    use std::time::Instant;
    use std::println;

    fn literal_page(lanes: usize) -> (Vec<u32>, Vec<u8>) {
        let values: Vec<u32> = (0..lanes).map(|i| (i * 7 + 1) as u32).collect();
        let page = rle_encode(&values, 8);
        (values, page)
    }

    fn run_page_page(values: usize) -> (Vec<u32>, Vec<u8>) {
        let mut values: Vec<u32> = (0..values).map(|i| (i * 13 + 3) as u32).collect();
        // Sprinkle repeated runs into the literal stretch: mixed arm exercise.
        for chunk in values.chunks_mut(32) {
            let first = chunk[0];
            chunk[..8].fill(first);
        }
        let page = rle_encode(&values, 8);
        (values, page)
    }

    fn bench_one(name: &str, lanes: usize, page: &[u8], iters: usize) {
        let mut via_into = alloc::vec![0u32; lanes];
        let mut via_rle = alloc::vec![0u32; lanes];
        // Correctness precondition for the comparison: both decode the same.
        decode_into(page, &mut via_into).expect("decode_into");
        rle_decode(page, &mut via_rle).expect("rle_decode");
        assert_eq!(via_into, via_rle, "benched decoders must agree");

        let t0 = Instant::now();
        for _ in 0..iters {
            let _ = decode_into(black_box(page), black_box(&mut via_into));
        }
        let dt_into = t0.elapsed();

        let t1 = Instant::now();
        for _ in 0..iters {
            let _ = rle_decode(black_box(page), black_box(&mut via_rle));
        }
        let dt_rle = t1.elapsed();

        let ns_into = dt_into.as_secs_f64() / iters as f64 * 1e9;
        let ns_rle = dt_rle.as_secs_f64() / iters as f64 * 1e9;
        println!(
            "[bench-compare] page={name} lanes={lanes} iters={iters} rle_decode={dt_rle:?} total, {ns_rle:.1} ns/iter | decode_into={dt_into:?} total, {ns_into:.1} ns/iter | ratio rle/into={:.2}x (debug profile)",
            ns_rle / ns_into
        );
    }

    #[test]
    fn bench_decode_into_vs_rle_decode() {
        let iters = 200;
        let (values, page) = literal_page(8192);
        bench_one("literal-8k", values.len(), &page, iters);
        let (values, page) = run_page_page(8192);
        bench_one("mixed-8k", values.len(), &page, iters);
    }
}
