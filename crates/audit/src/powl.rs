//! POWL v2 transition verifier — lane A15, full-lane slice (Wave 3).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A15-powl-ocel2.md` — §2.4 (normative
//! construct semantics), §5 (typed verdict/errors + allocation law), §6
//! (courts). Wave 1 froze the fixed-width ABI (`PowlViolation` /
//! `PowlConformance` + layout pins); this slice lands the logic on it:
//! model validation + compilation, binding projection (§5.1), the
//! allocation-free check path (§5.1 "no allocation in the check path
//! beyond the binding index" — proven by the alloc court), and `verify()`
//! with typed error channels.
//!
//! Semantics notes (recorded, not silent):
//! - Ordering within a binding is SLAB ORDER, never wall clock (§8
//!   timestamp-monotonicity risk mitigation).
//! - A binding is closed iff it contains a model-declared terminal event;
//!   an EMPTY terminal list selects the strict floor (every binding treated
//!   closed — checks enforced, never a silent pass; §8 loop-ambiguity
//!   mitigation degraded to enforcement because the frozen verdict ABI has
//!   no warning channel).
//! - The frozen verdict ABI (§5: `Result<PowlConformance, PowlViolation>`)
//!   carries ONE violation; `check()` returns the FIRST violation under
//!   node post-order x binding ascending-first-event-row order (Wave-8 D3:
//!   the verdict is a pure function of log bytes + model — pre-repair the
//!   binding order was HashMap iteration order, so which violation surfaced
//!   varied run-to-run). A full findings list needs an ABI extension
//!   (follow-up lane).
//! - Concurrency and Parallel share the presence check (§2.4 defines both
//!   as presence constraints); their distinction is ordering attitude,
//!   proved by the concurrency vacuity court (B-then-A must be ACCEPTED).
//! - Nested model trees: every operator node is checked independently over
//!   the full binding space, using each child's leaf-activity set (flat
//!   model checking; full nest-compositional semantics is out of this
//!   lane's scope).
//!
//! Wave 8 (defect repair, driven by re-derived wave-7 findings — no A15
//! wave-7 receipt file exists on disk, the divergence is recorded in the
//! wave receipt): two repairs, each with a court that flips on the
//! pre-repair baseline.
//! - D3 (powl): verdict evidence nondeterminism. `BindingIndex` iterated a
//!   `HashMap`, so with >= 2 violating bindings the evidence event id in
//!   the frozen wire ABI varied run-to-run. Bindings are now ordered by
//!   ascending first-event row. Court: 64 fresh index builds over a
//!   two-violation log must yield the identical verdict (per-instance
//!   RandomState makes the pre-repair flip ~2^-63 likely per run).
//! - D6 (powl): `compile` accepted sibling children with OVERLAPPING
//!   leaf-activity sets (`Choice[A, A]`); `ordinal_of` attributes every
//!   event to the first matching child, so the construct check could never
//!   fire — a silent tautology. Overlap is refused typed
//!   (`InvalidModel`). Court: overlap refused, disjoint control compiles.
//!
//! Wave 9 (cross-lane integration): A20 stress at the POWL surface —
//! `verify()` (validate → compile → project → check) over a 1,000,000-event
//! real append-path log, conforming verdict pinned, plus the violating
//! control (the same 1M rows with the first event flipped to the redo-side
//! arm is REJECTED with pinned evidence — the green court can see red).
//! The stress exposed D8 (below): the binding projection was O(n^2);
//! repaired this wave. Measured wall-clock is printed (dev profile, single
//! run, this host) — an observation, never a spec claim.

use core::mem::{align_of, offset_of, size_of};
use std::collections::HashMap;

use crate::ocel2_slab::{validate_log, Ocel2ColumnarLog};

/// §5: evidence cap per violation — max event ids carried inline (frozen at
/// Wave 1; the Wave-3 check path writes at most 1 evidence id per
/// violation, under the cap).
pub const POWL_EVIDENCE_CAP: usize = 8;

/// §2.4: five operators (Sequence, Choice, Parallel, Concurrency, Loop).
pub const POWL_CONSTRUCT_COUNT: usize = 5;

/// Operator arity ceiling: the check path classifies child ordinals in a
/// u64 presence bitmask, so an operator may have at most 64 children.
/// Above this, model compilation fails with a typed error.
pub const POWL_MAX_ARITY: usize = 64;

/// §2.4 POWL v2 constructs, wire discriminants frozen at Wave 1 (`repr(u8)`,
/// explicit values — these cross the boundary in `PowlViolation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PowlConstruct {
    /// A→B: every B preceded by A in the same binding (§2.4).
    Sequence = 1,
    /// A⊗B: mutually exclusive within one case binding (§2.4).
    Choice = 2,
    /// A∥B: both must occur in every complete case (§2.4).
    Parallel = 3,
    /// A⇄B: unrestricted interleaving; ordering unconstrained (§2.4).
    Concurrency = 4,
    /// A∘B: after a redo (B), A must re-occur before case completion (§2.4).
    Loop = 5,
}

/// §5 typed errors: "no `panic!` on malformed logs — typed
/// `Err(PowlError::MalformedLog)`". Discriminants frozen (`repr(u8)`).
///
/// Wave-3 addition (recorded, additive: discriminant 1 unchanged): 2 =
/// malformed MODEL (arity violations). A bad model is not a bad log; the
/// single `MalformedLog` variant could not carry that distinction without
/// lying in its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum PowlError {
    /// §5: malformed log input surfaces as a typed error, never a panic.
    MalformedLog = 1,
    /// Wave-3 additive: malformed model (operator arity outside the
    /// construct's legal range or above [`POWL_MAX_ARITY`]).
    InvalidModel = 2,
}

/// §5 fixed-width violation record: construct + evidence event ids —
/// "evidence as event ids (u64), never strings". Gap-free: offsets 0, 1-3,
/// 4-7, 8-71 fully occupied; exactly 72 bytes, align 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C, align(8))]
pub struct PowlViolation {
    /// offset 0, 1 B — construct whose constraint was violated (§5)
    pub construct: PowlConstruct,
    /// offset 1, 3 B — zero
    pub reserved: [u8; 3],
    /// offset 4, 4 B — valid entries in `evidence` (<= `POWL_EVIDENCE_CAP`;
    /// the check path writes at most 1)
    pub evidence_len: u32,
    /// offset 8, 64 B — evidence event ids (u64, never strings) (§5)
    pub evidence: [u64; POWL_EVIDENCE_CAP],
}

const _: () = assert!(size_of::<PowlViolation>() == 72);
const _: () = assert!(align_of::<PowlViolation>() == 8);
const _: () = assert!(size_of::<PowlViolation>() == 8 + POWL_EVIDENCE_CAP * 8); // gap-free self-check

const _: () = assert!(offset_of!(PowlViolation, construct) == 0);
const _: () = assert!(offset_of!(PowlViolation, reserved) == 1);
const _: () = assert!(offset_of!(PowlViolation, evidence_len) == 4);
const _: () = assert!(offset_of!(PowlViolation, evidence) == 8);

/// §5 fixed-width conformance record (the `Ok` arm of the verdict).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C, align(8))]
pub struct PowlConformance {
    /// offset 0, 4 B — number of model constructs evaluated
    pub constructs_checked: u32,
    /// offset 4, 4 B — number of object bindings walked
    pub bindings_checked: u32,
}

const _: () = assert!(size_of::<PowlConformance>() == 8);
const _: () = assert!(align_of::<PowlConformance>() == 8);
const _: () = assert!(offset_of!(PowlConformance, constructs_checked) == 0);
const _: () = assert!(offset_of!(PowlConformance, bindings_checked) == 4);

/// §5: "`verify(log, model) -> Verdict`" with "Verdict is a
/// `Result<PowlConformance, PowlViolation>`". §5 also names
/// `Err(PowlError::MalformedLog)` for malformed input. Wave-3 decision on
/// the two error channels (the Wave-1 freeze left the nesting open):
/// `verify` returns `Result<PowlVerdict, PowlError>` — the outer channel is
/// the typed `PowlError` (malformed log / invalid model), the inner is the
/// §5 verdict. The low-level [`check`] path returns the verdict directly
/// because it is defined only over validated inputs.
pub type PowlVerdict = Result<PowlConformance, PowlViolation>;

/// §2.4/§5: "A `PowlModel` is a tree of these operators."
///
/// Wave-3 addition (recorded, additive — this type was explicitly NOT
/// layout-frozen at Wave 1): `terminals`, the model-declared terminal
/// activities used to decide binding closure (§8 loop/parallel
/// completeness risks). Empty list = strict floor (every binding treated
/// closed; checks enforced, never silently skipped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowlModel {
    /// root operator (or single-activity leaf) of the model tree
    pub root: PowlNode,
    /// §8: declared terminal activities. A binding is closed iff it
    /// contains an event of one of these activities; an empty list selects
    /// the strict floor (every binding treated closed).
    pub terminals: Vec<u16>,
}

/// §2.4 model-tree node: a leaf activity (interned code, §4.1 intern table)
/// or an operator over child nodes. Heap children (`Vec`) are Wave-1
/// definitions, not pinned ABI — only the fixed-width wire types above are
/// layout-frozen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowlNode {
    /// leaf: interned activity code
    Activity(u16),
    /// operator over children; arity validation happens in
    /// [`compile`]/[`verify`] as a typed error (§5 — malformed models
    /// surface typed, never panic)
    Operator {
        construct: PowlConstruct,
        children: Vec<PowlNode>,
    },
}

// ---------------------------------------------------------------------------
// Wave 3: model compilation
// ---------------------------------------------------------------------------

/// One compiled operator node: the construct plus each child's leaf-activity
/// set (sorted, deduped). "Occurrence of child k" = an event whose activity
/// is in child k's leaf set. Sorted arrays keep the check path allocation
/// free (binary search, no map).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledNode {
    /// operator whose constraint this node enforces
    pub construct: PowlConstruct,
    /// per-child sorted leaf-activity sets (arity validated at compile)
    pub children: Vec<Vec<u16>>,
}

/// Flat compiled model: every operator node of the tree, in post-order, plus
/// the sorted terminal set. Produced by [`compile`]; consumed by [`check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPowlModel {
    /// operator nodes, post-order (children before parents)
    pub nodes: Vec<CompiledNode>,
    /// sorted, deduped terminal activities (closure decision, §8)
    pub terminals: Vec<u16>,
}

fn collect_leaves(node: &PowlNode, out: &mut Vec<u16>) {
    match node {
        PowlNode::Activity(a) => out.push(*a),
        PowlNode::Operator { children, .. } => {
            for c in children {
                collect_leaves(c, out);
            }
        }
    }
}

fn validate_model_node(node: &PowlNode) -> Result<(), PowlError> {
    match node {
        PowlNode::Activity(_) => Ok(()),
        PowlNode::Operator { construct, children } => {
            let legal = match construct {
                PowlConstruct::Sequence
                | PowlConstruct::Choice
                | PowlConstruct::Parallel
                | PowlConstruct::Concurrency => (2..=POWL_MAX_ARITY).contains(&children.len()),
                // Loop is the do/redo pair (§2.4)
                PowlConstruct::Loop => children.len() == 2,
            };
            if !legal {
                return Err(PowlError::InvalidModel);
            }
            // Wave-8 D6 (defect repair): sibling children must have
            // pairwise DISJOINT leaf-activity sets. With an overlap (e.g.
            // `Choice[A, A]`), `ordinal_of` attributes every event to the
            // FIRST matching child, so the construct check can never fire —
            // the model would vacuously conform to any log, a silent
            // tautology. Refused typed at compile; never a silent pass.
            let mut sets: Vec<Vec<u16>> = Vec::with_capacity(children.len());
            for c in children {
                let mut s = Vec::new();
                collect_leaves(c, &mut s);
                s.sort_unstable();
                s.dedup();
                sets.push(s);
            }
            for (i, si) in sets.iter().enumerate() {
                for sj in &sets[i + 1..] {
                    if si.iter().any(|a| sj.binary_search(a).is_ok()) {
                        return Err(PowlError::InvalidModel);
                    }
                }
            }
            for c in children {
                validate_model_node(c)?;
            }
            Ok(())
        }
    }
}

/// Compiles a model tree into the flat check form. Typed error on malformed
/// models (arity); never panics.
pub fn compile(model: &PowlModel) -> Result<CompiledPowlModel, PowlError> {
    validate_model_node(&model.root)?;
    let mut nodes = Vec::new();
    collect_nodes(&model.root, &mut nodes);
    let mut terminals = model.terminals.clone();
    terminals.sort_unstable();
    terminals.dedup();
    Ok(CompiledPowlModel { nodes, terminals })
}

fn collect_nodes(node: &PowlNode, out: &mut Vec<CompiledNode>) {
    match node {
        PowlNode::Activity(_) => {}
        PowlNode::Operator { construct, children } => {
            for c in children {
                collect_nodes(c, out);
            }
            let mut sets: Vec<Vec<u16>> = Vec::with_capacity(children.len());
            for c in children {
                let mut s = Vec::new();
                collect_leaves(c, &mut s);
                s.sort_unstable();
                s.dedup();
                sets.push(s);
            }
            out.push(CompiledNode {
                construct: *construct,
                children: sets,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Wave 3: binding projection (§5.1)
// ---------------------------------------------------------------------------

/// One projected binding: the sorted, deduped object-id key plus its event
/// row indices in slab order.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BindingEntry {
    key: Vec<u64>,
    events: Vec<u32>,
}

/// Binding index: events grouped by their (sorted, deduped) object-id set,
/// ordered by ASCENDING FIRST-EVENT ROW.
///
/// Wave-8 D3 (defect repair): the verdict is a frozen wire ABI whose
/// evidence event ids are what a court receipt cites — which violation
/// surfaces when two or more bindings violate MUST therefore be a pure
/// function of the log bytes + model. The pre-repair index iterated a
/// `HashMap` (per-instance `RandomState` hash order), so the reported
/// evidence event id varied run-to-run whenever two bindings violated —
/// the verdict was not replayable. Each binding's event list still
/// preserves SLAB ORDER (§8: slab order is the only ordering the checks
/// consult).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingIndex {
    /// bindings, ascending by first event row (the D3 determinism law).
    /// Private: no consumer outside this module (Wave-1 ABI hygiene — the
    /// frozen surface is the fixed-width wire types + the free functions;
    /// the index is an internal projection artifact).
    entries: Vec<BindingEntry>,
}

impl BindingIndex {
    /// Number of distinct bindings (all have >= 1 event; events with zero
    /// object references form no binding and are excluded from construct
    /// checks — §2.4 skip/silent transitions are tolerated by absence).
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.entries.len()
    }
}

/// Projects the log's event columns into per-binding event sequences
/// (§5.1). This is the allocating phase the allocation law excepts ("beyond
/// the binding index"); the check path itself is allocation-free.
#[must_use]
pub fn index_bindings(log: &Ocel2ColumnarLog) -> BindingIndex {
    let mut map: HashMap<Vec<u64>, Vec<u32>> = HashMap::with_capacity(log.event_count());
    // Wave-9 defect repair D8: projection previously called
    // `object_ids_for(i)` per event, recomputing the segment prefix sum
    // from row 0 each call — O(n^2) in event rows (the A20 1M-row stress
    // could not complete pre-repair; post-repair timing witness in the
    // wave report). One linear pass with a running segment offset;
    // identical clamping semantics to `object_ids_for` (saturating
    // accumulation, end clamped to the flat column, start clamped to end),
    // so the grouping law — and the D3 determinism law over it — is
    // unchanged on every log, consistent or not.
    let mut seg = 0usize;
    for i in 0..log.event_count() {
        let count = log.object_count.get(i).copied().unwrap_or(0) as usize;
        let end = seg.saturating_add(count).min(log.object_ids.len());
        let start = seg.min(end);
        let ids = &log.object_ids[start..end];
        seg = seg.saturating_add(count);
        if ids.is_empty() {
            continue;
        }
        let mut key = ids.to_vec();
        key.sort_unstable();
        key.dedup();
        let entry = map.entry(key).or_default();
        entry.push(i as u32);
    }
    // D3: order bindings by first event row so `check` walks them in slab
    // order — the violation reported into the wire ABI is then the first
    // one under that fixed order, a pure function of (log, model).
    let mut entries: Vec<BindingEntry> = map
        .into_iter()
        .map(|(key, events)| BindingEntry { key, events })
        .collect();
    entries.sort_by_key(|b| b.events[0]);
    BindingIndex { entries }
}

// ---------------------------------------------------------------------------
// Wave 3: the check path (allocation-free)
// ---------------------------------------------------------------------------

fn ordinal_of(node: &CompiledNode, activity: u16) -> Option<u32> {
    for (k, set) in node.children.iter().enumerate() {
        if set.binary_search(&activity).is_ok() {
            return Some(k as u32);
        }
    }
    None
}

fn violation_of(construct: PowlConstruct, evidence_event_id: u64) -> PowlViolation {
    PowlViolation {
        construct,
        reserved: [0; 3],
        evidence_len: 1,
        evidence: [
            evidence_event_id, 0, 0, 0, 0, 0, 0, 0,
        ],
    }
}

fn binding_closed(log: &Ocel2ColumnarLog, events: &[u32], terminals: &[u16]) -> bool {
    // Strict floor (§8): no terminals declared -> treat every binding as
    // closed so the completeness checks are ENFORCED, never silently
    // skipped.
    if terminals.is_empty() {
        return true;
    }
    events
        .iter()
        .any(|&e| terminals.binary_search(&log.activity[e as usize]).is_ok())
}

/// §2.4 Sequence: every occurrence of a later child's activity must be
/// preceded, in the same binding, by an occurrence of some earlier child's
/// activity. Slab-order walk with a u64 seen-bitmask (arity <= 64).
fn check_sequence(log: &Ocel2ColumnarLog, node: &CompiledNode, events: &[u32]) -> Option<PowlViolation> {
    let mut seen: u64 = 0;
    for &e in events {
        let activity = log.activity[e as usize];
        let Some(o) = ordinal_of(node, activity) else {
            continue;
        };
        if o > 0 {
            let prefix = (1u64 << o) - 1; // o < 64 guaranteed by compile arity cap
            if seen & prefix == 0 {
                return Some(violation_of(
                    PowlConstruct::Sequence,
                    log.event_id[e as usize],
                ));
            }
        }
        seen |= 1u64 << o;
    }
    None
}

/// §2.4 Choice: within one binding, children are mutually exclusive — once
/// an event of child k is seen, an event of any other child violates.
fn check_choice(log: &Ocel2ColumnarLog, node: &CompiledNode, events: &[u32]) -> Option<PowlViolation> {
    let mut first: Option<u32> = None;
    for &e in events {
        let Some(o) = ordinal_of(node, log.activity[e as usize]) else {
            continue;
        };
        match first {
            None => first = Some(o),
            Some(f) if f != o => {
                return Some(violation_of(
                    PowlConstruct::Choice,
                    log.event_id[e as usize],
                ));
            }
            Some(_) => {}
        }
    }
    None
}

/// §2.4 Parallel/Concurrency presence check: every CLOSED binding must
/// contain at least one event of EVERY child. Open bindings are not
/// complete cases; they are skipped, never flagged (§8).
fn check_presence(
    log: &Ocel2ColumnarLog,
    node: &CompiledNode,
    events: &[u32],
    closed: bool,
) -> Option<PowlViolation> {
    if !closed {
        return None;
    }
    let mut seen: u64 = 0;
    for &e in events {
        if let Some(o) = ordinal_of(node, log.activity[e as usize]) {
            seen |= 1u64 << o;
        }
    }
    for k in 0..node.children.len() as u32 {
        if seen & (1u64 << k) == 0 {
            let last = events[events.len() - 1];
            return Some(violation_of(node.construct, log.event_id[last as usize]));
        }
    }
    None
}

/// §2.4 Loop: children are (do, redo). For every redo event in a CLOSED
/// binding, a later do event must exist in the same binding (slab order).
fn check_loop(
    log: &Ocel2ColumnarLog,
    node: &CompiledNode,
    events: &[u32],
    closed: bool,
) -> Option<PowlViolation> {
    if !closed {
        return None;
    }
    for pos in 0..events.len() {
        let e = events[pos];
        if ordinal_of(node, log.activity[e as usize]) == Some(1) {
            let has_later_do = events[pos + 1..]
                .iter()
                .any(|&f| ordinal_of(node, log.activity[f as usize]) == Some(0));
            if !has_later_do {
                return Some(violation_of(PowlConstruct::Loop, log.event_id[e as usize]));
            }
        }
    }
    None
}

/// Runs every compiled construct over every binding. Defined ONLY over a
/// validated log and a compiled model (see [`verify`]); allocation-free by
/// construction — reads columns and the prebuilt index, returns the verdict
/// by value. This is the path the §5 allocation law and its court target.
#[must_use]
pub fn check(log: &Ocel2ColumnarLog, index: &BindingIndex, compiled: &CompiledPowlModel) -> PowlVerdict {
    for node in &compiled.nodes {
        for binding in &index.entries {
            let events: &[u32] = &binding.events;
            let closed = binding_closed(log, events, &compiled.terminals);
            let found = match node.construct {
                PowlConstruct::Sequence => check_sequence(log, node, events),
                PowlConstruct::Choice => check_choice(log, node, events),
                PowlConstruct::Parallel => check_presence(log, node, events, closed),
                PowlConstruct::Concurrency => check_presence(log, node, events, closed),
                PowlConstruct::Loop => check_loop(log, node, events, closed),
            };
            if let Some(violation) = found {
                return Err(violation);
            }
        }
    }
    Ok(PowlConformance {
        constructs_checked: compiled.nodes.len() as u32,
        bindings_checked: index.entries.len() as u32,
    })
}

/// §5 public API: validates the log and model (typed errors, never a
/// panic), projects bindings, and runs the construct checks.
pub fn verify(log: &Ocel2ColumnarLog, model: &PowlModel) -> Result<PowlVerdict, PowlError> {
    validate_log(log).map_err(|_| PowlError::MalformedLog)?;
    let compiled = compile(model)?;
    let index = index_bindings(log);
    Ok(check(log, &index, &compiled))
}

// ---------------------------------------------------------------------------
// Courts (§6.1): conforming + violating fixtures per construct, built by the
// REAL append path. Each court MUST reject its violating fixture — a court
// that passes a violating log is FAIL (vacuous).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alloc_counter::with_alloc_tracking;
    use crate::ocel2_slab::{Ocel2ColumnarLog, OcelEventVector, OCEL2_ATTR_WORDS};

    const A: u16 = 1;
    const B: u16 = 2;
    const C: u16 = 3;
    const OBJ: u64 = 100;

    fn vec_for(event_id: u64, ts_ns: u64, activity: u16, object_id: u64) -> OcelEventVector {
        OcelEventVector {
            event_id,
            ts_ns,
            activity,
            qualifier: 0,
            object_count: 1,
            reserved: 0,
            object_ids: [object_id, 0, 0, 0, 0, 0, 0, 0],
            attr_blob: [0; OCEL2_ATTR_WORDS],
        }
    }

    fn log_with(events: &[(u64, u64, u16, u64)]) -> Ocel2ColumnarLog {
        let mut log = Ocel2ColumnarLog::with_capacity(events.len(), events.len(), 1);
        for &(id, ts, a, o) in events {
            log.append(&vec_for(id, ts, a, o)).unwrap();
        }
        log
    }

    fn model(construct: PowlConstruct, acts: &[u16]) -> PowlModel {
        PowlModel {
            root: PowlNode::Operator {
                construct,
                children: acts.iter().map(|&a| PowlNode::Activity(a)).collect(),
            },
            terminals: Vec::new(),
        }
    }

    /// Unwraps the outer typed-error channel and the verdict's Ok arm,
    /// returning the violation. Panics (loudly, in-court) on conforming
    /// input — that panic IS the court flip for a violating fixture.
    fn violation_of_verdict(verdict: PowlVerdict) -> PowlViolation {
        match verdict {
            Ok(conf) => panic!("court: expected violation, got Ok({conf:?})"),
            Err(vio) => vio,
        }
    }

    #[test]
    fn sequence_court_conforming_and_violating() {
        let m = model(PowlConstruct::Sequence, &[A, B]);
        // Conforming: A then B, same binding.
        let ok = verify(&log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ)]), &m)
            .unwrap()
            .unwrap();
        assert_eq!(
            ok,
            PowlConformance {
                constructs_checked: 1,
                bindings_checked: 1
            }
        );
        // Violating: B before A — the court MUST reject (anti-vacuity flip).
        let vio =
            violation_of_verdict(verify(&log_with(&[(1, 10, B, OBJ), (2, 20, A, OBJ)]), &m).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Sequence);
        assert_eq!(vio.evidence_len, 1);
        assert_eq!(vio.evidence[0], 1, "evidence is the out-of-order B event id");
    }

    #[test]
    fn sequence_court_bindings_are_independent() {
        let m = model(PowlConstruct::Sequence, &[A, B]);
        // Binding 100 conforms; binding 200 violates. The violation must be
        // found and pinned to binding 200's event — never laundered across
        // bindings.
        let log = log_with(&[(1, 10, A, 100), (2, 20, B, 100), (3, 30, B, 200), (4, 40, A, 200)]);
        let vio = violation_of_verdict(verify(&log, &m).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Sequence);
        assert_eq!(vio.evidence[0], 3);
    }

    #[test]
    fn choice_court_conforming_and_violating() {
        let m = model(PowlConstruct::Choice, &[A, B]);
        let ok = verify(&log_with(&[(1, 10, A, OBJ)]), &m).unwrap().unwrap();
        assert_eq!(ok.constructs_checked, 1);
        // Violating: both A and B in one binding — MUST reject.
        let log = log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ)]);
        let vio = violation_of_verdict(verify(&log, &m).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Choice);
        assert_eq!(vio.evidence[0], 2, "evidence is the second exclusive arm's event");
    }

    #[test]
    fn parallel_court_conforming_and_violating() {
        let m = model(PowlConstruct::Parallel, &[A, B]);
        let ok = verify(&log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ)]), &m)
            .unwrap()
            .unwrap();
        assert_eq!(ok.constructs_checked, 1);
        // Violating: binding missing B entirely — MUST reject.
        let log = log_with(&[(1, 10, A, OBJ)]);
        let vio = violation_of_verdict(verify(&log, &m).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Parallel);
        assert_eq!(vio.evidence[0], 1, "evidence is the incomplete binding's last event");
    }

    #[test]
    fn concurrency_vacuity_court_accepts_b_then_a() {
        // §6.1 Concurrency row: the checker must NOT reject B-then-A. If the
        // check imposed ordering this would fail — the court's flip.
        let m = model(PowlConstruct::Concurrency, &[A, B]);
        let ok = verify(&log_with(&[(1, 10, B, OBJ), (2, 20, A, OBJ)]), &m)
            .unwrap()
            .unwrap();
        assert_eq!(
            ok,
            PowlConformance {
                constructs_checked: 1,
                bindings_checked: 1
            }
        );
        // The check is not tautological: a binding missing an arm violates.
        let log = log_with(&[(1, 10, A, OBJ)]);
        let vio = violation_of_verdict(verify(&log, &m).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Concurrency);
    }

    #[test]
    fn loop_court_conforming_and_violating() {
        let m = model(PowlConstruct::Loop, &[A, B]);
        // Conforming: do A, redo B, do A again — every redo closed.
        let ok = verify(&log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ), (3, 30, A, OBJ)]), &m)
            .unwrap()
            .unwrap();
        assert_eq!(ok.constructs_checked, 1);
        // Violating: redo B with no later do A — MUST reject.
        let log = log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ)]);
        let vio = violation_of_verdict(verify(&log, &m).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Loop);
        assert_eq!(vio.evidence[0], 2, "evidence is the dangling redo event id");
    }

    #[test]
    fn terminal_declared_open_binding_skips_loop_check() {
        // §8: a binding is open iff no declared terminal occurred. With a
        // terminal declared and absent, the dangling redo is NOT a
        // violation (the case is still open); with the same log under the
        // strict floor (no terminals), it IS.
        let m_open = PowlModel {
            root: PowlNode::Operator {
                construct: PowlConstruct::Loop,
                children: vec![PowlNode::Activity(A), PowlNode::Activity(B)],
            },
            terminals: vec![9],
        };
        let log = log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ)]);
        assert!(verify(&log, &m_open).unwrap().is_ok());
        // Strict floor: same log, no terminals -> enforced.
        let m_strict = model(PowlConstruct::Loop, &[A, B]);
        let vio = violation_of_verdict(verify(&log, &m_strict).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Loop);
        // Closed by terminal: terminal present -> dangling redo violates.
        let log_closed = log_with(&[
            (1, 10, A, OBJ),
            (2, 20, B, OBJ),
            (3, 30, A, OBJ),
            (4, 40, B, OBJ),
            (5, 50, A, OBJ),
            (6, 60, 9, OBJ),
        ]);
        assert!(verify(&log_closed, &m_open).unwrap().is_ok());
        let log_dangling_closed = log_with(&[
            (1, 10, A, OBJ),
            (2, 20, B, OBJ),
            (3, 30, A, OBJ),
            (4, 40, B, OBJ),
            (5, 50, 9, OBJ),
        ]);
        let vio2 = violation_of_verdict(verify(&log_dangling_closed, &m_open).unwrap());
        assert_eq!(vio2.construct, PowlConstruct::Loop);
    }

    #[test]
    fn check_path_is_allocation_free_beyond_the_binding_index() {
        // §5.1: "no allocation in the check path beyond the binding index."
        let log = log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ), (3, 30, A, OBJ)]);
        let m = model(PowlConstruct::Loop, &[A, B]);
        let compiled = compile(&m).unwrap();
        let index = index_bindings(&log);
        let (verdict, report) = with_alloc_tracking(|| check(&log, &index, &compiled));
        assert!(verdict.is_ok(), "conforming fixture must conform: {verdict:?}");
        assert!(
            report.is_zero(),
            "check path allocated beyond the binding index: {report:?}"
        );
        // Anti-vacuity of THIS court: the binding-index phase on the same
        // log does allocate — the tracker is measuring the real paths.
        let (_, idx_report) = with_alloc_tracking(|| index_bindings(&log));
        assert!(
            idx_report.count > 0,
            "binding index built with zero allocations; court would be vacuous: {idx_report:?}"
        );
    }

    #[test]
    fn malformed_log_yields_typed_error_not_panic() {
        // §5/§7.8: malformed logs surface as Err(PowlError::MalformedLog).
        let mut log = log_with(&[(1, 10, A, OBJ), (2, 20, B, OBJ)]);
        log.event_id[1] = 1; // duplicate event id — §4.1 monotonicity broken
        assert_eq!(verify(&log, &model(PowlConstruct::Sequence, &[A, B])).unwrap_err(), PowlError::MalformedLog);

        let mut log2 = log_with(&[(1, 10, A, OBJ)]);
        log2.activity[0] = 0; // activity code 0 is reserved (§4.3)
        assert_eq!(verify(&log2, &model(PowlConstruct::Sequence, &[A, B])).unwrap_err(), PowlError::MalformedLog);
    }

    #[test]
    fn malformed_model_yields_typed_invalid_model() {
        // Loop arity != 2.
        let m = model(PowlConstruct::Loop, &[A]);
        assert_eq!(verify(&log_with(&[]), &m).unwrap_err(), PowlError::InvalidModel);
        // Unary Sequence.
        let m2 = model(PowlConstruct::Sequence, &[A]);
        assert_eq!(verify(&log_with(&[]), &m2).unwrap_err(), PowlError::InvalidModel);
        // Arity above the bitmask ceiling.
        let many: Vec<u16> = (0..=POWL_MAX_ARITY as u16 + 1).map(|i| i + 1).collect();
        let m3 = model(PowlConstruct::Parallel, &many);
        assert_eq!(verify(&log_with(&[]), &m3).unwrap_err(), PowlError::InvalidModel);
    }

    #[test]
    fn empty_log_vacuously_conforms_with_zero_bindings() {
        let m = model(PowlConstruct::Sequence, &[A, B]);
        let log = Ocel2ColumnarLog::new();
        let ok = verify(&log, &m).unwrap().unwrap();
        assert_eq!(
            ok,
            PowlConformance {
                constructs_checked: 1,
                bindings_checked: 0
            }
        );
    }

    /// Wave-8 defect repair D3: the verdict is a frozen wire ABI whose
    /// evidence event ids are what a court receipt cites, so which violation
    /// surfaces when two bindings violate must be a pure function of the
    /// log bytes + model. Pre-repair, `check` iterated bindings in
    /// `HashMap` order — each fresh index carries a different per-instance
    /// `RandomState`, so the reported evidence id varied run-to-run. Law:
    /// bindings are checked in ascending first-event-row order; the
    /// reported violation is the first found under that order. Non-vacuity:
    /// 64 fresh index builds sample 64 independent hash orderings on
    /// pre-repair code (P(uniform) ~ 2^-63) — witnessed in the wave
    /// receipt, sandbox run SB-A15-M-D3.
    #[test]
    fn verdict_is_deterministic_across_repeated_index_builds() {
        let m = model(PowlConstruct::Sequence, &[A, B]);
        // Binding 100: rows [B, A] -> violation, evidence event id 1.
        // Binding 200: rows [B, A] -> violation, evidence event id 3.
        let log = log_with(&[(1, 10, B, 100), (2, 20, A, 100), (3, 30, B, 200), (4, 40, A, 200)]);
        let expected = PowlViolation {
            construct: PowlConstruct::Sequence,
            reserved: [0; 3],
            evidence_len: 1,
            evidence: [1, 0, 0, 0, 0, 0, 0, 0],
        };
        for _ in 0..64 {
            let v = violation_of_verdict(verify(&log, &m).unwrap());
            assert_eq!(v, expected, "verdict evidence must not vary per index build");
        }
        // Both violations are real and findable — the court is not hiding
        // one behind the other: with binding 100 removed, binding 200's
        // violation surfaces with its own evidence.
        let log2 = log_with(&[(3, 30, B, 200), (4, 40, A, 200)]);
        let v2 = violation_of_verdict(verify(&log2, &m).unwrap());
        assert_eq!(v2.evidence[0], 3);
    }

    /// Wave-8 defect repair D6: sibling children with OVERLAPPING
    /// leaf-activity sets (e.g. `Choice[A, A]`) made `ordinal_of` attribute
    /// every event to the first matching child, so the construct check
    /// could never fire — the model vacuously conformed to any log, a
    /// silent tautology. Repair: overlap refused typed (`InvalidModel`) at
    /// compile. Non-vacuity: on the wave-7 baseline this test FAILS (the
    /// overlapping models compiled and vacuously conformed — witnessed in
    /// the wave receipt, sandbox run SB-A15-M-D6).
    #[test]
    fn overlapping_sibling_leaf_sets_refused_typed() {
        for construct in [
            PowlConstruct::Choice,
            PowlConstruct::Sequence,
            PowlConstruct::Parallel,
            PowlConstruct::Concurrency,
            PowlConstruct::Loop,
        ] {
            let m = model(construct, &[A, A]);
            assert_eq!(
                verify(&log_with(&[]), &m).unwrap_err(),
                PowlError::InvalidModel,
                "{construct:?} with duplicated child must be refused"
            );
        }
        // Control: disjoint siblings still compile.
        assert!(compile(&model(PowlConstruct::Sequence, &[A, B])).is_ok());
        assert!(compile(&model(PowlConstruct::Loop, &[A, B])).is_ok());
        // Overlap through nesting: Choice[A,B] and Choice[A,C] share A, so
        // the parent Sequence's sibling leaf sets intersect.
        let nested = PowlModel {
            root: PowlNode::Operator {
                construct: PowlConstruct::Sequence,
                children: vec![
                    PowlNode::Operator {
                        construct: PowlConstruct::Choice,
                        children: vec![PowlNode::Activity(A), PowlNode::Activity(B)],
                    },
                    PowlNode::Operator {
                        construct: PowlConstruct::Choice,
                        children: vec![PowlNode::Activity(A), PowlNode::Activity(C)],
                    },
                ],
            },
            terminals: Vec::new(),
        };
        assert_eq!(
            verify(&log_with(&[]), &nested).unwrap_err(),
            PowlError::InvalidModel,
            "overlap through nested leaf sets must be refused"
        );
        // Disjoint nested control: Choice[A,B] and Choice[C, B]... overlap
        // on B — use disjoint sets {A,B} and {C} with legal arities.
        let disjoint = PowlModel {
            root: PowlNode::Operator {
                construct: PowlConstruct::Sequence,
                children: vec![
                    PowlNode::Operator {
                        construct: PowlConstruct::Choice,
                        children: vec![PowlNode::Activity(A), PowlNode::Activity(B)],
                    },
                    PowlNode::Activity(C),
                ],
            },
            terminals: Vec::new(),
        };
        assert!(
            compile(&disjoint).is_ok(),
            "disjoint nested leaf sets must still compile"
        );
    }

    /// Wave 9 — A20 stress at the POWL surface: `verify()` (the REAL public
    /// path: validate → compile → project → check) over a 1,000,000-event
    /// log built by the REAL append path. 1,000 bindings, each carrying 500
    /// (A,B) pairs in slab order — Sequence must conform over every one.
    /// Measured wall-clock is printed; the number is an observation (dev
    /// profile, single run, this host), never a spec claim.
    ///
    /// Flip / anti-vacuity: the same 1M rows with the FIRST event's
    /// activity flipped to B (B before A in binding 1000) MUST be rejected
    /// with `PowlConstruct::Sequence` and evidence event id 1 — a green
    /// 1M-row court that cannot see red is decoration. This court is also
    /// the D8 repair witness: pre-repair the projection was O(n^2) and this
    /// test did not complete (timing witness in the wave report).
    #[test]
    fn a20_stress_one_million_rows_powl_verify() {
        const ROWS: u64 = 1_000_000;
        const OBJECTS: u64 = 1_000;
        let mut log = Ocel2ColumnarLog::with_capacity(ROWS as usize, ROWS as usize, OBJECTS as usize);
        for i in 0..ROWS {
            let activity = if i % 2 == 0 { A } else { B };
            let object = OBJ + ((i / 2) % OBJECTS);
            log.append(&vec_for(i + 1, 1_700_000_000_000 + i, activity, object))
                .unwrap();
        }
        let model = model(PowlConstruct::Sequence, &[A, B]);

        let t = std::time::Instant::now();
        let verdict = verify(&log, &model).unwrap();
        let verify_s = t.elapsed().as_secs_f64();
        assert_eq!(
            verdict,
            Ok(PowlConformance {
                constructs_checked: 1,
                bindings_checked: OBJECTS as u32,
            })
        );
        println!(
            "a20-1m-stress(powl): rows=1000000 bindings=1000 verify={verify_s:.3}s \
             (dev profile, single run, this host — measured, not targeted)"
        );

        // Anti-vacuity: one flipped event at 1M-row scale is still seen.
        let mut bad = log;
        bad.activity[0] = B; // binding OBJ: first event is B, no preceding A
        let vio = violation_of_verdict(verify(&bad, &model).unwrap());
        assert_eq!(vio.construct, PowlConstruct::Sequence);
        assert_eq!(vio.evidence_len, 1);
        assert_eq!(vio.evidence[0], 1);
    }
}
