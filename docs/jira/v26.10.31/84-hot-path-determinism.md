# Hot-Path Determinism Court (wave-14 lane 19)

Date: 2026-10-31. Lane: wave-14 lane 19 (recovery). Repo: /Users/sac/avec (one
canonical checkout, no worktrees). Subject: `crates/avec-arrow/src/predicates.rs`
— `filter_quads_eq` plus the `ct_eq` / `ct_lt` / `ct_select` family. Probe: a
standalone cargo crate at `/tmp/avec-det-probe19` (path dependency on
`/Users/sac/avec/crates/avec-arrow`; the repo gained no build artifacts from
this lane — the court never touched the workspace build).

Status: **ALIVE** — every command below was executed this session with its exit
code quoted. Court verdict: **PASS, 0 mismatches** on the working-tree subject;
**non-vacuous** (kills both injected mutants).

## Court Design

Fixed-seed adversarial court over 100,000 quad sequences + 50,000 primitive
draws. Generator: Knuth MMIX 64-bit LCG
(`s = s*6364136223846793005 + 1442695040888963407`), seed
`0xD3ADBEEF00000019`; only the high 32 bits are consumed per draw (low LCG bits
have short periods). Seed and generator are fixed so the entire corpus is
byte-replayable.

Sequence shapes drawn per trial (LCG-chosen salt class, needle biased to
boundary values {0, u32::MAX, 0x8000_0000} plus full-range draws):

- `n_quads` mostly 1..=96, plus forced edge sizes: every 997th trial `n_quads
  == 0` (101 trials; exercises the ct zero-trip/dummy-quad path at
  predicates.rs:117-127) and every 499th `n_quads == 1` (200 trials).
- Salt classes: full-random lanes; constant corpus; near-needle quads (exactly
  one lane off by one bit, rotating lane); sentinel-heavy alternating; boundary
  lane values; lane-uniform quads; forced all-match corpus.

Per trial the court performs four kernel calls with independent poison buffers
(0xDEADBEEF / 0xFEEDFACE / 0x0BADC0DE / 0x5EED5EED pre-fills, proving the
kernel never reads its output buffer):

1. **(a) Scalar-reference differential.** Kernel count and `out[..count]`
   content compared against an independently computed scalar filter
   (reference computed BEFORE the kernel call, from the data vector directly).
2. **(b) Byte-identical replay.** The same sequence is filtered twice into
   different poison buffers; `out[..count]` must be byte-identical (FNV-1a 64
   hash + direct slice equality). Total bytes hashed: 842,270.
3. **(c) Shuffled-order commutation.** A Fisher-Yates permutation `perm` is
   applied to the quads (`dp[q'] = data[perm[q']]`) and the permuted sequence
   is filtered; expected output is the ascending list of permuted positions
   whose quad CONTENT matches — i.e. each per-quad verdict is a function of
   quad content only, never of position or call history (no hidden cross-call
   state via the input path).
4. **(d) Interleaved re-run.** The original sequence is filtered a third time
   between other kernel calls; output must equal run 1 exactly.

Primitive differential (50,000 draws, 400,000 checks): `ct_eq_u32` /
`ct_lt_u32` vs `==` / `<` references; `ct_select_u32` / `ct_select_u64` /
`ct_select_usize` vs branch reference under all-ones/all-zeros masks;
`ct_eq_u64_mask` / `ct_lt_u64_mask` vs u64 references (full-range and
32-bit-derived operands, boundary-biased every 3rd draw); `ct_quad_eq_u32` vs
the 4-lane scalar conjunction, with a forced 3-lanes-equal-1-off adversarial
case every 7th draw.

## Results (subject = working-tree predicates.rs, 2026-10-31)

Build: rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0, macOS 26.2, arm64,
release profile `opt-level = 3`.

```
$ cd /tmp/avec-det-probe19 && cargo build --release && ./target/release/avec-det-probe19
seed=0xd3adbeef00000019 trials=100000 prim_draws=50000
shapes: n==0 trials=101 n==1 trials=200 all-match=13937 no-match=76785
(a) count+content vs scalar reference: checks=100000 mismatches=0
(b) same-sequence-twice byte-identical: mismatches=0 (bytes hashed total=842270)
(c) shuffled-order membership preserved: mismatches=0
(d) interleaved re-run identical: mismatches=0
ct-primitives vs reference: draws=50000 checks=400000 mismatches=0
TOTAL_MISMATCHES=0
RESULT: PASS
$ echo $?
0
```

Replay: a second execution printed a byte-identical receipt (`diff` clean,
stderr empty both runs), exit 0. The court binary exits 1 on any mismatch and
prints the trial, n, needle, and observed-vs-expected data to stderr.

## Non-Vacuity (mutant kills, /tmp copies — repo untouched)

A court that cannot flip is not a court. Two mutants were built from copies of
the crate at `/tmp/avec-arrow-mutA` / `-mutB` (standalone manifests; the repo
subject was never edited) and the identical court was run against each:

- **Mutant A** — `step` decoupled from the quad mask (`let step: usize = 1`):
  killed at trial 0 (the n=0 ct-trip path corrupts first: `got=1 want_len=0`).
  Exit 1, TOTAL_MISMATCHES=172025, RESULT: FAIL.
- **Mutant B** — hidden cross-call state (function-local `AtomicUsize` call
  counter; every 3rd call drops the last matching index): killed by court (b)
  at trial 2 (`B-FAIL trial=2 n=66 needle=0x80000000`, divergent replay
  hashes). Exit 1, TOTAL_MISMATCHES=46121, RESULT: FAIL.

## Court-Spec Error Witnessed, Then Corrected

The first court-C formulation was wrong and the court caught it in itself: the
expectation was built as the SORTED set of original indices `{perm[q] : q
matches}` while the kernel legitimately emits ascending positions in its OWN
index space; counts matched on every failure (`gotp == wantp_len` on the first
run) while content diverged — the exact signature of a mis-specified
expectation, not a kernel defect. The expectation was corrected to the true
commutation law (verdict at permuted position q' == reference verdict on the
quad content living there); kernel behavior was identical before and after the
fix, and the corrected court passes. Recorded because a determinism court that
never failed against its own first draft would itself be suspect.

## Honest Note: Timing-Constancy vs RESULT-Constancy

This court witnesses **RESULT-constancy only**: deterministic, order-independent,
position-independent outputs under adversarial input. It says nothing about
timing. The branchless claim — data-independent *instruction streams* and
therefore constant-time behavior — is a property of disassembled `.text`, and
its evidence class is the disassembly audit gate (doc 27 §3,
`scripts/audit_branchless.sh` over `scripts/branchless_manifest.toml`, owned by
the scripts lane I14). The two are orthogonal:

- a kernel can be result-deterministic yet branchy (timing varies with data);
- a kernel can be branch-free in `.text` yet result-divergent (hidden state).

Neither court subsumes the other; both are required. Until the disassembly gate
witnesses clean `.text`, the predicates here remain **M=1 CANDIDATES,
UNVERIFIED for timing** exactly as stated in the predicates.rs module header —
this receipt must not be read as upgrading that standing.

## Replay

The probe crate is a /tmp lane artifact (ephemeral by design). The corpus is
fully determined by: the LCG constants and seed `0xD3ADBEEF00000019`, the shape
rules in Court Design, and the working-tree subject. Any probe rebuilt to that
specification against the same subject must print `TOTAL_MISMATCHES=0` /
`RESULT: PASS` and exit 0. Mutant receipts quoted above were captured in the
same session; `/tmp` artifacts are not durable replay keys.
