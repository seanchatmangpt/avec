# Receipt: gate-shacl-v3

| Field | Value |
|---|---|
| Receipt ID | gate-shacl-v3 |
| Date | 2026-10-31T03:03:37Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `bash /Users/sac/avec/scripts/validate_ontology.sh` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (62 entries) |
| Evidence digest (blake3 of stdout tail) | `09264085e6084470da7ddbbd7c961d9267b0e71cdbbc6db71389c983bf3cfbf7` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== SHACL self-validation: /tmp/avec-shacl.6D6quS/ontology.ttl ==
Validation Report
Conforms: True
== ANTI-VACUITY: PASS (mutant rejected: REFUSAL-13 violation on settlementLatencyNs) ==

VERDICT: PASS — ontology conforms to its own shapes; anti-vacuity court live.
```

<!-- Tree state: dirty (62 entries) -->

## Lane note (wave-7 lane 12)

Fixture provenance refresh (../19-ontology-ttl.md): `avec:settlementLatencyNs
31400` -> `14900` (receipts/gate-e2e-ctq05.md: min-of-2000 = 14.9 us;
receipts/gate-ctq-report-v2.md: CTQ-05 12.875 us). `avec:decodeThroughputGiBs
74.8` retained but marked TARGET-not-measured. `searchTimeVarianceNs` 0.00
design figure retired in a provenance comment (CTQ-04 residual stdev 1508 ns /
mean 16169 ns, receipts/gate-ctq-report-v2.md).

Disclosed minimal unblock to a shared file: `scripts/validate_ontology.sh`
anti-vacuity sed pattern updated `31400` -> `14900` to match the receipted
fixture value (without it the court would FAIL "perturbation did not mutate").
No other validator logic changed.
