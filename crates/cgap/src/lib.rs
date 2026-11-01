//! cgap umbrella facade: one dependency for the published cgap kernel crates.
//!
//! Re-exports the published surfaces verbatim — this crate carries no logic
//! of its own (zero extra abstraction over the kernels it names).
#![no_std]

pub use cgap_arrow;
pub use cgap_blake3;
pub use cgap_git;
pub use cgap_parquet;
