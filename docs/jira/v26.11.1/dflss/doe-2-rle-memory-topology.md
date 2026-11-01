# doe-2-rle-memory-topology

- Artifact: `C12-doe-2-rle-topology` (charter-conversion engineer C12,
  lane-runner). Lease: this file ONLY. Written 2026-11-01.
- Standing of this document: `UNVERIFIED` — every number below is a TARGET
  until the section 11 receipt is filled with command+exit evidence run in
  this session.
- Source of truth: `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md`,
  section 4 Phase 3 item 2 (DOE-2, RLE Decode Memory Topology), plus the
  Phase 2 anti-vacuity control of the same charter.

## 1. Charter mandate

Phase 3 item 2: measure direct-into-caller buffer decoding against staged
intermediate vector decoding across bit widths 1-32. If staged buffering
outperforms direct writing on specific widths, record the divergence and
adapt the kernel. The charter frames this as generate-and-kill: empirical
hardware numbers arbitrate the architecture, not design preference.

Phase 2 binding control (same charter): each benchmark must undergo one
deliberate structural degradation in a sandbox — here, injecting a heap
vector into decoding. If the measured Divan column fails to shift, the
benchmark is declared `VACUOUS` and graduation from `TARGET` to `MEASURED`
is `REFUSED`.

## 2. Subjects under test (grounded this session)

All anchors are in `crates/cgap-parquet/src/rle.rs`, read this session:

- Arm A (direct): `decode_into(src: &[u8], out: &mut [u32])
  -> Result<usize, ParquetError>` — rle.rs:192. Zero-allocation by
  construction: no `Vec::new`/`vec!`/`collect`/`to_vec` on the path
  (rle.rs:189-191); guarded by a counting-allocator court
  (rle.rs:330-383: counting `GlobalAlloc`, thread-local counters, asserts a
  measured 0 during a full decode).
- Full-buffer wrapper: `rle_decode(src, out) -> Result<(), ParquetError>`
  (rle.rs:169) — delegates to `decode_into`, same wire format, requires the
  page to exactly fill `out`.
- Fixture encoder: `rle_encode(values: &[u32], bits: u8) -> Vec<u8>`
  (rle.rs:88) — builds identical pages for both arms at each width.
- Kernel width gate: `matches!(bits, 1 | 2 | 4 | 8)` at rle.rs:196; any
  other width is `ParquetError::UnsupportedBitWidth(bits)`.
- Harness home: `crates/bench/` (workspace member; `divan = "0.1.21"`
  dev-dependency; path dep on `cgap-parquet`; precedent
  `crates/bench/benches/v26_10_31.rs` uses `divan::black_box` at lines 24,
  62, 92-93, 104). Bench code lands only in `crates/bench/`, never in
  kernel crates.

Grounding fact D-G1: the gate at rle.rs:196 admits exactly {1, 2, 4, 8}.
Widths 3, 5-7, 9-32 return `UnsupportedBitWidth` today. The 1-32 sweep
reports those widths as loud `[SKIP] UnsupportedBitWidth` rows (exit-code 3
class), never silent; each becomes a kernel-adaptation order (section 8).

## 3. Question and hypotheses

Question: does direct-into-caller decoding (`decode_into` writing lanes
straight into the caller's buffer) dominate staged decoding (an intermediate
heap `Vec<u32>` between decode and caller handoff) across bit widths 1-32,
on the Divan allocation column and on latency?

- H-A (zero-heap): arm A reports 0 B allocated per `decode_into` call at
  every kernel-admitted width. Hard requirement, not a preference.
- H-B (direct latency): arm A latency <= arm B latency at every admitted
  width; expected margin grows with run share (fill amortizes; staged pays
  decode plus copy).
- H-C (divergence candidate D-1): staged may win on wide, literal-dominated
  widths (17-32 class) where byte-per-lane literal traffic dominates and
  the extra copy is pure overhead — the charter's named divergence case,
  recorded per section 8 if observed.
- H-0 (anti-vacuity): the allocation column is a live instrument — under
  the section 7 mutation it must leave 0 B. A flat 0 B column under
  mutation kills the benchmark, not the claim.

## 4. Independent variable and fixture corpus

Width axis: 1..=32, swept via Divan const parameters
(`#[divan::consts]` over width groups; one const per width keeps the
`decode_into` call site monomorphic and the sweep declarative).

Fixture construction, identical for both arms at every width:

- Values: SplitMix64 stream from the charter-pinned seed
  `0xBF58_476D_1CE4_E5B9`; outputs wrapped in `divan::black_box` so the
  optimizer cannot fold the corpus away.
- Pages: encoded with the real `rle_encode` at width w. Two page classes
  per width — run-heavy (repeated runs dominate) and literal-heavy — plus
  one fixed mixed page for cross-width comparison.
- State checksum: the sum of decoded lanes asserted equal across arms
  before timing; a mismatch kills the run (arms must decode the same page).
- Determinism: same seed, same page bytes, same checksums every run; the
  seed byte pattern is checked against the charter pin before the sweep
  (silent seed drift is a named charter risk).

## 5. Arms

- A DIRECT: `decode_into(black_box(src), black_box(&mut out))` where `out`
  is the caller-owned buffer (stack array or caller-provided slice). The
  zero-intermediate-heap claim rides on this arm: no heap traffic between
  decode entry and caller handoff.
- B STAGED: the real `decode_into` into a per-call `Vec<u32>` sized to the
  lane count, then `out.copy_from_slice(&staged)`. Same kernel, same wire
  bytes; only the memory topology differs. The staged vector is the
  deliberate intermediate the charter names.
- C MUTATION (sandbox only, section 7): arm A's body with the staged
  vector injected. Never committed to the shared tree; lives only in the
  /tmp court copy.

## 6. Metrics, gates, exit contract

- PRIMARY: the Divan allocation column. Arm A MUST report 0 B allocated at
  every admitted width. Any nonzero byte total on arm A REFUSES the
  zero-intermediate-heap claim — the path is not direct; fix the kernel or
  withdraw the claim. Checked per width, never in aggregate.
- SECONDARY: Divan time column (mean per-call latency) and derived
  throughput (lanes/s). Latency arbitrates H-B/H-C only; it never
  overrides the 0 B gate.
- Admitted widths today: {1, 2, 4, 8} (D-G1). All other widths are loud
  `[SKIP] UnsupportedBitWidth` (exit-code 3 class), listed per width in the
  receipt — a skip is never a pass.
- Exit-code contract (repo gate discipline): 0 = run clean, all gates
  hold; 1 = gate fail (0 B gate broken, or mutation court did not flip);
  2 = subject missing (crate or bench absent); 3 = loud skip.

## 7. Anti-vacuity mutation court (charter-mandated)

Purpose: prove the allocation column can move. A benchmark whose alloc
column stays at 0 B while the code under test heap-allocates measures
nothing; charter Phase 2 makes the flip mandatory before any TARGET to
MEASURED graduation of the zero-heap claim.

Protocol — sandbox is a /tmp copy of the bench file with sha256 fail-closed
restore (the repo shim-court discipline; the shared tree is never mutated):

1. BASELINE: sha256 the bench file; record it.
2. BEFORE: run the unmutated bench; witness arm A alloc column = 0 B at an
   admitted width (TARGET until run).
3. SHIM: in the /tmp copy only, inject the staged topology into arm A:
   declare a `Vec::with_capacity(lanes)` intermediate, decode into it, then
   `out.copy_from_slice(&staged)`. Mark the diff `SHIM-COURT`.
4. AFTER: run the mutated bench. FLIP CONDITION: arm A's allocation column
   leaves 0 B (nonzero B per iteration, scaling with lane count). If it
   stays 0 B the benchmark is `VACUOUS` — exit nonzero and the graduation
   of the zero-heap claim is `REFUSED`.
5. RESTORE: sha256 of the tree file must equal BASELINE byte-exact; any
   mismatch is fail-closed (the run is invalid, not "mostly restored").
6. SELF-CHECK: the court procedure is exercised once against a deliberately
   corrupted restore on a scratch copy to prove restore-detection fires
   (witnessed nonzero exit); the scratch is then deleted.

A court that cannot flip is FAIL per repo discipline; the flip here is the
alloc column leaving 0 B under a real staged-buffer injection. Skips are
loud (`[SKIP] <reason>`), never silent substitutions for a run.

## 8. Divergence rule (the doctrine learns)

Arm allocation columns are bounded by construction (A: 0 B; B: at least
4 * lanes bytes per call, TARGET), so latency is the arbiter wherever both
arms run. If staged beats direct on any specific width, the receipt records
width, fixture class, both alloc columns, both latencies, and the delta.
Silent acceptance of a divergence is banned.

Consequences, in order:

1. Record the divergence row (width, fixture class, delta) in the receipt.
2. Adapt the kernel: a per-width dispatch (staged fast path for the
   offending literal-dominated widths, direct elsewhere) becomes a work
   order. The doctrine learns that memory topology is width-conditional,
   not absolute.
3. Re-run this DOE on the adapted kernel; the divergence row must shrink
   or invert, else the adaptation is reverted (fix forward, no dead paths).

Widths the kernel refuses today ({3, 5-7, 9-32}, D-G1) are not staging
wins — they are `UNSUPPORTED` adaptation orders: loud in the receipt, and
widening the gate is kernel work, never a benchmark fudge.

## 9. Predicted outcomes (all TARGET, none MEASURED)

| Width class | Arm A alloc | Arm B alloc | Latency expectation |
| --- | --- | --- | --- |
| 1, 2, 4, 8 | 0 B (TARGET) | >= 4*lanes B/call (TARGET) | A <= B; grows with run share (TARGET) |
| 3, 5-7, 9-32 | [SKIP] UnsupportedBitWidth | same | UNSUPPORTED; adaptation orders (sec. 8) |

Fixture-class note: run-heavy pages favor A strongly (fill vs decode plus
copy); literal-heavy wide pages are the H-C divergence candidates (D-1).
A flat "direct always wins" table is NOT the success criterion — a faithful
table is. If D-1 fires, the receipt records it and the kernel adapts.

## 10. Run protocol (owned by the bench lane, not this lease)

1. Land the bench: `crates/bench/benches/doe2_rle_topology.rs`.
2. `cargo bench -p bench --bench doe2_rle_topology` — per-width allocation
   and time columns; exit 0 expected on a clean pass.
3. Mutation court: section 7 protocol over the /tmp copy; flip witnessed;
   restore byte-exact; self-check executed.
4. Spec validations: `awk 'length > 100'
   docs/jira/v26.11.1/dflss/doe-2-rle-memory-topology.md` (expect empty
   output) and H1 check (`head -1` equals `# doe-2-rle-memory-topology`).

## 11. Receipt template (fill with MEASURED values only)

    artifact_id: C12-doe-2-rle-topology
    subject: cgap-parquet @ <git sha>, rle.rs <sha256>
    commands: <every command + exit code run this session>
    widths_admitted: <for each w in {1,2,4,8}: alloc_A=<B>, alloc_B=<B>,
      t_A=<ns>, t_B=<ns>, checksum=<match>, winner=<A|B>>
    widths_skipped: <for each w outside {1,2,4,8}: [SKIP] UnsupportedBitWidth>
    zero_heap_gate: <PASS|REFUSED> per admitted width
    mutation_court: BASELINE=<sha256>; BEFORE alloc_A=0 B; SHIM=<diff id>;
      AFTER alloc_A=<nonzero B> => FLIPPED (or VACUOUS => REFUSED);
      RESTORE sha256 match=<yes|no>; SELF-CHECK=<fired|not-fired>
    divergences: <D-n rows or "none">
    standing: <ALIVE|PARTIAL|BLOCKED|REFUSED|UNSUPPORTED|UNVERIFIED>

## 12. Standing

This specification is `UNVERIFIED` until the section 11 receipt is executed
with command+exit evidence. Graduation ladder: TARGET (this doc) ->
MEASURED (bench-lane receipt with real runs) -> ALIVE (0 B gate and flip
court both witnessed on the exact subject). Nothing graduates by intent;
compile-abort is not a measurement; a flat mutation column is VACUOUS.
