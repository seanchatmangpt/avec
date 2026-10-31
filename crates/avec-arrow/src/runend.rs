//! Run-end encoded array support (wave-3 lane W3-REE owns this file).
//!
//! Arrow "+r" layout (ontology `av:RunEndEncoded`): a logical vector stored as
//! two physical children — `run_ends` (i32, exclusive ends, strictly
//! increasing) and `values`. Equal consecutive logical rows collapse into one
//! run, so sorted/rLE-shaped data stores `O(runs)` cells for `O(rows)` rows.
//!
//! Freestanding `no_std` + `alloc`: the only collaborator is the real global
//! allocator (Chicago discipline — no doubles possible or needed).

use crate::buffer::Buf;
use crate::primitive::{Primitive, PrimitiveBuilder, Scalar};
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Arrow format string for the run-end encoded layout.
pub const RUN_END_FORMAT_CODE: &str = "+r";

/// Frozen run-end encoded vector (Arrow "+r").
///
/// The `I` parameter is carried for the layout's index-width seam (future
/// run-end widths); the current children are fixed: i32 run ends in an
/// aligned [`Buf`], an all-valid i32 [`Primitive`] values child (Arrow law:
/// null_count == 0 -> no validity buffer).
///
/// # Shape laws
/// - `run_ends` is strictly increasing, positive, and `run_ends.last() ==
///   logical_len`.
/// - Row `i` belongs to run `r` iff `run_ends[r-1] <= i < run_ends[r]`
///   (with `run_ends[-1] = 0`).
pub struct RunEndEncoded<I: Scalar> {
    run_ends: Buf,
    values: Primitive<i32>,
    logical_len: usize,
    _marker: PhantomData<fn() -> I>,
}

impl<I: Scalar> RunEndEncoded<I> {
    pub(crate) fn from_parts(run_ends: Buf, values: Primitive<i32>, logical_len: usize) -> Self {
        debug_assert_eq!(
            run_ends.as_typed::<i32>().last().copied().unwrap_or(0),
            logical_len as i32,
            "shape law: last run end must equal logical length"
        );
        RunEndEncoded {
            run_ends,
            values,
            logical_len,
            _marker: PhantomData,
        }
    }

    /// Arrow format code of this layout ("+r").
    #[must_use]
    pub const fn format_code() -> &'static str {
        RUN_END_FORMAT_CODE
    }

    /// Total logical row count (what a dense decode produces).
    #[must_use]
    pub fn logical_len(&self) -> usize {
        self.logical_len
    }

    /// True when the vector has zero logical rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.logical_len == 0
    }

    /// Physical run count (length of the run_ends / values children).
    #[must_use]
    pub fn num_runs(&self) -> usize {
        self.run_ends().len()
    }

    /// Exclusive run ends, strictly increasing (`run_ends[r]` = first row
    /// index NOT in run `r`).
    #[must_use]
    pub fn run_ends(&self) -> &[i32] {
        self.run_ends.as_typed::<i32>()
    }

    /// Values child: exactly one i32 per run.
    #[must_use]
    pub fn values(&self) -> &Primitive<i32> {
        &self.values
    }

    /// Charter M-03 proof: run_ends buffer and values-child buffers are all
    /// 64-byte aligned.
    #[must_use]
    pub fn buffers_aligned_64(&self) -> bool {
        (self.run_ends.as_ptr() as usize).is_multiple_of(crate::buffer::ALIGNMENT)
            && self.values.buffers_aligned_64()
    }

    /// Resolve logical row `i` to its value via binary search over the run
    /// ends (`partition_point` — one probe path, no per-iteration branches
    /// beyond the standard midpoint test).
    ///
    /// # Panics
    /// When `i >= logical_len` (loud, typed message — never a wrong value).
    #[must_use]
    pub fn get(&self, i: usize) -> i32 {
        assert!(
            i < self.logical_len,
            "run-end get: row {i} out of range for logical_len {}",
            self.logical_len
        );
        let ends = self.run_ends();
        // First run whose exclusive end exceeds i: row i lives exactly there.
        let run = ends.partition_point(|&end| (end as usize) <= i);
        self.values.values()[run]
    }

    /// Dense decode: expand every run into consecutive rows of `dst`.
    ///
    /// # Panics
    /// When `dst.len() != logical_len` — a partially-addressable decode is a
    /// caller bug, not a truncation.
    pub fn decode_to_dense(&self, dst: &mut [i32]) {
        assert_eq!(
            dst.len(),
            self.logical_len,
            "run-end decode: destination must hold exactly logical_len rows"
        );
        let ends = self.run_ends();
        let vals = self.values.values();
        let mut row = 0;
        for (run, &end) in ends.iter().enumerate() {
            let end = end as usize;
            while row < end {
                dst[row] = vals[run];
                row += 1;
            }
        }
    }
}

/// Builder producing a frozen [`RunEndEncoded`]. `push` auto-merges equal
/// consecutive values into the open run; any value change closes it.
pub struct RunEndEncodedBuilder<I: Scalar> {
    run_ends: Vec<i32>,
    values: Vec<i32>,
    logical_len: usize,
    _marker: PhantomData<fn() -> I>,
}

impl<I: Scalar> Default for RunEndEncodedBuilder<I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Scalar> RunEndEncodedBuilder<I> {
    #[must_use]
    pub fn new() -> Self {
        RunEndEncodedBuilder {
            run_ends: Vec::new(),
            values: Vec::new(),
            logical_len: 0,
            _marker: PhantomData,
        }
    }

    /// Append one logical row, extending the open run when the value equals
    /// the last pushed value, opening a new run otherwise.
    pub fn push(&mut self, v: i32) {
        match self.values.last().copied() {
            Some(last) if last == v => {
                // Open run extends by one row; run_ends.last() is Some
                // whenever values is non-empty (they grow in lockstep).
                if let Some(end) = self.run_ends.last_mut() {
                    *end += 1;
                }
            }
            _ => {
                self.run_ends.push(self.logical_len as i32 + 1);
                self.values.push(v);
            }
        }
        self.logical_len += 1;
    }

    /// Rows appended so far (logical, not physical).
    #[must_use]
    pub fn len(&self) -> usize {
        self.logical_len
    }

    /// True when no rows appended yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.logical_len == 0
    }

    /// Freeze into the aligned vector. Re-encodes from the already-merged run
    /// lists; repeatable (does not consume the builder).
    #[must_use]
    pub fn finish(&self) -> RunEndEncoded<I> {
        let n = self.run_ends.len();
        // Zero-fill first (write-before-read), then copy run ends in LE.
        let mut run_ends = Buf::new_zeroed(n * core::mem::size_of::<i32>());
        let bytes = run_ends.as_mut_slice();
        let mut r = 0;
        while r < n {
            self.run_ends[r].write_le(&mut bytes[r * 4..(r + 1) * 4]);
            r += 1;
        }
        let mut values = PrimitiveBuilder::<i32>::new();
        let mut v = 0;
        while v < self.values.len() {
            values.push(Some(self.values[v]));
            v += 1;
        }
        RunEndEncoded::from_parts(run_ends, values.finish(), self.logical_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a dense slice through the builder (real path, no test double).
    fn encode(dense: &[i32]) -> RunEndEncoded<i32> {
        let mut b = RunEndEncodedBuilder::<i32>::new();
        for &x in dense {
            b.push(x);
        }
        b.finish()
    }

    /// Dense decode through the public API.
    fn decode(arr: &RunEndEncoded<i32>) -> Vec<i32> {
        let mut out = alloc::vec![0; arr.logical_len()];
        arr.decode_to_dense(&mut out);
        out
    }

    #[test]
    fn round_trip_encode_decode() {
        let dense = [7, 7, 7, 2, 2, 9];
        let arr = encode(&dense);
        assert_eq!(arr.logical_len(), 6);
        assert_eq!(arr.num_runs(), 3);
        assert_eq!(arr.run_ends(), &[3, 5, 6]);
        assert_eq!(arr.values().values(), &[7, 2, 9]);
        assert_eq!(decode(&arr), dense.to_vec());
        // Per-row accessor agrees with the dense decode at every index.
        for (i, &x) in dense.iter().enumerate() {
            assert_eq!(arr.get(i), x, "row {i}");
        }
    }

    #[test]
    fn equal_neighbors_merge_boundaries_at_run_edges() {
        let arr = encode(&[1, 1, 1, 2, 2, 1]);
        // Adjacent equal values merge; a recurring value after a change does
        // NOT merge across the intervening run (boundary law).
        assert_eq!(arr.num_runs(), 3);
        assert_eq!(arr.run_ends(), &[3, 5, 6]);
        assert_eq!(arr.values().values(), &[1, 2, 1]);
        // Exact boundaries: last row of a run vs first row of the next.
        assert_eq!(arr.get(2), 1);
        assert_eq!(arr.get(3), 2);
        assert_eq!(arr.get(4), 2);
        assert_eq!(arr.get(5), 1);
        assert_eq!(decode(&arr), alloc::vec![1, 1, 1, 2, 2, 1]);
    }

    #[test]
    fn single_run_extreme() {
        let arr = encode(&[5; 10]);
        assert_eq!(arr.num_runs(), 1);
        assert_eq!(arr.run_ends(), &[10]);
        assert_eq!(arr.logical_len(), 10);
        assert_eq!(arr.values().values(), &[5]);
        for i in 0..10 {
            assert_eq!(arr.get(i), 5);
        }
        assert_eq!(decode(&arr), alloc::vec![5; 10]);
    }

    #[test]
    fn all_distinct_extreme() {
        let dense: Vec<i32> = (0..8).collect();
        let arr = encode(&dense);
        assert_eq!(arr.num_runs(), 8);
        assert_eq!(arr.run_ends(), &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(decode(&arr), dense);
        // DE-generate compression: physical == logical, never lossy.
        assert_eq!(arr.num_runs(), arr.logical_len());
    }

    #[test]
    fn empty_round_trip() {
        let arr = encode(&[]);
        assert!(arr.is_empty());
        assert_eq!(arr.logical_len(), 0);
        assert_eq!(arr.num_runs(), 0);
        assert!(arr.run_ends().is_empty());
        assert!(arr.values().is_empty());
        assert!(arr.values().validity().is_none());
        let mut empty_dst: [i32; 0] = [];
        arr.decode_to_dense(&mut empty_dst);
        assert!(decode(&arr).is_empty());
        // Shape laws hold on the empty vector.
        assert!(arr.buffers_aligned_64());
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn get_beyond_logical_len_panics_loudly() {
        // The panic is the assertion; the must_use return is intentionally
        // discarded here.
        let _ = encode(&[1, 1, 2]).get(3);
    }

    #[test]
    #[should_panic(expected = "exactly logical_len")]
    fn decode_wrong_size_panics_loudly() {
        encode(&[1, 2]).decode_to_dense(&mut [0; 5]);
    }

    #[test]
    fn alignment_law_holds_via_buf_api() {
        let arr = encode(&[9, 9, 9, 4]);
        // Direct Buf-API assertion on the run_ends buffer...
        let ends_view = arr.run_ends();
        let ends_ptr = ends_view.as_ptr();
        assert_eq!(ends_ptr as usize % 64, 0, "run_ends buffer misaligned");
        assert_eq!(ends_view, &[3, 4]);
        // ...and on the values child (values + validity).
        assert!((arr.values().values_ptr() as usize).is_multiple_of(64));
        assert!(arr.values().buffers_aligned_64());
        assert!(arr.buffers_aligned_64());
        // Larger, builder-heavy case keeps the law (allocation path varies).
        let mut b = RunEndEncodedBuilder::<i32>::new();
        for i in 0..1000 {
            b.push(i / 7);
        }
        let arr = b.finish();
        assert_eq!(arr.num_runs(), 143);
        assert!(arr.buffers_aligned_64());
        assert_eq!(arr.run_ends().as_ptr() as usize % 64, 0);
        // Spot-check the merge under churn: values 0..=141 own seven rows
        // each (ends 7..994), value 142 owns the final six (end 1000).
        assert_eq!(arr.run_ends()[141], 994);
        assert_eq!(arr.run_ends()[142], 1000);
        assert_eq!(arr.get(999), 142);
    }

    #[test]
    fn format_code_is_plus_r() {
        assert_eq!(RunEndEncoded::<i32>::format_code(), "+r");
        assert_eq!(RUN_END_FORMAT_CODE, "+r");
    }
}
