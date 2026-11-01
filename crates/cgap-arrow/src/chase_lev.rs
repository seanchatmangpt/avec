//! Branchless Chase-Lev work-stealing deque.
//!
//! Monotonically advancing integer indices eliminate index modulo arithmetic.
//! Straight-line slot access with bitwise power-of-two masking and pure atomic
//! compare-and-swap contention resolution.
//!
//! # Target-dependent index width (RV32IMC blocker fix, v26.11.1)
//!
//! Index width follows the target's CAS capability:
//!
//! - `target_has_atomic = "64"` (host aarch64, x86_64, RV64): `i64` indices in
//!   `AtomicI64` — the pre-fix representation, orderings unchanged.
//! - `target_has_atomic = "32"` only (32-bit A-extension targets): machine-word
//!   `isize` indices in `AtomicIsize` — the classic Chase-Lev machine-word
//!   form. The `bottom - top` difference is bounded by `DEQUE_CAP`, so 32-bit
//!   wrapping preserves the slot-mask invariants; only the wrap period
//!   shortens (2^32 vs 2^64 advances).
//! - Load/store only (riscv32imc: no A extension, no RMW at any width —
//!   probed: `Atomic<u32>::compare_exchange` is E0599 on that target): the
//!   owner-only core (push/pop with the SeqCst fence) still runs on
//!   `AtomicIsize` load/store, but the steal turnstile needs CAS, which the
//!   target cannot provide. The capability boundary is typed, not silent:
//!   [`WorkStealingDeque::SUPPORTS_STEAL`] is `false` and
//!   [`WorkStealingDeque::steal`] returns `None`.
//!
//! Protocol orderings (Acquire/Release/SeqCst turnstile) are identical on
//! every tier that has a CAS; the protocol is never weakened where it exists.
//! On the load/store-only tier the owner is the sole `top`-writer (a stealer
//! cannot claim a slot without CAS), so `pop`'s last-element branch advances
//! the cursors with plain stores — single-writer safe by construction.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::Ordering;

const DEQUE_CAP: usize = 512;
const DEQUE_LINE: usize = 64;

/// Index arithmetic width where the target has 64-bit CAS.
#[cfg(target_has_atomic = "64")]
mod index_width {
    use core::sync::atomic::AtomicI64;

    /// Signed wrapping index arithmetic width.
    pub type Index = i64;
    /// Atomic cell backing the `top` / `bottom` cursors.
    pub type AtomicIndex = AtomicI64;
    /// `Index` zero.
    pub const ZERO: Index = 0;
    /// Low-bit slot mask, `Index`-typed.
    pub const MASK: Index = (super::DEQUE_CAP - 1) as Index;
}

/// Index arithmetic width where only machine-word CAS exists.
#[cfg(all(target_has_atomic = "32", not(target_has_atomic = "64")))]
mod index_width {
    use core::sync::atomic::AtomicIsize;

    pub type Index = isize;
    pub type AtomicIndex = AtomicIsize;
    pub const ZERO: Index = 0;
    pub const MASK: Index = (super::DEQUE_CAP - 1) as Index;
}

/// Index width on load/store-only targets (riscv32imc): machine word in
/// `AtomicIsize` — load/store exist there (`target_has_atomic_load_store`),
/// CAS does not.
#[cfg(not(any(target_has_atomic = "64", target_has_atomic = "32")))]
mod index_width {
    use core::sync::atomic::AtomicIsize;

    pub type Index = isize;
    pub type AtomicIndex = AtomicIsize;
    pub const ZERO: Index = 0;
    pub const MASK: Index = (super::DEQUE_CAP - 1) as Index;
}

/// Compile-time capability predicate: `true` iff the target provides a CAS
/// wide enough to run the Chase-Lev steal turnstile.
pub const SUPPORTS_STEAL: bool = cfg!(any(
    target_has_atomic = "64",
    target_has_atomic = "32"
));

use index_width::{AtomicIndex, Index, MASK, ZERO};

/// Lock-free(steal) work-stealing deque; owner push/pop, thieves steal FIFO.
#[repr(C, align(64))]
pub struct WorkStealingDeque<T: Copy> {
    top: AtomicIndex,
    _pad0: [u8; DEQUE_LINE - core::mem::size_of::<AtomicIndex>()],
    bottom: AtomicIndex,
    _pad1: [u8; DEQUE_LINE - core::mem::size_of::<AtomicIndex>()],
    buffer: [UnsafeCell<MaybeUninit<T>>; DEQUE_CAP],
}

unsafe impl<T: Copy + Send> Sync for WorkStealingDeque<T> {}

impl<T: Copy> Default for WorkStealingDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy> WorkStealingDeque<T> {
    /// Compile-time steal capability (see module docs; mirrors the module
    /// level [`SUPPORTS_STEAL`]).
    pub const SUPPORTS_STEAL: bool = crate::chase_lev::SUPPORTS_STEAL;

    pub const fn new() -> Self {
        const { assert!(DEQUE_CAP.is_power_of_two(), "Capacity must be power of two") };
        Self {
            top: AtomicIndex::new(ZERO),
            _pad0: [0; DEQUE_LINE - core::mem::size_of::<AtomicIndex>()],
            bottom: AtomicIndex::new(ZERO),
            _pad1: [0; DEQUE_LINE - core::mem::size_of::<AtomicIndex>()],
            buffer: [const { UnsafeCell::new(MaybeUninit::uninit()) }; DEQUE_CAP],
        }
    }

    /// Owner pushes locally: single-threaded fast path
    #[inline(always)]
    pub fn push(&self, item: T) -> bool {
        let b = self.bottom.load(Ordering::Relaxed);
        let t = self.top.load(Ordering::Acquire);

        if b.wrapping_sub(t) >= DEQUE_CAP as Index {
            return false; // Full
        }

        unsafe {
            (*self.buffer[(b & MASK) as usize].get()).write(item);
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
            let item = unsafe { (*self.buffer[(b & MASK) as usize].get()).assume_init() };
            if t == b {
                // Contention resolution turnstile against a concurrent
                // stealer. On a load/store-only target no stealer can claim
                // (no RMW), the owner is the sole `top`-writer, so a plain
                // store advances the turnstile safely.
                #[cfg(any(target_has_atomic = "64", target_has_atomic = "32"))]
                {
                    if self
                        .top
                        .compare_exchange(
                            t,
                            t.wrapping_add(1),
                            Ordering::SeqCst,
                            Ordering::Relaxed,
                        )
                        .is_err()
                    {
                        self.bottom.store(t.wrapping_add(1), Ordering::Relaxed);
                        return None;
                    }
                }
                #[cfg(not(any(target_has_atomic = "64", target_has_atomic = "32")))]
                {
                    self.top.store(t.wrapping_add(1), Ordering::SeqCst);
                }
                self.bottom.store(t.wrapping_add(1), Ordering::Relaxed);
            }
            Some(item)
        } else {
            self.bottom.store(t, Ordering::Relaxed);
            None
        }
    }

    /// Competitor steals from foreign thread: pure atomic CAS turnstile.
    ///
    /// On a load/store-only target (no RMW primitive at any width) the
    /// turnstile cannot claim a slot; the capability boundary is typed via
    /// [`WorkStealingDeque::SUPPORTS_STEAL`] and this returns `None` — never
    /// a claim without CAS.
    #[inline(always)]
    pub fn steal(&self) -> Option<T> {
        #[cfg(not(any(target_has_atomic = "64", target_has_atomic = "32")))]
        {
            let _ = self; // cursors untouched: owner is the sole top-writer
            None
        }
        #[cfg(any(target_has_atomic = "64", target_has_atomic = "32"))]
        {
            let t = self.top.load(Ordering::Acquire);
            core::sync::atomic::fence(Ordering::SeqCst);
            let b = self.bottom.load(Ordering::Acquire);

            if t < b {
                let item = unsafe { (*self.buffer[(t & MASK) as usize].get()).assume_init() };
                if self
                    .top
                    .compare_exchange(t, t.wrapping_add(1), Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    return Some(item);
                }
            }
            None
        }
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

    /// Anti-vacuity for the width gate: the host must select the 64-bit CAS
    /// tier; the capability predicate must agree with the cfg selection.
    #[test]
    fn chase_lev_tier_selection_host_is_64bit_cas() {
        assert!(SUPPORTS_STEAL);
        assert!(WorkStealingDeque::<u64>::SUPPORTS_STEAL);
        assert_eq!(core::mem::size_of::<Index>(), core::mem::size_of::<u64>());
    }

    /// Deterministic last-element turnstile (t == b branch, CAS success path)
    /// plus cursor reuse after the turnstile wrapped both cursors.
    #[test]
    fn chase_lev_last_element_turnstile_single_thread() {
        let q = WorkStealingDeque::<u64>::new();
        assert!(q.push(7));
        // Drain to the single element: b == t forces the turnstile branch.
        assert_eq!(q.pop(), Some(7));
        assert_eq!(q.pop(), None);
        // Deque must be reusable: cursors advanced together, invariants held.
        assert!(q.push(8));
        assert_eq!(q.pop(), Some(8));
        assert_eq!(q.pop(), None);
    }
}
