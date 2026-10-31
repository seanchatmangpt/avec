# T03: KAT File Formats and ACVP Harness Shape (v26.11.1)

| Field | Value |
|-------|-------|
| Component | PQC conformance gate surface (research ticket) |
| Wave | v26.11.1 PQC wave, agent 4 of 20 |
| Priority | Blocker (the wave-2 KAT court parses these files) |
| Status | OPEN — research; uncertain claims tagged UNVERIFIED-KAT |
| Scope | This file only; no code, no git actions |

The wave-2 KAT court is the conformance gate: while it is red, every
throughput number in the wave is void. This ticket pins the exact file
formats that court must parse, where the official files live, what
committing them into avec costs legally, and the court design — so wave-2
implements against a verified surface, not a remembered one.

Receipt basis for the format claims: this ticket was written against the
shipped NIST sample files (`ML-KEM-keyGen-FIPS203` and
`ML-KEM-encapDecap-FIPS203` prompt/expectedResult JSON in
`usnistgov/ACVP-Server` `gen-val/json-files`), the ACVP ML-KEM JSON spec
(draft-celi-acvp-ml-kem-01), the mlkem-native and mldsa-native ACVP clients,
the pq-crystals NIST-KAT generator source, and the NIST PQC archive page.
Where a claim rests on a source I could not open, it is tagged
UNVERIFIED-KAT. Full URL list in the Receipts section.

## 1. The NIST ML-KEM KAT structure

Two formats matter. The official CAVP surface today is JSON (ACVP-Server);
the classic `.rsp` (PQCinterim/PQCinterop) format survives in submission-era
and community corpora. Wave-2's parser handles both, dispatched on shape.

### 1.1 ACVP JSON — the official CAVP surface (VERIFIED from shipped files)

Layout: per algorithm directory under `gen-val/json-files/`, a four-file set
— `registration.json`, `prompt.json`, `internalProjection.json`,
`expectedResults.json`. The top-level `isSample` flag selects behavior:
`isSample: true` generates fewer tests and always ships a complete
`expectedResults.json`; `isSample: false` may omit expectedResult files
(ACVP-Server README). For wave-2 fixtures, prefer `isSample: true` sets so
the expected bytes are guaranteed present.

Top-level prompt shape (observed verbatim in shipped samples):

```json
{
  "vsId": 42,
  "algorithm": "ML-KEM",
  "mode": "keyGen",
  "revision": "FIPS203",
  "isSample": false,
  "testGroups": []
}
```

keyGen: one group per parameter set; per-test fields are exactly `tcId`,
`d`, `z` in the prompt and `tcId`, `ek`, `dk` in expectedResult:

```json
{
  "tgId": 1,
  "testType": "AFT",
  "parameterSet": "ML-KEM-512",
  "tests": [
    {
      "tcId": 1,
      "d": "<32 B hex>",
      "z": "<32 B hex>"
    }
  ]
}
```

The shipped keyGen `dk` values end with the corresponding `ek` as a suffix —
consistent with the FIPS 203 expanded decapsulation key layout
`dk = (dk_pke || ek || H(ek))`.

encapDecap groups add `function` and (tr1) `keyFormat`; per-test fields are
conditional on `function`:

| function | prompt fields | response fields |
|----------|---------------|-----------------|
| encapsulation | `ek`, `m` (32 B) | `c`, `k` |
| decapsulation | `ek`, `c`, plus `dk` (expanded) or `seed` (64 B d/z, tr1) | `k` |
| encapsulationKeyCheck | `ek` | `testPassed` |
| decapsulationKeyCheck | `dk` | `testPassed` |

- `testType` is `AFT` for keyGen and encapsulation, `VAL` for decapsulation
  (implicit rejection path) and both key checks.
- `keyFormat` ("seed" / "expanded") appears on decapsulation groups in the
  FIPS203-tr1 revision; base FIPS203 supplies expanded `dk` only.
- Shared-secret spelling differs across formats: ACVP JSON uses `k`, the
  .rsp world uses `ss`. The parser aliases them.

Revisions: `FIPS203` and `FIPS203-tr1`. tr1 adds the seed/expanded
key-format groups. Both PQCP reference clients gate tr1 on ACVP-Server
version >= `v1.1.0.43` (`acvp_version_has_tr1` in mlkem-native and
mldsa-native `test/acvp/acvp_client.py`).

Observed sample sizes: 25 test cases per parameter set per mode (75 total in
the keyGen sample; encapDecap tcIds continue across groups, 25 per group).

UNVERIFIED-KAT: a `setUpExpr` field ("deterministic" / "randomized") appears
in secondary descriptions of the ML-KEM ACVP surface but is absent from the
shipped FIPS203 samples, from draft-celi-acvp-ml-kem-01, and from both PQCP
clients' parsers. Do not build the wave-2 parser around `setUpExpr` until
confirmed against the ACVTS demo server. The practice surface is: the server
always supplies `d`, `z`, and `m`, so every AFT vector is a deterministic
replay regardless.

### 1.2 Byte sizes (FIPS 203 — parser length checks)

Verified sizes from FIPS 203: ek is 384k+32; expanded dk is 768k+96; ct is
32(du+dv)+32; seed-format dk is 64 B (d||z); ss is 32 B. Per set:

- ML-KEM-512 (k=2): ek 800, dk expanded 1632, ct 768
- ML-KEM-768 (k=3): ek 1184, dk expanded 2400, ct 1088
- ML-KEM-1024 (k=4): ek 1568, dk expanded 3168, ct 1568
- dk seed format: 64 B in all sets; ss 32 B in all sets

Parsers length-check every field against these constants and REFUSE on
deviation. A wrong-length field is a parse failure, not a vector failure —
it means the wrong file was fed to the court.

### 1.3 Classic .rsp (PQCinterim / PQCinterop)

Verified from the pq-crystals NIST-KAT generator (`ref/nistkat/
PQCgenKAT_sign.c` in pq-crystals/dilithium — the same generator template the
KEM submissions used): the .rsp file opens with a `# <ALGNAME>` comment and
one vector block per count. Signing-scheme fields: `count`, `seed`,
`mlen`, `msg`, `pk`, `sk`, `smlen`, `sm`. KEM blocks use `count`, `seed`,
`pk`, `sk`, `ct`, `ss`.

The load-bearing generator mechanics, verified from that source:

- `seed` is 48 bytes of hex; it initializes an AES-256-CTR DRBG
  (`randombytes_init(seed, NULL, 256)`) which generates all per-vector
  randomness. Replay requires that DRBG, not just the seed bytes.
- Parsing convention: `FindMarker(fp, "label = ")` then read hex — and the
  generator writes labels with the value on the same line in .rsp
  (`fprintBstr(fp_rsp, "pk = ", ...)`) but emits bare labels in .req
  (`fprintf(fp_req, "pk =\n")`). A robust parser scans for the label and
  reads the hex that follows on the same or the next line.

FIPS-203-era variant (the coordinator-specified layout this ticket adopts as
the wave-2 target): `count`, `seed` = 64 bytes = `d || z`, then `ek`, `dk`,
`ct`, `ss`. No key-expanding DRBG is needed — split the 64-byte seed into
d and z and call KeyGen directly. Dispatch rule for wave-2: read the `seed`
length; 48 B means PQCinterim DRBG replay, 64 B means direct d||z. Accept
the label aliases `pk`/`ek` and `sk`/`dk` — corpora differ.

UNVERIFIED-KAT: which official corpus ships the 64-byte-seed .rsp variant.
The NIST archive's intermediate-values ZIP is draft-era (see section 2), and
the ACVP-Server surface is JSON-only. If no trustworthy 64-byte-seed .rsp
corpus can be pinned by wave-2, fall back to JSON as the sole fixture format
rather than regenerating vectors locally (regenerating would make the court
self-referential). Exact seed-split and label spellings of any candidate
corpus are UNVERIFIED-KAT until the file's SHA256 is pinned in the receipt.

## 2. Where to obtain the official KAT files; licensing and redistribution

### 2.1 Sources

Primary fixture source (official, complete, versioned):

- `github.com/usnistgov/ACVP-Server` → `gen-val/json-files/`
- Directories relevant to this wave:
  - `ML-KEM-keyGen-FIPS203/`
  - `ML-KEM-encapDecap-FIPS203/`
  - `ML-KEM-encapDecap-FIPS203-tr1/`
  - `ML-DSA-keyGen-FIPS204/`
  - `ML-DSA-sigGen-FIPS204/`
  - `ML-DSA-sigVer-FIPS204/`
  - `ML-DSA-sigGen-FIPS204-tr1/` (seed/expanded-key sigGen groups)
- Pin an ACVP-Server release tag (the PQCP clients' compatibility gate is
  `v1.1.0.43`) and record the tag plus each file's SHA256 in the fixture
  provenance header. Commit the four-file sets for the modes avec supports.

Supporting sources:

- NIST PQC archive page links `PQC-Intermediate-Values-nov2023.zip` — these
  are DRAFT-era values with two documented deviations from final FIPS 203
  (swapped XOF inputs i/j in KeyGen/Encrypt; a transposed bHat matrix for
  convenience). NIST itself says to use the final CAVP vectors instead.
  Use only as a debug corpus for intermediate-value stepping, never as
  release fixtures.
- NIST pqc-forum announcement of negative, intermediate, and edge-case
  ML-KEM test vectors — evaluate as the negative-test corpus for the court
  (modified-ciphertext decapsulation, oversized polynomial coefficients,
  corrupted H(ek) in dk). URL in Receipts.

### 2.2 Licensing and redistribution posture

Observed facts:

- The ACVP-Server repository exposes no license to the GitHub license API
  (license: null) and the pqc-archive page states no licensing terms for
  the intermediate-values files.
- The content is NIST work product; as a US Government work it sits in the
  public domain in the US under 17 USC 105. The ecosystem already treats
  these vectors as freely embeddable: wolfSSL documents CAVP vector usage,
  and the PQCP projects (mlkem-native, mldsa-native) ship NIST-derived
  vectors under permissive spdx headers (Apache-2.0 OR ISC OR MIT).

Posture for avec:

- Commiting the NIST JSON vectors as test fixtures is standard practice;
  treat them as facts (US-gov work, no creative expression), not as
  licensed code. avec remains a sovereign zero-dependency tree: vectors
  only, never vendored implementation code from PQCP projects.
- Every fixture file gets a provenance header record (in a sidecar
  manifest, since JSON cannot take comments) with: upstream repo + tag,
  source path, SHA256 of the file at pin time, revision string (FIPS203 /
  FIPS204 / tr1), and pin date. The court verifies the SHA256 before
  parsing — a drifted fixture is a REFUSED run, not a silent mismatch.
- Residual concerns are tagged UNVERIFIED-KAT: non-US jurisdiction status
  of US-gov test data, and any NIST server-side terms attached to files
  fetched from the live ACVTS demo server (as opposed to the GitHub repo,
  which is plain cloning). If the demo server imposes terms, the GitHub
  path avoids them.

## 3. Byte-parity court design (the conformance gate surface)

Pipeline — five stages, fail-closed at each:

1. **Pin**: verify fixture SHA256 against the provenance manifest. Mismatch
   → REFUSED (exit 1), no parsing.
2. **Parse**: strict JSON/.rsp parse. Unknown fields, wrong lengths
   (section 1.2 constants), zero vectors, or an expectedResult-less prompt
   → REFUSED with the offending path and tcId. A file with zero vectors is
   a refused run, never a pass (anti-vacuity: an empty court proves
   nothing).
3. **Run**: per vector, invoke avec's ML-KEM primitives:
   - keyGen AFT: KeyGen(d, z) from the vector's seeds.
   - encapsulation AFT: Encaps(ek, m).
   - decapsulation VAL: Decaps(dk or seed-expanded dk, c) — including
     implicit-rejection vectors (ciphertext mismatch must yield the
     vector's pseudorandom ss, not an error).
   - key checks VAL: ek/dk structural checks must return the vector's
     `testPassed` verdict.
4. **Compare**: byte-exact comparison of every artifact the vector carries
   — ek, dk, ct, ss/k (naming-aliased per section 1.1). All four, every
   vector; partial comparison is a court defect. Constant-time equality is
   not required (public test material) but costs nothing to reuse.
5. **Mint**: a receipt per fixture file, in the v26.10.31 receipt style
   (subject, commands + exits, verification ladder, standing). Subject
   identity = (fixture SHA256, ACVP-Server tag, tcId range, avec version,
   feature profile). Body carries a BLAKE3 digest over the per-vector
   verdict table (avec-blake3, the repo's own primitive) plus pass/fail
   counts. A missing or mismatched receipt field makes the run
   unverifiable — same discipline as the P28-SIL evidentiary chain in
   `docs/jira/v26.10.31/34-p28-sil-compliance.md`.

Gate semantics (the gate outranks every throughput number):

- The KAT gate is wired into the wave's verify battery (the
  `verify_v26.11.1.sh` pattern from v26.10.31) ahead of every bench gate.
  Battery rule: a FAIL on the KAT gate voids all bench receipts minted on
  the same subject — ctq_report and the wave receipts must consume the KAT
  verdict and print THROUGHPUT-VOID when it is red.
- All-or-nothing: one mismatching byte in one vector fails the gate. No
  percentage pass, no skip-without-reason. A skipped vector set prints
  `[SKIP] <reason>` and the gate reports PARTIAL, never PASS.
- Anti-vacuity mutation control (the repo's fsck-court pattern): the court
  self-test injects a one-byte flip into a locally generated vector and
  asserts the court FAILs it; a court that passes the mutant is vacuous
  and exits nonzero as a gate defect. Same shape as the corruption control
  in `crates/avec-git/tests/fsck_court.rs`.
- Determinism control: running the court twice on the same subject must
  produce identical verdict tables (ML-KEM replay is deterministic; see
  section 5). A nondeterministic court is a defect.

Placement: `crates/integration` tests or a dedicated court binary named
`kat-mlkem` (later `kat-mldsa`), registered in the gate list following the
v26.10.31 verify-README convention. It is deterministic replay — no
timing assertions, no sleeps, no polling needed; see the no-sleep testing
discipline for why timing asserts would be a category error here.

## 4. ML-DSA ACVP format (forward-looking, wave 3)

Same discipline; the JSON surface is verified from the mldsa-native ACVP
client's parser (field names quoted from that code):

Top level: `algorithm` = "ML-DSA"; `mode` in `keyGen` / `sigGen` /
`sigVer`; `revision` = "FIPS204" (plus "FIPS204-tr1" adding seed/expanded
key-format sigGen groups, same `v1.1.0.43` client gate).

Test-group flags (orthogonal, per group):

| Flag | Values | Selects |
|------|--------|---------|
| `testType` | "AFT" (keyGen, sigGen) | replay class |
| `parameterSet` | "ML-DSA-44" / "ML-DSA-65" / "ML-DSA-87" | parameter set |
| `preHash` | "preHash" | pre-hash signing; then `hashAlg` per test |
| `keyFormat` | "seed" / "expanded" | `seed` vs `sk` as key input |
| `deterministic` | boolean | deterministic vs hedged signing |
| `signatureInterface` | "external" / "internal" | message vs mu interface |
| `externalMu` | boolean | mu supplied by vector (64 B) |

Per-test fields: `tcId`, `seed` (32 B keyGen seed), `sk`, `message`
(not `msg`), `context` (<= 255 B), `hashAlg` (SHA2-224..SHA2-512/256,
SHA3-224..SHA3-512, SHAKE-128, SHAKE-256, or "none"), `rnd` (32 B hedged
randomness), `mu` (64 B, externalMu; the client asserts
`len(mu) == 128` hex), `pk`, `signature`. sigVer responses carry
`testPassed`.

Verified sizes (FIPS 204): seed 32 B; rnd 32 B; mu 64 B; context <= 255 B;
ML-DSA-44 pk 1312 B / sig 2420 B; ML-DSA-65 pk 1952 / sig 3309; ML-DSA-87
pk 2592 / sig 4627. (sk sizes omitted here; pin them from FIPS 204 Table 2
at wave-3 parse time.)

.rsp note for wave 3: if a .rsp corpus is used instead of JSON, the seed
line is 64 bytes = keygen seed (32 B) || signing randomness seed (32 B);
the exact split convention of the chosen corpus is UNVERIFIED-KAT until
its file SHA256 is pinned.

## 5. Deterministic vs randomized KAT semantics

ML-KEM (FIPS 203):

- KeyGen is deterministic by construction: ML-KEM.KeyGen_internal(d, z) —
  the (d, z) seeds are the entire entropy input. ACVP keyGen AFT vectors
  supply d and z, so keyGen replay is byte-exact by design; this is what
  makes a KAT court possible at all.
- Encaps takes one 32-byte random value `m` per vector (server-supplied in
  AFT). Given (ek, m), ct and k are deterministic.
- Decaps has the implicit-rejection branch: a corrupted ciphertext must
  produce the pseudorandom shared secret K = J(e || H(ek), z)... the
  precise formula is FIPS 203 Algorithm 18's implicit-rejection output;
  the court asserts the vector's ss bytes, never a local recomputation of
  the formula.
- "Randomized vs hedged" is an ML-DSA vocabulary item, not ML-KEM. FIPS
  203 has no hedged mode. The ACVP ML-KEM surface expresses randomness
  purely as server-supplied inputs (d, z, m), and the residual
  deterministic/randomized `setUpExpr` question is UNVERIFIED-KAT per
  section 1.1.
- The 64-byte question decomposes into two unrelated 64-byte facts; do
  not conflate them: (a) ML-KEM seed-format dk is 64 B (d||z) — a key
  serialization; (b) the FIPS-203-era .rsp seed line is 64 B (d||z) — the
  KeyGen entropy input. Neither is a "randomized mode".

ML-DSA (FIPS 204):

- Hedged signing is the FIPS 204 default; deterministic signing is an
  explicit variant. ACVP encodes this as the group's `deterministic`
  boolean (no literal "hedged" string exists on the wire).
- Byte-comparability rule for the court: a hedged group is byte-comparable
  only when the vector supplies the full randomness — `rnd` (external
  interface) or `mu` (externalMu). A hedged vector without vector-supplied
  randomness is REFUSED, never compared: comparing against a signature
  whose coins the court did not fully fix would test the vector's RNG,
  not avec's signer.
- Deterministic groups are byte-comparable unconditionally.

## 6. Open items

1. Pin the ACVP-Server release tag and per-file SHA256s; write the fixture
   provenance manifest format (sidecar TOML next to fixtures).
2. Confirm or kill `setUpExpr` ("deterministic"/"randomized") against the
   ACVTS demo server before wave-2 freezes its parser schema.
   UNVERIFIED-KAT today.
3. Decide fixture scope: all three ML-KEM parameter sets and all four
   encapDecap functions, or a subset for wave 2; tr1 in or out.
4. Evaluate the NIST pqc-forum negative/edge-case vector pack as the
   VAL-class corpus; confirm its file formats (UNVERIFIED-KAT).
5. Settle the .rsp question: if no official 64-byte-seed .rsp corpus can
   be pinned, JSON-only fixtures and a JSON-only parser.
6. Legal: record the redistribution posture receipt (section 2.2);
   confirm the GitHub-clone path carries no demo-server terms.
7. Wire the THROUGHPUT-VOID rule into ctq_report / wave receipts so the
   gate outranking is mechanical, not prose.
8. Wave 3 carry-over: pin ML-DSA sk sizes from FIPS 204 Table 2; pin the
   mldsa-native-derived .rsp seed split if that corpus is used.

## Receipts (sources consulted for this ticket)

- ACVP-Server shipped sample vectors (field names verified against the
  files): github.com/usnistgov/ACVP-Server, `gen-val/json-files/
  ML-KEM-keyGen-FIPS203/{prompt,expectedResults}.json` and
  `ML-KEM-encapDecap-FIPS203/prompt.json`
- ACVP ML-KEM JSON spec: pages.nist.gov/ACVP/draft-celi-acvp-ml-kem.html
- ACVP-Server README (isSample semantics, gen-val layout):
  github.com/usnistgov/ACVP-Server
- mlkem-native ACVP client (ML-KEM parsing, tr1 gate):
  github.com/pq-code-package/mlkem-native, `test/acvp/acvp_client.py`
- mldsa-native ACVP client (ML-DSA parsing, group flags):
  github.com/pq-code-package/mldsa-native, `test/acvp/acvp_client.py`
- Classic NIST-KAT .rsp generator:
  github.com/pq-crystals/dilithium, `ref/nistkat/PQCgenKAT_sign.c`
- NIST PQC archive (intermediate values ZIP, draft deviations):
  csrc.nist.gov/projects/post-quantum-cryptography/pqc-archive
- NIST pqc-forum negative/edge-case vectors announcement:
  groups.google.com/a/list.nist.gov/g/pqc-forum/c/aCAX-2QrUFw
