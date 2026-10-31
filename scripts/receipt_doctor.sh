#!/usr/bin/env bash
# receipt_doctor.sh — receipt freshness scanner (wave-10 lane 8).
#
# Turns the one-shot snapshot in docs/jira/v26.10.31/64-receipts-inventory-final.md
# into a living tool: for every receipt under docs/jira/v26.10.31/receipts/ plus the
# top-level stray gate-branchless-manifest.md, it
#   1. parses the receipt's Command field (minter table row, prose form, or the
#      hand-written fallback),
#   2. maps the command to the files/dirs it exercises (heuristic table below —
#      EDIT HERE when a command family gains a surface),
#   3. classifies the receipt against the live tree:
#        CURRENT           no touched file is newer than the receipt's mtime
#        STALE-UNREMINTED  a touched file IS newer (premise moved, no successor)
#        REPLACED-BY       a same-base -vN successor (or registered cross-family
#                          successor) with a later mtime exists
#        UNREPLAYABLE      the minted command consumed a transient input
#                          (e.g. /tmp/) that no longer exists
#        UNMAPPED          the heuristic table has no rule for the command
#                          (never silently CURRENT — edit the table instead)
#      HAND-MINTED is an annotation, not a class: hand-written receipts are still
#      classified, but flagged so their refresh is not routed through the minter.
#   4. emits a classified table + a refresh worklist (STALE-UNREMINTED receipts
#      with their exact re-mint commands; names are suggested next-free -vN
#      because the minter is append-only and refuses existing names, exit 73).
#
# Clock: all comparisons use file mtimes (local clock, whole seconds) — the
# receipt file's own mtime is the mint instant; the Date field is display-only
# (parquet-throughput-baseline's Date is date-only, so it is not comparable).
#
# Exit codes: 0 clean; 65 no receipts found; 66 receipts dir missing;
# 1 heuristic table has UNMAPPED holes (loud, never a silent pass).
#
# Usage: scripts/receipt_doctor.sh [--repo PATH]
set -u

# ============================================================================
# EDITABLE HEURISTIC TABLE — edit here when commands/surfaces change.
# ============================================================================

REPO_DEFAULT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$REPO_DEFAULT"
RECEIPTS_DIR="$REPO/docs/jira/v26.10.31/receipts"
# Hand-written receipts recorded for relocation by the operator (inventory §5).
STRAY_RECEIPTS="docs/jira/v26.10.31/gate-branchless-manifest.md"
# Non-receipts in the receipts dir (minter convention docs, template).
NON_RECEIPTS="README.md template.md"

# Receipts minted outside scripts/mint_receipt.sh — annotated HAND-MINTED,
# still classified. Re-mint by hand re-run + hand re-write (new name), since
# the minter template cannot hold their substance.
HAND_MINTED="gate-cfg-homology parquet-throughput-baseline gate-branchless-manifest"

# Any token in this list found in a receipt's command => UNREPLAYABLE: the
# command consumed a transient input; replay requires re-capturing it first
# (for divan: run benches -> capture stdout -> divan2receipt -> mint).
UNREPLAYABLE_TOKENS="/tmp/"

# Cross-family supersession edges (same measured surface, different base name;
# inventory §3 chains 4 and 9). Format: receipt:successor pairs.
CROSS_FAMILY_SUCCESSOR="
gate-blake3-zero-alloc:gate-blake3-tests-v3
gate-e2e-ctq05:gate-e2e-integration-v3
"

# Commands with no file surface on purpose (demos) — exempt from UNMAPPED.
# One regex per line (entries contain spaces; whitespace splitting breaks them).
DEMO_COMMANDS='^cargo --version$
^false$'

# map_command_to_paths CMD  -> prints exercised paths, one per line.
# Empty output + non-demo command => UNMAPPED (the scanner exits 1).
# Order matters: bin-specific rules BEFORE the generic `cargo -p` clause.
map_command_to_paths() {
    local cmd="$1"
    case "$cmd" in
        *"validate_ontology"*)
            echo "$REPO/docs/jira/v26.10.31/19-ontology-ttl.md"
            echo "$REPO/scripts/validate_ontology.sh"
            ;;
        *"audit_branchless"*)
            echo "$REPO/scripts/audit_branchless.sh"
            echo "$REPO/scripts/branchless_manifest.toml"
            echo "$REPO/crates/avec-arrow/src"
            ;;
        *"cfg_homology"*)
            echo "$REPO/scripts/cfg_homology.sh"
            echo "$REPO/scripts/cfg_homology_cases"
            echo "$REPO/crates/avec-arrow/src"
            echo "$REPO/crates/avec-blake3/src"
            echo "$REPO/crates/avec-git/src"
            ;;
        *"divan2receipt"*)
            echo "$REPO/scripts/divan2receipt.py"
            ;;
        *"perf_guard"*)
            echo "$REPO/scripts/perf_guard.sh"
            ;;
        *"blake3_throughput"*)
            echo "$REPO/crates/bench"
            echo "$REPO/crates/avec-blake3/src"
            ;;
        *"ctq_report"*|*"cpk_study"*)
            # CTQ-03 parquet roundtrip, CTQ-05 admission pipeline, CTQ-06
            # blake3, CTQ-04 transpose (crates/bench/src/bin headers).
            echo "$REPO/crates/bench"
            echo "$REPO/crates/avec-arrow/src"
            echo "$REPO/crates/avec-blake3/src"
            echo "$REPO/crates/avec-parquet/src"
            echo "$REPO/crates/integration/src"
            echo "$REPO/crates/integration/tests"
            ;;
        *"gage_rr"*)
            echo "$REPO/crates/bench"
            echo "$REPO/crates/avec-blake3/src"
            ;;
        *"parquet_throughput"*)
            echo "$REPO/crates/bench"
            echo "$REPO/crates/avec-parquet/src"
            ;;
        *"verify_v26"*)
            echo "$REPO/crates/cli/src"
            ;;
        cargo\ --version|false)
            return 0
            ;;
        *--workspace*)
            # cargo check --workspace compiles every member: manifests + sources.
            echo "$REPO/Cargo.toml"
            for d in "$REPO"/crates/*/; do
                [ -f "${d}Cargo.toml" ] && echo "${d}Cargo.toml"
                [ -d "${d}src" ] && echo "${d}src"
                [ -d "${d}tests" ] && echo "${d}tests"
            done
            ;;
        *" -p "*)
            # Generic cargo test/check/run/build -p CRATE: that crate's surface.
            # Crate NAME -> dir via the workspace map (avec-integration lives in
            # crates/integration, avec-cli in crates/cli, bench in crates/bench).
            local crate cdir
            crate="$(printf '%s' "$cmd" | sed -E 's/.* -p ([^ ]+).*/\1/')"
            cdir="$(crate_dir "$crate")"
            [ -z "$cdir" ] && cdir="$REPO/crates/$crate"
            echo "$cdir/Cargo.toml"
            echo "$cdir/src"
            echo "$cdir/tests"
            ;;
    esac
}

# ============================================================================
# Scanner internals (no edits needed below this line for table changes).
# ============================================================================

while [ $# -gt 0 ]; do
    case "$1" in
        --repo) REPO="$2"; shift 2 ;;
        *) { echo "usage: $0 [--repo PATH]" >&2; exit 64; } ;;
    esac
done
RECEIPTS_DIR="$REPO/docs/jira/v26.10.31/receipts"

[ -d "$RECEIPTS_DIR" ] || { echo "FATAL: no receipts dir: $RECEIPTS_DIR" >&2; exit 66; }

TMPDIR_RD="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_RD"' EXIT
INDEX="$TMPDIR_RD/index.tsv"          # name <TAB> epoch <TAB> relpath
REPORT="$TMPDIR_RD/report.tsv"        # name <TAB> class <TAB> standing <TAB> minted <TAB> detail <TAB> note
CRATE_MAP="$TMPDIR_RD/crates.tsv"     # workspace crate name <TAB> crates/ dir
: >"$INDEX"
: >"$REPORT"

epoch_of() { stat -f "%m" "$1" 2>/dev/null || echo 0; }

# Extract the minted command from a receipt file.
parse_command() {
    local f="$1" c=""
    # 1) minter table row: | Command | `cmd` |
    c="$(sed -n 's/^| Command | `\(.*\)` |[[:space:]]*$/\1/p' "$f" | head -1)"
    # 2) prose form: **Command**: `cmd`
    if [ -z "$c" ]; then
        c="$(sed -n 's/.*\*\*Command\*\*:[[:space:]]*`\([^`]*\)`.*/\1/p' "$f" | head -1)"
    fi
    # 3) hand-written fallback: first backticked span that looks like a command
    if [ -z "$c" ]; then
        c="$(grep -oE '`[^`]+`' "$f" | tr -d '`' | grep -E '^(bash|cargo|python3|scripts/|target/|\./|BRANCHLESS_SCOPE=)' | head -1)"
    fi
    printf '%s' "$c"
}

parse_standing() {
    local f="$1" s
    s="$(sed -n 's/^| Standing | \(.*\) |[[:space:]]*$/\1/p' "$f" | head -1)"
    [ -n "$s" ] || s="$(grep -oE '\b(ALIVE|FAILED|PARTIAL)\b' "$f" | head -1)"
    printf '%s' "${s:-UNKNOWN}"
}

parse_date() {
    local f="$1" d
    d="$(sed -n 's/^| Date | \(.*\) |[[:space:]]*$/\1/p' "$f" | head -1)"
    printf '%s' "$d"
}

base_of()   { printf '%s' "$1" | sed -E 's/-v[0-9]+$//'; }
version_of() {
    local v
    v="$(printf '%s' "$1" | sed -nE 's/^.*-v([0-9]+)$/\1/p')"
    printf '%s' "${v:-0}"
}

# is_hand_minted NAME -> 0/1 via membership in $HAND_MINTED
is_hand_minted() {
    local n
    for n in $HAND_MINTED; do [ "$n" = "$1" ] && return 0; done
    return 1
}

# is_demo_command CMD -> 0/1 via one-regex-per-line $DEMO_COMMANDS membership.
# IFS is applied to `read` only — never mutate global IFS inside a function;
# a leaked IFS silently breaks every later word-splitting loop (witnessed:
# cross-family successor scan dead after one demo lookup).
is_demo_command() {
    local re
    while IFS= read -r re; do
        [ -n "$re" ] || continue
        printf '%s' "$1" | grep -Eq -- "$re" && return 0
    done <<EOF
$DEMO_COMMANDS
EOF
    return 1
}

# Build a workspace crate-name -> directory map once (crate names are NOT
# always their directory names: -p avec-integration lives in crates/integration,
# -p avec-cli in crates/cli, -p bench in crates/bench).
build_crate_map() {
    : >"$CRATE_MAP"
    local d name
    for d in "$REPO"/crates/*/; do
        [ -f "${d}Cargo.toml" ] || continue
        name="$(sed -nE 's/^name = "([^"]+)"/\1/p' "${d}Cargo.toml" | head -1)"
        [ -n "$name" ] && printf '%s\t%s\n' "$name" "${d%/}" >>"$CRATE_MAP"
    done
}

# crate_dir CRATE-NAME -> prints the crate's directory path (or nothing).
crate_dir() {
    local hit
    hit="$(awk -F'\t' -v n="$1" '$1==n {print $2; exit}' "$CRATE_MAP")"
    printf '%s' "$hit"
}

# successor_of NAME EPOCH -> prints successor receipt name, or nothing.
successor_of() {
    local name="$1" epoch="$2" base ver n e bn vn best="" bestver=-1
    base="$(base_of "$name")"; ver="$(version_of "$name")"
    while IFS="$(printf '\t')" read -r n e _; do
        [ "$n" = "$name" ] && continue
        [ "$(base_of "$n")" = "$base" ] || continue
        vn="$(version_of "$n")"
        [ "$vn" -gt "$ver" ] || continue
        [ "$e" -gt "$epoch" ] || continue
        if [ "$vn" -gt "$bestver" ]; then best="$n"; bestver="$vn"; fi
    done <"$INDEX"
    if [ -z "$best" ]; then
        local pair s r
        for pair in $CROSS_FAMILY_SUCCESSOR; do
            s="${pair%%:*}"; r="${pair#*:}"
            [ "$s" = "$name" ] || continue
            e="$(awk -F'\t' -v n="$r" '$1==n {print $2}' "$INDEX")"
            [ -n "$e" ] && [ "$e" -gt "$epoch" ] && best="$r"
        done
    fi
    printf '%s' "$best"
}

# next_name_of NAME -> suggested next free receipt name (append-only minter).
next_name_of() {
    local name="$1" base ver maxvn=0 vn n
    base="$(base_of "$name")"; ver="$(version_of "$name")"
    while IFS="$(printf '\t')" read -r n _; do
        [ "$(base_of "$n")" = "$base" ] || continue
        vn="$(version_of "$n")"; [ "$vn" -gt "$maxvn" ] && maxvn="$vn"
    done <"$INDEX"
    if [ "$maxvn" -eq 0 ]; then printf '%s-v2' "$base"
    else printf '%s-v%d' "$base" "$((maxvn + 1))"; fi
}

# newest_under PATH -> prints "epoch<TAB>relpath" for the newest file under PATH.
newest_under() {
    local p="$1" out=""
    if [ -f "$p" ]; then
        printf '%s\t%s\n' "$(epoch_of "$p")" "${p#"$REPO"/}"
    elif [ -d "$p" ]; then
        find "$p" -type f -exec stat -f "%m%t%N" {} + 2>/dev/null \
            | sort -rn | head -1 \
            | while IFS="$(printf '\t')" read -r e fp; do printf '%s\t%s\n' "$e" "${fp#"$REPO"/}"; done
    fi
}

# ---------------------------------------------------------------------------
# Pass 1: index every receipt (receipts/ dir + strays).
# ---------------------------------------------------------------------------
idx_receipt() { # $1 = file path
    local p="$1" name
    name="$(basename "$p" .md)"
    printf '%s\t%s\t%s\n' "$name" "$(epoch_of "$p")" "${p#"$REPO"/}" >>"$INDEX"
}
for f in "$RECEIPTS_DIR"/*.md; do
    [ -f "$f" ] || continue
    b="$(basename "$f")"
    skip=0
    for n in $NON_RECEIPTS; do [ "$b" = "$n" ] && skip=1; done
    [ "$skip" -eq 0 ] && idx_receipt "$f"
done
for rel in $STRAY_RECEIPTS; do
    [ -f "$REPO/$rel" ] && idx_receipt "$REPO/$rel"
done

N_TOTAL="$(wc -l <"$INDEX" | tr -d ' ')"
[ "$N_TOTAL" -gt 0 ] || { echo "FATAL: zero receipts found in $RECEIPTS_DIR" >&2; exit 65; }
build_crate_map

# ---------------------------------------------------------------------------
# Pass 2: classify every receipt.
# ---------------------------------------------------------------------------
N_CURRENT=0; N_REPLACED=0; N_STALE=0; N_UNREPLAY=0; N_UNMAPPED=0; N_HAND=0

while IFS="$(printf '\t')" read -r name epoch relpath; do
    f="$REPO/$relpath"
    cmd="$(parse_command "$f")"
    standing="$(parse_standing "$f")"
    # Keep the standing token only: cells like "FAILED (exit 1)" or the long
    # hand-minted PARTIAL sentence break the table otherwise.
    standing="${standing%% *}"
    minted="$(parse_date "$f")"
    [ -n "$minted" ] || minted="$(date -u -r "$epoch" '+%Y-%m-%dT%H:%M:%SZ')"
    note=""

    if is_hand_minted "$name"; then note="HAND-MINTED"; N_HAND=$((N_HAND + 1)); fi
    case "$relpath" in
        docs/jira/v26.10.31/receipts/*) : ;;
        *) note="${note:+$note }STRAY(top-level)" ;;
    esac

    cls=""
    if printf '%s' "$cmd" | grep -qF -- "$UNREPLAYABLE_TOKENS"; then
        cls="UNREPLAYABLE"
        detail="transient input in command: $cmd"
        N_UNREPLAY=$((N_UNREPLAY + 1))
    else
        succ="$(successor_of "$name" "$epoch")"
        if [ -n "$succ" ]; then
            cls="REPLACED-BY"
            detail="$succ"
            N_REPLACED=$((N_REPLACED + 1))
        else
            newest_epoch=0; newest_file=""; found_any=0
            mapped=""
            mapped="$(map_command_to_paths "$cmd")"
            if [ -z "$mapped" ] && ! is_demo_command "$cmd"; then
                cls="UNMAPPED"
                detail="no heuristic for command: $cmd"
                N_UNMAPPED=$((N_UNMAPPED + 1))
            else
                while IFS= read -r p; do
                    [ -n "$p" ] || continue
                    line="$(newest_under "$p")"
                    [ -n "$line" ] || continue
                    found_any=1
                    e="${line%%$'\t'*}"
                    fp="${line#*$'\t'}"
                    if [ "$e" -gt "$newest_epoch" ]; then newest_epoch="$e"; newest_file="$fp"; fi
                done <<EOF
$mapped
EOF
                if [ "$found_any" -eq 0 ] && ! is_demo_command "$cmd"; then
                    # Anti-vacuity: a heuristic that resolves to zero existing
                    # paths must never read as CURRENT.
                    cls="UNMAPPED"
                    detail="heuristic paths all missing on disk: $cmd"
                    N_UNMAPPED=$((N_UNMAPPED + 1))
                elif [ "$newest_epoch" -gt "$epoch" ]; then
                    cls="STALE-UNREMINTED"
                    delta=$((newest_epoch - epoch))
                    detail="+${delta}s $newest_file"
                    N_STALE=$((N_STALE + 1))
                else
                    cls="CURRENT"
                    if [ -n "$newest_file" ]; then
                        detail="newest touched: $newest_file"
                    else
                        detail="no file surface"
                    fi
                    N_CURRENT=$((N_CURRENT + 1))
                fi
            fi
        fi
    fi
    if [ "$cls" = "CURRENT" ] && is_demo_command "$cmd"; then
        note="${note:+$note }demo(no-surface)"
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$cls" "$standing" "$minted" "$detail" "$note" >>"$REPORT"
done <"$INDEX"

# ---------------------------------------------------------------------------
# Pass 3: emit table, summary, worklist.
# ---------------------------------------------------------------------------
AS_OF="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "== receipt_doctor — $N_TOTAL receipts (as-of $AS_OF; mtimes compared, Date fields display-only) =="
echo
printf '%-32s %-17s %-10s %-21s %s\n' "RECEIPT" "CLASS" "STANDING" "MINTED (UTC)" "DETAIL / NOTE"
{
    sort -t"$(printf '\t')" -k1,1 "$REPORT"
} | while IFS="$(printf '\t')" read -r name cls standing minted detail note; do
    col_note="$note"
    [ -n "$col_note" ] && detail="$detail [$col_note]"
    printf '%-32s %-17s %-10s %-21s %s\n' "$name" "$cls" "$standing" "$minted" "$detail"
done

echo
echo "== summary =="
echo "CURRENT=$N_CURRENT REPLACED-BY=$N_REPLACED STALE-UNREMINTED=$N_STALE UNREPLAYABLE=$N_UNREPLAY UNMAPPED=$N_UNMAPPED HAND-MINTED(note)=$N_HAND"
[ $((N_CURRENT + N_REPLACED + N_STALE + N_UNREPLAY + N_UNMAPPED)) -eq "$N_TOTAL" ] \
    || { echo "FATAL: class counts ($((N_CURRENT + N_REPLACED + N_STALE + N_UNREPLAY + N_UNMAPPED))) != receipts ($N_TOTAL)" >&2; exit 70; }

echo
echo "== refresh worklist (STALE-UNREMINTED; run from repo root; minter is append-only — exit 73 on existing names) =="
if [ "$N_STALE" -eq 0 ]; then
    echo "(none — every chain terminates at a current-tree receipt)"
else
    sort -t"$(printf '\t')" -k1,1 "$REPORT" | while IFS="$(printf '\t')" read -r name cls standing minted detail note; do
        [ "$cls" = "STALE-UNREMINTED" ] || continue
        cmd="$(parse_command "$REPO/$(awk -F'\t' -v n="$name" '$1==n {print $3}' "$INDEX")")"
        hint=""
        case "$cmd" in
            *target/release*) hint="  # rebuild first: cargo build --release -p bench (bin predates source edits)" ;;
        esac
        printf 'bash scripts/mint_receipt.sh %s %s%s\n' "$(next_name_of "$name")" "$cmd" "$hint"
    done
    echo
    echo "HAND-MINTED stale rows (re-run by hand + hand re-write under a new name; minter template cannot hold them):"
    sort -t"$(printf '\t')" -k1,1 "$REPORT" | while IFS="$(printf '\t')" read -r name cls standing minted detail note; do
        case "$cls $note" in "STALE-UNREMINTED *HAND-MINTED"*|*"HAND-MINTED"*) [ "$cls" = "STALE-UNREMINTED" ] || continue
            cmd="$(parse_command "$REPO/$(awk -F'\t' -v n="$name" '$1==n {print $3}' "$INDEX")")"
            printf '  %s -> hand re-run: %s\n' "$(next_name_of "$name")" "$cmd"
        ;; esac
    done
fi

if [ "$N_UNREPLAY" -gt 0 ]; then
    echo
    echo "== UNREPLAYABLE (excluded from worklist; replay needs the transient input re-captured first) =="
    awk -F'\t' '$2=="UNREPLAYABLE" {printf "  %s — %s\n", $1, $5}' "$REPORT" | sort
    echo "divan2receipt re-capture path: cargo bench -p bench --bench v26_10_31 -- --color never > /tmp/divan.txt; python3 scripts/divan2receipt.py /tmp/divan.txt; then mint (receipts/README.md §divan2receipt)."
fi

# Fail loudly if the heuristic table has holes.
[ "$N_UNMAPPED" -eq 0 ] || exit 1
exit 0
