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
// allocate three 64-byte-aligned buffers (the cgap alignment invariant), fill
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
const ALIGN = 64;                        // cgap buffer alignment invariant

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

// ===========================================================================
// W=4 page-body decode: differential court + throughput bench.
// ===========================================================================
//
// Subject: wasm_decode_page_body_w4 — the W=4 constant-parameter
// specialization of cgap_parquet::rle::decode_into exported by the same
// artifact (crates/wasm-footprint/src/lib.rs): width byte admitted only if
// exactly 4, then the record loop with the width dispatch folded away
// (literals = 8 byte-per-lane words, lane k = byte k%8 & 0x0F; repeated
// runs = (count<<1)|0 with one LE value byte; run fill values are NOT
// masked — exact mirror of the general decoder's unmasked from_le_bytes
// fill).
//
// Differential chain (fail-closed, before any timing):
//   1. Fixture provenance: BODY_HEX/LANES_HEX below were minted by the
//      Rust court `w4_courts::w4_mints_js_fixture` in
//      crates/wasm-footprint/src/lib.rs — body bytes from the real
//      cgap_parquet::rle::rle_encode, lane values proven lane-exact
//      against the real cgap_parquet::rle::decode_into (test
//      `w4_courts::differential_vs_decode_into`). Regenerate with
//      `cargo test --lib w4_mints_js_fixture -- --nocapture` from
//      crates/wasm-footprint and re-paste (minted 2026-10-29, version
//      26.10.29).
//   2. wasm(fixture) must equal LANES_HEX exactly — the compiled artifact
//      agrees with the Rust decode_into oracle.
//   3. JS reference(fixture) must equal LANES_HEX exactly — the reference
//      below is proven against the same oracle, not just self-consistent.
//   4. wasm(bench body) must equal reference(bench body) byte-exact over
//      all lanes — then the reference verifies every timed decode.
//   5. Width refusal: a width-8 body is a typed status 1 with lanes
//      untouched.
//
// Timing: same method as the mask bench — 2 untimed warmups, min of 5
// timed calls (hrtime.bigint), deterministic body, poisoned out consumed
// after the run. Bytes moved per timed call = body bytes read + 4 B/lane
// written.

const RLE4_FIXTURE_BODY_HEX =
    '040b01080f060d040b020900070e050c030a01080f060d040b020900070e050c030a01080f060d040b022807030102040800000000';
const RLE4_FIXTURE_LANES_HEX =
    '00000001000000080000000f000000060000000d000000040000000b00000002'
    + '0000000900000000000000070000000e000000050000000c000000030000000a'
    + '00000001000000080000000f000000060000000d000000040000000b00000002'
    + '0000000900000000000000070000000e000000050000000c000000030000000a'
    + '00000001000000080000000f000000060000000d000000040000000b00000002'
    + '0000000700000007000000070000000700000007000000070000000700000007'
    + '0000000700000007000000070000000700000007000000070000000700000007'
    + '0000000700000007000000070000000700000001000000020000000400000008';

function hexToBytes(hex) {
    if (hex.length % 2 !== 0) fail(`fixture hex odd length: ${hex.length}`);
    const out = new Uint8Array(hex.length / 2);
    for (let i = 0; i < out.length; i++) {
        out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
        if (Number.isNaN(out[i])) fail(`fixture hex bad byte at ${i}`);
    }
    return out;
}

function hexToLanes(hex) {
    if (hex.length % 8 !== 0) fail(`fixture lanes hex not u32-aligned: ${hex.length}`);
    const out = new Uint32Array(hex.length / 8);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 8, i * 8 + 8), 16);
    return out;
}

// JS reference decoder: exact w=4 mirror of decode_into (see header comment).
function decodeIntoW4Reference(src, outLen) {
    if (src.length === 0) throw new Error('UnexpectedEof');
    if (src[0] !== 4) throw new Error(`UnsupportedBitWidth(${src[0]})`);
    let pos = 1;
    let oi = 0;
    const out = new Uint32Array(outLen);
    while (oi < outLen) {
        if (pos === src.length) break; // streaming end at record boundary
        let header = 0;
        let shift = 0;
        for (;;) {
            if (pos >= src.length) throw new Error('UnexpectedEof');
            const b = src[pos++];
            header += (b & 0x7F) * 2 ** shift;
            if ((b & 0x80) === 0) break;
            shift += 7;
            if (shift >= 64) throw new Error('UnexpectedEof (varint shift >= 64)');
        }
        if (header % 2 === 0) {
            // repeated run: (count<<1)|0, one LE value byte, fill UNMASKED
            const runLen = Math.floor(header / 2);
            if (runLen > outLen - oi) throw new Error(`RunOverrun(run=${runLen}, budget=${outLen - oi})`);
            if (pos >= src.length) throw new Error('UnexpectedEof (run value)');
            const v = src[pos];
            pos += 1; // ceil(4/8) = 1 value byte
            out.fill(v, oi, oi + runLen);
            oi += runLen;
        } else {
            // literal batch: groups 8-lane byte-per-lane words, lane & 0x0F
            const groups = Math.floor(header / 2);
            const byteLen = groups * 8;
            const count = Math.min(byteLen, outLen - oi);
            if (pos + byteLen > src.length) throw new Error(`LiteralOverrun(claimed=${byteLen}, avail=${src.length - pos})`);
            for (let k = 0; k < count; k++) out[oi + k] = src[pos + k] & 0x0F;
            oi += count;
            pos += byteLen;
        }
    }
    return out;
}

// --- Instantiate the w=4 surface on the same artifact ----------------------
if (typeof e.wasm_decode_page_body_w4 !== 'function') {
    fail('export missing: wasm_decode_page_body_w4 (w=4 court requires it)');
}

// Second memory.grow for the page-decode buffers (grow detaches old views;
// the mask section is fully reported above, so fresh views are made here).
const LANES4 = 96 * 41666;               // 3,999,936 lanes (96-lane cycle)
const OUT4_BYTES = LANES4 * 4;           // 15,999,744 B out buffer
const BODY4_CAP = Math.ceil(LANES4 * 3 / 4) + 4096; // upper bound before gen
const PAGES4 = Math.ceil((OUT4_BYTES + BODY4_CAP + 65536) / 65536);
let oldPages4;
try {
    oldPages4 = e.memory.grow(PAGES4);
} catch (err) {
    fail(`memory.grow(${PAGES4}) for w=4 section failed: ${err}`);
}
if (!Number.isInteger(oldPages4)) fail(`second memory.grow returned ${oldPages4}`);
const u8v = new Uint8Array(e.memory.buffer);   // fresh views post-grow
const u32v = new Uint32Array(e.memory.buffer);

// --- Fixture differential (Rust-minted bytes, Rust-oracle lanes) -----------
const fixtureBody = hexToBytes(RLE4_FIXTURE_BODY_HEX);
const fixtureWant = hexToLanes(RLE4_FIXTURE_LANES_HEX);

const fixBodyOff = alloc(fixtureBody.length);
u8v.set(fixtureBody, fixBodyOff);
const fixOutOff = alloc(fixtureWant.length * 4);
for (let i = 0; i < fixtureWant.length; i++) u32v[(fixOutOff >> 2) + i] = 0xDEADBEEF;
const fixRet = e.wasm_decode_page_body_w4(fixBodyOff, fixtureBody.length, fixOutOff, fixtureWant.length);
if (fixRet !== 0) fail(`fixture decode returned ${fixRet}, want 0`);
const fixGot = u32v.slice((fixOutOff >> 2), (fixOutOff >> 2) + fixtureWant.length);
if (!fixGot.every((v, i) => v === fixtureWant[i]) ) {
    fail(`fixture differential vs Rust decode_into FAILED at lane ${
        fixGot.findIndex((v, i) => v !== fixtureWant[i])}`);
}

const refFixture = decodeIntoW4Reference(fixtureBody, fixtureWant.length);
for (let i = 0; i < fixtureWant.length; i++) {
    if (refFixture[i] !== fixtureWant[i]) {
        fail(`JS reference vs Rust oracle MISMATCH at lane ${i}: ref=${refFixture[i]} oracle=${fixtureWant[i]}`);
    }
}
console.log(`w4 court:  fixture ${fixtureBody.length} B / ${fixtureWant.length} lanes byte-exact vs Rust decode_into; JS reference agrees (5 chain steps)`);
// Width refusal: same page footprint.sh's court uses for width 8.
{
    const w8 = [0x08, 0x0A, 0x2A];
    const off = alloc(w8.length);
    for (let i = 0; i < w8.length; i++) u8v[off + i] = w8[i];
    const o = alloc(5 * 4);
    for (let i = 0; i < 5; i++) u32v[(o >> 2) + i] = 0xDEADBEEF;
    const ret = e.wasm_decode_page_body_w4(off, w8.length, o, 5);
    if (ret !== 1) fail(`width-8 refusal returned ${ret}, want typed 1`);
    for (let i = 0; i < 5; i++) {
        if (u32v[(o >> 2) + i] !== 0xDEADBEEF) fail(`width-8 refusal touched lane ${i}`);
    }
}
console.log('w4 court:  width-8 body refused (ret=1), lanes untouched');

// --- Deterministic w=4 bench body + reference decode ------------------------
// 96-lane cycle: 32 literal lanes (per-lane (i*7+1)&0xF, never 3-equal),
// 32 literal lanes, then a 32-lane repeated run — both record arms, mixed.
const body4 = new Uint8Array(BODY4_CAP);
let body4Len = 0;
body4[body4Len++] = 0x04; // width byte
for (let i = 0; i < LANES4; ) {
    const phase = (i / 32) % 3;
    if (phase === 2) {
        body4[body4Len++] = (32 << 1);            // run header varint(64)
        body4[body4Len++] = (i * 7 + 1) & 0xF;    // value byte
        i += 32;
    } else {
        body4[body4Len++] = (4 << 1) | 1;         // literal header, 4 groups
        for (let k = 0; k < 32; k++) {
            body4[body4Len++] = (i * 7 + 1) & 0xF;
            i++;
        }
        // lint guard: header varint is single-byte (9), no continuation
    }
}
const body4Bytes = body4.subarray(0, body4Len);
const BYTES_MOVED4 = body4Len + OUT4_BYTES;

const bodyOff = alloc(body4Len);
u8v.set(body4Bytes, bodyOff);
const out4Off = alloc(OUT4_BYTES);
for (let i = 0; i < LANES4; i++) u32v[(out4Off >> 2) + i] = 0xDEADBEEF; // poison

const want4 = decodeIntoW4Reference(body4Bytes, LANES4);

function callDecode4() {
    return e.wasm_decode_page_body_w4(bodyOff, body4Len, out4Off, LANES4);
}
for (let r = 0; r < WARMUP; r++) {
    const ret = callDecode4();
    if (ret !== 0) fail(`w=4 warmup ${r + 1} returned ${ret}`);
}
const runs4 = [];
for (let r = 0; r < RUNS; r++) {
    let ns;
    try {
        const t0 = process.hrtime.bigint();
        const ret = callDecode4();
        const t1 = process.hrtime.bigint();
        ns = t1 - t0;
        if (ret !== 0) fail(`w=4 timed call ${r + 1} returned ${ret}`);
    } catch (err) {
        fail(`w=4 timed call ${r + 1} trapped: ${err}`);
    }
    runs4.push(Number(ns) / 1e6);
}
const min4 = Math.min(...runs4);
if (!(min4 > 0)) fail(`w=4 min run time is ${min4} ms — unusable measurement`);

// Consume the timed output: full byte-exact compare against the reference.
for (let i = 0; i < LANES4; i++) {
    if (u32v[(out4Off >> 2) + i] !== want4[i]) {
        fail(`w=4 consumption: lane ${i} got ${u32v[(out4Off >> 2) + i]} want ${want4[i]}`);
    }
}
let folded4 = 0;
for (const idx of [0, 1, LANES4 - 2, LANES4 - 1]) {
    folded4 = (folded4 ^ want4[idx]) >>> 0;
}

console.log(`layout4:    n = ${LANES4.toLocaleString('en-US')} lanes x 4 B out; body ${body4Len.toLocaleString('en-US')} B (literal+run mix, 96-lane cycle)`);
console.log(`bytes moved4: ${BYTES_MOVED4.toLocaleString('en-US')} B per timed call (body ${body4Len.toLocaleString('en-US')} read + ${OUT4_BYTES.toLocaleString('en-US')} written)`);
console.log(`runs4:      ${WARMUP} warmup + min of ${RUNS} timed (hrtime.bigint)`);
const gibps4 = BYTES_MOVED4 / (min4 / 1000) / (1024 ** 3);
console.log(`wasm page-decode w=4: ${gibps4.toFixed(2)} GiB/s`);
console.log(`runs4_ms:   [${runs4.map((m) => m.toFixed(3)).join(', ')}] (min ${min4.toFixed(3)})`);
console.log(`consume4:   3,999,936 lanes byte-exact vs JS reference (fixture-proven vs Rust oracle); folded=0x${folded4.toString(16).padStart(8, '0')}`);
process.exit(0);
