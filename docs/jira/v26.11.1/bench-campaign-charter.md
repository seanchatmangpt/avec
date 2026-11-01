# Bench Campaign Charter — Divan Membrane (v26.11.1)

Status: CHARTERED 2026-11-01, dispatch after `wf_f175cd34-71f` completes +
resume backfill. Companion to `../v26.11.1` campaign charter (five Morning
DoD gates unchanged). Source of the pivot: perf claims in the campaign
charter are TARGETs; this campaign graduates them to MEASURED with Divan's
table as the stated method.

## Membrane

- `crates/bench` (exists) becomes the Divan membrane.
- `[dev-dependencies] divan = "<exact 0.1.x — run cargo search at authoring
  time; record the pin in the receipt>"`.
- Path deps ONLY on kernel crates. `[[bench]] name = "kernels",
  harness = false`; `fn main() { divan::Divan::default().run(); }`.
- Rule: NO `[[bench]]` targets inside kernel crates — divan/std must never
  enter a `no_std` kernel's dep graph. Bare-metal cross builds stay clean.
- `cargo test --workspace` (Gate 1) compiles bench targets but does not run
  them — the membrane's build is witnessed by Gate 1 for free.

## Bench map (kernel -> measurement -> claim converted)

| Lane | Benchmark | Converts |
|---|---|---|
| A01 | bitmask popcount-validity at consts [64, 512, 4096, 1_048_576]; slab alloc/index | zero-alloc + branch-free claims |
| A02 | bit-unpack widths 1-32 (`consts`); RLE decode direct-into-caller vs staged Vec | alloc column = 0 B proves zero-intermediate-heap |
| A04 | `cgap_admit_frame` ns/frame, bytes/s | ingress hot path |
| A06 | SlotId BLAKE3+truncate ns/lexeme | truncation cost |
| A11-A13 | 24B statement scan GB/s; unify ns/pair; builtin ns | cache-aligned scan throughput |
| A14 | admission ns/payload; 1M-row ingress | sub-4ns claim + Gate 3 throughput method |
| A17 | attestation verify (640-bit) ns | spoliation-check cost |
| A19 | extern "C" round-trip loop | sovereign boundary overhead |

## Experiments (generate-and-kill by measurement)

1. Chase-Lev portability tax: host AtomicI64 path vs AtomicU32-pair
   fallback — quantifies the Phase-2 blocker fix cost on 64-bit.
2. RLE decode: direct-into-caller vs intermediate buffer. If staged wins,
   the doctrine learns — record the divergence.
3. Penstock: branch-free filter vs early-exit. Make it a case, not faith.
4. Unifier: u64-lane compare vs bitset, const-generic capacities swept.

## Method discipline

- Receipt = `cargo bench` command + exit + full Divan table text, per lane,
  into `waves/bench-<lane>-receipt.md`. TARGET -> MEASURED only then.
- Seeded corpora via canonical SplitMix64 (`0xBF58_476D_1CE4_E5B9` — the
  corruption-witness constant; a non-canonical constant in bench code is
  the known write-corruption class).
- `black_box` every consumed output; assert a checksum on results so benches
  are Chicago (real kernel, real memory, final state asserted).
- Min-of-N across runs; no Process.sleep; flat-tail claims cite Divan's
  per-iteration distribution.

## Anti-vacuity (a bench that cannot flip is decoration)

Each bench names one structural mutation that must move its number
(e.g. inject a Vec intermediate into RLE decode -> alloc column must leave
0 B; insert a branch on the hot path -> ns must move). Court-auditor flips
one bench per lane in a /tmp sandbox: mutation -> column moves -> restore.
A bench whose number is mutation-invariant is VACUOUS and the lane's
perf-claim conversion is REFUSED.

## Dispatch shape (follow-up Workflow, after backfill)

- Pipeline over lanes with kernel benches: lane-runner writes bench + runs
  `cargo bench` + writes receipt (one dispatch per lane; leases = bench
  files + their lane's receipt only).
- Then court-auditor flip per lane (sandbox mutation, column moves).
- Then gate-runner: aggregate table -> `waves/bench-campaign-receipt.md`,
  cross-link Gate 3 throughput column + Gate 5 note (Divan reports host
  numbers only; Gate 5 stays `llvm-size` over the riscv32imc ELF).
- Coordinator owns git transitions; publish steps remain coordinator-only.

## Caveats

- Divan numbers are host-machine numbers. Gate 5 silicon fit is a build-
  artifact measurement, not a Divan measurement.
- Divan allocation counting covers the profiled closures only — zero-alloc
  Gate 2 evidence remains nm/zero-string based.
