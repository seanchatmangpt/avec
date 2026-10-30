//! Zero-copy array slicing (wave-3 lane W3-SLICE owns this file).
//!
//! # Zero-copy contract
//!
//! A [`Sliced`] borrows its parent [`Primitive`]: it holds exactly one
//! reference plus two `usize`s (offset, len) — no buffer is copied,
//! duplicated, or reallocated at any point. `slice()` performs no heap
//! allocation (proven by the allocation-counting test below), and every
//! [`Sliced::get`] reads through to the parent's buffers by pointer
//! arithmetic (`values_ptr` base + `(offset + i) * T::WIDTH`); the validity
//! bit for logical row `i` is parent bit `offset + i` of the parent's
//! validity bitmap.

use crate::bitmap::Bitmap;
use crate::primitive::{Primitive, Scalar};

/// Typed failure of a [`slice`](slice) bounds check (no panics on bad input).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceError {
    /// `offset + len` exceeds the parent row count.
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
        match self {
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

/// Zero-copy window over a borrowed [`Primitive`].
///
/// Constructed via [`slice`] / [`Primitive::slice_window`]. See the [module
/// docs](self) for the zero-copy contract.
pub struct Sliced<'a, T: Scalar> {
    array: &'a Primitive<T>,
    offset: usize,
    len: usize,
}

impl<'a, T: Scalar> Sliced<'a, T> {
    /// Logical row count of the window.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when the window is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}