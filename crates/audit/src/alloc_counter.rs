//! Allocation-counting harness (CTQ-02, docs/jira/v26.10.31/29-benchmark-harness-plan.md §2).
//!
//! A `GlobalAlloc` wrapper over [`System`] with thread-safe counters plus a
//! scoped guard, [`with_alloc_tracking`], that reports the allocations a
//! closure performed. Intended consumers: tests and benches asserting the
//! zero-allocation structural invariant (decode scan, admission path).
//! cgap-blake3's zero-alloc test lane (I10) writes its own; this crate is the
//! shared primitive going forward.
//!
//! The wrapper is installed as the process `#[global_allocator]` in this
//! crate's test binary (and in the audit cdylib, where it is a pass-through
//! cost: two atomic adds per alloc on threads not under tracking).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

/// Process-wide allocation call count since start (all threads).
pub static TOTAL_ALLOCS: AtomicUsize = AtomicUsize::new(0);
/// Process-wide allocated byte total since start (all threads).
pub static TOTAL_ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Counting pass-through allocator delegating to [`System`].
pub struct CountingAlloc;

// Per-thread tracking state. `Copy + Cell` + const-init TLS so the alloc hook
// never allocates while reading it.
#[derive(Clone, Copy)]
struct Tracking {
    active: bool,
    allocs: usize,
    bytes: usize,
}

thread_local! {
    static TRACKING: Cell<Tracking> = const { Cell::new(Tracking { active: false, allocs: 0, bytes: 0 }) };
}

#[global_allocator]
static COUNTING: CountingAlloc = CountingAlloc;

// Safety: forwards every call to System with identical semantics; counting
// uses only lock-free atomics / const-init thread-locals (no allocation,
// no panic path inside the hook).
unsafe impl GlobalAlloc for CountingAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            TOTAL_ALLOCS.fetch_add(1, Relaxed);
            TOTAL_ALLOC_BYTES.fetch_add(layout.size(), Relaxed);
            TRACKING.with(|t| {
                let mut s = t.get();
                if s.active {
                    s.allocs += 1;
                    s.bytes += layout.size();
                }
                t.set(s);
            });
        }
        ptr
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            TOTAL_ALLOCS.fetch_add(1, Relaxed);
            TOTAL_ALLOC_BYTES.fetch_add(new_size, Relaxed);
            TRACKING.with(|t| {
                let mut s = t.get();
                if s.active {
                    s.allocs += 1;
                    s.bytes += new_size;
                }
                t.set(s);
            });
        }
        new_ptr
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            TOTAL_ALLOCS.fetch_add(1, Relaxed);
            TOTAL_ALLOC_BYTES.fetch_add(layout.size(), Relaxed);
            TRACKING.with(|t| {
                let mut s = t.get();
                if s.active {
                    s.allocs += 1;
                    s.bytes += layout.size();
                }
                t.set(s);
            });
        }
        ptr
    }
}

/// Allocations observed during one [`with_alloc_tracking`] scope.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllocReport {
    /// Number of allocation calls (alloc/realloc/alloc_zeroed successes).
    pub count: usize,
    /// Sum of requested sizes in bytes.
    pub bytes: usize,
}

impl AllocReport {
    /// True when the tracked scope performed no allocations at all.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.count == 0
    }
}

/// Runs `f` with per-thread allocation tracking active and returns
/// `(value, report)` where `report` counts only allocations made on the
/// calling thread inside `f`. Scopes nest: an inner scope's report excludes
/// allocations accounted to the enclosing scope's baseline.
pub fn with_alloc_tracking<T>(f: impl FnOnce() -> T) -> (T, AllocReport) {
    TRACKING.with(|t| {
        let prev = t.get();
        t.set(Tracking {
            active: true,
            allocs: 0,
            bytes: 0,
        });
        let value = f();
        let done = t.get();
        // Fold this scope's counts into the enclosing scope's running totals
        // so an outer report covers ALL its allocations (inner sees only its
        // own delta).
        t.set(Tracking {
            active: prev.active,
            allocs: prev.allocs + done.allocs,
            bytes: prev.bytes + done.bytes,
        });
        (
            value,
            AllocReport {
                count: done.allocs,
                bytes: done.bytes,
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_only_closure_allocates_nothing() {
        let (v, report) = with_alloc_tracking(|| {
            let mut acc = 0u64;
            for i in 0..1000u64 {
                acc = acc.wrapping_mul(3).wrapping_add(i);
            }
            acc
        });
        assert_ne!(v, u64::MAX); // value computed, not optimized into nothing
        assert!(report.is_zero(), "stack-only closure allocated: {report:?}");
        assert_eq!(report.bytes, 0);
    }

    #[test]
    fn vec_push_reports_nonzero_count_and_bytes() {
        let (vec, report) = with_alloc_tracking(|| {
            let mut v = Vec::with_capacity(0);
            for i in 0..100u64 {
                v.push(i);
            }
            v
        });
        assert_eq!(vec.len(), 100);
        assert!(report.count > 0, "expected allocations, got {report:?}");
        assert!(report.bytes > 0);
    }

    #[test]
    fn nested_inner_scope_excludes_outer_allocations() {
        let ((_v, inner), outer) = with_alloc_tracking(|| {
            let _pad = vec![0u8; 4096];
            with_alloc_tracking(|| vec![1u8; 8])
        });
        assert_eq!(_v.len(), 8);
        assert!(inner.count >= 1, "inner scope lost its alloc: {inner:?}");
        assert!(
            outer.count > inner.count,
            "outer must exceed inner: {outer:?} vs {inner:?}"
        );
    }

    #[test]
    fn multithreaded_totals_add_up() {
        let baseline = TOTAL_ALLOCS.load(Relaxed);
        let handles: Vec<_> = (0..4)
            .map(|t| {
                std::thread::spawn(move || {
                    let (sum, report) = with_alloc_tracking(move || {
                        let mut v = Vec::with_capacity(1000);
                        for i in 0..1000u64 {
                            v.push((t as u64) * 1000 + i);
                        }
                        v.into_iter().sum::<u64>()
                    });
                    assert!(report.count >= 1);
                    assert!(report.bytes >= 8000, "1000 u64s: {report:?}");
                    sum
                })
            })
            .collect();
        let total: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(total, 4 * (999 * 1000 / 2) + 1000 * (1000 + 2000 + 3000));
        assert!(
            TOTAL_ALLOCS.load(Relaxed) >= baseline + 4,
            "global counter did not advance across threads"
        );
    }

    #[test]
    fn tracking_is_reset_after_scope() {
        let (_, first) = with_alloc_tracking(|| vec![0u8; 32]);
        assert!(first.count >= 1);
        let (_, second) = with_alloc_tracking(|| 1 + 1);
        assert!(second.is_zero(), "state leaked across scopes: {second:?}");
    }
}
