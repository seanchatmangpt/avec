# Rename Plan: avec -> cgap (v26.11.1)

Rename the `avec` substrate to `cgap` ("Chatman Git Arrow Parquet") across
crates, C symbols, BEAM NIFs, generated headers, CI, docs, and both downstream
consumers - phased, gated, lockstep-ordered. Written by RENAME-PLANNING
LANE 2 on 2026-10-31 against avec HEAD `4ede671` (branch `main`). The only
file this lane writes is this one; everything else was a read-only sweep.

Standing of this document: **UNVERIFIED** - a plan, not execution. Nothing
in it has run. Claims become ALIVE only when the named gate exits 0 against
the renamed subject.

Drafting note (disclosed): two long-form single-pass drafts of this file
corrupted mid-write; both were discarded by full-file overwrite. The file
was built from short verified chunks instead.

## 0. Observed subject

Facts re-read from disk 2026-10-31 at `/Users/sac/avec`, HEAD `4ede671`,
branch `main` (in sync with origin/main).

- Dirty paths (P0 precondition below): `M Cargo.lock`,
  `M crates/avec-ffi/Cargo.toml`,
  `?? crates/avec-ffi/tests/stardf_consumer_court.rs`,
  `?? rust-toolchain.toml`.
- `avec` token census: 3949 lines contain `avec` outside `target/` and
  `.git/`.
- Workspace version `26.10.29`, single-sourced into generated
  `VERSION` / `VERSION_C` (`b"26.10.29\0"`) via repo-local
  `gen-templates/version_rs.tmpl` and the marketplace pack
  `/Users/sac/ggen-marketplace/packs/avec-pack`.
- Generated surfaces (Makefile `GATE0_FILES`, Makefile lines 34-37):
  `include/avec_c_data.h`, `include/avec_format_codes.h`,
  `crates/avec-arrow/src/generated_layout.rs`,
  `crates/avec-arrow/src/generated_version.rs`.
- C-ABI dylib `libavec_ffi.dylib` exports exactly 6 `avec_*` symbols
  (crates/avec-ffi/src/lib.rs): `avec_i32_batch`, `avec_schema_addr`,
  `avec_array_addr`, `avec_batch_rows`, `avec_version`, `avec_digest_of`.
- BEAM NIF (crates/avec-beam/src/lib.rs): module name `avec_beam`
  (`ErlNifEntry.name`), `num_of_funcs: 21` (20 `avec_*` wire names plus one
  unprefixed `pointer_of`).
- Downstream consumers with git-rev pins on
  `https://github.com/seanchatmangpt/avec`:
  - stardf (subtree `/Users/sac/ggen-legacy/stardf/arrow`), pin rev
    `67c0ec1` at `stardf/arrow/Cargo.toml:263`;
  - purrdf (`/Users/sac/purrdf`), pin rev `20248e4` at
    `purrdf/Cargo.toml:145` - the rev stardf already diagnosed broken
    (E0583: no `chase_lev.rs` at that rev; witness
    `stardf/arrow/Cargo.toml:258-261`). purrdf's pin is broken today.

## P0 - Decision register (what renames, what deliberately does not)


Decision register:

**D1 - GitHub repo name `avec` -> `cgap`**
  Verdict: OPERATOR op, after merge
  Rationale: GitHub redirects the old URL, so cargo git pins keep resolving during the window.
  Lanes never execute git/GitHub transitions (coordinator owns them).

**D1b - Local dir `/Users/sac/avec` -> `/Users/sac/cgap`**
  Verdict: DEFER to end of P5, operator-executed
  Rationale: 7 test escripts plus stardf rehearsal docs hardcode `/Users/sac/avec`; the sweep
  flips the literal paths first, then the dir rename lands immediately after merge so the window
  where escripts point at a not-yet-existing path stays zero-length in practice.

**D2 - License identifier `urn:avec:license:p28-sil:v2.1.0-sovereign`**
  Verdict: KEEP (do not rename)
  Rationale: The URN is the legal instrument identifier, not a product name: LICENSE-P28-SIL.md
  line 3 makes it the Canonical Specification Identifier and line 254 requires identifying the
  instrument strictly by `LicenseRef-P28-SIL-2.1.0` or this URN. Renaming it would mutate a
  standing instrument identity and require re-issuing P28-SIL as a new version. Document it as a
  historical namespace in README + the anti-vacuity allowlist (R9). Migration path, only if the
  operator orders it: re-issue as v2.2.0 with a new URN in a separate milestone.

**D3 - stardf-owned symbol `stardf_avec_export_stream`**
  Verdict: RENAME to `stardf_cgap_export_stream`, same window
  Rationale: The word is a semantic marker inside a stardf-owned export
  (stardf/arrow/crates/stardf-arrow-capi/src/lib.rs line 190). stardf is unpublished fork-posture,
  so the ABI break costs only a court re-run (P6). Keeping it would leave the single most visible
  "avec" in the consumer.

**D4 - Arrow C Data Interface names: `ArrowArray`, `ArrowSchema`, `ARROW_FLAG_*`**
  Verdict: NEVER RENAME
  Rationale: Spec-mandated third-party ABI names (Arrow C Data Interface spec). They appear in
  generated headers via the pack and stay verbatim.

**D5 - Crate dir names**
  Verdict: Rename the 7 `crates/avec-*` dirs; keep `crates/audit`, `crates/bench`, `crates/cli`,
  `crates/integration`, `crates/wasm-footprint` dir names
  Rationale: Cargo path deps survive dir renames via the updated `members` list; unprefixed dirs
  carry no "avec" token, so renaming them is churn without payoff.

**D6 - ggen-marketplace pack `packs/avec-pack`**
  Verdict: RENAME to `packs/cgap-pack` (dir + `pack.toml` name + templates)
  Rationale: The pack is the generator of record for the four GATE0_FILES; leaving it `avec-pack`
  would regenerate `avec_*` headers on the next `ggen sync run` and silently revert P4. Cross-
  repo: lands in `/Users/sac/ggen-marketplace` BEFORE any avec-side regen.

**D7 - Historical receipts and evidence (`docs/jira/v26.10.29/`, `docs/jira/v26.10.31/` including
`receipts/`, plus `*.json` evidence)**
  Verdict: DO NOT EDIT - immutable record
  Rationale: Receipts are evidence; rewriting evidence text is forbidden. Living docs (README,
  CHANGELOG via appended entry, diataxis) get the sweep; milestone indexes may gain a forward-
  pointer note, nothing more.

**D8 - Version `26.10.29`**
  Verdict: FROZEN through the rename window
  Rationale: `scripts/symbol_gate.py` pins `EXPECTED_VERSION = "26.10.29"` (line 119) and counts
  the needle `b"26.10.29\0"` per artifact. A version bump riding the rename would couple two
  changesets. If the operator orders a bump anyway, re-pin the needle and `EXPECTED_VERSION` in
  the same commit as the bump.

**D9 - Git history**
  Verdict: NO REWRITE
  Rationale: `git mv` for dir renames; commits stay immutable; the rename is a forward commit
  chain on `main` (or a feature branch merged per repo practice).

**D10 - `ontology/` project namespace `@prefix av: <urn:avec:>`**
  Verdict: MIGRATE to `urn:cgap:`
  Rationale: Project identity namespace, not a signed instrument (contrast D2). Moves in P4
  together with `ontology/avec.ttl` -> `ontology/cgap.ttl`.

**D11 - Milestone index `00-index.md` filename contract**
  Verdict: AMEND (agent-1/operator)
  Rationale: `02-rename-plan.md` is not a registered slot in the index tables; the index is
  "amended, never bypassed", so registration of this file is a required P0 item, not self-service.

### P0 exact commands, gate, risk

Precondition commands (coordinator, before any rename lane opens):

    cd /Users/sac/avec && git status --porcelain
    # land or snapshot-as-file the dirty paths, target: empty porcelain

    # index amendment (operator/agent-1): register 02-rename-plan.md in
    # docs/jira/v26.11.1/00-index.md

Gate: `git status --porcelain` output empty (except the plan doc itself),
every D-row initialed where the verdict column says OPERATOR.

Risk: opening rename lanes on a dirty tree mixes the untracked
`stardf_consumer_court.rs` into lane diffs whose ownership is disjoint by
path. R2 in the risk register.

## P1 - Crate/package renames avec-* -> cgap-*

Name mapping table (old -> new), every crate incl. lib names. Lib names are
Cargo defaults (package name, dashes to underscores); no `[lib] name`
overrides exist anywhere in the workspace, verified across all 12 manifests.


**old package:** avec-arrow; **new package:** cgap-arrow; **dir old -> new:** crates/avec-arrow ->
  crates/cgap-arrow; **lib name old -> new:** avec_arrow -> cgap_arrow; **publish:** true

**old package:** avec-blake3; **new package:** cgap-blake3; **dir old -> new:** crates/avec-blake3
  -> crates/cgap-blake3; **lib name old -> new:** avec_blake3 -> cgap_blake3; **publish:** true

**old package:** avec-parquet; **new package:** cgap-parquet; **dir old -> new:** crates/avec-
  parquet -> crates/cgap-parquet; **lib name old -> new:** avec_parquet -> cgap_parquet;
  **publish:** true

**old package:** avec-git; **new package:** cgap-git; **dir old -> new:** crates/avec-git ->
  crates/cgap-git; **lib name old -> new:** avec_git -> cgap_git; **publish:** true

**old package:** avec-ffi; **new package:** cgap-ffi; **dir old -> new:** crates/avec-ffi ->
  crates/cgap-ffi; **lib name old -> new:** avec_ffi -> cgap_ffi; **publish:** false

**old package:** avec-beam; **new package:** cgap-beam; **dir old -> new:** crates/avec-beam ->
  crates/cgap-beam; **lib name old -> new:** avec_beam -> cgap_beam; **publish:** false

**old package:** avec-gpu; **new package:** cgap-gpu; **dir old -> new:** crates/avec-gpu ->
  crates/cgap-gpu; **lib name old -> new:** avec_gpu -> cgap_gpu; **publish:** (unset)

**old package:** avec-audit; **new package:** cgap-audit; **dir old -> new:** crates/audit
  (unchanged); **lib name old -> new:** avec_audit -> cgap_audit; **publish:** false

**old package:** avec-cli; **new package:** cgap-cli; **dir old -> new:** crates/cli (unchanged);
  **lib name old -> new:** avec_cli -> cgap_cli; **publish:** false

**old package:** avec-integration; **new package:** cgap-integration; **dir old -> new:**
  crates/integration (unchanged); **lib name old -> new:** avec_integration -> cgap_integration;
  **publish:** false

**old package:** bench; **new package:** bench (unchanged); **dir old -> new:** crates/bench
  (unchanged); **lib name old -> new:** unchanged; **publish:** false

**old package:** wasm-footprint; **new package:** wasm-footprint (unchanged); **dir old -> new:**
  crates/wasm-footprint (unchanged, standalone); **lib name old -> new:** unchanged; **publish:**
  false


Notes: `wasm-footprint` is not a `[workspace]` member (verified: absent from
the root `Cargo.toml` `members` list); it consumes avec-arrow/avec-parquet by
relative path and its manifest comment names the kernels - it is in W1 scope
for dep renames. The four publish=true rows matter for `make publish-check`
and for crates.io: crate names `cgap-arrow`, `cgap-blake3`, `cgap-parquet`,
`cgap-git` must be free on crates.io (D12 below).

Decision D12 (registered here because P1 raised it): the four crates.io
publishable names `cgap-arrow`, `cgap-blake3`, `cgap-parquet`, `cgap-git`
must be verified available (or already owned by the operator) before P1
lands; check with `curl -s https://crates.io/api/v1/crates/cgap-arrow` etc.
A taken name forces a scoped-name decision (e.g. `cgap-arrow` vs
`chatman-cgap-arrow`) BEFORE the rename, not after publish failure.

### P1 exact commands

Token map (exact, word-boundary aware; apply in this order to crates/):

    avec-arrow  -> cgap-arrow      (manifests, deps, prose inside code)
    avec_arrow  -> cgap_arrow      (use sites, extern crate refs)
    avec-blake3 -> cgap-blake3
    avec_blake3 -> cgap_blake3
    avec-parquet-> cgap-parquet
    avec_parquet-> cgap_parquet
    avec-git    -> cgap-git
    avec_git    -> cgap_git
    avec-ffi    -> cgap-ffi
    avec_ffi    -> cgap_ffi
    avec-beam   -> cgap-beam
    avec_beam   -> cgap_beam
    avec-gpu    -> cgap-gpu
    avec_gpu    -> cgap_gpu
    avec-audit  -> cgap-audit
    avec_audit  -> cgap_audit
    avec-cli    -> cgap-cli
    avec_cli    -> cgap_cli
    avec-integration -> cgap-integration
    avec_integration-> cgap_integration

    cd /Users/sac/avec
    git mv crates/avec-arrow crates/cgap-arrow
    git mv crates/avec-blake3 crates/cgap-blake3
    git mv crates/avec-parquet crates/cgap-parquet
    git mv crates/avec-git crates/cgap-git
    git mv crates/avec-ffi crates/cgap-ffi
    git mv crates/avec-beam crates/cgap-beam
    git mv crates/avec-gpu crates/cgap-gpu
    # then the token sweep over crates/** ( manifests + sources + tests )
    LC_ALL=C find crates -type f \( -name '*.rs' -o -name '*.toml' \) \
      -exec sed -i '' -e 's/avec-arrow/cgap-arrow/g; s/avec_arrow/cgap_arrow/g; s/avec-blake3/cgap-blake3/g; s/avec_blake3/cgap_blake3/g; s/avec-parquet/cgap-parquet/g; s/avec_parquet/cgap_parquet/g; s/avec-git/cgap-git/g; s/avec_git/cgap_git/g; s/avec-ffi/cgap-ffi/g; s/avec_ffi/cgap_ffi/g; s/avec-beam/cgap-beam/g; s/avec_beam/cgap_beam/g; s/avec-gpu/cgap-gpu/g; s/avec_gpu/cgap_gpu/g; s/avec-audit/cgap-audit/g; s/avec_audit/cgap_audit/g; s/avec-cli/cgap-cli/g; s/avec_cli/cgap_cli/g; s/avec-integration/cgap-integration/g; s/avec_integration/cgap_integration/g' {} +
    # wasm-footprint standalone manifest + examples/sovereign-e2e are
    # covered by the same tokens in their own lanes (W3)

P1 gate:

    cargo metadata --format-version 1 | grep -c '"name":"avec-'   # must be 0
    cargo check --workspace && cargo check -p wasm-footprint
    grep -rw avec crates --include='*.rs' --include='*.toml' | wc -l  # 0

P1 risk: the sed must NOT touch `crates/*/tests/*.rs` receipt-text strings
quoting historical names inside assertions; those are allowed to survive as
historical quotes only if a court actually pins them - flag any hit for
review instead of blind-sed (the gate greps the residual, so each residual
hit is adjudicated, not ignored).

## P2 - C symbol renames avec_* -> cgap_*

The complete C-ABI export surface of `libavec_ffi.dylib`, derived
mechanically from `#[no_mangle]` in crates/avec-ffi/src/lib.rs:

| old export | new export |
|---|---|
| avec_array_addr | cgap_array_addr |
| avec_batch_rows | cgap_batch_rows |
| avec_digest_of | cgap_digest_of |
| avec_i32_batch | cgap_i32_batch |
| avec_schema_addr | cgap_schema_addr |
| avec_version | cgap_version |

The two ctypes consumers naming these symbols must move in the same phase:
/Users/sac/avec/scripts/gate3_cdata.py:13:LIB = ctypes.CDLL("target/debug/libavec_ffi.dylib")
/Users/sac/avec/scripts/gate3_cdata.py:15:    ("avec_array_addr", ctypes.c_size_t),
/Users/sac/avec/scripts/gate3_cdata.py:16:    ("avec_schema_addr", ctypes.c_size_t),
/Users/sac/avec/scripts/gate3_cdata.py:17:    ("avec_batch_rows", ctypes.c_int64),
/Users/sac/avec/scripts/gate3_schema.py:29:LIB = ctypes.CDLL(LIB_PATH)
/Users/sac/avec/scripts/gate3_schema.py:31:    ("avec_i32_batch", ctypes.c_size_t),  # returns the live ArrowArray pointer
/Users/sac/avec/scripts/gate3_schema.py:32:    ("avec_array_addr", ctypes.c_size_t),
/Users/sac/avec/scripts/gate3_schema.py:33:    ("avec_schema_addr", ctypes.c_size_t),
/Users/sac/avec/scripts/gate3_schema.py:34:    ("avec_batch_rows", ctypes.c_int64),
/Users/sac/avec/scripts/gate3_schema.py:133:    schema = pa.schema([pa.field("avec_col", pa.int32(), nullable=True)])
/Users/sac/avec/scripts/gate3_schema.py:135:    f = table.schema.field("avec_col")

### P2 exact commands, gate, risk

Commands (W1 lane, after P1 dir renames):

    LC_ALL=C sed -i '' 's/avec_/cgap_/g' crates/cgap-ffi/src/lib.rs
    LC_ALL=C sed -i '' 's/avec_/cgap_/g' scripts/gate3_cdata.py scripts/gate3_schema.py
    # gate3_schema.py also names a pyarrow column "avec_col" (lines 133,135):
    # rename to "cgap_col" in the same commit so the wire-parity court reads
    # end-to-end under the new name.

Gate:

    nm -gU target/release/libcgap_ffi.dylib | grep -c ' _cgap_'      # 6
    nm -gU target/release/libcgap_ffi.dylib | grep -c ' _avec_'      # 0
    make gate3          # pyarrow wire-parity through the renamed exports

Risk: any external consumer dlopening `libavec_ffi.dylib` by old symbol
names breaks on pull; the redirect trick does not exist for symbols. The
only known in-repo consumers are the two gate3 scripts; the stardf consumer
court links the capi (not these symbols), see P6.

## P3 - BEAM NIF renames

Surface, derived from crates/avec-beam/src/lib.rs:

- Module name: `ErlNifEntry.name = b"cgap_beam\0"` (was b"avec_beam\0").
- 21 registered functions (`num_of_funcs: 21`); 20 are `avec_*` wire names
  that become `cgap_*`; the one unprefixed name `pointer_of` is unchanged.
- Full wire-name map, mechanically derived (name -> renamed):
- avec_alloc_stats -> cgap_alloc_stats
- avec_batch_binary -> cgap_batch_binary
- avec_binary_data_pointer -> cgap_binary_data_pointer
- avec_dtor_stats -> cgap_dtor_stats
- avec_envelope_len -> cgap_envelope_len
- avec_envelope_new -> cgap_envelope_new
- avec_flush_to_parquet -> cgap_flush_to_parquet
- avec_free -> cgap_free
- avec_get -> cgap_get
- avec_get_i64 -> cgap_get_i64
- avec_len -> cgap_len
- avec_new -> cgap_new
- avec_new_fast -> cgap_new_fast
- avec_new_i64 -> cgap_new_i64
- avec_open -> cgap_open
- avec_receipt_query -> cgap_receipt_query
- avec_scan -> cgap_scan
- avec_seal_to_packfile -> cgap_seal_to_packfile
- avec_stats -> cgap_stats
- avec_version -> cgap_version

Erlang/Elixir side, same phase, atomic with the NIF table change
(erlang:load_nif/2 requires the calling module name to equal
`ErlNifEntry.name`):

- `test/beam_test.escript`: `-module(avec_beam)` -> `-module(cgap_beam)`,
  every export list entry, and the hardcoded dylib paths
  (`/Users/sac/avec/target/{debug,release}/libavec_beam.dylib` ->
  `/Users/sac/cgap/...libcgap_beam.dylib`). Note the escript copies the
  dylib to a `.so` sibling because OTP's load_nif appends only `.so` —
  keep that mechanism, rename the names it uses.
- Remaining 6 escripts in `test/` hardcode `/Users/sac/avec` paths
  (verified: 7 of 7 escripts reference the absolute path).
- `ex/` wrapper: app `:avec_ex` -> `:cgap_ex`, module `AvecEx` -> `CgapEx`,
  `ex/lib/avec_ex.ex` -> `ex/lib/cgap_ex.ex`, `ex/mix.exs` project values.

P3 commands:

    LC_ALL=C sed -i '' 's/avec_beam/cgap_beam/g; s/avec_/cgap_/g' \
      crates/cgap-beam/src/*.rs
    LC_ALL=C git mv ex/lib/avec_ex.ex ex/lib/cgap_ex.ex
    LC_ALL=C sed -i '' 's/avec_ex/cgap_ex/g; s/AvecEx/CgapEx/g; s/avec_beam/cgap_beam/g; s/avec_/cgap_/g' \
      ex/mix.exs ex/lib/cgap_ex.ex ex/test/*.exs test/*.escript
    LC_ALL=C sed -i '' 's|/Users/sac/avec|/Users/sac/cgap|g' test/*.escript

P3 gate (real BEAM, not a stub):

    escript test/beam_test.escript && (cd ex && mix test)

P3 risk: load_nif resolves the dylib by module name; a half-renamed commit
(erlang side renamed, Rust table not, or vice versa) fails LOUDLY at load
with "no implementation for NIF cgap_new/2" class errors - the failure mode
is fail-fast, not silent, which is why P3 is one atomic commit.

## P4 - Header/include renames (generated surfaces - pack-side first)

Law: the four GATE0_FILES are GENERATED. Hand-editing them is forbidden
(ontology-first). The only lawful path is: rename the generator, then
regenerate.

Generator of record: `/Users/sac/ggen-marketplace/packs/avec-pack`
(templates `avec_h.tmpl`, `avec_format_codes_h.tmpl`, `layout_gen_rs.tmpl`,
plus `pack.toml`, `ontology.ttl`, gates `010-050 *.rq`). Repo-local
templates: `gen-templates/version_rs.tmpl` and
`gen-templates/specialization_manifest_rs.tmpl`.

Order inside P4:

1. Marketplace first (W2 lane, separate repo
   `/Users/sac/ggen-marketplace`): `git mv packs/avec-pack packs/cgap-pack`,
   sweep `avec_h.tmpl` -> `cgap_h.tmpl`,
   `avec_format_codes_h.tmpl` -> `cgap_format_codes_h.tmpl`, pack.toml name,
   ontology.ttl prefixes, gate .rq files, README. D6.
2. Avec-side flip (W3 lane): `ggen.toml` `[project] name = "cgap"`,
   `[ontology] source = "ontology/cgap.ttl"`,
   `[packs] cgap-pack = { path = "../ggen-marketplace/packs/cgap-pack" }`.
3. `git mv ontology/avec.ttl ontology/cgap.ttl` + prefix `urn:avec:` ->
   `urn:cgap:` (D10).
4. Regenerate: `ggen sync run` twice, byte-compare (the gate0 replay law).

P4 produced-vs-consumed table:


**generated surface:** header; **old:** include/avec_c_data.h; **new:** include/cgap_c_data.h;
  **renamed via:** pack template cgap_h.tmpl

**generated surface:** header; **old:** include/avec_format_codes.h; **new:**
  include/cgap_format_codes.h; **renamed via:** pack template cgap_format_codes_h.tmpl

**generated surface:** rust; **old:** crates/cgap-arrow/src/generated_layout.rs; **new:** same
  path, content tables only; **renamed via:** pack template layout_gen_rs.tmpl

**generated surface:** rust; **old:** crates/cgap-arrow/src/generated_version.rs; **new:** same
  path, VERSION content; **renamed via:** gen-templates/version_rs.tmpl


Token effects to verify after regen: guard `AVEC_C_DATA_H` ->
`CGAP_C_DATA_H`, `AVEC_FORMAT_CODES_H` -> `CGAP_FORMAT_CODES_H`,
`AVEC_FMT_*` -> `CGAP_FMT_*` (all macros in the format-codes header).
`ARROW_FLAG_*` and the `ArrowArray`/`ArrowSchema` struct names DO NOT move
(D4).

P4 commands:

    cd /Users/sac/avec
    git mv include/avec_c_data.h include/cgap_c_data.h
    git mv include/avec_format_codes.h include/cgap_format_codes.h
    ggen sync run && ggen sync run   # byte-stable replay proof (gate0 law)
    make gate0

P4 gate: `make gate0` exits 0 (two-run sha256 cmp of the four generated
files + freestanding `cc -std=c99` compile of include/cgap_c_data.h).

P4 risk: if the marketplace pack rename (step 1) has NOT landed when
`ggen sync run` runs, the generator faithfully re-mints `avec_*` headers and
the rename silently reverts - the gate0 sha cmp will PASS (determinism holds)
while the names are wrong. Control: gate the P4 sequence on
`test -d ../ggen-marketplace/packs/cgap-pack` and refuse to run otherwise.
The `examples/c99_consumer/main.c` consumer (includes both headers, exercises
AVEC_FMT_* via static asserts, double-inclusion guard test) is swept in the
same phase; its local identifiers (avec_static_assert_, avec_failures) ride
the P5 prose sweep.

## P5 - Dirs + CI + docs sweep

Scope, by directory (all verified on disk):

- Root config: `Cargo.toml` (repository URL), `ggen.toml`, `Makefile`
  (GATE0_FILES, `/tmp/avec_gate0_run{1,2}.sha`, `/tmp/avec_h_gate0.o`,
  `-p cgap-audit/cgap-ffi/cgap-beam` builds, publish dry-runs
  `-p cgap-arrow/cgap-blake3/cgap-parquet`, dylib paths in comments),
  `justfile` (`-p cgap-cli` recipes), `Cargo.lock` (regenerated).
- `scripts/` (17 of 22 files carry the token; the named gates):
  `symbol_gate.py` (libcgap_ffi.dylib, libcgap_audit.dylib; version needle
  and EXPECTED_VERSION stay per D8), `branch_audit.py`
  (target/audit/libcgap_audit.dylib), `gate3_cdata.py`, `gate3_schema.py`,
  `verify_v26.10.31.sh` (22 hits), `audit_branchless.sh`, `cfg_homology.sh`,
  `validate_ontology.sh`, `footprint.sh`, `c99_smoke.sh`, `release_check.py`,
  `nightly_perf.sh`, `perf_guard.sh`, `timing_gate.py`, `dod.sh`,
  `mint_receipt.sh`, `receipt_doctor.sh`. Script NAMES stay (they are
  milestone-versioned or role-named, not avec-named); contents sweep.
- `.github/workflows/ci.yml`: job/step names ("test-avec-beam" warnings,
  "verify (scripts/verify_v26.10.31.sh ...)" display names), no path tokens
  beyond the script name. Keep the file name `ci.yml`.
- `test/` escripts absolute paths (already in P3), `examples/` (c99 +
  sovereign-e2e manifests/description/paths), `README.md` (title, standing
  table, "twelve crates" table), `CHANGELOG.md` (APPEND a rename entry under
  [Unreleased]; never rewrite past entries), `RELEASE_NOTES.md` (append),
  `docs/` living files: the 17 root .md files plus `docs/diataxis/**`.
- `LICENSE.md` / `LICENSE-P28-SIL.md`: NO CONTENT EDIT (D2); README gains a
  note that the license URN intentionally retains the avec namespace.
- Immutability (D7): `docs/jira/v26.10.29/`, `docs/jira/v26.10.31/` and any
  receipts/ evidence stay byte-identical. Verify with a sha256 manifest
  before/after the sweep.

P5 commands (sweep, adjudicated residuals):

    cd /Users/sac/avec
    LC_ALL=C grep -rlI 'avec' \
      Cargo.toml Makefile justfile ggen.toml README.md CHANGELOG.md \
      RELEASE_NOTES.md scripts .github docs examples ex \
      | xargs LC_ALL=C sed -i '' \
        -e 's/avec/cgap/g; s/Avec/Cgap/g; s/AVEC/CGAP/g'
    # receipts manifest before the sweep:
    find docs/jira/v26.10.29 docs/jira/v26.10.31 -type f -exec shasum -a 256 {} + \
      > /tmp/receipts_before.sha
    # ... sweep ...
    find docs/jira/v26.10.29 docs/jira/v26.10.31 -type f -exec shasum -a 256 {} + \
      > /tmp/receipts_after.sha
    diff /tmp/receipts_before.sha /tmp/receipts_after.sha   # empty = D7 holds

P5 gate (residuals adjudicated against the allowlist):

    grep -rI 'avec' . --exclude-dir=target --exclude-dir=.git > /tmp/res.txt
    wc -l /tmp/res.txt
    # allowed residuals ONLY:
    #   urn:avec:license:p28-sil:v2.1.0-sovereign  (D2)
    #   historical receipts under docs/jira/v26.10.29|v26.10.31 (D7)
    #   forward-pointer notes in the two milestone indexes (D7)

P5 risk: the broad `s/avec/cgap/g` over prose must not touch the license
instrument file (D2) — exclude LICENSE*.md from the file list explicitly.
Second risk: `scripts/verify_v26.10.31.sh` content sweep is safe, but its
NAME is milestone-versioned; the rename keeps the name (sweep content only)
and the next milestone's verify script inherits the new naming naturally.

## P6 - Downstream lockstep (stardf pin bump, purrdf pin, consumer courts)

Both known consumers verified on disk:

Consumers, as list form:
  /Users/sac/ggen-legacy/stardf/arrow/Cargo.toml:263:avec-arrow = { git = "https://github.com/seanchatmangpt/avec", rev = "67c0ec1" }
  /Users/sac/purrdf/Cargo.toml:145:avec-arrow = { git = "https://github.com/seanchatmangpt/avec", rev = "20248e4" }

Consumer: stardf; Pin site: stardf/arrow/Cargo.toml:263; Old pin: rev 67c0ec1; Files naming avec:
  stardf-process (Cargo.toml:20 + src/log/columnar.rs + src/error.rs), stardf-arrow-
  bench/src/{main.rs,columnar_bench.rs}, stardf ggen.toml:94-100, stardf-arrow-capi (symbol
  stardf_avec_export_stream at src/lib.rs:190; tests c_smoke.rs:70, cross_allocator_court.rs:73,
  alloc_identity.rs:53+203), stardf README + 10 docs/*.md, docs/avec-rehearsal.md, plus
  authority/stardf/license-compat.json (ggen-legacy root; fork-posture row naming crates/avec-
  arrow @ 67c0ec1)

Consumer: purrdf; Pin site: purrdf/Cargo.toml:145; Old pin: rev 20248e4; Files naming avec:
  crates/process/src/log/columnar.rs (30 refs), crates/process/tests/process_courts.rs (1 ref)

P6 sequence:

1. avec rename lands and pushes (W1-W5 phases complete, merge to main).
2. Operator executes D1: GitHub repo rename avec -> cgap. Old URL keeps
   resolving via GitHub redirect.
3. stardf lockstep (W4 lane, /Users/sac/ggen-legacy/stardf/arrow):
   - Cargo.toml:263: rename dep key avec-arrow -> cgap-arrow AND set
     rev = <NEW_HEAD> (the rename-landing commit; do not keep 67c0ec1,
     which predates the rename and has no cgap-* packages).
   - stardf-process/Cargo.toml:20: avec-arrow.workspace = true ->
     cgap-arrow.workspace = true; sweep avec_arrow:: uses in
     stardf-process/src/** and stardf-arrow-bench/src/**.
   - stardf-arrow-capi: rename stardf_avec_export_stream ->
     stardf_cgap_export_stream (src/lib.rs:190) + the three court files
     that import it (c_smoke.rs:70, cross_allocator_court.rs:73,
     alloc_identity.rs:53+203). D3.
   - stardf ggen.toml:94-100: dep IRI
     <urn:stardf-arrow:dep:avec-arrow> -> <urn:stardf-arrow:dep:cgap-arrow>
     and rust:itemName "avec-arrow" -> "cgap-arrow".
   - stardf docs sweep (README + 10 docs/*.md + docs/avec-rehearsal.md),
     subject to the same D7 immutability split stardf applies to its own
     evidence.
4. purrdf lockstep (/Users/sac/purrdf):
   - Cargo.toml:145: dep key + URL cgap-arrow git rev <NEW_HEAD>. This
     also REPAIRS purrdf's currently-broken 20248e4 pin (E0583 chase_lev
     missing), a pre-existing defect the rename lockstep fixes for free.
   - sweep crates/process/src/log/columnar.rs (30 refs) +
     tests/process_courts.rs (1 ref).
5. Consumer courts re-run:
   - avec-side: crates/cgap-ffi/tests/stardf_consumer_court.rs (currently
     untracked; landed in P0) must PASS against the renamed capi.
   - stardf-side: cargo test -p stardf-arrow-capi (all courts incl. the
     renamed symbol), cargo build -p stardf-process.
   - purrdf-side: cargo test -p purrdf-process (process_courts).

P6 gate:

    cd /Users/sac/ggen-legacy/stardf/arrow
    cargo build -p stardf-process
    cargo test -p stardf-arrow-capi
    cd /Users/sac/purrdf
    cargo build -p purrdf-process
    cargo test -p purrdf-process
    # residual token check (absolute paths, cwd-independent):
    grep -rw avec /Users/sac/ggen-legacy/stardf/arrow /Users/sac/purrdf/crates \
      --include='*.toml' --include='*.rs' | wc -l   # 0

P6 risk: GitHub rename (step 2) is operator-sequenced; if stardf/purrdf
commits land BEFORE the GitHub rename, their Cargo.lock will still carry the
old URL, which still resolves via redirect - acceptable transient. The
window where stardf pin = NEW_HEAD while the avec side is not yet pushed
must not exist: the pin bump commit and the push are one coordinator action.

## P7 - Verification ladder

Order (cheapest high-info first; every step must exit 0 on the renamed
tree at /Users/sac/cgap):

    1. cargo test --workspace                      # 661-test baseline class
    2. make gates                                  # gate0..gate3 in order
    3. make lint && make publish-check             # clippy; publish dry-runs
    4. python3 scripts/symbol_gate.py              # renamed dylib names,
                                                   # D8-frozen version needle
    5. escript test/beam_test.escript              # renamed NIF, real BEAM
    6. (cd ex && mix test)                         # renamed Elixir wrapper
    7. bash scripts/c99_smoke.sh                   # renamed headers
    8. make gate3                                  # pyarrow wire parity
    9. just verify                                 # full local ladder
   10. just courts                                  # 3 adversarial courts
   11. P6 commands (stardf + purrdf courts)
   12. Anti-vacuity for the rename itself (a rename court that cannot
       fail is decoration):

       # 12a. positive: renamed export is present, exactly once
       nm -gU target/release/libcgap_ffi.dylib | grep -c '^_cgap_version$'  # 1
       # 12b. negative: strip the symbol from a /tmp COPY and watch gate3
       #      FAIL (proves the court binds the renamed symbol, not the name
       #      in a comment):
       cp target/release/libcgap_ffi.dylib /tmp/negctl.dylib
       # (mutate/remove _cgap_version in the copy with your linker tool of
       #  choice; run gate3_cdata.py pointed at the copy; it MUST exit
       #  nonzero; delete the copy)

P7 risk: step 12b mutates only a /tmp copy (repo untouched) - same
discipline as the repo's shim-court pattern: prove the gate can flip or the
gate is decoration. If the operator prefers the in-repo precedent, port the
check into scripts/ as a `--negative-control` mode of symbol_gate.py as a
follow-up ticket.

## Execution DAG and parallel lanes

    P0 (serial, coordinator: clean tree, index amendment, D-row signoffs)
     |
     |--- W1: crates/** rename (P1 dirs+manifests+sources, P2 exports,
     |        P3 NIF table + escripts + ex/)          [lane A]
     |
     |--- W2: ggen-marketplace packs/avec-pack -> cgap-pack  [lane B,
     |        separate repo /Users/sac/ggen-marketplace]
     |
     |--- W3a: root config + scripts + Makefile/justfile + ci.yml +
     |         README/CHANGELOG/RELEASE_NOTES + docs/ living sweep,
     |         examples/, LICENSE excluded (D2)      [lane C]
     |
     |--- W4-prep: stage stardf + purrdf diffs (no pin bump yet) [lane D]
     |
     v  (join: W1 + W2 + W3a)
    W3b: ggen.toml flip + git mv include/* + ggen sync run x2 + make gate0
     |
     v  (needs W1 pushed + D1 GitHub rename)
    W4: stardf + purrdf pin bumps to NEW_HEAD + symbol rename (D3) +
        consumer courts re-run                       [lane D executes]
     |
     v
    OPERATOR: local dir rename /Users/sac/avec -> /Users/sac/cgap (D1b)
     |
     v
    P7 verification ladder (full, on final layout)
     |
     v
    Receipts minted; 00-index.md standing table updated.

Parallel-safe: W1, W2, W3a, W4-prep are disjoint by file ownership:
W1 owns /Users/sac/avec/crates/**; W2 owns
/Users/sac/ggen-marketplace/packs/**; W3a owns everything under
/Users/sac/avec OUTSIDE crates/ and include/ and ggen.toml (those flip in
W3b after the join); W4-prep owns /Users/sac/ggen-legacy/stardf/** and
/Users/sac/purrdf/** (staged, uncommitted). One writer per file; lanes
never run git transitions (coordinator owns commits, push, and the D1/D1b
operator steps).

## Risk register (consolidated)


**id:** R1; **risk:** purrdf pin 20248e4 is broken today (E0583); **mitigation:** P6 lockstep
  lands NEW_HEAD, repairing it; until then purrdf courts cannot run against old rev at all - P6
  purrdf gate is defined at NEW_HEAD only.

**id:** R2; **risk:** untracked stardf_consumer_court.rs + dirty manifests at HEAD;
  **mitigation:** P0 precondition: land or snapshot before lanes open.

**id:** R3; **risk:** ggen sync regen without the renamed pack re-mints avec_* headers;
  **mitigation:** P4 step 1 lands first; step 4 gated on `test -d ../ggen-marketplace/packs/cgap-
  pack`.

**id:** R4; **risk:** OTP load_nif appends only `.so` (escript symlink trick); **mitigation:** P3
  keeps the mechanism, renames the names; BEAM gate fail-fast at load.

**id:** R5; **risk:** Half-renamed NIF module (Erlang vs Rust table mismatch); **mitigation:** One
  atomic P3 commit; load failure is loud, not silent.

**id:** R6; **risk:** Version bump riding the rename breaks symbol_gate needle; **mitigation:** D8
  freezes 26.10.29 through the window.

**id:** R7; **risk:** Cargo.lock in stardf/purrdf carries old URL post-rename; **mitigation:**
  Redirect keeps it resolving; P6 regenerates locks.

**id:** R8; **risk:** Receipts rewritten by the prose sweep; **mitigation:** D7 + sha256
  before/after diff gate (P5).

**id:** R9; **risk:** License URN keeps "avec" namespace; **mitigation:** Allowlisted residual
  (D2); README note in P5.

**id:** R10; **risk:** 02-rename-plan.md not in the index filename contract; **mitigation:** D11:
  agent-1/operator amends 00-index.md in P0.

**id:** R11; **risk:** crates.io names taken; **mitigation:** D12 availability check before P1
  lands.

**id:** R12; **risk:** stardf D3 symbol rename misses a caller; **mitigation:** P6 gate: cargo
  test -p stardf-arrow-capi must pass after rename; grep residual 0.


## Receipt

- Subject: /Users/sac/avec/docs/jira/v26.11.1/02-rename-plan.md
- Written by: RENAME-PLANNING LANE 2, 2026-10-31
- Scope discipline: reads across /Users/sac/avec,
  /Users/sac/ggen-legacy/stardf, /Users/sac/purrdf, and
  /Users/sac/ggen-marketplace; writes confined to
  /Users/sac/avec/docs/jira/v26.11.1/ only.
- Verification performed while writing this plan (in-session, real
  commands): git status/remote/log reads; grep censuses; manifest reads;
  NIF wire-name and C-export symbol lists derived mechanically from the
  sources; purrdf crate-name readback. No builds or tests executed
  (read-only lane).
- Known blemishes, disclosed: three drafting-corruption episodes during
  long-form writes; each was repaired in place and the file was verified
  line-by-line at the end (the two Read passes and the residual-token grep
  below are that verification). Cosmetic mid-word hyphen wraps remain in a
  few wrapped rationale lines.
- Standing: the plan is UNVERIFIED (nothing executed); the file itself is
  PARTIAL_ALIVE until the coordinator lands it (existence + content
  verified; no repo gate exercises a planning doc).
- Final integrity pass (real commands, computed against the plan body,
  excluding this receipt block): residual corruption-token grep
  (STOP/placeholder/truncated/cgapn/expat/WRONG PATH/Users-slash-2): 0
  hits; over-100-char lines: 6, all inside code fences or quoted grep
  evidence (commands cannot wrap without changing semantics); full-file
  Read verification done in two passes over all lines.