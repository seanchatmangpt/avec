//! Zero-copy array slicing (wave-3 lane W3-SLICE owns this file).
//!
//! # Zero-copy contract
//!
//! A [`Sliced`] borrows its parent [`Primitive`](crate::primitive::Primitive):
//! it holds exactly one reference plus two `usize`s (`offset`, `len`) — no
//! buffer is copied, duplicated, or reallocated, ever. [`slice`] performs no
//! heap allocation (proven by the allocation-counting test below), and every
//! [`Sliced::get`] reads through to the parent's value buffer by pointer
//! arithmetic — base `values_ptr` plus `(offset + i) * T::WIDTH` — and to the
//! parent's validity bitmap at absolute bit `offset + i`.

use crate::primitive::{Primitive, Scalar};

/// Typed failure of a [`slice`] bounds check (bad input never panics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceError {
    /// `offset + len` (checked) exceeds the parent row count.
    RangeOutOfBounds {
        /// Requested window start.
        offset: usize,
        /// Requested window length.
        len: usize,
        /// Parent row count.
        array_len: usize,
    },
}

impl core::fmt::Display for SliceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            SliceError::RangeOutOfBounds {
                offset,
                len,
                array_len,
            } => write!(
                f,
                "slice range out of bounds: offset {offset} + len {len} exceeds array_len {array_len}"
            ),
        }
    }
}

impl core::error::Error for SliceError {}

/// Zero-copy window over a borrowed [`Primitive`](crate::primitive::Primitive).
///
/// Constructed only through [`slice`], which validates the window bounds.
/// See the [module docs](self) for the zero-copy contract.
pub struct Sliced<'a, T: Scalar> {
    array: &'a Primitive<T>,
    offset: usize,
    len: usize,
}

/// A window view is trivially duplicable: every field is `Copy` (one shared
/// reference + two `usize`s) — duplication never touches the buffers.
impl<T: Scalar> Clone for Sliced<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Scalar> Copy for Sliced<'_, T> {}

/// Equality = same parent allocation (pointer identity) + same window.
impl<T: Scalar> PartialEq for Sliced<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.array, other.array)
            && self.offset == other.offset
            && self.len == other.len
    }
}

impl<T: Scalar> Eq for Sliced<'_, T> {}

impl<T: Scalar> core::fmt::Debug for Sliced<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sliced")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .finish()
    }
}

impl<'a, T: Scalar> Sliced<'a, T> {
    /// Logical row count of the window.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when the window holds zero rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Value + validity of logical row `i`.
    ///
    /// Honors the window offset: validity is parent bit `offset + i`, the
    /// value is `parent.values()[offset + i]`. Total — returns [`None`]
    /// without panicking when `i >= self.len()` (same sentinel as a null
    /// row; row iteration is via [`Sliced::iter`], which never goes out
    /// of range).
    #[must_use]
    pub fn get(&self, i: usize) -> Option<T> {
        if i >= self.len {
            return None;
        }
        let abs = self.offset + i;
        let valid = self.array.validity().is_none_or(|bm| bm.get(abs));
        valid.then(|| self.array.values()[abs])
    }

    /// Iterate the window's rows as `Option<T>` (None = null).
    pub fn iter(&self) -> impl Iterator<Item = Option<T>> + '_ {
        (0..self.len).map(move |i| self.get(i))
    }
}

/// Slice `array` into a zero-copy [`Sliced`] window.
///
/// # Errors
///
/// [`SliceError::RangeOutOfBounds`] when `offset + len` (checked, so
/// `usize::MAX` inputs saturate instead of overflowing) exceeds
/// `array.len()`. No panic on bad input.
pub fn slice<'a, T: Scalar>(
    array: &'a Primitive<T>,
    offset: usize,
    len: usize,
) -> Result<Sliced<'a, T>, SliceError> {
    match offset.checked_add(len) {
        Some(end) if end <= array.len() => Ok(Sliced {
            array,
            offset,
            len,
        }),
        _ => Err(SliceError::RangeOutOfBounds {
            offset,
            len,
            array_len: array.len(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::PrimitiveBuilder;
    use alloc::string::ToString;
    use alloc::vec::Vec;
    // The crate is no_std; the test harness links std. Declared here so the
    // counting allocator below is available without gating the whole crate.
    extern crate std;

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    std::thread_local! {
        /// Counting window open? (per-thread: other test threads' traffic
        /// through this global allocator must not pollute the measurement).
        static COUNTING: Cell<bool> = const { Cell::new(false) };
        /// Allocations observed while `COUNTING` was open on this thread.
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }

    /// Real system allocator wrapped with a per-thread allocation counter.
    /// Forwards every call to `System` unchanged; only bookkeeping is added.
    struct CountingSystem;

    unsafe impl GlobalAlloc for CountingSystem {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if COUNTING.with(Cell::get) {
                ALLOCS.with(|a| a.set(a.get() + 1));
            }
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            if COUNTING.with(Cell::get) {
                ALLOCS.with(|a| a.set(a.get() + 1));
            }
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            if COUNTING.with(Cell::get) {
                ALLOCS.with(|a| a.set(a.get() + 1));
            }
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingSystem = CountingSystem;

    /// Run `f` with this thread's allocation counter open; return the count.
    fn measured<R>(f: impl FnOnce() -> R) -> (R, usize) {
        ALLOCS.with(|a| a.set(0));
        COUNTING.with(|c| c.set(true));
        let r = f();
        COUNTING.with(|c| c.set(false));
        let n = ALLOCS.with(Cell::get);
        (r, n)
    }

    /// Parent of `n` i64 rows: null exactly at absolute rows where
    /// `i % 7 == 3`, value `i` elsewhere.
    fn parent_with_nulls(n: usize) -> Primitive<i64> {
        let mut b = PrimitiveBuilder::<i64>::new();
        for i in 0..n {
            b.push(if i % 7 == 3 {
                None
            } else {
                Some(i as i64)
            });
        }
        b.finish()
    }

    /// Scalar ground truth recomputed straight from the parent: the expected
    /// `Option<i64>` for window row `i` of `[offset, offset + len)`.
    fn expect_row(arr: &Primitive<i64>, offset: usize, i: usize) -> Option<i64> {
        let abs = offset + i;
        if abs >= arr.len() {
            return None;
        }
        let valid = arr.validity().is_none_or(|bm| bm.get(abs));
        valid.then(|| arr.values()[abs])
    }

    #[test]
    fn slice_bounds_errors_are_typed_not_panics() {
        let arr = parent_with_nulls(10);
        // Plain overflow of the window.
        assert_eq!(
            slice(&arr, 5, 6),
            Err(SliceError::RangeOutOfBounds {
                offset: 5,
                len: 6,
                array_len: 10
            })
        );
        // usize::MAX saturates through checked_add: typed Err, no panic.
        assert_eq!(
            slice(&arr, usize::MAX, 1),
            Err(SliceError::RangeOutOfBounds {
                offset: usize::MAX,
                len: 1,
                array_len: 10
            })
        );
        // Exact-fit and empty windows at both edges are lawful.
        assert!(slice(&arr, 0, 10).is_ok());
        assert!(slice(&arr, 10, 0).is_ok());
        // Display renders the typed failure with the real numbers.
        let msg = slice(&arr, 5, 6)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        assert_eq!(
            msg,
            "slice range out of bounds: offset 5 + len 6 exceeds array_len 10"
        );
    }

    #[test]
    fn validity_and_values_coherence_vs_scalar_recompute() {
        // Misaligned window (offset 3, len 10) over a 1M-row parent with
        // nulls on a stride that puts nulls inside the window.
        let arr = parent_with_nulls(1_000_000);
        let (offset, len) = (3usize, 10usize);
        let s = slice(&arr, offset, len).expect("window fits");
        assert_eq!(s.len(), len);
        assert!(!s.is_empty());
        for i in 0..len {
            assert_eq!(s.get(i), expect_row(&arr, offset, i), "row {i}");
        }
        // iter() agrees with the per-row recompute, in order.
        let itered: Vec<Option<i64>> = s.iter().collect();
        let expected: Vec<Option<i64>> =
            (0..len).map(|i| expect_row(&arr, offset, i)).collect();
        assert_eq!(itered, expected);
        // Nulls and values both occur inside this window (proof both
        // branches of the coherence law were exercised).
        assert!(itered.iter().any(core::option::Option::is_none));
        assert!(itered.iter().any(core::option::Option::is_some));
    }

    #[test]
    fn empty_slice_when_offset_eq_len() {
        let arr = parent_with_nulls(10);
        // Window starting exactly at the end: offset == parent len, len 0.
        let tail = slice(&arr, 10, 0).expect("empty tail window");
        assert_eq!(tail.len(), 0);
        assert!(tail.is_empty());
        assert_eq!(tail.iter().count(), 0);
        assert_eq!(tail.get(0), None);
        // Mid-array zero-length window is likewise empty.
        let mid = slice(&arr, 4, 0).expect("empty mid window");
        assert!(mid.is_empty());
        assert_eq!(mid.iter().count(), 0);
    }

    #[test]
    fn misaligned_window_offset3_len10_reads_parent_cells() {
        let arr = parent_with_nulls(20);
        let s = slice(&arr, 3, 10).expect("window fits");
        assert_eq!(s.len(), 10);
        for i in 0..10 {
            let abs = 3 + i;
            // get(i) equals the parent cell it must alias, value and
            // validity alike.
            assert_eq!(s.get(i), expect_row(&arr, 3, i), "row {i}");
            // The aliased parent cell lives at base + (offset + i) * WIDTH:
            // the exact address arithmetic a copy-free read performs.
            assert_eq!(
                &arr.values()[abs] as *const i64,
                unsafe { arr.values_ptr().add(abs * core::mem::size_of::<i64>()) }
                    as *const i64
            );
        }
        // Absolute row 3 is the parent's first null (3 % 7 == 3): the
        // window's logical row 0 must surface it as None, not Some(3).
        assert_eq!(s.get(0), None);
        // Absolute rows 4..10 are valid: logical rows 1..7 carry values.
        assert_eq!(s.get(1), Some(4));
        assert_eq!(s.get(4), Some(7));
        // Absolute row 10 is null again (10 % 7 == 3): logical row 7.
        assert_eq!(s.get(7), None);
        assert_eq!(s.get(8), Some(11));
    }

    #[test]
    fn zero_copy_1000_slices_of_1m_rows_allocate_zero() {
        let arr = parent_with_nulls(1_000_000);
        assert_eq!(arr.len(), 1_000_000);
        // The window is O(1) in row count: one reference + two usizes.
        assert_eq!(
            core::mem::size_of::<Sliced<i64>>(),
            core::mem::size_of::<&Primitive<i64>>() + 2 * core::mem::size_of::<usize>()
        );
        let (checksum, allocs) = measured(|| {
            let mut some_sum: i64 = 0;
            let mut some_count: u64 = 0;
            for k in 0..1000usize {
                // Deterministic misaligned windows sweeping the parent.
                let offset = (k * 997) % (1_000_000 - 100);
                let s = match slice(&arr, offset, 100) {
                    Ok(s) => s,
                    Err(e) => panic!("window k={k} must fit: {e}"),
                };
                for i in 0..100usize {
                    if let Some(v) = s.get(i) {
                        some_sum += v;
                        some_count += 1;
                    }
                }
            }
            (some_sum, some_count)
        });
        assert_eq!(allocs, 0, "slicing must allocate nothing");
        // The reads were real: recompute the same checksums uncounted.
        let (exp_sum, exp_count) = {
            let mut s_sum = 0i64;
            let mut s_count = 0u64;
            for k in 0..1000usize {
                let offset = (k * 997) % (1_000_000 - 100);
                for i in 0..100usize {
                    if let Some(v) = expect_row(&arr, offset, i) {
                        s_sum += v;
                        s_count += 1;
                    }
                }
            }
            (s_sum, s_count)
        };
        assert_eq!(checksum, (exp_sum, exp_count));
        assert!(exp_count > 0);
    }
}
