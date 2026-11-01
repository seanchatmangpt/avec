# CC=1 Branch Register — audit-vacuity sweep, 2026-10-29

Scoping law: CC=1 binds data-plane inner-loop bodies. Loop latches,
dispatch, bounds checks, error/release paths are control flow — itemized,
never hidden. Classes: A true violation, B hidden kernel, C control path,
D equivalent. All evidence is compiled-instruction level (xcrun llvm-objdump
over target/audit + target/release artifacts, rustc 1.97).

## R1. The original 9/9 PASS was vacuous-by-wrapper

All 6 loop-bearing wrappers (`_audit_mask_words/count_valid/unpack_1bit/
2bit/4bit/8bit`) compile to 4–5-instruction shims ending in an unconditional
tail `b` into mangled callees (`__RNvNtCs9DwzS8jbwzX_10cgap_arrow6bitmap10mask_words`
@0xf20 etc.) the audit never scanned; the COND regex deliberately does not
match bare `b`. The "MISSING SYMBOLS" guard checked wrapper existence only,
laundering the vacuity. **Class B (audit defect, not kernel defect).** Fixed
by the callee-extent rewrite (scripts/branch_audit.py, in flight at this
writing; independent sweeps reproduce its per-symbol budgets exactly).

## R2. True kernel branch budgets (18:33 dylib, independently double-counted)

| Kernel (true callee) | latch | body | dispatch | note |
|---|---|---|---|---|
| bitmap::mask_words @0xf20 | 2 | 0 | 3 | LLVM auto-vectorized (4x16b AND) |
| bitmap::count_valid @0xfc0 | 2 | 0 | 3 | NEON popcount body branch-free |
| silicon::mask_words_neon @0x10d4 | 3 | 0 | 4 | scalar tail re-vectorized by LLVM |
| unpack_1bit/2bit/4bit | 4 | 3 | 1 | + `bl panic_bounds_check` (C) |
| unpack_8bit | 6 | 9 | 1 | + panic bl (C) |

3 leaf kernels genuinely branch-free: get_bit, set_bit, is_inline
(4-insn sub+lsr). No A-class finding inside any audited extent.

## R3. Uncovered kernels (must be named by the gate)

- `Bitmap::is_all_valid` (bitmap.rs:174), `count_one_bits` (bitmap.rs:200):
  real validity scans, 3 cond/iter (2 D-class provably-untaken bounds + 1
  latch) + per-iter out-of-line `bl word`. GC'd from every audit artifact
  (no audit-crate caller). Routed to the gate lane for wrapping or explicit
  UNCOVERED naming.
- cgap-parquet: **zero** symbols were ever audited. Compiled findings:

| Site | file:line | Class | per-lane conditionals |
|---|---|---|---|
| decode_into literal fill | rle.rs:229 | B | 2 (bounds, provably untaken but issuing every lane) |
| rle_encode literal fill | rle.rs:122 | B | 2 (pad select runtime-taken + bounds) |
| unpack scalar tails / w=8 unrolled | unpack.rs | B | 1/lane; w=8: 8/word |
| unpack_bits_tight lane loop | packing.rs:63 | clean | 0 (strongest kernel) |
| SWAR vector bulk loops | unpack.rs | clean | 0 (latch only) |
| record dispatch / width guards | rle.rs:188, unpack.rs:80 | C | per record/call |

The one genuinely data-dependent per-bit branch (rle_encode's bit test)
compiles to CSEL/SIMD selects — the branchless-arithmetic claim is real.
The branchy residue is bounds checks inside per-lane loops.

## R4. Claim corrections landed (source + docs)

- bitmap.rs header: "zero conditional branches" → "branch-free inner-loop
  bodies; latches/dispatch itemized" (was falsified by the symbol-level
  disassembly).
- lib.rs headline: same scoping.
- silicon.rs: "tail adds exactly one latch" → compiled reality is 3 latches
  + 4 dispatch (LLVM re-vectorized the tail).
- README/charter/receipts wording to follow the same scoping (gate lane's
  output is the enforcement; docs lanes transcribe).

## R5. Class counts (sweep total)

A: 0 (no branch inside any audited/claimed inner-loop body).
B: 9 findings — 6 wrapper-vacuity (audit defect), is_all_valid/count_one_bits
uncovered, 3 parquet per-lane-bounds sites under the blanket "branchless"
crate claim.
C: per-record dispatch, per-word driver bounds, builder/finish loops,
release null-checks, Varlen::row bounds, offsets_monotonic (debug_assert-
gated validation loop).
D: is_inline (proven 4-insn), pack_bits capacity branch (exact
with_capacity), Bitmap::word bounds, is_all_valid/count_one_bits in-loop
bounds.

No claim in the repo survives as "zero conditional branches" without the
body/latch/dispatch scoping. The honest claim, now enforced by the gate:
**inner-loop bodies are branch-free; all other conditionals are itemized.**

## Addendum — truth-pass transcription complete, 2026-10-29 (lane REL-L11, repo v26.10.29)

Fresh `make branch-audit` this session, exit 0, over `target/audit/libcgap_audit.dylib`:

- The R1 callee-extent rewrite has landed. Per-symbol budgets reproduce R2 exactly:
  mask_words 2/0/3, count_valid 2/0/3, mask_words_neon 3/0/4, unpack_1/2/4bit 4/3/1
  each, unpack_8bit 6/9/1 — and R3's two uncovered kernels are now wrapped:
  is_all_valid 1/2/4, count_one_bits 1/2/4 (2 `slice_index_fail` fail-paths each),
  with the gate printing "UNCOVERED KERNELS: none — every cgap kernel extent is
  audited". R3's parquet `rle.rs:229` / `rle.rs:122` / `packing.rs:63` sites remain
  OUTSIDE the audited symbol set (the audit crate wraps the parquet `unpack` family
  only); the R5 class counts are unchanged by this session.
- Verbatim summary line: "BRANCH AUDIT PASS: branch-free inner-loop bodies: 3/9
  kernels; loop-latch/dispatch/bounds conditionals itemized: 71 total across 9
  kernels (+12 branch-free wrapper shims); panic/fail paths: 8; unaccounted: 0".
- R4's docs transcription is complete: `README.md` and `docs/DFLSS_CHARTER.md` now
  carry the scoped wording everywhere (no unscoped "zero branches"/"branchless"
  claim remains in either); this register stays the claim of record.
