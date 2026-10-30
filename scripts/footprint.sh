#!/bin/sh
# scripts/footprint.sh — wasm32-wasip1 footprint gate for avec.
#
# Charter CTQ-4: <= 100 KiB stripped (TARGET), 250 KiB USL.
#
# Subject: crates/wasm-footprint — a no_std cdylib wrapper that supplies the
# consumer-side link glue the measured libs deliberately omit:
#   - bump GlobalAlloc over a static 1 MiB, 64-byte-aligned array
#     (alignment honored by construction),
#   - #[panic_handler] = halt loop,
#   - stable-form `rust_eh_personality` stub (#[no_mangle] pub extern "C";
#     the #[lang] form is nightly-only),
# and exports three kernels over a C ABI: wasm_mask_words
# (avec_arrow::bitmap), wasm_unpack_1bit + wasm_rle_decode (avec_parquet).
# Because the exports reference kernel code, every retained byte after
# wasm-opt -Oz is code a real wasm consumer linking these kernels would
# carry — no whole-archive relink tricks (the previous harness was retired;
# dep rlib archive members are pulled lazily, so a glue-only cdylib measures
# ~213 bytes of nothing).
#
# Fail-closed: missing tooling, a failed build, or a missing export aborts
# non-zero. The export-surface court keeps the gate non-vacuous — an
# artifact that silently lost its kernels must FAIL, not pass on size alone.
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_CRATE="$ROOT/crates/wasm-footprint"
TARGET="wasm32-wasip1"
OPT_BIN="/opt/homebrew/bin/wasm-opt"
OBJDUMP_BIN="wasm-objdump"
TARGET_BYTES=102400   # 100 KiB charter target (CTQ-4)
USL_BYTES=256000      # 250 KiB USL

fail() {
    echo "FAIL: $*" >&2
    exit 2
}

command -v cargo >/dev/null 2>&1 || fail "cargo not on PATH"
command -v rustc >/dev/null 2>/dev/null || fail "rustc not on PATH"
[ -x "$OPT_BIN" ] || fail "wasm-opt not found/executable at $OPT_BIN"
command -v "$OBJDUMP_BIN" >/dev/null 2>&1 || fail "wasm-objdump not on PATH (export-surface court requires it)"
[ -f "$WASM_CRATE/Cargo.toml" ] || fail "crate manifest missing: $WASM_CRATE/Cargo.toml"
[ -f "$WASM_CRATE/src/lib.rs" ] || fail "crate source missing: $WASM_CRATE/src/lib.rs"

echo "== avec WASM footprint gate (crates/wasm-footprint) =="
echo "rustc:    $(rustc --version)"
echo "cargo:    $(cargo --version)"
echo "wasm-opt: $("$OPT_BIN" --version)"
echo "target:   $TARGET"
echo "subject:  crates/wasm-footprint (standalone manifest; NOT a root-workspace member)"

# ---------------------------------------------------------------------------
# Build the standalone crate. It deliberately carries its own empty
# [workspace] table (the root Cargo.toml is coordinator-owned), so this
# build uses a crate-local Cargo.lock and crate-local target/ directory.
# ---------------------------------------------------------------------------
echo "-- build (standalone manifest)..."
(cd "$WASM_CRATE" && cargo build --release --target "$TARGET")

WASM="$WASM_CRATE/target/$TARGET/release/wasm_footprint.wasm"
[ -f "$WASM" ] || fail "expected artifact not found: $WASM"

# ---------------------------------------------------------------------------
# Export-surface court (non-vacuity). Size alone cannot distinguish a real
# artifact from a mis-built empty shell (~213 bytes of nothing); a missing
# export means the kernels are gone and the gate must refuse.
# ---------------------------------------------------------------------------
echo "-- export-surface court..."
for sym in rust_eh_personality wasm_mask_words wasm_unpack_1bit wasm_rle_decode; do
    if ! "$OBJDUMP_BIN" -x "$WASM" | grep -q -- "\"$sym\""; then
        fail "export missing from artifact: $sym (gate refuses a kernel-less artifact)"
    fi
    echo "   export present: $sym"
done

# ---------------------------------------------------------------------------
# Optimize (no -g => name/debug sections dropped = stripped) and measure.
# ---------------------------------------------------------------------------
echo "-- wasm-opt -Oz..."
OPT="$WASM_CRATE/target/$TARGET/release/wasm_footprint.opt.wasm"
"$OPT_BIN" -Oz "$WASM" -o "$OPT"
[ -f "$OPT" ] || fail "wasm-opt produced no output: $OPT"

raw=$(wc -c < "$WASM" | tr -d ' ')
bytes=$(wc -c < "$OPT" | tr -d ' ')

target_verdict="FAIL (> $TARGET_BYTES bytes = 100 KiB charter target)"
[ "$bytes" -le "$TARGET_BYTES" ] && target_verdict="PASS (<= $TARGET_BYTES bytes = 100 KiB charter target)"
usl_verdict="FAIL (> $USL_BYTES bytes = 250 KiB USL)"
exit_code=1
if [ "$bytes" -le "$USL_BYTES" ]; then
    usl_verdict="PASS (<= $USL_BYTES bytes = 250 KiB USL)"
    exit_code=0
fi

echo ""
echo "== result =="
echo "artifact:              $OPT"
printf 'raw cdylib:            %s bytes\n' "$raw"
printf 'wasm-opt -Oz stripped: %s bytes\n' "$bytes"
echo "TARGET (100 KiB):      $target_verdict"
echo "USL (250 KiB):         $usl_verdict"
if [ "$exit_code" -eq 0 ]; then
    echo "GATE: PASS"
else
    echo "GATE: FAIL"
fi
exit "$exit_code"
