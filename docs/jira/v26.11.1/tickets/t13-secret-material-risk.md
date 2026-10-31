# T13: Secret-Material Risk Survey (v26.11.1 PQC)

Status: UNVERIFIED (Phase-0 research ticket). This is the threat model the
secret-pool/zeroize deliverable defends against. Findings here receipt the
exploration, not the engineering: no pool code exists in this checkout yet
(the only arena today is `MmuBuf`, whose `Drop` is a bare `dealloc` with no
wipe — `crates/avec-arrow/src/mmu_arena.rs:104-111`). Every FIPS section
reference is a spec-reading claim for the t01/t02 spec lanes to pin against
the published text before it gates anything.

Companions: t08 (hand-rolled zeroize substrate), t11 (BEAM NIF surface),
t12 (constant-time audit), t14 (risk register). This ticket feeds them.

## 1. Key Material at Risk

| Material | What it is | Live when | Class |
|---|---|---|---|
| `dk` | ML-KEM decaps key (FIPS 203 §7) | arena, whole key life | long-lived |
| `sk` | ML-DSA sign key (FIPS 204 §5) | arena, whole key life | long-lived |
| `s, s1, s2, e` | secret-expanded vectors, keygen/sign/decap | transient per op | transient |
| `y` | ML-DSA per-signature mask from the private seed | transient per signature | transient |
| `ss` | the shared secret (FIPS 203: `K`), 32 bytes | derivation to handoff | short, top value |
| RNG/XOF | DRBG state, seed pairs, per-expansion XOF state | whenever derivation runs | root |

Details the table compresses:

- `dk = dk_PKE || ek || H(ek) || z`, where `dk_PKE` is the encoded
  secret vector `s`.
- `sk` is stored expanded `(rho, K, tr, s1, s2, t0)` or as the seed
  pair `(zeta, xi)` from which the whole key expands.
- The RNG row is the root row: everything above derives from it.

The seed model is what makes the last row the root row: FIPS 203 keygen is
`KeyGen_internal(d, z)` — the `(d, z)` pair reconstructs `dk` entirely
(`dk_PKE`, `ek`, `H(ek)`, `z` all derive from `d` plus `z`). Whoever holds
the seed pair holds the key, whether or not the expanded `dk` still exists.

Substrate fact: the only existing arena primitive, `MmuBuf`, is 16 KiB
page-aligned and wipes nothing on drop (bare `dealloc` at
`mmu_arena.rs:104-111`). Its bytes outlive the logical free until some
later allocation overwrites them. That is the default this work replaces.

## 2. Attack Classes

### 2a. Swap / cold-boot (passive, post-hoc)

DRAM is non-volatile on the timescale that matters: residual charge holds
for seconds to minutes at room temperature and extends further under
cooling (Halderman et al., "Lest We Remember", USENIX Security 2008).
An attacker with power-cut + physical access re-reads the DIMMs; an
attacker with later disk access reads the swap partition or a hibernation
image, where pages landed minutes earlier.

- In scope (mitigation): `mlock` to keep secret pages resident and
  unswept. This is a std-only libc call; the substrate default is
  `no_std` + `alloc` (`crates/avec-arrow/src/lib.rs` declares
  `#![no_std]` with `#[cfg(feature = "std")] extern crate std;`), so
  mlock is cfg-gated behind a std-only feature, never on the no_std
  path. Best-effort by nature: RLIMIT_MEMLOCK caps on Linux, Mach wire
  semantics on macOS.
- Out of scope, boundary named: encrypted swap / encrypted-at-rest is
  pre-boot and OS-domain. The pool's job ends at process memory.
- Residual carried to §4: hibernation (suspend-to-disk) writes RAM to
  disk regardless of mlock.

### 2b. Heap reuse-after-free / heap spraying

Freed `dk` pages retain content. In a long-lived VM the reuse window is
long: pages sit in allocator free lists (or in a pool ring awaiting
reuse) holding the old key. A disclosure primitive elsewhere in the
process combined with heap spraying into the stale pages turns residual
bytes into key recovery.

- Mitigation 1 — zeroize-on-Drop (the pool deliverable): every secret
  region is volatile-wiped before the deallocator sees it, so the free
  list holds zeros, not keys. Hand-rolled per governing discipline 2
  (zero external dependencies, E=0 runtime externals) — no `zeroize`
  crate; volatile stores so dead-store elimination cannot elide the
  wipe.
- Mitigation 2 — the alloc counter proves no copies escape:
  `avec_audit::alloc_counter::with_alloc_tracking`
  (`crates/audit/src/alloc_counter.rs`) is the existing scoped-counting
  primitive. The PQC court reuses it: keygen/sign/decap must allocate
  exactly the expected envelope, byte-exact counts — a hidden copy of
  `ss` shows up as an allocation the court can refuse. Final-state
  assertion on real counters, not call-counting of collaborators.

### 2c. Core-dump / ptrace — and the BEAM membrane question

mlock keeps pages resident; it does nothing against ptrace or a process
core dump. Process-hardening (`RLIMIT_CORE=0`, coredump filters) is
named and out of scope (process-launch domain), but the BEAM case is in
scope because the NIF surface is ours (t11).

The membrane question: a BEAM resource binary
(`enif_make_resource_binary`, zero-copy wrap — `resources.rs` /
`resbin.rs` in `crates/avec-beam`) keeps the underlying envelope alive
as long as any Erlang process holds a reference; the resource dtor fires
only on GC of the last reference (`dtor_stats.rs` records it). For the
non-secret `BatchEnvelope` that was fine. For `dk`/`ss` it means an
Erlang-side reference extends secret lifetime arbitrarily past the
operation that created it — the exact hazard the pool exists to bound.

The pool TTL is the answer: each secret envelope carries a TTL checked
at every NIF boundary crossing; an expired envelope answers with a
typed refusal, not a stale read. Erlang references cannot extend secret
life past the TTL no matter how long they hold on.

Subsidiary hazard: `erl_crash.dump` captures the BEAM heap. Secrets that
cross into BEAM binaries are in the dump. Zero-copy resource wrapping is
the structural defense — the secret bytes stay NIF-side. Note the
existing persist path serializes envelope payloads to disk in PLAIN
encoding (`ops_persist.rs`); secrets must refuse that path outright
(type-state, P-4) or a "protected" key lands in a file.

### 2d. RNG-state hazards

The deterministic-seed model inverts the usual intuition: the XOF/DRBG
state IS the key. FIPS 203 `KeyGen_internal(d, z)` derives everything
from the seed pair; FIPS 204's seed form `(zeta, xi)` likewise expands to
the full `sk`. Capture the DRBG/XOF state and every key derived from it
is reconstructable, past and future.

- The RNG-freshness court requirement: a court must prove one seed, one
  key — that keygen consumes fresh DRBG output and refuses seed replay
  across two generated keys. Falsifiable form: replaying the same seed
  through two keygen calls must be refused (the determinism that makes
  KATs possible is exactly the property the freshness court fences).
- Adjacent: KAT fixtures themselves are secret-shaped material — KAT
  files print `(d, z)` seeds on disk. Test-fixture handling is a real
  requirement, not hygiene theater (open item O-5).

## 3. Pool Design Requirements Implied

Each requirement traces to the attack class it answers.

- **P-1 Zeroize-on-Drop ring** (answers 2b). Every secret region —
  including transient `y`/`w0` scratch, not just `dk` — lives in pool
  pages volatile-wiped on Drop, before the deallocator sees them.
- **P-2 TTL expiry** (answers 2c). Every envelope carries a TTL checked
  at each NIF boundary; expired means typed refusal, never stale read.
- **P-3 mlock cfg-gating** (answers 2a). Std-only feature; the no_std
  default stays clean; lock state is loud — locked/unlocked/skipped
  reported, never silent.
- **P-4 No-Clone secret types** (answers 2b, 2d). Secret handles are
  move-only (no `Copy`, no `Clone`), no `Debug`/`Display` (format-leak
  path), no serde. Lifecycle as type-state — live, expired, wiped — so
  an expired read is a construct error, not a runtime hope. Secrets
  also refuse the persist path (2c, O-7).
- **P-5 Alloc-counter court** (answers 2b, 2d). `with_alloc_tracking`
  over keygen/sign/decap asserts the exact allocation envelope; any
  hidden copy refuses.
- **P-6 Constant-time wipe court** (answers 2b). Wipe via volatile
  stores (DSE-proof); the court walks the real buffer post-Drop and
  asserts zeros — Chicago discipline, final memory state, the same
  pattern as the `crates/integration/tests/membranes.rs` buffer walks.

Placement is an open item (O-6): pool in `avec-arrow` behind the std
feature, or a sibling crate consuming it. The no_std default must keep
compiling either way.

## 4. Residual Risks (the honest frontier)

The pool defends the passive/forensic class: power-cut forensics,
post-mortem heap inspection, crash-dump hygiene, stale-page reuse. It
does NOT defend the active-compromise class. Stated plainly:

- A hostile OS or hypervisor defeats all of §3. Swap encryption, memory
  ballooning, live-migration snapshots, page dedup, and hypervisor
  checkpointing are all OS/hypervisor-controlled. mlock is a request the
  kernel is free to honor or not.
- Hibernation images bypass mlock entirely: suspend-to-disk writes the
  pages mlock pinned.
- ptrace/coredump remain open against an in-process attacker; TTL +
  zeroize shrink the window, never close it.
- The persist path (`ops_persist.rs`) is a standing disk-escape route
  for anything that reaches it; P-4 must make secrets refuse it, and
  that refusal needs a court (open item O-7).
- Hardware attack classes (DMA/Thunderbolt, Spectre-class speculative
  leakage, Rowhammer, EM/power side channels) are out of scope; the CT
  domain is t12's, these are its hardware frontier, not the pool's.
- The eventual answer for the active class is sealing keys into
  hardware (HSM/TPM) — named as boundary, out of scope this milestone
  (O-8).

## 5. Open Items

- O-1 Index contract mismatch: `00-index.md` names this slot
  `t13-court-corpus-design.md`; the coordinator dispatched this file as
  `t13-secret-material-risk.md`. Agent 1 owns the index and must amend
  or renumber so the filename contract holds. Until then this file is a
  known contract violation, flagged here rather than papered over.
- O-2 Wave-numbering ambiguity: the coordinator calls zeroize-on-Drop
  "the wave-4 pool deliverable"; the index's W4 row is FN-DSA Falcon.
  Agent 1 reconciles.
- O-3 TTL semantics: wall-clock vs call-count vs BEAM-generation
  counter — decided in the integration plan (t11/W5 owner), with a
  court for whichever is chosen.
- O-4 mlock failure semantics: hard error vs loud-skip per platform
  (Linux RLIMIT_MEMLOCK, macOS Mach wire). The court must be able to
  distinguish "locked" from "skipped" — a gate that cannot flip is
  FAIL per house court discipline.
- O-5 KAT fixture handling: KAT files contain live seed material on
  disk; decide wipe-at-test-end vs sealed fixtures, and whether the
  KAT corpus carries the same P-4 handling as runtime secrets.
- O-6 Pool placement: `avec-arrow` std-feature vs sibling crate; the
  E=0 dependency discipline applies either way.
- O-7 Persist-path refusal court: prove a secret type cannot reach
  `ops_persist.rs` serialization (compile-time via type-state, with a
  mutation test as non-vacuity check).
- O-8 HSM/TPM sealing: named boundary; a future milestone's answer to
  the active-compromise class, not this one.
- O-9 FIPS erasure language: t01/t02 pin the exact FIPS 203/204
  erasure obligations ("cryptographically secure erasure" of
  intermediate values) and convert them into the court requirements
  this ticket assumes.

## See Also

- [00-index.md](00-index.md) — milestone charter and filename contract
  (this file is its known O-1 deviation).
- t08 (zero-dep crypto substrate), t11 (BEAM NIF surface), t12 (CT
  audit method), t14 (risk register) — sibling Phase-0 slots this
  ticket feeds.
- `crates/avec-arrow/src/mmu_arena.rs` — the existing arena primitive
  whose bare-dealloc Drop is the default being replaced.
- `crates/audit/src/alloc_counter.rs` — the scoped allocation-counting
  primitive P-5 reuses.
- `crates/avec-beam/src/resources.rs`, `resbin.rs`, `dtor_stats.rs`,
  `ops_persist.rs` — the BEAM resource membrane and its persist path.
- `docs/jira/v26.10.31/33-risk-register.md` — the parent milestone's
  DFMEA method this survey's successors inherit.
