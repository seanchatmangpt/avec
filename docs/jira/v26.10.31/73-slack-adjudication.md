# cfg_homology Transpose-Slack Adjudication

Status: ADJUDICATED (2026-10-31, wave-11 lane 3). The live +17 transpose row in
`scripts/cfg_homology.sh` is fully adjudicated per ADR-14: every one of the 19
emitted branch sites maps to a declared remainder class in
`scripts/branchless_manifest.toml`; the normal-mode proxy FAIL is a
proxy/slack mismatch, not an unadjudicated defect. Recommendation: amend the
transpose row slack 4 -> 20 via reviewed diff (observed+1; see §6). The
script itself is NOT edited by this lane.

## 1. What Is Adjudicated

`scripts/cfg_homology.sh` (DMADV Analyze proxy) compares textual source
decision constructs against emitted banned-branch opcodes per declared hot
kernel. Its transpose row:

```text
KERNEL                       SOURCE-CONSTRUCTS    EMITTED   DELTA   SLACK  VERDICT
transpose                              2         19      17       4  FAIL
```

Live re-witness this session (twice, 2026-10-31):

```text
$ bash scripts/cfg_homology.sh      # exit 1, both runs
    [transpose] ...9transpose9transpose: 12 emitted branch(es)
    [transpose] ...11QuadColumns11untranspose: 7 emitted branch(es)
transpose                                 2         19      17       4  FAIL
== PROXY GATE FAIL — see table; fix the kernel or amend slack as a reviewed diff
```

The row's emitted side sums two kernels: `transpose::transpose` (12 sites) and
`QuadColumns::untranspose` (7 sites). The source side is the whole-file textual
count for `crates/avec-arrow/src/transpose.rs` = 2 constructs — exactly the two
`for` loop headers (the file's non-test region has no `if`/`&&`/`||`/`.map(`/
`.filter(` and the doc comments strip out of the counter). Structural fact: the
row is a family row — the unanchored symbol regex `transpose` substring-matches
`untranspose`, and the source side counts one file.

## 2. Evidence Base

| Receipt / artifact | What it witnesses |
|---|---|
| live run this session (exit 1, x2) | transpose row 2/19/+17 vs slack 4; per-symbol 12 + 7 |
| receipts/gate-cfg-homology.md (wave-6) | same +17 row exit 1; slack-amend follow-up never landed |
| receipts/gate-cfg-homology-v2.md | mutant FLIP exit 0: clean delta 4 PASS, branchy 13 FAIL |
| receipts/gate-branchless-manifest-v4.md | audit gate GATE PASS exit 0; 12 sites MANIFEST-OK |
| scripts/branchless_manifest.toml | 8-kernel inventory; per-class remainder + ceilings |
| 69-validation-receipt.md V15 | independent lane: mutant FLIPS; honest-FAILs transpose +17 |
| 68-wave-10-receipt.md G3 | untranspose manifest entry + ADR-17 landed; blind spot armed |

## 3. The 19 Sites, Enumerated by Class

Provenance: `scripts/branchless_manifest.toml` (counts + addresses verified
2026-10-30 by direct disassembly; llvm-objdump 21.1.7 and Apple objdump agree
byte-for-byte) plus the live per-symbol counts above.

### transpose::transpose — 12 sites (manifest "quad transpose / unpack")

| opcode | count | manifest class | note |
|---|---|---|---|
| cbz | 9 | ALLOCATOR_NULL_CHECK | 0x16c..0x284; Vec alloc null-checks the toolchain emits |
| cbz | 1 | ZERO_TRIP_GUARD | 0x150; guard before the loop |
| b | 1 | LOOP_LATCH (entry edge) | 0x228 |
| b.ne | 1 | LOOP_LATCH (latch) | 0x2dc; `cmp count,4*len; b.ne`; no latch-free design yet |

### QuadColumns::untranspose — 7 sites (manifest "quad untranspose")

| opcode | count | manifest class | note |
|---|---|---|---|
| b.ls | 1 | CAPACITY_OVERFLOW_GUARD | `len*64` overflow -> alloc-error path (entry block) |
| cbz | 1 | ALLOCATOR_NULL_CHECK | alloc returned null -> retry/error |
| cbz x2 + cbnz + b | 4 | ZERO_TRIP_GUARD | byte-len==0 skips alloc; len==0 skips copy loop |
| b.ne | 1 | LOOP_LATCH | dynamic back-edge; no latch-free design exists yet |

Total: 12 + 7 = 19 emitted; 2 source constructs; delta +17 > slack 4. The
ZERO_TRIP_GUARD cluster covers both zero-trip edges including the len-0 exit
edge into the epilogue (full text: the manifest's `quad untranspose` entry).

Correction vs the dispatch prompt's "10 alloc cbz": the receipted split is
9 alloc cbz + zero-trip cbz + entry b + latch b.ne = 12
(`branchless_manifest.toml` + 27-branchless-kernel-abi.md §6, verified
2026-10-30 with addresses). The disk receipt is authoritative.

## 4. Why the Proxy FAILs: the Honest Split

The two gates measure different laws on different attribution mechanisms:

- **cfg_homology** (source-construct proportionality): attributes instructions
  by walking the symbol table, so it sees BOTH kernels (12+7=19) and judges
  the delta against a slack derived from source-construct proportionality.
- **audit_branchless** (the ADR-14 law): label-scoped counting sees
  `transpose` (12); the untranspose region renders `<ltmp0>` and lands in the
  loud `skipped_out_of_scope` count — the documented blind spot (ADR-17); the
  manifest entry is armed and adjudicates automatically if a future objdump
  renders the symbol.

The honest split of the two surfaces:

- cfg_homology measures source-construct proportionality (a drift detector:
  emitted branching should track source decisions + bounded compiler
  overhead). It has no notion of ADR-14 remainder classes.
- The audit gate + manifest measure the ADR-14 law: hot data paths carry 0
  data-dependent branches; guard/latch branches are class-adjudicated
  remainder, listed per site, never silenced.

ADR-14's Context is direct about why a proportionality slack under-counts:
allocator null-checks and runtime latches are "inherent to the machine code
the toolchain emits, not kernel-author choices" — i.e. remainder that is by
construction NOT proportional to source constructs. The transpose kernels'
source is deliberately branch-free (two `for` headers, exact-capacity allocs,
fixed-stride copies); 17 of the 19 emitted sites are allocator/guard/latch
machinery for that exact shape. A slack of 4 encodes a proportionality
assumption ADR-14 explicitly refuses.

## 5. Adjudication per ADR-14

Reviewed-diff resolution, recorded: **the manifest entries (8-kernel state) ARE
the adjudication.** Each of the 19 sites carries a declared class, a mnemonic
allowlist, and a ceiling (transpose <= 24 covering the dual-artifact counting
mode; untranspose <= 8 = observed+1). gate-branchless-manifest-v4 runs the
audit gate under `BRANCHLESS_SCOPE=transpose|untranspose`: GATE PASS, exit 0,
every hit MANIFEST-OK, zero unexplained hits. The cfg_homology normal-mode
FAIL is a proxy/slack mismatch against a tighter, differently-derived slack —
not an unadjudicated defect, and not gate silence: the proxy prints the full
per-symbol counts and the audit gate lists every site by class.

One subtlety the mismatch hides: the 7 untranspose sites are invisible to
audit_branchless (ltmp0) and visible only to cfg_homology's symbol-table
attribution. cfg_homology is therefore the only gate that
currently observes the full 19; the manifest's 7-site count comes from direct
disassembly of the nm-bounded region (both disassemblers byte-identical). Two
gates, complementary blind spots, one manifest — the manifest is the only
surface that names all 19 sites with classes.

## 6. Recommendation (picked)

**Amend the cfg_homology transpose row slack 4 -> 20 via reviewed diff.**
Basis:

- 19 observed across the family row; 20 = observed+1, the ceiling convention
  every manifest entry already uses (untranspose 8=7+1, 6lookup 23=22+1,
  6offset 4=3+1, chunks_4way 4=3+1).
- The row is structurally a family row (§1); its slack should cover the
  family's adjudicated remainder, not a single kernel's proportionality.
- It restores the proxy as a live drift detector: exit 0 at the adjudicated
  baseline, exit 1 on growth beyond +20 — with the audit gate + manifest
  still carrying the ADR-14 law (class/mnemonic/ceiling) underneath.
- It closes the follow-up obligation recorded in receipts/gate-cfg-homology.md
  ("bring transpose::transpose back under slack or amend slack as a reviewed
  diff" — owner kernel lanes; never landed).
- The mutant court is unaffected: MUTANT_SLACK stays 4; the fixtures and the
  FLIP witness are independent of the transpose row's slack.

The reviewed diff should also annotate the row as a transpose-family row
(regex substring-matches untranspose; source side = one file) so the next
reader does not re-derive the mismatch. The diff itself is a later reviewed
edit to `scripts/cfg_homology.sh` — this lane does not make it.

## 7. Alternative Considered and Rejected

Scope-note only (keep slack 4, annotate the family scope in the script): a
permanent normal-mode exit 1 with the mismatch documented only in prose. After
this adjudication the standing FAIL carries no new bits — house law: a gate
nobody reads is vacuous by neglect. It would also leave the wave-6 receipt's
follow-up obligation open indefinitely. The scope annotation is preserved
inside the §6 recommendation (comment on the row, alongside the amend); the
load-bearing part is the slack amend.

## 8. Falsifiers

This adjudication is FALSE if any of these is observed:

1. Any of the 19 sites lacks a declared class in
   `scripts/branchless_manifest.toml` (site-to-class map §3 must be complete).
2. cfg_homology per-symbol emitted counts diverge from the manifest's
   per-kernel counts (12/7) on the same artifacts.
3. audit_branchless begins rendering the untranspose symbol and its manifest
   entry does not adjudicate the region MANIFEST-OK at <= 8.
4. A kernel edit moves the family count off 19: the slack-20 proposal is then
   stale and must be re-adjudicated (re-run both gates, re-derive from the
   new observed+1).

## See Also

- 22-adr-register.md ADR-14 (remainder policy), ADR-17 (transpose
  reconstitution + blind spot)
- 27-branchless-kernel-abi.md §5-6 (manifest schema, remainder policy)
- 69-validation-receipt.md (V15 lane), 68-wave-10-receipt.md (G3)
- receipts/gate-cfg-homology.md, gate-cfg-homology-v2.md,
  gate-branchless-manifest-v4.md
