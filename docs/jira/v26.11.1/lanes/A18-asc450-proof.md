# A18: ASC 450-20 / SOX 404 Liability-Proof Emitter — `cgap-p28` (PROPOSED), `asc450.rs`

Lane: A18 of 20, v26.11.1 overnight campaign. Repo: `/Users/sac/cgap` (single
canonical checkout; no worktrees, no git commands from this lane). Wave 3
window. File ownership: this document only; the crate `cgap-p28` it specifies
is PROPOSED — A17 holds crate authority for `cgap-p28`; this lane specifies
one module (`asc450.rs`) inside that PROPOSED crate and cites A17 as the
crate authority for all crate-level decisions (manifest, license header,
module registry, attestation format).

## 1. Status and Standing

- Lane doc: WRITTEN (this file).
- Crate `cgap-p28`: PROPOSED, authority A17.
- Module `crates/cgap-p28/src/asc450.rs`: PROPOSED — no code in this lane's
  deliverable.
- Exit gate (design-level, enforced later by the court): unlicensed ingestion
  whose accumulated value crosses `safe_harbor_ceiling_cents` (default
  $50,000,000 = `5_000_000_000 cents`) emits a cryptographically signed GAAP
  contingent-liability proof — one proof per crossing, tamper-evident,
  replayable from the proof bytes alone.
- Every claim below is a design claim (UNVERIFIED until the sweep court runs).
  Nothing in this file asserts executed verification.

## 2. Scope & Non-Scope (read first)

**This mechanism emits a SIGNED TECHNICAL RECEIPT recording a threshold
crossing. It is not legal advice, it is not a legal opinion, and it does not
by itself create, admit, or settle any liability.** Whether a contingency is
probable, reasonably possible, or remote — and whether any amount is estimable
— is a judgment reserved to the entity's management and its auditors under
ASC 450-20; SOX 404 control assertions remain the entity's own. What this
module does mechanically: it meters a defined ingestion stream against a
configured ceiling, and when the ceiling is crossed it produces a signed,
tamper-evident record of that crossing for whatever downstream human or
machine process (audit prep, disclosure committee workflow, ledger
annotation) chooses to consume it. The proof's claim set is deliberately
narrow and stated in the record itself: usage counters, ceiling, crossing
sequence number, digest chain, signature. Nothing in the record asserts
probability, estimability, or legal character.

The non-scope statement above does not weaken the mechanism spec. The
mechanism is fully specified: deterministic threshold semantics, exact
boundary behavior (emit at `total > ceiling`, never at `total == ceiling`),
byte-exact record format, digest chaining, signature, and boundary-exact
anti-vacuity courts. The legal questions are out of scope BY CONSTRUCTION —
the record format simply does not carry fields for them.

## 3. Context

ADR-004's consequence — that every durable cgap artifact carries a signed
attestation and that admission is receipted — extends here from build
artifacts to usage accounting: if ingestion volume itself is an admitted
fact, then the moment it crosses a legal-signal threshold is also a fact
deserving a receipt. Under ASC 450-20, a loss contingency that is both
probable and reasonably estimable must be accrued and disclosed; under SOX
404, the controls over the information feeding that judgment must be
evidenced. Today that evidence is produced by hand: someone reads a usage
dashboard, notices a threshold, and writes a memo. The memo is exactly the
class of artifact the receipt discipline exists to replace — a claim with no
identity, no replay, and no standing. The mechanism's value is that the
threshold crossing becomes an observation with a receipt, not a recollection.

The honest boundary (Section 2) is part of the design, not a disclaimer
bolted on: because the record does not carry probability or estimability
fields, it cannot be mistaken for an accrual memo. It is evidence FOR the
accrual process, not a substitute for it.

Dependencies on sibling lanes (both are authority cites, not reads — their
files may not exist yet in this fan-out):

- **A16** — the sealer (signature authority) and the tick-based time source
  (the no_std time lease). This module must not call any wall-clock API; all
  timestamps in the proof record are tick counters obtained through A16's
  lease.
- **A17** — `cgap-p28` crate authority and the P28 attestation format whose
  digest chain this module extends.

## 4. ADR: Threshold-Crossing Proof Emitter

### 4.1 Decision

`cgap-p28::asc450` implements a **no_std usage accumulator with
checked, overflow-rejecting accounting; boundary-exact threshold semantics;
and one-shot signed proof emission per crossing**:

- `Asc450Meter::new(safe_harbor_ceiling_cents: u64)` — a per-entity meter
  holding: `accumulated_cents: u64`, `event_count: u64`,
  `last_seq: u64`, and A16's tick lease.
- `record_licensed(amount_cents: u64, tick: u64)` — adds to the accumulator
  but NEVER emits; licensed ingestion is out of the gate's scope by
  definition (the gate watches unlicensed ingestion only).
- `record_unlicensed(amount_cents: u64, tick: u64) -> Option<SignedProof>`
  — adds `amount_cents` to the accumulator with checked arithmetic; returns
  `Some(proof)` exactly when the accumulator transitions
  from `<= ceiling` to `> ceiling`.
- `SignedProof` is a fixed-layout byte record (Section 5), signed by A16's
  sealer.

### 4.2 Boundary semantics (exact)

Let `prev = accumulated_cents` before the event, `next = prev + amount`:

- If `prev <= ceiling` and `next > ceiling`: EMIT. The proof records `next`
  as `crossing_total_cents`, the event's `tick` as `crossing_tick`, and the
  event's sequence number as `crossing_seq`.
- If `next == ceiling` exactly: NO emit (the ceiling is a safe harbor; being
  at it is still inside it).
- If `prev > ceiling` (already crossed): NO re-emit; the meter is latched.
  Subsequent unlicensed events still accumulate (checked) and bump
  `last_seq` so the post-crossing exposure is visible in the meter state,
  but emit nothing.
- Overflow: `prev + amount` is computed with checked arithmetic; on overflow
  the event is REJECTED (typed error, accumulator unchanged). Saturation is
  rejected as a design option: it would silently understate exposure near
  the exact boundary this module exists to watch, so a saturated add must
  never be able to mask a crossing. Rejection is the loud failure; silence
  is the one this lane exists to prevent.

### 4.3 Overflow and counter semantics in no_std

- All counters are `u64` with checked arithmetic; `event_count` and
  `last_seq` wrap-reject identically (reject on overflow, state unchanged).
- No allocation anywhere in the emitter path. `SignedProof` is
  `#[repr(C, packed)]` fixed-size (Section 5), produced into a caller
  buffer or returned by value — no `Vec`, no `String`.
- No wall clock: `tick` values arrive as arguments from A16's tick lease.
  The module NEVER reads time itself; a test asserts no time-source call is
  reachable from the emitter path (compile-level: the module does not depend
  on any time crate).

### 4.4 Latching and counter-reset attacks

The latch is the reset-attack surface: an adversary who can reset the
accumulator to zero after a crossing silently un-crosses. Countermeasures,
in strength order:

1. **Digest chain to the P28 attestation (A17)**: the proof embeds
   `attestation_digest` (Section 5) and `prev_proof_digest` (digest of the
   previous proof this meter emitted, or the meter's genesis digest). A
   reset must also forge a chain, not just zero a counter.
2. **One-shot latch with monotone seq**: `crossing_seq` and
   `prev_proof_digest` make a post-reset emission produce a proof whose
   chain does not link to the original — detectable by any consumer holding
   both proofs.
3. **Sealed meter state**: A16's sealer signs a meter-state digest on every
   emission and on every explicit `seal_state()`; a consumer can demand a
   fresh sealed state at any time, and a reset between two sealed states
   shows as a non-monotone `accumulated_cents` at equal-or-higher `last_seq`
   between two valid signatures.

Reset is never *prevented* in-process (the process can always be killed and
restarted at zero) — it is made *detectable*, which is the achievable and
honest claim. The falsifier for the whole section: a reset followed by
re-crossing MUST be distinguishable from the original crossing by bytes
alone.

### 4.5 Who consumes the proof (non-goals fenced)

Consumers read; none may write the meter. Named consumers:

- Audit-prep workflow (reads proofs, renders disclosure-committee packets).
- Ledger annotation process (records proof digest as a ledger note).
- Court harnesses (Section 6) — the only writer in tests.

No consumer computes liability. The record does not carry fields that would
let one (Section 2).

### 4.6 C4 architecture (Mermaid)

Context level — the meter sits between the ingestion stream and the
audit-side consumers:

```mermaid
C4Context
    title A18: ASC 450-20 threshold-proof emitter — system context
    Person(auditor, "Disclosure committee / auditor",
        "consumes signed crossing proofs; never writes the meter")
    System(cgap, "cgap-p28::asc450",
        "no_std unlicensed-ingestion meter; emits one signed GAAP
        contingent-liability proof per safe-harbor crossing")
    System_Ext(ingest, "Ingestion volume stream",
        "per-event (amount_cents, licensed flag, A16 tick)")
    System_Ext(a16, "A16 sealer + tick lease",
        "Ed25519-class signatures; tick time; sealed meter state")
    System_Ext(a17, "A17 P28 attestation",
        "attestation digest anchoring the proof chain")
    Rel(ingest, cgap, "record_licensed / record_unlicensed events")
    Rel(a16, cgap, "signs proofs; provides ticks; seals meter state")
    Rel(a17, cgap, "attestation_digest at meter genesis")
    Rel(cgap, auditor, "SignedProof bytes (168 B) + sealed state")
```

Container level — meter, emitter, and the proof path:

```mermaid
C4Container
    title A18: containers inside cgap-p28::asc450
    ContainerDb(meter, "Asc450Meter", "no_std struct",
        "checked u64 accumulator + event_count + last_seq + latch
        + prev_proof_digest")
    Container(emitter, "Proof emitter", "no_std fn",
        "emit iff prev <= ceiling && next > ceiling; latched thereafter")
    Container(seal, "Seal bridge", "trait",
        "A16 sealer over proof bytes 0..104; seal_state() digests")
    Container(proof, "SignedProof", "168 B packed record",
        "ceiling, crossing total/seq/tick, attestation + chain digests,
        signature")
    Rel(ingest_ext, meter, "events with tick", "function call")
    Rel(meter, emitter, "crossing transition")
    Rel(emitter, seal, "request signature")
    Rel(seal, proof, "writes signature field")
    Rel(proof, auditor_ext, "byte-exact record, replayable from bytes")
```

(External systems `ingest_ext` and `auditor_ext` are the stream and the
auditor from the context diagram; the container diagram shows only the
emission path through them.)

## 5. Proof Record Format (exact bytes)

Fixed layout, little-endian, `#[repr(C, packed)]`, total 168 bytes. All
multi-byte integers little-endian; digest = BLAKE3 32-byte; signature per
A16's sealer (64 bytes, Ed25519-class). Magic `A450` is bytes
`0x30 0x35 0x34 0x41` little-endian.

| offset | len | field | source |
|---|---|---|---|
| 0 | 4 | magic `A450` | const |
| 4 | 2 | `record_version = 1` | const |
| 6 | 2 | reserved = 0 | const |
| 8 | 8 | `ceiling_cents` | meter config |
| 16 | 8 | `crossing_total_cents` | first `next > ceiling` |
| 24 | 8 | `crossing_seq` | meter's monotone event seq |
| 32 | 8 | `crossing_tick` | A16 tick-lease value |
| 40 | 32 | `attestation_digest` | BLAKE3 of A17 P28 attestation |
| 72 | 32 | `prev_proof_digest` | prior proof digest (genesis `[0u8; 32]`) |
| 104 | 64 | `signature` | A16 sealer over bytes 0..104 |

Any byte flip anywhere in bytes 0..168 must fail signature verification.
Court C6 (Section 6) asserts `size_of::<SignedProof>() == 168`, the field
offsets above, and the signed span 0..104, so doc-to-code layout drift is a
test failure, never a silent mismatch.

## 6. Chicago Test Plan with Anti-Vacuity Courts

Real collaborators, no mocks: tests run real meter instances and a REAL
sealer — the sealer from A16's crate as a workspace dependency, or, if
A16's crate does not yet exist in this fan-out, a real Ed25519 sealer over
the same interface (a hand-written real interface implementation is not a
mock). Assert on final state (proof bytes), never call counts.

- **C1 — boundary-exact threshold court**: drive the meter to exactly
  `ceiling` with unlicensed events — assert no proof, meter unlatched. Then
  +1 cent — assert `Some(proof)`, `crossing_total == ceiling + 1`, meter
  latched. Must FAIL any `>=` implementation (at ceiling exactly it would
  wrongly emit).
- **C2 — signature court**: flip each byte in bytes 0..168 of a real
  emitted proof in turn; every flip must fail verification. Also: a proof
  whose `signature` field alone is replaced by an equal-length random
  signature must fail. This is the tamper-evidence law: any byte flip
  anywhere fails.
- **C2b — signed-state monotonicity (reset-attack) court**: emit at
  crossing, take sealed state S1; record 1 more unlicensed cent, seal S2;
  reset accumulator to 0, record 1 cent, seal S3. S3's `accumulated_cents`
  is lower than S2's at higher seq under a valid signature — the
  non-monotone triple proves reset detection works; a consumer holding
  S1..S3 detects the reset by bytes alone (Section 4.4).
- **C3 — negative-control court (never-emit mutant killer)**: plant a real
  over-ceiling run (feed real events summing above ceiling) and assert
  emission. Suite-level law: the suite MUST fail under the never-emit
  mutation and MUST pass unmutated; a suite that survives the never-emit
  mutation exits 1 as VACUOUS GATE.
- **C4 — latch court**: post-crossing events emit nothing (latch holds)
  while `last_seq`/accumulator still advance (no silent drop of
  post-crossing exposure); a second crossing after a legal reset carries a
  `prev_proof_digest` chain that does not link to the original.
- **C5 — overflow court**: event that would overflow the accumulator
  (amount near `u64::MAX`) is rejected with a typed error; accumulator and
  seq unchanged; no emission. Must FAIL any saturating implementation
  (saturation would mask a crossing near the boundary).
- **C6 — layout court**: instantiate `SignedProof`, assert
  `size_of == 168`, field offsets equal the Section 5 table, and re-derive
  the signed span (0..104).
- **C7 — no-wall-clock court**: ticks arrive as arguments (A16 lease); the
  module never reads time itself. Compile-level: no time-crate dependency
  from `asc450.rs`; introducing `SystemTime`/`Instant` anywhere in the
  emitter path fails. Behavioral: identical tick sequences in produce
  byte-identical proofs out (determinism law).

## 7. DoD (8+)

1. `crates/cgap-p28/src/asc450.rs` specified to the byte-layout and court
   level in this doc; crate manifest and crate-level decisions deferred to
   A17 (authority).
2. Normative record layout (168 bytes) fixed in Section 5; C6 enforces it.
3. Boundary law `emit iff prev <= ceiling && next > ceiling` fixed; C1
   enforces it boundary-exactly.
4. Overflow law: checked add, reject-and-leave-unchanged; C5 enforces.
5. Latch + chain + sealed-state reset detection specified; C2b/C4 enforce.
6. No-allocation, no-wall-clock no_std constraints stated; C7 enforces.
7. Scope & Non-Scope section present and non-weakening; the record format
   carries no probability/estimability fields by construction.
8. Court suite C1..C7 specified, each with its own falsifier; suite-level
   vacuity gate (never-emit mutant) via C3.
9. Dependencies on A16 (sealer, tick lease) and A17 (crate authority, P28
   attestation digest) cited as authority cites, not reads.
10. All claims UNVERIFIED standing, nothing asserted as executed.

## 8. Risks

- **Counter reset attacks**: addressed by latch + digest chain + sealed
  monotone states (Section 4.4); never prevented in-process, always
  detectable by bytes — that is the honest, achievable claim.
- **Clock absence in no_std**: solved by A16's tick lease — time enters as
  arguments, never read inside the module; C7 enforces both compile-level
  and determinism.
- **Sealer absence in this fan-out** (A16's crate not yet landed): tests
  bind a real Ed25519 sealer over the same interface; noted in Section 6.
- **Legal overclaim risk**: Section 2 fences it; the record format carries
  no fields that could be read as a legal assertion — overclaim is
  structurally impossible in the artifact, not merely discouraged in prose.
- **Layout drift between doc and code**: C6 re-derives offsets from the
  Section 5 table at test time so drift is a test failure, not a silent
  mismatch.

## 9. Wave

Wave 3. Sibling dependencies cited: A16 (sealer + tick lease), A17
(`cgap-p28` crate authority + P28 attestation digest source). This lane
writes only this file; the crate is PROPOSED.
