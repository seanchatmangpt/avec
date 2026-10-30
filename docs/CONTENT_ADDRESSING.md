# CONTENT_ADDRESSING

Content-addressing surface of the avec substrate: the 64-bit
mutation-detection fold exported through the FFI membrane, what it claims,
what it refuses to claim, and the tree-hash upgrade path. Lane HWL6,
2026-10-29.

## The fold (ALIVE)

Canon: `crates/avec-arrow/src/digest.rs` (`digest_of`, lane PCL5). The FFI
export `avec_digest_of` (`crates/avec-ffi/src/lib.rs`) re-folds the raw
ArrowArray buffers with a byte-for-byte identical fold; cross-membrane
parity is test-pinned, not assumed
(`digest_ffi::ffi_fold_parity_matches_digest_of` covers empty, odd tail
lane, single- and multi-word validity, all-valid, and a 64-row boundary).

    acc = 0
    for each pair of value lanes (v[i], v[i+1]):   # i even
        acc = (acc << 1 | acc >> 63) XOR (v[i] as u32 | v[i+1] as u32 << 32)
    if odd tail lane exists: acc = rotl1(acc) XOR tail_lane_as_u32
    for each 8-byte validity word w (zero-filled tail):   # only if the
        acc = rotl1(acc) XOR w                            # buffer exists

Every folded word contributes one rotate + one xor, so any single-bit flip
that reaches the folded words changes the digest. All-valid batches carry
no validity buffer (Arrow law) and fold none; builder padding bits never
reach the fold (tail word reads only `ceil(length/8)` bytes).

## Claims (exactly these)

- Detects ACCIDENTAL mutation: transport bit rot, buffer aliasing, a stale
  snapshot replayed as fresh. Witnessed end to end: a single value-byte
  flip through a raw pointer on the live export moves the digest; restoring
  the byte restores the digest (corruption courts on both sides of the
  membrane, receipts below).
- Order-sensitive and content-sensitive: different batch content with the
  same length folds differently; re-exporting identical content folds
  identically (a deterministic function of the physical buffers, not of
  the address).
- Validity is load-bearing: a validity-state change with identical value
  bytes changes the digest.
- Cheap: one rotate + one xor per word — 2 ops per 8 bytes (values) or per
  64 rows (validity), branch-free, no allocation, no dependencies.

## Not claims (refused by design)

- NOT a cryptographic hash: rotate-xor, no avalanche stage, no keying, no
  collision resistance. An adversary can construct collisions trivially.
  It will lose to an adversary; it is not supposed to win.
- NOT a content identity for cross-representation dedup: the digest is a
  function of the PHYSICAL buffers — offset, null encoding, and layout
  choices are folded. Two arrays that are logically equal but differently
  laid out fold differently. (This representation sensitivity is wanted
  for mutation detection and wrong for canonical identity.)
- NOT a Merkle proof: a 64-bit fold cannot prove anything about a slice;
  it attests the whole buffer state at digest time, all-or-nothing.
- NOT an integrity seal against a malicious writer: whoever can write the
  buffer can re-fold and re-stamp a matching digest.

## Tree-hash upgrade path (cited-context sketch, NOT BUILT)

Ordered so each step is independently landable and the 64-bit fold stays
useful as a prefilter at every step:

1. Widen the membrane: export `avec_digest_of_wide(array_addr, out* ->
   [u64; 4])` alongside the u64 fold. Same buffers, wider accumulator.
2. Swap the fold for a keyed or finalized mixer (e.g. multiply-xor high
   bits at the end) — still zero-dependency, still non-cryptographic but
   avalanche-improved.
3. Content identity: BLAKE3 over a domain-separated preimage
   (schema-format bytes, length, null_count, then each buffer), producing
   a canonical content hash. This is where representation normalization
   (offset stripping, canonical null encoding) must be decided first —
   the hash is only as canonical as the normalization.
4. Merkle slice proofs: hash each buffer in fixed chunks (e.g. 1 KiB,
   BLAKE3's native chunk size) into a binary tree; a row-range proof is
   the sibling path from the row's chunks to the root. The validity tree
   binds into the same root, so a proof covers nullability too. Wire form:
   root (32 B) + sibling hashes (~log2(bytes/1 KiB) x 32 B).

Steps 1-2 reuse the existing membrane; step 3 changes the meaning (digest
to identity) and needs the representation-normalization law pinned before
landing; step 4 adds the proof surface.

## Cited-context notes

- BLAKE3 ~84 GB/s SIMD throughput is CITED-CONTEXT from the BLAKE3 paper
  (O'Connor, Aumasson, Neves, Wilcox-O'Hearn, 2020) and the blake3 crate
  README — not measured in this repo; avec currently pulls zero external
  crates and the zero-dependency law holds (`crates/avec-arrow/src/
  lib.rs`: `no_std` + `alloc` only).
- Arrow C Data Interface struct/layout claims (ArrowArray/ArrowSchema
  fields, release contract) are per the Arrow spec as mirrored in
  `crates/avec-arrow/src/c_abi.rs`.
- Null slots are stored ZERO at build time (`Scalar::ZERO` law,
  `crates/avec-arrow/src/primitive.rs`), so the digest is a deterministic
  function of the physical buffers.

## Verification receipt (2026-10-29, lane HWL6)

- `cargo test -p avec-ffi` — exit 0, 4 passed
  (`digest_ffi::corruption_court_value_byte_flip_is_detected_and_restored`,
  `digest_ffi::ffi_fold_parity_matches_digest_of`,
  `digest_ffi::empty_folds_to_zero_and_validity_is_load_bearing`,
  `version_cstr_parity::c_form_matches_version`).
- `cargo build -p avec-ffi` — exit 0; `nm -gU target/debug/
  libavec_ffi.dylib | grep avec_digest` -> `_avec_digest_of` exported (T).
- `python3 -m pytest bindings/python` — exit 0, 16 passed, 1 skipped (the
  skip is the pre-existing test_pages.py page-pipeline gap, unrelated).
- 5 new python tests in `bindings/python/avec/tests/test_digest.py`:
  version law, stability across two calls, different-batch change,
  corruption court (flip + restore), empty-folds-to-zero.

## See Also

- `docs/RECEIPTS.md`
- `docs/BRANCH_REGISTER.md`
