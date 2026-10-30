# AVEC v26.10.29 DFLSS Charter

DMADV charter for avec v26.10.29, derived only from artifacts present in this repository.
Every claim carries its verification method; status vocabulary is
ALIVE (ran + passed this session, command cited), PARTIAL (receipt cited by path),
BLOCKED (cause cited), NO-RECEIPT (harness exists, no receipt in repo), NO-MECHANISM (no
verification path exists in the repo), UNVERIFIED (default, never rounded up).

| field | value |
|---|---|
| version | 26.10.29 |
| release date | 2026-10-29 |
| gate surface | `Makefile` (`make test gate0 gate1 gate2 gate3 gates lint publish-check`) |
| project graph | `ontology/avec.ttl` (av:version, av:releaseDate, av:ctq1..av:ctq5) |
| generated surfaces | 4 files: 2 under `include/`, 2 under `crates/avec-arrow/src/` (section 4) |

## 1. Define — scope

avec is a freestanding Apache Arrow columnar substrate. The attacked failure
classes, read from the code and the graph:

1. allocator jitter: unaligned buffers break vector-alignment assumptions (AVX-512/NEON);
2. branchy kernels: conditional branches bound scan throughput below line rate;
3. copy-based FFI: marshalling copies where the C Data Interface permits ownership handoff.

In scope for v26.10.29:

- `crates/avec-arrow`: in-memory columnar substrate, `no_std` + `alloc`, zero external
  dependencies (empty `[dependencies]` in `crates/avec-arrow/Cargo.toml`).
- `crates/avec-parquet`: Parquet persistence kernels, `no_std`, zero deps: SWAR
  bit-unpacking (1/2/4/8-bit lanes), SWAR RLE/bit-packed hybrid codec, and a minimal
  DataPageV2-style page framing (Thrift compact `PageHeader` + RLE body;
  `crates/avec-parquet/src/page.rs`, `crates/avec-parquet/src/thrift.rs`). This is
  NOT the full Parquet format — no row groups, no column/file metadata, no footer
  magic, no compression (page.rs module doc, quoted in README Scope notes).
- `crates/audit` (`avec-audit`): cdylib forcing every audited kernel into the artifact so
  the branch audit inspects real compiled machine code, not inlined remnants.
- `crates/bench`: Gate 2 harness — SWAR mask throughput plus allocation interception
  (three bins: `bench`, `parquet_bench`, `page_bench`).
- Membranes: `crates/avec-ffi` (C-ABI cdylib, Gate 3), `crates/avec-beam` (erl_nif
  membrane; latency measured, see BEAM note below).
- Ontology + generated surfaces: `ontology/avec.ttl`, `ggen.toml`, `ggen.lock`, the four
  generated files, projected from `../ggen-marketplace/packs/avec-pack`.
- Gate surface: `Makefile` targets `test gate0 gate1 gate2 gate3 gates lint publish-check`
  plus `bench-all bench-page bench-gate gate-parquet footprint branch-audit symbol-gate`.

Out of scope for v26.10.29 (declared, not merely absent):

- Full Parquet file format: the repository has RLE/bit-packed kernels plus the minimal
  DataPageV2-style page framer only — no row groups, no column/file metadata, no footer
  magic, no compression codec.
- BEAM runtime qualification: `crates/avec-beam` compiles in the workspace test run and
  its NIF-membrane latency is measured (`docs/BEAM_LATENCY.md`, verdict FAIL as
  measured, noise disclosure in that doc), but no BEAM/AtomVM runtime test exists here.
- wasm artifact qualification beyond the footprint gate: the harness exists
  (`scripts/footprint.sh`, `crates/wasm-footprint`) and `docs/wasm-footprint.txt`
  records a receipt-cited PASS (10123 bytes combined stripped); the gate was not
  re-run in the docs-truth-pass session (its script postdates the recorded artifact).

## 2. Measure — CTQ register

Canonical numbering is the project graph (`ontology/avec.ttl`, av:ctq1..av:ctq5). Code
citations resolve here: `crates/avec-arrow/src/bitmap.rs` cites charter CTQ-2;
`crates/avec-arrow/src/buffer.rs` cites charter M-03 (section 3).

| id | target (source) | gate | status |
|---|---|---|---|
| CTQ-1 | SWAR bitmap masking >= 9.0 GiB/s (av:ctq1) | `make gate2` | PASS |
| CTQ-2 | CC=1 branch audit gate (av:ctq2) | `make gate1` | PASS (scoped, see baseline) |
| CTQ-3 | 0 segfaults — mutation court (av:ctq3) | cargo-mutants court | NO-RECEIPT (backlog listed; closure lanes in flight) |
| CTQ-4 | wasm <= 250 KiB stripped wasm32-wasip1 (av:ctq4) | `make footprint` | PARTIAL — receipt-cited PASS, gate not re-run this session |
| CTQ-5 | 100% C Data Interface parity (av:ctq5) | `make gate3` | PASS |
| A-1 | zero post-init allocations in measured scan | `make gate2` | PASS |

Measured baseline, 2026-10-29, macOS arm64, stable Rust, exit codes captured
(lane REL-L11 truth pass; all commands executed that session):

- CTQ-1 / A-1 (`cargo run --release -p bench`, exit 0): MIN of 7 rounds per pattern —
  71.903 / 72.301 / 73.435 / 75.290 GiB/s over the four predicate patterns, 0
  allocations during every scan; 16-lane aggregate 67.7 GiB/s (secondary, noisy).
  Host load averages 10.28-11.82 on 16 logical CPUs with concurrent lanes running;
  fresh MINs sat 3-6% below the recorded floors in `docs/BENCH_FLOORS.json`, within
  that gate's 25% margin. Machine-local numbers; MIN-of-N is the receipt statistic;
  every run clears the 9.0 GiB/s target by at least 7.9x.
- CTQ-2 (`cargo build --profile audit -p avec-audit` exit 0; `make branch-audit` exit 0):
  inner-loop bodies branch-free in 3/9 loop kernels (mask_words, count_valid,
  mask_words_neon, BODY 0/0 each); latches/dispatch/bounds itemized — 71 conditionals
  across 9 kernels (+12 branch-free `audit_*` wrapper shims), panic/fail paths 8,
  unaccounted 0; "UNCOVERED KERNELS: none". Per-symbol budgets reproduce
  `docs/BRANCH_REGISTER.md` R2 exactly. The `rle`/`packing` kernel sites named in
  register R3 are outside the audited symbol set.
- CTQ-5 (`cargo build -p avec-ffi` exit 0; `python3 scripts/gate3_cdata.py` exit 0):
  int32 batch, 100000 rows, 14285 nulls, bitwise buffer parity through
  `pa.Array._import_from_c` (pyarrow 23.0.1).
- CTQ-3: `docs/mutation-court-backlog.txt` exists and lists 114 mutation sites as a
  plain list — the file carries no header and no verdict; no pass/fail court receipt
  is in the repository; closure lanes are in flight. `make gate-parquet` names this
  file as the court receipt pointer and does not re-run the court inline.
- CTQ-4: the footprint harness exists (`scripts/footprint.sh`, harness crate
  `crates/wasm-footprint`) and `docs/wasm-footprint.txt` records combined stripped
  10123 bytes — TARGET <= 100 KiB PASS, USL <= 250 KiB PASS (the target of record in
  `ontology/avec.ttl` av:ctq4 is 250 KiB). An earlier run of the script's prior
  revision was BLOCKED exit 101 (`docs/RECEIPTS.md` receipt 7, direct-cdylib build
  failed: no global allocator / panic handler). The script's mtime postdates the
  recorded artifact, so the PASS is receipt-cited, not session-verified.

## 3. Analyze — invariants and failure-mode controls

Invariant register (M-numbers are cited by code comments; this is the defining document;
crate-local paths below are under `crates/avec-arrow/src/` unless noted):

| id | invariant | cited in | verified by |
|---|---|---|---|
| M-01 | `no_std` + `alloc` only; zero external deps | crate docs | empty `[dependencies]` |
| M-02 | zero-copy CDI; double release = typed no-op | `c_abi.rs` | ABI tests; Gate 3 parity |
| M-03 | buffers 64-byte aligned at alloc | `buffer.rs`, `primitive.rs` | `buffers_aligned_64` |
| M-04 | scan kernels' inner-loop bodies compile branch-free; latches/dispatch/bounds itemized | `bitmap.rs`, `silicon.rs` | `make branch-audit` |
| M-05 | fail-closed decode; no panic on bad input | `rle.rs`, `unpack.rs`, `page.rs` | typed-error tests; `crates/avec-parquet/tests/robustness.rs` fuzz court |

Failure-mode controls (FMEA rows realized in code):

| failure mode | control | proof |
|---|---|---|
| double release of exported array | atomic swap-to-NULL release | `export_then_release_is_single_free` (`c_abi.rs:220`); `c_abi_export_import_roundtrip_bitwise_and_double_release` (`crates/integration/tests/cross_crate.rs:32`) |
| offset overread, non-monotonic offsets | monotonicity check at freeze | `offsets_monotonic` (`varlen.rs:116`) |
| malformed Parquet page / decode input | typed `ParquetError` (EOF, width, overrun) | typed-error tests; robustness fuzz court (3 tests green this session); zero-alloc page court `tests/page_zero_alloc.rs` (2 tests) |
| hostile `n` OOM via NIF | `MAX_N = 1 << 28` row cap | `crates/avec-beam/src/lib.rs:213` + range guards at :236/:269 |

## 4. Design — layer map and generated-file flow

```text
ontology/avec.ttl  +  ../ggen-marketplace/packs/avec-pack
        |                              |
        +--------- ggen sync run ------+
                       |
     include/avec_c_data.h        include/avec_format_codes.h
     crates/avec-arrow/src/generated_layout.rs
     crates/avec-arrow/src/generated_version.rs
                       |
              hand-written kernels (irreducible residue)
```

- `ggen.toml` pins the pack (`avec-pack = { path = "../ggen-marketplace/packs/avec-pack" }`);
  `ggen.lock` records the pack content hash (blake3) and 2 subjects.
- The four generated files open with `do not edit`; regenerate with `ggen sync run`.
  Gate 0 proves replay determinism: two consecutive syncs leave all four files
  byte-identical, then the C header compiles freestanding under
  `cc -std=c99 -ffreestanding -Wall -Wextra -pedantic`. Latest executed receipt:
  `docs/RECEIPTS.md` R5 (ALIVE).
- Kernel crates: `bench` and `avec-audit` depend on `avec-parquet` -> `avec-arrow`;
  membranes `avec-ffi` and `avec-beam` depend on `avec-arrow`. Path deps inside the
  workspace root resolve as workspace members.
- Profiles: `release` (thin LTO, `codegen-units = 1`) is the measured artifact; `audit`
  (LTO off, `codegen-units = 16`, strip off) keeps machine code symbol-legible for
  `scripts/branch_audit.py`.

## 5. Verify — gate list, receipts, execution tracker

Gate list (authoritative recipes in `Makefile`):

```console
make test     # cargo test --workspace
make gate0    # ggen sync run x2, byte-identical replay; freestanding C header compile
make gate1    # cargo build --profile audit -p avec-audit; python3 scripts/branch_audit.py
make gate2    # cargo run --release -p bench
make gate3    # cargo build -p avec-ffi; python3 scripts/gate3_cdata.py (needs pyarrow)
make gates    # gate0 gate1 gate2 gate3, in order
```

Receipts — every command below was executed 2026-10-29 (local) in `/Users/sac/avec`
by lane REL-L11, exit codes captured:

| gate | command | exit | observed |
|---|---|---|---|
| tests | `cargo test --workspace` | 0 | 142 passed, 0 failed (+35 default-filtered) |
| gate1 build | `cargo build --profile audit -p avec-audit` | 0 | audit cdylib linked (warm) |
| gate1 audit | `make branch-audit` | 0 | bodies 3/9 branch-free; 71 itemized (+12 shims); unaccounted 0 |
| gate2 | `cargo run --release -p bench` | 0 | MIN 71.903-75.290 GiB/s, 0 allocs |
| gate3 build | `cargo build -p avec-ffi` | 0 | cdylib linked; 1 warning (transient mid-edit failure earlier in session, clean on re-run) |
| gate3 parity | `python3 scripts/gate3_cdata.py` | 0 | bitwise parity, 100000 rows |
| gate0 | `make gate0` | n/a | not run; docs lane does not regenerate shared surfaces |

Session history (recorded, not hidden): earlier in the session, one `cargo build
-p avec-ffi` attempt failed (2 compile errors) while a concurrent lane was mid-edit on
that crate; an immediate re-run compiled clean (exit 0, one unused-import warning).
`docs/RECEIPTS.md` additionally records an earlier-session `cargo test --workspace`
failure pair (duplicate `panic_impl`; `dyld_stub_binder`, `_bzero`/`_free` link errors)
repaired by the owning lanes.

Execution tracker:

| workstream | status | evidence |
|---|---|---|
| avec-arrow in-memory substrate | ALIVE | tests 40+1+1+6 green; gate1 audit scoped PASS |
| avec-parquet persistence kernels | ALIVE | tests 70+13+2+3 green; fail-closed fuzz court |
| branch audit gate (CTQ-2 / M-04) | ALIVE | `make branch-audit` exit 0, scoped claim |
| throughput gate (CTQ-1) | ALIVE | bench exit 0, MIN 71.903-75.290 GiB/s, 0 allocs |
| C Data Interface parity (CTQ-5 / M-02) | ALIVE | bitwise parity, 100000 rows |
| avec-ffi C membrane | PARTIAL | parity green; no runtime suite |
| avec-beam NIF membrane | PARTIAL | compiles; membrane latency measured FAIL vs 50 us gate (`docs/BEAM_LATENCY.md`, noise-bound differential, idle floor 60x-220x gate); no BEAM runtime test in repo |
| mutation court (CTQ-3) | NO-RECEIPT | `docs/mutation-court-backlog.txt`: 114 sites, no header/verdict; closure lanes in flight |
| wasm footprint gate (CTQ-4) | PARTIAL | `scripts/footprint.sh` exists; `docs/wasm-footprint.txt` receipt-cited PASS (10123 B stripped, target+USL); earlier revision exit 101 (`docs/RECEIPTS.md` R7) |

Testing discipline: Chicago school — real collaborators, assertions on final state, no
mocks (the C ABI tests run real export/release cycles including double release; Gate 3
imports through real pyarrow; the parquet robustness suite drives the real decoder with
seeded adversarial inputs). No fixed sleeps: gates are synchronous commands.

See Also: `README.md` (developer entry), `Makefile` (gate recipes),
`ontology/avec.ttl` (CTQ facts), `scripts/branch_audit.py`, `scripts/gate3_cdata.py`,
`docs/BRANCH_REGISTER.md` (branch-claim register), `docs/BEAM_LATENCY.md`,
`docs/RECEIPTS.md`, `docs/wasm-footprint.txt`, `docs/mutation-court-backlog.txt`.
