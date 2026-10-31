//! Compile-only no_std portability court for the avec-arrow core primitives.
//!
//! Two registers of assertion:
//!
//! 1. **Const (compiler-decided)**: the 64-byte quad layout facts and the
//!    sentinel constants — size/align/sentinel values are facts about the
//!    type surface, decidable without a running process.
//! 2. **Runtime (host test binary)**: the `avec_arrow::predicates`
//!    primitives are plain `#[inline(always)]` fns (not `const fn` — that is
//!    upstream's choice, not this test's), so their correctness is asserted
//!    in a `#[test]`. They remain no_std-compatible: the same code compiles
//!    for wasm32 (see NOTES-v26.10.31.md receipts).
//!
//! Crates covered: `avec_arrow::predicates` (CSETM/CMOV-class masks, spec
//! docs/jira/v26.10.31/27-branchless-kernel-abi.md), `avec_arrow::layout`
//! (docs/jira/v26.10.31/23-64b-quad-spec.md), `avec_arrow::transpose`.
//!
//! The stronger per-target gate is an actual
//! `cargo build --target wasm32-unknown-unknown` of the core crates — see
//! NOTES-v26.10.31.md for the receipts.

#![no_std]

use avec_arrow::{
    layout::{Quad64, BOTTOM_TERM_ID, QUAD_ALIGN, QUAD_SIZE, WILDCARD_TERM_ID},
    predicates::{
        ct_eq_u32, ct_eq_u64_mask, ct_lt_u32, ct_quad_eq_u32, ct_select_u32, ct_select_u64,
        ct_select_usize,
    },
    transpose::QuadColumns,
};
use avec_blake3::multi_chunk::{compress_chunks_4way, compress_chunks_4way_x2};

// --- Const register: layout shape + sentinels (compiler-decided) ---

const _: () = assert!(core::mem::size_of::<Quad64>() == QUAD_SIZE);
const _: () = assert!(core::mem::align_of::<Quad64>() == 64);
const _: () = assert!(QUAD_SIZE == 64 && QUAD_ALIGN == 64);
const _: () = assert!(BOTTOM_TERM_ID == 0);
const _: () = assert!(WILDCARD_TERM_ID == u32::MAX);

// --- Runtime register: predicates behave, no_std types nameable ---

#[test]
fn predicates_masks() {
    // ct_eq_u32: all-ones iff equal (CSETM form).
    assert_eq!(ct_eq_u32(7, 7), u32::MAX);
    assert_eq!(ct_eq_u32(7, 8), 0);

    // ct_lt_u32: unsigned compare, all-ones iff strictly less.
    assert_eq!(ct_lt_u32(1, 2), u32::MAX);
    assert_eq!(ct_lt_u32(2, 1), 0);
    assert_eq!(ct_lt_u32(2, 2), 0);

    // Two-way select, both polarities.
    assert_eq!(ct_select_u32(u32::MAX, 10, 20), 10);
    assert_ct_select_polarities();

    // u64 forms exist and behave.
    assert_eq!(ct_select_u64(u64::MAX, 1, 2), 1);
    assert_eq!(ct_eq_u64_mask(9, 9), u64::MAX);
}

#[inline]
fn assert_ct_select_polarities() {
    assert_eq!(ct_select_u32(0, 10, 20), 20);
}

#[test]
fn quad_layout_surface() {
    // Quad64::new / ZERO honor the sentinel convention; fields nameable
    // from a no_std-adjacent consumer.
    let q = Quad64::new(1, 2, 3, 4, 5, 6, 7, [0u8; 32]);
    assert_eq!(q.s_id, 1);
    assert_eq!(q.inline_digest.len(), 32);
    assert_eq!(Quad64::ZERO.s_id, BOTTOM_TERM_ID);
    assert!(Quad64::is_bottom(BOTTOM_TERM_ID));
    assert!(Quad64::is_wildcard(WILDCARD_TERM_ID));
    assert!(!Quad64::is_bottom(WILDCARD_TERM_ID));
}

#[test]
fn transpose_surface_nameable() {
    // The AoS->SoA transpose surface is nameable from a no_std consumer;
    // the behavioral courts live in avec-arrow's own test suite (lane I15).
    assert!(core::mem::size_of::<QuadColumns>() > 0);
}

#[test]
fn wave4_new_surfaces_nameable_no_std() {
    // Wave-4 additions must remain nameable from a no_std consumer.

    // predicates: new ct helpers from the predicates rework lane.
    // ct_quad_eq_u32 is a quad-WIDE verdict (all-ones iff all four lanes
    // match), not a per-lane packed mask.
    assert_eq!(ct_quad_eq_u32([1, 2, 3, 4], [1, 2, 3, 4]), u32::MAX);
    assert_eq!(ct_quad_eq_u32([1, 2, 3, 4], [1, 2, 3, 5]), 0);
    assert_eq!(ct_select_usize(u32::MAX as usize, 7, 9), 7);
    assert_eq!(ct_select_usize(0, 7, 9), 9);

    // avec-blake3: multi_chunk (lane 5) public fn nameable; behavioral
    // courts live in avec-blake3's own suite.
    // compress_chunks_4way operates on full 1024-byte blocks (asserts a
    // 4096-byte input — one BLAKE3 chunk of 4 blocks).
    let outs = compress_chunks_4way(&[0u8; 4096], 0);
    assert_eq!(outs.len(), 4);
    assert!(outs.iter().all(|s| s.len() == 16));

    // NOT landed: `hash_parallel` — no such pub symbol exists in
    // avec-blake3 this session (grep receipt in NOTES).
}

#[test]
fn wave6_multi_chunk_x2_nameable_no_std() {
    // Wave-6 lanes 1-2 added `compress_chunks_4way_x2` (NEON-gated on
    // aarch64, scalar elsewhere). It must stay nameable AND behave from a
    // `#![no_std]` consumer — proving the NEON gating does not leak any
    // std/arch assumption into the portable surface.
    let data = [0u8; 8192]; // 2 groups x 1 chunk of 4 blocks.
    let outs = compress_chunks_4way_x2(&data, 0);

    // Consistency: x2 == two flattened compress_chunks_4way calls.
    let a = compress_chunks_4way(&data[0..4096], 0);
    let b = compress_chunks_4way(&data[4096..8192], 4);
    assert_eq!(outs[0], a);
    assert_eq!(outs[1], b);
}
