# Architecture: Logic Plane over Physical Plane

Two-plane architecture: a semantic logic layer (`purrdf`) governing a physical
data-plane substrate (`avec`). The logic plane decides; the physical plane moves
bytes at silicon line rate.

## purrdf — Semantic Logic & Turnstile Layer

`purrdf` is an RDF-native toolkit operating as the semantic law layer above the
physical substrate.

- **Semantic reasoning**: RDF graph reasoning over typed, addressable knowledge.
- **SHACL shape validation**: constraint shapes that declare what lawful data
  looks like before it is admitted.
- **Process mining** (`purrdf-process`): deriving process structure from event
  streams rather than hard-coding it.
- **Deterministic turnstile state admission**: the governing mechanism. State
  transitions over `avec` are admitted or refused by a deterministic turnstile
  keyed on the semantic law — nothing crosses a transition without admission.

## avec — Physical Data Plane Substrate

`avec` is a zero-dependency columnar substrate: it moves and shapes bytes, and
it does not decide.

- **Apache Arrow in-memory representations**: fixed-width, non-nullable,
  64-byte-aligned columnar buffers as the canonical in-memory form.
- **Parquet persistence kernels**: DataPageV2-derived on-disk layout, level-free
  pages mapping 1:1 onto Arrow record batches.
- **Branchless execution**: data-parallel kernels structured for silicon line
  rate — no SQL overhead, no compute bloat, no query engine ambitions.
- **Zero-copy substrate**: reconciles BEAM memory constraints by keeping data
  outside the BEAM heap and portable by pointer, never by copy.

## Architecture Diagram

```
+--------------------------------------------------------------+
|  LOGIC PLANE — purrdf (semantic law)                         |
|                                                              |
|  +----------------+  +----------------+  +----------------+  |
|  | semantic       |  | SHACL shape    |  | process mining |  |
|  | reasoning      |  | validation     |  | (purrdf-proc.) |  |
|  +----------------+  +----------------+  +----------------+  |
|            \              |               /                  |
|         +-------------------------------------+              |
|         | deterministic turnstile admission   |              |
|         +-------------------------------------+              |
+-------------------------||-----------------------------------+
                          ||  governs: admit / refuse
                          ||  transitions
                          \/  over avec
+--------------------------------------------------------------+
|  PHYSICAL PLANE — avec (physical substrate)                  |
|                                                              |
|  zero-dependency columnar                                    |
|  +---------------------------+  +-------------------------+  |
|  | Arrow in-memory kernels   |  | Parquet persistence     |  |
|  | (fixed-width, 64B align)  |  | (DataPageV2, level-free)|  |
|  +---------------------------+  +-------------------------+  |
|        branchless execution -> silicon line rate             |
|        zero-copy: pointer-portable, never copied             |
+--------------------------------------------------------------+
          |
          | sub-BEAM: below the BEAM NIF/port boundary
          v
+------------------+     +-----------------------------------+
| silicon / memory |     | BEAM runtime (planes 2-4, OTP)    |
+------------------+     +-----------------------------------+
```

## Scope Boundary

The plane decomposition places exactly one plane under `avec`'s authority.

- **Plane 1 — semantic law (RDF/SHACL)**: owned by `stardf`/`graphlaw`, not
  `avec`. Reasoning, shapes, and admission live here.
- **Planes 2-4 — monotonic clocks/HLC, mailbox transport/distribution, actor
  sandboxing**: owned by BEAM/OTP, not `avec`.
- **avec is NOT** a platform, framework, or distributed runtime. It is
  exclusively the sub-BEAM physical substrate between silicon and the BEAM
  NIF/port boundary.

### Adopted Open Specifications

`avec` adopts exactly three open specs, at the byte-layout level:

1. **Apache Arrow flat columnar core** — fixed-width, non-nullable,
   64-byte-aligned buffers.
2. **Apache Parquet DataPageV2** — level-free pages, 1:1 with Arrow record
   batches.
3. **Git Packfile v2 + idx v2** — byte-level pack and index layouts.

## Invariants avec Guarantees to BEAM

The contract at the NIF/port boundary is threefold:

- **Zero allocations on ingestion**: admitting data into `avec` buffers never
  allocates on the BEAM heap or in the ingestion path.
- **Deterministic M=1 complexity**: every kernel's cost model is linear in
  input size — no hidden super-linear behavior.
- **Pure pointer portability**: an Arrow batch flushes to a Parquet file
  descriptor or a Git packfile chunk without any representation change — the
  same bytes, reinterpreted by layout, never transformed.

## Standing

PARTIAL — architecture intent, with one invariant now receipted: the
zero-copy pointer-portability contract at the BEAM NIF boundary is ALIVE in
`docs/jira/v26.10.31/receipts/gate-beam-bridge-v2.md` (pointer identity
across GC, alloc-spy read delta {0,0}, destructor exactly-once). The
remaining invariants (zero allocations on ingestion, deterministic M=1
complexity) stay design intent, not receipted.

## See Also

- [03-subsystem-mapping.md](03-subsystem-mapping.md)
- [18-dflss-charter.md](18-dflss-charter.md)
