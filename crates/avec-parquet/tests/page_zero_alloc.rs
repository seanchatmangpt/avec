//! Zero-allocation court for the page decode path (lane OPT-L4, v26.10.29).
//!
//! Property under test: [`avec_parquet::page::decode_page_into`] — header
//! parse plus direct-slice body decode — performs ZERO heap allocations on
//! both its success and its failure paths.
//!
//! Why this court lives in `tests/` and not in `page.rs`'s `#[cfg(test)]`
//! module: it needs a counting `#[global_allocator]`, and the lib test
//! binary already has one (`rle.rs`'s `alloc_proof` court, which is outside
//! this lane's file ownership). rustc 1.97.0 rejects a second allocator
//! with "cannot define multiple global allocators" — measured this session.
//! An integration test binary links the lib WITHOUT `cfg(test)`, so this
//! file's counting allocator is that binary's single allocator. Same house
//! pattern as `avec-arrow/tests/mutation_kills.rs`.
//!
//! Method (Chicago school: real allocator, real pages, measured deltas):
//! the allocator is a pass-through to `System` that bumps a thread-local
//! counter (thread-locals keep concurrently running tests from polluting
//! each other's measurement window). A positive-control test proves the
//! counter is live — a court that cannot flip is FAIL, never a pass.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::thread_local;

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

// SAFETY: pure pass-through to System with a counted bump; the contract of
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

/// Mixed literal+run page: exercises both decode arms of the hybrid body.
/// Values stay inside the declared 8-bit domain — the codec truncates
/// out-of-range values by contract, so a fixture that overflows would fail
/// its own value-equality assert (measured, then fixed, this session).
fn court_page(lanes: usize) -> (Vec<u32>, Vec<u8>) {
    let values: Vec<u32> = (0..lanes)
        .map(|i| if i < lanes / 2 { ((i * 7 + 1) & 0xFF) as u32 } else { 42 })
        .collect();
    let page = avec_parquet::page::write_data_page_v2(&values, 8, lanes as u32);
    (values, page)
}

#[test]
fn decode_page_into_zero_allocations_success_and_error_paths() {
    let lanes = 4096;
    let (values, page) = court_page(lanes);
    let mut sink = vec![0xDEAD_BEEFu32; lanes];

    // Success path: header parse + full body decode, zero allocations.
    let before = current_allocs();
    let n = avec_parquet::page::decode_page_into(&page, &mut sink).expect("decode");
    let delta = current_allocs() - before;
    println!("[page-alloc-court] decode_page_into ok-path allocs={delta} lanes={lanes}");
    assert_eq!(
        delta, 0,
        "decode_page_into allocated {delta} times — zero-alloc law violated"
    );
    assert_eq!(n, lanes);
    assert_eq!(sink, values, "decoded lanes must match the page's values");

    // Error path: corrupt header (truncated to half) must be a typed Err
    // and must not allocate either — fail-closed includes allocation.
    let before = current_allocs();
    let err = avec_parquet::page::decode_page_into(&page[..page.len() / 2], &mut sink);
    let delta = current_allocs() - before;
    println!("[page-alloc-court] decode_page_into err-path allocs={delta}");
    assert!(err.is_err(), "half a page must fail closed");
    assert_eq!(
        delta, 0,
        "the error path allocated {delta} times — zero-alloc law violated"
    );

    // Run-dominant page: the repeated-run arm, same law.
    let run_values = vec![7u32; 2048];
    let run_page = avec_parquet::page::write_data_page_v2(&run_values, 4, 2048);
    sink.fill(0);
    let before = current_allocs();
    let n = avec_parquet::page::decode_page_into(&run_page, &mut sink).expect("run decode");
    let delta = current_allocs() - before;
    println!("[page-alloc-court] decode_page_into run-page allocs={delta} lanes=2048");
    assert_eq!(delta, 0, "run-page decode allocated {delta} times");
    assert_eq!(n, 2048);
    // sink is the 4096-lane buffer from the first page: only the first
    // 2048 lanes are the run page's decode target.
    assert_eq!(&sink[..n], run_values.as_slice(), "run lanes must decode exactly");
    assert!(
        sink[2048..].iter().all(|&x| x == 0),
        "lanes beyond the page's claim must stay zero-filled"
    );
}

#[test]
fn positive_control_counter_is_live() {
    // Anti-vacuity: the same measurement window around an allocation MUST
    // move the counter. If this ever fails, the zero-alloc assertions above
    // are measuring nothing.
    let before = current_allocs();
    let probe: Vec<u8> = (0..64u8).collect();
    let delta = current_allocs() - before;
    println!("[page-alloc-court] positive-control allocs={delta}");
    assert!(delta >= 1, "counting allocator counted {delta} allocs for a fresh Vec — court is vacuous");
    assert_eq!(probe.len(), 64);
}
