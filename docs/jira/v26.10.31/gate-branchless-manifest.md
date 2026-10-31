# gate-branchless-manifest

Wave-7 lane 5 (FR-05 census close). Manifest-adjudicated branchless ABI gate.

## Subject

- Repo: /Users/sac/avec (no git ops performed; lane-owned files only)
- Files: `scripts/audit_branchless.sh`, `scripts/branchless_manifest.toml`
- Spec: docs/jira/v26.10.31/27-branchless-kernel-abi.md; ADR-14

## Diff

1. `scripts/branchless_manifest.toml`: schema docs extended with
   `observed_mnemonics` field; both kernels declare their accepted opcodes
   (transpose: cbz, b, b.ne; predicates: b.lo, b.ne). Ceilings unchanged
   (24 / 2), gate_mode INFORMATIONAL unchanged.
2. `scripts/audit_branchless.sh`: after per-symbol hit counting, hits are
   adjudicated against the manifest (pure awk/POSIX parse, no python):
   - symbol count <= expected_max_count AND all observed mnemonics listed in
     the kernel's `observed_mnemonics` AND scope_pattern matches -> MANIFEST-OK
     (report, no fail);
   - count > ceiling, unaccepted mnemonic, or symbol no manifest kernel
     claims -> GATE FAIL (exit 1) regardless of gate_mode;
   - malformed gate_mode / zero-kernel / missing manifest -> exit 3.
   `BRANCHLESS_MANIFEST` env override added (used for the negative control).
   Positive-control / scope-demo selftest path untouched.

## Gate output (receipt)

- `BRANCHLESS_SCOPE='transpose|predicates' ./scripts/audit_branchless.sh`
  -> exit 0. 4 artifacts audited, scanned=348, artifacts_with_hits=4
  (same symbols counted once per artifact copy):
  - transpose::transpose count=12/artifact (x2 artifacts = 24 aggregate,
    ceiling 24) mnemonics [cbz, b, b.ne] -> MANIFEST-OK (INFORMATIONAL)
  - predicates::filter_quads_eq count=2/artifact (x2 artifacts = 4
    aggregate, ceiling 2 per-symbol) mnemonics [b.lo, b_ne] -> MANIFEST-OK
  - "GATE PASS (all hits adjudicated MANIFEST-OK; zero unexplained hits)"
- `./scripts/audit_branchless.sh --selftest` -> exit 0; positive control
  FIRED (4 banned ops in branchy.o), scope demo 0 hits.
- Negative control (non-vacuity of the manifest path):
  `BRANCHLESS_MANIFEST=<copy with expected_max_count 2 -> 0>` -> exit 1,
  "GATE FAIL ... count>0 or unaccepted mnemonic".
- Default-scope caveat: default SCOPE_RE `ct_` substring-captures non-kernel
  symbols (`release_struct_schema`, `perfect_hash::try_build`); those are
  correctly FAILed as unlisted. Manifest's documented verification scope is
  `transpose|predicates`; that is the scope used for the passing receipt.

## Standing

ALIVE: exact subject executed this session; gate exits 0 on manifest-covered
remainder, 1 on any unexplained hit, 2 on vacuous control.
