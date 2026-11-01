//! Git Packfile v2 writer for cgap snapshot chunks (FR-04).
//!
//! Conforms to the byte layout in
//! `docs/jira/v26.10.31/25-git-packfile-integration-spec.md`. v26.10.31
//! FR-04: object bodies are zlib streams (in-crate DEFLATE, `deflate`
//! module) — no deltas, no new type codes. SHA-1, CRC32, and DEFLATE are
//! implemented in-crate; zero external dependencies.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

use core::fmt;

pub mod deflate;
pub mod pack_write;

pub use pack_write::{write_idx_v2, write_pack_v2, IdxEntry, PackBuilder};

/// 20-byte SHA-1 object name.
pub type Oid = [u8; 20];

/// Errors from pack/idx writing and parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    /// Object count exceeds the pack v2 u32 object-count field or the spec's
    /// <= 2^16 ladder-window cap (open item L6-1).
    TooManyObjects(usize),
    /// Pack content checksum mismatch.
    PackChecksumMismatch,
    /// Index content checksum mismatch.
    IdxChecksumMismatch,
    /// Bad magic or version in pack or idx.
    BadMagic,
    /// Truncated pack or idx, or object body shorter than its header claimed.
    Truncated,
    /// Malformed varint header (invalid type code or size encoding).
    BadVarint,
    /// Declared size exceeds u32 range; not representable in idx small
    /// offsets table (u32) or unsupported here.
    SizeOverflow,
    /// An offset exceeds 2^31 - 1; large-offset table not implemented in v1.
    OffsetTooLarge,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::TooManyObjects(n) => write!(
                f,
                "too many objects in pack: {n} exceeds the pack-v2 ladder-window cap of 2^16 \
                 (open item L6-1) — split the snapshot across multiple packs"
            ),
            GitError::PackChecksumMismatch => write!(
                f,
                "pack trailer SHA-1 does not match the pack bytes preceding it: content was \
                 corrupted or altered after the pack was written — re-generate or re-fetch the \
                 pack; do not retry against the same bytes"
            ),
            GitError::IdxChecksumMismatch => write!(
                f,
                "idx trailer SHA-1 does not match the idx bytes preceding it: the index was \
                 corrupted after writing — re-generate the idx from the pack (write_idx_v2) \
                 rather than patching bytes"
            ),
            GitError::BadMagic => write!(
                f,
                "bad magic/version in pack or idx: leading bytes are not 'PACK' + version 2 \
                 (pack) or 0xff 't' 'O' 'c' + version 2 (idx) — confirm the file is a git \
                 packfile-v2/idx-v2 artifact, not a loose object or other format"
            ),
            GitError::Truncated => write!(
                f,
                "pack or idx bytes ended mid-structure: the byte stream is shorter than the \
                 header/count fields declare — re-check the file was fully written (atomic \
                 temp+rename) and not truncated in transfer"
            ),
            GitError::BadVarint => write!(
                f,
                "malformed varint object header: type code or MSB size continuation is not valid \
                 pack-v2 encoding — the pack was not written by a conforming writer or is \
                 corrupt; re-generate it"
            ),
            GitError::SizeOverflow => write!(
                f,
                "object size exceeds u32: the declared size does not fit the u32 the idx v2 \
                 small-offset table admits — shrink the object or add the large-offset path; \
                 the entry cannot be encoded as-is"
            ),
            GitError::OffsetTooLarge => write!(
                f,
                "offset exceeds 2^31-1: pack offsets past 2^31-1 need the v2 large-offset table, \
                 which is not implemented — keep individual packs under 2 GiB or add the \
                 large-offset table"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oid_is_20_bytes() {
        let oid: Oid = [0u8; 20];
        assert_eq!(oid.len(), 20);
    }

    #[test]
    fn error_display() {
        use alloc::string::ToString;
        assert_eq!(
            GitError::TooManyObjects(70_000).to_string(),
            "too many objects in pack: 70000 exceeds the pack-v2 ladder-window cap of 2^16 \
             (open item L6-1) — split the snapshot across multiple packs"
        );
    }
}

// ---- lane I7 wire-in (appended per wave contract; see idx_read.rs/ladder.rs) ----
pub mod idx_read;
pub mod ladder;

// ---- wave-4 lane 2 wire-in (inflate: RFC 1951/1950 read side) ----
pub mod inflate;

// v26.11.1 moonshot lanes (coordinator-scaffolded, additive only):
pub mod dag;
