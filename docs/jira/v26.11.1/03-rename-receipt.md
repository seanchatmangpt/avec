# 03-rename-receipt

Manufacturing receipt for the avec -> cgap rename. Subject:
`/Users/sac/cgap` on `main`, lineage `a512927..d03a6b4`. Status of the
rename as a whole: **PARTIAL_ALIVE** — push, D1 (GitHub repo rename),
D1b (local dir rename), and crates.io publication of `cgap-arrow` /
`cgap-blake3` executed 2026-10-31 (receipts below); remaining:
`cgap-git` / `cgap-parquet` publication (rate-limited to
2026-11-01T06:33:33Z) and the stardf/purrdf consumer transitions (WIP
upstream, pins deferred).

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
| D1 | GitHub repo rename, operator op post-merge | DONE 2026-10-31 —
  `gh repo rename cgap -R seanchatmangpt/avec`; `gh repo view
  seanchatmangpt/cgap` confirms (old URL redirects) |
| D1b | Local dir rename, operator op | DONE — checkout now
  `/Users/sac/cgap`; `/Users/sac/avec` absent |
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

## Post-rename execution (2026-10-31, this session)

Executed after the operator moved the checkout (`D1b`) and per the
operator's amended plan (crates.io publication **before** consumer git
pins):

1. **L10 committed** first so NEW_HEAD includes the receipt: `663ed36`
   (03-rename-receipt.md + 00-index standing).
2. **D1**: `gh repo rename cgap -R seanchatmangpt/avec` exit 0;
   `gh repo view seanchatmangpt/cgap` -> `cgap
   https://github.com/seanchatmangpt/cgap`. origin URL updated to the
   new location.
3. **Push**: `4ede671..663ed36 main -> main`, then `663ed36..d03a6b4`
   (see 5).
4. **Pin-bump reversal**: initial stardf/purrdf git-pin edits were
   reverted at operator direction (publish-first plan).
5. **cgap de-stardf** (`d03a6b4`): operator direction — cgap carries NO
   stardf dependency. The stardf-arrow-capi dev-dep in `cgap-ffi`
   blocked all four `cargo publish` runs (cargo resolves every member's
   dev-deps during packaging) and cgap-ffi's court moved to its subject:
   `ggen-legacy/stardf/arrow/crates/stardf-arrow-capi/tests/
   cgap_consumer_court.rs` (dev-deps `cgap-ffi` + `cgap-audit` from the
   stardf side; DYLIB const points at the stardf workspace target dir).
   Also exposed a stardf-side pre-existing breakage (separate, not
   repaired here): stardf root `[workspace.package]` lacks
   `description`/`keywords`/`categories`/`license`/`rust-version`
   inherited by ~21-39 members — `cargo metadata` exit 101 at `46416e74`.
6. **crates.io publication** (token from operator in-session; 26.10.29
   per D8 freeze):
   - `cgap-blake3 v26.10.29` — Published, exit 0.
   - `cgap-arrow v26.10.29` — Published, exit 0.
   - `cgap-git v26.10.29` — Published, exit 0 (window opened 06:33Z).
   - `cgap-parquet v26.10.29` — Published, exit 0 (`--allow-dirty`: its
     lane-written README was on disk, lane complete + verified; window
     opened 06:43Z after the sliding reset).
   - `cgap v26.10.29` (umbrella facade, `c108256`) — Published, exit 0
     (`--allow-dirty`, same README posture); window opened 06:53Z.
   - Preceded by four dry-run exits 0 and one commit `d03a6b4` (the
     license-file lines + court move) so registry content matches a
     committed rev.
7. **Registry ALIVE witness**: fresh `/tmp/cgap-consumer` crate,
   `cargo add cgap-arrow` -> `cargo build` exit 0; and a fresh
   `/tmp/cgap-umbrella-check*` consumer: `cargo add cgap` ->
   `cargo build` exit 0, Cargo.lock source =
   `registry+https://github.com/rust-lang/crates.io-index`. crates.io
   API: ALL FIVE crates max_version 26.10.29 (user-agent
   `cgap-publish-receipt`): cgap, cgap-arrow, cgap-blake3, cgap-git,
   cgap-parquet.

Standing after this session: rename **PARTIAL_ALIVE -> publication
complete** — ALL FIVE crates (cgap umbrella + 4 kernels) ALIVE on
crates.io at 26.10.29 with fresh-consumer build witnesses. Remaining
deferred: consumer pins (stardf WIP upstream; purrdf pin still
`avec-arrow 20248e4` broken E0583) — operator call when stardf's
workspace manifest repair lands.

## See Also

- [01-rename-impact-survey.md](01-rename-impact-survey.md) — ALIVE census
  and D-point inputs.
- [02-rename-plan.md](02-rename-plan.md) — the executable spec (P0-P7,
  D1-D12, R1-R12); this receipt does not supersede it.
- [00-index.md](00-index.md) — milestone index (rename subsection).
