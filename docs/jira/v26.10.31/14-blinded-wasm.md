# Blinded WASM Turnstiles: Notary-Distributed Stripped Kernels

## Topology

```
                        NOTARY ( knows everything )
                              |
                emits W: stripped ephemeral WASM (<32KB)
                [ nonce N | ephemeral logic | circuit hash ]
                              |
        +---------------------+---------------------+
        v                                           v
   PARTY A (asset proof)                      PARTY B (liquidity proof)
   runs W in zero-host-import                 runs W in zero-host-import
   sandbox over PRIVATE Arrow buffer          sandbox over PRIVATE Arrow buffer
        |                                           |
        +-- returns 128B frame <--------------------+
            { payload | t_rx | t_tx | execution digest }
                              |
                              v
                 NOTARY SIEVE COURT
                 - distance bounding: RTT <= epsilon
                 - BLAKE3(result) invariant check
                 - commit receipt to avec Parquet log
```

The notary ships an ephemeral, stripped WebAssembly kernel `W` to each party.
Parties execute `W` locally over their private Arrow buffers and return only a
128-byte attested frame. Raw data never leaves a party's address space; only
the notary can reconstruct what `W` computes, and only because it authored `W`.

## Why Stripping Defeats Inspection

A party (or the LLM/proxy governing the party) that inspects `W` before
running it must not be able to recover the business logic. The stripping
pipeline removes every recoverable semantic channel:

- `wasm-opt --strip-all` removes name sections, symbol tables, DWARF debug
  info, and string literals. No function is named `check_liquidity_ratio`;
  no error string says "insufficient collateral".
- Control flow is flattened to `call_indirect` through pointer tables. The
  dispatch graph is data, not structure; a static decompiler sees a table
  walk, not a decision tree.
- Business predicates become opaque arithmetic: `i64.xor`, `i32.rotr`, raw
  linear-memory offsets with no symbol binding. The ratio test is a bitmask
  fold over scratch pages.
- Because `W` is <32KB and single-purpose, there is no redundant context
  (no library code, no logging, no alternate paths) from which an intercepting
  LLM could infer intent by elimination.

An intercepting LLM or MITM proxy sees a blob of arithmetic with a nonce in
it. There is no reconstructable semantics to prompt against or to reroute.

## Hermeticity

`W` declares **zero host imports**: no sockets, no files, no clock, no OS
access. The sandbox is a pure function over linear memory.

Single entrypoint:

```text
eval_turnstile(params: *kernel_params_t,
               private_buffer: *const u8,
               out_receipt:   *mut kernel_receipt_t)
```

`kernel_params_t` (notary-authored, nonce-bound):

| field | type | meaning |
|---|---|---|
| `nonce` | `[u8; 32]` | fresh per-turnstile nonce N |
| `buffer_len` | `u32` | expected private-buffer byte length |
| `expected_shape_id` | `[u8; 32]` | SHACL/POWL fingerprint of the Arrow schema |

`kernel_receipt_t` (exactly 128 bytes, the only output):

| field | type | meaning |
|---|---|---|
| `state_digest` | `[u8; 32]` | BLAKE3 fold of the computed state |
| `instruction_ticks` | `u64` | witness of executed instruction count |
| `status_mask` | `u32` | typed pass/refusal bitfield |
| padding | to 128B | fixed-width wire frame |

No dynamic allocation, no I/O, no ambient authority. The kernel cannot
exfiltrate the private buffer even if it wanted to: there is no egress
instruction in its vocabulary.

## Cycle Traps

The notary knows `W`'s exact instruction count because it authored `W` and
pinned it by `BLAKE3(W)`. This converts tampering into a timing failure:

- Any party that emulates, disassembles, or modifies `W` adds instructions:
  an emulator loop, a tracer, a re-synthesis pass. The inflated count shows
  up in `instruction_ticks` and in wall-clock latency (CNTVCT on aarch64,
  RDTSC on x86-64).
- The frame must return inside the distance-bounding window
  (`RTT <= epsilon`, see 12-distance-bounding-rtt.md). Interpretation
  overhead of even a thin emulator blows the processing budget
  (`t_tx - t_rx` accounting isolates the party's local execution time).
- Result: honest native execution is the only strategy that fits the
  window. Late or over-tick frames are refused outright; the trap does not
  need to identify *how* the party cheated, only *that* it did.

Scoping note (measured ceiling): the ~1.8us `dt_proc` budget only holds for
bounded scratch inputs. The measured single-core hashing ceiling is
1.3-1.85 GiB/s/core (48-lsl-rederivation.md), so folding a 256 MiB buffer
takes ~0.6s — six orders of magnitude past the distance-bounding window.
Cycle traps are a small-frame mechanism, not a bulk-data mechanism.

## Mechanical Invariants

| invariant | check | failure class |
|---|---|---|
| bytecode identity | `BLAKE3(W)` matches notary pin; else disqualification | mismatch |
| execution budget | `dt_proc <= ~1.8us` for the pinned kernel | REFUSAL-04:
EXECUTION_TIME_EXCURSION |
| zero allocation | delta linear pages == 0; heap-thrash abort | allocation observed |
| entropy binding | `BLAKE3(State \|\| N) == R_receipt` | REFUSAL-01: RECEIPT_MISMATCH |

The entropy binding ties the receipt to this turnstile's nonce: replaying a
frame from a prior turnstile (different `N`) fails the fold check. The
bytecode hash pins which kernel produced the frame; the nonce pins which
invocation. Together they make the 128-byte frame a replay-proof,
kernel-proof attestation.

## Structural Consequences

- **Zero information leakage.** Parties never share raw proprietary datasets.
  The only cross-party artifact is a 128-byte digest that is computationally
  uninformative without the private buffer.
- **No prompt-injection surface.** An LLM governing the node can only drive
  I/O (hand `W` the buffer, emit the frame). It cannot influence the
  computation: the logic is pinned bytecode it cannot read meaningfully and
  cannot modify without failing the hash and cycle checks.
- **Instant verification.** The notary precomputes expected witness values
  (instruction counts, digest shapes) when it authors `W`. Checking an
  incoming frame is a hash compare and two integer compares — microseconds
  per party, so the sieve court scales to wide fan-out.

## Standing

UNVERIFIED for the turnstile itself — no stripped-WASM kernel, distance-bounded
execution, or sieve-court run has been executed in this repo.

Landed context (receipted substrate the design now sits on): the sovereign
BLAKE3 core landed under AVEC-111 with receipts
(docs/jira/v26.10.29/AVEC-111-metal-gpu-and-blake3.md);
`blake3_profile` and `blake3_throughput` benchmark bins exist under
crates/bench; the branchless-manifest gate is live
(scripts/branchless_manifest.toml + scripts/audit_branchless.sh, see
gate-branchless-manifest.md).

The BLAKE3-pinned-artifact discipline parallels the existing graphlaw WASM
module (graphlaw kernel as a hash-pinned artifact consumed via wasmex); the
parallel is noted for provenance only — the current graphlaw module does not
implement stripped ephemeral turnstiles.

## See Also

- 15-arrow-microarch-probe.md
- 12-distance-bounding-rtt.md
- 13-time-crypto-prior-art.md
