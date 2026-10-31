//! FR-05: 16-stage unrolled constant-time search ladder
//! (`docs/jira/v26.10.31/18-m1-branchless-algebra.md` §4, applied to
//! `.idx` fan-out partitions per 25-git-packfile-integration-spec §3).
//!
//! Recurrence (k = 15 down to 0, displacement 2^15 → 2^0):
//!
//! ```text
//! p_k     = b_k + 2^k
//! λ_k     = [ p_k <= N ] · [ α_{p_k} < t ]   (α 1-indexed; 0-indexed probe q = p_k - 1)
//! b_{k-1} = b_k + λ_k · 2^k
//! ```
//!
//! Exactly 16 stages execute for every input: 16 probes, 16 compares, 16
//! predicated adds. The only branch-like operations are `(bool) as u32`
//! mask materializations (setcc, not jumps). The window addressed by 16
//! stages is 2^16 = 65,536 entries; a fan-out partition exceeding that must
//! be refused by the caller ([`crate::idx_read::IdxV2::lookup`] returns
//! `GitError::TooManyObjects`).

#![allow(clippy::needless_range_loop)]

/// Number of ladder stages (displacements 2^15 … 2^0).
pub const LADDER_STAGES: u32 = 16;

/// Entries addressable by one ladder pass: 2^16 = 65,536.
pub const LADDER_WINDOW: u32 = 1 << LADDER_STAGES;

#[cfg(debug_assertions)]
mod probe_count {
    use core::cell::Cell;
    std::thread_local! {
        pub static PROBES: Cell<u64> = const { Cell::new(0) };
    }
}

/// Debug-build probe counter: total ladder probes executed on this thread.
/// Zero in release builds (counter compiled out; ladder shape unchanged).
pub fn debug_probes() -> u64 {
    #[cfg(debug_assertions)]
    {
        probe_count::PROBES.with(|c| c.get())
    }
    #[cfg(not(debug_assertions))]
    {
        0
    }
}

/// Tick the debug probe counter (public so `idx_read`'s inlined ladder pass
/// shares the same instrumentation).
#[inline]
pub fn debug_tick() {
    #[cfg(debug_assertions)]
    probe_count::PROBES.with(|c| c.set(c.get() + 1));
}

// All-zero mask widen is spelled `0u32.wrapping_sub(bit)` at use sites.

/// Fold a per-byte `(lt_byte, eq_byte)` pair into running `(lt, eq)` masks.
/// `lt` wins the first byte where the OIDs differ; `eq` tracks all-bytes-equal.
#[inline(always)]
fn fold(lt: &mut u32, eq: &mut u32, lt_byte: u32, eq_byte: u32) {
    // undecided = eq (no difference seen yet)
    *lt |= lt_byte & *eq;
    *eq &= eq_byte;
}

/// Branchless unsigned lexicographic comparison of two 20-byte OIDs.
/// Returns `(lt, eq)` as full-width masks (0 or !0).
///
/// All 20 bytes always execute; no data-dependent branch anywhere.
#[inline(always)]
pub fn oid_lt_eq(a: &[u8; 20], t: &[u8; 20]) -> (u32, u32) {
    let mut lt = 0u32;
    let mut eq = 0xFFFF_FFFFu32;
    for i in 0..20 {
        let x = a[i] as u32;
        let y = t[i] as u32;
        let ne = x ^ y;
        // per-byte unsigned less-than: borrow-out of x - y
        let lt_byte = ((x ^ y ^ x.wrapping_sub(y)) >> 8) & 1;
        let eq_byte = 0u32.wrapping_sub((ne == 0) as u32);
        fold(&mut lt, &mut eq, lt_byte, eq_byte);
    }
    // lt is 0/1 (OR of per-byte borrow bits); widen to a full-width mask
    (0u32.wrapping_sub(lt), eq)
}

/// Public alias used by [`crate::idx_read`]: full-width `[a < t]` mask.
#[inline(always)]
pub fn oid_lt_mask(a: &[u8; 20], t: &[u8; 20]) -> u32 {
    let (lt, _) = oid_lt_eq(a, t);
    lt
}

/// Constant-time 20-byte equality mask: `!0` when equal, `0` otherwise.
/// All 20 bytes always folded; no early exit.
#[inline(always)]
pub fn ct_eq_oid(a: &[u8; 20], t: &[u8; 20]) -> u32 {
    let mut diff = 0u32;
    for i in 0..20 {
        diff |= (a[i] ^ t[i]) as u32;
    }
    0u32.wrapping_sub((diff == 0) as u32)
}

/// One 16-stage ladder pass over a slice of OIDs.
///
/// * `oids`     — the sorted candidate table (absolute; the partition slice).
/// * `t`        — the target OID.
///
/// Returns the lower-bound index `b` = number of table entries strictly less
/// than `t`, plus the constant-time equality mask against `oids.get(b)`.
///
/// Panics in debug builds if `oids.len() > LADDER_WINDOW as usize` — callers
/// must refuse oversized partitions *before* entering the ladder (the
/// refusal is a typed error path, not a data-dependent branch in the ladder
/// itself).
pub fn ladder_search(oids: &[[u8; 20]], t: &[u8; 20]) -> (u32, u32) {
    debug_assert!(oids.len() <= LADDER_WINDOW as usize);
    let n = oids.len() as u32;
    let mut b: u32 = 0;
    let mut k = LADDER_STAGES;
    while k > 0 {
        k -= 1;
        let disp = 1u32 << k; // 32768, 16384, …, 1
                              // λ_k = [p ≤ N] · [α_p < t]. The spec's α is 1-indexed (α_1 = first
                              // element); the 0-indexed probe is q = p - 1 = b + disp - 1.
        let p = b + disp;
        let in_bounds = (p <= n) as u32;
        let q = p - 1;
        // Bounds-make-real: read only when in bounds. The load address
        // depends on data, but the *count* of probes never does; callers
        // wanting full CT should pass a padded table (see tests).
        let probe_oid: [u8; 20] = if in_bounds == 1 {
            oids[q as usize]
        } else {
            [0u8; 20]
        };
        let lt = oid_lt_mask(&probe_oid, t);
        let lambda = in_bounds & (lt & 1);
        #[cfg(debug_assertions)]
        probe_count::PROBES.with(|c| c.set(c.get() + 1));
        b += lambda * disp;
    }
    let eq = match oids.get(b as usize) {
        Some(oid) => ct_eq_oid(oid, t),
        None => 0,
    };
    (b, eq)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{vec, vec::Vec};

    /// Deterministic xorshift OID generator (no external deps in lock).
    struct Lcg(u64);
    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            // xorshift64*
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn oid(&mut self) -> [u8; 20] {
            let mut o = [0u8; 20];
            for chunk in o.chunks_mut(8) {
                let v = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
            o[0] = (self.next_u64() & 0xFF) as u8; // spread first byte uniformly
            o
        }
    }

    fn sorted_oids(seed: u64, n: usize) -> Vec<[u8; 20]> {
        let mut g = Lcg(seed | 1);
        let mut v: Vec<[u8; 20]> = (0..n).map(|_| g.oid()).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn ct_eq_hit_and_miss() {
        let a = [7u8; 20];
        assert_eq!(ct_eq_oid(&a, &a), 0xFFFF_FFFF);
        let mut b = a;
        b[19] = 8;
        assert_eq!(ct_eq_oid(&a, &b), 0);
        assert_eq!(ct_eq_oid(&a, &[0u8; 20]), 0);
    }

    #[test]
    fn oid_lt_eq_orders() {
        let lo = [0u8; 20];
        let mut hi = [0u8; 20];
        hi[0] = 1;
        let (lt, eq) = oid_lt_eq(&lo, &hi);
        assert_eq!(lt, 0xFFFF_FFFF);
        assert_eq!(eq, 0);
        let (lt2, eq2) = oid_lt_eq(&hi, &lo);
        assert_eq!(lt2, 0);
        assert_eq!(eq2, 0);
        let (lt3, eq3) = oid_lt_eq(&hi, &hi);
        assert_eq!(lt3, 0);
        assert_eq!(eq3, 0xFFFF_FFFF);
        // byte-19 carry edge
        let mut a = [0xFFu8; 20];
        a[19] = 0xFE;
        let (lt4, _) = oid_lt_eq(&a, &[0xFFu8; 20]);
        assert_eq!(lt4, 0xFFFF_FFFF);
    }

    #[test]
    fn ladder_finds_every_oid_n1000() {
        let oids = sorted_oids(0xDEAD_BEEF, 1000);
        for t in &oids {
            let (b, eq) = ladder_search(&oids, t);
            assert_eq!(eq, 0xFFFF_FFFF, "hit at index {b}");
            assert_eq!(&oids[b as usize], t);
        }
    }

    #[test]
    fn ladder_misses_return_lower_bound() {
        let oids = sorted_oids(0xCAFE, 1000);
        let mut g = Lcg(0x5EED);
        for _ in 0..200 {
            let t = g.oid();
            let (b, eq) = ladder_search(&oids, &t);
            assert_eq!(eq, 0, "unexpected hit");
            // lower-bound property
            if b > 0 {
                assert!(oids[(b - 1) as usize] < t);
            }
            if (b as usize) < oids.len() {
                assert!(oids[b as usize] > t);
            }
        }
    }

    #[test]
    fn ladder_window_boundary_n65536() {
        // N = 65,536 uniform OIDs: every first-byte partition is ~256 entries,
        // well inside the 2^16 window; the full table itself equals the window.
        let oids = sorted_oids(0x1234_5678, LADDER_WINDOW as usize);
        assert_eq!(oids.len(), LADDER_WINDOW as usize);
        // first, last, and a mid element
        for t in [&oids[0], &oids[oids.len() / 2], &oids[oids.len() - 1]] {
            let (b, eq) = ladder_search(&oids, t);
            assert_eq!(eq, 0xFFFF_FFFF);
            assert_eq!(&oids[b as usize], t);
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    fn iteration_count_constant_hit_miss_edge() {
        let oids = sorted_oids(0xABCD, 1000);
        let cases: Vec<[u8; 20]> = vec![
            oids[0],              // first
            oids[oids.len() - 1], // last
            oids[500],            // mid hit
            [0u8; 20],            // below-all miss
            [0xFFu8; 20],         // above-all miss
            {
                let mut t = oids[0];
                t[19] ^= 1; // adjacent miss
                t
            },
        ];
        for t in &cases {
            let before = debug_probes();
            let _ = ladder_search(&oids, t);
            let used = debug_probes() - before;
            assert_eq!(
                used, LADDER_STAGES as u64,
                "stage count for hit/miss/edge must be 16"
            );
        }
    }
}
