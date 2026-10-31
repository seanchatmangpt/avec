//! RLE codec court (wave-4 lane 8): round-trip byte-equality across every
//! supported bit width on random + adversarial corpora, and the regression
//! court for the bits>8 shift-overflow panic class (witnessed at bits=12).
//!
//! Chicago discipline: real encoder, real decoder, assert final state
//! (decoded lanes and/or emitted wire bytes). No mocks, no interaction
//! assertions.

use avec_parquet::rle::{decode_into, rle_decode, rle_encode, ParquetError};
use avec_parquet::ParquetError as ReExportedParquetError;

/// Deterministic LCG — stable vectors, no external rng dependency.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

const KERNEL_WIDTHS: [u8; 4] = [1, 2, 4, 8];

fn round_trip(values: &[u32], bits: u8) {
    let page = rle_encode(values, bits);
    let mut out = vec![0xDEAD_BEEFu32; values.len()];
    rle_decode(&page, &mut out).expect("decode");
    assert_eq!(out, values, "bits={bits}");
}

/// Every supported width round-trips a mixed corpus: literal stretches,
/// short runs, >=3 runs, alternating lanes, max-value runs, all-ones runs.
#[test]
fn every_kernel_width_round_trips_random_and_adversarial() {
    for &bits in &KERNEL_WIDTHS {
        let alpha = 1u32 << bits;
        let max = alpha - 1;

        // Adversarial: all-max runs (values hit the width's ceiling).
        round_trip(&vec![max; 64], bits);
        // All-ones runs (value 1, not just 0/max).
        round_trip(&[1; 33], bits);
        // Alternating two values (no runs >= 3 anywhere).
        let alternating: Vec<u32> = (0..97).map(|i| (i & 1) as u32).collect();
        round_trip(&alternating, bits);
        // Alternating max/min.
        let alt_max: Vec<u32> = (0..80).map(|i| if i & 1 == 0 { max } else { 0 }).collect();
        round_trip(&alt_max, bits);
        // Runs of 1s embedded between literals and max-runs.
        let mut mixed: Vec<u32> = Vec::new();
        for i in 0..24u32 {
            mixed.push(i % max.max(1));
            mixed.push(1);
            mixed.push(1);
        }
        mixed.extend(vec![max; 7]);
        round_trip(&mixed, bits);

        // Random corpora (LCG, bounded to the width's alphabet).
        for seed in 0..8u64 {
            let mut s = Lcg(seed ^ 0x5EED);
            let n = 1 + (s.next() % 512) as usize;
            let values: Vec<u32> = (0..n).map(|_| (s.next() as u32) % alpha).collect();
            round_trip(&values, bits);
        }
    }
}

/// Literal-arm exhaustive: every lane value of every supported width, in one
/// literal batch (odd lane counts force a zero-padded partial final group).
#[test]
fn exhaustive_lane_values_round_trip() {
    for &bits in &KERNEL_WIDTHS {
        let alpha = 1u32 << bits;
        let values: Vec<u32> = (0..alpha).collect();
        round_trip(&values, bits);
        // Same alphabet split across many batches with interleaved runs.
        let mut values2: Vec<u32> = (0..alpha).collect();
        values2.extend((0..alpha).map(|v| alpha - 1 - v));
        values2.push(0);
        values2.push(0);
        values2.push(0);
        round_trip(&values2, bits);
    }
}

/// Regression: the bits=12 overflow class. Before the fix, the literal arm
/// shifted past the u8 accumulator (`byte |= bit_val << bit` with bit >= 8)
/// and panicked mid-loop for widths 9..=32; the run arm sliced
/// `to_le_bytes()[..vb]` past 4 for bits >= 33. Both classes are now refused
/// loudly at the contract boundary, for every out-of-domain width.
#[test]
fn out_of_domain_widths_refused_loudly_not_overflow_panics() {
    let mixed = vec![7u32, 7, 7, 1, 2, 3, 4, 5, 6, 7, 8];
    let runs = vec![9u32; 10];
    // Widths 3,5,6,7 are inside the encoder's byte-lane domain (1..=8) and
    // keep encoding; every width outside 1..=8 refuses.
    for bits in [0u8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 31, 32, 64, 255] {
        let msg = std::panic::catch_unwind(|| rle_encode(&mixed, bits))
            .err()
            .and_then(|a| a.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        assert!(
            msg.contains("UnsupportedBitWidth"),
            "bits={bits}: expected the loud domain refusal, got panic message: {msg:?}"
        );
        // Run-only inputs hit the run arm's slice panic class (bits >= 33)
        // and must refuse identically.
        let msg_run = std::panic::catch_unwind(|| rle_encode(&runs, bits))
            .err()
            .and_then(|a| a.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        assert!(
            msg_run.contains("UnsupportedBitWidth"),
            "bits={bits} (run-only): expected the loud domain refusal, got: {msg_run:?}"
        );
    }
}

/// The witnessed defect input class end-to-end: encode at bits=12 must be a
/// loud refusal (never the mid-loop shift-overflow), and the kernel widths
/// around it keep encoding.
#[test]
fn bits12_witness_class_regression() {
    let values: Vec<u32> = (0..64).map(|i| (i * 37 + 11) & 0xFFF).collect();
    let msg = std::panic::catch_unwind(|| rle_encode(&values, 12))
        .err()
        .and_then(|a| a.downcast_ref::<String>().cloned())
        .unwrap_or_default();
    assert!(
        msg.contains("UnsupportedBitWidth(12)"),
        "witnessed bits=12 class must refuse with UnsupportedBitWidth(12), got: {msg:?}"
    );
    // Adjacent in-kernel widths still encode + round-trip the same values
    // (masked into their alphabets).
    {
        let bits = 8u8;
        let masked: Vec<u32> = values.iter().map(|&v| v & 0xFF).collect();
        round_trip(&masked, bits);
    }
}

/// The decoder's typed refusal is unchanged: out-of-kernel headers fail
/// closed (this is WHY the encoder must not mint such pages).
#[test]
fn decoder_still_refuses_out_of_kernel_headers() {
    for bits in [0u8, 3, 5, 6, 7, 12, 16] {
        let mut out = [0u32; 8];
        assert_eq!(
            rle_decode(&[bits, 0], &mut out),
            Err(ParquetError::UnsupportedBitWidth(bits))
        );
    }
}

/// Parity: the fast encoder (which delegates out-of-domain widths to the
/// baseline) refuses the same widths, and produces byte-identical pages on
/// the supported widths.
#[test]
fn fast_encoder_parity_with_baseline() {
    use avec_parquet::encode_fast::rle_encode_fast;

    for &bits in &KERNEL_WIDTHS {
        let mut s = Lcg(0xC0FFEE ^ u64::from(bits));
        let alpha = 1u32 << bits;
        let values: Vec<u32> = (0..300).map(|_| (s.next() as u32) % alpha).collect();
        assert_eq!(
            rle_encode_fast(&values, bits),
            rle_encode(&values, bits),
            "bits={bits}"
        );
    }
    for bits in [0u8, 9, 12, 16] {
        let values = vec![1u32, 2, 3, 4];
        let refused = std::panic::catch_unwind(|| rle_encode_fast(&values, bits)).is_err();
        assert!(refused, "bits={bits}: fast path must refuse too (by delegation)");
    }
}

/// `decode_into` streaming contract still holds alongside the encoder fix:
/// partial page reports lanes written, tail untouched.
#[test]
fn decode_into_contract_unaffected() {
    let values: Vec<u32> = (0..16u32).collect();
    let page = rle_encode(&values, 4);
    let mut out = vec![0xC0FFEEu32; 40];
    let n = decode_into(&page, &mut out).expect("decode_into");
    assert_eq!(n, 16);
    assert_eq!(&out[..16], &values[..]);
    assert!(out[16..].iter().all(|&x| x == 0xC0FFEE));
}

/// Re-export parity: `avec_parquet::ParquetError` is the same type the
/// rle module returns.
#[test]
fn parquet_error_reexport_is_same_type() {
    let e = ParquetError::UnsupportedBitWidth(12);
    let r: ReExportedParquetError = e;
    assert_eq!(r, ReExportedParquetError::UnsupportedBitWidth(12));
}
