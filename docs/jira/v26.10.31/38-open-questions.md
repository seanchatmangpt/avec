# Open Questions & UNKNOWN Register: v26.10.31

Standing: UNVERIFIED as a register of the milestone's honest frontier. Every entry below
carries status UNKNOWN (stale 2026-10-31: entries are updated in place below — several now
ADMITTED/PARTIAL) — UNKNOWN ≠ ADMITTED per house standing vocabulary. An entry
becomes ADMITTED only via a receipt (command + exit) or a court decision, never prose.

Close condition (all entries): a receipt or court decision lands and is cited in the
entry's Status line; prose-only closure is refused.

Companions: [33-risk-register.md](33-risk-register.md),
[20-prd.md](20-prd.md), [21-ard.md](21-ard.md), [22-adr-register.md](22-adr-register.md).

## 1. Layout (23-64b-quad-spec.md)

- **OQ-01 · Layout** — What are the exact sentinel values for the bottom type and
  wildcard TermIds, and are `0x0000_0000`/`0xFFFF_FFFF` the admitted pair?
  Why it blocks: FR-02 and the entire no-bitmap law (ADR-01) hang on sentinels; interning
  must refuse to mint either value, and every pattern-engine rule references them.
  Candidate path: confirm 23-64b-quad-spec.md §1.1 (BOTTOM `0x0000_0000`,
  WILDCARD `0xFFFF_FFFF`) against `timestamps.rs:79` null-slot-zero precedent; run the
  R1 sentinel round-trip gate (31-verification-ladder.md) plus a mutation flip
  (flip one encode bit; suite must fail).
  Status: ADMITTED (2026-10-31). Sentinels landed in crates/avec-arrow/src/layout.rs;
  sentinel courts + mutation-kill tests executed — flipping one encode bit fails
  the suite. Pair confirmed: BOTTOM `0x0000_0000`, WILDCARD `0xFFFF_FFFF`.
  See Also: 23-64b-quad-spec.md §1.1 · 31-verification-ladder.md R1.

- **OQ-02 · Layout** — Is the endianness freeze (little-endian everywhere, big-endian
  targets byte-swap explicitly) final, or does the packfile plane's big-endian `.idx`/
  `.pack` integer ordering (25 §1/§2) force a mixed-endian contract?
  Why it blocks: a single wrong read on a BE target corrupts `t_ns` ordering and fan-out
  math silently; the two specs currently state opposite byte orders for different planes.
  Candidate path: one ADR in 22-adr-register.md declaring per-plane byte order (quad
  buffers LE per 23 §5; packfile integers BE per Git format law) and an R1/R2 property
  test decoding both planes on a byte-swapped fixture.
  Status: ADMITTED-DESIGN (2026-10-31, wave-11 lane 3). The contract is
  per-plane mixed by design: quad plane LE — pinned in code
  (crates/avec-arrow/src/layout.rs:150-182 `to_le_bytes`/`from_le_bytes`;
  tests :241-274 assert explicit LE byte sequences) and spec (23 §5);
  packfile plane BE by git format law (25 §1-2) and in code (idx_read.rs
  `from_be_bytes`, deflate.rs adler32 BE). LE side receipted:
  gate-arrow-tests-v3 (`cargo test -p avec-arrow --lib`, 93 passed, exit 0)
  plus live re-run this session (layout filter: 14 passed / 0 failed). BE
  side receipted via real-git acceptance courts (V7: 48/0 over real
  git-produced BE bytes). Decision recorded as ADR-18. Still open (named
  residual, not a design UNKNOWN): the byte-swapped-fixture cross-plane
  property test (R1/R2) does not exist on disk.
  See Also: 23-64b-quad-spec.md §5 · 25-git-packfile-integration-spec.md §1-2 ·
  22-adr-register.md ADR-18 · 73-slack-adjudication.md (same lane).

- **OQ-03 · Layout** — What is the reserved-bit policy for `policy_mask` top-4-bits
  (`0xF000_0000`): writers clear, readers ignore — is that the permanent rule, and who
  owns bit assignment?
  Why it blocks: the policy layer (21-ard.md) owns bit meaning; without a pinned owner and
  ignore-on-read law, a future writer setting a reserved bit silently changes read-path
  permission semantics across versions.
  Candidate path: promote 23 §5 forward-compat rule into a SHACL shape in
  19-ontology-ttl.md (writer-clears/reader-ignores as mechanical validation) and a round-
  trip property test asserting reserved bits survive write→read as zero.
  Status: UNKNOWN (evidence note 2026-10-31, wave-11 lane 3): the law is
  spec prose only — 23 §5 pins writers-clear/readers-ignore and 23 §1 pins
  bit-assignment ownership in the policy layer (21-ard). layout.rs does not
  enforce it: `policy_mask` is a plain u32 that round-trips verbatim, and the
  module's own round-trip fixture sets `0x8000_0001` (a reserved-bit value)
  and asserts byte-exact survival (layout.rs:204, :271). No SHACL reserved-bit
  shape exists in 19-ontology-ttl.md; no round-trip-as-zero property test
  exists. Prose-only closure refused — stays UNKNOWN per the register rule.
  See Also: 23-64b-quad-spec.md §5 · 19-ontology-ttl.md.

- **OQ-04 · Layout** — Is 65,536 objects per partition a hard cap or must ladder depth
  K = ⌈log₂(max partition size)⌉ be parameterized for skewed/adversarial OID
  distributions?
  Why it blocks: 25 §3 already flags this as open item L6-1: beyond 2^16 objects in one
  first-byte partition the 16-stage ladder under-addresses and lookups break silently;
  packs are currently REQUIRED to stay ≤ 2^16 objects until closed.
  Candidate path: either (a) derive and receipt the K-generation rule with a disassembly
  audit per K, or (b) admit the hard cap and enforce pack-splitting above it with a typed
  refusal; court: shuffled adversarial OID corpus over a >2^16 fixture.
  Status: ADMITTED, option (b) (2026-10-31): hard cap MAX_OBJECTS = 65,536 with
  typed `TooManyObjects` refusal; boundary courts run at exactly 65,536
  objects (cap accepted, 65,537 refused).
  See Also: 25-git-packfile-integration-spec.md §3 · 18-m1-branchless-algebra.md.

## 2. Packfile (25-git-packfile-integration-spec.md)

- **OQ-05 · Packfile** — How do avec-authored 64-byte-aligned chunks ride as git-
  verifiable blobs while preserving 64B alignment through git's byte streams?
  Why it blocks: 25 §2 permits only type-3 full blobs; zlib (RFC 1950) framing and git's
  object layer are byte-streams with no alignment contract, so the "mmap of the pack chunk
  == mmap of the matrix" invariant (21-ard I3) may be unenforceable through unmodified git
  inflate.
  Candidate path: (a) store chunks as stored (compression-level-0) non-delta blobs so the
  inflated bytes are the pack bytes verbatim, plus a receipt proving inflated-output
  64-alignment; or (b) accept re-alignment on read with a documented copy seam. Court:
  `git cat-file` the blob into a buffer and assert address%64==0 or quantify the copy.
  Status: UNKNOWN — 25 permits blob placement but never resolves alignment persistence.
  See Also: 25-git-packfile-integration-spec.md §2/§6 · 21-ard.md I3.

- **OQ-06 · Packfile** — What is the exact SHA-1/SHA-256 (git OIDs) ↔ BLAKE3 (avec
  receipts) dual-attestation mapping, and where does it live?
  Why it blocks: 25 §5's acceptance requires `b3sum chunk == manifest blake3` AND
  `git hash-object chunk == tree OID` to agree; the binding is asserted but its home
  (commit message? schema-descriptor blob? separate manifest blob?) is unpinned — a
  binding with no canonical location cannot be verified by a third party replaying the
  repo.
  Candidate path: pin the manifest-blob form (a schema/manifest blob referenced from the
  tree, keyed BLAKE3→OID pairs), then a cross-check court asserting both identities for
  every chunk on a generated repo; sha256 object-format extension is out of scope until
  this lands.
  Status: UNKNOWN.
  See Also: 25-git-packfile-integration-spec.md §2/§5 · 26-blake3-engine-spec.md.

## 3. Parquet (24-parquet-projection-spec.md)

- **OQ-07 · Parquet** — SIMD-BP128-class bit-packing vs stored-PLAIN for hot decode:
  which encoding is the admitted default?
  Why it blocks: 24 lists bit-packing as a candidate in "preference order" but PLAIN is
  the only encoding with a proven zero-decode path; a wrong default either burns bus
  bandwidth (PLAIN on compressible data) or adds a decode body the CTQ-03 floor (70 GiB/s
  LSL) cannot survive.
  Candidate path: paired Divan benches on the pinned M3 Max profile (29-benchmark-harness-
  plan.md) over compressible and incompressible fixtures; ratio fence vs memcpy baseline;
  the winner becomes an ADR row in 22-adr-register.md with the bench receipt.
  Status: UNKNOWN — candidate decision, no bench receipt.
  See Also: 24-parquet-projection-spec.md (Page Mapping) · 29-benchmark-harness-plan.md
  CTQ-03.

- **OQ-08 · Parquet** — What is the acceptable cold-open cost of Thrift footer parsing,
  and is parse-once-cache-forever sufficient for long-lived mmap holders?
  Why it blocks: 24 mandates footer parse exactly once at open with offsets cached; but
  cache invalidation on file replacement/growth, and the one-time cost on huge footers
  (many row groups), are unquantified — a cold-open stall violates the deterministic-
  latency NFR if unbounded.
  Candidate path: bench footer-parse latency vs row-group count on the pinned profile;
  pin a bounded cache-invalidation law (offset cache keyed on (file identity, size,
  mtime) or fail-closed on change); receipt in the ADR register.
  Status: UNKNOWN.
  See Also: 24-parquet-projection-spec.md (mmap Scan Architecture, File Layout).

- **OQ-09 · Parquet** — What is the row-group sizing law (rows_per_rowgroup) that
  satisfies the 256B block grid, the 16 KiB device-page multiple, and the <0.1% page-
  header overhead target simultaneously?
  Why it blocks: 24 states three constraints but no formula; a wrong choice silently
  breaks either the 4-quad transpose unit (23 §2 rule 3) or the throughput floor, and
  Hook 8 (alignment arithmetic test) cannot be written without the formula.
  Candidate path: derive LCM-based sizing formula (per-column span multiple of 256 B,
  row-group payload multiple of 16 KiB, page payload ≥ 1000× header), land as a constant
  or config law with Hook 8's alignment-arithmetic test as the receipt.
  Status: UNKNOWN.
  See Also: 24-parquet-projection-spec.md (File Layout, Falsification Hooks).

## 4. BLAKE3 (26-blake3-engine-spec.md, AVEC-111)

- **OQ-10 · BLAKE3** — Which backend is the conformance reference: GPU path (avec-gpu /
  Metal, AVEC-111) or CPU SIMD, for receipt authority?
  Why it blocks: 26 makes byte-identical outputs across backends an acceptance gate but
  names no authoritative backend; receipts (Chatman Equation `R_receipt`) computed on
  Metal must equal CPU-derived roots or the commit-OID identity is split-brained.
  Candidate path: declare CPU SIMD the reference (vector suite + b3sum subprocess court
  run on CPU), then a differential court proving GPU==CPU over a random corpus; Metal
  receipts admitted only after that receipt lands, recorded in the AVEC-111 ledger.
  Status: UNKNOWN — half landed (wave 9): the GPU_CROSSOVER_BYTES = 16 MiB
  dispatch hook exists (crates/avec-blake3/src/lib.rs:33,
  crates/avec-arrow/src/blake3.rs:29), but no GPU differential court
  (GPU==CPU over a corpus) has run; the reference-backend question is
  undecided.
  See Also: 26-blake3-engine-spec.md (Compliance, Conformance Surface) ·
  ../v26.10.29/AVEC-111-metal-gpu-and-blake3.md.

- **OQ-11 · BLAKE3** — Is dual-runtime (CPU SIMD vs Metal GPU) determinism witnessed,
  or only assumed from spec compliance?
  Why it blocks: 26's falsifier covers each backend against official vectors separately;
  no hook forces the two runtimes to agree on the same input corpus in the same run —
  exactly the gap a GPU rounding/ordering regression would slip through.
  Candidate path: add a differential falsification hook to 26's surface: hash one
  deterministic corpus on both backends in one test, byte-compare roots, mutation =
  perturb one GPU constant; wire into CI per 31's gates table.
  Status: UNKNOWN.
  See Also: 26-blake3-engine-spec.md (Falsification Hooks) · 31-verification-ladder.md
  (Gates Table).

- **OQ-12 · BLAKE3** — Does the 10 GiB/s/core LSL apply to the Metal GPU path, and how
  is "per core" defined for a GPU?
  Why it blocks: the LSL 8.5 gate (29 CTQ-06) is phrased per-core for CPU
  backends; GPU throughput is device-level and non-comparable, so the gate is either
  vacuous or needs a separate device-level fence for Metal.
  Candidate path: separate CTQ row for the GPU backend with a device-level fence measured
  on the pinned M3 Max profile; receipts labeled by profile per 29's hardware-pinning
  law.
  Status: UNKNOWN.
  See Also: 26-blake3-engine-spec.md · 29-benchmark-harness-plan.md CTQ-06.

## 5. BEAM (28-beam-nif-handoff.md)

- **OQ-13 · BEAM** — Who owns allocation across the NIF boundary for each of the four
  allocation forms (host binary wrap, mmap wrap, avec-allocated aligned wrap, and
  subresource views), and does the AVEC-105 turnstile gate all of them?
  Why it blocks: 28 defines destructor laws per form but the audit items are open: the
  current `crates/avec-beam` `lib.rs` registry is a slot-lease model, not the resource-
  binary alias model; until reconciled, a double-free or leak path across the boundary is
  structurally possible.
  candidate path: land the AVEC-105 turnstile audit receipt
  (`crates/avec-beam/src/resources.rs` AtomicU8 on both dtor edges), reconcile or
  document the two regimes, then acceptance test 3 (supervisor-kill, turnstile reaches
  0b00 exactly once) as the court.
  Status: PARTIAL (2026-10-31): receipt gate-beam-bridge-v2 ALIVE (alloc_spy
  delta {0,0}, dtor exactly-once 64, pointer identity) covers the
  turnstile-audit clause; the full four-form allocation-ownership
  reconciliation remains open.
  See Also: 28-beam-nif-handoff.md (Ownership, Prior Art) ·
  ../v26.10.29/AVEC-105-beam-arrow-dual-turnstile.md.

- **OQ-14 · BEAM** — What is the dirty-scheduler budget rule: which ops exceed the
  bounded work quantum, and is the quantum a constant, a measured bound, or per-kernel
  declared?
  Why it blocks: 28 requires "no NIF may hold a normal scheduler dirty" and says kernels
  must state their bound up front, but no quantum value or measurement protocol exists —
  registration-time refusal is unenforceable without one.
  Candidate path: pin a numeric work quantum (e.g. max instructions or ns budget), bench
  each of the five ops against it on the pinned profile, encode the per-op classification
  (normal / dirty-CPU / dirty-IO) in the NIF registration table with a test asserting the
  classification.
  Status: PARTIAL (2026-10-31, wave 9): the per-op scheduler classification is
  landed in the NIF registration table (crates/avec-beam/src/lib.rs:570-608) —
  avec_flush_to_parquet / avec_seal_to_packfile dirty-CPU
  (ERL_NIF_DIRTY_JOB_CPU_BOUND); avec_open / avec_scan / avec_receipt_query
  normal. Still open: the numeric quantum value and the classification-assert
  test.
  See Also: 28-beam-nif-handoff.md (Scheduling Discipline).

- **OQ-15 · BEAM** — Are resource-binary alignment guarantees (`enif_make_resource_binary`
  aliasing a 64B-aligned avec buffer) preserved on every OTP version targeted, or only
  OTP 28.3.1 where the extern was witnessed?
  Why it blocks: 28's witness receipt is OTP 28.3.1 only; alignment propagation is a
  constructor precondition per 28, but BEAM binary semantics across OTP versions (refc
  binary vs heap binary thresholds) could break alias/alignment on another version —
  silently demoting zero-copy to copy.
  Candidate path: version-matrix acceptance test 1 (pointer-identity assert) across every
  OTP version in the support matrix (real BEAM subprocess per version, Chicago-style);
  pin the supported set in 22-adr-register.md.
  Status: UNKNOWN.
  See Also: 28-beam-nif-handoff.md (Boundary Contract, Acceptance Tests) ·
  31-verification-ladder.md R3 nif-zero-copy gate.

## 6. Verification (31-verification-ladder.md, 29-benchmark-harness-plan.md)

- **OQ-16 · Verification** — Which SHACL engine is the conformance authority for the
  ontology/turnstile shapes?
  Why it blocks: 19-ontology-ttl.md says the artifact must be consumable by standard
  SHACL engines but validates nothing in-repo; 08/10 depend on SHACL pre-flight as a
  refusal mechanism — an unchosen engine means REFUSAL-01..14 trip behavior is
  unverified across engines.
  Candidate path: pick one engine as authority (pyshacl as offline court vs an in-repo
  Rust/WASM engine as runtime gate), differential-test both against the negative test
  suite; ADMIT via court receipt in 30-shacl-validation-suite.md.
  Status: ADMITTED — pySHACL (2026-10-31): `scripts/validate_ontology.sh` is the
  conformance court; receipts gate-shacl-v2 and gate-shacl-v3 ALIVE; the
  anti-vacuity mutant (SHACL-weakening perturbation) was witnessed rejected.
  See Also: 19-ontology-ttl.md (validation section) · 30-shacl-validation-suite.md ·
  08-pr-turnstile-llm-safety.md.

- **OQ-17 · Verification** — How does the disassembly audit gate (`just gate ladder-abi`,
  zero-`jcc` on hot path) integrate with existing CI, and which disassembler is the
  authority (objdump vs llvm-objdump)?
  Why it blocks: 29 names both tools without pinning one; 31's gates table requires a
  witnessed mutation flip per gate, and disassembler disagreement (instruction decode
  differences) could make the gate pass on one tool and fail on another — a vacuous or
  flaky gate.
  Candidate path: pin one disassembler + flags, wrap as `just gate ladder-abi` with
  shim-court non-vacuity (introduce one branch → must fail), wire into CI on the pinned
  toolchain; receipt with mutation witness stored alongside.
  Status: PARTIAL (2026-10-31): the gate exists with manifest-aware
  adjudication (gate-branchless-manifest.md ALIVE, negative control) and CI
  wiring per 54; the disassembler authority (objdump vs llvm-objdump) remains
  unpinned.
  See Also: 31-verification-ladder.md (Gates Table) · 29-benchmark-harness-plan.md
  CTQ-01 · 27-branchless-kernel-abi.md.

- **OQ-18 · Verification** — What hardware pinning suffices for bench reproducibility,
  and are non-M3-Max receipts admissible evidence for CTQ floors?
  Why it blocks: 29 pins Apple M3 Max and says other-hardware receipts "are not
  comparable" but does not say whether they are admissible at all — a CI runner on
  different silicon could admit or block a milestone wrongly either way.
  Candidate path: ADR in 22-adr-register.md: M3 Max receipts are the only
  milestone-admitting evidence; other profiles land labeled as advisory; CI either runs
  on pinned hardware or skips-loud (`[SKIP] <reason>` per 31's gates law).
  Status: UNKNOWN.
  See Also: 29-benchmark-harness-plan.md (CI Wiring) · 31-verification-ladder.md.

## 7. Protocol / Design (back-referenced docs)

- **OQ-19 · Protocol** — Does relay-attack resilience hold under NTP-class clock
  synchronization, or is PTP/NIC hardware timestamping a hard precondition?
  Why it blocks: 12's honesty note admits host-software clocks across machines have
  jitter larger than the flight-time budget; the RTT ceiling (REFUSAL-04) is
  unenforceable on NTP-synced hosts, so any multi-host deployment of the notary scheme is
  currently unprotectable.
  Candidate path: declare deployment-domain bounds (single host / NUMA domain = enforced;
  cross-network = PTP-hardware-timestamp precondition) as an ADR; validate with a
  measured `dt_proc` distribution court on the pinned profile before fixing the 2500 ns
  budget.
  Status: UNKNOWN (12 flags the limit; no deployment-domain decision exists).
  See Also: 12-distance-bounding-rtt.md (Honesty Note) · 13-time-crypto-prior-art.md.

- **OQ-20 · Protocol** — Blinded-WASM probe: stripped-kernel execution on emulated
  hardware vs native — does the cycle-trap actually exclude emulation on faster hosts?
  Why it blocks: 14's cycle trap assumes emulator overhead blows the RTT/`instruction_
  ticks` window; a host fast enough to emulate W with overhead inside the window defeats
  the trap, and no measurement of thin-WASM-emulator overhead vs the ~1.8 µs budget
  exists.
  Candidate path: measure thin-interpreter/JIT emulation overhead for a <32 KB kernel on
  the pinned profile vs native `instruction_ticks`; either witness the trap or add a
  hardware-evidence channel (HSM counters, per 12's channel 2) as a required second
  factor.
  Status: UNKNOWN.
  See Also: 14-blinded-wasm.md (Cycle Traps) · 12-distance-bounding-rtt.md.

- **OQ-21 · Protocol** — P28-SIL dispute-test: does the dispute-test mechanism (ASC 450
  recognition + covenant-not-to-sue regularization) survive a hostile accounting/legal
  challenge, or is it untested design prose?
  Why it blocks: 21-ard.md's P28-SIL section ($50M threshold, Irrevocable Covenant Not to
  Sue, SOX 302/404 via BLAKE3 receipts) is a legal/accounting posture with no adversarial
  court; "digest equality denies nothing" is an assertion, and a failed dispute-test
  invalidates the licensing governance layer the EU AI Act claims rest on.
  Candidate path: construct the adversarial dispute scenario set (covenant enforceability,
  receipt-as-evidence admissibility, ASC 450 recognition timing) and run it as a skeptic
  court with domain-expert adjudication; outcome recorded as ADR or registered risk in
  33-risk-register.md.
  Status: UNKNOWN.
  See Also: 21-ard.md (Regulatory & Licensing Governance) · 33-risk-register.md ·
  10-notary-escrow-powl.md (dispute resolution exits).

- **OQ-22 · Protocol** — stardf reconstitution ordering: what ships first via ggen, and
  in what order do the 5-link chain stages admit?
  Why it blocks: 01 defines the chain (contract graph → ggen metamodel → deterministic
  compilation → differential falsification → sunset authorization) but no stage ordering
  or admission criterion per stage is pinned; the differential-vector stage (4) requires
  the legacy purrdf harness to be runnable, which is itself unverified.
  Candidate path: pin a stage-admission DAG (which receipts gate which transitions),
  starting with `purrdf_contract.ttl` extraction as stage-1 receipt; the legacy-harness
  runnability check is the first falsifier; order recorded in the work order (composition
  C-family, primitive G).
  Status: UNKNOWN (01 is standing UNVERIFIED, nothing executed).
  See Also: 01-stardf-reconstitution.md · 19-ontology-ttl.md.

## Register Integrity

- 22 entries; as of 2026-10-31 (wave-11 lane 3): 3 ADMITTED (OQ-01, OQ-04,
  OQ-16), 1 ADMITTED-DESIGN (OQ-02 — design pinned in code+spec, ADR-18;
  swapped-fixture BE test residual open), 3 PARTIAL (OQ-13, OQ-14, OQ-17),
  15 still UNKNOWN (incl. OQ-03, evidence-noted this session) — each flipped
  entry cites its receipt or court above.
- ADMITTED-DESIGN is weaker than ADMITTED: the design decision is pinned and
  receipted on its implemented side; a named verification residual stays
  open. It is not a prose closure — see the OQ-02 status line.
- Remaining entries are UNKNOWN; no receipt or court decision exists for them.
- ADMITTED transition rule: cite receipt (command + exit) or court decision in the entry's
  Status line and flip Status to ADMITTED — never prose-only.
- Re-verify at use time: any count or status recalled from this register later is stale by
  default (receipt-cited counts re-read from the receipt).
