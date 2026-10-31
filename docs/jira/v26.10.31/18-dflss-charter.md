# DFLSS Project Charter: AVEC Substrate (DMADV)

Project Identifier: DFLSS-AVEC-2026-Q4
Classification: core sub-BEAM physical data substrate
Methodology: Design for Lean Six Sigma (DMADV) / Toyota Production System
Status: UNVERIFIED — charter targets only, no phase-gate criteria executed.

## 1. Business Case and Problem Statement

The legacy columnar/warehouse stack is structurally incapable of meeting
deterministic-latency and audit requirements:

- Pipeline flushes and TLB thrash destroy over 90% of available CPU cycles;
  current state is equivalent to Cp < 0.67 (far below any capable process).
- Cloud warehouse rent runs $50k-$250k/month for comparable throughput.
- Audit non-determinism breaks sub-2us distance-bounding: the same logical
  read produces different physical timings, so proofs are non-replicable.

### Desired State

- no_std substrate, single-assignment everywhere (M=1).
- Decode throughput at or above 74 GiB/s.
- Zero heap bytes allocated on the hot path.
- Settlement latency at or below 35us.
- Process capability C_pk at or above 1.67 (6 sigma, at most 3.4 DPMO).

### Consequence of Inaction

Every quarter on the legacy stack compounds rent, audit exposure, and
latency-pathology remediation cost. The substrate cannot be retrofitted;
it must be designed (DMADV), not improved (DMAIC).

## 2. VOC to CTQ Flowdown

| Voice of the Customer | CTQ | Quantified Target |
| --- | --- | --- |
| Execution must be deterministic / no side channels | M=1 single-path | branch-miss 0.000% |
| Reads must saturate memory controllers | decode bandwidth | at least 74 GiB/s; at least 95%
bandwidth saturation |
| Audits must be non-repudiable | per-node crypto verification | at most 340 ns/node; invariant OID
search time |
| No stalls under concurrency | hot-path allocation discipline | 0 heap bytes hot path; settlement
at most 35us |

## 3. Scope Boundaries

### In Scope

- Fixed-width, non-nullable, 64-byte-aligned Arrow vectors.
- Level-free DataPageV2 encoding.
- 16-stage `.idx` v2 ladder.
- BLAKE3 tree reduction for content addressing and audit receipts.
- NEON / AVX2 / WASM kernels.
- no_std C-ABI surface plus zero-copy NIF handoffs.

### Out of Scope

- RDF / SHACL semantics (owned by stardf / graphlaw).
- Actor lifecycle, supervision, hybrid logical clocks (owned by BEAM/OTP).
- Network sockets and RPC transport.
- GC or heap-allocated text representations.

## 4. CTQ Quantitative Table

| ID | CTQ | Baseline | Target | Spec Limit |
| --- | --- | --- | --- | --- |
| CTQ-01 | M (assignments per path) | 18+ | 1 | USL = 1 |
| CTQ-02 | Hot-path allocations per block | 14,000 | 0 | 0 tolerated |
| CTQ-03 | Decode throughput | 14.2 GiB/s (arrow-rs) | 74.0 GiB/s or more | LSL = 70.0 GiB/s |
| CTQ-04 | OID lookup variance | sigma ~ 120 ns | sigma^2 = 0 | USL = 1.5 ns |
| CTQ-05 | Admission round-trip | 400 ms - 2.4 s | 35us or less | USL = 50us |
| CTQ-06 | Hash throughput per core | 450 MiB/s (SHA-256) | 10 GiB/s/core or more | LSL = 8.5 GiB/s
|

All values are charter targets, not measured results.

## 5. Phase-Gate Plan (DMADV)

| Phase | Deliverables | Exit Criteria |
| --- | --- | --- |
| Define | Charter, VOC-to-CTQ flowdown, scope boundaries | Charter signed; CTQs quantified |
| Measure | Measurement system analysis; baseline capture | Gage R&R on CNTVCT_EL0 / RDTSC with
under 1% variance |
| Analyze | Failure-mode analysis; capability model | CFG homology proof (source vs emitted control
flow) |
| Design | Optimized kernels; page/idx formats; DFMEA controls | DFMEA RPN below thresholds;
poka-yoke in place |
| Verify | Statistical validation; static verification | llvm-objdump M=1 static verification; Divan
stats with C_pk at least 1.67 |

The Gage R&R gate validates that the timer infrastructure itself is a
capable measurement system before any throughput claim is admitted.

## 6. DFMEA High-Severity Table

| Failure Mode | S | O | D | RPN | Control |
| --- | --- | --- | --- | --- | --- |
| LLVM de-optimization jump injection | 9 | 7 | 2 | 126 | naked asm + objdump CI gate |
| Cache-line splits | 7 | 5 | 3 | 105 | `repr(C, align(64))` + `posix_memalign` |
| .idx large-offset MSB overflow | 8 | 4 | 3 | 96 | branchless MSB extraction |
| SIMD register spill | 6 | 4 | 2 | 48 | register-bank limits per kernel |

Mitigation threshold: any mode with RPN at or above 48 requires an
engineered control (poka-yoke), not review discipline.

## 7. Governance

- Sponsor: Sean Chatman.
- Methodology: DFLSS / TPS.
- Validation environment: Apple M3 Max.
- Formal target: MISRA-C / ISO 26262 ASIL-D single-path equivalence.

### Poka-Yoke Execution Checklist

1. Align every hot-path structure to 64 bytes; enforce by type, not review.
2. Compile hot kernels to fixed instruction sequences; diff against objdump
   in CI; any drift fails the build.
3. Replace every branch on hot paths with branchless arithmetic; verify by
   CFG homology.
4. Assert zero-allocation hot paths at runtime in tests; any heap byte on
   the hot path fails loudly.

## 8. P28-SIL DFLSS Addendum: License as Evidentiary Substrate

Traditional licensing is a litigation instrument; P28-SIL makes the license
an evidence instrument. BLAKE3 receipts bind usage into a non-deniable
record, converting disputes into arithmetic.

### Commercial Mechanics

- $50M safe harbor: zero-friction for over 99% of adopters.
- Above the safe-harbor threshold: commercial regularization plus an
  irrevocable covenant not to sue upon regularization.
- FASB ASC 450 probable-and-estimable accrual plus SOX 302/404
  material-weakness mechanics replace litigation as the enforcement path:
  unregularized usage becomes a balance-sheet liability the adopter's own
  auditors must surface.

### DFMEA: Traditional Licensing vs P28-SIL

| Traditional Failure Mode | P28-SIL Control |
| --- | --- |
| Willful infringement | Receipted usage makes intent arithmetic, not argument |
| Injunction risk | Covenant-not-to-sue upon regularization removes the remedy |
| License phobia (adoption chilling) | $50M safe harbor: zero friction below threshold |
| Evidentiary disputes | BLAKE3 content-addressed receipts; non-deniable |

### Binding into the Turnstile

Settlement entropy delta and the lease invariant bind under the P28-SIL
sum: `ΔS + Φ_lease ≡ R_receipt (mod Σ_P28-SIL)`. A settlement without a
BLAKE3 receipt is not a settlement; a lease without a receipt is not a
lease. The license, the audit trail, and the turnstile are one substrate.

## Measure-gate status (wave 5, 2026-10-30)

- **Gage R&R bin**: landed (wave-5 lane 6) at
  `crates/bench/src/bin/gage_rr.rs`. Its actual CV result is not yet
  receipted — no `receipts/gate-*` file covers it. Status:
  **PENDING-RECEIPT**; the Measure-gate exit criterion (timer variance
  under 1%) remains unadmitted until that receipt lands.
- **Divan harness**: landed (wave 3) at
  `crates/bench/benches/v26_10_31.rs` and
  `crates/bench/src/benches/v26_10_31.rs`.
- **divan2receipt**: landed (wave-5 lane 7) per the wave-5 contract; no
  conversion receipt exists yet in `docs/jira/v26.10.31/receipts/` — also
  **PENDING-RECEIPT**.
- **CTQ-05 (admission round-trip)** and **CTQ-06 (hash throughput per
  core)** currently FAIL per `ctq_report` output; owner lanes assigned at
  wave-5 dispatch. The FAILs are recorded as-is, not rounded.

Charter targets remain targets until receipts flip them. No CTQ in
section 4 is admitted as met by this status section.

## Measure-gate status (wave 6, 2026-10-30)

- **Gage R&R bin**: landed (wave-6 L5). `target/release/gage_rr` built.
  Still **PENDING-RECEIPT**: no `receipts/gate-*` file covers a quiet-run
  CV result; the Measure-gate exit criterion (variance under 1%) remains
  unadmitted until that receipt lands.
- **ctq_report v2**: landed. Latest observed run this session
  (exit 1, verdict FAIL, failed_gates=[CTQ-06]):
  - CTQ-02 zero-alloc: alloc_events=0 over 64 MiB, 0.0529 s -> PASS
  - CTQ-04 transpose variance: stdev/mean=6.68%, USL 15% -> PASS
  - CTQ-05 e2e admission: 12.916 us/batch vs USL 35 us,
    path=hash_many_simd -> PASS
  - CTQ-03 parquet roundtrip: informational REPORT only, no gate
  - CTQ-06 BLAKE3: hash_wide_simd 0.43 GiB/s, hash_many_simd 1.18 GiB/s
    vs LSL 8.5 GiB/s each (min-of-5, 256 MiB) -> FAIL both rows;
    owner lanes wave-6 L1-L3
- **C_pk capability study**: wired through the CTQ gates' REAL verdicts
  (CTQ-02/04/05/06 gate=REAL with pinned LSL/USL); no standalone C_pk
  symbol exists in `ctq_report.rs`/`gage_rr.rs` on disk.
- Prior wave-5 PENDING-RECEIPT items (gage_rr quiet-run CV,
  divan2receipt conversion) remain open.

Charter targets remain targets until receipts flip them. No CTQ in
section 4 is admitted as met by this status section.

---

Charter targets, not measured results.

See Also: 04-benchmark-contract.md, 17-post-2028-computing.md,
19-press-release.md
