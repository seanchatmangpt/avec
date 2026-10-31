#!/usr/bin/env bash
# audit_branchless.sh — FR-05/CTQ-01 branchless kernel ABI gate
# Spec: docs/jira/v26.10.31/27-branchless-kernel-abi.md §2-4
#
# USAGE:
#   scripts/audit_branchless.sh [OBJECT|STATICLIB ...]   audit given artifacts
#       (default: builds avec-arrow + avec-blake3 release libs into a TEMP
#        CARGO_TARGET_DIR; the repo tree is never disturbed)
#   scripts/audit_branchless.sh --selftest
#       runs the positive control (must detect branches in a fixture) and a
#       scope-demo (a clean fixture must pass) ; exits 0 only if both hold.
#
# BEHAVIOR:
#   Disassembles .text (disassembler: llvm-objdump if present, else objdump)
#   and counts banned opcodes per spec §2:
#     x86_64 : jmp + full jcc family
#     ARM64  : b, b.cond, br, cbz, cbnz, tbz, tbnz   (bl/ret permitted)
#   Scope (spec §3 honesty): whole-crate .text contains legitimate branches
#   outside hot kernels, so only symbols whose mangled name matches a prefix
#   in the SCOPE regex are audited. Default scope (avec-arrow hot-kernel
#   candidates): ladder|predicates|transpose|ct_ . Override with
#   BRANCHLESS_SCOPE="p1|p2" (matched UNANCHORED against the demangled-
#   shape symbol with any leading mach-o underscore stripped, so mangled
#   Rust names like __ZN9avec_arrow7ladder... match "ladder"; pass ".*"
#   to audit everything — expect failures).
#   Symbols outside scope are reported as SKIPPED counts, never silently.
#
# EXIT CODES:
#   0  pass (positive control fired; audited scope clean)
#   1  gate violation: banned opcodes found (file:symbol:count listed)
#   2  VACUOUS GATE: positive control did NOT detect injected branches
#   3  fail-closed: missing tool, missing object, unparseable disassembly
#
# MANIFEST (ADR-14): scripts/branchless_manifest.toml declares per-kernel
# accepted-remainder classes, expected_max_count and gate_mode
# (ENFORCED|INFORMATIONAL). Hits are aggregated per symbol; a symbol whose
# count <= expected_max_count for a manifest kernel whose scope_pattern
# matches, with every observed mnemonic listed in that kernel's
# observed_mnemonics, is MANIFEST-OK (warn, not fail). Exceeding the ceiling,
# an unaccepted mnemonic, or a symbol no manifest kernel claims is a FAIL
# regardless of gate_mode. The positive-control fixture is unlisted, so the
# control path is unaffected.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
CASES="$REPO/scripts/audit_branchless_cases"
MANIFEST="${BRANCHLESS_MANIFEST:-$REPO/scripts/branchless_manifest.toml}"
SCOPE_RE="${BRANCHLESS_SCOPE:-ladder|predicates|transpose|ct_}"

# --- disassembler detection (fail-closed, spec §3.4) ------------------------
DISASM=""
for t in llvm-objdump objdump; do
  if command -v "$t" >/dev/null 2>&1; then DISASM="$t"; break; fi
done
[ -n "$DISASM" ] || { echo "FATAL: no llvm-objdump/objdump on PATH" >&2; exit 3; }
echo "== disassembler: $DISASM ($( $DISASM --version 2>/dev/null | head -1 ))"

# --- banned-opcode regex (spec §2) -------------------------------------------
# x86_64: jmp + jcc family; ARM64: b, b.cond, br, cbz, cbnz, tbz, tbnz
# (bl/ret permitted). Applied to the mnemonic token of each disassembly line.
#
# TRANSPORT: this reaches awk via ENVIRON, not `awk -v re=...`. The -v
# assignment escape-decodes its value, so `b\.[a-z]+` arrived as `b.[a-z]+`
# and the unescaped dot false-matched non-branch mnemonics (bic, blr) —
# REGEX_TRANSPORT_FP in branchless_manifest.toml / receipt
# gate-branchless-manifest-v2. ENVIRON values reach awk byte-exact
# (witnessed: bic/blr 1 under -v, 0 under ENVIRON; b.eq 1 under both).
# Fix scoped to BANNED_RE only; scope= transport unchanged (no backslashes).
export BANNED_RE='^(jmp|je|jne|jg|jl|jge|jle|ja|jae|jb|jbe|js|jns|jo|jno|jp|jnp|jc|jnc|b|br|b\.[a-z]+|cbz|cbnz|tbz|tbnz)$'

# audit one single object file; appends to ALL_HITS, sets SCANNED/SKIPPED/HITS
audit_object() {
  local f="$1"
  local dis
  dis="$("$DISASM" -d "$f" 2>/dev/null)" || { echo "FATAL: unparseable disassembly: $f" >&2; exit 3; }
  [ -n "$dis" ] || { SCANNED=0; SKIPPED=0; HITS=""; return 0; }  # no __text (e.g. data-only member)
  local out
  out="$(printf '%s\n' "$dis" | awk -v scope="$SCOPE_RE" -v fname="$f" '
    BEGIN { re = ENVIRON["BANNED_RE"] }
    /^[0-9a-f]+ </ {
      s=$0; sub(/^[0-9a-f]+ </, "", s); sub(/>.*$/, "", s)
      bare=s; sub(/^_/, "", bare)
      inscope = (bare ~ ("(" scope ")"))
      next
    }
    /^[ \t]*[0-9a-f]+:/ {
      if (!inscope) { sk++ ; next }
      sc++
      line=$0
      if (match(line, /\t[ \t]*[a-z][a-z0-9.]*/)) {
        m=substr(line, RSTART+1); sub(/^[ \t]+/, "", m); sub(/[ \t].*$/, "", m)
        if (m ~ re)
          print fname ":" s " :: " line
      }
      next
    }
    END { print "SCANNED " sc+0 " SKIPPED " sk+0 }
  ')"
  SCANNED="$(echo "$out" | awk '/^SCANNED/{print $2}')"
  SKIPPED="$(echo "$out" | awk '/^SCANNED/{print $4}')"
  HITS="$(echo "$out" | grep -v '^SCANNED' || true)"
}

# audit one artifact (object OR ar archive: rlib/staticlib members extracted);
# aggregates into TOTAL_* and ALL_HITS
audit_artifact() {
  local f="$1"
  [ -f "$f" ] || { echo "FATAL: missing artifact: $f" >&2; exit 3; }
  SCANNED=0; SKIPPED=0; HITS=""
  local as; as="$(file -b "$f")"
  local members=()
  if [[ "$as" == *"ar archive"* ]]; then
    local tmp; tmp="$(mktemp -d)"
    (cd "$tmp" && ar -x "$f") || { echo "FATAL: cannot extract archive: $f" >&2; rm -rf "$tmp"; exit 3; }
    while IFS= read -r m; do members+=("$tmp/$m"); done < <(cd "$tmp" && ls)
  else
    members+=("$f")
  fi
  local m
  local A_SCANNED=0 A_SKIPPED=0
  for m in "${members[@]}"; do
    local ms; ms="$(file -b "$m")"
    case "$ms" in
      *Mach-O*|*ELF*) ;;                       # real object -> audit
      *) echo "   (skipped non-object archive member: $(basename "$m") [$ms])"
         continue ;;
    esac
    audit_object "$m"
    A_SCANNED=$((A_SCANNED + SCANNED)); A_SKIPPED=$((A_SKIPPED + SKIPPED))
    TOTAL_SCANNED=$((TOTAL_SCANNED + SCANNED))
    TOTAL_SKIPPED=$((TOTAL_SKIPPED + SKIPPED))
    if [ -n "$HITS" ]; then
      TOTAL_HIT_FILES=$((TOTAL_HIT_FILES + 1))
      ALL_HITS+="$HITS"$'\n'
    fi
  done
  [[ "$as" == *"ar archive"* ]] && rm -rf "${members[0]%/*}"
  SCANNED=$A_SCANNED; SKIPPED=$A_SKIPPED
}
TOTAL_SCANNED=0; TOTAL_SKIPPED=0; TOTAL_HIT_FILES=0; ALL_HITS=""

# --- positive control (anti-vacuity, spec §3 positive-control) ---------------
positive_control() {
  local tmp; tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  # control fixture: deliberate if-branch -> must contain b.cond/b
  cc -c -O0 "$CASES/branchy.c" -o "$tmp/branchy.o" || { echo "FATAL: fixture compile failed" >&2; exit 3; }
  local found
  found="$("$DISASM" -d "$tmp/branchy.o" | awk '
    BEGIN { re = ENVIRON["BANNED_RE"] }
    /^[ \t]*[0-9a-f]+:/ {
      if (match($0, /\t[ \t]*[a-z][a-z0-9.]*/)) {
        m=substr($0, RSTART+1); sub(/^[ \t]+/, "", m); sub(/[ \t].*$/, "", m)
        if (m ~ re) { print; n++ }
      }
    } END { exit !(n>0) }')"
  if [ -z "$found" ]; then
    echo "VACUOUS GATE: positive control found 0 banned opcodes in branchy fixture" >&2
    return 2
  fi
  echo "== positive control: FIRED ($(echo "$found" | wc -l | tr -d ' ') banned ops in branchy.o) — gate is not vacuous"
  return 0
}

# --- scope demo: clean straight-line fixture must produce zero hits ----------
scope_demo() {
  local tmp; tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  cc -c -O2 "$CASES/branchless.c" -o "$tmp/branchless.o" || { echo "FATAL: fixture compile failed" >&2; exit 3; }
  local hits
  hits="$("$DISASM" -d "$tmp/branchless.o" | awk '
    BEGIN { re = ENVIRON["BANNED_RE"] }
    /^[ \t]*[0-9a-f]+:/ {
      if (match($0, /\t[ \t]*[a-z][a-z0-9.]*/)) {
        m=substr($0, RSTART+1); sub(/^[ \t]+/, "", m); sub(/[ \t].*$/, "", m)
        if (m ~ re) print
      }
    }')"
  if [ -n "$hits" ]; then
    echo "SCOPE-DEMO FAIL: clean fixture produced hits:" >&2; echo "$hits" >&2
    return 1
  fi
  echo "== scope demo: clean branchless fixture -> 0 hits"
}

# --- selftest ----------------------------------------------------------------
if [ "${1:-}" = "--selftest" ]; then
  echo "== SELFTEST"
  positive_control || { rc=$?; [ $rc -eq 2 ] && exit 2; exit 3; }
  scope_demo || exit 1
  echo "== SELFTEST PASS"
  exit 0
fi

positive_control || { rc=$?; [ $rc -eq 2 ] && exit 2; exit 3; }

# --- artifact selection ------------------------------------------------------
TMPDIR_GATE="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_GATE"' EXIT
ARTIFACTS=()
if [ $# -gt 0 ]; then
  ARTIFACTS=("$@")
else
  echo "== building avec-arrow, avec-blake3 (--profile audit: lto=false; the workspace release
   profile is thin-LTO and emits bitcode-only rlibs, which objdump cannot disassemble)
 into $TMPDIR_GATE/target"
  (cd "$REPO" && CARGO_TARGET_DIR="$TMPDIR_GATE/target" cargo build --profile audit -p avec-arrow -p avec-blake3) \
    || { echo "FATAL: cargo build failed" >&2; exit 3; }
  while IFS= read -r f; do ARTIFACTS+=("$f"); done \
    < <(find "$TMPDIR_GATE/target/audit" -maxdepth 2 -name 'libavec_*.rlib' 2>/dev/null)
  [ ${#ARTIFACTS[@]} -gt 0 ] || { echo "FATAL: no artifacts produced by cargo build" >&2; exit 3; }
fi

echo "== scope regex: ($SCOPE_RE)  (unanchored; audit limited to hot-kernel candidate symbols; see header)"
echo "== auditing ${#ARTIFACTS[@]} artifact(s)"
rc=0
for f in "${ARTIFACTS[@]}"; do
  audit_artifact "$f" || rc=1
  echo "   $f: scanned=$SCANNED skipped(out-of-scope)=$SKIPPED hits=$(echo -n "$HITS" | grep -c . || true)"
  [ -n "$HITS" ] && ALL_HITS+="$HITS"$'\n'
done

echo "== total: scanned_instructions=$TOTAL_SCANNED skipped_out_of_scope=$TOTAL_SKIPPED artifacts_with_hits=$TOTAL_HIT_FILES"
if [ "$TOTAL_SCANNED" -eq 0 ]; then
  echo "FATAL: 0 in-scope instructions audited across all artifacts — cannot claim pass" >&2
  exit 3
fi
if [ -n "$ALL_HITS" ]; then
  [ -f "$MANIFEST" ] || { echo "FATAL: manifest missing: $MANIFEST" >&2; exit 3; }
  echo "== hits present; adjudicating against manifest: $MANIFEST"

  # per-symbol tally + observed mnemonic set from ALL_HITS lines:
  #   <file>:<symbol> :: <instruction text>
  SYMBOL_ROWS="$(printf '%s\n' "$ALL_HITS" | awk '
    / :: / {
      head=$0; sub(/ :: .*$/, "", head)
      sym=head; sub(/^.*:[^:]*:/, "", sym); sub(/^_/, "", sym)
      rest=$0; sub(/^.* :: /, "", rest)
      mn="" ; if (match(rest, /\t[a-z][a-z0-9.]*/)) { mn=substr(rest, RSTART+1); sub(/[ \t].*$/, "", mn) }
      cnt[sym]++; if (index(mnset[sym], " " mn " ") == 0) mnset[sym] = mnset[sym] " " mn " "
    }
    END { for (s in cnt) printf "%s\t%d\t%s\n", s, cnt[s], mnset[s] }
  ')"
  [ -n "$SYMBOL_ROWS" ] || SYMBOL_ROWS=""

  # manifest records: name<TAB>scope_pattern<TAB>max<TAB>mode<TAB>mnemonics(space-sep)
  MANIFEST_ROWS="$(awk '
    /^\[\[kernel\]\]/ { if (name != "") emit(); name=""; scope=""; max=""; mode=""; mn="" }
    /^name[ \t]*=/         { name=$0;  sub(/^[^=]*=[ \t]*/, "", name);  gsub(/"/, "", name) }
    /^scope_pattern[ \t]*=/{ scope=$0; sub(/^[^=]*=[ \t]*/, "", scope); gsub(/"/, "", scope) }
    /^expected_max_count[ \t]*=/ { max=$0;  sub(/^[^=]*=[ \t]*/, "", max);  gsub(/[ \t]/, "", max) }
    /^gate_mode[ \t]*=/    { mode=$0; sub(/^[^=]*=[ \t]*/, "", mode); gsub(/"/, "", mode) }
    /^observed_mnemonics[ \t]*=/ { line=$0; sub(/^[^=]*=[ \t]*\[/, "", line); sub(/\].*$/, "", line); gsub(/[" \t]/, "", line); gsub(/,/, " ", line); mn=line }
    function emit() { printf "%s\t%s\t%s\t%s\t%s\n", name, scope, max, mode, mn }
    END { if (name != "") emit() }
  ' "$MANIFEST")"
  [ -n "$MANIFEST_ROWS" ] || { echo "FATAL: manifest parsed to zero kernels: $MANIFEST" >&2; exit 3; }

  MANIFEST_FAIL=0
  while IFS=$'\t' read -r sym count mns; do
    [ -n "$sym" ] || continue
    verdict=""
    while IFS=$'\t' read -r kname kscope kmax kmode kmn; do
      [ -n "$kname" ] || continue
      if printf '%s' "$sym" | grep -Eq "($kscope)"; then
        ok_max=0; [ "$count" -le "$kmax" ] && ok_max=1
        ok_mn=1; for m in $mns; do case " $kmn " in *" $m "*) ;; *) ok_mn=0 ;; esac; done
        verdict="claimed kernel=\"$kname\""
        if [ $ok_max -eq 1 ] && [ $ok_mn -eq 1 ]; then
          echo "MANIFEST-OK  sym=$sym count=$count mnemonics:[$mns ] kernel=\"$kname\" (count<=$kmax, mnemonics accepted) gate_mode=$kmode"
          if [ "$kmode" != "ENFORCED" ] && [ "$kmode" != "INFORMATIONAL" ]; then
            echo "GATE FAIL  sym=$sym — manifest gate_mode '$kmode' is not ENFORCED|INFORMATIONAL"; MANIFEST_FAIL=1
          fi
        else
          echo "GATE FAIL  sym=$sym count=$count mnemonics:[$mns ] kernel=\"$kname\" — count>$kmax or unaccepted mnemonic (mode=$kmode)"
          MANIFEST_FAIL=1
        fi
        break
      fi
    done <<EOF2
$MANIFEST_ROWS
EOF2
    if [ -z "$verdict" ]; then
      echo "GATE FAIL  sym=$sym count=$count mnemonics:[$mns ] — no manifest kernel claims this symbol (unlisted symbol)"
      MANIFEST_FAIL=1
    fi
  done <<EOF3
$SYMBOL_ROWS
EOF3

  if [ "$MANIFEST_FAIL" -ne 0 ]; then
    echo "GATE FAIL — unexplained banned opcodes inside scoped symbols (full listing):"
    echo "$ALL_HITS"
    exit 1
  fi
  echo "GATE PASS (all hits adjudicated MANIFEST-OK; zero unexplained hits)"
  exit 0
fi
echo "GATE PASS"
exit 0
