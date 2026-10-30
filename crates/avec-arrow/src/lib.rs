//! # avec-core
//!
//! Freestanding, branchless Apache Arrow columnar substrate.
//!
//! - `no_std` + `alloc` only; zero external dependencies.
//! - Every buffer 64-byte aligned at allocation (AVX-512 / NEON invariant).
//! - Scan kernels compile to zero conditional branches (branch audit gate).
//! - C Data Interface (`ArrowArray` / `ArrowSchema`) zero-copy export/import
//!   with atomic release-swap double-release control.
//!
//! Layout ground truth is generated (`generated_layout.rs`) from the avec-pack
//! ontology via `ggen sync run`; the kernels here are the hand-written
//! irreducible residue.

#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

pub mod bitmap;
pub mod buffer;
pub mod c_abi;
pub mod primitive;
pub mod silicon;
pub mod varlen;
pub mod views;

mod generated_layout;
mod generated_version;

/// Release version projected from the project graph (`ontology/avec.ttl`).
pub const VERSION: &str = generated_version::VERSION;

/// Tests assert on real state (Chicago discipline); alloc is the only
/// collaborator and it is real.
pub use buffer::Buf;
pub use c_abi::{export, ArrowArray, ArrowSchema, AVEC_FLAG_NULLABLE};
pub use primitive::{Primitive, PrimitiveBuilder};
pub use varlen::{Varlen, VarlenBuilder};
pub use views::{ViewVec, ViewVecBuilder};
