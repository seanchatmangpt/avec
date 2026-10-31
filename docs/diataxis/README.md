# AVEC Documentation Router

Task-oriented hub for the v26.10.31 milestone corpus
([docs/jira/v26.10.31/](../jira/v26.10.31/) — 72 files plus the machine-minted
[receipts/](../jira/v26.10.31/receipts/) directory). Entries route by **what
you want to do**, never by wave number. For the quadrant-by-quadrant narrative
set (one worked tutorial, one hardware how-to, one API reference, one
architecture explanation), see [INDEX.md](INDEX.md).

Every link below was verified to exist on disk at write time (2026-10-30).
Planned artifacts from other wave-10 lanes that have not landed are written as
plain text with a `pending wave-10 lane N` marker — never as a link.

## 1. Tutorials — see the substrate work end to end

Learning-oriented; follow start to finish.

- [tutorials/getting-started.md](tutorials/getting-started.md) — build
  64B-aligned batches, run compressed-domain predicates, and hash with
  `avec-blake3`; the hands-on walkthrough for the substrate core.
- [examples/c99_consumer/](../../examples/c99_consumer/) — a foreign-language
  (C99) consumer over the substrate; run via `scripts/c99_smoke.sh`.
- `GETTING_STARTED.md` — pending wave-10 (milestone-wide walkthrough; not yet
  landed in this tree).
- `examples/sovereign-e2e` — pending wave-10 lane 12 (end-to-end sovereign
  pipeline example; not yet landed in this tree).

## 2. How-to guides — one task, one doc

Each entry is a job to do; commands are the real invocations the docs and
receipts use.

- **Run the full gate battery** —
  [verify-README.md](../jira/v26.10.31/verify-README.md):
  `./scripts/verify_v26.10.31.sh`; latest full result in
  [verify-latest.md](../jira/v26.10.31/verify-latest.md).
- **Run one court** —
  [52-courts-audit.md](../jira/v26.10.31/52-courts-audit.md) inventories every
  court family with its non-vacuity verdict; run a single suite as
  `cargo test -p <crate> <court_name>`.
- **Mint a receipt** —
  [receipts/README.md](../jira/v26.10.31/receipts/README.md):
  `scripts/mint_receipt.sh <name> <command>`; append-only, fail-closed.
- **Read the benchmark numbers** —
  [29-benchmark-harness-plan.md](../jira/v26.10.31/29-benchmark-harness-plan.md)
  defines the CTQ set; `target/release/ctq_report` (source:
  [crates/bench/src/bin/ctq_report.rs](../../crates/bench/src/bin/ctq_report.rs))
  emits the JSON verdict — exit 1 iff a REAL CTQ gate fails.
- **Ratify a CTQ** —
  [59-ratification-package.md](../jira/v26.10.31/59-ratification-package.md)
  is assembled for operator decision, nothing pre-flipped; see also
  [60-release-readiness.md](../jira/v26.10.31/60-release-readiness.md);
  `OPERATOR_DECISIONS` — pending wave-10 lane 16.
- **Add a branchless kernel to the manifest** — header of
  [scripts/branchless_manifest.toml](../../scripts/branchless_manifest.toml)
  (schema + inventory-completeness law) plus
  [27-branchless-kernel-abi.md](../jira/v26.10.31/27-branchless-kernel-abi.md)
  §5; gate: [scripts/audit_branchless.sh](../../scripts/audit_branchless.sh).
- **Add a SHACL shape** —
  [19-ontology-ttl.md](../jira/v26.10.31/19-ontology-ttl.md) plus
  [scripts/validate_ontology.sh](../../scripts/validate_ontology.sh).
- **Interpret a red gate** — `TROUBLESHOOTING.md` — pending wave-10 lane 9
  (not yet landed in this tree).
- **Re-mint stale receipts** — `scripts/receipt_doctor.sh` — pending wave-10
  lane 8 (not yet landed in this tree).

## 3. Reference — look it up

Information-oriented; for checking a fact, not for reading through.

- **Requirements**: [20-prd.md](../jira/v26.10.31/20-prd.md),
  [21-ard.md](../jira/v26.10.31/21-ard.md)
- **Design specs**: [23-64b-quad-spec.md](../jira/v26.10.31/23-64b-quad-spec.md),
  [24-parquet-projection-spec.md](../jira/v26.10.31/24-parquet-projection-spec.md),
  [25-git-packfile-integration-spec.md](../jira/v26.10.31/25-git-packfile-integration-spec.md),
  [26-blake3-engine-spec.md](../jira/v26.10.31/26-blake3-engine-spec.md),
  [27-branchless-kernel-abi.md](../jira/v26.10.31/27-branchless-kernel-abi.md),
  [28-beam-nif-handoff.md](../jira/v26.10.31/28-beam-nif-handoff.md)
- **Traceability**:
  [32-traceability-matrix.md](../jira/v26.10.31/32-traceability-matrix.md) —
  PRD to ARD to specs to verification, bidirectional
- **Glossary**: [37-glossary.md](../jira/v26.10.31/37-glossary.md)
- **Receipts inventory**:
  [64-receipts-inventory-final.md](../jira/v26.10.31/64-receipts-inventory-final.md)
  — as-of HEAD `ead4032`; supersedes the corpus tables in
  [50-receipts-audit.md](../jira/v26.10.31/50-receipts-audit.md)

## 4. Explanation — understand why

Understanding-oriented; read when you need the reasoning, not the procedure.

- [18-m1-branchless-algebra.md](../jira/v26.10.31/18-m1-branchless-algebra.md)
  — why M=1: the algebraic/topological case for branchless kernels.
- [16-ahdclt.md](../jira/v26.10.31/16-ahdclt.md) — AHDCLT (adversarial
  hyperdimensional cognitive load theory); UNVERIFIED doctrine, labeled as
  such in the doc itself.
- [48-lsl-rederivation.md](../jira/v26.10.31/48-lsl-rederivation.md) —
  first-principles re-derivation of the CTQ-06 BLAKE3 lower spec limit.
- [69-validation-receipt.md](../jira/v26.10.31/69-validation-receipt.md) —
  independent 17-lane validation sweep with a findings register.
- [00-index.md](../jira/v26.10.31/00-index.md) — the whole story: hub for the
  full 00-19 context and 20-39 spec series.

## See Also

- [INDEX.md](INDEX.md) — quadrant narrative set (tutorial / how-to /
  reference / explanation guides)
- [DFLSS Charter](../DFLSS_CHARTER.md) — DMADV scope, CTQs, FMEA
- [Phase-Change Receipts](../PHASE_CHANGE_RECEIPTS.md) — measured hardware
  claims scoreboard
- [Branch Register](../BRANCH_REGISTER.md) — audited machine-code branch
  budgets per symbol
