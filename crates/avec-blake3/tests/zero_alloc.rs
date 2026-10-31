//! FR-06 zero-allocation court (`AVEC-111` / v26.10.31 spec §5).
//!
//! Installs a counting `GlobalAlloc` wrapper (test-binary scoped) and asserts that a
//! large hashing pass through the crate's public API performs ZERO heap allocations.
//!
//! Chicago-school: real 64 MiB buffer, real hashing, assert on allocator state — no mocks.
//!
//! Parallel-safe by construction, not by documentation:
//!
//! 1. Every zero-alloc test serializes on a process-wide mutex taken at test START
//!    (before buffer construction), so sibling test threads block instead of
//!    allocating inside a peer's measured window. `--test-threads=1` is the
//!    uncontended special case.
//! 2. Counting is per-THREAD (`thread_local!` counters, const-initialized so TLS
//!    access from the allocator never allocates). The harness's own threads (libtest
//!    prints "test ... ok" from the main thread, which never takes our lock) allocate
//!    concurrently; per-thread counting makes those invisible to the measured delta.
//! 3. Counters are reset under the held lock immediately before the measured body,
//!    discarding this thread's own buffer-construction traffic.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::{Mutex, MutexGuard};

struct CountingAlloc;

thread_local! {
    /// Per-thread allocation count. `const` init => TLS access from the global
    /// allocator never allocates (no lazy-init allocation on foreign threads).
    static T_ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
    /// Per-thread allocation bytes.
    static T_ALLOC_BYTES: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // CountingAlloc: counting wrapper, real System underneath
        T_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        T_ALLOC_BYTES.with(|b| b.set(b.get() + layout.size()));
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        unsafe { System.dealloc(_ptr, _layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        T_ALLOC_COUNT.with(|c| c.set(c.get() + 1));
        T_ALLOC_BYTES.with(|b| b.set(b.get() + new_size));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// 64 MiB hashed in 1 KiB chunks (spec §5 workload).
const TOTAL: usize = 64 * 1024 * 1024;
const CHUNK: usize = 1024;

fn fill_pseudo(buf: &mut [u8]) {
    let mut x: u64 = 0x9E3779B97F4A7C15;
    for b in buf.iter_mut() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (x >> 33) as u8;
    }
}

/// Process-wide measurement lock: only one test thread measures at a time; all
/// other zero-alloc tests block here instead of allocating inside a peer's window.
static MEASURE_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the measurement lock. Called at test START (before buffer construction),
/// so sibling tests - including their 64 MiB buffer builds - cannot allocate inside
/// this test's measured window. A poisoned lock (a prior assert failure) is recovered,
/// not propagated: the next test must still run.
fn enter_measure() -> MutexGuard<'static, ()> {
    MEASURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Zero this thread's counters under the held lock, immediately before the measured
/// body, discarding the thread's own buffer-construction traffic.
fn reset_counters() {
    T_ALLOC_COUNT.with(|c| c.set(0));
    T_ALLOC_BYTES.with(|b| b.set(0));
}

/// Snapshot the allocator counters around a closure body (lock must be held).
/// Returns (allocation count delta, allocation byte delta).
fn count_allocs(guard: &MutexGuard<'static, ()>, f: impl FnOnce()) -> (usize, usize) {
    let _ = &guard; // lock held for the whole measured window
    let before_c = T_ALLOC_COUNT.with(Cell::get);
    let before_b = T_ALLOC_BYTES.with(Cell::get);
    f();
    (
        T_ALLOC_COUNT.with(Cell::get) - before_c,
        T_ALLOC_BYTES.with(Cell::get) - before_b,
    )
}

fn make_buffer() -> Vec<u8> {
    let mut buf = vec![0u8; TOTAL];
    fill_pseudo(&mut buf);
    buf
}

#[test]
fn hash_scalar_64mib_is_zero_alloc() {
    let guard = enter_measure();
    let buf = make_buffer();
    reset_counters();
    let (n, bytes) = count_allocs(&guard, || {
        let d = avec_blake3::hash(&buf);
        std::hint::black_box(d);
    });
    assert_eq!(n, 0, "avec_blake3::hash allocated {n} times ({bytes} bytes)");
}

#[test]
fn hash_wide_simd_64mib_is_zero_alloc() {
    let guard = enter_measure();
    let buf = make_buffer();
    reset_counters();
    let (n, bytes) = count_allocs(&guard, || {
        let d = avec_blake3::hash_wide_simd(&buf);
        std::hint::black_box(d);
    });
    assert_eq!(
        n, 0,
        "avec_blake3::hash_wide_simd allocated {n} times ({bytes} bytes)"
    );
}

#[test]
fn chunked_1kib_pass_is_zero_alloc() {
    // The spec workload: 64 MiB in 1 KiB chunk hashings through the public API.
    let guard = enter_measure();
    let buf = make_buffer();
    reset_counters();
    let (n, bytes) = count_allocs(&guard, || {
        let mut acc = 0u8;
        let mut off = 0;
        while off < TOTAL {
            let d = avec_blake3::hash(&buf[off..off + CHUNK]);
            acc = acc.wrapping_add(std::hint::black_box(d[0]));
            off += CHUNK;
        }
        std::hint::black_box(acc);
    });
    assert_eq!(n, 0, "chunked pass allocated {n} times ({bytes} bytes)");
}

#[test]
fn compress_is_zero_alloc() {
    let block = [0xA5u32; 16];
    let cv = [0x11223344u32; 8];
    let guard = enter_measure();
    reset_counters();
    let (n, _) = count_allocs(&guard, || {
        let out = avec_blake3::compress(&cv, &block, 0, 64, 0);
        std::hint::black_box(out);
    });
    assert_eq!(n, 0);
}

#[test]
fn positive_control_counter_is_live() {
    // Anti-vacuity (COURT-UNPROVEN #2): the zero-alloc assertions above are only
    // meaningful if the counters actually observe allocations. Inside a tracked
    // window, a fresh heap Vec MUST move this thread's counter. Then the counters
    // are reset and the real claim — the blake3 hashing path reports 0 — is
    // re-asserted in the same test, on the same thread, under the same lock.
    let guard = enter_measure();

    reset_counters();
    let (n, bytes) = count_allocs(&guard, || {
        let probe: Vec<u8> = (0..64u8).collect();
        std::hint::black_box(&probe);
    });
    assert!(
        n >= 1,
        "counting allocator counted {n} allocs ({bytes} bytes) for a fresh Vec — court is vacuous"
    );

    // The real claim, re-proven under the same held lock on the same thread.
    let buf = make_buffer();
    reset_counters();
    let (n, bytes) = count_allocs(&guard, || {
        let d = avec_blake3::hash(&buf);
        std::hint::black_box(d);
    });
    assert_eq!(n, 0, "avec_blake3::hash allocated {n} times ({bytes} bytes)");
}
