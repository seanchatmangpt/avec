# t04: ML-DSA Rejection Bounds and Hint Mechanics

| | |
|---|---|
| Ticket | t04 (v26.11.1 PQC wave, agent 5 of 20) |
| Type | Research — parameter, rejection-loop, and hint-encoding analysis; no code |
| Standing | RESEARCH-ONLY (input to the ML-DSA wave plan) |
| Flags | `UNVERIFIED-KAT` = claim not executed against real vectors this session;
`UNVERIFIED-SPEC` = recalled from FIPS 204 / round-3 spec, not re-fetched from
csrc.nist.gov this session |

Slot note: the dispatch assigned this filename; `00-index.md`'s Phase-0 table lists
t04 as `t04-falcon-fn-dsa-feasibility.md` (t07/t14 already diverge the same way).
Index amendment belongs to lane 1; this ticket claims the t04 slot as dispatched.

## 1. The Parameter Sets

All three sets share n = 256, q = 8380417, and R_q = Z_q[X]/(X^256 + 1). One table,
with every dispatch-prompt guess corrected inline where the arithmetic contradicts it.

| Parameter | ML-DSA-44 | ML-DSA-65 | ML-DSA-87 | Note |
|---|---|---|---|---|
| n | 256 | 256 | 256 | coefficients per polynomial |
| q | 8380417 | 8380417 | 8380417 | = 2^23 - 2^13 + 1 |
| d (dropped bits) | 13 | 13 | 13 | t = t1*2^d + t0; t0 packed, t1 in pk |
| tau (challenge wt) | 39 | 49 | 60 | nonzero +/-1 coeffs of c |
| lambda (target sec) | 128 | 192 | 256 | c-tilde is lambda/4 bytes |
| gamma1 | 2^17 = 131072 | 2^19 = 524288 | 2^19 = 524288 | see flag F1 below |
| gamma2 | 95232 | 261888 | 261888 | (q-1)/88 resp. (q-1)/32 |
| eta | 2 | 4 | 2 | s1, s2 coeff range [-eta, eta] |
| beta = tau*eta | 78 | 196 | 120 | norm margin for z and r0 checks |
| omega (max hints) | 80 | 55 | 75 | hint budget in sig encoding |
| k x l (A is k x l) | 4 x 4 | 6 x 5 | 8 x 7 | A-hat is stored NTT-domain |
| pk bytes | 1312 | 1952 | 2592 | derived below |
| sk bytes | 2560 | 4032 | 4896 | derived below |
| sig bytes | 2420 | 3309 | 4627 | derived below |

Corrections to the dispatch prompt, with receipts:

1. **Lambda is 128/192/256, not "112/192/192".** The 112 guess is the AES-128-class
   category legacy, not the FIPS 204 column. Receipt: c-tilde lengths 32/48/64 B are
   exactly lambda/4, and those lengths are themselves forced by the published
   signature sizes (see the derivation below). Any other lambda breaks the size
   arithmetic. (UNVERIFIED-SPEC, but internally load-tested against three size
   receipts per set.)
2. **gamma1 for ML-DSA-87 is 2^19 = 524288 (flagged F1).** The signature-size
   arithmetic admits only one value: 4627 = 64 (c-tilde) + 7*256*20/8 (z at 20
   bits/coeff) + 75 + 8 (hint = omega + k). z needs log2(2*gamma1) bits, so 20 bits
   forces gamma1 = 2^19. With 2^18 the z block is 4256 B and the sig is 4403 B, which
   is not the published number. Secondary sources recalling 2^18 are the failure mode
   ADR-19 calls out (unverified parameter numbers entering as facts). Flag stays
   until the byte-exact FIPS 204 Table 2 re-derivation in the wave plan closes it.
3. **"Expected iterations ~ tau*e" is not the ML-DSA shape.** No tau*e factor appears
   anywhere in the uniform-y rejection analysis. That e-shaped acceptance belongs to
   Gaussian-sampler rejection (Lyu12-style, and FN-DSA/Falcon), not to ML-DSA's
   uniform masking polynomial. See section 2.3 for the actual model.

Size derivation receipts (each row re-derives the published byte count exactly; the
published counts are UNVERIFIED-SPEC until the wave-2 KAT receipts land):

```text
pk = 32 (rho) + k * 256 * 10 / 8            (t1: 10 bits/coeff; range [0,1023])
    44: 32 + 1280 = 1312   65: 32 + 1920 = 1952   87: 32 + 2560 = 2592   OK

sk = 128 (rho 32 + K 32 + tr 64)            (tr is 64 B in FIPS 204, not 32)
   + l*256*b(eta)/8 + k*256*b(eta)/8 + k*256*13/8   (t0: 13 bits/coeff)
    b(eta) = 3 bits for eta=2 (2*eta+1=5 values), 4 bits for eta=4 (9 values)
    44: 128 + 384 + 384 + 1664 = 2560   OK
    65: 128 + 640 + 768 + 2496 = 4032   OK
    87: 128 + 672 + 768 + 3328 = 4896   OK

sig = lambda/4 + l*256*(2*log2(gamma1))/8 + (omega + k)
    44: 32 + 4*256*18/8 + (80+4)  = 32 + 2304 + 84 = 2420   OK
    65: 48 + 5*256*20/8 + (55+6)  = 48 + 3200 + 61 = 3309   OK
    87: 64 + 7*256*20/8 + (75+8)  = 64 + 4480 + 83 = 4627   OK
```

Nine exact hits across three formulas is the strongest evidence this session produced
that the table above is the FIPS 204 table. It is still not a receipt; it is a court
input.

## 2. The Rejection Loop

### 2.1 The loop shape

Per FIPS 204 signing (structure-faithful sketch; the byte-exact pseudocode is wave-2
KAT material, not quoted from memory here):

```text
mu  = H(tr || ctx || M)          # 64-byte message representative
rho' = H(K || mu || rnd)         # rnd = 0 for deterministic, 32 B for hedged
kappa = 0
loop:
    y  = ExpandMask(rho', kappa) # l polys, coeffs uniform in (-gamma1, gamma1)
    w  = A * y                   # NTT domain: w-hat = A-hat o y-hat, then INTT
    c  = SampleInBall(H(mu || w1Encode(HighBits(w))))   # tau nonzero +/-1 coeffs
    z  = y + c*s1
    r  = w - c*s2
    reject if ||z||_inf >= gamma1 - beta          # masking-norm check
    r0 = LowBits(w - c*s2)
    reject if ||r0||_inf >= gamma2 - beta         # rounding-stability check
    reject if ||c*s2||_inf >= gamma2 - beta       # hint-count guard
    h  = MakeHint(-c*s2, w - c*s2)                # see section 3
    reject if popcount(h) > omega
    output sigma = (c-tilde, z, h)
    kappa += l
```

The split the loop is built on: with alpha = 2*gamma2, every coefficient of r is
written r = r1*alpha + r0 with r0 in (-gamma2, gamma2] (centered decomposition;
HighBits/LowBits). The r0 check bounds the low part so that adding the small
correction c*s2 (sup-norm <= beta) cannot flip the high bucket: HighBits(w) and
HighBits(w - c*s2) agree coefficientwise. That is the whole game — the verifier only
ever sees high buckets.

### 2.2 Why signing is rejection-loop-dominant

Per iteration the loop pays: l forward NTTs, k*l pointwise products, k inverse NTTs,
two sup-norm folds, one hash, one encode. The accepted-iteration tail (hash, hint,
z-encode) is O(1) against that. So sign time is approximately E[iterations] x
loop-body, with the count's variance the only timing variance — which is why the loop
is where the performance budget and the constant-time budget both live.

### 2.3 Iteration-count model (first-order)

Per coefficient, r0 is roughly uniform over an interval of width ~alpha; the check
|r0| < gamma2 - beta fails with probability ~beta/gamma2 per coefficient.
Independence across 256 coefficients gives pass ~ (1 - beta/gamma2)^n. The z check
analogously loses ~(beta + 1 + E|c*s1|)/(2*gamma1) per coefficient. First-order
estimates:

```text
        r0-pass     z-pass      combined     E[iterations]
44      0.811       ~0.93-0.95   0.75-0.77    ~1.30-1.34
65      0.808       ~0.95        0.77         ~1.30
87      0.883       ~0.97        0.86         ~1.16-1.20
```

UNVERIFIED-KAT on two counts: (a) the r0 and z events are not independent and the
per-coefficient uniformity is approximate; (b) these are model numbers, never
measured here. Expect measured E[iterations] in the ~1.2-1.5 band; the wave-2 plan
should measure, not assume (open item 2). The dispatch's tau*e guess would have put
ML-DSA-44 at ~106 iterations per signature; the model above says ~1.3, and the
difference is a factor of ~80 in the performance budget — worth closing with a real
number before anyone tunes anything.

### 2.4 The leak surface, stated exactly

This is the part the dispatch got right, and it is the load-bearing constant-time
claim for wave 2. In `docs/DATA_INDEPENDENCE_AUDIT.md` vocabulary:

Must never be observable (secret-grade, CONTENT-INDEPENDENT discipline):

- y itself, and hence w of any iteration. A-hat is a random k x l matrix mod q with
  full column rank whp, so w determines y exactly; the accepted y plus the public z
  gives s1 = z - y with c public. **A single leaked accepted-iteration w is key
  recovery, not a side channel.** Rejected-iteration w/y are one step weaker and
  treated as secret-grade anyway.
- r0 and rejected candidate z: biased by the rejection event, so not zero-knowledge;
  discarding them is only as good as the zeroization of the scratch buffers.
- Per-coefficient control flow inside the norm checks: no early exit on the first
  violating coefficient. The check must fold to a violation predicate (unsigned
  compare per coefficient, OR-reduce) and branch once per loop body.

May leak (public-grade by construction):

- The iteration count. It appears in no output field; its distribution is a function
  of (gamma1, gamma2, beta, n, tau) and the public challenge distribution only — no
  key-dependent bits. Timing on the count is key-independent lottery, the PD class
  (PATTERN-DEPENDENT-NOT-SECRET), not TL. The loop's outer trip count may vary; what
  must not vary with data is the work *inside* one iteration.
- The accepted c-tilde, z, h: they are the signature.

Versus ML-KEM: decapsulation has no data-dependent loop at all (failed ciphertexts
rejection-branch into a pseudorandom key, one fixed path). ML-DSA signing is the
first kernel in the avec PQC plan whose runtime trip count varies, so it is the
first place the "outer count varies, inner work is fixed" pattern has to be stated
as a discipline and courted. Strict profiles may still prefer a fixed-trip loop
(run E[iterations]-rounded-up iterations always, release on pass) at the cost of
paying full price every sign — a wave-2 policy decision (open item 3).

### 2.5 Branchless join

The norm checks, the centered split, and MakeHint/UseHint all join the branchless
manifest (docs/BRANCH_REGISTER.md method: observed_mnemonics, `make branch-audit`
CC=1). The shape is exactly the `mask_words`/`count_valid` row class already
registered: fixed-stride compares and OR/popcount folds, no data-addressed branches.
The rejection branch itself is the single allowed branch per iteration.

## 3. Hint Mechanics

### 3.1 What the hint repairs

The verifier recomputes w-approx = A*z - c*t1*2^d = w - c*t0 (mod q) — the signer's
r = w - c*s2 perturbed by c*t0 instead of c*s2. UseHint(h, w-approx) must land on
HighBits(w - c*s2) = HighBits(w) exactly. The hint marks the coefficients where the
perturbation straddles a bucket boundary. Per coefficient, Z_q is partitioned into
m = ceil(q/alpha) buckets of width alpha = 2*gamma2: m = 44 for ML-DSA-44, m = 16
for ML-DSA-65/87 (190464*44 = 8380416 = q-1; 523776*16 = 8380416 = q-1 — exact, so
the buckets tile q-1 with one value to spare).

- MakeHint(z, r): h_i = 1 iff HighBits(r_i) != HighBits((r_i + z_i) mod q).
- UseHint(h, r): r1 = HighBits(r); for each h_i = 1: r1_i = (r1_i + 1) mod m.

The +1-mod-m convention and the boundary edge cases are FIPS 204 pseudocode
detail — UNVERIFIED-SPEC, closes against the reference implementation in wave 2.

### 3.2 Wire encoding: sparse index list + offsets

h has at most omega ones among k*256 coefficient positions (omega = 80/55/75). The
FIPS 204 encoding is two segments:

```text
seg 1: omega bytes   coefficient indices, per-poly runs in increasing order
seg 2: k bytes       slice boundaries: first-index of each polynomial's run
```

Decode is a bounds-checked monotone walk; validity requires strictly increasing
indices within each run, monotone boundaries, and total count <= omega. Expected
hint weight (first-order, n*k*beta/gamma2): ~8.4 (44), ~11.5 (65), ~9.4 (87) — the
omega budget is a proven bound from the security reduction, not the expectation
(UNVERIFIED-KAT on the proof-side detail).

### 3.3 The avec sparse-arena angle

The two-segment encoding is already an arena shape: a fixed-capacity index pool of
omega entries plus a k-entry boundary header. Mapped onto avec primitives:

- **In-memory representation**: preallocated pool `[u8; omega]` + `[u8; k]` counts,
  populated by a fixed-stride scan (the MakeHint pass writes only on predicate hits —
  a compaction-shaped kernel, store-on-predicate, exactly the `Bitmap::from_bools`
  pattern class). Structure stays fixed-capacity; only the count varies, and the
  count is public (it is in the signature), so the layout is PD-not-secret, not TL.
- **Dense alternative**: one Bitmap row of 256 bits per polynomial (128 B/poly,
  512 B/1 KiB fixed) — CI by construction, no compaction step, larger. For in-memory
  hint construction the dense bitmap may be the simpler CI court target, with the
  sparse form produced only at encode time.
- **Wire**: sparse always (omega + k bytes << dense 128k bytes).
- Decode fuzz corpus: malformed index/offset combos (unsorted, boundary past pool,
  count > omega, duplicate indices) must decode-fail loudly, never read OOB — joins
  the fuzz/mutation court corpus from wave-14's hardening pattern.

## 4. The NTT over q = 8380417

### 4.1 Factorization and root structure

The dispatch's "q - 1 = 2^23 * ..." is off: the 2^23 is the Proth-form power, not
the 2-adic valuation.

```text
q     = 8380417 = 2^23 - 2^13 + 1        (Proth form: 1023 * 2^13 + 1, k < 2^13)
q - 1 = 8380416 = 2^13 * 1023 = 2^13 * 3 * 11 * 31
v2(q-1) = 13   ->  primitive 2^13-th roots of unity exist in Z_q
```

A negacyclic NTT of length 256 needs a primitive 512th root: 2^9 | q-1 holds with
four powers of two to spare (the ring would support negacyclic NTTs up to length
2^12). Consequence: X^256 + 1 splits completely into 256 linear factors, the NTT is
a full 8-stage butterfly network, and the pointwise multiply is a plain
componentwise multiply with one Montgomery reduction per coefficient pair.

### 4.2 Contrast with ML-KEM's NTT

Same ring R_q = Z_q[X]/(X^256 + 1) for both schemes; the difference is split depth.

```text
                ML-DSA                  ML-KEM
q               8380417                 3329
q - 1           2^13 * 3 * 11 * 31      2^8 * 13
v2(q-1)         13                      8
512 | q-1       yes                     no
split depth     256 linear factors      128 quadratic factors (X^2 - z*X + 1)
stages          8 full butterfly        7 butterflies, leaves are quadratics
pointwise mul   1 coeff-wise mult       2-coeff blocks + extra twiddle mult
```

ML-KEM's v2(q-1) = 8 means no 512th root exists, so its NTT cannot finish the
split; its basemul operates on coefficient pairs inside each quadratic. This is a
real implementation consequence: the two kernels look similar but their basemul
layers are structurally different, and the twiddle table orderings (bitrev in/out
conventions) differ per spec. Shared-NTT-code review between the two waves should
assume nothing transfers except, possibly, butterfly scaffolding. All constants
above per spec — UNVERIFIED-SPEC, close in wave-2 KAT.

### 4.3 Montgomery machinery

q needs 23 bits, so products need 64-bit intermediates. A 16-bit Montgomery radix
(R = 2^16) is arithmetically feasible (q*(q-1)/R^2... the product q*(q-1) = 7.0e13
over R gives 1.07e9 < 2^32, fits u32), but reference implementations use 32-bit
Montgomery (R = 2^32) with the q^-1 mod 2^32 reduction constant; the canonical
primitive element used to derive twiddle tables is 1753 in the pq-crystals reference
(both constants UNVERIFIED-KAT — lift and KAT them in wave 2, never hard-code from a
survey). Limb width is a wave-2 design decision; the ML-KEM wave's 16-bit helpers
do not automatically transfer.

## 5. Open Items for the Wave-Plan Lanes

Dispatch phrased this as "wave-3"; per `00-index.md` numbering, ML-DSA is wave 2 /
`p03-wave2-plan.md` (wave 3 is SLH-DSA). Items below feed p03 primarily; items 1 and
6 also feed p04 if SLH-DSA proceeds. Numbering aside, the content is wave-plan
input either way.

1. **Byte-exact FIPS 204 Table 2 re-derivation court.** Closes flags F1 (gamma1 for
   87) and every UNVERIFIED-SPEC cell in section 1, plus the 1753 root and Montgomery
   constants. Authority: the FIPS 204 document itself + reference-implementation
   constants; the section 1 size-derivation receipts are the cross-check harness.
2. **Measured iteration counts.** Instrument the wave-2 sign loop; per-set measured
   E[iterations] replaces the section 2.3 model. Advisory only (conformance outranks
   throughput), but it sets the real sign-cost budget (~1.3x loop-body, not ~80x).
3. **Iteration-count policy.** Natural rejection loop vs fixed-trip loop; document
   the leak surface per section 2.4 in the kernel spec, and join the branchless
   manifest with the norm-check and hint kernels' observed_mnemonics.
4. **Hint arena layout + fuzz corpus.** Pick pool/bitmap in-memory form; spec the
   decode validity rules; feed the malformed-encoding corpus to the mutation courts.
5. **NTT court.** Round-trip + convolution KAT against schoolbook X^256+1 multiply;
   decide limb width; explicit review gate on any code shared with the ML-KEM wave.
6. **KAT corpus.** ACVP-style sign/verify vectors per parameter set are the only
   closer for every UNVERIFIED-KAT flag in this ticket. Coordinate with t02 (the
   FIPS 204 spec ticket) so exactly one canonical parameter table exists across the
   milestone — two lanes holding parameter tables is O(N^2) drift.

## Sources

- FIPS 204 (FIPS 205 companion volume), csrc.nist.gov — parameter table, algorithm
  shapes, sizes; recalled, not re-fetched this session (UNVERIFIED-SPEC).
- CRYSTALS-Dilithium round-3 specification and pq-crystals/dilithium reference
  implementation (params.h constants; 1753 root, Montgomery setup) — UNVERIFIED-KAT.
- CRYSTALS-Kyber round-3 specification (NTT split-depth contrast, q = 3329).
- Repo grounding: docs/DATA_INDEPENDENCE_AUDIT.md (CI/PD/TL classes),
  docs/BRANCH_REGISTER.md (observed_mnemonics, branch-audit gate),
  crates/avec-arrow/src/bitmap.rs (mask_words, count_valid, Bitmap),
  docs/jira/v26.11.1/00-index.md (slot contract, wave numbering),
  docs/jira/v26.11.1/tickets/t07-hqc-bike-survey.md (format + CT precedent).
