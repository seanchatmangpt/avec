//! AoS -> SoA transpose for the 64-byte quad record
//! (spec: docs/jira/v26.10.31/23-64b-quad-spec.md Sections 1-4).
//!
//! The interleaved record is a local `#[repr(C, align(64))]` type mirroring
//! the spec'd 64-byte layout; a generated-layout quad type does not exist
//! yet (layout.rs lane), so this module is self-contained by design and the
//! type is the coordination point when the layout lane lands.
//!
//! Branchless-kernel discipline (doc 27 Sections 2-4): the transpose body
//! allocates every output column at exact capacity up front and then writes
//! through raw pointers in a single fixed-trip loop — no `push`/`extend`
//! growth edges, no bounds-checked indexing, no early exits. The only
//! branches remaining in the audited `.text` are (a) loop-latch back-edges
//! for the runtime trip count `len` and (b) the allocator's null-return
//! check feeding `handle_alloc_error` — both non-data-dependent edges,
//! documented remainder (see the audit receipt in the lane report).

use alloc::vec::Vec;
use core::ptr;

/// Interleaved 64-byte quad record: spec Section 1 field table as a
/// `#[repr(C, align(64))]` type. Size 64 / align 64 are conformance
/// assertions (spec Section 2, rule 1).
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quad64 {
    pub s_id: u32,
    pub p_id: u32,
    pub o_id: u32,
    pub g_id: u32,
    pub t_ns: u64,
    pub vector_clock: u32,
    pub policy_mask: u32,
    pub inline_digest: [u8; 32],
}

const _: () = {
    assert!(core::mem::size_of::<Quad64>() == 64);
    assert!(core::mem::align_of::<Quad64>() == 64);
};

/// Columnar projection of a quad stream (spec Section 3), all non-nullable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuadColumns {
    pub s_id: Vec<u32>,
    pub p_id: Vec<u32>,
    pub o_id: Vec<u32>,
    pub g_id: Vec<u32>,
    /// Monotonic nanoseconds, u64 little-endian on the wire (spec Sec. 5).
    pub t_ns: Vec<u64>,
    pub vector_clock: Vec<u32>,
    pub policy_mask: Vec<u32>,
    /// Concatenated inline digests; length is exactly `len * 32` bytes.
    pub digest: Vec<u8>,
}

/// Allocate a `Vec<T>` with capacity exactly `len` and length `len`, with
/// contents UNINITIALIZED. Every slot is written by the caller before any
/// read; all element types here are plain old data (no drop glue), so an
/// unwound partial fill has no drop obligations. This removes the
/// per-element growth/capacity branch chain that `push`/`extend` inlines
/// into the kernel body (the dominant banned-opcode source in the prior
/// revision).
// Allowed: deliberate uninitialized-reserve pattern — the Safety comment
// below records why (full write-before-read, T: Copy, no drop glue).
#[allow(clippy::uninit_vec)]
#[inline]
fn col<T: Copy>(len: usize) -> Vec<T> {
    let mut v = Vec::with_capacity(len);
    // Safety: capacity is exactly `len`; slots are written in full by the
    // caller before any read; T: Copy has no drop glue.
    unsafe { v.set_len(len) };
    v
}

impl QuadColumns {
    /// SoA -> AoS, the exact inverse of [`transpose`].
    pub fn untranspose(&self) -> Vec<Quad64> {
        let len = self.s_id.len();
        debug_assert_eq!(self.p_id.len(), len);
        debug_assert_eq!(self.o_id.len(), len);
        debug_assert_eq!(self.g_id.len(), len);
        debug_assert_eq!(self.t_ns.len(), len);
        debug_assert_eq!(self.vector_clock.len(), len);
        debug_assert_eq!(self.policy_mask.len(), len);
        debug_assert_eq!(self.digest.len(), len * 32);

        // Exact-capacity allocation + raw writes: no `vec![elem; len]`
        // (its monomorphized `extend_with` is an in-scope, branchy symbol).
        let mut out: Vec<Quad64> = col(len);
        let sp = self.s_id.as_ptr();
        let pp = self.p_id.as_ptr();
        let op = self.o_id.as_ptr();
        let gp = self.g_id.as_ptr();
        let tp = self.t_ns.as_ptr();
        let vp = self.vector_clock.as_ptr();
        let mp = self.policy_mask.as_ptr();
        let dp = self.digest.as_ptr();
        // Safety: every source pointer has exactly `len` live elements
        // (debug-asserted above); `out` has `len` writable slots.
        unsafe {
            let o = out.as_mut_ptr();
            for i in 0..len {
                let d = dp.add(i * 32);
                ptr::copy_nonoverlapping(d, (*o.add(i)).inline_digest.as_mut_ptr(), 32);
                let r = &mut *o.add(i);
                r.s_id = *sp.add(i);
                r.p_id = *pp.add(i);
                r.o_id = *op.add(i);
                r.g_id = *gp.add(i);
                r.t_ns = *tp.add(i);
                r.vector_clock = *vp.add(i);
                r.policy_mask = *mp.add(i);
            }
        }
        out
    }
}

/// AoS -> SoA transpose. Exact-capacity output columns plus a single
/// fixed-trip write loop over `len`; the loop latch is the only
/// len-derived control edge in the kernel body.
pub fn transpose(aos: &[Quad64]) -> QuadColumns {
    let len = aos.len();
    let mut out = QuadColumns {
        s_id: col(len),
        p_id: col(len),
        o_id: col(len),
        g_id: col(len),
        t_ns: col(len),
        vector_clock: col(len),
        policy_mask: col(len),
        digest: col(len * 32),
    };

    // Safety: `aos` has `len` readable records; every output column has
    // exactly `len` (`len * 32` for digest) writable slots.
    unsafe {
        let src = aos.as_ptr();
        let s = out.s_id.as_mut_ptr();
        let p = out.p_id.as_mut_ptr();
        let o = out.o_id.as_mut_ptr();
        let g = out.g_id.as_mut_ptr();
        let t = out.t_ns.as_mut_ptr();
        let v = out.vector_clock.as_mut_ptr();
        let m = out.policy_mask.as_mut_ptr();
        let d = out.digest.as_mut_ptr();
        for i in 0..len {
            let r = &*src.add(i);
            *s.add(i) = r.s_id;
            *p.add(i) = r.p_id;
            *o.add(i) = r.o_id;
            *g.add(i) = r.g_id;
            *t.add(i) = r.t_ns;
            *v.add(i) = r.vector_clock;
            *m.add(i) = r.policy_mask;
            // Digest is 8-byte aligned (offset 32 in a 64-byte record), so a
            // fixed 4x u64 copy replaces ptr::copy_nonoverlapping — the call
            // form inlines to a memcpy expansion whose remainder check emits
            // a branch (cbz); the fixed copy is straight-line.
            let dw = d.cast::<u64>().add(i * 4);
            let rw = (r as *const Quad64).cast::<u64>().add(4);
            *dw = *rw;
            *dw.add(1) = *rw.add(1);
            *dw.add(2) = *rw.add(2);
            *dw.add(3) = *rw.add(3);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random corpus; no external deps, fixed seed.
    fn corpus(len: usize) -> Vec<Quad64> {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        (0..len)
            .map(|_| Quad64 {
                s_id: next() as u32,
                p_id: (next() >> 32) as u32,
                o_id: next() as u32,
                g_id: (next() >> 32) as u32,
                t_ns: next(),
                vector_clock: next() as u32,
                policy_mask: (next() >> 32) as u32 & 0x0FFF_FFFF,
                inline_digest: core::array::from_fn(|b| (next() >> (b % 8)) as u8),
            })
            .collect()
    }

    #[test]
    fn size_and_align_are_64() {
        assert_eq!(core::mem::size_of::<Quad64>(), 64);
        assert_eq!(core::mem::align_of::<Quad64>(), 64);
    }

    #[test]
    fn round_trip_random_corpora() {
        for len in [0usize, 1, 2, 3, 4, 5, 7, 8, 13, 31, 64, 257] {
            let aos = corpus(len);
            let cols = transpose(&aos);
            assert_eq!(cols.s_id.len(), len);
            assert_eq!(cols.digest.len(), len * 32);
            assert_eq!(cols.untranspose(), aos);
        }
    }

    #[test]
    fn digest_buffer_len_is_len_times_32() {
        let cols = transpose(&corpus(37));
        assert_eq!(cols.digest.len(), 37 * 32);
        assert_eq!(cols.t_ns.len(), 37);
    }

    #[test]
    fn zero_length_yields_empty_columns() {
        let cols = transpose(&[]);
        assert!(cols.s_id.is_empty());
        assert!(cols.digest.is_empty());
        assert!(cols.untranspose().is_empty());
    }

    #[test]
    fn odd_length_remainder_is_correct() {
        let aos = corpus(9);
        let cols = transpose(&aos);
        assert_eq!(cols.s_id[8], aos[8].s_id);
        assert_eq!(cols.t_ns[8], aos[8].t_ns);
        assert_eq!(cols.untranspose(), aos);
    }
}
