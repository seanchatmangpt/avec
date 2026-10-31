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

## Tree-hash upgrade path (ALIVE via `avec-blake3`)

Ordered so each step is independently landable and the 64-bit fold stays
useful as a prefilter at every step:

1. Widen the membrane: export `avec_digest_of_wide(array_addr, out* ->
   [u64; 4])` alongside the u64 fold. Same buffers, wider accumulator, or use
   the in-register $GF(2^{128})$ polynomial accumulator (`crates/avec-arrow/src/fingerprint.rs`,
   4.08 GiB/s line rate, 0 extra memory passes).
2. Swap the fold for a keyed or finalized mixer (e.g. multiply-xor high
   bits at the end) — still zero-dependency, still non-cryptographic but
   avalanche-improved.
3. Content identity (ALIVE): Sovereign, standalone `#![no_std]` BLAKE3
   (`crates/avec-blake3` and `crates/avec-arrow/src/blake3.rs`, zero external
   crates, zero heap allocations) over a contiguous byte slice or domain-separated
   preimage (`avec_blake3::hash` / `avec_blake3::hash_wide_simd`).
4. Merkle slice proofs & SIMD parent reduction (ALIVE): `avec-blake3` hashes
   each 1 KiB chunk across up to 16 sequential 64-byte blocks (`hash_chunk`)
   and folds binary parent subtrees on a 54-entry stack (`[[u32; 8]; 54]`) or
   via 4-way ARM64 NEON (`neon_parent::compress_parents_4way_neon`) and 8-way
   x86_64 AVX2 (`avx2_parent::compress_parents_8way_avx2`) horizontal level
   reduction (`reduce_level_simd`, 1.31 GiB/s / 21.9 Mparents/s on Apple Silicon).
5. CPU SIMD vs. Unified Memory GPU Crossover (`GPU_CROSSOVER_BYTES = 16 MiB`):
   Payloads $< 16\text{ MiB}$ stay on sub-100ns L1/L2/L3 CPU SIMD (`should_dispatch_gpu(len) == false`);
   massive batches $\ge 16\text{ MiB}$ (16,384+ independent 1 KiB chunks) can wrap
   zero-copy buffers via `crates/avec-gpu` (`MetalSharedBuffer`, `MTLResourceStorageModeShared`).

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
