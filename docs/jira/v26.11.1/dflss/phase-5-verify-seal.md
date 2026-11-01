# phase-5-verify-seal

- Charter source: `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md`
  (section 4, Phase 5)
- Conversion lane: `C10-phase-5-verify-seal`
- Artifact class: dispatch-ready phase prompt (hand verbatim to each Verify lane)
- Standing law: nothing in this file has been executed yet. Every gate verdict,
  publish receipt, and consumer-build claim starts `UNVERIFIED` until a lane runs
  it this session and cites command + exit. `UNVERIFIED` never rounds up to
  `ALIVE`, and a seal claimed without a full five-gate receipt is `REFUSED`.

## 1. The Re-Seal Law (Phase 5 in one paragraph)

A seal is a statement about the tree, and the tree keeps moving: Divan bench
membranes (Phase 2), DOE verdicts (Phase 3), and cognitive retirements (Phase 4)
all land as new capital between seals. The re-seal is the proof they did not
break anything: whenever new capital lands after a seal, the five terminal gates
are re-run in full, sequentially, on the new head — not diffed, not sampled,
not assumed — and only a full re-run with all five exits 0 re-admits the seal.
Capital that cannot survive a re-seal is not capital; it is regression with a
receipt. The re-run is mandatory even when the landing looks semantically
isolated, because the gates exist precisely to catch what review cannot.

## 2. Dispatch Contract Block (paste into every Verify lane prompt)

- Repo: `/Users/sac/cgap` — ONE canonical checkout; no worktrees, clones, copies.
- Never run git state-mutating commands; the coordinator owns all git
  transitions and all registry publish transitions.
- Write lease: EXACTLY the output files your dispatch names; all else read-only.
- Chicago discipline: real collaborators, assert final state, no mocks.
- Courts mutate `/tmp` sandbox copies with sha256 fail-closed restore, never
  the shared tree.
- No `Process.sleep`; poll observables with bounded deadlines.
- Exit codes: 0 pass / 1 gate fail / 2 subject missing / 3 loud skip.
- A court that cannot flip is FAIL; a skip is loud (`[SKIP] <reason>`), never
  silent. A green gate run with no mutation witness is `VACUOUS` (nonzero exit).
- Status claims carry command + exit from this session; `UNVERIFIED` stays
  `UNVERIFIED`; perf numbers stay `TARGET` until measured with stated method.

## 3. The Five Terminal Gates (sequential; order is load-bearing)

Execute in this order, stopping on first nonzero: a GATE-1 fail means the rest
do not run, and a later gate's pass never retroactively admits an earlier fail.
The executor-facing procedure for each gate lives in
`docs/jira/v26.11.1/dflss/gate-runner-playbook.md` (canonical path; being
authored concurrently — reference it, do not inline or depend on its content).

| Gate | Name | Subject (charter wording) |
| --- | --- | --- |
| GATE-1 | Workspace Purity | workspace compiles clean, no stray patches or debris |
| GATE-2 | Binary/Symbol Purity | symbol purity + zero `.rodata` domain strings |
| GATE-3 | 1M-Row Sovereign Ledger Proof | ledger proof at 1M-row scale |
| GATE-4 | 640-Bit Spoliation & Liability Metering | P28 attestation verify + metering |
| GATE-5 | Bare-Metal Silicon Fit | `riscv32imc` footprint via `llvm-size` |

Gate rules:

- GATE-1 through GATE-4 run on the host; GATE-5 runs the cross-compiled
  `riscv32imc-unknown-none-elf` ELF. Where a secondary embedded toolchain (e.g.
  Xtensa) is absent locally, GATE-5 executes the loud-`[SKIP]` protocol (exit 3,
  `[SKIP] toolchain absent`) — never a fabricated pass, never a release block.
- Each gate names its anti-vacuity mutation up front (playbook Section: court
  mutations). A gate that cannot flip on its mutation is FAIL by definition.
- The seed guard (charter section 6) precedes GATE-1: canonical SplitMix64
  multiplier `0xBF58_476D_1CE4_E5B9` verified byte-exact in the working tree,
  plus the `12/12` court-verdict baseline, before any gate runs. Seed drift
  (`...E5B9` -> `...E5BA` class) is `REFUSED` before gates, not after.
- Gate verdicts are recorded per gate with command + exit in the wave receipt;
  a partial sequence is `PARTIAL`, never a seal.

## 4. Coordinator-Only Version Lock to 26.11.1

- The workspace currently reads `26.10.29` (root `Cargo.toml` line 39, read this
  session). Moving every published crate to `26.11.1` is a coordinator-only
  transition: agents never edit versions.
- Frozen byte-pins are untouched by the lock — a diff that moves one byte of
  either pin is `REFUSED` material:
  - License URN `urn:avec:license:p28-sil:v2.1.0-sovereign`
    (on disk today in `crates/cgap-gate/src/penstock.rs`, `crates/cgap-crypto/Cargo.toml`,
    `crates/cgap-core/Cargo.toml`, and `crates/cgap-core` tests/README — grep
    receipt this session).
  - v26.10.31 SHACL corpus IRIs (`avec:` prefix ->
    `https://avec.chatman.dev/`).
- Post-lock, the pin greps must return the identical byte strings; any pin
  diff hunk in the version-lock commit is a coordinator-side gate failure.

## 5. CLI Regen Replay (byte-stable apart from version)

- Command (coordinator-owned transition, replayed for proof, not for edit):
  `cd crates/cli && rm Cargo.toml && ggen sync run && python3 manifest_patch.py`
- Court: `sha256` the generated set before and after
  (`crates/cli/src/{main,lib,generated_cli,generated_behavior,generated_boundaries}.rs`,
  `tests/zero_code_cli.rs`, `docs/command-reference.md`, `Cargo.toml`,
  `ggen.lock`). Replay must be byte-identical except version-bearing lines.
- Any other byte drift is `REFUSED`: it means a hand edit bypassed the graph,
  and the graph is the source, not the bytes. This is the AC -> DC projection
  discontinuity exercised at the terminal gate.
- `ggen.lock` pin drift is a pin violation (same class as section 4 pins).

## 6. Sequential Registry Publication (coordinator-only, strict order)

Publish order is a dependency order, never a convenience: `cgap-blake3` ->
`cgap-arrow` -> `cgap-git` -> `cgap-parquet` -> `cgap` (facade). Each crate
publishes only after the predecessor is verified visible on crates.io (real
registry query, not local cache), because a facade published ahead of its
kernels is a broken HEAD for every external consumer.

429 protocol (charter section 6, ~25% attrition class):

- On HTTP 429: read `Retry-After`, report `BLOCKED` with the absolute timestamp
  and the retry deadline in the receipt, and stop. Never blind-retry.
- One `BLOCKED` receipt per rate-limit event; the coordinator resumes at the
  deadline or via same-session cache replay of only the dropped node.
- No silent caps, no dropped-item hiding: the publish receipt names every
  attempt, not only the successful ones.

## 7. External Consumer Verification Build (`/tmp`, registry source only)

- After the `cgap` facade is visible on crates.io, build a fresh consumer in
  `/tmp` from registry source only: `cargo new`, add the five published crates
  by version `26.11.1` (path/patch sections forbidden in this consumer),
  `cargo build`, plus a minimal facade smoke use.
- Chicago discipline: this is a real build with real artifacts — assert the
  built binary runs and links the facade, not that files exist.
- A consumer that resolves any crate from a local path silently fails the
  gate: verify the resolved `Cargo.lock` sources are `crates.io` before
  asserting `ALIVE`.

## 8. Agent-Prohibition Boundary Table (charter section 5, Subagent Publish Isolation)

| Transition | Agents / subagents | Coordinator only |
| --- | --- | --- |
| `git commit` / `git push` | BARRED | yes |
| `cargo publish` (any of the five) | BARRED | yes |
| Version edits (any manifest) | BARRED | yes |
| Credential / token access | BARRED | yes (never in agent context) |
| Publish command construction | allowed (see below) | executes |
| Gate runs, courts, `/tmp` sandboxes | allowed | audits receipts |

- "Publish command construction allowed" means: an agent may return a
  `PUBLISH-HANDOFF` block — the exact commands it would run, with the exact
  versions, in the exact order — for the coordinator to execute. Agents return
  commands, never credentials; the handoff block contains no token material by
  construction, and a handoff block that embeds or references a secret is
  `REFUSED` on sight.
- Any agent running a barred transition is an authority violation, not a
  shortcut: the artifact is refused regardless of outcome correctness.

## 9. Kill Criteria Summary (courts, not coordinators)

| Check | Court refuses when |
| --- | --- |
| Re-seal | any gate re-run skipped, sampled, or assumed after new capital lands |
| Seed guard | canonical seed `0xBF58_476D_1CE4_E5B9` not byte-exact pre-GATE-1 |
| Mutation | a gate has no named anti-vacuity mutation, or cannot flip |
| Version lock | any frozen byte-pin moved by one byte, or agents edited versions |
| CLI replay | regenerated bytes drift apart from version-bearing lines |
| Publish | order violated, a 429 blind-retried, or a crate published before its dep is visible |
| Consumer | any crate resolved from a local path instead of `crates.io` |

## 10. Status Registry (this artifact, at write time)

- `docs/jira/v26.11.1/dflss/gate-runner-playbook.md` — absent on disk at write
  time (`PENDING-LAND`, concurrent lane); section 3 references its canonical
  path only. If absent at dispatch time, gate procedure detail is `BLOCKED` on
  its landing, not improvised by lanes.
- Workspace version on disk: `26.10.29` (root `Cargo.toml`, read this session).
- Five gates, version lock, CLI replay, publishes, and consumer build: not
  executed this session — `UNVERIFIED`.
- Pin locations witnessed this session (grep exit 0): license URN in
  `crates/cgap-gate/src/penstock.rs`, `crates/cgap-crypto/Cargo.toml`,
  `crates/cgap-core/Cargo.toml` (+ core tests/README); `just doctor` and
  `python3 scripts/zero_string_gate.py --self-check` remain the standing
  enforcement commands for the pins' gates.
