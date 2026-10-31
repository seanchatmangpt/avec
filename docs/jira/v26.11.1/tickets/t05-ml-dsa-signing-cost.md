# t05: ML-DSA (FIPS 204) Signing Flow and Cost Profile

Research ticket, v26.11.1 PQC wave, agent 6 of 20. Standing: UNVERIFIED — this
is a research/capture ticket; no code, no bench, no KAT run in this lane.
Claims checked against the FIPS 204 standard text are cited by algorithm and
table number; all timing, cycle, and optimization-payoff claims are marked
UNVERIFIED-KAT (they are exactly what the KAT/bench court must pin later).
Primary source of record: NIST FIPS 204, `NIST.FIPS.204.pdf` (2024-08-13);
text extracted to text during this wave (pdftotext) and cited as Alg 6/7/8
(keygen/sign/verify), Alg 16/20/21 (packing), Alg 28/29/34 (w1Encode,
SampleInBall, ExpandMask), Alg 30-33 (rejection samplers), Alg 37-40
(Decompose family), Tables 1/2/3. Cross-checked once against the
pq-crystals/dilithium reference `ref/sign.c` (master branch): the mu-chain
and rho-double-prime derivation agree.

Companions: t03 (KAT/ACVP formats), t04 (rejection + hints), t14 (GPU dispatch),
`docs/jira/v26.10.31/27-branchless-kernel-abi.md`,
`docs/jira/v26.10.31/76-gpu-row-design.md`, `docs/jira/v26.10.31/48-lsl-rederivation.md`
(CTQ-06 target-setting lesson), `docs/BRANCH_REGISTER.md`.

## 1. The signing flow (FIPS 204 Algorithm 7)

Setup, once per sign call (Algorithm 7 lines 1-7, outside the loop):

1. `skDecode` -> `(rho, K, tr, s1, s2, t0)`. No mask seed is stored; the
   signer re-derives it per signature.
2. `s1hat <- NTT(s1)`, `s2hat <- NTT(s2)`, `t0hat <- NTT(t0)` (l + k + k
   forward NTTs, once, outside the loop).
3. `Ahat <- ExpandA(rho)` (Algorithm 32): each entry from
   `RejNTTPoly(rho || s || r)` — SHAKE128 (the XOF `G`), 3-byte squeezes,
   rejection of values >= q. Output IS the NTT-domain matrix. k*l entries.
4. mu chain (the dispatch's "tr' chaining"): `mu <- H(BytesToBits(tr) || M',
   64)`, a 64-byte message representative. `tr = H(pk, 64)` was fixed at
   keygen (Algorithm 6 line 9). For pure ML-DSA,
   `M' = BytesToBits(IntegerToBytes(0,1) || IntegerToBytes(|ctx|,1) || ctx)
   || M` (Algorithm 2 line 10); the first byte 0 is the pure domain tag. For
   pre-hash HashML-DSA the tag is 1 and `M'` carries `|ctx|, ctx, DER(OID of
   the pre-hash), PH` — the `ph` term of the dispatch formula. mu may be
   computed in a different cryptographic module (standard's own note).
5. `rho'' <- H(K || rnd || mu, 64)` (Algorithm 7 line 7). Hedged variant:
   `rnd` fresh from an RBG; deterministic: `rnd = {0}32`. This is the ONLY
   difference between the two variants (FIPS 204 Section 6.2).

Rejection-sampling loop (Algorithm 7 lines 10-32), Fiat-Shamir-with-aborts:

1. `y <- ExpandMask(rho'', kappa)` (Algorithm 34): per polynomial r, squeeze
   `H(rho'' || (kappa + r), 2 bytes appended, 32c bits)` with
   `c = 1 + bitlen(gamma1 - 1)` = 18 bits (44) / 20 bits (65, 87), then
   BitUnpack into the range `[-gamma1 + 1, gamma1]`.
2. `w <- NTT^-1(Ahat o NTT(y))` — THE dominant product: k*l NTT-domain
   basemuls (pointwise 256-coefficient modular multiplies), plus l forward
   NTTs of y and k inverse NTTs.
3. `w1 <- HighBits(w)` — high/low split at `2*gamma2` (Decompose,
   Algorithm 37). Componentwise over k polynomials.
4. `ctilde <- H(mu || w1Encode(w1), lambda/4)` — 32/48/64 bytes. w1Encode is
   17-bit (44) or 18-bit (65/87) fixed-width SimpleBitPack per coefficient.
5. `c <- SampleInBall(ctilde)` (Algorithm 29): first 8 bytes of ctilde give
   the sign bits; Fisher-Yates position draws by 1-byte squeezes with
   rejection (Table 3 bound: <= 121 squeezes, <= 221 XOF bytes). c has
   exactly tau coefficients in {-1, 0, +1}.
6. `chat <- NTT(c)`; `<<cs1>> <- NTT^-1(chat o s1hat)`;
   `<<cs2>> <- NTT^-1(chat o s2hat)` (lines 17-19).
7. `z <- y + <<cs1>>` (response); `r0 <- LowBits(w - <<cs2>>)` (line 21).
8. Reject-or-reroll (line 23): if `||z||_inf >= gamma1 - beta` OR
   `||r0||_inf >= gamma2 - beta`, discard, `kappa <- kappa + l`, loop.
   Note the asymmetric bounds: z is checked at gamma1 - beta, r0 at
   gamma2 - beta. (Round-3 Dilithium checked r0 at gamma1 - beta; FIPS 204
   final tightened this to gamma2 - beta — verified in the extracted
   Algorithm 7 line 23 and in the reference `sign.c`.)
9. Pass path (lines 25-28): `<<ct0>> <- NTT^-1(chat o t0hat)`;
   `h <- MakeHint(-<<ct0>>, w - <<cs2>> + <<ct0>>)`. MakeHint(z, r) per
   coefficient is `[HighBits(r) != HighBits(r + z)]` (Algorithm 39). Second
   rejection: `||ct0||_inf >= gamma2` or the number of 1s in h exceeds omega
   -> reroll.
10. `sigma <- sigEncode(ctilde, z mod+- q, h)` (Algorithm 26).

Sizes (Table 2): pk 1312/1952/2592 B; sk 2560/4032/4896 B;
sig 2420/3309/4627 B for ML-DSA-44/65/87. Signature layout: ctilde
(lambda/4 = 32/48/64 B) + z (l * 256 * 18/20 bits) + hint (omega + k B,
Algorithm 20: k count bytes then <= omega position bytes).

## 2. The verify flow (FIPS 204 Algorithm 8)

1. `pkDecode`; `sigDecode` — HintBitUnpack (Algorithm 21) validates
   monotone positions and per-poly counts; malformed hint returns false
   immediately (line 3).
2. `tr <- H(pk, 64)`; `mu <- H(BytesToBits(tr) || M', 64)` — same mu chain
   as signing; the verifier recomputes both hashes.
3. `c <- SampleInBall(ctilde)`.
4. Reconstruction: `wApprox' <- NTT^-1(Ahat o NTT(z) - NTT(c) o NTT(t1 *
   2^d))`, i.e. `wApprox' = A*z - c*t1*2^d`. l forward NTTs (z), k*l
   basemuls, k basemuls for the c*t1 term, k inverse NTTs.
5. `w1' <- UseHint(h, wApprox')` (Algorithm 40): per coefficient,
   `m = (q-1)/(2*gamma2)`; Decompose; hint bit set shifts r1 by +-1 mod m.
6. `ctilde' <- H(mu || w1Encode(w1'), lambda/4)`.
7. Return `[||z||_inf < gamma1 - beta] AND [ctilde' == ctilde]`.

The bridge identity, derived from `t = A*s1 + s2` and `t = t1*2^d + t0`:

```text
A*z - c*t1*2^d = (w - c*s2) + c*t0
```

so the hint must carry the verifier from its computed wApprox' to
`HighBits(w - c*s2)`... precisely: UseHint(h, wApprox') = HighBits(w), the
committed w1, because MakeHint at line 26 was taken with
`r = w - c*s2 + c*t0` and `z = -c*t0`: `r + z = w - c*s2`... no — the hint
is *checked* against HighBits(w) indirectly: w1 committed inside ctilde IS
HighBits(w), and the pair (h, wApprox') reconstructs it exactly when
||ct0||_inf < gamma2 and #h <= omega hold. The delta between wApprox' and w
is `c*t0 - c*s2`; both are small by the rejection checks (r0 bound gamma2 -
beta and the ct0 bound gamma2).

Verify is rejection-free: exactly one pass, no reroll, deterministic
latency. Every input to the timing profile (pk, sigma, M) is public, so a
constant-time verify is structurally easy — verify is the constant-time-cheap
path, and the whole masking/branchless engineering budget concentrates on the
signer loop.

## 3. Cost profile

### 3.1 Ranked operations (per sign iteration)

Tier 1 — dominant, unconditional:

| op | count per iter | 44 | 65 | 87 | nature |
|---|---|---|---|---|---|
| basemuls in `A*y` | k*l | 16 | 30 | 56 | 256 coeff-mults each, widening 32x32->64 + reduction |
| forward NTTs (y) | l | 4 | 5 | 7 | 1024 butterflies each (8 layers x 128) |
| inverse NTTs (w) | k | 4 | 6 | 8 | same butterfly count |
| ExpandMask XOF | l polynomials | 2304 | 3200 | 4480 | bytes squeezed per iteration |

Tier 2 — comparable to each other (exact order UNVERIFIED-KAT):

| op | count | 44 | 65 | 87 |
|---|---|---|---|---|
| sparse cs1+cs2 (polys) | l+k | 8 | 11 | 15 |
| ctilde absorb (bytes) | 64 + w1 bytes | 2240 | 3520 | 4672 |
| sparse ct0 (polys, pass path) | k | 4 | 6 | 8 |
| w1Encode output (bytes) | k * 544/576 | 2176 | 3456 | 4608 |

Ranking argument: A*y carries the only O(k*l*n) widening-multiplier work and
runs unconditionally every iteration; everything else is add/sub, hash, or
bit-shuffle. Hashing is minor against Tier 1 (KB-scale XOF volume at
GiB/s-class throughput) but is NOT free at optimized arithmetic speeds; the
exact Tier-2 ordering (sparse mults vs SHAKE volume) is UNVERIFIED-KAT until
benched on the target silicon. Note also the repo has a BLAKE3 receipt
(1.85 GiB/s/core NEON, `48-lsl-rederivation.md`) and ZERO SHAKE receipts —
Keccak-f1600 throughput on Apple Silicon is UNVERIFIED-KAT.

### 3.2 The precompute tuples / w = A*y anchor

- FIPS 204 itself writes the precompute-tuple discipline: Algorithm 7 stores
  `<<cs1>>`, `<<cs2>>`, `<<ct0>>` to avoid recomputation (standard's own
  double-angle-bracket note, Section 6.2).
- w = A*y is computed at line 12, BEFORE the ctilde hash (line 15) and before
  any rejection decision (line 23). It is unconditional per-iteration work:
  the fixed floor cost of every reroll. The with-Gemini wave-3 deliverable
  "w=Ay precompute tuples" anchors exactly here — the batchable tuple is
  `(rho'', kappa) -> (yhat, w, w1)` and its expected iteration count is
  Table 1's repetitions column: 4.25 / 5.1 / 3.85 for 44/65/87.
- E[sign] = repetitions x (Tier-1 + Tier-2 floor) + pass-path tail (ct0,
  MakeHint, encodings). The Appendix C cap is 814 iterations (Table 3) at a
  2^-256 reach probability; a capped implementation must destroy intermediates
  and return a constant-shape error.
- The standard's own precompute lever (Section 4): "one can precompute and
  store Ahat to speed up signing and verifying" — storage-vs-recompute is an
  explicit tunable: sk stores the 32-byte rho seed, so Ahat (k*l*256*4 B =
  16/30/56 KB per parameter set) is a per-key RAM cache decision.

### 3.3 What masked/branchless implementations must protect

Secret ledger (never leaks via timing, branch, or memory pattern):
xi (seed), rho' (ExpandS seed), K, rnd, rho'', s1, s2, t0, y through the
entire loop, and the unreleased (z, h).

Public by commitment: ctilde, c, z, h after sigEncode; w1 is public once
committed into ctilde; w1' at verify is public (computed from public
inputs). Only w1' at verify is public, and verify has no secrets at all —
the dispatch line "y remains secret through the loop; only w1' at verify is
public" is exactly right, with the addition that w0 = w - c*s2 intermediates
and ct0 are also secret-shaped on the signer side.

Concrete timing channels to close:

1. The rejection loop count itself: iteration count correlates with the
   norms of y-derived quantities. Per-iteration work is constant-time, but
   the loop count is data-dependent. Mitigations (dummy iterations, masking)
   are UNVERIFIED-KAT; Appendix C permits capping.
2. Norm checks with early exit — must lower to M=1 predicated reductions
   (CSEL/CSINC/CSETM per `27-branchless-kernel-abi.md`), never `if max >
   bound { return }` inside the loop.
3. The hint-count check (#1s > omega) and MakeHint's inequality — same M=1
   treatment; the hint count is public data but the branch pattern still
   must be input-shape-invariant per the branch register discipline.
4. RejNTTPoly/RejBoundedPoly loops: control flow depends only on the public
   XOF stream, so they are CT-safe in principle; the OUTPUTS s1/s2 are
   secret, so the kernels still go in the branch register for audit.
5. HintBitUnpack early exits on malformed input: input is public
   (attacker-controlled), so early exit is legitimate — but the CC=1 audit
   still requires the validation restructured as fixed-shape scan + select
   (see Section 4, hint packing).

## 4. The avec angles

1. A*y as the batch NTT problem. The k*l basemuls are independent 256-lane
   23-bit modular work units; k accumulator polynomials each reduce l
   basemuls. This is the same shape as avec-blake3's `hash_many_simd`
   (independent units interleaved in vector registers: 4-way NEON, 8-way
   AVX2 parent reduction). Dispatch law from `76-gpu-row-design.md` applies
   unchanged: per-signature arithmetic state is 16-56 KB — deep in the
   CPU-plane small-batch class; GPU dispatch latency (4-8 us Apple UMA)
   would dominate, so a GPU NTT row is refused by the existing crossover
   analysis, not by a new argument. Bench-row discipline: measure basemuls/s
   and signs/s/core min-of-5 first, THEN ratify an LSL — the CTQ-06 lesson
   (`48-lsl-rederivation.md`) is that hand-picked targets get refuted.
2. c*s1 sparse multiplication. c has exactly tau nonzero +-1 coefficients, so
   `c*p = sum of +-shift(p, i)` under negacyclic reduction
   (`X^i mod (X^256 + 1)` negates the wrapped half) — O(tau*n) integer
   add/sub with zero multiplications: 9984/12544/15360 coefficient-ops per
   polynomial for tau = 39/49/60. This replaces Algorithm 7's NTT-domain
   route (NTT(c) + l or k basemuls + INTTs per product) with a
   branchless-shift-add kernel. Accumulator width is safe in 32 bits:
   tau * max|t0| = 60 * 2^12 < q, and tau * eta * eta < q trivially.
   Whether sparse beats the NTT route on Apple Silicon is UNVERIFIED-KAT.
3. Hint packing as a CC=1 kernel problem. HintBitPack is data-dependent
   compaction (nonzero positions of h into omega + k bytes); NEON has no
   vector compact/scatter (SVE2 `compact` exists but is out of target ISA).
   A branchless form needs fixed-shape prefix-scan compaction or masked
   per-lane stores. HintBitUnpack adds sequential validation
   (monotone positions, count bounds) with early `false` — restructure as
   fixed-shape scan + select, the same discipline as the parquet RLE
   decoder's malformed-input handling.
4. Fixed-width bit-packing reuse. w1Encode (17/18-bit), zEncode (18/20-bit),
   t1Encode (10-bit) are LSB-first fixed-width field packing — the same
   class as `avec-parquet` `bitslice.rs::pack_vertical(values: &[u32],
   w: u8)`, which is already width-parameterized. ML-DSA encodings are
   new call sites for an existing primitive family, not new primitives.
5. XOF is the missing primitive. FIPS 204 pins SHAKE128/SHAKE256 (FIPS 202);
   the repo's sovereign BLAKE3 does not substitute. Needed: keccak-f1600
   with multi-stream interleaving (the sponge state is 25 x 64-bit lanes —
   the same many-at-once shape as `hash_many_simd`). Zero keccak code
   exists in the repo today; throughput and the right interleaving width
   are UNVERIFIED-KAT.
6. 64-byte quad alignment. The Quad64 layout discipline (one cache line per
   record, `23-64b-quad-spec.md`) maps onto polynomial storage: one 256-coeff
   polynomial is exactly 1 KiB in u32 lanes — a natural 64-B-aligned
   `avec-arrow` Buf; Ahat rows and the w/y work sets want the same
   alignment discipline as every other avec buffer.

## 5. Open items

1. KAT/ACVP court: every byte-level chain claim in Section 1 (mu inputs and
   order, rho'' derivation, ctilde length, encodings, hint layout) must be
   pinned by FIPS 204 ACVP vectors before any implementation lands.
2. Bench before targets: signs/s/core and basemuls/s/core, min-of-5 with a
   CV gate, per parameter set, before any LSL is proposed (CTQ-06 lesson).
3. Sparse-vs-NTT crossover for c*s1/c*s2/c*t0 on Apple Silicon
   (UNVERIFIED-KAT): bench both routes, per parameter set.
4. Ahat cache policy: seed-only sk vs 16/30/56 KB Ahat residency per key;
   interacts with the one-canonical-checkout memory discipline for long-run
   signing services.
5. SHAKE engine family: interleaving width (2/4/8-way), whether to share
   the hash_many_simd dispatch shell, and its own throughput receipt.
6. HintBitPack/Unpack CC=1 kernels: pick fixed-shape compaction strategy;
   add both to the branch register with instruction-level receipts.
7. Pre-hash (HashML-DSA) and external-mu API surface: the standard allows
   mu computation in a separate module — decide the avec API split
   (mu-compute vs sign-with-mu) so the turnstile/notary path can pre-hash
   large payloads.
8. Iteration-cap policy: whether to bound the sign loop (Appendix C permits
   up to 814) and the constant-shape failure for cap exhaustion.
9. Masked implementation scope: Section 3.3's channel list is the audit
   input, but no masking scheme is evaluated in this ticket (all masking
   cost claims would be UNVERIFIED-KAT).

## See Also

- `docs/jira/v26.10.31/27-branchless-kernel-abi.md` — M=1 predication primitives
- `docs/jira/v26.10.31/76-gpu-row-design.md` — CPU/GPU dispatch law
- `docs/jira/v26.10.31/48-lsl-rederivation.md` — CTQ-06 target-setting lesson
- `docs/BRANCH_REGISTER.md` — CC=1 audit register of record
- NIST FIPS 203/204/205 — ML-KEM / ML-DSA / SLH-DSA
