# Gate Receipt: CFG Homology Proxy (DMADV Analyze)

- **Gate**: `scripts/cfg_homology.sh` — source decision constructs vs emitted
  banned-branch opcodes per declared hot kernel (documented PROXY, not a
  branch-freedom proof; see script header LIMITS).
- **Wave**: 6, lane 6, repo `/Users/sac/avec` (single canonical checkout).
- **Command**: `./scripts/cfg_homology.sh` — **exit 1 (GATE FAIL)**.
- **Disassembler**: objdump (Xcode); 6 rlibs, `--profile audit` into temp
  `CARGO_TARGET_DIR` (repo tree undisturbed).

## Homology table (2026-10-30, this run)

| kernel | source constructs | emitted branches | delta | slack | verdict |
|---|---|---|---|---|---|
| transpose | 2 | 19 (`transpose::transpose` 12 + `QuadColumns::untranspose` 7) | **+17** | 4 | **FAIL** |
| predicates/filter_quads_eq | 2 | 2 | 0 | 4 | PASS |
| blake3/compress_chunks_4way | 11 | 5 (`compress_chunks_4way` 2 + `_x2` 3) | -6 | 6 | PASS |
| avec-git/ladder | 5 | 8 (`ladder_search` 8) | +3 | 4 | PASS |

**Verdict: FAILED** — transpose delta +17 exceeds slack 4. The other three
rows PASS.

## Cross-check vs `scripts/audit_branchless.sh` (same session)

Per-symbol emitted-branch counts agree exactly where scopes overlap:
`transpose::transpose` 12, `filter_quads_eq` 2 — identical in both gates'
disassembly counts. `audit_branchless.sh` also exits 1 this session (same
transpose + predicates symbols carry the cbz/b.cond hits), so the two gates
are consistent: the FAIL is a real property of the current artifacts, not a
homology-proxy artifact.

Wave-4/5 noted numbers (transpose 24, predicates 2) vs current: predicates 2
unchanged; transpose is now 12+7=19 across `transpose::transpose` +
`QuadColumns::untranspose` (wave-6 lane churn may have moved the split;
current numbers recorded above are authoritative as of this run).

## Compile-freeze unblock (disclosed, concurrent-lane collision)

`crates/avec-blake3/src/multi_chunk.rs` `compress_chunks_4way_x2_neon` did
not compile: `#[forbid(unsafe_op_in_unsafe_fn)]` + Rust-2024 rule — 48
E0133 errors. Lane 6 added the missing closing `}` for a body-level
`unsafe { }` wrapper; a concurrent lane added the opening wrapper with
canonical indentation (the two edits landed coherently). Verified:
`CARGO_TARGET_DIR=/tmp/wh6-target cargo build --profile audit -p avec-blake3`
→ Finished, exit 0 (1 pre-existing `unused_unsafe` warning at line 333,
harmless, pre-dates this session's fix shape).

## Proxy defect found + fix (by another lane, verified by lane 6)

The blake3 row's symbol regex originally included bare `hash_wide_simd`,
which also matched `avec_arrow::blake3::hash_wide_simd` (a different crate,
different source file, not in this row's source column — inflated the
blake3 emitted count 253 vs 5). Fixed during this session in
`scripts/cfg_homology.sh` (row now matches `compress_chunks_4way` only,
with an out-of-scope NOTE at lines 55-59); lane 6 verified the re-run
produces the table above. Lane 6 made no further edit to the script.

## Standing

- Gate: **ALIVE, verdict FAILED** (exit 1, receipted above). The gate is not
  vacuous: it both detects the transpose violation and passes the three
  clean rows.
- Follow-up (owner: kernel lanes, not this receipt): bring
  `transpose::transpose` back under slack or amend slack as a reviewed diff.
