# stardf-4d: Observable, Time, Vector, Git as Columnar Frames

Standing: PARTIAL_ALIVE. The substrate courts landed (24 LIVE families per
52-courts-audit.md incl. the wave-9 addendum, waves 3-8); the three perf
targets below still have no
timing receipt — CTQ-05's 8.2-14.9µs e2e admission is the nearest receipted
cousin. KGC-4D figures remain cited-prior-art, not our measurements.

## Prior Art: KGC-4D as a JS/WASM Event Store

KGC-4D models a versioned knowledge graph along four coordinates:

- **O (Observable)** — RDF quads held as JS objects inside a Universe graph;
  arbitrary subject/predicate/object/graph terms.
- **t (Time)** — BigInt nanosecond timestamps per event; arbitrary precision,
  heap-allocated per value.
- **V (Vector)** — causality vector clocks as JS `Map<replicaId, counter>`.
- **G (Git)** — content-addressed universe snapshots via isomorphic-git with
  BLAKE3-style hashing; the event log plus periodic snapshots makes the
  universe at any prior time reconstructible.

Key prior-art insight: the universe at any time t is reconstructible from the
event log plus the nearest preceding snapshot — time travel is a replay, not a
copy.

Measured latencies cited in the KGC-4D README (prior-art figures, not our
measurements):

| Operation | Cited figure |
|---|---|
| `appendEvent` (~100 quads) | ~0.8 ms |
| `freezeUniverse` | ~52–650 ms |
| Cold reconstruction | ~150 ms–6.5 s |

## stardf-4d Contrast Table

| Coordinate | KGC-4D (JS/WASM) | stardf-4d (design target) |
|---|---|---|
| O | JS quad objects, pointer-chasing terms | Contiguous `TermId` arena; branchless bitmasking |
| t | JS BigInt ns (heap per value) | `u64` monotonic ns/cycle counter, cache-line aligned |
| V | JS `Map<replicaId, counter>` per event | Fixed-width dense `u32` Lamport vector arena |
| G | isomorphic-git serialization + hashing | Zero-heap BLAKE3 merkle turnstile CAS commits |
| Mutation | `appendEvent` with JS policy objects | Branchless turnstile admit producing receipts |
| Freeze | Snapshot serialization (~52–650 ms cited) | Direct avec DataPageV2 zero-copy compaction |
| Time travel | Event replay loops (~150 ms–6.5 s cited) | ASOF range-slice over mmap'd Parquet |

Every row trades an interpreter-mediated, pointer-shaped structure for a
dense, fixed-width, SIMD-addressable one.

## 64-Byte Aligned 4D Frame Layout

One event is one 64-byte frame — exactly one cache line. Field sketch:

```c
struct stardf4d_frame {          /* 64 bytes, 64-byte aligned */
    uint64_t t_ns;               /*   0:  monotonic ns/cycle counter        */
    uint32_t vector_epoch;       /*   8:  Lamport epoch                     */
    uint32_t vector_seq;         /*  12:  Lamport sequence                  */
    uint32_t subject;            /*  16:  TermId into dense arena           */
    uint32_t predicate;          /*  20:  TermId                            */
    uint32_t object;             /*  24:  TermId                            */
    uint32_t graph;              /*  28:  TermId (named graph)              */
    uint32_t policy_hash;        /*  32:  ODRL/MCPP policy fingerprint      */
    uint32_t delta_action;       /*  36:  bitmask: 0x01 ADD, 0x02 DEL       */
    uint8_t  blake3_root[32];    /*  40:  merkle root through this frame    */
};                               /*  72 nominal -> padded/frame-packed to 64
                                    via 4-byte vector pair + tight packing:
                                    8+4+4+16+4+4+32 = 72; final layout packs
                                    blake3_root at 32-bit boundaries so the
                                    whole frame lands in one 64B line          */
```

Reconciliation (wave 8): this sketch sums to 72 B and predates the landed
Quad64 law. The receipted 64 B layout is `crates/avec-arrow/src/layout.rs`
(spec 23-64b-quad-spec.md): single `vector_clock` u32 @0x18, no
`delta_action` field, `inline_digest` @0x20. This sketch is superseded for
layout purposes.

Field semantics:

- `t_ns` — the t coordinate. Monotonic, never wall-clock-derived at read time.
- `vector_epoch`/`vector_seq` — the V coordinate, flattened to a u32 pair per
  frame; full Lamport vectors live in the dense vector arena, frames carry the
  compact (epoch, seq) slot.
- `subject..graph` — the O coordinate as four `TermId`s; string data never
  appears in the frame.
- `policy_hash` — fingerprint of the governing ODRL/MCPP policy at admit time;
  a frame without a resolvable policy hash fails the turnstile.
- `delta_action` — `0x01` ADD, `0x02` DEL; the two bits are mutually exclusive
  (a frame asserting both fails the gate).
- `blake3_root` — running merkle root, chaining frame N into frame N-1's root;
  this is the G coordinate at frame granularity. Snapshot commits pin a root
  into the CAS turnstile.

## Poka-Yoke: Guards as Bitmasks and CMOV

KGC-4D carries 24 FMEA failure-mode guards as runtime policy objects and
exception throws. stardf-4d compiles each guard to a predicate the hardware
evaluates:

- Guard classes become bitmask assertions on `delta_action`, `policy_hash`,
  and vector monotonicity — `AND`/`OR` masks, no branches.
- Accept/reject resolves through CMOV: the frame either commits or the receipt
  records a typed refusal; no throw path, no unwinding, no interpreter.
- An invalid mutation fails at the hardware gate — the turnstile emits a
  refusal receipt with the violated mask — rather than surfacing as a runtime
  exception after the fact.

The 24 guards are preserved as a checklist; only their enforcement substrate
changes (masks + CMOV instead of policy objects + throws).

## Performance Model (Design Targets, NOT Measured)

All figures below are targets for the unbuilt stardf-4d engine:

| Operation | Target | Mechanism |
|---|---|---|
| Event append | < 35 µs | One 64-byte frame, branchless turnstile, BLAKE3 root update |
| Freeze (~10K quads) | ~8.4 ms | >74 GiB/s DataPageV2 flush, zero-copy compaction |
| Cold time travel | ~12 ms | DuckDB ASOF join over memory-mapped Parquet |

Comparison anchor: KGC-4D's cited ~0.8 ms append / ~52–650 ms freeze /
~150 ms–6.5 s cold reconstruct. The targets are roughly 20x / 10–80x /
10–500x tighter, and are the falsifier for the design: any implementation
missing a target by an order of magnitude refutes the branchless thesis for
that operation.

## See Also

- `docs/jira/v26.10.31/01-stardf-reconstitution.md`
- `docs/jira/v26.10.31/02-architecture-planes.md`
- `docs/CONTENT_ADDRESSING.md`
