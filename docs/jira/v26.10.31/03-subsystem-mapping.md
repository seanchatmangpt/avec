# Subsystem Mapping: purrdf-process, -arrow, -sql to stardf

Standing: PARTIAL — the physical-layout layer has landed in avec
(`crates/avec-arrow/src/layout.rs`, receipted via gate-arrow-tests);
stardf-process/-sql (POWL/OCEL arenas, DuckDB `stardf_scan()` extension)
remain unexecuted — no stardf scan or DuckDB extension exists in the
workspace. Design capture for the unlanded layers.

## Overview

Three legacy purrdf subsystems reconstitute into stardf layers. The dependency
direction is strictly layered: process sits on arrow, arrow sits under sql.
Nothing in a higher layer may reach past the layer below it.

```
+---------------------------------------------------------+
|  stardf-process                                          |
|  POWL 2.0 scheduling, OCEL 2.0 arenas, bounded HTN       |
+---------------------------------------------------------+
                          |  emits events / reads batches
                          v
+---------------------------------------------------------+
|  stardf-arrow                                            |
|  128-byte framed wire contract, zero-copy columnar       |
+---------------------------------------------------------+
                          |  zero-copy record batches
                          v
+---------------------------------------------------------+
|  stardf-sql                                              |
|  stardf_scan() -> DuckDB vector functions, TermId        |
|  filter pushdown                                         |
+---------------------------------------------------------+
```

## purrdf-process -> stardf-process

Core responsibilities:

- Semantic process execution and lineage.
- Process mining and conformance checking against declared models.
- Task lifecycles and causality tracking across events and objects.

Mechanics, data structures, protocols (as designed):

- POWL 2.0 (partially ordered workflow language) as the process model surface:
  strict partial order, operator nodes for choice/loop composition, silent
  activities represented as model-internal, not log-internal.
- OCEL 2.0 event logs as the observed-reality record: events with attributes,
  objects with types, qualified object-to-event relationships.
- Bounded HTN planning: task decomposition with explicit bounds so planning
  terminates on incomplete or cyclic domains.

stardf reconstitution mapping:

- Branchless flat-array POWL scheduling. The partial order compiles into index
  arrays (successor offsets, ready masks) so the scheduler walks contiguous
  memory with no pointer chasing and no branch-heavy dispatch.
- Contiguous OCEL 2.0 event arena with turnstile admission replacing
  pointer-heavy HTN trees. Events append into an arena; causal/lease state
  flips at turnstile boundaries; HTN decomposition materializes as ranges over
  the arena rather than linked structures.

## purrdf-arrow -> stardf-arrow

Core responsibilities:

- Zero-copy memory layout and inter-process wire transport.
- Deterministic framing so any two processes agree on byte layout without
  negotiation.

Mechanics, data structures, protocols (as designed):

- Deterministic 128-byte framed wire format: fixed-size frame header, fixed
  field order, no variable-length preamble; frames are self-delimiting and
  alignment-stable.
- Zero-copy Arrow IPC layouts: buffers shared or mapped, validity bitmaps and
  value buffers consumed in place, no deserialization copy.
- Hardware-aligned record batches: alignment chosen for SIMD and cache-line
  boundaries so column scans vectorize without fixup.

stardf reconstitution mapping:

- The 128-byte framed wire contract formalized onto the avec zero-copy
  columnar format: the frame layout becomes a typed avec schema, and layout
  guarantees are enforced with compile-time layout assertions rather than
  runtime checks.
- Compile-time assertions cover field offsets, total frame size, and buffer
  alignment; a violation is a build error, never a runtime path.

## purrdf-sql -> stardf-sql

Core responsibilities:

- Relational and vectorized analytical query engine over process/RDF data.
- SQL-level access without ETL between storage and query.

Mechanics, data structures, protocols (as designed):

- DuckDB extension integration as the query substrate.
- Vectorized PIVOT, ASOF joins, and sliding-window aggregates.
- SQL-over-RDF relational virtualization: RDF graphs exposed as relational
  tables.
- Integer-based joins over interned `TermId` dense arrays: joins compare
  fixed-width integers, not strings; string resolution happens once at the
  dictionary boundary.
- Canonical 5-table Parquet layout (triples/quads, term dictionary, and
  companion tables) eliminating ETL: query reads the canonical layout
  directly.

stardf reconstitution mapping:

- Flat `TermId` arrays bridged to DuckDB extension vector functions through a
  zero-copy `stardf_scan()` table function. The scan yields Arrow-compatible
  vectors that DuckDB consumes without copy.
- Branchless `TermId` filter pushdowns: predicates compile into mask
  operations over dense integer arrays, evaluated inside the scan before rows
  materialize.

## SQL-over-OCEL design pattern (ASOF / windows)

The stardf-sql layer queries OCEL arenas relationally. Five-table schema
sketch:

```
ocel_events        (event_id, activity, timestamp_ns, case_key, arena_range)
ocel_event_metrics (event_id, metric_key TermId, value, unit TermId)
ocel_state_leases  (object_id, lease_key TermId, acquired_ns, released_ns,
                    holder_id)
terms              (term_id TermId, lexical, datatype)
event_object       (event_id, object_id, qualifier)
```

Example query classes:

1. **ASOF telemetry binding.** Bind sensor telemetry to the nearest process
   event at or before the sample, recording the gap as `observation_lag_ns`:

   ```sql
   SELECT e.event_id, s.sample_ns,
          s.sample_ns - e.timestamp_ns AS observation_lag_ns,
          s.value
   FROM sensor_samples s
   ASOF JOIN ocel_events e
     ON s.object_id = e.object_id
    AND s.sample_ns >= e.timestamp_ns;
   ```

2. **Multi-key ASOF lease attribution.** Attribute each event to the lease
   holder whose interval covers the event, with a staleness check so an
   expired lease never attributes:

   ```sql
   SELECT e.event_id, l.holder_id,
          e.timestamp_ns - l.acquired_ns AS lease_age_ns
   FROM ocel_events e
   ASOF JOIN ocel_state_leases l
     ON e.object_id = l.object_id
    AND e.timestamp_ns >= l.acquired_ns
   WHERE l.released_ns IS NULL
      OR e.timestamp_ns <= l.released_ns
      AND e.timestamp_ns - l.acquired_ns <= l.max_staleness_ns
      -- is_lease_valid staleness check applied post-ASOF
   ```

3. **Activity-transition latency chains.** Per-object LAG over ordered events
   yields the delay from one activity to the next in the chain:

   ```sql
   SELECT object_id, activity, timestamp_ns,
          timestamp_ns
            - LAG(timestamp_ns) OVER (
                PARTITION BY object_id ORDER BY timestamp_ns) AS transition_ns
   FROM ocel_events e
   JOIN event_object eo USING (event_id);
   ```

4. **Trailing-window concurrency and inter-arrival p99.** Count overlapping
   open intervals in a trailing window, and take `QUANTILE_CONT` over
   inter-arrival gaps:

   ```sql
   WITH gaps AS (
     SELECT timestamp_ns
       - LAG(timestamp_ns) OVER (ORDER BY timestamp_ns) AS gap_ns
     FROM ocel_events)
   SELECT quantile_cont(gap_ns, 0.99) AS inter_arrival_p99
   FROM gaps
   WHERE gap_ns IS NOT NULL;
   -- concurrency: range-join each event against trailing-window opens
   ```

All four classes run over the same arena-backed tables; `stardf_scan()`
supplies the vectors and applies TermId pushdowns before materialization.

## See Also

- [23-64b-quad-spec.md](23-64b-quad-spec.md)
- [02-architecture-planes.md](02-architecture-planes.md)
