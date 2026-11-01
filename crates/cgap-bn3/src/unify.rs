//! Register-file ABI (Wave 1 freeze) + Wave 2 branchless-selection kernel
//! for the SIMD unifier. Spec: `docs/jira/v26.11.1/lanes/A12-simd-unifier.md`
//! Sections 4.2-4.4 and 7, with the statement ABI **cited, not redefined**
//! from `docs/jira/v26.11.1/lanes/A11-bn3-arena.md` Sections 4.1-4.3
//! (`crate::sigil::BN3Statement` / term words, frozen in `sigil.rs`).
//!
//! Wave 1 froze the register ABI: band partition, widths, and the typed
//! error surface, pinned by compile-time asserts. Wave 2 (this file now)
//! adds the core engines: [`RuleCompiler`] (rule words -> register
//! assignments), the per-atom mask kernels over columnar term words, the
//! nested-loop join (spec 3.2, chosen for v1), selection emission, and the
//! fixpoint driver that feeds the [`crate::project::ConsequentProjector`].
//!
//! Authority note (spec A11 Section 5): A11 guarantees the 8-bit term-word
//! payload transports a register number `R_n`, `n` in `0..=255`; the
//! *binding-band* semantics — which registers are constants, variables,
//! and scratch — are adjudicated here (A12 is the register authority).
//!
//! Recorded divergences (spec yields to authoritative lanes / reality):
//!
//! 1. **Input shape**: spec 4.3 step 1 says Arrow record batches
//!    (`UInt8`/`UInt32`/dictionary columns). `cgap-bn3` has no
//!    `cgap-arrow` dependency and the manifest is coordinator-owned, so
//!    Wave 2 evaluates over [`StatementColumns`] — SoA slices of A11 term
//!    words. The Arrow binding is the Wave-3 integration seam; the M12-6
//!    determinism court already pins chunk-partition independence, which
//!    is the property Arrow chunking needs.
//! 2. **Statement layout**: spec 4.1's 24-byte record
//!    (`s: u64, p: u64, object: u32, flags: u16`) is superseded by the
//!    authoritative A11 layout (three 64-bit term words, no pad field).
//!    Consequence: the spec 7.3 M12-4 pad-corruption mutation has no
//!    subject under A11 — recorded N/A, not silently dropped.
//! 3. **Join micro-architecture**: spec 4.3 step 3 scatters bound values
//!    into scratch registers via a masked gather. Wave 2 implements the
//!    spec 3.2 v1 strategy — nested-loop with mask AND — where the join
//!    probe is a value-equality test of the candidate row against the
//!    match's bound values. The masked-gather form is the BM0-gated
//!    Wave-3 micro-architecture; M12-3's join-swap mutation class still
//!    applies (swapping the probe direction flips the differential court).
//! 4. **Compaction**: spec 4.3 step 4 wants prefix-sum branch-free
//!    compaction. Wave 2 compacts by mask test during match construction
//!    (correctness first); the prefix-sum form lands with the BM0 court
//!    in Wave 3, where it is measurable.
//!
//! Wave 4 (hardening + DoD closure):
//!
//! 5. **Zero-allocation scan+project path** (spec 7.4, DoD 6): every heap
//!    buffer the per-chunk evaluation touches now lives in
//!    [`EvalScratch`], reused across chunks, rounds, and court runs; the
//!    projector gained [`crate::project::ConsequentProjector::emit_into`]
//!    over caller-owned [`crate::project::DeltaRows`] + canonical-byte
//!    buffers. After a warmup run sizes the buffers on identical input, a
//!    second scan+project run touches the allocator zero times — courted
//!    below with a counting global allocator (house pattern:
//!    `crates/cgap-parquet/tests/page_zero_alloc.rs`). The mutation "put
//!    per-call allocation back" MUST flip that court. Evidence standing
//!    (Wave-5, 2026-11-01): the Wave-4 lane report this note previously
//!    cited is absent from the repo (`docs/jira/v26.11.1/waves/` holds
//!    only the a11/a17/a20 receipts), so the claim rests on the Wave-5
//!    re-execution: shim `W5-M7` re-introduced a per-call allocation in
//!    `atom_local_match_into` and flipped
//!    `zero_alloc_court_scan_project_hot_path_allocates_zero_after_warmup`
//!    (rc=101), with byte-exact restore. The full Wave-5 shim ledger
//!    (M12-1/3/5/6/7/8 + 7.4 + A20-M10/M11 replays, 8/8 FLIP, 0 vacuous)
//!    is recorded in the lane's Wave-5 report.
//! 6. **no_std decision (DoD 1, explicit — registered, not hand-waved)**:
//!    `cgap-bn3` targets `std` for v1. The BM0 perf court needs hosted
//!    hardware counters, the digest needs `Vec`, and the court binary
//!    needs the std test harness. The seam is real, not aspirational:
//!    after Wave 4, all hot-path heap use funnels through [`EvalScratch`]
//!    and the caller-owned output buffers, so a `no_std` port is a
//!    manifest/feature change (swapping `std` for `alloc` in
//!    [`EvalScratch`] and the row stores), not a kernel rewrite. The port
//!    itself is out of scope for v26.11.1.
//! 7. **`naive.rs` as a feature-gated file**: the spec's test-support
//!    reference unifier lives in this file's `#[cfg(test)]` module (the
//!    `reference-unifier` feature would require editing
//!    `Cargo.toml`/`lib.rs`, which are coordinator-owned). The differential
//!    court is unchanged — it runs the real naive recursive unifier as
//!    ground truth; only the file/feature packaging diverges.
//! 8. **`benches/branchy_baseline.rs`**: not created — a new file outside
//!    this lane's dispatch scope, and the BM0 ratio fence additionally
//!    needs hardware counters the court host must expose (receipted
//!    separately). Registered as BLOCKED scope, not silently dropped.
//!
//! Wave 9 (cross-lane integration, v26.11.1 wave 9): the scale court —
//! one million rows through the fixpoint driver + projector
//! (`stress_court_1m_rows_converges_chunk_stable_matches_closed_form_model`)
//! with an independent closed-form model as oracle, per-rule/per-round
//! popcount equality, derived-set equality, and M12-6 chunk-partition
//! stability AT SCALE (chunk sizes 262144 vs [`DEFAULT_CHUNK_ROWS`]). The
//! wave-9 emission-side seams (A04 frame identity, A05 power-cut WAL
//! record codec + per-offset sweep) live in `project.rs` where the
//! [`crate::project::DeltaBatch`] identity is minted.

use crate::project::{ConsequentProjector, DeltaBatch, DeltaRows};
use crate::sigil::{self, Sigil};
use std::collections::{HashMap, HashSet};

/// Total width of the virtual register file: `R0..=R255`, the full 8-bit
/// register-number space the `Sigil::VarUni`/`VarExi` payloads transport.
/// (Spec 4.2; A11 Section 5.)
pub const REGISTER_COUNT: usize = 256;

/// Width of one virtual register: 8 bytes = one `u64` lane group.
/// (Spec 4.2: "256 virtual registers, each 8 bytes".)
pub const REGISTER_WIDTH_BYTES: usize = 8;

/// Total bytes of the register file: 256 x 8 = 2 KiB — an L1-resident
/// working set. (Arithmetic consequence of spec 4.2.)
pub const REGISTER_FILE_BYTES: usize = REGISTER_COUNT * REGISTER_WIDTH_BYTES;

/// Size of the constant-pool band `R0..=R63`: interned values loaded from
/// the rule's constant bindings at rule-compile time. (Spec 4.2.)
pub const CONST_POOL_REGS: usize = 64;

/// Size of the variable band `R64..=R191`: one register per distinct rule
/// variable. (Spec 4.2.)
pub const VAR_REGS: usize = 128;

/// Size of the scratch band `R192..=R255`: mask temporaries, join scatter
/// targets, consequent output staging. (Spec 4.2.)
pub const SCRATCH_REGS: usize = 64;

/// First register number of the variable band (`R64`). (Spec 4.2.)
pub const VAR_BAND_FIRST: usize = 64;

/// First register number of the scratch band (`R192`). (Spec 4.2.)
pub const SCRATCH_BAND_FIRST: usize = 192;

/// Maximum distinct variables a single rule may bind: the variable band
/// width. Exceeding this is the typed `UnifyError::RegisterOverflow`
/// (spec 4.2: "max 128 variables per rule; the corpus generator never
/// exceeds 8").
pub const MAX_RULE_VARIABLES: usize = VAR_REGS;

/// Default rows per atom-0 scan chunk (spec 4.3 step 1: batches arrive
/// chunked; the M12-6 court proves the partition does not change output).
pub const DEFAULT_CHUNK_ROWS: usize = 4096;

/// Default fixpoint round budget. A rule corpus that has not converged by
/// then is refused with `FixpointBudgetExceeded`, never spun forever.
pub const DEFAULT_MAX_ROUNDS: u32 = 64;

// --- ABI freeze: compile-time layout laws (spec 4.2) --------------------
// The Wave-1 gate's size/align pins. The struct/const definitions above are
// the frozen register ABI; these `const` asserts make any width, band-
// partition, or A11-transport drift a BUILD failure, not a court surprise
// (house pattern: `crate::sigil` lines 407-425,
// `crates/cgap-arrow/src/layout.rs`, `project.rs` lines 147-149). This
// block closes the gap this lane's Wave-1 slice names: the file's docstring
// claimed compile-time pins that were not on disk (grep `const _` = 0
// before this block).
//
// Anti-vacuity mutations (build MUST fail, E0080):
//   - widen `REGISTER_WIDTH_BYTES` 8 -> 16 (register-width + file-bytes
//     laws flip);
//   - shrink `VAR_REGS` 128 -> 127 (band-partition closure law flips).
const _: () = assert!(REGISTER_COUNT == 256);
// A11 transport width: the 8-bit VarUni/VarExi payload space this file's
// registers occupy (cite, not redefine — dispatch law).
const _: () = assert!(REGISTER_COUNT == crate::sigil::REGISTER_COUNT);
const _: () = assert!(REGISTER_WIDTH_BYTES == core::mem::size_of::<u64>());
const _: () = assert!(REGISTER_FILE_BYTES == REGISTER_COUNT * REGISTER_WIDTH_BYTES);
const _: () = assert!(REGISTER_FILE_BYTES == 2048);
const _: () = assert!(CONST_POOL_REGS == 64);
const _: () = assert!(VAR_REGS == 128);
const _: () = assert!(SCRATCH_REGS == 64);
// Band partition: contiguous, non-overlapping, exactly covering R0..=R255.
const _: () = assert!(CONST_POOL_REGS + VAR_REGS + SCRATCH_REGS == REGISTER_COUNT);
const _: () = assert!(VAR_BAND_FIRST == CONST_POOL_REGS);
const _: () = assert!(SCRATCH_BAND_FIRST == VAR_BAND_FIRST + VAR_REGS);
const _: () = assert!(SCRATCH_BAND_FIRST + SCRATCH_REGS == REGISTER_COUNT);
const _: () = assert!(MAX_RULE_VARIABLES == VAR_REGS);
const _: () = assert!(DEFAULT_CHUNK_ROWS > 0);
const _: () = assert!(DEFAULT_MAX_ROUNDS >= 1);
// Register-file size/align: 256 x 8 bytes, u64-aligned — the L1-resident
// working set of spec 4.2.
const _: () = assert!(core::mem::size_of::<RegisterFile>() == REGISTER_FILE_BYTES);
const _: () = assert!(core::mem::align_of::<RegisterFile>() == core::mem::size_of::<u64>());

/// Typed error surface of the unifier (spec 4.2). Wave 1 froze
/// `RegisterOverflow`; Wave 2 lands the kernel/join/digest error variants
/// the Wave-1 note deferred. Every boundary is a typed error, never a
/// lossy coercion (house rule, A11 spec Section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnifyError {
    /// A rule body introduced more than [`MAX_RULE_VARIABLES`] (128)
    /// distinct variables — the variable band `R64..=R191` cannot hold
    /// the binding set. (Spec 4.2.)
    RegisterOverflow,
    /// A rule introduced more than [`CONST_POOL_REGS`] (64) distinct
    /// constants — the constant band `R0..=R63` cannot hold them.
    /// (Spec 4.2 band law.)
    ConstantPoolOverflow,
    /// `FormulaRef` in a rule's predicate slot — banned by A12 (A11
    /// `sigil.rs`: "Banned in the `p` slot by A12").
    FormulaRefInPredicate,
    /// A body/head term word carried a sigil the Wave-2 kernel does not
    /// unify: `VarExi` (existential scoping is a later semantic layer),
    /// `BuiltinOp` (A13's vocabulary), or a RESERVED code (A11: the
    /// decoder MUST refuse).
    UnsupportedTermSigil(u8),
    /// A `VarUni` payload exceeded the 8-bit register space `R0..R255`.
    RegisterOutOfRange(u64),
    /// A register access crossed its band (e.g. a constant store above
    /// `R63`). A compiled rule can only cause this through a compiler
    /// bug — surfaced as a typed error, never a silent wrap.
    RegisterBandViolation { reg: usize },
    /// A head template referenced a variable no body atom binds; the
    /// derivation would be undefined. (Typed refusal, spec 4.2.)
    UnboundHeadVariable,
    /// A rule with an empty body — nothing to select on.
    EmptyRuleBody,
    /// `chunk_size == 0` passed to the batch kernel.
    InvalidChunkSize,
    /// Statement columns of unequal length (malformed SoA input).
    ColumnLengthMismatch { s: usize, p: usize, o: usize },
    /// The fixpoint driver exceeded its round budget without converging.
    FixpointBudgetExceeded { rounds: u32 },
}

/// Term slot inside a body atom or head template. (Spec 4.1 s/p/o.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TermSlot {
    /// Subject position.
    Subject,
    /// Predicate position.
    Predicate,
    /// Object position.
    Object,
}

impl TermSlot {
    fn index(self) -> usize {
        match self {
            TermSlot::Subject => 0,
            TermSlot::Predicate => 1,
            TermSlot::Object => 2,
        }
    }
}

const TERM_SLOTS: [TermSlot; 3] =
    [TermSlot::Subject, TermSlot::Predicate, TermSlot::Object];

/// A body atom: three raw A11 term words (subject, predicate, object).
/// Words are decoded and register-assigned at compile time; laws (p-slot
/// FormulaRef ban, sigil refusals, band widths) are enforced there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyAtom {
    /// Subject term word.
    pub s: u64,
    /// Predicate term word.
    pub p: u64,
    /// Object term word.
    pub o: u64,
}

impl BodyAtom {
    /// Build an atom from three raw term words.
    pub fn new(s: u64, p: u64, o: u64) -> Self {
        BodyAtom { s, p, o }
    }
}

/// An implication rule `body => head` (spec Section 1). Evaluation order
/// of `body` atoms is the corpus author's (spec 3.2: hand-ordered v1; the
/// differential court covers order-independence of *correctness*).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Emitting-rule id carried into [`DeltaBatch::rule_id`].
    pub rule_id: u32,
    /// Body atoms, evaluated in order (nested-loop join, spec 3.2).
    pub body: Vec<BodyAtom>,
    /// Head template term words `(s, p, o)`.
    pub head: [u64; 3],
}

/// A body/head term after word decode and register assignment (spec 4.2):
/// `Const` holds a constant-pool register `R0..=R63`; `Var` indexes the
/// variable band (`R64 + idx`, `idx < 128`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompiledTerm {
    /// Constant pool register `R_n`, `n < CONST_POOL_REGS`.
    Const(u8),
    /// Variable band index: register `R{VAR_BAND_FIRST + idx}`.
    Var(u8),
}

/// A rule lowered to the register program (spec 4.2: "The op list is
/// emitted once per rule per batch-size class, not per row"). Constants
/// live in [`CompiledRule::const_pool`] (`R0..`), variables in the band
/// (`R64 + idx`); [`CompiledRule::var_first`] records each variable's
/// binding provenance `(atom index, term slot)` — the register-op
/// `BindVar` position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledRule {
    /// Emitting-rule id.
    pub rule_id: u32,
    /// Body atoms in compiled form, evaluation order.
    pub atoms: Vec<[CompiledTerm; 3]>,
    /// Head template in compiled form.
    pub head: [CompiledTerm; 3],
    /// Constant pool `R0..Rk` (first-seen order over body, then head).
    pub const_pool: Vec<u64>,
    /// Distinct variable count (band width used).
    pub var_count: usize,
    /// Per variable index: the `(atom, slot)` that binds it (first
    /// occurrence, body scan order). `usize::MAX` atom = head-only
    /// variable (refused at compile time).
    pub var_first: Vec<(usize, usize)>,
}

/// SoA statement columns over A11 term words (recorded divergence 1: the
/// Arrow-batch form is the Wave-3 seam; the columnar contract — equal
/// lengths, position-aligned rows — is identical).
#[derive(Clone, Copy, Debug)]
pub struct StatementColumns<'a> {
    /// Subject column.
    pub s: &'a [u64],
    /// Predicate column.
    pub p: &'a [u64],
    /// Object column.
    pub o: &'a [u64],
}

impl<'a> StatementColumns<'a> {
    /// Build columns; refuses unequal lengths (typed, never silent).
    pub fn new(s: &'a [u64], p: &'a [u64], o: &'a [u64]) -> Result<Self, UnifyError> {
        if s.len() != p.len() || s.len() != o.len() {
            return Err(UnifyError::ColumnLengthMismatch {
                s: s.len(),
                p: p.len(),
                o: o.len(),
            });
        }
        Ok(StatementColumns { s, p, o })
    }

    /// Row count.
    pub fn len(&self) -> usize {
        self.s.len()
    }

    /// True when the store has no rows.
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    /// Column lookup by term-slot index. Out-of-range slots are LOUD
    /// (Wave-5 defect-D2 repair): the previous `_ => self.o` fallback arm
    /// silently served the object column to a miscomputed slot index —
    /// exactly the silent-wrap class mutation M12-3 / A20-audit-M10
    /// (`col(si)` -> `col(si + 1)`) exploits. Slot indices are
    /// compiler-produced (`TERM_SLOTS`, 0..=2); any other value is a
    /// caller bug and panics with a typed message instead of silently
    /// comparing against the wrong column. Courted by
    /// `columns_court_out_of_range_slot_is_loud`.
    fn col(&self, slot: usize) -> &[u64] {
        match slot {
            0 => self.s,
            1 => self.p,
            2 => self.o,
            other => unreachable!("StatementColumns::col: slot {other} out of range 0..=2"),
        }
    }
}

/// The `R0..R255` register file (spec 4.2): scalar staging for constant
/// loads and per-match variable binding. Band accesses are checked — a
/// violation is the typed `UnifyError::RegisterBandViolation`, never a
/// silent wrap into a neighboring band.
#[derive(Clone, Debug)]
pub struct RegisterFile {
    regs: [u64; REGISTER_COUNT],
}

impl RegisterFile {
    /// A zeroed file (constants and variables overwrite their slots).
    pub fn zeroed() -> Self {
        RegisterFile {
            regs: [0; REGISTER_COUNT],
        }
    }

    /// `LoadConst`: store a rule constant into the constant band
    /// `R0..=R63`.
    pub fn load_const(&mut self, reg: usize, word: u64) -> Result<(), UnifyError> {
        if reg >= CONST_POOL_REGS {
            return Err(UnifyError::RegisterBandViolation { reg });
        }
        self.regs[reg] = word;
        Ok(())
    }

    /// `BindVar`: store a variable binding into `R{64 + idx}`,
    /// `idx < 128`.
    pub fn bind_var(&mut self, var_idx: usize, word: u64) -> Result<(), UnifyError> {
        let reg = VAR_BAND_FIRST + var_idx;
        if reg >= SCRATCH_BAND_FIRST {
            return Err(UnifyError::RegisterBandViolation { reg });
        }
        self.regs[reg] = word;
        Ok(())
    }

    /// Read a register.
    pub fn load(&self, reg: usize) -> Result<u64, UnifyError> {
        if reg >= REGISTER_COUNT {
            return Err(UnifyError::RegisterBandViolation { reg });
        }
        Ok(self.regs[reg])
    }
}

/// All full-body matches over one batch, aggregated across chunks (spec
/// 4.3 steps 2-4). Match `m` is: one row per body atom
/// ([`MatchSet::atom_rows`]) plus one bound value per variable
/// ([`MatchSet::var_values`]) — the selection vector and binding map the
/// projector consumes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchSet {
    /// Number of matches.
    pub len: usize,
    /// Per variable index (`R64 + idx`), the bound value for each match.
    pub var_values: Vec<Vec<u64>>,
    /// Per body atom, the matched store row for each match.
    pub atom_rows: Vec<Vec<u32>>,
}

/// The rule compiler (spec 5.2 C2: rule text -> register ops). Decodes A11
/// term words, interns constants into `R0..`, assigns variables into
/// `R64..`, records binding provenance, and enforces every band/ABI law as
/// a typed refusal.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuleCompiler;

impl RuleCompiler {
    /// Compile a rule. Typed refusals: [`UnifyError::EmptyRuleBody`],
    /// [`UnifyError::FormulaRefInPredicate`],
    /// [`UnifyError::UnsupportedTermSigil`],
    /// [`UnifyError::RegisterOutOfRange`],
    /// [`UnifyError::ConstantPoolOverflow`],
    /// [`UnifyError::RegisterOverflow`],
    /// [`UnifyError::UnboundHeadVariable`].
    pub fn compile(rule: &Rule) -> Result<CompiledRule, UnifyError> {
        if rule.body.is_empty() {
            return Err(UnifyError::EmptyRuleBody);
        }
        let mut it = Interner::default();
        let mut atoms = Vec::with_capacity(rule.body.len());
        for (ai, atom) in rule.body.iter().enumerate() {
            let mut ct = [CompiledTerm::Const(0); 3];
            for (si, &word) in [atom.s, atom.p, atom.o].iter().enumerate() {
                ct[si] = it.intern(word, TERM_SLOTS[si], ai)?;
            }
            if it.const_pool.len() > CONST_POOL_REGS {
                return Err(UnifyError::ConstantPoolOverflow);
            }
            if it.vars.len() > MAX_RULE_VARIABLES {
                return Err(UnifyError::RegisterOverflow);
            }
            atoms.push(ct);
        }
        let mut head = [CompiledTerm::Const(0); 3];
        for (si, &word) in rule.head.iter().enumerate() {
            // Head terms bind nothing: atom index usize::MAX marks a
            // head-only variable.
            head[si] = it.intern(word, TERM_SLOTS[si], usize::MAX)?;
        }
        if it.const_pool.len() > CONST_POOL_REGS {
            return Err(UnifyError::ConstantPoolOverflow);
        }
        if it.vars.len() > MAX_RULE_VARIABLES {
            return Err(UnifyError::RegisterOverflow);
        }
        for term in &head {
            if let CompiledTerm::Var(idx) = *term {
                if it.var_first[idx as usize].0 == usize::MAX {
                    return Err(UnifyError::UnboundHeadVariable);
                }
            }
        }
        Ok(CompiledRule {
            rule_id: rule.rule_id,
            atoms,
            head,
            const_pool: it.const_pool,
            var_count: it.vars.len(),
            var_first: it.var_first,
        })
    }
}

#[derive(Default)]
struct Interner {
    const_pool: Vec<u64>,
    const_index: HashMap<u64, u8>,
    vars: Vec<u8>,
    var_index: HashMap<u8, u8>,
    var_first: Vec<(usize, usize)>,
}

impl Interner {
    fn intern(
        &mut self,
        word: u64,
        slot: TermSlot,
        atom_idx: usize,
    ) -> Result<CompiledTerm, UnifyError> {
        let sig = (word >> sigil::SIGIL_SHIFT) as u8;
        if sig == Sigil::VarUni as u8 {
            let payload = word & sigil::PAYLOAD_MASK;
            if payload >= sigil::REGISTER_COUNT as u64 {
                return Err(UnifyError::RegisterOutOfRange(payload));
            }
            let vp = payload as u8;
            let next = self.vars.len();
            let idx = *self
                .var_index
                .entry(vp)
                .or_insert_with(|| {
                    self.vars.push(vp);
                    self.var_first.push((atom_idx, slot.index()));
                    next as u8
                });
            return Ok(CompiledTerm::Var(idx));
        }
        if sig == Sigil::FormulaRef as u8 && slot == TermSlot::Predicate {
            return Err(UnifyError::FormulaRefInPredicate);
        }
        if sig == Sigil::SlotConst as u8 || sig == Sigil::FormulaRef as u8 {
            let next = self.const_pool.len();
            let reg = *self
                .const_index
                .entry(word)
                .or_insert_with(|| {
                    self.const_pool.push(word);
                    next as u8
                });
            return Ok(CompiledTerm::Const(reg));
        }
        Err(UnifyError::UnsupportedTermSigil(sig))
    }
}

/// Reusable per-chunk evaluation scratch (Wave 4 hardening, spec 7.4):
/// every heap buffer the nested-loop evaluation touches, owned once and
/// reused across chunks, fixpoint rounds, and court runs. The
/// zero-allocation hot path's substrate: after a warmup run sizes the
/// buffers on identical input, a second scan run performs zero
/// allocations (courted in the tests below; the mutation "reintroduce
/// per-call allocation" MUST flip that court).
#[derive(Debug, Default)]
pub struct EvalScratch {
    binds: Vec<(usize, u64)>,
    seen: Vec<(usize, usize)>,
    bound_flags: Vec<bool>,
    bound_vars: Vec<usize>,
    bound_slots: Vec<(usize, usize)>,
    cand_rows: Vec<u32>,
    cand_spans: Vec<(usize, usize)>,
    cand_binds: Vec<(usize, u64)>,
    cur_rows: Vec<Vec<u32>>,
    cur_vars: Vec<Vec<u64>>,
    nxt_rows: Vec<Vec<u32>>,
    nxt_vars: Vec<Vec<u64>>,
    /// Per-column capacity high-water marks (`rows` and `vars` sides).
    /// Updated at the end of every chunk from both buffer roles; used at
    /// chunk start to reserve BOTH sides of the cur/nxt double-buffer
    /// pair, so warm scratch keeps the expansion off the allocator no
    /// matter which role a column is serving.
    hw_rows: Vec<usize>,
    hw_vars: Vec<usize>,
}

/// Per-atom local constraints over one candidate row: constant equality
/// and within-atom variable repetition (the self-join `?x p ?x`). Slots
/// whose variable is already bound (`bound[idx]`) are skipped here — the
/// per-parent probe loop checks them against the match's binding. On
/// success the fresh bindings this row contributes are written to
/// `binds` (cleared first); returns `false` if the row fails any slot.
/// Allocation-free after warmup: `binds`/`seen` are caller-owned scratch.
fn atom_local_match_into(
    const_pool: &[u64],
    atom: &[CompiledTerm; 3],
    cols: &StatementColumns<'_>,
    row: usize,
    bound: &[bool],
    binds: &mut Vec<(usize, u64)>,
    seen: &mut Vec<(usize, usize)>,
) -> bool {
    binds.clear();
    seen.clear();
    for si in 0..3 {
        let w = cols.col(si)[row];
        match atom[si] {
            CompiledTerm::Const(creg) => {
                if w != const_pool[creg as usize] {
                    return false;
                }
            }
            CompiledTerm::Var(idx) => {
                let idx = idx as usize;
                if bound[idx] {
                    continue;
                }
                match seen.iter().copied().find(|&(_, fi)| fi == idx) {
                    Some((fs, _)) => {
                        if w != cols.col(fs)[row] {
                            return false;
                        }
                    }
                    None => {
                        seen.push((si, idx));
                        binds.push((idx, w));
                    }
                }
            }
        }
    }
    true
}

/// Evaluate one compiled rule over one batch (all body atoms), atom-0
/// scan chunked over `[0, cols.len())`. Join atoms (1..) probe the FULL
/// column set, so a match pair is never split by a chunk boundary — this
/// is the chunk-partition independence the M12-6 determinism court pins:
/// match order is (atom-0 row ascending, join candidate ascending)
/// regardless of partitioning (recorded divergence 3 for the
/// micro-architecture).
///
/// Allocation profile (Wave 4): with a warm [`EvalScratch`] and a
/// `MatchSet` retaining capacity from a prior run over identical input,
/// this performs zero heap allocations (court `zero_alloc_court_...`).
pub fn unify_batch(
    rule: &CompiledRule,
    cols: &StatementColumns<'_>,
    chunk_size: usize,
) -> Result<MatchSet, UnifyError> {
    let mut scratch = EvalScratch::default();
    let mut out = MatchSet::default();
    unify_batch_into(rule, cols, chunk_size, &mut scratch, &mut out)?;
    Ok(out)
}

/// [`unify_batch`] over caller-owned buffers (Wave 4, spec 7.4): reuses
/// `scratch` across chunks and appends into `out` (cleared first,
/// capacity retained) — the zero-allocation hot path's entry point.
pub fn unify_batch_into(
    rule: &CompiledRule,
    cols: &StatementColumns<'_>,
    chunk_size: usize,
    scratch: &mut EvalScratch,
    out: &mut MatchSet,
) -> Result<(), UnifyError> {
    if chunk_size == 0 {
        return Err(UnifyError::InvalidChunkSize);
    }
    // Output columns sized to the rule; `resize` grows only on first use,
    // so a reused `MatchSet` keeps its high-water capacity.
    if out.atom_rows.len() < rule.atoms.len() {
        out.atom_rows.resize(rule.atoms.len(), Vec::new());
    }
    if out.var_values.len() < rule.var_count {
        out.var_values.resize(rule.var_count, Vec::new());
    }
    out.len = 0;
    for col in out.atom_rows.iter_mut() {
        col.clear();
    }
    for col in out.var_values.iter_mut() {
        col.clear();
    }
    let n = cols.len();
    let mut start = 0usize;
    while start < n {
        let end = (start + chunk_size).min(n);
        eval_chunk_into(rule, cols, start, end, scratch, out);
        start = end;
    }
    debug_assert!(out
        .var_values
        .iter()
        .take(rule.var_count)
        .all(|v| v.len() == out.atom_rows[0].len()));
    out.len = out.atom_rows[0].len();
    Ok(())
}

/// One chunk of the nested-loop evaluation (Wave-2 algorithm, Wave-4
/// buffers): identical match semantics and match order to the Wave-2
/// `eval_chunk`, with every heap buffer drawn from [`EvalScratch`].
fn eval_chunk_into(
    rule: &CompiledRule,
    cols: &StatementColumns<'_>,
    start: usize,
    end: usize,
    sc: &mut EvalScratch,
    out: &mut MatchSet,
) {
    // Chunk-local match state, REBUILT per atom expansion: every accepted
    // join pair pushes exactly one entry into every variable column (the
    // parent's value for already-bound vars, the candidate's value for
    // fresh vars), so column alignment holds by construction. The output
    // accumulators (`out`) only see the final, aligned chunk state.
    for col in sc.cur_rows.iter_mut() {
        col.clear();
    }
    for col in sc.cur_vars.iter_mut() {
        col.clear();
    }
    if sc.cur_rows.len() < rule.atoms.len() {
        sc.cur_rows.resize(rule.atoms.len(), Vec::new());
    }
    if sc.cur_vars.len() < rule.var_count {
        sc.cur_vars.resize(rule.var_count, Vec::new());
    }
    if sc.nxt_rows.len() < rule.atoms.len() {
        sc.nxt_rows.resize(rule.atoms.len(), Vec::new());
    }
    if sc.nxt_vars.len() < rule.var_count {
        sc.nxt_vars.resize(rule.var_count, Vec::new());
    }
    // Capacity high-water: keep BOTH roles of the cur/nxt pair warm (see
    // `EvalScratch::hw_rows`). The reserve is a no-op once satisfied —
    // after warmup it allocates nothing; on the first runs it sizes the
    // expansion-side buffers, which the double-buffer swap alone would
    // leave cold every second pass.
    sc.hw_rows.resize(rule.atoms.len(), 0);
    sc.hw_vars.resize(rule.var_count, 0);
    for a in 0..rule.atoms.len() {
        sc.cur_rows[a].reserve(sc.hw_rows[a]);
        sc.nxt_rows[a].reserve(sc.hw_rows[a]);
    }
    for idx in 0..rule.var_count {
        sc.cur_vars[idx].reserve(sc.hw_vars[idx]);
        sc.nxt_vars[idx].reserve(sc.hw_vars[idx]);
    }

    // Atom 0: candidates are this chunk's rows (spec 4.3 step 2); no
    // variable is bound yet.
    sc.bound_flags.clear();
    sc.bound_flags.resize(rule.var_count, false);
    for i in start..end {
        if atom_local_match_into(
            &rule.const_pool,
            &rule.atoms[0],
            cols,
            i,
            &sc.bound_flags,
            &mut sc.binds,
            &mut sc.seen,
        ) {
            sc.cur_rows[0].push(i as u32);
            for (idx, val) in sc.binds.iter().copied() {
                sc.cur_vars[idx].push(val);
            }
        }
    }

    // Atoms 1..: nested-loop join (spec 3.2 v1). Candidates come from the
    // full store; bound variables constrain each candidate against the
    // match's bindings; fresh variables bind from the candidate row.
    for (ai, atom) in rule.atoms.iter().enumerate().skip(1) {
        // Vars with first binding before this atom are bound for every
        // current match; the rest are fresh here. Scratch columns are
        // cleared FIRST (they carry the previous atom's state).
        sc.bound_flags.clear();
        sc.bound_vars.clear();
        sc.bound_flags.resize(rule.var_count, false);
        for (idx, &(atom_idx, _)) in rule.var_first.iter().enumerate() {
            sc.bound_flags[idx] = atom_idx < ai;
            if atom_idx < ai {
                sc.bound_vars.push(idx);
            }
        }
        // Candidates: local constraints only; the bound-variable probe
        // runs per (match, candidate) below. Flattened into three
        // parallel buffers (rows, per-candidate bind spans, binds) so no
        // per-candidate allocation exists.
        sc.cand_rows.clear();
        sc.cand_spans.clear();
        sc.cand_binds.clear();
        for j in 0..cols.len() {
            if atom_local_match_into(
                &rule.const_pool,
                atom,
                cols,
                j,
                &sc.bound_flags,
                &mut sc.binds,
                &mut sc.seen,
            ) {
                sc.cand_spans.push((sc.cand_binds.len(), sc.binds.len()));
                sc.cand_binds.extend_from_slice(&sc.binds);
                sc.cand_rows.push(j as u32);
            }
        }
        // Bound slots: (slot, var index) pairs the join probe checks.
        sc.bound_slots.clear();
        for (si, term) in atom.iter().enumerate() {
            if let CompiledTerm::Var(idx) = *term {
                if sc.bound_flags[idx as usize] {
                    sc.bound_slots.push((si, idx as usize));
                }
            }
        }
        let cur = sc.cur_rows[0].len();
        // Double-buffer swap: `nxt` carries the previous atom's state
        // (cleared for use below), `cur` the current expansion's; the
        // swap moves buffer ownership without touching the allocator.
        for col in sc.nxt_rows.iter_mut().take(ai + 1) {
            col.clear();
        }
        for col in sc.nxt_vars.iter_mut() {
            col.clear();
        }
        for m in 0..cur {
            for (c, &j) in sc.cand_rows.iter().enumerate() {
                let (b0, blen) = sc.cand_spans[c];
                let mut ok = true;
                for &(si, vidx) in &sc.bound_slots {
                    if cols.col(si)[j as usize] != sc.cur_vars[vidx][m] {
                        ok = false;
                        break;
                    }
                }
                if !ok {
                    continue;
                }
                for a in 0..ai {
                    sc.nxt_rows[a].push(sc.cur_rows[a][m]);
                }
                sc.nxt_rows[ai].push(j);
                // Expansion: every variable bound for the parent match
                // carries its value into the new match; then this atom's
                // fresh bindings take the candidate's values.
                for &vidx in &sc.bound_vars {
                    let v = sc.cur_vars[vidx][m];
                    sc.nxt_vars[vidx].push(v);
                }
                for &(idx, val) in &sc.cand_binds[b0..b0 + blen] {
                    sc.nxt_vars[idx].push(val);
                }
            }
        }
        std::mem::swap(&mut sc.cur_rows, &mut sc.nxt_rows);
        std::mem::swap(&mut sc.cur_vars, &mut sc.nxt_vars);
    }

    // Append the aligned chunk state into the shared accumulators. The
    // scratch may hold extra columns from a previous, wider rule (the
    // driver shares one scratch across rules) — bound every loop to THIS
    // rule's widths, never the scratch's.
    for (a, column) in sc.cur_rows.iter_mut().enumerate().take(rule.atoms.len()) {
        out.atom_rows[a].extend_from_slice(column);
    }
    for (idx, column) in sc.cur_vars.iter_mut().enumerate().take(rule.var_count) {
        out.var_values[idx].extend_from_slice(column);
    }

    // Record the chunk's per-column high-water usage from both buffer
    // roles (post-final-swap `cur` holds the matches, `nxt` the previous
    // atom's state — between them they cover every expansion peak).
    for a in 0..rule.atoms.len() {
        let peak = sc.cur_rows[a].len().max(sc.nxt_rows[a].len());
        sc.hw_rows[a] = sc.hw_rows[a].max(peak);
    }
    for idx in 0..rule.var_count {
        let peak = sc.cur_vars[idx].len().max(sc.nxt_vars[idx].len());
        sc.hw_vars[idx] = sc.hw_vars[idx].max(peak);
    }
}

/// Evaluate a rule set to fixpoint (spec 7.2: "run the register unifier to
/// fixpoint"). Each round evaluates every rule over the store-so-far,
/// emits one [`DeltaBatch`] per rule via the projector, and appends newly
/// derived rows to the store. The `seen` set is termination machinery of
/// the driver only — the projector never dedups (M12-7 division), and
/// batches re-emit re-derivations for A15 to collapse.
///
/// Epochs are the round numbers (monotonically increasing, spec 4.4).
/// Deterministic: rule order as given, match order per [`unify_batch`].
pub fn unify_to_fixpoint(
    rules: &[Rule],
    cols: &StatementColumns<'_>,
) -> Result<Vec<DeltaBatch<DeltaRows>>, UnifyError> {
    unify_to_fixpoint_opt(rules, cols, DEFAULT_MAX_ROUNDS, DEFAULT_CHUNK_ROWS)
}

/// [`unify_to_fixpoint`] with explicit round budget and atom-0 chunk size
/// (the M12-6 court sweeps the chunk size through this entry point).
pub fn unify_to_fixpoint_opt(
    rules: &[Rule],
    cols: &StatementColumns<'_>,
    max_rounds: u32,
    chunk_size: usize,
) -> Result<Vec<DeltaBatch<DeltaRows>>, UnifyError> {
    let compiled: Vec<CompiledRule> = rules
        .iter()
        .map(RuleCompiler::compile)
        .collect::<Result<Vec<_>, _>>()?;
    let mut store_s: Vec<u64> = cols.s.to_vec();
    let mut store_p: Vec<u64> = cols.p.to_vec();
    let mut store_o: Vec<u64> = cols.o.to_vec();
    // Termination machinery only (see fn docs): membership, never order.
    let mut seen: HashSet<(u64, u64, u64)> = (0..cols.len())
        .map(|i| (cols.s[i], cols.p[i], cols.o[i]))
        .collect();

    let projector = ConsequentProjector::new();
    // Wave 4: one scratch + one match set, reused across rounds x rules
    // (allocation discipline; match semantics unchanged).
    let mut scratch = EvalScratch::default();
    let mut matches = MatchSet::default();
    let mut batches = Vec::new();
    let mut epoch: u32 = 0;
    loop {
        epoch += 1;
        let mut new_rows = 0usize;
        for rule in &compiled {
            let snapshot = StatementColumns::new(&store_s, &store_p, &store_o)?;
            unify_batch_into(rule, &snapshot, chunk_size, &mut scratch, &mut matches)?;
            let batch = projector.project(rule, epoch as u64, &matches)?;
            for i in 0..batch.rows.len() {
                let t = (batch.rows.s[i], batch.rows.p[i], batch.rows.o[i]);
                if seen.insert(t) {
                    store_s.push(t.0);
                    store_p.push(t.1);
                    store_o.push(t.2);
                    new_rows += 1;
                }
            }
            batches.push(batch);
        }
        if new_rows == 0 {
            return Ok(batches);
        }
        if epoch >= max_rounds {
            return Err(UnifyError::FixpointBudgetExceeded { rounds: epoch });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    // --- Wave-1 ABI courts (kept verbatim) --------------------------------

    /// Band-partition court (spec 4.2): the three bands are contiguous,
    /// non-overlapping, and exactly cover `R0..=R255`.
    /// Mutation that MUST flip this court: shrink `VAR_REGS` 128 -> 127
    /// (the const asserts above fail the build first; this runtime court
    /// is the redundant receipt).
    #[test]
    fn band_partition_court_bands_cover_the_file() {
        assert_eq!(CONST_POOL_REGS, 64);
        assert_eq!(VAR_REGS, 128);
        assert_eq!(SCRATCH_REGS, 64);
        assert_eq!(VAR_BAND_FIRST, 64);
        assert_eq!(SCRATCH_BAND_FIRST, 192);
        assert_eq!(CONST_POOL_REGS + VAR_REGS + SCRATCH_REGS, REGISTER_COUNT);
        // Contiguity: each band starts where the previous band ends, and
        // the scratch band closes the file at R255.
        assert_eq!(SCRATCH_BAND_FIRST + SCRATCH_REGS, REGISTER_COUNT - 1 + 1);
        // Every register number the 8-bit payload can transport falls in
        // exactly one band.
        for n in 0u64..REGISTER_COUNT as u64 {
            let in_const = n < CONST_POOL_REGS as u64;
            let in_var = (VAR_BAND_FIRST as u64..SCRATCH_BAND_FIRST as u64).contains(&n);
            let in_scratch = n >= SCRATCH_BAND_FIRST as u64;
            assert!(
                (in_const ^ in_var ^ in_scratch) && !(in_const && in_var && in_scratch),
                "R{n} banded {in_const}/{in_var}/{in_scratch}",
            );
        }
    }

    /// Register-width court (spec 4.2): one register is one `u64` lane
    /// group; the whole file is 2 KiB.
    /// Mutation that MUST flip this court: widen `REGISTER_WIDTH_BYTES`
    /// 8 -> 16 (the `size_of::<u64>()` const law fails the build first).
    #[test]
    fn register_width_court_file_is_2048_bytes() {
        assert_eq!(REGISTER_WIDTH_BYTES, core::mem::size_of::<u64>());
        assert_eq!(REGISTER_FILE_BYTES, 2048);
    }

    /// Typed-error court (spec 4.2): `RegisterOverflow` is the named
    /// refusal for a rule exceeding the variable band.
    /// Mutation that MUST flip this court: rename the variant or drop it
    /// from the enum (build failure).
    #[test]
    fn typed_error_court_register_overflow_is_named() {
        let err = UnifyError::RegisterOverflow;
        assert_eq!(err, UnifyError::RegisterOverflow);
        assert!(format!("{err:?}").contains("RegisterOverflow"));
    }

    // --- Test substrate: term-word builders, corpus, naive oracle --------
    // (Self-contained: the oracle re-implements word decoding and rule
    // matching row-at-a-time with a hash map, a genuinely different
    // algorithm from the kernel — the differential court's independence.)

    const SIG_VAR: u64 = 0x01;

    /// SlotConstant term word (sigil 0x00): word == interned id.
    fn cw(id: u64) -> u64 {
        id
    }
    /// VarUni term word: register number in the payload.
    fn vw(reg: u8) -> u64 {
        (SIG_VAR << 56) | reg as u64
    }

    struct Corpus {
        s: Vec<u64>,
        p: Vec<u64>,
        o: Vec<u64>,
        rules: Vec<Rule>,
    }

    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state >> 33
    }

    /// Rule classes of spec 7.1: single-match, join, self-join,
    /// constant-bound, head-rebinds-swapped. Predicates: P1=100, P2=200,
    /// P3=300; node pool ids 1..=6.
    fn rule_set() -> Vec<Rule> {
        vec![
            // 0: single-match, same-order rebind
            Rule {
                rule_id: 0,
                body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
                head: [vw(10), cw(300), vw(11)],
            },
            // 1: join ?x P1 ?y . ?y P2 ?z => ?z P1 ?x
            Rule {
                rule_id: 1,
                body: vec![
                    BodyAtom::new(vw(10), cw(100), vw(11)),
                    BodyAtom::new(vw(11), cw(200), vw(12)),
                ],
                head: [vw(12), cw(100), vw(10)],
            },
            // 2: self-join ?x P2 ?x => ?x P1 (const 1)
            Rule {
                rule_id: 2,
                body: vec![BodyAtom::new(vw(10), cw(200), vw(10))],
                head: [vw(10), cw(100), cw(1)],
            },
            // 3: constant-bound (1 P1 ?y) => (?y P2 1)
            Rule {
                rule_id: 3,
                body: vec![BodyAtom::new(cw(1), cw(100), vw(11))],
                head: [vw(11), cw(200), cw(1)],
            },
            // 4: head re-binds body variables in swapped order
            Rule {
                rule_id: 4,
                body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
                head: [vw(11), cw(300), vw(10)],
            },
        ]
    }

    /// Deterministic (seeded) corpus: 24 rows, self-loop rows every 5th
    /// (feeds the self-join rule). Same seed -> same corpus (asserted by
    /// `corpus_court_same_seed_is_identical`).
    fn gen_corpus(seed: u64) -> Corpus {
        let mut st = seed | 1;
        let mut rows: Vec<(u64, u64, u64)> = Vec::new();
        for r in 0..24u64 {
            let subj = cw(1 + lcg(&mut st) % 6);
            if r % 5 == 0 {
                rows.push((subj, cw(200), subj));
            } else {
                let pred = cw(100 + (lcg(&mut st) % 2) * 100);
                let obj = cw(1 + lcg(&mut st) % 6);
                rows.push((subj, pred, obj));
            }
        }
        Corpus {
            s: rows.iter().map(|r| r.0).collect(),
            p: rows.iter().map(|r| r.1).collect(),
            o: rows.iter().map(|r| r.2).collect(),
            rules: rule_set(),
        }
    }

    /// Naive row-at-a-time matcher: recursive nested loops over row
    /// indices with a hash-map binding — the ground-truth algorithm of the
    /// differential court (spec 7.2). Decodes term words independently of
    /// the kernel.
    fn naive_match_rule(
        rule: &Rule,
        s: &[u64],
        p: &[u64],
        o: &[u64],
        binding: &mut HashMap<u8, u64>,
        out: &mut Vec<(u64, u64, u64)>,
    ) {
        fn resolve(w: u64, binding: &HashMap<u8, u64>) -> Option<u64> {
            if w >> 56 == SIG_VAR {
                binding.get(&((w & sigil::PAYLOAD_MASK) as u8)).copied()
            } else {
                Some(w)
            }
        }
        fn go(
            rule: &Rule,
            ai: usize,
            s: &[u64],
            p: &[u64],
            o: &[u64],
            binding: &mut HashMap<u8, u64>,
            out: &mut Vec<(u64, u64, u64)>,
        ) {
            if ai == rule.body.len() {
                let mut h = [0u64; 3];
                for (si, &w) in rule.head.iter().enumerate() {
                    match resolve(w, binding) {
                        Some(v) => h[si] = v,
                        // Unbound head variable: not derivable here (the
                        // kernel refuses such rules at compile time; the
                        // witnessed corpora contain none).
                        None => return,
                    }
                }
                out.push((h[0], h[1], h[2]));
                return;
            }
            let atom = &rule.body[ai];
            let slots = [atom.s, atom.p, atom.o];
            let columns: [&[u64]; 3] = [s, p, o];
            for j in 0..s.len() {
                let saved = binding.clone();
                let mut ok = true;
                for si in 0..3 {
                    let want = slots[si];
                    let have = columns[si][j];
                    if want >> 56 == SIG_VAR {
                        let key = (want & sigil::PAYLOAD_MASK) as u8;
                        match binding.get(&key) {
                            Some(&v) if v == have => {}
                            Some(_) => {
                                ok = false;
                                break;
                            }
                            None => {
                                binding.insert(key, have);
                            }
                        }
                    } else if want != have {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    go(rule, ai + 1, s, p, o, binding, out);
                }
                *binding = saved;
            }
        }
        go(rule, 0, s, p, o, binding, out);
    }

    /// Naive fixpoint oracle: rebuilds full-closure column views per round
    /// so the matcher sees derived rows too (genuinely row-at-a-time over
    /// the whole closure, independent of the kernel's store handling).
    /// Returns (full closure set, per-rule fired witness) — the witness is
    /// the "every rule fires" property of spec DoD 7 / M12-5.
    fn naive_closure_full(
        rules: &[Rule],
        s: &[u64],
        p: &[u64],
        o: &[u64],
    ) -> (BTreeSet<(u64, u64, u64)>, Vec<bool>) {
        let mut closure: BTreeSet<(u64, u64, u64)> =
            (0..s.len()).map(|i| (s[i], p[i], o[i])).collect();
        let mut fired = vec![false; rules.len()];
        loop {
            let cs: Vec<(u64, u64, u64)> = closure.iter().copied().collect();
            let vs: Vec<u64> = cs.iter().map(|r| r.0).collect();
            let vp: Vec<u64> = cs.iter().map(|r| r.1).collect();
            let vo: Vec<u64> = cs.iter().map(|r| r.2).collect();
            let mut grew = false;
            for (ri, rule) in rules.iter().enumerate() {
                let mut out = Vec::new();
                let mut binding = HashMap::new();
                naive_match_rule(rule, &vs, &vp, &vo, &mut binding, &mut out);
                for row in out {
                    if closure.insert(row) {
                        fired[ri] = true;
                        grew = true;
                    }
                }
            }
            if !grew {
                break;
            }
        }
        (closure, fired)
    }

    /// Generator with the "every rule fires" witness (spec DoD 7): the
    /// corpus is retried deterministically until every rule derives at
    /// least one statement; otherwise M12-5 (rule drop) would be vacuous.
    fn gen_witnessed_corpus(seed: u64) -> Corpus {
        for k in 0..200u64 {
            let seed_k = seed.wrapping_mul(7919).wrapping_add(k);
            let c = gen_corpus(seed_k);
            let (_, fired) = naive_closure_full(&c.rules, &c.s, &c.p, &c.o);
            if fired.iter().all(|&f| f) {
                return c;
            }
        }
        panic!("corpus generator: no witnessed corpus within 200 retries for seed {seed}");
    }

    // --- Compile-law courts ----------------------------------------------

    /// Register-assignment court (spec 4.2): constants intern into R0..
    /// in first-seen order; variables map into the band `R64 + idx`;
    /// body and head share registers for the same variable.
    /// Mutation that MUST flip this court: assigning variables from R0
    /// (band mix) or per-occurrence registers (no sharing).
    #[test]
    fn compile_court_register_band_assignment() {
        let rule = Rule {
            rule_id: 1,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
            head: [vw(11), cw(300), vw(10)],
        };
        let c = RuleCompiler::compile(&rule).unwrap();
        assert_eq!(c.const_pool, vec![cw(100), cw(300)]);
        assert_eq!(c.var_count, 2);
        // var 10 -> idx 0 (R64), var 11 -> idx 1 (R65); provenance both
        // from atom 0 (slots 0 and 2).
        assert_eq!(c.var_first, vec![(0, 0), (0, 2)]);
        assert_eq!(c.atoms[0], [CompiledTerm::Var(0), CompiledTerm::Const(0), CompiledTerm::Var(1)]);
        // Head shares the body's registers: Var(1), Const(1), Var(0).
        assert_eq!(c.head, [CompiledTerm::Var(1), CompiledTerm::Const(1), CompiledTerm::Var(0)]);
    }

    /// RegisterOverflow court (spec 4.2): a rule with 129 distinct
    /// variables is refused with the typed variant.
    /// Mutation that MUST flip this court: raising the band past the
    /// payload's 8-bit space or silently truncating register numbers.
    #[test]
    fn compile_court_register_overflow_is_typed() {
        let mut body = Vec::new();
        for v in 0..129u8 {
            body.push(BodyAtom::new(vw(v), cw(100), vw(v)));
        }
        let rule = Rule {
            rule_id: 1,
            body,
            head: [vw(0), cw(100), vw(0)],
        };
        assert_eq!(RuleCompiler::compile(&rule), Err(UnifyError::RegisterOverflow));
    }

    /// ConstantPoolOverflow court (band law, spec 4.2): 65 distinct
    /// constants exceed `R0..=R63`.
    /// Mutation that MUST flip this court: wrapping constant registers
    /// modulo 64.
    #[test]
    fn compile_court_constant_pool_overflow_is_typed() {
        let mut body = Vec::new();
        for k in 0..65u64 {
            body.push(BodyAtom::new(cw(1000 + k), cw(100), cw(2000 + k)));
        }
        let rule = Rule {
            rule_id: 1,
            body,
            head: [cw(1000), cw(100), cw(2000)],
        };
        assert_eq!(RuleCompiler::compile(&rule), Err(UnifyError::ConstantPoolOverflow));
    }

    /// Predicate-slot court (A12 law, A11 sigil.rs note): FormulaRef in
    /// the p slot is refused; in s/o slots it is an ordinary constant
    /// word (word equality).
    /// Mutation that MUST flip this court: allowing FormulaRef in p.
    #[test]
    fn compile_court_formula_ref_banned_in_predicate() {
        let fref = (0x03u64 << 56) | 7;
        let bad = Rule {
            rule_id: 1,
            body: vec![BodyAtom::new(vw(10), fref, vw(11))],
            head: [vw(10), cw(100), vw(11)],
        };
        assert_eq!(RuleCompiler::compile(&bad), Err(UnifyError::FormulaRefInPredicate));
        let ok = Rule {
            rule_id: 1,
            body: vec![BodyAtom::new(fref, cw(100), vw(11))],
            head: [vw(11), cw(100), fref],
        };
        assert!(RuleCompiler::compile(&ok).is_ok());
    }

    /// Sigil-refusal court (A11: RESERVED must be refused; A12 Wave 2
    /// refuses VarExi and BuiltinOp loudly rather than guessing scoping).
    /// Mutation that MUST flip this court: treating unknown sigils as
    /// constants (silent widening).
    #[test]
    fn compile_court_unsupported_sigils_are_typed_refusals() {
        let cases = [
            ((0x02u64 << 56) | 5, UnifyError::UnsupportedTermSigil(0x02)), // VarExi
            ((0x04u64 << 56) | 9, UnifyError::UnsupportedTermSigil(0x04)), // BuiltinOp
            ((0xFFu64 << 56) | 1, UnifyError::UnsupportedTermSigil(0xFF)), // RESERVED
        ];
        for (word, expect) in cases {
            let rule = Rule {
                rule_id: 1,
                body: vec![BodyAtom::new(word, cw(100), vw(11))],
                head: [vw(11), cw(100), vw(11)],
            };
            assert_eq!(RuleCompiler::compile(&rule), Err(expect));
        }
    }

    /// Register-range court: a VarUni payload beyond R255 is refused.
    /// Mutation that MUST flip this court: truncating payloads to 8 bits.
    #[test]
    fn compile_court_register_out_of_range_is_typed() {
        let rule = Rule {
            rule_id: 1,
            body: vec![BodyAtom::new(vw(10), cw(100), (0x01u64 << 56) | 300)],
            head: [vw(10), cw(100), vw(10)],
        };
        assert_eq!(RuleCompiler::compile(&rule), Err(UnifyError::RegisterOutOfRange(300)));
    }

    /// Head-binding court: a head variable no body atom binds is refused.
    /// Mutation that MUST flip this court: instantiating unbound head
    /// variables as wildcard/zero.
    #[test]
    fn compile_court_unbound_head_variable_is_typed() {
        let rule = Rule {
            rule_id: 1,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
            head: [vw(12), cw(100), vw(10)],
        };
        assert_eq!(RuleCompiler::compile(&rule), Err(UnifyError::UnboundHeadVariable));
    }

    /// Empty-body court.
    #[test]
    fn compile_court_empty_body_is_typed() {
        let rule = Rule {
            rule_id: 1,
            body: vec![],
            head: [cw(1), cw(100), cw(2)],
        };
        assert_eq!(RuleCompiler::compile(&rule), Err(UnifyError::EmptyRuleBody));
    }

    /// Register-file band court (spec 4.2): constant stores above R63,
    /// variable binds past R191, and reads past R255 are typed
    /// violations.
    /// Mutation that MUST flip this court: removing a band check.
    #[test]
    fn register_file_court_band_violations_are_typed() {
        let mut rf = RegisterFile::zeroed();
        assert_eq!(
            rf.load_const(CONST_POOL_REGS, 1),
            Err(UnifyError::RegisterBandViolation { reg: CONST_POOL_REGS })
        );
        assert_eq!(
            rf.bind_var(VAR_REGS, 1),
            Err(UnifyError::RegisterBandViolation { reg: SCRATCH_BAND_FIRST })
        );
        assert_eq!(
            rf.load(REGISTER_COUNT),
            Err(UnifyError::RegisterBandViolation { reg: REGISTER_COUNT })
        );
        // Lawful accesses round-trip.
        rf.load_const(0, 42).unwrap();
        rf.bind_var(0, 7).unwrap();
        assert_eq!(rf.load(0).unwrap(), 42);
        assert_eq!(rf.load(VAR_BAND_FIRST).unwrap(), 7);
    }

    // --- Kernel courts ----------------------------------------------------

    /// Single-atom selection court: const/var mask arithmetic selects
    /// exactly the rows satisfying the atom, with bindings materialized.
    /// Mutation that MUST flip this court (spec 7.3 M12-1 class):
    /// combining the three per-slot masks with OR instead of AND.
    #[test]
    fn kernel_court_single_atom_selection_and_bindings() {
        let s = [cw(1), cw(2), cw(1)];
        let p = [cw(100), cw(100), cw(200)];
        let o = [cw(3), cw(4), cw(5)];
        let rule = Rule {
            rule_id: 0,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
            head: [vw(11), cw(300), vw(10)],
        };
        let c = RuleCompiler::compile(&rule).unwrap();
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let m = unify_batch(&c, &cols, 2).unwrap(); // chunked on purpose
        assert_eq!(m.len, 2);
        assert_eq!(m.atom_rows[0], vec![0, 1]);
        assert_eq!(m.var_values[0], vec![cw(1), cw(2)], "?x bindings");
        assert_eq!(m.var_values[1], vec![cw(3), cw(4)], "?y bindings");
    }

    /// Self-join court (`?x p ?x`): within-atom variable repetition
    /// compares both slots' columns row-wise.
    /// Mutation that MUST flip this court (spec 7.3 M12-3 class):
    /// swapping the comparison direction of the repeated variable.
    #[test]
    fn kernel_court_self_join_within_atom() {
        let s = [cw(1), cw(1), cw(3)];
        let p = [cw(200), cw(200), cw(200)];
        let o = [cw(1), cw(2), cw(3)];
        let rule = Rule {
            rule_id: 2,
            body: vec![BodyAtom::new(vw(10), cw(200), vw(10))],
            head: [vw(10), cw(100), cw(1)],
        };
        let c = RuleCompiler::compile(&rule).unwrap();
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let m = unify_batch(&c, &cols, 4096).unwrap();
        assert_eq!(m.len, 2);
        assert_eq!(m.atom_rows[0], vec![0, 2]);
    }

    /// Cross-row join court: `?x P1 ?y . ?y P2 ?z` over a hand-computed
    /// store, including a candidate that passes atom-1 locally but fails
    /// the bound-variable probe.
    /// Mutation that MUST flip this court (spec 7.3 M12-3): swapping the
    /// join's bound-value probe direction (probing the binding against
    /// the candidate column instead of the candidate against bindings)
    /// scrambles at least one match here.
    #[test]
    fn kernel_court_cross_row_join() {
        // 0: (A,P1,B)  1: (B,P2,C)  2: (A,P1,D)  3: (D,P2,E)
        // 4: (B,P1,C)  5: (C,P2,F)  6: (A,P1,A)  <- y=A has no P2 out
        let s = [cw(1), cw(2), cw(1), cw(4), cw(2), cw(3), cw(1)];
        let p = [cw(100), cw(200), cw(100), cw(200), cw(100), cw(200), cw(100)];
        let o = [cw(2), cw(3), cw(4), cw(5), cw(3), cw(6), cw(1)];
        let rule = Rule {
            rule_id: 1,
            body: vec![
                BodyAtom::new(vw(10), cw(100), vw(11)),
                BodyAtom::new(vw(11), cw(200), vw(12)),
            ],
            head: [vw(12), cw(100), vw(10)],
        };
        let c = RuleCompiler::compile(&rule).unwrap();
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let m = unify_batch(&c, &cols, 3).unwrap();
        assert_eq!(m.len, 3);
        // (x=A,y=B,z=C) (x=A,y=D,z=E) (x=B,y=C,z=F)
        assert_eq!(m.atom_rows[0], vec![0, 2, 4], "atom-0 anchors");
        assert_eq!(m.atom_rows[1], vec![1, 3, 5], "joined rows");
        assert_eq!(
            m.var_values[2],
            vec![cw(3), cw(5), cw(6)],
            "?z bindings"
        );
    }

    /// No-match court: a constant bound to an absent value selects
    /// nothing and yields a well-formed empty match set.
    /// Mutation that MUST flip this court: emitting phantom matches.
    #[test]
    fn kernel_court_no_match_is_empty() {
        let s = [cw(1)];
        let p = [cw(100)];
        let o = [cw(2)];
        let rule = Rule {
            rule_id: 0,
            body: vec![BodyAtom::new(cw(999), cw(100), vw(10))],
            head: [vw(10), cw(200), cw(1)],
        };
        let c = RuleCompiler::compile(&rule).unwrap();
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let m = unify_batch(&c, &cols, 4096).unwrap();
        assert_eq!(m.len, 0);
        assert!(m.atom_rows.iter().all(|v| v.is_empty()));
        assert_eq!(m.var_values[0], Vec::<u64>::new());
    }

    /// Chunk-size court: `chunk_size == 0` is a typed refusal.
    #[test]
    fn kernel_court_zero_chunk_size_is_typed() {
        let s = [cw(1)];
        let p = [cw(100)];
        let o = [cw(2)];
        let rule = Rule {
            rule_id: 0,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
            head: [vw(10), cw(100), vw(11)],
        };
        let c = RuleCompiler::compile(&rule).unwrap();
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        assert_eq!(unify_batch(&c, &cols, 0), Err(UnifyError::InvalidChunkSize));
    }

    /// Column-law court: unequal columns are a typed refusal.
    #[test]
    fn columns_court_length_mismatch_is_typed() {
        let s = [cw(1), cw(2)];
        let p = [cw(100)];
        let o = [cw(2)];
        assert!(StatementColumns::new(&s, &p, &o).is_err());
    }

    /// Out-of-range slot court (Wave-5 defect-D2 repair): `col` refuses a
    /// slot index beyond the three term slots LOUDLY — never a silent
    /// fallback to the object column (the silent-wrap class mutation
    /// M12-3 / A20-audit-M10 exploits).
    /// Mutation that MUST flip this court: restoring the `_ => self.o`
    /// fallback arm — no panic is raised, `should_panic` FAILS
    /// (shim-court W5-M10 re-witnesses this flip on the repaired file).
    #[test]
    #[should_panic(expected = "out of range")]
    fn columns_court_out_of_range_slot_is_loud() {
        let s = [cw(1)];
        let p = [cw(100)];
        let o = [cw(2)];
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let _ = cols.col(3);
    }

    // --- Driver courts -----------------------------------------------------

    /// Fixpoint termination court: a self-deriving rule (`?x P1 ?x =>
    /// ?x P1 ?x` over `(A,P1,A)`) converges after one round — the driver
    /// cannot spin forever on a fixed point, and re-derivations are
    /// re-emitted (the projector emits derivations; dedup is A15's).
    /// Mutation that MUST flip this court: looping forever (budget hang)
    /// or deduping in the projector (popcount drops to 0).
    #[test]
    fn driver_court_self_derivation_terminates_and_reemits() {
        let s = [cw(1)];
        let p = [cw(100)];
        let o = [cw(1)];
        let rule = Rule {
            rule_id: 7,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(10))],
            head: [vw(10), cw(100), vw(10)],
        };
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let batches = unify_to_fixpoint(&[rule], &cols).unwrap();
        assert_eq!(batches.len(), 1, "one round, one rule");
        let b = &batches[0];
        assert_eq!(b.rule_id, 7);
        assert_eq!(b.epoch, 1);
        assert_eq!(b.source_mask_popcount, 1);
        assert_eq!(b.rows.len(), 1, "re-derivation emitted, not collapsed");
        assert_eq!(b.rows.row(0), (cw(1), cw(100), cw(1), 0));
    }

    /// Budget court: `max_rounds` is enforced with the typed variant.
    /// Mutation that MUST flip this court: ignoring the budget (unbounded
    /// loop) or returning Ok past the budget.
    #[test]
    fn driver_court_round_budget_is_typed() {
        let s = [cw(1)];
        let p = [cw(100)];
        let o = [cw(2)];
        let rule = Rule {
            rule_id: 0,
            body: vec![BodyAtom::new(vw(10), cw(100), vw(11))],
            head: [vw(10), cw(200), vw(11)],
        };
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        // The rule derives a fresh row in round 1; a budget of 1 must
        // refuse before any second round.
        assert_eq!(
            unify_to_fixpoint_opt(&[rule], &cols, 1, 4096),
            Err(UnifyError::FixpointBudgetExceeded { rounds: 1 })
        );
    }

    /// Corpus determinism court (spec 7.1): the same seed generates the
    /// identical corpus.
    #[test]
    fn corpus_court_same_seed_is_identical() {
        for seed in 0..10u64 {
            let a = gen_corpus(seed);
            let b = gen_corpus(seed);
            assert_eq!(a.s, b.s);
            assert_eq!(a.p, b.p);
            assert_eq!(a.o, b.o);
            assert_eq!(a.rules, b.rules);
        }
    }

    /// Differential court (spec 7.2, the lane's anti-silent-wrongness
    /// anchor): over 100 witnessed (corpus, batch) pairs, the register
    /// unifier's fixpoint derivations equal the naive recursive
    /// unifier's — and every rule's id appears in the emission (the
    /// driver-side M12-5 witness; dropping a rule is observable).
    /// Mutations that MUST flip this court: M12-1 (mask AND -> OR),
    /// M12-3 (join probe swap), M12-5 (silent rule drop), M12-8 (oracle
    /// occurs-check mutation — the court's own positive control).
    #[test]
    fn differential_court_matches_naive_oracle_over_100_witnessed_corpora() {
        for seed in 0..100u64 {
            let c = gen_witnessed_corpus(seed);
            let cols = StatementColumns::new(&c.s, &c.p, &c.o).unwrap();
            // Two chunkings: default and one-row chunks (forces every
            // join probe across a chunk boundary — recorded divergence 1
            // property: chunking never changes derivations).
            for chunk in [DEFAULT_CHUNK_ROWS, 1] {
                let batches =
                    unify_to_fixpoint_opt(&c.rules, &cols, DEFAULT_MAX_ROUNDS, chunk).unwrap();
                let mut emitted: BTreeSet<(u64, u64, u64)> = BTreeSet::new();
                let mut rule_ids: HashSet<u32> = HashSet::new();
                for b in &batches {
                    rule_ids.insert(b.rule_id);
                    for i in 0..b.rows.len() {
                        emitted.insert((b.rows.s[i], b.rows.p[i], b.rows.o[i]));
                    }
                }
                let (oracle, fired) = naive_closure_full(&c.rules, &c.s, &c.p, &c.o);
                assert!(fired.iter().all(|&f| f), "seed {seed}: witness failed");
                assert_eq!(
                    rule_ids.len(),
                    c.rules.len(),
                    "seed {seed} chunk {chunk}: every rule emitted (M12-5 witness)"
                );
                // The projector emits derivations, including re-derivations
                // of base rows (dedup is A15's). So the driver's store
                // closure must equal base ∪ emitted, and every emitted row
                // must be in the oracle closure (no phantom derivations).
                let base: BTreeSet<(u64, u64, u64)> =
                    (0..c.s.len()).map(|i| (c.s[i], c.p[i], c.o[i])).collect();
                let mut union = emitted.clone();
                union.extend(base.iter().copied());
                assert!(
                    emitted.is_subset(&oracle),
                    "seed {seed} chunk {chunk}: phantom derivation"
                );
                assert_eq!(
                    union, oracle,
                    "seed {seed} chunk {chunk}: differential mismatch"
                );
            }
        }
    }

    /// Determinism court (spec 7.3 M12-6): chunk partitioning the atom-0
    /// scan — including one-row chunks — leaves every batch byte-stable:
    /// rule ids, epochs, digests, popcounts, and row tuples.
    /// Mutation that MUST flip this court: any chunk-order-dependent
    /// emission (would break A15's digest anchoring).
    #[test]
    fn determinism_court_m12_6_chunk_partition_never_changes_batch_bytes() {
        for seed in 0..5u64 {
            let c = gen_witnessed_corpus(seed);
            let cols = StatementColumns::new(&c.s, &c.p, &c.o).unwrap();
            let mut reference: Option<Vec<(u32, u64, [u8; 32], u32, Vec<(u64, u64, u64, u16)>)>> =
                None;
            for &chunk in &[1usize, 3, 64, DEFAULT_CHUNK_ROWS] {
                let batches =
                    unify_to_fixpoint_opt(&c.rules, &cols, DEFAULT_MAX_ROUNDS, chunk).unwrap();
                let mut shape = Vec::new();
                for b in &batches {
                    let mut rows = Vec::new();
                    for i in 0..b.rows.len() {
                        rows.push(b.rows.row(i));
                    }
                    shape.push((b.rule_id, b.epoch, b.digest, b.source_mask_popcount, rows));
                }
                match &reference {
                    None => reference = Some(shape),
                    Some(r) => assert_eq!(
                        r, &shape,
                        "seed {seed} chunk {chunk}: batch bytes diverged"
                    ),
                }
            }
        }
    }

    /// Witness-independence court (DoD 7): the witnessed generator really
    /// does produce corpora where every rule fires — the M12-5 court's
    /// non-vacuity premise, asserted over the differential court's seed
    /// range.
    #[test]
    fn witness_court_every_rule_fires_across_seeds() {
        for seed in 0..100u64 {
            let c = gen_witnessed_corpus(seed);
            let (_, fired) = naive_closure_full(&c.rules, &c.s, &c.p, &c.o);
            assert!(fired.iter().all(|&f| f), "seed {seed}");
        }
    }

    // --- Wave 4: zero-allocation hot path (spec 7.4, DoD 6) --------------

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::thread_local;

    thread_local! {
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
        static IN_ALLOC: Cell<bool> = const { Cell::new(false) };
    }

    /// Pass-through counting allocator (house pattern:
    /// `crates/cgap-parquet/tests/page_zero_alloc.rs`). Thread-local
    /// counters keep concurrently running tests from polluting each
    /// other's measurement window. This is the `cgap-bn3` lib test
    /// binary's single `#[global_allocator]` (rustc rejects a second;
    /// the crate defines no other).
    struct Counting;

    impl Counting {
        fn bump() {
            if IN_ALLOC.with(Cell::get) {
                return;
            }
            IN_ALLOC.with(|c| c.set(true));
            ALLOCS.with(|c| c.set(c.get() + 1));
            IN_ALLOC.with(|c| c.set(false));
        }
    }

    // SAFETY: pure pass-through to System with a counted bump; the
    // contract of each method is delegated unchanged.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            Self::bump();
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            Self::bump();
            unsafe { System.realloc(ptr, layout, new_size) }
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            Self::bump();
            unsafe { System.alloc_zeroed(layout) }
        }
    }

    #[global_allocator]
    static COUNTING_ALLOC: Counting = Counting;

    fn current_allocs() -> usize {
        ALLOCS.with(Cell::get)
    }

    /// Positive control (anti-vacuity): the same measurement window around
    /// a fresh `Vec` MUST move the counter — a zero-alloc court whose
    /// counter cannot leave zero is measuring nothing. Mutation that MUST
    /// flip this court: bypassing `Counting::bump` (the counter reads 0
    /// through any allocation).
    #[test]
    fn zero_alloc_court_positive_control_counter_is_live() {
        let before = current_allocs();
        let probe: Vec<u8> = (0..64u8).collect();
        let delta = current_allocs() - before;
        println!("[za-court] positive-control allocs={delta}");
        assert!(
            delta >= 1,
            "counting allocator counted {delta} allocs for a fresh Vec — court is vacuous"
        );
        assert_eq!(probe.len(), 64);
    }

    /// Zero-allocation court (spec 7.4, DoD 6): after a warmup run sizes
    /// every buffer on this exact input, a second scan+project run over
    /// the same pre-built batch performs ZERO heap allocations, and both
    /// runs agree row-for-row with identical digests. Real kernel, real
    /// projector, real allocator — Chicago, final state asserted.
    /// Mutation that MUST flip this court: reintroducing per-call
    /// allocation in the eval path (executed shim: shadow the
    /// caller-owned scratch in `atom_local_match_into` with fresh
    /// `Vec::new()`s — the measured delta jumps to thousands and the
    /// court FAILS; see the Wave-4 lane report for the receipt).
    #[test]
    fn zero_alloc_court_scan_project_hot_path_allocates_zero_after_warmup() {
        // Pre-built 4096-row batch; the full 5-rule corpus (every rule
        // class of spec 7.1). Rule compilation happens OUTSIDE the
        // measured window (spec 7.4: compilation may allocate once).
        let rules = rule_set();
        let mut s = Vec::new();
        let mut p = Vec::new();
        let mut o = Vec::new();
        for i in 0..4096u64 {
            let subj = cw(1 + i % 7);
            s.push(subj);
            if i % 5 == 0 {
                // Self-loop feeds the self-join rule.
                p.push(cw(200));
                o.push(subj);
            } else {
                p.push(cw(100 + (i % 2) * 100));
                o.push(cw(1 + (i * 3) % 11));
            }
        }
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let compiled: Vec<CompiledRule> = rules
            .iter()
            .map(RuleCompiler::compile)
            .collect::<Result<_, _>>()
            .unwrap();

        let projector = ConsequentProjector::new();
        let mut scratch = EvalScratch::default();
        let mut out = MatchSet::default();
        let mut rows = crate::project::DeltaRows::default();
        let mut canon = Vec::new();

        // Warmup: per rule, size every buffer on this exact input, and
        // record the reference emission (rows + digest). TWO passes: the
        // scan's cur/nxt double buffer carries per-role capacity, so the
        // first pass warms the match-side buffers and the second pass
        // warms the expansion-side reserve — steady state (zero allocs)
        // is the third and later identical runs.
        let mut reference: Vec<([u8; 32], Vec<(u64, u64, u64, u16)>)> = Vec::new();
        for _pass in 0..2 {
            for c in &compiled {
                unify_batch_into(c, &cols, DEFAULT_CHUNK_ROWS, &mut scratch, &mut out).unwrap();
                let (d, _) = projector.emit_into(c, 1, &out, &mut rows, &mut canon).unwrap();
                if _pass == 0 {
                    reference.push((d, (0..rows.len()).map(|i| rows.row(i)).collect()));
                }
            }
        }

        // Pre-reserve the capture buffers from the warmup's exact shapes:
        // every allocation the capture could need happens here, outside
        // the measured window.
        let mut measured: Vec<([u8; 32], Vec<(u64, u64, u64, u16)>)> = Vec::new();
        for (d, rows_snapshot) in &reference {
            measured.push((*d, Vec::with_capacity(rows_snapshot.len())));
        }

        // Measured window, split into phases so a leak localizes itself:
        // (a) scan only, (b) project only, (c) capture.
        let mut scan_delta = 0usize;
        let mut project_delta = 0usize;
        let mut capture_delta = 0usize;
        for (k, c) in compiled.iter().enumerate() {
            let before = current_allocs();
            unify_batch_into(c, &cols, DEFAULT_CHUNK_ROWS, &mut scratch, &mut out).unwrap();
            scan_delta += current_allocs() - before;

            let before = current_allocs();
            let (d, _) = projector.emit_into(c, 1, &out, &mut rows, &mut canon).unwrap();
            project_delta += current_allocs() - before;

            let before = current_allocs();
            let slot = &mut measured[k];
            slot.0 = d;
            for i in 0..rows.len() {
                slot.1.push(rows.row(i));
            }
            capture_delta += current_allocs() - before;
        }
        let delta = scan_delta + project_delta + capture_delta;
        println!(
            "[za-court] scan={scan_delta} project={project_delta} capture={capture_delta} total={delta}"
        );
        assert_eq!(
            delta, 0,
            "scan+project hot path allocated {delta} times after warmup (scan={scan_delta} project={project_delta} capture={capture_delta})"
        );
        assert_eq!(measured, reference, "warm run diverged from the warmup run");
    }

    /// Stress court (Wave 4 hardening): the fixpoint driver over a
    /// 256-row store with the full rule corpus converges within an
    /// explicit round budget, derives rows, and the emitted batch
    /// sequence is byte-identical (ids, epochs, digests, popcounts)
    /// across chunk sizes 1/64/256 — M12-6 partition independence at
    /// scale.
    /// Mutation that MUST flip this court: any chunk-order-dependent
    /// emission or a budget overrun.
    #[test]
    fn stress_court_fixpoint_converges_and_stays_chunk_stable_at_256_rows() {
        let mut s = Vec::new();
        let mut p = Vec::new();
        let mut o = Vec::new();
        for i in 0..256u64 {
            let subj = cw(1 + i % 48);
            s.push(subj);
            if i % 5 == 0 {
                p.push(cw(200));
                o.push(subj);
            } else {
                p.push(cw(100 + (i % 2) * 100));
                o.push(cw(1 + (i * 7 + 3) % 48));
            }
        }
        let cols = StatementColumns::new(&s, &p, &o).unwrap();
        let rules = rule_set();
        let mut reference: Option<Vec<(u32, u64, [u8; 32], u32)>> = None;
        for &chunk in &[1usize, 64, 256] {
            let batches =
                unify_to_fixpoint_opt(&rules, &cols, 200, chunk).expect("fixpoint converges");
            let shape: Vec<(u32, u64, [u8; 32], u32)> = batches
                .iter()
                .map(|b| (b.rule_id, b.epoch, b.digest, b.source_mask_popcount))
                .collect();
            let total_rows: usize = batches.iter().map(|b| b.rows.len()).sum();
            assert!(total_rows > 0, "chunk {chunk}: no derivations");
            match &reference {
                None => reference = Some(shape),
                Some(r) => assert_eq!(r, &shape, "chunk {chunk}: batch shape diverged"),
            }
        }
    }
    // ====================================================================
    // Wave 9: cross-lane scale court (wave-9 slice item "A20 1M-row
    // stress"): the fixpoint driver + projector at one million rows —
    // convergence, closed-form derivation expectations, and M12-6
    // chunk-partition stability AT SCALE.
    // ====================================================================

    /// One-million-row stress court (wave 9): 1,000,000-row store, three
    /// single-atom rules with controlled selectivity whose expected closure
    /// is computable in closed form by an INDEPENDENT single-pass model
    /// (the court's oracle — same role as the naive unifier, no kernel
    /// machinery reused). Asserts:
    ///   1. convergence inside the round budget (Ok, never BudgetExceeded);
    ///   2. per-rule, per-round popcounts equal the model's match counts;
    ///   3. the derived distinct set equals the model's set (no phantom,
    ///      no missing derivation at 1M scale);
    ///   4. epochs are 1 then 2 per rule (round 2 re-emits, adds nothing);
    ///   5. batch shapes — ids, epochs, digests, popcounts — are identical
    ///      across chunk sizes 262144 and DEFAULT_CHUNK_ROWS (M12-6 at 1M).
    /// Mutation that MUST flip this court: silently skipping one of the
    /// three rules (the derived-set equality drops below the model), any
    /// mask/join corruption at scale, or a budget bypass.
    #[test]
    fn stress_court_1m_rows_converges_chunk_stable_matches_closed_form_model() {
        const ROWS: u64 = 1_000_000;
        const P1: u64 = 100;
        const P2: u64 = 200;
        const P4: u64 = 400;
        const BUDGET: u32 = 10;

        let s: Vec<u64> = (0..ROWS).map(|i| cw(1 + i % 7)).collect();
        let p: Vec<u64> = (0..ROWS)
            .map(|i| if i % 5 == 0 { P2 } else { P1 })
            .collect();
        let o: Vec<u64> = (0..ROWS).map(|i| {
            if i % 5 == 0 {
                cw(1 + i % 7)
            } else {
                cw(1 + (i * 3) % 11)
            }
        }).collect();
        let cols = StatementColumns::new(&s, &p, &o).unwrap();

        let rules = vec![
            // R0: ?x P1 ?y => ?x P4 ?y — renaming, no new join fuel.
            Rule {
                rule_id: 0,
                body: vec![BodyAtom::new(vw(10), cw(P1), vw(11))],
                head: [vw(10), cw(P4), vw(11)],
            },
            // R1: ?x P2 ?x => ?x P4 c(1) — self-join.
            Rule {
                rule_id: 1,
                body: vec![BodyAtom::new(vw(10), cw(P2), vw(10))],
                head: [vw(10), cw(P4), cw(1)],
            },
            // R2: (c(2), P1, ?y) => (?y, P4, c(2)) — constant-bound.
            Rule {
                rule_id: 2,
                body: vec![BodyAtom::new(cw(2), cw(P1), vw(11))],
                head: [vw(11), cw(P4), cw(2)],
            },
        ];

        // Independent model: one O(N) pass, straight row scan — no kernel
        // machinery (the court's oracle).
        let mut n_p1 = 0u64;
        let mut n_self = 0u64;
        let mut n_r2 = 0u64;
        let mut model: BTreeSet<(u64, u64, u64)> = BTreeSet::new();
        for i in 0..ROWS as usize {
            let (sv, pv, ov) = (s[i], p[i], o[i]);
            if pv == P1 {
                n_p1 += 1;
                model.insert((sv, P4, ov));
            } else {
                n_self += 1;
                model.insert((sv, P4, cw(1)));
            }
            if sv == cw(2) && pv == P1 {
                n_r2 += 1;
                model.insert((ov, P4, cw(2)));
            }
        }
        assert!(n_p1 > 500_000, "model selectivity sanity: n_p1={n_p1}");
        assert!(n_self == 200_000, "model selectivity sanity: n_self={n_self}");
        assert!(n_r2 > 50_000, "model selectivity sanity: n_r2={n_r2}");

        let mut reference: Option<Vec<(u32, u64, [u8; 32], u32)>> = None;
        for chunk in [262_144usize, DEFAULT_CHUNK_ROWS] {
            let batches = unify_to_fixpoint_opt(&rules, &cols, BUDGET, chunk)
                .expect("1M fixpoint converges within budget");
            assert_eq!(batches.len(), 6, "3 rules x 2 rounds");
            for (k, b) in batches.iter().enumerate() {
                let round = k / 3 + 1;
                assert_eq!(b.epoch, round as u64, "batch {k} epoch");
                let expect_popcount = match b.rule_id {
                    0 => n_p1,
                    1 => n_self,
                    2 => n_r2, // identical in both rounds: P4 rows match no body
                    other => panic!("unexpected rule id {other}"),
                };
                assert_eq!(b.source_mask_popcount as u64, expect_popcount, "batch {k}");
                assert!(b.rows.len() > 0, "every rule derives at 1M scale");
            }
            // Distinct derived set == model set (no phantom, no missing).
            let mut derived: BTreeSet<(u64, u64, u64)> = BTreeSet::new();
            for b in &batches {
                for i in 0..b.rows.len() {
                    let (sv, pv, ov, fl) = b.rows.row(i);
                    assert_eq!(fl, 0);
                    assert_eq!(pv, P4, "all derivations carry the P4 marker");
                    derived.insert((sv, pv, ov));
                }
            }
            assert_eq!(derived, model, "derived set vs closed-form model");
            let shape: Vec<(u32, u64, [u8; 32], u32)> = batches
                .iter()
                .map(|b| (b.rule_id, b.epoch, b.digest, b.source_mask_popcount))
                .collect();
            match &reference {
                None => reference = Some(shape),
                Some(r) => assert_eq!(r, &shape, "1M: batch shape diverged at chunk {chunk}"),
            }
        }
    }
}
