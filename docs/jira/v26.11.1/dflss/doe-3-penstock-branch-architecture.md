# doe-3-penstock-branch-architecture

- Charter source: `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` (section 4,
  Phase 3, item 3)
- Conversion lane: `C13-doe-3-penstock-branch`
- Artifact class: DOE specification (realizes `phase-3-analyze-doe.md` §3 DOE-3; that
  file's binding rules R1-R6 and DOE-3 kill criteria bind this experiment)
- Standing: every predicted number in this file is `TARGET` (directional nominal,
  unmeasured — including the ~4.0 ns/frame directional nominal). Standing claims use
  only `ALIVE` / `PARTIAL` / `BLOCKED` / `REFUSED` / `UNSUPPORTED` / `UNVERIFIED`.
  Nothing here is MEASURED; nothing rounds up to ALIVE.

## 1. Charter Quote (Verbatim Source of Truth)

> **DOE-3 (Penstock Branch Architecture):** Compare branch-free bitwise predicate
> filtering against early-exit conditional branching across varying selectivity
> distributions.

Quantities predicted beyond the quote carry `TARGET` standing and are arbitrated by
measurement (R1: hardware numbers decide; R2: TARGET until measured with cited
command + exit).

## 2. Subjects Under Test (On Disk, Witnessed This Session)

Kernel subject: `crates/cgap-gate/src/penstock.rs` (1719 lines; session receipts in §10).

The landed `ingress` chain IS the early-exit arm. DOE-3 compares the landed architecture
against a challenger built to identical semantics — the incumbent is not a strawman.

- `pub fn ingress(bytes: &[u8], len: usize, out: Option<&mut FrameView>) -> u32`
  (penstock.rs:491) — Arm A: the landed early-exit chain, rows 1-5 in fixed normative
  order, first failure returns (lane doc A14 §4; taxonomy table mirrored at
  penstock.rs:482-489). Probe mode (`out = None`) is the measured call shape (the
  landed fence's form, penstock.rs:1670).
- `pub struct FrameView<'bytes>` (penstock.rs:454) — zero-copy admit out-view; mints no
  heap state (module doc :444-453).
- `pub const HEADER_LEN: usize = 128` (penstock.rs:209) — frame geometry (A04 §6).
- `fn null_baseline` (penstock.rs:1590) — §8.4 ratio-fence baseline; secondary metric.
- `throughput_ratio_fence_null_baseline` court (penstock.rs:1608) — prior method
  witness: min-of-11 paired rounds, `black_box`, release-only with loud `[SKIP]` in
  debug (:1618-1624).
- `read_u64_le` / `read_u16_le` (penstock.rs:596 / :609) — panic-free LE readers;
  byte-loop shape load-bearing (in-file witness :591-594: slice/copy forms measured
  6.3 ns/frame vs 4.9 byte-loop on 2026-11-01 — cited, not re-measured by this lane).

Row taxonomy (A14 §5): rows 1-2 `CGAP_ERR_INVALID_MAGIC`; row 3
`CGAP_ERR_OABI_MISMATCH`; rows 4-5 `CGAP_ERR_P28_SPOLIATION`; row 6 (header flags bits
4-7) ADMITTED. Session witness (this session, command + exit): `cargo test -p cgap-gate
--release` -> `running 11 tests ... 11 passed; 0 failed ... finished in 7.11s`, exit 0.
Arm A subject is ALIVE this session.

## 3. Arms

### 3.1 Arm A — early-exit conditional chain (incumbent, on disk)

`ingress` (penstock.rs:491) evaluates the taxonomy rows in one fixed order and returns
on the first failure: rows 1-2 -> `CGAP_ERR_INVALID_MAGIC`; row 3a/3b ->
`CGAP_ERR_OABI_MISMATCH`; rows 4-5 -> `CGAP_ERR_P28_SPOLIATION`; a clean frame writes
the caller's out-view and returns `CGAP_OK` (:496-586). Early-exit yield is the lane
doc's own design intent (A14: "highest early-exit yield", magic as cheapest
discriminator) — this DOE tests that doctrine against measured numbers (R1: faith does
not vote).

### 3.2 Arm B — branch-free bitwise predicate fold (challenger; specified here, absent on disk)

Construction law for the implementing lane (the arm lives in the bench crate; it must
NOT edit `penstock.rs` — file ownership stays with A14):

1. Same panic-free LE reader discipline as penstock.rs:588-614 (byte-loop shape,
   no slice copies across helper boundaries; in-file witness :591-594).
2. Predicate fold: evaluate ALL row predicates unconditionally into violation bits
   v1..v5, no branch on frame bytes:
   - v1: `len < 128` OR `len > bytes.len()` (row-1 class incl. caller-geometry
     fail-closed, :506-508)
   - v2: magic/version inequality — fold bytes 0..8 against `{MAGIC, version}`,
     reserved bytes 6..8 masked to zero before the compare (ignore-on-read)
   - v3: digest OR-reduction over the 32 B field equals zero (u64 lanes OR-folded)
     OR `declared > len - 128` (:526-537, :541-544)
   - v4: attestation magic inequality (bytes 48..52 vs `P28S`)
   - v5: P28 law set — version, reserved-flags mask, lineage-popcount law,
     ceiling bound, receipt law ROOT-xor-receipt-zero (:553-576)
3. Priority-select the A19 code branchlessly, preserving first-failure-wins order
   (v1 > v2 > v3 > v4 > v5) via mask-select: `mask = 0u32.wrapping_sub(bit as u32)`;
   `code = (code & !mask) | (err & mask)`, folding from v5 upward.
4. Row 6 (header flags bits 4-7) is ADMITTED — Arm B must NOT fold it as a violation
   (an equality-court case pins this).
5. Identical admit semantics: clean frame returns `CGAP_OK` (probe mode, no out-view
   write in the measured loop).

The experiment compares source-level branch architectures AS COMPILED on the host;
codegen inspection (`--emit=asm`, cmov density) is secondary evidence — recorded if
run, UNVERIFIED until then.

## 4. Selectivity Distributions (Controlled Factor)

### 4.1 Required cells (charter-named)

| Cell | Drop rate d | Composition |
| --- | --- | --- |
| S1 | 0.1% | 99.9% admit / 0.1% reject (honest-traffic regime) |
| S2 | 50% | 50/50 admit / reject (neutral regime) |
| S3 | 99.9% | 0.1% admit / 99.9% reject (adversarial-scan regime) |

### 4.2 Edge anchors (regime-map completeness; reported, non-voting)

S0 = 100% admit; S4 = 100% reject. Pre-registered: anchors do not vote in §7's
decision; they bound the regime map only.

### 4.3 Reject-row mix (controlled variable)

Every reject is drawn uniformly over fault rows 1-5 (20% each, seeded), so arm-cost
deltas attribute to branch architecture, not fault-row placement. Row 1 emits truncated
buffers; rows 2-5 emit a valid base plus the matching single mutation of the landed
generator's classes (penstock.rs:799-807): MC_MAGIC -> row 2; MC_LEN_OVERRUN /
MC_DIGEST_ZERO -> row 3; MC_ATT_MAGIC -> row 4; MC_ATT_RANGE / MC_ATT_RECEIPT -> row 5.
A fault-row-mix sweep is out of scope this DOE (recorded, not silent).

### 4.4 Corpus construction law (determinism; Chicago realism)

- Generator state = the charter-pinned canonical constant `0xBF58_476D_1CE4_E5B9`
  (charter §4 Phase 2) driven through the SplitMix64 schedule landed at
  penstock.rs:770-778 (gamma `0x9E37_79B9_7F4A_7C15`; multipliers `0xBF58_476D_1CE4_E5B9`,
  `0x94D0_49BB_1331_11EB`). Frame class, fault row, and bytes are a pure function of
  (seed, cell, i); two generations are byte-equal (final-state assert; the landed
  determinism-court pattern, penstock.rs:1025-1033).
- Per cell: N = 65,536 frames; valid frames 128..384 B (payload 0..256 B uniform
  seeded); frames packed contiguously into one slab + (offset, len) table ONCE, outside
  every measured region (the landed fence's discipline, penstock.rs:1632-1641).
- Manifest: the expected A19 code per frame is known by construction (the generator
  knows which law it broke; the `Expected` pattern, penstock.rs:786-796) and is the
  equality court's reference.
- Seed-corruption guard: any state constant other than `0xBF58_476D_1CE4_E5B9`
  (e.g. `...E5BA`) is the known corruption class — the bench is REFUSED (charter §6
  risk row 1).

## 5. Primary Metric and Measurement Method

### 5.1 Primary metric

ns/frame — nanoseconds per payload frame, one frame being the payload unit the
transport hands `ingress` (128 B header plus a seeded 0..256 B payload where present).
ns/frame = min batch wall time across paired rounds / frames per batch (65,536).

### 5.2 Method (binding)

1. Instrument: Divan membrane `crates/bench` (per `bench-membrane-spec.md`). Bench file
   `crates/bench/benches/doe3_penstock_branch.rs` — absent on disk this session
   (`ls crates/bench/benches` exit 0 lists only `v26_10_31.rs`; status: specified, not
   landed, UNVERIFIED until a run exists).
2. Wiring required (verified absent by Read this session): `crates/bench/Cargo.toml`
   `[dependencies]` gains `cgap-gate = { path = "../cgap-gate" }`; a
   `[[bench]] doe3_penstock_branch` with `harness = false`; the divan pin exacted to
   `=0.1.21` per membrane-spec divergence 1 (on-disk pin today is caret `0.1.21`).
3. Command: `cargo bench -p bench --bench doe3_penstock_branch` (cargo bench is
   release-profile; assert `!cfg!(debug_assertions)` in-harness and `[SKIP]` loudly
   otherwise — the landed fence's rule, penstock.rs:1618-1624). Host disclosed in the
   receipt.
4. `black_box` every consumed input and produced output; AFTER the timed region assert
   both arms' code vectors equal the setup manifest (differential equality — Chicago
   final-state assert, not call-count).
5. min-of-N: N >= 11 PAIRED rounds (an A round then a B round, repeated; host drift
   cancels across the pair — the landed fence's pairing law, penstock.rs:1648-1675).
   Each cell's column is the min over its 11 rounds.
6. Secondary columns: ratio vs `null_baseline` (A14 §8.4 method); ns/byte over the
   128 B header geometry; Divan allocation column (must read 0 B for both arms — the
   §8.2 zero-alloc gate law).
7. No Process.sleep anywhere; the exit code is the observable (contract: 0 pass /
   1 gate fail / 2 subject missing / 3 loud skip).
8. Tail law: the CTQ's flat-p99 claim is cited only against Divan's per-iteration
   distribution column — never a percentile the table does not print.

## 6. Hypotheses (every magnitude and predicted decision is TARGET)

- H1 (crossover exists — predicted): no single arm wins all three required cells.
  Predicted regime map (TARGET): Arm A wins S1 (the admit path is fully predictable and
  both arms pay the full predicate set; the predictable conditional chain edges out);
  Arm B wins S2 and S3 (data-dependent exit points maximize misprediction as reject
  density rises). Falsifier: one arm wins S1 AND S2 AND S3 — H1 is dead and the §7.3
  single-winner decision fires.
- H2 (zero-alloc, both arms): Divan alloc column = 0 B for A and B in every cell.
  Falsifier: any nonzero alloc column disqualifies that arm (killed, not ranked).
- H3 (semantic identity): Arm B's code vector equals Arm A's code vector on every frame
  of every corpus, including RejectAny-class garbage (the priority encoding must
  reproduce first-failure-wins). Falsifier: any mismatched code — Arm B is inadmissible,
  killed not ranked; no speed number for a killed arm is admissible.

## 7. Decision Rule (Pre-Registered; Numbers Decide)

### 7.1 Admissibility gates (fixed order)

1. H3 equality PASS (else kill the disagreeing arm); 2. H2 zero-alloc PASS (else kill);
3. §8 anti-vacuity flip PASS (else the whole bench is VACUOUS -> graduation REFUSED,
   exit 1). Only admissible arms are ranked; a killed arm reports no winner.

### 7.2 Per-cell winner and noise band

Lower min-of-N ns/frame wins the cell. Noise band: a cell is WON only if the loser's
min exceeds the winner's min by more than the losing arm's own round spread,
spread = (max - min) / min over its N rounds; inside the band the cell is a TIE and
ties resolve to the incumbent Arm A (no kernel churn on noise; conservative,
pre-registered).

### 7.3 Single winner vs crossover

- SINGLE-WINNER (same arm wins all three required cells): the winning architecture
  becomes the default; the losing arm does not ship ungated (phase-3 §5 kill criterion).
  Winner = Arm B: the R6 typed transition lands as a kernel diff in whichever lane owns
  `penstock.rs`, whose flip witness is this DOE's measured columns plus the equality
  court re-run on the adapted kernel. Winner = Arm A: the challenger dies; the kill is
  recorded in the receipt as doctrine confirmed by measurement, never narrated.
- CROSSOVER (winners differ across cells): a measured REGIME MAP (which arm wins which
  regime) is mandatory output — a crossover claimed without a measured regime map is
  the phase-3 §5 kill class. Pre-registered crossover default: the winner of the WORST
  CELL (largest min-of-N ns/frame among S1/S2/S3) becomes the default arm — the CTQ's
  flat-tail law makes the worst cell the binding constraint. A selectivity-adaptive
  runtime hybrid at ingress is UNSUPPORTED in this DOE by design: selectivity is not
  observable before classification, and a selectivity predictor would itself be a
  data-dependent branch — the very architecture under test. Adopting one is new lane
  scope, not this DOE's decision. The regime map and worst-cell default are recorded as
  doctrine-learning (R3), never argued away.

## 8. Anti-Vacuity Mutation Court

Named mutation (dispatch; membrane-spec ingress row adopted verbatim): insert a
data-dependent branch OR an escaped allocation into the WINNING arm; the measured
column must shift, else the benchmark is VACUOUS.

Protocol (sandboxed; sha256 fail-closed — membrane spec §anti-vacuity, penstock §8.3
discipline):

1. Copy the tree to a /tmp sandbox (never mutate the shared tree).
2. sha256 the bench file + `crates/bench/Cargo.toml` (+ every touched file) BEFORE any
   edit.
3. M-BR-1 (primary flip): into the winning arm's hot loop insert a data-dependent
   early-out keyed on a payload byte (shape: `if slab[off + 5] == 0xA5 { continue; }`).
   Predicted effect (TARGET): ns/frame moves beyond the min-of-N spread. Direction is
   NOT pre-pinned — a coherent predictor may speed or slow the mutated form; the claim
   under test is SHIFT, not improvement.
4. M-BR-2 (second flip, allocation class): inject an ESCAPED heap allocation into the
   measured closure (a Vec written during the loop and read after it) — the escape
   dodges the dead-malloc-elision class (a20-wave3 M4a, cited in the penstock wave-6
   notes, penstock.rs:72-74); the Divan alloc column must leave 0 B.
5. Re-run the bench in the sandbox; the named column must shift beyond the baseline
   run-to-run spread.
6. Restore byte-exact; sha256 must match step 2 (fail-closed on mismatch).
7. Verdict: column shifted -> bench non-vacuous. Column invariant -> bench is VACUOUS
   and the TARGET -> MEASURED graduation is REFUSED (exit 1). A court that cannot flip
   is FAIL; a skip is loud (`[SKIP] <reason>`), never silent.

## 9. Receipt Template

Receipt file: `docs/jira/v26.11.1/waves/bench-doe3-penstock-branch-receipt.md`
(membrane discipline: `bench-<lane>-receipt.md`). Required fields:

```
# bench-doe3-penstock-branch receipt
- date; host (CPU, RAM, OS); rustc; divan pin; profile
- subject identities: penstock.rs sha256; bench file sha256; corpus seed constant
- commands + exits: cargo test -p cgap-gate --release; cargo bench -p bench
  --bench doe3_penstock_branch (full Divan table text pasted verbatim)
- cell table: per cell x arm -> min-of-N ns/frame, alloc column, round spread
- H3 equality-court verdict + output checksum line
- anti-vacuity record: mutation id (M-BR-1 / M-BR-2), sandbox path, before/after
  sha256, before/after column values, verdict
- regime map + decision (SINGLE-WINNER, or CROSSOVER + worst-cell default)
- standing: columns MEASURED on this receipt; every unmeasured number stays TARGET;
  any refusal carries its broken term
```

## 10. Session Receipts (commands + exits, this session, cwd /Users/sac/cgap)

- `ls /Users/sac/cgap/crates/cgap-gate /Users/sac/cgap/crates/cgap-gate/src` -> exit 0
  (lib.rs + penstock.rs present)
- `wc -l /Users/sac/cgap/crates/cgap-gate/src/penstock.rs` -> 1719 lines
- `grep -n "pub fn\|pub struct\|FrameView" /Users/sac/cgap/crates/cgap-gate/src/penstock.rs`
  -> exit 0 (line cites in §2)
- `ls /Users/sac/cgap/docs/jira/v26.11.1/dflss/doe-3-penstock-branch-architecture.md`
  -> exit 1 (absent before this write; created fresh, not overwritten)
- `ls /Users/sac/cgap/crates/bench /Users/sac/cgap/crates/bench/benches` -> exit 0
  (only `v26_10_31.rs` in benches/; the DOE-3 bench is absent)
- Read `/Users/sac/cgap/crates/bench/Cargo.toml` -> no cgap-gate dependency; divan pin
  is caret `0.1.21` (§5.2 wiring records both)
- `cargo test -p cgap-gate --release` -> `running 11 tests ... 11 passed; 0 failed ...
  finished in 7.11s`, exit 0 — Arm A subject ALIVE this session (§2)

## 11. Instrument Status Table

| Instrument | Status (this session) |
| --- | --- |
| `crates/cgap-gate/src/penstock.rs` (Arm A subject) | ALIVE — release suite 11/11, exit 0 (§10) |
| `cgap-gate` public surface (lib.rs `pub mod penstock;`) | ALIVE — the 11-test suite exercises it; ingress/FrameView/consts are pub |
| Divan membrane `crates/bench` (manifest + `[[bench]] v26_10_31`) | UNVERIFIED — not built this session; manifest read (§10) |
| `crates/bench/benches/doe3_penstock_branch.rs` | UNVERIFIED — absent on disk; §5 specifies it |
| `cgap-gate` path dep in `crates/bench/Cargo.toml` | UNVERIFIED — absent today; §5.2 wiring required |
| Anti-vacuity sandbox court (M-BR-1/M-BR-2) | UNVERIFIED — specified, not executed this session |

## 12. Divergences (recorded, not silent)

1. Membrane manifest gap: `bench-membrane-spec.md`'s `[dependencies]` shape lists only
   cgap-arrow / cgap-blake3 / cgap-parquet; DOE-3 needs `cgap-gate` as a path dep.
   §5.2 item 2 adds it; the membrane spec yields or records (no silent clobber).
2. Seed-state choice: charter Phase 2 pins `0xBF58_476D_1CE4_E5B9` as the corpus seed;
   the landed in-file fence uses `CORPUS_SEED = 0xC6A4_A793_8547_1D01` as SplitMix64
   state with `0xBF58_476D_1CE4_E5B9` as the second-round multiplier (penstock.rs:765,
   :774). DOE-3 corpora use the charter constant as state; the in-file fence is
   untouched. Recorded, not silent.
3. Standing vocabulary: sibling `doe-1-atomic-portability-tax.md` carries a
   `PENDING-LAND` token; this spec uses only the dispatch-sanctioned vocabulary
   (TARGET for numbers; UNVERIFIED for instruments not run this session). Recorded.
4. Prior in-file ns/frame witnesses (4.89 / 7.55 / 7.60 ns/frame, 2026-11-01 waves,
   penstock.rs:96-99, :112-114, :188-193) are the fence court's own receipts, cited
   not re-measured by this lane and not comparable 1:1 to Divan columns (different
   instrument); the ~4.0 ns/frame charter nominal stays TARGET in every context.
5. phase-3-analyze-doe.md §6 lists this file as absent on disk; this write closes that
   pointer. The dispatch's overwrite clause did not trigger — the file did not exist.
