//! Content-addressed Merkle DAG over Parquet snapshot blocks (lane A03).
//!
//! Wave-2/5 status: the frozen Wave-1 ABI now carries its Wave-2 bodies —
//! in-crate BLAKE3 (regular mode, ADR-03-3), `commit_blocks`, `merkle_root`,
//! [`LineageProof`] + [`verify_lineage`], the §4.2 [`FrameFields`] contract,
//! and the §5.1/§5.2 wire codecs. Courts D-1..D-10 of
//! `docs/jira/v26.11.1/lanes/A03-git-merkle.md` §6 run in this file; the
//! code-level anti-vacuity mutations run as shim courts outside the file
//! (BASELINE -> SHIM -> AFTER -> sha256-verified byte-exact RESTORE), with
//! receipts in `docs/jira/v26.11.1/waves/a03-wave5-audit-receipt.md`
//! (wave-6 addition: F-1 fix + hostile-count court, receipts in this
//! file's test doc comments and the lane's wave-6 report).
//!
//! Two registers (ADR-03-1/03-2): git-compat SHA-1 OIDs for anything that
//! touches packfile bytes; BLAKE3 32-byte digests for the cgap-internal
//! receipt register. The two never mix: node payloads bind SHA-1 OIDs
//! *inside* BLAKE3-hashed bytes, never the reverse.
//!
//! Node payload layout (§3.1, 128 bytes fixed):
//!
//! ```text
//! offset  size  field
//! 0       32    left_child_digest  (32 x 0x00 if leaf)
//! 32      32    right_child_digest (32 x 0x00 if leaf)
//! 64      20    block_git_oid      (SHA-1 of the Parquet block as a git blob)
//! 84      20    parent_git_oid     (parent snapshot head; 20 x 0x00 = genesis)
//! 104     8     block_len          (u64 BE, uncompressed Parquet block length)
//! 112     8     seq                (u64 BE, ordering index, 0-based)
//! 120     4     depth              (u32 BE, leaf = 0)
//! 124     4     flags              (u32 BE: bit0 = genesis, bit1 = has_parent)
//! ```
//!
//! # Recorded divergences from the lane doc (doc yields to reality)
//!
//! 1. **Internal node payload.** §3.2.3 hashes `left || right || seq_parent
//!    || depth`, but the frozen §5.2 proof wire pins a sibling entry at 33
//!    bytes (side u8 + 32-byte digest) and the frozen static assert pins
//!    the 618-byte proof at L = 2^16 — per-level seq/depth fields cannot
//!    fit the frozen wire, and §3.3's fold (`H(h||sib)` / `H(sib||h)`)
//!    binds only child digests. Internal payload is therefore
//!    `left_digest || right_digest` (64 bytes); `depth` stays in the node
//!    table and the path length is a function of L, so determinism and
//!    O(1) verification are unaffected.
//! 2. **Node table order.** §5.1 labels the `.cgap-dag` node table
//!    "preorder", but a promoted tree has mixed-depth children (a root of
//!    depth 3 over children of depths 2 and 0), so no single-pass preorder
//!    validator can decide leaf-vs-internal locally. The table is written
//!    in **postorder**, which reduces to the root with a two-element stack
//!    while re-verifying every internal digest. Count law (2L-1) and all
//!    field sizes are unchanged.
//! 3. **Leaf ordering.** §3.2 orders leaves by `seq` (matches D-2 and the
//!    path-bit derivation); ADR-03-4's "ordered by block OID bytes" phrase
//!    is the anomaly and is not implemented — seq order is what D-2 courts.
//!
//! # Wave-9 receipt (cross-lane integration, 2026-11-01, failed-lane restart)
//!
//! The wave-9 slice ran the fleet's cross-lane integration suites against
//! this lane's surface; every suite exited 0 this session:
//!
//! - A04 identity court: `cargo test -p cgap-core --test identity_court`
//!   -> exit 0, 4 passed (wire+memory bit-identity over 8 payload sizes,
//!   re-derived digest == embedded digest via real cgap-blake3).
//! - A14 dam over A04 frames: `cargo test -p cgap-gate` -> exit 0,
//!   11 passed (penstock FrameView over real CgapFrameHeader bytes).
//! - A20 1M-row stress: `CARGO_PROFILE_DEV_OPT_LEVEL=2
//!   CARGO_TARGET_DIR=$PWD/target-a20 cargo test -p cgap-court --test
//!   court_verdict` -> exit 0, 11 passed; `tests/court_verdict.rs` sha256
//!   `cab5c2f5…6b86` byte-identical pre/post (no concurrent-mutant
//!   interference in this run's window).
//! - A05 power-cut sweep: `cargo test -p cgap-storage` -> exit 0, 21
//!   passed across 6 binaries (`court_sweep` executes real cuts at every
//!   byte offset of a commit transaction). Cross-lane A03 consumption
//!   witnessed: `court_crc_identity` pins `cgap_git::pack_write::crc32`
//!   byte-for-byte; `court_faults` enforces the A03 dag-seq law
//!   (`DAG_SEQ <= base refuses`, strict).
//! - A07 zero-string gate: `python3 scripts/zero_string_gate.py` ->
//!   exit 0; scanned=45 artifacts over crates=22, hits=0,
//!   emitter_hits=2 (registered emitter-exempt class, loud),
//!   skipped_artifacts=0; cgap-git scanned=2 artifacts, 0 hits.
//!
//! New in this file (wave 9):
//! [`wave9_powercut_sweep_dag_wire`] — the A05 cut-at-every-offset
//! discipline transplanted onto the §5.1 dag wire; every proper prefix
//! of a real 416-byte L=5 file refuses typed `Truncated`, Ok anchor at
//! full length, four-layer defense-in-depth disclosed.
//!
//! Wave-9 divergences (doc yields; recorded, not lane-fixed):
//!
//! 1. The §4.2-anticipated A04 header slots (root / parent_git_oid /
//!    block_count / pack_sha1) do not exist in the landed A04 header
//!    (cgap-core/src/frame.rs: magic CGAP / version / flags bit3 GitPack /
//!    reserved / payload_byte_len / digest(32) / attestation(80) = 128 B).
//!    [`FrameFields`] remains the A03-side supply contract (d5); there is
//!    no A04-side consumer yet, so the A03-root-over-frames seam is
//!    supplied but not exercised end-to-end cross-crate. Coordinator
//!    item: A04 embedding of FrameFields or an explicit ADR retiring the
//!    slot expectation.
//! 2. The lane doc §10 still reads "Status: UNVERIFIED — no code landed
//!    this session"; stale since wave 2 (a03-wave5-audit-receipt.md
//!    divergence 1). Coordinator item, unchanged this wave.
//! 3. Two baseline suites ran against the shared `target/` cache before
//!    the lane switched to its `target-a03` lease (disclosed in the
//!    wave-9 report); cache-hit runs, no source writes.

#![forbid(unsafe_code)]

use core::fmt;
use core::mem::offset_of;

use alloc::{boxed::Box, vec, vec::Vec};

use crate::Oid;
use crate::pack_write::{git_object_oid, sha1};

// ---------------------------------------------------------------------------
// Digest + OID registers
// ---------------------------------------------------------------------------

/// BLAKE3 digest width in bytes (receipt register, ADR-03-1).
pub const BLAKE3_DIGEST_LEN: usize = 32;

/// A 32-byte BLAKE3 digest (receipt register). Never a git OID; the git
/// register is [`Oid`] (SHA-1, 20 bytes).
pub type DagDigest = [u8; BLAKE3_DIGEST_LEN];

/// Genesis parent OID marker: 20 zero bytes ([`Oid`]).
pub const GENESIS_OID: Oid = [0u8; 20];

/// Zeroed child-digest sentinel: leaves carry `32 x 0x00` in both child
/// slots of [`DagNodePayload`].
pub const ZERO_DIGEST: DagDigest = [0u8; BLAKE3_DIGEST_LEN];

// ---------------------------------------------------------------------------
// Node payload consts (§3.1)
// ---------------------------------------------------------------------------

/// Fixed node payload size: 128 bytes (§3.1).
pub const DAG_NODE_SIZE: usize = 128;

/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_LEFT_CHILD: usize = 0;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_RIGHT_CHILD: usize = 32;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_BLOCK_OID: usize = 64;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_PARENT_OID: usize = 84;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_BLOCK_LEN: usize = 104;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_SEQ: usize = 112;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_DEPTH: usize = 120;
/// Field offsets within the 128-byte node payload (§3.1).
pub const DAG_OFF_FLAGS: usize = 124;

/// `flags` bit 0: snapshot is genesis (no parent snapshot).
pub const DAG_FLAG_GENESIS: u32 = 1 << 0;
/// `flags` bit 1: node carries a non-genesis `parent_git_oid`.
pub const DAG_FLAG_HAS_PARENT: u32 = 1 << 1;

// ---------------------------------------------------------------------------
// `.cgap-dag` file consts (§5.1)
// ---------------------------------------------------------------------------

/// `.cgap-dag` file magic: `"CGAPDAG\0"`.
pub const DAG_MAGIC: [u8; 8] = *b"CGAPDAG\0";

/// `.cgap-dag` format version (u32 BE field value).
pub const DAG_FORMAT_VERSION: u32 = 1;

/// Fixed `.cgap-dag` header size: 72 bytes (§5.1).
pub const DAG_HEADER_SIZE: usize = 72;

/// Per-node `.cgap-dag` entry size: depth u32 + 32-byte digest.
pub const DAG_ENTRY_SIZE: usize = 36;

/// `.cgap-dag` trailer size: git-style SHA-1 over everything above it.
pub const DAG_TRAILER_SIZE: usize = 20;

/// Fixed (header + trailer) `.cgap-dag` size; nodes add
/// `node_count x [`DAG_ENTRY_SIZE`]`.
pub const DAG_FILE_FIXED_SIZE: usize = DAG_HEADER_SIZE + DAG_TRAILER_SIZE;

// ---------------------------------------------------------------------------
// Proof wire consts (§5.2)
// ---------------------------------------------------------------------------

/// Proof wire fixed prefix: block_oid(20) + parent_oid(20) + seq(8) +
/// block_len(8) + sib_count u16 BE.
pub const PROOF_WIRE_FIXED: usize = 58;

/// One sibling entry on the proof wire: side u8 + 32-byte digest.
pub const PROOF_WIRE_SIBLING: usize = 33;

/// Proof wire root field size.
pub const PROOF_WIRE_ROOT: usize = 32;

/// Sibling-path side byte: `h = BLAKE3(sib || h)` (the sibling is the
/// LEFT child).
pub const PROOF_SIDE_LEFT: u8 = 0;
/// Sibling-path side byte: `h = BLAKE3(h || sib)` (the sibling is the
/// RIGHT child).
pub const PROOF_SIDE_RIGHT: u8 = 1;

/// Maximum sibling-path levels: `ceil(log2 L)` with `L <= MAX_BLOCKS`.
pub const MAX_PROOF_LEVELS: usize = 16;

/// Maximum BLAKE3 invocations per single-proof lineage verification: 1 leaf
/// reconstruction + [`MAX_PROOF_LEVELS`] internal nodes (DoD-4 budget;
/// per-proof cost is independent of snapshot size N).
pub const MAX_PROOF_HASHES: usize = MAX_PROOF_LEVELS + 1;

// ---------------------------------------------------------------------------
// Internal payload size (§3.2; divergence note 1)
// ---------------------------------------------------------------------------

/// Internal node payload: `left_digest || right_digest` (§3.2 as bound by
/// the frozen §5.2 wire; see the module divergence note 1).
pub const INTERNAL_PAYLOAD_SIZE: usize = 64;

// ---------------------------------------------------------------------------
// Cap single-source (R-4)
// ---------------------------------------------------------------------------

/// Maximum blocks per committed snapshot. Single source of truth is the
/// 16-stage ladder window (`crate::ladder::LADDER_WINDOW`); the pack
/// writer cap (`crate::pack_write::MAX_OBJECTS`) is cross-pinned to the
/// same number by a static assert below so the three can never drift.
pub const MAX_BLOCKS: usize = crate::ladder::LADDER_WINDOW as usize;

// ---------------------------------------------------------------------------
// repr(C) wire structs — size/align/offset frozen by static asserts
// ---------------------------------------------------------------------------

/// The 128-byte node payload that gets BLAKE3-hashed (§3.1).
///
/// Field order is the wire order; integer fields serialize big-endian.
/// `repr(C)` + static asserts guarantee no hidden padding and pin every
/// offset to the table above.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DagNodePayload {
    /// Left child digest; [`ZERO_DIGEST`] for leaves.
    pub left_child: DagDigest,
    /// Right child digest; [`ZERO_DIGEST`] for leaves.
    pub right_child: DagDigest,
    /// SHA-1 of the Parquet block as a git blob (git-compat register).
    pub block_oid: Oid,
    /// Parent snapshot head OID; [`GENESIS_OID`] for genesis.
    pub parent_oid: Oid,
    /// Uncompressed Parquet block byte length.
    pub block_len: u64,
    /// Ordering index within the snapshot, 0-based.
    pub seq: u64,
    /// Tree depth: leaf = 0.
    pub depth: u32,
    /// [`DAG_FLAG_GENESIS`] / [`DAG_FLAG_HAS_PARENT`] bits.
    pub flags: u32,
}

/// One node entry of the `.cgap-dag` node table (§5.1, postorder —
/// divergence note 2).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DagTreeNode {
    /// Depth of the subtree this node covers (leaf = 0).
    pub depth: u32,
    /// BLAKE3 digest of this node's payload.
    pub digest: DagDigest,
}

/// Fixed prefix of the `.cgap-dag` file (§5.1), followed by
/// `node_count x [`DagTreeNode`]` and a 20-byte SHA-1 trailer.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DagHeader {
    /// [`DAG_MAGIC`] (`"CGAPDAG\0"`).
    pub magic: [u8; 8],
    /// [`DAG_FORMAT_VERSION`] (u32 BE on the wire).
    pub version: u32,
    /// BLAKE3 Merkle root: the A04 frame commitment.
    pub root: DagDigest,
    /// Parent snapshot git OID as given to `commit_blocks`.
    pub parent_oid: Oid,
    /// Leaf count L.
    pub block_count: u32,
    /// Total tree node count (2L-1 with odd-leaf promotion).
    pub node_count: u32,
}

// ---------------------------------------------------------------------------
// Value types (heap-holding; ABI frozen by field types, not repr(C))
// ---------------------------------------------------------------------------

/// Lineage proof for one leaf (§3.3): [`verify_lineage`] checks the claimed
/// parent first (§4.3, D-3/D-8), then folds the sibling path to the root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageProof {
    /// The claim: this block's git blob OID.
    pub block_git_oid: Oid,
    /// The claimed parent snapshot head OID.
    pub parent_git_oid: Oid,
    /// Ordering index of the leaf (drives the deterministic path bits).
    pub seq: u64,
    /// Uncompressed Parquet block byte length.
    pub block_len: u64,
    /// Sibling path bottom-up: `(side, digest)` per level; `side` is
    /// [`PROOF_SIDE_LEFT`] / [`PROOF_SIDE_RIGHT`].
    pub siblings: Vec<(u8, DagDigest)>,
    /// The committed root this proof verifies against.
    pub root: DagDigest,
}

/// The committed DAG state (mirrors the `.cgap-dag` file body, §5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagTree {
    /// BLAKE3 Merkle root (equals `Commit::root`; asserted at construction).
    pub root: DagDigest,
    /// Parent snapshot git OID (as given to `commit_blocks`).
    pub parent_oid: Oid,
    /// Leaf count L.
    pub block_count: u32,
    /// Total node count (2L-1 with odd-leaf promotion).
    pub node_count: u32,
    /// Postorder node table (divergence note 2).
    pub nodes: Vec<DagTreeNode>,
}

/// Output of `commit_blocks` (§4.1): the A04 frame commitment fields plus
/// the committed tree and the per-block git OIDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// BLAKE3 Merkle root shipped in the A04 frame header.
    pub root: DagDigest,
    /// Per-block git blob OIDs, input order.
    pub oids: Vec<Oid>,
    /// The committed DAG.
    pub tree: DagTree,
}

/// A04 frame-header commitment fields (§4.2), all derived by A03; A04 must
/// not recompute them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameFields {
    /// BLAKE3 Merkle root (receipt register).
    pub root: DagDigest,
    /// Parent snapshot head (git-compat register).
    pub parent_git_oid: Oid,
    /// Leaf count L.
    pub block_count: u32,
    /// SHA-1 over the pack bytes (transport checksum, distinct from content
    /// OIDs).
    pub pack_sha1: Oid,
}

impl Commit {
    /// Derive the §4.2 frame fields from a commit plus the SHA-1 over the
    /// pack bytes (`pack_write::write_pack_v2` output).
    pub fn frame_fields(&self, pack_sha1: Oid) -> FrameFields {
        FrameFields {
            root: self.root,
            parent_git_oid: self.tree.parent_oid,
            block_count: self.tree.block_count,
            pack_sha1,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors (dag-local; `DagError` is the Wave-1/2 spelling until the variants
// are lifted into `crate::GitError` at coordinator wire-in — lib.rs is off
// limits to this lane)
// ---------------------------------------------------------------------------

/// Typed DAG outcomes for §4.1 (commit), §4.2 (frames), §4.3 (A05 rollback)
/// and §5.1 (wire parse). Variant names mirror the lane doc's `GitError::`
/// names where the doc names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagError {
    /// Block count exceeds [`MAX_BLOCKS`] (ladder cap, shared with the
    /// pack writer).
    TooManyObjects(usize),
    /// Two blocks claim the same `seq` (§4.1).
    DuplicateSeq(u64),
    /// A zero-length block body was refused (§4.1).
    EmptyBlock(u64),
    /// `commit_blocks` on a zero-block snapshot: no root exists (§4.1 has
    /// no empty-commit semantics).
    EmptyCommit,
    /// A `lineage_proof` seq is not in the committed block set.
    UnknownSeq(u64),
    /// Proof's `parent_git_oid` does not match the expected parent
    /// (§4.3 rollback target). Checked before the root fold (D-3/D-8).
    LineageMismatch,
    /// Sibling path does not fold to the committed root, the proof's root
    /// differs from the committed root, or the path exceeds
    /// [`MAX_PROOF_LEVELS`] (§4.3).
    RootMismatch,
    /// `.cgap-dag` SHA-1 trailer mismatch, or a decoded header root
    /// disagrees with the tree recomputed from the node table (§5.1).
    ChecksumMismatch,
    /// `.cgap-dag` magic is not [`DAG_MAGIC`].
    BadMagic,
    /// `.cgap-dag` version is not [`DAG_FORMAT_VERSION`].
    VersionMismatch(u32),
    /// `.cgap-dag` bytes end before the fixed header+trailer, or the node
    /// region length disagrees with `node_count`.
    Truncated,
    /// `.cgap-dag` node table does not reduce to a single root under the
    /// 2L-1 law (bad `node_count` vs `block_count`, or a corrupted
    /// depth/digest sequence).
    ShapeMismatch,
}

impl fmt::Display for DagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DagError::TooManyObjects(n) => write!(
                f,
                "too many blocks for one DAG snapshot: {n} exceeds the ladder-window cap of \
                 65536 (MAX_BLOCKS) — split the snapshot across multiple commits"
            ),
            DagError::DuplicateSeq(seq) => write!(
                f,
                "duplicate block seq {seq}: every block in a snapshot needs a distinct \
                 ordering index — renumber the blocks and recommit"
            ),
            DagError::EmptyBlock(seq) => write!(
                f,
                "block at seq {seq} has an empty body: zero-byte Parquet blocks cannot be \
                 content-addressed — drop the block or fix the producer"
            ),
            DagError::EmptyCommit => write!(
                f,
                "refusing to commit a zero-block snapshot: a DAG root over an empty leaf \
                 set does not exist — a snapshot must carry at least one block"
            ),
            DagError::UnknownSeq(seq) => write!(
                f,
                "no block with seq {seq} in the committed set: a lineage proof needs the \
                 seq of a block that was committed into this snapshot"
            ),
            DagError::LineageMismatch => write!(
                f,
                "lineage mismatch: the proof's parent_git_oid does not match the expected \
                 parent (rollback target) — the block does not descend from that parent"
            ),
            DagError::RootMismatch => write!(
                f,
                "root mismatch: the sibling path does not fold to the committed root — the \
                 proof or the root was altered after commit"
            ),
            DagError::ChecksumMismatch => write!(
                f,
                "dag file checksum mismatch: the SHA-1 trailer (or the decoded root vs the \
                 recomputed tree) does not match the bytes — the file was altered or \
                 truncated after write"
            ),
            DagError::BadMagic => write!(
                f,
                "dag file magic is not CGAPDAG\\0: this is not a `.cgap-dag` artifact"
            ),
            DagError::VersionMismatch(v) => write!(
                f,
                "dag file version {v} is not supported: this reader only accepts format \
                 version 1 (DAG_FORMAT_VERSION)"
            ),
            DagError::Truncated => write!(
                f,
                "dag file is truncated: bytes end before the fixed 72-byte header + 20-byte \
                 trailer, or the node region does not match node_count"
            ),
            DagError::ShapeMismatch => write!(
                f,
                "dag file node table violates the 2L-1 law: the (depth, digest) sequence \
                 does not reduce to a single root — refuse rather than guess"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Static asserts: the ABI freeze. Any drift in size, alignment, field
// offset, cap, or wire constant fails the build (court ABI-1; anti-vacuity
// mutation: change any field type/order or constant — the build must fail;
// witnessed by shim court SHIM-ABI; receipt in
// docs/jira/v26.11.1/waves/a03-wave5-audit-receipt.md).
// ---------------------------------------------------------------------------

// -- DagNodePayload: 128 bytes, offsets per §3.1 --
const _: () = assert!(size_of::<DagNodePayload>() == DAG_NODE_SIZE);
const _: () = assert!(align_of::<DagNodePayload>() == 8);
const _: () = assert!(offset_of!(DagNodePayload, left_child) == DAG_OFF_LEFT_CHILD);
const _: () = assert!(offset_of!(DagNodePayload, right_child) == DAG_OFF_RIGHT_CHILD);
const _: () = assert!(offset_of!(DagNodePayload, block_oid) == DAG_OFF_BLOCK_OID);
const _: () = assert!(offset_of!(DagNodePayload, parent_oid) == DAG_OFF_PARENT_OID);
const _: () = assert!(offset_of!(DagNodePayload, block_len) == DAG_OFF_BLOCK_LEN);
const _: () = assert!(offset_of!(DagNodePayload, seq) == DAG_OFF_SEQ);
const _: () = assert!(offset_of!(DagNodePayload, depth) == DAG_OFF_DEPTH);
const _: () = assert!(offset_of!(DagNodePayload, flags) == DAG_OFF_FLAGS);

// -- DagTreeNode: 36 bytes, no padding --
const _: () = assert!(size_of::<DagTreeNode>() == DAG_ENTRY_SIZE);
const _: () = assert!(align_of::<DagTreeNode>() == 4);
const _: () = assert!(offset_of!(DagTreeNode, depth) == 0);
const _: () = assert!(offset_of!(DagTreeNode, digest) == 4);

// -- DagHeader: 72 bytes, offsets per §5.1 --
const _: () = assert!(size_of::<DagHeader>() == DAG_HEADER_SIZE);
const _: () = assert!(align_of::<DagHeader>() == 4);
const _: () = assert!(offset_of!(DagHeader, magic) == 0);
const _: () = assert!(offset_of!(DagHeader, version) == 8);
const _: () = assert!(offset_of!(DagHeader, root) == 12);
const _: () = assert!(offset_of!(DagHeader, parent_oid) == 44);
const _: () = assert!(offset_of!(DagHeader, block_count) == 64);
const _: () = assert!(offset_of!(DagHeader, node_count) == 68);

// -- register widths --
const _: () = assert!(BLAKE3_DIGEST_LEN == 32);
const _: () = assert!(size_of::<DagDigest>() == 32);
const _: () = assert!(size_of::<Oid>() == 20);

// -- cap single-source (R-4): ladder window == pack writer cap == 2^16 --
const _: () = assert!(MAX_BLOCKS == 1 << 16);
const _: () = assert!(MAX_BLOCKS == crate::pack_write::MAX_OBJECTS);
const _: () = assert!(MAX_BLOCKS == 65_536);

// -- proof budget (DoD-4): 16 levels, 17 hashes, 618-byte wire at L=2^16 --
const _: () = assert!(MAX_PROOF_LEVELS == 16);
const _: () = assert!((1usize << MAX_PROOF_LEVELS) == MAX_BLOCKS);
const _: () = assert!(MAX_PROOF_HASHES == 17);
const _: () = assert!(PROOF_WIRE_FIXED + MAX_PROOF_LEVELS * PROOF_WIRE_SIBLING
    + PROOF_WIRE_ROOT
    == 618);

// -- internal payload (divergence note 1) --
const _: () = assert!(INTERNAL_PAYLOAD_SIZE == BLAKE3_DIGEST_LEN * 2);

// -- wire pins --
// elementwise: const-context array `==` (PartialEq) is not const-stable
const _: () = assert!(
    DAG_MAGIC[0] == b'C'
        && DAG_MAGIC[1] == b'G'
        && DAG_MAGIC[2] == b'A'
        && DAG_MAGIC[3] == b'P'
        && DAG_MAGIC[4] == b'D'
        && DAG_MAGIC[5] == b'A'
        && DAG_MAGIC[6] == b'G'
        && DAG_MAGIC[7] == 0
);
const _: () = assert!(DAG_FORMAT_VERSION == 1);
const _: () = assert!(DAG_FILE_FIXED_SIZE == 92);
const _: () = assert!(DAG_FLAG_GENESIS == 1);
const _: () = assert!(DAG_FLAG_HAS_PARENT == 2);
const _: () = assert!(PROOF_SIDE_LEFT == 0);
const _: () = assert!(PROOF_SIDE_RIGHT == 1);

// ---------------------------------------------------------------------------
// In-crate BLAKE3 (regular mode) — receipt-register hash (ADR-03-2/03-3).
// Structure follows the official reference implementation: 64-byte blocks,
// 1024-byte chunks, 7-round compression, chunk stack with odd-promotion.
// KATs: court D-6 (official vectors, see tests).
// ---------------------------------------------------------------------------

const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

const B3_BLOCK_LEN: usize = 64;
const B3_CHUNK_LEN: usize = 1024;

const CHUNK_START: u32 = 1 << 0;
const CHUNK_END: u32 = 1 << 1;
const PARENT: u32 = 1 << 2;
const ROOT: u32 = 1 << 3;

fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    // Mix the columns.
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    // Mix the diagonals.
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn compress(
    chaining_value: [u32; 8],
    block_words: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state: [u32; 16] = [
        chaining_value[0], chaining_value[1], chaining_value[2], chaining_value[3],
        chaining_value[4], chaining_value[5], chaining_value[6], chaining_value[7],
        IV[0], IV[1], IV[2], IV[3],
        counter as u32, (counter >> 32) as u32, block_len, flags,
    ];
    let mut block = block_words;
    for _ in 0..7 {
        round(&mut state, &block);
        let mut permuted = [0u32; 16];
        for i in 0..16 {
            permuted[i] = block[MSG_PERMUTATION[i]];
        }
        block = permuted;
    }
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= chaining_value[i];
    }
    state
}

fn words_from_le_bytes(bytes: &[u8; 64]) -> [u32; 16] {
    let mut out = [0u32; 16];
    for (i, word) in out.iter_mut().enumerate() {
        *word = u32::from_le_bytes([
            bytes[i * 4],
            bytes[i * 4 + 1],
            bytes[i * 4 + 2],
            bytes[i * 4 + 3],
        ]);
    }
    out
}

fn first_8_words(state: [u32; 16]) -> [u32; 8] {
    let mut cv = [0u32; 8];
    cv.copy_from_slice(&state[..8]);
    cv
}

fn parent_cv(left: [u32; 8], right: [u32; 8]) -> [u32; 8] {
    let mut block_words = [0u32; 16];
    block_words[..8].copy_from_slice(&left);
    block_words[8..].copy_from_slice(&right);
    first_8_words(compress(IV, block_words, 0, B3_BLOCK_LEN as u32, PARENT))
}

struct ChunkState {
    cv: [u32; 8],
    chunk_counter: u64,
    block: [u8; B3_BLOCK_LEN],
    block_len: usize,
    blocks_compressed: usize,
}

impl ChunkState {
    fn new(chunk_counter: u64) -> Self {
        ChunkState {
            cv: IV,
            chunk_counter,
            block: [0u8; B3_BLOCK_LEN],
            block_len: 0,
            blocks_compressed: 0,
        }
    }

    fn len(&self) -> usize {
        B3_BLOCK_LEN * self.blocks_compressed + self.block_len
    }

    fn start_flag(&self) -> u32 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.block_len == B3_BLOCK_LEN {
                let block_words = words_from_le_bytes(&self.block);
                self.cv = first_8_words(compress(
                    self.cv,
                    block_words,
                    self.chunk_counter,
                    B3_BLOCK_LEN as u32,
                    self.start_flag(),
                ));
                self.blocks_compressed += 1;
                self.block = [0u8; B3_BLOCK_LEN];
                self.block_len = 0;
            }
            let want = B3_BLOCK_LEN - self.block_len;
            let take = want.min(input.len());
            self.block[self.block_len..self.block_len + take]
                .copy_from_slice(&input[..take]);
            self.block_len += take;
            input = &input[take..];
        }
    }

    fn output(&self) -> Output {
        Output {
            input_chaining_value: self.cv,
            block_words: words_from_le_bytes(&self.block),
            counter: self.chunk_counter,
            block_len: self.block_len as u32,
            flags: self.start_flag() | CHUNK_END,
        }
    }
}

struct Output {
    input_chaining_value: [u32; 8],
    block_words: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
}

impl Output {
    fn chaining_value(&self) -> [u32; 8] {
        first_8_words(compress(
            self.input_chaining_value,
            self.block_words,
            self.counter,
            self.block_len,
            self.flags,
        ))
    }

    fn root_digest_32(&self) -> DagDigest {
        let state = compress(
            self.input_chaining_value,
            self.block_words,
            self.counter,
            self.block_len,
            self.flags | ROOT,
        );
        let mut digest = [0u8; 32];
        for (i, word) in state[..8].iter().enumerate() {
            digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        digest
    }
}

struct Hasher {
    chunk_state: ChunkState,
    chunk_stack: Vec<[u32; 8]>,
}

impl Hasher {
    fn new() -> Self {
        Hasher {
            chunk_state: ChunkState::new(0),
            chunk_stack: Vec::new(),
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.chunk_state.len() == B3_CHUNK_LEN {
                let chunk_cv = self.chunk_state.output().chaining_value();
                let total_chunks = self.chunk_state.chunk_counter + 1;
                self.add_chunk_chaining_value(chunk_cv, total_chunks);
                self.chunk_state = ChunkState::new(total_chunks);
            }
            let want = B3_CHUNK_LEN - self.chunk_state.len();
            let take = want.min(input.len());
            self.chunk_state.update(&input[..take]);
            input = &input[take..];
        }
    }

    fn add_chunk_chaining_value(&mut self, mut new_cv: [u32; 8], mut total_chunks: u64) {
        while total_chunks & 1 == 0 {
            let popped = self
                .chunk_stack
                .pop()
                .expect("chunk stack carries one entry per pending even merge");
            new_cv = parent_cv(popped, new_cv);
            total_chunks >>= 1;
        }
        self.chunk_stack.push(new_cv);
    }

    fn finalize_32(&self) -> DagDigest {
        let mut output = self.chunk_state.output();
        for parent in self.chunk_stack.iter().rev() { // SHIM-B3 target line
            let mut block_words = [0u32; 16];
            block_words[..8].copy_from_slice(parent);
            block_words[8..].copy_from_slice(&output.chaining_value());
            output = Output {
                input_chaining_value: IV,
                block_words,
                counter: 0,
                block_len: B3_BLOCK_LEN as u32,
                flags: PARENT,
            };
        }
        output.root_digest_32()
    }
}

/// One-shot BLAKE3 (regular mode). Private: the receipt register is only
/// reachable through the DAG API; D-6 KATs exercise it directly in-file.
fn blake3(data: &[u8]) -> DagDigest {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize_32()
}

// ---------------------------------------------------------------------------
// Payload serialization + tree construction
// ---------------------------------------------------------------------------

/// Serialize a leaf [`DagNodePayload`] to its 128-byte §3.1 wire form
/// (big-endian integer fields, offsets pinned by the static asserts).
pub fn payload_bytes(p: &DagNodePayload) -> [u8; DAG_NODE_SIZE] {
    let mut out = [0u8; DAG_NODE_SIZE];
    out[DAG_OFF_LEFT_CHILD..DAG_OFF_RIGHT_CHILD].copy_from_slice(&p.left_child);
    out[DAG_OFF_RIGHT_CHILD..DAG_OFF_BLOCK_OID].copy_from_slice(&p.right_child);
    out[DAG_OFF_BLOCK_OID..DAG_OFF_PARENT_OID].copy_from_slice(&p.block_oid);
    out[DAG_OFF_PARENT_OID..DAG_OFF_BLOCK_LEN].copy_from_slice(&p.parent_oid);
    out[DAG_OFF_BLOCK_LEN..DAG_OFF_SEQ].copy_from_slice(&p.block_len.to_be_bytes());
    out[DAG_OFF_SEQ..DAG_OFF_DEPTH].copy_from_slice(&p.seq.to_be_bytes());
    out[DAG_OFF_DEPTH..DAG_OFF_FLAGS].copy_from_slice(&p.depth.to_be_bytes());
    out[DAG_OFF_FLAGS..DAG_NODE_SIZE].copy_from_slice(&p.flags.to_be_bytes());
    out
}

/// BLAKE3 over the 128-byte §3.1 leaf payload serialization.
fn hash_leaf(p: &DagNodePayload) -> DagDigest {
    blake3(&payload_bytes(p))
}

fn leaf_flags(parent_oid: &Oid) -> u32 {
    if *parent_oid == GENESIS_OID {
        DAG_FLAG_GENESIS
    } else {
        DAG_FLAG_HAS_PARENT
    }
}

fn leaf_payload(block_oid: Oid, parent_oid: Oid, block_len: u64, seq: u64) -> DagNodePayload {
    DagNodePayload {
        left_child: ZERO_DIGEST,
        right_child: ZERO_DIGEST,
        block_oid,
        parent_oid,
        block_len,
        seq,
        depth: 0,
        flags: leaf_flags(&parent_oid),
    }
}

fn internal_digest(left: &DagDigest, right: &DagDigest) -> DagDigest {
    let mut buf = [0u8; INTERNAL_PAYLOAD_SIZE];
    buf[..BLAKE3_DIGEST_LEN].copy_from_slice(left);
    buf[BLAKE3_DIGEST_LEN..].copy_from_slice(right);
    blake3(&buf)
}

/// Internal tree node. Promotion law: a promoted node is the SAME node at
/// every level it survives (counted once — hence node_count == 2L-1).
struct TreeNode {
    depth: u32,
    digest: DagDigest,
    seq_leftmost: u64,
    leaf_index: usize,
    leaf_count: usize,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

/// Build the committed tree over `blocks` (leaves ordered by `seq`, §3.2).
/// Returns the root node and the per-block OIDs in INPUT order.
/// SHIM-ORD mutation target: the seq sort comparator below.
fn build_node_tree(
    blocks: &[(&[u8], u64)],
    parent_oid: &Oid,
) -> Result<(Box<TreeNode>, Vec<Oid>), DagError> {
    if blocks.is_empty() {
        return Err(DagError::EmptyCommit);
    }
    if blocks.len() > MAX_BLOCKS {
        return Err(DagError::TooManyObjects(blocks.len()));
    }
    let mut order: Vec<usize> = (0..blocks.len()).collect();
    order.sort_by_key(|&i| blocks[i].1); // SHIM-ORD mutation target line
    for window in order.windows(2) {
        if blocks[window[0]].1 == blocks[window[1]].1 {
            return Err(DagError::DuplicateSeq(blocks[window[0]].1));
        }
    }
    let mut oids: Vec<Oid> = vec![GENESIS_OID; blocks.len()];
    let mut level: Vec<Box<TreeNode>> = Vec::with_capacity(blocks.len());
    let mut ordinal = 0usize;
    for &i in &order {
        let (body, seq) = blocks[i];
        if body.is_empty() {
            return Err(DagError::EmptyBlock(seq));
        }
        let oid = git_object_oid("blob", body);
        oids[i] = oid;
        let payload = leaf_payload(oid, *parent_oid, body.len() as u64, seq);
        level.push(Box::new(TreeNode {
            depth: 0,
            digest: hash_leaf(&payload),
            seq_leftmost: seq,
            leaf_index: ordinal,
            leaf_count: 1,
            left: None,
            right: None,
        }));
        ordinal += 1;
    }
    while level.len() > 1 {
        let mut next: Vec<Box<TreeNode>> = Vec::with_capacity((level.len() + 1) / 2);
        let mut iter = level.into_iter();
        while let Some(left) = iter.next() {
            match iter.next() {
                Some(right) => {
                    let depth = left.depth.max(right.depth) + 1;
                    let digest = internal_digest(&left.digest, &right.digest);
                    let leaf_count = left.leaf_count + right.leaf_count;
                    let seq_leftmost = left.seq_leftmost;
                    let leaf_index = left.leaf_index;
                    next.push(Box::new(TreeNode {
                        depth,
                        digest,
                        seq_leftmost,
                        leaf_index,
                        leaf_count,
                        left: Some(left),
                        right: Some(right),
                    }));
                }
                None => next.push(left), // promoted unchanged: same node identity
            }
        }
        level = next;
    }
    match level.pop() {
        Some(root) => Ok((root, oids)),
        None => Err(DagError::EmptyCommit),
    }
}

fn emit_postorder(node: &TreeNode, out: &mut Vec<DagTreeNode>) {
    if let Some(left) = &node.left {
        emit_postorder(left, out);
    }
    if let Some(right) = &node.right {
        emit_postorder(right, out);
    }
    out.push(DagTreeNode {
        depth: node.depth,
        digest: node.digest,
    });
}

fn sibling_path(root: &TreeNode, target_leaf_index: usize) -> Vec<(u8, DagDigest)> {
    let mut path = Vec::new();
    let mut node = root;
    let mut k = target_leaf_index;
    loop {
        let left = match &node.left {
            Some(left) => left,
            None => break,
        };
        match &node.right {
            None => node = left, // promoted single child: no sibling at this level
            Some(right) => {
                if k < left.leaf_count {
                    path.push((PROOF_SIDE_RIGHT, right.digest));
                    node = left;
                } else {
                    k -= left.leaf_count;
                    path.push((PROOF_SIDE_LEFT, left.digest));
                    node = right;
                }
            }
        }
    }
    path.reverse(); // §3.3: bottom-up — the fold consumes deepest sibling first
    path
}

fn build_all(
    blocks: &[(&[u8], u64)],
    parent_oid: &Oid,
) -> Result<(DagDigest, DagTree, Vec<Oid>), DagError> {
    let (root_node, oids) = build_node_tree(blocks, parent_oid)?;
    let root = root_node.digest;
    let mut nodes: Vec<DagTreeNode> = Vec::with_capacity(2 * blocks.len() - 1);
    emit_postorder(&root_node, &mut nodes);
    let tree = DagTree {
        root,
        parent_oid: *parent_oid,
        block_count: blocks.len() as u32,
        node_count: nodes.len() as u32,
        nodes,
    };
    Ok((root, tree, oids))
}

/// Commit a snapshot: validate, content-address every block (git blob
/// SHA-1), build the deterministic Merkle DAG (§3.2), return the
/// [`Commit`] with root, input-order OIDs and the committed [`DagTree`].
pub fn commit_blocks(blocks: &[(&[u8], u64)], parent_oid: &Oid) -> Result<Commit, DagError> {
    let (root, tree, oids) = build_all(blocks, parent_oid)?;
    Ok(Commit { root, oids, tree })
}

/// Deterministic BLAKE3 root over the ordered block set (§3.2). A pure
/// function of (block bodies, seqs, parent OID).
pub fn merkle_root(blocks: &[(&[u8], u64)], parent_oid: &Oid) -> Result<DagDigest, DagError> {
    let (root_node, _) = build_node_tree(blocks, parent_oid)?;
    Ok(root_node.digest)
}

/// Build the [`LineageProof`] for the block with `seq`, rebuilding the
/// tree. Proof *generation* is O(L); proof *verification* is the O(1) side
/// (§3.4).
pub fn lineage_proof(
    blocks: &[(&[u8], u64)],
    parent_oid: &Oid,
    seq: u64,
) -> Result<LineageProof, DagError> {
    let (root_node, oids) = build_node_tree(blocks, parent_oid)?;
    let mut order: Vec<usize> = (0..blocks.len()).collect();
    order.sort_by_key(|&i| blocks[i].1);
    let pos = match order.iter().position(|&i| blocks[i].1 == seq) {
        Some(pos) => pos,
        None => return Err(DagError::UnknownSeq(seq)),
    };
    let input_index = order[pos];
    let (body, _) = blocks[input_index];
    Ok(LineageProof {
        block_git_oid: oids[input_index],
        parent_git_oid: *parent_oid,
        seq,
        block_len: body.len() as u64,
        siblings: sibling_path(&root_node, pos),
        root: root_node.digest,
    })
}

/// Verify a lineage proof against a committed root and the expected parent
/// (§4.3). Typed-outcome order: parent check first
/// ([`DagError::LineageMismatch`], D-3/D-8), then root binding
/// ([`DagError::RootMismatch`], D-4). Per-proof cost: 1 leaf hash +
/// `siblings.len()` internal hashes, bounded by [`MAX_PROOF_HASHES`]
/// (DoD-4).
pub fn verify_lineage(
    root: &DagDigest,
    proof: &LineageProof,
    expected_parent: &Oid,
) -> Result<(), DagError> {
    if proof.parent_git_oid != *expected_parent {
        return Err(DagError::LineageMismatch); // SHIM-PARENT target line
    }
    if proof.root != *root {
        return Err(DagError::RootMismatch);
    }
    fold_to_root(&proof.root, proof)?;
    Ok(())
}

fn fold_to_root(expected_root: &DagDigest, proof: &LineageProof) -> Result<usize, DagError> {
    if proof.siblings.len() > MAX_PROOF_LEVELS {
        return Err(DagError::RootMismatch);
    }
    let payload = leaf_payload(
        proof.block_git_oid,
        proof.parent_git_oid,
        proof.block_len,
        proof.seq,
    );
    let mut h = hash_leaf(&payload);
    let mut count = 1usize;
    for &(side, sibling) in &proof.siblings {
        let mut buf = [0u8; INTERNAL_PAYLOAD_SIZE];
        match side {
            PROOF_SIDE_LEFT => {
                buf[..BLAKE3_DIGEST_LEN].copy_from_slice(&sibling);
                buf[BLAKE3_DIGEST_LEN..].copy_from_slice(&h);
            }
            PROOF_SIDE_RIGHT => {
                buf[..BLAKE3_DIGEST_LEN].copy_from_slice(&h);
                buf[BLAKE3_DIGEST_LEN..].copy_from_slice(&sibling);
            }
            _ => return Err(DagError::RootMismatch),
        }
        h = blake3(&buf);
        count += 1;
    }
    if h != *expected_root {
        return Err(DagError::RootMismatch);
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// §5.1 `.cgap-dag` wire codec
// ---------------------------------------------------------------------------

/// Encode a committed [`DagTree`] to the §5.1 `.cgap-dag` file form:
/// 72-byte header, postorder (depth, digest) node table (divergence note
/// 2), SHA-1 trailer over everything above it.
pub fn encode_dag_file(tree: &DagTree) -> Vec<u8> {
    let mut out = Vec::with_capacity(DAG_FILE_FIXED_SIZE + tree.nodes.len() * DAG_ENTRY_SIZE);
    out.extend_from_slice(&DAG_MAGIC);
    out.extend_from_slice(&DAG_FORMAT_VERSION.to_be_bytes());
    out.extend_from_slice(&tree.root);
    out.extend_from_slice(&tree.parent_oid);
    out.extend_from_slice(&tree.block_count.to_be_bytes());
    out.extend_from_slice(&tree.node_count.to_be_bytes());
    for node in &tree.nodes {
        out.extend_from_slice(&node.depth.to_be_bytes());
        out.extend_from_slice(&node.digest);
    }
    let trailer = sha1(&out);
    out.extend_from_slice(&trailer);
    out
}

/// Parse a §5.1 `.cgap-dag` file: magic, version, lengths, SHA-1 trailer,
/// the 2L-1 node-count law, and a full structural reduction of the
/// (depth, digest) postorder sequence back to a single root that must
/// equal the header root.
pub fn decode_dag_file(bytes: &[u8]) -> Result<DagTree, DagError> {
    if bytes.len() < DAG_FILE_FIXED_SIZE {
        return Err(DagError::Truncated);
    }
    if bytes[..8] != DAG_MAGIC {
        return Err(DagError::BadMagic);
    }
    let version = u32::from_be_bytes(
        bytes[8..12].try_into().map_err(|_| DagError::Truncated)?,
    );
    if version != DAG_FORMAT_VERSION {
        return Err(DagError::VersionMismatch(version));
    }
    let root: DagDigest = bytes[12..44].try_into().map_err(|_| DagError::Truncated)?;
    let parent_oid: Oid = bytes[44..64].try_into().map_err(|_| DagError::Truncated)?;
    let block_count =
        u32::from_be_bytes(bytes[64..68].try_into().map_err(|_| DagError::Truncated)?);
    let node_count =
        u32::from_be_bytes(bytes[68..72].try_into().map_err(|_| DagError::Truncated)?);
    // F-1 (wave 6, a03-wave5-audit-receipt): the 2L-1 law and the node-region
    // length are evaluated in u64. In u32 arithmetic a hostile header count
    // >= 0x8000_0000 overflows the multiply in debug builds (panic) and wraps
    // in release (a wrapped-law pass); widening makes every hostile count a
    // typed refusal on every profile. Exact F-1 witness: block_count =
    // 0x8000_0000 with node_count = 0xFFFF_FFFF (law holds exactly, file
    // carries no such table) -> Truncated; the wrap-divergent witness
    // block_count = 0x8000_0001 with node_count = 1 (law holds only wrapped)
    // -> ShapeMismatch. Courted by `f1_hostile_counts_typed_refusal`.
    let block_count64 = u64::from(block_count);
    let node_count64 = u64::from(node_count);
    if block_count64 == 0 || node_count64 != 2 * block_count64 - 1 {
        return Err(DagError::ShapeMismatch);
    }
    let nodes_len = node_count64 * DAG_ENTRY_SIZE as u64;
    let expected_len = DAG_HEADER_SIZE as u64 + nodes_len + DAG_TRAILER_SIZE as u64;
    if bytes.len() as u64 != expected_len {
        return Err(DagError::Truncated);
    }
    let body_len = bytes.len() - DAG_TRAILER_SIZE;
    let trailer: [u8; 20] = bytes[body_len..].try_into().map_err(|_| DagError::Truncated)?;
    if sha1(&bytes[..body_len]) != trailer {
        return Err(DagError::ChecksumMismatch);
    }
    let mut nodes: Vec<DagTreeNode> = Vec::with_capacity(node_count as usize);
    let mut pos = DAG_HEADER_SIZE;
    for _ in 0..node_count {
        let depth =
            u32::from_be_bytes(bytes[pos..pos + 4].try_into().map_err(|_| DagError::Truncated)?);
        let digest: DagDigest = bytes[pos + 4..pos + 36]
            .try_into()
            .map_err(|_| DagError::Truncated)?;
        nodes.push(DagTreeNode { depth, digest });
        pos += DAG_ENTRY_SIZE;
    }
    // Structural reduction: postorder (depth, digest) -> single root.
    let mut stack: Vec<(u32, DagDigest)> = Vec::new();
    for entry in &nodes {
        let mut incoming = (entry.depth, entry.digest);
        while stack.len() >= 2 {
            let (right_depth, right_digest) = stack[stack.len() - 1];
            let (left_depth, left_digest) = stack[stack.len() - 2];
            let candidate_depth = left_depth.max(right_depth) + 1;
            if candidate_depth != incoming.0 {
                break;
            }
            let candidate = internal_digest(&left_digest, &right_digest);
            if candidate != incoming.1 {
                break;
            }
            stack.pop();
            stack.pop();
            incoming = (candidate_depth, candidate);
        }
        stack.push(incoming);
    }
    if stack.len() != 1 {
        return Err(DagError::ShapeMismatch);
    }
    if stack[0].1 != root {
        return Err(DagError::ChecksumMismatch);
    }
    Ok(DagTree {
        root,
        parent_oid,
        block_count,
        node_count,
        nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::{String, ToString};
    use alloc::vec;

    // -- helpers --

    fn hex20(s: &str) -> Oid {
        let mut out = [0u8; 20];
        for i in 0..20 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("hex digit pair");
        }
        out
    }

    fn to_hex(bytes: &[u8]) -> String {
        let mut out = String::new();
        for b in bytes {
            out.push_str(&format!("{b:02x}"));
        }
        out
    }

    /// The official BLAKE3 corpus: first N bytes of the 251-byte cycle
    /// 0,1,...,250 (input definition of the official test vectors).
    fn kat_input(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 251) as u8).collect()
    }

    /// D-2 value pin: BLAKE3 Merkle root of blocks `alpha`/`beta`/`gamma`
    /// at seq 0/1/2 with genesis parent, from the first compliant build
    /// (this session). SHIM-ORD (descending-seq-sort shim) must move this
    /// pin or the court is vacuous.
    const PINNED_ROOT_HEX: &str = "37a732b2b62526fd323bb2fa92877ca9a46854155a7a8041efefe11f8f7fdd6c";

    /// Owned copies of the literal bodies (keeps temporaries alive).
    fn owned_bodies(bodies: &[&[u8]]) -> Vec<Vec<u8>> {
        bodies.iter().map(|b| b.to_vec()).collect()
    }

    /// References into `owned` with per-block seqs; `owned` must stay
    /// alive and unmodified while the result is used.
    fn refs_of<'a>(owned: &'a [Vec<u8>], seqs: &[u64]) -> Vec<(&'a [u8], u64)> {
        let mut refs: Vec<(&[u8], u64)> = owned.iter().map(|b| (b.as_slice(), 0u64)).collect();
        for (i, &seq) in seqs.iter().enumerate() {
            refs[i].1 = seq;
        }
        refs
    }

    // -- courts --

    /// D-1 root-determinism (§6): same block set -> byte-identical root
    /// across two fresh commits. Anti-vacuity mutation: flip one bit in
    /// one block body; the root MUST change or the court is VACUOUS.
    #[test]
    fn d1_root_determinism_and_bitflip_mutation() {
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let c1 = commit_blocks(&refs, &GENESIS_OID).expect("commit");
        let c2 = commit_blocks(&refs, &GENESIS_OID).expect("commit");
        assert_eq!(c1, c2);

        let mut mutated = owned.clone();
        mutated[1][0] ^= 0x01;
        let mrefs = refs_of(&mutated, &[0, 1, 2]);
        let c3 = commit_blocks(&mrefs, &GENESIS_OID).expect("commit");
        assert_ne!(c1.root, c3.root, "VACUOUS: bit flip did not move the root");
    }

    /// D-2 ordering court (§6): permuting the input order (same seqs)
    /// yields the same root; permuting the seqs yields a different root;
    /// the root VALUE is pinned to `PINNED_ROOT_HEX` (value KAT, so an
    /// internal reordering shim cannot pass by staying self-consistent —
    /// SHIM-ORD shim court must move this pin).
    #[test]
    fn d2_ordering_and_pinned_root() {
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let root = merkle_root(&refs, &GENESIS_OID).expect("root");

        let mut shuffled = refs.clone();
        shuffled.swap(0, 2);
        assert_eq!(
            merkle_root(&shuffled, &GENESIS_OID).expect("root"),
            root,
            "input permutation with same seqs must not move the root"
        );

        let reseq = refs_of(&owned, &[1, 2, 3]);
        assert_ne!(
            merkle_root(&reseq, &GENESIS_OID).expect("root"),
            root,
            "VACUOUS: re-seqing did not move the root"
        );

        assert_eq!(
            to_hex(&root),
            PINNED_ROOT_HEX,
            "pinned root value drifted (blocks alpha/beta/gamma @0/1/2, genesis)"
        );
    }

    /// D-3 lineage court (§6): a genuine proof verifies Ok. Anti-vacuity
    /// mutation: flip one byte of `parent_git_oid` inside the proof;
    /// verification MUST return LineageMismatch (never Ok, never a
    /// different variant — the parent check precedes the fold).
    #[test]
    fn d3_lineage_parent_mutation() {
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let parent: Oid = [0x11; 20];
        let commit = commit_blocks(&refs, &parent).expect("commit");
        let mut proof = lineage_proof(&refs, &parent, 1).expect("proof");
        assert_eq!(verify_lineage(&commit.root, &proof, &parent), Ok(()));

        proof.parent_git_oid[3] ^= 0x01;
        assert_eq!(
            verify_lineage(&commit.root, &proof, &parent),
            Err(DagError::LineageMismatch),
            "VACUOUS: flipped parent did not yield LineageMismatch"
        );
    }

    /// D-4 root court (§6): proofs verify against the shipped root.
    /// Anti-vacuity mutations: flip one sibling digest byte ->
    /// RootMismatch; flip one byte of the proof's own root ->
    /// RootMismatch.
    #[test]
    fn d4_root_mutation() {
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let commit = commit_blocks(&refs, &GENESIS_OID).expect("commit");
        let mut proof = lineage_proof(&refs, &GENESIS_OID, 0).expect("proof");
        assert_eq!(verify_lineage(&commit.root, &proof, &GENESIS_OID), Ok(()));

        proof.siblings[0].1[5] ^= 0x01;
        assert_eq!(
            verify_lineage(&commit.root, &proof, &GENESIS_OID),
            Err(DagError::RootMismatch),
            "VACUOUS: sibling flip did not yield RootMismatch"
        );

        let mut proof = lineage_proof(&refs, &GENESIS_OID, 0).expect("proof");
        proof.root[0] ^= 0x01;
        assert_eq!(
            verify_lineage(&commit.root, &proof, &GENESIS_OID),
            Err(DagError::RootMismatch),
            "VACUOUS: root flip did not yield RootMismatch"
        );
    }

    /// D-5 git-compat court (§6): `git_object_oid("blob", b"hello\n")`
    /// matches the canonical git vector (cross-checked against real `git
    /// hash-object` this session), the pack of the committed blocks
    /// round-trips through the in-crate parser, and the §4.2 frame fields
    /// derive from the commit without recomputation.
    #[test]
    fn d5_git_compat_and_frame_fields() {
        assert_eq!(
            git_object_oid("blob", b"hello\n"),
            hex20("ce013625030ba8dba906f756967f9e9ca394464a"),
            "git blob OID KAT (real git: printf 'hello\\n' | git hash-object --stdin)"
        );

        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let commit = commit_blocks(&refs, &GENESIS_OID).expect("commit");
        let bodies: Vec<&[u8]> = vec![b"alpha", b"beta", b"gamma"];
        let pack = crate::pack_write::write_pack_v2(&bodies).expect("pack");
        let parsed = crate::pack_write::parse_pack_v2(&pack).expect("parse");
        assert_eq!(parsed.len(), 3, "pack must round-trip: 3 objects");
        for (object, body) in parsed.iter().zip(bodies.iter()) {
            assert_eq!(object.obj_type, 3, "blob type code");
            assert_eq!(object.size, body.len() as u64, "declared size matches");
        }

        let fields = commit.frame_fields(sha1(&pack));
        assert_eq!(fields.root, commit.root);
        assert_eq!(fields.parent_git_oid, GENESIS_OID);
        assert_eq!(fields.block_count, 3);
        assert_eq!(fields.pack_sha1, sha1(&pack));
    }

    /// D-6 BLAKE3 KAT court (§6): the in-crate regular-mode BLAKE3 matches
    /// the official vectors (generated this session with the official
    /// `b3sum` over the official 0..=250 cycle corpus; the empty vector
    /// additionally matches the published constant af1349b9...). Lengths
    /// span single chunk (0/1/64/1024) and multi-chunk chunk-chaining
    /// (1025/2048/2500/4096). Anti-vacuity mutation (SHIM-B3 shim court):
    /// skip the final chunk-stack fold; the multi-chunk vectors MUST fail
    /// while the single-chunk vectors keep passing.
    #[test]
    fn d6_blake3_official_kats() {
        let kats: &[(&str, &str)] = &[
            (
                "0",
                "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
            ),
            (
                "1",
                "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213",
            ),
            (
                "64",
                "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98",
            ),
            (
                "1024",
                "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
            ),
            (
                "1025",
                "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444",
            ),
            (
                "2048",
                "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a",
            ),
            (
                "2500",
                "95e809954ebdd862debeb347d5715c09e73d150cd0704d4bbd834993a730e2ff",
            ),
            (
                "4096",
                "015094013f57a5277b59d8475c0501042c0b642e531b0a1c8f58d2163229e969",
            ),
        ];
        for (len, want) in kats {
            let got = to_hex(&blake3(&kat_input(len.parse::<usize>().expect("decimal length"))));
            assert_eq!(&got, want, "BLAKE3 KAT at input length {len}");
        }
    }

    /// D-7 cap court (§6): 65,537 blocks -> TooManyObjects. The cap cannot
    /// be raised from this file silently: MAX_BLOCKS derives from
    /// `ladder::LADDER_WINDOW` and is cross-pinned to
    /// `pack_write::MAX_OBJECTS` by static asserts (SHIM-CAP shim court:
    /// perturbing the const fails the BUILD).
    #[test]
    fn d7_cap_court() {
        let n = MAX_BLOCKS + 1;
        let owned: Vec<Vec<u8>> = (0..n).map(|_| vec![0u8; 8]).collect();
        let seqs: Vec<u64> = (0..n as u64).collect();
        let refs = refs_of(&owned, &seqs);
        assert_eq!(
            commit_blocks(&refs, &GENESIS_OID),
            Err(DagError::TooManyObjects(n)),
            "cap must refuse 65,537 blocks"
        );
        assert_eq!(MAX_BLOCKS, crate::pack_write::MAX_OBJECTS);
    }

    /// D-8 rollback court (§6, §4.3): a proof with parent P verifies
    /// against P; the same proof against rollback target P' returns
    /// LineageMismatch. Anti-vacuity mutation (SHIM-PARENT shim court):
    /// neutralize the parent check in verify_lineage; D-3 and D-8 MUST
    /// fail.
    #[test]
    fn d8_rollback_court() {
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let parent: Oid = [0x22; 20];
        let other: Oid = [0x33; 20];
        let commit = commit_blocks(&refs, &parent).expect("commit");
        let proof = lineage_proof(&refs, &parent, 2).expect("proof");
        assert_eq!(verify_lineage(&commit.root, &proof, &parent), Ok(()));
        assert_eq!(
            verify_lineage(&commit.root, &proof, &other),
            Err(DagError::LineageMismatch),
            "VACUOUS: rollback target accepted"
        );
    }

    /// Patch a byte inside a `.cgap-dag` file and recompute the SHA-1
    /// trailer, modeling corruption that carries a VALID checksum (a
    /// lying-trailer threat). Only this reaches the structural checks.
    fn resealed(bytes: &[u8], at: usize) -> Vec<u8> {
        let mut out = bytes.to_vec();
        let body_len = out.len() - DAG_TRAILER_SIZE;
        out[at] ^= 0x01;
        let trailer = sha1(&out[..body_len]);
        out.truncate(body_len);
        out.extend_from_slice(&trailer);
        out
    }

    /// D-9 wire round-trip court (§6, §5.1): encode -> decode -> identical
    /// tree. Anti-vacuity mutations: flip a trailer SHA-1 byte ->
    /// ChecksumMismatch; flip magic -> BadMagic; flip version ->
    /// VersionMismatch; truncate -> Truncated; and — each under a re-sealed
    /// (valid-checksum) corruption — patch node_count -> ShapeMismatch;
    /// flip a node depth byte -> ShapeMismatch; flip a header root byte ->
    /// ChecksumMismatch (root vs recomputed tree).
    #[test]
    fn d9_wire_roundtrip_and_mutations() {
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let commit = commit_blocks(&refs, &GENESIS_OID).expect("commit");
        let encoded = encode_dag_file(&commit.tree);

        let decoded = decode_dag_file(&encoded).expect("decode");
        assert_eq!(decoded, commit.tree, "round-trip must be identity");
        assert_eq!(encode_dag_file(&decoded), encoded, "re-encode is stable");
        assert_eq!(commit.tree.node_count, 2 * 3 - 1, "2L-1 node law");

        // trailer flip -> ChecksumMismatch
        let mut bad = encoded.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0x01;
        assert_eq!(decode_dag_file(&bad), Err(DagError::ChecksumMismatch));

        // magic flip -> BadMagic
        let mut bad = encoded.clone();
        bad[0] ^= 0x01;
        assert_eq!(decode_dag_file(&bad), Err(DagError::BadMagic));

        // version flip -> VersionMismatch(2)
        let mut bad = encoded.clone();
        bad[11] ^= 0x03;
        assert_eq!(decode_dag_file(&bad), Err(DagError::VersionMismatch(2)));

        // truncation -> Truncated
        assert_eq!(
            decode_dag_file(&encoded[..encoded.len() - 1]),
            Err(DagError::Truncated)
        );

        // node_count patch (re-sealed) -> ShapeMismatch via the 2L-1 law
        assert_eq!(
            decode_dag_file(&resealed(&encoded, 68)),
            Err(DagError::ShapeMismatch)
        );

        // node depth flip (re-sealed) -> ShapeMismatch via the reduction
        assert_eq!(
            decode_dag_file(&resealed(&encoded, DAG_HEADER_SIZE)),
            Err(DagError::ShapeMismatch)
        );

        // header root flip (re-sealed) -> ChecksumMismatch (root vs
        // recomputed tree)
        assert_eq!(
            decode_dag_file(&resealed(&encoded, 12)),
            Err(DagError::ChecksumMismatch)
        );
    }

    /// Hostile `.cgap-dag` header builder: valid 92-byte frame (magic,
    /// version, root, parent) with attacker-chosen counts and a zero
    /// trailer. Law/length checks run before the trailer, so the zero
    /// trailer never influences these outcomes.
    fn hostile_dag_file(block_count: u32, node_count: u32) -> Vec<u8> {
        let mut out = vec![0u8; DAG_FILE_FIXED_SIZE];
        out[..8].copy_from_slice(&DAG_MAGIC);
        out[8..12].copy_from_slice(&DAG_FORMAT_VERSION.to_be_bytes());
        out[12..44].copy_from_slice(&[7u8; 32]);
        out[44..64].copy_from_slice(&[9u8; 20]);
        out[64..68].copy_from_slice(&block_count.to_be_bytes());
        out[68..72].copy_from_slice(&node_count.to_be_bytes());
        out
    }

    /// F-1 hostile-count court (wave 6, a03-wave5-audit-receipt): hostile
    /// header counts are typed refusals, never panics, never wraps.
    ///
    /// Witness A is the audit's exact F-1 input: block_count = 0x8000_0000,
    /// node_count = 0xFFFF_FFFF. The law holds exactly in u64 (2L-1 ==
    /// 0xFFFF_FFFF) so the refusal is Truncated (the bytes cannot carry the
    /// claimed table); in u32 arithmetic this line panicked in debug builds
    /// (multiply overflow) and wrapped silently in release.
    ///
    /// Witness B is the wrap-divergent input: block_count = 0x8000_0001,
    /// node_count = 1. The u32 law passes only by wrap-around; the u64 law
    /// fails -> ShapeMismatch. This witness flips the court in BOTH
    /// profiles under the named mutation.
    ///
    /// Named anti-vacuity mutation (SHIM-F1, applied in a sandbox copy this
    /// session): revert the law to u32 arithmetic (`node_count != 2 *
    /// block_count - 1`). Debug: witnesses A and B panic -> court FAIL.
    /// Release: witness B returns Truncated (wrapped-law pass) instead of
    /// ShapeMismatch -> court FAIL.
    ///
    /// Over-correction anchor: a legitimate encode->decode must stay Ok, so
    /// the guard cannot be "fixed" by refusing everything.
    #[test]
    fn f1_hostile_counts_typed_refusal() {
        // Witness A: the audit's exact F-1 hostile file.
        let a = hostile_dag_file(0x8000_0000, 0xFFFF_FFFF);
        assert_eq!(a.len(), 92, "F-1 witness is the minimal 92-byte file");
        assert_eq!(
            decode_dag_file(&a),
            Err(DagError::Truncated),
            "hostile counts must refuse typed (audit F-1 debug-panic witness)"
        );

        // Witness B: law holds only under u32 wrap -> ShapeMismatch.
        let b = hostile_dag_file(0x8000_0001, 0x0000_0001);
        assert_eq!(
            decode_dag_file(&b),
            Err(DagError::ShapeMismatch),
            "u32-wrapped 2L-1 pass must refuse as ShapeMismatch"
        );

        // Over-correction anchor: real files still decode.
        let owned = owned_bodies(&[b"alpha", b"beta", b"gamma"]);
        let refs = refs_of(&owned, &[0, 1, 2]);
        let commit = commit_blocks(&refs, &GENESIS_OID).expect("commit");
        let decoded = decode_dag_file(&encode_dag_file(&commit.tree)).expect("decode");
        assert_eq!(decoded.node_count, 5, "legit L=3 tree still decodes (2L-1)");
    }

    /// Wave-9 cross-lane integration court — the A05 power-cut discipline
    /// (cgap-storage `tests/court_sweep.rs`: a real cut at EVERY byte
    /// offset `k in 0..N` of a commit transaction, typed roll-forward,
    /// never a simulated sleep) transplanted onto this lane's `.cgap-dag`
    /// wire (§5.1).
    ///
    /// Law: for a real committed tree (L = 5, promoted/unbalanced, 9-node
    /// postorder table), EVERY proper prefix `&encoded[..k]`, `k in
    /// 0..N`, must decode to the typed refusal [`DagError::Truncated`] —
    /// never a panic, never `Ok`. A proper prefix can never satisfy the
    /// decoder's exact-length law `len == 72 + (2L-1)*36 + 20`, so an
    /// `Ok` on a prefix would mean the wire parser accepted a
    /// non-self-describing fragment as lineage authority — precisely the
    /// failure class A05's sweep exists to forbid.
    ///
    /// Anti-vacuity, disclosed: the decoder enforces the prefix-refusal
    /// law at FOUR redundant layers (minimum-size guard, exact-length
    /// law, SHA-1 trailer check, structural reduction + root compare), so
    /// NO single-site mutation flips this sweep — defense in depth,
    /// witnessed by reading `decode_dag_file`, not assumed. The named
    /// maximal shim mutation (accept-anything decoder: `Ok` on every
    /// input) MUST flip the court: the first sweep step (`k = 0`, empty
    /// bytes) then decodes `Ok` and the mismatch fires. The full-length
    /// `Ok` anchor below proves the sweep is not vacuously green via
    /// universal refusal.
    #[test]
    fn wave9_powercut_sweep_dag_wire() {
        // L = 5: promoted (unbalanced) tree with distinct bodies.
        let owned: Vec<Vec<u8>> = (0..5u32).map(|i| vec![(i as u8) + 1; 8]).collect();
        let seqs: Vec<u64> = (0..5u64).collect();
        let refs = refs_of(&owned, &seqs);
        let commit = commit_blocks(&refs, &GENESIS_OID).expect("commit L=5");
        let encoded = encode_dag_file(&commit.tree);
        let n = encoded.len();
        assert_eq!(
            n,
            DAG_HEADER_SIZE + 9 * DAG_ENTRY_SIZE + DAG_TRAILER_SIZE,
            "L=5 wire: 72 + 9*36 + 20 = 416 bytes"
        );

        // Ok anchor: the uncut file decodes (sweep cannot pass by
        // universal refusal).
        assert_eq!(
            decode_dag_file(&encoded),
            Ok(commit.tree),
            "full-length anchor: decode Ok is reachable"
        );

        // The sweep: cut at EVERY byte offset.
        let mut truncated = 0usize;
        for k in 0..n {
            match decode_dag_file(&encoded[..k]) {
                Err(DagError::Truncated) => truncated += 1,
                other => panic!(
                    "power cut at offset {k}/{n} must refuse typed Truncated, got {other:?}"
                ),
            }
        }
        assert_eq!(truncated, n, "every one of the {n} cut points refuses typed");
    }

    /// DoD-4 (§7): for a 65,536-block snapshot a proof carries exactly 16
    /// sibling levels and verification performs exactly 17 BLAKE3
    /// invocations (1 leaf + 16 internal).
    #[test]
    fn dod4_proof_budget_17_hashes() {
        let n = MAX_BLOCKS;
        let owned: Vec<Vec<u8>> = (0..n).map(|i| vec![(i % 251) as u8; 8]).collect();
        let seqs: Vec<u64> = (0..n as u64).collect();
        let refs = refs_of(&owned, &seqs);
        let parent: Oid = [0x44; 20];
        let commit = commit_blocks(&refs, &parent).expect("commit 65,536 blocks");
        assert_eq!(commit.tree.block_count, n as u32);
        assert_eq!(commit.tree.node_count, (2 * n - 1) as u32);

        let proof = lineage_proof(&refs, &parent, 0).expect("proof");
        assert_eq!(proof.siblings.len(), MAX_PROOF_LEVELS, "16 levels at L=2^16");
        assert_eq!(MAX_PROOF_HASHES, 17);
        let count = fold_to_root(&proof.root, &proof).expect("fold");
        assert_eq!(count, MAX_PROOF_HASHES, "17 hashes per proof");
        assert_eq!(verify_lineage(&commit.root, &proof, &parent), Ok(()));

        // §5.2 wire length pin at L=2^16: 618 bytes.
        let wire_len =
            PROOF_WIRE_FIXED + proof.siblings.len() * PROOF_WIRE_SIBLING + PROOF_WIRE_ROOT;
        assert_eq!(wire_len, 618);
    }

    /// R-3 (§8): unbalanced promoted trees. For L = 1..=7: determinism
    /// across two commits, the 2L-1 node law, and every leaf's proof
    /// verifies against the committed root.
    #[test]
    fn r3_unbalanced_trees() {
        for l in 1..=7usize {
            let bodies: Vec<&[u8]> = (0..l).map(|_| &b"body"[..]).collect();
            let seqs: Vec<u64> = (0..l as u64).collect();
            let owned = owned_bodies(&bodies);
            let refs = refs_of(&owned, &seqs);
            let commit = commit_blocks(&refs, &GENESIS_OID).expect("commit");
            assert_eq!(commit.tree.node_count as usize, 2 * l - 1, "2L-1 at L={l}");
            let again = commit_blocks(&refs, &GENESIS_OID).expect("commit");
            assert_eq!(commit.root, again.root, "determinism at L={l}");

            for seq in 0..l as u64 {
                let proof = lineage_proof(&refs, &GENESIS_OID, seq).expect("proof");
                assert_eq!(
                    verify_lineage(&commit.root, &proof, &GENESIS_OID),
                    Ok(()),
                    "proof for seq {seq} at L={l}"
                );
            }

            // promoted paths stay shorter than the full-height bound
            if l >= 3 {
                let last = lineage_proof(&refs, &GENESIS_OID, l as u64 - 1).expect("proof");
                let bitlen = (usize::BITS - (l as usize).leading_zeros()) as usize;
                assert!(
                    last.siblings.len() < bitlen,
                    "promoted path must be shorter than bitlen(L) at L={l}"
                );
            }
        }
    }

    /// §4.1 commit validation: duplicate seq, empty body, empty commit,
    /// unknown seq for proof generation — every typed refusal exercised;
    /// genesis vs has-parent flags inside the leaf payload.
    #[test]
    fn commit_validation_errors() {
        let dup_owned = owned_bodies(&[b"a", b"b"]);
        let dup = refs_of(&dup_owned, &[0, 0]);
        assert_eq!(
            commit_blocks(&dup, &GENESIS_OID),
            Err(DagError::DuplicateSeq(0))
        );

        let empty_owned = owned_bodies(&[b"a", b""]);
        let empty = refs_of(&empty_owned, &[0, 1]);
        assert_eq!(
            commit_blocks(&empty, &GENESIS_OID),
            Err(DagError::EmptyBlock(1))
        );

        let none: Vec<(&[u8], u64)> = vec![];
        assert_eq!(
            commit_blocks(&none, &GENESIS_OID),
            Err(DagError::EmptyCommit)
        );

        let refs = refs_of(&dup_owned, &[0, 1]);
        assert_eq!(
            lineage_proof(&refs, &GENESIS_OID, 9),
            Err(DagError::UnknownSeq(9))
        );

        let genesis = leaf_payload([7u8; 20], GENESIS_OID, 8, 0);
        let child = leaf_payload([7u8; 20], [9u8; 20], 8, 0);
        assert_eq!(genesis.flags, DAG_FLAG_GENESIS);
        assert_eq!(child.flags, DAG_FLAG_HAS_PARENT);
        assert_ne!(hash_leaf(&genesis), hash_leaf(&child));
    }

    /// Runtime mirror of the compile-time ABI asserts plus the wave-2
    /// additions: same pins, asserted on final state so `cargo test`
    /// carries a receipt too. The anti-vacuity mutation is the shim court
    /// (change a field type/order or constant -> build fails; SHIM-ABI).
    #[test]
    fn abi_frozen() {
        assert_eq!(size_of::<DagNodePayload>(), 128);
        assert_eq!(align_of::<DagNodePayload>(), 8);
        assert_eq!(offset_of!(DagNodePayload, block_oid), 64);
        assert_eq!(offset_of!(DagNodePayload, parent_oid), 84);
        assert_eq!(offset_of!(DagNodePayload, block_len), 104);
        assert_eq!(offset_of!(DagNodePayload, seq), 112);
        assert_eq!(offset_of!(DagNodePayload, depth), 120);
        assert_eq!(offset_of!(DagNodePayload, flags), 124);
        assert_eq!(size_of::<DagTreeNode>(), 36);
        assert_eq!(size_of::<DagHeader>(), 72);
        assert_eq!(offset_of!(DagHeader, node_count), 68);
        assert_eq!(MAX_BLOCKS, 65_536);
        assert_eq!(MAX_BLOCKS, crate::pack_write::MAX_OBJECTS);
        assert_eq!(MAX_BLOCKS, crate::ladder::LADDER_WINDOW as usize);
        assert_eq!(MAX_PROOF_HASHES, 17);
        assert_eq!(
            PROOF_WIRE_FIXED + MAX_PROOF_LEVELS * PROOF_WIRE_SIBLING + PROOF_WIRE_ROOT,
            618
        );
        assert_eq!(DAG_MAGIC, *b"CGAPDAG\0");
        assert_eq!(INTERNAL_PAYLOAD_SIZE, 64);
    }

    /// Wave-1 witness retained: the value types round-trip derive behavior
    /// (Clone/PartialEq) independent of the hashing/verification logic
    /// added in Wave 2.
    #[test]
    fn value_types_are_inert() {
        let leaf = DagNodePayload {
            left_child: ZERO_DIGEST,
            right_child: ZERO_DIGEST,
            block_oid: [7u8; 20],
            parent_oid: GENESIS_OID,
            block_len: 512,
            seq: 0,
            depth: 0,
            flags: DAG_FLAG_GENESIS,
        };
        assert_eq!(leaf, leaf.clone());
        assert_ne!(leaf, DagNodePayload { seq: 1, ..leaf });

        let proof = LineageProof {
            block_git_oid: [7u8; 20],
            parent_git_oid: GENESIS_OID,
            seq: 0,
            block_len: 512,
            siblings: vec![(PROOF_SIDE_LEFT, [9u8; 32])],
            root: [1u8; 32],
        };
        assert_eq!(proof.siblings.len(), 1);
        assert_eq!(proof, proof.clone());

        let err = DagError::DuplicateSeq(3);
        assert_eq!(err.to_string(), "duplicate block seq 3: every block in a snapshot needs a \
             distinct ordering index — renumber the blocks and recommit");
        assert_ne!(DagError::LineageMismatch, DagError::RootMismatch);
    }
}
