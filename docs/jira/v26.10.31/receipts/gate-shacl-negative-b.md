# Receipt: gate-shacl-negative-b

| Field | Value |
|---|---|
| Receipt ID | gate-shacl-negative-b |
| Date | 2026-10-31T15:18:37Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `python3 scripts/shacl_negative_corpus_b.py` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | 85cdb75f82bf1a4e830a69c2b2e4bf9d3a93b217 |
| Tree state | dirty (16 entries) |
| Evidence digest (blake3 of stdout tail) | `7cde0968690bf690848477bf6871fe24f4e152727c2eb1fdcbc9880e4f9b1ca4` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== SHACL negative corpus B (cases N-08..N-14) ==
artifact: docs/jira/v26.10.31/19-ontology-ttl.md
ttl sha256: 56162c0e0990ea3c0621eeb97f23484d345937f646dcdf9dd5034de109b2fc71 (13841 bytes)
engine: pyshacl 0.31.0 / rdflib 7.6.0
baseline self-validation: Conforms=True (canon sha256 dbaf3497fcebfc84..., violations=0)
cross-ref validate_ontology.sh: exit 0 (ANTI-VACUITY PASS)

case   probe                                  verdict            pinned               observed
N-08   fanoutTableEntries=128                 FLIP               REFUSAL-08           ['REFUSAL-08'] [iso-exact]
N-08   property absent                        BINDING-ON-ABSENCE -                    ['conforms=False', 'REFUSAL-07', 'REFUSAL-08', 'REFUSAL-09'] [iso-exact]
N-09   searchTimeVarianceNs=4.0               FLIP               REFUSAL-10           ['REFUSAL-10'] [iso-exact]
N-10   heapAllocationBytes=4096               FLIP               REFUSAL-11           ['REFUSAL-11'] [iso-exact]
N-10   property absent                        BINDING-ON-ABSENCE -                    ['conforms=False', 'REFUSAL-11', 'REFUSAL-12'] [iso-exact]
N-11   branchMissRate=0.003                   FLIP               REFUSAL-12           ['REFUSAL-12'] [iso-exact]
N-11   property absent                        BINDING-ON-ABSENCE -                    ['conforms=False', 'REFUSAL-11', 'REFUSAL-12'] [iso-exact]
N-12   settlementLatencyNs=40000              FLIP               REFUSAL-13           ['REFUSAL-13'] [iso-exact]
N-13   processCapabilityCpk=1.4               FLIP               REFUSAL-14           ['REFUSAL-14'] [iso-exact]
N-14   AUM=49999999.99                        HARBOR(duty-free)  odrl:lt|odrl:gte tile harbor=True duty=False [iso-exact]
N-14   AUM=50000000.00                        DUTY(duty=https://avec.chatman.dev/ontology/26.10.31#regularize) odrl:lt|odrl:gte tile harbor=False duty=True [iso-exact]
N-14   AUM=50000000.01                        DUTY(duty=https://avec.chatman.dev/ontology/26.10.31#regularize) odrl:lt|odrl:gte tile harbor=False duty=True [iso-exact]
N-14   mutant odrl:gte -> odrl:gt             FLIP               boundary 50000000.00 unrouted gap detected [iso-exact]
PC-1   corrupted pin REFUSAL-99 vs observed ['REFUSAL-08'] DETECTED           REFUSAL-99           ['REFUSAL-08']
restores: 13/13 isomorphism-exact (bnode-stable graph hash == baseline; artifact bytes pinned by ttl sha256)
hasValue ABSENCE finding: engine BINDS presence on absence (['N-08', 'N-10', 'N-11'] rejected with ['REFUSAL-07', 'REFUSAL-08', 'REFUSAL-09', 'REFUSAL-11', 'REFUSAL-12']) — the 30-shacl-validation-suite.md implementer note (hasValue vacuous on absence) is REFUTED by pyshacl 0.31.0; no sh:minCount companion needed for presence.

VERDICT: PASS — corpus B (N-08..N-14) flips=7/7 families=Git,Turnstile,ODRL
{"verdict": "PASS", "artifact": "docs/jira/v26.10.31/19-ontology-ttl.md", "ttl_sha256": "56162c0e0990ea3c0621eeb97f23484d345937f646dcdf9dd5034de109b2fc71", "canon_base_sha256": "dbaf3497fcebfc848f22645df4020b2fd3339a37f7d4b0810be8e3f6f2bec759", "engine": "pyshacl 0.31.0 / rdflib 7.6.0", "rows": [{"case": "N-08", "probe": "fanoutTableEntries=128", "verdict": "FLIP", "pinned": "REFUSAL-08", "observed": ["REFUSAL-08"], "restore": "iso-exact"}, {"case": "N-08", "probe": "property absent", "verdict": "BINDING-ON-ABSENCE", "pinned": "-", "observed": ["conforms=False", "REFUSAL-07", "REFUSAL-08", "REFUSAL-09"], "restore": "iso-exact"}, {"case": "N-09", "probe": "searchTimeVarianceNs=4.0", "verdict": "FLIP", "pinned": "REFUSAL-10", "observed": ["REFUSAL-10"], "restore": "iso-exact"}, {"case": "N-10", "probe": "heapAllocationBytes=4096", "verdict": "FLIP", "pinned": "REFUSAL-11", "observed": ["REFUSAL-11"], "restore": "iso-exact"}, {"case": "N-10", "probe": "property absent", "verdict": "BINDING-ON-ABSENCE", "pinned": "-", "observed": ["conforms=False", "REFUSAL-11", "REFUSAL-12"], "restore": "iso-exact"}, {"case": "N-11", "probe": "branchMissRate=0.003", "verdict": "FLIP", "pinned": "REFUSAL-12", "observed": ["REFUSAL-12"], "restore": "iso-exact"}, {"case": "N-11", "probe": "property absent", "verdict": "BINDING-ON-ABSENCE", "pinned": "-", "observed": ["conforms=False", "REFUSAL-11", "REFUSAL-12"], "restore": "iso-exact"}, {"case": "N-12", "probe": "settlementLatencyNs=40000", "verdict": "FLIP", "pinned": "REFUSAL-13", "observed": ["REFUSAL-13"], "restore": "iso-exact"}, {"case": "N-13", "probe": "processCapabilityCpk=1.4", "verdict": "FLIP", "pinned": "REFUSAL-14", "observed": ["REFUSAL-14"], "restore": "iso-exact"}, {"case": "N-14", "probe": "AUM=49999999.99", "verdict": "HARBOR(duty-free)", "pinned": "odrl:lt|odrl:gte tile", "observed": "harbor=True duty=False", "restore": "iso-exact"}, {"case": "N-14", "probe": "AUM=50000000.00", "verdict": "DUTY(duty=https://avec.chatman.dev/ontology/26.10.31#regularize)", "pinned": "odrl:lt|odrl:gte tile", "observed": "harbor=False duty=True", "restore": "iso-exact"}, {"case": "N-14", "probe": "AUM=50000000.01", "verdict": "DUTY(duty=https://avec.chatman.dev/ontology/26.10.31#regularize)", "pinned": "odrl:lt|odrl:gte tile", "observed": "harbor=False duty=True", "restore": "iso-exact"}, {"case": "N-14", "probe": "mutant odrl:gte -> odrl:gt", "verdict": "FLIP", "pinned": "boundary 50000000.00 unrouted", "observed": "gap detected", "restore": "iso-exact"}, {"case": "PC-1", "probe": "corrupted pin REFUSAL-99 vs observed ['REFUSAL-08']", "verdict": "DETECTED", "pinned": "REFUSAL-99", "observed": ["REFUSAL-08"]}], "findings": [], "cross_ref": "cross-ref validate_ontology.sh: exit 0 (ANTI-VACUITY PASS)"}
```

<!-- Tree state: dirty (16 entries) -->

## Lane note (wave-14 lane 15, relaunch)

Corpus B (cases N-08..N-14 of docs/jira/v26.10.31/30-shacl-validation-suite.md)
executed by `scripts/shacl_negative_corpus_b.py` — standalone rdflib+pyshacl,
structured mutations (conforming class instance + exactly one perturbed
property, corpus-namespaced focus nodes), assert-flip discipline (BEFORE
conforms -> MUTANT rejected with the single pinned code -> restore proven by
bnode-stable isomorphic graph hash; artifact bytes pinned by
ttl sha256 `56162c0e0990ea3c0621eeb97f23484d345937f646dcdf9dd5034de109b2fc71`,
13841 bytes, from `19-ontology-ttl.md`).

Flip table (observed codes are the full REFUSAL set in the results graph):

| Case | Probe | Verdict | Pinned | Observed |
|---|---|---|---|---|
| N-08 | fanoutTableEntries=128 | FLIP | REFUSAL-08 | REFUSAL-08 |
| N-08 | property absent | BINDING-ON-ABSENCE | - | REFUSAL-07/08/09 |
| N-09 | searchTimeVarianceNs=4.0 | FLIP | REFUSAL-10 | REFUSAL-10 |
| N-10 | heapAllocationBytes=4096 | FLIP | REFUSAL-11 | REFUSAL-11 |
| N-10 | property absent | BINDING-ON-ABSENCE | - | REFUSAL-11/12 |
| N-11 | branchMissRate=0.003 | FLIP | REFUSAL-12 | REFUSAL-12 |
| N-11 | property absent | BINDING-ON-ABSENCE | - | REFUSAL-11/12 |
| N-12 | settlementLatencyNs=40000 | FLIP | REFUSAL-13 | REFUSAL-13 |
| N-12 | cross-ref | PASS | - | validate_ontology.sh exit 0, ANTI-VACUITY PASS |
| N-13 | processCapabilityCpk=1.4 | FLIP | REFUSAL-14 | REFUSAL-14 |
| N-14 | AUM=49999999.99 | HARBOR (duty-free) | tile | harbor=True duty=False |
| N-14 | AUM=50000000.00 | DUTY (regularize) | tile | harbor=False duty=True |
| N-14 | AUM=50000000.01 | DUTY (regularize) | tile | harbor=False duty=True |
| N-14 | mutant odrl:gte -> odrl:gt | FLIP | boundary unrouted | gap detected at 50000000.00 |
| PC-1 | corrupted pin REFUSAL-99 | DETECTED | - | code pinning not decorative |

NOT-BINDING finding (refutes the suite spec's implementer note): pyshacl
0.31.0 enforces `sh:hasValue` as PRESENCE-BINDING — a bare typed node without
the property is rejected (N-08/Git: REFUSAL-07/08/09; N-10/N-11/Turnstile:
REFUSAL-11/12). The "sh:hasValue is satisfied vacuously when the property is
absent" note in 30-shacl-validation-suite.md is wrong for this engine; no
`sh:minCount 1` companion is needed for presence enforcement. There are no
not-binding shapes in corpus B: every case binds and flips. Related fixture
note (pre-existing, from the TTL provenance comments): `searchTimeVarianceNs`
and `processCapabilityCpk` are absent from the Genesis fixture, so the
inequality shapes (REFUSAL-10/14) are unexercised by the positive fixture —
the negative corpus is what gives them witnessed teeth.

N-14 ODRL layering: SHACL does not evaluate odrl: operator semantics; ODRL
evaluation is application-layer. The tiling is pinned by SPARQL ASK over the
policy graph with odrl:lt -> `FILTER(?aum < ?limit)` and odrl:gte ->
`FILTER(?aum >= ?limit)` executed by the rdflib SPARQL engine (xsd:decimal,
no Python-side money comparison), plus the `odrl:duty avec:regularize`
presence pattern. Boundary 50000000.00 routes to gte (the statutory pin);
mutant odrl:gte -> odrl:gt leaves the boundary unrouted and the court catches
the gap. What is qualified here is constraint-level tiling only; full
duty-discharge evaluation remains application-layer.

Replay: `python3 scripts/shacl_negative_corpus_b.py` (exit 0 = all flips held;
any VACUOUS/CODE-MISS/CODE-EXTRA/RESTORE-MISMATCH/PIN-VACUOUS exits 1).
