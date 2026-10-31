#!/usr/bin/env bash
# dod.sh — machine-checkable Definition-of-Done checker, v26.10.31
# Manifest: docs/jira/v26.10.31/dod-manifest.toml
# (entry schema + expect semantics documented in the manifest header)
#
# USAGE:
#   scripts/dod.sh                 run every DoD entry, print verdict table
#   scripts/dod.sh --selftest      prove the checker flips (anti-vacuity)
#   scripts/dod.sh <manifest>      run an alternate manifest
#
# SELFTEST CONTRACT: builds two throwaway manifests and asserts the checker
# marks a passing entry green, a deliberately failing entry red, a
# command-not-found red (fail-closed), a pending-holds entry yellow PENDING,
# a moved pending state red STALE, an advisory exit-2 yellow ADV-SKIP, an
# embedded-= check value parsed byte-exact, and that a manifest holding only
# PASS+PENDING exits 0 (pending does not fail the run). Any mismatch prints
# VACUOUS-CHECKER and exits 1: a checker that cannot flip is not a checker.
#
# EXIT: 0 no red (PENDING / ADV-SKIP yellow listed, non-failing)
#       1 at least one red (FAIL or STALE)
#       2 usage error
#       3 fail-closed: manifest missing or parsed to zero entries
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ -t 1 ]; then
  C_G=$'\033[0;32m'; C_Y=$'\033[0;33m'; C_R=$'\033[0;31m'; C_N=$'\033[0m'
else
  C_G=""; C_Y=""; C_R=""; C_N=""
fi

# parse_manifest <file> — emits id<TAB>class<TAB>expect<TAB>description<TAB>
# check<TAB>evidence rows. Line-oriented like the audit gate's manifest
# reader: anchored prefix-strip through the FIRST '=', so later '=' chars
# inside a check value survive byte-exact. The variable is named expt, not
# exp, to avoid colliding with the awk builtin function name.
parse_manifest() {
  awk '
    # Rows are joined with ASCII 31 (unit separator), NOT a tab: bash `read`
    # treats tab as IFS whitespace and DISCARDS empty fields, shifting every
    # column (witnessed: cls swallowed expt when it was empty). 31 is not IFS
    # whitespace, so empty fields survive the split.
    function emit() {
      sep = sprintf("%c", 31)
      print id sep cls sep expt sep desc sep chk sep ev
    }
    function stripq(s) { sub(/^[ \t]*"/, "", s); sub(/"[ \t]*$/, "", s); return s }
    /^\[\[entry\]\]/ { if (id != "") emit(); id=""; cls=""; expt=""; desc=""; chk=""; ev="" }
    /^id[ \t]*=/          { v=$0; sub(/^[^=]*=/, "", v); gsub(/[" \t]/, "", v); id=v }
    /^class[ \t]*=/       { v=$0; sub(/^[^=]*=/, "", v); gsub(/[" \t]/, "", v); cls=v }
    /^expect[ \t]*=/      { v=$0; sub(/^[^=]*=/, "", v); gsub(/[" \t]/, "", v); expt=v }
    /^description[ \t]*=/ { v=$0; sub(/^[^=]*=/, "", v); desc=stripq(v) }
    /^check[ \t]*=/       { v=$0; sub(/^[^=]*=/, "", v); chk=stripq(v) }
    /^evidence[ \t]*=/    { v=$0; sub(/^[^=]*=/, "", v); ev=stripq(v) }
    END { if (id != "") emit() }
  ' "$1"
}

# run_manifest <manifest> — prints the verdict table.
# Returns: 0 no red | 1 red present | 3 fail-closed (missing/empty manifest).
run_manifest() {
  local manifest="$1"
  [ -f "$manifest" ] || { echo "FATAL: manifest missing: $manifest" >&2; return 3; }
  local rows
  rows="$(parse_manifest "$manifest")"
  if [ -z "$rows" ]; then
    echo "FATAL: manifest parsed to zero entries: $manifest" >&2
    return 3
  fi
  echo "== Definition of Done — milestone v26.10.31"
  echo "manifest: $manifest ($(printf '%s\n' "$rows" | wc -l | tr -d ' ') entries)"
  local red=0 yellow=0 green=0 pending_ids="" red_ids=""
  local id cls expt desc chk ev out rc status detail col row
  while IFS= read -r row; do
    IFS=$'\x1f' read -r id cls expt desc chk ev <<< "$row"
    status=""
    if [ -z "$chk" ]; then
      rc="parser"; out="manifest defect: empty check field"; status=FAIL
    else
      out="$(bash -c "$chk" 2>&1)"; rc=$?
    fi
    if [ "$expt" = "pass" ] && [ "$rc" -eq 0 ]; then status=PASS
    elif [ "$expt" = "advisory" ] && [ "$rc" -eq 0 ]; then status=PASS
    elif [ "$expt" = "advisory" ] && [ "$rc" -eq 2 ]; then status=ADV-SKIP
    elif [ "$expt" = "pending-holds" ] && [ "$rc" -eq 0 ]; then status=PENDING
    elif [ "$expt" = "pending-holds" ]; then status=STALE
    fi
    [ -n "$status" ] || status=FAIL
    case "$status" in
      PASS) green=$((green+1)); col=$C_G ;;
      ADV-SKIP|PENDING)
        yellow=$((yellow+1)); col=$C_Y
        [ "$status" = PENDING ] && pending_ids="$pending_ids $id" ;;
      *) red=$((red+1)); col=$C_R; red_ids="$red_ids $id" ;;
    esac
    detail="$(printf '%s\n' "$out" | grep -m1 . || true)"
    detail="${detail:0:110}"
    printf '%s[%-7s]%s %s\n' "$col" "$status" "$C_N" "$id"
    printf '         desc: %s\n' "$desc"
    printf '         exit=%s | out: %s\n' "$rc" "${detail:--}"
    printf '         evidence: %s\n' "$ev"
  done <<< "$rows"

  echo
  if [ "$red" -gt 0 ]; then
    printf '%s== DoD verdict: NOT DONE -- red=%d yellow=%d green=%d%s\n' \
      "$C_R" "$red" "$yellow" "$green" "$C_N"
    echo "   red:$red_ids"
    echo "   FAIL/STALE blocks the milestone; fix the subject or update the manifest."
    return 1
  elif [ "$yellow" -gt 0 ]; then
    printf '%s== DoD verdict: GATES GREEN -- %d PENDING (green=%d)%s\n' \
      "$C_Y" "$yellow" "$green" "$C_N"
    echo "   pending:$pending_ids"
    echo "   PENDING = recorded operator decision; ADV-SKIP = noisy-machine skip."
    echo "   Listed and recorded, non-failing."
    return 0
  else
    printf '%s== DoD verdict: DONE -- all %d entries green%s\n' \
      "$C_G" "$green" "$C_N"
    return 0
  fi
}

# selftest — anti-vacuity court for the checker itself: generate two
# throwaway manifests, assert every status class flips as specified, and
# fail loudly (VACUOUS-CHECKER) on any mismatch.
selftest() {
  local t
  t="$(mktemp -d "${TMPDIR:-/tmp}/dod-selftest.XXXXXX")"
  DOD_SELFTEST_TMP="$t"   # global: the EXIT trap outlives these locals (set -u)
  trap 'rm -rf "$DOD_SELFTEST_TMP"' EXIT
  printf 'const GATE_F = None;\n' > "$t/fixture"
  local m="$t/a.toml"
  {
    echo '[[entry]]'
    echo 'id = "selft-pass"'
    echo 'expect = "pass"'
    echo 'description = "passing entry"'
    echo 'check = "true"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-fail"'
    echo 'expect = "pass"'
    echo 'description = "deliberately failing entry"'
    echo 'check = "exit 7"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-error"'
    echo 'expect = "pass"'
    echo 'description = "command not found"'
    echo 'check = "dod_selftest_missing_binary_xyz --any"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-pending"'
    echo 'expect = "pending-holds"'
    echo 'description = "pending state still holds"'
    echo 'check = "true"'
    echo 'evidence = "selftest"'
  } > "$m"
  {
    echo '[[entry]]'
    echo 'id = "selft-stale"'
    echo 'expect = "pending-holds"'
    echo 'description = "pending state moved to landed"'
    echo 'check = "exit 1"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-adv-ok"'
    echo 'expect = "advisory"'
    echo 'description = "advisory check quiet"'
    echo 'check = "true"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-adv-skip"'
    echo 'expect = "advisory"'
    echo 'description = "advisory SKIP-NOISY"'
    echo 'check = "exit 2"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-parse"'
    echo 'expect = "pass"'
    echo 'description = "parser keeps embedded = byte-exact"'
    echo "check = \"grep -q 'F = None;' $t/fixture\""
    echo 'evidence = "selftest"'
  } >> "$m"
  local outA rcA fails=0 line
  outA="$(run_manifest "$m")"; rcA=$?
  printf '%s\n' "$outA" | sed 's/^/    | /'
  assert_line() {
    local id="$1" want="$2"
    line="$(printf '%s\n' "$outA" | grep -F "$id" | head -1)"
    if [[ "$line" == *"[$want"* ]]; then
      printf '  ok  %-14s -> [%s\n' "$id" "$want"
    else
      printf '  VACUOUS-CHECKER: %-14s expected [%s got: %s\n' \
        "$id" "$want" "${line:-<none>}"
      fails=$((fails+1))
    fi
  }
  assert_line selft-pass PASS
  assert_line selft-fail FAIL
  assert_line selft-error FAIL
  assert_line selft-pending PENDING
  assert_line selft-stale STALE
  assert_line selft-adv-ok PASS
  assert_line selft-adv-skip ADV-SKIP
  assert_line selft-parse PASS
  if [ "$rcA" -ne 1 ]; then
    printf '  VACUOUS-CHECKER: manifest A exit=%s expected 1 (reds present)\n' "$rcA"
    fails=$((fails+1))
  else
    printf '  ok  manifest A exit=1 (red present -> NOT DONE)\n'
  fi
  local mb="$t/b.toml"
  {
    echo '[[entry]]'
    echo 'id = "selft-pass2"'
    echo 'expect = "pass"'
    echo 'description = "passing entry"'
    echo 'check = "true"'
    echo 'evidence = "selftest"'
    echo
    echo '[[entry]]'
    echo 'id = "selft-pending2"'
    echo 'expect = "pending-holds"'
    echo 'description = "pending does not fail the run"'
    echo 'check = "true"'
    echo 'evidence = "selftest"'
  } > "$mb"
  local outB rcB
  outB="$(run_manifest "$mb")"; rcB=$?
  printf '%s\n' "$outB" | sed 's/^/    | /'
  if [ "$rcB" -eq 0 ] && printf '%s\n' "$outB" | grep -q 'GATES GREEN' \
     && printf '%s\n' "$outB" | grep -q 'selft-pending2'; then
    printf '  ok  manifest B exit=0, PENDING listed (pending does not fail the run)\n'
  else
    printf '  VACUOUS-CHECKER: manifest B exit=%s — pending must be listed + exit 0\n' "$rcB"
    fails=$((fails+1))
  fi
  if [ "$fails" -gt 0 ]; then
    printf '== SELFTEST FAIL: checker VACUOUS (%d assertion(s) failed)\n' "$fails"
    return 1
  fi
  echo '== SELFTEST PASS: fail/error->red, pending->yellow non-failing, stale->red,'
  echo '   advisory-2->yellow, parser keeps embedded = byte-exact, red blocks, pending exits 0'
  return 0
}

# --- main dispatch -----------------------------------------------------------
case "${1:-}" in
  --selftest) selftest; exit $? ;;
  -h|--help)
    awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print } /^$/{exit}' "$0" | head -40
    exit 0 ;;
  "")
    run_manifest "$ROOT/docs/jira/v26.10.31/dod-manifest.toml"
    exit $? ;;
  *)
    run_manifest "$1"
    exit $? ;;
esac
