# A06: B-ONT v1 Scalar Primitives — crate `cgap-bont` (PROPOSED)

- **Lane**: A06 of 20, Squadron 2, overnight-campaign doc fan-out, v26.11.1
- **Status**: PROPOSED (design lane; no `crates/cgap-bont` exists yet)
- **Files in scope (PROPOSED)**: `crates/cgap-bont/src/primitives.rs`,
  `crates/cgap-bont/src/oabi.rs`
- **Coordinator owns**: `Cargo.toml` workspace membership, any edit outside this lane.
- **Read-only receipt basis**: `/Users/sac/cgap/Cargo.toml` (workspace, edition 2021,
  `#![no_std]` convention), `/Users/sac/cgap/crates/cgap-blake3/src/lib.rs`
  (in-repo zero-dependency BLAKE3 with 256-bit root output — the hash substrate
  this lane consumes; no external blake3 dependency needed).

## 1. Context (ADR-002: fixed-width binary ontology, zero runtime strings)

B-ONT ("Binary ONTology") is the v26.11.1 representation layer in which every
ontology term is a fixed-width integer derived once at build time from its
lexical form. Rationale:

- **No runtime strings on the hot path.** RDF/OWL pipelines pay comparison,
  hashing, and allocation costs per IRI touch. B-ONT terms compare in one
  machine instruction and are `Copy`.
- **Wire and memory parity.** The in-memory scalar IS the on-disk scalar; the
  `.oabi` pack format (Section 6) is a raw reinterpretation of a `#[repr(C)]`
  struct, no codec step.
- **Replayability.** A term's identity is a pure function `lexeme -> scalar`
  via BLAKE3 (already vendored in-repo as `cgap-blake3`), so any producer
  (A09/A10) regenerates identical IDs from the same lexeme corpus without a
  shared registry service.

This lane defines the four scalar primitives and the pack container. It does
NOT implement producers (A09/A10 own graph emission).

## 2. Scalar inventory (the ABI this lane locks in Wave 1)

| Type | Width | Alignment | Meaning |
|---|---|---|---|
| `SlotId` | `u64` (8 B) | 8 | term slot = `truncate64(BLAKE3(lexeme))`, Section 4 |
| `ClassId` | `u32` (4 B) | 4 | dense class index assigned by the build-time producer |
| `EnumId` | `u16` (2 B) | 2 | dense enumeration-variant index |
| `LexHash` | `[u8; 32]` | 1 | full 256-bit BLAKE3 digest of the lexeme (pre-truncation) |

Design decisions:

- `SlotId` is the *sparse*, collision-vetted identity (64-bit space, Section 5).
  `ClassId`/`EnumId` are *dense* producer-assigned indices — small enough for
  direct-indexed tables (class vtables, enum dispatch) and cheap enough to
  embed in `.oabi` records. `LexHash` is carried so a consumer can verify a
  `SlotId` against the lexeme without re-reading the string table.
- `LexHash` is the truncation *preimage witness*: `SlotId = low64(LexHash)`.
  Carrying both is what makes the deterministic court (Section 7) non-vacuous.

## 3. Rejected alternatives

1. **RDF IRIs as strings (`&str`/`String`/`Box<str>`).** Rejected: per-compare
   cost O(len), heap allocation per term, non-`Copy`, unbounded width breaks
   the `#[repr(C)]` ABI and the `.oabi` fixed-record layout. Also rejected the
   intermediate form (offset+length into a string table) — still forces a
   memcmp on every equality and makes zero-copy FFI (cgap-ffi) lossy.
2. **Runtime string-interning table** (global `RwLock<HashTable<Arc<str>>>`).
   Rejected: introduces a global contention point, an init-order dependency
   for `no_std` targets, and a registry service dependency that breaks the
   "regenerate IDs from lexemes offline" replay property. Interning is fine
   at build time (producers do it); the runtime must not need it.
3. **128-bit `SlotId` (full `u128` truncation of BLAKE3).** Rejected: doubles
   `.oabi` record size for collision headroom the namespace scale (Section 5)
   does not require; u128 is not a single-register compare on 64-bit targets.
4. **SipHash/xxHash instead of BLAKE3.** Rejected: non-cryptographic hashes
   make collision analysis (Section 5) unverifiable under adversarial lexemes
   and add a second hash implementation where `cgap-blake3` already exists.

## 4. Truncation specification

```
LexHash  = BLAKE3(lexeme_bytes)                  // 32-byte root digest
SlotId   = u64::from_le_bytes(LexHash[0..8])     // FIRST 8 bytes, little-endian
```

- **Which 64 bits**: bytes `0..8` of the BLAKE3 root digest, interpreted
  little-endian. BLAKE3's root output is uniformly distributed across the full
  256 bits; any fixed window is equivalent in collision profile, so the
  *prefix* window is chosen purely for human debuggability — `SlotId` matches
  the leading hex of `LexHash` when both are rendered (LE rendering keeps the
  u64 load single-instruction; the hex-prefix readability argument holds for
  the LE byte order, not the numeric order — this is an accepted cosmetic
  asymmetry).
- **Why not a folding mix (xor of hi/lo halves)**: folding doubles the code on
  the FFI boundary for zero collision benefit and breaks `SlotId`-is-a-prefix
  debuggability.
- **Frozen**: the truncation window is part of the Wave 1 ABI freeze. Shifting
  the window silently remaps every `SlotId` in every pack; the determinism
  court (Section 7) exists to make that failure loud.

## 5. Collision analysis obligation (shown math, birthday bound)

`SlotId` space N = 2^64 ≈ 1.845e19. Birthday collision probability for n
distinct lexemes:

```
P(collision) ≈ 1 - exp(-n(n-1)/(2N))
  n = 1e6  (typical ontology):       n²/2N ≈ 2.7e-8
  n = 1e8  (pathological corpus):    n²/2N ≈ 2.7e-4
  n = 6.1e9:                          P ≈ 50%
```

Obligations this lane imposes on producers (A09/A10) and on the pack loader:

- Producers MUST detect collisions at build time: build a `LexHash -> SlotId`
  map; any two distinct lexemes yielding equal `SlotId` is a **hard build
  error** naming both lexemes (fail-closed, never silent truncation drift).
- The `.oabi` loader MUST re-derive `SlotId` from `LexHash` for every record
  and refuse the pack on mismatch (guards truncated-window drift across
  producer versions; this is what makes the Section 7 mutation court
  end-to-end rather than unit-local).
- Lexeme domain-separation: hash the tagged bytes `b"bont\x01" ++ lexeme` so a
  future `SlotId` namespace bump cannot collide with an old namespace's IDs.
  The tag byte `\x01` is reserved for namespace v1 and is part of the freeze.

## 6. `.oabi` pack format v1 sketch (container this lane defines; A09/A10 produce)

```
offset  size  field
0       8     magic          = b"OABIv1\0\0"
8       4     header_len     (u32 LE) = 64
12      2     version_major  = 1
14      2     version_minor  = 0
16      4     flags          (bit0: little-endian; bit1: LexHash present)
20      4     class_count    (u32 LE)  — ClassId space is [0, class_count)
24      4     enum_count     (u32 LE)  — EnumId space is [0, enum_count)
28      4     slot_count     (u32 LE)  — SlotId records follow
32      4     record_size    (u32 LE) = 48 (SlotId + ClassId + EnumId + pad + LexHash)
36      28    reserved       (zero; must be zero to load)

-- then slot_count × record (48 B each, 8-byte aligned):
  SlotId   (8) | ClassId (4) | EnumId (2) | pad (2) | LexHash (32)
```

- Fixed 48-byte records; no varints in v1. Padding is explicit so
  `#[repr(C)]` layout == on-disk layout byte-for-byte.
- Loader checks: magic, `header_len == 64`, `record_size == 48`,
  `slot_count` consistent with section length, `SlotId == truncate64(LexHash)`
  per record, reserved bytes zero.
- Consumers may mmap the record region and reinterpret as
  `&[OabiRecordV1]` — this is why every width below is asserted, not assumed.

## 7. Static assertions (exit gate) — code excerpts

`primitives.rs` (compile-time, zero-cost):

```rust
use static_assertions::{assert_eq_align, assert_eq_size, const_assert};

#[repr(C, align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SlotId(pub u64);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ClassId(pub u32);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EnumId(pub u16);

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LexHash(pub [u8; 32]);

// ---- ABI freeze: any widening here must fail the build ----
assert_eq_size!(slot_id_is_u64, SlotId, u64);
assert_eq_size!(class_id_is_u32, ClassId, u32);
assert_eq_size!(enum_id_is_u16, EnumId, u16);
assert_eq_size!(lex_hash_is_32b, LexHash, [u8; 32]);
assert_eq_align!(slot_id_align8, SlotId, u64);
assert_eq_align!(class_id_align4, ClassId, u32);
assert_eq_align!(enum_id_align2, EnumId, u16);

const_assert!(core::mem::size_of::<SlotId>() == 8);
const_assert!(core::mem::align_of::<LexHash>() == 1);
```

`oabi.rs` (record layout == pack layout):

```rust
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct OabiRecordV1 {
    pub slot: SlotId,
    pub class: ClassId,
    pub enum_: EnumId,
    pub _pad: [u8; 2],
    pub lex: LexHash,
}

assert_eq_size!(oabi_record_is_48b, OabiRecordV1, [u8; 48]);
assert_eq_align!(oabi_record_align8, OabiRecordV1, u64);
const_assert!(OABI_HEADER_LEN == 64);
const_assert!(OABI_RECORD_SIZE == 48);
```

Derivation (deterministic, `const`-friendly call path into `cgap-blake3`):

```rust
pub const BONT_NS_V1: &[u8] = b"bont\x01";

pub fn slot_id(lexeme: &[u8]) -> SlotId {
    let mut h = cgap_blake3::hasher();
    h.update(BONT_NS_V1);
    h.update(lexeme);
    let LexHash(d) = LexHash(h.finalize_32());
    SlotId(u64::from_le_bytes(d[0..8].try_into().unwrap()))
}
```

(Exact `cgap-blake3` API surface — `hasher`/`finalize_32` naming — is
confirmed against `crates/cgap-blake3/src/lib.rs` at implementation time;
the lane pins the *behavior*, not the method names.)

## 8. Chicago test plan (real collaborators, anti-vacuity mutations)

Chicago discipline: real lexeme corpora and real byte buffers, assert on
final state; no mocks (nothing here needs one — every collaborator is an
in-process pure function or an in-memory byte slice).

**T1 — Determinism court (anti-vacuity: fails if the truncation window
shifts).** For a fixed corpus (>= 1,000 real lexemes, including empty, 1-byte,
non-UTF8, and >1 MiB), assert `slot_id(l)` equals a table of golden `u64`
values generated once and committed. A truncation-window change
(`d[0..8]` -> `d[8..16]`) or a namespace-tag change remaps every value and
fails the court. *Vacuity check*: the court must FAIL under the mutation
"window shifted by 8 bytes" and under "namespace tag dropped" — verified by
running the shimmed build before accepting the court.

**T2 — Size courts (anti-vacuity: fails on any field widening).** A
byte-level round-trip: serialize a known `OabiRecordV1` to `[u8; 48]` via
`core::mem::transmute`-free `unsafe { slice::from_raw_parts }` over a
`#[repr(C)]` value and compare against a committed 48-byte golden hex blob.
Any field widening changes the blob. *Vacuity check*: mutation "ClassId
u32 -> u64" must fail both the compile-time `assert_eq_size!` gate and T2.

**T3 — Pack loader refusal court.** Real in-memory `.oabi` buffers, mutated
per case: (a) bad magic, (b) `header_len != 64`, (c) `record_size != 48`,
(d) one record's `SlotId` != `truncate64(LexHash)`, (e) nonzero reserved
bytes. Loader must return a typed error naming the field for each; a loader
that accepts any mutated buffer is FAIL.

**T4 — SlotId = truncate64(BLAKE3(lexeme)) exit gate.** Independent
re-derivation: for N corpus lexemes, compute BLAKE3 via `cgap-blake3`,
apply the Section 4 spec by hand in the test (not by calling `slot_id`),
and assert equality with `slot_id(l)`. Catches spec-vs-impl drift (e.g.
someone "optimizing" by hashing without the namespace tag — T1 catches the
value change, T4 catches the impl no longer matching the written spec).

**T5 — Producers' obligation fixture.** A collision-detector fixture: feed a
synthetic corpus of two crafted lexemes (chosen so their 64-bit prefixes
agree — search over suffixes at test time using the real hash, accept the
few-ms cost) and assert the detector reports both lexemes as a hard error.

## 9. Definition of Done (8+)

1. `crates/cgap-bont` exists in the workspace, `#![no_std]`, zero
   dependencies except `cgap-blake3` (workspace path dep).
2. `SlotId`/`ClassId`/`EnumId`/`LexHash` defined `#[repr(C)]` with all
   Section 7 static asserts compiling.
3. `OabiRecordV1` + header constants compile with 48-byte/64-byte layout
   asserts; doc-comment carries the Section 6 offset table.
4. `slot_id(lexeme)` implemented with namespace tag `b"bont\x01"` and
   LE prefix truncation; T4 spec-rederivation test passes.
5. T1 determinism court passes against committed golden table, and its
   anti-vacuity mutations (window shift, tag drop) are witnessed failing.
6. T2 size court passes against committed golden blob; u32->u64 widening
   mutation witnessed failing the compile gate.
7. T3 pack-loader refusal court covers all five mutation cases with typed
   errors; zero mutated buffer accepted.
8. T5 collision-detector fixture finds a real prefix collision pair and
   reports a hard error naming both lexemes.
9. `cargo build -p cgap-bont` and `cargo test -p cgap-bont` clean in the
   shared checkout; lane leaves no build-root artifacts.
10. ABI widths (SlotId u64, ClassId u32, EnumId u16, LexHash 32 B) signed
    off with A01/A11/A17 in the Wave 1 freeze; sign-off recorded in
    `RESOLUTIONS.md` by the coordinator.

## 10. Risks

- **Collision probability** (Section 5 math): negligible at ontology scale
  (2.7e-8 at 1e6 lexemes) but *unbounded* if producers skip the build-time
  collision check on adversarial corpora — the obligation is load-bearing,
  hence T5. Residual risk after the check: zero (check is exhaustive over
  the actual corpus).
- **Truncation-window drift across producer versions**: silent total remap.
  Mitigated by T1 golden court + T3 per-record loader re-derivation
  (defense in depth: unit-level and wire-level).
- **`cgap-blake3` API surface mismatch**: the lane pins behavior, not method
  names; if `finalize_32`-style streaming differs, adapt the call, never the
  spec. Risk is implementation friction only, not correctness.
- **ABI freeze coupling**: this lane locks ClassId/EnumId widths that A01/A11
  embed. If the freeze slips past Wave 1 (00:00-02:00), downstream lanes
  inherit UNKNOWN widths — escalation path is coordinator resolution, not
  local widening.
- **Endianness**: v1 packs are little-endian only (flags bit0). Big-endian
  targets load via explicit byte-swap path; out of scope for v1 but the
  flag bit is reserved now to avoid a v2 header break.

## 11. Wave 1 (00:00-02:00) — ABI freeze with A01/A11/A17

- This lane locks: `SlotId = u64`, `ClassId = u32`, `EnumId = u16`,
  `LexHash = [u8; 32]`, truncation window `d[0..8]` LE, namespace tag
  `b"bont\x01"`, `.oabi` v1 header (64 B) and record (48 B).
- A01/A11/A17 consume these widths; any widening after the freeze is a
  breaking ABI change requiring a new lane, not a patch.
- Deliverable by 02:00: widths + truncation spec agreed in `RESOLUTIONS.md`;
  implementation (Sections 7-8) may land later in the wave per coordinator
  scheduling.

## See Also

- `crates/cgap-blake3/src/lib.rs` (hash substrate, read-only basis)
- `Cargo.toml` (workspace membership — coordinator-owned)
- Sibling lanes: A01, A09 (producer), A10 (producer), A11, A17
