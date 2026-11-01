# risk-controls

Mechanical control procedures for the v26.11.1 DFLSS charter Section 6 risk
matrix. Source of truth:
`docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` §6 (four risk rows,
lines 99-106). Audience: lane-runner / gate-runner / court-auditor agents.
Rule for every control below: a runnable command + an expected output shape
+ an expected exit. A court that cannot flip is FAIL; a skip is loud
(`[SKIP] <reason>`), never a fake pass. Campaign exit-code contract:
0 pass / 1 gate fail / 2 subject missing / 3 loud skip. All witnesses are
from `/Users/sac/cgap` on 2026-11-01 unless marked otherwise.

## Standing legend

| Mark | Meaning |
|---|---|
| ALIVE | ran this session (2026-11-01) in /Users/sac/cgap; command + exit cited inline |
| TARGET | directional nominal from charter §3; never reported as MEASURED |
| PINNED | byte-frozen convention per CLAUDE.md (never sweep) |
| PENDING-LAND | instrument named by the charter but absent from disk at authoring time |
| UNVERIFIED | named here but not run this session; stays UNVERIFIED until its receipt lands |
| REFUSED | typed refusal: the artifact/gate-run is rejected, never silently skipped |

## Control index

| ID | Risk | Command surface | Flip (anti-vacuity mutation) |
|---|---|---|---|
| S1-S4 | Seed / constant drift | git diff + grep + baseline | S2 flips on final-nibble drift |
| R1-R2 | 429 attrition (TARGET ~25%) | journal.jsonl census | R2 flips on a dropped node |
| R3-R4 | 429 replay + no-silent-caps | resumeFromRunId (coordinator) | R4 flips on hidden caps |
| V1-V3 | Vacuous bench harness | black_box/checksum + flip court | V3: flip or REFUSED |
| A1-A3 | Premature target-lock abort | journal poll + pgrep census | A3: no stop while lanes run |

## Canonical pin (all sections)

Seed constant, byte-frozen: `0xBF58_476D_1CE4_E5B9` (canonical SplitMix64
multiplier; charter §2 item 2 + §6 row 1; "corruption-witness constant" per
`docs/jira/v26.11.1/bench-campaign-charter.md` Method discipline). Any other
final nibble (`...E5BA`, `...E5B8`) is the known write-corruption class.
A non-canonical seed in repo sources or outputs = artifact REFUSED.

---

## 1. Silent Seed / Constant Drift

Failure mode (charter §6 row 1): a subagent corrupts the PRNG seed
(`...E5B9` -> `...E5BA`), invalidating corpus determinism. Control:
working-tree diff check + presence sweeps + 12/12 court baseline before any
gate run.

### S1 Canonical-presence sweep — ALIVE

```bash
grep -rn "0xBF58_476D_1CE4_E5B9" --include="*.rs" crates tests
```

Expected: exit 0, exactly 13 lines (witnessed 2026-11-01). Sites: 2 in
crates/bench/src/bin/const_time_bench.rs, 1 each in bench/digest_bench.rs,
bench/neon_ab_bench.rs, cgap-arrow/fingerprint.rs, cgap-arrow/silicon.rs,
cgap-arrow/slab.rs, cgap-blake3/tests/thread_differential.rs,
cgap-gate/penstock.rs, cgap-parquet/ffor.rs,
cgap-parquet/tests/robustness.rs, integration/tests/membranes.rs,
tests/court_verdict.rs. A count drop = a seed site was swept (repair it);
a count rise = a new seed site (owning lane names it in its receipt).

### S2 Near-miss sweep — ALIVE — the flip

```bash
grep -rnE "0xBF58_476D_1CE4_E5B[0-9A-Fa-f]" --include="*.rs" crates tests \
  | grep -v "0xBF58_476D_1CE4_E5B9"
```

Expected: zero printed lines; pipeline exit 1 (the trailing `grep -v` finds
nothing to keep). Exit 1 = PASS. Any printed line = the seed-corruption
class; the gate run is REFUSED and the owning lane repairs its lease before
any gate run. This is the section's flip: S2 fires on a single last-nibble
drift (`...E5B9` -> `...E5BA`), which S1's raw count cannot distinguish.

### S3 Working-tree diff guard — ALIVE

```bash
git diff --unified=0 | grep "BF58_476D_1CE4_E5B"
```

Expected: empty output, grep exit 1 = PASS (witnessed 2026-11-01 with the
session's uncommitted lane edits present in the tree: no in-flight diff
line touches a seed site). Any printed line = an in-flight edit is mutating
a seed constant: stop that lane, restore via its lease, re-run S1 + S2.

### S4 cgap-court 12/12 verdict baseline — ALIVE

```bash
cargo test -p cgap-court --test court_verdict
```

Expected: exit 0 and a `test result: ok. N passed; 0 failed; ...` line with
N >= 12 — the invariant is `failed=0 AND passed>=12`. Witnessed 2026-11-01:
`test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered
out; finished in 13.42s` (divergence D1: the charter's `12/12` reads as a
floor; the corpus now carries 13 courts). Required prior to every gate run
(charter §6 row 1). Fail shape: exit nonzero + `test result: FAILED.`.

### S5 Zero-string court suite — UNVERIFIED (supplementary)

```bash
python3 scripts/zero_string_gate.py --courts
```

Expected on pass: exit 0, final line `COURTS-SUMMARY total=N failed=0`,
N >= 12. Not re-run this session (superseded as primary baseline by S4 per
dispatch); prior a07-wave5/6 audit receipts carry `total=13 failed=0`, so
this stays UNVERIFIED here until a fresh run receipts it. Gate-fail shape:
exit 1, one `COURT <name> FAIL` row per breach. Subject-missing: exit 2.
Loud skip: exit 3 (`[SKIP] <reason>`).

---

## 2. API Rate-Limit Attrition (429)

Failure mode (charter §6 row 2): ~25% (TARGET) of subagent calls drop
during high-concurrency waves. Control: journal event census + loud-gap
counter, then ONE same-session `resumeFromRunId` cache-replay pass at
completion (never mid-wave; CLAUDE.md "Agent fan-outs").

Journal record shape (witnessed 2026-11-01 against the ClaudeWorkflowReactor
fixture `/Users/sac/gym-ecosystem/vendor/beam4pm/qualification/fixtures/
claude-workflows/journal.jsonl` — the reactor that writes cgap wave
journals): one JSON object per line,
`{"type":"started"|"result","key":...,"agentId":...}`. The live journal for
`wf_f175cd34-71f` had not landed on disk at authoring time (handoff
`~/.claude/workflow-handoff/wf_f175cd34-71f.json`, status `launched`) — the
live path is PENDING-LAND; the commands below take any journal path as
`$J`.

### R1 Journal event census — shape ALIVE (fixture), live path PENDING-LAND

```bash
python3 - <<'PY' "$J"
import json, collections, sys
c = collections.Counter()
with open(sys.argv[1]) as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        try:
            c[json.loads(line).get("type", "<malformed>")] += 1
        except Exception:
            c["<malformed>"] += 1
print(dict(c))
sys.exit(1 if c["<malformed>"] else 0)
PY
```

Expected pass: stdout `{'started': Ns, 'result': Nr}`, exit 0. Fixture
witness: `{'started': 19, 'result': 19}`, exit 0. Exit 1 = at least one
undecodable line (typed refusal; never silently skipped — the count names
it).

### R2 Loud-gap counter — shape ALIVE (fixture + /tmp flip), live PENDING-LAND

```bash
python3 - <<'PY' "$J"
import json, sys
started, result = set(), set()
with open(sys.argv[1]) as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        rec = json.loads(line)
        if rec.get("type") == "started":
            started.add(rec.get("key"))
        elif rec.get("type") == "result":
            result.add(rec.get("key"))
gaps = sorted(started - result)
print("LOUD-GAPS", len(gaps), gaps)
sys.exit(1 if gaps else 0)
PY
```

Expected pass: `LOUD-GAPS 0 []`, exit 0 (fixture witness). Flip witness:
on a /tmp copy of the fixture with one synthetic extra `started` line,
R2 prints `LOUD-GAPS 1 [<key>]` and exits 1. A `started` with no `result`
is a dropped (429/null) node; R2 is the control that makes attrition
non-hideable.

### R3 Same-session resumeFromRunId replay — coordinator-owned

ONE pass, at completion only. Never stop/resume mid-wave (kills in-flight
work; CLAUDE.md). Cached calls replay from cache; only dropped nodes
(R2's gap list) re-execute, re-dispatched at low concurrency. The
lane-runner never issues `resumeFromRunId`; the coordinator does. Post-replay
verification = re-run R1 + R2 on the full journal: the run is drained only
when R2 prints `LOUD-GAPS 0 []` (exit 0). Any node still gapped after the
replay pass is loud, not capped: proceed to R4.

### R4 No-silent-caps rule — the receipt flip

A wave receipt that closes with R2 gaps > 0 must carry one loud row per
dropped node: `[SKIP] <reason> journal-line key=<key> agentId=<id>`. Check:

```bash
grep -c "^\[SKIP\]" <wave-receipt>.md
```

Expected relationship: `grep -c` output == R2's LOUD-GAPS count. Equal = the
attrition is fully surfaced; fewer `[SKIP]` rows than gaps = a hidden cap =
the receipt is REFUSED. (Shape check only this session — no wave closed
during authoring; the relationship itself is the flip.)

---

## 3. Vacuous Benchmark Harness

Failure mode (charter §6 row 3): dead-code elimination optimizes away the
kernel under test, reporting fake ~0 ns metrics. Control: black_box
wrapping + state checksum assertions + a witnessed mutation column-shift
court (bench-campaign-charter.md "Anti-vacuity": a bench whose number is
mutation-invariant is VACUOUS and the lane's perf-claim conversion is
REFUSED).

### V1 black_box presence court — ALIVE

```bash
grep -rn "black_box" crates/bench/src/bin/ crates/bench/benches/ \
  | grep -v "//" | wc -l
```

Expected: exit 0, count >= 100 (witnessed 2026-11-01: 162 black_box sites
across the bench bins and benches/v26_10_31.rs; `divan::black_box` in
benches/v26_10_31.rs at lines 24, 62, 92-93, 104). Count 0 = every bench
lost its black_box = the whole membrane is vacuous = REFUSED. Per-lane
rule: a NEW bench file whose own grep count is 0 fails this control.

### V2 State checksum assertions — ALIVE

```bash
grep -rn "checksum\|digest vacuous\|PAYLOAD-SUM" crates/bench/src/bin/*.rs
```

Expected: exit 0, hits in at least three bins. Witnessed 2026-11-01:
digest_bench.rs:93 `assert_ne!(digest, 0, "digest vacuous: mixed patterns
folded to zero")`; arrow_accessors_bench.rs:298 window-vs-direct checksum
assert + :390 get-checksum-drift assert; parquet_throughput.rs:122-134
`PAYLOAD-SUM ... (checksum {:016x})` printed per receipt line. A bench
asserting no final state (no checksum/digest line) is London-shaped and
fails the membrane checklist item M-8.

### V3 Mutation column-shift court — mechanism NAMED, flip UNVERIFIED-here

Procedure (owned by court-auditor, per bench-campaign-charter dispatch
shape): copy the bench's subject crate to a /tmp sandbox; apply the bench's
named structural mutation (e.g. inject a heap `Vec` intermediate into the
decode path, or add a branch to the hot loop); re-run the bench; require
the watched Divan column to move; restore; re-run; require the column to
return.

```bash
cargo bench -p bench --bench v26_10_31 -- --color never
```

Flip law: mutated-column == baseline-column => bench VACUOUS =>
`TARGET -> MEASURED` graduation REFUSED for that lane. Not executed this
session (release bench run + sandbox owned by the court-auditor dispatch);
the parser-side turnstile `python3 scripts/divan2receipt.py <stdout.txt>`
(exit 0 + receipt only if the gated CTQ passes; exit 1 + NO receipt on
breach or unparseable/missing bench) is on disk and its fail-closed
contract is read from its header this session — the flip itself stays
UNVERIFIED until a receipt lands.

---

## 4. Premature Target-Lock Abort

Failure mode (charter §6 row 4): an operator mistakes serialized `cargo`
compilation lock waits for a deadlocked workflow and issues TaskStop,
destroying in-flight lane work. Control: check journal progression + child
compiler processes BEFORE any TaskStop; never kill advancing lanes.

### A1 Journal progression poll — bounded, no fixed-wait

```bash
A=$(wc -l < "$J"); B="$A"
for i in 1 2 3 4 5 6; do
  B=$(wc -l < "$J")
  [ "$B" -gt "$A" ] && break
  sleep 10  # SLEEP-CLASS: POLL-STEP (the audited poll's only sleep)
done
if [ "$B" -gt "$A" ]; then
  echo "ADVANCING: $A -> $B lines"
else
  echo "STALLED: no growth in 60s window (last=$B)"
fi
```

Bounded deadline: 6 x 10 s = 60 s maximum, exits early on the first
observable advance. Output shape: `ADVANCING: N -> M lines` (journal
growing = the workflow is alive; TaskStop is REFUSED) or
`STALLED: no growth in 60s window (last=N)`. Live-journal path PENDING-LAND
as in §2; the poll itself is the audited implementation and holds the only
sleep.

### A2 Child compiler census — ALIVE (both arms)

```bash
pgrep -fl "cargo|rustc"
```

Expected: exit 0 + PID/command rows while a build is in flight; exit 1
(no match) when idle. Witnessed 2026-11-01, arm 1: during the S4 baseline
compile, `pgrep -fl cargo` returned the live `cargo test -p cgap-court
--test court_verdict` process (PID 91412). Witnessed, arm 2: after the
baseline finished, the same pgrep exits 1 (no compiler alive). A `cargo`
process with no listener output for one A1 window is a serialized
target-lock wait (the charter's named misread), not a deadlock signal.

### A3 TaskStop decision tree — the control

Issue TaskStop ONLY after all three checks refuse to save the run:

1. Run A1. `ADVANCING` => NO TaskStop (never kill advancing lanes; this
   is terminal for the stop request). `STALLED` => continue.
2. Run A2. Any `cargo`/`rustc` row => NO TaskStop (compile lock wait;
   re-run A1 after the compiler exits, bounded). Empty => continue.
3. Re-run A1 once. Still `STALLED` AND A2 still empty => TaskStop is
   lawful; the stop receipt must quote both observables' outputs (A1's
   STALLED line, A2's empty-exit-1) or the stop is an undocumented abort
   and is REFUSED as such.

Flip: A3's outcome changes on A2's arm (exit 0 vs exit 1) — the census, not
the clock, decides. Skipping checks 1-2 and stopping on elapsed-time
suspicion alone is the charter's named failure mode and is REFUSED.

---

## Divergences

- D1: charter §6 row 1 says `12/12` court baseline; the on-disk corpus is
  13 courts (S4 witness). Control adopts the invariant `failed=0 AND
  passed>=12`.
- D2: the live journal for `wf_f175cd34-71f` was not on disk at authoring
  time; R1/R2 shapes are witnessed against the beam4pm reactor fixture, and
  the live path is carried as PENDING-LAND.
- D3: the superseded risk-controls.md named `zero_string_gate.py --courts`
  as the §1 baseline; the dispatch names the court-verdict cargo test.
  Both are kept (S4 primary, S5 supplementary, S5 UNVERIFIED this session).
- D4: charter §6's "black_box" safeguard names Divan; existing `src/bin`
  benches use `std::hint::black_box` (equivalent DCE shield). V1 greps
  both surfaces.
