//! Isomorphism harness (lane A04, Wave 2 core engines).
//!
//! Spec: `docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md` §8.1
//! (tier definitions) and §8.2 (identity-court failure contract). Wave 1
//! froze the type-level ABI; Wave 2 lands `identity_assert` (§3: "the
//! bit-for-bit identity assertion and its per-tier projections") and the
//! §8.2 fail-loud report line. The executed court itself lives in
//! `tests/identity_court.rs`.

#![allow(dead_code)] // Kernel ABI for consumer lanes (A14/A17/A19); unused here by design.

/// The triumvirate tiers (§8.1): wire (network frame), memory (DMA ring
/// slot), disk (Arrow batch view / Parquet page view). The court asserts
/// the same bytes across all four projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Wire tier: `CgapFrameHeader` + payload crossing the socket/DMA/write(2).
    Wire,
    /// Memory tier: bytes published/read through the `DmaRing` slot.
    Memory,
    /// Disk tier, Arrow: (offset, len) views over the region, no copy (§8.1).
    Arrow,
    /// Disk tier, Parquet: thrift-free self-describing page view (§8.1).
    Parquet,
}

impl Tier {
    /// §8.2 fail-line tier token (`tier=<t>`).
    pub fn name(self) -> &'static str {
        match self {
            Tier::Wire => "wire",
            Tier::Memory => "memory",
            Tier::Arrow => "arrow",
            Tier::Parquet => "parquet",
        }
    }
}

/// Typed divergence report for the identity court, mirroring the §8.2
/// fail-loud line `IDENTITY_COURT_FAIL tier=<t> offset=<i> wire=0x.. mem=0x..
/// disk=0x..`: the divergent tier, the first divergent byte offset, and the
/// per-tier observed bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierMismatch {
    /// Tier whose bytes diverged from the wire projection.
    pub tier: Tier,
    /// First divergent byte offset into the frame region.
    pub offset: usize,
    /// Wire-tier byte at `offset`.
    pub wire: u8,
    /// Memory-tier byte at `offset`.
    pub mem: u8,
    /// Disk-tier byte at `offset`.
    pub disk: u8,
}

impl core::fmt::Display for TierMismatch {
    /// The §8.2 fail-loud line, verbatim shape:
    /// `IDENTITY_COURT_FAIL tier=<t> offset=<i> wire=0x.. mem=0x.. disk=0x..`
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "IDENTITY_COURT_FAIL tier={} offset={} wire=0x{:02x} mem=0x{:02x} disk=0x{:02x}",
            self.tier.name(),
            self.offset,
            self.wire,
            self.mem,
            self.disk
        )
    }
}

/// §8.1 bit-for-bit identity assertion: `projection` (a non-wire tier's view
/// of the frame region — memory slot bytes, disk-view bytes) must equal the
/// canonical wire bytes `wire` over the FULL region (header + payload).
///
/// Identity is over values, not addresses (§8.1: "`addr_of`-independent").
/// Returns `Ok(())` on identity; on the first divergence — differing byte or
/// length difference (one region a prefix of the other) — returns
/// [`TierMismatch`] naming the tier, the first divergent offset, and the
/// per-tier bytes at that offset (`0x00` where a region has already ended).
///
/// Tier-specific re-derivation obligations (digest, `payload_byte_len`) are
/// asserted by [`crate::frame::CgapFrameHeader::verify_payload`] at the call
/// site; the executed combination is `tests/identity_court.rs`.
pub fn identity_assert(
    wire: &[u8],
    tier: Tier,
    projection: &[u8],
) -> Result<(), TierMismatch> {
    let report = |offset: usize| TierMismatch {
        tier,
        offset,
        wire: wire.get(offset).copied().unwrap_or(0),
        mem: if tier == Tier::Memory {
            projection.get(offset).copied().unwrap_or(0)
        } else {
            0
        },
        disk: if matches!(tier, Tier::Arrow | Tier::Parquet) {
            projection.get(offset).copied().unwrap_or(0)
        } else {
            0
        },
    };
    if wire.len() != projection.len() {
        return Err(report(wire.len().min(projection.len())));
    }
    for (i, (w, p)) in wire.iter().zip(projection.iter()).enumerate() {
        if w != p {
            return Err(report(i));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_holds_on_equal_regions() {
        let a = [0u8, 1, 2, 3, 250];
        assert_eq!(identity_assert(&a, Tier::Memory, &a), Ok(()));
        assert_eq!(identity_assert(&a, Tier::Parquet, &a), Ok(()));
    }

    #[test]
    fn first_divergent_byte_named_with_tier_and_offset() {
        // §8.3 mutation row: flip 1 byte in any tier's layout → court FAILs,
        // names tier+offset.
        let wire = [1u8, 2, 3, 4, 5];
        let mut mem = wire;
        mem[3] ^= 0x01;
        let m = identity_assert(&wire, Tier::Memory, &mem).unwrap_err();
        assert_eq!(m.tier, Tier::Memory);
        assert_eq!(m.offset, 3);
        assert_eq!(m.wire, 4);
        assert_eq!(m.mem, 4 ^ 0x01);
        assert_eq!(m.disk, 0); // memory-tier divergence: disk slot unobserved
    }

    #[test]
    fn length_divergence_names_prefix_end() {
        let wire = [1u8, 2, 3, 4, 5];
        let short = [1u8, 2, 3];
        let m = identity_assert(&wire, Tier::Arrow, &short).unwrap_err();
        assert_eq!(m.tier, Tier::Arrow);
        assert_eq!(m.offset, 3);
        assert_eq!(m.disk, 0);
    }

    #[test]
    fn display_prints_the_doc_fail_line_shape() {
        // §8.2: `IDENTITY_COURT_FAIL tier=<t> offset=<i> wire=0x.. mem=0x.. disk=0x..`
        let m = TierMismatch {
            tier: Tier::Memory,
            offset: 8,
            wire: 0xAB,
            mem: 0xCD,
            disk: 0x00,
        };
        assert_eq!(
            m.to_string(),
            "IDENTITY_COURT_FAIL tier=memory offset=8 wire=0xab mem=0xcd disk=0x00"
        );
    }
}
