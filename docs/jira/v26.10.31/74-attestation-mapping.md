# AVEC v26.10.31 Attestation Mapping: Git OID ↔ BLAKE3 Receipt

> Dual-attestation map between git object names and AVEC content receipts.
> Settles the home of the OQ-06 binding (38-open-questions.md) — the open item
> adjacent to OQ-05 raised by 25-git-packfile-integration-spec.md §5. Status:
> mapping PINNED (src + spec cites on disk); each half of the binding is
> court-witnessed ALIVE; the combined per-chunk cross-check court is named in
> §4 and remains open.

## 1. The two identity laws

| Law | Name | Width | Owner | Failure class |
|---|---|---|---|---|
| Git object name | SHA-1 | 20 bytes | git wire/format (packfile v2, `.idx`) | Adversarial naming |
| AVEC receipt | BLAKE3 | 32 bytes | avec content layer (quads, pages) | Content tamper |

The two laws are not rivals; they govern different attacks. SHA-1 names decide
where an object sits in the git object graph; BLAKE3 digests decide whether the
bytes are the bytes that were admitted. Neither substitutes for the other.

## 2. The mapping

Git names every object `SHA-1("<type> <size>\0" + content)` — pinned in src at
`crates/avec-git/src/pack_write.rs:130-131` (`git_object_oid`) and restated
from the public pack-format spec in 25-git-packfile-integration-spec.md §1-2.

- **Blob OID** = `SHA-1("blob N\0" + payload)`, where `payload` is the inflated
  64-byte-aligned quad chunk. The payload's BLAKE3 receipt `R` is carried
  in-band in the 64-byte quad `inline_digest` column (`[u8; 32]` at offset
  0x20; 23-64b-quad-spec.md §1, `crates/avec-arrow/src/layout.rs:54,68` with
  compile-time `offset_of!` asserts) and is re-derivable from the inflated
  blob: inflate, locate the digest column, compare against the manifest entry.
- **Commit OID** = `SHA-1(commit-bytes)`. The commit message body is free, so
  the binding does not live there. The commit is a **transitive Merkle
  witness**: commit → tree → blob OIDs, each OID committing to its content,
  down to leaf blobs whose content embeds the BLAKE3 receipt `R`. Owning the
  commit OID therefore transitively fixes the BLAKE3-receipted content of
  every blob under the tree.

## 3. Home of the binding (settles OQ-06)

OQ-06 asked where the SHA-1↔BLAKE3 binding lives — commit message, descriptor
blob, or separate manifest blob — since a binding with no canonical location
cannot be third-party-replayed. Pinned:

- **In-band carrier:** the quad `inline_digest` column rides inside the blob
  payload itself. Any party that can read the blob can re-derive `R` without
  a sidecar.
- **Manifest blob form:** a schema/manifest blob referenced from the tree,
  keyed `BLAKE3 → OID` pairs, one row per chunk (the OQ-06 candidate path,
  adopted). The commit OID is the apex name for the whole key set.
- **The pack contains both.** A packfile v2 archive carries the blob bytes
  and (via the tree/manifest objects it ships) the manifest. That is the
  physical binding: one artifact, two identity laws.

## 4. Verification commands

The binding is executed, not asserted — a third party replays:

```sh
# 1. Re-derive the payload from the git name (wire law half).
git cat-file blob <blob-oid> > /tmp/chunk.bin

# 2. Re-derive the BLAKE3 receipt from the inflated payload (content law half).
b3sum /tmp/chunk.bin
#   compare against the manifest row for this chunk and the inline_digest
#   column of every 64-byte quad in the chunk.

# 3. Object-graph integrity (git side).
git fsck --full <repo>
git verify-pack -v <pack>.idx

# 4. Binding witnesses already green in this repo:
#    crates/avec-git/tests/fsck_court.rs (2/2):
#      fsck_court_accepts_avec_git_pack
#      negative_control_corrupted_git_built_pack_is_rejected
#    crates/avec-git/tests/git_interop.rs (3/3):
#      direction_a_git_idx_parsed_by_crate_reader
#      direction_a_fanout_total_matches_git_census
#      direction_b_git_accepts_avec_pack
#    receipt: receipts/gate-git-tests-v3.md (ALIVE, exit 0,
#    2026-10-31T05:33:29Z, HEAD ead40329a699878a91b070378066b5e50541ba33)
```

**Open court item:** a per-chunk cross-check court on a generated repo that
asserts, for every chunk, both `b3sum chunk == manifest blake3` AND
`git hash-object chunk == tree entry OID` simultaneously (25 §5's two lines as
one court, over all chunks). Neither existing court compares both identities in
one assertion; fsck_court certifies the git half via real `git fsck`, git_interop
certifies pack/idx agreement. Until that court exists, per-chunk dual identity
is a design claim, not a receipted one.

## 5. Trust argument

- **SHA-1 is the weak link for adversarial naming.** SHA-1 collision resistance
  is broken (practical chosen-prefix collisions); an adversary who can invest
  in a collision can try to mint two payloads under one blob OID. Git itself
  carries this exposure; avec inherits it on the naming half only.
- **BLAKE3 is the content-integrity law.** Every payload also carries its
  BLAKE3 receipt inline (`inline_digest`) and in the manifest. A SHA-1
  collision that smuggles different payload bytes still must present payload
  bytes whose BLAKE3 digest equals the recorded `R` in the manifest and in
  every quad row. The adversary must break both laws at once on the same
  artifact.
- **The pack binds the two.** The packfile v2 archive is the single artifact
  containing both names and contents; `git fsck`/`verify-pack` and the b3sum
  cross-check are both executed over it. Forging the pair means forging the
  pack that both courts read — the two identity laws are bound by the pack
  containing both.
- Residual: if BLAKE3 preimage resistance holds, the content law holds; the
  practical attack surface is limited to SHA-1 naming tricks that still need
  to satisfy the BLAKE3 manifest, which changes the payload and therefore the
  manifest row — the collision must be constructed under both laws.

## 6. Open hardening item

**SHA-256 object-format migration** (git `extensions.objectFormat = sha256`):
removes the SHA-1 naming weakness entirely by making the wire law SHA-256.
Out of scope for v26.10.31 — OQ-06's own boundary note: sha256 object-format
extension is out of scope until the binding lands. Landing order: (1) the
per-chunk cross-check court of §4, (2) object-format negotiation in
25 §2's write path, (3) re-run fsck/interop courts under the sha256 object
format. This document is the migration's baseline: every mapping statement
here survives object-format migration unchanged except the digest function
in §2's name law (SHA-1 → SHA-256, 20 → 32 bytes).

## Standing

- Mapping: PINNED — design law with src/spec cites on disk (this document).
- Git-name half: ALIVE — receipts/gate-git-tests-v3.md (fsck 2/2, git_interop
  3/3, exit 0, HEAD ead4032).
- BLAKE3 content half: ALIVE for the digest engine
  (receipts/gate-blake3-tests-v3.md); the inline_digest re-derivation from an
  inflated git blob is UNVERIFIED until the §4 per-chunk cross-check court runs.
- OQ-06 home: SETTLED by §3 (in-band quad column + manifest blob referenced
  from the tree; commit OID as apex witness). The OQ register still reads
  UNKNOWN until its owner flips it from this document.

## See Also

- 25-git-packfile-integration-spec.md — §2 blob placement, §5 interop checks
- 23-64b-quad-spec.md — inline_digest column law
- 26-blake3-engine-spec.md — BLAKE3 engine and receipt form
- 38-open-questions.md — OQ-06 (the question this document settles)
