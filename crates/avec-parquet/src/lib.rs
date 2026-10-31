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

// The `std` feature links std for the hosted-target reader (`scan`) and the
// `std::error::Error` impls. Without this, the feature flag was decorative:
// `impl std::error::Error` under the gate failed to resolve (E0433).
#[cfg(feature = "std")]
extern crate std;

pub mod bitslice;
pub mod byteslice;
pub mod compiled_manifest;
pub mod dictionary;
pub mod dispatch_table;
pub mod encode_fast;
pub mod ffor;
pub mod level_free;
pub mod oblivious;
pub mod packed_filter;
pub mod packing;
pub mod page;
pub mod page_aligned;
pub mod rle;
// Scan is the hosted-target (std) reader; gated per the crate's std-feature
// pattern so the no_std build (wasm-footprint link glue, wasm32 targets)
// stays std-free. Compile-freeze unblock by lane I16 — see
// crates/wasm-footprint/NOTES-v26.10.31.md; scan tests run with
// `cargo test -p avec-parquet --features std`.
#[cfg(feature = "std")]
pub mod scan;
pub mod stream_compact;
pub mod thrift;
pub mod unpack;
pub mod unpack_filter;

pub use rle::{rle_decode, rle_encode, ParquetError};
