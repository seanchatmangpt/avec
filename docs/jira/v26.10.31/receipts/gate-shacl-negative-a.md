# Receipt: gate-shacl-negative-a

| Field | Value |
|---|---|
| Receipt ID | gate-shacl-negative-a |
| Date | 2026-10-31T15:17:04Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `python3 scripts/shacl_negative_corpus.py` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | 85cdb75f82bf1a4e830a69c2b2e4bf9d3a93b217 |
| Tree state | dirty (14 entries) |
| Evidence digest (blake3 of stdout tail) | `59ef9eed3087f2f2510b8fb71d474d59ae9c3f8ea660715cb2c57595bb65b74c` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== SHACL negative corpus court (N-01..N-07) ==
engine: pyshacl 0.31.0 / rdflib 7.6.0
artifact: docs/jira/v26.10.31/19-ontology-ttl.md
ttl sha256: 56162c0e0990ea3c0621eeb97f23484d345937f646dcdf9dd5034de109b2fc71
base self-validation: Conforms True, REFUSAL codes: []
self-check (code-pinning positive control): PASS — corrupted expectation REFUSAL-99 -> NOT-BINDING, court not vacuous

CASE  MUTATION                                    EXPECTED    OBSERVED                CONTROL   STATUS
N-01  cacheLineAlignmentBytes 64 -> 32            REFUSAL-01  REFUSAL-01              {}        FLIPPED
N-02  nullBitmapPresent false -> true             REFUSAL-02  REFUSAL-02              {}        FLIPPED
N-03  cyclomaticComplexity 1 -> 2 (kernel M=2)    REFUSAL-03  REFUSAL-03              {}        FLIPPED
N-04  definitionLevelByteLength 0 -> 4            REFUSAL-04  REFUSAL-04              {}        FLIPPED
N-05  repetitionLevelByteLength 0 -> 1            REFUSAL-05  REFUSAL-05              {}        FLIPPED
N-06  decodeThroughputGiBs 74.8 -> 60 (< LSL 70)  REFUSAL-06  REFUSAL-06              {}        FLIPPED
N-07  searchLadderStages 16 -> 32                 REFUSAL-09  REFUSAL-09              {}        FLIPPED

NOTE code-mapping divergence: N-07: suite-table code REFUSAL-07 -> artifact code REFUSAL-09 (artifact wins)
NOTE control runs may carry ClosedConstraintComponent results on rdf:type (plane shapes are sh:closed with no ignoredProperties); these carry no REFUSAL code and the binding assertion is on the pinned-code set.
NOTE probed on pyshacl 0.31.0: sh:hasValue VIOLATES on absent property (not vacuous) — mutation nodes are fully populated with pinned values except the perturbed path; the 30-shacl-validation-suite.md vacuity note is contradicted.

VERDICT: 7/7 cases FLIPPED, self-check PASS — exit 0
```

<!-- Tree state: dirty (14 entries) -->
