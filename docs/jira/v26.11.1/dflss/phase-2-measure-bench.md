# phase-2-measure-bench

Status: PHASE PROMPT (DMADV Phase 2 — Measure), converted 2026-11-01 from
`docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` Section 4, Phase 2
("The Divan Benchmark Membrane", charter lines 54-58). Per-lane bench map,
experiments, and dispatch shape live in
`../bench-campaign-charter.md` (cited, not duplicated) — every executing
agent reads that map for its lane→benchmark row before writing code.

Standing preamble: every performance number in this file carries standing
`TARGET` (directional nominal). Nothing graduates to `MEASURED` except
through the receipt + court protocol of Section 7. Nothing rounds up.
`UNVERIFIED` is the default state of every claim until its receipt lands.

## 1. Agent contract (binds every executing agent)

- Repo: `/Users/sac/cgap` — the ONE canonical checkout. No worktrees, no
  clones, no per-agent copies.
- Never run `git add`/`commit`/`push`/`checkout`/`stash`; the coordinator
  owns all git transitions. Read-only git (`status`/`diff`) allowed.
- Lease = exactly the files your dispatch names: your bench file(s) under
  `crates/bench/` plus your lane's receipt
  `docs/jira/v26.11.1/waves/bench-<lane>-receipt.md`. Write nothing else.
- No `Process.sleep` anywhere in bench or court code — the runtime owns
  time; poll observables with bounded deadlines.
- Exit-code contract: `0` pass / `1` gate fail / `2` subject missing /
  `3` loud skip. A skip is loud (`[SKIP] <reason>`), never a fake pass.
- Standing vocabulary only: `ALIVE` / `PARTIAL` / `BLOCKED` / `REFUSED` /
  `UNSUPPORTED` / `UNVERIFIED`.
- Chicago discipline: real kernel, real buffers, real memory; assert final
  state. No mocks, no stub kernels, no fabricated timing.

## 2. Inputs (existence witnessed 2026-11-01, this session)

- `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` — EXISTS. Section 4
  Phase 2 is the source being converted; risk matrix lines 101-105 carry
  the vacuous-benchmark and seed-drift controls.
- `docs/jira/v26.11.1/bench-campaign-charter.md` — EXISTS. Per-lane bench
  map + dispatch shape. Consult; do not duplicate into receipts.
- `crates/bench/` — EXISTS; workspace member. On-disk manifest:
  `[dev-dependencies] divan = "0.1.21"`; path deps on `cgap-arrow`,
  `cgap-blake3`, `cgap-parquet` only; existing `[[bench]] name =
  "v26_10_31", harness = false`.
- `crates/bench/benches/v26_10_31.rs` — EXISTS; Divan precedent using
  `divan::black_box` (witnessed at lines 24, 62, 92-93, 104).
- `crates/bench/src/bin/neon_ab_bench.rs:90` — EXISTS; seed precedent:
  `pattern(0xBF58_476D_1CE4_E5B9, n)`.
- `scripts/zero_string_gate.py` — EXISTS; Gate 2 string diode
  (`--self-check` = non-vacuity proof).
- `scripts/boundary_gate.py` — EXISTS; campaign boundary gate.
- `scripts/projection_coverage.py` — NOT ON DISK as of 2026-11-01; do not
  invoke it in Phase 2 (charter CTQ row C10/C11 defers to Phase 4).

## 3. Membrane construction checklist (all items mandatory, in order)

Each item carries its own witness command; an item without its witness is
not done.

- [ ] M-1 Membrane home: `crates/bench/` (workspace member). ALL Divan
  benches land there. No bench code lands in any kernel crate.
- [ ] M-2 Exact divan pin: run `cargo search divan` at authoring time and
  pin the exact `0.1.x` version returned; record the pin in every lane
  receipt. Witnessed this session: `cargo search divan` exit 0 -> latest
  `0.1.21`, equal to the on-disk
  `[dev-dependencies] divan = "0.1.21"` in `crates/bench/Cargo.toml`.
  Never a caret/wildcard range; an exact pin only.
- [ ] M-3 Path deps only: `crates/bench` depends on kernel crates by
  `path` (witnessed: `cgap-arrow`, `cgap-blake3`, `cgap-parquet`). No new
  registry deps beyond the divan dev-dep.
- [ ] M-4 Bench target shape: `[[bench]] name = "kernels", harness =
  false` with entry `fn main() { divan::Divan::default().run(); }` in
  `crates/bench/Cargo.toml`. `harness = false` is mandatory — Divan owns
  the runner; libtest must never wrap it.
- [ ] M-5 Kernel purity: no `[[bench]]` target and no divan entry in any
  kernel manifest. Witness:
  `grep -n "divan\|bench" crates/cgap-*/Cargo.toml` -> zero matches
  (exit 1; witnessed 2026-11-01). divan/std must never enter a `#![no_std]`
  kernel dep graph; bare-metal cross builds stay clean.
- [ ] M-6 Gate 1 coupling: `cargo test --workspace` compiles bench targets
  without running them — the membrane build is witnessed by Gate 1 for
  free; a membrane build break is a Gate 1 break.
- [ ] M-7 Seeding rule of Section 4 appears verbatim in every corpus
  generator (grep for the constant is the witness).
- [ ] M-8 black_box + checksum rules of Section 5 wired into every bench.
- [ ] M-9 Anti-vacuity flip court of Section 6 executed per lane BEFORE
  any `TARGET`->`MEASURED` graduation.

## 4. Deterministic seeding rule (normative)

All benchmark corpora are seeded deterministically with the canonical
SplitMix64 multiplier, pinned character-for-character:

0xBF58_476D_1CE4_E5B9

```rust
const CANONICAL_SEED: u64 = 0xBF58_476D_1CE4_E5B9;

/// SplitMix64 mix step; the multiplier above is the canonical constant.
/// Normative for all bench corpora in this campaign.
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(CANONICAL_SEED);
    z = (z ^ (z >> 27)).wrapping_mul(CANONICAL_SEED);
    z ^ (z >> 31)
}

/// Corpus stream: golden-ratio increment, then canonical mix.
fn corpus_word(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mix64(*state)
}
```

Rules:

- Every corpus generator contains the exact literal
  `0xBF58_476D_1CE4_E5B9`. On-disk precedent:
  `crates/bench/src/bin/neon_ab_bench.rs:90`
  (`pattern(0xBF58_476D_1CE4_E5B9, n)`).
- Any other variant (for example `...E5BA`, or a de-digit-grouped rewrite)
  is the known write-corruption class: the artifact is REFUSED, not
  repaired in place. Re-derive from this section.
- Seeds are `const`, never runtime-random. Two runs of the same bench must
  feed the kernel identical bytes; a corpus that varies run-to-run is
  REFUSED (it makes the Divan table non-replayable).
- Before any gate run: working-tree diff check for constant drift plus the
  `12/12` court verdict baseline (charter risk matrix, line 103) must pass.

## 5. black_box + checksum rules (anti-DCE, Chicago realism)

- Wrap every consumed output in `std::hint::black_box(...)` (or Divan's
  `divan::black_box`, same semantics). On-disk precedent:
  `crates/bench/benches/v26_10_31.rs` lines 24, 62, 92-93, 104.
- Assert a checksum over results after the bench loop; a bench that cannot
  show its final state is not Chicago. The assertion runs with the bench
  and fails it (nonzero exit) on mismatch — a wrong kernel output aborts,
  never re-runs silently.
- Chicago-school realism: real collaborators only — real kernel entry
  points compiled from path deps, real buffers, real memory; assert final
  state, never call counts. No mocks, no stub kernels, no fabricated
  timing, no synthetic data path standing in for the kernel. A bench that
  fakes a collaborator is REFUSED.
- DCE presumption: a `0 ns` reading or a mutation-invariant column is
  presumed dead-code elimination until the Section 6 court proves
  otherwise.
- Runs: min-of-N across runs; flat-tail claims cite Divan's per-iteration
  distribution; no `Process.sleep` in any bench or harness loop.

## 6. Anti-vacuity measurement validation court (ENFORCED)

A benchmark that cannot flip is decoration. Each bench names ONE structural
mutation that must move its measured Divan column; the court flips that
mutation in a sandbox and reads the column. This is an enforced court, not
guidance: no flip, no graduation.

Court shape follows the repo shim-court protocol: mutate `/tmp` sandbox
copies, never the shared tree; sha256 fail-closed restore.

Protocol (per bench):

1. BASELINE — sha256 the bench file and any touched kernel file.
2. BEFORE — run the bench, record the target column value.
3. SHIM — apply the named structural mutation in a `/tmp` sandbox copy
   (mark the mutation `SHIM-COURT` in the diff).
4. AFTER — rerun; the target column MUST move beyond run-to-run noise.
5. RESTORE — sha256 byte-exact vs BASELINE; mismatch = fail-closed abort;
   the court run is VOID.

Mutation classes (non-exhaustive; each bench picks exactly one):

| Mutation (structural, named by the bench) | Column that must move |
| --- | --- |
| inject a heap `Vec` intermediate into decoding | alloc column leaves `0 B` |
| add a branch to the hot path | ns/frame (or ns/pair) moves |
| swap the seeded corpus for zeros | throughput column moves |
| drop `black_box` from the consumed output | court must catch it |

Verdicts:

- Column moved -> bench `ALIVE`; the graduation path opens.
- Column invariant -> bench `VACUOUS`; `TARGET`->`MEASURED` graduation is
  `REFUSED` (exit `1`).
- Bench file missing -> exit `2` (`[SKIP] subject missing`).
- Mutation infeasible -> exit `3` (`[SKIP] <reason>`); recorded, never
  silent.
- Mutation-free green run = `VACUOUS GATE` (exit nonzero). A court that
  cannot flip is `FAIL`.

## 7. Receipt format + graduation protocol

Per-lane receipt = `docs/jira/v26.11.1/waves/bench-<lane>-receipt.md`
containing, in order:

1. Header: lane id, date, divan pin (from M-2), host one-liner (CPU/OS,
   from `sysctl`/`uname` output pasted into the receipt).
2. The exact `cargo bench` command run (e.g.
   `cargo bench -p bench --bench kernels -- <filter>`) and its exit code.
3. The FULL Divan table text, verbatim, untruncated — not a summary, not a
   screenshot, not selected rows.
4. The Section 6 flip-court verdict for that bench (mutation named,
   BEFORE column, AFTER column, restore sha256 match, verdict token).
5. Graduation lines: each claim converted, old standing `TARGET`, new
   standing `MEASURED`, citing the table row above it.

Graduation rules:

- `TARGET`->`MEASURED` requires BOTH (a) the lane receipt exists with the
  full Divan table, and (b) the Section 6 flip court passed for that
  bench. One without the other is not graduation; the claim stays
  `UNVERIFIED`/`TARGET`.
- Graduation happens in the receipt only — code comments and docs keep
  `TARGET` standing.
- Min-of-N across runs; the receipt names N. No `Process.sleep`.
- Aggregation into `waves/bench-campaign-receipt.md` is the gate-runner's
  step, not the lane's.

## 8. Claims under conversion (all standing TARGET; charter Section 3)

| CTQ claim (lanes) | Design target (TARGET standing) | Instrument |
| --- | --- | --- |
| Frame admission (A04, A14) | `~4.0 ns`/frame; flat p99 tail | Divan latency (`cgap_admit_frame`) |
| Hot-path alloc (A01, A02) | `0 Bytes` on fast path | Divan alloc column + `nm` symbol table |
| 24B scan (A06, A11-A13) | `> 1.0 GB/s`; sub-us unify; N in [64, 1M] | Divan throughput table |
| 640-bit verify (A17, A19) | sub-us verify; min `extern "C"` delta | Divan latency + flip court |
| Bare metal (riscv32imc) | `< 64 KB` Flash; `< 32 KB` SRAM | `llvm-size` over 32-bit ELF |
| String diode (Gate 2) | `0` `.rodata` domain strings | `zero_string_gate.py` (+ self-check) |

Host-vs-silicon boundary: Divan reports host-machine numbers only. Gate 5
silicon fit stays `llvm-size` over the cross-compiled 32-bit ELF artifact;
it is never a Divan measurement. Divan allocation counting covers the
profiled closures only — zero-alloc Gate 2 evidence remains `nm` /
zero-string based.

## 9. Execution ladder (per-lane dispatch)

1. Read the Section 2 inputs plus your lane's row in the bench map
   (`../bench-campaign-charter.md`).
2. Build or extend the membrane per the Section 3 checklist.
3. Write your bench: seeded corpus (Section 4), black_box + checksum
   (Section 5), named mutation (Section 6).
4. Run `cargo bench -p bench --bench kernels -- <your-filter>`; capture
   the exit code and the full Divan table text.
5. Write your lane receipt (Section 7).
6. Execute the Section 6 flip court for your bench; record the verdict in
   the receipt.
7. Graduate numbers only per Section 7.

## 10. Provenance

- Source: `00-dflss-charter-source.md` Section 4 Phase 2 (lines 54-58);
  risk-matrix controls (lines 101-105); CTQ targets (lines 35-42).
- Companion (cited, not duplicated): `../bench-campaign-charter.md` —
  membrane rules, per-lane bench map, experiments, dispatch shape, caveats.
- Conversion standing: this file is a phase prompt, not an execution
  receipt. Phase 2 execution standing is carried by the per-lane receipts
  it dispatches; until they land, all claims here remain TARGET/UNVERIFIED.
