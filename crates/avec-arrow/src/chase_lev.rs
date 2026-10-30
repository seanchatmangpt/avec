//! Branchless Chase-Lev work-stealing deque.
//!
//! Monotonically advancing unbounded 64-bit integer indices eliminate index
//! modulo arithmetic. Straight-line slot access with bitwise power-of-two masking
//! and pure atomic compare-and-swap contention resolution.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicI64, Ordering};

const DEQUE_CAP: usize = 512;
const DEQUE_MASK: i64 = (DEQUE_CAP - 1) as i64;

#[repr(C, align(64))]
pub struct WorkStealingDeque<T: Copy> {
    top: AtomicI64,
    _pad0: [u8; 56],
    bottom: AtomicI64,
    _pad1: [u8; 56],
    buffer: [UnsafeCell<MaybeUninit<T>>; DEQUE_CAP],
}

unsafe impl<T: Copy + Send> Sync for WorkStealingDeque<T> {}

impl<T: Copy> Default for WorkStealingDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy> WorkStealingDeque<T> {
    pub const fn new() -> Self {
        const { assert!(DEQUE_CAP.is_power_of_two(), "Capacity must be power of two") };
        Self {
            top: AtomicI64::new(0),
            _pad0: [0; 56],
            bottom: AtomicI64::new(0),
            _pad1: [0; 56],
            buffer: [const { UnsafeCell::new(MaybeUninit::uninit()) }; DEQUE_CAP],
        }
    }

    /// Owner pushes locally: single-threaded fast path
    #[inline(always)]
    pub fn push(&self, item: T) -> bool {
        let b = self.bottom.load(Ordering::Relaxed);
        let t = self.top.load(Ordering::Acquire);

        if b.wrapping_sub(t) >= DEQUE_CAP as i64 {
            return false; // Full
        }

        unsafe {
            (*self.buffer[(b & DEQUE_MASK) as usize].get()).write(item);
        }
        self.bottom.store(b.wrapping_add(1), Ordering::Release);
        true
    }

    /// Owner pops locally: single-threaded fast path
    #[inline(always)]
    pub fn pop(&self) -> Option<T> {
        let b = self.bottom.load(Ordering::Relaxed).wrapping_sub(1);
        self.bottom.store(b, Ordering::Relaxed);
        core::sync::atomic::fence(Ordering::SeqCst);

        let t = self.top.load(Ordering::Relaxed);
        if t <= b {
            // Straight-line extraction: slot index masked without modulo
            let item = unsafe { (*self.buffer[(b & DEQUE_MASK) as usize].get()).assume_init() };
            if t == b {
                // Contention resolution turnstile
                if self
                    .top
                    .compare_exchange(t, t + 1, Ordering::SeqCst, Ordering::Relaxed)
                    .is_err()
                {
                    self.bottom.store(t + 1, Ordering::Relaxed);
                    return None;
                }
                self.bottom.store(t + 1, Ordering::Relaxed);
            }
            Some(item)
        } else {
            self.bottom.store(t, Ordering::Relaxed);
            None
        }
    }

    /// Competitor steals from foreign thread: pure atomic CAS turnstile
    #[inline(always)]
    pub fn steal(&self) -> Option<T> {
        let t = self.top.load(Ordering::Acquire);
        core::sync::atomic::fence(Ordering::SeqCst);
        let b = self.bottom.load(Ordering::Acquire);

        if t < b {
            let item = unsafe { (*self.buffer[(t & DEQUE_MASK) as usize].get()).assume_init() };
            if self
                .top
                .compare_exchange(t, t + 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                return Some(item);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use alloc::vec::Vec;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn chase_lev_push_pop_lifo() {
        let q = WorkStealingDeque::<u64>::new();
        assert_eq!(q.pop(), None);
        assert!(q.push(10));
        assert!(q.push(20));
        assert!(q.push(30));

        assert_eq!(q.pop(), Some(30));
        assert_eq!(q.pop(), Some(20));
        assert_eq!(q.pop(), Some(10));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn chase_lev_concurrent_stealing() {
        let q = Arc::new(WorkStealingDeque::<usize>::new());
        let n = 200;

        for i in 0..n {
            assert!(q.push(i));
        }

        let q_stealer = q.clone();
        let stealer = thread::spawn(move || {
            let mut stolen = Vec::new();
            for _ in 0..10_000 {
                if let Some(item) = q_stealer.steal() {
                    stolen.push(item);
                }
            }
            stolen
        });

        let mut popped = Vec::new();
        while let Some(item) = q.pop() {
            popped.push(item);
        }

        let stolen = stealer.join().unwrap();
        assert_eq!(stolen.len() + popped.len(), n);

        let mut all = stolen;
        all.extend(popped);
        all.sort_unstable();
        for (i, v) in all.into_iter().enumerate() {
            assert_eq!(i, v);
        }
    }
}
