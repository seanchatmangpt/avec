# A17: P28 Derivative Attestation — crate `cgap-p28` (PROPOSED)

- **Lane**: A17 of 20, Squadron 4, overnight-campaign doc fan-out, v26.11.1
- **Status**: PROPOSED (design lane; no `crates/cgap-p28` exists yet)
- **Files in scope (PROPOSED)**: `crates/cgap-p28/src/attestation.rs`,
  `crates/cgap-p28/src/trap.rs`
- **Coordinator owns**: workspace `Cargo.toml` membership, any edit outside
  this lane.
- **Read-only receipt basis**: `/Users/sac/cgap/LICENSE-P28-SIL.md` (canonical
  P28-SIL instrument, URN
  `urn:avec:license:p28-sil:v2.1.0-sovereign` — FROZEN per
  `docs/jira/v26.11.1/03-rename-receipt.md` decision **D2**: "License URN
  KEEP — LICENSE*.md untouched"), `/Users/sac/cgap/crates/cgap-blake3`
  (in-repo zero-dependency BLAKE3, 256-bit output — the digest substrate this
  lane consumes; same basis as A06).

## 1. Context (ADR-004: in-band attestation; boundary invariant 5)

The P28-SIL instrument establishes that every machine execution deriving from
the AVEC sovereign substrate yields an immutable, content-addressed
cryptographic proof of its own traversal (LICENSE-P28-SIL.md, Preamble: *res
ipsa loquitur* — the topological trace of computation is self-authenticating
evidence under FRE 902(13)/(14)). Today that proof obligation is discharged
out-of-band: license compliance is a repository-level fact (`LICENSE*.md`
files), invisible to the compiled artifact that actually executes.

ADR-004 inverts this: **the attestation travels in-band with the binary**.
Every derivative artifact (ELF binary, WASM module, CGAP frame, parquet
file) embeds an 80-byte `P28LicenseAttestation` record that names its
license lineage cryptographically. Boundary invariant 5 then makes the
attestation load-bearing: a loader that cannot validate the invariant must
not admit the artifact. This lane owns the attestation record itself plus
the spoliation trap that enforces invariant 5 at load time.

What this lane is NOT:

- **Legal enforceability is OUT of scope.** This is a technical mechanism —
  digests, link sections, and a halt path. It makes tampering *detectable
  and fail-closed*, not legally actionable. No claim is made that
  `ERR_P28_SPOLIATION` has any legal effect; the instrument's own estoppel
  theory lives in LICENSE-P28-SIL.md, not here.

## 2. The 80-byte P28LicenseAttestation (Wave 1 ABI freeze — locks with A01/A06/A11)

```rust
#[repr(C, align(8))]
pub struct P28LicenseAttestation {
    pub license_magic: [u8; 4],             // b"P28S"
    pub version: u16,                        // 1
    pub attestation_flags: u16,              // bitfield, below
    pub safe_harbor_ceiling_cents: u64,      // 5_000_000_000 = $50M
    pub origin_authority_digest: [u8; 32],   // BLAKE3, Section 3
    pub upstream_receipt: [u8; 32],          // BLAKE3 of parent receipt
}

const _: () = assert!(size_of::<P28LicenseAttestation>() == 80);
const _: () = assert!(align_of::<P28LicenseAttestation>() == 8);
```

### 2.1 Byte-offset table (MANDATORY — frozen at Wave 1)

| Offset | Size | Field | Type / constraint |
|---|---|---|---|
| 0 | 4 | `license_magic` | `[u8;4]` = `'P','2','8','S'` (0x50 0x32 0x38 0x53) |
| 4 | 2 | `version` | `u16` LE, currently `1` |
| 6 | 2 | `attestation_flags` | `u16` LE bitfield (Section 2.2) |
| 8 | 8 | `safe_harbor_ceiling_cents` | `u64` LE; MUST be <= 5_000_000_000 |
| 16 | 32 | `origin_authority_digest` | `[u8;32]`, BLAKE3 (Section 3) |
| 48 | 32 | `upstream_receipt` | `[u8;32]`, BLAKE3 of parent; zero only if Root |
| 80 | — | end | `size_of == 80`, `align_of == 8` asserted at compile time |

Total: exactly 80 bytes, no padding holes (offsets 0-7, 8-15, 16-47, 48-79
are fully occupied; the `align(8)` guarantees the record itself starts on an
8-byte boundary inside any link section).

### 2.2 `attestation_flags` bit assignment

| Bit | Name | Meaning |
|---|---|---|
| 0 | `FLAG_ROOT` | derives directly from the P28-SIL instrument; receipt all-zero |
| 1 | `FLAG_DERIVATIVE` | derives from a Root artifact; `upstream_receipt` MUST be nonzero |
| 2 | `FLAG_TRANSPILED` | transpile target (beam -> WASM); receipt = transpiler's output receipt |
| 3-15 | reserved | MUST be 0; nonzero reserved bits are a spoliation condition (Section 6) |

Exactly one of bits 0-2 MUST be set; a record with zero or 2+ lineage bits
set is malformed = spoliated (Section 6).

## 3. `origin_authority_digest` derivation (deterministic, replayable)

```
origin_authority_digest =
  BLAKE3( canonical_serialization(P28-SIL instrument header) )
```

- Input: the header block of `/Users/sac/cgap/LICENSE-P28-SIL.md` — the
  first 5 lines (H1 through the `Operational Framework` line, through the
  first `---` separator), UTF-8 bytes, exactly as on disk.
- Hash: BLAKE3, 256-bit output, via the in-repo `cgap-blake3` crate. No
  external hash dependency; identical to A06's substrate, so all
  v26.11.1 lanes derive digests from one hash identity.
- The URN `urn:avec:license:p28-sil:v2.1.0-sovereign` is the *de-referenced
  identifier* and is FROZEN under D2 — the digest is computed over the
  instrument bytes, and the URN is carried out-of-band (documentation,
  registry) as the de-reference target, matching the receipt's D2/D10
  distinction between the kept license URN and renamed RDF IRIs.
- The canonical byte span MUST be pinned by SHA-256 of
  `LICENSE-P28-SIL.md` in this lane's receipt when implemented, so the
  digest is replayable against the exact instrument revision.

## 4. `.p28_invariant` link section (ELF) and WASM custom section

The attestation is carried as a named binary section so loaders can find it
without parsing program content.

### 4.1 ELF

- Section name: `.p28_invariant`, type `SHT_PROGBITS`, flags
  `SHF_ALLOC` (read-only at runtime), `sh_addralign = 8`.
- Contents: exactly the 80-byte record, Section 2 byte layout, native
  endianness fields per the table (all multi-byte fields little-endian to
  match the wire spec).
- Emission: a `#[used] #[link_section = ".p28_invariant"] static
  P28_ATTESTATION: P28LicenseAttestation` in the deriving crate, plus a
  linker-script fragment pinning the section as alloc + readonly so
  `--gc-sections` cannot drop it.
- A loader walks `readelf -S` (or `elf` crate parse) looking up
  `.p28_invariant`; absence is a validation failure (Section 6), not a
  silent pass.

### 4.2 WASM

- Custom section, name `p28_invariant` (no leading dot — WASM custom
  sections take a bare name; the ELF name keeps the dot per ELF naming
  convention).
- Contents: identical 80 bytes, so one parser serves both formats
  (`attestation.rs::from_bytes`).
- Emission: for the cgap-beam -> WASM transpile path (FLAG_TRANSPILED),
  the transpiler copies the parent's attestation, sets bit 2, and replaces
  `upstream_receipt` with the transpiler's output receipt — the copy is a
  byte-preserving transform plus two field edits, testable as a pure
  function (Section 8).

### 4.3 Other carriers (sibling-lane hooks)

- **Frame header (A04)**: the frame header reserves an attestation slot;
  A04 owns the offset. `cgap-p28` provides
  `from_bytes`/`to_bytes` and the validator; A04 embeds.
- **Parquet footer (A02)**: same — A02's footer carries a
  `p28_invariant` KV metadata entry holding the 80 bytes;
  `cgap-p28` supplies the codec and the court, A02 the container slot.

## 5. Validation semantics (trap.rs)

Validation is total and ordered; first failure wins, all failures map to
the same halt class so attackers cannot fingerprint the failing check:

1. Section present (`find_in_elf` / `find_in_wasm`) — absence fails.
2. `license_magic == b"P28S"` — mismatch fails.
3. `version == 1` — unknown version fails (forward incompat is fail-closed).
4. `attestation_flags`: exactly one of bits 0-2 set; bits 3-15 all zero.
5. FLAG_ROOT xor `upstream_receipt == [0;32]` consistency.
6. `origin_authority_digest` recomputed from the pinned instrument bytes
   matches.
7. `upstream_receipt` verified against the parent artifact's receipt (only
   when FLAG_DERIVATIVE/FLAG_TRANSPILED).
8. `safe_harbor_ceiling_cents <= 5_000_000_000`.

Any failure at any step => spoliation (Section 6). There is no
"degraded-but-continue" mode; boundary invariant 5 admits or halts.

## 6. Spoliation trap: halt, not errno

**Exit gate**: mutating 1 bit of the P28 header or stripping the parent
receipt triggers immediate kernel halt
`ERR_P28_SPOLIATION = 0x503238FF`.

### 6.1 Why halt rather than errno

- An errno return is advisory: the caller can ignore it, log it, and keep
  running the spoliated artifact. Boundary invariant 5 requires admission
  to be **fail-closed** — the only lawful outcome of a failed invariant
  check is that the artifact never executes another instruction.
- C-ABI surface (for the `cgap-ffi` consumers; A19 owns the FFI contract):
  the trap is exported both as a check-then-halt function and as a value
  the embedder can poll:

```c
/* Halt the process immediately if `att` fails validation.
   Never returns on spoliation; returns 0 iff valid. */
uint32_t cgap_p28_invariant_check(const uint8_t att[80]);

#define ERR_P28_SPOLIATION 0x503238FFu  /* "P28" + 0xFF sentinel */
```

  `0x503238FF` reads as ASCII `'P' '2' '8' 0xFF` — the magic is carried in
  the error constant so a crash dump identifies the trap without symbols.
  In WASM the same check compiles to `unreachable` after writing the
  constant to a known linear-memory location, so the embedder traps with
  the same discriminator. A19's FFI lane consumes this; this lane owns the
  constant.

### 6.2 Trap path

`attestation.rs` owns parse + validate (pure, total, `#[no_panic]`-style:
no allocation, no I/O); `trap.rs` owns the halt:
`std::process::abort()` on native (SIGABRT with the constant staged in a
register / global for the crash handler), `unreachable` in WASM. The split
exists so the pure validator is testable Chicago-style with real bytes and
the halt is a thin, separately-mutable edge.

## 7. Chicago test plan with anti-vacuity mutations (the spoliation court)

Real bytes, real artifacts, real halts — no mocks. Every court asserts on
final state (process exited / artifact admitted), never on call counts.

### 7.1 Bit-flip sweep court (the anti-vacuity core)

- **Corpus**: build one valid Root attestation and one valid Derivative
  attestation; embed each in a real ELF (via the link-section static) and
  a real WASM module.
- **Mutation**: for each of the 640 header bits (80 bytes x 8), flip that
  single bit in the section bytes, re-embed, and re-run validation in a
  real subprocess. 640 mutations x 2 records x 2 formats.
- **Law**: **every** single-bit mutation must halt. A mutated record that
  validates is a hole in the invariant; the sweep reports
  `bit (byte, bit) = (i, j)` survived.
- **Anti-vacuity**: a halt-free sweep (or a sweep harness that skips
  mutations) is the vacuous case and MUST exit nonzero. Corollary — a trap
  removed by mutation must be caught by the sweep: if a refactor disables
  the halt path, the sweep's subprocesses stop dying, the harness observes
  640 live artifacts, and the court fails loudly. The court cannot pass
  with the trap absent; that is the point.
- **Known non-halting case, handled explicitly**: flipping a bit in
  `safe_harbor_ceiling_cents` that keeps it under the ceiling still passes
  step 8 — so the court pins the ceiling at its maximum
  (5_000_000_000): any ceiling-bit flip then fails the `<=` check. The
  sweep uses the max-ceiling record precisely so all 640 bits are
  load-bearing. If a future field addition creates genuinely redundant
  bits, the sweep must be re-derived, not weakened.

### 7.2 Stripping court

- Remove `.p28_invariant` from the ELF (section-table rewrite with a real
  ELF tool path) / drop the custom section from WASM -> loader must halt
  (step 1).
- Parent-receipt stripping (Derivative record with `upstream_receipt`
  zeroed) -> halt (steps 5/7).

### 7.3 Positive controls

- Untampered Root + Derivative records validate and admit, both formats.
- The transpile transform (Section 4.2) is asserted as pure function
  equality: output record = parent record with bit 2 set and receipt
  replaced.
- Determinism: two independent derivations of `origin_authority_digest`
  from the pinned instrument bytes are byte-identical (replayability).

### 7.4 Court mechanics

- Subprocess-per-mutation (real process death, no in-process halting that
  a test harness could absorb); harness collects exit statuses; assert
  all 640 deaths carry the staged `0x503238FF` discriminator where the
  platform exposes it.
- Any `[SKIP]` (e.g. no linker on the platform) is loud and counted, never
  a silent pass.

## 8. Definition of Done (8+)

1. `crates/cgap-p28` exists with `attestation.rs` + `trap.rs`; workspace
   membership added by coordinator.
2. `#[repr(C, align(8))]` struct compiles with the 80-byte and align-8
   static asserts passing (`cargo build` exit 0).
3. Byte-offset table (Section 2.1) matches the compiled layout, verified
   by a layout test asserting every field offset from the table.
4. `origin_authority_digest` derived from pinned `LICENSE-P28-SIL.md`
   bytes via `cgap-blake3`; determinism test green; instrument SHA-256
   recorded in the lane receipt.
5. ELF `.p28_invariant` emission + `find_in_elf` round-trip on a real
   built artifact.
6. WASM custom-section emission + round-trip on a real built module.
7. Bit-flip sweep: 640/640 mutations halt, per record per format; harness
   exits 0 only on full kill; halt-free or partial sweep exits nonzero
   (anti-vacuity witnessed by injecting a "skip mutations" shim and
   watching the court fail).
8. Stripping court: section-removed -> halt; `upstream_receipt` zeroed ->
   halt; both witnessed as real subprocess deaths.
9. Positive controls: untampered records admit; transpile transform
   asserted by pure-function equality.
10. `ERR_P28_SPOLIATION = 0x503238FF` exported from the C-ABI header and
   staged at halt; A19 FFI lane acknowledges the constant.
11. Wave 1 ABI freeze signed off with A01/A06/A11 (offsets + flag bits +
   ceiling constant locked).

## 9. Risks and boundaries

- **Legal enforceability is OUT of scope.** The trap is a technical
  fail-closed mechanism; it does not, and claims to do nothing about,
  contractual remedy, jurisdiction, or estoppel. The instrument's legal
  theory stays in `LICENSE-P28-SIL.md`.
- **Redundant bits break the sweep.** Any field whose mutation is
  semantically tolerated (the ceiling case, Section 7.1) creates sweep
  holes; the mitigation is pinning values at their boundary so every bit
  is load-bearing. Future fields must satisfy "all 640 bits halt" or the
  sweep law must be explicitly restated.
- **Loader adoption.** The trap only protects artifacts loaded through a
  `cgap-p28`-aware loader; a hostile loader can ignore the section. This
  is accepted: invariant 5 binds the substrate's own admission path, not
  third-party loaders.
- **Instrument drift.** The digest is pinned to one instrument revision
  via SHA-256; a P28-SIL edit invalidates every attestation. That is
  correct behavior (fail-closed) but requires a re-attestation run on any
  license edit — coordinated via D2 (license files untouched).
- **Endianness.** Fields are LE by spec; big-endian targets must byteswap
  before validate, else magic/ceiling checks misfire (still fail-closed,
  but diagnose via offset table first).

## See Also

- `docs/jira/v26.11.1/03-rename-receipt.md` (D2: license URN frozen;
  D10: `urn:avec:` -> `urn:cgap:` RDF-only amendment)
- `/Users/sac/cgap/LICENSE-P28-SIL.md` (canonical instrument)
- Sibling lanes: A01/A06/A11 (Wave 1 ABI freeze), A04 (frame header
  attestation slot), A02 (parquet footer), A19 (C-ABI/FFI), A06
  (`cgap-blake3` digest substrate)
