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
# and exports over a C ABI: wasm_version (NUL-terminated version string,
# single-sourced from the crate manifest), wasm_mask_words
# (avec_arrow::bitmap), wasm_unpack_1bit + wasm_rle_decode (avec_parquet).
# Because the exports reference kernel code, every retained byte after
# wasm-opt -Oz is code a real wasm consumer linking these kernels would
# carry — no whole-archive relink tricks (the previous harness was retired;
# dep rlib archive members are pulled lazily, so a glue-only cdylib measures
# ~213 bytes of nothing).
#
# Courts (fail-closed, in order):
#   1. export-surface court  — wasm-objdump over the raw cdylib: every
#      required symbol must be present; a kernel-less shell must FAIL, not
#      pass on size alone.
#   2. node execution court  — instantiates the OPTIMIZED artifact in node's
#      WebAssembly runtime and asserts on real returned bytes: the three
#      kernels on concrete vectors (AND mask, 1-bit unpack incl. a
#      word-crossing run, RLE repeated + literal runs, RLE typed error) plus
#      wasm_version byte-exact (incl. NUL) against the manifest version.
#   3. shim court            — external (not run by this script): rename an
#      export in crates/wasm-footprint/src/lib.rs and this gate must exit 2;
#      restore byte-exact and it must pass again.
#
# Fail-closed: missing tooling, a failed build, a missing export, or a failed
# execution assertion aborts non-zero (2 for courts, 1 for the size USL).
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
command -v node >/dev/null 2>&1 || fail "node not on PATH (node execution court requires it)"
[ -f "$WASM_CRATE/Cargo.toml" ] || fail "crate manifest missing: $WASM_CRATE/Cargo.toml"
[ -f "$WASM_CRATE/src/lib.rs" ] || fail "crate source missing: $WASM_CRATE/src/lib.rs"

echo "== avec WASM footprint gate (crates/wasm-footprint) =="
echo "rustc:    $(rustc --version)"
echo "cargo:    $(cargo --version)"
echo "wasm-opt: $("$OPT_BIN" --version)"
echo "node:     $(node --version)"
echo "target:   $TARGET"
echo "subject:  crates/wasm-footprint (standalone manifest; NOT a root-workspace member)"

# Expected version: derived from the crate manifest (version law 26.10.29).
# The node court asserts the artifact's wasm_version string equals this —
# so a lib.rs/manifest version drift fails the gate rather than passing.
EXPECTED_VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$WASM_CRATE/Cargo.toml" | head -n 1)"
printf '%s' "$EXPECTED_VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
    || fail "crate manifest version not x.y.z: '$EXPECTED_VERSION'"
echo "version:  $EXPECTED_VERSION (expected from manifest)"

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
# Court 1: export-surface (non-vacuity). Size alone cannot distinguish a real
# artifact from a mis-built empty shell (~213 bytes of nothing); a missing
# export means the kernels (or the version surface) are gone and the gate
# must refuse.
# ---------------------------------------------------------------------------
echo "-- export-surface court..."
for sym in rust_eh_personality wasm_mask_words wasm_unpack_1bit wasm_rle_decode wasm_version; do
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

# ---------------------------------------------------------------------------
# Court 2: node execution (real execution on the optimized artifact — the
# same bytes the size verdict below counts). The court script instantiates
# the module, grows the module's own memory for scratch (the cdylib exports
# memory but no allocator), calls the exports, and asserts on returned
# bytes. First failed assertion exits 2 (fail-closed).
# ---------------------------------------------------------------------------
echo "-- node execution court (optimized artifact, real execution)..."
NODE_COURT_JS="$(mktemp "${TMPDIR:-/tmp}/avec_node_court.XXXXXX.js")"
cleanup() { rm -f "$NODE_COURT_JS"; }
trap cleanup EXIT INT TERM
cat > "$NODE_COURT_JS" <<'NODE_EOF'
'use strict';
// avec node execution court: instantiate the optimized wasm footprint
// artifact and assert on real returned bytes. Exit 0 all pass; exit 2 on
// the first failure.
const fs = require('fs');

const [artifactPath, expectedVersion] = process.argv.slice(2);
if (!artifactPath || !expectedVersion) {
    console.error('   node court FAIL: usage: node_court.js <artifact.wasm> <expected-version>');
    process.exit(2);
}
if (!/^[0-9]+\.[0-9]+\.[0-9]+$/.test(expectedVersion)) {
    console.error(`   node court FAIL: expected version not x.y.z: "${expectedVersion}"`);
    process.exit(2);
}
function fail(msg) {
    console.error(`   node court FAIL: ${msg}`);
    process.exit(2);
}
let checks = 0;
function check(name, ok, detail) {
    if (!ok) fail(`${name}${detail ? ' — ' + detail : ''}`);
    checks += 1;
    console.log(`   node court: ${name}${detail ? ' — ' + detail : ''}`);
}

let e;
try {
    e = new WebAssembly.Instance(
        new WebAssembly.Module(fs.readFileSync(artifactPath)), {}
    ).exports;
} catch (err) {
    fail(`instantiation of ${artifactPath} failed: ${err}`);
}

// Export presence on the INSTANTIATED module (complements the objdump court:
// a name can survive while the function traps or the memory export is gone).
for (const name of ['memory', 'rust_eh_personality', 'wasm_mask_words',
                    'wasm_unpack_1bit', 'wasm_rle_decode', 'wasm_version']) {
    check(`export instantiated: ${name}`, typeof e[name] !== 'undefined');
}

// Scratch: the cdylib exports memory but no allocator, so the court grows
// the module's own memory and bumps-allocates in the fresh region. Views are
// created after grow (grow detaches the old buffer).
const PAGES = 16; // 1 MiB scratch
let oldPages;
try { oldPages = e.memory.grow(PAGES); }
catch (err) { fail(`memory.grow failed: ${err}`); }
if (!Number.isInteger(oldPages)) fail(`memory.grow returned ${oldPages}, not a page count`);
const bumpBase = oldPages * 65536;
let bump = bumpBase;
function alloc(bytes, align = 8) {
    const at = Math.ceil(bump / align) * align;
    bump = at + bytes;
    return at;
}
const u8 = new Uint8Array(e.memory.buffer);
const u32 = new Uint32Array(e.memory.buffer);
const u64 = new BigUint64Array(e.memory.buffer);
const POISON = 0xDEADBEEF; // Number: Uint32Array views reject BigInt writes.

// --- rust_eh_personality: callable without trapping.
try { e.rust_eh_personality(); }
catch (err) { fail(`rust_eh_personality trapped: ${err}`); }
check('rust_eh_personality callable (no trap)', true);

// --- wasm_mask_words: out = valid & pred, wordwise.
{
    const offV = alloc(8), offP = alloc(8), offO = alloc(8), offO2 = alloc(8);
    const V = 0xAA55AA55AA55AA55n, P = 0xFF00FF00FF00FF00n;
    u64[offV >> 3] = V;
    u64[offP >> 3] = P;
    u64[offO >> 3] = BigInt(POISON);
    e.wasm_mask_words(offV, offP, offO, 64);
    check('wasm_mask_words n=64 lanes',
        u64[offO >> 3] === (V & P),
        `got 0x${u64[offO >> 3].toString(16)} want 0x${(V & P).toString(16)}`);
    u64[offO2 >> 3] = BigInt(POISON);
    e.wasm_mask_words(offV, offP, offO2, 0);
    check('wasm_mask_words n=0 is a no-op', u64[offO2 >> 3] === BigInt(POISON),
        'poison lane untouched');
}

// --- wasm_unpack_1bit: packed bit b of word w -> lane 64w+b.
{
    const srcOff = alloc(8);
    u64[srcOff >> 3] = 11n; // 0b1011
    const outOff = alloc(4 * 8);
    for (let i = 0; i < 4; i++) u64[(outOff >> 3) + i] = BigInt(POISON);
    e.wasm_unpack_1bit(srcOff, 1, outOff, 4);
    const want = [1n, 1n, 0n, 1n];
    const got = [0, 1, 2, 3].map(i => u64[(outOff >> 3) + i]);
    check('wasm_unpack_1bit 4 lanes of 0b1011', want.every((w, i) => got[i] === w),
        `got [${got}] want [${want}]`);

    // Word-crossing run: src words [2, 1], 130 out lanes — lanes 128..129
    // come from the zero fill past src_len (driver contract: packed=0).
    const s2 = alloc(16);
    u64[s2 >> 3] = 2n;
    u64[(s2 >> 3) + 1] = 1n;
    const o2 = alloc(130 * 8);
    e.wasm_unpack_1bit(s2, 2, o2, 130);
    let ok = true;
    for (let i = 0; i < 130; i++) {
        const w = i < 64 ? 2n : (i < 128 ? 1n : 0n);
        if (u64[(o2 >> 3) + i] !== ((w >> BigInt(i & 63)) & 1n)) { ok = false; break; }
    }
    check('wasm_unpack_1bit 130 lanes across word boundary', ok,
        'lanes 0..63 <- word0=2, 64..127 <- word1=1, 128..129 <- zero fill');
}

// --- wasm_rle_decode: repeated run, literal run, typed error.
{
    // Repeated run: bits=8, header=varint(5<<1)=0x0A, value=0x2A -> 5x42.
    const srcOff = alloc(3);
    u8[srcOff] = 0x08; u8[srcOff + 1] = 0x0A; u8[srcOff + 2] = 0x2A;
    const outOff = alloc(5 * 4);
    for (let i = 0; i < 5; i++) u32[(outOff >> 2) + i] = POISON;
    let ret = e.wasm_rle_decode(srcOff, 3, outOff, 5);
    let ok = ret === 0;
    for (let i = 0; ok && i < 5; i++) ok = u32[(outOff >> 2) + i] === 42;
    check('wasm_rle_decode repeated run', ok,
        `ret=${ret} lanes=${[0, 1, 2, 3, 4].map(i => u32[(outOff >> 2) + i])} want 5x42`);

    // Literal run: bits=8, groups=1 -> header=(1<<1)|1=0x03, 8 payload bytes
    // -> lanes [1..8].
    const src2 = alloc(10);
    const payload = [0x08, 0x03, 1, 2, 3, 4, 5, 6, 7, 8];
    payload.forEach((b, i) => { u8[src2 + i] = b; });
    const out2 = alloc(8 * 4);
    for (let i = 0; i < 8; i++) u32[(out2 >> 2) + i] = POISON;
    ret = e.wasm_rle_decode(src2, 10, out2, 8);
    ok = ret === 0;
    for (let i = 0; ok && i < 8; i++) ok = u32[(out2 >> 2) + i] === i + 1;
    check('wasm_rle_decode literal run', ok,
        `ret=${ret} lanes=${[0, 1, 2, 3, 4, 5, 6, 7].map(i => u32[(out2 >> 2) + i])} want [1..8]`);

    // Typed error: bit width 3 is unsupported -> status 1 (fail-closed ABI;
    // no panic crosses the C ABI) and out lanes stay untouched.
    const src3 = alloc(1);
    u8[src3] = 0x03;
    const out3 = alloc(4 * 4);
    for (let i = 0; i < 4; i++) u32[(out3 >> 2) + i] = POISON;
    ret = e.wasm_rle_decode(src3, 1, out3, 4);
    ok = ret === 1;
    for (let i = 0; ok && i < 4; i++) ok = u32[(out3 >> 2) + i] === POISON;
    check('wasm_rle_decode typed error (bit width 3)', ok,
        `ret=${ret} want 1; out lanes still poisoned=${[0, 1, 2, 3].map(i => u32[(out3 >> 2) + i].toString(16))}`);
}

// --- wasm_version: NUL-terminated, byte-exact vs the manifest version.
{
    const ptr = e.wasm_version();
    if (typeof ptr !== 'number' || !Number.isInteger(ptr) || ptr <= 0) {
        fail(`wasm_version returned ${ptr}, expected a positive i32 pointer`);
    }
    let nul = -1;
    for (let i = 0; i < 64; i++) {
        if (u8[ptr + i] === 0) { nul = i; break; }
    }
    if (nul <= 0) fail(`wasm_version string empty or unterminated within 64 bytes (nul at ${nul})`);
    const got = Buffer.from(u8.slice(ptr, ptr + nul + 1));
    const want = Buffer.from(expectedVersion + '\0', 'ascii');
    check('wasm_version byte-exact (incl NUL)', got.equals(want),
        `bytes ${[...got].join(' ')} == "${expectedVersion}\\0" at ptr ${ptr}`);
}

console.log(`   node court: ${checks} checks passed on ${artifactPath}`);
process.exit(0);
NODE_EOF
node "$NODE_COURT_JS" "$OPT" "$EXPECTED_VERSION" \
    || fail "node execution court refused (artifact mis-executes or version mismatch)"
cleanup

# ---------------------------------------------------------------------------
# Size verdicts (charter CTQ-4).
# ---------------------------------------------------------------------------
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
echo "version proof:         wasm_version() -> \"$EXPECTED_VERSION\" (node court, byte-exact incl NUL)"
echo "courts:                export-surface (5 exports), node execution (15 checks), size (CTQ-4)"
if [ "$exit_code" -eq 0 ]; then
    echo "GATE: PASS"
else
    echo "GATE: FAIL"
fi
exit "$exit_code"
