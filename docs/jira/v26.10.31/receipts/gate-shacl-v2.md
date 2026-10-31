# Receipt: gate-shacl-v2

| Field | Value |
|---|---|
| Receipt ID | gate-shacl-v2 |
| Date | 2026-10-31T02:15:58Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `bash /Users/sac/avec/scripts/validate_ontology.sh` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (53 entries) |
| Evidence digest (blake3 of stdout tail) | `217caff42d18a4a787235fe6997395579a134716f0c24eacdc70a6379fc0570c` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== SHACL self-validation: /tmp/avec-shacl.kDjvL2/ontology.ttl ==
Validation Report
Conforms: True
== ANTI-VACUITY: PASS (mutant rejected: REFUSAL-13 violation on settlementLatencyNs) ==

VERDICT: PASS — ontology conforms to its own shapes; anti-vacuity court live.
```

<!-- Tree state: dirty (53 entries) -->
