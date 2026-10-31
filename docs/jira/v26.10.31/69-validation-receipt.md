# Validation Receipt: 17-Lane Thesis-Court Sweep

17 read-only validation lanes, 2026-10-30, each mapping a thesis chapter
claim to its implementation and running the real courts. Nothing written
in-repo by the lanes; this receipt is the coordinator's synthesis.

## Per-Claim Verdicts

| Lane | Thesis claim | Verdict |
|---|---|---|
| V1 | Ch2 quad geometry: 64B/64B, offsets, sentinels, 4x64=256 | VALIDATED (tests + /tmp probe + cli FR-01 sweep agree) |
| V2 | Ch2.2 transpose as linear map | **PARTIAL — NEON transpose REFUTED**: never built (no cfg(target_arch) in transpose.rs; NEON exists only in silicon.rs + blake3). Round-trip courts VALIDATED (5/5). untranspose: 7 branch sites INVISIBLE to the gate (ltmp0 rendering), absent from manifest |
| V3 | Ch3.3 predication = projection | VALIDATED (7/7 tests; disassembly: cmeq/bic/umaxv/csel interior, 2 remainder = assert+latch; one off-by-one: exhaustive domain is 0..=15 not 0..=16) |
| V4 | Ch3.4 ladder M=1, Var(T)=0, typed cap | VALIDATED (16-stage constancy asserted across 6 edge classes; 3.49M lookups/s; fsck 2/2 + interop 3/3 vs real git 2.51.2; TooManyObjects typed refusal) |
| V5 | Ch4 level-free isometry | VALIDATED (15 tests: zero levels, byte-equality writer↔scanner both configs, refusals; zero-alloc scoped to page decode — open allocates the footer index by design) |
| V6 | Ch4.3 scan viscosity | VALIDATED (Thrift once at open; PAYLOAD-SUM 62.1 GiB/s fresh; ACCESSOR 8.0; honest gap labeled in the bin) |
| V7 | Ch5 packfile free monoid | VALIDATED (48/0: SHA-1 FIPS vectors, CRC32 vectors, python-zlib cross-checks both directions, git fixtures, real-git acceptance + negative control) |
| V8 | Ch5.2 BLAKE3 tree + refutation | VALIDATED (6-vector conformance, zero-alloc + live positive control, 0.43 GiB/s reproduces, refutation record intact; LSL_RATIFIED=None at ctq_report.rs:45 — docs cite :44, off-by-one) |
| V9 | Ch6 admission congruence | VALIDATED (determinism + round-trip + receipt-survival asserted; typed refusals per crate; fresh 10.3µs release probe; caveat: bare parquet test run silently collects 0 tests without --features std) |
| V10 | BEAM membrane | VALIDATED (real BEAM: 12/12 court markers; pointer identity; 1000-read delta {0,0}; dtor exactly-once 64; five-op surface 21/21 registered) |
| V11 | C Data Interface membrane | VALIDATED (5 zero-copy + 4 alloc courts x3 runs flake-free; audit crate's counter anti-vacuity witnessed) |
| V12 | Ontology under SHACL | VALIDATED (Conforms True x2; mutant genuinely flips, REFUSAL-13 from the real report; fixture↔receipt provenance matches; 1-of-14 negative corpus honestly disclosed) |
| V13 | wasm portability | VALIDATED (6/6 builds exit 0; cdylib 4,309 B byte-exact vs witness; NEON correctly cfg'd out; NOTE: wasm-footprint is a standalone manifest — build from the crate dir) |
| V14 | e2e pipeline | VALIDATED stages 1-3; the e2e git-stage SKIP premise is STALE (all four symbols re-exported; two lines close it — recorded, not edited) |
| V15 | M=1 enforcement machinery | VALIDATED (selftest non-vacuous; manifest adjudication 12/2 MANIFEST-OK post-regex-fix; cfg_homology mutant FLIPS, normal mode honest-FAILs transpose +17; ENVIRON fix confirmed in BOTH gates) |
| V16 | Whole-system battery | VALIDATED — independently reproduced: 10/10 gates PASS exit 0; ctq FAIL [CTQ-06] as designed; spot-census matches 63 exactly (661/0/2 closes) |
| V17 | Claim→receipt chains | 6/6 CHAINS INTACT; findings: two receipt-less "fresh" cpk rows in 58/61 (unreplayable, unregistered in 64); stale-copy defects (admission_e2e skip text, fsck_court panic message, :44-vs-:45 cite, 20-prd 8.2µs cite without pointer); gate-git-tests-v2 STALE-UNREMINTED pending v3 |

## New Findings Register (for wave 10 or operator)

1. **REFUTED + unrecorded**: NEON transpose (spec docs 23/27 specify ZIP/UZP
   transpose; never implemented). Record as refuted design target.
2. **Gate blind spot**: untranspose (7 sites, b.ls/cbnz unmanifested) —
   ltmp0 symbol rendering defeats both gates' name scoping.
3. e2e git-stage wiring gap: two lines (dev-dep + real stage) close it.
4. admission_e2e/fsck_court stale skip/panic texts (cosmetic but false).
5. ctq_report.rs LSL_RATIFIED is line 45 (docs say 44).
6. Receipt-less cpk rows in 58/61; gate-git-tests-v3 mint pending.
7. Bare `cargo test -p avec-parquet` silently collects 0 refusal tests.

Standing of the validated surface: the thesis's empirical spine (Ch.8)
holds — battery reproduced independently, chains intact, one design
target (NEON transpose) newly refuted and now recorded here.

See Also: 68-wave-10-receipt.md (following wave), 61-final-verdict-v3.md,
63-test-census.md.
