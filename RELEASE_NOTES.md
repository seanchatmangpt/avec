# Release Notes — cgap v26.10.29

Release v26.10.29, dated 2026-10-29 (`docs/DFLSS_CHARTER.md`, version table).

Sources for every number in this document: `docs/RECEIPTS.md`,
`docs/BEAM_LATENCY.md`, `docs/BRANCH_REGISTER.md`, `docs/wasm-footprint.txt`,
`docs/BENCH_FLOORS.json`, plus `docs/DFLSS_CHARTER.md`,
`docs/mutation-court-backlog.txt`, and the `Makefile` gate surface. Receipt
docs conflict on several values; conflicts are named in
[Discrepancies](#discrepancies), never silently reconciled.

## What Ships in v26.10.29

### Crates

| crate | role (receipt docs) | publish |
|---|---|---|
| cgap-arrow | in-memory columnar substrate, `no_std` + `alloc`, zero deps | true |
| cgap-parquet | branchless Parquet persistence kernels, `no_std`, zero deps | true |
| cgap-ffi | C-ABI cdylib membrane (Gate 3 surface) | false |
| cgap-beam | erl_nif membrane (BEAM latency receipt) | false |
| cgap-audit | cdylib forcing kernels into the branch-audit artifact | false |
| bench | Gate 2 throughput + allocation-interception harness | false |
| integration | integration test crate (`publish = false`) | false |
| wasm-footprint | standalone footprint / symbol-gate artifact crate | false |

Publish classification is transcribed from `publish = true|false` in each
`crates/*/Cargo.toml`; the two publish-enabled crates are cgap-arrow and
cgap-parquet.

### Kernel inventory (modules present on disk)

cgap-arrow (`crates/cgap-arrow/src/`): `bitmap`, `primitive`, `varlen`,
`views`, `c_abi` (+ `c_abi_nested`), `silicon`, `buffer`, `runend`, `slice`,
plus the generated `generated_layout` / `generated_version` pair.

cgap-parquet (`crates/cgap-parquet/src/`): `rle`, `packing`, `unpack`,
`encode_fast`, `dictionary`, `thrift`, `page`.

## Gate States (as receipted)

Gate surface: `make test gate0 gate1 gate2 gate3 gates lint publish-check`
(`Makefile`); auxiliary targets `footprint`, `branch-audit`, `symbol-gate`,
`gate-parquet`, `bench-all`, `bench-page`, `bench-gate`.

| gate | subject | receipted state |
|---|---|---|
| gate0 | ggen sync replay determinism | ALIVE |
| gate1 | CC=1 branch audit (extent-scoped) | ALIVE, claim rescored |
| gate2 | release bench | ALIVE, machine-local |
| gate3 | C Data Interface wire parity | ALIVE |
| gate-parquet | mutation-court receipt | ledger open |
| footprint | wasm footprint (CTQ-4) | CONFLICTED |
| symbol-gate | zero-panic Jidoka (wasm-primary) | no run receipt |
| BEAM latency | scheduler-impact CTQ | FAIL as measured |

### Gate 0 — ggen sync replay (byte-stable)

Two consecutive `ggen sync run` passes exited 0; all four generated surfaces
byte-identical in both runs (`written: []`, "skipped: unchanged: content
identical" x5 decisions), sha256 of all four files stable across three
readings; the C header compiles freestanding under
`cc -std=c99 -ffreestanding -Wall -Wextra -pedantic`, exit 0, zero warnings
(708864-byte object produced). (`docs/RECEIPTS.md` R5.)

### Gate 1 — CC=1 branch audit (extent-scoped)

`scripts/branch_audit.py` exit 0: 9 audited kernel symbols located in the
audit-profile disassembly (missing-symbol guard non-vacuous), with the
extent-scoped result: **inner-loop bodies are branch-free; latches, dispatch,
and bounds paths are itemized, never hidden** (`docs/BRANCH_REGISTER.md`).

- 3 leaf kernels fully branch-free: `get_bit`, `set_bit`, `is_inline`.
- True callee budgets (latch/body/dispatch): `mask_words` 2/0/3,
  `count_valid` 2/0/3, `mask_words_neon` 3/0/4, `unpack_1bit/2bit/4bit`
  4/3/1 (+ out-of-line panic call), `unpack_8bit` 6/9/1 (+ panic call).
- The earlier "9/9 kernels, 0 conditional branches" headline was
  vacuous-by-wrapper (Class B audit defect, 6 loop-bearing wrappers compiled
  to tail-call shims the audit never scanned); fixed by the callee-extent
  rewrite. Class counts: A=0, B=9, C and D itemized in the register.

The task's framing "Gate 1 = mutation court" does not match the `Makefile`
(`gate1` = branch audit; the mutation-court receipt is `make gate-parquet`);
see Discrepancies.

### Gate 2 — release bench (machine-local)

`cargo run --release -p bench` exit 0, single run, machine-local:
single-core mask scan 58.903 GiB/s over 8 rounds with 0 allocations during
scan; 16-lane wall 40096 us (40.1 ms), aggregate 74.8 GiB/s.
(`docs/RECEIPTS.md` R2.) Charter-baseline runs the same day measured
43.127 / 57.870 / 62.026 GiB/s; per-pattern minima are recorded in
`docs/BENCH_FLOORS.json`. Wall numbers vary with concurrent host load; the
9.0 GiB/s CTQ-1 target is cleared by every run (charter: at least 4.8x).

Per-pattern floors (`docs/BENCH_FLOORS.json`):

```text
mask 0x0 = 72.873 GiB/s   mask 0x1 = 77.39 GiB/s
mask 0x5 = 77.135 GiB/s   mask 0xf = 77.75 GiB/s
rle_decode w4: 5.724-79.382 GiB/s by pattern
rle_encode w4: 0.994-10.084 GiB/s by pattern
unpack random64: 19.186-39.074 GiB/s (w8 -> w1)
```

### Gate 3 — C Data Interface wire parity

`cargo build -p cgap-ffi` + `python3 scripts/gate3_cdata.py`, both exit 0:
int32 batch of 100000 rows, null every 7th row (null_count 14285 =
floor(100000/7)), imported through pyarrow `pa.Array._import_from_c`
(pyarrow 23.0.1 per charter); validity and values buffers verified BITWISE
against python-recomputed ground truth. (`docs/RECEIPTS.md` R4.)

### gate-parquet — mutation-court receipt

`make gate-parquet` requires `docs/mutation-court-backlog.txt`; the ledger is
present with **114 entries** (survivor mutants itemized per file:line). The
charter still carries CTQ-3 (mutation court) as NO-RECEIPT — no court-run
totals exist in any receipt doc. The 114-entry ledger is open work.

### footprint — wasm footprint (CTQ-4)

Not established as a single value at release-notes time: the receipt docs
disagree (10123 B combined stripped PASS per `docs/wasm-footprint.txt`; exit
101 BLOCKED with no artifact per `docs/RECEIPTS.md` R7; harness-absent
NO-MECHANISM with a 250 KiB target per the charter). All values named in
Discrepancies; the coordinator reconciles. The measurement machinery ships:
`scripts/footprint.sh` plus the standalone `crates/wasm-footprint` harness
crate (public-API closure method, wasm-opt -Oz stripped).

### symbol-gate — zero-panic Jidoka (wasm-primary)

Mechanism (from `scripts/symbol_gate.py`): PRIMARY (gating) scans the
optimized wasm32-wasip1 artifact of `crates/wasm-footprint` for forbidden
panicking/unwind/abort symbols — 0 forbidden symbols required; SECONDARY
(informational, never gating) reports the std-linked host membranes
`libcgap_audit.dylib` / `libcgap_ffi.dylib`; ANTI-VACUITY requires a
panic-containing no_std probe to FIRE the scanner at both pipeline stages.
**No run receipt for the symbol gate exists in `docs/`**; its state in this
release's receipt set is UNVERIFIED. See Discrepancies.

### BEAM latency (cgap-beam membrane)

CTQ: scheduler impact <= 50 us under heavy NIF churn. Receipt run
(`test/beam_latency.escript`, OTP 28 / ERTS 16.2, host loadavg 14.14,
machine-local): **FAIL as measured** — differential impact 7985.791 us and
absolute loaded max 11128.208 us against the 50 us gate, exit 1. No threshold
was adjusted. Disclosed stability: a second run flipped the differential
sign (impact -1706.958 us, exit 0); the idle 1 ms timer floor alone
(3.1-11.2 ms max) is 60x-220x the gate before any NIF churn, so this
instrument on this host cannot resolve the CTQ either way; the absolute gate
reads FAIL in both runs. Attribution probes: dirty-CPU flag honored (no
churn-proportional drift at 1-churner scale).

Other receipted numbers: `cgap_get/2` median 0.084 us, p99 0.167 us
(calibration floor 0.042 us median, NIF round-trip ~0.04 us class);
`cgap_new(16384, 0)` median 742.333 us on the dirty path, debug build.
(`docs/BEAM_LATENCY.md`.)

## Known Open

1. **Gate 1 mutation-court survivors**: `docs/mutation-court-backlog.txt`
   lists 114 survivor mutants; no court-run totals are receipted (charter
   CTQ-3 NO-RECEIPT).
2. **BEAM scheduler-impact CTQ FAIL**: both receipt runs read the absolute
   gate FAIL; the instrument cannot resolve 50 us under host load
   (idle floor 60x-220x the gate). Needs a noise-resolving instrument or an
   idle-host run.
3. **symbol-gate secondary scope**: host-membrane scans are informational by
   design (std-linked membranes would measure std, not cgap); no run receipt
   exists for either scope.
4. **Parquet = kernels + minimal page framing, not the full file format**:
   charter out-of-scope declaration ("no page/file reader-writer");
   `crates/cgap-parquet/src/` carries the page-framing modules (`page`,
   `thrift`, `dictionary`) as kernels.
5. **CTQ-4 wasm footprint** has no single receipted value (three-way doc
   conflict, Discrepancies 1).
6. **cgap-ffi / cgap-beam runtime suites**: charter tracker marks both
   membranes PARTIAL (no runtime suite / no BEAM runtime test in repo;
   the BEAM latency receipt postdates that row).

## Next Milestone

1. Close the 114-entry mutation-court survivor ledger and receipt court-run
   totals against CTQ-3.
2. Re-measure the BEAM scheduler-impact CTQ with an instrument that resolves
   50 us (idle host or a drift-resistant sensor), and land a charter CTQ row
   for it.
3. Reconcile CTQ-4: one footprint receipt of record (harness state, artifact
   byte size, target 100 vs 250 KiB) and a run receipt for `symbol-gate`.
4. Extend the branch audit to the register's named uncovered kernels
   (`Bitmap::is_all_valid`, `count_one_bits`, and the three cgap-parquet
   per-lane bounds sites) per `docs/BRANCH_REGISTER.md` R3.
5. Land the register's doc-correction sweep (R4) everywhere the unscoped
   "zero conditional branches" phrasing survives.

## Discrepancies

Conflicts are listed, not reconciled. Format: claim — value A (source A) vs
value B (source B).

1. **Wasm footprint byte size (CTQ-4)** — 2225 B stripped vs 100 KiB target
   (release work order) vs **10123 B** combined stripped, PASS vs <= 100 KiB
   (`docs/wasm-footprint.txt`: arrow 10123 / parquet 245 / combined 10123
   stripped; raw 24801) vs **exit 101, no wasm artifact produced, CTQ-4 NOT
   measured** (`docs/RECEIPTS.md` R7: cgap-arrow wasm32-wasip1 cdylib fails
   for lack of `#[global_allocator]` / `#[panic_handler]`) vs **harness
   absent, NO-MECHANISM, 250 KiB** (`docs/DFLSS_CHARTER.md`: CTQ-4 and
   out-of-scope section). Charter also records gate0 "not run" while
   RECEIPTS.md R5 ran it twice (see item 7).
2. **Mutation-court totals** — 384/572 (release work order) is stated in no
   receipt doc. Charter CTQ-3: NO-RECEIPT ("no court receipt in repository");
   `docs/mutation-court-backlog.txt`: 114 entries. No doc states caught/total
   counts.
3. **Gate numbering for the mutation court** — release work order maps the
   mutation court to Gate 1; `Makefile` gate1 = branch audit and
   `gate-parquet` = mutation-court receipt. Charter section 2 separately
   says the gate1 recipe "points at docs/RECEIPTS.md, which does not exist"
   (RECEIPTS.md exists; Makefile gate1 runs the branch audit).
4. **Branch-audit headline** — "BRANCH AUDIT PASS: 0 conditional branches
   across all audited kernels" (`docs/RECEIPTS.md` R3, 9/9 OK) vs
   non-zero latch/dispatch budgets per kernel and the vacuous-by-wrapper
   finding (`docs/BRANCH_REGISTER.md` R1/R2: inner-loop bodies branch-free;
   latches/dispatch itemized). Charter (CTQ-2/M-04) still carries the
   unscoped "0 conditional branches" phrasing.
5. **Gate 2 bench values** — 58.903 GiB/s single-core / 16-lane 74.8 GiB/s,
   wall 40096 us (RECEIPTS.md R2) vs 43.127 / 57.870 / 62.026 GiB/s
   single-core, 16-lane 43.1-54.3 GiB/s (charter Measure section, same day)
   vs per-pattern minima 72.873-77.75 GiB/s (BENCH_FLOORS.json). All
   machine-local single runs under concurrent host load.
6. **BEAM CTQ register row** — the scheduler-impact CTQ (<= 50 us) is
   measured and FAIL in `docs/BEAM_LATENCY.md` but has no row in the charter
   CTQ register (CTQ-1..CTQ-5; M-04 is the branch-audit invariant); the
   charter tracker's "cgap-beam PARTIAL, no BEAM runtime test in repo" row
   predates the latency receipt.
7. **Gate 0 run state** — RECEIPTS.md R5: two sync runs + freestanding C99
   header compile, exit 0, byte-identical replay vs charter receipt table:
   gate0 "not run; docs lane does not regenerate shared surfaces".
8. **symbol-gate wasm-primary PASS** — claimed by the release work order;
   no run receipt exists anywhere in `docs/` (gate mechanism present:
   `Makefile` + `scripts/symbol_gate.py`). Status here: UNVERIFIED, not
   PASS.

## Toolchain (receipted)

rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0, ggen 26.10.11, wasm-opt
version 112, pyarrow 23.0.1 (charter), OTP 28 / ERTS 16.2 (BEAM receipt).
Host: macOS arm64 (Darwin 25.2.0). All bench/latency numbers are
machine-local single runs under concurrent host load.

---

## Rename: avec→cgap (2026-10-31)

Post-v26.10.29 rename: avec→cgap across crates, C ABI, BEAM NIFs, headers,
docs; version frozen at 26.10.29 (no re-release — v26.10.29 receipts retain
their recorded values). The license URN `urn:avec:license:p28-sil:v2.1.0-sovereign`
intentionally retains the `avec` namespace (P28-SIL §9 instrument identity,
unchanged by the rename).
