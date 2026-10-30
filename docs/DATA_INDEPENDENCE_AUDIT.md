# Data Independence Audit

Lane PCL4, 2026-10-29, avec version 26.10.29. Standing: PARTIAL_ALIVE —
the register below is executed against real sources (file:line re-read this
session) and the contrasts the existing bench binaries embed were measured
(`scripts/timing_gate.py`, commands + exits in "Measured Evidence"). The
per-index leakage surface is NOT yet measured; those cells say so, loudly.

Thesis: branch-free bodies are not constant time. Every hot path here was
audited for data-dependent MEMORY ACCESS (addresses, scan lengths, branch
selection), classified, and measured where today's binaries permit.

## Verdict Summary

- No kernel in the repo currently supports a "constant time" or
  "data-oblivious" claim. Access patterns split into CONTENT-INDEPENDENT
  (fixed strides), PATTERN-DEPENDENT-NOT-SECRET (structure visible in
  public framing), and TIMING-LEAKING (a secret would change timing).
- Even the cleanest kernel (`mask_words`, fixed-stride AND, zero body
  branches) shows directional cycle-level content coupling on this
  machine: the dense predicate was slowest in 5/5 runs of the 5-run
  batch, within-run selectivity spread 1.15x-1.88x. At the gate's 3-run
  calibration the same contrasts read within the (wide) noise envelope —
  so the honest verdict is: coupling observed directionally, NOT
  separable from scheduler/store noise with 3-5 runs. Access-pattern
  constancy holds; cycle constancy is neither claimed nor measured here,
  and any future cycles-constant claim must defeat this effect
  explicitly.
- Structural leaks needing rewrite before any oblivious claim:
  `RunEndEncoded::get` binary search, `rle_encode`/`rle_encode_fast`
  run-detection scan, `decode_page` dictionary gather,
  `DictionaryBuilder::push` BTreeMap intern, `ViewVec::row` /
  `Varlen::row` secret-length row addressing.

## Threat Model and Class Definitions

Assumed adversary: a co-tenant measuring timing of the process holding the
data (shared cache hierarchy, branch predictors, memory bus). Schema and
column metadata (format code, logical length, bit width, dictionary card in
the page header) are public framing. Secret: the logical values, the
null pattern, run structure, payload lengths, and dictionary indices —
whenever the underlying column is secret.

Classes:

- CONTENT-INDEPENDENT (CI): addresses and work are fixed strides over
  length; values change bytes, never addresses or trip counts.
- PATTERN-DEPENDENT-NOT-SECRET (PD): work/branches depend on structure
  that is carried in public framing (wire headers, run-end arrays, encoded
  buffers an adversary holding the artifact can read directly).
- TIMING-LEAKING (TL): the dependent variable is data a secret would live
  in; timing difference is a covert channel. Conditional upgrade note
  applies where the dependent variable may be public framing.

## Register (kernel -> access pattern -> class)

Every row cites the audited source location (re-read this session,
version 26.10.29).

### CONTENT-INDEPENDENT

| kernel | location | access pattern |
|---|---|---|
| `mask_words` | `crates/avec-arrow/src/bitmap.rs:34-41` | fixed-stride AND, latch only; NEON twin `silicon.rs:41-71`, body branch-free |
| `count_valid` | `crates/avec-arrow/src/bitmap.rs:46-55` | fixed stride + popcount |
| `unpack_1/2/4/8bit` | `crates/avec-parquet/src/unpack.rs:7-76` | fixed stride, clamped tail; width dispatch `unpack.rs:80-89` is framing |
| `pack_bits` | `crates/avec-parquet/src/packing.rs:28-50` | fixed per-lane shift/or, bit-accumulator |
| `unpack_bits_tight` | `crates/avec-parquet/src/packing.rs:63-91` | fixed per-lane work over `out.len()` |
| `PrimitiveBuilder::push` | `crates/avec-arrow/src/primitive.rs:153-159` | `Option` discriminant branch but both arms are one value store + one bool store; work = f(n) only |
| `Sliced::get` | `crates/avec-arrow/src/slice.rs:107-114` | fixed base + `(offset+i)*WIDTH`; validity bit read fixed |

Caveat measured on this class: address-independence is not
cycle-independence. See the `mask_words` measurement below.

### PATTERN-DEPENDENT-NOT-SECRET

| kernel | location | dependent variable |
|---|---|---|
| `decode_into` | `crates/avec-parquet/src/rle.rs:170-239` | record-type branch on wire header (`rle.rs:188`); per-record work = run-fill `fill` vs literal unpack; page record structure is the wire's public framing. Conditional TL if the page itself is secret (enclave decode) |
| `RunEndEncoded::decode_to_dense` | `crates/avec-arrow/src/runend.rs:122-138` | writes are exactly 1/row (fixed); branch cadence and `vals[run]` touches follow run boundaries carried in the (visible) `run_ends` buffer. Conditional TL if the array is secret |
| `Bitmap::from_bools` | `crates/avec-arrow/src/bitmap.rs:104-118` | store executed only for `true` (store count = popcount); the all-valid early return (`bitmap.rs:105-107`) makes validity-buffer existence data-dependent — observable metadata framing |
| `Bitmap::set` | `crates/avec-arrow/src/bitmap.rs:150-157` | set vs clear RMW choice on data; both touch the same byte (address-independent) |

### TIMING-LEAKING

| kernel | location | leak channel |
|---|---|---|
| `RunEndEncoded::get` | `crates/avec-arrow/src/runend.rs:105-115` | `partition_point` (`runend.rs:113`) probe path = f(run_ends, i): per-row timing resolves run structure; log-probe count and probed cache lines depend on the row's position among secret run boundaries |
| `rle_encode` run scan | `crates/avec-parquet/src/rle.rs:82-84`, literal coalesce `rle.rs:94-103` | scan length = run length in the value stream: encode time is a ruler for the run-length histogram of secret data |
| `rle_encode_fast` | `crates/avec-parquet/src/encode_fast.rs:83-86`, `batch_end` `encode_fast.rs:41-56` | same run scan + 64-lane `windows(3)` first-triple search; identical dependence |
| `DictionaryBuilder::push` | `crates/avec-parquet/src/dictionary.rs:103-114` | `BTreeMap::get` descent path = f(value); first-seen insert allocates: timing maps the distinct-value structure |
| `decode_page` gather | `crates/avec-parquet/src/dictionary.rs:206-214` | `dict[ix]` with `ix` = decoded data lane: gather addresses are secret-dependent; channel is coarse (dict <= 1 KiB <= 16 cache lines) |
| `ViewVec::row` | `crates/avec-arrow/src/views.rs:60-70` | branch at `views.rs:64` on `is_inline(len)` selects which buffer the returned address comes from; address + length of the slice = f(payload length). Note: `is_inline` itself is branchless (`views.rs:18-23`) but `row()` branches on its result — the module's "branchless" claim covers the classifier arithmetic, not the row branch |
| `ViewVecBuilder::push` | `crates/avec-arrow/src/views.rs:103-128` | inline/foreign branch; foreign path appends to `data`, so the NEXT row's foreign offset = f(all prior lengths) |
| `VarlenBuilder::push` / `Varlen::row` | `crates/avec-arrow/src/varlen.rs:157-169` / `100-105` | offsets reads are fixed-stride, but the payload slice `[a..b]` = f(offsets); offsets ARE the secret payload lengths |

## Measured Evidence

Machine: macOS arm64 (Darwin 25.2.0), concurrent lanes building/benching on
the same host — numbers are load-bound; envelopes are the honest comparison
basis, absolute rates are not. Prebuilt release binaries used (mtimes newer
than their sources: `bench` 19:09:55 vs src 17:40:42; `dict_bench`
19:36:55 vs 19:36:46; `arrow_accessors_bench` 19:39:00 vs 19:38:49),
so no rebuild was needed and no shared-target cargo lock was taken.

Commands (all exit 0; raw outputs `/tmp/pcl4/*.txt`):

```
for i in 1..5: target/release/bench                 # 5 runs
for i in 1..3: target/release/dict_bench            # 3 runs
for i in 1..3: target/release/arrow_accessors_bench # 3 runs
python3 scripts/timing_gate.py --runs 3 --json /tmp/pcl4/gate_receipt.json
```

### mask_words: CONTENT contrast (fixed addresses, varying bits)

Cross-selectivity max/min of the per-run MIN GiB/s, 5 runs:

```
run1 1.268   run2 1.150   run3 1.613   run4 1.274   run5 1.876
```

The dense predicate (`0xffff...`) was the slowest selectivity in 5/5 runs.
Cross-run envelope per selectivity: 1.27-1.63 (min-of-7 inside the binary,
min-of-5 across runs). Interpretation: the sweep moves 64 MiB through the
same addresses regardless of bits set; the spread is store-side coupling
(write-allocate / dirty-line traffic scales with set bits in the output).
The bench's own doc (`crates/bench/src/main.rs:77-79`) calls a large spread
"a finding, not a feature". This is that finding.

### dict kernels: distribution + table-size contrast (card 2 vs 256)

Mean over 3 runs, contrast ratio vs cross-run noise envelope:

```
build          0.733 vs 0.182 GiB/s   ratio 4.02  envelope 1.14  BEYOND-NOISE
encode         4.405 vs 4.821 GiB/s   ratio 1.10  envelope 1.27  within-noise
decode(gather) 3.096 vs 2.951 GiB/s   ratio 1.07  envelope 1.19  within-noise
literal_decode 4.796 vs 5.040 GiB/s   ratio 1.05  envelope 1.10  within-noise
```

- build (BTreeMap intern): 3.75x-4.27x in each of 3 individual runs. The
  measured confirmation of the `DictionaryBuilder::push` TL row: tree
  descent + first-seen insert scale with the distinct-value structure.
- gather: NOT resolved at card 2 vs 256. Honest reading: the dictionary
  table is L1-resident at every lawful size (<= 1 KiB), so the structural
  gather leak (secret `ix` -> cache line) is not visible through the
  binary's two coarse shapes; the TL row stands on structure, the
  measurement gap is named, not papered over.
- encode (run scan): the two LCG shapes (card 2 -> run-heavy, card 256 ->
  run-rare) do not separate the encoders beyond noise, partly confounded
  with kernel width (1 vs 8 bits). The run-scan TL row stands on
  structure; a per-distribution driver measurement is required before any
  number here is quotable as a leak bound.

### ViewVec::row: inline vs foreign (12-byte threshold sides)

3 runs, matched ~64 MiB touched scale (`arrow_accessors_bench`):

```
GiB/s over touched bytes: 2.88x / 2.56x / 2.88x  (envelopes 1.07 / 1.20)
per-row Mrows/s:          1.14x / 1.28x / 1.14x
```

Far beyond envelope. The shape is defined solely by which side of the
12-byte inline threshold each row's payload length falls on — i.e. the
measured cost of the data-dependent address selection in `ViewVec::row`.

### Single-shape stability (no contrast available today)

```
varlen row-scan      envelope 1.004-1.005  (tightest kernel in the repo)
runend get           envelope 1.116 (166.0..185.3 Mrows/s, one fixed 100-run array)
runend decode_dense  envelope 1.017
sliced vs primitive  rates overlap; zero-copy overhead ~0 (as documented)
```

`runend get` per-index timing (early vs late row; few vs many runs) is the
structurally worst leak row and is UNMEASURED: today's binary iterates all
rows of one fixed array and takes min-of-5. Needs the `const_time_bench`
driver (concurrent lane; `crates/bench/src/bin/const_time_bench.rs` was
verified absent on disk at audit time, 2026-10-29 19:38) or a
distribution-varied driver. `scripts/timing_gate.py --const-file` consumes
its output when it lands (tolerant JSON/line parse; absent file = loud
SKIP).

## Timing Gate

`scripts/timing_gate.py` (exit 0 measure / 1 claim breach / 2 env error):

```
python3 scripts/timing_gate.py --runs 3                # measure + report
python3 scripts/timing_gate.py --runs 5 --json out.json
python3 scripts/timing_gate.py --claim runend_get=oblivious   # exit 1: unverifiable
```

A `--claim` verdict is a property of the run that measured it, not a
standing pass: at the standing 3-run calibration the mask contrasts read
within-noise (the claim would hold), while the 5-run batch's dense-slowest
5/5 direction shows the coupling the envelope absorbs. Verify live.

The only threshold is the measured noise envelope (cross-run max/min of
the metric itself) — never a hand-picked constant. `--claim K=oblivious`
exits 1 when K's contrast exceeds its envelope or when no contrast exists
to verify the claim (loud refusal, not a silent pass).

Standing gate receipt (3 runs per binary, 2026-10-29, exit 0, receipt at
`/tmp/pcl4/gate_receipt.json`): dict build card2-vs-256 ratio 4.635 vs
envelope 1.035 -> BEYOND-NOISE (median per-run 4.624); views inline vs
foreign per-row ratio 1.105 vs envelope 1.043 -> BEYOND-NOISE (median
1.118); mask selectivity contrasts 1.103/1.165 vs envelopes 1.407/1.339
-> within-noise at this calibration. Each gate run re-measures; numbers
are receipts of that run, not standing properties.

Witnessed pre-existing flake (bench lane's file, not this gate's): one
`bench` invocation exited 1 with `CTQ-5 FAIL: 2 allocations inside the
16-lane scan` — thread-spawn allocations landing inside the barrier-counted
window (`crates/bench/src/main.rs:139-165`). The gate treats it as an infra
failure (exit 1) — loud, retryable.

## What a True Data-Oblivious Claim Requires

Ordered by leak severity x feasibility:

1. `RunEndEncoded::get` (`runend.rs:113`): constant-round probe — always
   `ceil(log2(num_runs))` iterations with unconditional compare + masked
   select (cmov), or serve reads from a dense decode. Today the probe path
   is a map of the run structure.
2. `rle_encode` / `rle_encode_fast` run scan (`rle.rs:82-84`,
   `rle.rs:94-103`, `encode_fast.rs:84-86`, `encode_fast.rs:41-56`):
   fixed-window SWAR equality scans with no early exit (the repo already
   owns the SWAR pattern — `bitmap.rs:34-41`), record emission via masked
   stores; work fixed at f(n) for fixed n.
3. `decode_page` gather (`dictionary.rs:206-214`): unconditional full-dict
   read per lane with a `2^bits`-way masked select (256-way cmov gather),
   or oblivious scatter. Cost moves to O(n * dict_size) or O(n log dict)
   fixed rounds — the honest price of the claim.
4. `DictionaryBuilder::push` (`dictionary.rs:103-114`): replace BTreeMap
   intern with a fixed-shape `2^bits`-slot table + linear masked scan;
   pre-size every buffer (no first-seen-dependent allocation).
5. `ViewVec::row` / `Varlen::row` + their builders (`views.rs:64`,
   `varlen.rs:100-105`): oblivious only when lengths are public framing;
   otherwise fixed-max-width padding or masked full-segment reads.
6. Scope: even after 1-5, "constant time" can only mean constant ACCESS
   pattern. The measured `mask_words` result (dense output slowest 5/5)
   shows cycle-level content coupling survives in fixed-stride kernels;
   a cycles-constant claim additionally needs store-elision-resistant
   implementation and must be re-measured per host.

## Gaps and Honesty Notes

- Per-index distributions (which row index, which dictionary index, which
  run count) are unmeasured — today's binaries embed exactly one shape per
  kernel. Listed cells are structural classifications, not measured leaks.
- The gather's cache-line channel is coarse and unresolved at the binary's
  shapes; absence of a measured contrast there is not evidence of absence.
- Envelopes from 3-5 runs are themselves noisy (1.05-1.63 observed);
  `--runs N` controls this. Ratios shown are means of 3 runs with the
  median per-run ratio in the gate output.
- The 16-lane aggregate and build/encode absolute rates are load-bound
  (concurrent lanes on this host); only within-batch contrasts with
  envelope calibration are quotable.
- `const_time_bench.rs` (concurrent lane) was absent at audit time; the
  gate's `--const-file` contract (JSON cases or `<kernel> <case> <value>
  <unit>` lines) is ANTICIPATED and must be reconciled when that lane
  lands.
- FFI surfaces (`c_abi.rs`, `c_abi_nested.rs`) delegate to the kernels
  above and inherit their classifications; not separately audited.

## ADDENDUM — lane HWL5, 2026-10-29: two of the three named TL surfaces

Standing: PARTIAL_ALIVE. Two fixed-access-pattern kernels landed in
`crates/avec-parquet/src/oblivious.rs` (new file, 490 lines, no_std, zero
deps): `oblivious_gather_small` and `oblivious_masked_compact`. Both are
differentially proven and measured against their leaky originals. The
timing numbers below are receipts of the 21:17 run on the load-bound host;
only within-run contrasts vs the leaky same-run references are quotable.

### What landed

- `oblivious_gather_small(dict, codes, out)` — positional dictionary
  gather by 64-slot broad-side select: per lane, all 64 slots are read and
  slot-index-vs-code compared unconditionally, match flag OR-selects the
  value. Every dictionary cache line touched every lane; trip counts =
  `64 * codes.len()` (framing). BOUNDARY: dictionaries > 64 entries are
  refused typed with the TRUE unclamped width (65→7, 257→9, 1000→10),
  mirroring `required_bits`' reporting law. Out-of-domain codes are caught
  by an oblivious branch-free OR-accumulate → typed error; the one tail
  branch publishes nothing the return value does not already carry.
- `oblivious_masked_compact(values, valid, out) -> usize` — O(n)
  branch-free compaction: every input row read every cycle; store address
  = cross-word running popcount PREFIX (the per-word prefix accumulation),
  never WHICH rows are valid — two patterns with equal prefixes produce
  identical store-address streams. Store COUNT still scales with validity
  density (dirty-line coupling class, per the mask_words finding); the
  return publishes density by definition. Budget: dense output
  (`out.len() >= values.len()`, loud assert on the framing-only budget,
  `packed_filter` "mask buffer too small" precedent); slots at/after the
  returned count are transient scratch. Missing validity words mean
  all-invalid rows; padding bits past `n` ignored. An exact-fit-out
  contract was rejected during development: with an exact-fit budget the
  fixed store stream writes past the last valid row and either panics or
  needs a data-dependent bounds branch — the leak itself.
- Wiring status: `lib.rs` is coordinator-owned (bounded-poll window); the
  file is NOT yet `mod oblivious;`-wired. Compile-clean is proven
  standalone: a scratch crate (/tmp/hwl5-scratch) path-includes the exact
  bytes with a `crate::rle::ParquetError` shim; `cargo build --release`
  exit 0, zero warnings.

### Claims and non-claims

Claimed: ACCESS-PATTERN constancy (addresses and trip counts are f(public
framing) only). NOT claimed: cycle constancy — the register's mask_words
store-coupling finding applies to compaction with more force; the spread
tables below are the honest statement of residual coupling.

### Differential evidence (all green, exit 0)

`cargo test --release` in /tmp/hwl5-scratch: 7 passed, 0 failed. Gather:
dict sizes {1,2,3,5,16,17,32,63,64} x code sizes {0,1,63,64,1000,1M} LCG
grids + adversarial shapes (all-first/all-last/alternating/clustered/
single-spike) at cards 1,2,64, full-output equality vs the leaky
`dict[c]` reference. Compact: sizes {0,1,63,64,1000,1M} x {all, none,
alternating, single x3, clustered, p10/p50/p90}, prefix+count equality vs
the branch-per-row reference; typed-gate table (65/128/257/1000 entries →
7/7/9/10); should_panic budget gate; padding/missing-word semantics.

### Timing spread (pasted; instrument mirrors const_time_bench laws)

rows=1048576, K=100 instances/class, min-of-3, splitmix64 keyed seeds,
instance 0 of every class differential-checked untimed (0 anomalies in
all four tables). Command: `cargo run --release` in /tmp/hwl5-scratch,
exit 0. Leaky references measured in the same run, same methodology.

```
== oblivious gather (card=64) ==          == leaky gather (dict[c] ref) ==
sorted               4811000 ns           sorted                789708 ns
reverse              4812375 ns           reverse               790000 ns
clustered            4826000 ns           clustered             814209 ns
constant             4814583 ns           constant              778500 ns
alternating          4812125 ns           alternating           780000 ns
random-high-entropy  4815417 ns           random-high-entropy   784584 ns
permuted             4833167 ns           permuted              784833 ns
spread 1.005x (max permuted / min sorted) spread 1.046x (max clu / min const)

== oblivious compact ==                   == leaky compact (branch/row) ==
all-valid            567333 ns            all-valid             933667 ns
none-valid           567125 ns            none-valid            560166 ns
alternating          567834 ns            alternating           746875 ns
clustered            569209 ns            clustered             776500 ns
random-90            568250 ns            random-90            1612875 ns
random-50            567625 ns            random-50            4660916 ns
random-10            567166 ns            random-10            1366959 ns
spread 1.004x (max clu / min none)        spread 8.321x (max p50 / min none)
```

Cost of obliviousness: gather 4.81 ms vs 0.78 ms per 1M lanes (6.1x — the
O(n x 64) broad-side price); compact 567 us — flat at the leaky kernel's
BEST-case floor (567 vs 560 us) and up to 8.2x cheaper than its worst
case. The bar: REE get ~1.001x floor (bench lane). Measured: 1.004-1.005x,
inside every noise envelope the register has measured (1.05-1.63x), and
the residual 0.4-0.5% is directionally consistent with the store-side
coupling the register's mask_words finding predicts for any kernel whose
store count varies with content.

First measurement of the gather cell: the register recorded the
`decode_page` gather contrast as "NOT resolved at card 2 vs 256" (dict is
L1-resident at every lawful size). At card 64 x 7 code-shape classes x K=100
the coarse channel now MEASURES: 1.046x (clustered vs constant), beyond the
varlen row-scan floor (1.004-1.005x) but far under the leaky compact's
8.3x branch channel. The TL row stood on structure; it now has a number.

### Register impact

- `decode_page` gather (dictionary.rs:206-214): **BOUNDED**, not closed.
  The primitive carries dicts <= 64 entries; `decode_page` itself still
  gathers per `dict[ix]` at SWAR widths {1,2,4,8} (cards to 256). The row
  closes when `decode_page` routes through the oblivious primitive (or a
  wider broad-side select) — integration outside this lane's file
  ownership.
- `oblivious_masked_compact`: **NEW CAPACITY** — no register row closed;
  it is the fixed-pattern building block a filtered materialization needs
  (the branch-per-row shape it replaces is the 8.3x channel measured
  above).
- Still OPEN of the three named surfaces:
  - **run-scan** (`rle.rs:82-84`, `rle.rs:94-103`, `encode_fast.rs:83-86`,
    `encode_fast.rs:41-56`): the run detector's scan length IS the run
    length. Rewrite sketch: fixed-window SWAR equality over the value
    stream — compare every lane against its neighbor at one fixed
    stride (the repo owns the pattern: `bitmap.rs:34-41`), never exit
    early; per-position run-completion flags accumulate branch-free into a
    per-word mask; record boundaries are then emitted by masked stores at
    popcount-prefix addresses (the compact kernel's own law). Work fixed
    at f(n) for fixed n; the wire shape stays the encoder's output, the
    TIMING stops being a ruler for the run-length histogram.
  - **REE binary search** (`runend.rs:113`): `partition_point`'s probe
    path is a map of the run structure. Rewrite sketch: fixed-round
    resolve — always `ceil(log2(num_runs))` probe iterations, each an
    unconditional compare + masked select (cmov) of the run index, then
    one fixed `values[run]` read; or serve `get` from a dense decode when
    the caller sweeps rows. Constant probe count kills the per-row timing
    map; the probed cache lines become f(num_runs) only — framing.

### Honesty notes

- Single run, load-bound host (concurrent lanes); within-run contrasts
  with the same-run leaky references are the quotable numbers; absolute
  rates are not.
- The compact budget contract (dense out + loud assert) is a lane
  decision, documented in the module; an exact-fit contract is
  structurally branch-hostile — recorded so a reviewer doesn't mistake it
  for an oversight.
- The timing instrument lives in the scratch driver (std bin), not in the
  kernel file, because the kernel file must compile no_std; the test
  module carries the differential proofs, the driver carries the
  measurement. Reconcile with the bench lane's `const_time_bench` output
  contract when the coordinator wires the module.
