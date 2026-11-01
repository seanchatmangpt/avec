//! # cgap-arrow
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
//! Layout ground truth is generated (`generated_layout.rs`) from the cgap-pack
//! ontology via `ggen sync run`; the kernels here are the hand-written
//! irreducible residue.

#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "std")]
extern crate std;

// GATE-2 SEAL repair (v26.11.1): enacts the alloc-cfg gate the A01 lane
// doc reserves as the W1-3 freeze decision (A01 §2.3 "Per-module alloc
// gating | lib.rs is shared surface; alloc-cfg gate is a W1-3 freeze
// decision (A06/A11/A17)" and §2.5.4). On no-OS targets
// (`target_os = "none"`, e.g. riscv32imc-unknown-none-elf) the
// alloc-backed host facade is compiled out and the embedded surface is
// exactly the alloc-free, panic-free core: slab, bitmask, blake3 (the
// slab digest donor), chase_lev. `extern crate alloc` rides the same
// predicate — no kept module references alloc on those targets, and the
// §2.5.4 compile-blocker ("lib.rs unconditionally does `extern crate
// alloc`") is discharged with it.
#[cfg(not(target_os = "none"))]
extern crate alloc;

// Embedded (no-OS) surface: the alloc-free, panic-free core only.
// Everything else is the alloc-backed host facade, compiled out below.
pub mod bitmask;
pub mod blake3;
pub mod chase_lev;
pub mod slab;

#[cfg(not(target_os = "none"))]
pub mod batch;
#[cfg(not(target_os = "none"))]
pub mod bitmap;
#[cfg(not(target_os = "none"))]
pub mod branchless;
#[cfg(not(target_os = "none"))]
pub mod buffer;
#[cfg(not(target_os = "none"))]
pub mod c_abi;
#[cfg(not(target_os = "none"))]
pub mod c_abi_nested;
#[cfg(not(target_os = "none"))]
pub mod digest;
#[cfg(not(target_os = "none"))]
pub mod fingerprint;
#[cfg(not(target_os = "none"))]
pub mod foreign;
#[cfg(not(target_os = "none"))]
pub mod layout;
#[cfg(not(target_os = "none"))]
pub mod mmu_arena;
#[cfg(not(target_os = "none"))]
pub mod perfect_hash;
#[cfg(not(target_os = "none"))]
pub mod predicates;
#[cfg(not(target_os = "none"))]
pub mod primitive;
#[cfg(not(target_os = "none"))]
pub mod runend;
#[cfg(not(target_os = "none"))]
pub mod silicon;
#[cfg(not(target_os = "none"))]
pub mod slice;
#[cfg(not(target_os = "none"))]
pub mod timestamps;
#[cfg(not(target_os = "none"))]
pub mod transpose;
#[cfg(not(target_os = "none"))]
pub mod varlen;
#[cfg(not(target_os = "none"))]
pub mod views;

#[cfg(not(target_os = "none"))]
mod generated_layout;
mod generated_version;
pub use generated_version::VERSION_C;

/// Release version projected from the project graph (`ontology/cgap.ttl`).
pub const VERSION: &str = generated_version::VERSION;

/// Tests assert on real state (Chicago discipline); alloc is the only
/// collaborator and it is real.
#[cfg(not(target_os = "none"))]
pub use batch::{Batch, BatchBuilder, BatchError, Column, ColumnBuilder};
#[cfg(not(target_os = "none"))]
pub use buffer::Buf;
#[cfg(not(target_os = "none"))]
pub use c_abi::{export, ArrowArray, ArrowSchema, CGAP_FLAG_NULLABLE};
#[cfg(not(target_os = "none"))]
pub use foreign::{format_of, is_valid, primitive_view, ForeignError};
#[cfg(not(target_os = "none"))]
pub use primitive::{Primitive, PrimitiveBuilder};
#[cfg(not(target_os = "none"))]
pub use timestamps::{TimestampMicro, TimestampMilli, TimestampNano, TimestampSecond};
#[cfg(not(target_os = "none"))]
pub use varlen::{Varlen, VarlenBuilder};
#[cfg(not(target_os = "none"))]
pub use views::{ViewVec, ViewVecBuilder};

// v26.11.1 moonshot lanes (coordinator-scaffolded, additive only):
// slab + bitmask are declared in the embedded-surface block above.
