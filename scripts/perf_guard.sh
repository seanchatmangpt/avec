#!/usr/bin/env bash
# perf_guard.sh — wave-7 lane 14. Fail-closed CTQ-05 regression canary.
#
# Purpose: prevent wave-7 kernel work from silently regressing CTQ-05
# (end-to-end admission pipeline latency).
#
# IMPORTANT — advisory ceiling, NOT the spec gate:
#   The CTQ-05 PRD spec USL is 35 µs, enforced by `target/release/ctq_report`
#   itself (exit 1 on >35 µs). This guard applies a LOOSER advisory ceiling of
#   30 µs as an early-warning canary: a value between 30 and 35 µs still passes
#   the spec gate but trips this canary, signaling drift toward the USL before
#   the spec gate fires. This guard supplements, never replaces, ctq_report's
#   own REAL gate.
#
# Load-awareness (DMADV Measure discipline): a timing gate run on a noisy
# machine measures the machine, not the kernel. Before measuring, run
# `gage_rr --reps 2000`; if its trimmed coefficient of variation exceeds 2%,
# the environment is too noisy to trust any timing verdict — exit 2
# SKIP-NOISY with the CV recorded. Never fail on noise; never pass on noise
# either.
#
# Exit codes:
#   0  PASS      — machine quiet, CTQ-05 <= 30 µs
#   1  FAIL      — build broke, gage/ctq tool crashed/misparse, or CTQ-05 > 30 µs
#   2  SKIP-NOISY — gage CV > 2% (machine noisy; no timing verdict issued)
#
# Scope: the verdict covers CTQ-05 only. ctq_report failures confined to
# other gates (CTQ-02/04/06) are reported loudly on stdout but do not flip
# this guard's verdict — ctq_report remains the authority for those gates.
#
# Receipt: mint with scripts/mint_receipt.sh (exit 2 propagates as FAILED in
# the receipt but the receipt text records SKIP-NOISY explicitly).
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
CEILING_US=30.0
NOISE_CV_PCT=2.0
cd "$REPO"

log() { echo "[perf_guard] $*" >&2; }

die() { log "FAIL: $*"; exit 1; }

# ------------------------------------------------------- 1. build --------
log "building release (cgap-blake3, cgap-integration, bench)..."
cargo build --release -p cgap-blake3 -p cgap-integration -p bench \
  || die "release build failed (BUILD-BROKEN)"

CTQ_BIN="$REPO/target/release/ctq_report"
GAGE_BIN="$REPO/target/release/gage_rr"
[[ -x "$CTQ_BIN" ]] || die "missing $CTQ_BIN"
[[ -x "$GAGE_BIN" ]] || die "missing $GAGE_BIN"

# --------------------------------------------- 2. gage (noise gate) ------
log "gage R&R probe (reps=2000, noise CV threshold ${NOISE_CV_PCT}%)..."
GAGE_OUT="$(mktemp)"
GAGE_OUT_ERR="$(mktemp)"
CTQ_OUT=""
CTQ_OUT_ERR="$(mktemp)"
trap 'rm -f "$GAGE_OUT" "$GAGE_OUT_ERR" "$CTQ_OUT" "$CTQ_OUT_ERR" 2>/dev/null || true' EXIT
set +e
"$GAGE_BIN" --reps 2000 >"$GAGE_OUT" 2>"$GAGE_OUT_ERR"
GAGE_EXIT=$?
set -e

CV="$(python3 - "$GAGE_OUT" <<'PY'
import json, sys
try:
    print(json.load(open(sys.argv[1]))["cv_trimmed_pct"])
except Exception as e:
    print(f"PARSE_ERROR:{e}")
PY
)"
if [[ "$CV" == PARSE_ERROR* ]]; then
  die "gage output unparseable ($CV); gage exit=$GAGE_EXIT"
fi
log "gage cv_trimmed=${CV}% (gage_pass_exit=$GAGE_EXIT)"

# A gage crash (not the expected fail-on-noisy exit 1) is a tool failure.
if [[ "$GAGE_EXIT" -ne 0 && "$GAGE_EXIT" -ne 1 ]]; then
  die "gage_rr crashed (exit $GAGE_EXIT)"
fi

AWK_CHECK="$(awk -v cv="$CV" -v t="$NOISE_CV_PCT" 'BEGIN{print (cv+0 > t) ? "NOISY" : "QUIET"}')"
if [[ "$AWK_CHECK" == "NOISY" ]]; then
  log "SKIP-NOISY: gage cv_trimmed=${CV}% > ${NOISE_CV_PCT}% — machine too noisy for a timing verdict"
  echo "SKIP-NOISY cv_trimmed_pct=${CV}"
  exit 2
fi

# ------------------------------------- 3. ctq_report + CTQ-05 canary -----
# Scope note: this guard's verdict covers CTQ-05 only. ctq_report also gates
# CTQ-02/04/06; those belong to other lanes/workstreams. A ctq_report failure
# confined to other gates is reported loudly (WARNING + failed-gate list on
# stdout) but does not flip THIS guard's verdict — the authoritative CTQ-05
# verdict here is the extracted value vs the 30 µs canary.
CTQ_OUT="$(mktemp)"
log "running ctq_report..."
set +e
"$CTQ_BIN" >"$CTQ_OUT" 2>"$CTQ_OUT_ERR"
CTQ_EXIT=$?
set -e

read -r CTQ05 FAILED_GATES <<<"$(python3 - "$CTQ_OUT" <<'PY'
import json, sys
try:
    d = json.load(open(sys.argv[1]))
    c5 = d["ctq05_e2e_admission"]
    if c5.get("metric") != "us_per_batch":
        raise ValueError(f"unexpected metric {c5.get('metric')!r}")
    print(c5["value"], ",".join(d.get("failed_gates", [])) or "-")
except Exception as e:
    print(f"PARSE_ERROR:{e}")
PY
)"
if [[ "${CTQ05:-}" == PARSE_ERROR* || -z "${CTQ05:-}" ]]; then
  die "ctq_report output unparseable (${CTQ05:-empty}); exit=$CTQ_EXIT"
fi
log "ctq_report exit=$CTQ_EXIT CTQ-05=${CTQ05} us (canary ceiling ${CEILING_US} us; spec USL 35 us)"

if [[ "$CTQ_EXIT" -ne 0 ]]; then
  if [[ ",$FAILED_GATES," == *",CTQ-05,"* ]]; then
    die "ctq_report REAL gate CTQ-05 FAILED (spec USL 35 us exceeded)"
  fi
  log "WARNING: ctq_report exit=$CTQ_EXIT from non-CTQ-05 gates [$FAILED_GATES] — out of this guard's scope; ctq_report remains the authority for those gates"
  echo "PERF-GUARD-OTHER-GATES-FAILED [$FAILED_GATES] (informational; not this guard's verdict)"
fi

OVER="$(awk -v v="$CTQ05" -v c="$CEILING_US" 'BEGIN{print (v+0 > c) ? "OVER" : "OK"}')"
if [[ "$OVER" == "OVER" ]]; then
  log "FAIL: CTQ-05=${CTQ05} us exceeds advisory canary ceiling ${CEILING_US} us"
  echo "PERF-GUARD-FAIL ctq05_us=${CTQ05} ceiling_us=${CEILING_US}"
  exit 1
fi

log "PASS: CTQ-05=${CTQ05} us <= ${CEILING_US} us (spec USL 35 us unchanged)"
echo "PERF-GUARD-PASS ctq05_us=${CTQ05} ceiling_us=${CEILING_US} gage_cv_pct=${CV}"
exit 0
