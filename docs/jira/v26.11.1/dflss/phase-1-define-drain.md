# phase-1-define-drain

Dispatch-ready phase prompt for DFLSS Phase 1 (Define & Drain — Active Fleet
Completion) of the v26.11.1 campaign. Audience: lane-runner and gate-runner
subagents. Repo: /Users/sac/cgap — ONE canonical checkout, no worktrees, no
clones. Source of truth:
docs/jira/v26.11.1/dflss/00-dflss-charter-source.md section 4, "Phase 1:
Define & Drain (Active Fleet Completion)" (safeguard rows 429-attrition and
premature-target-lock-abort from section 6).

## Contract (breach = artifact REFUSED)

- ONE canonical checkout; lanes hold disjoint file-ownership leases, never
  copies. Lane build roots (`target-*/`) are leases, not assets.
- NEVER run git state-mutating commands (add/commit/push/checkout/stash) —
  the coordinator owns every git transition. Read-only git (status/diff)
  is allowed.
- Chicago discipline: real collaborators, assert final state, no mocks.
  Every court names its anti-vacuity mutation; a court that cannot flip is
  FAIL; skips are loud (`[SKIP] <reason>`), never silent.
- No Process.sleep — poll observables with bounded, deadline-checked polls.
- Standing vocabulary only: ALIVE / PARTIAL / BLOCKED / REFUSED /
  UNSUPPORTED / UNVERIFIED. UNVERIFIED stays UNVERIFIED; never round up.
- Exit-code contract for gates: 0 pass / 1 gate fail / 2 subject missing /
  3 loud skip.
- Write ONLY your leased files. Report status; never narrate intent.

## Objective

Drain every unverified node of the twenty-lane fleet to witnessed standing
(command+exit run this session), then hand clean state to the coordinator
for integration — without polluting the `no_std` dependency graph and
without destroying in-flight work.

## 1. Fleet completion rule — no mid-wave preemption

- The active multi-wave workflow completes before any transition. Do NOT
  stop/resume it mid-wave: a stop kills in-flight work (half-applied
  leases, court runs lost mid-execution). The charter names run
  `wf_f175cd34-71f`; the authoritative run id is whatever the current
  session's journal carries — read it, never assume it.
- Run ONE `resumeFromRunId` pass at completion. Cached calls replay;
  only failed nodes re-execute. Never loop backfill until green.
- Expected attrition: ~25% of subagent calls drop (API 429) at high
  concurrency. Attrition is a backfill input, not an intervention trigger.
- A node that already returned a receipt is never re-run by backfill.
- Lanes report standing; they never mutate the tree outside their lease
  and never issue wave transitions.

## 2. Journal inspection — cargo lock serialization vs genuine stall

The operational journal (`journal.jsonl`, one JSON line per agent event)
is session-scoped: its path lives with the workflow run, not in the repo.
Inspect the run's own journal; do not fabricate a canonical path. Line
shape: agent/node id, event type (dispatch / result / error), timestamp,
status.

Before ANY stop request, establish genuine stall — not target-lock
serialization — using both observables:

1. Event-type progression over time: bucket journal lines by timestamp;
   confirm the event mix still advances (new dispatch and result lines
   after observation start). Zero result lines across the whole window is
   stall evidence; continued interleaving is advancement, not a stall.
2. `pgrep -fl cargo` (and `pgrep -fl rustc`): a live compiler child means
   lanes are serialized behind the workspace target lock — latency, not
   deadlock. The lock clears when the holder finishes.
3. Genuine stall = no journal progression AND no advancing compiler
   process tree across a bounded poll window. Only then escalate to the
   coordinator with both observables attached. Never kill an advancing
   lane; "check journal progression and child compiler processes before
   TaskStop" is the charter's own control (section 6, premature abort).

## 3. Single same-session resumeFromRunId backfill

Execute exactly ONE backfill pass, in the same session that observed the
attrition:

- Cached replay: `resumeFromRunId` replays completed calls from the run
  cache; only nodes that returned an error (429) or a null result
  re-execute.
- Loud-gap counting: after the pass, every expected node must carry a
  terminal journal line — pass, fail, or typed `[SKIP]`. Nodes with no
  terminal line are LOUD GAPS: count them and print the count in the
  phase receipt. No silent caps, no dropped-item hiding.
- Gaps > 0 are reported, not looped away. A zero-gap completion claim is
  lawful only when the journal census is re-read at use time and shows
  zero.
- Re-run receipts obey standing law: command+exit run this session, else
  the node stays UNVERIFIED.

## 4. Bare-metal 32-bit blockers (resolve, then witness)

Both blocker files verified on disk this session (`ls -la` receipts):

- crates/cgap-arrow/src/chase_lev.rs — exists, 10794 bytes, 2026-11-01
  11:50. Charter blocker: 64-bit `AtomicI64` index cells in the
  work-stealing deque. On-disk state: `mod index_width` at lines 40-52
  behind `#[cfg(target_has_atomic = "64")]` (AtomicI64 tier), with a
  32-bit `AtomicIsize` tier and a load/store-only tier
  (`SUPPORTS_STEAL = false`, typed) below it. Fix scaffolding present;
  32-bit compile = UNVERIFIED until the cross-compile court runs.
- crates/cgap-arrow/src/c_abi_nested.rs — exists, 22551 bytes,
  2026-11-01 11:43. Charter blocker: `AtomicPtr::swap` call sites in the
  nested C-ABI release path. On-disk state: target-class split in
  `release_swap` (lines 62-73): real `swap` (AcqRel) where
  `target_has_atomic = "ptr"`, else `load`/`store` with the single-hart
  serialization argument. Fix scaffolding present; 32-bit compile =
  UNVERIFIED until the cross-compile court runs.

Resolve means the court witnesses one of, never a claim:

- `rustup target list --installed` — record whether
  `riscv32imc-unknown-none-elf` is present.
- Present: `cargo check -p cgap-arrow
  --target riscv32imc-unknown-none-elf` exit 0 is the blocker-cleared
  witness. Any error = BLOCKED, cite file:line.
- Absent: `[SKIP] toolchain riscv32imc-unknown-none-elf not installed`,
  gate exit 3, standing UNSUPPORTED(toolchain) — never a silent pass,
  never fabricated compliance (charter section 5 boundary).

## 5. Coordinator commit integration

- Lanes never stage, commit, push, or stash. At wave completion the
  coordinator integrates; all transitions are coordinator-owned.
- Commit messages: `git commit -F <file>` only — never inline `-m`
  (shell tokenization corrupts multi-line prose).
- Stage explicit paths per lane; `git add -A` is banned. One atomic
  commit per lane (or coherent lane group) so receipts attribute diffs.
- Integration sweep: delete lane build-root leases (`target-*/`) before
  the final commit; `.gitignore` is the backstop, not the mechanism.
- Unattributed diffs block integration: every working-tree diff must map
  to a named lane lease before any commit runs.

## 6. Completion definition (Define gate)

Phase 1 exits when ALL hold, each cited command+exit in the receipt:

- Journal census re-read at use time: 0 loud gaps across the fleet.
- Both 32-bit blocker courts witness pass or loud skip (gate exit 0 or
  3; never a silent 1, never a fabricated pass).
- Working tree has no unattributed diffs; coordinator staged only
  explicit leased paths.
- Receipt fields present: identity (run id, node ids), authority
  (coordinator), consequence (leased files), replay (journal + command
  list), standing per lane.

## 7. Anti-vacuity mutation (named)

A Define-phase court that cannot flip is FAIL. Mutation: mark one node
drained with no terminal journal line (or claim blocker-cleared without
the cross-compile exit). The census (section 3) must report the loud gap
and the court (section 4) must refuse the claim — nonzero exit. A green
run with no mutation attempted is VACUOUS (exit nonzero).
