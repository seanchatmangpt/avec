//! SPSC monotonic ring buffer with branchless power-of-2 index masking.
//!
//! Enforces a power-of-two capacity ($2^k$) so that monotonically advancing
//! 64-bit sequence counters can be converted to slot indices by a simple
//! bitwise `& (CAPACITY - 1)`. 128-byte logical separation between head and tail
//! prevents false sharing on ARM64 and x86_64.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU64, Ordering};

pub const RING_CAPACITY: usize = 1024; // Must be 2^k
const RING_MASK: u64 = (RING_CAPACITY - 1) as u64;

#[repr(C, align(64))]
pub struct CachePadded<T> {
    pub value: T,
    _pad: [u8; 64],
}

#[repr(C, align(64))]
pub struct SpscRing<T> {
    // Producer writes to head; separated by 128 bytes from tail
    head: CachePadded<AtomicU64>,
    _separation: [u8; 64],
    tail: CachePadded<AtomicU64>,
    slots: [UnsafeCell<MaybeUninit<T>>; RING_CAPACITY],
}

unsafe impl<T: Send> Sync for SpscRing<T> {}

impl<T: Copy> Default for SpscRing<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy> SpscRing<T> {
    pub const fn new() -> Self {
        const {
            assert!(
                RING_CAPACITY.is_power_of_two(),
                "Capacity must be power of two"
            )
        };
        Self {
            head: CachePadded {
                value: AtomicU64::new(0),
                _pad: [0; 64],
            },
            _separation: [0; 64],
            tail: CachePadded {
                value: AtomicU64::new(0),
                _pad: [0; 64],
            },
            slots: [const { UnsafeCell::new(MaybeUninit::uninit()) }; RING_CAPACITY],
        }
    }

    #[inline(always)]
    pub fn try_push(&self, item: T) -> bool {
        let current_head = self.head.value.load(Ordering::Relaxed);
        let current_tail = self.tail.value.load(Ordering::Acquire);

        // Branchless capacity probe: occupancy calculated without conditional jumps
        let occupancy = current_head.wrapping_sub(current_tail);
        if occupancy >= RING_CAPACITY as u64 {
            return false; // Queue full
        }

        // Unconditional bitwise power-of-two mask
        let slot = (current_head & RING_MASK) as usize;
        unsafe {
            (*self.slots[slot].get()).write(item);
        }

        // Advance monotonic counter; wraps naturally at u64::MAX without branch
        self.head
            .value
            .store(current_head.wrapping_add(1), Ordering::Release);
        true
    }

    #[inline(always)]
    pub fn try_pop(&self) -> Option<T> {
        let current_tail = self.tail.value.load(Ordering::Relaxed);
        let current_head = self.head.value.load(Ordering::Acquire);

        if current_tail == current_head {
            return None; // Queue empty
        }

        let slot = (current_tail & RING_MASK) as usize;
        let item = unsafe { (*self.slots[slot].get()).assume_init() };

        self.tail
            .value
            .store(current_tail.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        let h = self.head.value.load(Ordering::Relaxed);
        let t = self.tail.value.load(Ordering::Relaxed);
        h.wrapping_sub(t) as usize
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn ring_single_thread_roundtrip_and_capacity() {
        let ring = SpscRing::<u64>::new();
        assert!(ring.is_empty());
        assert_eq!(ring.try_pop(), None);

        for i in 0..RING_CAPACITY {
            assert!(ring.try_push(i as u64));
        }
        assert!(!ring.try_push(999_999));
        assert_eq!(ring.len(), RING_CAPACITY);

        for i in 0..RING_CAPACITY {
            assert_eq!(ring.try_pop(), Some(i as u64));
        }
        assert_eq!(ring.try_pop(), None);
        assert!(ring.is_empty());
    }

    #[test]
    fn ring_concurrent_producer_consumer() {
        let ring = std::sync::Arc::new(SpscRing::<u64>::new());
        let r_prod = ring.clone();
        let n: u64 = 50_000;

        let prod = thread::spawn(move || {
            for i in 0..n {
                while !r_prod.try_push(i) {
                    core::hint::spin_loop();
                }
            }
        });

        let mut received = Vec::with_capacity(n as usize);
        while (received.len() as u64) < n {
            if let Some(v) = ring.try_pop() {
                received.push(v);
            } else {
                core::hint::spin_loop();
            }
        }
        prod.join().unwrap();

        for (i, v) in received.into_iter().enumerate() {
            assert_eq!(v, i as u64);
        }
    }
}
