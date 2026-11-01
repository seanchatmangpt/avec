# gate-runner-playbook

Charter-conversion artifact C04 (DFLSS v26.11.1). Converts the five Morning
Definition-of-Done gates (`docs/jira/v26.11.1/dflss/00-dflss-charter-source.md`,
section 4 Phase 5) into runnable gate-runner checklists over the single
canonical checkout at `/Users/sac/cgap`.

Standing law: every performance and footprint number below is a **TARGET**
(directional design nominal) until a benchmark or size receipt graduates it.
Vocabulary: `ALIVE / PARTIAL / BLOCKED / REFUSED / UNSUPPORTED / UNVERIFIED`.
A gate leg with no runnable command on disk is marked PENDING-LAND; its
planned command is intent, never executed work.

## 0. Shared gate contract

Fail-closed exit contract (campaign law): `0` pass, `1` gate fail / typed
refusal (includes `VACUOUS GATE`), `2` subject missing, `3` loud skip.

Per-script reality (verified this session, do not assume):
- `scripts/boundary_gate.py`: implements `{0,1}` only — `EXIT_PASS = 0`,
  `EXIT_REFUSE = 1` (`scripts/boundary_gate.py:139-141`).
- `scripts/zero_string_gate.py`: implements `{0,1}` only — `main()` returns 1
  on every `GateRefused`/internal-error path (`zero_string_gate.py:2231-2238`).
- `scripts/dod.sh`: full `{0,1,2,3}` contract (header: 0 no red, 1 any red,
  2 usage error, 3 fail-closed manifest missing/zero entries).

Divergence note: the campaign classes 2 (subject missing) and 3 (loud skip)
are implemented today only in `scripts/dod.sh`. The two Python gates emit
loud `[SKIP]`/refusal rows on stderr but never dedicate an exit code to them;
the runner records rows verbatim alongside the exit.

Anti-vacuity law (binds every gate): a green run with no witnessed mutation
is vacuous. Each gate names its mutation below. A court that cannot flip is
FAIL. A mutation-free green gate run is VACUOUS and must itself exit nonzero.
Skips are loud (`[SKIP] <reason>`), never silent, never a faked pass. No
`Process.sleep` in any gate step — poll observables with deadlines.

Evidence discipline: every gate run captures (a) exact command line, (b) exit
code, (c) the gate's own SUMMARY/verdict lines verbatim, (d) ledger/report
JSON path + sha256 when the mode emits one, (e) host fingerprint via
`rustc -vV` and `git rev-parse HEAD` — read-only git only.

Prohibitions binding the runner: no `git add/commit/push/stash/checkout`, no
registry `publish`, no worktrees or clones; mutations happen only in /tmp or
gate-managed sandbox copies with sha256 fail-closed restore (never the shared
tree); unclassified sleeps are banned (runtime owns time).

## GATE-1 Workspace Purity

Law: the workspace builds clean and carries zero external runtime
dependencies behind the kernel crates.

### Commands
1. `cargo build --workspace` -> expect exit 0, zero errors. (Command ABI
   verified on disk; not executed by C04 this session.)
2. `python3 scripts/boundary_gate.py --deps` -> expect exit 0 with `E=0`
   runtime externals for every covered crate (A20 §6.4). Consumes the A07
   register at runtime; register unavailable/refused/unparsable/wrong
   version/zero patterns each refuse typed (fail-closed consumption).
3. `python3 scripts/zero_string_gate.py --register` -> expect exit 0 and a
   non-empty register (version 2, >= 1 pattern). Executed this session:
   exit 0 (appendix §8).

### Evidence
Build stderr tail (0 errors), `--deps` report JSON, register JSON sha256.

### Vacuity conditions
- Non-vacuity leg before any seal run: `python3 scripts/boundary_gate.py
  --shim-inert` -> expected exit 1 (`VACUOUS GATE` refusal machinery proven
  alive), paired with `--shim-court-only` -> expected exit 0.
- `--deps` green while the register is empty/unparsable must be impossible
  (typed refusal); if ever observed, the gate binary itself is broken.

### Loud-[SKIP] rules
None chartered for GATE-1. A covered crate with zero artifacts is a typed
refusal (exit 1), never a skip; do not downgrade a refusal.

## GATE-2 Binary/Symbol Purity

Law: zero ontology-vocabulary tokens in execution/consumption artifact
fail-set sections (`.rodata`/`.text` families; `__TEXT` on Mach-O), scanned
from COMPILED ARTIFACTS, never sources.

### Commands
1. `cargo build --workspace` first — the scan refuses when a crate scans
   zero artifacts.
2. `python3 scripts/zero_string_gate.py` (default scan) -> exit 0 requires
   ALL of: >= 1 artifact scanned per enumerated non-excluded crate;
   vocabulary loaded non-empty; zero vocabulary hits in fail-set sections;
   zero refusals. `EMITTER-EXEMPT-HIT` rows are loud and count in
   `emitter_hits=N` without feeding the fail predicate (A07 wave-7 producer
   exemption, artifact-scoped).
3. `python3 scripts/zero_string_gate.py --self-check` -> exit 0 only if the
   poison fixture HITs (naming the planted token) and the clean twin does
   not. Executed this session: exit 0, `SELF-CHECK RESULT PASS`,
   poison `hits=2 fired=True`, clean `hits=0` (appendix §8).
4. `python3 scripts/boundary_gate.py --strings` -> expect exit 0. This is
   the cross-gate seam: it consumes the canonical scan verdict + register at
   runtime; canonical red or typed refusal propagates as CROSS-GATE refusal.
5. `python3 scripts/boundary_gate.py --cross-gate-court` -> expect exit 0.
   Named mutation: `seam-neuter` in a sandbox clone — un-neutered refuses on
   the canonical red; neutered returns to exit 0. A `--strings` that stays
   green under seam-neuter is a vacuous seam; the court exits 1.

### Evidence
SUMMARY line verbatim, `emitter_hits` count, self-check PASS line,
`--strings` report JSON sha256, `[SKIP]` rows verbatim.

### Vacuity conditions
- `--self-check` IS the named mutation probe (poison vs clean twin, real
  cargo builds in a temp dir). A scan green with `scanned=0` is vacuous and
  refused. A `--strings` green while the canonical scan is red must refuse
  (CROSS-GATE reason). `--self-check` is byte-exactness-witnessed: fixture
  artifacts are sha256'd before and after the scan.

### Loud-[SKIP] rules
`[SKIP]` rows name artifact + reason (rustc metadata member, artifact
vanished mid-run under concurrent cargo build). A skip that drops any crate
to zero scanned artifacts must refuse the gate, never pass. Never
`--no-cross-check` at seal (strings(1) cross-extractor is part of the
verdict).

## GATE-3 1M-Row Sovereign Ledger Proof

Law: the pinned deterministic corpus (1,000,000 rows; seed
`0xA20C0FFEE5A7E000`, `boundary_gate.py:145-146`) is byte-reproducible, and
the mutation campaign over the court kernel kills at `kill_rate >= 0.98`
with witnessed non-vacuity (A20 §9 items 2 and 5).

### Commands
1. `python3 scripts/boundary_gate.py --shim-court-only` -> expect exit 0
   (§6.3: the committed known-killable mutant — SplitMix64 mix-constant
   bump — must be killed).
2. `python3 scripts/boundary_gate.py --shim-inert` -> expect exit 1
   (negative control: a pass means the gate itself is broken).
3. `python3 scripts/boundary_gate.py --mutate --report <path>.json` ->
   expect exit 0 with `kill_rate >= 0.98` in the report and the first
   ledger row a SHIM-COURT kill. Campaign holds `target/.a20-campaign.lock`
   (a second concurrent campaign refuses typed, exit 1). Not executed by
   C04 (heavy, ~15 min class); ABI verified via `--help` exit 0.
4. Corpus receipt (A20 §9.5): the 1M-row generator's pinned sha256 matches
   the regenerated corpus hash, asserted inside court run #1 of the
   `court_verdict` harness; a mismatch fails the suite before any other
   court. Harness status: PROPOSED scaffold — `crates/cgap-court/` contains
   `Cargo.toml` only this session (no `src/`, no `tests/`). Until it lands,
   this leg is UNVERIFIED and the seal run is BLOCKED on A20's
   `court_verdict` landing — loudly, never silently skipped.

Two pinned constants; do not conflate:
- `0xBF58_476D_1CE4_E5B9` — SplitMix64 multiplier seeding the Phase-2 Divan
  benchmark corpora (charter §2, §4 Phase 2).
- `0xA20C0FFEE5A7E000` — boundary-gate §5.1 corpus seed (display form).

### Evidence
Ledger JSON (schema keys: abi_version, mutant_id, verdict, killed_by,
passing_courts, mutated_sha256, kill_rate) + report sha256; first-row
SHIM-COURT verdict line; kill_rate line.

### Vacuity conditions
Legs 1+2 are the non-vacuity pair (kill must happen; inert shim must
refuse). A green `--mutate` whose first ledger row is not a SHIM-COURT kill
is VACUOUS — the gate itself refuses exit 1 per §6.3. Survivors are
`SurvivorDefect` rows until hand-triaged via `--equivalent` (one-line
justification, recorded); an untriaged survivor refuses the gate.

### Loud-[SKIP] rules
An absent lane surface at seal time is recorded `[SKIP] <reason>` in the
ledger (A20 §10) — loud, never faked as a pass. Kernel-target campaigns
(`--targets`) without the committed shim refuse fail-closed
(SHIM HOST ABSENT / SHIM SITE NOT FOUND).

## GATE-4 640-Bit Spoliation & Liability Metering

Law: every one of the 640 bits of the 80-byte P28LicenseAttestation halts
(traps) on a single-bit flip — 640/640 per record per format (A17 mutation
court) — and every `CgapStatus` value is reachable via an injected fault on
real inputs (A19 §6.2 status court). Liability metering (ASC 450) lives in
`crates/cgap-p28/src/asc450.rs`.

### Commands
1. `cargo test -p cgap-p28` -> expect exit 0. In-crate courts exist
   (`attestation.rs` `#[cfg(test)]` from line 740; 10+ `#[test]` fns
   witnessed by grep this session). Not executed by C04 (heavy).
2. Exhaustive single-bit-flip sweep: chartered (A17: "640 mutations x 2
   records x 2 formats"; "every single-bit mutation must halt") but NOT
   landed — `crates/cgap-p28/tests/` is absent this session. Planned shape:
   `cargo test -p cgap-p28 --test spoliation_bitflip_sweep` (final name set
   by the A17 owner). Standing: UNVERIFIED until the sweep lands and runs;
   never fabricate its result.
3. Liability metering courts: run inside `cargo test -p cgap-p28`
   (`asc450.rs`, `trap.rs` on disk this session).
4. Status-register reachability: `cargo test -p cgap-ffi` -> expect exit 0
   (`sovereign.rs` carries the CgapStatus register + reachability law with
   21 in-file `#[test]` fns; `tests/alloc_courts.rs`,
   `tests/zero_copy.rs` on disk). Not executed by C04 (heavy).

### Evidence
`cargo test` result lines (N passed / 0 failed) verbatim per crate; sweep
verdict `640/640 halt` once landed; for the spoliation discriminator, every
death carries the staged `0x503238FF` trap discriminator (A17).

### Vacuity conditions
The 640-sweep is its own mutation court: success is DEATH of every mutant,
so vacuity would look like a survivor — any mutated record that verifies
fails the court loudly. A sweep that cannot halt a known-bad mutation is
FAIL. GATE-4 has no synthetic non-vacuity shim because the court's pass
condition is itself a 640-way mutation corpus.

### Loud-[SKIP] rules
None chartered for GATE-4. A missing sweep test is PENDING-LAND (seal
BLOCKED on A17), never `[SKIP]`.

## GATE-5 Bare-Metal Silicon Fit

Law: on `riscv32imc-unknown-none-elf`, flash image (text+rodata) TARGET
< 64 KB; static SRAM (data+bss) TARGET < 32 KB; 0 `.rodata` domain strings.
Budgets are ESTIMATE-class in A19 §5 until measured on the real target —
measured numbers replace them at DoD; none are MEASURED today.

### Commands
1. Preflight: `rustup target list --installed | grep riscv32imc` -> exit 0
   (witnessed this session); `xcrun -f llvm-size` -> exit 0 (witnessed:
   Xcode toolchain path).
2. `cargo build -p cgap-ffi --target riscv32imc-unknown-none-elf --release`
   (A19 T1, staticlib) -> expect exit 0. Not executed by C04.
3. `xcrun llvm-size -A <elf>` -> mechanically compare: flash =
   text+rodata < 65536 (TARGET); SRAM = data+bss < 32768 (TARGET). Record
   the raw table in the receipt.
4. Zero-string leg on the 32-bit artifact: rerun the GATE-2 scan with
   `--target-dir` aimed at the cross build so the riscv artifacts are the
   scanned subject; expect 0 hits in fail-set sections (charter CTQ-05:
   llvm-size + zero_string_gate.py pair).
5. Xtensa T2 (A19): `rustup toolchain list | grep -i xtensa` -> exit 1
   this session (toolchain absent). Chartered protocol (A19 §6.3): the
   court prints `[SKIP] xtensa toolchain absent — T2 unexecuted`, exits 0
   as a LOUD skip, and the receipt records T2 UNVERIFIED — never a silent
   pass, never fabricated compliance. T1 (RISC-V) is the primary gate.
6. Compile court runnable today: `cargo test -p cgap-core --test
   rv32imc_court` -> expect exit 0 (A04 court compiles `frame.rs`/
   `dma_ring.rs`/`isomorphism.rs` for the real target, full rlib codegen).
7. XIP link leg: `crates/cgap-ffi/xip.ld` is an untracked PROPOSED sketch
   (A19 §5); the page-alignment court (misaligned `.oabi` -> linker ASSERT
   fires) is PENDING-LAND. Named vacuity mutation for the link leg: a
   deliberately misaligned `.oabi` input section must fire the linker
   ASSERT at link time; an integrated link that passes misaligned input is
   vacuous.

### Evidence
rustup/xcrun exits; llvm-size raw table + budget comparison line; `[SKIP]`
line verbatim when T2 is skipped; rv32imc_court pass line; xip.ld ASSERT
firing transcript when the alignment court lands.

### Vacuity conditions
Link-leg mutation: misaligned `.oabi` must fire the xip.ld ASSERT.
Size-leg degenerate-subject guard: a build that silently drops the
`.oabi`/`.bn3` sections must fail (xip.ld asserts `SIZEOF(.oabi) > 0` /
`SIZEOF(.bn3) > 0` at link time) — a size receipt over an empty-subject ELF
is vacuous.

### Loud-[SKIP] rules
Only T2 (Xtensa) carries a chartered skip: toolchain absent -> loud
`[SKIP] xtensa toolchain absent — T2 unexecuted`, exit 0, receipt row
UNVERIFIED. Every other GATE-5 leg is either run (exit recorded) or
PENDING-LAND (loudly BLOCKED at seal), never skipped.

## 6. Coordinator-only boundary table

Charter §5 "Subagent Publish Isolation": autonomous workers are strictly
barred from `git commit`, `git push`, or registry `publish` transitions; §4
Phase 5 reserves version locking (26.11.1) and sequential registry
publication to the coordinator.

| Transition | Executor | Agent stance |
|---|---|---|
| `git add/commit/push/stash/checkout` | coordinator only | NEVER (read-only status/diff OK) |
| Version lock `26.11.1` | coordinator only | NEVER |
| Registry publish (blake3 -> arrow -> git -> parquet -> cgap facade) | coordinator only | NEVER |
| Post-publish external consumer verification build | coordinator only | NEVER |
| Lane surface landing (root Cargo.toml, crate lib.rs) | coordinator only | NEVER (leases only) |
| Gate runs + evidence capture | gate runner | REQUIRED (read-only over the tree) |

## 7. Receipt template (per gate run)

    gate: GATE-N <name>
    date: <UTC>
    host: <rustc -vV summary>; <git rev-parse HEAD>  (read-only)
    commands: <command> -> exit <n>   (one line per leg)
    evidence: <SUMMARY lines verbatim>; <report sha256>
    vacuity witness: <named mutation> -> flip observed? y/n
    skips: <[SKIP] rows verbatim> | "none"
    standing: <ALIVE|PARTIAL|BLOCKED|REFUSED|UNSUPPORTED|UNVERIFIED> per leg

## 8. Session grounding (C04, 2026-11-01, /Users/sac/cgap)

- `python3 scripts/zero_string_gate.py --help` -> exit 0
- `python3 scripts/boundary_gate.py --help` -> exit 0
- `python3 scripts/zero_string_gate.py --register` -> exit 0 (register v2,
  a07-zero-string-dep-audit)
- `python3 scripts/zero_string_gate.py --self-check` -> exit 0
  (`SELF-CHECK RESULT PASS`, poison `hits=2 fired=True`, clean `hits=0`)
- `xcrun -f llvm-size` -> exit 0
- `rustup target list --installed | grep riscv32imc` -> exit 0
- `rustup toolchain list | grep -i xtensa` -> exit 1 (absent -> T2 loud-skip
  path is the chartered behavior in this environment)
- `ls crates/cgap-court/` -> Cargo.toml only (court_verdict PENDING-LAND)
- `ls crates/cgap-p28/tests/` -> absent (640-sweep PENDING-LAND)
- `ls crates/bench/benches/` -> v26_10_31.rs only (ctq-01..04 Divan benches
  PENDING-LAND, consistent with ctq-registry.yaml)
- NOT executed by C04 (runner executes at seal): `cargo build --workspace`,
  boundary_gate `--deps/--strings/--mutate/--shim-court-only/--shim-inert/
  --cross-gate-court`, `cargo test -p cgap-p28`, `cargo test -p cgap-ffi`,
  `cargo test -p cgap-core --test rv32imc_court`, riscv release build.

## 9. Divergences recorded

1. Exit-code reality: `boundary_gate.py` and `zero_string_gate.py`
   implement `{0,1}`; the campaign classes 2/3 (subject missing, loud skip)
   exist only in `scripts/dod.sh`. The runner records `[SKIP]` rows and
   exits as emitted, never reinterprets them.
2. `llvm-size` is not on PATH; the macOS invocation is `xcrun llvm-size`.
   (`xcrun -f llvm-size` exit 0 this session.)
3. A20 §9 names five DoD items that map onto but do not literally equal the
   charter's five Phase-5 gates (A20 §9.2 maps to GATE-3, §9.3 to GATE-1,
   §9.4 to GATE-2; A20 §9.5 corpus receipt is a GATE-3 leg). Charter gate
   names are primary here.
4. `ctq-registry.yaml` records a `--self-check` PASS from its own authoring
   session; C04 re-executed it first-hand (§8) and the playbook cites the
   C04 run.
5. A17's exhaustive 640-bit sweep and the A20 `court_verdict` harness are
   chartered but not landed (no test files on disk); both legs are carried
   UNVERIFIED/PENDING-LAND — the playbook forbids fabricating their
   verdicts at seal.

## See Also

- `00-dflss-charter-source.md` — source charter (§3 CTQ, §4 Phase 5)
- `ctq-registry.yaml` — CTQ rows + GATE-1..GATE-5 ids (sibling artifact)
- `risk-controls.md` — risk matrix conversion (sibling artifact)
- `../../lanes/A20-court-verdict.md` §9 — DoD command surface
- `../../lanes/A07-bosym-slab.md`, `../../lanes/A17-p28-derivative.md`,
  `../../lanes/A19-libcgap-ffi-xip.md` — gate-owning lane specs
