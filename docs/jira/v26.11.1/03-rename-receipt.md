# 03-rename-receipt

Manufacturing receipt for the avec -> cgap rename. Subject:
`/Users/sac/avec` on `main`, lineage `a512927..8b86157`. Status of the
rename as a whole: **PARTIAL_ALIVE** — every local gate ran and passed on
the renamed tree, but the rename is not complete until push, D1 (GitHub
repo rename), and D1b (local dir rename) execute. Those are operator
transitions; lanes never run git/GitHub transitions.

## Subject lineage

| Commit | What it is |
|---|---|
| `a512927` | P0 precondition work landed (index amendment, clean-tree |
| | preconditions) before any rename lane opened |
| `2c136a3` | Wave 1: content sweep P1-P3, P5 partial — 224 files, |
| | 2483(+/2454-) across crates/, Makefile, CI, README, escripts |
| `fb474a5` | Gates fix: frozen-graph byte-pins (v26.10.31 canonical TTL |
| | prefix `avec:` -> `https://avec.chatman.dev/`) swept by wave 1 were |
| | restored; fail-closed gates caught the sweep ("perturbation did not |
| | mutate the TTL" -> VACUOUS); SHACL validate + corpora A/B exit 0 |
| `d63c7d1` | BEAM repair: NIF stub lists were incomplete (12/21, 8/21) — |
| | pre-existing load_nif desync witnessed on pristine `a512927` via |
| | git-archive scratch ("Function not found avec_beam:pointer_of/1"); |
| | all 21 stubs now parsed from NIF_FUNCS; beam_test exit 0, mix test |
| | 15/0 (1 skipped) exit 0 |
| `8b86157` | Wave 2 / L8 integration: ontology flip (`ontology/cgap.ttl`, |
| | `av:` -> `https://cgap.chatmangpt.com/`), header flip |
| | `include/cgap_*.h`, ggen sync x2, ggen.lock re-lock, gate0 exit 0 |

## Lanes

The 10-lane dispatch record is the coordinator's wave record; per-lane
attribution below reconstructs from the plan DAG (02-rename-plan.md
"Execution DAG and parallel lanes") plus commit receipts, not from a
per-lane file in this repo.

| Lane | Landed |
|---|---|
| A (W1) | crates/** rename: dirs, manifests, sources, exports, NIF |
| | table, escripts, ex/ (commit `2c136a3`, the 224-file sweep) |
| B (W2) | ggen-marketplace `packs/avec-pack` -> `cgap-pack` (separate |
| | repo, `110030ca2+2664bcaf5`, landed before avec-side regen per D6) |
| C (W3) | examples/, LICENSE excluded per D2, README/CHANGELOG prose |
| | sweep (inside `2c136a3`) |
| D (W4) | stardf/purrdf diffs staged in consumer repos, no pin bump |
| | until NEW_HEAD (P6 courts re-run after push) |
| L8 | Wave 2 integration (commit `8b86157`: ontology + headers + regen) |
| L10 | This receipt + anti-vacuity negative control (below) |

## Decision outcomes (D1-D12)

| D | Verdict | Outcome |
|---|---|---|
| D1 | GitHub repo rename, operator op post-merge | PENDING (operator) |
| D1b | Local dir rename, operator op | PENDING (operator) |
| D2 | License URN `urn:avec:...` KEEP | KEPT — LICENSE*.md untouched; |
| | | cited as historical namespace exception |
| D3 | `stardf_avec_export_stream` rename | DONE (renamed to |
| | | `stardf_cgap_export_stream`; consumer courts owed post-push) |
| D4 | Arrow C Data Interface names NEVER RENAME | HELD — `ArrowArray`, |
| | | `ArrowSchema`, `ARROW_FLAG_*` verbatim in generated headers |
| D5 | 7 `crates/avec-*` dirs rename | DONE (`crates/cgap-*`); |
| | | unprefixed dirs kept |
| D6 | marketplace pack rename | DONE (`packs/cgap-pack`, pre-regen) |
| D7 | Historical receipts DO NOT EDIT | HELD — `docs/jira/v26.10.*` |
| | | untouched |
| D8 | Version 26.10.29 FROZEN | HELD — `EXPECTED_VERSION`/needle |
| | | untouched through the window |
| D9 | No history rewrite | HELD — forward commit chain on main |
| D10 | `urn:avec:` -> `urn:cgap:` | AMENDED: operator chose |
| | | `https://cgap.chatmangpt.com/` (W3C-style dereferenceable |
| | | https IRI space over a non-dereferenceable `urn:` form; |
| | | project identity namespace, not a signed instrument) |
| D12 | crates.io name availability | RECEIPTED 2026-10-31: all four |
| | | names return HTTP 404 — `cgap-arrow`, `cgap-blake3`, |
| | | `cgap-parquet`, `cgap-git` ("crate does not exist") with a |
| | | declared user-agent |

## Namespace decision

Operator decision recorded in `8b86157`: project namespace is
`https://cgap.chatmangpt.com/` — W3C-style dereferenceable https IRI
space, no `urn:` form, because the project identity namespace is a
de-reference target, not a signed instrument (contrast D2). Two
frozen-graph exceptions remain pinned to the historical namespace:

- License URN `urn:avec:license:p28-sil:v2.1.0-sovereign` (D2 — the URN
  IS the legal instrument identity; renaming would re-issue the
  instrument).
- SHACL gates pin the v26.10.31 canonical TTL's `avec:` prefix bound to
  `https://avec.chatman.dev/` as byte-pins (fb474a5 restored them after
  wave 1 swept the needles; value-only mutation discipline).

## Verification ladder (as-of `8b86157`)

Results below are coordinator-supplied wave receipts from the wave runs;
the anti-vacuity control in the next section was executed by this lane
(L10) on 2026-10-31.

- `cargo test --workspace`: 506 passed / 0 failed — exit 0.
- gate0 PASS post-relock (two-run byte-stability + c99 compile, exit 0).
- gate1: **KNOWN FAIL, pre-existing** — floating-nightly codegen drift vs
  pinned machine-code budgets; pristine `a512927` scratch fails
  identically (MISSING extents); ADR-19 re-derivation ticket owed.
- gate2 throughput PASS — 49.8 GiB/s.
- gate3 (pyarrow C Data Interface wire parity) PASS — exit 0; re-witnessed
  by this lane against the release dylib during the negative control
  (below).
- `scripts/symbol_gate.py` PASS.
- `make lint` / `make publish-check` PASS.
- `bash scripts/c99_smoke.sh` PASS.
- SHACL: `validate_ontology` PASS, negative corpora A and B exit 0.
- `just verify` PASS; `just courts` PASS (3 adversarial courts).
- `escript test/beam_test.escript` PASS (exit 0) post-`d63c7d1`; `(cd ex
  && mix test)` 15/0 (1 skipped) exit 0.

## Anti-vacuity negative control (P7 step 12, L10)

Proves the symbol gate binds the renamed symbol, not a name in a comment.
Court: gate3_cdata.py (repo) sed-copied to /tmp with two mutations: LIB
path via env, and one probe line binding `_cgap_version` presence
(`LIB.cgap_version` immediately after load, before any FFI call). The
same court binary-code runs against both subjects — a real flip, not a
mutated-assertion-only court.

- Release artifact: `target/release/libcgap_ffi.dylib` present at
  `8b86157`.
- Positive (court + real dylib): exit 0 —
  `GATE 3 PASS: bitwise parity through C Data Interface`.
- Negative (court + /tmp stub dylib `negctl.c` exporting 5 of 6 renamed
  symbols, `_cgap_version` deliberately absent; `cc -dynamiclib`): exit 1
  — `AttributeError: dlsym(0x75056ed0, cgap_version): symbol not found`,
  naming the missing symbol. A court that cannot flip is decoration; this
  one flips.
- Positive `nm` count:
  `nm -gU target/release/libcgap_ffi.dylib | grep -c '_cgap_version$'`
  -> `1`.
- /tmp artifacts (`negctl.c`, `negctl.dylib`, `gate3_negctl.py`) removed;
  `ls /tmp | grep -i negctl` empty. Repo tree untouched (court mutations
  were sed-copies in /tmp only; `git status` clean at receipt time).

## Operator steps pending

1. D1: GitHub repo rename avec -> cgap (GitHub redirects the old URL;
   cargo git pins keep resolving through the window).
2. D1b: local dir `/Users/sac/avec` -> `/Users/sac/cgap` immediately
   after push (escript paths already flipped in `2c136a3`; window kept
   zero-length in practice).
3. Pin bumps to NEW_HEAD in consumers (stardf, purrdf) + P6 consumer
   courts re-run after push — strictly after D1.
4. Marketplace note: the rename commits sit on the local branch
   `blake3-digest-keys`, which strictly contains `origin/main`; branch
   state must be disclosed when the pin bumps land.

## See Also

- [01-rename-impact-survey.md](01-rename-impact-survey.md) — ALIVE census
  and D-point inputs.
- [02-rename-plan.md](02-rename-plan.md) — the executable spec (P0-P7,
  D1-D12, R1-R12); this receipt does not supersede it.
- [00-index.md](00-index.md) — milestone index (rename subsection).
