//! Oblivious (fixed-access-pattern) kernels for two of the timing-leaking
//! surfaces the data-independence register names
//! (`docs/DATA_INDEPENDENCE_AUDIT.md`, v26.10.29). Lane HWL5.
//!
//! # Closures claimed by this module
//!
//! - **BOUNDED**: the `decode_page` dictionary-gather leak
//!   (`crates/avec-parquet/src/dictionary.rs:206-214`). [`oblivious_gather_small`]
//!   serves every output lane by a 64-slot broad-side select: all four
//!   dictionary cache lines (64 x 4 B = 256 B) are touched for EVERY lane,
//!   whatever the codes hold. The gather's address stream is
//!   `f(dict_cardinality, lane_count)` — both public page-header framing —
//!   never `f(codes)`. Dictionaries above 64 entries are refused typed (the
//!   honest boundary; see the error-mapping note).
//! - **NEW CAPACITY**: [`oblivious_masked_compact`] — validity-masked
//!   compaction whose READ pattern is fixed (every input row is read every
//!   cycle) and whose store addresses follow the popcount PREFIX only.
//!   This closes no existing register row; it is a new building block that
//!   removes the branch-per-row compaction shape (a `if valid { out[j++] =
//!   v }` loop) wherever a filtered materialization is needed.
//!
//! # What "oblivious" claims here — and what it does not
//!
//! Claimed: ACCESS-PATTERN constancy. Addresses and trip counts are
//! functions of public framing alone (slice lengths, dictionary
//! cardinality). A co-tenant timing the kernel learns lengths it already
//! owns (they are arguments), never secret lane values, codes, or validity
//! bits through the address stream.
//!
//! NOT claimed: cycle constancy. The register's measured `mask_words`
//! finding (dense output slowest 5/5 runs; store-side coupling survives in
//! fixed-stride kernels) applies here with more force: compaction's store
//! ADDRESSES are `f(popcount prefix)`, so dirty-line traffic varies with
//! validity DENSITY. The measured spread table in the audit ADDENDUM is the
//! honest statement of that residual coupling; any cycles-constant claim
//! must defeat it explicitly, per the register's scope note.
//!
//! # Error-mapping honesty note
//!
//! [`ParquetError`] has no variant meaning "dictionary too large for the
//! oblivious kernel" or "code out of domain". This module maps, per the
//! crate's documented precedent (`dictionary.rs` module honesty note):
//!
//! - `dict.len() > 64` → [`ParquetError::UnsupportedBitWidth`] carrying the
//!   TRUE unclamped index width `ceil(log2(dict.len()))` (65 entries → 7,
//!   257 → 9, 1000 → 10), exactly as `required_bits` reports. The width IS
//!   the thing that is wrong: this kernel's code domain is 6 bits (64
//!   entries). Documented boundary, not a lie about the width.
//! - `out.len() != codes.len()` → [`ParquetError::UnexpectedEof`]: the
//!   full-buffer contract's sanctioned mapping (same as `decode_page`'s
//!   wrong-lane-count mapping — no truthful variant exists for "budget
//!   mismatch").
//! - any code `>= dict.len()` → [`ParquetError::UnexpectedEof`] under the
//!   same documented mapping as `decode_page`'s out-of-domain index
//!   ("malformed lane — a code named a slot the dictionary does not
//!   have"). The check is OBLIVIOUS: a branch-free OR-accumulate of
//!   `code >= card` per lane; the single tail branch executes once, its
//!   outcome is already caller-visible through the returned error, so the
//!   timing adds nothing the return value does not publish.
//!
//! On any error path `out` contents are undefined — do not use them (same
//! contract note as `decode_page`).

use crate::rle::ParquetError;

/// Largest dictionary the oblivious gather carries: a 6-bit code domain.
///
/// 64 entries x 4 B = 256 B = 4 cache lines, all touched every lane.
pub const OBLIVIOUS_GATHER_MAX_DICT: usize = 64;

/// Oblivious dictionary gather for small dictionaries (<= 64 entries).
///
/// Semantics: `out[i] = dict[codes[i]]` (positional gather), fail-closed.
///
/// Access pattern: per lane, one sweep of all 64 dictionary slots — slot
/// `j` is read and compared (`j == codes[i]`) unconditionally; the match
/// flag selects the slot value by masked OR-accumulate (`acc |= slot &
/// -flag`). Every dictionary cache line is touched every lane. Trip counts
/// are `64 * codes.len()` — public framing — whatever the codes hold.
/// (Selecting by slot INDEX, not slot value: a value-equality select would
/// compute the inverse map, not the positional gather.)
///
/// Gates (each typed, framing-only branches before any secret is touched):
/// `dict.len() > 64` refuses with the true needed width; length mismatch
/// refuses; any out-of-domain code refuses via the oblivious
/// OR-accumulate. Padding slots (indices >= dict.len()) read 0; an
/// in-domain code never selects them (its index is below `dict.len()`), so
/// the padding value is unobservable on the `Ok` path.
pub fn oblivious_gather_small(
    dict: &[u32],
    codes: &[u8],
    out: &mut [u32],
) -> Result<(), ParquetError> {
    if dict.len() > OBLIVIOUS_GATHER_MAX_DICT {
        // True unclamped width, mirroring `required_bits`' reporting law
        // (dict.len() >= 65 here, so the leading-zeros arithmetic is
        // defined).
        let needed = (usize::BITS - (dict.len() - 1).leading_zeros()) as u8;
        return Err(ParquetError::UnsupportedBitWidth(needed));
    }
    if out.len() != codes.len() {
        // Full-buffer contract, sanctioned mapping (module honesty note).
        return Err(ParquetError::UnexpectedEof);
    }

    let card = dict.len() as u32;
    let mut padded = [0u32; OBLIVIOUS_GATHER_MAX_DICT];
    padded[..dict.len()].copy_from_slice(dict);

    let mut bad: u32 = 0;
    for (lane, o) in codes.iter().zip(out.iter_mut()) {
        let code = u32::from(*lane);
        // Branch-free in-domain accumulate (see honesty note).
        bad |= u32::from(code >= card);
        // Broad-side select: all 64 slots, fixed trip count. The slot's
        // INDEX is compared against the code (positional gather); the
        // match flag selects the slot value into the OR-accumulator.
        let mut acc: u32 = 0;
        for (j, &slot) in padded.iter().enumerate() {
            let m = u32::from(j as u32 == code).wrapping_neg();
            acc |= slot & m;
        }
        *o = acc;
    }
    if bad != 0 {
        // Documented mapping: an out-of-domain code exists. One branch,
        // once; its outcome is published by the return value itself.
        return Err(ParquetError::UnexpectedEof);
    }
    Ok(())
}

/// Oblivious validity-masked compaction: `out[..count]` = the `values` rows
/// whose validity bit is set, in order; returns `count`.
///
/// # Access pattern (what is fixed and what is not)
///
/// - READS: fixed. Every input row is read every cycle (`values[...]`
///   unconditional per bit); validity words are swept word-by-word,
///   bit-by-bit at fixed strides. Trip counts are `f(values.len())` —
///   public framing.
/// - STORES: address = cross-word running popcount PREFIX (`j`) — the
///   per-word popcount prefix accumulation. The address depends on the
///   popcount PREFIX (how many valid rows came before), never on WHICH
///   rows are valid: two validity patterns with identical prefixes produce
///   identical store-address streams. What this does NOT mean: the store
///   COUNT and the returned value scale with validity DENSITY, so
///   dirty-line traffic and the return still publish density (and the
///   coarse running prefix), never the validity bit pattern itself.
///
/// # Budget contract (loud, framing-only, house precedent)
///
/// `out.len() >= values.len()` is the caller's obligation; a short buffer
/// fails LOUD (`assert`, "out budget too small") exactly as
/// `packed_filter::filter_eq_packed` refuses a short mask buffer. The
/// dense budget is what makes the fixed store pattern realizable
/// branch-free: with `j <= count <= out.len()` and the store always
/// landing at a live address, no data-dependent bounds branch is needed.
/// Slots at and beyond the returned count hold TRANSIENT OVERWRITE
/// garbage (the fixed store stream keeps writing scratch rows) — only
/// `out[..count]` is meaningful. This is the price of the fixed store
/// pattern; a compaction that leaves excess lanes untouched must branch on
/// the validity bit, which is the leak this kernel removes.
///
/// # Shape semantics
///
/// Validity words are LSB-first, word `w` covering rows `[64w, 64w+64)`.
/// Bits past `values.len()` in the last word are ignored (padding is not
/// rows). Words beyond `ceil(values.len()/64)` are ignored. MISSING words
/// mean "all rows in that word invalid" — a defined total semantic, not a
/// silent failure: the caller can recompute the count as the popcount of
/// the words it passed.
#[must_use]
pub fn oblivious_masked_compact(values: &[u32], valid: &[u64], out: &mut [u32]) -> usize {
    let n = values.len();
    assert!(
        out.len() >= n,
        "oblivious_masked_compact: out budget too small: need >= {n} (dense output), got {}",
        out.len()
    );

    let full_words = n / 64;
    let tail_rows = n % 64;
    let mut j = 0usize;
    for w in 0..full_words {
        let word = if w < valid.len() { valid[w] } else { 0 };
        for b in 0..64 {
            let m = ((word >> b) & 1) as usize;
            out[j] = values[w * 64 + b];
            j += m;
        }
    }
    if tail_rows != 0 {
        let word = (if full_words < valid.len() {
            valid[full_words]
        } else {
            0
        }) & ((1u64 << tail_rows) - 1);
        for b in 0..tail_rows {
            let m = ((word >> b) & 1) as usize;
            out[j] = values[full_words * 64 + b];
            j += m;
        }
    }
    j
}

#[cfg(test)]
mod tests {
    use super::oblivious_gather_small;
    use super::oblivious_masked_compact;
    use super::ParquetError as PE;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Deterministic LCG (house test-vector shape, as packed_filter).
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

    /// Leaky positional-gather reference (shares no code with the kernel).
    fn reference_gather(dict: &[u32], codes: &[u8]) -> Vec<u32> {
        codes.iter().map(|&c| dict[c as usize]).collect()
    }

    /// Leaky branch-per-row compaction reference.
    fn reference_compact(values: &[u32], valid: &[u64], n: usize) -> Vec<u32> {
        let mut out = Vec::new();
        for i in 0..n {
            let bit = (valid[i / 64] >> (i % 64)) & 1;
            if bit == 1 {
                out.push(values[i]);
            }
        }
        out
    }

    /// Pack per-row validity bits into LSB-first words.
    fn pack_valid(bits: &[bool]) -> Vec<u64> {
        let mut words = alloc::vec![0u64; bits.len().div_ceil(64)];
        for (i, &b) in bits.iter().enumerate() {
            if b {
                words[i / 64] |= 1u64 << (i % 64);
            }
        }
        words
    }

    // -------------------------------------------------------------------
    // Gather: differential vs the leaky reference.
    // -------------------------------------------------------------------

    /// Grid: dict sizes x code-stream lengths x LCG codes. Full output
    /// equality against the reference.
    #[test]
    fn gather_differential_grid() {
        let sizes = [0usize, 1, 63, 64, 1000, 1_000_000];
        let dict_sizes = [1usize, 2, 3, 5, 16, 17, 32, 63, 64];
        for &card in &dict_sizes {
            let dict: Vec<u32> = (0..card).map(|i| 1_000 + i as u32 * 37).collect();
            for &n in &sizes {
                let mut rng = Lcg(0x5EED_0000 + card as u64 * 7919 + n as u64);
                let codes: Vec<u8> = (0..n).map(|_| (rng.next() % card as u64) as u8).collect();
                let mut out = alloc::vec![0xAAAA_u32; n];
                oblivious_gather_small(&dict, &codes, &mut out)
                    .unwrap_or_else(|e| panic!("gather card={card} n={n}: {e:?}"));
                assert_eq!(
                    out,
                    reference_gather(&dict, &codes),
                    "differential card={card} n={n}"
                );
            }
        }
    }

    /// Adversarial code shapes: all-first, all-last, alternating,
    /// clustered blocks, single-position spikes, at the boundary cards
    /// 1, 2, 64.
    #[test]
    fn gather_adversarial_shapes() {
        let shapes_card = [1usize, 2, 64];
        for &card in &shapes_card {
            let dict: Vec<u32> = (0..card).map(|i| 7_000 + i as u32 * 13).collect();
            for &n in &[0usize, 1, 63, 64, 1000] {
                let mut rng = Lcg(0xC0DE_0000 + card as u64 * 31 + n as u64);
                // all-first / all-last
                for &code in &[0u8, (card - 1) as u8] {
                    let codes = alloc::vec![code; n];
                    let mut out = alloc::vec![0u32; n];
                    oblivious_gather_small(&dict, &codes, &mut out).expect("in-domain");
                    assert_eq!(out, reference_gather(&dict, &codes));
                }
                // alternating 0 / card-1
                let codes: Vec<u8> = (0..n)
                    .map(|i| if i & 1 == 0 { 0 } else { (card - 1) as u8 })
                    .collect();
                let mut out = alloc::vec![0u32; n];
                oblivious_gather_small(&dict, &codes, &mut out).expect("in-domain");
                assert_eq!(out, reference_gather(&dict, &codes));
                // clustered blocks
                let mut codes: Vec<u8> = Vec::with_capacity(n);
                while codes.len() < n {
                    let block = 64 + (rng.next() % 128) as usize;
                    let v = (rng.next() % card as u64) as u8;
                    let end = (codes.len() + block).min(n);
                    while codes.len() < end {
                        codes.push(v);
                    }
                }
                let mut out = alloc::vec![0u32; n];
                oblivious_gather_small(&dict, &codes, &mut out).expect("in-domain");
                assert_eq!(out, reference_gather(&dict, &codes));
                // single-position spike: one foreign code in a flat stream
                if n > 0 {
                    let mut codes = alloc::vec![0u8; n];
                    codes[n / 2] = (card - 1) as u8;
                    let mut out = alloc::vec![0u32; n];
                    oblivious_gather_small(&dict, &codes, &mut out).expect("in-domain");
                    assert_eq!(out, reference_gather(&dict, &codes));
                }
            }
        }
    }

    /// Gates: > 64 dict refuses with the TRUE needed width; budget
    /// mismatch; out-of-domain codes; empty-dict edges.
    #[test]
    fn gather_gates_fail_typed() {
        let mut out = [0u32; 4];
        // True unclamped width law: 65 -> 7, 128 -> 7, 257 -> 9, 1000 -> 10.
        for (card, needed) in [(65usize, 7u8), (128, 7), (257, 9), (1000, 10)] {
            let dict: Vec<u32> = (0..card as u32).collect();
            assert_eq!(
                oblivious_gather_small(&dict, &[0u8; 4], &mut out),
                Err(PE::UnsupportedBitWidth(needed)),
                "dict card {card} must refuse with width {needed}"
            );
        }
        // Budget mismatch (both directions of the full-buffer contract).
        let dict = [10u32, 20, 30];
        assert_eq!(
            oblivious_gather_small(&dict, &[0u8, 1], &mut [0u32; 3]),
            Err(PE::UnexpectedEof),
            "out shorter than codes must refuse"
        );
        assert_eq!(
            oblivious_gather_small(&dict, &[0u8, 1, 2], &mut [0u32; 5]),
            Err(PE::UnexpectedEof),
            "out longer than codes must refuse"
        );
        // Out-of-domain codes: aggregate refusal, type unchanged.
        let dict2 = [100u32, 200];
        assert_eq!(
            oblivious_gather_small(&dict2, &[0u8, 1, 2], &mut [0u32; 3]),
            Err(PE::UnexpectedEof),
            "code 2 >= card 2"
        );
        assert_eq!(
            oblivious_gather_small(&[5u32], &[u8::MAX], &mut [0u32; 1]),
            Err(PE::UnexpectedEof),
            "code 255 vs card 1"
        );
        // Empty dict: any code is out of domain.
        let empty: [u32; 0] = [];
        assert_eq!(
            oblivious_gather_small(&empty, &[0u8], &mut [0u32; 1]),
            Err(PE::UnexpectedEof)
        );
        // Empty everything: Ok, nothing written.
        assert_eq!(oblivious_gather_small(&empty, &[], &mut []), Ok(()));
    }

    // -------------------------------------------------------------------
    // Compact: differential vs the leaky reference.
    // -------------------------------------------------------------------

    /// Grid: row counts x validity patterns (the task's adversarial set:
    /// all-valid, none, alternating, single, clustered, plus density
    /// sweeps). Asserts the full out-prefix equality and the count.
    #[test]
    fn compact_differential_grid() {
        let sizes = [0usize, 1, 63, 64, 1000, 1_000_000];
        for &n in &sizes {
            let mut rng = Lcg(0x0C0C_04C7 + n as u64);
            let values: Vec<u32> = (0..n).map(|_| rng.next() as u32).collect();

            let bits_all: Vec<bool> = alloc::vec![true; n];
            let bits_none: Vec<bool> = alloc::vec![false; n];
            let bits_alt: Vec<bool> = (0..n).map(|i| i & 1 == 0).collect();
            let mut singles: Vec<Vec<bool>> = Vec::new();
            if n > 0 {
                for pos in [0usize, n / 2, n - 1] {
                    let mut b = alloc::vec![false; n];
                    b[pos] = true;
                    singles.push(b);
                }
            }
            let mut bits_clu: Vec<bool> = Vec::with_capacity(n);
            while bits_clu.len() < n {
                let block = 64 + (rng.next() % 128) as usize;
                let v = rng.next().is_multiple_of(2);
                let end = (bits_clu.len() + block).min(n);
                while bits_clu.len() < end {
                    bits_clu.push(v);
                }
            }
            let mut dens: Vec<Vec<bool>> = Vec::new();
            for p in [10u64, 50, 90] {
                dens.push((0..n).map(|_| rng.next() % 100 < p).collect());
            }

            let mut cases: Vec<(alloc::string::String, Vec<bool>)> = vec![
                ("all".to_string(), bits_all),
                ("none".to_string(), bits_none),
                ("alternating".to_string(), bits_alt),
            ];
            for (i, d) in dens.into_iter().enumerate() {
                cases.push((alloc::format!("p{}", [10, 50, 90][i]), d));
            }
            cases.push(("clustered".to_string(), bits_clu));
            for (i, s) in singles.into_iter().enumerate() {
                cases.push((alloc::format!("single{i}"), s));
            }

            for (name, bits) in &cases {
                let valid = pack_valid(bits);
                let mut out = alloc::vec![0xDEAD_u32; n];
                let got = oblivious_masked_compact(&values, &valid, &mut out);
                let want = reference_compact(&values, &valid, n);
                assert_eq!(got, want.len(), "count n={n} pattern={name}");
                assert_eq!(&out[..want.len()], &want[..], "rows n={n} pattern={name}");
            }
        }
    }

    /// Budget contract: a short `out` fails LOUD (house precedent,
    /// packed_filter's "mask buffer too small").
    #[test]
    #[should_panic(expected = "out budget too small")]
    fn compact_short_out_loud() {
        let values = [1u32, 2, 3, 4, 5, 6];
        let valid = [0b0010_0101u64]; // count 3, but out < values.len()
        let mut out = [0x7E7Eu32; 2];
        let _ = oblivious_masked_compact(&values, &valid, &mut out);
    }

    /// Exact semantics on a dense buffer: prefix equals the reference,
    /// count exact; tail slots are scratch (documented).
    #[test]
    fn compact_dense_prefix_exact() {
        let values = [1u32, 2, 3, 4, 5, 6];
        let valid = [0b0010_0101u64]; // rows 0, 2, 5 -> count 3
        let mut out = [0u32; 6];
        let got = oblivious_masked_compact(&values, &valid, &mut out);
        assert_eq!(got, 3);
        assert_eq!(&out[..3], &[1, 3, 6]);

        // Empty input: count 0, nothing required of out.
        let got0 = oblivious_masked_compact(&[], &[], &mut []);
        assert_eq!(got0, 0);
    }

    /// Missing validity words mean all-invalid rows; tail bits past `n`
    /// are ignored (padding is not rows); excess words ignored.
    #[test]
    fn compact_shape_semantics() {
        // n = 63: bit 63 of word 0 is padding and must be ignored even set.
        let values: Vec<u32> = (0..63).collect();
        let valid = [u64::MAX; 2]; // excess word 1 ignored too
        let mut out = alloc::vec![0u32; 63];
        let got = oblivious_masked_compact(&values, &valid, &mut out);
        assert_eq!(got, 63, "padding bit 63 not counted");
        assert_eq!(out, values);

        // n = 100, valid has only word 0 (rows 0..64): rows 64..100 invalid.
        let values: Vec<u32> = (0..100).collect();
        let valid = [u64::MAX];
        let mut out = alloc::vec![0u32; 100];
        let got = oblivious_masked_compact(&values, &valid, &mut out);
        assert_eq!(got, 64);
        assert_eq!(&out[..64], &values[..64]);
    }
}
