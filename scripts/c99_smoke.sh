#!/bin/sh
# scripts/c99_smoke.sh — REL-L2 (v26.10.29): build + run the freestanding C99
# consumer against the ggen-generated avec headers. Header-only: no avec
# library is linked; the headers are the whole product. Exit 0 only on a
# warning-clean compile AND a fully passing run.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SRC="$ROOT/examples/c99_consumer/main.c"
OUT="$ROOT/examples/c99_consumer/c99_consumer_bin"

test -f "$SRC"
test -f "$ROOT/include/avec_c_data.h"
test -f "$ROOT/include/avec_format_codes.h"

# Contract flags: C99, freestanding, -Wall -Wextra -pedantic, -I include.
cc -std=c99 -ffreestanding -Wall -Wextra -pedantic -I "$ROOT/include" \
  -o "$OUT" "$SRC"

"$OUT"
