//! BN3 term-word ABI (Wave 1 freeze): the `Sigil` kind tags and the
//! 24-byte `BN3Statement` (spec: `docs/jira/v26.11.1/lanes/A11-bn3-arena.md`
//! Sections 4.1-4.3).
//!
//! Wave 2 adds the typed refusal surface `BN3Error` (spec Sections 6-7) and
//! the term codec: `encode_term`/`decode_term`, the `BN3Statement`
//! encode/decode/validate surface, and `to_u56`. Decode is total over the
//! reserved codes — it returns `Err`, never panics (spec Section 6:
//! "the `#![no_std]` crate has no panic path on the decode boundary").
//!
//! Wave 1 slice was ABI freeze only: struct/const definitions plus
//! size/align/offset static asserts. Layout ground truth is pinned by
//! compile-time asserts — a regression is a build failure, not a runtime
//! surprise (house convention, `crates/cgap-arrow/src/layout.rs`).
//!
//! Wave 7 (defect repair driven by the wave-6 court receipts,
//! 2026-11-01): the a03-wave5 F-1 class ("debug-profile arithmetic
//! overflow panic on hostile bytes in a public parser") audited against
//! the term-word codec — `decode_term` is bit-ops only, so the class is
//! clean here, proven by execution with
//! `hostile_word_court_codec_total_and_typed_over_full_domains`: the
//! FULL raw-code domain (0x00..=0xFF at payload bounds 0 and
//! `0x00FF_FFFF_FFFF_FFFF`) plus every single-bit word through
//! `decode_term` and `BN3Statement::validate`, exact typed outcome
//! asserted per row, in the debug profile where overflow panics. Court
//! receipts are the arena.rs Wave 7 campaign (7/7 mutations flip; M4 and
//! M5 flip this file's hostile court).
//!
//! Wave 9 (cross-lane integration, 2026-11-01): A04's identity law
//! ("memory form and wire form are the same bytes", A04 §1/§6.1) applied
//! at the BN3 ABI — [`BN3Statement::to_le_bytes`] /
//! [`BN3Statement::from_le_bytes`], the LE image functions named by
//! spec 4.1's endianness note ("the (future) wire/pack encoding is
//! little-endian, matching A06's .oabi decision") and A04 §6.1's
//! normative LE rule. `wave9_a04_identity_court_statement_le_bytes_round_trip`
//! pins round-trip identity over all five sigils at both payload bounds
//! plus the byte-offset/word-offset map; its endianness pin (image byte 0
//! == low byte of `s`) is the anti-vacuity witness — the court-sandbox
//! mutation M-W9-1 (`to_be_bytes` shim) flips exactly it.

/// Bit offset of the sigil field in a term word: bits 63..56. (Spec 4.1)
pub const SIGIL_SHIFT: u32 = 56;

/// Payload width in bits: bits 55..0, stored as the low 56 bits of a
/// `u64`. (Spec 4.1)
pub const PAYLOAD_BITS: u32 = 56;

/// Sigil field width in bits: the top byte of the term word (bits
/// 63..56). (Spec 4.1)
pub const SIGIL_BITS: u32 = 8;

/// Mask selecting the low 56 payload bits of a term word. (Spec 4.1)
pub const PAYLOAD_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;

/// Maximum legal payload value (`u56`); encode asserts at or below this.
/// (Spec 4.1, 6)
pub const MAX_PAYLOAD: u64 = PAYLOAD_MASK;

/// Highest assigned sigil code (`0x04`, `BuiltinOp`). Codes above this are
/// RESERVED and the decoder MUST refuse them — typed refusal, never a
/// fallback (spec 4.2, 6).
pub const SIGIL_LAST_KNOWN: u8 = 0x04;

/// First RESERVED sigil code (`0x05`). The range
/// `SIGIL_RESERVED_FIRST..=0xFF` carries no forward-compatible silent
/// widening. (Spec 4.2)
pub const SIGIL_RESERVED_FIRST: u8 = 0x05;

/// Width of a variable register number inside `VarUni`/`VarExi` payloads:
/// registers `R0..R255` (spec Section 5). Layout is frozen here; binding
/// semantics are lane A12's authority.
pub const REGISTER_BITS: u32 = 8;

/// Full legal register range `R0..R255` (spec Section 5): every
/// `VarUni`/`VarExi` payload register number is `< REGISTER_COUNT`.
/// Defined by this lane (layout), adjudicated by A12 (unifier).
pub const REGISTER_COUNT: usize = 1 << REGISTER_BITS;

/// Frozen exit-gate size of `BN3Statement` in bytes: 3 term words.
/// (Spec 4.3; the size court asserts `size_of` equals this.)
pub const BN3_STATEMENT_SIZE: usize = 24;

/// Term kind tag occupying bits 63..56 of every term word. The `u8`
/// discriminant codes are the frozen ABI (spec 4.2); variant spelling is
/// Rust-idiomatic CamelCase of the spec names.
///
/// Payload semantics per code:
///
/// | Code | Variant | Payload |
/// |---|---|---|
/// | `0x00` | `SlotConst` | A06 `SlotId` truncated to 56 bits; full id in the A06 pack header |
/// | `0x01` | `VarUni` | universally quantified variable register `R_n` |
/// | `0x02` | `VarExi` | existentially quantified variable register `R_n` |
/// | `0x03` | `FormulaRef` | formula-table index (the quoted graph `{ ... }`) |
/// | `0x04` | `BuiltinOp` | dense builtin opcode (not an A06 `SlotId`) |
/// | `0x05..=0xFF` | RESERVED | decoder MUST refuse: `BN3Error::UnknownSigil(u8)` |
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sigil {
    /// `0x00` — A06 `SlotId` truncated to 56 bits (payload truncation
    /// `SlotId >> 8` is PROVISIONAL pending A06's collision-vetting space
    /// decision; spec Section 10).
    SlotConst = 0x00,
    /// `0x01` — universally quantified variable register `R_n`, `n` in
    /// `0..=255` (spec Section 5; A12 owns binding semantics).
    VarUni = 0x01,
    /// `0x02` — existentially quantified variable register `R_n` (spec
    /// Section 5; A12 owns binding semantics).
    VarExi = 0x02,
    /// `0x03` — formula-table index: the quoted graph `{ ... }` flattened
    /// to a `BN3FormulaHeader` interval (spec 4.4). Banned in the `p` slot
    /// (spec Section 7, `FormulaInPredicate`; A12 adjudicates).
    FormulaRef = 0x03,
    /// `0x04` — dense builtin opcode (A13's vocabulary; dense index, not a
    /// `SlotId`).
    BuiltinOp = 0x04,
}

impl Sigil {
    /// Typed refusal decoder for raw sigil codes (spec 4.2, 6): codes
    /// `0x05..=0xFF` are RESERVED — `Err(UnknownSigil(code))`, never a
    /// fallback (spec Section 7, last row).
    pub const fn from_code(code: u8) -> Result<Sigil, BN3Error> {
        match code {
            0x00 => Ok(Sigil::SlotConst),
            0x01 => Ok(Sigil::VarUni),
            0x02 => Ok(Sigil::VarExi),
            0x03 => Ok(Sigil::FormulaRef),
            0x04 => Ok(Sigil::BuiltinOp),
            // A11-M2-ANCILLARY: this arm is `from_code`'s refusal, kept
            // separate from `decode_term`'s so the M4 shim mutates exactly
            // one decode path.
            _ => Err(BN3Error::UnknownSigil(code)),
        }
    }

    /// The frozen `u8` ABI code of this sigil (spec 4.2 table).
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Typed error surface of the BN3 codec and flatten (spec Sections 6-7).
/// Every Section 7 boundary is one of these variants — a typed refusal,
/// never a lossy coercion (spec Section 7 preamble).
///
/// Crate-level ABI object shared by A11 (this file, owner), A12
/// (`UnifyError` is separate and register-scoped), and A13 (whose Wave-1
/// receipt pins `BN3Error::UnknownSigil(u8)` as living here).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BN3Error {
    /// Unknown/reserved sigil code on decode (spec Section 7 row 7): the
    /// RESERVED range must refuse loudly, no forward-compatible silent
    /// widening (spec 4.2). Carries the offending code.
    UnknownSigil(u8),
    /// Literal/term payload above the 56-bit ceiling (spec Section 7
    /// row 1).
    PayloadOverflow,
    /// More than 256 live registers in one scope (spec Section 7 rows 2
    /// and 5): the 8-bit register number space cannot name register 256+.
    /// Encode-side refusal; decode stays total over the word space.
    RegisterExhausted,
    /// Quoted graph in the predicate slot (spec Section 7 row 3): `p`
    /// bans `FORMULA_REF` (spec Section 7; A12 adjudicates the rule, this
    /// crate's encode/validate enforces it at mint time).
    FormulaInPredicate,
    /// Overlapping or mis-nested formula intervals, a formula ref to a
    /// non-ancestor scope, or an interval whose size witness disagrees
    /// (spec Section 7 row 4, Section 4.4 invariant court).
    IntervalOverlap,
    /// Quotation nested deeper than 64 levels (spec Section 7 row 6;
    /// depth cap keeps the recursive flatten `no_std`-stack-safe).
    FlattenDepthExceeded,
    /// A statement index above the `u32` interval-endpoint space (spec
    /// 4.4: `start`/`end` are `u32`; the Wave-1 constant doc in
    /// `arena.rs` pins "Wave 2 flatten enforces this"). Named extension
    /// beyond the seven-row table — recorded divergence.
    StatementIndexOverflow,
    /// Wave 9 (A05 power-cut class, cross-lane): a restore image violates
    /// its own framing — truncated at a region boundary (A05 §5 torn-write
    /// classes C1-C3), a count prefix whose arithmetic cannot address the
    /// image, or a nonzero reserved field. `expected` is the value the
    /// framing law demands, `got` the value observed in the image. The
    /// restore boundary is all-or-nothing: a refused image admits NO arena
    /// (never a half-admitted prefix — the C4 "commit, not corruption"
    /// distinction is exact-length-only).
    ///
    /// Second named extension beyond the seven-row table, minted for the
    /// Wave 9 restore surface (`Arena::from_image`); recorded divergence.
    ImageMalformed {
        /// Value the violated framing law demands.
        expected: u64,
        /// Value actually observed in the image.
        got: u64,
    },
}

impl core::fmt::Display for BN3Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BN3Error::UnknownSigil(code) => write!(f, "unknown sigil code 0x{code:02X}"),
            BN3Error::PayloadOverflow => write!(f, "payload exceeds the 56-bit ceiling"),
            BN3Error::RegisterExhausted => {
                write!(f, "register number exceeds R0..R255 (8-bit register file)")
            }
            BN3Error::FormulaInPredicate => write!(f, "FORMULA_REF in the predicate slot"),
            BN3Error::IntervalOverlap => write!(f, "formula intervals overlap or mis-nest"),
            BN3Error::FlattenDepthExceeded => {
                write!(f, "quotation nesting exceeds the 64-level cap")
            }
            BN3Error::StatementIndexOverflow => {
                write!(f, "statement index exceeds the u32 interval space")
            }
            BN3Error::ImageMalformed { expected, got } => {
                write!(f, "restore image violates its framing: demanded {expected}, observed {got}")
            }
        }
    }
}

impl core::error::Error for BN3Error {}

/// Encode one term word (spec Section 6 pseudocode `encode_term`):
/// `(sigil << 56) | payload` with two typed refusals.
///
/// - `payload > 0x00FF_FFFF_FFFF_FFFF` → [`BN3Error::PayloadOverflow`]
///   (spec Section 7 row 1).
/// - `VarUni`/`VarExi` with payload ≥ 256 →
///   [`BN3Error::RegisterExhausted`] (spec Section 7 rows 2/5; the
///   day-one exhaustion court of spec Section 10 risk 3). The register
///   bound is an *encode-side* law: `decode_term` stays total over the
///   word space so the frozen 56-bit transport width round-trips at both
///   Section 8 bounds.
///
/// No panic path. `Sigil` values are known codes by construction, so the
/// pseudocode's `assert sigil_is_known` is a type-level given here; raw
/// codes go through [`Sigil::from_code`], which refuses RESERVED values.
pub fn encode_term(sigil: Sigil, payload: u64) -> Result<u64, BN3Error> {
    if payload > MAX_PAYLOAD {
        return Err(BN3Error::PayloadOverflow);
    }
    match sigil {
        Sigil::VarUni | Sigil::VarExi if payload >= REGISTER_COUNT as u64 => {
            Err(BN3Error::RegisterExhausted)
        }
        _ => Ok(((sigil as u64) << SIGIL_SHIFT) | payload),
    }
}

/// Decode one term word (spec Section 6 pseudocode `decode_term`): the
/// top byte is the sigil code, the low 56 bits the payload. Total over
/// reserved codes — `Err(UnknownSigil(code))`, never a panic (spec
/// Section 6: no panic path on the decode boundary).
///
/// Spec 4.1's "MAY assert bit 55 is 0" redundancy check is NOT taken:
/// bit 55 is the top bit of the mandated 56-bit payload, so a maximally
/// packed `u56` (the Section 8 round-trip bound) sets it. Divergence
/// recorded at the module level.
pub const fn decode_term(word: u64) -> Result<(Sigil, u64), BN3Error> {
    let code = (word >> SIGIL_SHIFT) as u8;
    let sigil = match Sigil::from_code(code) {
        Ok(sigil) => sigil,
        // A11-M4-REFUSE: the typed-refusal arm the M4 shim targets.
        Err(_) => return Err(BN3Error::UnknownSigil(code)),
    };
    Ok((sigil, word & PAYLOAD_MASK))
}

/// Truncate an A06 64-bit `SlotId` to the 56-bit BN3 payload space (spec
/// Section 6 `to_u56`: the high 56 bits of the 64-bit id). PROVISIONAL —
/// spec Section 10 risk 1: 8 bits are dropped, so collision vetting must
/// move to the truncated space or full ids live in the A06 pack header.
pub const fn to_u56(a06_slot: u64) -> u64 {
    a06_slot >> 8
}

/// One BN3 statement: exactly three term words, exactly 24 bytes.
/// `repr(C)` so the FFI surface (cgap-ffi) can reinterpret without a codec
/// (spec 4.3). Each word is `sigil << 56 | payload` (spec 4.1); s/p/o
/// positions hold subject/predicate/object terms.
///
/// No pointer, no `Box`, no `Vec` in the payload — the flat arena is the
/// only representation (ADR-003, spec Sections 1-2).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BN3Statement {
    /// Subject term word (offset 0x00).
    pub s: u64,
    /// Predicate term word (offset 0x08). `FormulaRef` banned here (spec
    /// Section 7 row 3; enforced by [`BN3Statement::encode_terms`] and
    /// [`BN3Statement::validate`]).
    pub p: u64,
    /// Object term word (offset 0x10).
    pub o: u64,
}

impl BN3Statement {
    /// Raw word constructor: the FFI reinterpretation surface (`repr(C)`
    /// field-for-field). No structural laws are enforced here by design —
    /// [`BN3Statement::validate`] is the lawful gate for raw words.
    pub const fn from_words(s: u64, p: u64, o: u64) -> BN3Statement {
        BN3Statement { s, p, o }
    }

    /// Encode a statement from three `(sigil, payload)` terms (spec
    /// Section 6, `encode_all_three_terms`). Enforces the predicate-slot
    /// ban: a `FormulaRef` in `p` is [`BN3Error::FormulaInPredicate`]
    /// (spec Section 7 row 3). All other refusals come from
    /// [`encode_term`].
    pub fn encode_terms(
        s: (Sigil, u64),
        p: (Sigil, u64),
        o: (Sigil, u64),
    ) -> Result<BN3Statement, BN3Error> {
        if p.0 == Sigil::FormulaRef {
            return Err(BN3Error::FormulaInPredicate);
        }
        Ok(BN3Statement {
            s: encode_term(s.0, s.1)?,
            p: encode_term(p.0, p.1)?,
            o: encode_term(o.0, o.1)?,
        })
    }

    /// Decode all three term words (spec Section 6, total over reserved
    /// codes). The predicate-slot ban is NOT re-checked on decode: decode
    /// is the transport surface, structural laws ride [`BN3Statement::validate`].
    pub fn decode_terms(&self) -> Result<[(Sigil, u64); 3], BN3Error> {
        Ok([
            decode_term(self.s)?,
            decode_term(self.p)?,
            decode_term(self.o)?,
        ])
    }

    /// Structural validation of a (possibly raw-word) statement: decodes
    /// every word and enforces the `p`-slot `FormulaRef` ban. This is the
    /// lawful gate for words minted via [`BN3Statement::from_words`]
    /// (FFI reinterpretation).
    pub fn validate(&self) -> Result<(), BN3Error> {
        self.decode_terms()?;
        if (self.p >> SIGIL_SHIFT) as u8 == Sigil::FormulaRef as u8 {
            return Err(BN3Error::FormulaInPredicate);
        }
        Ok(())
    }

    /// Wave 9 cross-lane integration (A04 identity law at the BN3 ABI):
    /// the little-endian byte image of this statement. A04 §6.1 makes LE
    /// the fleet wire convention ("all multi-byte integers are
    /// little-endian", `docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md`)
    /// and spec 4.1 defers bn3's wire encoding to exactly that decision,
    /// so this is the canonical image: memory form and wire form are the
    /// same 24 bytes, and `from_le_bytes(to_le_bytes(x)) == x` is the
    /// in-crate half of A04's identity court. Frame carriage (A04
    /// `CgapFrameHeader` `FLAG_BN3` payload) is the A14/A19 tier — this
    /// lane has no `cgap-core` dependency edge (manifest off-limits), so
    /// the image functions are the integration seam it ships.
    ///
    /// Word `w` occupies image bytes `[8w..8w+8)` — byte 0 of the image is
    /// the low byte of `s`, the endianness pin the identity court asserts
    /// (a `to_be_bytes` shim cannot satisfy it).
    pub const fn to_le_bytes(&self) -> [u8; BN3_STATEMENT_SIZE] {
        let s = self.s.to_le_bytes();
        let p = self.p.to_le_bytes();
        let o = self.o.to_le_bytes();
        let mut out = [0u8; BN3_STATEMENT_SIZE];
        let mut i = 0;
        while i < 8 {
            out[i] = s[i];
            out[8 + i] = p[i];
            out[16 + i] = o[i];
            i += 1;
        }
        out
    }

    /// Reassemble a statement from its 24-byte LE image (inverse of
    /// [`BN3Statement::to_le_bytes`]). Total and unvalidating by design —
    /// this is the FFI/transport reinterpretation surface, the byte-level
    /// twin of [`BN3Statement::from_words`]; structural laws ride
    /// [`BN3Statement::validate`] (the restore boundary's dam, see
    /// `Arena::from_image`).
    pub const fn from_le_bytes(bytes: [u8; BN3_STATEMENT_SIZE]) -> BN3Statement {
        let mut s = [0u8; 8];
        let mut p = [0u8; 8];
        let mut o = [0u8; 8];
        let mut i = 0;
        while i < 8 {
            s[i] = bytes[i];
            p[i] = bytes[8 + i];
            o[i] = bytes[16 + i];
            i += 1;
        }
        BN3Statement {
            s: u64::from_le_bytes(s),
            p: u64::from_le_bytes(p),
            o: u64::from_le_bytes(o),
        }
    }
}

// --- ABI freeze: compile-time layout laws (spec 4.3) --------------------
// Any field addition, widening, or reorder must fail here, not in a
// unifier six months from now. Size-court mutation for anti-vacuity
// (spec Section 8): widen `s: u64` to `u128` — the build MUST fail.
const _: () = assert!(core::mem::size_of::<BN3Statement>() == BN3_STATEMENT_SIZE);
const _: () = assert!(core::mem::align_of::<BN3Statement>() == 8);
const _: () = assert!(core::mem::offset_of!(BN3Statement, s) == 0);
const _: () = assert!(core::mem::offset_of!(BN3Statement, p) == 8);
const _: () = assert!(core::mem::offset_of!(BN3Statement, o) == 16);
const _: () = assert!(PAYLOAD_BITS == 56);
// [A13 unblock 2026-11-01] replaced the prior run's `SIGIL_SHIFT ==
// 64 - PAYLOAD_BITS` (56 == 8, E0080) with `SIGIL_SHIFT == PAYLOAD_BITS`.
// [A11 restatement 2026-11-01] A13's line was true only by coincidence of
// the 8/56 split; the lawful field-split identities follow, with the sigil
// width named explicitly. A11 owns this file.
const _: () = assert!(SIGIL_BITS == 8);
const _: () = assert!(SIGIL_SHIFT == 64 - SIGIL_BITS);
const _: () = assert!(SIGIL_BITS + PAYLOAD_BITS == 64);
const _: () = assert!(PAYLOAD_MASK == (1u64 << PAYLOAD_BITS) - 1);
const _: () = assert!(MAX_PAYLOAD == 0x00FF_FFFF_FFFF_FFFF);
const _: () = assert!(REGISTER_BITS == 8);
const _: () = assert!(REGISTER_COUNT == 256);
const _: () = assert!(REGISTER_COUNT == 1usize << REGISTER_BITS);

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    /// Size court (spec Section 8): a real `BN3Statement` value.
    /// Exit gate: 24 bytes. Mutation that MUST flip this court: widen
    /// `s: u64` to `u128`, or add a 4th field (build failure, not test
    /// failure — the const asserts above are the primary court).
    #[test]
    fn size_court_statement_is_24_bytes() {
        assert_eq!(size_of::<BN3Statement>(), 24);
        assert_eq!(align_of::<BN3Statement>(), 8);
        // repr(C): one u64 stride per field; three statements pack exactly
        // into 72 bytes with no tail padding.
        assert_eq!(size_of::<[BN3Statement; 3]>(), 72);
        // Cache-line arithmetic (spec Section 2, rejected alternative 5):
        // 64/24 = 2.67 statements per line — 2 whole statements (48 B)
        // plus 16 B spare; 8 statements tile exactly 3 lines (192 B).
        // [A11 fix 2026-11-01] prior run's tail assert computed
        // 512/192 = 2 (int div) and asserted == 1; it had never executed
        // because the crate did not compile.
        assert_eq!(64 / BN3_STATEMENT_SIZE, 2);
        assert_eq!(64 % BN3_STATEMENT_SIZE, 16);
        assert_eq!(3 * 64 / BN3_STATEMENT_SIZE, 8);
    }

    /// ABI-freeze court: the discriminant codes are the frozen ABI (spec
    /// 4.2). Mutation that MUST flip this court: renumber `SlotConst` from
    /// `0x00` to `0x01` (collides with `VarUni`), or swap the
    /// `VarUni`/`VarExi` codes (spec Section 8 scope-court mutation).
    #[test]
    fn abi_court_sigil_codes_match_spec() {
        assert_eq!(Sigil::SlotConst as u8, 0x00);
        assert_eq!(Sigil::VarUni as u8, 0x01);
        assert_eq!(Sigil::VarExi as u8, 0x02);
        assert_eq!(Sigil::FormulaRef as u8, 0x03);
        assert_eq!(Sigil::BuiltinOp as u8, 0x04);
        assert_eq!(SIGIL_LAST_KNOWN, Sigil::BuiltinOp as u8);
        assert_eq!(SIGIL_RESERVED_FIRST, SIGIL_LAST_KNOWN + 1);
        // Register file width (spec Section 5): R0..R255, 8-bit numbers.
        assert_eq!(REGISTER_COUNT, 256);
        assert_eq!(REGISTER_BITS, 8);
    }

    /// Word-layout court: the mask/shift consts encode the 8-bit sigil +
    /// 56-bit payload split of spec 4.1. Mutation that MUST flip: change
    /// `PAYLOAD_MASK` to `u64::MAX` (drops the sigil field).
    #[test]
    fn word_court_field_split_is_8_56() {
        assert_eq!(SIGIL_SHIFT, 56);
        assert_eq!(PAYLOAD_BITS, 56);
        assert_eq!(PAYLOAD_MASK, 0x00FF_FFFF_FFFF_FFFF);
        assert_eq!(MAX_PAYLOAD, 0x00FF_FFFF_FFFF_FFFF);
        // A maximally packed word keeps sigil and payload disjoint:
        // sigil in 63..56, payload in 55..0.
        let word = (0x04u64 << SIGIL_SHIFT) | MAX_PAYLOAD;
        assert_eq!(word >> SIGIL_SHIFT, 0x04);
        assert_eq!(word & PAYLOAD_MASK, MAX_PAYLOAD);
    }

    // --- Wave 2 courts (spec Section 8) ----------------------------------

    /// Round-trip court (spec Section 8): encode/decode over all 5 sigils
    /// at payload bounds 0 and `0x00FF_FFFF_FFFF_FFFF`.
    ///
    /// The two register sigils refuse at the upper bound on encode
    /// (`RegisterExhausted`, spec Section 7) — the upper-bound round trip
    /// for them rides the raw transport word + the total decoder, which is
    /// the lawful reading of "decoded (sigil, payload) equals input":
    /// decode is total over the word space (spec Section 6), the register
    /// bound is encode-side discipline.
    ///
    /// Mutation that MUST flip this court: renumber `SLOT_CONST` from
    /// `0x00` to `0x01` (spec's named mutation; collides with `VarUni` —
    /// build failure via duplicate discriminant, and the abi court's
    /// frozen-code asserts).
    #[test]
    fn round_trip_court_all_five_sigils_at_bounds() {
        let all = [
            Sigil::SlotConst,
            Sigil::VarUni,
            Sigil::VarExi,
            Sigil::FormulaRef,
            Sigil::BuiltinOp,
        ];
        for sigil in all {
            // Lower bound: every sigil encodes and decodes at payload 0.
            let word = encode_term(sigil, 0).expect("payload 0 encodes for every sigil");
            assert_eq!(word, (sigil as u64) << SIGIL_SHIFT);
            assert_eq!(decode_term(word), Ok((sigil, 0)));
        }
        for sigil in all {
            // Upper bound: the full u56 transport width.
            if matches!(sigil, Sigil::VarUni | Sigil::VarExi) {
                // Encode refuses (spec Section 7 rows 2/5)...
                assert_eq!(encode_term(sigil, MAX_PAYLOAD), Err(BN3Error::RegisterExhausted));
                // ...while the transport word round-trips through the
                // total decoder (spec Section 8 upper bound, spec
                // Section 6 totality).
                let word = ((sigil as u64) << SIGIL_SHIFT) | MAX_PAYLOAD;
                assert_eq!(decode_term(word), Ok((sigil, MAX_PAYLOAD)));
            } else {
                let word = encode_term(sigil, MAX_PAYLOAD).expect("non-register sigil encodes");
                assert_eq!(decode_term(word), Ok((sigil, MAX_PAYLOAD)));
            }
        }
    }

    /// Encode-field court: the assembled word is exactly `sigil << 56 |
    /// payload`, sigil in 63..56, payload in 55..0. Mutation that MUST
    /// flip: any field-split constant change (word court above flips
    /// first via the const asserts).
    #[test]
    fn encode_court_word_assembly_is_sigil_shift_or_payload() {
        let word = encode_term(Sigil::BuiltinOp, 5).expect("encodes");
        assert_eq!(word, (0x04u64 << 56) | 5);
        assert_eq!(word >> SIGIL_SHIFT, 0x04);
        assert_eq!(word & PAYLOAD_MASK, 5);
        // Mid-range payload keeps the fields disjoint (literal is
        // exactly 56 bits: 0x0012_3456_789A_BCDE).
        let word = encode_term(Sigil::SlotConst, 0x0012_3456_789A_BCDE).expect("encodes");
        assert_eq!(word >> SIGIL_SHIFT, 0x00);
        assert_eq!(word & PAYLOAD_MASK, 0x0012_3456_789A_BCDE);
    }

    /// Refusal court, sigil-side rows of spec Section 7 (the interval /
    /// flatten rows live in `arena.rs`'s consolidated court). Every
    /// boundary returns its exact typed error, never a fallback coercion.
    /// Mutation that MUST flip: replace the `decode_term` refusal arm
    /// with a fallback-const coercion (spec Section 8 refusal-court
    /// mutation) — the `UnknownSigil` asserts below then fail.
    #[test]
    fn refusal_court_sigil_rows_typed() {
        // Row 1: payload > 56 bits -> PayloadOverflow.
        assert_eq!(
            encode_term(Sigil::SlotConst, MAX_PAYLOAD + 1),
            Err(BN3Error::PayloadOverflow)
        );
        assert_eq!(
            encode_term(Sigil::BuiltinOp, u64::MAX),
            Err(BN3Error::PayloadOverflow)
        );
        // Rows 2 and 5: > 256 registers in one scope ->
        // RegisterExhausted (encode-side; the day-one exhaustion court of
        // spec Section 10 risk 3).
        assert_eq!(
            encode_term(Sigil::VarUni, REGISTER_COUNT as u64),
            Err(BN3Error::RegisterExhausted)
        );
        assert_eq!(
            encode_term(Sigil::VarExi, MAX_PAYLOAD),
            Err(BN3Error::RegisterExhausted)
        );
        // Row 3: FormulaRef in the predicate slot -> FormulaInPredicate.
        assert_eq!(
            BN3Statement::encode_terms(
                (Sigil::SlotConst, 1),
                (Sigil::FormulaRef, 0),
                (Sigil::SlotConst, 2),
            ),
            Err(BN3Error::FormulaInPredicate)
        );
        // Row 7: reserved sigil codes -> UnknownSigil(code), the typed
        // refusal, at the boundary codes and across the RESERVED range.
        assert_eq!(decode_term(0x05u64 << 56), Err(BN3Error::UnknownSigil(0x05)));
        assert_eq!(
            decode_term((0xFFu64 << 56) | 7),
            Err(BN3Error::UnknownSigil(0xFF))
        );
        assert_eq!(Sigil::from_code(0x80), Err(BN3Error::UnknownSigil(0x80)));
        // Boundary of the legal range: 0x04 decodes, 0x05 refuses.
        assert_eq!(decode_term(0x04u64 << 56), Ok((Sigil::BuiltinOp, 0)));
    }

    /// Statement codec court: `encode_terms`/`decode_terms` round-trip a
    /// real three-term statement; `validate` gates raw words (the FFI
    /// surface) on the same laws. Mutation that MUST flip: drop the
    /// `FormulaInPredicate` check in either path — the `Err` asserts
    /// below fail.
    #[test]
    fn statement_court_encode_decode_validate() {
        let stmt = BN3Statement::encode_terms(
            (Sigil::SlotConst, 0xA1B2),
            (Sigil::VarUni, 42),
            (Sigil::VarExi, 7),
        )
        .expect("legal terms encode");
        assert_eq!(stmt.s, (0x00u64 << 56) | 0xA1B2);
        assert_eq!(stmt.p, (0x01u64 << 56) | 42);
        assert_eq!(stmt.o, (0x02u64 << 56) | 7);
        let decoded = stmt.decode_terms().expect("legal words decode");
        assert_eq!(
            decoded,
            [
                (Sigil::SlotConst, 0xA1B2),
                (Sigil::VarUni, 42),
                (Sigil::VarExi, 7),
            ]
        );
        assert_eq!(stmt.validate(), Ok(()));
        // Raw words (the FFI reinterpretation path) are gated by
        // validate: a FormulaRef in the p slot is the typed refusal even
        // when minted raw.
        let raw = BN3Statement::from_words(
            (0x00u64 << 56) | 1,
            (0x03u64 << 56) | 0,
            (0x00u64 << 56) | 2,
        );
        assert_eq!(raw.validate(), Err(BN3Error::FormulaInPredicate));
        // And a reserved code in any slot refuses on decode.
        let raw = BN3Statement::from_words(0x05u64 << 56, 0, 0);
        assert_eq!(raw.validate(), Err(BN3Error::UnknownSigil(0x05)));
    }

    /// `to_u56` court (spec Section 6): the truncation keeps the HIGH 56
    /// of the 64-bit A06 `SlotId`. Mutation that MUST flip: shift the
    /// other way (`<< 8`) — the high/low asserts below swap.
    #[test]
    fn to_u56_court_keeps_high_56_bits() {
        assert_eq!(to_u56(0x1234_5678_9ABC_DEF0), 0x0012_3456_789A_BCDE);
        assert_eq!(to_u56(0xFF), 0); // low 8 bits are dropped
        assert_eq!(to_u56(u64::MAX), MAX_PAYLOAD); // high 56 kept exactly
        assert_eq!(to_u56(1 << 63), 1 << 55); // top bit moves down one
    }

    /// Wave 7 hostile-word court (receipt driver: a03-wave5 F-1, the class
    /// A13's Wave 6 repaired in `builtins.rs` — "debug-profile arithmetic
    /// overflow panic on hostile bytes in a public parser violates the
    /// typed-refusal contract"): the term-word codec is total and typed
    /// over the FULL raw-code domain (0x00..=0xFF at both payload bounds)
    /// and over every single-bit word, run in the debug profile where
    /// overflow actually panics. `decode_term` is bit-ops only — this
    /// court pins that fact against regression.
    ///
    /// Mutations that MUST flip: M4 (the `A11-M4-REFUSE` arm replaced by a
    /// const fallback) flips the RESERVED-range and single-bit refusal
    /// rows; any sigil renumber flips the exact-code asserts (and the
    /// renamed code's word form).
    #[test]
    fn hostile_word_court_codec_total_and_typed_over_full_domains() {
        // 1. Full raw-code domain x payload bounds {0, 2^56 - 1}: known
        //    codes decode to exactly their sigil + payload; RESERVED codes
        //    return exactly `UnknownSigil(code)`.
        for code in 0..=255u8 {
            for &payload in &[0u64, MAX_PAYLOAD] {
                let word = ((code as u64) << SIGIL_SHIFT) | payload;
                match decode_term(word) {
                    Ok((sigil, got)) => {
                        assert!(code <= SIGIL_LAST_KNOWN, "code {code:#04X}");
                        assert_eq!(sigil.code() as u64, code as u64);
                        assert_eq!(got, payload);
                    }
                    Err(BN3Error::UnknownSigil(offender)) => {
                        assert!(code >= SIGIL_RESERVED_FIRST, "code {code:#04X}");
                        assert_eq!(offender, code);
                    }
                    Err(other) => {
                        panic!("word {word:#018X} (code {code:#04X}) produced {other:?}")
                    }
                }
            }
        }
        // 2. Every single-bit word, exact expected decode per bit; the
        //    raw-word statement surface (the FFI reinterpretation path)
        //    rides the same totality — `validate` mirrors the decode.
        for bit in 0..64u32 {
            let word = 1u64 << bit;
            let expected = if bit < 56 {
                Ok((Sigil::SlotConst, word))
            } else {
                match bit {
                    56 => Ok((Sigil::VarUni, 0)),
                    57 => Ok((Sigil::VarExi, 0)),
                    58 => Ok((Sigil::BuiltinOp, 0)),
                    _ => Err(BN3Error::UnknownSigil((1u64 << (bit - 56)) as u8)),
                }
            };
            assert_eq!(decode_term(word), expected, "bit {bit}");
            let stmt = BN3Statement::from_words(word, word, word);
            assert_eq!(stmt.validate(), expected.map(|_| ()), "bit {bit}");
        }
    }

    /// Display/error court: the typed errors render distinct messages
    /// (no-std `core::fmt`), and `BN3Error` implements `core::error::Error`.
    /// Mutation that MUST flip: deleting any enum variant (build failure
    /// at the match sites).
    #[test]
    fn error_court_display_is_distinct() {
        let msgs = [
            BN3Error::UnknownSigil(0x42).to_string(),
            BN3Error::PayloadOverflow.to_string(),
            BN3Error::RegisterExhausted.to_string(),
            BN3Error::FormulaInPredicate.to_string(),
            BN3Error::IntervalOverlap.to_string(),
            BN3Error::FlattenDepthExceeded.to_string(),
            BN3Error::StatementIndexOverflow.to_string(),
            BN3Error::ImageMalformed { expected: 1, got: 0 }.to_string(),
        ];
        for m in &msgs {
            assert!(!m.is_empty());
        }
        // Pairwise distinct: every boundary is nameable from its message.
        for i in 0..msgs.len() {
            for j in (i + 1)..msgs.len() {
                assert_ne!(msgs[i], msgs[j], "i={i} j={j}");
            }
        }
        let _: &dyn core::error::Error = &BN3Error::PayloadOverflow;
    }

    // --- Wave 9: A04 identity law at the statement ABI ------------------

    /// A04 identity law at the 24-byte statement ABI: memory form and LE
    /// wire form are the same bytes. Real collaborator: statements over
    /// every sigil at both payload bounds. Asserts (a) round-trip identity
    /// `from_le_bytes(to_le_bytes(x)) == x`, (b) the word/byte-offset map
    /// (word `w` occupies bytes `[8w..8w+8)`), and (c) the endianness pin:
    /// image byte 0 is the LOW byte of `s` (A04 §6.1 little-endian) —
    /// mutation M-W9-1 (`to_be_bytes` shim) MUST flip exactly this arm.
    #[test]
    fn wave9_a04_identity_court_statement_le_bytes_round_trip() {
        // Word-space coverage over every assigned sigil at both payload
        // bounds: `from_words` is the lawful raw constructor, the p slot
        // carries `BuiltinOp` (legal at any payload — the `FormulaRef`
        // predicate ban and the 8-bit register bound are encode/validate
        // laws this court does not re-litigate), and `validate` is
        // asserted clean per row so the identity law runs over ADMITTED
        // statements only.
        for code in 0x00u64..=0x04 {
            for payload in [0u64, MAX_PAYLOAD] {
                let s_word = (code << SIGIL_SHIFT) | payload;
                let p_word = (0x04u64 << SIGIL_SHIFT) | payload;
                let o_word = (code << SIGIL_SHIFT) | payload;
                let stmt = BN3Statement::from_words(s_word, p_word, o_word);
                assert_eq!(stmt.validate(), Ok(()), "code {code:#04x} payload {payload:#x}");
                let image = stmt.to_le_bytes();
                assert_eq!(image.len(), BN3_STATEMENT_SIZE);
                assert_eq!(BN3Statement::from_le_bytes(image), stmt, "code {code:#04x}");
                // Word/byte-offset map + endianness pins (low byte first,
                // A04 §6.1): the arm the M-W9-1 `to_be_bytes` shim flips.
                assert_eq!(u64::from(image[0]), stmt.s & 0xFF);
                assert_eq!(u64::from(image[7]), stmt.s >> 56);
                assert_eq!(u64::from(image[8]), stmt.p & 0xFF);
                assert_eq!(u64::from(image[8 + 7]), stmt.p >> 56);
                assert_eq!(u64::from(image[16]), stmt.o & 0xFF);
                assert_eq!(u64::from(image[16 + 7]), stmt.o >> 56);
            }
        }
        // A mixed real statement through the encode path (three distinct
        // sigils; the register sigil at its lawful 8-bit ceiling): identity
        // holds on the general form, not only the uniform loop above.
        let mixed = BN3Statement::encode_terms(
            (Sigil::SlotConst, MAX_PAYLOAD),
            (Sigil::VarUni, 255),
            (Sigil::FormulaRef, 7),
        )
        .unwrap_or_else(|e| panic!("mixed statement must encode: {e}"));
        assert_eq!(BN3Statement::from_le_bytes(mixed.to_le_bytes()), mixed);
    }

    /// `from_le_bytes` is the transport reinterpretation surface: TOTAL
    /// over garbage images, no panic, no validation — the same law as
    /// `from_words`, with `validate` as the lawful gate behind it. Asserts
    /// the all-0xFF image reassembles to the 0xFF…F words (identity of
    /// representation, not validity), and that `validate` then refuses it
    /// typed (0xFF sigil is RESERVED).
    #[test]
    fn wave9_from_le_bytes_is_total_unvalidating_validate_is_the_gate() {
        let hostile = [0xFFu8; BN3_STATEMENT_SIZE];
        let stmt = BN3Statement::from_le_bytes(hostile);
        assert_eq!(stmt.s, u64::MAX);
        assert_eq!(stmt.p, u64::MAX);
        assert_eq!(stmt.o, u64::MAX);
        assert_eq!(stmt.validate(), Err(BN3Error::UnknownSigil(0xFF)));
        // And a valid image re-validates clean: the gate, not the
        // transport, discriminates.
        let good = BN3Statement::encode_terms(
            (Sigil::SlotConst, 1),
            (Sigil::BuiltinOp, 2),
            (Sigil::VarExi, 3),
        )
        .unwrap_or_else(|e| panic!("good statement must encode: {e}"));
        assert_eq!(BN3Statement::from_le_bytes(good.to_le_bytes()).validate(), Ok(()));
    }
}
