# A13: BN3 Native Builtins — log:/math:/crypto: as `BUILTIN_OP` sigil dispatch

- **Lane**: A13 of 20, Squadron 3, overnight-campaign doc fan-out, v26.11.1
- **Status**: PROPOSED (design lane; `crates/cgap-bn3` does not exist yet)
- **Files in scope (PROPOSED)**: `crates/cgap-bn3/src/builtins.rs`
- **Coordinator owns**: workspace `Cargo.toml`, everything outside this lane.
- **Read-only receipt basis**:
  - `/Users/sac/cgap/crates/cgap-blake3/src/lib.rs` — the real hash surface,
    read this session (`hash` :197, `hash_many_simd` :824,
    `hash_wide_simd` :696, `compress_scalar` :77,
    `GPU_CROSSOVER_BYTES` = 16 MiB).
  - `docs/jira/v26.11.1/lanes/A11-bn3-arena.md` — **sigil authority**. A11
    owns `sigil.rs` and freezes the sigil inventory; this lane consumes the
    `0x04 BUILTIN_OP` sigil ("dense builtin opcode, dense index, not a
    `SlotId`", A11 §4.2) and defines only the opcode space it indexes into.
    Any sigil renumber is A11's, not ours.
  - `docs/jira/v26.11.1/lanes/A06-bont-scalars.md` — scalar vocabulary that
    `math:` results land back into as `SlotId` payloads.

## 1. Context: builtin as sigil dispatch, not function pointer

A11 defines the BN3 term as one `u64` word: `sigil<<56 | payload(56 bits)`.
The `0x04 BUILTIN_OP` sigil is the single sanctioned route from a term to a
native operation. The term-word form of a builtin application is:

```text
BUILTIN_OP word:  sigil<<56 | payload(56 bits)
payload layout:   [opcode:u8 | dst:u8 | a:u8 | b:u8]  (register ids R0..R255)
```

The evaluator performs one `match` on the opcode byte and executes the arm
directly on the register file. There is no `Vec<Box<dyn Builtin>>`, no
`fn(&[u64]) -> u64` table, no registry object — a builtin is an arm of a
`match`, same as A11's sigil decode, which is itself a `match`.

Against the W3C N3 baseline (builtins as named predicates resolved through
a dictionary at eval time): dictionary resolution is a hash lookup per
evaluation; the sigil form pays a few bits in the term word and zero at
eval time.

## 2. ADR: closed registry, compile-time dispatch

**Decision**: the builtin registry is **closed** and dispatched at compile
time via `match opcode { ... }` on the u8 opcode. The full registry must
fit u8 (256 opcodes, dense assignment, namespace-gapped, Section 3).

Rejected alternatives:

1. **Rejected — `HashMap<&str, BuiltinFn>` name registry.** Per-call hash
   plus a function-pointer call the compiler cannot inline; the registry is
   runtime state, so "what builtins exist" is not visible to the compiler
   and not court-able from emitted code.
2. **Rejected — `dyn` builtin trait objects.** Vtable indirection on the
   hot path; each call is an indirect branch; the module stops being
   trivially auditable.
3. **Rejected — open registration API (`register_builtin!`).** An open
   registry lets two crates claim the same opcode and the collision is
   silent. A closed `match` on u8 makes duplicate opcode literals a hard
   compile error, so opcode uniqueness is guaranteed by the compiler, not
   by review.
4. **Rejected — 16-bit opcode space.** The 56-bit payload budget packs
   opcode + three register ids; widening the opcode steals register-
   addressing bits for no catalog need (9 of 256 codes used).

### 2.1 Zero-indirection claim, honestly graded

The exit gate claims "zero function-pointer indirection". Status: **the
claim is structural today, UNVERIFIED as an emitted-code fact.** The
`match`-on-u8 construction makes function-pointer dispatch impossible by
construction — there are no `fn` values in the design to indirect through.
But "the compiler emits no jump table of function pointers" is a property
of rustc/LLVM output, not of our source. Blocking fact: `crates/cgap-bn3`
does not exist, so there is no build artifact to run `nm`/`objdump`
against. Section 5.3 defines the court that flips the status; until then
the claim stands at UNVERIFIED with that blocking fact.

## 3. Builtin catalog (the full registry — only what this lane specs)

Registers are `R0..R255` (A11/A12's file; this lane addresses, A12
adjudicates semantics). Dense assignment, namespace-gapped: log `0x01`-`0x03`,
math `0x10`-`0x13`, crypto `0x20`-`0x21`. Total: 9 of 256 u8 codes; 247
codes remain for future namespaces with no renumber pressure.

### 3.1 `log:` namespace

| Op | Name | Signature | Semantics | Failure mode |
|----|------|-----------|-----------|--------------|
| 0x01 | `log:trace` | `(dst)` | Trace-sink write; result = input (identity) | None |
| 0x02 | `log:assert` | `(dst, exp)` | `R[dst]==exp` else typed err; ok -> dst=1 | `AssertFailed` |
| 0x03 | `log:counters` | `(dst)` | Latch eval counter into dst; identity | None |

### 3.2 `math:` namespace (operands are 56-bit unsigned payloads)

| Op | Name | Signature | Semantics | Failure mode |
|----|------|-----------|-----------|--------------|
| 0x10 | `math:greater_than` | `(dst, a, b)` | `R[dst] = (a > b) as u64` (0 or 1) | None — total |
| 0x11 | `math:sum` | `(dst, a, b)` | `R[dst]=a+b`, checked (§7 law) | `Overflow { a, b }` |
| 0x12 | `math:mul10` | `(dst, a)` | `R[dst]=a*10`, checked; b must be 0 | `Overflow`/`ReservedOp` |
| 0x13 | `math:min` | `(dst, a, b)` | `R[dst] = min(a, b)` | None — total |

### 3.3 `crypto:` namespace

| Op | Name | Signature | Semantics | Failure mode |
|----|------|-----------|-----------|--------------|
| 0x20 | `crypto:blake3` | `(dst, ptr, len)` | BLAKE3-256 via `hash`; to dst | `InvalidRange` |
| 0x21 | `crypto:blake3_wide` | `(dst, ptr, len)` | SIMD `hash_many_simd`; = 0x20 | `InvalidRange` |

Exit-gate mapping: `MathGreaterThan` = 0x10, `MathSum` = 0x11,
`CryptoBlake3` = 0x20 — the three execute directly on registers through the
Section 2 dispatch.

## 4. Mermaid C4 (level 2)

```mermaid
C4_Component
  title A13 builtins inside cgap-bn3
  Component(builtins, "builtins.rs", "closed u8 registry, match dispatch")
  Component(sigil, "sigil.rs (A11)", "0x04 BUILTIN_OP decode authority")
  Component(arena, "arena.rs (A11)", "register file / slab layout")
  Component(unify, "unifier (A12)", "eval loop; adjudicates register discipline")
  Component(blake3, "cgap-blake3", "hash / hash_many_simd substrate")
  Rel(builtins, sigil, "consumes BUILTIN_OP sigil, defines opcode space")
  Rel(builtins, arena, "reads/writes registers")
  Rel(unify, builtins, "calls dispatch(opcode, regs)")
  Rel(builtins, blake3, "calls hash/hash_many_simd for 0x20/0x21")
```

## 5. Chicago test plan (real collaborators, no mocks)

All tests execute the real dispatch against a real register array. The
collaborator is the real `builtins` module and the real `cgap-blake3`
crate — never a stub.

### 5.1 Known-answer tests per builtin

Every builtin has a known-answer test asserting final register state:

- `math:greater_than`: `(3,2)->1`, `(2,3)->0`, `(3,3)->0`, bound
  `(2^56-1, 0)->1`.
- `math:sum`: `(1,2)->3`; overflow pair `(2^56-1, 1)` -> `Err(Overflow)`;
  `(2^56-1, 0) -> 2^56-1` (no wrap, no silent clamp — checked, typed).
- `math:mul10`: `floor(Max/10)` -> `Ok`; `floor(Max/10)+1` ->
  `Err(Overflow)`; exact final register values asserted.
- `math:min`: total; bounds `0` and `Max`.
- `crypto:blake3`: input `b"abc"` -> exact digest
  `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`,
  asserted both against the literal and against a direct
  `cgap_blake3::hash(bytes)` call (byte-for-byte).
- `crypto:blake3_wide`: 4 KiB input through dispatch vs direct
  `hash_many_simd` — byte-identical or the lane fails.
- `log:assert`: success writes 1 to dst; failure returns `AssertFailed`
  carrying the actual register value.
- `log:trace` / `log:counters`: identity; counters latch observed through
  the real eval-loop counter struct.

### 5.2 Dispatch court (collision anti-vacuity)

- A `const ALL_OPCODES: [u8; N]` lives in the same module as the `match`;
  a test asserts it has no duplicates. A duplicate literal arm in the
  `match` is additionally a compile error, so collision detection is
  doubly fenced: compiler + test.
- **Mutation 1 (must-kill)**: change `math:sum`'s opcode constant `0x11`
  to `0x10` — build must fail or the `ALL_OPCODES` test must fail. A court
  that passes under this mutation is VACUOUS.
- **Mutation 2 (must-kill)**: delete the `math:min` arm — dispatch of
  `0x13` must return `Err(BN3Error::UnknownOpcode(0x13))` (typed refusal,
  never a fallback-to-identity); test asserts the typed error.
- **Mutation 3 (must-kill)**: swap `crypto:blake3` and
  `math:greater_than` opcode constants — every known-answer test must fail
  (digest mismatch + boolean inversion), proving the known-answer tests
  are load-bearing.

### 5.3 Zero-indirection court (emitted code)

Blocking fact: **this court cannot run today** — no crate, no artifact.
When it can:

- `cargo build --release`; disassemble the object containing `dispatch`
  (`objdump -d` / `nm`).
- Assert: no indirect `call` whose target is loaded from a data-section
  table of distinct function symbols in the dispatch region. Intra-function
  branch tables (jump tables to basic blocks) are allowed and expected for
  a 9-arm match; the gate bans tables of *function* pointers only.
- An indirect call through a data table of function addresses downgrades
  the claim to PARTIAL, with the disassembly lines cited in the receipt.
- Until the artifact exists: **UNVERIFIED**, blocking fact stated in §2.1.

## 6. Definition of Done

1. `crates/cgap-bn3/src/builtins.rs` exists; workspace `cargo build` green.
2. Closed registry: 9 opcodes, dense, namespace-gapped, fits u8; the
   registry table derives from the dispatch `match` (single source).
3. `math:greater_than`, `math:sum`, `crypto:blake3` execute directly on the
   register file via the u8 match — no `dyn`, no `fn` values, no registry
   object (source-level fact, cited by the match itself).
4. All §5.1 known-answer tests green, including the real-digest blake3
   check against `cgap_blake3::hash`.
5. Dispatch court green; all three §5.2 anti-vacuity mutations killed.
6. Zero-indirection court executed against the release artifact, OR the
   claim marked UNVERIFIED with the blocking fact stated (§5.3).
7. Typed refusals only: `UnknownOpcode(u8)`, `Overflow { a, b }`,
   `AssertFailed`, `InvalidRange`, `ReservedOperand` — no panic path.
8. `cargo build` with `-D warnings` green; no `unsafe` introduced here.
9. No hash logic re-implemented: 0x20/0x21 call the real
   `cgap_blake3::{hash, hash_many_simd}` surface.

## 7. Risks

- **Overflow semantics in 56-bit payloads (top correctness risk).** Two
  56-bit operands decoded as u64 sum below 2^57, so `checked_add` alone
  never fails on inputs that overflow the 56-bit ceiling — the check is
  `checked_add` AND a `> 0x00FF_FFFF_FFFF_FFFF` range check. The §5.1
  overflow known-answer pins both arms of the check.
- **CryptoBlake3 needs cgap-blake3.** Dependency read and real:
  `crates/cgap-blake3/src/lib.rs` is `#![no_std]`, zero-dependency, and
  exposes `hash(&[u8]) -> [u8; 32]` (:197), `hash_many_simd` (:824),
  `hash_wide_simd` (:696), `compress_scalar` (:77). `(src_ptr, src_len)`
  must resolve to a real byte range of the arena slab; out-of-bounds is
  `InvalidRange`, never a wild read (slice construction is bounds-checked
  by Rust). `GPU_CROSSOVER_BYTES` = 16 MiB dispatch is out of scope —
  register-resident payloads sit far below crossover. Coordinator owns the
  workspace `Cargo.toml` edge; the `b"abc"` digest known-answer doubles as
  the integration fence for this dependency.
- **A11 sigil authority.** Opcodes live inside the payload, so the coupling
  to A11's sigil numbering is one-way: an A11 sigil renumber does not move
  our opcode space.
- **`log:` failure modes are honest.** trace/counters cannot fail; only
  assert can.
- **Wave 3 sequencing.** Lands after A11 (arena/sigil) and A12 (unifier,
  register-discipline authority); until A12 accepts the register-addressing
  scheme, §5 tests are provisional.

## 8. Standing

- PROPOSED. Nothing executed this session; no build artifact exists.
- Sources read this session: `crates/cgap-blake3/src/lib.rs` (pub surface,
  line numbers above), `docs/jira/v26.11.1/lanes/A11-bn3-arena.md` (sigil
  inventory §4.2, `BUILTIN_OP` = 0x04).
- Zero-indirection exit gate: **UNVERIFIED** (no artifact to disassemble;
  §5.3 is the court that flips it).
