# Diataxis Documentation — `cgap` v26.10.29

Documentation for the freestanding `cgap` Apache Arrow & Parquet columnar substrate, organized according to the [Diataxis](https://diataxis.fr/) four-quadrant architecture.

| Quadrant | Focus | Document |
|---|---|---|
| **1. Tutorials** | Learning-oriented, hands-on step-by-step introduction | [tutorials/getting-started.md](tutorials/getting-started.md) — Building 64B-aligned batches, compressed-domain execution, and `cgap-blake3` Merkle hashing |
| **2. How-To Guides** | Task-oriented recipes for specific operational goals | [how-to/zero-copy-gpu-and-blake3.md](how-to/zero-copy-gpu-and-blake3.md) — Zero-copy Apple Silicon Metal Unified Memory wrapping, SIMD BLAKE3 parent reduction, and gate verification |
| **3. Reference** | Information-oriented API, crate layout, and CTQ specifications | [reference/crates-and-apis.md](reference/crates-and-apis.md) — Complete crate inventory (`cgap-arrow`, `cgap-blake3`, `cgap-parquet`, `cgap-gpu`, `cgap-ffi`, `cgap-beam`) and API signatures |
| **4. Explanation** | Understanding-oriented hardware & architectural trade-offs | [explanation/architecture-and-phase-changes.md](explanation/architecture-and-phase-changes.md) — Civilizational phase changes, `BODY 0/0` branchless execution, and the 16 MiB CPU-SIMD vs. GPU crossover law |

## Related Canonical Ledger Files
- [DFLSS Charter](../DFLSS_CHARTER.md) — DMADV scope, CTQs, FMEA, and verification receipts.
- [Phase-Change Receipts](../PHASE_CHANGE_RECEIPTS.md) — Scoreboard of measured hardware claims.
- [Content Addressing](../CONTENT_ADDRESSING.md) — Mutation-detection fold vs. cryptographic `cgap-blake3` Merkle identity.
- [Branch Register](../BRANCH_REGISTER.md) — Audited machine-code branch budgets per symbol.
