# Branchless Predication Kernel ABI & Audit Gate

Status: UNVERIFIED — this document specifies; nothing herein has been executed or audited.

Scope: kernel-level ABI that operationalizes M=1 (single conditional-materialization
mode) across the v26.10.31 hot-kernel surface. Extends the branchless topology and
in-loop branch elimination discipline established in AVEC-107/AVEC-108 (v26.10.29)
to the current surface; audit items only, no performance or correctness claims.

## 1. Per-Architecture Predication Primitives

Every data-dependent decision in a kernel MUST lower to one of the primitives
below. Each primitive is a selection/transpose/masking op with no control-flow
edge: its M=1 property is that both value alternatives are evaluated and one is
chosen by data, so the instruction stream is invariant under input predicates.

### ARM64 (AArch64)

| Primitive | Purpose | M=1 property |
|---|---|---|
| `CSEL Rd, Rn, Rm, cond` | 2-way register selection | both inputs computed; cond picks, no branch |
| `CSINC Rd, Rn, Rm, cond` | select-with-increment idiom (`cset`-family lowering) | data-selected, constant latency |
| `CSETM Rd, cond` | all-ones/-zeros mask materialization | predicate becomes vectorizable mask data |
| `VZIP1/2Q` (NEON) | interleave lanes of two vectors (quad transpose) | lane permute only — no predicated edge |
| `VUZP1/2Q` (NEON) | de-interleave lanes (inverse transpose) | same: pure shuffle |
| `VTRN` (NEON) | 2x2 lane transpose | pure shuffle, compile-time-indexed |

The `CSETM -> AND/ORR/EOR` chain is the canonical branchless equality-mask form:
compare once, widen to a mask, then combine masks with bitwise ops.

> Correction (2026-10-30, ADR-17): the NEON ZIP/UZP transpose kernel this
> section's primitive table names was specified, never built — the shipped
> transpose is fixed-stride scalar (see 22-adr-register.md ADR-17).

### x86_64

| Primitive | Purpose | M=1 property |
|---|---|---|
| `CMOVB/CMOVNZ/CMOVAE` | 2-way register selection on flags | both inputs computed; data-selected |
| `VPUNPCKLDQ/H` | 32-bit lane interleave (transpose halves) | pure lane shuffle |
| `VPERM2I128` | 128-bit-lane permute across YMM/ZMM | full-lane reshuffle, no edge |
| `VPMASKMOV(D/Q)` | masked load/store under a computed mask | fault-suppressed, address-invariant stream |

Masked loads/stores keep the memory access pattern input-shape-invariant; the
mask is data, not control flow.

### WASM

| Primitive | Purpose | M=1 property |
|---|---|---|
| `i32.select` / `i64.select` | scalar 2-way select | both operands on stack; no branch target |
| `v128.bitselect` | lane-wise select under a v128 mask | mask is data; full-width both-ways eval |
| `v128.shuffle` (imm) | compile-time lane permutation | static shuffle indices — transpose/masking |

WASM shuffles take immediate lane indices, so transposes lower to straight-line
shuffle sequences with zero reachability variance.

## 2. Banned Instruction Classes in Hot `.text`

Within designated kernel sections (Section 5 inventory), the following are banned:

| Arch | Banned | Note |
|---|---|---|
| x86_64 | `jmp`, full `jcc` family (`je jne jg jl jge jle ja jae jb jbe js jns jo jno jp jnp jc jnc`) | control transfer on data = M>1 |
| ARM64 | `b`, `b.cond`, `cbz`, `cbnz`, `tbz`, `tbnz` | same class |
| both | — | `ret` permitted as the single exit of each kernel |

Loop constructs are permitted only with compile-time trip counts realized as
unrolled straight-line code. Any dynamic loop back-edge in a designated kernel
section is a gate violation.

## 3. Disassembly Audit Gate (CI)

A CI job must, per supported architecture:

1. Build the kernel object files (the manifest in Section 5).
2. Disassemble designated sections with `llvm-objdump -d` (fallback `objdump -d`).
3. Assert zero banned opcodes from the Section 2 table appear in designated
   sections. Zero banned opcodes = pass; any hit = fail with the offending
   instruction and address in the job log.
4. Fail-closed: missing tool, missing object, or unparseable disassembly is a
   failure, not a skip.

### Positive-control (anti-vacuity)

An audit that cannot detect an injected jump is vacuous. The gate must include a
mutation test: copy one kernel object, inject a branch (e.g. patch a `nop` to a
conditional branch, or relink a mutated source copy), run the gate on the copy,
and assert the gate fires. If the mutated copy passes, the gate exits nonzero
itself and reports VACUOUS GATE. The gate's own pass requires both:
clean objects pass, mutated copy fails.

## 4. LLVM De-Optimization Hazard

`InstCombine`/`SimplifyCFG` may convert `select` idiom chains back into branches
during lowering, silently reintroducing `jcc`/`b.cond`. Mitigations, ranked:

1. **Explicit assembly**: naked functions or `core::arch::asm!` with explicit
   `cmov`/`csel`. The emitted opcodes are the authored ones; nothing for the
   optimizer to re-branch. Highest assurance, highest maintenance cost.
2. **`#[inline(always)]` + `#[target_feature]`**: keep select idioms inside one
   inlined body compiled with the arch feature set, so the select survives
   through to ISel without an outlining edge.
3. **Post-build objdump diff (backstop)**: regardless of 1 or 2, the Section 3
   gate runs on the final artifacts — the disassembled bytes are the verdict,
   never the source intent.

Mitigation choice is per-kernel and recorded in the manifest. The backstop (3)
is mandatory for every kernel.

## 5. Kernel Inventory (Manifest)

The gate applies to exactly these kernels; each must be listed in a manifest
mapping kernel -> owning file -> designated section:

| Kernel | Class | Owning file (manifest entry) |
|---|---|---|
| ladder step (Montgomery/field step) | selection | owner file recorded in manifest |
| quad transpose / unpack | transpose | owner file recorded in manifest |
| equality masks | masking | owner file recorded in manifest |
| offset MSB select | selection | owner file recorded in manifest |
| BLAKE3 lanes | masking/transpose | `crates/avec-blake3/` surface + `crates/avec-arrow/src/blake3.rs` |

The manifest file (path and schema to be assigned by the integration lane) is
the authoritative inventory; this table is the seed. A kernel present in
`lib` hot paths but absent from the manifest is a gate violation on the
inventory-completeness check.

## 6. Remainder Policy (ADR-14)

ADR-14 (`22-adr-register.md`) refines the §2/§3 zero-tolerance reading: hot
data paths carry 0 data-dependent branches; loop latches and assert-guard
branches are **accepted remainder**, documented per-kernel — never silenced.
The gate stays 0-tolerance in the sense that it reports every banned opcode it
sees and can exit 1; what changed is the admission criterion.

The authoritative per-kernel declaration is `scripts/branchless_manifest.toml`
(§5's manifest, path now assigned): per kernel it records the symbol
`scope_pattern` (same unanchored matching the gate's `BRANCHLESS_SCOPE` uses),
the accepted remainder classes (`ALLOCATOR_NULL_CHECK`, `ZERO_TRIP_GUARD`,
`LOOP_LATCH`, `CAPACITY_ASSERT`), an `expected_max_count`, and a `gate_mode`.
For the two currently audited kernels the mode is `INFORMATIONAL` — no
latch-free design exists yet, so counts above `expected_max_count` exit 1,
counts at or below are reported honestly as remainder. Growth of the
accepted-remainder list is a reviewed diff, not a default (ADR-14
consequence). When a latch-free design lands for a kernel, flip its
`gate_mode` to `ENFORCED` and drop `LOOP_LATCH` from its remainder list.

Current audited residuals, verified 2026-10-30 via
`BRANCHLESS_SCOPE='transpose|predicates' scripts/audit_branchless.sh`
(audit profile, aarch64): transpose 12 unique sites (9x allocator cbz +
zero-trip cbz + entry b + latch b.ne; 24 when the rlib and its deps/
duplicate are both counted), filter_quads_eq 2 (capacity assert b.lo +
latch b.ne).

**Manifest parsing amendment (wave 7)**: the earlier note that "the gate
script does not yet parse the manifest" is now FALSE.
`scripts/audit_branchless.sh` fully adjudicates the manifest: awk parse of
`scope_pattern` / `expected_max_count` / `gate_mode` /
`observed_mnemonics` per kernel, `MANIFEST-OK` / `MANIFEST-FAIL` classes
with exit 3 on manifest violation, and a `BRANCHLESS_MANIFEST` env
override for the manifest path. Receipt `gate-branchless-manifest.md` is
ALIVE with a negative control (max_count 2 -> 0 => exit 1).

**Inventory-completeness honesty**: the manifest covers 2 of the §5 seed
kernels (transpose, predicates). `ladder.rs`, the BLAKE3 lanes, and the
offset-MSB select remain unmanifested — an open violation of this doc's
own inventory-completeness law until manifest entries land.

## Standing

UNVERIFIED — no gate job, no mutation run, no disassembly receipt exists for
this spec as of 2026-10-30. Every claim above is specification, not observation.
Partial exception (wave 7): receipts `gate-branchless-selftest-v2.md` and
`gate-branchless-manifest.md` exist for the gate script itself (selftest and
manifest adjudication, including the max_count negative control); per-kernel
audit receipts above remain the standing evidence base.

## See Also

- `scripts/branchless_manifest.toml` — authoritative kernel inventory
- `../v26.10.29/18-m1-branchless-algebra.md`
- `../v26.10.29/AVEC-107-branchless-primitives-topology.md`
- `../v26.10.29/AVEC-108-in-loop-branch-elimination.md`
