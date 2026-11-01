# lanes-ctq-coverage-map

Lane C15 (charter conversion) coverage matrix: lanes A01-A20 x CTQ ctq-01..ctq-06.
Repo: /Users/sac/cgap (single canonical checkout, no worktrees). Date: 2026-11-01.

Sources (read this session):

- docs/jira/v26.11.1/dflss/00-dflss-charter-source.md section 3 (VOC -> CTQ
  flowdown table), section 4 Phase 3 (DOE-1..DOE-4), Phase 4 (AM->FM diode,
  `strings_admitted: 0` per receipt).
- All twenty lane specs docs/jira/v26.11.1/lanes/A01-arrow-slab.md through
  A20-court-verdict.md (ls exit 0; every mapped file was read in full).
- Alignment-only, not depended on: dflss/ctq-registry.yaml (concurrent) and
  dflss/receipt-schema-v2.schema.json (the receipt field names used in
  section 3). CTQ ids here come from the charter table, not the YAML.

Standing: this map is ALIVE as a written artifact (grounded in the reads
above). Every mapped lane SUBJECT is PROPOSED/UNVERIFIED per its own lane doc;
mapping a lane to a CTQ claims service, never measurement. All CTQ targets
remain TARGET until the Divan membrane measures them (charter Phase 2).

Legend (matrix cells):

- N = lane named for that CTQ row in charter section 3.
- S = spec-declared: the lane doc itself asserts the CTQ's metric class;
  section cited in the notes after the receipt table.
- I = lane owns the CTQ row's named measurement instrument.
- - = no service claimed (neither charter-named nor spec-declared).

## 1. CTQ index (charter section 3, ids ctq-01..ctq-06)

Cells are abbreviated for the 100-char line law; ids and lane names are the
normative keys (charter section 3). zsg = scripts/zero_string_gate.py.

| id | VOC | metric (nominal) | named lanes | instrument |
| --- | --- | --- | --- | --- |
| ctq-01 | Ingress security | ~4 ns/frame; flat p99 | A04, A14 | Divan admit_frame |
| ctq-02 | GC immunity | 0 B hot path | A01, A02 | Divan alloc + nm |
| ctq-03 | Line-speed reasoning | >1 GB/s; sub-us unify | A06, A11-A13 | Divan N=[64,1M] |
| ctq-04 | Tamper-evident govn | sub-us 640-bit verify | A17, A19 | Divan + 640-bit court |
| ctq-05 | Bare-metal portability | <64K flash; <32K SRAM | (none) | llvm-size + zsg |
| ctq-06 | Mfg efficiency | machinery share trend | (none; C10/C11) | proj_cov + journal |

## 2. Lane x CTQ matrix

| lane | subject files (per lane spec) | 01 | 02 | 03 | 04 | 05 | 06 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A01 | cgap-arrow: slab.rs, bitmask.rs | - | N | - | - | S | - |
| A02 | cgap-parquet: mmap_writer.rs, footer.rs | - | N | - | - | - | - |
| A03 | cgap-git: dag.rs, pack.rs | - | - | - | - | - | - |
| A04 | cgap-core: dma_ring.rs, isomorphism.rs, frame.rs | N | S | - | S | S | - |
| A05 | cgap-storage: atomic_commit.rs | - | - | - | - | - | - |
| A06 | cgap-bont: primitives.rs, oabi.rs | - | - | N | - | S | - |
| A07 | cgap-bosym crate + scripts/zero_string_gate.py | - | - | - | - | I | - |
| A08 | cgap-bont: shacl_compiler.rs | - | - | S | - | - | - |
| A09 | ontologies/fibo: lib.rs, fibo_sec_dbt_v1.oabi | - | - | S | - | S | - |
| A10 | ontologies/cloud: lib.rs, tosca_cimi_v1.oabi | - | - | S | - | - | - |
| A11 | cgap-bn3: arena.rs, sigil.rs | - | - | N | - | - | - |
| A12 | cgap-bn3: unify.rs, project.rs | - | S | N | - | - | - |
| A13 | cgap-bn3: builtins.rs | - | - | N | - | - | - |
| A14 | cgap-gate: penstock.rs | N | S | - | S | - | - |
| A15 | crates/audit (cgap-audit): powl.rs, ocel2_slab.rs | - | S | - | - | S | - |
| A16 | cgap-crypto: chatman.rs, lease.rs | - | - | - | - | - | - |
| A17 | cgap-p28: attestation.rs, trap.rs | - | - | - | N | - | - |
| A18 | cgap-p28: asc450.rs | - | S | - | S | - | - |
| A19 | cgap-ffi: lib.rs, include/cgap.h, xip.ld | - | S | - | N | S | - |
| A20 | tests/court_verdict.rs, scripts/boundary_gate.py | - | - | - | - | - | - |

Rows with no N/S/I cell in any CTQ column: A03, A05, A16, A20 — coverage
holes, flagged loudly in section 5. ctq-06 has zero serving lanes in A01-A20.

## 3. Receipt-fields column (schema v2: executor, strings_admitted, ctq_refs, doe_refs)

All four fields are REQUIRED on every receipt (dflss/receipt-schema-v2.schema.json
`required` list). The per-lane column below is therefore the VALUE each lane's
receipt must emit. `strings_admitted` is 0 for every lane (AM->FM diode, charter
Phase 4); the schema admits an honest nonzero record and the zero-string gate
refuses it — this map never pre-empties that falsifier.

| lane | executor | strings_admitted | ctq_refs | doe_refs |
| --- | --- | --- | --- | --- |
| A01 | agent | 0 | ctq-02, ctq-05 | DOE-1 |
| A02 | agent | 0 | ctq-02 | DOE-2 |
| A03 | agent | 0 | (empty - loud, hole H-2) | (empty) |
| A04 | agent | 0 | ctq-01, ctq-02, ctq-04, ctq-05 | DOE-1 |
| A05 | agent | 0 | (empty - loud, hole H-3) | (empty) |
| A06 | agent | 0 | ctq-03, ctq-05 | (empty) |
| A07 | gate + generator | 0 | ctq-05 | (empty) |
| A08 | agent | 0 | ctq-03 | (empty) |
| A09 | generator | 0 | ctq-03, ctq-05 | (empty) |
| A10 | generator | 0 | ctq-03 | (empty) |
| A11 | agent | 0 | ctq-03 | DOE-4 |
| A12 | agent | 0 | ctq-02, ctq-03 | DOE-3, DOE-4 |
| A13 | agent | 0 | ctq-03 | (empty) |
| A14 | agent | 0 | ctq-01, ctq-02, ctq-04 | DOE-3 |
| A15 | agent | 0 | ctq-02, ctq-05 | (empty) |
| A16 | agent | 0 | (empty - loud, hole H-5) | (empty) |
| A17 | agent | 0 | ctq-04 | (empty) |
| A18 | agent | 0 | ctq-02, ctq-04 | (empty) |
| A19 | agent | 0 | ctq-04, ctq-05 | DOE-1 |
| A20 | court + gate | 0 | (empty - loud, hole H-6) | (empty) |

Executor classes follow the schema enum {agent, gate, generator, court}: A07
(deliverable is the admission gate + build-time generator), A09/A10 (pack
harvester/emitter is the transform), A20 (deliverable is the court + mutation
gate). All other lanes are implementation lanes: agent.

### Spec-declared (S) and instrument-owner (I) citations

- A01 -> ctq-05: riscv32imc ELF gate C6, symbol audit, footprint item
  (A01 sections 2.5, 7 Wave 3, DoD 7).
- A04 -> ctq-02: zero-alloc source-scan gate (A04 section 8.3, DoD 5);
  ctq-04: 80-byte attestation carriage + corruption rows (A04 sections 6.2,
  8.3); ctq-05: riscv32imc clean-build DoD 8.
- A06 -> ctq-05: zero-runtime-strings B-ONT position (A06 sections 1, 3.1).
- A07 -> ctq-05 (I): owns zero_string_gate.py, the ctq-05 named instrument,
  with `--self-check` non-vacuity (A07 gate spec).
- A08 -> ctq-03: register-resident O(1) ontological rule evaluation (A08
  section 6).
- A09 -> ctq-03: u64 slot-compare lookup, not IRI parse (A09 section 1);
  ctq-05: zero runtime strings (A09 section 1).
- A10 -> ctq-03: shared slot vocabulary resolved via libcgap lookups (A10
  sections 1, 5).
- A12 -> ctq-02: zero-allocation hot path under counting allocator (A12
  section 7.4).
- A14 -> ctq-02: 0-alloc dam court over CountingAlloc (A14 section 8.2);
  ctq-04: P28 chain-presence reject rows (A14 section 5 rows 4-5).
- A15 -> ctq-02: no-alloc check path under with_alloc_tracking (A15 section
  5.1); ctq-05: zero-string boundary court (A15 sections 2.2, 6.1).
- A18 -> ctq-02: no allocation in the emitter path (A18 section 4.3);
  ctq-04: liability-metering half of the ctq-04 VOC (A18 sections 1-3).
- A19 -> ctq-02: callee performs no allocation in admit/eval/verify (A19
  ownership-rules table); ctq-05: exit gate <64 KB flash / <32 KB SRAM is
  exactly the ctq-05 target pair (A19 sections 4-5).

## 4. DOE participation (charter section 4, Phase 3)

| doe | charter experiment (short) | doe_refs lanes | primary subject |
| --- | --- | --- | --- |
| DOE-1 | AtomicI64 host vs AtomicU32-pair on RV32 | A01, A04, A19 | cgap-arrow atomics (H-4) |
| DOE-2 | RLE decode direct vs staged, widths 1-32 | A02 | cgap-parquet rle/page |
| DOE-3 | penstock branch-free vs early-exit | A14 | cgap-gate penstock |
| DOE-4 | u64-lane vs bitset, arena sweep | A11, A12 | cgap-bn3 arena/unifier |

DOE-3 note: A12's BM0 court (branch-free unifier vs branchy baseline ratio
fence, A12 section 6) reuses the DOE-3 method on the unifier; the charter
names penstock (A14) for DOE-3, so A12 carries DOE-3 as spec-declared method
reuse, disclosed here rather than silently folded in.

## 5. COVERAGE HOLES (loud — a lane serving no CTQ or a CTQ served by no lane)

H-1. ctq-06 is UNSERVED by every lane A01-A20. Charter section 3 names only
compositions C10/C11 (retirement compounding; one admission kernel) and the
instruments projection_coverage.py + workflow journal hop ratio; no lane lease
covers them. Any claim of full CTQ coverage over the twenty lanes is false
until a lane or conversion node owns ctq-06's instruments.

H-2. A03 (git-merkle) serves no CTQ row. Its tamper-evidence is block lineage
(Merkle root in A04 frames), while ctq-04's metric is 640-bit P28 attestation
verify + FFI delta and its witness names A17/A19 only. A03 receipts must emit
ctq_refs: [] until the charter grows a row or A03 is retargeted.

H-3. A05 (power-cut WAL) serves no CTQ row: crash-atomicity appears in no CTQ
metric. The spec deploys on embedded flash media, but it claims no footprint
or RAM measurement, so it does not qualify as ctq-05 service.

H-4. DOE-1's primary subjects have no lane lease. The charter Phase-1 32-bit
blockers live in crates/cgap-arrow/src/chase_lev.rs (64-bit atomics in the
work-stealing queue) and crates/cgap-arrow/src/c_abi_nested.rs (pointer-swap
in nested C-ABI); A01's lease is slab.rs + bitmask.rs only. The DOE-1
experiment is ownerless at file level; A01/A04/A19 carry DOE-1 as adjacent
stakeholders, not subject owners.

H-5. A16 (chatman sealer) serves no CTQ metric. PQC receipt authenticity is
adjacent to ctq-04's Tamper-Evident Governance VOC, but the row's metric and
witness (640-bit P28 verify, FFI delta) name A17/A19 only. A16 is a dependency
of A18's ctq-04 service (sealer + tick lease); recorded as adjacency, not
service.

H-6. A20 (court verdict) serves no CTQ row. It is the campaign's cross-cutting
witness — its exit gates are the mutation kill rate and the boundary gates —
i.e. a witness OF the CTQ courts, not a CTQ metric itself. Flagged so the gap
stays a decision, not an accident.

H-7. ctq-05 names no lane in charter section 3 (registry lanes: []), yet four
lanes serve it (A01 S, A04 S, A07 I, A19 S). The charter's named-lanes column
understates real coverage; this map is the record of that delta.

## 6. Method notes and divergences

- A15 self-contradiction: its owner-files line says
  crates/cgap-audit/src/powl.rs, while its own context places the crate at
  crates/audit/ (published crate name cgap-audit). The real directory is
  crates/audit/ (ls exit 0 this session); the matrix uses the real path.
- A14 internal path drift: header says crate cgap-gate; DoD 1 says
  crates/gate/. Matrix uses the header form: cgap-gate/src/penstock.rs.
- ctq-registry.yaml was being written concurrently; per dispatch this map
  references ctq ids from the charter table only and does not depend on the
  YAML. The registry's per-row `lanes` fields (charter-named only) match this
  map's N markers; S/I rows here extend, not contradict, the registry.
- A17 notes a URI divergence vs repo frozen pins: A17 section 3 cites
  `urn:cgap:license:...` inside A04's digest comment while the repo-frozen
  license URN is `urn:avec:license:p28-sil:v2.1.0-sovereign` (A17 cites the
  avec URN as canonical for LICENSE-P28-SIL.md). Carried here because the
  ctq-04 metric hashes that instrument.
- Standing of mapped subjects: every lane doc reviewed self-reports
  PROPOSED/UNVERIFIED (A08 PARTIAL_ALIVE design-only); no CTQ metric is
  MEASURED. This map assigns service, not capability.
