#!/usr/bin/env bash
# cfg_homology.sh — DMADV Analyze gate: CFG homology PROXY
# Wave-5 lane 8 — docs/jira/v26.10.31/
#
# WHAT THIS IS:
#   A PROXY, NOT A PROOF. For each declared hot kernel it compares
#   (a) source-level decision constructs (textual keyword/operator counts)
#   against (b) emitted control-flow instructions (banned-branch opcodes,
#   same regex/family as scripts/audit_branchless.sh §2).
#
# LIMITS (loud, deliberate):
#   - Source side is a grep-class textual count, not a Rust parser: `if`
#     inside macro fragments, cfg_attr, generics bounds, or `?` operators
#     are handled only by textual coincidence; iterator combinators are
#     counted via an explicit .map(/.filter( heuristic; monomorphized
#     generics may emit MORE copies than the source describes.
#   - `#[cfg(test)]` region is excluded by cutting from a line matching
#     '^#[cfg(test)]' to EOF (conventional layout assumed).
#   - Emitted side counts only the banned-branch opcode family (jmp/jcc on
#     x86_64; b/b.cond/br/cbz/cbnz/tbz/tbnz on arm64). It does NOT see
#     cmov/csel/select lowering, so PASS means "no branching control flow
#     beyond declared slack", not "no compiler-injected control flow".
#   - Symbol matching is unanchored on the mangled-shaped name with the
#     leading mach-o underscore stripped (same convention as
#     scripts/audit_branchless.sh, whose objdump pipeline this reuses
#     without editing it).
#
# FAIL-CLOSED: missing source, missing artifact, build failure, kernel
#   symbol not found in disassembly, unparseable output -> exit 3.
#   Kernel emitted branches > source constructs + declared slack -> exit 1.
#
#   scripts/cfg_homology.sh --mutant
#       runs the mutant court (COURT-UNPROVEN #3): compiles the
#       scripts/cfg_homology_cases/ fixtures at -O0 (deterministic
#       one-branch-per-construct lowering) and re-runs the SAME homology
#       delta over them. The court FLIPS only if the clean fixture PASSES
#       within the tight slack AND the branchy fixture FAILS it; anything
#       else is a VACUOUS gate (exit 2).
#
# EXIT CODES: 0 pass | 1 gate violation | 2 vacuous mutant court | 3 fail-closed environment error
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
DISASM=""
for t in llvm-objdump objdump; do
  if command -v "$t" >/dev/null 2>&1; then DISASM="$t"; break; fi
done
[ -n "$DISASM" ] || { echo "FATAL: no llvm-objdump/objdump on PATH" >&2; exit 3; }

# banned-branch opcode regex — same family as scripts/audit_branchless.sh §2.
# TRANSPORT: exported and read inside awk via ENVIRON, never `awk -v
# banned=` — the -v assignment escape-decodes its value, so `b\.[a-z]+`
# arrived as `b.[a-z]+` and false-matched bic/blr (the REGEX_TRANSPORT_FP
# fixed in scripts/audit_branchless.sh:62-79; ENVIRON values reach awk
# byte-exact).
export BANNED_RE='^(jmp|je|jne|jg|jl|jge|jle|ja|jae|jb|jbe|js|jns|jo|jno|jp|jnp|jc|jnc|b|br|b\.[a-z]+|cbz|cbnz|tbz|tbnz)$'

# ----------------------------------------------------------------------------
# Kernel table: name : source file : symbol regex : slack
#   slack = tolerated (emitted_branches - source_constructs): compiler-
#   injected branching (bounds-check panics, unroll epilogues, panic
#   plumbing). Recorded per kernel; raising slack is a reviewed diff.
# ----------------------------------------------------------------------------
KERNELS=(
  "transpose:crates/avec-arrow/src/transpose.rs:transpose:4"
  "predicates/filter_quads_eq:crates/avec-arrow/src/predicates.rs:filter_quads_eq|ct_quad_eq_u32:4"
  "blake3/compress_chunks_4way:crates/avec-blake3/src/multi_chunk.rs:compress_chunks_4way:6"
  # NOTE: when the 4-way kernel is inlined into its caller (hash_wide_simd,
  # defined in lib.rs of the same crate), only the surviving compress_chunks_4way
  # symbols are audited here; the caller is OUT OF SCOPE for this kernel row
  # because its source constructs live in a different file. If the symbol
  # disappears entirely (fully inlined), this gate fails closed (exit 3).
  "avec-git/ladder:crates/avec-git/src/ladder.rs:ladder_search|ct_eq_oid:4"
)

# --- source-side construct count (documented textual proxy) ------------------
source_constructs() {
  local f="$1"
  [ -f "$REPO/$f" ] || { echo "FATAL: missing source $REPO/$f" >&2; exit 3; }
  awk '
    /^#\[cfg\(test\)\]/ { intests=1 }
    intests { next }
    { sub(/\/\/.*$/, "") }
    {
      gsub(/\/\*/, " "); gsub(/\*\//, " ")
      gsub(/"[^"]*"/, " ")
      n += gsub(/&&/, " ")
      n += gsub(/\|\|/, " ")
      n += gsub(/\.map\(/, " ")
      n += gsub(/\.filter\(/, " ")
      # keyword counting via tokenization (BSD awk has no word boundaries)
      l = toupper($0); c = split(l, tok, /[^A-Z0-9_]+/)
      for (i = 1; i <= c; i++) {
        if (tok[i] == "IF" || tok[i] == "MATCH" || tok[i] == "WHILE" ||
            tok[i] == "LOOP" || tok[i] == "FOR") n++
      }
    }
    END { print n + 0 }
  ' "$REPO/$f" || { echo "FATAL: unparseable source: $f" >&2; exit 3; }
}

# --- object side: per-symbol banned-branch counts (audit_branchless pipeline)
# prints "SYMBOL<TAB>COUNT" lines for symbols matching $1 across artifacts $2..
count_symbols() {
  local symre="$1"; shift
  local f m
  for f in "$@"; do
    [ -f "$f" ] || { echo "FATAL: missing artifact $f" >&2; exit 3; }
    local as; as="$(file -b "$f")"
    local tmp; tmp="$(mktemp -d)"
    if [[ "$as" == *"ar archive"* ]]; then
      (cd "$tmp" && ar -x "$f") || { echo "FATAL: cannot extract $f" >&2; rm -rf "$tmp"; exit 3; }
      for m in "$tmp"/*; do
        case "$(file -b "$m")" in
          *Mach-O*|*ELF*) disassemble_count "$m" "$symre" ;;
          *) : ;;
        esac
      done
    else
      disassemble_count "$f" "$symre"
    fi
    rm -rf "$tmp"
  done
}

DISASM_SECTION_BREAK="===CFGHOM_BREAK==="
disassemble_count() {
  local m="$1" symre="$2"
  # Xcode objdump emits <ltmp0> labels for many Mach-O rcgu members, so pure
  # label-based scoping (as in audit_branchless.sh) silently misses symbols.
  # Fail-closed alternative: attribute every instruction to its enclosing
  # symbol via the symbol table (-t): all __text symbols sorted by address,
  # current symbol = greatest start address <= instruction address; banned
  # branches are counted only while inside a symbol matching the kernel regex.
  local table dis
  table="$("$DISASM" -t "$m" 2>/dev/null)" || { echo "FATAL: unparseable symbol table: $m" >&2; exit 3; }
  [ -n "$table" ] || return 0
  dis="$("$DISASM" -d "$m" 2>/dev/null)" || { echo "FATAL: unparseable disassembly: $m" >&2; exit 3; }
  [ -n "$dis" ] || return 0
  printf '%s\n' "$table" "$DISASM_SECTION_BREAK" "$dis" | awk -v brk="$DISASM_SECTION_BREAK" \
      -v re="$symre" '
    BEGIN { banned = ENVIRON["BANNED_RE"] }
    function hexdec(h) { d=0; sub(/^0[xX]/, "", h)
      for (i = 1; i <= length(h); i++) {
        c = substr(h, i, 1)
        d = d * 16 + index("0123456789abcdef", tolower(c)) - 1
      }
      return d
    }
    $0 == brk { mode=1; next }
    !mode {
      if ($0 ~ /__TEXT,__text/ || $0 ~ /[ \t]\.text[ \t]/) {
        name=$NF; addr=hexdec($1)
        symaddr[++n]=addr; symname[n]=name
        bare=name
        iskern[n] = (bare ~ ("(" re ")")) || (bare ~ ("^_+(" re ")"))
      }
      next
    }
    mode {
      if ($1 ~ /^[0-9a-f]+:$/) {
        a1=$1; sub(/:$/, "", a1); a=hexdec(a1)
        while (cur+1 <= n && symaddr[cur+1] <= a) cur++
        if (cur < 1 || cur > n) next
        if (!iskern[cur]) next
        line=$0
        if (match(line, /\t[ \t]*[a-z][a-z0-9.]*/)) {
          mn=substr(line, RSTART+1); sub(/^[ \t]+/, "", mn); sub(/[ \t].*$/, "", mn)
          if (mn ~ banned) cnt[cur]++
        }
      }
      next
    }
    END { for (k in cnt) print symname[k] "\t" cnt[k]+0 }
  '
}

# --- mutant mode (COURT-UNPROVEN #3): witness the homology delta flipping ----
# Tight slack. At -O0 clang lowers each decision construct to 1 conditional
# branch + 2 straight-line plumbing jumps, so the clean fixture (2 constructs)
# has a deterministic delta floor of 4; slack=4 admits exactly that floor.
# The branchy fixture (5 constructs; iteration levels add conditional exits
# AND unconditional back-edges) lands near delta 12 and must exceed slack.
# Both cases run through the SAME source_constructs + count_symbols pipeline
# as normal mode — no separate logic to drift. Fixture headers avoid the
# counted keywords because the textual counter reads comment text too.
MUTANT_SLACK=4
MUTANT_CASES_DIR="$REPO/scripts/cfg_homology_cases"
mutant_court() {
  local mt mc cc="" case_name obj src symcounts emit delta
  local -A verdicts=()
  for mc in clean.c branchy.c; do
    [ -f "$MUTANT_CASES_DIR/$mc" ] || { echo "FATAL: missing fixture $MUTANT_CASES_DIR/$mc" >&2; exit 3; }
  done
  for mt in cc clang gcc; do
    if command -v "$mt" >/dev/null 2>&1; then cc="$mt"; break; fi
  done
  [ -n "$cc" ] || { echo "FATAL: no C compiler (cc/clang/gcc) on PATH" >&2; exit 3; }
  local mt_tmp; mt_tmp="$(mktemp -d)"
  trap 'rm -rf "$mt_tmp"' EXIT
  echo "== MUTANT COURT: homology delta over cfg_homology_cases fixtures (-O0, slack=$MUTANT_SLACK)"
  printf '%-28s %14s %10s %7s %7s  %s\n' KERNEL SOURCE-CONSTRUCTS EMITTED DELTA SLACK VERDICT
  for case_name in clean branchy; do
    obj="$mt_tmp/cfg_$case_name.o"
    "$cc" -O0 -c "$MUTANT_CASES_DIR/$case_name.c" -o "$obj" \
      || { echo "FATAL: $cc failed on $case_name.c" >&2; exit 3; }
    src="$(source_constructs "scripts/cfg_homology_cases/$case_name.c")"
    symcounts="$(count_symbols "homology_${case_name}_kernel" "$obj")"
    if [ -z "$symcounts" ]; then
      echo "FATAL: no symbol matching homology_${case_name}_kernel in $case_name.o" >&2
      exit 3
    fi
    emit=0
    while IFS=$'\t' read -r _s c; do emit=$((emit + c)); done <<< "$symcounts"
    delta=$((emit - src))
    verdicts[$case_name]=PASS
    if [ "$delta" -gt "$MUTANT_SLACK" ]; then verdicts[$case_name]=FAIL; fi
    printf '%-28s %14s %10s %7s %7s  %s\n' \
      "mutant/$case_name" "$src" "$emit" "$delta" "$MUTANT_SLACK" "${verdicts[$case_name]}"
  done
  if [ "${verdicts[clean]}" = PASS ] && [ "${verdicts[branchy]}" = FAIL ]; then
    echo "== MUTANT COURT FLIPPED: clean PASS && branchy FAIL — delta verdict is witnessed, not vacuous"
    exit 0
  fi
  echo "== VACUOUS GATE (exit 2): court did not flip (clean=${verdicts[clean]} branchy=${verdicts[branchy]})" >&2
  exit 2
}

# --- dispatch: mutant court never touches the normal-mode build path ---------
if [ "${1:-}" = "--mutant" ]; then mutant_court; fi

# --- build into temp target dir (repo tree undisturbed) ----------------------
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
echo "== PROXY Analyze gate: CFG homology (source constructs vs emitted branches)"
echo "== building avec-arrow, avec-blake3, avec-git --profile audit into $TMP/target"
(cd "$REPO" && CARGO_TARGET_DIR="$TMP/target" cargo build --profile audit \
   -p avec-arrow -p avec-blake3 -p avec-git) \
  || { echo "FATAL: cargo build failed" >&2; exit 3; }
ARTIFACTS=()
while IFS= read -r f; do ARTIFACTS+=("$f"); done \
  < <(find "$TMP/target/audit" -maxdepth 2 -name 'libavec_*.rlib' 2>/dev/null)
[ ${#ARTIFACTS[@]} -gt 0 ] || { echo "FATAL: no rlibs produced" >&2; exit 3; }
echo "== artifacts: ${#ARTIFACTS[@]} rlib(s); disassembler: $DISASM"

# --- homology table ----------------------------------------------------------
printf '%-28s %14s %10s %7s %7s  %s\n' KERNEL SOURCE-CONSTRUCTS EMITTED DELTA SLACK VERDICT
rc=0
for spec in "${KERNELS[@]}"; do
  IFS=':' read -r kname kfile ksym kslack <<< "$spec"
  src="$(source_constructs "$kfile")"
  symcounts="$(count_symbols "$ksym" "${ARTIFACTS[@]}")"
  if [ -z "$symcounts" ]; then
    echo "FATAL: no symbols matching ($ksym) in any artifact" >&2
    exit 3
  fi
  emit=0
  declare -A uniq=()
  # dedup: the same symbol can appear in both libX.rlib and deps/libX-*.rlib
  # (identical code). One observation per unique symbol name: keep the max.
  # NOTE: if the declared kernel fn was inlined, the regex matches its
  # containing symbol instead (e.g. blake3 4-way kernel inlined into
  # hash_wide_simd, defined in lib.rs) — the delta then carries the
  # container's extra constructs and PASS/FAIL is judged against slack.
  while IFS=$'\t' read -r s c; do
    if [ -z "${uniq[$s]:-}" ] || [ "$c" -gt "${uniq[$s]}" ]; then uniq[$s]="$c"; fi
  done <<< "$symcounts"
  for s in "${!uniq[@]}"; do
    emit=$((emit + uniq[$s]))
    echo "    [$kname] $s: ${uniq[$s]} emitted branch(es)"
  done
  delta=$((emit - src))
  verdict=PASS
  if [ "$delta" -gt "$kslack" ]; then verdict=FAIL; rc=1; fi
  printf '%-28s %14s %10s %7s %7s  %s\n' "$kname" "$src" "$emit" "$delta" "$kslack" "$verdict"
done

if [ "$rc" -eq 0 ]; then
  echo "== PROXY GATE PASS (deltas within declared slack; this is a proxy, not a proof)"
else
  echo "== PROXY GATE FAIL — see table; fix the kernel or amend slack as a reviewed diff"
fi
exit "$rc"
