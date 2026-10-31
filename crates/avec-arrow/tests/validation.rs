//! Arrow-validity cross-checks over the public `avec-arrow` API
//! (lane W3-VALID, file `crates/avec-arrow/tests/validation.rs`).
//!
//! Six laws, each checked against an independent oracle (the builder's own
//! push log), every randomized case driven by a seeded `xorshift64*` so any
//! failure replays bit-exactly:
//!
//! 1. **null_count consistency**: `validity().map_or(0, Bitmap::null_count)`
//!    equals the number of `None` pushes; zero nulls ⇒ validity buffer is
//!    `None` (Arrow: null_count == 0 drops the validity buffer).
//! 2. **null slots carry ZERO bytes**: at every null position the raw bytes
//!    under the typed `values()` slice are all zero, and the typed slot equals
//!    `T::ZERO` (documented determinism: `Scalar::ZERO` slot filler).
//! 3. **Varlen offset ladder**: `offsets()[0] == 0`, `offsets()[N] ==
//!    data().len()`, `offsets()[i+1] - offsets()[i] == pushed row len ==
//!    row(i).len()` for every row; null rows have zero-length slots.
//! 4. **ViewVec descriptor law**: descriptor len field == pushed len ==
//!    `row(i).len()` for inline AND foreign rows; `is_inline` classification
//!    matches; foreign `offset + len <= data().len()`; inline payload prefix
//!    and foreign data-block bytes round-trip.
//! 5. **Bitmap tail-mask law**: `word(w)` has no bits set at or after
//!    `n_bits` for every word (`word(w) & !live_mask == 0`), for n_bits that
//!    are not multiples of 64 included; popcount(live bits) +
//!    `null_count() == n_bits`.
//! 6. **format codes**: all 10 `Scalar` types' `FORMAT_CODE` match the
//!    literal Arrow spec table `["c","C","s","S","i","I","l","L","f","g"]`,
//!    via both `Scalar::FORMAT_CODE` and `Primitive::format_code()`.
//!
//! Chicago discipline: real collaborators only — `alloc` is the only
//! collaborator and it is real; oracles are the builder's own push log, not
//! stubs. Zero unsafe except the raw-byte read under the typed values slice,
//! which is the point of law 2 (bytes, not typed equality alone).

use avec_arrow::bitmap::{count_one_bits, Bitmap};
use avec_arrow::primitive::{Primitive, PrimitiveBuilder, Scalar};
use avec_arrow::varlen::{Offset, VarlenBuilder};
use avec_arrow::views::{is_inline, ViewVecBuilder};

// ---------------------------------------------------------------------------
// seeded xorshift64* — every randomized case replays from one u64 seed
// ---------------------------------------------------------------------------

struct Xs(u64);

impl Xs {
    fn new(seed: u64) -> Self {
        Xs(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish index in `0..n` (panics-free: `n == 0` returns 0).
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.u64() % n as u64) as usize
        }
    }

    fn byte(&mut self) -> u8 {
        (self.u64() & 0xFF) as u8
    }
}

/// Random payload bytes of length `len`.
fn rand_bytes(xs: &mut Xs, len: usize) -> Vec<u8> {
    (0..len).map(|_| xs.byte()).collect()
}

/// The 10 Scalar types as (type, spec format code) pairs.
macro_rules! scalar_types {
    ($m:ident) => {
        $m!(i8, "c");
        $m!(u8, "C");
        $m!(i16, "s");
        $m!(u16, "S");
        $m!(i32, "i");
        $m!(u32, "I");
        $m!(i64, "l");
        $m!(u64, "L");
        $m!(f32, "f");
        $m!(f64, "g");
    };
}

// ---------------------------------------------------------------------------
// law 1 — null_count consistency
// ---------------------------------------------------------------------------

#[test]
fn null_count_matches_pushed_none_count() {
    let mut cases = 0usize;
    macro_rules! one_type {
        ($t:ty, $code:expr) => {
            for k in 0..5u64 {
                // Null regime by phase: 0% / 100% / seeded per-mille.
                let mut xs = Xs::new(0x5EED_0001 ^ (k << 24));
                let n = match k {
                    0 => 0, // empty array edge
                    _ => 1 + xs.below(220),
                };
                let null_permille = match k {
                    0 => 0,     // empty
                    1 => 0,     // zero nulls -> validity buffer dropped
                    2 => 1000,  // all nulls
                    _ => 1 + xs.below(998),
                };
                let mut b = PrimitiveBuilder::<$t>::new();
                let mut pushed_nulls = 0usize;
                for _ in 0..n {
                    if xs.below(1000) < null_permille {
                        b.push(None);
                        pushed_nulls += 1;
                    } else {
                        b.push(Some(xs.u64() as $t));
                    }
                }
                let arr = b.finish();
                assert_eq!(arr.len(), n, "type {} seed {}", $code, k);

                // The law: bitmap popcount oracle == builder push-log oracle.
                let observed = arr.validity().map_or(0, Bitmap::null_count);
                assert_eq!(
                    observed, pushed_nulls,
                    "type {} phase {}: bitmap null_count {} != {} None pushes",
                    $code, k, observed, pushed_nulls
                );

                // Arrow law riding along: zero nulls => no validity buffer.
                if pushed_nulls == 0 {
                    assert!(
                        arr.validity().is_none(),
                        "type {} phase {}: null-free array must drop the validity buffer",
                        $code,
                        k
                    );
                } else {
                    assert!(
                        arr.validity().is_some(),
                        "type {} phase {}: array with nulls must carry a validity buffer",
                        $code,
                        k
                    );
                }
                cases += 1;
            }
        };
    }
    scalar_types!(one_type);
    println!("null_count_matches_pushed_none_count: {} cases", cases);
    assert!(cases >= 50, "case budget: got {}", cases);
}

// ---------------------------------------------------------------------------
// law 2 — null slots carry ZERO bytes
// ---------------------------------------------------------------------------

/// Read the real bytes under slot `i` of a typed values slice. The one
/// unsafe block in this file, and the point of law 2: assert on raw memory,
/// not typed equality alone.
fn raw_bytes<T: Scalar>(values: &[T], i: usize) -> &[u8] {
    // SAFETY: i < values.len(); slot i is a live `T` in this slice, and u8
    // reads of its WIDTH bytes are always valid.
    unsafe {
        let base = values.as_ptr() as *const u8;
        core::slice::from_raw_parts(base.add(i * T::WIDTH), T::WIDTH)
    }
}

#[test]
fn null_slots_carry_zero_bytes() {
    let mut cases = 0usize;
    let mut slots_checked = 0usize;
    macro_rules! one_type {
        ($t:ty, $code:expr) => {
            for k in 0..4u64 {
                let mut xs = Xs::new(0x5EED_0002 ^ (k << 24) ^ ($code.as_bytes()[0] as u64));
                let n = 1 + xs.below(160);
                let null_permille = 100 + xs.below(700); // 10%..80%
                let mut b = PrimitiveBuilder::<$t>::new();
                let mut null_positions = Vec::new();
                for i in 0..n {
                    if i == 0 || xs.below(1000) < null_permille {
                        // Slot 0 forced null so every case has >= 1 null.
                        b.push(None);
                        null_positions.push(i);
                    } else {
                        b.push(Some(xs.u64() as $t));
                    }
                }
                let arr = b.finish();
                assert_eq!(arr.len(), n, "type {} seed {}", $code, k);
                assert_eq!(
                    arr.validity().map_or(0, Bitmap::null_count),
                    null_positions.len(),
                    "type {} seed {}: null_count != forced-null count",
                    $code,
                    k
                );
                for &i in &null_positions {
                    // (a) validity bit is clear at that slot
                    assert!(
                        !arr.validity().expect("bitmap present").get(i),
                        "type {} seed {}: slot {} null but validity bit set",
                        $code,
                        k,
                        i
                    );
                    // (b) typed slot is exactly T::ZERO
                    assert_eq!(
                        arr.values()[i],
                        <$t as Scalar>::ZERO,
                        "type {} seed {}: slot {} typed value != ZERO",
                        $code,
                        k,
                        i
                    );
                    // (c) every raw byte under the slot is zero
                    let raw = raw_bytes(arr.values(), i);
                    assert!(
                        raw.iter().all(|&byte| byte == 0),
                        "type {} seed {}: slot {} raw bytes non-zero: {:?}",
                        $code,
                        k,
                        i,
                        raw
                    );
                    slots_checked += 1;
                }
                cases += 1;
            }
        };
    }
    scalar_types!(one_type);
    println!(
        "null_slots_carry_zero_bytes: {} cases, {} null slots byte-checked",
        cases, slots_checked
    );
    assert!(cases >= 40 && slots_checked >= 40, "case budget: {} cases, {} slots", cases, slots_checked);
}

// ---------------------------------------------------------------------------
// law 3 — Varlen offset ladder
// ---------------------------------------------------------------------------

#[test]
fn varlen_offset_ladder_u32_and_u64() {
    let mut cases = 0usize;
    macro_rules! one_family {
        ($o:ty, $tag:expr) => {
            for k in 0..24u64 {
                let mut xs = Xs::new(0x5EED_0003 ^ (k << 20) ^ ($tag.as_bytes()[0] as u64));
                let n = 1 + xs.below(120);
                let mut b = VarlenBuilder::<$o>::new();
                let mut pushed_lens = Vec::new();
                let mut pushed_nulls = 0usize;
                for _ in 0..n {
                    if xs.below(1000) < 300 {
                        b.push(None);
                        pushed_lens.push(0usize);
                        pushed_nulls += 1;
                    } else {
                        let len = xs.below(24); // includes empty (0) rows
                        let bytes = rand_bytes(&mut xs, len);
                        b.push(Some(&bytes));
                        pushed_lens.push(len);
                    }
                }
                let v = b.finish();
                assert_eq!(v.len(), n, "family {} seed {}", $tag, k);

                let off = v.offsets();
                // N+1 entries
                assert_eq!(off.len(), n + 1, "family {} seed {}", $tag, k);
                // offsets[0] == 0
                assert_eq!(off[0].to_u64(), 0, "family {} seed {}: first offset != 0", $tag, k);
                // offsets[N] == data len
                assert_eq!(
                    off[n].to_u64() as usize,
                    v.data().len(),
                    "family {} seed {}: last offset {} != data len {}",
                    $tag,
                    k,
                    off[n].to_u64(),
                    v.data().len()
                );
                // monotonic (validated at freeze; assert independently)
                assert!(v.offsets_monotonic(), "family {} seed {}", $tag, k);
                // per-row: diff == pushed len == row(i).len(); nulls empty
                for i in 0..n {
                    let diff = (off[i + 1].to_u64() - off[i].to_u64()) as usize;
                    assert_eq!(
                        diff,
                        pushed_lens[i],
                        "family {} seed {} row {}: offset diff {} != pushed len {}",
                        $tag,
                        k,
                        i,
                        diff,
                        pushed_lens[i]
                    );
                    assert_eq!(
                        v.row(i).len(),
                        pushed_lens[i],
                        "family {} seed {} row {}: row len != pushed len",
                        $tag,
                        k,
                        i
                    );
                    if pushed_lens[i] == 0 {
                        assert!(
                            v.row(i).is_empty(),
                            "family {} seed {} row {}: zero-len slot not empty",
                            $tag,
                            k,
                            i
                        );
                    }
                }
                // validity popcount == pushed nulls
                assert_eq!(
                    v.validity().map_or(0, Bitmap::null_count),
                    pushed_nulls,
                    "family {} seed {}: null_count != pushed nulls",
                    $tag,
                    k
                );
                cases += 1;
            }
        };
    }
    one_family!(u32, "u32");
    one_family!(u64, "u64");
    println!("varlen_offset_ladder_u32_and_u64: {} cases", cases);
    assert!(cases >= 48, "case budget: {}", cases);
}

// ---------------------------------------------------------------------------
// law 4 — ViewVec descriptor law (inline AND foreign)
// ---------------------------------------------------------------------------

#[test]
fn viewvec_descriptor_law_inline_and_foreign() {
    let mut cases = 0usize;
    for k in 0..36u64 {
        let mut xs = Xs::new(0x5EED_0004 ^ (k << 20));
        let n = 3 + xs.below(40);
        let mut b = ViewVecBuilder::new();
        let mut pushed: Vec<Option<Vec<u8>>> = Vec::new();
        for i in 0..n {
            // Deterministic length bands so every case mixes inline, foreign
            // and null rows: band by row phase.
            let len = match (k + i as u64) % 4 {
                0 => xs.below(13),        // inline: 0..=12
                1 => 13 + xs.below(36),   // foreign: 13..=48
                2 => xs.below(13),        // inline
                _ => {
                    pushed.push(None);
                    b.push(None);
                    continue;
                }
            };
            let bytes = rand_bytes(&mut xs, len);
            b.push(Some(&bytes));
            pushed.push(Some(bytes));
        }
        let v = b.finish();
        assert_eq!(v.len(), pushed.len(), "seed {}", k);

        let mut saw_inline = false;
        let mut saw_foreign = false;
        for i in 0..pushed.len() {
            let base = i * 16;
            let d = v.views_bytes();
            let dlen = u32::from_le_bytes(d[base..base + 4].try_into().expect("16-byte view")) as usize;
            let pushed_len = pushed[i].as_ref().map_or(0, |r| r.len());

            // (a) descriptor len field == pushed len == row(i).len()
            assert_eq!(
                dlen, pushed_len,
                "seed {} row {}: descriptor len {} != pushed len {}",
                k, i, dlen, pushed_len
            );
            assert_eq!(
                v.row(i).len(),
                pushed_len,
                "seed {} row {}: row(i).len() != pushed len",
                k,
                i
            );

            // (b) inline classification matches the length band
            let inline = is_inline(dlen as u32) == 1;
            assert_eq!(
                inline,
                dlen <= 12,
                "seed {} row {}: is_inline({}) = {} disagrees with length",
                k,
                i,
                dlen,
                inline
            );
            match &pushed[i] {
                Some(row) => {
                    if inline {
                        saw_inline = true;
                        // (c) inline payload lives in the descriptor prefix
                        assert_eq!(
                            &d[base + 4..base + 4 + dlen],
                            row.as_slice(),
                            "seed {} row {}: inline payload mismatch",
                            k,
                            i
                        );
                    } else {
                        saw_foreign = true;
                        // (d) foreign: [buffer_index][offset]; bounds law
                        let off =
                            u32::from_le_bytes(d[base + 12..base + 16].try_into().expect("offset")) as usize;
                        assert!(
                            off + dlen <= v.data().len(),
                            "seed {} row {}: foreign offset {} + len {} > data len {}",
                            k,
                            i,
                            off,
                            dlen,
                            v.data().len()
                        );
                        // and the data block bytes are the pushed row
                        assert_eq!(
                            &v.data()[off..off + dlen],
                            row.as_slice(),
                            "seed {} row {}: data block bytes mismatch",
                            k,
                            i
                        );
                        // row(i) reads the same bytes back
                        assert_eq!(v.row(i), row.as_slice(), "seed {} row {}: row(i) mismatch", k, i);
                    }
                }
                None => {
                    // (e) null rows: zero-length inline descriptor
                    assert_eq!(dlen, 0, "seed {} row {}: null row descriptor len != 0", k, i);
                    assert!(inline, "seed {} row {}: null row not inline-classified", k, i);
                    assert!(v.row(i).is_empty(), "seed {} row {}: null row payload not empty", k, i);
                }
            }
        }
        assert!(
            saw_inline && saw_foreign,
            "seed {}: case must mix inline and foreign rows (inline={} foreign={})",
            k,
            saw_inline,
            saw_foreign
        );
        cases += 1;
    }
    println!("viewvec_descriptor_law_inline_and_foreign: {} cases", cases);
    assert!(cases >= 36, "case budget: {}", cases);
}

// ---------------------------------------------------------------------------
// law 5 — Bitmap word()/n_bits tail-mask coherence
// ---------------------------------------------------------------------------

#[test]
fn bitmap_word_tail_mask_law() {
    let mut cases = 0usize;

    // One seeded builder shared by the fixed edge set and the random sweep.
    let mut xs = Xs::new(0x5EED_0005);

    // Fixed edge set around word boundaries (n_bits NOT multiples of 64
    // included), plus seeded random sizes.
    let mut sizes: Vec<usize> = vec![1, 7, 8, 9, 63, 64, 65, 127, 128, 129, 191, 192, 193, 255, 256, 257];
    for _ in 0..30 {
        sizes.push(1 + xs.below(400));
    }

    for n_bits in sizes {
        // Random validity with at least one null (from_bools drops an
        // all-valid bitmap by Arrow law).
        let mut flags: Vec<bool> = (0..n_bits).map(|_| xs.below(1000) >= 300).collect();
        flags[0] = false;

        let bm = Bitmap::from_bools(&flags).expect(">=1 null present");
        assert_eq!(bm.n_bits(), n_bits, "n_bits {}", n_bits);
        assert_eq!(bm.words_len(), n_bits.div_ceil(64), "n_bits {}", n_bits);

        // Tail-mask law: every word has zero bits at or after n_bits.
        for w in 0..bm.words_len() {
            let base = w * 64;
            let live = n_bits.saturating_sub(base).min(64);
            let live_mask: u64 = if live == 64 { u64::MAX } else { (1u64 << live) - 1 };
            assert_eq!(
                bm.word(w) & !live_mask,
                0,
                "n_bits {} word {}: bits set at or after n_bits (live={}, word={:#x}, mask={:#x})",
                n_bits,
                w,
                live,
                bm.word(w),
                live_mask
            );
        }

        // Coherence: live popcount + null_count == n_bits, and the live
        // popcount re-derived per-word from get() agrees.
        let ones = count_one_bits(&bm);
        assert_eq!(
            ones + bm.null_count() as u64,
            n_bits as u64,
            "n_bits {}: ones {} + nulls {} != n_bits",
            n_bits,
            ones,
            bm.null_count()
        );
        let get_bits: u64 = (0..n_bits).map(|i| (bm.get(i) as u64)).sum();
        assert_eq!(
            get_bits, ones,
            "n_bits {}: per-bit get() popcount != count_one_bits",
            n_bits
        );

        cases += 1;
    }
    println!("bitmap_word_tail_mask_law: {} cases", cases);
    assert!(cases >= 46, "case budget: {}", cases);
}

// ---------------------------------------------------------------------------
// law 6 — format codes match the literal Arrow spec table
// ---------------------------------------------------------------------------

#[test]
fn scalar_format_codes_match_spec_table() {
    let mut cases = 0usize;
    macro_rules! one_type {
        ($t:ty, $code:expr) => {
            // via the trait constant
            assert_eq!(
                <$t as Scalar>::FORMAT_CODE,
                $code,
                "Scalar::FORMAT_CODE for {} must be {}",
                stringify!($t),
                $code
            );
            // via the array-level accessor
            assert_eq!(
                Primitive::<$t>::format_code(),
                $code,
                "Primitive::<{}>::format_code() must be {}",
                stringify!($t),
                $code
            );
            // width is the honest sizeof
            assert_eq!(
                <$t as Scalar>::WIDTH,
                core::mem::size_of::<$t>(),
                "Scalar::WIDTH for {} must be size_of",
                stringify!($t)
            );
            cases += 1;
        };
    }
    scalar_types!(one_type);
    println!("scalar_format_codes_match_spec_table: {} types checked", cases);
    assert_eq!(cases, 10, "all 10 Scalar types must be checked");
}
