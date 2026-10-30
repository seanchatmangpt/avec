//! avec-parquet: Parquet persistence kernels with branch-free inner-loop
//! bodies; per-lane bounds checks and record dispatch are itemized control
//! flow (docs/BRANCH_REGISTER.md).
//!
//! Scope (charter v26.10.29, layer 3): vectorized bit-unpacking, SWAR
//! RLE/bit-packed hybrid decode, direct page-to-buffer decode. Fail-closed
//! on malformed pages: bounded slice arithmetic + typed error, never a panic
//! on external input.

#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod bitslice;
pub mod byteslice;
pub mod compiled_manifest;
pub mod dictionary;
pub mod encode_fast;
pub mod ffor;
pub mod packing;
pub mod oblivious;
pub mod page;
pub mod page_aligned;
pub mod packed_filter;
pub mod rle;
pub mod thrift;
pub mod unpack;
pub mod unpack_filter;

pub use rle::{rle_decode, rle_encode, ParquetError};
