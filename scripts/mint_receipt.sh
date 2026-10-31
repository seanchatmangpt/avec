#!/usr/bin/env bash
# mint_receipt.sh — fail-closed benchmark/gate receipt minter.
#
# Usage: scripts/mint_receipt.sh <name> <command> [args...]
#
# Runs <command>, captures command line, exit code, stdout tail, timestamp,
# git HEAD sha (read-only `git rev-parse HEAD`), dirty-tree flag, and the
# BLAKE3 (or SHA-256 fallback) of the captured stdout. Renders
# docs/jira/v26.10.31/receipts/<name>.md from template.md.
#
# Fail-closed: the script exits nonzero when the minted command exits nonzero.
# The receipt is still written and marked FAILED — never silent.
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <name> <command> [args...]" >&2
  exit 64
fi

NAME="$1"
shift
CMD_STR="$*"

REPO="$(cd "$(dirname "$0")/.." && pwd)"
RECEIPT_DIR="$REPO/docs/jira/v26.10.31/receipts"
TEMPLATE="$RECEIPT_DIR/template.md"
OUT="$RECEIPT_DIR/$NAME.md"

if [[ ! -f "$TEMPLATE" ]]; then
  echo "FATAL: template missing: $TEMPLATE" >&2
  exit 66
fi

case "$NAME" in
  *[^A-Za-z0-9._-]*) echo "FATAL: name must be [A-Za-z0-9._-] only: $NAME" >&2; exit 64 ;;
esac
if [[ -e "$OUT" ]]; then
  echo "FATAL: receipt already exists (receipts are append-only): $OUT" >&2
  exit 73
fi

TMP_OUT="$(mktemp)"
trap 'rm -f "$TMP_OUT"' EXIT

set +e
"$@" >"$TMP_OUT" 2>&1
EXIT_CODE=$?
set -e

TS="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
HEAD_SHA="$(git -C "$REPO" rev-parse HEAD 2>/dev/null || echo UNKNOWN)"
DIRTY_LINES="$(git -C "$REPO" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
if [[ "$DIRTY_LINES" -gt 0 ]]; then CLEAN_DIRTY="dirty ($DIRTY_LINES entries)"; else CLEAN_DIRTY="clean"; fi
MACHINE="$(sw_vers 2>/dev/null | tr '\n' ' '; sysctl -n hw.model 2>/dev/null)"
MACHINE="${MACHINE%% }"

DIGEST="$(b3sum "$TMP_OUT" 2>/dev/null | awk '{print $1}')"
HASH_ALG="blake3"
if [[ -z "$DIGEST" ]]; then
  DIGEST="$(shasum -a 256 "$TMP_OUT" | awk '{print $1}')"
  HASH_ALG="sha256"
fi

EVIDENCE="$(tail -n 40 "$TMP_OUT")"
if [[ -z "$EVIDENCE" ]]; then
  EVIDENCE="(empty stdout — no evidence captured)"
fi

if [[ $EXIT_CODE -eq 0 ]]; then
  STANDING="ALIVE"
else
  STANDING="FAILED (exit $EXIT_CODE)"
fi

# CMD_STR is operator-supplied and can contain the sed delimiter (`|`, as in
# any explicit BRANCHLESS_SCOPE='a|b') or replacement-side metacharacters
# (`&`, `\`). Escape them so the {{COMMAND}} substitution is literal.
# (Witnessed 2026-10-30: BRANCHLESS_SCOPE='transpose|untranspose' aborted the
# mint with `sed: bad flag in substitute command` and left a 0-byte artifact.)
CMD_STR_SED="$(printf '%s' "$CMD_STR" | sed -e 's/[\\&|]/\\&/g')"

{
  sed \
    -e "s|{{RECEIPT_ID}}|$NAME|" \
    -e "s|{{DATE}}|$TS|" \
    -e "s|{{SUBJECT}}|minted by scripts/mint_receipt.sh|" \
    -e "s|{{COMMAND}}|\`$CMD_STR_SED\`|" \
    -e "s|{{EXIT}}|$EXIT_CODE|" \
    -e "s|{{STANDING}}|$STANDING|" \
    -e "s|{{MACHINE}}|$MACHINE|" \
    -e "s|{{INPUT_SHA}}|$HEAD_SHA|" \
    -e "s|{{DIRTY}}|$CLEAN_DIRTY|" \
    -e "s|{{HASH_ALG}}|$HASH_ALG|" \
    -e "s|{{DIGEST}}|$DIGEST|" \
    "$TEMPLATE"
} >"$OUT"

# Evidence block appended (multi-line, cannot go through sed safely).
{
  printf '\n<!-- auto-generated block; part of the receipt -->\n'
  printf '\n## Evidence (stdout tail, last 40 lines)\n\n'
  printf '```text\n%s\n```\n' "$EVIDENCE"
  printf '\n<!-- Tree state: %s -->\n' "$CLEAN_DIRTY"
} >>"$OUT"

echo "receipt: $OUT (exit=$EXIT_CODE, standing=$STANDING, $HASH_ALG=$DIGEST)"

# Fail-closed: propagate the minted command's exit status.
exit "$EXIT_CODE"
