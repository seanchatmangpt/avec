//! Compiled specialization manifest: monomorphization-vs-bloat made
//! mechanical.
//!
//! Law: [`SPECIALIZATION_MANIFEST`] lists exactly the (width, encoding)
//! kernel pairs the avec-pack ontology models via `av:BufferSlot` /
//! `av:elementBitWidth`
//! (`/Users/sac/ggen-marketplace/packs/avec-pack/ontology.ttl`, read
//! 2026-10-29, wave HWL4). Distinct modeled widths: 1, 8, 16, 32, 64, 128,
//! 256 (one validity-bitmap slot at width 1 plus data/offset/view slots at
//! the six data widths).
//!
//! Parity with the graph is mechanical, not maintained by hand: the ggen
//! template `gen-templates/specialization_manifest_rs.tmpl` selects DISTINCT
//! `elementBitWidth` from the same graph and renders the same table; the
//! wave HWL4 render proof diffs that output against this hand-written table
//! (see module test `manifest_widths_match_ontology_register` for the local
//! drift register, and the wave receipt for the `ggen sync run` diff).
//!
//! Encoding derivation (the graph models no encoding property — reported as
//! the optional `av:encoding` ontology extension, coordinator-owned): a
//! width is [`Enc::Rle`] iff the SWAR hybrid codec carries it
//! (`crate::rle::decode_into` accepts widths {1, 2, 4, 8}; ontology widths
//! ∩ that set = {1, 8}); every other modeled width is [`Enc::Plain`]
//! (little-endian byte-packed, W/8 bytes per element, no header byte).
//!
//! Plain lane semantics ([`plain_w`]): one W-bit element is
//! `div_ceil(W, 32)` u32 lanes in little-endian word order — the u32-lane
//! view of the element's bytes. W ≤ 32: the single lane is the element
//! value. W > 32: the lanes are the element's W/32 LE words, lowest word
//! first. No value is truncated: a 256-bit element lands as 8 whole words.

use crate::ParquetError;
use alloc::vec::Vec;

/// Encoding arm of a specialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enc {
    /// SWAR RLE/bit-packed hybrid ([`crate::rle`]); lane width ≤ 8 bits.
    Rle,
    /// Plain little-endian byte-packed elements ([`plain_w`]).
    Plain,
}

/// Kernel shape: decode `src` into `out` as u32 lanes, return the number of
/// lanes written (streaming: may be < `out.len()` at a boundary; typed
/// error on malformed input, never a panic).
pub type Kernel = fn(&[u8], &mut [u32]) -> Result<usize, ParquetError>;

/// One (width, encoding) specialization with its monomorphized kernel.
///
/// `width` is `u16`, not `u8`: the ontology models 256-bit elements
/// (av:Decimal256), which no `u8` can represent. The graph is the law.
pub struct Spec {
    pub width: u16,
    pub encoding: Enc,
    pub kernel: Kernel,
}

/// RLE hybrid decode pinned to width `W` at compile time.
///
/// The page's leading wire byte must equal `W` — a page of any other width
/// is a typed [`ParquetError::UnsupportedBitWidth`], never a silent decode.
/// On match, delegates to [`crate::rle::decode_into`] with its full
/// fail-closed streaming contract (bounded reads, `Ok(n)` lanes written,
/// tail untouched).
pub fn rle_w<const W: u16>(src: &[u8], out: &mut [u32]) -> Result<usize, ParquetError> {
    // Per-monomorphization compile-time domain check (inline const blocks
    // may reference const-generic parameters; inner `const` items may not).
    const {
        assert!(
            matches!(W, 1 | 2 | 4 | 8),
            "rle_w: W must be a SWAR kernel width: 1, 2, 4 or 8"
        )
    };
    match src.first() {
        None => Err(ParquetError::UnexpectedEof),
        Some(&bits) if u16::from(bits) != W => Err(ParquetError::UnsupportedBitWidth(bits)),
        Some(_) => crate::rle::decode_into(src, out),
    }
}

/// Plain little-endian decode into u32 lanes, pinned to width `W` at
/// compile time.
///
/// Wire form: concatenated LE elements of `W/8` bytes each, no header byte.
/// Each complete element becomes `div_ceil(W, 32)` u32 lanes (see the
/// module docs for the lane view).
///
/// Streaming contract (mirrors [`crate::rle::decode_into`]): complete
/// elements only — decode stops at the last element that fits the `out`
/// budget and returns the lanes written; a trailing partial element is
/// ignored (bounded reads, no overread). Zero allocation, zero panics.
pub fn plain_w<const W: u16>(src: &[u8], out: &mut [u32]) -> Result<usize, ParquetError> {
    const {
        assert!(
            W >= 8 && W.is_multiple_of(8) && W <= 256,
            "plain_w: W must be a byte-multiple in 8..=256"
        )
    };
    let eb: usize = W as usize / 8;
    let lpe: usize = (W as usize).div_ceil(32);
    let n_elems = src.len() / eb;
    let mut oi = 0usize;
    let mut e = 0usize;
    while e < n_elems {
        if oi + lpe > out.len() {
            break;
        }
        let mut words = [0u32; 8];
        let base = e * eb;
        for j in 0..eb {
            words[j / 4] |= u32::from(src[base + j]) << ((j % 4) * 8);
        }
        out[oi..oi + lpe].copy_from_slice(&words[..lpe]);
        oi += lpe;
        e += 1;
    }
    Ok(oi)
}

/// Plain little-endian encode: exact inverse of [`plain_w`].
///
/// `lanes` is grouped per element: `div_ceil(W, 32)` lanes per group. For
/// W ≤ 32 each lane is one element value (bits above W are dropped — the
/// same mask-as-you-pack law as [`crate::packing::pack_bits`]); for W > 32
/// each `W/32`-lane group is one element's LE words. A trailing partial
/// group is dropped (complete elements only, matching [`plain_w`]).
pub fn plain_encode<const W: u16>(lanes: &[u32]) -> Vec<u8> {
    const {
        assert!(
            W >= 8 && W.is_multiple_of(8) && W <= 256,
            "plain_encode: W must be a byte-multiple in 8..=256"
        )
    };
    let eb: usize = W as usize / 8;
    let lpe: usize = (W as usize).div_ceil(32);
    let mut out = Vec::with_capacity(lanes.len() / lpe * eb);
    for group in lanes.chunks_exact(lpe) {
        for j in 0..eb {
            out.push((group[j / 4] >> ((j % 4) * 8)) as u8);
        }
    }
    out
}

/// Resolve a kernel from the manifest: exact (width, encoding) match.
///
/// Unmodeled pairs return `None` — dispatch is the manifest's law, not a
/// fallback chain.
pub fn dispatch(width: u16, encoding: Enc) -> Option<Kernel> {
    SPECIALIZATION_MANIFEST
        .iter()
        .find(|s| s.width == width && s.encoding == encoding)
        .map(|s| s.kernel)
}

/// The law, hand-written to the graph:
/// `av:BufferSlot av:elementBitWidth` DISTINCT values in avec-pack
/// ontology.ttl (read 2026-10-29): 1, 8, 16, 32, 64, 128, 256. One entry
/// per modeled width; encoding per the module derivation rule.
///
/// Graph parity is mechanical: `gen-templates/specialization_manifest_rs.tmpl`
/// renders this table from the ontology via SPARQL (wave HWL4 proof), so a
/// width added to the graph changes the rendered table and the drift is
/// caught by diff, not by memory.
pub const SPECIALIZATION_MANIFEST: &[Spec] = &[
    Spec {
        width: 1,
        encoding: Enc::Rle,
        kernel: rle_w::<1>,
    },
    Spec {
        width: 8,
        encoding: Enc::Rle,
        kernel: rle_w::<8>,
    },
    Spec {
        width: 16,
        encoding: Enc::Plain,
        kernel: plain_w::<16>,
    },
    Spec {
        width: 32,
        encoding: Enc::Plain,
        kernel: plain_w::<32>,
    },
    Spec {
        width: 64,
        encoding: Enc::Plain,
        kernel: plain_w::<64>,
    },
    Spec {
        width: 128,
        encoding: Enc::Plain,
        kernel: plain_w::<128>,
    },
    Spec {
        width: 256,
        encoding: Enc::Plain,
        kernel: plain_w::<256>,
    },
];

#[cfg(test)]
mod tests {
    use super::{dispatch, plain_encode, plain_w, rle_w, Enc, SPECIALIZATION_MANIFEST};
    use crate::rle::rle_encode;
    use crate::ParquetError;
    use alloc::vec;
    use alloc::vec::Vec;

    /// The register of ontology-modeled widths this manifest was hand-written
    /// against (2026-10-29). Drift between the graph and this const fails
    /// here; graph-side parity is additionally proved mechanically by the
    /// ggen template render diff (wave HWL4 receipt).
    const EXPECTED_MODELED_WIDTHS: [u16; 7] = [1, 8, 16, 32, 64, 128, 256];

    #[test]
    fn manifest_widths_match_ontology_register() {
        let widths: Vec<u16> = SPECIALIZATION_MANIFEST.iter().map(|s| s.width).collect();
        assert_eq!(
            widths,
            EXPECTED_MODELED_WIDTHS.to_vec(),
            "manifest widths drift from the ontology register"
        );
        assert_eq!(widths.len(), 7);
    }

    #[test]
    fn dispatch_resolves_every_modeled_width() {
        // Every modeled (width, encoding) pair resolves to a kernel. Kernel
        // behavior through dispatch is asserted by the two round-trip courts
        // below (real calls on real fixtures); kernel identity is never
        // compared by fn address (fn addresses are not guaranteed unique).
        for spec in SPECIALIZATION_MANIFEST {
            assert!(
                dispatch(spec.width, spec.encoding).is_some(),
                "dispatch({},{:?}) must resolve for every modeled pair",
                spec.width,
                spec.encoding
            );
        }
        // Codec-supported but ontology-unmodeled pairs dispatch to nothing.
        assert!(
            dispatch(4, Enc::Rle).is_none(),
            "width 4 is not modeled by the graph"
        );
        assert!(dispatch(2, Enc::Plain).is_none());
        assert!(
            dispatch(16, Enc::Rle).is_none(),
            "width 16 exceeds the SWAR lane ceiling"
        );
        assert!(dispatch(0, Enc::Plain).is_none());
    }

    #[test]
    fn rle_kernels_round_trip_tiny_fixture() {
        for w in [1u16, 8] {
            let values: Vec<u32> = (0..19u32).map(|i| i % (1 << w)).collect();
            let page = rle_encode(&values, u8::try_from(w).expect("rle widths fit u8"));
            let mut out = vec![0u32; values.len()];
            let kernel = dispatch(w, Enc::Rle).expect("rle width dispatch");
            let n = kernel(&page, &mut out).expect("rle decode");
            assert_eq!(n, values.len(), "width {w}");
            assert_eq!(out, values, "width {w}");

            // Same call through the named const-generic kernel.
            let mut out2 = vec![0u32; values.len()];
            let got = match w {
                1 => rle_w::<1>(&page, &mut out2),
                _ => rle_w::<8>(&page, &mut out2),
            };
            assert_eq!(got, Ok(values.len()), "width {w}");
            assert_eq!(out2, values, "width {w}");
        }
    }

    #[test]
    fn plain_kernels_round_trip_tiny_fixture() {
        // The five Plain-modeled widths (width 8 is tabled as Rle — the SWAR
        // codec carries it — so dispatch(8, Plain) is None by derivation).
        for w in [16u16, 32, 64, 128, 256] {
            // Per-position distinct lanes within the width's value domain.
            let lanes: Vec<u32> = match w {
                16 => (0..23u32).map(|i| (i * 77 + 3) % 0x1_0000).collect(),
                32 => (0..13u32).map(|i| i * 0x0100_0001 + 7).collect(),
                _ => (0..(5 * u32::from(w) / 32))
                    .map(|i| i.wrapping_mul(0x9E37_79B9) | 1)
                    .collect(),
            };
            let page = match w {
                16 => plain_encode::<16>(&lanes),
                32 => plain_encode::<32>(&lanes),
                64 => plain_encode::<64>(&lanes),
                128 => plain_encode::<128>(&lanes),
                _ => plain_encode::<256>(&lanes),
            };
            let mut out = vec![0u32; lanes.len()];
            let kernel = dispatch(w, Enc::Plain).expect("plain width dispatch");
            let n = kernel(&page, &mut out).expect("plain decode");
            assert_eq!(n, lanes.len(), "width {w}");
            assert_eq!(out, lanes, "width {w}");
        }
    }

    #[test]
    fn plain_w_budget_and_partial_element_streaming() {
        // Three 64-bit elements = 6 u32 lanes.
        let lanes: [u32; 6] = [
            0x0403_0201,
            0x0807_0605,
            0x0C0B_0A09,
            0x100F_0E0D,
            0x1413_1211,
            0x1817_1615,
        ];
        let page = plain_encode::<64>(&lanes);
        assert_eq!(page.len(), 24);

        // Budget for exactly 2 elements: decode stops at the element
        // boundary, tail untouched.
        let mut out = [0xDEAD_BEEFu32; 6];
        let n = plain_w::<64>(&page, &mut out[..4]).expect("decode");
        assert_eq!(n, 4);
        assert_eq!(&out[..4], &lanes[..4]);
        assert_eq!(out[4], 0xDEAD_BEEF);
        assert_eq!(out[5], 0xDEAD_BEEF);

        // Trailing partial element (20 bytes = 2.5 elements): bounded read,
        // complete elements only.
        let n = plain_w::<64>(&page[..20], &mut out).expect("decode");
        assert_eq!(n, 4, "2.5 elements must decode as 2 (4 lanes)");

        // Truncated to less than one element: zero lanes, no panic.
        let n = plain_w::<64>(&page[..4], &mut out).expect("decode");
        assert_eq!(n, 0);
    }

    #[test]
    fn rle_width_pinning_fails_closed() {
        // A codec-legal but ontology-unmodeled width-4 page handed to the
        // width-8 kernel is a typed error, never a silent decode.
        let page4 = rle_encode(&[1, 2, 3, 4, 5, 6, 7, 8], 4);
        let mut out = [0u32; 8];
        assert_eq!(
            rle_w::<8>(&page4, &mut out),
            Err(ParquetError::UnsupportedBitWidth(4))
        );
        // Empty page: typed EOF, not a panic.
        assert_eq!(rle_w::<1>(&[], &mut out), Err(ParquetError::UnexpectedEof));
        // Plain kernels never read a header: empty src is Ok(0) lanes.
        assert_eq!(plain_w::<16>(&[], &mut out), Ok(0));
    }
}
