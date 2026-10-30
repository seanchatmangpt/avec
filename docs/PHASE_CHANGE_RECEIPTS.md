# Phase-Change Receipts — v26.10.29 scoreboard

Consolidated verdicts for every phase-change and hardware-conscious design claim.
Method law: every essay/PRD number is a hypothesis with a falsifier; lane receipts
replace them as CONFIRMED / PARTIAL / REFUTED / FAIL-honest / EXTRAPOLATION-RETIRED.
Published-system figures (BitWeaving, FastLanes, BtrBlocks, BLAKE3) are cited-context
— never ours.

## L2 — Aligned zero-decode pages

| claim | verdict | receipt |
|---|---|---|
| Uncompressed PLAIN pages decode by pointer slice | CONFIRMED | `cargo test -p avec-parquet` aligned_interop 4/4; `aligned_decode: 38.59 GiB/s vs RLE 0.66` (PCL2 run) |
| 64B header padding is external-reader safe | REFUTED | PCL2: every standard reader parses the declared Thrift length — padding reads as values; padding scheme dropped pre-ship |
| avec pages load in external tools | CONFIRMED (via page_aligned) | `PYARROW-RECEIPT pyarrow=23.0.1 rows=256 type=fixed_size_binary[1] fidelity=EXACT`; corruption test flips fidelity=MISMATCH |
| crate::thrift frames interoperable pages | REFUTED -> FIXED (this session) | PCL2: DataPageHeaderV2 slots 4-7 swapped vs parquet.thrift — internally self-consistent, externally rejected. Writer/reader/fixtures corrected to 4=encoding, 5=def_len, 6=rep_len, 7=is_compressed; thrift 16/16 + page 31/31 green. Fresh pyarrow receipt through the FIXED writer pending (page_aligned carries the byte-exact proof meanwhile) |

## L3 — Compressed-domain filter

| claim | verdict | receipt |
|---|---|---|
| 300-600 GiB/s effective | EXTRAPOLATION-RETIRED | measured 0.91-2.29 packed GiB/s; 1,662-19,685 Mrows/s logical = 3.0x-40.6x materialize-then-mask single-thread (the extrapolation silently assumed SIMD + multicore) |
| packed-domain wins vs materialize | PARTIAL | wins when the page stays packed; loses when selectivity forces materialization anyway (PCL3's honest paragraph) |

## L4 — Data independence

| claim | verdict | receipt |
|---|---|---|
| BODY 0/0 implies constant-time | REFUTED | register: dict gathers, REE binary search, RLE run-scans are data-dependent channels (docs/DATA_INDEPENDENCE_AUDIT.md + ADDENDUM) |
| dict intern leak is structural | CONFIRMED | dict build card2-vs-256 ratio 4.635 vs 1.035 noise envelope |
| ViewVec 12B branch is real | CONFIRMED | 2.56-2.88x GiB/s, 1.105x/row beyond noise |
| 2 of 3 leaks closable round-1 | PARTIAL | oblivious gather 1.005x vs leaky 1.046x; compact 1.004x vs 8.321x (flat at leaky best-case); run-scan + REE search OPEN with rewrite sketches |

## L5 — In-register digest

| claim | verdict | receipt |
|---|---|---|
| 0% hash penalty | PARTIAL (better than claimed) | penalty [-7.7%, +3.1%] across 7 runs — sign tracks execution order: hidden under memory bandwidth, not free |
| mutation detection | CONFIRMED | single-bit flips detected both inputs; FFI corruption court green both membranes; NOT cryptographic (documented) |

## L6 — Fused unpack+filter

| claim | verdict | receipt |
|---|---|---|
| one-pass beats two-pass | CONFIRMED | fused 13.94 vs 5.92 GiB/s logical (w=4, 2.35x); 13.81 vs 9.28 (w=8, 1.49x); 56-case differential + oracle tests |

## L7 — WASM compiled port

| claim | verdict | receipt |
|---|---|---|
| compiled W=4 decode in sandbox | CONFIRMED | `wasm page-decode w=4: 8.45 GiB/s`; `wasm mask: 52.89-53.34 GiB/s`; differential vs Rust decode_into byte-exact; footprint 3435 B |

## L8 — Timing dispersion

| claim | verdict | receipt |
|---|---|---|
| kernels are timing-flat | REFUTED as stated; instrument landed | decode 13.6x spread = WIRE-SIZE channel (6B vs 1.05MB pages), not per-row timing; encode_fast 1.949x; REE::get 1.001x; mask 1.097x (timer floor) |

## L1/L9/L10 lane accounting

| lane | verdict |
|---|---|
| L1 ontology-compiled static decoders | NOT BUILT (held on the mutation scan; the HW wave's compiled_manifest landed the ontology-bounded dispatch approach instead: 7 entries, 264 B rodata, ggen template byte-identical) |
| L9 scoreboard | THIS DOC |
| L10 audit extension for new kernels | PARTIAL — extent-scoped gate covers the 9 arrow kernels; the parquet HW kernels (packed_filter, unpack_filter, ffor, bitslice, byteslice, oblivious, digest) are NOT yet in the audited symbol set — open item |

## Consumer-surface extras landed this phase

FFOR fused unpack (9.782 vs 6.347 GiB/s two-pass, width-shrink proven);
byteslice (scan 2.789 GiB/s flat, point_get 0.91 ns; essay's <0.5 cycles/value
cited-context — ours ~3 cycles); digest receipts through Python; shm SPSC ring
(cross-process 2.2 GiB/s verified, ~40 us RTT, darwin CLOEXEC/EACCES law
receipted); BEAM resource envelope + resource binaries (zero-copy projection);
sub-us sensor (get 0.042 us; 50 us CTQ UNSUPPORTED instrument-class); scheduling
crossover (dirty flag never wins n=1000..262144); thrift slot-swap fix (this
session, pending fresh external receipt).

## Open ledger (honest)

1. Parquet mutation settlement: PARTIAL — Run A receipted 337/360 (284 caught,
   4 missed at snapshot); 17 kills landed in tests/mutation_kills.rs; full
   settlement cancelled by user decision. Ledger: docs/mutation-court-backlog.txt.
2. BEAM <=50 us CTQ: UNSUPPORTED (instrument class) — sensor-limited, NIF get
   receipted at 0.042 us.
3. Literal encode 5.3-5.5 GiB/s vs 10+ target (SIMD attempt declined this
   wave; page write path REWIRED to the fast encoder this session, golden
   byte-equality green). Measured: page write literal 0.96-1.25 -> 5.472
   GiB/s, allocs 28 -> 6 (throughput bar >=5.0 MET; alloc bar <=2 not
   fully met — 6 measured). Page decode repeated 81.371 GiB/s / 0 allocs.
4. Not constant-time: register + addendum name the open channels.
5. HW-kernel audit extension: open (L10 partial).
6. Diataxis surface: pending final wave.
