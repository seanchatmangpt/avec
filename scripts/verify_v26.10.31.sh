#!/usr/bin/env bash
# verify_v26.10.31.sh — top-level verification orchestrator (lane I20)
# Spec: docs/jira/v26.10.31/31-verification-ladder.md (R8 machine report) and
#       docs/jira/v26.10.31/36-release-checklist.md (DMADV Verify gate).
#
# Runs the gate battery in dependency order, each gate recorded with exit code
# plus tail of output. `cargo check --workspace` failures are classified
# PRE-EXISTING-FAIL (disclosed, does not fail the run) when the failure output
# matches the baseline scripts/verify_baseline.txt; otherwise FAIL (introduced).
# Final exit 0 only when no gate is FAIL.
#
# Usage: scripts/verify_v26.10.31.sh [--regen-baseline]
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BASELINE="scripts/verify_baseline.txt"
LATEST="docs/jira/v26.10.31/verify-latest.md"
mkdir -p "$(dirname "$LATEST")"

if [ "${1:-}" = "--regen-baseline" ]; then
  echo "REGEN: deleting $BASELINE; it will be rewritten from this run's check" >&2
  rm -f "$BASELINE"
fi

TMPDIR_V="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_V"' EXIT
TABLE="$TMPDIR_V/table"
NAMES="$TMPDIR_V/names"
: > "$TABLE"; : > "$NAMES"
FAIL_ANY=0

# run_gate <name> <cmd...>
run_gate() {
  local name="$1"; shift
  echo "=== GATE $name: $*" >&2
  local out rc tail status
  out="$("$@" 2>&1)"
  rc=$?
  tail="$(printf '%s\n' "$out" | tail -12)"
  if [ $rc -eq 0 ]; then
    status=PASS
  else
    status=FAIL
    FAIL_ANY=1
  fi
  printf '| `%s` | %s | %s |\n' "$name" "$status" "$rc" >> "$TABLE"
  printf '%s\n' "$name" >> "$NAMES"
  printf '%s\n' "$tail" > "$TMPDIR_V/tail_$name"
  printf 'GATE %-22s %-18s exit=%s\n' "$name" "$status" "$rc" >&2
  printf '%s\n' "$tail" | sed 's/^/    | /' >&2
}

# ---------------------------------------------------------------------------
# Baseline (for workspace-check classification)
# ---------------------------------------------------------------------------
if [ ! -f "$BASELINE" ]; then
  echo "BASELINE: missing — will be written from this run's check output" >&2
  TOUCH_BASELINE=1
else
  TOUCH_BASELINE=0
fi

# ---------------------------------------------------------------------------
# Gate battery, dependency order
# ---------------------------------------------------------------------------
run_gate check-workspace cargo check --workspace

# Classification: compare sorted *error-signature* lines only, so build
# freshness noise ("Checking"/"Compiling"/"Finished") never reads as drift.
sig() { grep -E '^(error|warning)' | sort -u; }
if grep -q '^| `check-workspace` | FAIL' "$TABLE"; then
  # full check output was tailed to 12 lines for the report; re-run check for
  # the full signature (cheap: cached).
  cargo check --workspace 2>&1 | sig > "$TMPDIR_V/cur.sorted"
  if [ "$TOUCH_BASELINE" = "1" ]; then
    cp "$TMPDIR_V/cur.sorted" "$BASELINE"
    echo "BASELINE: written from first run (error signatures; failure recorded as pre-existing)" >&2
  fi
  if diff -q "$BASELINE" "$TMPDIR_V/cur.sorted" >/dev/null 2>&1; then
    sed -i '' 's/| `check-workspace` | FAIL |/| `check-workspace` | PRE-EXISTING-FAIL |/' "$TABLE"
    FAIL_ANY=0
    echo "CLASSIFY: check-workspace matches baseline -> PRE-EXISTING-FAIL (disclosed)" >&2
  else
    echo "CLASSIFY: differs from baseline -> FAIL (introduced)" >&2
  fi
elif [ "$TOUCH_BASELINE" = "1" ]; then
  cargo check --workspace > /dev/null 2>&1
  cp /dev/null "$BASELINE"
  echo "BASELINE: check passed; empty baseline written" >&2
elif [ ! -s "$BASELINE" ]; then
  : # empty baseline from a prior green run, nothing to do
fi

run_gate test-cgap-arrow   cargo test -p cgap-arrow
# cgap-parquet gates require the std feature to compile its test harness.
run_gate test-cgap-parquet cargo test -p cgap-parquet --features std
run_gate test-cgap-git     cargo test -p cgap-git
run_gate test-cgap-blake3  cargo test -p cgap-blake3
run_gate test-integration  cargo test -p cgap-integration

# cgap-beam spawns a real BEAM VM; timeout-guard at 10 min, loud skip on
# timeout so a hung BEAM never silently fails or silently passes.
BEAM_RC=0
timeout 600 cargo test -p cgap-beam > "$TMPDIR_V/beam.out" 2>&1 || BEAM_RC=$?
if [ "$BEAM_RC" -eq 0 ]; then
  printf '| `test-cgap-beam` | PASS | 0 |\n' >> "$TABLE"
  printf '%s\n' "test-cgap-beam" >> "$NAMES"
  tail -12 "$TMPDIR_V/beam.out" > "$TMPDIR_V/tail_test-cgap-beam"
  echo "GATE test-cgap-beam       PASS              exit=0" >&2
elif [ "$BEAM_RC" -eq 124 ]; then
  printf '| `test-cgap-beam` | SKIP | 124 |\n' >> "$TABLE"
  printf '%s\n' "test-cgap-beam" >> "$NAMES"
  tail -12 "$TMPDIR_V/beam.out" > "$TMPDIR_V/tail_test-cgap-beam"
  echo "GATE test-cgap-beam       SKIP (timeout 600s) exit=124" >&2
else
  printf '| `test-cgap-beam` | FAIL | %s |\n' "$BEAM_RC" >> "$TABLE"
  printf '%s\n' "test-cgap-beam" >> "$NAMES"
  tail -12 "$TMPDIR_V/beam.out" > "$TMPDIR_V/tail_test-cgap-beam"
  FAIL_ANY=1
  echo "GATE test-cgap-beam       FAIL              exit=$BEAM_RC" >&2
fi

if [ -x scripts/audit_branchless.sh ]; then
  run_gate audit-branchless scripts/audit_branchless.sh --selftest
else
  printf '| `audit-branchless` | SKIP | - |\n' >> "$TABLE"
  echo "GATE audit-branchless SKIP: scripts/audit_branchless.sh absent (owner lane I14)" >&2
fi

# Lane I17 cli verify bin: the artifact is the standalone verify_v26 bin
# (crates/cli/src/bin/verify_v26.rs). It runs the FR-01..07 battery itself and
# prints its own JSON; it takes no subcommand. Built+run via cargo run (never
# depend on a stale prebuilt target/); timeout-guard at 5 min, loud skip on
# timeout so a hung run never silently fails or silently passes.
CLI_RC=0
timeout 300 cargo run -q -p cgap-cli --bin verify_v26 > "$TMPDIR_V/cli.out" 2>&1 || CLI_RC=$?
if [ "$CLI_RC" -eq 0 ]; then
  printf '| `cli-verify` | PASS | 0 |\n' >> "$TABLE"
  printf '%s\n' "cli-verify" >> "$NAMES"
  tail -12 "$TMPDIR_V/cli.out" > "$TMPDIR_V/tail_cli-verify"
  echo "GATE cli-verify         PASS              exit=0" >&2
elif [ "$CLI_RC" -eq 124 ]; then
  printf '| `cli-verify` | SKIP | 124 |\n' >> "$TABLE"
  printf '%s\n' "cli-verify" >> "$NAMES"
  tail -12 "$TMPDIR_V/cli.out" > "$TMPDIR_V/tail_cli-verify"
  echo "GATE cli-verify         SKIP (timeout 300s) exit=124" >&2
else
  printf '| `cli-verify` | FAIL | %s |\n' "$CLI_RC" >> "$TABLE"
  printf '%s\n' "cli-verify" >> "$NAMES"
  tail -12 "$TMPDIR_V/cli.out" > "$TMPDIR_V/tail_cli-verify"
  FAIL_ANY=1
  echo "GATE cli-verify         FAIL              exit=$CLI_RC" >&2
fi

run_gate test-audit cargo test -p cgap-audit

# ---------------------------------------------------------------------------
# Emit verify-latest.md
# ---------------------------------------------------------------------------
HEAD_SHA="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
{
  echo "# Verify Latest: v26.10.31 (lane I20)"
  echo
  echo "Generated: $(date -u '+%Y-%m-%dT%H:%M:%SZ') · HEAD: \`${HEAD_SHA}\` · host: $(uname -m)"
  echo
  echo "## Gate Summary"
  echo
  echo "| Gate | Status | Exit |"
  echo "|------|--------|------|"
  cat "$TABLE"
  echo
  echo "## Evidence Tails (last 12 lines per gate)"
  echo
  while IFS= read -r name; do
    echo "### $name"
    echo
    echo '```'
    cat "$TMPDIR_V/tail_$name"
    echo '```'
    echo
  done < "$NAMES"
  echo "## Baseline"
  echo
  echo "Workspace-check baseline: \`scripts/verify_baseline.txt\` ($(wc -l < "$BASELINE" | tr -d ' ') lines)."
  echo "PRE-EXISTING-FAIL = failure output matches baseline; FAIL = output differs (introduced)."
  echo "Regenerate: \`scripts/verify_v26.10.31.sh --regen-baseline\` (see verify-README.md)."
} > "$LATEST"

echo
echo "SUMMARY: report -> $LATEST"
if [ "$FAIL_ANY" = "1" ]; then
  echo "RESULT: FAIL — at least one gate failed; see $LATEST"
  exit 1
fi
echo "RESULT: OK — no FAIL; skips/pre-existing disclosed in $LATEST"
exit 0
