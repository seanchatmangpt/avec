# T11: Existing avec-Asset Reuse Map (v26.11.1 PQC)

Status: UNVERIFIED (Phase-0 research ticket). This is the reuse map from
the v26.10.31 substrate onto the v26.11.1 PQC milestone: what already
exists, what it reduces to, what needs modification, and the one class
that is genuinely new. Nothing here has executed against PQC code — no
PQC crate exists in this checkout yet.

Slot note: `../00-index.md` lists t11 as `t11-beam-nif-surface.md`; this
recovery wave dispatches `t11-avec-asset-reuse.md`. Both claim lane t11.
The index owner must reconcile (renumber one side); this file does not
edit the index. `t13` already cross-references the NIF-surface t11, so
the collision is live, not theoretical.

Companions: t06 (branchless crypto arithmetic — consumes section 1),
t08 (zero-dep substrate — owns the zeroize deliverable of section 3),
t09 (ADR-19 CTQ amendment — owns section 5's policy), t12 (CT audit
method — owns the dudect half of sections 5-6), t13 (court corpus).

## 1. Mask Algebra: `predicates.rs` -> ML-KEM / ML-DSA Conditional Control

`crates/avec-arrow/src/predicates.rs` carries the complete all-ones/
all-zeros mask algebra the PQC kernels need, `no_std`, exhaustively
tested on small domains (tests at predicates.rs:164-225):

- `ct_eq_u32` — xor/negate/arithmetic-sign-shift to full mask
  (predicates.rs:17-22); `ct_eq_u64_mask` same form at 64 bits (53-56).
- `ct_lt_u32` — widening u64 subtract, MSB negated into a mask
  (predicates.rs:26-31); `ct_lt_u64_mask` widens to u128 (60-64).
- `ct_select_u32/u64/usize` — `(b_true & mask) | (b_false & !mask)`
  (predicates.rs:35-49).
- `ct_quad_eq_u32` — four lane masks ANDed into one verdict (71-77).

Standing caveat inherited by every consumer below: the header (lines
8-13) marks these M=1 CANDIDATES, UNVERIFIED — constant-time is a
property of disassembled `.text`, and LLVM may lower to branches (doc
27 §4 de-optimization hazard). "Reuse" here means "reuse as candidates
with a standing disassembly-gate obligation," never "reuse as proven."

### 1a. ML-KEM conditional reductions (FIPS 203)

Every coefficient carry-reduction in the NTT, base multiplication, and
compression path reduces to one masked conditional subtraction:

```rust
// Canonical representative mod q=3329 without a branch.
// ge is all-ones iff c >= Q; bitwise NOT is trivially constant-time.
let ge = ct_lt_u32(c, Q) ^ u32::MAX;
let c  = c - (ge & Q);
```

Intermediate bounds decide how many passes: post-basemul coefficients
below 2q need one pass; wider intermediates need two (subtract-then-
subtract-again under the same mask idiom). No new primitive is
required — `ct_lt_u32` + xor-not + and-dec covers the whole family.

Decapsulation's implicit rejection is the same algebra at block width:
re-encrypt, compare the 32-byte ciphertext as four u64 limbs, AND the
limb masks (the `ct_quad_eq_u32` AND-of-lanes shape at u64 width), then
`ct_select_u64(mask, k_direct, k_fail)` picks the returned shared
secret. FIPS 203 §7.3's always-`J(c || m')` formulation is already
select-shaped; the mask algebra is what keeps the select off a branch.

### 1b. ML-DSA rejection logic (FIPS 204)

`RejNTTPoly`/`RejBoundedBits` accept a candidate iff redundant top bits
are zero. Per candidate:

```rust
let accept   = ct_eq_u32(w >> 20, 0);        // redundant bits == 0
let step     = (accept & 1) as usize;
let slot     = ct_select_usize(step.wrapping_neg(), pos, SINK);
out_or_sink[slot] = coeff;
pos += step;
```

That is `filter_quads_eq`'s loop body transposed (see 1c). The norm
restart test (infinity-norm vs `gamma - beta` / `gamma1 - beta`) is
`ct_lt` under an absolute-value idiom (`(a ^ m) - m` with `m = a >> 31`
— itself mask algebra), producing a restart mask. The restart latch
itself is a loop-latch branch: register it as `LOOP_LATCH` remainder
per the manifest precedent (section 6), do not pretend it away.

### 1c. `filter_quads_eq` sink-slot pattern = coefficient compaction model

`filter_quads_eq` (predicates.rs:109-156) is the reusable skeleton for
any "keep accepted coefficients, ignore rejects, in input order" loop:

- Write position advances by `mask & 1`; the written slot is a
  `ct_select` between the real index and a sink slot (n_quads), so the
  write stream is input-shape-invariant (predicates.rs:143-148).
- Zero-trip is handled in ct arithmetic: `trip = n + (n == 0)` and a
  never-matching dummy quad via ct pointer select (117-127).
- Bounds-check elimination: raw-pointer form discharges the in-bounds
  invariant once (debug_assert + capacity contract) instead of per-lane
  `b.eq` panic branches (93-108).
- Exactly one conditional branch remains: the loop latch — the honest
  irreducible count (99-108).

For RejNTTPoly this is the whole design: SINK is one past the real
capacity, rejects write through it, `pos` is the accept count. The
capacity contract and the sink discipline transfer verbatim.

## 2. Quad64 Dense-Arena Lesson -> Polynomial Coefficient Arenas

The Quad64 stack (`crates/avec-arrow/src/layout.rs`,
`transpose.rs`, avec-parquet level-free codec) proves a pattern, not
just a struct: fixed-stride dense records, layout ground truth pinned
by compile-time asserts (regression is a build failure), SoA columns
for hot loops, byte-identical encode/decode roundtrip receipts.
Transposed to PQC:

| Quad64 invariant | Polynomial arena invariant |
|---|---|
| 64 B fixed record stride, no padding | `Poly` = 256 x u16 = 512 B fixed stride; NTT in place |
| compile-time size/align/offset asserts | compile-time asserts on N, q, packed width, strides |
| dense arena, zero per-record heap | polys preallocated in arenas outside the measured region |
| SoA transpose for column loops | coefficient-wise kernels see contiguous u16 lanes |
| level-free byte-identical roundtrip | 12-bit pack/unpack roundtrip court, byte-exact |
| sentinel law (BOTTOM/WILDCARD) | canonical-form law: every coefficient < q before encode |

The canonical-form row is load-bearing: a non-canonical encoded byte is
a KAT failure, so the structural roundtrip court and the KAT court are
the same discipline at two widths.

Existing caveat: the only arena today is `MmuBuf` — 16 KiB page-aligned
with `Drop` = bare `dealloc`, no wipe
(`crates/avec-arrow/src/mmu_arena.rs:104-111`; t13 section 1 receipts
this). The arena mechanics transfer; the secret
hygiene gap is exactly what t08/pool replaces.

## 3. `spsc_ring.rs` -> Secret-Pool Ring Basis

`crates/avec-beam/src/spsc_ring.rs` contributes: power-of-two capacity
with monotonic u64 counters masked to slots (no wrap branch,
spsc_ring.rs:60-100), 128-byte logical head/tail separation against
false sharing (15-28), `MaybeUninit` slots, `T: Copy` bound.

Secret-pool shape: pre-allocated page-aligned secret slots (MmuBuf
pages) owned by a pool; an `SpscRing<SlotHandle>` carries Copy handles
from releaser to acquirer; the consumer wipes before releasing.

Zeroize-on-drop wrapper proposal — with its real constraint named:

- `SpscRing<T: Copy>` cannot hold a `Drop` type (Drop types are not
  Copy). So the ring never stores the secret; it stores a Copy
  `SlotHandle { index, generation }`.
- The pool owns the bytes. A `Secret<T>` newtype handed to the consumer
  implements `Drop` as volatile byte-wise zeroization (hand-rolled, per
  discipline 2 — no external zeroize crate), then returns the handle to
  the free ring. Wipe fires at consumer drop, before reuse.
- The generation counter defeats stale-handle reuse across wraps.
- mlock/swap hardening is cfg-gated std-only, per t13 §2a — never on
  the no_std path.

This is REUSE-WITH-MODIFICATION: the ring stays as-is; the
handle indirection and the wrapper are new, small, and owned by t08.

## 4. `audit` CountingAlloc -> Zero-Alloc Claim Infrastructure

`crates/audit/src/alloc_counter.rs` is already the designated shared
primitive (its own header, lines 5-8): `CountingAlloc` pass-through
`GlobalAlloc` with process-wide counters (19-21) plus
`with_alloc_tracking` — nested per-thread scopes whose inner report
excludes the outer baseline (127-153) — and `AllocReport::is_zero`.

For PQC this converts "no hidden allocation" from prose to receipt:

```rust
let (k, report) = with_alloc_tracking(|| encapsulate(&ek, &m));
assert!(report.is_zero(), "encaps allocated: {report:?}");
```

Precedent: CTQ-02 gates a 64 MiB BLAKE3 pass at 0 allocation events and
exits 1 on violation (ctq_report.rs:495-536). Every PQC path —
keygen, encaps, decaps, sign, verify, NTT — gets the same gate. The
gate's failure mode is the design tool: any `Vec::with_capacity` inside
a measured path fails it loudly, which forces the section-2 arenas.

Two facts to respect:

1. Two implementations of one calculus exist: the audit crate's scoped
   per-thread form and ctq_report's inline global reset/read counters
   (ctq_report.rs:55-82). For single-threaded kernel claims the scoped
   form is strictly better (no cross-thread noise). Reuse audit's;
   the inline variant stays harness-local.
2. Boundary: the hook sees the Rust `GlobalAlloc` seam only. BEAM-side
   or direct FFI `malloc` is invisible to it. The zero-alloc claim is
   scoped to "Rust-side GlobalAlloc seam" — section 6's dudect and the
   NIF lane (t11-beam-nif-surface) cover what this cannot.

Zero-alloc is not constant-time; it kills the allocation-count/size
channel only. The timing channel is dudect's job (t12).

## 5. Inherited CTQ Machinery From Day One

Four assets arrive finished; the milestone's job is new rows, not new
scaffolding.

- `crates/bench/src/bin/gage_rr.rs` — gage R&R court: 10k reps of a
  fixed workload, trimmed CV < 1%, fail-closed exit. Run before any
  timing CTQ is trusted on a machine.
- `crates/bench/src/bin/ctq_report.rs` (v2) — the gate grammar:
  REAL gates vs informational REPORT rows; exit 1 iff a REAL gate
  fails; per-row correctness screen before timing (digest mismatch ->
  DISQUALIFIED regardless of speed — discipline 1 made mechanical);
  `LSL_RATIFIED: Option<f64>` amendment hook emitting
  AMENDMENT-RATIFIED (39-48); inline mini-gage, CV > 2% -> SKIP-NOISY
  load-gating (225-281); retired pins retained as provenance
  (`retired_pin`, 210-216).
- `scripts/dod.sh` — manifest DoD checker: `[[entry]]` rows with
  `expect = pass|advisory|pending-holds`; ADV-SKIP on exit 2;
  PENDING non-failing, STALE red; VACUOUS-CHECKER selftest (a checker
  that cannot flip is not a checker); exit 3 fail-closed on a missing
  or empty manifest. Invoke with an explicit v26.11.1 manifest path.
- `scripts/mint_receipt.sh` — fail-closed receipt minter: captures
  command, exit, HEAD sha, dirty flag, BLAKE3 digest of stdout tail;
  append-only (exit 73 on collision); propagates nonzero exits as
  FAILED standing. One edit needed: the receipt dir is hardcoded to
  v26.10.31 (mint_receipt.sh, RECEIPT_DIR line) — parameterize or
  point at `docs/jira/v26.11.1/receipts/`.

ADR-19 posture (22-adr-register.md, ADR-19, RATIFIED 2026-10-31): CTQ
limits are hypotheses with falsifiers, amendable when proven impossible
via the doc-48 pattern — ceiling math, measured distribution,
architecture attribution, retained refutation record. The three new PQC
CTQ families enter as amendable from day one:

- cycles/op per kernel (NTT, base mul, keccak_f1600, sign, decaps).
- dudect t-stat per constant-time claim (t12 owns method).
- secret-pool acquire/release latency.

Pin each behind an `LSL_RATIFIED`-style hook: while unratified, the
gate honestly FAILs against the provisional number — never silently
absent, never pre-relaxed. And every timing gate above inherits the
mini-gage precondition: dudect and cycles/op run SKIP-NOISY, not FAIL,
on a contended machine.

## 6. `audit_branchless.sh` -> Manifest Scope-Extension for Crypto Symbols

The gate (`scripts/audit_branchless.sh`) transfers whole: fail-closed
disassembler detection (exit 3), banned-mnemonic regex reaching awk via
ENVIRON — the `-v` escape-decode FP is a retired class, keep the
transport (audit_branchless.sh:62-69) — positive control that must fire
(exit 2 VACUOUS GATE otherwise), clean-fixture scope demo, and manifest
adjudication where an unlisted symbol is a FAIL.

The extension pattern for the PQC kernels:

1. Extend default `BRANCHLESS_SCOPE` (`ladder|predicates|transpose|ct_`)
   with the crypto alternatives (e.g. `|ntt|basemul|keccak|rej|csub`).
   `ct_` already substring-matches the predicates-class symbols, so
   section-1 kernels are in default scope today; the new ones are not
   until the scope grows.
2. One `[[kernel]]` row per crypto kernel in
   `scripts/branchless_manifest.toml`: `owner` pointing at the
   avec-pqc crate, `accepted_remainder` classes named after real
   mechanisms (LOOP_LATCH, ZERO_TRIP_GUARD, RESULT_ERROR_GUARD are the
   precedent classes; mask-selected sink writes add no branch, so there
   is no "reject sink" class to accept).
3. `observed_mnemonics` captured from audit-profile disassembly —
   build with `--profile audit` because thin-LTO release emits
   bitcode-only rlibs objdump cannot read (audit_branchless.sh:204-206).
4. Ceiling = observed + 1; an unclassified mnemonic is a FAIL even
   under the ceiling; unlisted symbol is a FAIL.
5. Scope-gap law: a kernel whose scope_pattern the default scope does
   not cover adjudicates only under explicit scope — that is a
   documented gap (wave-9's `6lookup` precedent), not a silent one. For
   PQC, extend the default scope at kernel-landing time so the gap
   never opens.

Inventory-completeness law (manifest header): every hot-path crypto
kernel appears in the manifest or the inventory check fails. This is
the admission path for every kernel waves W1-W5 land; it is inherited,
not redesigned.

Complementarity note: the manifest proves branch-freedom statically;
dudect proves timing-independence statistically. Different falsifiers;
a CT claim needs both (t12).

## 7. NOT Reusable: SHAKE-128/256 XOF — the One New Primitive Class

FIPS 203/204 pin SHA3-256, SHA3-512, SHAKE128, SHAKE256 over the
Keccak-f[1600] sponge. `avec-blake3` exists, is fast, and passes
CTQ-02/CTQ-06 — and buys nothing here. BLAKE3 is a BLAKE2s-class
compression function over a 16-word u32 state with a fixed message
schedule; it is not a Keccak permutation, has no rate/capacity split,
no Keccak padding or domain-separation encoding, and produces different
bytes for the same input. There is no substitution, no adapter, no
"close enough": KAT byte-parity (discipline 1) requires the exact NIST
functions.

What does transfer from the blake3 lane is the machinery around the
primitive: the zero-alloc gate, the throughput-CTQ-with-amendment
pattern, the manifest-row discipline, receipt minting. The primitive
itself does not.

Keccak-f[1600] is therefore the one genuinely new primitive class in
the milestone: a bit-level sponge permutation (theta/rho/pi/chi/iota),
no substrate analog, new kernel, new manifest row, its own disassembly
audit lane. Absorb/squeeze streaming and domain separation have
blake3-shaped analogs to copy as patterns, but the permutation and the
FIPS XOF wrapper are new code.

New-adjacent (new artifacts built on reused patterns, not reuse):
dudect harness (gage-pattern reuse, new statistics), the polynomial
arenas (section 2 pattern, new code), the `Secret` wrapper (section 3),
the FIPS byte/bit encodings (trivial arithmetic, new code).

## 8. Reuse / New Split

| Asset | Disposition |
|---|---|
| ct_eq / ct_lt / ct_select / ct_quad_eq | REUSE as-is (M=1 candidates, gate obligation carries) |
| filter_quads_eq sink-slot compaction | REUSE as pattern; new instance per reject kernel |
| Quad64 layout asserts / SoA / roundtrip | REUSE as pattern; new Poly arena code |
| MmuBuf page arena | REUSE with modification — needs wipe-on-drop (t08) |
| SpscRing | REUSE with modification — SlotHandle indirection |
| Secret<T> zeroize-on-drop wrapper | NEW (t08-owned), built on ring + arena |
| CountingAlloc / with_alloc_tracking | REUSE as-is (audit crate form) |
| gage_rr | REUSE as-is |
| ctq_report v2 | REUSE as pattern; new REAL CTQ rows |
| dod.sh | REUSE as-is (explicit manifest arg) |
| mint_receipt.sh | REUSE with one edit (receipt dir parameterization) |
| branchless gate + manifest | REUSE as-is; scope + kernel rows extend |
| Keccak-f[1600] / SHA3 / SHAKE XOF | NEW — the one new primitive class |
| dudect harness | NEW machinery on gage patterns (t12) |

Count: 12 reused (3 with modification) vs 2 new artifact families, one
of which (Keccak/SHAKE) is the only new primitive class. Everything
else the milestone needs is composition of existing capital.

## Falsifiers

- A section-1 primitive lowers to a data-dependent branch in the audit
  profile -> the reuse claim was wrong for that kernel; it FAILs the
  gate and the consumer cannot ship as M=1.
- A reject-compaction kernel shows per-lane bounds-check branches ->
  the raw-pointer/sink-slot pattern was not carried; FAIL.
- A pool slot returns to the free ring with nonzero bytes -> the
  wipe-on-release design is wrong; FAIL (test with the alloc spy,
  `crates/avec-beam/src/alloc_spy.rs`).
- Zero-alloc gate passes while a crypto path allocates via FFI/BEAM ->
  expected, not a gate defect: the claim's stated boundary is the Rust
  GlobalAlloc seam.
- A crypto kernel lands without a manifest row -> inventory check must
  fail; if it does not, the checker is vacuous.

## See Also

- `../../v26.10.31/27-branchless-kernel-abi.md` (M=1 ABI, §3 gate, §4 hazard)
- `../../v26.10.31/22-adr-register.md` (ADR-14 remainder policy, ADR-19)
- `../../v26.10.31/29-benchmark-harness-plan.md` (CTQ/gage origin)
- `../../v26.10.31/48-lsl-rederivation.md` (ADR-19 amendment pattern)
- `t06-branchless-crypto-arithmetic.md`, `t08-zero-dep-crypto-substrate.md`,
  `t09-adr19-ctq-amendment.md`, `t12-constant-time-audit-method.md`,
  `t13-court-corpus-design.md` (v26.11.1 companions)
