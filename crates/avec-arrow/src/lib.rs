//! # avec-arrow
//!
//! Freestanding Apache Arrow columnar substrate: branch-free kernel
//! inner-loop bodies; latches, dispatch and bounds checks are itemized
//! control flow (docs/BRANCH_REGISTER.md).
//!
//! - `no_std` + `alloc` only; zero external dependencies.
//! - Every buffer 64-byte aligned at allocation (AVX-512 / NEON invariant).
//! - Scan kernels: branch-free inner-loop bodies; latches/dispatch itemized
//!   by the branch audit gate over true callee extents.
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
pub mod c_abi_nested;
pub mod digest;
pub mod primitive;
pub mod runend;
pub mod silicon;
pub mod slice;
pub mod timestamps;
pub mod varlen;
pub mod views;

mod generated_layout;
mod generated_version;
pub use generated_version::VERSION_C;

/// Release version projected from the project graph (`ontology/avec.ttl`).
pub const VERSION: &str = generated_version::VERSION;

/// Tests assert on real state (Chicago discipline); alloc is the only
/// collaborator and it is real.
pub use buffer::Buf;
pub use c_abi::{export, ArrowArray, ArrowSchema, AVEC_FLAG_NULLABLE};
pub use primitive::{Primitive, PrimitiveBuilder};
pub use timestamps::{TimestampMicro, TimestampMilli, TimestampNano, TimestampSecond};
pub use varlen::{Varlen, VarlenBuilder};
pub use views::{ViewVec, ViewVecBuilder};
