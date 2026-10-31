# t09: Baseline Landscape — liboqs and SUPERCOP as Advisory Oracles

| | |
|---|---|
| Ticket | t09 (v26.11.1 PQC wave, agent 10 of 20) |
| Type | Research — survey + bench design, no code |
| Standing | SURVEY-ONLY; liboqs/SUPERCOP API claims grounded from primary sources 2026-10-31 |
| Flags | `UNVERIFIED-KAT` = claim not executed against real vectors this session;
`UNVERIFIED-EXTERNAL` = published number recalled/found, never re-measured here;
both are orientation inputs, not receipted facts |

Note: the v26.11.1 index (`00-index.md`) lists the t09 slot as
`t09-adr19-ctq-amendment.md`; this file is the dispatched name
(`t09-baseline-landscape-liboqs.md`). Flagged for coordinator reconciliation
(open item 1); no row of the index was edited by this lane.

## 1. The liboqs API Surface

liboqs is the Open Quantum Safe project's C library: one build, many PQ
algorithms behind per-family structs. The two structs avec's milestone cares
about (headers read from `open-quantum-safe/liboqs` main, 2026-10-31; exact
field lists drift between releases, so treat as shape not contract):

`OQS_KEM` carries metadata + lengths + function pointers:

```c
typedef struct OQS_KEM {
    const char *method_name;      /* e.g. "ML-KEM-768" */
    const char *alg_version;
    uint8_t claimed_nist_level;   /* 1-5, submitter's claim */
    bool ind_cca;                 /* IND-CCA vs IND-CPA */
    size_t length_public_key;
    size_t length_secret_key;
    size_t length_ciphertext;
    size_t length_shared_secret;
    size_t length_keypair_seed;   /* derandomized keygen (KAT machinery) */
    size_t length_encaps_seed;
    /* fn ptrs: keypair/encaps/decaps (+ *_derand variants for KAT replay) */
} OQS_KEM;
```

The C API is length-first and buffer-caller-allocates:

```c
OQS_KEM *OQS_KEM_new(const char *method_name);   /* NULL if unknown/disabled */
OQS_STATUS OQS_KEM_keypair(const OQS_KEM *kem,
                           uint8_t *public_key, uint8_t *secret_key);
OQS_STATUS OQS_KEM_encaps(const OQS_KEM *kem,
                          uint8_t *ciphertext, uint8_t *shared_secret,
                          const uint8_t *public_key);
OQS_STATUS OQS_KEM_decaps(const OQS_KEM *kem,
                          uint8_t *shared_secret,
                          const uint8_t *ciphertext, const uint8_t *secret_key);
```

`OQS_SIG` is the same shape for signatures:

```c
typedef struct OQS_SIG {
    const char *method_name;
    const char *alg_version;
    uint8_t claimed_nist_level;
    bool euf_cma;             /* EUF-CMA security claim */
    bool suf_cma;             /* SUF-CMA security claim */
    bool sig_with_ctx_support;
    size_t length_public_key;
    size_t length_secret_key;
    size_t length_signature;  /* max; sign returns actual via len out-param */
    /* fn ptrs: keypair / sign / verify (+ *_with_ctx_str variants) */
} OQS_SIG;
```

```c
OQS_SIG *OQS_SIG_new(const char *method_name);
OQS_STATUS OQS_SIG_keypair(const OQS_SIG *sig,
                           uint8_t *public_key, uint8_t *secret_key);
OQS_STATUS OQS_SIG_sign(const OQS_SIG *sig,
                        uint8_t *signature, size_t *signature_len,
                        const uint8_t *message, size_t message_len,
                        const uint8_t *secret_key);
OQS_STATUS OQS_SIG_verify(const OQS_SIG *sig,
                          const uint8_t *message, size_t message_len,
                          const uint8_t *signature, size_t signature_len,
                          const uint8_t *public_key);
```

What a comparison bench would actually call, per (algorithm, operation):
`new` -> allocate pk/sk/ct/ss buffers per the `length_*` fields -> one op ->
check `OQS_SUCCESS` -> tear down. The `_derand`/seed variants matter for KAT
replay (deterministic keygen/encaps), not for speed runs. `claimed_nist_level`
is the submitter's claim, not avec's adjudication — it is exactly the field
that must never be promoted into a receipt by linking.

**Why avec does not depend on it.** The v26.10.31 dependency audit
(`../v26.10.31/82-dependency-audit.md`) receipts the substrate at **zero
runtime external dependencies** (35 pkgs: 11 members + 24 dev/build-only
externals). Discipline 2 of this milestone requires E=0 runtime externals for
every PQC crate. A liboqs link would (a) break the E=0 audit, (b) drag a C
toolchain + build system into `build.rs`, and (c) put a third party's
`claimed_nist_level` one `unsafe` call away from standing in for an avec
conformance claim. All three are rejected.

**Why it is still the right advisory oracle.** The repo already runs this
pattern twice:

- **b3sum** — an independent digest oracle, executed as a real subprocess over
  a deterministic corpus; never a crate dependency
  (`../v26.10.31/26-blake3-engine-spec.md`; `docs/GETTING_STARTED.md` lists it
  as an optional gate tool).
- **pyshacl** — the SHACL court's external engine; when absent, the validator
  refuses loudly (`docs/TROUBLESHOOTING.md` §9) rather than degrading to a
  fake pass.

Same pattern for PQC: a liboqs-linked scratch binary under `/tmp`, built once
outside the workspace, invoked as a subprocess, its output merged as data.
Never referenced by any `Cargo.toml`; absence of the binary is a loud
`SKIP-ORACLE-ABSENT` verdict (pyshacl precedent), never a faked comparison.
The oracle answers "is avec's kernel in the same performance class as the
mature reference" — a question conformance courts cannot ask and throughput
gates must not.

## 2. The SUPERCOP Harness Shape

SUPERCOP (bench.cr.yp.to/supercop.html; page read 2026-10-31) is the
long-running eBACS measurement machine. Its measured surface, per primitive
family:

- `crypto_kem/{keypair,enc,dec}.h` — one header per operation, each declaring
  the function and the size macros (`CRYPTO_PUBLICKEYBYTES`,
  `CRYPTO_SECRETKEYBYTES`, `CRYPTO_CIPHERTEXTBYTES`, `CRYPTO_BYTES`; exact
  macro spellings per the nistapi envelope: UNVERIFIED-EXTERNAL).
- `crypto_sign/{keypair,sign,open}.h` — same envelope for signatures.
- Every implementation is measured as (implementation x compiler) pairs; the
  harness keeps the pair minimizing cycles, then measures the winner across
  message sizes.

Cycle-measurement convention (from the SUPERCOP page): a `cpucycles`
abstraction over the hardware counter (RDTSC/RDPMC on amd64; CNTVCT-class on
aarch64), an explicit `cpucycles_persecond` record, counter read -> compute ->
counter read, every measurement recorded and the program re-run multiple times,
with a robust statistic over the distribution — historically median/quartiles,
recently a stabilized second quartile (StQ2). Its accuracy requirements are
worth stealing wholesale: idle machine, core pinning, known frequency, no
turbo noise.

**The mapping onto avec's ctq_report pattern is direct:**

| SUPERCOP | avec (crates/bench/src/bin) |
|---|---|
| `cpucycles()` around one op | `Instant` deltas around one op (`ctq_report.rs`) |
| `cpucycles_persecond` | ns-first reporting; cycles as derived field (open item 2) |
| robust stat over repeats | min-of-N (`best` over rounds; CTQ-03/05/06 pattern) |
| idle machine + pinning | mini-gage admission: trimmed CV > 2% -> SKIP-NOISY (CTQ-04) |
| StQ2/median over repeats | min-of-N + gage R&R on the timer (`gage_rr.rs`, cv_trimmed < 1%) |

`gage_rr.rs` is the load-bearing piece: the timing infrastructure is itself
gage-validated (N=10,000 reps of a fixed 4 KiB workload, trimmed CV < 1%) per
`../v26.10.31/29-benchmark-harness-plan.md` §3 (DMADV Measure gate). Any
wave-5 comparison bench inherits that gate: no throughput claim from a harness
whose gage failed. Same min-of-N discipline, different vocabulary.

## 3. Reference Performance Classes (all UNVERIFIED-EXTERNAL)

Orientation numbers only. These are published-literature classes for
optimized (NEON/assembly) implementations on Apple-Silicon-class cores; they
exist to sanity-check wave-5 results by an order of magnitude, never to pin a
target, floor, or CTQ limit.

**ML-KEM-768** (Kyber768-class, NEON-optimized, Apple M1 ~3.2 GHz class;
anchor: "Neon NTT: Faster Dilithium, Kyber, and Saber on Cortex-A72 and Apple
M1", TCHES 2022):

| Op | Cycle class | Wall-clock at ~3 GHz |
|---|---|---|
| KeyGen | ~2.5-4 x 10^4 cycles | ~8-13 us |
| Encaps | ~3-4 x 10^4 cycles | ~10-13 us |
| Decaps | ~3.5-5 x 10^4 cycles | ~12-17 us |

**ML-DSA-65** (Dilithium3-class, same NEON literature family; numbers are
thinner than Kyber's — see open item 5):

| Op | Cycle class | Wall-clock at ~3 GHz |
|---|---|---|
| KeyGen | ~10^5 class | ~tens of us |
| Sign | ~10^6 class | ~0.25-0.5 ms |
| Verify | ~3-5 x 10^5 class | ~0.1-0.17 ms |

Class-level relativities worth carrying (UNVERIFIED-EXTERNAL as a class
claim): portable-C reference builds typically land ~5-20x slower than the
optimized builds above; signing dominates verification for ML-DSA by roughly
2-4x; and every number above is frequency-and-microarchitecture dependent —
the cycle counter on Apple Silicon does not normalize across P-core/E-core
scheduling, so cross-machine comparison is advisory-grade by construction.

The ADR-19 lesson (`../v26.10.31/22-adr-register.md`, ADR-19) is exactly
about this class of number: the original 8.5 GiB/s CTQ-06 limit was
spec-derived, physically unattainable on the target hardware, and stayed
FAILing until the operator ratified a re-derived limit through the
`../v26.10.31/48-lsl-rederivation.md` pattern. Published cycles/op numbers
imported as limits would mint the same defect on day one. Orientation, never
targets.

## 4. Comparison-Bench Design for Wave 5

Two processes, one merged artifact, zero shared linkage:

1. **avec side** — an advisory bench binary in `crates/bench/src/bin`
   (sibling of `gage_rr.rs` / `ctq_report.rs`), release-built, measuring
   ML-KEM-768 keygen/encaps/decaps and ML-DSA-65 keygen/sign/verify against
   avec's own kernels. Measurement contract copied from the existing courts:
   - gage admission first (mini-gage trimmed CV <= 2%, else SKIP-NOISY;
     optionally full `gage_rr` validation when the run is load-bearing);
   - KAT-correctness screen before timing — an output that does not reproduce
     the reference (KAT or reference-path) digest disqualifies the row, per
     the CTQ-05/CTQ-06 candidate-disqualification pattern;
   - min-of-N `Instant` loops, `black_box` on every output, no sleeps;
   - same-buffer discipline: all buffers (pk/sk/ct/ss/sig) allocated once
     outside the timed window, inputs reused across rounds, exactly the
     CTQ-02 fill-outside-window pattern; a counting-allocator pass (CTQ-02)
     asserts zero allocation events inside the window;
   - JSON rows to stdout, one per (alg, op), human logs to stderr.
2. **oracle side** — the `/tmp` liboqs scratch binary, built once (section 5),
   invoked as a subprocess over the same (alg, op) grid, emitting the same
   row shape. Absent binary -> `SKIP-ORACLE-ABSENT` row, exit still 0 on the
   advisory bench (loud, not failing: the oracle is context, not a gate).
3. **merge** — a JSON artifact carrying both sides' rows plus the liboqs
   build provenance (version/commit SHA captured at build time). This is a
   new artifact class; `docs/BENCH_FLOORS.json` stays floors-only (open
   item 6).

**The honesty rule.** Comparison outputs are ORACLE data, never CTQ gates.
The advisory bench's exit code is unaffected by any measured number (all
rows `gate: "informational"`, verdict always `REPORT`, the CTQ-06
legacy-row pattern). Specifically:

- no LSL/USL may be pinned from liboqs rows;
- a liboqs number may appear in a receipt only as context with the
  liboqs build SHA cited;
- any future throughput limit for a PQC kernel enters through the ADR-19 /
  48-lsl-rederivation path: ceiling math + measured distribution + architecture
  attribution + retained refutation record, on the exact avec subject;
- conformance remains the only gate: KAT byte-parity outranks every number in
  this ticket (index discipline 1).

## 5. Build Friction Notes (liboqs scratch build)

All scratch, all under `/tmp`, all deletable:

- **Toolchain**: cmake + a C compiler (Apple CLT) + make/ninja. liboqs is
  cmake-only on the build side (UNVERIFIED-EXTERNAL: some releases pull
  submodules; a `--depth 1` clone of the main repo plus its submodules is the
  safe shape).
- **Location**: `mkdir -p /tmp/liboqs-oracle && git clone --depth 1 ...`
  clone, configure, build, and record the HEAD SHA into the merge JSON —
  provenance is the point of the oracle, so the SHA is captured at build
  time, not recalled later. Never inside `/Users/sac/avec`; nothing added to
  any manifest; cleanup is an admitted transition (delete the dir, same
  fan-out cleanup law as lane build roots).
- **Minimal build**: configure only the two algorithms (an
  `OQS_MINIMAL_BUILD`-style flag exists for exactly this — exact syntax
  UNVERIFIED-EXTERNAL, verify against the release's CMakeLists at build
  time), which cuts build time and removes every algorithm avec is not
  comparing against.
- **Expected friction**: arm64 asm/feature flags (Keccak/NEON paths) differ
  by release; macOS system Python may be needed by some cmake generators;
  first build is minutes-class (UNVERIFIED-EXTERNAL), so do it once and keep
  the binary for the wave.

## Open Items

1. **Filename reconciliation.** Index t09 slot is
   `t09-adr19-ctq-amendment.md`; this file is
   `t09-baseline-landscape-liboqs.md`. Coordinator owns the index; one of the
   two names must move (ADR-19 CTQ amendment content is covered in section 3
   here but deserves its own ticket row if the index stands).
2. **Cycles vs ns.** `Instant` gives ns; cycles need a frequency assumption.
   Cleanest: report ns as measured, cycles as a derived field using the
   frequency recorded at run time — decide the JSON schema before wave 5.
3. **Oracle pin + provenance.** Which liboqs release/SHA is the wave-5
   oracle; record at build time; a rebuilt oracle is a new oracle (numbers
   are not comparable across SHAs).
4. **Minimal-build flag syntax.** Verify `OQS_MINIMAL_BUILD` spelling and
   value grammar against the pinned release before relying on it.
5. **ML-DSA-65 numbers.** M1-class sign/verify figures are thinner in the
   literature than Kyber's; before quoting anything tighter than the class
   buckets in section 3, re-check the Neon NTT paper and successors.
6. **Oracle artifact placement.** Where merged comparison JSON lives (new
   `docs/` artifact vs `docs/jira/v26.11.1/` receipt), given
   `docs/BENCH_FLOORS.json` stays floors-only.
7. **SKIP-ORACLE-ABSENT semantics.** Confirm the loud-skip verdict string
   and exit-code convention against how `scripts/validate_ontology.sh`
   refuses when pyshacl is missing (same behavior class, name TBD by wave 5).

## Sources

- liboqs `src/kem/kem.h` and `src/sig/sig.h`, open-quantum-safe/liboqs main
  branch, fetched 2026-10-31 — API shape grounded; field lists drift by
  release (UNVERIFIED-KAT for any exact list at a pinned version).
- SUPERCOP documentation, bench.cr.yp.to/supercop.html, fetched 2026-10-31 —
  measurement convention, cpucycles/StQ2, accuracy requirements.
- Becker, Hwang, Kannwischer, Yang, "Neon NTT: Faster Dilithium, Kyber, and
  Saber on Cortex-A72 and Apple M1", TCHES 2022 — performance-class anchor
  (all numbers here UNVERIFIED-EXTERNAL, orientation only).
- Repo grounding: `crates/bench/src/bin/ctq_report.rs` (min-of-N, mini-gage,
  CTQ gate semantics, counting allocator), `crates/bench/src/bin/gage_rr.rs`
  (gage R&R court, cv_trimmed < 1%), `docs/BENCH_FLOORS.json`,
  `docs/GETTING_STARTED.md` (b3sum/pyshacl tool table),
  `docs/TROUBLESHOOTING.md` §9 (loud refusal on missing oracle),
  `../v26.10.31/82-dependency-audit.md` (zero runtime externals),
  `../v26.10.31/26-blake3-engine-spec.md` (b3sum differential oracle),
  `../v26.10.31/22-adr-register.md` (ADR-19),
  `../v26.10.31/29-benchmark-harness-plan.md` (gage-validated timer),
  `../v26.10.31/48-lsl-rederivation.md` (limit re-derivation pattern).

## See Also

- [t07-hqc-bike-survey.md](t07-hqc-bike-survey.md) — sibling survey; KAT
  provenance posture shared.
- [../00-index.md](../00-index.md) — milestone charter; the five governing
  disciplines this ticket cites.
- `docs/DATA_INDEPENDENCE_AUDIT.md` — the CT-claim vocabulary the CT screens
  must not overreach into.
