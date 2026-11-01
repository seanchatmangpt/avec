# A15: POWL v2 Transition Verifier + OCEL 2.0 Columnar Flight Recorder

Lane: A15 of 20 · Squadron 3 · milestone v26.11.1 ·
status: PROPOSED (this document is the plan; zero lines of `powl.rs` /
`ocel2_slab.rs` exist today — UNVERIFIED until courts pass).

Owner files (disjoint lease):

- `/Users/sac/cgap/crates/cgap-audit/src/powl.rs` — PROPOSED (new file)
- `/Users/sac/cgap/crates/cgap-audit/src/ocel2_slab.rs` — PROPOSED (new file)

Both modules extend the EXISTING crate at `crates/audit/` (published crate
name `cgap-audit`, see `crates/audit/Cargo.toml`); no new crate is minted.

Exit gate (from the lane contract): every admitted transition appends a
compressed OCEL 2.0 object-event vector directly to the Parquet audit slab.

---

## 1. Context & Problem Statement

### 1.1 What the audit crate already gives us (read from disk)

`crates/audit/src/lib.rs` is a cdylib whose `#[no_mangle] extern "C"`
exports wrap real kernels (`audit_mask_words`, `audit_mask_words_neon`,
`audit_count_valid`, `audit_get_bit`, ...) — the linker cannot elide an
exported symbol, so audits inspect the REAL compiled kernel. Two existing
surfaces anchor this lane:

- **CountingAlloc + with_alloc_tracking**
  (`crates/audit/src/alloc_counter.rs`): a `GlobalAlloc` wrapper
  (`static COUNTING: CountingAlloc`) plus
  `with_alloc_tracking(f) -> (T, AllocReport)` — a scoped guard that counts
  allocations a closure performs. This is the house pattern for
  "instrument, do not mock": real collaborators, observed side effects.
- **cdylib export discipline** (`lib.rs` header comment): every audited
  kernel is force-materialized through an export. The OCEL append path gets
  the same treatment (`audit_ocel2_append` export) so the append is real
  compiled behavior, not dead-code-eliminated.

### 1.2 The gap: process telemetry is prose today

cgap kernels are audited for allocation count and machine code, but nothing
records WHAT the kernels did as process history. When a court asks "did
transition T admit under authority A on object O?", the answer is currently
unretrievable — the event happened, nothing persisted it in a
machine-replayable form. OCEL 2.0 (Object-Centric Event Log, IEEE XES
successor, ocel-standard.org) is the mature process-mining interchange
format; POWL (Partially Ordered Workflow Language, van der Aa et al.) is its
companion model language for expressing partially-ordered workflow
constraints. Both are existing capital — prior art first: we encode OCEL 2.0
tables columnar and verify against POWL constructs rather than inventing a
bespoke event schema.

### 1.3 C02 lineage

Composition C02 ("factory that learns its own yield": planner cost model =
outcome rates mined from the OCEL of the manufacturing process itself) is
the downstream consumer. This lane manufactures the OCEL substrate C02
mines; it does NOT implement the planner cost model. Grounded boundary:
`ocel2_slab.rs` writes the log; C02 later reads it. Nothing in this lane
reads outcome rates.

---

## 2. ADR: why columnar OCEL inside the audit crate

### 2.1 Decision

OCEL 2.0 event and object tables are stored as fixed-width, binary-encoded
Arrow-style columns appended directly to the existing Parquet audit slab
(`cgap-parquet` `unpack` is already imported by `lib.rs`), and POWL v2
construct verification is expressed as pure constraint checks over those
columns.

### 2.2 Why columnar (vs. JSON/XES XML row logs)

- Boundary invariant 2 (zero strings per boundary): OCEL 2.0 XML/JSON
  interchange is string-laden. We keep the *interchange* out of the boundary:
  the at-rest form is binary columns (u64 event id, u32 activity code, u64
  object ids, u64 timestamp ns, u16 qualifier code). String interning to
  activity/object-type codes happens ONCE at admission, behind the boundary.
- Compression: Parquet-style columnar layout lets the existing compression
  court check "no silent row drop" per column, which is impossible to state
  sharply over a JSON log.
- Isomorphism discipline (A04 triumvirate lane): fixed-width rows keep the
  log a byte-addressable slab, same class as the rest of cgap's slabs.
- Flight-recorder semantics: appends are constant-cost, no deserialization
  of prior rows — recorder must never perturb the kernel it records.

### 2.3 Why inside cgap-audit and not cgap-parquet

cgap-audit already owns the "observe the real kernel" mandate
(CountingAlloc, cdylib exports). A flight recorder is an auditor, not a
storage engine: it observes admitted transitions and appends. The Parquet
slab is the shared substrate both consume; ownership of the append policy
belongs to the auditor.

### 2.4 POWL v2 constructs verified (definitions — normative for this lane)

POWL models a process as a partially ordered set of operators over
activities. We verify the log AGAINST the model: for each construct, a
constraint over the OCEL event columns. Normative semantics (from the POWL
model type semantics, van der Aa 2023):

- **Sequence(S = A→B)**: every occurrence of B with the same case/object
  binding is preceded by an occurrence of A. Check: for all e_B, exists
  e_A with e_A.ts < e_B.ts, same object set, no intervening re-entry
  violating order.
- **Choice(C = A⊗B)**: within one case, A and B are mutually exclusive.
  Check: no case binding contains both an A and a B event.
- **Parallel(P = A∥B)** (strict ordering partial, no interleaving
  constraint beyond partial order): A and B may occur in either order but
  both must occur in every complete case. Check: every complete case
  binding contains ≥1 A and ≥1 B.
- **Concurrency (A⇄B, unrestricted interleaving)**: no ordering constraint
  between A and B; check is vacuous-pass-guarded — the check asserts ONLY
  that both appear (if the model requires them), and the court proves the
  checker does NOT reject a log where B precedes A.
- **Loop(L = A∘B)**: after a redo (B), A must re-occur before case
  completion. Check: for every B event, a later A event exists in the same
  binding unless the case is still open.
- **Skip/silent transitions**: modeled as absent activities — the loop and
  sequence checks above must tolerate their absence without false failure.

A `PowlModel` is a tree of these operators; `verify(log, model) ->
Verdict` walks the tree and returns per-construct findings
(CONFORMANT / VIOLATED[construct, evidence event ids]).

### 2.5 Alternatives rejected

- **Flat event log + LTL checkers (Declare maps)**: mature, but string-keyed
  and case-centric; OCEL is object-centric and cgap is columnar — reshaping
  cost exceeds benefit.
- **Event sourcing into the existing git-merkle lane (A03)**: different
  cadence; OCEL needs high-frequency appends, Merkle needs content
  addressing. Composable later, not coupled now.

---

## 3. C4 Diagrams

### 3.1 C4 Level 2 — Container

```mermaid
c4container
    title Containers: POWL verifier + OCEL recorder inside cgap-audit
    System_Ext(kern, "cgap kernels", "cgap-arrow / cgap-parquet")
    Container_Boundary(audit, "cgap-audit (cdylib)") {
      Container(powl, "powl.rs", "Rust module",
        "POWL v2 construct verifier: verify(log, model) -> Verdict")
      Container(ocel, "ocel2_slab.rs", "Rust module",
        "OCEL 2.0 columnar append + Parquet slab writer")
    }
    Rel(kern, ocel, "admitted transitions appended as binary vectors")
    Rel(ocel, powl, "OcelLog columns")
    Rel(ocel, kern, "reads slab state")
```

### 3.2 C4 Level 1 — System context (C02 downstream)

```mermaid
c4container
    title System context: this lane feeds C02
    System(auditSys, "cgap-audit OCEL flight recorder", "admitted-transition log")
    System_Ext(c02, "C02 factory-learns-yield", "mines outcome rates from OCEL")
    Rel(auditSys, c02, "columnar OCEL log (read-only consumption, later lane)")
```

### 3.3 C4 Level 3 — Component (append path)

```mermaid
c4container
    title Component: the exit-gate append path
    Container_Boundary(ocel, "ocel2_slab.rs") {
      Component(intern, "intern_activity / intern_object_type",
        "u16/u32 codes; zero strings cross the boundary")
      Component(vec, "OcelEventVector", "fixed-width column buffers (see schema)")
      Component(comp, "compress_columns", "per-column compression, row-count pinned")
      Component(sl, "append_to_slab", "compressed vector -> Parquet audit slab")
    }
    Container_Boundary(powl, "powl.rs") {
      Component(verify, "verify", "constraint checks over OcelLog columns")
    }
    Rel(intern, vec, "codes")
    Rel(vec, comp, "columns")
    Rel(comp, sl, "compressed columns + pinned row count")
    Rel(sl, verify, "re-read for conformance court")
```

---

## 4. OCEL 2.0 Slab Schema (binary, zero strings at the boundary)

### 4.1 Event table (columnar, per append)

| column | type | encoding |
|---|---|---|
| `event_id` | u64 | monotonically increasing, slab-local |
| `activity` | u16 | interned code (intern table versioned in slab header) |
| `ts_ns` | u64 | UNIX epoch nanoseconds |
| `object_count` | u16 | length of the object ref list for this event |
| `object_ids` | u64 | flat column; segmented by object_count prefix sums |
| `qualifier` | u16 | interned qualifier code (0 = none) |
| `attr_blob` | u64 words | fixed-width attribute words (no strings; typed by intern table) |

### 4.2 Object table (columnar)

| column | type |
|---|---|
| `object_id` | u64 |
| `object_type` | u16 (interned) |
| `first_event_idx` | u32 |
| `last_event_idx` | POWL-check-relevant; u32 |

### 4.3 Qualifier encoding

OCEL 2.0 qualifiers (the per (event, object) relation qualifier — in OCEL
2.0 spec terms, `qualifier` on the event-to-object relations) are interned
u16 codes. The intern table (code → canonical string) lives in the slab
header, versioned; the boundary never carries a string — the interchange
exporter (out of scope, later lane) is the only string site, and it reads
the intern table.

### 4.4 Slab header

Fixed-width header records: magic `OCEL2SLAB`, schema version, intern-table
version, event row count (pinned before compression; compression court
compares against it), object row count, checksum (BLAKE3 per A03's
`cgap-blake3` crate — existing dependency, not new).

---

## 5. POWL verification semantics (constraint checks over columns)

`verify(log: &OcelLog, model: &PowlModel) -> PowlVerdict`:

1. Project event columns into per-object-binding sequences (grouping via
   the segmented `object_ids` column; no allocation in the check path
   beyond the binding index — verified by `with_alloc_tracking`).
2. Walk the POWL tree; each node evaluates its construct check from §2.4
   over binding sequences.
3. Return per-construct findings with evidence: `VIOLATED(Sequence,
   [e_41, e_88])` — evidence as event ids (u64), never strings.

Verdict is a `Result<PowlConformance, PowlViolation>` typed enum; no
`panic!` on malformed logs — typed `Err(PowlError::MalformedLog)`.

## 6. Chicago Test Plan (real collaborators, anti-vacuity courts)

All tests use REAL slab files on disk (tempdir) and REAL logs built by the
real append path — no mocks. The double-free exception is not invoked.

### 6.1 Courts + anti-vacuity mutations (one violating fixture per construct)

Each POWL construct has (a) a conforming log fixture and (b) a violating
fixture; the court MUST fail on the violating fixture. This is the
anti-vacuity proof: a court that passes a violating log is FAIL (VACUOUS).

| court | conforming fixture | violating fixture (court must REJECT) |
|---|---|---|
| Sequence | A then B, same binding | B before A (`seq-violating`) |
| Choice | A only | both A and B in one binding (`choice-violating`) |
| Parallel | A then B (both present) | case missing B entirely (`parallel-violating`) |
| Concurrency | B then A (allowed) | court must NOT reject; vacuity check asserts
  checker accepts B-then-A |
| Loop | A (B∘A redo closed) | B with no later A (`loop-violating`) |
| Compression | append 10 events, compressed column round-trips | court must
  fail if compressor silently drops a row: compresses 10, asserts 10 survive
  decompress (`compress-drop`) |
| OCEL append | append, re-read, values equal | mutate one column value
  post-append; court must fail (`ocel-mutate`) |
| Zero-string boundary | append runs under `with_alloc_tracking` | assert
  zero string allocations — any `String`/`format!` in the boundary flips the
  count (`string-leak`) |

### 6.2 Vacuity discipline

A court that cannot flip is FAIL. `compress-drop` is proven non-vacuous by
a hand-seeded drop mutation in the test harness (compress 10 → decompress 9
→ court MUST exit 1). Same pattern for `ocel-mutate`: seed one byte flip,
court must catch it, restore, re-run, must pass — real restore, real state
assert (shim-court discipline).

### 6.3 Exit-gate acceptance command

```
cargo test -p cgap-audit --features ocel2 -- --nocapture
```

Gate is met when all courts in §6.1 pass AND the violating fixtures are
rejected (checked by asserting `is_err()` — not by silence).

---

## 7. Definition of Done (8+)

1. `crates/cgap-audit/src/powl.rs` exists with typed POWL v2 model tree
   (Sequence/Choice/Parallel/Concurrency/Loop) and `verify()`.
2. `crates/cgap-audit/src/ocel2_slab.rs` exists with the §4 schema, intern
   tables, and `append_to_slab()` writing compressed columns to the Parquet
   slab.
3. Exit gate: an admitted transition produces a compressed OCEL 2.0 vector
   append — demonstrated by a test that appends, re-reads, and compares
   column-equal.
4. All six POWL construct courts pass on conforming fixtures and REJECT
   their violating fixtures (§6.1 table fully red).
5. `compress-drop` anti-vacuity: seeded row-drop mutation is caught by the
   compression court (exit 1), restore is byte-exact (sha256).
6. `ocel-mutate` anti-vacuity: seeded one-byte column mutation caught;
   restore byte-exact.
7. Zero strings cross the append boundary — proven by `with_alloc_tracking`
   (existing CountingAlloc, real allocator, no mock) with a string-class
   allocation assert.
8. `verify()` returns typed errors on malformed logs — zero `unwrap()`/
   `expect()`/`panic!` on the external-input path (grep-clean).
9. Cdylib export `audit_ocel2_append` exists and is exercised by a test
   through the real compiled symbol (not the Rust path only).
10. `cargo test -p cgap-audit` exit 0 with the ocel2 feature; no new
    dependencies beyond existing workspace crates (cgap-blake3 for the
    checksum; cgap-parquet for the slab).

---

## 8. Risks

- **OCEL 2.0 spec conformance scope**: this lane implements the *columnar
  at-rest form* of OCEL 2.0's event/object/qualifier model, NOT the XML/
  JSON interchange serializer. A conforming exporter is a follow-up lane;
  until it exists, "OCEL 2.0" here means the data model (events, objects,
  relations with qualifiers), not the wire format. Risk of overclaiming
  conformance is mitigated by stating this scope boundary explicitly and
  by gating the follow-up on the interchange exporter receipt.
- **Qualifier semantics drift**: OCEL 2.0 qualifiers are open-world strings
  in the spec; interning to u16 caps vocabulary at 65535. Mitigation:
  intern-table versioning in the slab header; overflow is a typed error,
  not a wraparound.
- **Timestamp monotonicity**: flight-recorder appends from concurrent
  kernels can reorder `ts_ns`. POWL sequence checks use slab order, not
  wall clock, for ordering within a binding; wall clock is advisory. This
  divergence from XES practice is documented, not hidden.
- **Loop "open case" ambiguity**: deciding whether a case is "still open"
  requires a notion of case completion OCEL does not carry natively.
  Mitigation: a binding is open iff no terminal event (model-declared) has
  occurred; the model must declare terminals or the loop check degrades to
  a warning, never a silent pass.

---

## 9. Wave 3

This lane lands in Wave 3: it depends on Wave 2 slab substrates (A01 arrow
slab, A02 Parquet mmap) being landed for the slab write path, and its
columnar log is consumed by later waves (C02 mining lane, A20 court-verdict
consumers). No Wave 1/2 file ownership conflicts: `crates/audit/src/` is
this lane's exclusive lease within the fan-out.

## See Also

`docs/jira/v26.11.1/lanes/A01-arrow-slab.md` (slab house style) ·
`docs/jira/v26.11.1/lanes/A02-parquet-mmap.md` (slab substrate) ·
`docs/jira/v26.11.1/lanes/A20-court-verdict.md` (downstream consumer) ·
`~/.claude/rules/testing-chicago-style.md` (court discipline) ·
`~/.claude/rules/no-overclaiming-rust.md` (status vocabulary)
