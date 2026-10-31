# AVEC v26.10.31 Canonical Ontology (Turtle)

## Public Ontology Mapping Matrix (Prior-Art First)

Zero domain-specific schema invention. Every concept maps to a published vocabulary, so
standard triplestores and SHACL engines consume the artifact directly.

| Concern | Vocabulary | Mapping |
|---|---|---|
| Execution lineage, merkle turnstiles | PROV-O (`prov:`) | `prov:Activity` (turnstile admission,
kernel execution), `prov:Entity` (batches, pages, packfiles, receipts), `prov:wasGeneratedBy`,
`prov:wasDerivedFrom` |
| Artifacts and distributions | DCAT 3 (`dcat:`) | `dcat:Distribution` with `dcat:mediaType`
(`application/vnd.apache.arrow.stream`, `application/vnd.apache.parquet`), `dcat:byteSize` |
| Statutory / license invariants | ODRL 2.2 (`odrl:`) | P28-SIL duties, EU AI Act log-emission duty,
monetary constraints as `odrl:Permission` / `odrl:Duty` / `odrl:Constraint` |
| Event / object lineage | OCEL 2.0 (`ocel:`) | event-to-object relations over the three planes |
| Digests | W3C Security / multihash (`sec:`) | `sec:blake3` digest identification on receipts |

The `TermId` integer mapping (`p_id` column) enables DuckDB filter pushdown on predicate
equality: a predicate lookup is a `u32` compare inside the 64-byte quad, no string interning
at scan time.

## Full Turtle Artifact

```turtle
@prefix owl:   <http://www.w3.org/2002/07/owl#> .
@prefix rdf:   <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs:  <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd:   <http://www.w3.org/2001/XMLSchema#> .
@prefix dcterms: <http://purl.org/dc/terms/> .
@prefix prov:  <http://www.w3.org/ns/prov#> .
@prefix dcat:  <http://www.w3.org/ns/dcat#> .
@prefix odrl:  <http://www.w3.org/ns/odrl/2/> .
@prefix ocel:  <https://w3id.org/ocel/2/> .
@prefix sec:   <https://w3id.org/security#> .
@prefix sh:    <http://www.w3.org/ns/shacl#> .
@prefix avec:  <https://avec.chatman.dev/ontology/26.10.31#> .

<https://avec.chatman.dev/ontology/26.10.31> a owl:Ontology ;
    owl:versionInfo "26.10.31" ;
    dcterms:title "AVEC Canonical Ontology"@en ;
    dcterms:license <https://avec.chatman.dev/license/P28-SIL> ;
    dcterms:publisher <https://avec.chatman.dev> .

### ---------- Classes ----------

avec:SubstrateTriad a owl:Class ;
    rdfs:label "Substrate Triad" ;
    rdfs:comment "Arrow execution + Parquet persistence + Git sequencing planes." .

avec:ArrowExecutionPlane a owl:Class ;
    rdfs:subClassOf avec:SubstrateTriad ;
    rdfs:label "Arrow Execution Plane" .

avec:ParquetPersistencePlane a owl:Class ;
    rdfs:subClassOf avec:SubstrateTriad ;
    rdfs:label "Parquet Persistence Plane" .

avec:GitSequencingPlane a owl:Class ;
    rdfs:subClassOf avec:SubstrateTriad ;
    rdfs:label "Git Sequencing Plane" .

avec:ColumnarQuadLayout a owl:Class ;
    rdfs:label "Columnar Quad Layout" ;
    rdfs:comment "64-byte quad: 4x u32 IDs + timestamp + vector clock + policy mask + digest." .

avec:HardwareKernel a owl:Class ;
    rdfs:label "Hardware Kernel" ;
    rdfs:comment "Target-specific SIMD kernel: M=1, one basic block, zero internal edges." .

avec:TurnstileAdmission a owl:Class ;
    rdfs:subClassOf prov:Activity ;
    rdfs:label "Turnstile Admission" .

avec:Blake3MerkleReceipt a owl:Class ;
    rdfs:subClassOf prov:Entity ;
    rdfs:label "BLAKE3 Merkle Receipt" .

### ---------- Datatype properties ----------

avec:cyclomaticComplexity a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt ;
    rdfs:comment "Kernel complexity M (must be 1)." .
avec:basicBlockCount a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:internalEdgeCount a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:branchMissRate a owl:DatatypeProperty ;
    rdfs:range xsd:decimal .
avec:heapAllocationBytes a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedLong .
avec:cacheLineAlignmentBytes a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:nullBitmapPresent a owl:DatatypeProperty ;
    rdfs:range xsd:boolean .
avec:definitionLevelByteLength a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:repetitionLevelByteLength a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:decodeThroughputGiBs a owl:DatatypeProperty ;
    rdfs:range xsd:decimal .
avec:fanoutTableEntries a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:searchLadderStages a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:searchTimeVarianceNs a owl:DatatypeProperty ;
    rdfs:range xsd:decimal .
avec:packfileVersion a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedInt .
avec:admittedStateDelta a owl:DatatypeProperty ;
    rdfs:range xsd:string .
avec:monotonicExecutionLease a owl:DatatypeProperty ;
    rdfs:range xsd:string .
avec:turnstileReceipt a owl:DatatypeProperty ;
    rdfs:range xsd:string .
avec:settlementLatencyNs a owl:DatatypeProperty ;
    rdfs:range xsd:unsignedLong .
avec:processCapabilityCpk a owl:DatatypeProperty ;
    rdfs:range xsd:decimal .

### ---------- 64-byte quad schema ----------

avec:QuadField a owl:Class ;
    rdfs:label "Quad Field" .

avec:quadField a owl:ObjectProperty ;
    rdfs:domain avec:ColumnarQuadLayout ;
    rdfs:range avec:QuadField .

avec:byteOffset a owl:DatatypeProperty ; rdfs:range xsd:unsignedInt .
avec:byteWidth a owl:DatatypeProperty ; rdfs:range xsd:unsignedInt .
avec:arrowLogicalType a owl:DatatypeProperty ; rdfs:range xsd:string .
avec:arrowCFormat a owl:DatatypeProperty ; rdfs:range xsd:string .
avec:duckdbType a owl:DatatypeProperty ; rdfs:range xsd:string .
avec:isNullable a owl:DatatypeProperty ; rdfs:range xsd:boolean .

avec:QuadLayout64B a avec:ColumnarQuadLayout ;
    rdfs:label "64-byte columnar quad layout" ;
    avec:quadField avec:F_s_id , avec:F_p_id , avec:F_o_id , avec:F_g_id ,
                   avec:F_t_ns , avec:F_vector_clock , avec:F_policy_mask ,
                   avec:F_inline_digest .

avec:F_s_id a avec:QuadField ;
    avec:byteOffset 0 ; avec:byteWidth 4 ;
    avec:arrowLogicalType "uint32" ; avec:arrowCFormat "I" ;
    avec:duckdbType "UINTEGER" ; avec:isNullable false .
avec:F_p_id a avec:QuadField ;
    avec:byteOffset 4 ; avec:byteWidth 4 ;
    avec:arrowLogicalType "uint32" ; avec:arrowCFormat "I" ;
    avec:duckdbType "UINTEGER" ; avec:isNullable false ;
    rdfs:comment "TermId integer; DuckDB filter pushdown on equality." .
avec:F_o_id a avec:QuadField ;
    avec:byteOffset 8 ; avec:byteWidth 4 ;
    avec:arrowLogicalType "uint32" ; avec:arrowCFormat "I" ;
    avec:duckdbType "UINTEGER" ; avec:isNullable false .
avec:F_g_id a avec:QuadField ;
    avec:byteOffset 12 ; avec:byteWidth 4 ;
    avec:arrowLogicalType "uint32" ; avec:arrowCFormat "I" ;
    avec:duckdbType "UINTEGER" ; avec:isNullable false .
avec:F_t_ns a avec:QuadField ;
    avec:byteOffset 16 ; avec:byteWidth 8 ;
    avec:arrowLogicalType "timestamp[ns]" ; avec:arrowCFormat "ttn" ;
    avec:duckdbType "TIMESTAMP_NS" ; avec:isNullable false .
avec:F_vector_clock a avec:QuadField ;
    avec:byteOffset 24 ; avec:byteWidth 4 ;
    avec:arrowLogicalType "uint32" ; avec:arrowCFormat "I" ;
    avec:duckdbType "UINTEGER" ; avec:isNullable false .
# PROVENANCE (wave 11): offsets/widths corrected to the LANDED Quad64 law
# (crates/avec-arrow/src/layout.rs offset asserts; spec 23). vector_clock
# u32 @0x18, policy_mask u32 @0x1C, inline_digest [u8;32] @0x20.
avec:F_policy_mask a avec:QuadField ;
    avec:byteOffset 28 ; avec:byteWidth 4 ;
    avec:arrowLogicalType "uint32" ; avec:arrowCFormat "I" ;
    avec:duckdbType "UINTEGER" ; avec:isNullable false .
avec:F_inline_digest a avec:QuadField ;
    avec:byteOffset 32 ; avec:byteWidth 32 ;
    avec:arrowLogicalType "fixed_size_binary[32]" ; avec:arrowCFormat "w:32" ;
    avec:duckdbType "BLOB" ; avec:isNullable false .

### ---------- Hardware kernel individuals ----------

avec:Kernel_ARM64_NEON a avec:HardwareKernel ;
    rdfs:label "ARM64 NEON kernel" ;
    avec:cyclomaticComplexity 1 ;
    avec:basicBlockCount 1 ;
    avec:internalEdgeCount 0 ;
    rdfs:comment "Primitives: CSEL (branchless select), VZIP (deinterleave)." .

avec:Kernel_x86_64_AVX2 a avec:HardwareKernel ;
    rdfs:label "x86-64 AVX2 kernel" ;
    avec:cyclomaticComplexity 1 ;
    avec:basicBlockCount 1 ;
    avec:internalEdgeCount 0 ;
    rdfs:comment "Primitives: CMOV (branchless select), VPUNPCK (deinterleave)." .

avec:Kernel_WASM32_SIMD128 a avec:HardwareKernel ;
    rdfs:label "WASM32 SIMD128 kernel" ;
    avec:cyclomaticComplexity 1 ;
    avec:basicBlockCount 1 ;
    avec:internalEdgeCount 0 ;
    rdfs:comment "Primitives: i32.select, v128.bitselect." .

### ---------- P28 ODRL policy ----------

avec:SovereignInvariantPolicy_v1 a odrl:Policy ;
    odrl:permission [
        a odrl:Permission ;
        odrl:action odrl:use ;
        odrl:constraint [
            a odrl:Constraint ;
            odrl:leftOperand avec:assetUnderManagement ;
            odrl:operator odrl:lt ;
            odrl:rightOperand "50000000"^^xsd:decimal ;
            odrl:unit <https://avec.chatman.dev/unit/USD>
        ] ;
        rdfs:comment "Safe harbor: AUM below $50M may use without regularization duty."
    ] , [
        a odrl:Permission ;
        odrl:action odrl:use ;
        odrl:constraint [
            a odrl:Constraint ;
            odrl:leftOperand avec:assetUnderManagement ;
            odrl:operator odrl:gte ;
            odrl:rightOperand "50000000"^^xsd:decimal ;
            odrl:unit <https://avec.chatman.dev/unit/USD>
        ] ;
        odrl:duty [
            a odrl:Duty ;
            odrl:action avec:regularize ;
            rdfs:comment "Remedy per FASB ASC 450 (loss contingency accrual) and SOX 302/404 (internal control certification)."
        ]
    ] ;
    odrl:obligation [
        a odrl:Duty ;
        odrl:action avec:emitTelemetryLog ;
        rdfs:comment "EU AI Act Art 12/14/62: log emission duty for high-risk system telemetry."
    ] .

avec:SettlementLatencyConstraint a odrl:Constraint ;
    odrl:leftOperand avec:settlementLatencyNs ;
    odrl:operator odrl:lte ;
    odrl:rightOperand "35000"^^xsd:long ;
    odrl:unit <https://avec.chatman.dev/unit/ns> .

### ---------- SHACL closed shapes ----------

avec:ArrowExecutionPlaneShape a sh:NodeShape ;
    sh:targetClass avec:ArrowExecutionPlane ;
    sh:closed true ;
    sh:property [
        sh:path avec:cacheLineAlignmentBytes ;
        sh:hasValue 64 ;
        sh:message "REFUSAL-01: Arrow plane must be 64-byte cache-line aligned."
    ] , [
        sh:path avec:nullBitmapPresent ;
        sh:hasValue false ;
        sh:message "REFUSAL-02: Arrow plane must not carry a validity/null bitmap."
    ] , [
        sh:path avec:cyclomaticComplexity ;
        sh:hasValue 1 ;
        sh:message "REFUSAL-03: Arrow plane kernel must be M=1 (one path, no branches)."
    ] .

avec:ParquetPersistencePlaneShape a sh:NodeShape ;
    sh:targetClass avec:ParquetPersistencePlane ;
    sh:closed true ;
    sh:property [
        sh:path avec:definitionLevelByteLength ;
        sh:hasValue 0 ;
        sh:message "REFUSAL-04: Parquet plane must carry zero definition levels."
    ] , [
        sh:path avec:repetitionLevelByteLength ;
        sh:hasValue 0 ;
        sh:message "REFUSAL-05: Parquet plane must carry zero repetition levels."
    ] , [
        sh:path avec:decodeThroughputGiBs ;
        sh:minInclusive 70 ;
        sh:message "REFUSAL-06: Parquet decode throughput must be >= 70 GiB/s."
    ] .

avec:GitSequencingPlaneShape a sh:NodeShape ;
    sh:targetClass avec:GitSequencingPlane ;
    sh:closed true ;
    sh:property [
        sh:path avec:packfileVersion ;
        sh:hasValue 2 ;
        sh:message "REFUSAL-07: Git plane must use packfile format v2."
    ] , [
        sh:path avec:fanoutTableEntries ;
        sh:hasValue 256 ;
        sh:message "REFUSAL-08: Git plane fanout table must have 256 entries."
    ] , [
        sh:path avec:searchLadderStages ;
        sh:hasValue 16 ;
        sh:message "REFUSAL-09: Git plane binary search ladder must be 16 stages."
    ] , [
        sh:path avec:searchTimeVarianceNs ;
        sh:maxInclusive 1.5 ;
        sh:message "REFUSAL-10: Git plane search time variance must be <= 1.5 ns."
    ] .

avec:TurnstileAdmissionShape a sh:NodeShape ;
    sh:targetClass avec:TurnstileAdmission ;
    sh:closed true ;
    sh:ignoredProperties ( rdf:type prov:generated ) ;
    sh:property [
        sh:path avec:heapAllocationBytes ;
        sh:hasValue 0 ;
        sh:message "REFUSAL-11: Turnstile admission must allocate zero heap bytes."
    ] , [
        sh:path avec:branchMissRate ;
        sh:hasValue 0.000 ;
        sh:message "REFUSAL-12: Turnstile admission must have zero branch misses."
    ] , [
        sh:path avec:settlementLatencyNs ;
        sh:maxInclusive 35000 ;
        sh:message "REFUSAL-13: Turnstile settlement latency must be <= 35000 ns."
    ] , [
        sh:path avec:processCapabilityCpk ;
        sh:minInclusive 1.67 ;
        sh:message "REFUSAL-14: Turnstile process capability Cpk must be >= 1.67."
    ] .

### ---------- Genesis release instance (worked example) ----------

avec:Release_v26_10_31 a prov:Entity ;
    rdfs:label "AVEC v26.10.31 release" ;
    dcterms:isVersionOf <https://avec.chatman.dev/project/avec> .

avec:Genesis_Arrow_Batch a dcat:Distribution , prov:Entity ;
    dcat:mediaType <application/vnd.apache.arrow.stream> ;
    prov:wasGeneratedBy avec:Genesis_Turnstile_Event ;
    prov:wasDerivedFrom avec:Release_v26_10_31 .

avec:Genesis_Parquet_Page a dcat:Distribution , prov:Entity ;
    dcat:mediaType <application/vnd.apache.parquet> ;
    # PROVENANCE: TARGET-not-measured. No receipted decode-throughput exists:
    # CTQ-03 roundtrip was unmeasurable at timer resolution and CTQ-06 measured
    # 0.43-1.18 GiB/s on blake3 hashing paths, neither of which is a decode
    # number (receipts/gate-ctq-report-v2.md). 74.8 is the design target only.
    avec:decodeThroughputGiBs 74.8 ;
    prov:wasDerivedFrom avec:Genesis_Arrow_Batch .

avec:Genesis_Git_Packfile a prov:Entity ;
    # PROVENANCE: searchTimeVarianceNs is carried only on the shape
    # (REFUSAL-10, <= 1.5 ns), not asserted on this fixture. Receipted variance
    # is nonzero: CTQ-04 transpose variance residual stdev 1508 ns on mean
    # 16169 ns (frac 0.0932, receipts/gate-ctq-report-v2.md); the design-stage
    # 0.00 figure is retired.
    avec:packfileVersion 2 ;
    avec:fanoutTableEntries 256 ;
    avec:searchLadderStages 16 ;
    prov:wasDerivedFrom avec:Genesis_Parquet_Page .

avec:Genesis_Turnstile_Event a avec:TurnstileAdmission , prov:Activity ;
    avec:heapAllocationBytes 0 ;
    avec:branchMissRate 0.000 ;
    # PROVENANCE: receipted. E2E admission loop min-of-2000 = 14.9 us = 14900 ns
    # (receipts/gate-e2e-ctq05.md); CTQ-05 min 12.875 us = 12875 ns
    # (receipts/gate-ctq-report-v2.md). Replaces the design-stage 31400 ns figure.
    avec:settlementLatencyNs 14900 ;
    prov:generated avec:Genesis_Blake3_Receipt .

avec:Genesis_Blake3_Receipt a avec:Blake3MerkleReceipt , prov:Entity ;
    sec:digest "blake3-PLACEHOLDER-SHAPE-VALUE"^^xsd:string ;
    prov:wasGeneratedBy avec:Genesis_Turnstile_Event .
```

## Interpretation Notes

- The `Genesis_*` individuals are a worked example and shape fixture. Measured
  individuals now carry receipted values with `# PROVENANCE:` comments (see the
  Receipts Linkage section); the BLAKE3 digest remains placeholder-shaped and
  the 74.8 GiB/s decode throughput is a TARGET-not-measured design value.
  Production receipts must carry real BLAKE3 digests from real runs.

## Receipts Linkage

- Every receipted number in the Genesis fixture carries an adjacent
  `# PROVENANCE:` Turtle comment naming the receipt file under
  `receipts/` that produced it. A number without provenance is either a
  design TARGET (explicitly marked `TARGET-not-measured`) or a defect.
- `receipts/gate-shacl-v2.md` — prior validator receipt: SHACL self-validation
  `Conforms: True` + anti-vacuity court PASS (mutant `settlementLatencyNs`
  rejected via REFUSAL-13), exit 0, standing ALIVE (2026-10-31T02:15:58Z).
- `receipts/gate-shacl-v3.md` — current validator receipt, minted after the
  fixture's provenance refresh (settlement 31400 -> 14900 ns; decode 74.8
  marked TARGET-not-measured).
- Receipts are append-only (`scripts/mint_receipt.sh` refuses to overwrite an
  existing receipt ID); superseded receipts are never edited in place.
- Standing: PARTIAL_ALIVE. The ontology now passes engine self-validation: `pyshacl`
  reports `Conforms: True` on the Genesis fixture with `scripts/validate_ontology.sh`
  (exit 0), and the gate's anti-vacuity court still rejects a perturbed
  `settlementLatencyNs` mutant (REFUSAL-13 fires). The shapes are closed except for
  `sh:ignoredProperties ( rdf:type prov:generated )` on `avec:TurnstileAdmissionShape`,
  which whitelists structural typing and the prov:generated edge to the Genesis receipt.
  Ongoing conformance on future ontology edits remains the standing qualification gate.
- Next-step candidates from the design conversation:
  - ggen SPARQL `CONSTRUCT` artifact generation (render code/docs from this graph).
  - N3/Datalog inference over P28-SIL compliance (ODRL duty derivations).
  - SHACL negative test suite: invalid boundary cases that must trip REFUSAL-01..14.

## See Also

- 01-stardf-reconstitution.md
- 18-dflss-charter.md
