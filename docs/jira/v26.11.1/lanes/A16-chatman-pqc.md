# A16: Chatman Equation Sealer — `cgap-crypto` (PROPOSED), `chatman.rs`, `lease.rs`

Lane: A16 of 20, v26.11.1 overnight campaign. Repo: `/Users/sac/cgap` (single
canonical checkout; no worktrees, no git commands from this lane). Wave 3.
File ownership: this document only; the crate `cgap-crypto` it specifies is
PROPOSED — this lane writes no code.

## 1. Status and Standing

- Lane doc: WRITTEN (this file).
- Crate `cgap-crypto` (modules `chatman.rs`, `lease.rs`): PROPOSED.
- Exit gate (design-level, enforced later by courts): (a) the `Phi_lease` tick
  counter is monotonic and TRAPS on exhaustion — a rewind attempt is
  unrepresentable in the type; (b) every valid equation transition emits a
  receipt = BLAKE3 digest + PQC seal; (c) the three anti-vacuity courts of
  Section 8 pass and are demonstrably non-vacuous.
- Every claim below is a design claim (UNVERIFIED until courts run). Nothing
  here asserts executed verification. One byte-level question is marked
  UNKNOWN rather than decorated (Section 3.4).

## 2. Context: The Equation as a Receipt-Sealing Discipline

The operating law is `A = mu(O*)`, `R = receipt(A)` with the receipt carrying
identity, authority, consequence, replay, standing. The campaign equation

```
DeltaS_admitted + Phi_lease == R_receipt   (mod Sigma_PQC)
```

is that law projected into cgap's storage layer: an admitted state delta,
executed under a lease, seals into a receipt — and the seal is a concrete
cryptographic operation, not notation. The failure mode this crate exists to
kill is the equation-as-decoration failure: documents that write the algebra
and never say what the symbols ARE. Each symbol gets a byte definition in
Section 3 or an honest UNKNOWN.

Why PQC at all: cgap receipts are durable artifacts (like the WAL of lane
A05) with a lifetime longer than classical signature assumptions. A receipt
that is replayable in 2040 must not depend on RSA/ECC unbroken in 2040. The
BLAKE3 digest covers integrity; the PQC seal covers authenticity of the
sealing act itself.

## 3. Byte Semantics of Each Symbol

### 3.1 `DeltaS_admitted` — the admitted state delta

`DeltaS_admitted` is a canonical byte string, not an abstraction:

- `domain_tag` : 8 bytes, ASCII `"CGAPDLT1"` (versioned domain separator;
  prevents cross-protocol signature reuse).
- `from_root` : 32 bytes, BLAKE3 digest of the prior state root (Git/Parquet
  root per lane A03/A05 conventions).
- `to_root` : 32 bytes, BLAKE3 digest of the new state root.
- `op_count` : 8 bytes little-endian, number of admitted operations folded
  into the delta.
- `admissions` : 32 bytes, BLAKE3 Merkle root over the ordered list of
  per-operation admission receipts (each a prior `R_receipt` — receipts chain).

`DeltaS_admitted := domain_tag || from_root || to_root || op_count ||
admissions` — fixed 112-byte encoding, little-endian fields, no optional
fields, no serialization choices (deterministic by construction; hand-editing
a generated encoding is banned by the same discipline as ggen projections).

### 3.2 `Phi_lease` — the lease epoch

`Phi_lease` is the authority term: proof that the delta was manufactured under
a live, non-exhausted, non-rewound lease.

- `lease_id` : 32 bytes, random at lease creation.
- `tick` : 64-bit unsigned little-endian, monotonic, starts at 0. One tick
  per sealed receipt issued under the lease.
- `tick_ceiling` : 64-bit unsigned little-endian, fixed at creation.
- `issued_root` : 32 bytes, BLAKE3 digest binding the lease to the key epoch
  (see Section 6).

`Phi_lease := lease_id || tick || tick_ceiling || issued_root` — 80 bytes.
Lease mechanics (monotonicity, exhaustion trap) are in Section 5.

### 3.3 `R_receipt` — the sealed receipt

```
R_receipt := magic || version || DeltaS_admitted || Phi_lease ||
             blake3_digest || pqc_seal
```

- `magic` : 4 bytes `"CGRC"`; `version` : 1 byte `0x01`.
- `blake3_digest` : 32 bytes = BLAKE3 over the exact concatenation
  `DeltaS_admitted || Phi_lease` (keyed mode, key = `issued_root`; this makes
  the digest lease-bound, so a receipt stolen from one lease epoch does not
  verify under another).
- `pqc_seal` : algorithm-dependent signature (Section 4) over
  `blake3_digest || algorithm_id`, where `algorithm_id` is a 2-byte code so
  the seal names its own algorithm (hybrid/agility without malleability).
- Receipt size: 117 bytes + seal size (e.g. 2420 for ML-DSA-65, Section 4).

### 3.4 `mod Sigma_PQC` — the seal operation, and the honest UNKNOWN

The `mod Sigma_PQC` term denotes: equality is checked *after* the sealed
components verify — i.e. `DeltaS_admitted + Phi_lease` "equals" `R_receipt`
iff the digest recomputes and the PQC signature verifies. Concretely the
verification function is:

1. parse receipt, check magic/version;
2. recompute BLAKE3(`DeltaS_admitted || Phi_lease`, key=`issued_root`) and
   require byte equality with `blake3_digest`;
3. verify `pqc_seal` over `blake3_digest || algorithm_id` against the epoch
   public key.

**UNKNOWN (honest):** the key-agreement/rotation story — how `issued_root`
rotates across epochs and how old receipts verify after rotation — is not
defined at byte level here. Options (static epoch registry vs receipts
carrying their epoch pubkey hash vs SLH-DSA self-certifying chain) have
different replay consequences and need a design pass with lane A17's trap
model. Marked UNKNOWN; not decorated as decided.

## 4. ADR: PQC Algorithm Choice

**Decision (proposed): ML-DSA-65 (FIPS 204, Dilithium3) for the seal;
SLH-DSA (FIPS 205) as the conservative fallback; rejected: hand-rolled
lattices, rejected: RSA/ECC as the sole seal.**

Candidates and criteria (no_std friendliness, signature size, security
level, maturity, audit surface):

| Candidate | sig size | sec. level | no_std status | verdict |
|---|---|---|---|---|
| ML-DSA-65 (FIPS 204) | 2420 B | Cat 3 | pure-Rust crates, wasm32 plausible | SELECT (proposed) |
| SLH-DSA-SHA2-128s (FIPS 205) | 7856 B | Cat 1 | hash-only; small trust base | FALLBACK |
| Falcon-512 | ~666 B | Cat 1 | float-heavy; painful in no_std/wasm | REJECT here |
| hand-rolled lattice | any | none | — | REJECT — canonical overclaiming failure |
| Ed25519 only | 64 B | classical | trivial | REJECT as sole seal (OK only in a later hybrid) |

Selection criteria, ranked: (1) no_std / wasm32 compilability (cgap ships a
wasm crate — `crates/wasm-footprint` exists today); (2) hash-based fallback
exists so the design degrades to assumptions BLAKE3 already makes; (3)
signature size bounded enough to keep receipts page-friendly; (4) NIST-
standardized only.

**UNSUPPORTED clause:** if no maintained pure-Rust ML-DSA crate survives the
no_std gate, the honest blocker path is: ship `lease.rs` (no crypto) +
`chatman.rs` against a `Sealer` trait with an in-repo BLAKE3-only "unsealed"
implementation clearly typed as `Unsealed` in the receipt type — never a
silent downgrade — and file the PQC seal as BLOCKED with the specific crate
gap named. The trait seam keeps the court suite (Section 8) valid either way.

## 5. Lease Mechanics (`lease.rs`)

- **Monotonic tick**: the counter is a `Tick(u64)` newtype whose only
  constructor from raw values is private; `tick()` returns the current value
  and advances it. There is no `set_tick`, no `Sub` impl, no API that accepts
  a caller-supplied tick. Rewind is unrepresentable, not merely unchecked.
- **Exhaustion = trap, not error return.** When `tick == tick_ceiling`, the
  next tick TRAPS (panic in hosted builds; `udt`-style abort / `trap` on
  wasm), following lane A17's trap discipline: exhaustion of authority is an
  invariant failure of the lease abstraction, not a recoverable condition —
  an error return would invite callers to "handle" authority exhaustion by
  minting a new lease mid-delta, silently splitting one epoch into two. A
  lease that needs to continue mints a NEW lease through the public API, with
  its own id and ceiling; the trap makes the quiet continuation impossible.
- `Phi_lease` bytes (Section 3.2) are produced only at seal time, from the
  lease's internal state; no public path exposes mutable lease bytes.
- Ceiling selection is a policy argument at construction; the crate fixes no
  default ceiling policy beyond "ceiling > 0".

## 6. Receipt Format Summary (`chatman.rs`)

- BLAKE3 is computed over: `DeltaS_admitted || Phi_lease` (112 + 80 bytes),
  keyed by `issued_root` — Section 3.3. Nothing else enters the digest; the
  seal then covers the digest plus the algorithm id, so signature and digest
  are chained, not parallel claims.
- Public API (proposed): `seal(delta: DeltaS, lease: &mut Lease) ->
  Receipt`, `verify(receipt: &Receipt, vk: &VerifyingKey) -> Result<_,
  VerifyError>`; `verify` is pure, `seal` is the only tick consumer.

## 7. Architecture (C4)

```mermaid
C4_Context
  title cgap receipt sealing - context
  Person(dev, "cgap consumer", "issues admitted deltas")
  System(cgapCrypto, "cgap-crypto", "Chatman equation sealer: lease + BLAKE3 + PQC seal")
  System_Ext(pqcLib, "PQC crate (ML-DSA/SLH-DSA)", "FIPS 204/205 signatures")
  Rel(dev, cgapCrypto, "seal(delta, lease), verify(receipt, vk)")
  Rel(cgapCrypto, pqcLib, "sign/verify seal bytes")
```

```mermaid
C4_Container
  title cgap-crypto containers
  Container_Boundary(c, "cgap-crypto") {
    Container(chatman, "chatman.rs", "Rust", "DeltaS/Receipt types, seal/verify, BLAKE3 digest")
    Container(lease, "lease.rs", "Rust", "Tick monotonic counter, exhaustion trap, Phi_lease bytes")
    Container(sealer, "Sealer trait", "Rust", "PQC seam; Unsealed impl loudly typed")
  }
  Rel(chatman, lease, "consumes one tick per seal, reads Phi_lease bytes")
  Rel(chatman, sealer, "sign(blake3_digest || algorithm_id)")
  Rel(lease, chatman, "trap on exhaustion")
```

## 8. Chicago Test Plan with Anti-Vacuity Mutations

Chicago discipline: real collaborators, assert on final state; no mocks. The
court suite must be non-vacuous — each court is paired with a mutation that
must make it FAIL.

1. **Monotonicity court.** Tick a real lease N times through the public API;
   assert the observed tick sequence is strictly increasing and each
   `Phi_lease` byte string differs. *Anti-vacuity mutation*: add a
   `set_tick`/rewind path (SHIM-COURT-marked) — the court MUST fail. A court
   that passes with the rewind shim in place is VACUOUS GATE, exit 1.
2. **Exhaustion court.** Construct a lease with `tick_ceiling = k`, tick `k`
   times, then tick once more; assert the process traps (panic/wasm trap
   observed via catch-unwind harness or wasm trap return — real behavior,
   not a log line). *Anti-vacuity mutation*: downgrade the trap to a warning
   return; the court MUST fail. A court satisfied by a warning is vacuous.
3. **Seal court.** Seal a known delta/lease pair; flip EVERY byte of
   `DeltaS_admitted`, `Phi_lease`, and `blake3_digest` in turn (exhaustive
   byte sweep, deterministic, no randomness-as-coordination) and assert
   `verify` fails for each flip. *Anti-vacuity mutation*: make `verify`
   compare only the first byte of the digest; the sweep MUST catch it.
4. **Chaining court.** Feed prior receipts into `admissions` of a new delta;
   assert a receipt cannot verify under a different lease's `issued_root`.
5. **Determinism court.** Same inputs, same bytes — receipt encoding is
   byte-identical across runs (no nondeterministic serializer).

## 9. Definition of Done (8+)

1. This lane doc written, <=100-char lines, H1 matches subject.
2. Byte-level definitions for `DeltaS_admitted`, `Phi_lease`, `R_receipt`
   present (Sections 3.1-3.3) — no symbol left as decoration.
3. The one unresolved byte-level question (key rotation, 3.4) marked UNKNOWN,
   not decided-by-prose.
4. ADR with candidate table, selection criteria, rejected alternatives, and
   the UNSUPPORTED/blocker path (Section 4).
5. Trap-vs-error semantics stated with the A17 trap-discipline citation
   (Section 5).
6. Receipt format states exactly what BLAKE3 covers and what the PQC seal
   covers (Section 6).
7. C4 context + container diagrams present and consistent with the types
   (Section 7).
8. Chicago test plan with >=3 courts, each with a named anti-vacuity mutation
   and a vacuity consequence (Section 8).
9. `cgap-crypto` marked PROPOSED throughout; no code written by this lane;
   no git commands run.
10. Risks section (Section 10) names the no_std blocker path honestly.

## 10. Risks

- **no_std PQC availability (primary).** Pure-Rust ML-DSA with
  `no_std` + `wasm32-unknown-unknown` support may not exist in a maintained,
  reviewed form. Honest blocker path: `Sealer` trait + loudly-typed
  `Unsealed` receipts + BLOCKED receipt naming the exact crate gap
  (Section 4, UNSUPPORTED clause). Never a silent downgrade to classical-only.
- **FIPS finalization drift.** FIPS 204/205 parameter names/encodings have
  shifted across drafts; pin the parameter set id in `algorithm_id` bytes so
  old receipts stay interpretable.
- **Signature size vs Parquet page layout.** ML-DSA-65 receipts (~2.5 KB) may
  not fit lane A06/A05's page assumptions; coordinate before crate landing.
- **Trap-on-wasm semantics** differ across runtimes; the exhaustion court must
  assert the wasm trap, not a hosted-panic proxy (lane A17 owns the shared
  trap vocabulary).

## See Also

- `docs/jira/v26.11.1/lanes/A05-powercut-wal.md` (durable artifact discipline)
- `docs/jira/v26.11.1/lanes/A03-git-merkle.md` (root digest conventions)
- lane A17 trap discipline doc (Squadron 4)
- `~/.claude/rules/operating-doctrine.md` (A = mu(O*), R = receipt(A))
- `~/.claude/rules/testing-chicago-style.md` (court + anti-vacuity law)
