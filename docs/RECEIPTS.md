# CGAP v26.10.29 Manufacturing Receipts

Collected: 2026-10-30T00:14:34Z - 00:14:37Z, host Darwin 25.2.0, repo `/Users/sac/cgap`.

Toolchain observed: rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0, ggen 26.10.11,
wasm-opt version 112.

Every number below was read from a run executed for this ledger (logs retained at
`/tmp/ave_receipts/`). No number is cited from a prior session.

## Verdict Summary

| Receipt | Gate | Exit | Verdict |
|---|---|---|---|
| R1 | cargo test --workspace | 0 | ALIVE |
| R2 | release bench | 0 | ALIVE |
| R3 | branch audit (audit profile) | 0 | ALIVE |
| R4 | Gate 3 C Data wire parity | 0 | ALIVE |
| R5 | ggen sync replay x2 + cc | 0 / 0 | ALIVE |
| R6 | marketplace check + courts | 0 / 0 | ALIVE |
| R7 | wasm footprint gate | 101 | BLOCKED (fail-closed) |

## Receipt 1: Workspace Tests

```bash
cargo test --workspace
```

Exit: 0. Build was warm ("Finished `test` profile ... in 0.05s").

| Crate | Target | Passed |
|---|---|---|
| cgap-arrow | lib unittests | 15 |
| cgap-parquet | lib unittests | 11 |
| cgap-audit | lib unittests | 0 (no tests) |
| cgap-beam | lib unittests | 0 (no tests) |
| cgap-ffi | lib unittests | 0 (no tests) |
| bench | bin unittests | 0 (no tests) |
| cgap-arrow / cgap-ffi / cgap-parquet | doc-tests | 0 each |

Total: 26 passed, 0 failed.

Verbatim excerpts:

```text
Running unittests src/lib.rs (target/debug/deps/cgap_arrow-76a7889e2db7accc)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; 0.00s

Running unittests src/lib.rs (target/debug/deps/cgap_parquet-a93d3cbc33c7ccab)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; 0.00s
```

Note: `crates/cgap-parquet` is not listed in `[workspace] members` but is a path
dependency of `crates/audit` and `crates/bench`, which auto-promotes it to workspace
membership; its 11 tests ran under `--workspace`.

Compiles clean of errors; warnings observed (unused_mut, dead_code, unused_imports,
unused_variables) in cgap-arrow, cgap-ffi, bench compile units.

Verdict: ALIVE - 26 passed / 0 failed across all workspace members.

## Receipt 2: Release Bench

```bash
cargo run --release -p bench
```

Exit: 0. Build warm ("Finished `release` profile [optimized] target(s) in 0.01s").

Observed output (verbatim):

```text
single-core mask: 58.903 GiB/s over 8 rounds, 0 allocs (sink 0x0)
16-lane wall: 40096 us, aggregate 74.8 GiB/s
sink 0x0 lanes0 0
```

Key numbers: single-core mask scan 58.903 GiB/s (8 rounds), 0 allocations during
scan; 16-lane wall 40096 us (40.1 ms), aggregate 74.8 GiB/s.

Verdict: ALIVE - single-core 58.903 GiB/s, 0 allocs; 16-lane 74.8 GiB/s.

## Receipt 3: Branch Audit (Zero Conditional Branches)

```bash
cargo build --profile audit -p cgap-audit
python3 scripts/branch_audit.py
```

Build exit: 0 (warm). Audit exit: 0.

Observed output (verbatim):

```text
audit_count_valid      OK
audit_get_bit          OK
audit_is_inline        OK
audit_mask_words       OK
audit_set_bit          OK
audit_unpack_1bit      OK
audit_unpack_2bit      OK
audit_unpack_4bit      OK
audit_unpack_8bit      OK
BRANCH AUDIT PASS: 0 conditional branches across all audited kernels
```

All 9 kernel symbols located in the disassembly of `target/audit/libcgap_audit.dylib`
(non-vacuous: a missing symbol exits 1); every kernel contains 0 conditional branches
(b.cc/cbz/cbnz/tbz/tbnz/x86 jcc family).

Verdict: ALIVE - 0 conditional branches across all 9 audited kernels.

## Receipt 4: Gate 3 - C Data Interface Wire Parity

```bash
cargo build -p cgap-ffi
python3 scripts/gate3_cdata.py
```

Build exit: 0 (warm). Gate exit: 0.

Observed output (verbatim):

```text
imported: int32 100000 nulls: 14285
GATE 3 PASS: bitwise parity through C Data Interface
```

Key numbers: i32 batch of 100000 rows, null every 7th row, imported through
pyarrow `pa.Array._import_from_c`; null_count 14285 = floor(100000 / 7); validity
and values buffers verified BITWISE against python-recomputed ground truth.

Verdict: ALIVE - bitwise wire parity through the C Data Interface.

## Receipt 5: ggen Sync Replay (Determinism) + C99 Header Compile

```bash
ggen sync run   # twice
shasum -a 256 include/cgap_c_data.h include/cgap_format_codes.h \
  crates/cgap-arrow/src/generated_layout.rs \
  crates/cgap-arrow/src/generated_version.rs   # pre-run, post-run-1, post-run-2
cc -std=c99 -ffreestanding -Wall -Wextra -pedantic -c include/cgap_c_data.h \
  -o /tmp/cgap_h_receipts.o
```

Both sync runs exit 0. Generator decisions in BOTH runs (verbatim):

```json
"decisions": {
  "crates/cgap-arrow/src/generated_layout.rs": "skipped: unchanged: content identical",
  "crates/cgap-arrow/src/generated_version.rs": "skipped: unchanged: content identical",
  "generated/subject-count.txt": "skipped: unchanged: content identical",
  "include/cgap_c_data.h": "skipped: unchanged: content identical",
  "include/cgap_format_codes.h": "skipped: unchanged: content identical"
},
"written": []
```

sha256 of all four files identical across pre-run / round 1 / round 2
(observed three times each):

- `include/cgap_c_data.h`
  - `05ac80b69f87ad2680ed406b1fcfc105dd530f26961b450c7ea79b20ac9cc07e`
- `include/cgap_format_codes.h`
  - `92b9b0bd352df990362672407615c13e819f6f41ee571f17187115db9da19eee`
- `crates/cgap-arrow/src/generated_layout.rs`
  - `86e89638756fe9bf025949f9e5263c44c5c4739a3d98e7bb1b90fc60c396f976`
- `crates/cgap-arrow/src/generated_version.rs`
  - `abfd4e705ef8a0163944535ce7ffd8d3990ef6dbcb6a3320e806a6088b643fff`

Closure identity: `graph_hash_hex`
`ca2bf16965ff9034d6cb9ffe36deb42b6f25c3cf2cfd0f5ee358142752fdaf00`, pack
`cgap-pack` `3e5357379e80de0c57e5ee2aede54a1f8ea1154ff6197e1007ecbd09d019ae7e`,
actuator `ggen@26.10.11`.

cc gate exit 0; produced `/tmp/cgap_h_receipts.o` (708864 bytes, observed via ls).

Verdict: ALIVE - byte-identical replay across two runs; header compiles clean
under C99 + -ffreestanding + -pedantic with zero warnings emitted (exit 0).

## Receipt 6: Marketplace Check + Gate Witness Courts

```bash
cd /Users/sac/ggen-marketplace
python3 scripts/marketplace.py check cgap-pack
python3 scripts/check_gate_witness_courts.py | grep cgap
```

Marketplace check exit 0 (verbatim):

```text
check ok packs=1 turtle=ok qualify=ok
```

cgap-pack court line from the courts JSON (fields transcribed verbatim):

- pack: cgap-pack
- case_count: 5
- require_fail: true, require_pass: true (anti-vacuity pair)
- schema: ggen.semantic-gate-witness-court/1
- standing: ALIVE

Marketplace overall: configured_pack_count 46, standing ALIVE.

Verdict: ALIVE - cgap-pack qualified (turtle=ok, qualify=ok) with ALIVE court
standing, 5 cases, both negative and positive required.

## Receipt 7: WASM Footprint Gate

```bash
bash scripts/footprint.sh
```

Exit: 101. Script present (lane landed it); gate ran and REFUSED - no wasm byte
size is measurable because the build step failed.

Observed output (verbatim excerpts):

```text
rustc:   rustc 1.97.0 (2d8144b78 2026-07-07)
cargo:   cargo 1.97.0 (c980f4866 2026-06-30)
wasm-opt: wasm-opt version 112 (version_112)
target:  wasm32-wasip1
-- cargo rustc --release --target wasm32-wasip1 -p cgap-arrow --crate-type cdylib
error: no global memory allocator found but one is required; link to std or add
`#[global_allocator]` to a static item that implements the GlobalAlloc trait
error: `#[panic_handler]` function required, but not found
error: could not compile `cgap-arrow` (lib) due to 2 previous errors
```

The wasm32-wasip1 cdylib build of cgap-arrow requires a `#[global_allocator]` and
`#[panic_handler]` (wasm cdylib target does not link the host std allocator).
No `target/wasm32-wasip1/release/*.wasm` artifact was produced, so no byte size
exists to report. The gate is fail-closed: it exits non-zero rather than reporting
a size.

Verdict: BLOCKED (fail-closed) - wasm artifact not producible at this snapshot;
charter CTQ-4 (<= 100 KiB) NOT measured.
