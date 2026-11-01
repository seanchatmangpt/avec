# A15 Wave 8 Audit Receipt

- **Lane**: A15 (`crates/audit`: `src/powl.rs`, `src/ocel2_slab.rs`), Wave 8
  defect-repair driven by the wave-7 court receipts, v26.11.1
- **Mode**: failed-lane restart (A08-wave7 / A16-wave6 method): no A15
  wave-7 receipt file exists on disk, so every wave-7 claim was re-derived
  from the landed artifact — the real gate was run, the wave-7 surface was
  attacked with compiler-level mutations in a sandbox, and the four real
  defects found were repaired with a flipping court each
- **Subject**: repo `/Users/sac/cgap`, branch `main`, working tree on top of
  coordinator scaffold c83619d; ONE canonical checkout, no worktrees/clones;
  no git write commands run (read-only `git status`/`git log`/`git diff`
  only)
- **Verdict**: **ALIVE** — real gate `CARGO_TARGET_DIR=target-a15 cargo test
  -p cgap-audit` exit 0, **32/32** passed (28 wave-7 + 4 wave-8 courts),
  run fresh this session (three times: pre-repair 28/28, post-repair 32/32,
  final re-run 32/32); **zero vacuous courts** — all 6 mutation classes
  flip their named courts (four wave-8 named, two wave-7 named re-witnessed
  at compiler level); repo left with only the two lane-owned files (+
  this receipt) changed.

## Wave-8 defect repairs (each with a court that flips its baseline)

| id | file | defect (re-derived) | repair | court (flip witnessed) |
|---|---|---|---|---|
| D3 | `powl.rs` | Verdict evidence nondeterminism: `check` iterated bindings in `HashMap` (per-instance RandomState) order, so with >= 2 violating bindings the evidence event ids in the frozen wire ABI varied run-to-run — the verdict was not replayable | `BindingIndex` now orders bindings by ascending first-event row; the verdict is a pure function of (log bytes, model) | `verdict_is_deterministic_across_repeated_index_builds` — 64 fresh index builds must yield the identical verdict (P(pass on baseline) ~ 2^-63) |
| D4 | `ocel2_slab.rs` | D1-sibling: `decode` accepted SPARSE-but-ascending intern codes; `[10, 65535]` saturates `next_code` onto the taken last code and the forward-only mint scan then answers `InternTableFull` while 65,533 codes are free — a wrong typed answer, same bijection-break class as D1 | decode enforces the DENSE ascending law (codes 1..=n, exactly the wire `encode` emits); the saturated full table (n=65535) is caught by intern's cap check instead | `intern_decode_refuses_sparse_codes` — sparse head/gap/saturation fixtures refused typed |
| D5 | `ocel2_slab.rs` | `validate_log` accepted duplicate object-registry rows (key collision the append path cannot emit); `open_slab`'s index build silently absorbed the collision by last-wins overwrite | validate refuses the collision typed; `open_slab` inherits the refusal via its `validate_log` gate | `validate_log_refuses_duplicate_object_rows` |
| D6 | `powl.rs` | `compile` accepted sibling children with OVERLAPPING leaf-activity sets (`Choice[A, A]`); `ordinal_of` attributes every event to the first matching child, so the construct check can never fire — a silent tautology | overlap refused typed (`InvalidModel`) at compile, sibling sets must be pairwise disjoint (incl. through nesting) | `overlapping_sibling_leaf_sets_refused_typed` |

## Court attack (all mutations in `/tmp/a15-w8-sb` only)

Method: standalone workspace at `/tmp/a15-w8-sb` (`crates/{audit,
cgap-arrow,cgap-parquet}` + `[workspace.package]` v26.10.29 edition 2021,
dev `panic = "abort"` matching the root + `LICENSE.md`; sandbox sources
sha256-identical to repo post-repair: `powl.rs af972866…b7d6c`,
`ocel2_slab.rs e34a6c20…29b68`). Baseline 32/32 green exit 0. One mutation
at a time via python3 exact-string replacement (anchor count asserted == 1
before every write); restore from `.pristine` copies after every mutation
with sha256 fail-closed compare; sandbox == repo byte-exact at the end.
Runs: `cargo test -p cgap-audit --lib --no-fail-fast`, flip-set = full
failed list, exit 1 every time.

| Mut | Named mutation (compiler-level) | Outcome |
|---|---|---|
| M-D3 | determinism sort removed (`entries.sort_by_key` → `;`) | **D3 court only** — isolated flip |
| M-D4 | density law reverted to strictly-ascending (wave-7 baseline law) | **D4 court only** — isolated flip |
| M-D5 | duplicate-row refusal disabled (`if rows.windows(2)…` → `if false {`) | **D5 court only** — isolated flip |
| M-D6 | overlap refusal disabled (dead branch) | **D6 court only** — isolated flip |
| M-D1 | wave-7 named: monotonicity check removed entirely (`if code == u16::MAX {`) | **D1 court flips** + D4 court flips as structural collateral (D4's law subsumes D1's; both fixtures are non-dense) |
| M-D2 | wave-7 named: intern length refusal disabled (`if s.len() > u16::MAX…` → `if false {`) | **D2 court only** — isolated flip |

Note: a first M-D1 attempt (`code == u16::MAX + 1`) was invalid Rust
(const-overflow compile error, no test run) and was re-run with a
compiling anchor — recorded, not silent.

## Verified surfaces (this session, command + exit)

| Surface | Spec | Evidence |
|---|---|---|
| Real gate | lane doc §6.3, DoD 10 | `CARGO_TARGET_DIR=target-a15 cargo test -p cgap-audit` exit 0, 32/32 (28 wave-7 courts unchanged + 4 wave-8) |
| Compression court + compress-drop | §6.1, DoD 5 | 10-row round-trip; seeded 9-row drop mutant refused `RowPinMismatch`; u16 class too |
| ocel-mutate court | §6.1, DoD 6 | payload/header/magic mutation classes caught; restores byte-exact |
| forged-header / intern-version / trailing-bytes courts | Wave-4 | typed refusals with byte-exact restores |
| string-leak court | §6.1, DoD 7 | append alloc-free on reserved log; unreserved control allocates |
| alloc court (powl) | §5 | check path alloc-free beyond the binding index; index control allocates |
| Six construct courts | §6.1, DoD 4 | conforming accepted, violating rejected with pinned evidence |
| D1/D2 (wave-7) | module header | flips re-witnessed at compiler level (M-D1/M-D2) |
| DoD 8 | §7.8 | zero `unwrap`/`expect`/`panic!` on non-test paths (scan re-run this session) |

## Divergences (doc yields; recorded, not silent)

1. **D-A15-1 (missing subject)**: no A15 wave-7 receipt file exists on
   disk; the wave-7 repairs D1/D2 are receipted in-code
   (`ocel2_slab.rs` module header, Wave-7 section). Cite this audit
   receipt plus the artifact; do not cite a lane receipt file.
2. **D-A15-2 (gate command)**: lane doc §6.3 names
   `cargo test -p cgap-audit --features ocel2`; the crate defines no
   features and `crates/audit/Cargo.toml` is outside this lane's lease —
   the gate is `cargo test -p cgap-audit` (exit 0). Doc yields.
3. **D-A15-3 (owner path)**: lane doc §1 lists owner files under
   `crates/cgap-audit/src/`; the actual crate dir is `crates/audit/`
   (published name `cgap-audit`). The dispatch's paths
   (`crates/audit/src/…`) are the ones used. Doc yields.
4. **D-A15-4 (court message amended, disclosed)**: the wave-7 D1 court's
   expected refusal message changed
   ("intern codes not strictly increasing" → "intern codes not dense
   ascending") because D4's dense law is strictly stronger and subsumes
   it. The `[2, 1]` fixture is refused under both laws; flip re-witnessed
   (M-D1). No court was weakened.
5. **D-A15-5 (unreaped sandbox lease)**: the mutation sandbox
   `/tmp/a15-w8-sb` could not be deleted at the end (rm denied by the
   permission system). It is outside the repo tree, contains no repo
   state, and is ephemeral; the coordinator may ignore it. Repo tree
   clean: only `powl.rs`/`ocel2_slab.rs` (lane lease) + this receipt
   changed by this lane.

## Standing

- POWL v2 verifier + OCEL 2.0 columnar flight recorder (`powl.rs` +
  `ocel2_slab.rs`): **ALIVE** — `cargo test -p cgap-audit` exit 0, 32/32,
  re-run this session in repo and (post-repair) sandbox.
- Wave-8 courts D3/D4/D5/D6: **ALIVE**, flips witnessed (M-D3..M-D6,
  isolated).
- Wave-7 courts D1/D2: **ALIVE**, compiler-level flips re-witnessed this
  session (receipts cited in-code were not on disk).
- All §6.1 courts (six constructs, compression + drop, ocel-mutate,
  string-leak, alloc, export, forged-header, intern-version,
  trailing-bytes): **ALIVE**.
- Perf numbers: none claimed (doc targets remain TARGETs; no wall-clock
  method run).
- Known remaining gap (recorded, not repaired — no court, zero-new-
  features law): `validate_log` does not bounds-check
  `OcelObjectRow.first_event_idx`/`last_event_idx` against the event row
  count; nothing in-repo reads them on a check path. Follow-up candidate
  with its own court.

## See Also

- `docs/jira/v26.11.1/lanes/A15-powl-ocel2.md` (normative lane doc)
- `docs/jira/v26.11.1/waves/a08-wave7-audit-receipt.md` (same restart
  method, D-A08-1 precedent)
- `docs/jira/v26.11.1/waves/a16-wave6-audit-receipt.md` (method origin)
- `crates/audit/src/powl.rs`, `crates/audit/src/ocel2_slab.rs` (audited
  artifacts)
