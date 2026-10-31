# The Arrow Buffer as a Covert Microarchitectural Assay

Escalation of the blinded-WASM design (see [14-blinded-wasm.md](14-blinded-wasm.md)): because
Arrow enforces deterministic, 64-byte-aligned contiguous layouts, a notary's stripped WASM kernel
running over a mapped Arrow buffer can double as a microarchitectural probe. The host sees opaque
integer work; the notary measures the physical machine underneath.

## The Inspection Surface

A linear Arrow buffer is predictable down to the individual cache line:

```
+--------------------------------------------------------------+
| line 0 | arena header (magic, epoch, nonce N)                |
| line 1 | interleaved challenge stride (64B-aligned challenge) |
| line 2 | predicate vector chunk (u32 lane predicates)         |
| line 3 | entangled state values (fused column payload)        |
|   ...  | payload, in the same stride pattern, every turn      |
+--------------------------------------------------------------+
```

Arrow's layout guarantees fix every offset: alignment is 64 bytes, columns are contiguous, and
the arena header pins the origin. Traversal of this buffer under a fixed kernel is therefore a
deterministic sequence of physical addresses. Induced access patterns are not incidental — they
are measurement instruments, chosen by the notary, executed by the host.

## Three Probe Classes

1. **Cache-line stride / memory-bus latency traps.** Interleaving 64B and 4096B access strides
   induces deterministic hit/miss patterns. On bare metal the cycle distribution is a tight band
   (illustrative: ~1,150 +/- 40 cycles for the fixed path). Under a hypervisor, emulator, or
   sandboxing proxy, TLB misses, translation drag, and cache pollution spike the same path by
   roughly 300-1000%. Deviation past the band is a fail-closed refusal.

2. **SIMD width verification.** The kernel is compiled against WASM `v128` sequences. Under an
   unoptimized runtime that falls back to scalar emulation, the processing interval `Delta_t_proc`
   stretches past the ~2 microsecond threshold. A SIMD-faithful host keeps the kernel inside the
   budget; a faking or degraded runtime cannot.

3. **Epistemic side-channel assays.** Sequences indistinguishable from business logic measure CMOV
   latency and speculation-mitigation state. These detect injected conditional branches and
   hardware-illusion tricks without ever looking like a check. All assay results fold into the
   receipt:

   `R_receipt = BLAKE3(Delta_S || N || Cycles || StrideChecksum)`

   where `Delta_S` is the state delta, `N` the epoch nonce, `Cycles` the measured cycle count,
   and `StrideChecksum` the digest of the induced access pattern.

## What Host Sees vs What Notary Measures

| Surface | Host sees | Notary measures |
|---|---|---|
| Code | Opaque `i64.load` / `xor` / `rotr` stream | Verification circuit plus a timing gate |
| Memory | Contiguous u32 column reads | L1/L2 prefetch timing, bus saturation |
| Arithmetic | Bitwise masking | Branchless execution integrity (injected branches detected; enforced via
  [scripts/branchless_manifest.toml](../../../scripts/branchless_manifest.toml) +
  [27 §6](27-branchless-kernel-abi.md), ADR-14 remainder — audited residuals:
  transpose 12 / filter 2, INFORMATIONAL mode) |
| Output | Opaque 128B frame | Proof-of-Bare-Metal and zero-allocation admission |

Every column is load-bearing: the host cannot distinguish the verification work from the payload
work, yet each surface contributes an independent measurement into the folded receipt.

## The Blinded Inspection Loop

1. `ggen` synthesizes the turnstile: a SHACL rule, a cache-stride test, and a nonce fold.
2. The output is a ~24KB stripped, zero-import WASM module — no imports, no names, no metadata.
3. The host maps the Arrow buffer into WASM linear memory and invokes `eval_turnstile`.
4. Execution completes in ~1.4 microseconds with zero heap allocations.
5. The notary checks:
   - `Delta_t_proc` against the bare-metal latency profile;
   - cycle count against the known instruction-path length;
   - the algebraic digest against the expected fold of `Delta_S` and `N`.

Any check failing outside its band refuses the turn. There is no retry path inside the RTT
window.

## Security Framing

The precision claim: the hosting party cannot know *what* is being measured, and cannot pause to
inspect the kernel without failing the real-time window. This is an **asymmetry claim** (see
[16-ahdclt.md](16-ahdclt.md) for the AHDCLT construction), not a formal proof.

Known host-side countermeasures exist: constant-time-faking emulators, and hardware illusion via
nested virtualization that presents fabricated timing. The design's answer is the physical time
budget — an emulator must either reproduce bare-metal cycle-exact latency or miss the window.
That budget is itself attackable in principle (a sufficiently faithful hardware simulator, or a
dedicated co-resident machine forwarded at wire speed), and this is flagged as an **open
adversarial question**: quantify the cost curve of emulating the full probe stack (stride traps +
SIMD width + side-channel assays + RTT budget) versus running honest bare metal.

## Standing

UNVERIFIED design hypothesis. No probe-class measurement, no WASM synthesis, no latency-profile
comparison has been executed in this repository. The ~1,150-cycle band, ~1.4 microsecond figure,
and 300-1000% spike range are illustrative, not measured.

Ceiling cross-reference: any real-hashing probe is bounded by the measured kernel hashing
ceiling of 1.3-1.85 GiB/s/core (see [48-lsl-rederivation.md](48-lsl-rederivation.md)); probe
designs must budget against that ceiling, not against memory-bus-saturating targets.

Timer anchor: the `gage_rr` gauge-R&R study landed with an ALIVE receipt
(gate-gage-rr.md), so timer precision on this host now has a measured anchor rather than an
assumed one.

## See Also

- [14-blinded-wasm.md](14-blinded-wasm.md) — the blinded-WASM substrate this escalates.
- [16-ahdclt.md](16-ahdclt.md) — the AHDCLT asymmetry claim cross-reference.
