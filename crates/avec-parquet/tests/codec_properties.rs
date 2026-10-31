//! Codec property court (wave-14 lane 10, recovery): LCG-seeded deterministic
//! property loops over the avec-parquet codec surface. No external rng /
//! property crate — every corpus is a pure function of its index, so any
//! failure is replayable bit-exactly from the failing case number printed in
//! the assert message.
//!
//! Chicago discipline: real encoder, real decoder, real page writer/reader,
//! real Quad64 layout — assert final state (decoded lanes, wire bytes, parsed
//! payload). No mocks, no interaction assertions.
//!
//! Properties and case counts:
//! 1. RLE round-trip identity, widths 1..=8 x 500 corpora = 4,000 cases.
//!    Identity asserted on the decoder kernel set {1,2,4,8}; widths {3,5,6,7}
//!    are encode-admissible but outside the decoder kernel set, so the
//!    round-trip contract there is the typed `UnsupportedBitWidth` refusal
//!    (never silent corruption).
//! 2. Constant-run decode identity, L = 1..=10,000 = 10,000 cases. Every
//!    literal-group boundary (multiples of 8) and the run threshold 3 are
//!    enumerated by construction; the run-arm engagement is additionally
//!    pinned by a wire-length bound (a literal fallback would be ~L bytes).
//! 3. Level-free page round-trip, rows {0,1,2,3,255,256,257,65535,65536} x
//!    {U32,U64,Bytes32} = 27 page cases. The contract's cross product IS the
//!    full population, so it is exhaustively enumerated (no sampling is
//!    possible); 395,535 per-row byte-equality assertions inside compose the
//!    payload-identity proof row by row.
//! 4. Quad64 to_le_bytes -> from_le_bytes identity, 5,000 LCG quads,
//!    sentinel/max-heavy (BOTTOM(0)/WILDCARD(u32::MAX) terms, max u64/u32
//!    fields, all-zero and all-FF digests, the ZERO quad itself).
//!
//! Wiring disclosure (wave-14 lane 10): the avec-arrow dev-dependency this
//! file requires is declared in crates/avec-parquet/Cargo.toml, mirroring the
//! avec-ffi precedent; avec-arrow is a zero-dependency workspace member, so
//! no external dependency is introduced.

use avec_arrow::layout::{Quad64, BOTTOM_TERM_ID, WILDCARD_TERM_ID};
use avec_parquet::level_free::{read_page_payload, write_page, LevelFreeType};
use avec_parquet::rle::{rle_decode, rle_encode, ParquetError};

/// Deterministic LCG — stable vectors, no external rng dependency (same
/// recurrence as tests/rle_court.rs).
struct Lcg(u64);
impl Lcg {
    const fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

/// Decoder kernel set: the widths the SWAR decode path admits.
const KERNEL_WIDTHS: [u8; 4] = [1, 2, 4, 8];
/// Encoder-admissible widths: the wire packs one byte per lane, 1..=8.
const ENCODE_WIDTHS: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// Corpus `c` at width `bits`: LCG-chosen mix of literal stretches and
/// >=3-runs, length cycling through the group-boundary ladder (multiples of
/// 8 and their +-1 neighbors, the run threshold, and page-scale lengths).
/// Values are masked into `0..2^bits` — the width's representable alphabet,
/// the domain over which round-trip identity is the contract.
fn corpus(c: usize, bits: u8) -> Vec<u32> {
    const LENGTHS: [usize; 20] = [
        1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 63, 64, 65, 127, 128, 255, 256, 257, 1024,
    ];
    let len = LENGTHS[c % LENGTHS.len()];
    let mask = (1u32 << bits) - 1;
    // The run/literal skeleton is a function of the corpus index ONLY, so
    // corpus c exercises the same arm structure at every width (differential
    // across widths); values are a function of (c, bits).
    let mut skel = Lcg::new(0xC0FF_EE00 ^ c as u64);
    let mut vals = Lcg::new(
        0x51CE_0000
            ^ (c as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (u64::from(bits) << 32),
    );
    let mut v = Vec::with_capacity(len);
    while v.len() < len {
        let r = skel.next();
        if r & 3 == 0 {
            // A >=3 equal stretch: the encoder's repeated-run arm.
            let run = 3usize + ((r >> 8) % 48) as usize;
            let val = vals.next() as u32 & mask;
            let take = run.min(len - v.len());
            for _ in 0..take {
                v.push(val);
            }
        } else {
            v.push(vals.next() as u32 & mask);
        }
    }
    v
}

/// Property 1 — RLE round-trip identity at every encoder-admissible width.
/// 8 widths x 500 corpora = 4,000 cases; 2,000 identity round-trips (kernel
/// widths) + 2,000 typed-refusal cases (encode-only widths).
#[test]
fn prop1_rle_round_trip_widths_1_to_8_x_500_corpora() {
    let mut cases = 0usize;
    let mut identities = 0usize;
    let mut refusals = 0usize;
    for &bits in &ENCODE_WIDTHS {
        for c in 0..500usize {
            let values = corpus(c, bits);
            let page = rle_encode(&values, bits); // encoder admits 1..=8
            let mut out = vec![0xDEAD_BEEFu32; values.len()];
            if KERNEL_WIDTHS.contains(&bits) {
                rle_decode(&page, &mut out).expect("kernel-width decode");
                assert_eq!(
                    out, values,
                    "P1 identity: bits={bits} corpus={c} len={}",
                    values.len()
                );
                identities += 1;
            } else {
                assert_eq!(
                    rle_decode(&page, &mut out),
                    Err(ParquetError::UnsupportedBitWidth(bits)),
                    "P1 refusal: bits={bits} corpus={c} — decode must refuse \
                     non-kernel widths loudly, never decode them"
                );
                refusals += 1;
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 4_000, "case census");
    assert_eq!(identities, 2_000, "identity census");
    assert_eq!(refusals, 2_000, "refusal census");
    println!("[P1] cases={cases} identities={identities} refusals={refusals}");
}

/// Property 2 — a constant run of every length L in 1..=10,000 decodes to
/// exactly L copies of the value. 10,000 cases; L=1..=10000 enumerates every
/// literal-group boundary (multiple of 8) and the >=3 run threshold.
#[test]
fn prop2_constant_run_decode_l_1_to_10000() {
    let mut rng = Lcg::new(0x5EED_5EED_5EED_5EED);
    for l in 1..=10_000usize {
        // Value mix: guaranteed zero-run and max-run cases every 64 lengths,
        // LCG value elsewhere (bits=8: the full byte alphabet).
        let v = if l % 64 == 0 {
            255u32
        } else if l % 64 == 1 {
            0u32
        } else {
            (rng.next() & 0xFF) as u32
        };
        let values = vec![v; l];
        let page = rle_encode(&values, 8);
        // Wire-shape law: L>=3 must engage the repeated-run arm — one width
        // byte, a varint header (L<<1 <= 20000 -> <= 3 bytes), one value byte.
        // A literal fallback would cost ~L bytes.
        if l >= 3 {
            assert!(
                page.len() <= 8,
                "P2 run-arm: L={l} page={} bytes — literal fallback suspected",
                page.len()
            );
        } else {
            assert!(
                page.len() <= 10,
                "P2 literal-arm: L={l} page={} bytes",
                page.len()
            );
        }
        let mut out = vec![0x5A5A_5A5Au32; l];
        rle_decode(&page, &mut out).expect("P2 decode");
        assert_eq!(out, values, "P2 identity: L={l} value={v:#x}");
    }
    println!("[P2] cases=10000 (L=1..=10000, boundaries enumerated)");
}

/// Property 3 — level-free page round-trip: write_page then read_page_payload
/// returns the identical payload bytes and row count, across the full
/// rows x type cross product. 27 page cases, 395,535 per-row byte assertions.
#[test]
fn prop3_level_free_page_round_trip_rows_x_types() {
    const ROWS_SET: [usize; 9] = [0, 1, 2, 3, 255, 256, 257, 65535, 65536];
    const TYPES: [LevelFreeType; 3] = [LevelFreeType::U32, LevelFreeType::U64, LevelFreeType::Bytes32];
    let mut page_cases = 0usize;
    let mut row_cases = 0usize;
    for &ty in &TYPES {
        let w = ty.width();
        for &rows in &ROWS_SET {
            let mut payload = vec![0u8; rows * w];
            let mut rng = Lcg::new(0xABCD_1234 ^ (rows as u64) ^ ((w as u64) << 32));
            for b in payload.iter_mut() {
                *b = rng.next() as u8;
            }
            let page = write_page(ty, &payload, rows).expect("write_page");
            let (got, got_rows) = read_page_payload(&page, ty).expect("read_page_payload");
            assert_eq!(got_rows, rows, "P3 rows: ty={ty:?} rows={rows}");
            // Row-by-row byte identity composes the payload-identity proof;
            // each row is one counted case.
            for r in 0..rows {
                assert_eq!(
                    &got[r * w..(r + 1) * w],
                    &payload[r * w..(r + 1) * w],
                    "P3 row bytes: ty={ty:?} rows={rows} row={r}"
                );
            }
            // Summary equality over the whole payload (composition check).
            assert_eq!(got, &payload[..], "P3 payload: ty={ty:?} rows={rows}");
            page_cases += 1;
            row_cases += rows;
        }
    }
    assert_eq!(page_cases, 27, "page-case census");
    assert_eq!(row_cases, 395_535, "row-case census");
    println!("[P3] page_cases={page_cases} row_cases={row_cases}");
}

/// Property 4 — Quad64 wire identity: from_le_bytes(to_le_bytes(q)) == q for
/// 5,000 LCG quads, sentinel/max-heavy. Case 0 is the ZERO quad itself
/// (all-BOTTOM terms, zero digest); every 5th case is sentinel-heavy
/// (BOTTOM/WILDCARD terms), every 7th (non-sentinel) case is max-heavy
/// (all-max fields, all-FF digest); the rest are full-range LCG fields.
#[test]
fn prop4_quad64_lcg_wire_identity_5000() {
    let mut rng = Lcg::new(0x0A11_CE5EED_0001);
    for c in 0..5_000usize {
        let q = if c == 0 {
            Quad64::ZERO
        } else if c % 5 == 0 {
            // Sentinel-heavy: term ids drawn from {BOTTOM, WILDCARD}.
            let bits = rng.next();
            let pick = |i: u32| if (bits >> i) & 1 == 0 { BOTTOM_TERM_ID } else { WILDCARD_TERM_ID };
            Quad64::new(
                pick(0),
                pick(1),
                pick(2),
                pick(3),
                if bits & 0x100 == 0 { 0 } else { u64::MAX },
                if bits & 0x200 == 0 { 0 } else { u32::MAX },
                if bits & 0x400 == 0 { 0 } else { u32::MAX },
                if bits & 0x800 == 0 { [0u8; 32] } else { [0xFFu8; 32] },
            )
        } else if c % 7 == 0 {
            // Max-heavy: every scalar field at its ceiling, all-FF digest.
            Quad64::new(
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u32::MAX,
                u64::MAX,
                u32::MAX,
                u32::MAX,
                [0xFFu8; 32],
            )
        } else {
            // Full-range LCG fields.
            let mut digest = [0u8; 32];
            for b in digest.iter_mut() {
                *b = rng.next() as u8;
            }
            Quad64::new(
                (rng.next() >> 32) as u32,
                rng.next() as u32,
                (rng.next() >> 32) as u32,
                rng.next() as u32,
                rng.next(),
                rng.next() as u32,
                rng.next() as u32,
                digest,
            )
        };
        let bytes = q.to_le_bytes();
        assert_eq!(bytes.len(), 64, "P4 wire length: case={c}");
        let back = Quad64::from_le_bytes(&bytes).expect("P4 parse: 64 bytes");
        assert_eq!(back, q, "P4 identity: case={c}");
    }
    println!("[P4] cases=5000 (case 0 = ZERO; 1/5 sentinel-heavy; 1/7 max-heavy)");
}
