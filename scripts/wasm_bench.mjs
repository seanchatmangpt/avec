// scripts/wasm_bench.mjs — OPT20-L13 wasm mask throughput bench.
//
// Times the OPTIMIZED footprint artifact's `wasm_mask_words` C ABI export
// (crates/wasm-footprint, the same bytes the node execution court in
// scripts/footprint.sh validates) over a 4M-lane buffer.
//
// Layout law (documented, matches the contract): a lane is one u64 word, so
// n = 4,000,000 u64 lanes per buffer = 32,000,000 B per buffer; the kernel
// reads valid (32 MB) + pred (32 MB) and writes out (32 MB), so bytes moved
// per timed call = 3 buffers x 4,000,000 x 8 B = 96,000,000 B.
// (`wasm_mask_words(valid, pred, out, n)` slices each pointer to `n` u64s —
// crates/wasm-footprint/src/lib.rs `u64_slice`.)
//
// Method: instantiate the artifact, grow the module's exported memory, bump-
// allocate three 64-byte-aligned buffers (the avec alignment invariant), fill
// valid/pred with deterministic xorshift64* patterns, poison `out`, run 2
// untimed warmup calls, time 5 calls (process.hrtime.bigint), report the MIN.
// Black-box-consumes the output: head/tail out-lanes are checked against the
// host-side AND of the actual valid/pred bytes in wasm memory and folded into
// a printed checksum, so the timed output is observed, not discarded.
//
// Fail-closed: missing artifact, failed instantiation, failed memory.grow, a
// kernel trap, or a failed head/tail consumption check exits 2. Prints
// `wasm mask: <X> GiB/s` on success only.

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DEFAULT_ARTIFACT = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    '..',
    'crates', 'wasm-footprint', 'target', 'wasm32-wasip1', 'release',
    'wasm_footprint.opt.wasm');

const artifactPath = process.argv[2] ?? DEFAULT_ARTIFACT;

function fail(msg) {
    console.error(`wasm bench FAIL: ${msg}`);
    process.exit(2);
}

// --- Sizing (documented accounting) ----------------------------------------
const WORDS = 4_000_000;                 // u64 lanes per buffer (n parameter)
const BUF_BYTES = WORDS * 8;             // 32,000,000 B per buffer
const BYTES_MOVED = 3 * BUF_BYTES;       // valid + pred reads, out write
const WARMUP = 2;
const RUNS = 5;
const MASK64 = (1n << 64n) - 1n;
const POISON = 0xDEADBEEFCAFEF00Dn;
const ALIGN = 64;                        // avec buffer alignment invariant

console.log('== wasm bench: wasm_mask_words on the optimized footprint artifact ==');
console.log(`artifact: ${artifactPath}`);

// --- Load + instantiate (fail-closed) --------------------------------------
let bytes;
try {
    bytes = readFileSync(artifactPath);
} catch (err) {
    fail(`artifact unreadable: ${artifactPath} — ${err.message}`);
}
console.log(`artifact bytes: ${bytes.length}`);

let e;
try {
    e = new WebAssembly.Instance(new WebAssembly.Module(bytes), {}).exports;
} catch (err) {
    fail(`instantiation of ${artifactPath} failed: ${err}`);
}
if (typeof e.wasm_mask_words !== 'function') {
    fail('export missing: wasm_mask_words (kernel-less artifact must refuse, not bench)');
}
if (!e.memory || typeof e.memory.grow !== 'function') {
    fail('export missing/bad: memory (bench bumps the module\'s own memory)');
}

// --- Scratch: grow the module's exported memory ----------------------------
const PAGES = Math.ceil((BYTES_MOVED + 65536) / 65536); // buffers + 1 page slack
let oldPages;
try {
    oldPages = e.memory.grow(PAGES);
} catch (err) {
    fail(`memory.grow(${PAGES}) failed: ${err}`);
}
if (!Number.isInteger(oldPages)) fail(`memory.grow returned ${oldPages}, not a page count`);
const bumpBase = oldPages * 65536;
let bump = bumpBase;
function alloc(bytes_) {
    const at = Math.ceil(bump / ALIGN) * ALIGN;
    bump = at + bytes_;
    if (bump > e.memory.buffer.byteLength) fail('bump allocation exceeded grown memory');
    return at;
}

// Views created AFTER grow (grow detaches the old buffer).
const u64 = new BigUint64Array(e.memory.buffer);

const vOff = alloc(BUF_BYTES);
const pOff = alloc(BUF_BYTES);
const oOff = alloc(BUF_BYTES);

// --- Deterministic fill (xorshift64*) --------------------------------------
function xorshift64star(seed) {
    let s = seed & MASK64;
    if (s === 0n) fail('seed must be nonzero');
    return () => {
        s ^= s >> 12n; s &= MASK64;
        s ^= s << 25n; s &= MASK64;
        s ^= s >> 27n; s &= MASK64;
        return (s * 2685821657736338717n) & MASK64;
    };
}
for (let i = 0, gen = xorshift64star(0x9E3779B97F4A7C15n); i < WORDS; i++) u64[(vOff >> 3) + i] = gen();
for (let i = 0, gen = xorshift64star(0xBF58476D1CE4E5B9n); i < WORDS; i++) u64[(pOff >> 3) + i] = gen();
for (let i = 0; i < WORDS; i++) u64[(oOff >> 3) + i] = POISON; // poison: consumption must see kernel bytes

// Ground truth read back from the actual wasm bytes (not recomputed patterns).
const wantHead = u64[vOff >> 3] & u64[pOff >> 3];
const wantTail = u64[(vOff >> 3) + WORDS - 1] & u64[(pOff >> 3) + WORDS - 1];

// --- Warmup + timed runs ----------------------------------------------------
function callMask() {
    // No arguments escape: valid/pred/out offsets + n all point at live memory.
    e.wasm_mask_words(vOff, pOff, oOff, WORDS);
}
for (let r = 0; r < WARMUP; r++) {
    try {
        callMask();
    } catch (err) {
        fail(`warmup call ${r + 1} trapped: ${err}`);
    }
}

const runsMs = [];
for (let r = 0; r < RUNS; r++) {
    let ns;
    try {
        const t0 = process.hrtime.bigint();
        callMask();
        const t1 = process.hrtime.bigint();
        ns = t1 - t0;
    } catch (err) {
        fail(`timed call ${r + 1} trapped: ${err}`);
    }
    runsMs.push(Number(ns) / 1e6);
}
const minMs = Math.min(...runsMs);
if (!(minMs > 0)) fail(`min run time is ${minMs} ms — unusable measurement`);

// --- Black-box consume the output ------------------------------------------
const gotHead = u64[oOff >> 3];
const gotTail = u64[(oOff >> 3) + WORDS - 1];
if (gotHead !== wantHead) {
    fail(`consumption check: out[0]=0x${gotHead.toString(16)} != valid[0]&pred[0]=0x${wantHead.toString(16)}`);
}
if (gotTail !== wantTail) {
    fail(`consumption check: out[n-1]=0x${gotTail.toString(16)} != valid[n-1]&pred[n-1]=0x${wantTail.toString(16)}`);
}
const folded = gotHead ^ u64[(oOff >> 3) + 1] ^ u64[(oOff >> 3) + WORDS - 2] ^ gotTail;

// --- Report -----------------------------------------------------------------
const gibps = BYTES_MOVED / (minMs / 1000) / (1024 ** 3);
console.log(`layout:     n = ${WORDS.toLocaleString('en-US')} u64 lanes/buffer x 8 B; 3 buffers (valid, pred, out)`);
console.log(`bytes moved: ${BYTES_MOVED.toLocaleString('en-US')} B per timed call (3 x ${BUF_BYTES.toLocaleString('en-US')} = 96 MB)`);
console.log(`runs:       ${WARMUP} warmup + min of ${RUNS} timed (hrtime.bigint)`);
console.log(`wasm mask: ${gibps.toFixed(2)} GiB/s`);
console.log(`runs_ms:    [${runsMs.map((m) => m.toFixed(3)).join(', ')}] (min ${minMs.toFixed(3)})`);
console.log(`consume:    out[0]=0x${gotHead.toString(16)} out[n-1]=0x${gotTail.toString(16)} folded=0x${folded.toString(16).padStart(16, '0')} (head+tail == valid&pred)`);
process.exit(0);
