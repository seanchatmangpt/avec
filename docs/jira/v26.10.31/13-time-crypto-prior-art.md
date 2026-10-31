# Prior Art in Time-Based Cryptography

Prior-art survey mapping the notary design onto established formal domains.
Standing: UNVERIFIED survey; citations as named in the design conversation. Per
prior-art-first discipline, this documents that the design's mechanisms have
named precedents — and where the design is claimed to be novel.
The kernel-speed premise is now bounded by measured ceilings (1.3-1.85
GiB/s/core, 48-lsl-rederivation.md); the sub-2µs processing budget is
contradicted at 16KiB scale (12.75µs receipted). Protocol standing stays
UNVERIFIED.

## Distance-Bounding Protocols

Brands & Chaum (1993) introduced distance bounding; Hancke & Kuhn gave the
widely deployed exchange shape. The verifier issues rapid bit challenges and
measures strict round-trip time (RTT) to bound the prover's physical distance,
defeating relay, mafia, and terrorist fraud.

The bounding relation: `distance <= c * (RTT - dt_proc) / 2`, where `dt_proc`
is prover processing time and `c` is signal propagation speed.

Historical weakness: `dt_proc` is variable under a software prover. An
attacker who can intercept the challenge, forward it, and still return inside
the acceptance window gains exactly that processing margin. The avec design
claim: deterministic, sub-microsecond-scale processing (<=2us) collapses that
margin to noise, shrinking the interception window below any usable relay
budget.

## Proof of Elapsed Time / Hardware Monotonic Enclaves

Intel's Proof of Elapsed Time (PoET) and SGX/TPM attestation bind execution
proofs to monotonic hardware cycle registers: a trusted enclave reads a
counter that only advances, and the signed reading is the evidence.

Design analog: reading `CNTVCT_EL0` (ARM) / `RDTSC` (x86) at frame ingress and
egress turns ordinary CPU work into attestation material. The timestamps are
not telemetry; they are the proof that the work happened inside a measured
interval on monotonic hardware.

## Verifiable Delay Functions

Boneh, Bonneau, Bunz, Fisch (2018): a Verifiable Delay Function (VDF) is
non-parallelizable sequential computation with instant verification, deployed
against time-travel and front-running attacks where an adversary must not be
able to shortcut a required delay.

Design relationship: the RTT turnstile is the inverse construction. A VDF
forces a minimum amount of sequential work; the turnstile enforces a bounded
transit window in which ANY extra sequential work by an intermediary proxy
causes structural timeout. VDF = lower bound on work; turnstile = upper bound
on latency with unforgeable consequence.

## Roughtime

Google's Roughtime chains signed UDP responses across independent servers,
embedding client nonces into signed time statements. A client whose clock is
tampered with is caught by cross-server disagreement, and servers cannot
rewrite history because earlier responses are chained into later ones.

Design analog: the notary acts as a sovereign time anchor issuing ephemeral
nonces; the parties' OCEL logs prove the interlock (challenge seen, work done,
response returned) inside that notary-issued time envelope.

## Novelty Claim (design hypothesis — requires adversarial review)

The survey's conclusion, offered as a design hypothesis and NOT an established
result: academic time-based protocols fail in practice due to two gaps.

1. Software jitter: GC pauses, context switches, and 10-50 ms JSON/WASM parse
   variance swamp the margins the protocols are sized against.
2. Missing state context: classical distance bounding proves physical
   proximity only — nothing about business logic or state ownership.

The avec design claim is that embedding hardware timestamps in a columnar wire
format, gated by SHACL turnstiles, merges physical distance bounding with
semantic graph law: the time proof and the state proof travel in the same
frame and are validated by the same admission machinery.

Per criticism discipline: this is a hypothesis with a named falsifier below,
not a verified property. It has not been adversarially reviewed and carries no
execution receipt.

## Falsifier Sketch

The unforgeability claim is refuted by any of:

- A relay attack completing inside the epsilon acceptance window on commodity
  networks with NTP-class clock agreement — i.e., an adversary forwarding the
  challenge and returning a valid frame faster than the turnstile timeout,
  without the ~2us deterministic processing budget holding.
- A proxy performing nontrivial sequential work (parse, transform, sign) in
  transit while remaining inside the window on any measurable fraction of
  runs.
- Two honest parties with NTP-class clocks disagreeing about window membership
  after replay of the same OCEL logs — the envelope fails to be
  time-deterministic across verifiers.

## See Also

- 12-distance-bounding-rtt.md
- 14-blinded-wasm.md
