# Bilateral OCEL v2 Micro-Window Entanglement

Standing: UNVERIFIED design. Cross-refs: 12-distance-bounding-rtt.md,
03-subsystem-mapping.md, 13-time-crypto-prior-art.md.

## The Temporal Collision Check

When both parties to a settlement must submit OCEL v2 event logs bound to a
small physical time window (|t_A - t_B| <= eps), retroactive fabrication
becomes physically hard: the traces must interlock with each other, with
continuous telemetry, and with hardware monotonic counters.

The notary ingests both event graphs and performs an ASOF-style
mutual-causality verification: for each causally-paired event in A's trace it
finds the nearest-in-time event in B's trace sharing the mutual object, and
checks the offset against the window ceiling.

```text
Party A trace                          Party B trace
------------                           ----------------
AssetCustodyLocked      t = T + 0us    FundsDeposited         t = T + 90us
   objects: [Asset, Escrow]                objects: [Escrow, Funds]
   |                                                       |
   |<-------------- delta_ns = 90us  (|dt| <= eps) -------->|
CustodyAttestation      t = T + 180us  EscrowAuthorized       t = T + 240us
   objects: [Asset, Custodian]             objects: [Escrow, License]
```

Delta-t bounding rule: every pair matched on the mutual escrow object must
satisfy |t_A - t_B| <= eps, and causality direction must agree with the
protocol's turnstile order (custody before authorization, liquidity check
before deposit commit).

## Three Unforgeability Mechanisms

1. **Hardware-anchored monotonicity.** Each event carries a 64-bit monotonic
   nanosecond stamp plus a hardware cycle counter (rdtsc-class). Within a
   host, dt/dc forms a clock gradient bounded by the known CPU frequency
   envelope. Inserting or padding fabricated events violates the gradient:
   an inserted event must claim a (t, c) pair lying on the observed
   gradient, and padding shifts the gradient outside the calibrated band.
   Fabrication must therefore re-derive a physically consistent counter
   trajectory, not just shift timestamps.

2. **Mutual object entanglement.** OCEL v2 events relate multiple objects
   (activity -> object lists). Both parties' traces must reference the same
   shared escrow object id within eps. An attacker cannot fabricate in
   isolation: the forged event must name an escrow object the counterparty
   also names, at a time the counterparty's hardware stamp also bounds.

3. **Continuous entropy interleaving.** Traces must interleave with ambient
   telemetry streams: GPS receiver drift samples, order-book L3 tick
   digests, NIC hardware timestamps. Within eps ~= 500us an adversary
   cannot precompute the environmental variables — the L3 tick digest in
   that window is unknown before it occurs, and a fabricated log cannot
   contain it. Post-hoc fabrication requires committing to an unpredictable
   ambient digest, which is exactly the anti-forgery property.

Mechanism 3 is the load-bearing one: mechanisms 1 and 2 bound the shape of
fabricated structure; mechanism 3 binds it to unguessable environment state.

## The Sieve-Court SQL Design

Terminology note: "sieve court" here is a design term for the admission judge
over entangled traces — distinct from the repo's mutation-court family
(52-courts-audit.md), which tests shipped code by mutation.

Bilateral ASOF join on mutual object ids; activity ids are interned strings
(e.g. `AssetCustodyLocked`, `FundsDeposited`).

```sql
WITH a AS (
  SELECT e.event_id, e.activity_id, e.ts_ns, o.object_id
  FROM ocel_events a_events e JOIN ocel_objects o USING (event_id)
  WHERE e.log_id = 'party_a'
    AND e.activity_id = 'AssetCustodyLocked'
),
b AS (
  SELECT e.event_id, e.activity_id, e.ts_ns, o.object_id
  FROM ocel_events b_events e JOIN ocel_objects o USING (event_id)
  WHERE e.log_id = 'party_b'
    AND e.activity_id = 'FundsDeposited'
),
paired AS (
  SELECT
    a.object_id                     AS escrow_id,
    a.ts_ns                         AS t_a,
    b.ts_ns                         AS t_b,
    b.ts_ns - a.ts_ns               AS delta_ns,
    a.ts_ns - lease_a.ts_turnstile  AS phi_a,
    b.ts_ns - lease_b.ts_turnstile  AS phi_b
  FROM a
  ASOF JOIN b ON a.object_id = b.object_id
             AND b.ts_ns >= a.ts_ns - 500000   -- eps window in ns
  JOIN turnstile_leases lease_a ON lease_a.object_id = a.object_id
  JOIN turnstile_leases lease_b ON lease_b.object_id = b.object_id
)
SELECT
  escrow_id,
  delta_ns,
  ABS(delta_ns) <= 250000          AS is_temporally_valid,  -- ceiling
  phi_a > 0 AND phi_b > 0          AS leases_active
FROM paired;
```

Verdict semantics:

- Row with `is_temporally_valid = true` and `leases_active = true`:
  admitted, the pair is temporally entangled.
- Null match (no B event within the eps window for A's escrow object):
  typed refusal `REFUSAL-04: TEMPORAL_WINDOW_EXCURSION`; the motion dies.
- `is_temporally_valid` uses the 250us ceiling (tighter than the 500us
  ingest window) so transport jitter is absorbed by the notary, not the
  admission gate.

Open item: this doc's `REFUSAL-04: TEMPORAL_WINDOW_EXCURSION` collides with
19-ontology-ttl.md, which pins REFUSAL-04 to the Parquet def-level violation.
Needs a refusal-code registry or a renumber; unresolved here. The 250us/500us
eps figures are also unreconciled against 12-distance-bounding-rtt.md's
2500ns budget.

## The Resulting Invariant Chain

1. Party A motion = CONSTRUCT + OCEL proof of asset lock within t +/- eps.
2. Party B second = CONSTRUCT + liquidity reservation proof within t +/- eps.
3. Notary verdict adds the temporal bound to the Chatman equation:
   `s.t. |t_event - t_turnstile| <= eps`. Admission is no longer
   order-shape-only; it is order-shape-AND-physical-time-bounded.
4. Settlement = squash-merge capturing both traces as columnar Parquet
   partitions in the packfile, giving replay a durable, queryable receipt
   of the entangled window.

Each stage consumes the previous stage's receipt: the notary cannot admit a
pair whose underlying motions were never CONSTRUCTed, and the squash-merge
cannot capture traces that never passed the temporal court.

## Honest Limits

- eps windows depend on clock discipline. NTP-class jitter (milliseconds)
  cannot support a 250us ceiling; PTP-class or crossbody timestamping can.
  The 250us number is a design target, not a measured property of any
  deployed host pair.
- Cross-host eps claims need hardware timestamping end to end. Software
  timestamps admit scheduler-induced skew that no notary can distinguish
  from forgery. Cross-ref: 13-time-crypto-prior-art.md.
- The clock-gradient invariant assumes calibrated frequency envelopes;
  thermal throttling and frequency scaling widen the band and weaken
  padding detection. Calibration cadence is unspecified here.
- LLM/clock-forgery assumptions are design-stage, not proven. No court has
  run against a real adversarial trace corpus; the mechanisms are
  plausible-by-construction and stand at UNVERIFIED until a sieve-court
  execution on synthetic forgeries measures the actual detection rate.

## See Also

- 12-distance-bounding-rtt.md
- 03-subsystem-mapping.md
- 13-time-crypto-prior-art.md
