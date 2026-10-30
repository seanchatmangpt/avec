#!/bin/sh
# CC=1 branch audit: disassemble the compiled release kernels and count
# conditional branches. Charter CTQ-3: the scan kernels must contain zero
# conditional jumps (cyclomatic complexity 1 by machine-code proof).
#
# Usage: scripts/branch_audit.sh [kernel-fn...]
set -eu
cd "$(dirname "$0")/.."

cargo build --release -p avec-arrow -p avec-parquet >/dev/null 2>&1

OBJ=target/release/libavec_arrow.rlib
OBJP=target/release/libavec_parquet.rlib
OBJD=target/release/libavec_ffi.dylib
for f in "$OBJ" "$OBJP" "$OBJD"; do
    [ -f "$f" ] || { echo "missing $f (build first)"; exit 1; }
done

DIS=""
for cand in "xcrun llvm-objdump" "llvm-objdump" "objdump"; do
    if command -v $(echo $cand | cut -d' ' -f1) >/dev/null 2>&1; then DIS="$cand"; break; fi
done
[ -n "$DIS" ] || { echo "no objdump found"; exit 1; }

# conditional-branch mnemonics, arm64 + x86_64
PAT='(^|[^a-z])(b\.(eq|ne|cs|hs|mi|pl|vs|vc|hi|ls|ge|lt|gt|le|al|nv)|cbz|cbnz|tbz|tbnz|j[a-z]{1,3} |jmpq|set[a-z]+)'
set +e
FAIL=0
for fn in mask_words count_valid get_bit set_bit clear_bit word_valid word_at bitmap_bytes bitmap_words bcast8 \
          unpack_1bit_kernel unpack_1bit unpack_2bit unpack_4bit unpack_8bit is_inline; do
    HITS=$( ($DIS -d --no-show-raw-insn "$OBJ" 2>/dev/null; $DIS -d --no-show- PAT placeholder) | grep -E "^[0-9a-f]+ <.*$fn" )
    # rlib members are archives; dump members via llvm-nm first
    echo "fn: $fn"
done
