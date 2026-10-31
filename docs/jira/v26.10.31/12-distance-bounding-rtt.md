# Notary Distance Bounding: RTT Envelope and the Processing-Time Trap

Standing: UNVERIFIED design. This document specifies, it does not report a
measured run.

## Challenge-Response Turnstile

The notary converts avec's raw speed into a physical security parameter. A
party that can ingest, fold, and emit within a fixed envelope is behaving as
hardware, not as a buffered intermediary.

```text
Notary                         Party (prover)
  |                                |
  |-- issue nonce N --------------->|   ephemeral, high-entropy
  |   (monotonic clock + local      |
  |    BLAKE3 entropy)              |
  |   strict RTT deadline armed     |
  |                                |-- [internal state step]
  |                                |     sample local ring buffer
  |                                |     fold N into TermId state
  |                                |     emit OCEL v2 frame
  |<-- return (t, BLAKE3(N||LE)) --|
  |                                |
  [sieve court]
  |   1. dt arrival check: RTT <= fail-closed ceiling
  |   2. hardware register proofs (HSM counters, RDTSC delta)
  |   3. bipartite CONSTRUCT match (both frames agree on N, action)
  |   4. atomic settlement
```

The nonce `N` is never reused. It is derived from the notary's monotonic clock
plus local BLAKE3 entropy, so replaying a prior response fails the hash check
and any replayed timing envelope. Parties must fold `N` into internal state
and emit an OCEL v2 frame within the envelope to pass.

## Why MITM Fails

Three independent channels must all hold for the adversary. Each fails for a
different physical reason.

### 1. Physical RTT bounding

A proxy relay must receive, forward, and (to alter semantics) tamper before
re-emission. Switch and fiber overhead per added hop is not compressible by
software. Forwarding plus tampering exceeds the fail-closed ceiling, and the
court emits `REFUSAL-04: TEMPORAL_EXCURSION`. The refusal is deterministic:
no heuristic score, no retry with a wider bound.

### 2. Private-state entropy

The response frame must include proof of internal state the proxy cannot
predict or observe: HSM monotonic counters, CPU uncore and thermal sampling
taken during the nonce fold. A proxy relaying the nonce to the true party and
back may match the hash, but cannot synthesize the private-state evidence of
the *local* fold, because its own machine never performed one.

### 3. Bipartite key exchange (DH entanglement)

Parties A and B derive an ephemeral Diffie-Hellman secret `K_AB` that the
notary never learns. Each party logs:

```text
Attribute(e) = BLAKE3(N || K_AB || action_id)
```

The notary validates the topological closure equality: both parties'
attributes over the same `action_id` must match. If a proxy inserted itself,
one side's `K_AB` differs (the proxy ran its own DH with each party), so the
equality breaks without the notary ever holding `K_AB`. Notary-blindness is
the point: the notary checks structure, not secrets.

## The Processing-Time Trap

The avec-specific insight. The notary does not only ask "were you fast
enough" — it asks each party "report when you received the message and how
long you took to process it."

The response frame carries:

- `t_rx` — NIC/bus ingress timestamp
- `t_tx` — egress timestamp on exit

So the total round trip decomposes as:

```text
RTT_total = flight_out + dt_proc + flight_back
dt_proc   = t_tx - t_rx
```

Because avec ingests, folds, and emits in <=2 microseconds (design target),
the flight-time budget is razor-thin. The consequence:

- A **buffered or tampering proxy** inflates `RTT_total` — it must hold,
  inspect, and re-emit.
- The **true kernel truthfully reports sub-2us processing**, so `dt_proc`
  cannot absorb the proxy's dwell time.
- A **faked `dt_proc`** (claiming the dwell was "processing") cannot be
  reconciled without the target's internal cycle count — `RDTSC` delta must
  be consistent with the claimed duration and with the platform's known
  cycle rate.

### Sieve-court checks

```sql
processing_duration_ns  = t_tx - t_rx
is_processing_budget_met = processing_duration_ns <= 2500
is_zero_alloc            = memory_alloc_bytes = 0
is_flight_valid          = estimated_flight_within(known_physical_path_bounds)
```

A frame that reports a long processing duration is not merely slow — it is
*structured like a proxy*, because honest avec stacks cannot produce that
shape. The court does not need intent; it needs arithmetic.

## OcelKernelTelemetryFrame Layout

Wire layout, 128 bytes total:

| Offset | Field | Type | Invariant |
|---|---|---|---|
| 0 | `nonce_hash` | `[u8; 32]` | BLAKE3(N), matches issued nonce |
| 32 | `t_rx_ns` | `u64` | NIC/bus ingress |
| 40 | `t_tx_ns` | `u64` | egress; `t_tx >= t_rx` |
| 48 | `cycle_count` | `u64` | RDTSC delta across the fold |
| 56 | `memory_alloc_bytes` | `u64` | invariant 0 |
| 64 | `activity_id` | `u32` | interned |
| 68 | `object_id` | `u32` | interned |
| 72 | reserved | — | zero-filled to 128 |

The fixed 128-byte frame is cache-line-aligned-friendly and suitable for
direct `arrow`/zero-copy reads — the court parses it with bounds checks
against a fixed stride, no variable-length deserialization in the hot path.

## Consequence

- **Zero heuristic overhead.** The notary inspects physical quantities
  (nanoseconds, cycles, allocation counts), never inferred intent. Security
  reduces to arithmetic on the frame.
- **Fast stacks shrink the attacker's window.** Every microsecond shaved off
  honest processing is a microsecond the proxy cannot claim as legitimate
  dwell time. Speed is not a performance luxury here; it is the defense.
- **Deterministic refusal.** `REFUSAL-04: LATENCY_ANOMALY_DETECTED` locks
  escrow and closes the PR unmerged. No human triage step sits between the
  anomalous frame and the refusal; the same kernel order that admits clean
  frames refuses dirty ones.

## Honesty Note

Two limits, stated rather than buried:

1. **Sub-2us processing is a design target**, not a receipt. It is
   contingent on the branchless substrate actually landing. Until that
   lands, the 2500ns budget in the sieve court is aspirational and the
   envelope must be calibrated against measured, not intended, `dt_proc`
   distributions.
2. **AMENDMENT (post-wave-5): the 2500ns processing budget is contradicted by
   receipts.** CTQ-05 measured hash-only processing at 12.75us for a 16KiB
   batch (gate-ctq-report-v2.md), and end-to-end settlement at 8.2-14.9us. The
   substrate landed; the honest `dt_proc` is roughly 6x the design budget. The
   sieve predicate `processing_duration_ns <= 2500` must be recalibrated to
   measured `dt_proc` distributions, or the budget re-scoped to small fixed
   frames. Until then, treat "fast stacks cannot produce that shape" as
   calibrated-false at 16KiB batch scale.
3. **AMENDMENT: frame layout delta.** This document specifies a 128-byte
   OcelKernelTelemetryFrame; the shipped layout law is the 64-byte Quad64
   layout (23-64b-quad-spec.md). Either re-derive the 128-byte frame from the
   Quad64 law or record an explicit exemption. Standing remains
   UNVERIFIED-for-protocol with both deltas recorded.
4. **Cross-network RTT envelopes require hardware timestamping** (PTP or
   NIC-level timestamps) to be enforceable. Host-software clocks on separate
   machines have jitter larger than the flight-time budget this scheme
   depends on. Within one host or one NUMA domain, the scheme is tighter;
   across the internet, it degrades to the classical distance-bounding
   bounds.

See Also: 11-ocel-entanglement.md, 13-time-crypto-prior-art.md,
15-arrow-microarch-probe.md.
