//! BN3 arena (Wave 2): the flat statement slab, the formula table of
//! pointer-free index intervals, the recursive flatten, and the interval
//! integrity checks (spec: `docs/jira/v26.11.1/lanes/A11-bn3-arena.md`
//! Sections 4.4, 6, 7; ADR-003 Sections 1-2).
//!
//! Wave 1 froze the `BN3FormulaHeader` ABI (16 bytes, `repr(C)`); Wave 2
//! adds the arena machinery around it. The stored representation stays
//! pointer-free (ADR-003): the slab is a contiguous statement vector, a
//! quoted graph `{ ... }` is a `[start, end)` interval, and nesting is
//! recursive sub-intervals. The [`FormulaNode`] input tree is the
//! transient parse surface only — never stored; the flat arena is the
//! only stored representation (spec Section 2, rejected alternative 1
//! targets the *stored* form).
//!
//! Two spec-internal conflicts are resolved here and recorded as
//! divergences (module docs are the receipt):
//!
//! 1. Spec Section 6 pseudocode closes the parent header *before*
//!    recursing (`push_formula_header` then children), which makes child
//!    intervals disjoint-from — never subsets of — the parent, and would
//!    trip the pseudocode's own `verify_intervals_disjoint_and_nested`
//!    gate on any program that has children. Section 4.4 and the flatten
//!    court demand child ⊆ parent, so the close is post-order: the
//!    header's `end` spans all descendant statements.
//! 2. Spec 4.1's "bit 55 always 0" aside contradicts the 56-bit payload
//!    and the Section 8 round-trip bound `0x00FF_FFFF_FFFF_FFFF` (that
//!    value sets bit 55). The MAY-assert is not taken. (Handled in
//!    `sigil.rs`.)
//!
//! Wave 4 (harden + close DoD, 2026-11-01): the formula-table reserve
//! path now refuses past the `u32` header index space
//! ([`header_index`], the same frozen index space as the slab) — the
//! last silent `usize`→`u32` truncation in the module is gone. (The
//! Wave 4 report this doc once cited was never landed — audit receipt
//! `docs/jira/v26.11.1/waves/a11-wave4-audit-receipt.md` Section 1 —
//! so the receipts below are the surviving record.)
//!
//! Wave 5 (defect repair, 2026-11-01): DoD 7's five anti-vacuity
//! mutations (spec Section 8) re-run against a real /tmp sandbox copy
//! of the crate — canonical tree never shimmed, sandbox hashes verified
//! equal to canonical before the court, every restore sha256
//! byte-exact, canonical hashes unchanged after the court. 5/5 flip,
//! rc=101 each:
//!
//! - M1 widen `s: u64`→`u128`: build fail — E0080 const-asserts
//!   (size/offset/align) + E0308.
//! - M2 `SlotConst = 0x01`: build fail — E0081 duplicate discriminant.
//! - M3 `close()` `end: end + 1` (the audit-corrected valid form; the
//!   v1 shorthand `end,` was a parse error — a build-flip masquerading
//!   as a court flip, audit receipt Section 3): 6 courts FAILED 72/6 —
//!   flatten_court_nested_quotation_intervals,
//!   flatten_court_overlap_mutation_flips_verifier,
//!   depth_boundary_court_64_ok_65_refuses,
//!   scope_court_innermost_containing_interval,
//!   arena_court_second_program_appends_disjoint,
//!   refusal_court_all_seven_spec_rows.
//! - M4 `decode_term` Err arm → `Ok((SlotConst, payload))` fallback
//!   coercion: 3 courts FAILED 75/3 — refusal_court_sigil_rows_typed,
//!   statement_court_encode_decode_validate,
//!   refusal_court_all_seven_spec_rows.
//! - M5 full consistent `VarUni`/`VarExi` code swap: 24 courts FAILED
//!   54/24, including the frozen-code witness
//!   `abi_court_sigil_codes_match_spec`. Scope-court note: the scope
//!   court is invariant under a *consistent* swap (its asserts compare
//!   `Sigil` variants and encode/decode all stay self-consistent), so
//!   its anti-swap protection rides the abi court's frozen-code asserts.
//!
//! Wave 7 (defect repair driven by the wave-6 court receipts,
//! 2026-11-01): the wave-6 receipts name one live code-defect class —
//! a03-wave5 F-1, "a debug-profile arithmetic-overflow panic on hostile
//! bytes in a public parser violates the typed-refusal contract" — the
//! class A13's Wave 6 repaired in `builtins.rs`. Class-audited here:
//! `decode_term` is bit-ops only and every interval subtraction
//! (`verify_intervals_disjoint_and_nested`, `innermost_scope`, `close`)
//! is short-circuit-guarded — no panic path exists. Proven by execution:
//! `hostile_domain_court_interval_validators_total_over_u32_extremes`
//! walks boundary u32 start/end/witness domains x hostile slab lengths
//! through the validators in the debug profile (W7-M7 flip below).
//! The same adversarial reading found F-A11-1, now FIXED: flatten was
//! not transactional — `assign_headers` reserves headers pre-order
//! before `emit` encodes, so any refusal after the reserve phase (depth,
//! payload, register, predicate, interval) left reserved placeholder
//! `(0,0,0,0)` headers and orphaned slab statements behind, and a
//! placeholder `(0,0)` nests inside any header with `start == 0`, so
//! `verify_intervals_disjoint_and_nested` stayed green over the residue
//! and the next `flatten` built on corrupted admitted state. Repair:
//! flatten truncates the slab and header table back to their pre-call
//! lengths on any `Err` (the `A11-W7-ROLLBACK` site); pinned by
//! `flatten_atomicity_court_refused_programs_leave_no_residue` (W7-M6
//! flip below). Mutation campaign receipt (this session, sandbox
//! `/tmp/a11w7-sb`: crate copies byte-identical to canonical
//! `34c12764…`/`6afeb317…` before each shim, baseline 87/87 rc=0, one
//! mutation at a time, restore sha256-verified after each; canonical
//! `unify.rs`/`project.rs`/`lib.rs` sha-unchanged pre/post): 7/7 flip,
//! rc=101 each, zero vacuous —
//!
//! - M1 `s: u64`→`u128` (sigil.rs): build-abort — E0308 x3 + E0080
//!   size/align const-asserts.
//! - M2 `SlotConst = 0x00`→`0x01` (sigil.rs): build-abort — E0081
//!   duplicate discriminant.
//! - M3 `close()` `end`→`end + 1` (arena.rs, the audit-corrected form):
//!   7 courts FAILED 80/7 — the Wave-4 six plus the new atomicity court.
//! - M4 `decode_term` refusal arm → `Ok((SlotConst, …))` fallback
//!   (sigil.rs): 4 courts FAILED 83/4 — refusal_court_all_seven_spec_rows,
//!   hostile_word_court_codec_total_and_typed_over_full_domains,
//!   refusal_court_sigil_rows_typed,
//!   statement_court_encode_decode_validate.
//! - M5 full consistent `VarUni`/`VarExi` swap (sigil.rs): 28 courts
//!   FAILED 59/28 — abi court, hostile word court, round-trip, statement,
//!   scope, and the A12 unify/project courts; a half-swap (M5a) is a
//!   build-abort E0081, matching the wave-4 audit's finding.
//! - W7-M6 rollback neutered (`if false && result.is_err()`, arena.rs):
//!   exactly the atomicity court FAILED 86/1 (isolated flip — the court
//!   is load-bearing and the residue defect is real).
//! - W7-M7 verifier guard reordered so `end - start` evaluates before
//!   its `start > end` guard (arena.rs): exactly the hostile-domain
//!   court FAILED 86/1 (isolated; debug `attempt to subtract with
//!   overflow` at the guard — the F-1 class, caught).
//!
//! Wave 9 (cross-lane integration, 2026-11-01, restarted lane): the five
//! named wave items, each landed at the seam bn3 owns — the restore
//! boundary and the byte image of its frozen ABIs.
//!
//! 1. **A04 identity court** (A04 §1: "memory form and wire form are the
//!    same bytes"; §6.1 LE normative): [`BN3Statement::to_le_bytes`]/
//!    [`BN3FormulaHeader::to_le_bytes`] + inverses — the identity law at
//!    the 24 B / 16 B / whole-arena tiers
//!    (`wave9_a04_identity_court_statement_le_bytes_round_trip`,
//!    `wave9_a04_identity_court_formula_header_le_bytes_round_trip`,
//!    `wave9_arena_image_identity_court_round_trip_both_directions`).
//!    Frame carriage (A04 `CgapFrameHeader` `FLAG_BN3` = 0x04, the bit
//!    A14's penstock pins) is the A14/A19 tier: bn3 has no `cgap-core`
//!    dependency edge and the manifest is off-limits to lanes, so the
//!    cross-crate frame+payload court is UNSUPPORTED in this lane — the
//!    image functions are the seam it ships (recorded divergence, not a
//!    silent gap).
//! 2. **A05 power-cut sweep** (A05 §5: sweep `k in 0..=N`, torn classes
//!    refuse, C4 commits): [`Arena::from_image`] is all-or-nothing —
//!    [`Arena::to_image`] output cut at EVERY `k` refuses typed
//!    `ImageMalformed` with exact demanded/observed lengths, `k = N`
//!    commits (`wave9_a05_powercut_sweep_court_every_cut_refuses_exact_length_commits`).
//! 3. **A14 dam**: the restore boundary refuses hostile images in
//!    layers — framing, per-statement `validate`, reserved-field pad
//!    law, interval gate, FormulaRef scope pass (spec Section 7 row 4 on
//!    the restore path) — exact typed outcome per row
//!    (`wave9_hostile_image_court_typed_refusals_dam_layers`, rows a-j).
//!    Payload-VALUE bit flips are outside the v1 integrity envelope (a
//!    flipped payload reloads to a different but well-formed arena); the
//!    payload-integrity tier is A04's frame `oabi_digest` when the image
//!    rides a frame. Recorded divergence.
//! 4. **A20 1M-row stress**: 1,000,000 real statements (16 quoted
//!    children x 31,250 + 500,000 root) through the REAL `Arena::flatten`,
//!    interval-gated, decode-scanned, imaged, restored, re-serialized
//!    (`wave9_a20_one_million_statement_stress_court`). MEASURED this
//!    session (stated method: debug-profile `std::time::Instant`, this
//!    host, single run): flatten + interval gate = 61 ms per 1M
//!    statements; full court including the 24 MB image round-trip =
//!    548 ms. A TARGET label still applies until a release-profile bench
//!    exists.
//! 5. **A07 zero-string gate**: fleet-level artifact scan re-run this
//!    session — `python3 scripts/zero_string_gate.py` → exit 0, SUMMARY
//!    `scanned=49 crates=22 hits=0 emitter_hits=2 adv_hits=0 vocab=9
//!    source=graph-derivation gate=1.4.0` (the 2 emitter hits are the
//!    registered A07 producer exemption, `EMITTER-EXEMPT-HIT
//!    emit-fibo-pack`). cgap-bn3 artifacts clean.
//!
//! Anti-vacuity campaign (sandbox `/tmp/a11w9-sb.SLlf`, crate copies
//! byte-identical to canonical `6851e106…`/`463f9818…` before each shim,
//! sandbox baseline 101/101 rc=0, one shim at a time, restore
//! sha256-verified after each, canonical tree never shimmed; runner
//! self-check: a deliberately corrupted sandbox copy was DETECTED by the
//! hash gate and restored byte-exact): 6/6 flip, rc=101 each, zero
//! vacuous, union covers all 7 wave9 courts —
//!
//! - M-W9-1 `to_le_bytes` s-word → `to_be_bytes` (sigil.rs): 5 courts
//!   FAILED — the named statement-identity court plus every downstream
//!   image court.
//! - M-W9-1b header `start` → `to_be_bytes` (arena.rs): 4 courts FAILED,
//!   including the named header-identity court.
//! - M-W9-2 interval gate skipped on restore (arena.rs): exactly the
//!   hostile dam court FAILED 100/1 (isolated — rows e/f reload Ok
//!   without the gate).
//! - M-W9-3 framing refusal neutered (`if false &&`): sweep + hostile
//!   FAILED — cut images reload/panic without the law.
//! - M-W9-4 statement `validate` skipped at the boundary: hostile +
//!   stress corrupted-copy arm FAILED.
//! - M-W9-5 FormulaRef scope pass skipped (`.take(0)`): exactly the
//!   hostile dam court FAILED 100/1 (rows h/i/i2 reload Ok).
//!
//! Named extension: `BN3Error::ImageMalformed { expected, got }` — the
//! second error variant beyond the seven-row spec Section 7 table
//! (recorded divergence, sigil.rs), minted for the restore boundary's
//! framing class. Restore-side scope strictness note: a top-level
//! `FormulaRef` (referenced by NO formula) is un-mintable shape — flatten
//! always wraps the program in the root header — so `from_image` refuses
//! it; stricter than the spec's minimum, recorded here.

extern crate alloc;

use crate::sigil::{BN3Error, BN3Statement, Sigil, PAYLOAD_MASK, SIGIL_SHIFT, BN3_STATEMENT_SIZE};
use alloc::vec::Vec;

// --- Wave 1 (frozen arena ABI, spec 4.4) --------------------------------

/// Frozen size of `BN3FormulaHeader` in bytes: four `u32` words.
/// (Spec 4.4; the size court asserts `size_of` equals this.)
pub const BN3_FORMULA_HEADER_SIZE: usize = 16;

/// Maximum quotation nesting depth the flatten accepts before the typed
/// refusal [`BN3Error::FlattenDepthExceeded`] (spec Section 7 row 6,
/// Section 10 risk 2). Frozen in Wave 1 so the refusal court has a
/// day-one constant.
pub const MAX_FORMULA_NESTING_DEPTH: u32 = 64;

/// Statement index width in bits: interval endpoints are `u32` (spec
/// 4.4), so a slab addresses at most 2^32 statements. Flatten enforces
/// this via [`Slab::push`]; the width itself is the frozen ABI.
pub const STATEMENT_INDEX_BITS: u32 = 32;

/// `u32` header-index guard for the formula table (Wave 4 hardening):
/// `FORMULA_REF` payloads are header indices (spec 4.4), and the
/// interval endpoints they resolve against are `u32`, so the header
/// index space is the same frozen `u32` space as the slab's
/// ([`STATEMENT_INDEX_BITS`]). Real function (no mock), mirroring
/// [`stmt_index`], so the overflow refusal is directly court-testable.
fn header_index(len: usize) -> Result<u32, BN3Error> {
    u32::try_from(len).map_err(|_| BN3Error::StatementIndexOverflow)
}

/// One quoted-graph header: 16 bytes, `repr(C)` so the FFI surface
/// (cgap-ffi) can reinterpret without a codec (spec 4.4). A `{ ... }`
/// quotation flattens to its statements appended contiguously to the slab
/// in source order, plus one header. Nesting flattens recursively: a
/// formula inside a formula is a sub-interval of its parent's interval,
/// and `FORMULA_REF` payloads are the *child* header indices (spec 4.4).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BN3FormulaHeader {
    /// Inclusive statement index of the first statement of the quoted
    /// graph in the arena slab.
    pub start: u32,
    /// Exclusive statement index one past the last statement of the
    /// quoted graph. Well-formedness `start <= end` is a flatten
    /// invariant (spec Section 9, item 4), checked by
    /// [`verify_intervals_disjoint_and_nested`].
    pub end: u32,
    /// Redundant size witness for the round-trip court (spec 4.4): the
    /// statement count of the interval, `end - start` when well-formed.
    pub term_words: u32,
    /// Reserved padding to the frozen 16-byte size. Zero-initialized by
    /// flatten; not read.
    pub _pad: u32,
}

impl BN3FormulaHeader {
    /// Wave 9 (A04 identity law at the header ABI): the little-endian
    /// byte image. Field order is the image order (start, end,
    /// term_words, _pad), 4 bytes each — byte 12 of the image is the low
    /// byte of `_pad`, the pin the Wave 9 identity court asserts (a
    /// flatten-produced header has `_pad == 0`, so byte 12 is 0 there).
    pub const fn to_le_bytes(&self) -> [u8; BN3_FORMULA_HEADER_SIZE] {
        let start = self.start.to_le_bytes();
        let end = self.end.to_le_bytes();
        let witness = self.term_words.to_le_bytes();
        let pad = self._pad.to_le_bytes();
        let mut out = [0u8; BN3_FORMULA_HEADER_SIZE];
        let mut i = 0;
        while i < 4 {
            out[i] = start[i];
            out[4 + i] = end[i];
            out[8 + i] = witness[i];
            out[12 + i] = pad[i];
            i += 1;
        }
        out
    }

    /// Reassemble a header from its 16-byte LE image (inverse of
    /// [`BN3FormulaHeader::to_le_bytes`]). Total and unvalidating by
    /// design; framing laws (`_pad == 0`, interval integrity, witness
    /// agreement) ride [`Arena::from_image`], the restore boundary.
    pub const fn from_le_bytes(bytes: [u8; BN3_FORMULA_HEADER_SIZE]) -> BN3FormulaHeader {
        let mut start = [0u8; 4];
        let mut end = [0u8; 4];
        let mut witness = [0u8; 4];
        let mut pad = [0u8; 4];
        let mut i = 0;
        while i < 4 {
            start[i] = bytes[i];
            end[i] = bytes[4 + i];
            witness[i] = bytes[8 + i];
            pad[i] = bytes[12 + i];
            i += 1;
        }
        BN3FormulaHeader {
            start: u32::from_le_bytes(start),
            end: u32::from_le_bytes(end),
            term_words: u32::from_le_bytes(witness),
            _pad: u32::from_le_bytes(pad),
        }
    }
}

// --- ABI freeze: compile-time layout laws (spec 4.4) --------------------
// Any field addition, widening, or reorder must fail here. Size-court
// mutation for anti-vacuity (spec Section 8): add a 5th field or widen
// `start: u32` to `u64` — the build MUST fail.
const _: () = assert!(core::mem::size_of::<BN3FormulaHeader>() == BN3_FORMULA_HEADER_SIZE);
const _: () = assert!(core::mem::align_of::<BN3FormulaHeader>() == 4);
const _: () = assert!(core::mem::offset_of!(BN3FormulaHeader, start) == 0);
const _: () = assert!(core::mem::offset_of!(BN3FormulaHeader, end) == 4);
const _: () = assert!(core::mem::offset_of!(BN3FormulaHeader, term_words) == 8);
const _: () = assert!(core::mem::offset_of!(BN3FormulaHeader, _pad) == 12);
const _: () = assert!(BN3_FORMULA_HEADER_SIZE == 16);
const _: () = assert!(MAX_FORMULA_NESTING_DEPTH == 64);
const _: () = assert!(STATEMENT_INDEX_BITS == 32);

// --- Wave 9 (cross-lane integration): the arena byte image ---------------

/// Byte length of the image framing prefix: two `u32` counts, little-
/// endian (statement count, header count). Wave 9, A04 §6.1 LE convention.
pub const ARENA_IMAGE_PREFIX_LEN: usize = 8;

// --- Wave 2: the flat arena ---------------------------------------------

/// Contiguous statement backing store (spec C4: "Contiguous backing
/// store; u32 statement indices"). Indices are `u32` (spec 4.4 interval
/// endpoints); a push past `u32::MAX` statements is the typed refusal
/// [`BN3Error::StatementIndexOverflow`].
pub struct Slab {
    stmts: Vec<BN3Statement>,
}

/// `u32` statement-index guard: the index space of spec 4.4's interval
/// endpoints. Real function (no mock), so the overflow refusal is
/// directly court-testable without allocating 48 GiB.
fn stmt_index(len: usize) -> Result<u32, BN3Error> {
    u32::try_from(len).map_err(|_| BN3Error::StatementIndexOverflow)
}

impl Slab {
    pub const fn new() -> Slab {
        Slab { stmts: Vec::new() }
    }

    /// Append one statement; returns its `u32` arena index. Refuses with
    /// [`BN3Error::StatementIndexOverflow`] above the `u32` index space.
    pub fn push(&mut self, stmt: BN3Statement) -> Result<u32, BN3Error> {
        let idx = stmt_index(self.stmts.len())?;
        self.stmts.push(stmt);
        Ok(idx)
    }

    /// Statement at `idx`, or `None` out of bounds (total accessor).
    pub fn get(&self, idx: u32) -> Option<BN3Statement> {
        self.stmts.get(idx as usize).copied()
    }

    /// Statement count (fits the `u32` index space by the push guard).
    pub fn len(&self) -> usize {
        self.stmts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty()
    }

    /// The contiguous statement slice (ADR-003: one contiguous slab).
    pub fn statements(&self) -> &[BN3Statement] {
        &self.stmts
    }

    /// Wave 7 (F-A11-1): roll the slab back to `len` statements after a
    /// refused flatten. Private to the transactional path — the atomicity
    /// court and its W7-M6 shim (rollback neutered) prove this is
    /// load-bearing, not decorative.
    fn truncate_to(&mut self, len: usize) {
        self.stmts.truncate(len);
    }
}

/// Formula table (spec C4: `Vec<BN3FormulaHeader>`; "Index intervals
/// [start,end) into the arena; no pointers").
pub struct FormulaTable {
    headers: Vec<BN3FormulaHeader>,
}

impl FormulaTable {
    pub const fn new() -> FormulaTable {
        FormulaTable { headers: Vec::new() }
    }

    /// Reserve a header slot pre-order (flatten phase 1); returns its
    /// index. The slot is a placeholder until phase 2's post-order close
    /// patches `start`/`end`/`term_words`. Refuses past the `u32` header
    /// index space (Wave 4: the same frozen index space as the slab —
    /// no silent `usize`→`u32` truncation remains in the table path).
    fn reserve(&mut self) -> Result<u32, BN3Error> {
        let idx = header_index(self.headers.len())?;
        self.headers.push(BN3FormulaHeader {
            start: 0,
            end: 0,
            term_words: 0,
            _pad: 0,
        });
        Ok(idx)
    }

    /// Patch a reserved header at post-order close. The `end` spans all
    /// descendant statements (module divergence 1). The
    /// `A11-M3-CLOSE` marker is the anti-vacuity shim site: bumping `end`
    /// by +1 here must fail the flatten courts (spec Section 8).
    fn close(&mut self, idx: u32, start: u32, end: u32) -> Result<(), BN3Error> {
        let h = self
            .headers
            .get_mut(idx as usize)
            .ok_or(BN3Error::IntervalOverlap)?;
        *h = BN3FormulaHeader {
            start,
            end, // A11-M3-CLOSE
            term_words: end - start,
            _pad: 0,
        };
        Ok(())
    }

    pub fn get(&self, idx: u32) -> Option<BN3FormulaHeader> {
        self.headers.get(idx as usize).copied()
    }

    pub fn len(&self) -> usize {
        self.headers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// The header slice, ascending by index (flatten assigns pre-order).
    pub fn headers(&self) -> &[BN3FormulaHeader] {
        &self.headers
    }

    /// Wave 7 (F-A11-1): roll the header table back to `len` headers after
    /// a refused flatten (drops reserved-but-never-closed placeholders).
    /// Private to the transactional path; see [`Slab::truncate_to`].
    fn truncate_to(&mut self, len: usize) {
        self.headers.truncate(len);
    }
}

/// Source term: `(sigil, payload)` before word assembly.
pub type SrcTerm = (Sigil, u64);

/// Transient parse surface for [`Arena::flatten`]: a formula node with
/// its direct statements and nested quotations. This tree is input only —
/// the stored arena stays pointer-free (ADR-003).
///
/// A `FormulaRef` term payload in input position is the *child ordinal*
/// (index into [`FormulaNode::children`]); flatten resolves it to the
/// child's reserved header index, so the stored `FORMULA_REF` payload is
/// the child header index exactly as spec 4.4 requires. An ordinal past
/// the end of `children` is [`BN3Error::IntervalOverlap`] (a ref to a
/// non-existent scope — spec Section 7 row 4).
#[derive(Debug, Clone, Default)]
pub struct FormulaNode {
    /// Direct statements in source order, each `[s, p, o]`.
    pub statements: Vec<[SrcTerm; 3]>,
    /// Nested quoted graphs, flattened after this node's statements
    /// (source order).
    pub children: Vec<FormulaNode>,
}

impl FormulaNode {
    /// A leaf formula with statements only.
    pub fn leaf(statements: Vec<[SrcTerm; 3]>) -> FormulaNode {
        FormulaNode {
            statements,
            children: Vec::new(),
        }
    }

    /// A formula with statements and nested quotations.
    pub fn quoted(statements: Vec<[SrcTerm; 3]>, children: Vec<FormulaNode>) -> FormulaNode {
        FormulaNode {
            statements,
            children,
        }
    }
}

/// Flatten assignment (private): the reserved header index of a node and
/// its children's assignments, for `FormulaRef` ordinal resolution.
struct HeaderAssign {
    header: u32,
    children: Vec<HeaderAssign>,
}

/// Flatten phase 1 (pre-order): reserve one header index per node and
/// build the assignment tree. Depth law: root at depth 0, each quotation
/// level +1; depth `> 64` refuses (spec Section 7 row 6 — 64 nested
/// quotation levels legal, 65 refuses).
fn assign_headers(
    node: &FormulaNode,
    ftab: &mut FormulaTable,
    depth: u32,
) -> Result<HeaderAssign, BN3Error> {
    if depth > MAX_FORMULA_NESTING_DEPTH {
        return Err(BN3Error::FlattenDepthExceeded);
    }
    let header = ftab.reserve()?;
    let children = node
        .children
        .iter()
        .map(|child| assign_headers(child, ftab, depth + 1))
        .collect::<Result<Vec<_>, BN3Error>>()?;
    Ok(HeaderAssign { header, children })
}

/// Resolve a `FormulaRef` child ordinal to the reserved child header
/// index; out-of-range ordinals are the typed refusal (spec Section 7
/// row 4: a ref to a non-existent/non-ancestor scope).
fn resolve_ref(term: SrcTerm, assign: &HeaderAssign) -> Result<SrcTerm, BN3Error> {
    if term.0 != Sigil::FormulaRef {
        return Ok(term);
    }
    let child = assign
        .children
        .get(term.1 as usize)
        .ok_or(BN3Error::IntervalOverlap)?;
    Ok((Sigil::FormulaRef, child.header as u64))
}

/// Flatten phase 2: encode + append one node's statements, recurse into
/// children, then close the node's header post-order (module divergence
/// 1: the `end` spans all descendant statements so child ⊆ parent holds,
/// as spec 4.4 and the flatten court demand).
fn emit(
    node: &FormulaNode,
    assign: &HeaderAssign,
    slab: &mut Slab,
    ftab: &mut FormulaTable,
) -> Result<(), BN3Error> {
    let start = stmt_index(slab.len())?;
    for stmt in &node.statements {
        let [s, p, o] = stmt;
        let s = resolve_ref(*s, assign)?;
        let p = resolve_ref(*p, assign)?;
        let o = resolve_ref(*o, assign)?;
        let encoded = BN3Statement::encode_terms(s, p, o)?;
        slab.push(encoded)?;
    }
    for (child, child_assign) in node.children.iter().zip(assign.children.iter()) {
        emit(child, child_assign, slab, ftab)?;
    }
    let end = stmt_index(slab.len())?;
    ftab.close(assign.header, start, end)
}

/// Verify interval integrity over a whole formula table (spec Section 6
/// `verify_intervals_disjoint_and_nested`): every pair of headers is
/// either disjoint or properly nested; each header is well-formed
/// (`start <= end`, witness `term_words == end - start`, and `end` within
/// the slab). Violations are the typed refusal
/// [`BN3Error::IntervalOverlap`] (spec Section 7 row 4).
pub fn verify_intervals_disjoint_and_nested(
    ftab: &[BN3FormulaHeader],
    slab_len: u32,
) -> Result<(), BN3Error> {
    fn nested(outer: &BN3FormulaHeader, inner: &BN3FormulaHeader) -> bool {
        outer.start <= inner.start && inner.end <= outer.end
    }
    fn disjoint(a: &BN3FormulaHeader, b: &BN3FormulaHeader) -> bool {
        a.end <= b.start || b.end <= a.start
    }
    for h in ftab {
        if h.start > h.end || h.term_words != h.end - h.start || h.end > slab_len {
            return Err(BN3Error::IntervalOverlap);
        }
    }
    for i in 0..ftab.len() {
        for j in (i + 1)..ftab.len() {
            let (a, b) = (&ftab[i], &ftab[j]);
            if !(nested(a, b) || nested(b, a) || disjoint(a, b)) {
                return Err(BN3Error::IntervalOverlap);
            }
        }
    }
    Ok(())
}

/// Formula-ref scope check (spec Section 7 row 4): the referenced header
/// must exist and lie within the referencing scope (`None` = top level,
/// which contains every formula). `ref_idx == scope` is trivially inside.
pub fn check_formula_ref(
    ftab: &[BN3FormulaHeader],
    scope: Option<u32>,
    ref_idx: u32,
) -> Result<(), BN3Error> {
    let r = ftab
        .get(ref_idx as usize)
        .ok_or(BN3Error::IntervalOverlap)?;
    match scope {
        None => Ok(()),
        Some(s) => {
            let sc = ftab.get(s as usize).ok_or(BN3Error::IntervalOverlap)?;
            if sc.start <= r.start && r.end <= sc.end {
                Ok(())
            } else {
                Err(BN3Error::IntervalOverlap)
            }
        }
    }
}

/// Innermost containing formula of a statement index (spec Section 5:
/// "a variable register is scoped by the innermost formula interval
/// containing its statement"). Smallest-width header containing
/// `stmt_idx`; `None` only when no header contains it (an empty table or
/// an index past the slab end).
pub fn innermost_scope(ftab: &[BN3FormulaHeader], stmt_idx: u32) -> Option<u32> {
    let mut best: Option<(u32, u32)> = None; // (width, header idx)
    for (idx, h) in ftab.iter().enumerate() {
        if h.start <= stmt_idx && stmt_idx < h.end {
            let width = h.end - h.start;
            if best.map_or(true, |(w, _)| width < w) {
                best = Some((width, idx as u32));
            }
        }
    }
    best.map(|(_, idx)| idx)
}

/// The BN3 arena: slab + formula table, with flatten as the single
/// admitted construction path.
pub struct Arena {
    /// The contiguous statement slab.
    pub slab: Slab,
    /// The formula table of index intervals.
    pub formulas: FormulaTable,
}

impl Arena {
    pub const fn new() -> Arena {
        Arena {
            slab: Slab::new(),
            formulas: FormulaTable::new(),
        }
    }

    /// Flatten a (possibly nested) formula program into the arena (spec
    /// Section 6 `flatten`): statements appended contiguously in source
    /// order, one header per formula, nesting as recursive sub-intervals,
    /// `FormulaRef` child ordinals resolved to child header indices, and
    /// the final whole-table interval gate. Appends to (does not clear)
    /// the arena; the gate runs over the whole table, so separate
    /// programs stay mutually disjoint. Depth cap 64 (spec Section 7
    /// row 6).
    ///
    /// Transactional (Wave 7, F-A11-1): a refused program leaves the arena
    /// unchanged — any `Err` truncates the slab and the header table back
    /// to their pre-call lengths before the error returns, so refused
    /// programs consume no index frontier and no reserved placeholder
    /// header or orphaned statement survives (before this repair a
    /// placeholder `(0,0)` header nested inside any `start == 0` header,
    /// so the interval verifier stayed green over the residue; court:
    /// `flatten_atomicity_court_refused_programs_leave_no_residue`).
    pub fn flatten(&mut self, program: &FormulaNode) -> Result<(), BN3Error> {
        let slab_base = self.slab.len();
        let ftab_base = self.formulas.len();
        let result = match assign_headers(program, &mut self.formulas, 0) {
            Ok(assign) => emit(program, &assign, &mut self.slab, &mut self.formulas).and_then(|()| {
                verify_intervals_disjoint_and_nested(
                    self.formulas.headers(),
                    self.slab.len() as u32,
                )
            }),
            Err(e) => Err(e),
        };
        if result.is_err() {
            // A11-W7-ROLLBACK: refused programs leave no admitted residue.
            self.slab.truncate_to(slab_base);
            self.formulas.truncate_to(ftab_base);
        }
        result
    }

    /// Innermost formula containing `stmt_idx` (spec Section 5 scope law).
    pub fn innermost_scope(&self, stmt_idx: u32) -> Option<u32> {
        innermost_scope(self.formulas.headers(), stmt_idx)
    }

    /// Formula-ref scope check against this arena's table (spec Section 7
    /// row 4).
    pub fn check_formula_ref(&self, scope: Option<u32>, ref_idx: u32) -> Result<(), BN3Error> {
        check_formula_ref(self.formulas.headers(), scope, ref_idx)
    }

    /// Wave 9 (A04 identity law, arena tier): the deterministic byte image
    /// of the whole arena — framing prefix `[u32 n_stmts][u32 n_headers]`
    /// little-endian, then the slab statements in index order, then the
    /// formula headers in index order. `to_image ∘ from_image` is the
    /// identity on flatten-admitted arenas, and re-serializing a restored
    /// arena reproduces the image byte-exact (the court asserts both
    /// directions). Counts fit `u32` by the [`Slab::push`] /
    /// `FormulaTable::reserve` guards.
    pub fn to_image(&self) -> Vec<u8> {
        let n = self.slab.len();
        let m = self.formulas.len();
        let mut image = Vec::with_capacity(ARENA_IMAGE_PREFIX_LEN + n * BN3_STATEMENT_SIZE + m * BN3_FORMULA_HEADER_SIZE);
        image.extend_from_slice(&(n as u32).to_le_bytes());
        image.extend_from_slice(&(m as u32).to_le_bytes());
        for stmt in self.slab.statements() {
            image.extend_from_slice(&stmt.to_le_bytes());
        }
        for header in self.formulas.headers() {
            image.extend_from_slice(&header.to_le_bytes());
        }
        image
    }

    /// Wave 9 (A05 power-cut law + A14 dam, restore boundary): rebuild an
    /// arena from its byte image, all-or-nothing. The refusal order is the
    /// dam's depth layers:
    ///
    /// 1. **framing** — the prefix counts must address the image exactly:
    ///    `len == 8 + 24·n + 16·m` (overflow-free in `u64` for `u32`
    ///    counts); a cut anywhere (A05 §5 classes C1-C3, `k < N`) or one
    ///    byte of trailing slack is [`BN3Error::ImageMalformed`], never a
    ///    half-admitted prefix (C4 "commit" is exact-length-only);
    /// 2. **statements** — every statement passes [`BN3Statement::validate`]
    ///    (reserved sigil → `UnknownSigil`; `FormulaRef` predicate →
    ///    `FormulaInPredicate`);
    /// 3. **reserved field** — a nonzero header `_pad` refuses (the pad is
    ///    zero-initialized by flatten; a nonzero pad is spoliation);
    /// 4. **intervals** — the whole table passes
    ///    [`verify_intervals_disjoint_and_nested`] against the restored
    ///    slab length;
    /// 5. **scope** — every stored `FormulaRef` resolves inside its
    ///    statement's innermost containing formula (spec Section 7 row 4 on
    ///    the restore path). Flatten cannot mint a `FormulaRef` outside
    ///    every formula (the root header always wraps the program), so a
    ///    top-level ref is un-mintable shape and refuses.
    ///
    /// Payload-value bit flips are OUTSIDE the v1 integrity envelope (a
    /// flipped payload byte reloads to a different but well-formed arena):
    /// the payload-integrity tier is A04's frame `oabi_digest`/CRC when the
    /// image rides a `FLAG_BN3` frame. Recorded divergence, not a silent
    /// gap.
    pub fn from_image(bytes: &[u8]) -> Result<Arena, BN3Error> {
        let got = bytes.len() as u64;
        if bytes.len() < ARENA_IMAGE_PREFIX_LEN {
            return Err(BN3Error::ImageMalformed {
                expected: ARENA_IMAGE_PREFIX_LEN as u64,
                got,
            });
        }
        let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let m = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        // `u32` count inputs bound the product: 2^32 · 40 + 8 < 2^38, no
        // `u64` overflow is reachable, so plain arithmetic is total here.
        let declared = ARENA_IMAGE_PREFIX_LEN as u64
            + n as u64 * BN3_STATEMENT_SIZE as u64
            + m as u64 * BN3_FORMULA_HEADER_SIZE as u64;
        if declared != got {
            // The A05 torn-write sweep: EVERY k < N lands here, whatever
            // the cut position — no torn image ever half-admits.
            return Err(BN3Error::ImageMalformed { expected: declared, got });
        }

        // Layer 2: statements, validated at the boundary.
        let mut stmts = Vec::with_capacity(n as usize);
        let mut cursor = ARENA_IMAGE_PREFIX_LEN;
        for _ in 0..n {
            let mut raw = [0u8; BN3_STATEMENT_SIZE];
            raw.copy_from_slice(&bytes[cursor..cursor + BN3_STATEMENT_SIZE]);
            cursor += BN3_STATEMENT_SIZE;
            let stmt = BN3Statement::from_le_bytes(raw);
            stmt.validate()?;
            stmts.push(stmt);
        }

        // Layers 3+4 parse: headers, pad law, then the interval gate.
        let mut headers = Vec::with_capacity(m as usize);
        for _ in 0..m {
            let mut raw = [0u8; BN3_FORMULA_HEADER_SIZE];
            raw.copy_from_slice(&bytes[cursor..cursor + BN3_FORMULA_HEADER_SIZE]);
            cursor += BN3_FORMULA_HEADER_SIZE;
            let header = BN3FormulaHeader::from_le_bytes(raw);
            if header._pad != 0 {
                return Err(BN3Error::ImageMalformed { expected: 0, got: header._pad as u64 });
            }
            headers.push(header);
        }
        verify_intervals_disjoint_and_nested(&headers, n)?;

        // Layer 5: every stored FormulaRef resolves inside its innermost
        // containing formula (spec Section 7 row 4 at the restore bound).
        for (idx, stmt) in stmts.iter().enumerate() {
            let stmt_idx = idx as u32;
            for word in [stmt.s, stmt.o] {
                if (word >> SIGIL_SHIFT) as u8 == Sigil::FormulaRef as u8 {
                    let payload = word & PAYLOAD_MASK;
                    // An index above the u32 header space addresses no
                    // header — refuse, never truncate the payload.
                    if payload > u32::MAX as u64 {
                        return Err(BN3Error::IntervalOverlap);
                    }
                    let scope =
                        innermost_scope(&headers, stmt_idx).ok_or(BN3Error::IntervalOverlap)?;
                    check_formula_ref(&headers, Some(scope), payload as u32)?;
                }
            }
        }

        Ok(Arena {
            slab: Slab { stmts },
            formulas: FormulaTable { headers },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sigil::{
        decode_term, encode_term, BN3_STATEMENT_SIZE, MAX_PAYLOAD, REGISTER_COUNT,
    };
    use core::mem::{align_of, offset_of, size_of};

    /// Real nested program for the courts: root { A; B->Q0 } with
    /// Q0 { C; { D } } — root has two direct statements (the second
    /// referencing the first child quotation by ordinal), Q0 holds one
    /// statement and a deeper grandchild quotation.
    fn nested_program() -> FormulaNode {
        FormulaNode::quoted(
            vec![
                [(
                    Sigil::SlotConst,
                    1,
                ), (Sigil::VarUni, 10), (Sigil::SlotConst, 2)],
                [
                    (Sigil::SlotConst, 3),
                    (Sigil::SlotConst, 4),
                    (Sigil::FormulaRef, 0), // children[0] = Q0
                ],
            ],
            vec![FormulaNode::quoted(
                vec![[(
                    Sigil::VarExi,
                    20,
                ), (Sigil::SlotConst, 5), (Sigil::SlotConst, 6)]],
                vec![FormulaNode::leaf(vec![[(
                    Sigil::SlotConst,
                    7,
                ), (Sigil::SlotConst, 8), (Sigil::SlotConst, 9)]])],
            )],
        )
    }

    /// Size court for the formula header (spec Section 8, size court
    /// second half): 16 bytes, `repr(C)`. Mutation that MUST flip this
    /// court: add a 5th field, or widen `start: u32` to `u64` (build
    /// failure via the const asserts — the primary court).
    #[test]
    fn size_court_formula_header_is_16_bytes() {
        assert_eq!(size_of::<BN3FormulaHeader>(), 16);
        assert_eq!(align_of::<BN3FormulaHeader>(), 4);
        assert_eq!(offset_of!(BN3FormulaHeader, start), 0);
        assert_eq!(offset_of!(BN3FormulaHeader, end), 4);
        assert_eq!(offset_of!(BN3FormulaHeader, term_words), 8);
        assert_eq!(offset_of!(BN3FormulaHeader, _pad), 12);
        // repr(C): no tail padding; four headers pack exactly into one
        // 64-byte cache line.
        assert_eq!(size_of::<[BN3FormulaHeader; 4]>(), 64);
    }

    /// Cross-module ABI court: statement and header sizes compose to the
    /// ADR-003 cache-line arithmetic. Mutation that MUST flip: any field
    /// widening in either struct.
    #[test]
    fn abi_court_arena_composition_laws() {
        assert_eq!(BN3_STATEMENT_SIZE, 24);
        assert_eq!(BN3_FORMULA_HEADER_SIZE, 16);
        assert_eq!(BN3_FORMULA_HEADER_SIZE, size_of::<BN3FormulaHeader>());
        assert_eq!(BN3_STATEMENT_SIZE, size_of::<BN3Statement>());
        // 2 statements (48 B) + 1 header (16 B) = exactly one cache line.
        assert_eq!(
            2 * BN3_STATEMENT_SIZE + BN3_FORMULA_HEADER_SIZE,
            64,
            "ADR-003: 2 statements + 1 header per cache line"
        );
    }

    /// Constant court: the frozen structural constants (spec Sections
    /// 4.4, 5, 7, 10). Mutation that MUST flip: change
    /// `MAX_FORMULA_NESTING_DEPTH` to 32 (a silent tightening of the
    /// quotation-depth refusal boundary).
    #[test]
    fn const_court_arena_laws() {
        assert_eq!(BN3_FORMULA_HEADER_SIZE, 16);
        assert_eq!(MAX_FORMULA_NESTING_DEPTH, 64);
        assert_eq!(STATEMENT_INDEX_BITS, 32);
        // A u32 interval endpoint really can index any statement address
        // the 32-bit index space names.
        assert_eq!(size_of::<u32>() * 8, STATEMENT_INDEX_BITS as usize);
        // Nesting-depth cap fits in the interval field type.
        assert!(MAX_FORMULA_NESTING_DEPTH <= u32::MAX);
    }

    /// Flatten court (spec Section 8): a real nested `{ { ... } }`
    /// program flattened into a real slab. Asserts: child interval ⊆
    /// parent interval; siblings disjoint; every payload ≤ the 56-bit max
    /// (no pointer payload class exists); `FORMULA_REF` payload is the
    /// child header index; scope resolves to the innermost interval.
    ///
    /// Mutation that MUST flip: bump one header's `end` by +1 so it
    /// overlaps a sibling (the `A11-M3-CLOSE` shim) — the witness,
    /// containment, and verify asserts below fail.
    #[test]
    fn flatten_court_nested_quotation_intervals() {
        let mut arena = Arena::new();
        arena.flatten(&nested_program()).expect("legal program flattens");

        // Three formulas: root(0) ⊇ Q0(1) ⊇ Q1(2); 4 statements.
        assert_eq!(arena.formulas.len(), 3);
        assert_eq!(arena.slab.len(), 4);
        let root = arena.formulas.get(0).expect("root header");
        let q0 = arena.formulas.get(1).expect("Q0 header");
        let q1 = arena.formulas.get(2).expect("Q1 header");

        // Child ⊆ parent, recursively (spec 4.4).
        assert!(root.start <= q0.start && q0.end <= root.end, "Q0 ⊆ root");
        assert!(q0.start <= q1.start && q1.end <= q0.end, "Q1 ⊆ Q0");

        // Whole-table law: disjoint or properly nested, no exceptions.
        assert_eq!(
            verify_intervals_disjoint_and_nested(
                arena.formulas.headers(),
                arena.slab.len() as u32
            ),
            Ok(())
        );

        // Size witnesses agree with the intervals.
        assert_eq!(q0.term_words, q0.end - q0.start);
        assert_eq!(q1.term_words, q1.end - q1.start);
        assert_eq!(root.term_words, root.end - root.start);
        assert_eq!(root.end as usize, arena.slab.len());

        // Every payload ≤ the 56-bit max (no pointer payload class).
        for i in 0..arena.slab.len() as u32 {
            let stmt = arena.slab.get(i).expect("in-bounds statement");
            for (_sigil, payload) in stmt.decode_terms().expect("legal words") {
                assert!(payload <= MAX_PAYLOAD);
            }
        }

        // The FORMULA_REF payload in root's second statement is exactly
        // Q0's header index (spec 4.4: refs are the *child* header
        // indices).
        let stmt1 = arena.slab.get(1).expect("second root statement");
        let decoded = stmt1.decode_terms().expect("legal words");
        assert_eq!(decoded[2], (Sigil::FormulaRef, 1)); // Q0 = header 1

        // Scope law (spec Section 5) on the same real arena: statements
        // 0-1 are root's direct body (innermost = root); statement 2 is
        // Q0's; statement 3 is Q1's.
        assert_eq!(arena.innermost_scope(0), Some(0));
        assert_eq!(arena.innermost_scope(1), Some(0));
        assert_eq!(arena.innermost_scope(2), Some(1));
        assert_eq!(arena.innermost_scope(3), Some(2));
        assert_eq!(arena.innermost_scope(4), None); // past the end
    }

    /// Flatten-court in-memory mutation complement: take the green table
    /// and apply the spec's named mutation (bump one header's `end` by
    /// +1) to the real built structure, then demand the verifier refuse
    /// with `IntervalOverlap`. A verifier that accepts the mutated table
    /// is a vacuous gate.
    #[test]
    fn flatten_court_overlap_mutation_flips_verifier() {
        let mut arena = Arena::new();
        arena.flatten(&nested_program()).expect("legal program flattens");
        // Bump Q1's end: now past the slab end with a disagreeing
        // witness — the verifier must name IntervalOverlap.
        let mut headers = arena.formulas.headers().to_vec();
        headers[2].end += 1;
        assert_eq!(
            verify_intervals_disjoint_and_nested(&headers, arena.slab.len() as u32),
            Err(BN3Error::IntervalOverlap)
        );
        // Same bump on the root header: end exceeds the slab.
        let mut headers = arena.formulas.headers().to_vec();
        headers[0].end += 1;
        assert_eq!(
            verify_intervals_disjoint_and_nested(&headers, arena.slab.len() as u32),
            Err(BN3Error::IntervalOverlap)
        );
    }

    /// Refusal court, consolidated over all seven spec Section 7 rows —
    /// each returns its exact typed error on a real surface (encode,
    /// decode, flatten, or the interval/ref validators). Mutation that
    /// MUST flip: replace the `decode_term` refusal arm with a
    /// fallback-const coercion (the `A11-M4-REFUSE` shim) — the
    /// `UnknownSigil` asserts below then fail.
    #[test]
    fn refusal_court_all_seven_spec_rows() {
        // Row 1: Literals with payloads > 56 bits -> PayloadOverflow.
        assert_eq!(
            encode_term(Sigil::SlotConst, MAX_PAYLOAD + 1),
            Err(BN3Error::PayloadOverflow)
        );
        // Rows 2 & 5: > 256 live registers in one scope /
        // @forSome/@forAll with > 256 names -> RegisterExhausted.
        assert_eq!(
            encode_term(Sigil::VarUni, 256),
            Err(BN3Error::RegisterExhausted)
        );
        assert_eq!(
            encode_term(Sigil::VarExi, 300),
            Err(BN3Error::RegisterExhausted)
        );
        // Row 3: quoted graph in predicate slot -> FormulaInPredicate.
        assert_eq!(
            BN3Statement::encode_terms(
                (Sigil::SlotConst, 1),
                (Sigil::FormulaRef, 0),
                (Sigil::SlotConst, 2),
            ),
            Err(BN3Error::FormulaInPredicate)
        );
        // Row 4: formula ref to a non-ancestor scope -> IntervalOverlap.
        // On the real flattened table: scope Q0 (header 1) cannot see
        // root (header 0) — root is not inside Q0.
        let mut arena = Arena::new();
        arena.flatten(&nested_program()).expect("legal program flattens");
        assert_eq!(
            arena.check_formula_ref(Some(1), 0),
            Err(BN3Error::IntervalOverlap)
        );
        // Out-of-bounds ref and out-of-bounds scope refuse too.
        assert_eq!(
            arena.check_formula_ref(Some(1), 9),
            Err(BN3Error::IntervalOverlap)
        );
        assert_eq!(
            arena.check_formula_ref(Some(9), 1),
            Err(BN3Error::IntervalOverlap)
        );
        // ...while the lawful refs hold: Q1 (header 2) is inside Q0.
        assert_eq!(arena.check_formula_ref(Some(1), 2), Ok(()));
        assert_eq!(arena.check_formula_ref(None, 0), Ok(()));
        // Row 6: quotation nested > 64 levels -> FlattenDepthExceeded.
        let mut deep = FormulaNode::leaf(vec![[
            (Sigil::SlotConst, 1),
            (Sigil::SlotConst, 2),
            (Sigil::SlotConst, 3),
        ]]);
        for _ in 0..65 {
            deep = FormulaNode::quoted(Vec::new(), vec![deep]);
        }
        let mut arena = Arena::new();
        assert_eq!(arena.flatten(&deep), Err(BN3Error::FlattenDepthExceeded));
        // Row 7: unknown/reserved sigil codes -> UnknownSigil(code).
        assert_eq!(
            decode_term(0x05u64 << 56),
            Err(BN3Error::UnknownSigil(0x05))
        );
        assert_eq!(
            decode_term((0xC7u64 << 56) | 0xAB),
            Err(BN3Error::UnknownSigil(0xC7))
        );
        // Statement-index-space guard (named extension): the u32 index
        // space of spec 4.4 refuses above 2^32 - 1 statements.
        assert_eq!(
            stmt_index(u32::MAX as usize + 1),
            Err(BN3Error::StatementIndexOverflow)
        );
        assert_eq!(stmt_index(u32::MAX as usize), Ok(u32::MAX));
        // Wave 4 hardening: the header-index guard mirrors the slab
        // guard — the formula table shares the same frozen u32 index
        // space (spec 4.4: FORMULA_REF payloads resolve against u32
        // interval endpoints).
        assert_eq!(
            header_index(u32::MAX as usize),
            Ok(u32::MAX)
        );
        assert_eq!(
            header_index(u32::MAX as usize + 1),
            Err(BN3Error::StatementIndexOverflow)
        );
    }

    /// Depth boundary court (spec Section 7 row 6, Section 10 risk 2):
    /// exactly 64 quotation levels flatten; 65 refuses. Mutation that
    /// MUST flip: lower `MAX_FORMULA_NESTING_DEPTH` (const court and the
    /// const asserts flip).
    #[test]
    fn depth_boundary_court_64_ok_65_refuses() {
        fn chain(levels: u32) -> FormulaNode {
            let mut node =
                FormulaNode::leaf(vec![[
                    (Sigil::SlotConst, 1),
                    (Sigil::SlotConst, 2),
                    (Sigil::SlotConst, 3),
                ]]);
            for _ in 0..levels {
                node = FormulaNode::quoted(Vec::new(), vec![node]);
            }
            node
        }
        let mut arena = Arena::new();
        assert!(arena.flatten(&chain(64)).is_ok(), "64 levels legal");
        let mut arena = Arena::new();
        assert_eq!(
            arena.flatten(&chain(65)),
            Err(BN3Error::FlattenDepthExceeded),
            "65 levels refuse"
        );
    }

    /// Slab court: contiguous push/get over real statements; total
    /// accessor; the u32 index guard. Mutation that MUST flip: drop the
    /// `stmt_index` guard — the overflow assert below fails (and pushes
    /// past 2^32 statements would silently truncate indices).
    #[test]
    fn slab_court_contiguous_push_get_and_index_guard() {
        let mut slab = Slab::new();
        assert!(slab.is_empty());
        let mut idxs = Vec::new();
        for i in 0..5u64 {
            let stmt = BN3Statement::encode_terms(
                (Sigil::SlotConst, i),
                (Sigil::VarUni, i + 1),
                (Sigil::SlotConst, i + 2),
            )
            .expect("legal");
            idxs.push(slab.push(stmt).expect("in-bounds"));
        }
        assert_eq!(idxs, vec![0, 1, 2, 3, 4]);
        assert_eq!(slab.len(), 5);
        assert!(!slab.is_empty());
        // Contiguity: index i reads back statement i (ADR-003).
        for (i, &idx) in idxs.iter().enumerate() {
            assert_eq!(idx as usize, i);
            let stmt = slab.get(idx).expect("in-bounds");
            let decoded = stmt.decode_terms().expect("legal");
            assert_eq!(decoded[0], (Sigil::SlotConst, i as u64));
        }
        assert!(slab.get(5).is_none()); // total accessor
        assert_eq!(slab.statements().len(), 5);
        // u32 index-space guard (named extension, spec 4.4 endpoints).
        assert_eq!(stmt_index(u32::MAX as usize), Ok(u32::MAX));
        assert_eq!(
            stmt_index(u32::MAX as usize + 1),
            Err(BN3Error::StatementIndexOverflow)
        );
    }

    /// Arena append court: a second flatten appends; the whole-table
    /// interval gate stays green; header indices continue monotonically.
    /// Mutation that MUST flip: clearing the table between programs
    /// would renumber headers — the continuity asserts below fail.
    #[test]
    fn arena_court_second_program_appends_disjoint() {
        let mut arena = Arena::new();
        arena
            .flatten(&FormulaNode::leaf(vec![[
                (Sigil::SlotConst, 1),
                (Sigil::SlotConst, 2),
                (Sigil::SlotConst, 3),
            ]]))
            .expect("first program");
        let first_headers = arena.formulas.len();
        let first_slab = arena.slab.len();
        arena
            .flatten(&FormulaNode::leaf(vec![[
                (Sigil::SlotConst, 4),
                (Sigil::SlotConst, 5),
                (Sigil::SlotConst, 6),
            ]]))
            .expect("second program");
        assert_eq!(arena.formulas.len(), first_headers + 1);
        assert_eq!(arena.slab.len(), first_slab + 1);
        let second = arena
            .formulas
            .get(first_headers as u32)
            .expect("new header");
        assert_eq!(second.start as usize, first_slab);
        assert_eq!(
            verify_intervals_disjoint_and_nested(
                arena.formulas.headers(),
                arena.slab.len() as u32
            ),
            Ok(())
        );
    }

    /// Scope court (spec Section 8): a real flattened `@forAll`-inside-`{}`
    /// program resolves variable scope to the innermost containing
    /// interval. Mutation that MUST flip: swap the `VAR_UNI`/`VAR_EXI`
    /// codes (spec's named mutation) — the decoded-sigil asserts below
    /// and the abi court fail.
    #[test]
    fn scope_court_innermost_containing_interval() {
        // Root: two forAll statements in its direct body (the second
        // referencing Q0); Q0 holds an exists statement and the deeper
        // Q1 with a second exists statement.
        let program = FormulaNode::quoted(
            vec![
                [(
                    Sigil::VarUni,
                    1,
                ), (Sigil::SlotConst, 100), (Sigil::SlotConst, 101)],
                [
                    (Sigil::VarUni, 2),
                    (Sigil::SlotConst, 102),
                    (Sigil::FormulaRef, 0),
                ],
            ],
            vec![FormulaNode::quoted(
                vec![[(
                    Sigil::VarExi,
                    3,
                ), (Sigil::SlotConst, 103), (Sigil::SlotConst, 104)]],
                vec![FormulaNode::leaf(vec![[(
                    Sigil::VarExi,
                    4,
                ), (Sigil::SlotConst, 105), (Sigil::SlotConst, 106)]])],
            )],
        );
        let mut arena = Arena::new();
        arena.flatten(&program).expect("legal program");
        // Statement 0 (root body): innermost = root header 0.
        assert_eq!(arena.innermost_scope(0), Some(0));
        // Statement 1 (root body, references Q0): innermost = root.
        assert_eq!(arena.innermost_scope(1), Some(0));
        // Statement 2 (Q0 body): innermost = Q0 = header 1.
        assert_eq!(arena.innermost_scope(2), Some(1));
        // Statement 3 (Q1 body): innermost = Q1 = header 2.
        assert_eq!(arena.innermost_scope(3), Some(2));
        // The variable terms decode as the quantifier kind that was
        // encoded: uni in root bodies, exi inside Q0's quotations.
        let s0 = arena
            .slab
            .get(0)
            .expect("stmt 0")
            .decode_terms()
            .expect("legal");
        assert_eq!(s0[0].0, Sigil::VarUni);
        let s2 = arena
            .slab
            .get(2)
            .expect("stmt 2")
            .decode_terms()
            .expect("legal");
        assert_eq!(s2[0].0, Sigil::VarExi);
        let s3 = arena
            .slab
            .get(3)
            .expect("stmt 3")
            .decode_terms()
            .expect("legal");
        assert_eq!(s3[0].0, Sigil::VarExi);
    }

    /// Wave 7 atomicity court (F-A11-1, this wave's own finding): a
    /// refused flatten leaves the arena byte-unchanged — no reserved
    /// placeholder headers, no orphaned statements — for every refusal
    /// class, and the next lawful flatten lands at exactly the pre-refusal
    /// index frontier. Mutation that MUST flip (W7-M6): neuter the
    /// `A11-W7-ROLLBACK` truncation (`if false && result.is_err()`), and
    /// the first slice-equality assert below fails on the residue.
    #[test]
    fn flatten_atomicity_court_refused_programs_leave_no_residue() {
        fn one(s: u64, p: u64, o: u64) -> [SrcTerm; 3] {
            [
                (Sigil::SlotConst, s),
                (Sigil::SlotConst, p),
                (Sigil::SlotConst, o),
            ]
        }
        let mut arena = Arena::new();
        arena
            .flatten(&FormulaNode::leaf(vec![one(1, 2, 3)]))
            .expect("lawful program flattens");
        let base_stmts = arena.slab.statements().to_vec();
        let base_headers = arena.formulas.headers().to_vec();
        assert_eq!(base_stmts.len(), 1);
        assert_eq!(base_headers.len(), 1);

        // Refusal class 1: depth cap — reserve-phase residue (65 headers
        // reserved before any statement encodes).
        let mut deep = FormulaNode::leaf(vec![one(4, 5, 6)]);
        for _ in 0..65 {
            deep = FormulaNode::quoted(Vec::new(), vec![deep]);
        }
        assert_eq!(
            arena.flatten(&deep),
            Err(BN3Error::FlattenDepthExceeded)
        );
        assert_eq!(arena.slab.statements(), base_stmts.as_slice());
        assert_eq!(arena.formulas.headers(), base_headers.as_slice());

        // Refusal class 2: payload overflow mid-emit — statement 0 lands,
        // statement 1 refuses (the orphaned-statement residue class).
        let overflow = FormulaNode::leaf(vec![
            one(7, 8, 9),
            [
                (Sigil::SlotConst, 10),
                (Sigil::SlotConst, 11),
                (Sigil::SlotConst, MAX_PAYLOAD + 1),
            ],
        ]);
        assert_eq!(arena.flatten(&overflow), Err(BN3Error::PayloadOverflow));
        assert_eq!(arena.slab.statements(), base_stmts.as_slice());
        assert_eq!(arena.formulas.headers(), base_headers.as_slice());

        // Refusal class 3: register exhaustion mid-emit.
        let reg = FormulaNode::leaf(vec![
            one(12, 13, 14),
            [
                (Sigil::VarUni, REGISTER_COUNT as u64),
                (Sigil::SlotConst, 15),
                (Sigil::SlotConst, 16),
            ],
        ]);
        assert_eq!(
            arena.flatten(&reg),
            Err(BN3Error::RegisterExhausted)
        );
        assert_eq!(arena.slab.statements(), base_stmts.as_slice());
        assert_eq!(arena.formulas.headers(), base_headers.as_slice());

        // Refusal class 4: formula-in-predicate on a node WITH a child —
        // the ordinal resolves, then the encode-side p-slot ban refuses.
        let pred = FormulaNode::quoted(
            vec![
                one(17, 18, 19),
                [
                    (Sigil::SlotConst, 20),
                    (Sigil::FormulaRef, 0),
                    (Sigil::SlotConst, 21),
                ],
            ],
            vec![FormulaNode::leaf(vec![one(22, 23, 24)])],
        );
        assert_eq!(
            arena.flatten(&pred),
            Err(BN3Error::FormulaInPredicate)
        );
        assert_eq!(arena.slab.statements(), base_stmts.as_slice());
        assert_eq!(arena.formulas.headers(), base_headers.as_slice());

        // The frontier never moved: the next lawful flatten lands at
        // exactly the pre-refusal indices (header 1, statement 1).
        arena
            .flatten(&FormulaNode::leaf(vec![one(25, 26, 27)]))
            .expect("lawful flatten after refusals");
        assert_eq!(arena.formulas.len(), 2);
        assert_eq!(arena.slab.len(), 2);
        let h1 = arena.formulas.get(1).expect("second header");
        assert_eq!((h1.start, h1.end, h1.term_words), (1, 2, 1));
        assert_eq!(
            verify_intervals_disjoint_and_nested(
                arena.formulas.headers(),
                arena.slab.len() as u32
            ),
            Ok(())
        );
    }

    /// Wave 7 hostile-domain court (receipt driver: a03-wave5 F-1, the
    /// class A13's Wave 6 repaired in `builtins.rs`): the interval and
    /// ref/scope validators are total and typed over hostile u32 domains
    /// in the debug profile, where subtract-underflow actually panics.
    /// Every `end - start` site is short-circuit-guarded; this court pins
    /// that against regression.
    ///
    /// Mutation that MUST flip (W7-M7): reorder the verifier's guard so
    /// `h.end - h.start` is evaluated before the `start > end` guard —
    /// the start > end rows panic (debug underflow) and this court fails.
    #[test]
    fn hostile_domain_court_interval_validators_total_over_u32_extremes() {
        // Boundary domain (deterministic, not exhaustive 2^96): the values
        // where the guarded arithmetic and the endpoint orderings flip.
        let domains = [0u32, 1, 2, u32::MAX / 2, u32::MAX - 1, u32::MAX];
        let slab_lens = [0u32, 1, u32::MAX];
        let benign = BN3FormulaHeader {
            start: 0,
            end: 1,
            term_words: 1,
            _pad: 0,
        };
        let mut calls = 0usize;
        for &slab_len in &slab_lens {
            for &start in &domains {
                for &end in &domains {
                    for &witness in &domains {
                        let h = BN3FormulaHeader {
                            start,
                            end,
                            term_words: witness,
                            _pad: 0,
                        };
                        // Single-header table.
                        match verify_intervals_disjoint_and_nested(&[h], slab_len) {
                            Ok(()) => {}
                            Err(BN3Error::IntervalOverlap) => {}
                            Err(other) => {
                                panic!("hostile header {h:?} slab {slab_len} -> {other:?}")
                            }
                        }
                        calls += 1;
                        // Pairwise path with a fixed benign sibling.
                        match verify_intervals_disjoint_and_nested(&[benign, h], slab_len) {
                            Ok(()) => {}
                            Err(BN3Error::IntervalOverlap) => {}
                            Err(other) => {
                                panic!("hostile pair {h:?} slab {slab_len} -> {other:?}")
                            }
                        }
                        calls += 1;
                        // Ref/scope validators over the same hostile table.
                        match check_formula_ref(&[h], Some(0), 0) {
                            Ok(()) => {}
                            Err(BN3Error::IntervalOverlap) => {}
                            Err(other) => {
                                panic!("hostile scope check {h:?} -> {other:?}")
                            }
                        }
                        calls += 1;
                        assert_eq!(
                            check_formula_ref(&[h], Some(u32::MAX), u32::MAX),
                            Err(BN3Error::IntervalOverlap),
                            "out-of-bounds scope and ref must refuse"
                        );
                        calls += 1;
                        if let Some(idx) = innermost_scope(&[h], start) {
                            assert!(idx < 1, "innermost index out of table");
                        }
                        calls += 1;
                    }
                }
            }
        }
        // Domain-size pin: shrinking the walk must be a visible edit, not
        // a silent erosion of the court (anti-vacuity against domain loss).
        assert_eq!(calls, 3 * 6 * 6 * 6 * 5);
    }

    // ===================================================================
    // Wave 9 — cross-lane integration slice
    // (A04 identity court, A05 power-cut sweep, A14 dam, A20 1M stress;
    // the A07 zero-string gate is a fleet-level artifact scan, receipt in
    // the module doc. Mutations M-W9-1..M-W9-5 run in a /tmp sandbox, one
    // shim at a time, restores sha256-verified; canonical never shimmed.)
    // ===================================================================

    use std::time::Instant;

    /// Build an arbitrary (possibly lying) image from parts — the
    /// hostile-image fixture helper. `n`/`m` are the DECLARED counts and
    /// may disagree with the actual part lengths.
    fn image_with(n: u32, m: u32, stmts: &[BN3Statement], headers: &[BN3FormulaHeader]) -> Vec<u8> {
        let mut image = Vec::new();
        image.extend_from_slice(&n.to_le_bytes());
        image.extend_from_slice(&m.to_le_bytes());
        for st in stmts {
            image.extend_from_slice(&st.to_le_bytes());
        }
        for h in headers {
            image.extend_from_slice(&h.to_le_bytes());
        }
        image
    }

    /// A04 identity law at the 16-byte header ABI (the statement-level
    /// twin runs in sigil.rs). Real collaborator: headers produced by a
    /// real flatten of the real nested program. Asserts round-trip
    /// identity, the field/byte map (bytes 0-3 start, 4-7 end, 8-11
    /// witness, 12-15 pad, all LE), and the pad-zero law on
    /// flatten-produced headers (image byte 12 is 0). Also pins that the
    /// IMAGE functions are pure representation — a nonzero-pad header
    /// round-trips (from(to(h)) == h) while the pad LAW lives at the
    /// restore boundary (`from_image`, hostile court case g).
    /// Mutation M-W9-1b (header `to_be_bytes` shim) MUST flip the LE pins.
    #[test]
    fn wave9_a04_identity_court_formula_header_le_bytes_round_trip() {
        let mut arena = Arena::new();
        arena
            .flatten(&nested_program())
            .unwrap_or_else(|e| panic!("nested program must flatten: {e}"));
        let headers: Vec<BN3FormulaHeader> = arena.formulas.headers().to_vec();
        assert_eq!(headers.len(), 3);
        for h in &headers {
            let image = h.to_le_bytes();
            assert_eq!(image.len(), BN3_FORMULA_HEADER_SIZE);
            assert_eq!(BN3FormulaHeader::from_le_bytes(image), *h);
            // Field/byte map + LE pins.
            assert_eq!(u32::from_le_bytes([image[0], image[1], image[2], image[3]]), h.start);
            assert_eq!(u32::from_le_bytes([image[4], image[5], image[6], image[7]]), h.end);
            assert_eq!(u32::from_le_bytes([image[8], image[9], image[10], image[11]]), h.term_words);
            assert_eq!(u32::from_le_bytes([image[12], image[13], image[14], image[15]]), h._pad);
            // Flatten zero-initializes the pad: the wire image of an
            // admitted header carries zeros in the reserved field.
            assert_eq!(h._pad, 0);
            assert_eq!(image[12], 0);
        }
        // Representation identity is pad-blind; the law is at from_image.
        let padded = BN3FormulaHeader { start: 1, end: 2, term_words: 1, _pad: 0xDEAD_BEEF };
        assert_eq!(BN3FormulaHeader::from_le_bytes(padded.to_le_bytes()), padded);
    }

    /// A04 identity law, arena tier: `to_image ∘ from_image` is the
    /// identity on a flatten-admitted arena, in both directions — slice
    /// equality AND byte-exact re-serialization. Also the empty-arena
    /// degenerate: image is the bare 8-byte prefix and restores to an
    /// empty arena.
    #[test]
    fn wave9_arena_image_identity_court_round_trip_both_directions() {
        let mut arena = Arena::new();
        arena
            .flatten(&nested_program())
            .unwrap_or_else(|e| panic!("nested program must flatten: {e}"));
        let image = arena.to_image();
        assert_eq!(image.len(), ARENA_IMAGE_PREFIX_LEN + 4 * BN3_STATEMENT_SIZE + 3 * BN3_FORMULA_HEADER_SIZE);
        // Prefix pins: counts LE, then statements, then headers.
        assert_eq!(image[0..4], 4u32.to_le_bytes());
        assert_eq!(image[4..8], 3u32.to_le_bytes());

        let restored = Arena::from_image(&image)
            .unwrap_or_else(|e| panic!("own image must restore: {e}"));
        assert_eq!(restored.slab.statements(), arena.slab.statements());
        assert_eq!(restored.formulas.headers(), arena.formulas.headers());
        // Reverse direction: re-serialization is byte-exact.
        assert_eq!(restored.to_image(), image);

        // Degenerate: the empty arena images to the bare prefix.
        let empty = Arena::new();
        let empty_image = empty.to_image();
        assert_eq!(empty_image.len(), ARENA_IMAGE_PREFIX_LEN);
        let restored_empty = Arena::from_image(&empty_image)
            .unwrap_or_else(|e| panic!("empty image must restore: {e}"));
        assert!(restored_empty.slab.is_empty());
        assert!(restored_empty.formulas.is_empty());
    }

    /// A05 power-cut sweep, arena-image form (A05 §5: sweep `k in 0..=N`,
    /// torn classes refuse, only C4-complete commits). Every cut `k < N`
    /// refuses typed [`BN3Error::ImageMalformed`] with the exact declared
    /// vs observed lengths; `k == N` commits to an arena equal to the
    /// original (C4 is a commit, not a corruption). No half-admission at
    /// ANY offset — the restore boundary is all-or-nothing.
    /// Mutation M-W9-3 (neutered framing refusal) MUST flip the sweep.
    #[test]
    fn wave9_a05_powercut_sweep_court_every_cut_refuses_exact_length_commits() {
        let mut arena = Arena::new();
        arena
            .flatten(&nested_program())
            .unwrap_or_else(|e| panic!("nested program must flatten: {e}"));
        let image = arena.to_image();
        let n_bytes = image.len();
        assert_eq!(n_bytes, ARENA_IMAGE_PREFIX_LEN + 4 * BN3_STATEMENT_SIZE + 3 * BN3_FORMULA_HEADER_SIZE);

        let mut committed = 0;
        let mut torn = 0;
        for k in 0..=n_bytes {
            let result = Arena::from_image(&image[..k]);
            if k == n_bytes {
                let restored = result.unwrap_or_else(|e| panic!("k=N must commit: {e}"));
                assert_eq!(restored.slab.statements(), arena.slab.statements());
                assert_eq!(restored.formulas.headers(), arena.formulas.headers());
                committed += 1;
            } else {
                match result {
                    Err(BN3Error::ImageMalformed { expected, got }) => {
                        // The demanded length is the law active at the
                        // cut: below the prefix the framing demands the
                        // 8-byte count prefix; at or above it, the
                        // declared total.
                        let want = if k < ARENA_IMAGE_PREFIX_LEN {
                            ARENA_IMAGE_PREFIX_LEN as u64
                        } else {
                            n_bytes as u64
                        };
                        assert_eq!(expected, want, "k={k}");
                        assert_eq!(got, k as u64, "k={k}");
                        torn += 1;
                    }
                    Err(e) => panic!("k={k} must refuse ImageMalformed, got {e:?}"),
                    Ok(_) => panic!("k={k} must refuse, committed instead"),
                }
            }
        }
        // Domain-size pins: the sweep covered every offset, exactly one
        // commit, all others torn. Shrinking the sweep must be a visible
        // edit (anti-vacuity against domain loss, A05 §8).
        assert_eq!(committed, 1);
        assert_eq!(torn, n_bytes);
    }

    /// The dam at the restore boundary (A14's typed-refusal role over
    /// bn3's own medium): layered hostile-image sweep, exact typed
    /// outcome per row. Mutations: M-W9-2 (interval verify skipped on
    /// restore) flips rows e/f; M-W9-4 (statement validate skipped)
    /// flips rows c/d; M-W9-5 (FormulaRef scope pass skipped) flips
    /// rows h/i; M-W9-3 flips rows a/b/g/j.
    #[test]
    fn wave9_hostile_image_court_typed_refusals_dam_layers() {
        let ok_stmt = |i: u64| {
            BN3Statement::from_words(
                (0x00u64 << 56) | i,
                (0x04u64 << 56) | i,
                (0x00u64 << 56) | i,
            )
        };

        // (a) count-prefix lie: declared n_stmts = actual + 1.
        let mut img = image_with(2, 0, &[ok_stmt(1)], &[]);
        img[0] = 2; // declared n = 2, actual 1
        let Err(BN3Error::ImageMalformed { expected, got }) = Arena::from_image(&img) else {
            panic!("(a) must refuse");
        };
        assert_eq!((expected, got), (56, 32));

        // (b) counts that cannot address any image (u32::MAX regions).
        let img = image_with(u32::MAX, u32::MAX, &[], &[]);
        let declared_max = ARENA_IMAGE_PREFIX_LEN as u64
            + u32::MAX as u64 * BN3_STATEMENT_SIZE as u64
            + u32::MAX as u64 * BN3_FORMULA_HEADER_SIZE as u64;
        let Err(BN3Error::ImageMalformed { expected, got }) = Arena::from_image(&img) else {
            panic!("(b) must refuse");
        };
        assert_eq!((expected, got), (declared_max, 8));

        // (c) reserved sigil byte in a statement (F1-class aging
        // corruption) → UnknownSigil, exact code. The sigil is the TOP
        // byte of the s word = statement byte 7 (LE image, byte 0 is the
        // payload LSB — a payload-byte flip is OUTSIDE the v1 envelope,
        // the recorded divergence).
        let mut img = image_with(1, 0, &[ok_stmt(1)], &[]);
        let s_at = img.len() - BN3_STATEMENT_SIZE;
        img[s_at + 7] = 0xF7;
        let Err(BN3Error::UnknownSigil(0xF7)) = Arena::from_image(&img) else {
            panic!("(c) must refuse UnknownSigil(0xF7)");
        };

        // (d) FormulaRef in the predicate slot → FormulaInPredicate. The
        // p-word sigil byte is statement byte 8+7 = 15 (top byte of the
        // LE word).
        let mut img = image_with(1, 0, &[ok_stmt(1)], &[]);
        img[ARENA_IMAGE_PREFIX_LEN + 8 + 7] = 0x03;
        let Err(BN3Error::FormulaInPredicate) = Arena::from_image(&img) else {
            panic!("(d) must refuse FormulaInPredicate");
        };

        // (e) header end beyond the restored slab → IntervalOverlap.
        let mut img = image_with(1, 1, &[ok_stmt(1)], &[BN3FormulaHeader {
            start: 0,
            end: 1,
            term_words: 1,
            _pad: 0,
        }]);
        let h_at = img.len() - BN3_FORMULA_HEADER_SIZE;
        img[h_at + 4] = 0xFF; // end = 0x000000FF > n=1
        let Err(BN3Error::IntervalOverlap) = Arena::from_image(&img) else {
            panic!("(e) must refuse IntervalOverlap");
        };

        // (f) genuinely overlapping sibling intervals (neither nested
        // nor disjoint) → IntervalOverlap.
        let img = image_with(
            3,
            2,
            &[ok_stmt(1), ok_stmt(2), ok_stmt(3)],
            &[
                BN3FormulaHeader { start: 0, end: 2, term_words: 2, _pad: 0 },
                BN3FormulaHeader { start: 1, end: 3, term_words: 2, _pad: 0 },
            ],
        );
        let Err(BN3Error::IntervalOverlap) = Arena::from_image(&img) else {
            panic!("(f) must refuse IntervalOverlap");
        };

        // (g) spoliated reserved field → ImageMalformed{expected: 0}.
        let mut img = image_with(1, 1, &[ok_stmt(1)], &[BN3FormulaHeader {
            start: 0, end: 1, term_words: 1, _pad: 0,
        }]);
        let h_at = img.len() - BN3_FORMULA_HEADER_SIZE;
        img[h_at + 12] = 0x2C;
        let Err(BN3Error::ImageMalformed { expected, got }) = Arena::from_image(&img) else {
            panic!("(g) must refuse");
        };
        assert_eq!((expected, got), (0, 0x2C));

        // (h) un-mintable shape: a top-level FormulaRef with NO formulas
        // (flatten always wraps the program in the root header) →
        // IntervalOverlap at the scope pass.
        let img = image_with(
            1,
            0,
            &[BN3Statement::from_words(
                (0x03u64 << 56) | 0, // FormulaRef(0) in s
                (0x04u64 << 56) | 1,
                (0x00u64 << 56) | 2,
            )],
            &[],
        );
        let Err(BN3Error::IntervalOverlap) = Arena::from_image(&img) else {
            panic!("(h) must refuse IntervalOverlap");
        };

        // (i) ref to a non-ancestor scope: statement 0 sits in header A
        // (0,1) but references header C (2,3), which A does not contain
        // (spec Section 7 row 4 on the restore path).
        let img = image_with(
            4,
            3,
            &[
                BN3Statement::from_words(
                    (0x03u64 << 56) | 2, // FormulaRef(2) = header C
                    (0x04u64 << 56) | 1,
                    (0x00u64 << 56) | 3,
                ),
                ok_stmt(1),
                ok_stmt(2),
                ok_stmt(3),
            ],
            &[
                BN3FormulaHeader { start: 0, end: 1, term_words: 1, _pad: 0 },
                BN3FormulaHeader { start: 1, end: 3, term_words: 2, _pad: 0 },
                BN3FormulaHeader { start: 2, end: 3, term_words: 1, _pad: 0 },
            ],
        );
        let Err(BN3Error::IntervalOverlap) = Arena::from_image(&img) else {
            panic!("(i) must refuse IntervalOverlap");
        };

        // (i2) ref payload above the u32 header-index space: refuse, never
        // truncate the payload to a "valid" index.
        let img = image_with(
            1,
            1,
            &[BN3Statement::from_words(
                (0x03u64 << 56) | (u32::MAX as u64 + 1), // 2^32: no header
                (0x04u64 << 56) | 1,
                (0x00u64 << 56) | 2,
        )],
            &[BN3FormulaHeader { start: 0, end: 1, term_words: 1, _pad: 0 }],
        );
        let Err(BN3Error::IntervalOverlap) = Arena::from_image(&img) else {
            panic!("(i2) must refuse IntervalOverlap");
        };

        // (j) one byte of trailing slack — the exact-length law bites in
        // the long direction too.
        let mut img = image_with(1, 0, &[ok_stmt(1)], &[]);
        img.push(0x00);
        let Err(BN3Error::ImageMalformed { expected, got }) = Arena::from_image(&img) else {
            panic!("(j) must refuse");
        };
        assert_eq!((expected, got), (32, 33));
    }

    /// A20 1M-row stress, arena form: 1,000,000 real statements (16
    /// quoted children x 31,250 + 500,000 root statements) flattened
    /// through the REAL `Arena::flatten`, interval-gated, decode-scanned,
    /// imaged, restored, and re-serialized. Perf is MEASURED with the
    /// stated method (debug-profile `Instant`, this host) and reported in
    /// the module receipt — a TARGET label until a release-profile bench
    /// exists. Mutation M-W9-4 (validate skipped at the restore boundary)
    /// flips the corrupted-copy arm.
    #[test]
    fn wave9_a20_one_million_statement_stress_court() {
        const CHILDREN: usize = 16;
        const PER_CHILD: usize = 31_250;
        const ROOT_STMTS: usize = 500_000;
        let root_stmts: Vec<[SrcTerm; 3]> = (0..ROOT_STMTS)
            .map(|i| {
                [
                    (Sigil::SlotConst, i as u64),
                    (Sigil::VarUni, (i & 0xFF) as u64),
                    (Sigil::SlotConst, i as u64),
                ]
            })
            .collect();
        let children: Vec<FormulaNode> = (0..CHILDREN)
            .map(|c| {
                FormulaNode::leaf(
                    (0..PER_CHILD)
                        .map(|i| {
                            [
                                (Sigil::SlotConst, (c * PER_CHILD + i) as u64),
                                (Sigil::SlotConst, 7),
                                (Sigil::SlotConst, 9),
                            ]
                        })
                        .collect(),
                )
            })
            .collect();
        let program = FormulaNode::quoted(root_stmts, children);

        let mut arena = Arena::new();
        let t0 = Instant::now();
        arena
            .flatten(&program)
            .unwrap_or_else(|e| panic!("1M program must flatten: {e}"));
        let flatten_time = t0.elapsed();

        // Structural outcome pins.
        assert_eq!(arena.slab.len(), 1_000_000);
        assert_eq!(arena.formulas.len(), 1 + CHILDREN);
        assert_eq!(
            verify_intervals_disjoint_and_nested(arena.formulas.headers(), 1_000_000),
            Ok(())
        );
        // Every statement decodes valid (3M words through the codec).
        for (i, stmt) in arena.slab.statements().iter().enumerate() {
            if i % 10_000 == 0 || i + 1 == arena.slab.len() {
                assert_eq!(stmt.validate(), Ok(()), "stmt {i}");
            }
        }
        // Scope law spot checks: root header is 0; children 1..=16.
        assert_eq!(arena.innermost_scope(0), Some(0));
        assert_eq!(arena.innermost_scope(500_000), Some(1));
        assert_eq!(arena.innermost_scope(999_999), Some(16));

        // Image identity at the stress scale + the corrupted-copy arm.
        let image = arena.to_image();
        let restored = Arena::from_image(&image)
            .unwrap_or_else(|e| panic!("stress image must restore: {e}"));
        assert_eq!(restored.slab.statements(), arena.slab.statements());
        assert_eq!(restored.to_image(), image);
        let mut corrupted = image.clone();
        // Last statement's s-word sigil byte: statements START at the
        // prefix and END before the 17 header records — `len - 24` would
        // land inside the header region, not on the last statement.
        let last_at = ARENA_IMAGE_PREFIX_LEN + (1_000_000 - 1) * BN3_STATEMENT_SIZE;
        corrupted[last_at + 7] = 0xF7;
        let Err(BN3Error::UnknownSigil(0xF7)) = Arena::from_image(&corrupted) else {
            panic!("corrupted copy must refuse UnknownSigil(0xF7)");
        };
        let total = t0.elapsed();
        println!(
            "wave9 a20 stress: flatten+verify 1,000,000 statements in {flatten_time:?} \
             (full court incl. 24MB image round-trip: {total:?}); \
             slab bytes = {}",
            arena.slab.len() * BN3_STATEMENT_SIZE
        );
    }
}
