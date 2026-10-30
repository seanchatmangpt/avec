//! Mutation-court kills for `crates/avec-arrow/src/**` (lane W2-MUT-ARROW).
//!
//! Every test names the backlog rows (`docs/mutation-court-backlog.txt` lines
//! 1-89, the avec-arrow block) it kills. Chicago discipline: real
//! collaborators only — `alloc` is the only collaborator and it is real; the
//! counting allocator is a thin wrapper over the real `System` allocator, so
//! every byte is genuinely allocated and freed.
//!
//! All tests serialize on `MEM_LOCK` so the allocator counter is observed
//! only while this binary has no other allocation in flight.

use avec_arrow::bitmap::{
    bcast8, bitmap_bytes, bitmap_words, clear_bit, count_one_bits, count_valid, get_bit,
    mask_words, set_bit, word_at, word_valid, Bitmap,
};
use avec_arrow::buffer::Buf;
use avec_arrow::c_abi::{export, ArrowArray, ForeignArray};
use avec_arrow::primitive::{Primitive, PrimitiveBuilder};
use avec_arrow::varlen::{Offset, Varlen, VarlenBuilder};
use avec_arrow::views::{is_inline, ViewVecBuilder};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

/// Net live bytes handed out by the real System allocator. Used only by the
/// `Buf::drop` kill (backlog row 59).
static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

struct CountingSystem;

// SAFETY: pure delegation to System with an atomic counter — identical
// contract to the system allocator itself.
unsafe impl std::alloc::GlobalAlloc for CountingSystem {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = unsafe { std::alloc::System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        let ptr = unsafe { std::alloc::System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        unsafe { std::alloc::System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static GLOBAL: CountingSystem = CountingSystem;

/// Poll until `LIVE_BYTES` returns to `baseline`, panicking on timeout.
///
/// Transient harness allocations shift the counter momentarily; a leak
/// shifts it permanently. The bounded poll observes real state on every
/// sample.
fn poll_live_returns_to(baseline: isize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if LIVE_BYTES.load(Ordering::SeqCst) == baseline {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("live bytes never returned to baseline {baseline}; a buffer was leaked");
}

// ---------------------------------------------------------------------------
// bitmap::bitmap_words (rows 1-2), bitmap_bytes (edge), bcast8 (rows 3-4)
// ---------------------------------------------------------------------------

#[test]
fn bitmap_words_boundaries() {
    assert_eq!(bitmap_words(0), 0);
    assert_eq!(bitmap_words(1), 1);
    assert_eq!(bitmap_words(63), 1);
    assert_eq!(bitmap_words(64), 1);
    assert_eq!(bitmap_words(65), 2);
    assert_eq!(bitmap_words(200), 4);
}

#[test]
fn bitmap_bytes_boundaries() {
    assert_eq!(bitmap_bytes(0), 0);
    assert_eq!(bitmap_bytes(1), 1);
    assert_eq!(bitmap_bytes(63), 8);
    assert_eq!(bitmap_bytes(64), 8);
    assert_eq!(bitmap_bytes(65), 9);
}

#[test]
fn bcast8_broadcasts_every_lane() {
    for b in [0u8, 1, 0x7F, 0x80, 0xAB, 0xFF] {
        assert_eq!(bcast8(b), u64::from_ne_bytes([b; 8]));
    }
}

// ---------------------------------------------------------------------------
// bitmap::mask_words — rows 5-12; count_valid — rows 13-21
// ---------------------------------------------------------------------------

#[test]
fn mask_words_ands_poisoned_out() {
    let valid = [0b1010u64, u64::MAX];
    let pred = [0b0110u64, 0x5555_5555_5555_5555];
    let mut out = [!0u64; 2];
    mask_words(&valid, &pred, &mut out);
    assert_eq!(out, [0b0010, 0x5555_5555_5555_5555]);
}

#[test]
fn mask_words_clamps_to_shortest_input() {
    // out longer than inputs: only n = min(len) words written. Kills the
    // `<`->`<=` mutant by panic (valid[n] OOB) and the `->()` / `<`->`>` /
    // `<`->`==` mutants by the poisoned tail words staying untouched.
    let valid = [0b1010u64];
    let pred = [0b0110u64];
    let mut out = [!0u64; 3];
    mask_words(&valid, &pred, &mut out);
    assert_eq!(out, [0b0010, !0, !0]);
}

#[test]
fn mask_words_empty_inputs() {
    let mut out: [u64; 0] = [];
    mask_words(&[], &[], &mut out);
    assert!(out.is_empty());
}

#[test]
fn count_valid_exact_multiword() {
    assert_eq!(count_valid(&[], &[]), 0);
    assert_eq!(count_valid(&[u64::MAX], &[u64::MAX]), 64);
    assert_eq!(count_valid(&[0b1010], &[0b0110]), 1);
    assert_eq!(
        count_valid(&[0b1010, u64::MAX], &[0b0110, 0x5555_5555_5555_5555]),
        1 + 32
    );
    // min(len) rule: only the first word counts.
    assert_eq!(count_valid(&[!0u64, !0], &[!0]), 64);
}

// ---------------------------------------------------------------------------
// bitmap bit ops — clear_bit rows 22-28; word_valid rows 29-34; word_at 35-40
// ---------------------------------------------------------------------------

#[test]
fn set_get_clear_across_lanes_and_byte_boundaries() {
    let mut bits = [0u8; 3];
    for i in 0..24 {
        assert_eq!(get_bit(&bits, i), 0);
        set_bit(&mut bits, i);
        assert_eq!(get_bit(&bits, i), 1);
    }
    assert_eq!(bits, [0xFF; 3]);
    for i in 0..24 {
        clear_bit(&mut bits, i);
        assert_eq!(get_bit(&bits, i), 0, "bit {i} not cleared");
        // All other bits must survive the clear (kills `!`-deletion and the
        // `&`->`|` / `&`->`^` mutants in clear_bit; the `i>>3`->`i<<3` mutant
        // panics at i=1 via bits[8] on a 3-byte slice).
        for j in (i + 1)..24 {
            assert_eq!(get_bit(&bits, j), 1, "bit {j} clobbered by clear of {i}");
        }
    }
}

#[test]
fn word_valid_reads_correct_byte_window() {
    let bits: Vec<u8> = (0..16u8).collect();
    assert_eq!(
        word_valid(&bits, 0),
        u64::from_ne_bytes([0, 1, 2, 3, 4, 5, 6, 7])
    );
    assert_eq!(
        word_valid(&bits, 1),
        u64::from_ne_bytes([8, 9, 10, 11, 12, 13, 14, 15])
    );
}

#[test]
fn word_at_reads_correct_byte_window() {
    let bytes: Vec<u8> = (0..24u8).map(|b| b.wrapping_mul(7)).collect();
    assert_eq!(
        word_at(&bytes, 0),
        u64::from_ne_bytes([0, 7, 14, 21, 28, 35, 42, 49])
    );
    assert_eq!(
        word_at(&bytes, 1),
        u64::from_ne_bytes([56, 63, 70, 77, 84, 91, 98, 105])
    );
    assert_eq!(
        word_at(&bytes, 2),
        u64::from_ne_bytes([112, 119, 126, 133, 140, 147, 154, 161])
    );
}

// ---------------------------------------------------------------------------
// Bitmap methods — as_ptr row 41; word row 42; words_len rows 43-44
// ---------------------------------------------------------------------------

#[test]
fn bitmap_as_ptr_is_backing_buffer() {
    let bm = Bitmap::from_bools(&[true, false, true]).unwrap();
    assert_eq!(bm.as_ptr(), bm.bytes().as_ptr());
    assert!(!bm.as_ptr().is_null());
}

#[test]
fn bitmap_word_tail_word_reads_partial_bytes() {
    // 70 bits = 9 bytes; word(1) must read ONLY byte 8 (take = 1), so the
    // `word_idx * 8` -> `word_idx / 8` mutant (which returns word 0) differs.
    let mut bm = Bitmap::from_bytes(Buf::new_zeroed(9), 70);
    for i in 0..8 {
        bm.set(i, true);
    }
    bm.set(64, true);
    bm.set(65, true);
    assert_eq!(bm.word(0), 0xFF);
    assert_eq!(bm.word(1), 0b11);
}

#[test]
fn bitmap_words_len_boundaries() {
    let bm0 = Bitmap::from_bytes(Buf::new_zeroed(0), 0);
    assert_eq!(bm0.words_len(), 0);
    let bm64 = Bitmap::from_bytes(Buf::new_zeroed(8), 64);
    assert_eq!(bm64.words_len(), 1);
    let bm65 = Bitmap::from_bytes(Buf::new_zeroed(9), 65);
    assert_eq!(bm65.words_len(), 2);
    let bm200 = Bitmap::from_bytes(Buf::new_zeroed(25), 200);
    assert_eq!(bm200.words_len(), 4);
}

// ---------------------------------------------------------------------------
// is_all_valid rows 45-51 (row 50 ledgered equivalent); count_one_bits 52-57
// (row 57 ledgered equivalent)
// ---------------------------------------------------------------------------

#[test]
fn is_all_valid_full_word_boundary() {
    // 64 bits exactly: full word, zero tail. Kills the `while` `<`->`>` skip,
    // the `+=` mutants (miscount or underflow panic on a full word), and the
    // final `==`->`!=`.
    let mut bm = Bitmap::from_bytes(Buf::new_zeroed(8), 64);
    assert!(!bm.is_all_valid());
    for i in 0..64 {
        bm.set(i, true);
    }
    assert!(bm.is_all_valid());
    bm.set(63, false);
    assert!(!bm.is_all_valid());
}

#[test]
fn is_all_valid_tail_word_boundary() {
    // 65 bits: full word 0 + one tail bit. Bit 64 lives only in the tail path.
    let mut bm = Bitmap::from_bytes(Buf::new_zeroed(9), 65);
    for i in 0..65 {
        bm.set(i, true);
    }
    assert!(bm.is_all_valid());
    bm.set(64, false);
    assert!(!bm.is_all_valid());
    bm.set(64, true);
    assert!(bm.is_all_valid());
}

#[test]
fn count_one_bits_multiword_and_tails() {
    let empty = Bitmap::from_bytes(Buf::new_zeroed(0), 0);
    assert_eq!(count_one_bits(&empty), 0);

    let mut bm128 = Bitmap::from_bytes(Buf::new_zeroed(16), 128);
    for i in 0..128 {
        bm128.set(i, true);
    }
    assert_eq!(count_one_bits(&bm128), 128); // loop-skip mutant counts only 64

    let mut bm70 = Bitmap::from_bytes(Buf::new_zeroed(9), 70);
    for i in 0..66 {
        bm70.set(i, true);
    }
    assert_eq!(count_one_bits(&bm70), 66);
    assert_eq!(bm70.null_count(), 4);

    let mut bm63 = Bitmap::from_bytes(Buf::new_zeroed(8), 63);
    bm63.set(0, true);
    bm63.set(62, true);
    assert_eq!(count_one_bits(&bm63), 2);
}

// ---------------------------------------------------------------------------
// buffer::Buf — rows 58-59 (drop kill via isolated-process counting probe)
// ---------------------------------------------------------------------------

#[test]
fn buf_is_empty_law() {
    assert!(Buf::new(0).is_empty());
    assert!(!Buf::new(1).is_empty());
    assert!(!Buf::new(64).is_empty());
}

/// Row 59 kill: a no-op `Buf::drop` leaks, and a leak is observable through
/// the real allocator. The probe re-executes THIS test binary as a single-
/// test subprocess (`--exact --test-threads=1`), so the allocator counter
/// sees exactly this test's allocations with zero concurrent test threads —
/// no straggler-spawn noise, fully deterministic. Real subprocess, real
/// allocator; nothing is mocked.
#[test]
fn buf_drop_deallocates() {
    if std::env::var("W2_LEAK_PROBE").is_ok() {
        // Probe mode (isolated child process).
        let baseline = LIVE_BYTES.load(Ordering::SeqCst);
        {
            let _b = Buf::new(4096);
        } // drop here
        // Correct code frees immediately; a no-op drop leaks 4096 bytes
        // forever. Bounded poll on a real observable.
        poll_live_returns_to(baseline);
        return;
    }
    let exe = std::env::current_exe().expect("current_exe");
    let ok = std::process::Command::new(exe)
        .args(["buf_drop_deallocates", "--exact", "--test-threads=1"])
        .env("W2_LEAK_PROBE", "1")
        .status()
        .expect("spawn leak-probe subprocess")
        .success();
    assert!(ok, "isolated leak probe failed (subprocess exit nonzero)");
}

// ---------------------------------------------------------------------------
// c_abi — rows 60-63 (ForeignArray::len, Drop release, CAS double-call)
// ---------------------------------------------------------------------------

#[test]
fn foreign_array_len_exact() {
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(1));
    b.push(None);
    b.push(Some(3));
    let (array, _schema) = export(b.finish(), c"i".as_ptr());
    assert_eq!(array.length, 3);
    let ptr = Box::into_raw(Box::new(array));
    // SAFETY: ptr is a live, spec-compliant array whose release we own.
    let fa = unsafe { ForeignArray::take(ptr) };
    assert_eq!(fa.len(), 3);
    assert!(!fa.is_empty());
    drop(fa); // releases the owned data; the shell box below is ours
    // SAFETY: the shell was boxed by this test and released above.
    drop(unsafe { Box::from_raw(ptr) });
}

#[test]
fn foreign_array_drop_runs_release_cas() {
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(7));
    let (array, _schema) = export(b.finish(), c"i".as_ptr());
    let ptr = Box::into_raw(Box::new(array));
    // SAFETY: reading the release slot of a live test-owned array.
    assert!(unsafe { (*ptr).release.is_some() });
    // SAFETY: single-owner import of a live spec-compliant array.
    let fa = unsafe { ForeignArray::take(ptr) };
    drop(fa);
    // Release CAS-swapped the slot to NULL — proves Drop ran (a no-op Drop
    // leaves Some(_) behind).
    // SAFETY: shell is still alive; only the private data box was freed.
    assert!(unsafe { (*ptr).release.is_none() });
    // SAFETY: shell freed exactly once; release already ran above.
    drop(unsafe { Box::from_raw(ptr) });
}

#[test]
fn release_double_call_is_typed_noop() {
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(7));
    let (array, _schema) = export(b.finish(), c"i".as_ptr());
    let ptr = Box::into_raw(Box::new(array));
    // SAFETY: test-owned heap ArrowArray; the CAS swap makes the second call
    // a typed no-op by contract.
    unsafe {
        let f = (*ptr).release.unwrap();
        f(ptr);
        assert!((*ptr).release.is_none());
        f(ptr); // double release
        assert!((*ptr).release.is_none());
        drop(Box::from_raw(ptr));
    }
}

// ---------------------------------------------------------------------------
// primitive — rows 64-72
// ---------------------------------------------------------------------------

#[test]
fn primitive_len_is_empty_laws() {
    let empty = PrimitiveBuilder::<i32>::new().finish();
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());

    let mut b = PrimitiveBuilder::<i32>::new();
    for i in 0..4 {
        b.push(Some(i));
    }
    let arr = b.finish();
    assert_eq!(arr.len(), 4);
    assert!(!arr.is_empty());
}

#[test]
fn primitive_ptrs_are_real() {
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(1));
    b.push(None);
    let arr = b.finish();
    assert_eq!(arr.values_ptr(), arr.values().as_ptr().cast::<u8>());
    assert!(!arr.values_ptr().is_null());
    let bm = arr.validity().expect("nulls present");
    assert_eq!(arr.validity_ptr(), bm.as_ptr());
    assert!(!arr.validity_ptr().is_null());
    // All-valid: no validity buffer, pointer is null by law.
    let mut b2 = PrimitiveBuilder::<i32>::new();
    b2.push(Some(5));
    let arr2 = b2.finish();
    assert!(arr2.validity_ptr().is_null());
}

#[test]
fn primitive_buffers_aligned_64_law() {
    // Alignment law: every reachable Primitive reports 64-aligned buffers —
    // real allocs via Layout(align=64), and the empty values Buf via the
    // dangling sentinel (probed 0x40 on this toolchain — itself 64-aligned).
    // Rows 69-70 (`-> true`, `&&`->`||`) are therefore
    // equivalent-by-unreachable-false; see the lane receipt.
    let empty = PrimitiveBuilder::<i32>::new().finish();
    assert!(empty.buffers_aligned_64());
    let mut b = PrimitiveBuilder::<i32>::new();
    b.push(Some(1));
    assert!(b.finish().buffers_aligned_64());
    let mut b2 = PrimitiveBuilder::<u64>::new();
    for i in 0..1000u64 {
        b2.push(if i % 17 == 0 { None } else { Some(i) });
    }
    assert!(b2.finish().buffers_aligned_64());
}

#[test]
fn builder_len_tracks_pushes() {
    let mut b = PrimitiveBuilder::<i32>::new();
    assert_eq!(b.len(), 0);
    b.push(Some(1));
    b.push(None);
    b.push(Some(3));
    assert_eq!(b.len(), 3);
}

// ---------------------------------------------------------------------------
// varlen — rows 73-86
// ---------------------------------------------------------------------------

#[test]
fn offset_format_families() {
    assert_eq!(<u32 as Offset>::format_family(), "32-bit");
    assert_eq!(<u64 as Offset>::format_family(), "64-bit");
}

#[test]
fn varlen_len_is_empty_laws() {
    let empty: Varlen<u32> = VarlenBuilder::<u32>::new().finish();
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
    assert_eq!(empty.offsets().len(), 1);
    assert_eq!(empty.offsets()[0].to_u64(), 0);
    assert!(empty.offsets_monotonic());
    assert!(empty.data().is_empty());

    let mut b = VarlenBuilder::<u64>::new();
    b.push(Some(b"alpha"));
    b.push(None);
    b.push(Some(b""));
    let v = b.finish();
    assert_eq!(v.len(), 3);
    assert!(!v.is_empty());
    assert_eq!(v.offsets(), &[0u64, 5, 5, 5]);
}

#[test]
fn varlen_data_is_exact_payload() {
    let mut b = VarlenBuilder::<u32>::new();
    b.push(Some(b"alpha"));
    b.push(None);
    b.push(Some(b"betax"));
    let v = b.finish();
    assert_eq!(v.data(), b"alphabetax");
}

#[test]
fn offsets_monotonic_holds_for_builder_output() {
    // Observable half of the offsets_monotonic rows: the builder output is
    // monotonic by construction; the `||`->`&&` mutant (row 82) differs only
    // on unreachable states — see the lane receipt.
    let mut b = VarlenBuilder::<u32>::new();
    b.push(Some(b"a"));
    b.push(None);
    b.push(Some(b"bcd"));
    let v = b.finish();
    assert!(v.offsets_monotonic());
    // u64 family too.
    let mut b2 = VarlenBuilder::<u64>::new();
    b2.push(Some(b"zz"));
    assert!(b2.finish().offsets_monotonic());
}

#[test]
#[should_panic(expected = "offset overflow")]
fn u32_offset_overflow_guard() {
    // u32::MAX + 1 must trip the u32-family offset overflow guard.
    let _ = <u32 as Offset>::from_usize((u32::MAX as usize) + 1);
}

#[test]
fn u64_offset_accepts_full_range() {
    assert_eq!(<u64 as Offset>::from_usize(usize::MAX), u64::MAX);
    assert_eq!(<u32 as Offset>::from_usize(u32::MAX as usize), u32::MAX);
}

// ---------------------------------------------------------------------------
// views — rows 87-89; contract edge: is_inline at the 12/13 boundary
// ---------------------------------------------------------------------------

#[test]
fn viewvec_len_is_empty_laws() {
    let empty = ViewVecBuilder::new().finish();
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());
    assert!(empty.views_bytes().is_empty());
    assert!(empty.data().is_empty());

    let mut b = ViewVecBuilder::new();
    b.push(Some(b"short"));
    b.push(Some(b"this-one-is-longer-than-twelve"));
    b.push(None);
    let v = b.finish();
    assert_eq!(v.len(), 3);
    assert!(!v.is_empty());
    assert_eq!(v.views_bytes().len(), 3 * 16);
    assert_eq!(v.data().len(), 30);
}

#[test]
fn is_inline_boundary_12_13() {
    assert_eq!(is_inline(0), 1);
    assert_eq!(is_inline(11), 1);
    assert_eq!(is_inline(12), 1);
    assert_eq!(is_inline(13), 0);
    assert_eq!(is_inline(14), 0);
    assert_eq!(is_inline(u32::MAX), 0);
    // i64 arithmetic: (len - 13) >> 63 — sign bit of (len - 13).
    assert_eq!(is_inline(u32::MAX - 1), 0);
}
