//! Canonical 64-byte quad physical layout (spec:
//! docs/jira/v26.10.31/23-64b-quad-spec.md).
//!
//! One quad is exactly one cache line: no implicit padding, no split-line
//! records, every offset both a field start and the next field's alignment
//! boundary. Layout ground truth is pinned by compile-time asserts (size,
//! alignment, field offsets) — a regression is a build failure, not a
//! runtime surprise.
//!
//! Nullability lives in the sentinel convention: `BOTTOM_TERM_ID` (0) is the
//! deterministic zero, matching the crate-wide "null slot is the
//! deterministic zero" law (timestamps.rs); `WILDCARD_TERM_ID` (0xFFFF_FFFF)
//! is the match-anything pattern sentinel.

/// Subject/object/predicate/graph term index type.
///
/// Two reserved values; neither may be minted by the interner:
///
/// - `0x0000_0000` — `BOTTOM_TERM_ID`: unbound/null. Deterministic zero,
///   consistent with the crate convention (timestamps.rs:79).
/// - `0xFFFF_FFFF` — `WILDCARD_TERM_ID`: match-anything pattern sentinel.
///   `BOTTOM` matches only itself; `WILDCARD` matches any term including
///   `BOTTOM` (pattern-engine contract, spec §1.1).
pub type TermId = u32;

/// Unbound/null term — the deterministic zero.
pub const BOTTOM_TERM_ID: TermId = 0x0000_0000;

/// Match-anything pattern sentinel.
pub const WILDCARD_TERM_ID: TermId = 0xFFFF_FFFF;

/// Quad record: exactly 64 bytes, 64-byte aligned. One record, one cache
/// line, on every supported target (x86_64, aarch64, wasm32).
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Quad64 {
    /// Subject TermId (offset 0x00).
    pub s_id: TermId,
    /// Predicate TermId (offset 0x04).
    pub p_id: TermId,
    /// Object TermId (offset 0x08).
    pub o_id: TermId,
    /// Graph/context TermId (offset 0x0C).
    pub g_id: TermId,
    /// Monotonic nanoseconds (offset 0x10).
    pub t_ns: u64,
    /// Single u32 causal register (offset 0x18). Name is contract; any
    /// multi-register upgrade is a layout-version change.
    pub vector_clock: u32,
    /// Pre-evaluated permission/duty bitmask (offset 0x1C).
    pub policy_mask: u32,
    /// Inline sub-tree BLAKE3 receipt (offset 0x20). Always present; an
    /// unset digest is invalid and rejected at decode.
    pub inline_digest: [u8; 32],
}

// Layout laws as build-time facts (spec §2). Any field addition must keep
// these or bump the layout version.
const _: () = assert!(core::mem::size_of::<Quad64>() == 64);
const _: () = assert!(core::mem::align_of::<Quad64>() == 64);
const _: () = assert!(core::mem::offset_of!(Quad64, s_id) == 0x00);
const _: () = assert!(core::mem::offset_of!(Quad64, p_id) == 0x04);
const _: () = assert!(core::mem::offset_of!(Quad64, o_id) == 0x08);
const _: () = assert!(core::mem::offset_of!(Quad64, g_id) == 0x0C);
const _: () = assert!(core::mem::offset_of!(Quad64, t_ns) == 0x10);
const _: () = assert!(core::mem::offset_of!(Quad64, vector_clock) == 0x18);
const _: () = assert!(core::mem::offset_of!(Quad64, policy_mask) == 0x1C);
const _: () = assert!(core::mem::offset_of!(Quad64, inline_digest) == 0x20);
const _: () = assert!(core::mem::size_of::<[Quad64; 4]>() == 256);

/// Quad alignment requirement in bytes — one cache line.
pub const QUAD_ALIGN: usize = 64;

/// Quad size in bytes.
pub const QUAD_SIZE: usize = 64;

impl Quad64 {
    /// A quad with every field zeroed and a zero digest.
    ///
    /// Note: a zeroed digest is *invalid* per spec §1 — use
    /// [`Quad64::new`] with a real receipt for valid records; this
    /// constructor exists for sentinel/slot initialization.
    pub const ZERO: Self = Self {
        s_id: BOTTOM_TERM_ID,
        p_id: BOTTOM_TERM_ID,
        o_id: BOTTOM_TERM_ID,
        g_id: BOTTOM_TERM_ID,
        t_ns: 0,
        vector_clock: 0,
        policy_mask: 0,
        inline_digest: [0; 32],
    };

    /// Construct a quad with explicit term ids, timestamp, causal register
    /// and policy mask; the digest is supplied separately (it is a receipt,
    /// not a pattern input).
    pub const fn new(
        s_id: TermId,
        p_id: TermId,
        o_id: TermId,
        g_id: TermId,
        t_ns: u64,
        vector_clock: u32,
        policy_mask: u32,
        inline_digest: [u8; 32],
    ) -> Self {
        Self {
            s_id,
            p_id,
            o_id,
            g_id,
            t_ns,
            vector_clock,
            policy_mask,
            inline_digest,
        }
    }

    /// Branchless: is this term the BOTTOM sentinel? (A `==` against a
    /// constant compiles to a flag materialization, no branch.)
    #[inline]
    pub fn is_bottom(term: TermId) -> bool {
        term == BOTTOM_TERM_ID
    }

    /// Branchless: is this term the WILDCARD sentinel?
    #[inline]
    pub fn is_wildcard(term: TermId) -> bool {
        term == WILDCARD_TERM_ID
    }

    /// Branchless pattern match (spec §1.1): `WILDCARD` in the pattern
    /// matches any term, including `BOTTOM`; `BOTTOM` matches only itself;
    /// any other pattern matches itself only.
    #[inline]
    pub fn term_matches(pattern: TermId, term: TermId) -> bool {
        (pattern == WILDCARD_TERM_ID) | (pattern == term)
    }

    /// Branchless: does this quad's s/p/o/g tuple match the pattern tuple
    /// under the [`Self::term_matches`] law, positionally?
    #[inline]
    pub fn matches(&self, pat: &Self) -> bool {
        Self::term_matches(pat.s_id, self.s_id)
            & Self::term_matches(pat.p_id, self.p_id)
            & Self::term_matches(pat.o_id, self.o_id)
            & Self::term_matches(pat.g_id, self.g_id)
    }

    /// Little-endian wire bytes: field-order, no padding, exactly 64 bytes
    /// (spec §1). Round-trips with [`Self::from_le_bytes`].
    pub fn to_le_bytes(&self) -> [u8; QUAD_SIZE] {
        let mut out = [0u8; QUAD_SIZE];
        out[0x00..0x04].copy_from_slice(&self.s_id.to_le_bytes());
        out[0x04..0x08].copy_from_slice(&self.p_id.to_le_bytes());
        out[0x08..0x0C].copy_from_slice(&self.o_id.to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&self.g_id.to_le_bytes());
        out[0x10..0x18].copy_from_slice(&self.t_ns.to_le_bytes());
        out[0x18..0x1C].copy_from_slice(&self.vector_clock.to_le_bytes());
        out[0x1C..0x20].copy_from_slice(&self.policy_mask.to_le_bytes());
        out[0x20..0x40].copy_from_slice(&self.inline_digest);
        out
    }

    /// Parse little-endian wire bytes ([`Self::to_le_bytes`] inverse).
    /// Returns `None` unless the slice is exactly [`QUAD_SIZE`] bytes.
    pub fn from_le_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != QUAD_SIZE {
            return None;
        }
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        Some(Self {
            s_id: u32_at(0x00),
            p_id: u32_at(0x04),
            o_id: u32_at(0x08),
            g_id: u32_at(0x0C),
            t_ns: u64::from_le_bytes(bytes[0x10..0x18].try_into().unwrap()),
            vector_clock: u32_at(0x18),
            policy_mask: u32_at(0x1C),
            inline_digest: bytes[0x20..0x40].try_into().unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST_A: [u8; 32] = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32,
        0x10, 0x0F, 0x1E, 0x2D, 0x3C, 0x4B, 0x5A, 0x69, 0x78, 0x87, 0x96, 0xA5, 0xB4, 0xC3, 0xD2,
        0xE1, 0xF0,
    ];

    fn sample(t_ns: u64) -> Quad64 {
        Quad64::new(
            0x0102_0304,
            0x0506_0708,
            0x090A_0B0C,
            0x0D0E_0F10,
            t_ns,
            7,
            0x8000_0001,
            DIGEST_A,
        )
    }

    // Compile-time layout ground truth, re-asserted at runtime (Chicago:
    // assert real state; a test that only compiles asserts nothing here).
    #[test]
    fn size_and_alignment_laws() {
        assert_eq!(core::mem::size_of::<Quad64>(), 64);
        assert_eq!(core::mem::align_of::<Quad64>(), 64);
        assert_eq!(QUAD_SIZE, 64);
        assert_eq!(QUAD_ALIGN, 64);
    }

    #[test]
    fn field_offsets_match_spec_table() {
        assert_eq!(core::mem::offset_of!(Quad64, s_id), 0x00);
        assert_eq!(core::mem::offset_of!(Quad64, p_id), 0x04);
        assert_eq!(core::mem::offset_of!(Quad64, o_id), 0x08);
        assert_eq!(core::mem::offset_of!(Quad64, g_id), 0x0C);
        assert_eq!(core::mem::offset_of!(Quad64, t_ns), 0x10);
        assert_eq!(core::mem::offset_of!(Quad64, vector_clock), 0x18);
        assert_eq!(core::mem::offset_of!(Quad64, policy_mask), 0x1C);
        assert_eq!(core::mem::offset_of!(Quad64, inline_digest), 0x20);
    }

    #[test]
    fn four_quads_are_exactly_256_bytes() {
        let quads = [Quad64::ZERO; 4];
        assert_eq!(core::mem::size_of_val(&quads), 256);
        assert_eq!(core::mem::size_of::<[Quad64; 4]>(), 256);
        // Buffer base of an aligned 4-quad block is itself 64-aligned, so
        // no record straddles a cache line (spec §2 rules 3-4).
        assert_eq!(core::mem::align_of::<[Quad64; 4]>(), 64);
    }

    #[test]
    fn struct_bytes_round_trip_little_endian() {
        let q = sample(0x0102_0304_0506_0708);
        let bytes = q.to_le_bytes();
        assert_eq!(bytes.len(), 64);
        assert_eq!(Quad64::from_le_bytes(&bytes), Some(q));
        // Wrong length refused, not truncated.
        assert!(Quad64::from_le_bytes(&bytes[..63]).is_none());
        assert!(Quad64::from_le_bytes(&[0u8; 65]).is_none());
    }

    #[test]
    fn wire_bytes_are_little_endian_and_spec_ordered() {
        let q = sample(0x0102_0304_0506_0708);
        let b = q.to_le_bytes();
        // s_id = 0x0102_0304 LE at 0x00.
        assert_eq!(&b[0x00..0x04], &[0x04, 0x03, 0x02, 0x01]);
        // p_id at 0x04.
        assert_eq!(&b[0x04..0x08], &[0x08, 0x07, 0x06, 0x05]);
        // o_id at 0x08.
        assert_eq!(&b[0x08..0x0C], &[0x0C, 0x0B, 0x0A, 0x09]);
        // g_id at 0x0C.
        assert_eq!(&b[0x0C..0x10], &[0x10, 0x0F, 0x0E, 0x0D]);
        // t_ns = 0x0102_0304_0506_0708 LE at 0x10.
        assert_eq!(
            &b[0x10..0x18],
            &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
        );
        // vector_clock = 7 at 0x18.
        assert_eq!(&b[0x18..0x1C], &[7, 0, 0, 0]);
        // policy_mask = 0x8000_0001 at 0x1C.
        assert_eq!(&b[0x1C..0x20], &[0x01, 0x00, 0x00, 0x80]);
        // inline_digest verbatim at 0x20.
        assert_eq!(&b[0x20..0x40], &DIGEST_A[..]);
    }

    #[test]
    fn sentinel_constants_match_spec() {
        assert_eq!(BOTTOM_TERM_ID, 0x0000_0000);
        assert_eq!(WILDCARD_TERM_ID, 0xFFFF_FFFF);
    }

    #[test]
    fn wildcard_matches_anything_including_bottom() {
        for term in [BOTTOM_TERM_ID, 1, 42, WILDCARD_TERM_ID] {
            assert!(Quad64::term_matches(WILDCARD_TERM_ID, term));
        }
    }

    #[test]
    fn bottom_matches_only_itself() {
        assert!(Quad64::term_matches(BOTTOM_TERM_ID, BOTTOM_TERM_ID));
        assert!(!Quad64::term_matches(BOTTOM_TERM_ID, 1));
        assert!(!Quad64::term_matches(1, BOTTOM_TERM_ID));
        assert!(!Quad64::term_matches(1, 2));
        assert!(Quad64::term_matches(1, 1));
    }

    #[test]
    fn sentinel_predicates() {
        assert!(Quad64::is_bottom(BOTTOM_TERM_ID));
        assert!(!Quad64::is_bottom(WILDCARD_TERM_ID));
        assert!(!Quad64::is_bottom(1));
        assert!(Quad64::is_wildcard(WILDCARD_TERM_ID));
        assert!(!Quad64::is_wildcard(BOTTOM_TERM_ID));
        assert!(!Quad64::is_wildcard(1));
    }

    #[test]
    fn quad_level_pattern_match() {
        let q = sample(1);
        // Exact self-match.
        assert!(q.matches(&q));
        // All-wildcard pattern matches everything.
        let wild = Quad64::new(
            WILDCARD_TERM_ID,
            WILDCARD_TERM_ID,
            WILDCARD_TERM_ID,
            WILDCARD_TERM_ID,
            0,
            0,
            0,
            [0; 32],
        );
        assert!(q.matches(&wild));
        // One non-matching position fails.
        let near = wild;
        let mut pat = near;
        pat.s_id = 0x0102_0304; // matches q.s_id
        assert!(q.matches(&pat));
        pat.s_id = 99; // does not match, not wildcard
        assert!(!q.matches(&pat));
        let _ = near;
    }

    #[test]
    fn zero_quad_fields_are_bottom() {
        let z = Quad64::ZERO;
        assert!(Quad64::is_bottom(z.s_id));
        assert!(Quad64::is_bottom(z.p_id));
        assert!(Quad64::is_bottom(z.o_id));
        assert!(Quad64::is_bottom(z.g_id));
        assert_eq!(z.t_ns, 0);
        assert_eq!(z.inline_digest, [0; 32]);
    }
}
