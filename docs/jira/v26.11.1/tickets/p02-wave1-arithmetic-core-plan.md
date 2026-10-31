# p02-wave1-arithmetic-core-plan

Wave-1 arithmetic-core execution plan for the v26.11.1 PQC milestone: the
lattice-arithmetic and hash kernel base that the ML-KEM and ML-DSA waves
both consume. Agent lane of 20; single-file lane; this file is the only
artifact; no git operations performed.

## 0. Provenance, Slot Reconciliation, Standing

- Written 2026-10-31 into the dispatched slot
  `docs/jira/v26.11.1/tickets/p02-wave1-arithmetic-core-plan.md`. This lane
  writes ONLY this file, into `docs/jira/v26.11.1/` only.
- **Slot reconciliation (integration defect for the index owner).** The
  milestone index (`../00-index.md`, agent-1-owned) maps `p02` to
  `tickets/p02-wave1-plan.md`, hook "Wave 1 (ML-KEM) execution plan". This
  dispatch names `p02-wave1-arithmetic-core-plan.md` and scopes wave-1 to
  the arithmetic core, under the same dispatch-local wave numbering p03
  records: wave-1 = arithmetic core, wave-2 = ML-KEM KEM layer, wave-4 =
  dk pool. The two readings cannot both be the index row. Per the index's
  own law ("the index is amended, never bypassed"), the index owner amends
  the p02 row to the dispatched filename and numbering. Until that
  amendment, this file is the authoritative p02 content per dispatch.
  Observed pattern, recorded for the same amendment pass: every landed
  phase-0/1 ticket diverges from its index-contract filename (t01 landed
  as `t01-ml-kem-params.md` vs contract `t01-fips-203-ml-kem-spec.md`;
  t04, t07, t09, t13, t14 likewise), so the reconciliation is one
  index-table edit, not per-ticket argument.
- **Inputs consumed (read from disk this session).**
  - t01 LANDED (`t01-ml-kem-params.md`): parameter ground truth, the
    corrections ledger, reduction constants, Quad64 cache-line arithmetic,
    and its section-7 open items addressed to this lane (disposition in
    1.5). Consumed.
  - t02 NOT LANDED: no `t02-*` file exists under `tickets/` as of this
    write. This plan is STANDALONE with respect to t02; t02's subject
    (FIPS 204, charter-W2) is a downstream consumer of this core, not an
    input to it, so nothing here blocks on it. If t02 lands with
    constants that contradict rows below, this file is amended by its
    owner.
  - p01 LANDED (`p01-go-no-go-adjudication.md`): ML-KEM GO, ML-DSA GO,
    SLH-DSA/FN-DSA/HQC DEFER; Keccak core + RNG-injection policy named as
    wave-1 prerequisites; CT-KEM-1 proposed with this lane as CTQ owner;
    "which sets ship, in what order" assigned here. Consumed in 1.5/6.
  - p03 LANDED (`p03-wave2-mlkem-plan.md`): its section-1.1 wave-1
    dependency contract is the consumer-side ABI this plan must satisfy;
    its manifest and seam rules are followed. Consumed in 1.4/2/3.
  - p06 LANDED (`p06-wave5-baselines-plan.md`): W5-F no-gap law, the
    fuzz-court dependency on this wave's KAT harness, CTQ id families,
    receipt-minter override. Consumed in 2/3/4/6.
- **Standing.** UNVERIFIED end to end. This ticket receipts a plan, not
  an engineering result: no kernel exists, no KAT has run, no command has
  been executed by this lane except the in-session hand-arithmetic
  re-derivations shown inline in section 1.3. Per index discipline 4,
  every parameter number below enters as a KAT-validation input and
  becomes a fact only when a KAT byte-parity receipt says so.
- License: every file this plan lands carries the P28-SIL v2.1.0 URN
  (`urn:avec:license:p28-sil:v2.1.0-sovereign`), no SPDX identifiers.

## 1. Scope

### 1.1 Deliverable: the arithmetic core

One new workspace member, `crates/avec-pqc/`, zero external dependencies,
containing the layer below the KEM ABI. Deliverables, in dependency
order:

1. **Polynomial arena.** Dense fixed-shape coefficient storage:
   `[i16; 256]`-class records in a 16 KiB-aligned slab. Fixed capacity,
   no growth path, no per-coefficient indirection. The arena allocates
   once at construction and Drop-zeroizes its whole slab (one erasure
   point per p01's hand-rolled-zeroize policy; see 1.3 for sizing).
2. **Barrett/Montgomery branchless reduction.** Both classes, sited (1.3,
   decision D-2): Montgomery-class reduction (`R = 2^16`, constant
   `QINV = 62209`, t01 section 3, exercised not assumed) on multiply-side
   accumulators; Barrett-class reduction (constant `v = 20159`) on
   add/sub-class canonicalization. Every conditional subtract, conditional
   negate, and canonical-domain clamp is realized with `ct_select`
   arithmetic (mask/cmov class) — zero data-dependent branches on data
   paths (ADR-04/ADR-14).
3. **NTT butterflies.** Forward NTT and inverse NTT over `q = 3329`,
   `n = 256`, in place, fixed trip counts: 7 stages x 128 butterflies =
   896 butterflies per transform (arithmetic: `log2(256) = 7`; 128
   butterflies per stage). Twiddles are `const` tables, never computed at
   runtime (see C2 for how the tables are policed).
4. **Basemul.** `MultiplyNTTs` (FIPS 203 Alg 11; BaseCaseMultiply, Alg 12,
   inside it) over NTT-domain operands — the pointwise product kernel the
   matrix/vector multiplies reduce to.
5. **du/dv compression bit-packing.** `Compress_d`/`Decompress_d` for the
   ciphertext depths (`du` in {10, 11}, `dv` in {4, 5} — t01 [T2]) plus
   the `ByteEncode_d`/`ByteDecode_d` bit-packing family for `d = 1..12`
   (d = 12 covers ek components, 384 B per poly; d = 1 the message bits).
   Exact-size wire records with explicit half-line tails is the endorsed
   default (t01 section 5); the wire-serialization decision itself is
   wave-2's (1.5, D-1).
6. **SHAKE-128/256 XOF — the one genuinely new primitive class.** A
   hand-rolled FIPS 202 Keccak-f[1600] permutation with sponge modes:
   SHAKE-128 (the XOF of SampleNTT), SHAKE-256 (PRF_eta and J), and — on
   the same permutation — SHA3-256 and SHA3-512 (H and G). p01 names the
   Keccak core a wave-1 prerequisite and orders it ahead of any ML-KEM
   kernel work; section 6 sequences it first.

### 1.2 Why SHAKE, and why BLAKE3 cannot substitute

- Conformance is byte-parity at KeyGen/Encaps/Decaps I/O (t01 section 6).
  Every derived byte transits the FIPS 203 section-4 suite: XOF =
  SHAKE-128, PRF/J = SHAKE-256, H = SHA3-256, G = SHA3-512. Substituting
  BLAKE3 anywhere in that chain does not produce a faster ML-KEM; it
  produces different bytes, i.e. a non-conforming scheme. BLAKE3 stays
  where it is: the substrate's content-address primitive (ADR-06/ADR-10).
- This is the only deliverable with no existing in-repo precedent class:
  the repo has a sovereign BLAKE3 core (ADR-06 pattern) but no Keccak.
  Wave-1 hand-rolls it the same way avec-blake3 was hand-rolled — spec
  compliance against external KAT vectors, never self-attestation.
- M=1 fit is by construction: Keccak-f[1600] is a fixed straight-line
  round function over public step constants (p01 evidence chain), so its
  data path admits zero data-dependent branches; only sponge trip-count
  scaffolding enters the remainder classes.

### 1.3 Quad64 geometry lessons applied

Working formats are line-exact, unlike the half-line-tailed wire objects
(t01 section 5):

- Dense u16 poly = 256 x 2 B = 512 B = 8 cache lines; dense u32 poly =
  1024 B = 16 lines; ByteEncode12 output = 384 B = 6 lines. Every working
  record is a whole number of cache lines — no split-line records, no
  implicit padding, the Quad64 laws (23-64b-quad-spec.md section 2) at
  record granularity.
- Slab: 16 KiB MMU-page-granule alignment, the `MmuBuf` pattern
  (`crates/avec-arrow/src/mmu_arena.rs` — 16,384 B = 256 x 64 B lines).
  One slab holds 32 dense-u16 polys. The k = 4 K-PKE workspace —
  A (k x k = 16 polys) + s, e (2k = 8) — is 24 polys = 12 KiB: one page,
  zero mid-wave allocation. (All four numbers in this paragraph are
  arithmetic, re-derived in-session; the workspace fit is a design
  consequence, UNVERIFIED until the arena exists.)
- **Dependency-zero interaction:** the lessons transfer as lessons, not
  as code. `avec-pqc` takes NO workspace path dependency on avec-arrow
  (or anything else — section 5); the arena is re-implemented in-crate.
  This is deliberate duplication in service of the sovereignty
  discipline; the drift guard is the court set (C6 zero-alloc, alignment
  asserts), not an import.
- **Secret-surface rule:** arena buffers hold secret coefficients
  (sampled s/e, decrypted message-domain polys). The slab's Drop runs
  hand-rolled zeroization (volatile writes + compiler fence over the
  full capacity — not the `zeroize` crate, per p01). Erasure points
  named per p01's requirement: exactly one, the arena Drop; the KEM wave
  adds key-tuple-level zeroize above it.

### 1.4 Reconciliation with p03's wave-1 dependency contract

p03 section 1.1 names the surface wave-2 consumes. Mapping:

| p03 contract item | This plan | Status |
|---|---|---|
| NTT / INTT, q=3329, n=256, in place, fixed trips | 1.1.3, C1/C2 | core, owned here |
| Montgomery/Barrett reduction, canonical rounding | 1.1.2, C1/C3 | core, owned here |
| Polynomial add / sub / multiply | 1.1.4 + `poly.rs` ops | core, owned here |
| ByteEncode/ByteDecode d=1..12 | 1.1.5 | core, owned here |
| du/dv Compress/Decompress | 1.1.5, C7 | core, owned here |
| H, G, J via SHA3-256/512 + SHAKE-128/256 | 1.1.6, C4 | core, owned here |
| Matrix sampling from rho | 1.5, D-3 | adjacency tier |
| CBD / PRF sampling | 1.5, D-3 | adjacency tier |

Adjacency tier: planned in-wave if the core lands early, not gating for
wave-1 done; the p03 fallback (wave-2 implements the missing piece as an
owned, loudly-disclosed module) stays armed per p03 section 1.1. The
dispatch for this lane names items 1-6; the two adjacency rows exist
because p03 consumes them from wave-1 and someone must say which lane
moves first.

### 1.5 Decisions pinned (t01 section-7 items, p01 owner assignments)

- **D-1 (t01 item 2, arena/wire layout).** Working polys: line-exact
  dense, 16 KiB slab (1.3). Wire records: exact-size with explicit
  half-line tails is the endorsed default; the final call is wave-2's
  because it is a serialization concern. The `inline_digest` slot-reuse
  question for ss (t01 item 3) is likewise wave-2's.
- **D-2 (t01 item 5, reduction strategy).** Both classes, sited:
  Montgomery on multiply-side accumulators (basemul products, butterfly
  twiddle multiplies) where lazy-reduction headroom pays; Barrett on
  add/sub-class canonicalization and the compress-path rounding. i16
  lanes carry Zq values (4 spare bits, t01 section 3); i32/u32 lanes
  carry pre-reduction accumulators. Constants `QINV = 62209`,
  `v = 20159` are UNVERIFIED-KAT until C1/C3 exercise them; both carry
  in-session hand checks: `3329 x 62209 = 207093761 = 3161 x 65536 + 1`
  (so 62209 = q^-1 mod 2^16) and `(2^26 + q/2) div q = 20159`.
- **D-3 (t01 item 4, SampleNTT loop policy).** Not decided here; the
  M=1 mask-compaction interaction is t06/t12 scope (t01's own routing).
  Wave-1 delivers the fixed-trip-count arithmetic underneath; the
  acceptance/rejection policy rides the adjacency tier.
- **D-4 (p01, "which sets ship, in what order").** Dissolves at this
  layer: every parameter set shares n = 256, q = 3329 and this exact
  kernel family; only k/eta/du/dv differ, and they live above the core.
  The core ships all three sets simultaneously by construction. Per-set
  API shipping order is re-delegated to the ML-KEM execution lane.
- **D-5 (p01, CT-KEM-1 ownership).** Recorded as accepted-proposal by
  this lane as CTQ owner. Realization is the FO-select work p03 sections
  1.2/3.3/3.5 already carry; wave-1's contribution is the substrate:
  the ct-select mask primitives in `reduce.rs` that the decaps-level
  select composes.
- **D-6 (t01 items 1/6/7/8/9).** Item 1 (t01 rename) and the p02 row:
  index owner. Item 6 (eta1(1024) re-verification): t07 acquisition.
  Items 7/8 (Encaps modulus-check negative court; KeyGen seed policy):
  wave-2. Item 9 ([T1] failure rates, [T4] loop-limit law as court
  inputs): recorded as negative-court inputs for the KEM waves, not
  wave-1 courts — wave-1's arithmetic is total (every well-formed input
  yields an output; no rejection path exists below the KEM layer).

### 1.6 Exclusions (explicit, non-re-invading)

- No KEM layer: no FO transform, no key tuples, no DecapsKey (p03).
- No ML-DSA-specific arithmetic: hint arithmetic, shift-and-round
  decompression, norm checks — the ML-DSA wave adds them on top of the
  same NTT core (p01: one sovereign NTT kernel serves both schemes).
- No BEAM NIF surface (p06 records it a non-goal), no GPU dispatch.
- No external dependencies, dev-time or runtime, including workspace
  path deps (section 5). No benchmark harness crate inside avec-pqc.
- No fixed sleeps anywhere (courts are iteration/count-driven).
- No worktrees, no per-lane copies; single canonical checkout; no git
  commands from this lane.

## 2. File ownership map

### 2.1 Owned by this wave

| path | content |
|---|---|
| `crates/avec-pqc/Cargo.toml` | member manifest; empty deps AND dev-deps |
| `src/lib.rs` | crate root, URN header, module docs |
| `src/params.rs` | Q, N, ZETA, QINV, Barrett v, n/2-inverse consts |
| `src/poly.rs` | dense Poly + 16 KiB PolyArena, add/sub, Drop-zeroize |
| `src/reduce.rs` | barrett/montgomery reduce + ct-select primitives |
| `src/ntt.rs` | const ZETAS/ZETAS_INV tables; ntt/intt butterflies |
| `src/basemul.rs` | MultiplyNTTs / BaseCaseMultiply |
| `src/compress.rs` | Compress/Decompress d + ByteEncode/Decode 1..12 |
| `src/keccak.rs` | Keccak-f[1600] permutation + sponge core |
| `src/xof.rs` | SHAKE-128/256 modes |
| `src/sha3.rs` | SHA3-256/512 modes |
| `tests/ntt_self_consistency.rs` | court C1 |
| `tests/zetas_kat.rs` | court C2 |
| `tests/basemul_properties.rs` | court C3 |
| `tests/xof_kat.rs` | court C4 |
| `tests/manifest_join.rs` | court C5 support (nm sweep, ordering check) |
| `tests/arena_zero_alloc.rs` | court C6 |
| `tests/compression_fidelity.rs` | court C7 |
| `tests/vectors/keccak/` | SHA3/SHAKE KAT corpus + provenance headers |
| `tests/vectors/ntt/` | zeta tables + fixed vectors + provenance |
| `benches/arith_ctq.rs` | harness = false, hand-rolled CTQ loop |

Rules: `#[inline(always)]` is banned on the manifest-joined entry symbols
(`ntt`, `intt`, `basemul`, `barrett_reduce`, `montgomery_reduce`, keccak
permutation) — same `<ltmp0>` blind-spot law p03 cites from the
`untranspose` manifest note. Single definition site for shared constants:
`src/params.rs` is the only place q/n/zeta are defined; p03's
`mlkem::params` consumes, never redefines (its ParamSet enum stays
wave-2-owned).

### 2.2 Shared seams this lane does not own

- Workspace root `Cargo.toml` members list: one-line append, integration
  lane, disclosed.
- `scripts/branchless_manifest.toml`: this wave appends its own
  `[[kernel]]` rows (C5 draft below) and nothing else; no edits to
  existing rows.
- `scripts/audit_branchless.sh` default `BRANCHLESS_SCOPE`: amendment is
  required (p06 W5-F no-gap law: a new kernel adjudicating only under an
  explicit scope is a plan violation, not a documented gap). Pinned:
  the default-scope amendment lands during wave-1, in the same
  transition as the first manifest rows, as a disclosed one-line diff
  through the integration lane. Until then the wave adjudicates under an
  explicit scope and discloses it in every receipt — an interim, not a
  resting state.
- Milestone index rows (p02 slot, wave numbering): index owner (0).
- Receipt minting: per p06 section 6, W5-0 infra (the `RECEIPT_DIR`
  override) precedes wave execution; wave-1 mints `w1kat-*`, `w1gate-*`,
  `w1bench-*` families into `docs/jira/v26.11.1/receipts/`. If W5-0 has
  not landed when wave-1 starts, the fallback is plain command+exit
  receipts in the wave-1 receipt doc — the blocker branches, it does not
  stop the wave.

### 2.3 Mangled-name scoping law

Manifest scope patterns are matched unanchored against mangled symbol
names (leading underscore stripped; first-match wins, manifest file
order). The house collision-proof form is the legacy-mangler
length-prefix ident (`6lookup`, `9try_build` precedent), so the new
rows scope on module idents, not substrings:

| kernel | scope_pattern | rationale |
|---|---|---|
| ntt butterflies | `3ntt` | module `ntt` ident; covers ntt/intt symbols |
| basemul | `7basemul` | module `basemul` ident |
| reduction | `6reduce` | module `reduce` ident; covers both reduces |
| keccak sponge | `6keccak` | module `keccak` ident |

Naming law, enforced at append time with an nm sweep: no avec-pqc
identifier (module, fn, test) may contain an existing scope substring —
`transpose`, `predicates`, `ladder`, `ct_`, `6lookup`, `6offset`,
`chunks_4way`, `release_struct_`, `9try_build` — and new patterns must
not substring-collide with each other. New rows append LAST so existing
rows keep first-match priority; the sweep receipt proves each new
pattern matches >= 1 real symbol at append time (the p06 F-5 zero-match
defect, caught a wave early). The `6reduce` prefix happens to share its
digit with `6offset`; ident text differs, and the sweep receipt is the
check, not hope.

## 3. Court inventory

Six dispatched courts (C1-C6) plus one adjacent (C7), each a real gate
that can exit 1.

### C1 — NTT self-consistency

- `INTT(NTT(f)) = f mod q` over seeded-deterministic random polys (fixed
  seed list, hand-rolled PRNG in-file — no `rand`), plus edge vectors:
  zero poly, one-hot at each of positions {0, 1, 127, 128, 255}, all-q-1,
  all-max-u16 inputs.
- Domain assert: outputs land in canonical Zq on both directions.
- Scale-factor pin: INTT's `n/2`-inverse constant is `3303`; hand check
  re-derived in-session: `128 x 3303 = 422784 = 127 x 3329 + 1`. The
  test asserts the constant in `params.rs` equals 3303.

### C2 — zetas KAT

- The `const` tables are policed three ways, so no table-vs-table
  circular check can pass:
  1. Independent re-derivation in-test: recompute `zeta^BitRev7(i) mod q`
     (forward) and `zeta^(2*BitRev7(i)+1) mod q` (basemul set) by
     repeated multiplication and bit-reversal, byte-compare against the
     const items.
  2. Root-of-unity law: `ZETA^128 = 3328` must hold against the const
     (`17^2=289; 17^4=296; 17^8=1062; 17^16=2642; 17^32=2580;
     17^64=1729; 17^128=3328` — chain re-derived in-session, matching
     t01 section 3).
  3. FIPS 203 Appendix A parity: the standard's tabulated values
     acquired into `tests/vectors/ntt/` with a provenance header
     (source doc SHA — t01 pins `fe1f12f3...b6cac`), asserted as KAT.
- A zeta table wrong under all three is impossible; a court that checks
  only (1) against a const copy of itself is the vacuous failure F-W1-06.

### C3 — basemul distributivity / domain equivalence

- The load-bearing property: multiply random poly pairs in the NTT
  domain via `basemul`, and the same pairs schoolbook-negacyclic
  (`(a*b) mod (x^256 + 1, q)`) in the coefficient domain; assert
  coefficient-domain equality after INTT. This pins basemul, NTT, and
  INTT against each other — C1 alone cannot.
- Distributivity over seeded random triples: `basemul(a, b + c) =
  basemul(a, b) + basemul(a, c)` in Zq.
- Montgomery/Barrett exercise: the basemul path runs both reduction
  classes with non-degenerate inputs (operands forced into the high
  12 bits), so the `QINV`/`v` constants are exercised, not defined.

### C4 — XOF/SHA3 KAT vs SHAKE vectors

- Corpus: NIST SHA-3 / SHAKE Known-Answer vectors (SHA3-256, SHA3-512,
  SHAKE-128, SHAKE-256; FixedLength and VariableLength classes), stored
  under `tests/vectors/keccak/`, hex-line format, hand-rolled reader in
  the test file (no serde/JSON), provenance header per file (source,
  fetch command, corpus sha256 as a committed literal — the
  `official_vectors.rs` b3sum idiom, `crates/avec-blake3/tests/`).
- Every mode every vector: byte-parity, any mismatch exit 1.
- Non-vacuity witness: a test-only mutated permutation (one round
  constant flipped, compiled only under `cfg(test)`) must FAIL the
  corpus. A KAT court that cannot detect a deliberately broken Keccak is
  a vacuous gate and FAILs as such (same law as `--selftest`).
- Wave-5 binding: p06 W5-E fuzzes this court's file parser with mutated
  corpora; the parser must therefore be a separate, named function in
  the test file, lawful verdicts = parse Err or loud FAIL, never panic.

### C5 — branchless manifest scope extension

New rows appended to `scripts/branchless_manifest.toml` (schema per the
existing manifest; patterns per 2.3):

```toml
[[kernel]]
name = "avec-pqc ntt butterflies"
owner = "crates/avec-pqc/src/ntt.rs"
scope_pattern = "3ntt"
accepted_remainder = ["ZERO_TRIP_GUARD", "LOOP_LATCH"]
expected_max_count = 0   # UNVERIFIED placeholder; observed+1 law at first audit
gate_mode = "ENFORCED"
observed_mnemonics = []  # filled from the first real audit-profile run

[[kernel]]
name = "avec-pqc basemul"
owner = "crates/avec-pqc/src/basemul.rs"
scope_pattern = "7basemul"
accepted_remainder = ["ZERO_TRIP_GUARD", "LOOP_LATCH"]
expected_max_count = 0   # UNVERIFIED placeholder
gate_mode = "ENFORCED"
observed_mnemonics = []

[[kernel]]
name = "avec-pqc reduction + ct-select"
owner = "crates/avec-pqc/src/reduce.rs"
scope_pattern = "6reduce"
accepted_remainder = ["ZERO_TRIP_GUARD", "LOOP_LATCH"]
expected_max_count = 0   # UNVERIFIED placeholder
gate_mode = "ENFORCED"
observed_mnemonics = []

[[kernel]]
name = "avec-pqc keccak sponge"
owner = "crates/avec-pqc/src/keccak.rs"
scope_pattern = "6keccak"
accepted_remainder = ["ZERO_TRIP_GUARD", "LOOP_LATCH"]
expected_max_count = 0   # UNVERIFIED placeholder
gate_mode = "INFORMATIONAL"  # promoted per observed+1 at first audit
observed_mnemonics = []
```

- The keccak row is beyond the dispatch's three named symbols; it is
  required by the manifest's inventory-completeness law (spec 27
  section 5: every hot-path kernel must appear; the XOF is a wave-1 hot
  kernel — the sampler feeds from it). Disclosed as an addition with its
  law, strikable by the index owner.
- Remainder classes reuse the existing named set only; no new class is
  proposed. The butterfly/basemul/reduce DATA PATHS carry zero sites —
  accepted classes cover loop scaffolding only. ENFORCED rows with
  placeholder ceilings follow the p03 ct_select precedent; ceilings
  re-pin at observed+1 from the first real audit run.
- Audit profile: build with `CARGO_TARGET_DIR` isolated audit profile,
  llvm-objdump (Apple objdump renders mach-o symbols as `<ltmp0>`,
  defeating name scoping — manifest wave-8/9 notes), explicit
  `BRANCHLESS_SCOPE` covering the new patterns plus the defaults during
  the interim (2.2).
- Non-vacuity: `BRANCHLESS_MANIFEST` override with zeroed ceilings must
  FAIL against a real avec-pqc artifact (existing mechanism); `--selftest`
  exit 0 re-receipted.

### C6 — zero-alloc arena court

- House idiom (`crates/avec-blake3/tests/zero_alloc.rs`): counting
  GlobalAlloc wrapper, per-thread const-init TLS counters, process-wide
  mutex at test start, counters reset inside the lock immediately before
  the measured body.
- Measured windows: poly add/sub; ntt; intt; basemul;
  compress/decompress; ByteEncode/ByteDecode d in {1, 4, 5, 10, 11, 12};
  SHAKE absorb+squeeze over arena-resident operands. The arena's single
  allocation happens at construction, outside every measured window.
- Asserts allocation delta == 0 AND byte delta == 0 in every window.
- Body-liveness guard: each window's body computes a checksum asserted
  non-trivial (non-zero, input-dependent), so an accidentally-empty
  measured body cannot pass as a zero-alloc success.
- Non-vacuity: an injected allocation in one measured body must flip
  the court to FAIL. Trivial to trip in principle; the court exists to
  keep it that way as the crate evolves.

### C7 — compression fidelity (adjacent, dispatch-plus)

- `Decompress_d(Compress_d(x))` error bound asserted EXHAUSTIVELY: every
  `x` in `[0, q)` for every `d` in {1, 4, 5, 10, 11} — `3329 x 5 =
  16645` decompress-after-compress checks, trivially cheap, stronger
  than sampling. Exact bound per `d` taken from the FIPS 203 definitions
  (t01-cited extraction), asserted as a KAT-validation input, promoted
  by the wave-2 KAT receipts.
- Bit-packing round-trip: `ByteDecode_d(ByteEncode_d(p)) = p` for
  canonical inputs, all d, exhaustive over the 256-coefficient layout
  with seeded random polys plus edges.
- Why it exists beyond the dispatch list: compression defects are
  otherwise first visible in wave-2 KAT failures, far from their cause;
  this court localizes them in-wave. It is small; the index owner may
  strike it.

## 4. CTQ proposals (per ADR-19)

Advisory until measured and ratified; id family `CTQ-W1-*` (p03 owns
`CTQ-PQC-*`; no collision). Ratio fences against an independent baseline
measured same-machine same-window (absolute bounds fence one leaked
constant, not performance). Baseline: a PQClean-class aarch64 reference
built in scratch, never a workspace dependency (p06 W5-A boundary).
`median_ns` is the primary number; a `cycles` column appears only when
backed by a real counter on the host (p06 cycles-honesty law).

| id | metric | lawful form | class |
|---|---|---|---|
| CTQ-W1-01 | NTT pair cycles (ntt+intt) | ratio <= 2.0x reference, min-of-N | advisory |
| CTQ-W1-02 | XOF cycles/byte (absorb+squeeze) | ratio <= 2.0x ref SHAKE, min-of-N | advisory |
| CTQ-W1-03 | basemul cycles/op | ratio <= 2.0x reference, min-of-N | advisory |
| CTQ-W1-04 | butterfly/basemul/reduce data-path branch sites | == 0, manifest-verified | GATE |
| CTQ-W1-05 | allocations in measured windows | 0 allocs / 0 bytes | GATE |

- CTQ-W1-04/05 are enumerable counts and conformance booleans, not
  numeric limits — per the ADR-19 boundary p06 records, they are NOT
  ADR-19-amendable; their only lawful change is ADR-14 remainder-list
  growth (a reviewed diff) or a spec change.
- Anchors are recorded at first run (UNVERIFIED; none claimed here).
  Ratification follows the CTQ-04 pattern
  (`../../v26.10.31/49-ctq04-variance-study.md`): spaced repeat runs,
  `stdev <= 15%` of mean, before any advisory limit is enforced.
- Harness: `benches/arith_ctq.rs`, `harness = false`, hand-rolled loop,
  no divan in avec-pqc (zero-dep discipline holds for dev-deps).

## 5. Dependency-zero proof (no new crates)

- Manifest shape: `[dependencies]` empty, `[dev-dependencies]` empty, no
  `[build-dependencies]`, no feature toggles that carry deps. The only
  workspace-level change is the one-line members append (2.2).
- Proof shape (planned; UNVERIFIED until executed at implementation
  time, method of `../../v26.10.31/82-dependency-audit.md`):
  1. `cargo tree -p avec-pqc` — exactly one node (the member itself).
  2. `Cargo.lock` diff after the members append — zero new package
     nodes beyond the member entry.
  3. `cargo build --offline -p avec-pqc` and
     `cargo test --offline -p avec-pqc` — exit 0 with
     `CARGO_NET_OFFLINE=true`, proving no registry reach needed.
- Where other crates would reach for a dependency, wave-1 hand-rolls:
  deterministic PRNG for courts (fixed-seed xorshift-class, seed list
  recorded in each court file); hex corpus reader (in-file); counting
  allocator (in-file); timing via `cntvct_el0` (p03 section 3.4
  precedent) — no `rand`, no `hex`, no `serde`, no `criterion`, no
  `zeroize` crate.
- Provenance digests for corpus headers are computed out-of-repo at
  acquisition time and committed as literals (the b3sum precedent —
  dev-time tooling is not a runtime dependency, and here it is not a
  dev-dependency either).
- RNG policy (p01): randomness is injected, never generated. The core is
  pure and deterministic — every function takes its bytes in; there is
  no RNG dependency to audit and nothing here to seed from the OS.
- The duplication note in 1.3 applies: avec-arrow lessons transfer as
  design, not as a path dependency.

## 6. Sequencing (p01 prerequisite ordering)

p01 orders the Keccak core ahead of any ML-KEM kernel work; the full
wave-1 order, each step ending in a minted receipt before the next:

1. S1: `params.rs` skeleton + `keccak.rs` + `xof.rs` + `sha3.rs`; C4
   lands first (the permutation is testable with zero lattice code).
2. S2: `poly.rs` arena + `reduce.rs`; C6 lands on real arena bodies.
3. S3: `ntt.rs` const tables + butterflies; C1 + C2.
4. S4: `basemul.rs`; C3 (the court that couples S3 and S4).
5. S5: `compress.rs`; C7.
6. S6: manifest rows (C5) + default-scope amendment transition (2.2).
7. S7: CTQ anchor runs (`benches/arith_ctq.rs`), advisory, recorded.
8. Wave-1 receipt: `docs/jira/v26.11.1/80-wave-1-receipt.md` (slot
   proposal recorded for the index owner; p06's numbering scheme
   reserves 90 for wave 5).

Adjacency-tier items (1.4) slot after S5, never blocking S6.

## 7. Falsifiers and definition of done

### 7.1 Falsifiers

| id | falsifiable claim | observation that kills it |
|---|---|---|
| F-W1-01 | arithmetic is correct | any KAT mismatch (C2/C3/C4/C7) exits the suite nonzero |
| F-W1-02 | data paths are branchless | an unexplained branch mnemonic under the manifest gate |
| F-W1-03 | arena is zero-alloc on measured windows | an injected allocation passes C6 |
| F-W1-04 | C1 alone is not sufficient | INTT∘NTT=id green while C3 domain-equivalence red |
| F-W1-05 | dependency-zero holds | `cargo tree -p avec-pqc` > 1 node; any non-empty deps section |
| F-W1-06 | zeta court is non-circular | a table-vs-table-only C2 passes |
| F-W1-07 | scope patterns match real symbols | a new pattern matching zero symbols at append |
| F-W1-08 | XOF court is non-vacuous | the mutated-permutation witness (C4) passes the corpus |
| F-W1-09 | no-gap law holds at wave-1 close | a kernel still on explicit scope after S6 |

Table amplifications: F-W1-01 — a mismatch that still exits 0 means the
court, not just the math, is broken. F-W1-02 — a gate exiting 0 with an
unexplained hit is a vacuous gate. F-W1-04 — C3 is load-bearing, never
droppable. F-W1-06 — const-table-to-const-table-only comparison is
vacuous by construction. F-W1-07 — p06 F-5 caught a wave early.
F-W1-09 — the 2.2 interim is not a resting state.

Dispatch falsifiers, stated plainly: KAT mismatch = arithmetic wrong; a
branch in .text (data path) = discipline failed.

### 7.2 Definition of done

A wave-1 receipt (`80-wave-1-receipt.md`) exists and carries all of:

1. C4 KAT receipts: per-mode vector counts, corpus sha256 literals,
   commands, exit 0; mutated-permutation non-vacuity witness (gate).
2. C1 + C2 receipts: self-consistency vector counts, zeta re-derivation
   and Appendix-A parity, exit 0 (gate).
3. C3 receipts: domain-equivalence + distributivity runs, exit 0 (gate).
4. C7 receipts: exhaustive fidelity + bit-pack round-trip, exit 0 (gate
   if the adjacent court is not struck).
5. Manifest join receipts: rows landed with `observed_mnemonics` from a
   real audit-profile run; gate exit 0 on the real tree; exit 1 on the
   zeroed-ceiling override (non-vacuity); `--selftest` exit 0;
   default-scope amendment landed or the deviation loudly registered.
6. C6 zero-alloc receipt: 0/0 deltas on every measured window, with the
   body-liveness guards recorded.
7. E=0 dependency proof: the three section-5 commands with exits.
8. CTQ-W1-01/02/03 first-run anchors recorded (advisory), ratification
   status per the 49-pattern.
9. P28-SIL URN header on every landed file; no SPDX anywhere.
10. Standing block in this file updated from UNVERIFIED with receipt
    pointers; index amendments filed (p02 slot/name, wave numbering,
    receipt-slot proposal) as integration defects for the index owner.

Until 1-10 exist, wave-1 standing is exactly: UNVERIFIED.

## Standing

UNVERIFIED end to end at write time. This plan consumes landed tickets
(t01, p01, p03, p06) and receipts decisions (D-1..D-6), sequencing
(S1-S8), file ownership, courts (C1-C7), and falsifiers — none of which
is execution. Nothing here promotes to PARTIAL/ALIVE except through
minted receipts per section 7.2.

## See Also

- [../00-index.md](../00-index.md) — milestone charter and disciplines.
- [t01-ml-kem-params.md](t01-ml-kem-params.md) — consumed parameter
  ground truth, constants, open items (section 7 answered in 1.5).
- [p01-go-no-go-adjudication.md](p01-go-no-go-adjudication.md) — GO
  verdicts; Keccak-core prerequisite; CT-KEM-1; RNG policy.
- [p03-wave2-mlkem-plan.md](p03-wave2-mlkem-plan.md) — consumer-side
  wave-1 dependency contract (its section 1.1) and manifest precedents.
- [p06-wave5-baselines-plan.md](p06-wave5-baselines-plan.md) — W5-F
  no-gap law, receipt minter, CTQ boundary rules.
- [../../v26.10.31/22-adr-register.md](../../v26.10.31/22-adr-register.md)
  — ADR-04/ADR-14 (M=1, remainder policy), ADR-19 (CTQ amendment),
  ADR-06 (sovereign core pattern).
- [../../v26.10.31/23-64b-quad-spec.md](../../v26.10.31/23-64b-quad-spec.md)
  — Quad64 layout laws this plan applies to working formats.
- [../../v26.10.31/gate-branchless-manifest.md](../../v26.10.31/gate-branchless-manifest.md)
  — manifest gate method and non-vacuity control.
- [../../v26.10.31/49-ctq04-variance-study.md](../../v26.10.31/49-ctq04-variance-study.md)
  — CTQ ratification pattern (stdev <= 15%).
- [../../v26.10.31/82-dependency-audit.md](../../v26.10.31/82-dependency-audit.md)
  — the E=0 proof method section 5 inherits.
- `/Users/sac/avec/crates/avec-blake3/tests/zero_alloc.rs` — C6 idiom.
- `/Users/sac/avec/crates/avec-blake3/tests/official_vectors.rs` —
  corpus provenance-header idiom.
- `/Users/sac/avec/crates/avec-arrow/src/mmu_arena.rs` — 16 KiB slab
  pattern (lessons transfer; no code import).
- `/Users/sac/avec/scripts/branchless_manifest.toml` — manifest schema
  C5 extends.
- `/Users/sac/avec/scripts/audit_branchless.sh` — the gate C5 joins.
- `/Users/sac/avec/LICENSE-P28-SIL.md` — governing license URN.
