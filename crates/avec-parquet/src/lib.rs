//! avec-parquet: branchless Parquet persistence kernels.
//!
//! Scope (charter v26.10.29, layer 3): vectorized bit-unpacking, SWAR
//! RLE/bit-packed hybrid decode, direct page-to-buffer decode. Fail-closed
//! on malformed pages: bounded slice arithmetic + typed error, never a panic
//! on external input.

#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod packing;
pub mod rle;
pub mod unpack;

pub use rle::{rle_decode, rle_encode, ParquetError};
