#!/usr/bin/env bash
#
# nightly_perf.sh — advisory nightly perf-log runner (avec v26.10.31).
#
# Lane provenance: wave-10 lane 14 (DX/QoL). Advisory only — never gate-wired,
# never fails on noise:
#   * gage CV > 2%            -> SKIP-NOISY log, exit 0
#   * gage unrunnable         -> SKIP-GAGE-FAILED log, exit 0
#   * any bench step fails    -> recorded in the log, run continues, exit 0
#   * log dir not creatable   -> exit 1 (the only fatal case)
#
# Flow (per wave-10 lane-14 dispatch):
#   1. gage precondition  : target/release/gage_rr --reps 2000; cv_trimmed > 2% -> SKIP-NOISY
#   2. ctq_report         : JSON captured verbatim
#   3. blake3_throughput  : GiB/s (report-only; no --gate)
#   4. parquet_throughput : PAYLOAD-SUM + ACCESSOR-SCAN
#   5. perf_baseline      : cargo test -p avec-git --release --test perf_baseline \
#                           -- --ignored --nocapture  (pack MiB/s + lookups/s)
#   6. log                : docs/jira/v26.10.31/perf/perf-YYYYMMDD-HHMM.md
#   7. concurrent-lane conditions recorded (uptime start/end + observed rustc/cargo count)
#
# Log format contract (parsed by this script on later runs):
#   The "## Metrics" section holds a ```text fenced block of `key = value`
#   lines. On every run after the first, DELTAS compares keys present in BOTH
#   the current and the newest prior perf-*.md log; |Δ| > 10% is flagged.
#   First run in a fresh perf/ dir is the baseline (no DELTAS comparison).
#
# Usage: scripts/nightly_perf.sh [--no-build]
#   --no-build : never invoke cargo build (assume binaries already present)

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$ROOT/docs/jira/v26.10.31/perf"
GAGE_CV_MAX_PCT="${NIGHTLY_PERF_CV_MAX:-2.0}" # lane precondition; the gage's own gate stays 1% upstream. Env override exists only to exercise the skip path in tests.
FLAG_PCT="10"         # DELTAS flag threshold, percent

mkdir -p "$LOG_DIR" || {
	echo "FATAL: cannot create $LOG_DIR" >&2
	exit 1
}

TS="$(date +%Y%m%d-%H%M)"
STAMP="$(date '+%Y-%m-%d %H:%M:%S %Z')"
OUT="$LOG_DIR/perf-$TS.md"
# Same-minute invocations must not overwrite a prior receipt: bump -2, -3, ...
_n=2
while [ -e "$OUT" ]; do
	OUT="$LOG_DIR/perf-$TS-$_n.md"
	_n=$((_n + 1))
done
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/avec-nightly-perf.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

BIN_DIR="$ROOT/target/release"
BINS="gage_rr ctq_report blake3_throughput parquet_throughput"

log() { printf '%s\n' "$*" >&2; }

# Newest prior perf log (before this run writes its own).
PRIOR="$(ls -t "$LOG_DIR"/perf-*.md 2>/dev/null | head -1 || true)"
PRIOR_NAME="none"
[ -n "$PRIOR" ] && PRIOR_NAME="$(basename "$PRIOR")"

# --- machine context -------------------------------------------------------
hw_model="$(sysctl -n hw.model 2>/dev/null || echo unknown)"
cpu_brand="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
ncores="$(sysctl -n hw.ncpu 2>/dev/null || echo unknown)"
mem_gib="$(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f", $1 / (1024*1024*1024)}')"
os_ver="$(sw_vers -productVersion 2>/dev/null || echo unknown)"
uptime_start="$(uptime)"
lane_procs="$(pgrep -f 'rustc|cargo' 2>/dev/null | wc -l | tr -d ' ')"

# --- step 0: build bench binaries if needed --------------------------------
BUILD_NOTE="binaries present"
missing=""
for b in $BINS; do
	[ -x "$BIN_DIR/$b" ] || missing="$missing $b"
done
if [ -n "$missing" ] && [ "${1:-}" != "--no-build" ]; then
	log "nightly_perf: building bench binaries:$missing"
	if cargo build --release -p bench >/dev/null 2>&1; then
		BUILD_NOTE="cargo build --release -p bench (was missing:$missing)"
	else
		BUILD_NOTE="FAILED cargo build --release -p bench (was missing:$missing)"
	fi
fi

still_missing=""
for b in $BINS; do
	[ -x "$BIN_DIR/$b" ] || still_missing="$still_missing $b"
done
if [ -n "$still_missing" ]; then
	{
		echo "# perf-$TS"
		echo
		echo "Advisory nightly perf log — **SKIP-BINARIES-MISSING** (advisory, exit 0)."
		echo
		echo "- Run: $STAMP"
		echo "- Missing after build attempt:$still_missing"
		echo "- Build note: $BUILD_NOTE"
		echo "- Host: $hw_model ($cpu_brand), $ncores cores, ${mem_gib:-?} GiB, macOS $os_ver"
		echo "- Load at start: $uptime_start"
	} >"$OUT"
	log "nightly_perf: SKIP-BINARIES-MISSING ($still_missing) -> $OUT"
	exit 0
fi

# --- step 1: gage precondition ---------------------------------------------
GAGE_JSON="$("$BIN_DIR/gage_rr" --reps 2000 2>"$SCRATCH/gage.err")"
GAGE_RC=$?
cv="$(printf '%s\n' "$GAGE_JSON" | sed -n 's/.*"cv_trimmed_pct":\([0-9.]*\).*/\1/p' | head -1)"
cv_raw="$(printf '%s\n' "$GAGE_JSON" | sed -n 's/.*"cv_raw_pct":\([0-9.]*\).*/\1/p' | tail -1)"
gage_mean="$(printf '%s\n' "$GAGE_JSON" | sed -n 's/.*"mean_trimmed_us":\([0-9.]*\).*/\1/p')"

# Skip-log writer shared by both precondition failures (advisory: exit 0).
write_skip_log() { # $1 = reason token (SKIP-NOISY | SKIP-GAGE-FAILED)
	local reason="$1"
	local verdict
	verdict="UNPARSED"
	[ -n "$cv" ] && verdict="$(awk -v c="$cv" -v m="$GAGE_CV_MAX_PCT" 'BEGIN{c += 0; m += 0; print (c <= m) ? "PASS" : "FAIL"}')"
	{
		echo "# perf-$TS"
		echo
		echo "Advisory nightly perf log — **$reason** (advisory, exit 0). No bench numbers collected."
		echo
		echo "## Run context"
		echo
		echo "- Run: $STAMP"
		echo "- Lane: wave-10 lane 14 (DX/QoL); concurrent-lane run."
		echo "- Reason: $reason"
		echo "- gage_rr --reps 2000: rc=$GAGE_RC cv_raw=${cv_raw:-unparsed}% cv_trimmed=${cv:-unparsed}% (precondition <= $GAGE_CV_MAX_PCT%: $verdict)"
		echo "- Prior log: $PRIOR_NAME (unchanged; no baseline consumed or produced)"
		echo
		echo "## Machine context"
		echo
		echo "- Host: $hw_model ($cpu_brand), $ncores cores, ${mem_gib:-?} GiB, macOS $os_ver"
		echo "- Load: $uptime_start"
		echo "- Observed rustc/cargo processes at start: $lane_procs"
		echo
		echo "## gage raw"
		echo
		echo '```json'
		[ -n "$GAGE_JSON" ] && printf '%s\n' "$GAGE_JSON" || echo "(empty stdout; rc=$GAGE_RC; stderr: $(tail -c 300 "$SCRATCH/gage.err" 2>/dev/null))"
		echo '```'
	} >"$OUT"
	log "nightly_perf: $reason (cv_trimmed=${cv:-unparsed}%) -> $OUT"
	exit 0
}

[ -z "$cv" ] && write_skip_log SKIP-GAGE-FAILED
[ "$(awk -v c="$cv" -v m="$GAGE_CV_MAX_PCT" 'BEGIN{c += 0; m += 0; print (c > m) ? 1 : 0}')" = "1" ] && write_skip_log SKIP-NOISY

# --- step 2-5: benches (failures recorded, never fatal) --------------------
t0=$(date +%s)
CTQ_JSON="$("$BIN_DIR/ctq_report" 2>"$SCRATCH/ctq.err")"
CTQ_RC=$?
t1=$(date +%s)

BLAKE_OUT="$("$BIN_DIR/blake3_throughput" 2>"$SCRATCH/blake3.err")"
BLAKE_RC=$?

PARQUET_OUT="$("$BIN_DIR/parquet_throughput" 2>"$SCRATCH/parquet.err")"
PARQUET_RC=$?

t2=$(date +%s)
PERF_OUT="$(cd "$ROOT" && cargo test -p avec-git --release --test perf_baseline -- --ignored --nocapture 2>"$SCRATCH/perf.err")"
PERF_RC=$?
t3=$(date +%s)

# --- metric extraction ------------------------------------------------------
ctq03="$(printf '%s\n' "$CTQ_JSON" | sed -n 's/.*ctq03_parquet_level_free_roundtrip.*"value":\([0-9.]*\).*/\1/p')"
ctq06w="$(printf '%s\n' "$CTQ_JSON" | sed -n 's/.*hash_wide_simd[^}]*"value":\([0-9.]*\).*/\1/p')"
blake_gib="$(printf '%s\n' "$BLAKE_OUT" | sed -n 's/.*throughput=\([0-9.]*\) GiB\/s.*/\1/p')"
p_min="$(printf '%s\n' "$PARQUET_OUT" | sed -n 's/^PAYLOAD-SUM:.*min \([0-9.]*\) GiB\/s.*/\1/p')"
p_mean="$(printf '%s\n' "$PARQUET_OUT" | sed -n 's/^PAYLOAD-SUM:.*mean \([0-9.]*\) GiB\/s.*/\1/p')"
a_min="$(printf '%s\n' "$PARQUET_OUT" | sed -n 's/^ACCESSOR-SCAN:.*min \([0-9.]*\) GiB\/s.*/\1/p')"
a_mean="$(printf '%s\n' "$PARQUET_OUT" | sed -n 's/^ACCESSOR-SCAN:.*mean \([0-9.]*\) GiB\/s.*/\1/p')"
pack_mibs="$(printf '%s\n' "$PERF_OUT" | sed -n 's/^PACK WRITE: best \([0-9.]*\) MiB\/s.*/\1/p')"
lookups="$(printf '%s\n' "$PERF_OUT" | sed -n 's/^IDX LOOKUP: \([0-9.]*\) lookups\/s.*/\1/p')"

# --- DELTAS extraction ------------------------------------------------------
# Current metrics snapshot (written again inside the log's Metrics section).
{
	[ -n "$cv" ] && echo "gage_cv_trimmed_pct = $cv"
	[ -n "$gage_mean" ] && echo "gage_mean_trimmed_us = $gage_mean"
	[ -n "$ctq03" ] && echo "ctq03_parquet_roundtrip_gib_s = $ctq03"
	[ -n "$ctq06w" ] && echo "ctq06_blake3_wide_gib_s = $ctq06w"
	[ -n "$blake_gib" ] && echo "blake3_throughput_gib_s = $blake_gib"
	[ -n "$p_min" ] && echo "parquet_payload_sum_min_gib_s = $p_min"
	[ -n "$p_mean" ] && echo "parquet_payload_sum_mean_gib_s = $p_mean"
	[ -n "$a_min" ] && echo "parquet_accessor_min_gib_s = $a_min"
	[ -n "$a_mean" ] && echo "parquet_accessor_mean_gib_s = $a_mean"
	[ -n "$pack_mibs" ] && echo "perf_pack_write_mib_s = $pack_mibs"
	[ -n "$lookups" ] && echo "perf_idx_lookups_s = $lookups"
} >"$SCRATCH/cur.metrics"

# Prior metrics (same parser this script consumes on the next run).
if [ -n "$PRIOR" ]; then
	awk '
		/^## Metrics/ {inm = 1; next}
		/^## / {inm = 0}
		inm && /^[A-Za-z_][A-Za-z_0-9]* = / {print}
	' "$PRIOR" >"$SCRATCH/prev.metrics"
fi

# --- write the log ----------------------------------------------------------
{
	echo "# perf-$TS"
	echo
	echo "Advisory nightly perf log (scripts/nightly_perf.sh, wave-10 lane 14)."
	echo "Advisory only — never gate-wired."
	echo
	echo "## Run context"
	echo
	echo "- Run: $STAMP"
	echo "- Lane: wave-10 lane 14 (DX/QoL); concurrent-lane run, see Concurrent-lane conditions."
	echo "- Prior log: $PRIOR_NAME (DELTAS baseline)"
	echo "- Build: $BUILD_NOTE"
	echo
	echo "## Machine context"
	echo
	echo "- Host: $hw_model ($cpu_brand)"
	echo "- Cores: $ncores; RAM: ${mem_gib:-?} GiB; macOS $os_ver"
	echo "- Load at start: $uptime_start"
	echo
	echo "## Preconditions"
	echo
	echo "- gage_rr --reps 2000: rc=$GAGE_RC cv_raw=${cv_raw:-unparsed}% cv_trimmed=${cv:-unparsed}% — precondition cv_trimmed <= $GAGE_CV_MAX_PCT%: $( [ -n "$cv" ] && awk -v c="$cv" -v m="$GAGE_CV_MAX_PCT" 'BEGIN{print (c <= m) ? "PASS" : "FAIL->SKIP-NOISY"}' || echo UNPARSED)"
	echo
	echo "## Metrics"
	echo
	echo '```text'
	cat "$SCRATCH/cur.metrics"
	echo '```'
	echo
	echo "Metric extraction failures, if any, are visible as absent keys (raw output below is authoritative)."
	echo

	echo "## DELTAS vs $PRIOR_NAME"
	echo
	if [ -z "$PRIOR" ]; then
		echo "No prior perf log in docs/jira/v26.10.31/perf/ — this run IS the baseline; nothing to compare."
	elif [ ! -s "$SCRATCH/prev.metrics" ] || [ ! -s "$SCRATCH/cur.metrics" ]; then
		echo "Prior log found ($PRIOR_NAME) but metric extraction produced no comparable keys — see raw sections."
	else
		awk -v flag="$FLAG_PCT" '
			BEGIN {FS = " = "}
			NR == FNR {cur[$1] = $2; next}
			{
				if ($1 in cur) {
					if ($2 + 0 == 0) {
						printf "- %s: %s -> %s (prior baseline is 0; ratio undefined)\n", $1, $2, cur[$1]
					} else {
						pct = (cur[$1] - $2) / $2 * 100
						mark = ""
						dir = "higher"
						if (pct < 0) {pct = -pct; dir = "lower"}
						if (pct > flag + 0) mark = " — FLAG >" flag "% (" dir ")"
						printf "- %s: %s -> %s (%+.1f%%%s)\n", $1, $2, cur[$1], ((cur[$1] - $2) / $2 * 100), mark
					}
				}
			}
		' "$SCRATCH/cur.metrics" "$SCRATCH/prev.metrics"
		echo
		echo 'Notes: gage_* keys are noise indicators, not throughput — their deltas are expected to move.'
		echo "Flags are informational; this lane never gates."
	fi
	echo

	echo "## Concurrent-lane conditions"
	echo
	echo "- Observed rustc/cargo processes at start: $lane_procs"
	echo "- Load at end: $(uptime)"
	echo "- Wave-10 fan-out was active on this host during this run; treat all numbers as"
	echo "  load-contaminated datapoints, not quiet-machine ceilings (matching the"
	echo "  perf_baseline test's own machine-context caveat)."
	echo

	echo "## Step status"
	echo
	echo "| step | rc | wall (s) |"
	echo "|---|---|---|"
	echo "| ctq_report | $CTQ_RC | $((t1 - t0)) |"
	echo "| blake3_throughput | $BLAKE_RC | - |"
	echo "| parquet_throughput | $PARQUET_RC | - |"
	echo "| perf_baseline (cargo test) | $PERF_RC | $((t3 - t2)) |"
	echo

	echo "## Raw output"
	echo
	echo "### gage_rr --reps 2000"
	echo
	echo '```json'
	if [ -n "$GAGE_JSON" ]; then
		printf '%s\n' "$GAGE_JSON"
	else
		echo "(empty stdout)"
	fi
	echo '```'
	[ $GAGE_RC -ne 0 ] && echo "(rc=$GAGE_RC — the gage's own 1% gate; this lane's precondition is $GAGE_CV_MAX_PCT%; stderr: $(tail -c 300 "$SCRATCH/gage.err" 2>/dev/null))"
	echo
	echo "### ctq_report"
	echo
	echo '```json'
	if [ -n "$CTQ_JSON" ]; then
		printf '%s\n' "$CTQ_JSON"
	else
		echo "(empty stdout)"
	fi
	echo '```'
	[ $CTQ_RC -ne 0 ] && echo "(rc=$CTQ_RC — ctq_report's own FAIL verdict; advisory for this lane, never gate-wired here; stderr tail: $(tail -c 300 "$SCRATCH/ctq.err" 2>/dev/null))"
	echo
	echo "### blake3_throughput"
	echo
	echo '```text'
	if [ -n "$BLAKE_OUT" ]; then
		printf '%s\n' "$BLAKE_OUT"
	else
		echo "(empty stdout; rc=$BLAKE_RC; stderr: $(tail -c 300 "$SCRATCH/blake3.err" 2>/dev/null))"
	fi
	echo '```'
	[ $BLAKE_RC -ne 0 ] && echo "(rc=$BLAKE_RC; stderr tail: $(tail -c 300 "$SCRATCH/blake3.err" 2>/dev/null))"
	echo
	echo "### parquet_throughput"
	echo
	echo '```text'
	if [ -n "$PARQUET_OUT" ]; then
		printf '%s\n' "$PARQUET_OUT"
	else
		echo "(empty stdout; rc=$PARQUET_RC; stderr: $(tail -c 300 "$SCRATCH/parquet.err" 2>/dev/null))"
	fi
	echo '```'
	[ $PARQUET_RC -ne 0 ] && echo "(rc=$PARQUET_RC; stderr tail: $(tail -c 300 "$SCRATCH/parquet.err" 2>/dev/null))"
	echo
	echo "### perf_baseline"
	echo
	echo "Command: \`cargo test -p avec-git --release --test perf_baseline -- --ignored --nocapture\`"
	echo
	echo '```text'
	if [ -n "$PERF_OUT" ]; then
		printf '%s\n' "$PERF_OUT" | sed -n '/=== avec-git perf baseline/,$p'
	else
		echo "(empty stdout; rc=$PERF_RC; stderr tail: $(tail -c 300 "$SCRATCH/perf.err" 2>/dev/null))"
	fi
	echo '```'
	[ $PERF_RC -ne 0 ] && echo "(rc=$PERF_RC; stderr tail: $(tail -c 300 "$SCRATCH/perf.err" 2>/dev/null))"
} >"$OUT"

log "nightly_perf: wrote $OUT (gage cv_trimmed=${cv:-unparsed}%, prior=$PRIOR_NAME)"
exit 0
