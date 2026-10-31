# Receipt: gate-shacl

| Field | Value |
|---|---|
| Receipt ID | gate-shacl |
| Date | 2026-10-31T01:55:09Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `scripts/validate_ontology.sh` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (48 entries) |
| Evidence digest (blake3 of stdout tail) | `aee5c2a1f62427e6335049b598cdf2227cabcef62269910d42d6cfdc3f93abc0` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== SHACL self-validation: /tmp/avec-shacl.Q2mEJ5/ontology.ttl ==
Validation Report
Conforms: False
Results (3):
Constraint Violation in ClosedConstraintComponent (http://www.w3.org/ns/shacl#ClosedConstraintComponent):
	Severity: sh:Violation
	Source Shape: avec:TurnstileAdmissionShape
	Focus Node: avec:Genesis_Turnstile_Event
	Value Node: avec:Genesis_Blake3_Receipt
	Result Path: prov:generated
	Message: Node avec:Genesis_Turnstile_Event is closed. It cannot have value: avec:Genesis_Blake3_Receipt
Constraint Violation in ClosedConstraintComponent (http://www.w3.org/ns/shacl#ClosedConstraintComponent):
	Severity: sh:Violation
	Source Shape: avec:TurnstileAdmissionShape
	Focus Node: avec:Genesis_Turnstile_Event
	Value Node: avec:TurnstileAdmission
	Result Path: rdf:type
	Message: Node avec:Genesis_Turnstile_Event is closed. It cannot have value: avec:TurnstileAdmission
Constraint Violation in ClosedConstraintComponent (http://www.w3.org/ns/shacl#ClosedConstraintComponent):
	Severity: sh:Violation
	Source Shape: avec:TurnstileAdmissionShape
	Focus Node: avec:Genesis_Turnstile_Event
	Value Node: prov:Activity
	Result Path: rdf:type
	Message: Node avec:Genesis_Turnstile_Event is closed. It cannot have value: prov:Activity
== ANTI-VACUITY: PASS (mutant rejected: REFUSAL-13 violation on settlementLatencyNs) ==

VERDICT: NON-CONFORMANT — Genesis fixture violates its own shapes (violations above).
```

<!-- Tree state: dirty (48 entries) -->
