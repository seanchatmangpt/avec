#!/usr/bin/env python3
"""A20 boundary gate — Wave 4: harden + close DoD items.

Spec: docs/jira/v26.11.1/lanes/A20-court-verdict.md §6 (mutation-gate
protocol), §6.2 (kill-rate definition), §6.3 (shim-court non-vacuity),
§6.4 (dependency/string gates — seal-time enforcement), §9 (campaign DoD
exits), §10 (loud [SKIP] class).

Wave-1 slice froze the gate ABI (exit classes, kill-rate threshold, ledger
schema keys, CLI mode names) and refused loudly. Wave 2 landed the core
mutation-loop engine (§6.1 loop, §6.3 shim-court, stride sampling, §6.2
triage/kill-rate). Wave 3 landed the FULL lane scope (§6.4 sealers). Wave 4
HARDENS the engine and closes the wave-3 audit divergences; Wave 5 (this
file now) closes DoD §9.2 with the committed §6.2 triage register (items
9-10 below):

1. §6.1 loop — baseline manifest -> red-baseline abort -> one mutation per
   candidate -> court run -> kill recording -> sha256 fail-closed restore
   -> JSON ledger. (Wave 2, unchanged.)
2. §6.3 shim-court non-vacuity — runs FIRST in every campaign; a committed
   known-killable mutant (SplitMix64 mix-constant bump) must be killed or
   the gate refuses `VACUOUS GATE` with exit 1. (Wave 2, unchanged.)
3. --shim-inert negative control — plants an inert shim; the gate MUST
   refuse VACUOUS GATE. A gate that passes its new negative control is
   itself broken. (Wave 2, unchanged.)
4. §6.4 sealers — --deps (E=0 runtime externals per covered crate) and
   --strings (cloud-string scan, no allowlist). (Wave 3, unchanged.)
5. §6.2 kill rate exactly: |KILLED| / |NON_EQUIVALENT|; survivors are
   SurvivorDefect rows until hand-triaged to Equivalent via --equivalent
   (one-line justification required, recorded in the ledger).
6. WAVE 4 — §6.1 kernel targets: --targets <csv> extends the campaign
   beyond the A20 court kernel to the covered crates' per-kernel sources
   (closes the wave-3 deferral, divergence #5). OPT-IN by design: a kernel
   mutant transiently writes another lane's source file (sha256 fail-closed
   restore per mutant), so the default campaign stays on A20's own file;
   kernel campaigns run in a sandbox copy or a quiet seal window. The
   committed §6.3 shim anchors in the court kernel, so a --targets set
   without it refuses fail-closed (SHIM HOST ABSENT / SHIM SITE NOT FOUND).
7. WAVE 4 — §6.1 operator set completed: `ret-remove` (early-return
   removal) and `mask-corrupt` (mask/shift constant corruption in the
   branchless kernels) land here; OPERATORS_DEFERRED empties.
8. WAVE 4 — --only-op <csv>: restrict the sampled corpus to named
   operators (recorded in the ledger; never a silent filter). Operator-
   focused campaigns for triage and operator kill-witnessing.
9. WAVE 5 (defect repair, per the wave-3 audit receipt) — §6.2 closure:
   the hand triage of sampled survivors is a COMMITTED register
   (EQUIVALENT_TRIAGE below: repo-relative file:line:op:index -> one-line
   property justification). Plain `--mutate` (the DoD §9.2 command)
   applies the register automatically; `--equivalent` CLI specs merge
   over it (CLI wins for one run). Fail-closed on drift: a register key
   matching no survivor row this run (line drift from any court-file
   edit, sample shift) refuses the gate — the register cannot silently
   pass stale. The ledger records each triaged row's verdict
   ("Equivalent") plus its justification, and config carries the
   register applied.
10. WAVE 5 — deterministic kill attribution: `killed_by` is the
    alphabetically-first failing court (replay-stable), closing wave-3
    audit divergence #4 (output-order nondeterminism under the shim).
11. WAVE 6 (defect repair, per the a07-wave5 audit receipt open item 4
    and the A07 register's own `divergences` row) — §6.4 single-list law
    CLOSED: `--strings` now CONSUMES the A07 register at runtime
    (`python3 scripts/zero_string_gate.py --register`, v1, courts
    T14a/T14b) as the canonical cloud-string pattern + allowlist source.
    The former inline CLOUD_STRING_PATTERNS copy and the
    `a07_register_deferral` ledger row are DELETED — the register exists
    since the A07 wave-5 repair, so the "stub" deferral text was false
    and the inline copy was the O(N^2) drift channel. Fail-closed
    consumption: register unavailable / refused / unparsable / wrong
    version / carrying zero patterns / carrying an unjustified allowlist
    entry — each refuses with a typed reason (a strings gate over an
    empty canonical list enforces nothing and must not pass).
12. WAVE 7 (defect repair, per the a07-wave6 audit receipt open item 4:
    "A20: reconcile the two gates' coverage sets") — CROSS-GATE SEAM:
    `--strings` now also CONSUMES the canonical A07 artifact-scan verdict
    (`scripts/zero_string_gate.py` default scan mode, gate v1.3.0+) at
    runtime. The a07-wave6 §5 finding — A20 `--strings` green while the
    canonical quarantine scan reported hits=2 on the SAME tree — was a
    two-verdicts-one-tree channel: two gates could disagree about one
    artifact. Closed structurally: the seal-time strings verdict now
    INHERITS the canonical verdict (GREEN requires the canonical scan
    green; canonical red or typed refusal -> A20 refuses with a typed
    CROSS-GATE reason). The emitter-exemption classification
    (`EMITTER_BINARIES`, A07 gate v1.3.0 wave-7 class) is consumed, never
    re-adjudicated: a summary with hits=0 emitter_hits=N is GREEN here.
    Consumption contract (fail-closed on every failure mode): spawn
    error, timeout, missing/unparsable SUMMARY line (e.g. GateRefused),
    exit-vs-summary mismatch, scanned=0 under GREEN (vacuous scan) —
    each refuses with a typed reason. The seam's own court:
    `--cross-gate-court` (three real arms: green end-to-end at the repo
    root; a REAL cargo-built fixture probe whose binary embeds a real
    ontology token -> canonical RED; and the named anti-vacuity mutation
    `seam-neuter` in a sandbox clone — un-neutered refuses on the
    canonical red, neutered returns to exit 0; a --strings that stays
    green under seam-neuter is a vacuous seam and the court exits 1).
13. WAVE 9 (this session's failed-lane restart): two repairs, both against
    the a20-wave9 audit receipt's recorded defects, plus the suite's new
    A04 identity court —
    a. D3 CLOSED — `write_report` creates the --report parent dir (the
       witnessed loss: a full ~15-min campaign completed all mutants, then
       died on FileNotFoundError and the ledger was lost). An unwritable
       path still fails loudly; only the missing-parent case is repaired.
    b. D2 PREVENTION — `acquire_campaign_lock()`: --mutate /
       --shim-court-only hold an exclusive non-blocking flock
       (target/.a20-campaign.lock) for the process lifetime; a second
       concurrent campaign on this checkout refuses typed
       (`CAMPAIGN LOCK HELD`, exit 1) instead of interleaving mutants.
       Detection (Wave-8 verify_unmutated) + prevention (this lock) close
       the witnessed gate-vs-gate interference class. Kernel-released on
       process death; no stale wedge. The lock file lives under target/
       (build state, not source).

Fail-closed law: restores are sha256-verified byte-exact; a mismatch aborts
the campaign with a non-zero exit before any further mutant runs.

Single-list law (§6.4, O(N^2) drift): A20 maintains NO pattern list and NO
allowlist. The A07 register (scripts/zero_string_gate.py, owned by lane
A07) is the canonical home of the cloud-string list + allowlist; this gate
consumes it at --strings runtime and re-derives nothing. (Wave-1..5
history: the register did not exist before the A07 wave-5 repair, so the
gate enforced the §6.4 doc-list directly with an inline copy and a loud
`a07_register_deferral` ledger row — that deferral is retired by item 11;
no second copy remains.)
"""

import argparse
import fcntl
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# --- Frozen gate ABI (A20 §6.2, §6.3, §9) — unchanged from Wave 1 -----------

EXIT_PASS = 0          # §9: gate passes
EXIT_REFUSE = 1        # §6.3: typed refusal — VACUOUS GATE, red baseline,
                       # corrupted restore, untriaged survivor, Wave-4 mode.

KILL_RATE_MIN = 0.98                       # §6.2: gate passes iff >= 0.98
VERDICT_ABI_VERSION = 1                    # must equal VERDICT_ABI_VERSION
CORPUS_SEED_HEX = "0xA20C0FFEE5A7E000"     # §5.1 pinned seed (display form)
CORPUS_ROWS = 1_000_000                    # §5.1 exactly one million rows
SHA256_BYTES = 32                          # §6.1 manifests, sha256 fail-closed
MAX_COURTS_PER_RUN = 32                    # mirrors tests/court_verdict.rs

# §9 DoD command surface — CLI mode names frozen at Wave 1.
CLI_MODES = ("--mutate", "--shim-court-only", "--deps", "--strings")

# §6.1 step 7 JSON ledger schema keys, frozen (order = emission order).
LEDGER_SCHEMA = (
    "abi_version",
    "mutant_id",
    "verdict",
    "killed_by",
    "passing_courts",
    "mutated_sha256",
    "kill_rate",
)

# VerdictClass wire names (must match tests/court_verdict.rs discriminants).
VERDICT_CLASSES = (
    "Baseline",
    "ShimCourt",
    "KilledByBuild",
    "KilledByTest",
    "Equivalent",
    "SurvivorDefect",
    "SkipLoud",
    # Wave 8: a pre-write interference check refused the campaign (typed,
    # fail-closed) because a target file changed under the gate between its
    # snapshot and a mutant write — witnessed this wave when a concurrent
    # same-checkout writer flipped court_verdict.rs mid-campaign and
    # contaminated ledger verdicts (spurious KilledByBuild rows that a
    # stable-snapshot re-run reclassified as KilledByTest/Equivalent).
    "InterferenceRefusal",
)

TYPED_REASON_NOT_IMPLEMENTED = (
    "NOT IMPLEMENTED (WAVE-4 PENDING): the §6.4 dependency/string gates "
    "consume the A07 register at campaign seal time; refusing fail-closed "
    "until the enforcement wiring lands (A20 §6.4)."
)

# --- Wave-2/4 engine configuration -------------------------------------------

# §6.1 step 1: mutable targets. DEFAULT campaign scope: the A20 court kernel
# (tests/court_verdict.rs — A20-sole-writer per §7 merge matrix), which is
# what the DoD §9.2 command runs. The §6.1 per-kernel sources of the covered
# crates enter via --targets (Wave 4; closes the wave-3 divergence #5
# deferral). Opt-in because a kernel mutant transiently writes another
# lane's source file (sha256-fail-closed restore per mutant): kernel
# campaigns belong in a sandbox copy or a quiet seal window, never the
# default.
MUTABLE_TARGETS = ["tests/court_verdict.rs"]

# §6.1 kernel sources the courts directly consume (repo-relative). NOT
# auto-included; --targets selects from these or any existing repo-relative
# file (validated fail-closed). Listed here for discoverability and for the
# ledger's disclosure of the intended §6.1 scope.
KERNEL_TARGET_CANDIDATES = (
    "crates/cgap-storage/src/atomic_commit.rs",  # A05 WAL/recovery kernels
    "crates/cgap-bn3/src/unify.rs",              # A12 unifier kernels
    "crates/cgap-gate/src/penstock.rs",          # A14 ingress boundary
    "crates/cgap-core/src/frame.rs",             # A04 frame codec
    "crates/cgap-blake3/src/lib.rs",             # BLAKE3 kernel
)

# Court command: the A20 suite is the court the gate proves can fail.
COURT_ARGS = ["cargo", "test", "-p", "cgap-court", "--test", "court_verdict"]
COURT_ENV_OVERRIDE = {"CARGO_PROFILE_DEV_OPT_LEVEL": "2"}
COURT_TIMEOUT_S = 900

# §6.1 step 3 operator set (string-level, deterministic). Wave 4 completes
# it: `ret-remove` deletes an early-return statement (control-flow change
# or, for value-returning sites, a typed build abort — both kills);
# `mask-corrupt` corrupts a mask literal (`& 0x…` / `| 0x…`, value - 1,
# same-width hex) or a shift/rotate amount (`<<`/`>>`/`rotate_left`/
# `rotate_right`, +1) — the branchless-kernel corruption class §6.1 names.
OPERATORS = (
    "hex-bump",
    "int-bump-up",
    "int-bump-down",
    "cmp-swap",
    "bool-flip",
    "ret-remove",
    "mask-corrupt",
)
# Wave 4: empty — both kernel operators landed (key retained so ledgers
# stay diff-stable against wave-3 reports).
OPERATORS_DEFERRED = {}

# The committed §6.3 shim mutant: bump the standard SplitMix64 mix constant
# in tests/court_verdict.rs. Every generated byte of the 1M-row corpus
# changes, so the pinned corpus receipt (§5.1) must flip
# `corpus_determinism_receipt` — a known-killable mutant by construction.
# Wave 4: the HOST is pinned, not searched — the anchor text also occurs in
# other covered sources (penstock.rs carries the same SplitMix64 constant;
# witnessed by the gate's own SHIM SITE AMBIGUOUS refusal this session), so
# textual uniqueness was never a sound host-selection rule.
SHIM_HOST = "tests/court_verdict.rs"
SHIM_MARKER = "0xBF58_476D_1CE4_E5B9"
SHIM_REPLACEMENT = "0xBF58_476D_1CE4_E5BA"
SHIM_ANCHOR = "wrapping_mul(0xBF58_476D_1CE4_E5B9)"
SHIM_OPERATOR = "hex-bump"

# §8 court runtime: mutants are stride-sampled above --max-mutants; the
# stride is recorded in the ledger — never hidden.
DEFAULT_MAX_MUTANTS = 24

# §6.2 committed Equivalent-triage register (Wave 5): the hand triage of
# the current stride-42/24 sample over tests/court_verdict.rs. Each row is
# a survivor proven observation-identical for the property that court
# pins — a mutant of the TEST'S OWN SETUP MATERIAL whose change no pinned
# observation can see. Justifications are property-based (what the court
# asserts), never outcome-based ("it passed"). Fail-closed law: any key
# here that matches no survivor row in a run (court-file line drift,
# sample shift) refuses the gate — refresh the register from that run's
# ledger, never widen it to force a pass.
#   Re-derived and re-witnessed this session on the Wave-5 tree (campaign
#   ledger + per-site property analysis; the wave-3 audit's survivor set
#   shrank by one: the Wave-4 row-0 anchor hardening removed the `i == 0`
#   survivor, and the Wave-5 `compiled.rule_id` pin converts the
#   `rule_id: 7 -> 6` survivor into a kill — witnessed this session, a
#   10 pass / 1 fail flip of a12_unifier_join_court with a sha256-exact
#   restore). Refreshed once this session after the pin shifted the
#   stride-42 sample: the three keys that fell out of the sample were
#   REMOVED (never widened), and the two newly-sampled sites were triaged
#   from the run ledger.
#   Wave-9 refresh (this session, failed-lane restart): the appended
#   a04_identity_court grew the enumerated corpus, the stride shifted, and
#   the fail-closed register check refused 3 stale keys (:918/:1085/:1626
#   no longer sampled) — removed per the register's own law; the two
#   currently-sampled survivor sites were triaged from the run ledger
#   (property analysis below); :1735 remains sampled and keeps its row.
EQUIVALENT_TRIAGE = {
    "tests/court_verdict.rs:827:int-bump-up:0": (
        "Row-0 admission anchor SOURCE index `rows[0]` -> `rows[1]`: the "
        "anchor's only obligation is to differ from row 0's key (row 0 is "
        "never degenerate, so row 0's admission check is `!=`), and "
        "prev_key is re-seeded from row 0 after the first iteration, so no "
        "later row consumes the anchor. Over the sha256-receipt-pinned "
        "corpus the two anchors are distinct values, so every pinned "
        "observation (1M admissions, nonzero checksum, 976-alloc budget) "
        "is identical. Corpus-pinned equivalence, disclosed as such."
    ),
    "tests/court_verdict.rs:1625:int-bump-down:0": (
        "Content-independence length list is a coverage sample: the "
        "pinned property is length-independence of the ingress verdict "
        "(same status at any payload_len, view length tracks the declared "
        "length — asserted per iteration), so `1024` -> `1023` swaps one "
        "non-boundary sample point for another."
    ),
    "tests/court_verdict.rs:1735:int-bump-down:9": (
        "KAT sweep length `200` -> `199`: the streaming-identity property "
        "holds at every length, the list is a coverage sample of block "
        "boundaries (54..57, 63..65, 119/120, 200, 1000), and the mutant "
        "swaps one non-boundary sample point for another."
    ),
}


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def read_bytes(path):
    with open(path, "rb") as f:
        return f.read()


def write_bytes(path, data):
    with open(path, "wb") as f:
        f.write(data)


# --- line analysis -----------------------------------------------------------

def code_regions(line):
    """Character ranges of a line that are CODE: not string interiors, not a
    comment suffix. Positions are original-line indices."""
    regions = []
    start = 0
    i = 0
    in_str = False
    while i < len(line):
        c = line[i]
        if in_str:
            if c == '"' and line[i - 1] != "\\":
                i += 1
                in_str = False
                continue
        elif c == '"':
            if i > start:
                regions.append((start, i))
            in_str = True
        elif c == "/" and i + 1 < len(line) and line[i + 1] == "/":
            if i > start:
                regions.append((start, i))
            return regions
        i += 1
    if in_str:
        regions.append((start, len(line)))
    elif len(line) > start:
        regions.append((start, len(line)))
    return regions


def in_code(regions, start, end):
    return any(a <= start and end <= b for a, b in regions)


def find_matches(line, regex):
    """Matches fully inside code regions: [(start, end, text)]."""
    regions = code_regions(line)
    out = []
    for m in regex.finditer(line):
        if in_code(regions, m.start(), m.end()):
            out.append((m.start(), m.end(), m.group(0)))
    return out


HEX_RE = re.compile(r"0[xX][0-9A-Fa-f_]+")
INT_RE = re.compile(r"(?<![\w.])(\d+)(?![\w.])")
CMP_RE = re.compile(r"(?<=[ (])(<=|>=|==|!=|<|>)(?=[ )])")
BOOL_RE = re.compile(r"\b(true|false)\b")
# §6.1 `ret-remove`: an early-return statement, single-line, fully inside a
# code region (multi-line returns are simply not enumerated — deterministic
# and recorded, never silently mis-mutated).
RET_RE = re.compile(r"\breturn\b[^;{}\n]*;")
# §6.1 `mask-corrupt`: (a) a mask literal directly following `&` or `|`;
# (b) a shift/rotate amount following `<<`/`>>` or inside
# rotate_left/rotate_right(...). Evaluated masks-first, then shifts,
# left-to-right — deterministic.
MASK_RE = re.compile(r"[&|]\s*(0[xX][0-9A-Fa-f_]+)")
SHIFT_RE = re.compile(r"(?:<<|>>)\s*(\d+)|rotate_(?:left|right)\(\s*(\d+)\s*\)")

CMP_SWAP = {"<": "<=", "<=": "<", "==": "!=", "!=": "==", ">": ">=", ">=": ">"}


def _rebuild_hex(lit, new_value):
    """Rebuild a hex literal at the same digit width, underscores kept in
    place; None when the value is not representable at that width."""
    digits = lit[2:].replace("_", "")
    width = len(digits)
    if new_value < 0:
        return None
    new_digits = format(new_value, "0%dX" % width)
    if len(new_digits) != width:
        return None
    out = []
    di = 0
    for ch in lit[2:]:
        if ch == "_":
            out.append("_")
        else:
            out.append(new_digits[di])
            di += 1
    return "0x" + "".join(out)


def mutate_line(line, operator, occurrence):
    """One operator instance on a line; None when the occurrence doesn't
    exist. Deterministic: occurrence counts matches left-to-right."""
    if operator == "hex-bump":
        for start, end, lit in find_matches(line, HEX_RE):
            digits = lit[2:].replace("_", "")
            if not digits:
                continue
            new_lit = _rebuild_hex(lit, int(digits, 16) + 1)
            if new_lit is None:
                continue  # fixed-width carry: not representable, skip
            if occurrence == 0:
                return line[:start] + new_lit + line[end:]
            occurrence -= 1
        return None
    if operator in ("int-bump-up", "int-bump-down"):
        for start, end, text in find_matches(line, INT_RE):
            n = int(text)
            if operator == "int-bump-down":
                if n == 0:
                    continue  # underflow not representable, skip
                new = str(n - 1)
            else:
                new = str(n + 1)
            if occurrence == 0:
                return line[:start] + new + line[end:]
            occurrence -= 1
        return None
    if operator == "cmp-swap":
        for start, end, tok in find_matches(line, CMP_RE):
            if occurrence == 0:
                return line[:start] + CMP_SWAP[tok] + line[end:]
            occurrence -= 1
        return None
    if operator == "bool-flip":
        for start, end, tok in find_matches(line, BOOL_RE):
            new = "false" if tok == "true" else "true"
            if occurrence == 0:
                return line[:start] + new + line[end:]
            occurrence -= 1
        return None
    if operator == "ret-remove":
        for start, end, _stmt in find_matches(line, RET_RE):
            if occurrence == 0:
                return line[:start] + line[end:]
            occurrence -= 1
        return None
    if operator == "mask-corrupt":
        # masks first, then shift/rotate amounts; each left-to-right. The
        # match span includes the `&`/`|`/`<<` context; only the trailing
        # literal/amount is replaced, never the operator token.
        for start, end, text in find_matches(line, MASK_RE):
            m_lit = HEX_RE.search(text)
            if not m_lit:
                continue
            lit = text[m_lit.start():]
            digits = lit[2:].replace("_", "")
            if not digits:
                continue
            new_lit = _rebuild_hex(lit, int(digits, 16) - 1)
            if new_lit is None:
                continue
            if occurrence == 0:
                return line[: start + m_lit.start()] + new_lit + line[end:]
            occurrence -= 1
        for start, end, text in find_matches(line, SHIFT_RE):
            last_digits = None
            for m_d in re.finditer(r"\d+", text):
                last_digits = m_d
            if last_digits is None:
                continue
            dstart = start + last_digits.start()
            dend = dstart + len(last_digits.group())
            n = int(last_digits.group())
            if occurrence == 0:
                return line[:dstart] + str(n + 1) + line[dend:]
            occurrence -= 1
        return None
    raise ValueError("unknown operator: %s" % operator)


def enumerate_mutants(lines):
    """All (line_no_1based, operator, occurrence) triples; deterministic
    order: line ascending, operator order = OPERATORS, occurrences L-to-R."""
    mutants = []
    for line_no, line in enumerate(lines, start=1):
        for op in OPERATORS:
            occ = 0
            while True:
                mutated = mutate_line(line, op, occ)
                if mutated is None:
                    break
                mutants.append((line_no, op, occ))
                occ += 1
    return mutants


# --- court runner ------------------------------------------------------------

def run_courts(target_dir=None):
    """Run the A20 court suite. Returns (returncode, output, timed_out)."""
    env = dict(os.environ)
    env.update(COURT_ENV_OVERRIDE)
    if target_dir:
        env["CARGO_TARGET_DIR"] = target_dir
    try:
        proc = subprocess.run(
            COURT_ARGS,
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            timeout=COURT_TIMEOUT_S,
        )
        return proc.returncode, proc.stdout + proc.stderr, False
    except subprocess.TimeoutExpired as exc:
        out = exc.stdout or ""
        err = exc.stderr or ""
        if isinstance(out, bytes):
            out = out.decode(errors="replace")
        if isinstance(err, bytes):
            err = err.decode(errors="replace")
        return 124, out + err, True


def classify_run(returncode, output, timed_out):
    """§6.1 steps 4-5 -> (verdict, killed_by, passing_courts)."""
    failed = re.findall(r"^\s*test (\S+) \.\.\. FAILED", output, re.M)
    passed = re.findall(r"^\s*test (\S+) \.\.\. ok", output, re.M)
    cap = passed[:MAX_COURTS_PER_RUN]
    if timed_out:
        return "KilledByTest", "timeout", cap
    if returncode != 0:
        build_failed = "test result:" not in output and "could not compile" in output
        if build_failed:
            return "KilledByBuild", "build", []
        if failed:
            # Wave 5 (audit divergence #4): the failing-court set under a
            # wide mutant (e.g. the shim flips the corpus receipt and the
            # courts downstream of it) is emitted in thread-scheduling
            # order; picking failed[0] made `killed_by` wobble run to run.
            # Sorted-first is replay-stable — the kill itself was always
            # real; only the attribution name wobbled.
            return "KilledByTest", sorted(failed)[0], cap
        return "KilledByTest", "suite-exit-nonzero", cap
    return "Baseline", "", cap


# --- ledger ------------------------------------------------------------------

def ledger_row(target, line_no, operator, occurrence, verdict, killed_by,
               passing_courts, mutated_sha, kill_rate):
    row = {
        "abi_version": VERDICT_ABI_VERSION,
        "mutant_id": {
            "file": target,
            "line": line_no,
            "operator": operator,
            "index": occurrence,
        },
        "verdict": verdict,
        "killed_by": killed_by,
        "passing_courts": passing_courts,
        "mutated_sha256": mutated_sha,
        "kill_rate": kill_rate,
    }
    assert tuple(row.keys()) == LEDGER_SCHEMA
    assert verdict in VERDICT_CLASSES
    return row


def restore_or_die(path, baseline, sha, ledger, report):
    """§6.1 step 6 — byte-exact restore, fail-closed. Wave 8: the verify is
    a real readback (the bytes on disk are re-hashed, not the in-memory
    baseline), so a restore racing a concurrent writer to the target file
    refuses instead of silently confirming an overwrite of foreign
    content."""
    write_bytes(path, baseline)
    reason = verify_unmutated(path, sha)
    if sha256_bytes(baseline) != sha or reason:
        print("RESTORE MISMATCH (sha256 fail-closed): %s%s"
              % (path, (" — %s" % reason) if reason else ""), file=sys.stderr)
        write_report(ledger, report)
        sys.exit(EXIT_REFUSE)


def write_report(ledger, report):
    # Wave 9 defect repair (a20-wave9 audit D3, witnessed: a full ~15-min
    # campaign completed all mutants then died here with FileNotFoundError
    # because the --report parent dir did not pre-exist, losing the ledger).
    # The ledger is the campaign's only durable receipt, so emitting it must
    # not depend on caller-side mkdir: the parent dir is created (or the
    # write failure still raises loudly — an unwritable path refuses, never
    # silently drops the ledger).
    if report:
        parent = os.path.dirname(os.path.abspath(report))
        os.makedirs(parent, exist_ok=True)
        with open(report, "w") as f:
            json.dump(ledger, f, indent=2)
            f.write("\n")


def verify_unmutated(path, expected_sha):
    """Wave 8 (§6.1 step 5 write path): pre-write interference check.
    Re-reads a target file and compares its sha256 against the campaign
    snapshot hash; returns None when the file is still byte-identical,
    else a typed refusal reason.

    Motivation (witnessed, not hypothetical): during the Wave-8 session a
    concurrent same-checkout writer flipped tests/court_verdict.rs between
    two content hashes mid-campaign; the 06:51 campaign recorded spurious
    KilledByBuild verdicts (three benign mutants the stable-snapshot re-run
    reclassified: :1580 KilledByTest, :1626/:1735 honest Equivalents) and
    every restore silently reverted foreign content. Mutating over a
    foreign write corrupts two ways at once: the ledger verdicts (the
    mutant actually exercised is foreign content, not the recorded mutant)
    and the other writer's in-flight work (a restore clobbers it). The
    check turns both into one loud typed refusal (exit 1) before any
    further write.
    """
    with open(path, "rb") as f:
        current = f.read()
    current_sha = sha256_bytes(current)
    if current_sha == expected_sha:
        return None
    return ("FOREIGN WRITE DETECTED on %s: sha256 %s... != snapshot %s... "
            "(concurrent writer or mid-campaign edit; refusing to mutate "
            "over foreign content)" % (path, current_sha[:16],
                                       expected_sha[:16]))


# --- campaign lock (Wave 9: interference prevention, a20-wave9 audit D2) -----

# Lock file lives under the repo's build dir (build state, not source; cargo
# creates `target/` on any build). Advisory flock, held for the campaign
# process lifetime; the kernel releases it even on crash or SIGKILL.
CAMPAIGN_LOCK_PATH = os.path.join(REPO, "target", ".a20-campaign.lock")

_campaign_lock_fd = None  # process-lifetime holder; None = not acquired


def acquire_campaign_lock():
    """Wave 9: exclusive non-blocking advisory lock over the shared tree's
    mutation campaign (modes that write target files in place: --mutate,
    --shim-court-only).

    Witnessed class (a20-wave9 audit D2, live interference incident): a
    campaign mutating `tests/court_verdict.rs` in place raced another
    session's court run / sandbox copy, which read a LIVE mutant as pristine
    and burned a full isolate-and-reproduce investigation on a false corpus-
    pin failure. The Wave-8 pre-write/pre-restore checks DETECT a racing
    writer; this lock PREVENTS the gate-vs-gate collision class: a second
    campaign on the same checkout refuses typed (exit 1) instead of
    interleaving mutants with the first.

    Returns nothing; refuses fail-closed (exit 1) when the lock is held.
    The fd is held for the process lifetime — flock is released by the
    kernel on process exit even after a crash, so no stale-lock file can
    wedge the gate shut (a stale file with no live holder acquires
    instantly).
    """
    global _campaign_lock_fd
    if _campaign_lock_fd is not None:
        return  # already held this process
    os.makedirs(os.path.dirname(CAMPAIGN_LOCK_PATH), exist_ok=True)
    fd = os.open(CAMPAIGN_LOCK_PATH, os.O_CREAT | os.O_RDWR, 0o644)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        holder = "unknown holder"
        try:
            with open(CAMPAIGN_LOCK_PATH, "rb") as f:
                holder = f.read().decode(errors="replace").strip() or holder
        except OSError:
            pass
        print("GATE REFUSED: CAMPAIGN LOCK HELD on %s (%s) — another "
              "mutation campaign is live on this checkout; two gate "
              "campaigns must never interleave mutants on one shared tree "
              "(witnessed: a20-wave9 audit D2). Retry when the holder "
              "exits; the lock is advisory flock, released by the kernel "
              "when the holder process dies." % (CAMPAIGN_LOCK_PATH, holder),
              file=sys.stderr)
        os.close(fd)
        sys.exit(EXIT_REFUSE)
    os.ftruncate(fd, 0)
    os.write(fd, ("pid=%d gate=boundary_gate.py mode=mutation-campaign\n"
                  % os.getpid()).encode())
    os.fsync(fd)
    _campaign_lock_fd = fd


# --- shim court (§6.3) -------------------------------------------------------

def plant_shim(lines):
    """Locate the committed shim site; returns (line_idx, mutated_line)."""
    for idx, line in enumerate(lines):
        if SHIM_ANCHOR in line:
            return idx, line.replace(SHIM_MARKER, SHIM_REPLACEMENT)
    return None, None


def shim_court(state, run, inert=False, ledger=None, report=None):
    """§6.3: plant -> run -> restore (fail-closed). Returns (killed, row).
    `state` is the target state dict of the court kernel (the shim host)."""
    target_path = state["path"]
    baseline = state["baseline"]
    sha = state["sha"]
    if inert:
        # Negative control: the "mutant" is the baseline bytes themselves.
        mutated = baseline
        mutant_sha = sha
        line_no = 0
        op = "shim-inert"
    else:
        idx, mutated_line = plant_shim(state["lines"])
        if idx is None:
            print("SHIM SITE NOT FOUND: %s" % SHIM_ANCHOR, file=sys.stderr)
            sys.exit(EXIT_REFUSE)
        lines2 = baseline.decode().split("\n")
        lines2[idx] = mutated_line
        mutated = "\n".join(lines2).encode()
        mutant_sha = sha256_bytes(mutated)
        line_no = idx + 1
        op = SHIM_OPERATOR
    # Wave 8: pre-write interference check — a foreign write to the target
    # between the gate's snapshot and this write must refuse, never be
    # mutated over (witnessed case in verify_unmutated's docstring).
    reason = verify_unmutated(target_path, sha)
    if reason:
        print("GATE REFUSED: %s" % reason, file=sys.stderr)
        if ledger is not None:
            ledger["rows"].append(ledger_row(
                target_path, line_no, op, 0, "InterferenceRefusal",
                "foreign-write", [], mutant_sha, None))
            write_report(ledger, report)
        sys.exit(EXIT_REFUSE)
    write_bytes(target_path, mutated)
    rc, output, timed_out = run()
    verdict, killed_by, passed = classify_run(rc, output, timed_out)
    # Wave 8 (e2e witness): a foreign write racing the RESTORE window gets
    # silently clobbered by the restore unless the restore itself checks —
    # witnessed live in the sandbox (the first injection was overwritten by
    # the gate's own restore and the campaign passed clean). The gate knows
    # the exact sha it wrote; if the disk content differs at restore time,
    # a third party wrote here and restoring would destroy it.
    race = verify_unmutated(target_path, mutant_sha)
    if race:
        print("GATE REFUSED: %s" % race, file=sys.stderr)
        if ledger is not None:
            ledger["rows"].append(ledger_row(
                target_path, line_no, op, 0, "InterferenceRefusal",
                "restore-race", [], sha256_bytes(read_bytes(target_path)),
                None))
            write_report(ledger, report)
        sys.exit(EXIT_REFUSE)
    restore_or_die(target_path, baseline, sha, ledger, report)
    killed = verdict != "Baseline"
    row = ledger_row(target_path, line_no, op, 0,
                     "ShimCourt" if killed else "Baseline",
                     killed_by, passed, mutant_sha, None)
    return killed, row, output


# --- sealers (§6.4, Wave 3) --------------------------------------------------

# §6.4 coverage set: the campaign's execution crates. Path-joined to REPO.
# Every row must exist and be parseable — a missing covered crate is a
# fail-closed refusal (§10: loud, never silently dropped).
COVERED_CRATES = (
    "crates/cgap-core",
    "crates/cgap-storage",
    "crates/cgap-bont",
    "crates/cgap-bosym",
    "crates/cgap-bn3",
    "crates/cgap-gate",
    "crates/cgap-crypto",
    "crates/cgap-p28",
    "crates/cgap-court",
    "crates/audit",
    "crates/cgap-arrow",
    "crates/cgap-blake3",
    "crates/cgap-git",
    "crates/cgap-parquet",
    "ontologies/fibo",
)

# §6.4 string gate input: the A07 register (Wave 6, item 11). A20 maintains
# NO pattern list and NO allowlist — `zero_string_gate.py --register` (lane
# A07, courts T14a/T14b) is the canonical home of the cloud-string patterns
# + allowlist, and this gate consumes it at runtime, never edits it. The
# pattern-tightening history (word boundaries; the `s3.<host>` endpoint form
# after the `s3.verify` false positive) lives where it is owned now: in the
# register, pinned by A07's T14a court.
A07_REGISTER_CMD = ("scripts/zero_string_gate.py", "--register")
A07_REGISTER_NAME = "a07-zero-string-dep-audit"
# Wave 7: consumed ABI v1 -> v2 (the register bumped mid-session by the
# restarted A07 lane; this gate refused fail-closed on v2 until pinned).
# Verified shape-compatible before the pin: v2 is additive on the v1 schema
# (new top-level keys: emitter_exemptions, single_list_law, ontology,
# consumers, owned_by, divergences); EVERY consumed field is unchanged in
# shape — cloud_strings.patterns ([regex, tag] pairs), cloud_strings.
# allowlist (justified entries), cloud_strings.a20_6_4_doc_list.
A07_REGISTER_VERSION_CONSUMED = 2
A07_REGISTER_TIMEOUT_S = 60

# Wave 7 (a07-wave6 audit open item 4): the canonical A07 artifact scan —
# `zero_string_gate.py` in its DEFAULT mode (cmd_scan) — is the single home
# of the artifact-quarantine predicate (vocab derivation, artifact
# discovery, emitter-exemption class). A20 consumes its VERDICT (the
# machine SUMMARY line + exit code) at --strings seal time; A20 maintains
# no second scanner, no artifact list, no exemption class (§6.4 single-
# kernel law). Exit ABI of the canonical gate: 0 = clean SUMMARY, 1 = hits
# red OR typed GateRefused — so exit alone is ambiguous by design and the
# SUMMARY line is the verdict carrier.
A07_SCAN_CMD = ("scripts/zero_string_gate.py",)
A07_SCAN_TIMEOUT_S = 300
# SUMMARY line grammar (emitted by cmd_scan only; "COURTS-SUMMARY" and
# "DEP-DIR-SUMMARY" do not match the ^SUMMARY anchor).
A07_SUMMARY_LINE_RE = re.compile(r"^SUMMARY (.+)$", re.M)
A07_SUMMARY_REQUIRED_INT_FIELDS = ("scanned", "crates", "hits")


def load_a07_register():
    """Emit + parse the A07 register (§6.4 single-list law). Returns a dict
    with the compiled patterns/allowlist and the emission's sha256
    provenance. Fail-closed on EVERY consumption failure mode: spawn error,
    timeout, nonzero emitter exit, unparsable JSON, wrong register name,
    unknown register version, zero canonical patterns, uncompilable
    pattern, allowlist entry without ALLOWLIST-JUSTIFICATION. A strings
    gate that cannot obtain its canonical list must refuse, never fall
    back to a private copy."""
    cmd = [sys.executable, os.path.join(REPO, A07_REGISTER_CMD[0])] + \
        list(A07_REGISTER_CMD[1:])
    try:
        proc = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True,
                              timeout=A07_REGISTER_TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as exc:
        print("A07 REGISTER UNAVAILABLE: %s (%s)" % (" ".join(cmd), exc),
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    if proc.returncode != 0:
        print("A07 REGISTER REFUSED (emitter exit %d): %s"
              % (proc.returncode, (proc.stderr or "").strip()[-400:]),
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    raw = proc.stdout
    try:
        reg = json.loads(raw)
    except json.JSONDecodeError as exc:
        print("A07 REGISTER UNPARSABLE: %s" % exc, file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    if not isinstance(reg, dict) or reg.get("register") != A07_REGISTER_NAME:
        print("A07 REGISTER UNRECOGNIZED: not the %s register"
              % A07_REGISTER_NAME, file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    version = reg.get("version")
    if version != A07_REGISTER_VERSION_CONSUMED:
        print("A07 REGISTER VERSION %r not consumable by this gate "
              "(consumed ABI: v%d); refusing rather than guessing the "
              "schema" % (version, A07_REGISTER_VERSION_CONSUMED),
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    cloud = reg.get("cloud_strings")
    if not isinstance(cloud, dict):
        print("A07 REGISTER SCHEMA: missing cloud_strings object",
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    raw_patterns = cloud.get("patterns")
    if not isinstance(raw_patterns, list) or not raw_patterns:
        print("A07 REGISTER VACUOUS: cloud_strings.patterns carries %r — a "
              "strings gate over an empty canonical list enforces nothing "
              "(anti-vacuity; refuse, never pass)" % (raw_patterns,),
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    compiled = []
    for entry in raw_patterns:
        try:
            pat, tag = entry
        except (TypeError, ValueError):
            print("A07 REGISTER SCHEMA: pattern entry is not [pattern, "
                  "tag]: %r" % (entry,), file=sys.stderr)
            sys.exit(EXIT_REFUSE)
        try:
            compiled.append((re.compile(pat), str(tag)))
        except re.error as exc:
            print("A07 REGISTER SCHEMA: pattern %r does not compile: %s"
                  % (pat, exc), file=sys.stderr)
            sys.exit(EXIT_REFUSE)
    raw_allow = cloud.get("allowlist", [])
    if not isinstance(raw_allow, list):
        print("A07 REGISTER SCHEMA: cloud_strings.allowlist is not a list",
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    allow = []
    for entry in raw_allow:
        match = entry.get("match") if isinstance(entry, dict) else None
        justification = entry.get("justification", "") \
            if isinstance(entry, dict) else ""
        if not match or not str(justification).startswith(
                "ALLOWLIST-JUSTIFICATION:"):
            print("A07 REGISTER SCHEMA: allowlist entry %r lacks a match or "
                  "an ALLOWLIST-JUSTIFICATION (T7 discipline is consumed "
                  "with the list)" % (entry,), file=sys.stderr)
            sys.exit(EXIT_REFUSE)
        allow.append(re.compile(match))
    return {
        "version": version,
        "sha256": sha256_bytes(raw.encode("utf-8")),
        "patterns": compiled,
        "allowlist": allow,
        "n_patterns": len(compiled),
        "n_allow": len(allow),
        "doc_list": list(cloud.get("a20_6_4_doc_list", [])),
    }


# --- canonical artifact-scan consumption (§6.4, Wave 7) ----------------------

def _parse_a07_summary(stdout):
    """Parse the canonical gate's machine SUMMARY line into a field dict.
    Returns None when the line is absent or unparsable — the caller treats
    that as a consumption failure (typed refusal), never a pass."""
    match = A07_SUMMARY_LINE_RE.search(stdout or "")
    if not match:
        return None
    fields = {}
    for token in match.group(1).split():
        if "=" not in token:
            return None
        key, value = token.split("=", 1)
        fields[key] = value
    for required in A07_SUMMARY_REQUIRED_INT_FIELDS:
        if required not in fields:
            return None
        try:
            fields[required] = int(fields[required])
        except (TypeError, ValueError):
            return None
    return fields


def a07_canonical_scan():
    """Run the canonical A07 artifact scan over THIS tree and consume its
    verdict (§6.4 single-kernel law, Wave 7). Returns a verdict dict:
    {"verdict": "GREEN"|"RED", "exit", "summary", "stdout_sha256", ...}.
    GREEN requires exit 0 + hits=0 + scanned>=1 (a green with zero scanned
    artifacts would be a vacuous scan); RED requires exit 1 + hits>=1.
    Every other outcome — spawn error, timeout, missing/unparsable SUMMARY
    (GateRefused lands here), exit-vs-summary mismatch — is a typed
    consumption failure and refuses (exit 1), never passes."""
    cmd = [sys.executable, os.path.join(REPO, A07_SCAN_CMD[0])] + \
        list(A07_SCAN_CMD[1:])
    try:
        proc = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True,
                              timeout=A07_SCAN_TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as exc:
        print("A07 CANONICAL SCAN UNAVAILABLE: %s (%s)"
              % (" ".join(cmd), exc), file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    fields = _parse_a07_summary(proc.stdout)
    if fields is None:
        tail = ((proc.stderr or "") + (proc.stdout or "")).strip()[-400:]
        print("A07 CANONICAL SCAN NOT GREEN: no parsable SUMMARY verdict "
              "line (typed GateRefused or crash — exit %d): %s"
              % (proc.returncode, tail), file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    try:
        proc_sha = sha256_bytes(proc.stdout.encode("utf-8"))
    except Exception:
        proc_sha = None
    hits = fields["hits"]
    scanned = fields["scanned"]
    if proc.returncode == 0 and hits == 0 and scanned >= 1:
        verdict = "GREEN"
    elif proc.returncode == 1 and hits >= 1:
        verdict = "RED"
    else:
        print("A07 CANONICAL SCAN CONSUMPTION REFUSED: exit %d vs SUMMARY "
              "hits=%s scanned=%s mismatch the gate ABI (0/hits=0/scanned>=1"
              " -> GREEN; 1/hits>=1 -> RED)" % (proc.returncode, hits,
                                                scanned), file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    return {
        "verdict": verdict,
        "exit": proc.returncode,
        "summary": fields,
        "stdout_sha256": proc_sha,
        "source": "python3 scripts/zero_string_gate.py (default scan mode; "
                  "A20 §6.4: verdict consumed, never re-adjudicated or "
                  "edited)",
    }


def canonical_scan_verdict_at(root, label):
    """Court helper (Wave 7): run the REPO's canonical gate against an
    arbitrary root via its real --root flag and return the parsed verdict
    dict WITHOUT exiting (returns "CONSUMPTION-REFUSED" as verdict.reason
    on failure modes). Used only by --cross-gate-court to witness a REAL
    canonical RED over a REAL fixture."""
    cmd = [sys.executable, os.path.join(REPO, A07_SCAN_CMD[0]),
           "--root", str(root)]
    try:
        proc = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True,
                              timeout=A07_SCAN_TIMEOUT_S)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"verdict": "CONSUMPTION-REFUSED", "exit": None,
                "reason": "spawn/timeout: %s" % exc}
    fields = _parse_a07_summary(proc.stdout)
    if fields is None:
        return {"verdict": "CONSUMPTION-REFUSED", "exit": proc.returncode,
                "reason": ("no parsable SUMMARY (GateRefused/crash): %s"
                           % ((proc.stderr or "")[-300:]))}
    hits = fields["hits"]
    scanned = fields["scanned"]
    if proc.returncode == 0 and hits == 0 and scanned >= 1:
        verdict = "GREEN"
    elif proc.returncode == 1 and hits >= 1:
        verdict = "RED"
    else:
        verdict = "CONSUMPTION-REFUSED"
        return {"verdict": verdict, "exit": proc.returncode,
                "reason": "exit-vs-summary mismatch: hits=%s scanned=%s"
                          % (hits, scanned)}
    return {"verdict": verdict, "exit": proc.returncode, "summary": fields}


def covered_crate_dirs():
    """Absolute dirs of the covered crates; refuse fail-closed on absence."""
    missing = [c for c in COVERED_CRATES if not os.path.isdir(os.path.join(REPO, c))]
    if missing:
        print("DEPS GATE: covered crates missing from the tree: %s" % missing,
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)


def parse_runtime_deps(crate_dir):
    """Runtime-external count per §6.4/A07 method: a [dependencies] entry is
    EXTERNAL iff it declares a registry `version`, a `git` source, or any
    source other than path/workspace. Dev-dependencies are not runtime.
    Returns (crate_name, external_deps, total_runtime_deps)."""
    manifest_path = os.path.join(REPO, crate_dir, "Cargo.toml")
    try:
        with open(manifest_path, "rb") as f:
            manifest = tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        print("DEPS GATE: cannot parse %s: %s" % (manifest_path, exc),
              file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    deps = manifest.get("dependencies", {})
    external = []
    for name, spec in deps.items():
        if isinstance(spec, str):
            # `name = "1.2"` is a registry (external) dependency.
            external.append(name)
            continue
        if not isinstance(spec, dict):
            print("DEPS GATE: %s: dep %s: unparseable spec %r"
                  % (manifest_path, name, spec), file=sys.stderr)
            sys.exit(EXIT_REFUSE)
        if "version" in spec or "git" in spec or "registry" in spec:
            external.append(name)
        elif "path" not in spec and "workspace" not in spec:
            # No source at all: cargo would treat it as registry — external.
            external.append(name)
    name = manifest.get("package", {}).get("name", os.path.basename(crate_dir))
    return name, external, len(deps)


def mode_deps(report=None):
    """§6.4 --deps: E=0 runtime externals for every covered crate."""
    covered_crate_dirs()
    rows = []
    total_external = 0
    for crate in COVERED_CRATES:
        name, external, total = parse_runtime_deps(crate)
        rows.append({
            "crate": crate,
            "name": name,
            "runtime_deps": total,
            "runtime_externals": external,
        })
        total_external += len(external)
        print("  %-26s E=%d  runtime_deps=%d%s"
              % (crate, len(external), total,
                 "" if not external else "  EXTERNAL: %s" % ",".join(external)))
    ledger = {
        "abi_version": VERDICT_ABI_VERSION,
        "mode": "deps",
        "kill_rate": None,
        "covered_crates": rows,
        "total_runtime_externals": total_external,
    }
    write_report(ledger, report)
    if total_external != 0:
        print("DEPS GATE FAILED: %d runtime external(s) across the covered "
              "crates (§6.4: E=0)" % total_external, file=sys.stderr)
        return EXIT_REFUSE
    print("DEPS GATE: E=0 runtime externals across %d covered crates "
          "(§6.4)" % len(COVERED_CRATES))
    return EXIT_PASS


def mode_strings(report=None):
    """§6.4 --strings: the cloud-string scan over covered crates' sources.
    Wave 6: the patterns + allowlist come from the A07 register at runtime
    (single-list law — A20 maintains no list); register consumption is
    fail-closed (load_a07_register), and the allowlist (empty by design
    upstream) suppresses a LINE only via a justified register entry."""
    covered_crate_dirs()
    reg = load_a07_register()
    compiled = reg["patterns"]
    allow = reg["allowlist"]
    rows = []
    hits = 0
    for crate in COVERED_CRATES:
        base = os.path.join(REPO, crate)
        for root, dirs, files in os.walk(base):
            dirs[:] = [d for d in dirs if d not in ("target", ".git")]
            for fn in sorted(files):
                if not (fn.endswith(".rs") or fn.endswith(".toml")):
                    continue
                p = os.path.join(root, fn)
                try:
                    body = open(p, encoding="utf-8", errors="replace").read()
                except OSError as exc:
                    print("STRINGS GATE: cannot read %s: %s" % (p, exc),
                          file=sys.stderr)
                    sys.exit(EXIT_REFUSE)
                rel = os.path.relpath(p, REPO)
                for n, line in enumerate(body.split("\n"), 1):
                    # A07 register grammar: a justified allowlist entry
                    # suppresses the line it matches (cloud allowlist is
                    # EMPTY upstream — every hit is a hit).
                    if any(rx.search(line) for rx in allow):
                        continue
                    for rx, tag in compiled:
                        if rx.search(line):
                            hits += 1
                            rows.append({
                                "crate": crate,
                                "file": rel,
                                "line": n,
                                "string": tag,
                                "text": line.strip()[:200],
                            })
                            print("  HIT %s:%d [%s] %s"
                                  % (rel, n, tag, line.strip()[:120]))
    # CROSS-GATE SEAM (Wave 7, a07-wave6 audit open item 4): §6.4 single-
    # kernel law at seal time — this gate's verdict INHERITS the canonical
    # A07 artifact-scan verdict over the same tree. The audit's finding —
    # A20 `--strings` green while the canonical quarantine scan was red on
    # the SAME tree — was a two-verdicts-one-tree channel; closed here
    # structurally. The emitter-exemption classification is consumed, never
    # re-adjudicated (hits=0 emitter_hits=N is GREEN by A07's own class).
    canon = a07_canonical_scan()
    if canon["verdict"] != "GREEN":
        print("STRINGS GATE FAILED: CROSS-GATE RED — the canonical A07 "
              "artifact scan over this tree is %s (exit=%s, hits=%s): the "
              "seal-time strings verdict inherits the canonical verdict "
              "(§6.4 single-kernel law); reconcile at the canonical gate "
              "(A07-owned emitter class / A09 quarantine routing)."
              % (canon["verdict"], canon["exit"],
                 canon.get("summary", {}).get("hits")), file=sys.stderr)
        canon_verdict = canon["verdict"]
        canon_for_ledger = canon
    else:
        canon_verdict = "GREEN"
        canon_for_ledger = canon

    ledger = {
        "abi_version": VERDICT_ABI_VERSION,
        "mode": "strings",
        "kill_rate": None,
        "patterns": [tag for _, tag in compiled],
        "a07_canonical_scan": {
            "verdict": canon_verdict,
            "exit": canon_for_ledger["exit"],
            "summary": canon_for_ledger.get("summary"),
            "stdout_sha256": canon_for_ledger.get("stdout_sha256"),
            "seam": "WAVE 7 (a07-wave6 open item 4): --strings inherits "
                    "the canonical artifact-scan verdict; consumption is "
                    "fail-closed (missing SUMMARY / exit-vs-summary "
                    "mismatch / scanned=0-green all refuse)",
            "source": canon_for_ledger.get("source"),
        },
        "a07_register": {
            "source": "python3 scripts/zero_string_gate.py --register "
                      "(A20 §6.4: consumed, never edited; T14a/T14b courts)",
            "version": reg["version"],
            "sha256": reg["sha256"],
            "n_patterns": reg["n_patterns"],
            "n_allowlist": reg["n_allow"],
            "doc_list_covered": reg["doc_list"],
            "single_list_law": "A20 maintains NO pattern list and NO "
                               "allowlist; the inline CLOUD_STRING_PATTERNS "
                               "copy and the a07_register_deferral row were "
                               "deleted in Wave 6 (a07-wave5 audit open "
                               "item 4)",
        },
        "hits": rows,
    }
    write_report(ledger, report)
    if hits:
        print("STRINGS GATE FAILED: %d cloud-string hit(s) (§6.4; A07 "
              "register v%d sha %s, allowlist=%d)"
              % (hits, reg["version"], reg["sha256"][:12], reg["n_allow"]),
              file=sys.stderr)
        return EXIT_REFUSE
    if canon_verdict != "GREEN":
        print("STRINGS GATE FAILED: CROSS-GATE RED — canonical A07 "
              "artifact scan verdict %s over this tree (exit=%s, hits=%s): "
              "the seal-time strings verdict inherits the canonical verdict "
              "(§6.4 single-kernel law; a07-wave6 open item 4). Reconcile "
              "at the canonical gate (A07-owned emitter class / A09 "
              "quarantine routing)."
              % (canon_verdict, canon_for_ledger["exit"],
                 (canon_for_ledger.get("summary") or {}).get("hits")),
              file=sys.stderr)
        return EXIT_REFUSE
    summary = canon_for_ledger.get("summary") or {}
    print("STRINGS GATE: 0 hits across %d covered crates (§6.4); A07 "
          "register v%d consumed (sha %s, %d canonical patterns, %d "
          "allowlist entries); CROSS-GATE: canonical A07 artifact scan "
          "GREEN (exit=%s, scanned=%s, hits=%s, emitter_hits=%s)"
          % (len(COVERED_CRATES), reg["version"],
             reg["sha256"][:12], reg["n_patterns"], reg["n_allow"],
             canon_for_ledger["exit"], summary.get("scanned"),
             summary.get("hits"), summary.get("emitter_hits")))
    return EXIT_PASS


def mode_deps_strings(which, report=None):
    """§6.4 dispatch. `which` is the requested mode set."""
    ran = False
    rc = EXIT_PASS
    if "deps" in which:
        ran = True
        rc = mode_deps(report)
    if "strings" in which:
        ran = True
        rc = mode_strings(report) or rc
    if not ran:  # unreachable via main()'s flag checks; defensive
        print(TYPED_REASON_NOT_IMPLEMENTED, file=sys.stderr)
        return EXIT_REFUSE
    return rc


def mode_cross_gate_court(report=None):
    """Wave 7 court for the CROSS-GATE SEAM (a07-wave6 audit open item 4).
    Chicago discipline: real collaborators only — the real canonical gate,
    a real cargo-built fixture binary, a real sandbox tree; assertions on
    final state (exit codes, ledger fields, scanned strings); no mocks.

    Arms:
      G (green end-to-end) — full `--strings` at the repo root: the source
          scan AND the canonical scan both run for real; the court requires
          exit 0 and a ledger whose a07_canonical_scan.verdict is GREEN
          with scanned>=1.
      R (real red) — a fixture workspace is built with real cargo: a probe
          bin crate embedding a real ontology token (read from the real
          ontologies/fibo/src/lib.rs) plus a clean `cgap-fibo` bin crate
          named so the canonical gate's emitter-exemption key
          ("cgap-fibo", "emit-fibo-pack") resolves. The REAL canonical gate
          (--root fixture) must report a REAL red: exit 1, SUMMARY
          hits>=1. This witnesses the exact verdict shape the seam refuses
          on, from the canonical gate itself — never a hand-faked verdict.
      F (flip) — a sandbox tree (real source clones + the built fixture) is
          run twice through a sandbox copy of this gate: un-neutered, the
          canonical red must flip the sandbox `--strings` to exit 1 with
          the typed CROSS-GATE reason; then the named anti-vacuity mutation
          `seam-neuter` (the seam call-site is replaced — anchored, count==1
          asserted) must flip it BACK to exit 0. A --strings that stays
          green under seam-neuter is a vacuous seam: the court exits 1.

    Any arm failure prints a typed reason and exits 1. Minted artifacts
    (fixture + sandbox temp trees) are deleted by this process (minter
    cleanup law); a failed deletion is printed loudly, never silent.
    """
    import shutil
    import tempfile

    failures = []
    token_re = re.compile(r"fibo:[A-Za-z0-9_]+(?::[A-Za-z0-9_]+)*")
    min_run = 8  # zero_string_gate MIN_RUN: tokens shorter are invisible

    def fail(msg):
        failures.append(msg)
        print("CROSS-GATE COURT FAIL: %s" % msg, file=sys.stderr)

    # --- Arm G: real green end-to-end at the repo root -----------------------
    ledger_g_path = os.path.join(tempfile.mkdtemp(prefix="a20cg-g-"),
                                 "ledger_g.json")
    rc_g = mode_strings(report=ledger_g_path)
    ledger_g = {}
    try:
        with open(ledger_g_path) as f:
            ledger_g = json.load(f)
    except (OSError, ValueError) as exc:
        fail("arm G: ledger unparsable: %s" % exc)
    canon_g = ledger_g.get("a07_canonical_scan") or {}
    if rc_g != EXIT_PASS:
        fail("arm G: mode_strings at repo root exited %s (expected %s)"
             % (rc_g, EXIT_PASS))
    if canon_g.get("verdict") != "GREEN":
        fail("arm G: ledger a07_canonical_scan.verdict=%r (expected GREEN)"
             % (canon_g.get("verdict"),))
    summary_g = canon_g.get("summary") or {}
    if summary_g.get("scanned", 0) < 1:
        fail("arm G: canonical summary scanned=%r (expected >=1)"
             % (summary_g.get("scanned"),))
    if not failures:
        print("CROSS-GATE COURT arm G: --strings exit 0; canonical GREEN "
              "(scanned=%s, hits=%s, emitter_hits=%s)"
              % (summary_g.get("scanned"), summary_g.get("hits"),
                 summary_g.get("emitter_hits")))

    # --- Arm R: real cargo-built fixture -> real canonical RED ---------------
    token = None
    try:
        with open(os.path.join(REPO, "ontologies/fibo/src/lib.rs"),
                  encoding="utf-8") as f:
            tokens = sorted(set(token_re.findall(f.read())))
        visible = [t for t in tokens if len(t) >= min_run]
        if not visible:
            fail("arm R: no ontology token >= %d chars in the real "
                 "ontologies/fibo/src/lib.rs" % min_run)
        token = visible[0]
    except OSError as exc:
        fail("arm R: cannot read the real ontology source: %s" % exc)
    token = token or "fibo:SecDbt:Bond"

    fixture = None
    sandbox = None
    if not failures:
        fixture = tempfile.mkdtemp(prefix="a20cg-fixture-")
        try:
            onto_dir = os.path.join(fixture, "ontologies", "fibo", "src")
            os.makedirs(onto_dir)
            shutil.copyfile(
                os.path.join(REPO, "ontologies/fibo/src/lib.rs"),
                os.path.join(onto_dir, "lib.rs"))
            with open(os.path.join(fixture, "Cargo.toml"), "w") as f:
                f.write('[workspace]\nmembers = ["cgap-probe", "cgap-fibo"]\n'
                        'resolver = "2"\n')
            probe = os.path.join(fixture, "cgap-probe")
            os.makedirs(os.path.join(probe, "src"))
            with open(os.path.join(probe, "Cargo.toml"), "w") as f:
                f.write('[package]\nname = "cgap-probe"\nversion = '
                        '"0.1.0"\nedition = "2021"\n')
            with open(os.path.join(probe, "src", "main.rs"), "w") as f:
                f.write('static PROBE_TOKEN: &str = "%s";\n'
                        'fn main() {\n'
                        '    if std::env::args().count() == 999 {\n'
                        '        println!("{}", PROBE_TOKEN);\n'
                        '    }\n'
                        '}\n' % token)
            emitter = os.path.join(fixture, "cgap-fibo")
            os.makedirs(os.path.join(emitter, "src"))
            with open(os.path.join(emitter, "Cargo.toml"), "w") as f:
                f.write('[package]\nname = "cgap-fibo"\nversion = "0.1.0"\n'
                        'edition = "2021"\n\n[[bin]]\nname = '
                        '"emit-fibo-pack"\npath = "src/main.rs"\n')
            with open(os.path.join(emitter, "src", "main.rs"), "w") as f:
                f.write('fn main() {}\n')
            build = subprocess.run(["cargo", "build"], cwd=fixture,
                                   capture_output=True, text=True,
                                   timeout=600)
            if build.returncode != 0:
                fail("arm R: fixture cargo build failed (rc=%d): %s"
                     % (build.returncode,
                        (build.stderr or "")[-400:]))
            if not failures:
                probe_bin = os.path.join(fixture, "target", "debug",
                                         "cgap-probe")
                if not os.path.isfile(probe_bin):
                    fail("arm R: fixture probe binary missing after build: "
                         "%s" % probe_bin)
                verdict_r = canonical_scan_verdict_at(fixture, "arm R")
                if verdict_r["verdict"] != "RED":
                    fail("arm R: canonical gate over the fixture returned "
                         "%r (expected a REAL RED); reason=%r summary=%r"
                         % (verdict_r.get("verdict"),
                            verdict_r.get("reason"),
                            verdict_r.get("summary")))
                else:
                    print("CROSS-GATE COURT arm R: REAL canonical RED "
                          "witnessed over the cargo-built fixture (exit=%s, "
                          "hits=%s) — probe token %r embedded in a real "
                          "binary" % (verdict_r["exit"],
                                      verdict_r["summary"]["hits"], token))
        except OSError as exc:
            fail("arm R: fixture construction failed: %s" % exc)

        # --- Arm F: sandbox flip under the named mutation `seam-neuter` ------
        if not failures:
            sandbox = tempfile.mkdtemp(prefix="a20cg-sandbox-")
            try:
                for entry in os.listdir(fixture):
                    if entry == "ontologies":
                        # the COVERED_CRATES clone below provides the REAL
                        # ontologies/fibo tree (incl. src/lib.rs) — the
                        # fixture's single-file copy would collide with it
                        continue
                    src_entry = os.path.join(fixture, entry)
                    dst_entry = os.path.join(sandbox, entry)
                    if os.path.isdir(src_entry):
                        shutil.copytree(
                            src_entry, dst_entry,
                            dirs_exist_ok=True,
                            ignore=shutil.ignore_patterns("incremental"))
                    else:
                        shutil.copyfile(src_entry, dst_entry)
                os.makedirs(os.path.join(sandbox, "scripts"))
                shutil.copyfile(
                    os.path.join(REPO, "scripts/boundary_gate.py"),
                    os.path.join(sandbox, "scripts/boundary_gate.py"))
                shutil.copyfile(
                    os.path.join(REPO, "scripts/zero_string_gate.py"),
                    os.path.join(sandbox, "scripts/zero_string_gate.py"))
                for crate in COVERED_CRATES:
                    shutil.copytree(
                        os.path.join(REPO, crate),
                        os.path.join(sandbox, crate),
                        ignore=shutil.ignore_patterns(
                            "target*", "*.rlib", "*.rmeta", "*.d"))
                if not os.path.isfile(
                        os.path.join(sandbox, "ontologies/fibo/src/lib.rs")):
                    fail("arm F: sandbox vocab source missing after the "
                         "crate clone (ontologies/fibo/src/lib.rs)")
                gate_src = open(
                    os.path.join(sandbox, "scripts/boundary_gate.py"),
                    encoding="utf-8").read()
                # Anchored to the call-site line form (newline + exactly the
                # 4-space-indented call) so the anchor literal inside THIS
                # court's own source cannot double-match.
                anchor = "\n    canon = a07_canonical_scan()\n"
                if gate_src.count(anchor) != 1:
                    fail("arm F: seam anchor count=%d (expected 1) — the "
                         "sandbox copy of this gate is not the audited "
                         "shape" % gate_src.count(anchor))
                else:
                    un_neutered = subprocess.run(
                        [sys.executable,
                         os.path.join(sandbox, "scripts/boundary_gate.py"),
                         "--strings"],
                        cwd=sandbox, capture_output=True, text=True,
                        timeout=600)
                    if un_neutered.returncode != EXIT_REFUSE:
                        fail("arm F (before): un-neutered sandbox --strings "
                             "exited %s over a canonically-red sandbox "
                             "(expected %s); stderr tail: %s"
                             % (un_neutered.returncode, EXIT_REFUSE,
                                (un_neutered.stderr or "")[-300:]))
                    else:
                        print("CROSS-GATE COURT arm F (before): un-neutered "
                              "sandbox --strings refused on the canonical "
                              "red (exit %s) — seam carries the verdict"
                              % un_neutered.returncode)
                    neutered = gate_src.replace(
                        anchor,
                        '\n    canon = {"verdict": "GREEN", "exit": '
                        '"NEUTERED", "summary": {}, '
                        '"neutered_by": "seam-neuter (court mutation)"}'
                        '  # SEAM-NEUTER (court mutation)\n')
                    with open(
                            os.path.join(sandbox, "scripts/boundary_gate.py"),
                            "w", encoding="utf-8") as f:
                        f.write(neutered)
                    after = subprocess.run(
                        [sys.executable,
                         os.path.join(sandbox, "scripts/boundary_gate.py"),
                         "--strings"],
                        cwd=sandbox, capture_output=True, text=True,
                        timeout=600)
                    if after.returncode != EXIT_PASS:
                        fail("arm F (after): seam-neutered sandbox "
                             "--strings exited %s (expected %s — the flip "
                             "that proves the seam is load-bearing); "
                             "stderr tail: %s"
                             % (after.returncode, EXIT_PASS,
                                (after.stderr or "")[-300:]))
                    else:
                        print("CROSS-GATE COURT arm F (after): seam-neutered "
                              "--strings returned to exit 0 — named "
                              "mutation `seam-neuter` FLIPS the gate; the "
                              "seam is load-bearing, court non-vacuous")
            except OSError as exc:
                fail("arm F: sandbox construction failed: %s" % exc)

    # --- cleanup (minter law): delete what this court minted -----------------
    for label, path in (("fixture", fixture), ("sandbox", sandbox)):
        if path and os.path.isdir(path):
            try:
                shutil.rmtree(path)
                print("CROSS-GATE COURT cleanup: %s removed (%s)"
                      % (label, path))
            except OSError as exc:
                print("CROSS-GATE COURT CLEANUP LEFTOVER (loud, not silent): "
                      "%s at %s: %s" % (label, path, exc), file=sys.stderr)

    if failures:
        print("CROSS-GATE COURT: %d arm failure(s) — the seam court REFUSES"
              % len(failures), file=sys.stderr)
        return EXIT_REFUSE
    print("CROSS-GATE COURT: G green end-to-end, R real-red witnessed, "
          "F seam-neuter flip witnessed — seam court PASSES")
    return EXIT_PASS


# --- Wave 8 interference court (§6.1 step 5 write path) ----------------------

# The court's anti-vacuity anchor: the one comparison line in
# verify_unmutated. The gate's OWN cmp-swap operator on this line flips
# `==` -> `!=`, inverting the check (refuses quiet, passes foreign) —
# witnessed on temp copies so the canonical tree is never written.
INTERFERENCE_ANCHOR = "if current_sha == expected_sha:"


def mode_interference_court(report=None):
    """Wave 8 anti-vacuity court for the pre-write interference check
    (verify_unmutated). Three real arms, every mutation on temp copies:

    G: the REAL court kernel file is unmutated right now (read-only sha
       check) and the intact check passes it (returns None).
    F: on a temp copy, a REAL foreign write (one flipped byte) is DETECTED
       by the intact check (typed reason, non-None) — the detector fires.
    V: the gate's OWN cmp-swap operator applied to the check's comparison
       line (`==` -> `!=`) INVERTS the check: on a quiet temp copy the
       neutered check refuses, and on a foreign-written temp copy the
       neutered check passes. Both inversions witnessed = the court's
       detector genuinely distinguishes intact-vs-neutered = the court can
       flip; a check whose refusal is unobservable cannot be tested, so an
       un-invertible check fails this court.
    """
    import shutil
    import tempfile
    failures = []
    target_rel = MUTABLE_TARGETS[0]
    target_abs = os.path.join(REPO, target_rel)
    gate_src = read_bytes(os.path.abspath(__file__))

    # --- Arm G: real file quiet + intact check passes -------------------
    real_sha = sha256_bytes(read_bytes(target_abs))
    if verify_unmutated(target_abs, real_sha) is not None:
        failures.append("G: the real court kernel failed the intact check "
                        "while quiet (check false-positives)")

    # --- Arm F: real detection on a temp copy ---------------------------
    # --- Arm V: the court's own inversion witness ------------------------
    with tempfile.TemporaryDirectory() as tmp:
        copy_path = os.path.join(tmp, "court_verdict.rs")
        base = read_bytes(target_abs)
        write_bytes(copy_path, base)
        copy_sha = sha256_bytes(base)

        intact_reason_quiet = verify_unmutated(copy_path, copy_sha)
        if intact_reason_quiet is not None:
            failures.append("F/V: the intact check refuses a quiet copy "
                            "(check broken: %s)" % intact_reason_quiet)

        foreign = base[:-1] + bytes([base[-1] ^ 0x01])
        write_bytes(copy_path, foreign)
        foreign_sha = sha256_bytes(foreign)
        intact_reason_foreign = verify_unmutated(copy_path, copy_sha)
        if intact_reason_foreign is None:
            failures.append("F: the intact check did NOT detect the real "
                            "foreign write (detector dead)")
        if foreign_sha == copy_sha:
            failures.append("F: the foreign write did not change the file "
                            "(court fixture broken)")

        # Arm V: neuter via the gate's own cmp-swap on the anchor line.
        anchor_lines = [i for i, l in enumerate(gate_src.decode().split("\n"), 1)
                        if l.strip().startswith("if current_sha ==")
                        and INTERFERENCE_ANCHOR in l]
        if len(anchor_lines) != 1:
            failures.append("V: anchor %r matches %d lines in the gate "
                            "(need exactly 1)" % (INTERFERENCE_ANCHOR,
                                                  len(anchor_lines)))
        else:
            neutered_path = os.path.join(tmp, "bg_neutered.py")
            g_lines = gate_src.decode().split("\n")
            flipped = mutate_line(g_lines[anchor_lines[0] - 1], "cmp-swap", 0)
            if flipped is None or INTERFERENCE_ANCHOR.replace("==", "!=") \
                    not in flipped:
                failures.append("V: cmp-swap did not invert the anchor "
                                "line (court machinery drifted)")
            else:
                g_lines[anchor_lines[0] - 1] = flipped
                write_bytes(neutered_path,
                            "\n".join(g_lines).encode())
                import importlib.util
                nspec = importlib.util.spec_from_file_location(
                    "bg_neutered", neutered_path)
                nmod = importlib.util.module_from_spec(nspec)
                try:
                    nspec.loader.exec_module(nmod)
                except Exception as exc:  # noqa: BLE001
                    failures.append("V: neutered module failed to load: %s"
                                    % exc)
                    nmod = None
                if nmod is not None:
                    ncheck = getattr(nmod, "verify_unmutated", None)
                    if ncheck is None:
                        failures.append("V: neutered module lost "
                                        "verify_unmutated")
                    else:
                        # Quiet state: base content, snapshot hash.
                        write_bytes(copy_path, base)
                        n_quiet = ncheck(copy_path, copy_sha)
                        if n_quiet is None:
                            failures.append(
                                "V: neutered check PASSES a quiet copy "
                                "(inversion not witnessed — the court "
                                "detector cannot distinguish intact from "
                                "neutered)")
                        # Foreign state: foreign content, same snapshot
                        # hash — exactly the real gate's pre-write view.
                        write_bytes(copy_path, foreign)
                        n_foreign = ncheck(copy_path, copy_sha)
                        if n_foreign is not None:
                            failures.append(
                                "V: neutered check REFUSES the foreign "
                                "copy (inversion not witnessed — the "
                                "court detector cannot distinguish intact "
                                "from neutered)")
        write_bytes(copy_path, base)
        if sha256_bytes(read_bytes(copy_path)) != copy_sha:
            failures.append("temp restore mismatch (fail-closed)")
        shutil.rmtree(tmp, ignore_errors=True)
        if os.path.exists(tmp):
            print("INTERFERENCE COURT CLEANUP LEFTOVER: %s (loud, not "
                  "silent)" % tmp, file=sys.stderr)
    if failures:
        for f in failures:
            print("INTERFERENCE COURT FAIL: %s" % f, file=sys.stderr)
        print("INTERFERENCE COURT: %d failure(s) — the interference court "
              "REFUSES (§6.3 discipline: a court that cannot flip is FAIL)"
              % len(failures), file=sys.stderr)
        return EXIT_REFUSE
    print("INTERFERENCE COURT: G real-file quiet, F real detection, "
          "V cmp-swap inversion witnessed — interference court PASSES")
    return EXIT_PASS


def parse_targets(spec):
    """--targets csv -> validated repo-relative target list. Fail-closed:
    absolute paths, `..` escapes, missing files, duplicates all refuse."""
    if not spec or not spec.strip():
        return list(MUTABLE_TARGETS)
    targets = [t.strip() for t in spec.split(",") if t.strip()]
    for t in targets:
        norm = os.path.normpath(t)
        if os.path.isabs(norm) or norm.startswith(".."):
            print("BAD TARGET (must stay inside the repo, repo-relative): %s"
                  % t, file=sys.stderr)
            sys.exit(EXIT_REFUSE)
        if not os.path.isfile(os.path.join(REPO, norm)):
            print("TARGET MISSING: %s" % t, file=sys.stderr)
            sys.exit(EXIT_REFUSE)
    if len(set(targets)) != len(targets):
        print("DUPLICATE TARGET in --targets", file=sys.stderr)
        sys.exit(EXIT_REFUSE)
    return targets


def main():
    ap = argparse.ArgumentParser(description="A20 boundary gate (Wave 4)")
    ap.add_argument("--mutate", action="store_true", help="§6.1 full campaign")
    ap.add_argument("--shim-court-only", action="store_true",
                    help="§6.3 non-vacuity check only")
    ap.add_argument("--shim-inert", action="store_true",
                    help="negative control: plant an INERT shim; the gate MUST "
                         "refuse VACUOUS GATE (exit 1). A pass here means the "
                         "gate itself is broken.")
    ap.add_argument("--cross-gate-court", action="store_true",
                    help="Wave 7 seam court (a07-wave6 open item 4): three "
                         "real arms — green end-to-end at the repo root, a "
                         "REAL cargo-built fixture probe -> canonical RED, "
                         "and the named mutation `seam-neuter` must flip the "
                         "gate in a sandbox (un-neutered refuses, neutered "
                         "passes). A --strings that stays green under "
                         "seam-neuter is a vacuous seam; the court exits 1.")
    ap.add_argument("--interference-court", action="store_true",
                    help="Wave 8 anti-vacuity court for the pre-write "
                         "interference check (verify_unmutated): three real "
                         "arms — the real court kernel passes quiet (G), a "
                         "real foreign write is detected (F), and the gate's "
                         "own cmp-swap operator inverts the check on temp "
                         "copies (V: neutered refuses quiet, passes "
                         "foreign). All mutations on temp copies; the "
                         "canonical tree is never written.")
    ap.add_argument("--deps", action="store_true", help="§6.4 dep gate")
    ap.add_argument("--strings", action="store_true",
                    help="§6.4 string gate")
    ap.add_argument("--report", default=None, help="ledger JSON output path")
    ap.add_argument("--max-mutants", type=int, default=DEFAULT_MAX_MUTANTS,
                    help="stride-sampled campaign size (recorded in ledger)")
    ap.add_argument("--equivalent", action="append", default=[],
                    help="triage a survivor: file:line:op:index=justification")
    ap.add_argument("--target-dir", default=None,
                    help="CARGO_TARGET_DIR for gate cargo runs (lane lease)")
    ap.add_argument("--targets", default=None,
                    help="Wave 4 (§6.1): comma-separated repo-relative "
                         "mutable sources; default is the A20 court kernel "
                         "only. Kernel targets transiently write other "
                         "lanes' files (sha256-fail-closed restore) — use a "
                         "sandbox or a quiet seal window. The set must "
                         "include the court kernel (it hosts the §6.3 shim).")
    ap.add_argument("--only-op", default=None,
                    help="Wave 4: comma-separated operator names; restrict "
                         "the sampled corpus to these operators (recorded "
                         "in the ledger, never a silent filter)")
    args = ap.parse_args()

    if args.mutate or args.shim_court_only:
        # Wave 9 (a20-wave9 audit D2): gate-vs-gate interference PREVENTION.
        # These modes write target files in place; two concurrent campaigns
        # on one shared checkout must refuse, never interleave mutants (the
        # witnessed D2 incident had a live mutant racing another session's
        # court run). Detection (Wave-8 verify_unmutated) + prevention (this
        # lock) close the class. Held for the process lifetime; advisory
        # flock releases on process death, so no stale wedge.
        acquire_campaign_lock()

    if args.deps or args.strings:
        which = []
        if args.deps:
            which.append("deps")
        if args.strings:
            which.append("strings")
        sys.exit(mode_deps_strings(which, args.report))

    if args.cross_gate_court:
        sys.exit(mode_cross_gate_court(args.report))

    if args.interference_court:
        sys.exit(mode_interference_court(args.report))

    if not (args.mutate or args.shim_court_only):
        print("no mode: one of %s required" % ", ".join(CLI_MODES),
              file=sys.stderr)
        return EXIT_REFUSE

    target_list = parse_targets(args.targets)
    states = {}
    for t in target_list:
        path = os.path.join(REPO, t)
        data = read_bytes(path)
        states[t] = {
            "path": path,
            "baseline": data,
            "sha": sha256_bytes(data),
            "lines": data.decode().split("\n"),
        }

    only_ops = None
    if args.only_op:
        only_ops = [o.strip() for o in args.only_op.split(",") if o.strip()]
        unknown = [o for o in only_ops if o not in OPERATORS]
        if unknown:
            print("BAD --only-op operator(s): %s (known: %s)"
                  % (",".join(unknown), ",".join(OPERATORS)), file=sys.stderr)
            return EXIT_REFUSE

    ledger = {
        "abi_version": VERDICT_ABI_VERSION,
        "config": {
            "court_args": COURT_ARGS,
            "env_override": COURT_ENV_OVERRIDE,
            "targets": target_list,
            "kernel_target_candidates": list(KERNEL_TARGET_CANDIDATES),
            "operators": list(OPERATORS),
            "operators_deferred": OPERATORS_DEFERRED,
            "only_op": only_ops,
            "equivalent_triage_register": dict(EQUIVALENT_TRIAGE),
            "kill_rate_min": KILL_RATE_MIN,
            "interference_check": (
                "Wave-8 interference verification: pre-write sha256 "
                "re-verification before every target write AND a "
                "pre-restore check that the disk still holds exactly the "
                "mutant bytes the gate wrote (a foreign write racing the "
                "restore window is otherwise silently clobbered — "
                "witnessed live), plus a readback-verified restore. A "
                "concurrent writer becomes a typed InterferenceRefusal "
                "(exit 1), never a silent mutate-over-foreign-content "
                "run"),
            "campaign_lock": (
                "Wave-9 interference prevention (a20-wave9 audit D2): "
                "exclusive non-blocking flock on %s held for the campaign "
                "process lifetime; a second concurrent campaign on this "
                "checkout refuses typed (CAMPAIGN LOCK HELD, exit 1). "
                "Kernel-released on process death — no stale wedge."
                % CAMPAIGN_LOCK_PATH),
            "shim": {
                "anchor": SHIM_ANCHOR,
                "marker": SHIM_MARKER,
                "replacement": SHIM_REPLACEMENT,
            },
        },
        "rows": [],
        "skips": [],
    }

    # §10 loud skips: mechanically verified absences, never silent.
    a05_src = os.path.join(REPO, "crates/cgap-storage/src/atomic_commit.rs")
    with open(a05_src) as f:
        a05_body = f.read()
    if not re.search(r"^\s*pub fn ", a05_body, re.M):
        ledger["skips"].append({
            "verdict": "SkipLoud",
            "what": "A05 tear-classification step (§5.2)",
            "reason": "cgap_storage::atomic_commit declares no pub fn this "
                      "session (Wave-1 ABI freeze); classification consumed "
                      "at Wave 4",
        })

    def run():
        return run_courts(args.target_dir)

    # §6.1 step 2: baseline run — a red baseline aborts the gate. One run;
    # the courts are target-independent. The row's file field anchors on
    # the first target.
    first_state = states[target_list[0]]
    rc, output, timed_out = run()
    verdict, killed_by, passed = classify_run(rc, output, timed_out)
    if verdict != "Baseline":
        print("RED BASELINE: courts fail unmutated (rc=%s, killed_by=%s); "
              "no mutation campaign over a red baseline" % (rc, killed_by),
              file=sys.stderr)
        ledger["rows"].append(
            ledger_row(first_state["path"], 0, "baseline", 0, "Baseline",
                       killed_by, passed, first_state["sha"], None))
        write_report(ledger, args.report)
        return EXIT_REFUSE
    ledger["config"]["baseline_sha256"] = {
        t: states[t]["sha"] for t in target_list
    }
    ledger["rows"].append(
        ledger_row(first_state["path"], 0, "baseline", 0, "Baseline", "",
                   passed, first_state["sha"], None))

    # §6.3: shim-court non-vacuity — always first, before any campaign.
    # The committed shim anchors in the A20 court kernel by design; the
    # host is PINNED (SHIM_HOST), never textually searched: the anchor
    # constant also occurs in other covered sources (witnessed: penstock.rs
    # carries the same SplitMix64 mix constant).
    shim_t = SHIM_HOST if SHIM_HOST in target_list else None
    if shim_t is None:
        print("SHIM HOST ABSENT: the §6.3 committed shim lives in %s; a "
              "--targets campaign must include it — non-vacuity is proven "
              "on the court kernel, never assumed" % SHIM_HOST,
              file=sys.stderr)
        write_report(ledger, args.report)
        return EXIT_REFUSE
    if plant_shim(states[shim_t]["lines"])[0] is None:
        print("SHIM SITE NOT FOUND: %s in %s" % (SHIM_ANCHOR, SHIM_HOST),
              file=sys.stderr)
        write_report(ledger, args.report)
        return EXIT_REFUSE
    ledger["config"]["shim"]["target"] = shim_t

    killed, shim_row, _ = shim_court(states[shim_t], run,
                                     inert=args.shim_inert, ledger=ledger,
                                     report=args.report)
    ledger["rows"].append(shim_row)
    # §9.2 receipt pointer (Wave 5): the report's non-vacuity kill is
    # carried mechanically, not by row-order convention — the ShimCourt
    # row sits at index 1 (§6.1 loop order: the red-baseline row precedes
    # the shim), before every campaign row.
    ledger["non_vacuity"] = {
        "row_index": len(ledger["rows"]) - 1,
        "verdict": shim_row["verdict"],
        "killed_by": shim_row["killed_by"],
    }
    if not killed:
        what = ("the INERT control was not killed (expected refusal — "
                "negative control behaved)" if args.shim_inert else
                "the committed shim mutant was NOT killed")
        print("VACUOUS GATE: %s; every pass this court ever produced is "
              "void (§6.3)" % what, file=sys.stderr)
        write_report(ledger, args.report)
        return EXIT_REFUSE
    if args.shim_inert:
        print("GATE BROKEN: the INERT shim produced a kill — the gate's "
              "runner/classifier is unsound; refusing (negative control "
              "failed)", file=sys.stderr)
        write_report(ledger, args.report)
        return EXIT_REFUSE
    if args.shim_court_only:
        print("SHIM-COURT: killed by %s — gate is non-vacuous (§6.3)"
              % shim_row["killed_by"])
        write_report(ledger, args.report)
        return EXIT_PASS

    # §6.1 steps 3-5: campaign over the combined per-target mutant corpus
    # (targets in --targets order, lines ascending within each).
    combined = []
    per_target_generated = {}
    for t in target_list:
        ms = enumerate_mutants(states[t]["lines"])
        per_target_generated[t] = len(ms)
        combined.extend((t, ln, op, occ) for (ln, op, occ) in ms)
    total_before_only_op = len(combined)
    if only_ops:
        opset = set(only_ops)
        combined = [m for m in combined if m[2] in opset]
    total = len(combined)
    if total > args.max_mutants:
        stride = -(-total // args.max_mutants)  # ceil division
        sampled = combined[::stride]
        ledger["config"]["sampling"] = {
            "total_generated": total,
            "total_before_only_op": total_before_only_op,
            "per_target_generated": per_target_generated,
            "selected": len(sampled),
            "stride": stride,
            "disclosure": "§8: stride-sampled above --max-mutants; recorded, "
                          "never hidden",
        }
    else:
        sampled = combined
        ledger["config"]["sampling"] = {
            "total_generated": total,
            "total_before_only_op": total_before_only_op,
            "per_target_generated": per_target_generated,
            "selected": len(sampled),
            "stride": 1,
            "disclosure": "full corpus run",
        }

    killed_count = 0
    survivors = []
    for (t, line_no, op, occ) in sampled:
        st = states[t]
        mutated_line = mutate_line(st["lines"][line_no - 1], op, occ)
        if mutated_line is None:
            continue
        # Wave 8: pre-write interference check before EVERY mutant write
        # (same refusal contract as the shim path above; the witnessed case
        # is in verify_unmutated's docstring).
        reason = verify_unmutated(st["path"], st["sha"])
        if reason:
            print("GATE REFUSED: %s" % reason, file=sys.stderr)
            ledger["rows"].append(ledger_row(
                st["path"], line_no, op, occ, "InterferenceRefusal",
                "foreign-write", [], sha256_bytes(read_bytes(st["path"])),
                None))
            ledger["config"]["interference_refusal"] = reason
            write_report(ledger, args.report)
            return EXIT_REFUSE
        parts = st["baseline"].decode().split("\n")
        parts[line_no - 1] = mutated_line
        mutated = "\n".join(parts).encode()
        mutant_sha = sha256_bytes(mutated)
        write_bytes(st["path"], mutated)
        rc, output, timed_out = run()
        verdict, killed_by, passed = classify_run(rc, output, timed_out)
        # Wave 8: pre-restore interference check (same witnessed race as
        # the shim path above) — restoring would clobber foreign content.
        race = verify_unmutated(st["path"], mutant_sha)
        if race:
            print("GATE REFUSED: %s" % race, file=sys.stderr)
            ledger["rows"].append(ledger_row(
                st["path"], line_no, op, occ, "InterferenceRefusal",
                "restore-race", [], sha256_bytes(read_bytes(st["path"])),
                None))
            ledger["config"]["interference_refusal"] = race
            write_report(ledger, args.report)
            return EXIT_REFUSE
        restore_or_die(st["path"], st["baseline"], st["sha"], ledger,
                       report=args.report)
        if verdict == "Baseline":
            survivors.append((line_no, op, occ, passed))
            row = ledger_row(st["path"], line_no, op, occ, "SurvivorDefect",
                             "", passed, mutant_sha, None)
        else:
            killed_count += 1
            row = ledger_row(st["path"], line_no, op, occ, verdict, killed_by,
                             passed, mutant_sha, None)
        ledger["rows"].append(row)

    # §6.2 triage: the committed EQUIVALENT_TRIAGE register (standing,
    # reviewed record) seeds the triage; --equivalent CLI specs merge over
    # it (CLI wins for one run). The unmatched-key refusal below covers
    # register keys too — a stale register key refuses the gate instead
    # of silently passing (drift law: refresh from the run's ledger).
    triaged = {}
    for key, justification in EQUIVALENT_TRIAGE.items():
        parts_reg = key.split(":")
        if len(parts_reg) != 4:
            print("BAD EQUIVALENT_TRIAGE key (need file:line:op:index): %s"
                  % key, file=sys.stderr)
            return EXIT_REFUSE
        rf, rl, ro, ri = parts_reg
        triaged[(rf, int(rl), ro, int(ri))] = justification
    for spec in args.equivalent:
        if "=" not in spec:
            print("BAD --equivalent spec (need file:line:op:index=justification):"
                  " %s" % spec, file=sys.stderr)
            return EXIT_REFUSE
        key, justification = spec.split("=", 1)
        parts_spec = key.split(":")
        if len(parts_spec) != 4:
            print("BAD --equivalent key: %s" % spec, file=sys.stderr)
            return EXIT_REFUSE
        f, l, o, i = parts_spec
        triaged[(f, int(l), o, int(i))] = justification

    equivalent_count = 0
    defect_rows = []
    used_keys = set()
    for row in ledger["rows"]:
        if row["verdict"] != "SurvivorDefect":
            continue
        mid = row["mutant_id"]
        key = (mid["file"], mid["line"], mid["operator"], mid["index"])
        rel = os.path.relpath(mid["file"], REPO)
        relkey = (rel, mid["line"], mid["operator"], mid["index"])
        if key in triaged or relkey in triaged:
            row["verdict"] = "Equivalent"
            row["equivalent_justification"] = triaged.get(key) or triaged[relkey]
            used_keys.add(key)
            used_keys.add(relkey)
            equivalent_count += 1
        else:
            defect_rows.append(row)
    # Fail-closed triage (Wave 4): a --equivalent key matching no survivor
    # row is a stale or typo'd triage (line drift, wrong operator) — refuse
    # loudly, never silently ignore it. Without this, a drifted key would
    # silently shrink the applied triage instead of failing the gate.
    unmatched = sorted(k for k in triaged if k not in used_keys)
    if unmatched:
        print("BAD --equivalent / EQUIVALENT_TRIAGE: %d key(s) matched no "
              "survivor row (stale line/op drift?):" % len(unmatched),
              file=sys.stderr)
        for k in unmatched:
            where = k[0] if not os.path.isabs(k[0]) else os.path.relpath(k[0], REPO)
            print("  %s:%d:%s:%d" % (where, k[1], k[2], k[3]), file=sys.stderr)
        current = [r for r in ledger["rows"]
                   if r["verdict"] == "SurvivorDefect"]
        if current:
            print("current survivor sites (refresh the register from these, "
                  "never widen it to force a pass):", file=sys.stderr)
            for row in current:
                mid = row["mutant_id"]
                print("  %s:%d:%s:%d"
                      % (os.path.relpath(mid["file"], REPO), mid["line"],
                         mid["operator"], mid["index"]), file=sys.stderr)
        write_report(ledger, args.report)
        return EXIT_REFUSE

    # §6.2 kill rate over NON_EQUIVALENT mutants of the campaign corpus (the
    # stride-sampled corpus when sampling is active — disclosed in
    # ledger["config"]["sampling"], never hidden).
    denominator = len(sampled) - equivalent_count
    kill_rate = (killed_count / denominator) if denominator else 0.0
    for row in ledger["rows"]:
        row["kill_rate"] = round(kill_rate, 4)

    ledger["kill_rate"] = round(kill_rate, 4)
    ledger["summary"] = {
        "total_generated": total,
        "total_before_only_op": total_before_only_op,
        "sampled": len(sampled),
        "killed": killed_count,
        "equivalent": equivalent_count,
        "survivor_defects": len(defect_rows),
    }

    write_report(ledger, args.report)

    if defect_rows:
        print("SURVIVOR DEFECTS: %d untriaged survivor(s) — assertion gaps "
              "against the owning lane (§6.2):" % len(defect_rows),
              file=sys.stderr)
        for row in defect_rows:
            mid = row["mutant_id"]
            print("  %s:%d %s#%d (passed: %s)"
                  % (os.path.relpath(mid["file"], REPO), mid["line"],
                     mid["operator"], mid["index"],
                     ",".join(row["passing_courts"])), file=sys.stderr)
        return EXIT_REFUSE
    if kill_rate < KILL_RATE_MIN:
        print("KILL RATE %.4f < %.2f (§6.2)" % (kill_rate, KILL_RATE_MIN),
              file=sys.stderr)
        return EXIT_REFUSE

    print("MUTATION CAMPAIGN: %d/%d killed (%.4f >= %.2f), %d equivalent, "
          "0 survivor defects — gate PASSES (§6.2)"
          % (killed_count, denominator, kill_rate, KILL_RATE_MIN,
             equivalent_count))
    return EXIT_PASS


if __name__ == "__main__":
    raise SystemExit(main())
