# Doctest Coverage

Standing: ALIVE — every count is a re-readable receipt from
`cargo test --doc` run against `/Users/sac/avec` on 2026-10-31 (wave-14 lane
12). Verdict: **zero doctests exist anywhere in the workspace**. All 8 crates
with a lib target run 0 doc tests; 3 crates (bench, avec-cli, wasm-footprint)
have no doctest target at all (bin-only / cdylib-only); avec-beam skipped per
lane contract. Five proposed doctests for the highest-value public APIs were
drafted and verified to compile AND execute against the real crates via a
/tmp probe crate (5/5 PASS, receipts below). The snippets are proposals —
they are not yet inserted into source (lane is new-files-only, read-only on
sources).

## Count Receipts (cargo test --doc, per crate)

Ran from the workspace root unless noted. Exact command: `cargo test --doc -p
<crate>`; wasm-footprint ran from `crates/wasm-footprint` (standalone
workspace root with its own target dir).

| Crate | Lib target | `cargo test --doc` result | Receipt |
|---|---|---|---|
| avec-arrow | yes | 0 passed | `Doc-tests avec_arrow` / `running 0 tests` / `ok. 0 passed` |
| avec-blake3 | yes | 0 passed | same shape, `Doc-tests avec_blake3` |
| avec-parquet | yes | 0 passed | same shape, `Doc-tests avec_parquet` |
| avec-ffi | yes | 0 passed | same shape, `Doc-tests avec_ffi` |
| avec-gpu | yes | 0 passed | same shape, `Doc-tests avec_gpu` |
| avec-git | yes | 0 passed | same shape, `Doc-tests avec_git` |
| avec-audit | yes | 0 passed | same shape, `Doc-tests avec_audit` |
| avec-integration | yes | 0 passed | same shape, `Doc-tests avec_integration` |
| bench | no | error: no library targets found | bin-only crate; no doctest target exists |
| avec-cli | no | error: no library targets found | bin-only crate; no doctest target exists |
| avec-beam | — | SKIPPED | lane contract ("beam skip") |
| wasm-footprint | no | error: no library targets found | cdylib-only; ran from its own dir |

Totals: 8 lib-carrying crates × 0 doctests = **0 workspace doctests**. The
doc comments themselves are dense (module headers, `///` contracts, refutation
notes), but none carry a ```rust example block — rustdoc has nothing to
compile or execute.

## Why the zeros are the finding

The workspace's testing discipline (branch-free kernel bodies, Chicago-style
integration tests, conformance courts) is real, but its documentation surface
is exec-verifiable nowhere: every documented contract (`filter_quads_eq`'s
n_quads+1 sink-slot rule, `write_page`'s 1:1 payload mapping, `hash_many_simd`
byte-identity claims) is prose-only. A doctest is the cheapest gate that keeps
those prose claims from drifting from the code — each snippet below pins one
contract as an executable assertion.

## Five proposed doctests (verified compile + run, 5/5 PASS)

Each snippet below is the exact text verified by the probe crate
(`/tmp/avec-doctest-probe`, path-deps on the four workspace crates; probe
`cargo run` output: 5/5 PASS, exit 0, 2026-10-31). Each lands as a ```rust
block appended to the named doc comment; `cargo test --doc -p <crate>` then
counts 1 for that crate.

### 1. `Quad64` transpose/untranspose round-trip

Location: `crates/avec-arrow/src/transpose.rs`, on `pub fn transpose` (or the
`Quad64` struct doc). Pins: field-wise columnar projection, 32 B/row digest
concatenation, and exact round-trip identity.

```rust
use avec_arrow::transpose::{transpose, Quad64};

let quads: Vec<Quad64> = (0..8).map(|i| Quad64 {
    s_id: i * 3, p_id: i * 7, o_id: i * 11, g_id: i,
    t_ns: 1_000_000 + i as u64,
    vector_clock: i as u32, policy_mask: 0xFFFF_0000,
    inline_digest: [i as u8; 32],
}).collect();

let cols = transpose(&quads);
assert_eq!(cols.s_id, quads.iter().map(|q| q.s_id).collect::<Vec<_>>());
assert_eq!(cols.digest.len(), quads.len() * 32); // 32 B per digest, concatenated
assert_eq!(cols.untranspose(), quads); // round-trip is exact
```

### 2. `filter_quads_eq` compaction

Location: `crates/avec-arrow/src/predicates.rs`, on
`pub fn filter_quads_eq`. Pins: all-four-lanes-equal match semantics,
n_quads+1 output capacity (slot n_quads is the sink), indices-only output.

```rust
use avec_arrow::predicates::filter_quads_eq;

// data = the s_id column, 4 lanes per quad. Quads 0 and 3 are all-7.
let data: Vec<u32> = vec![7, 7, 7, 7,  7, 7, 7, 8,  9, 9, 9, 9,  7, 7, 7, 7];
let n_quads = 4;
let needle = 7u32;
// Capacity contract: n_quads + 1 (slot n_quads is the mask-selected sink).
let mut out = vec![0u32; n_quads + 1];

let count = filter_quads_eq(&data, n_quads, needle, out.as_mut_slice());
assert_eq!(count, 2);
out.truncate(count);
assert_eq!(out, vec![0, 3]);
```

### 3. `level_free::write_page` payload round-trip

Location: `crates/avec-parquet/src/level_free.rs`, on `pub fn write_page`.
Pins: payload is carried verbatim past the compact-protocol header (1:1
Arrow-buffer mapping), typed `PayloadMismatch` on the way in.

```rust
use avec_parquet::level_free::{write_page, LevelFreeType};

let rows: [u32; 3] = [10, 20, 30];
let payload: Vec<u8> = rows.iter().flat_map(|v| v.to_le_bytes()).collect();

let page = write_page(LevelFreeType::U32, &payload, rows.len()).unwrap();

// Header + contiguous payload; the tail bytes are the payload verbatim.
assert!(page.len() > payload.len());
assert_eq!(&page[page.len() - payload.len()..], payload.as_slice());
```

### 4. `PackBuilder` append-then-regenerate flow

Location: `crates/avec-git/src/pack_write.rs`, on `pub struct PackBuilder`.
Pins: pack v2 magic/version, idx v2 magic, object count, and the
append-then-regenerate flow end to end.

```rust
use avec_git::pack_write::parse_pack_v2;
use avec_git::PackBuilder;

let mut b = PackBuilder::new();
b.push(b"alpha");
b.push(b"beta blob");
assert_eq!(b.len(), 2);

let (pack, idx) = b.finish().unwrap();
assert_eq!(&pack[0..4], b"PACK");
assert_eq!(&pack[4..8], &2u32.to_be_bytes());
assert_eq!(&idx[0..4], &[0xFF, 0x74, 0x4F, 0x63]); // \xFF tOc — idx v2 magic
assert_eq!(parse_pack_v2(&pack).unwrap().len(), 2);
```

### 5. `hash_many_simd` byte-identity

Location: `crates/avec-blake3/lib.rs`, on `pub fn hash_many_simd`. Pins the
crate's central claim (multi-chunk SIMD path is byte-identical to the scalar
`hash`) as an executable assertion.

```rust
use avec_blake3::{hash, hash_many_simd};

// 4096 B = 4 full 1024-B chunks -> the multi-chunk SIMD Merkle path.
let data: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();

assert_eq!(hash_many_simd(&data), hash(&data)); // byte-identical digests
assert_eq!(hash_many_simd(&data), hash_many_simd(&data)); // deterministic
```

## Probe receipt

Probe crate `/tmp/avec-doctest-probe` (its own workspace root, path-deps on
avec-arrow / avec-blake3 / avec-git / avec-parquet). `cargo run` from the
probe dir, 2026-10-31:

```text
Compiling avec-git v26.10.29 (/Users/sac/avec/crates/avec-git)
Compiling avec-arrow v26.10.29 (/Users/sac/avec/crates/avec-arrow)
Compiling avec-parquet v26.10.29 (/Users/sac/avec/crates/avec-parquet)
Compiling avec-blake3 v26.10.29 (/Users/sac/avec/crates/avec-blake3)
Compiling avec-doctest-probe v0.1.0 (/private/tmp/avec-doctest-probe)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.97s
Running `target/debug/avec-doctest-probe`
snippet 1 Quad64 round-trip: PASS
snippet 2 filter_quads_eq: PASS
snippet 3 level_free write_page: PASS
snippet 4 PackBuilder: PASS
snippet 5 hash_many_simd: PASS
```

Each probe function body is the exact snippet text above (so a PASS is a
compile + execute receipt for that snippet, not a paraphrase). Closed loop:
the ```rust blocks were then extracted mechanically FROM this file and
re-compiled/re-run as a second probe (`/tmp/avec-doctest-probe2`) — 5/5 PASS,
exit 0. The doc text and the executable text are the same text.

## Admission path

1. Paste each snippet into the named doc comment as a ```rust block.
2. Re-run `cargo test --doc -p <crate>` — each crate should count exactly 1
   (arrow/predicates snippet and transpose snippet both live in avec-arrow, so
   avec-arrow goes 0 -> 2).
3. Optional gate: add the per-crate doctest counts to the existing gate
   battery so a count regression (a deleted example) is a loud failure, not a
   silent doc trim.

## Standing

ALIVE for the counts and the probe receipts (commands + outputs above, run
this session against the canonical checkout); PROPOSED for the five snippets
(compile+run verified, not yet inserted into source). Falsifier: run
`cargo test --doc -p avec-arrow` — if it counts > 0, this doc's counts are
stale; if the probe crate fails to reproduce 5/5 PASS, the snippet receipts
are false.
