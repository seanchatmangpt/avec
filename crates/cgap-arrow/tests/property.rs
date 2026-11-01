//! Property-style tests over the public `cgap-arrow` API.
//!
//! Hand-rolled xorshift64* PRNG (fixed seeds, no external crates), so any
//! red run is replayable: every failure message embeds the master seed, the
//! family seed, the iteration, and the row index.
//!
//! Families (500 seeded cases each, 2500 total):
//! 1. bools   — `Bitmap::from_bools` round-trip + null-count/all-valid laws
//! 2. i64     — `PrimitiveBuilder` finish: bitwise value round-trip + 64B alignment
//! 3. varlen  — `VarlenBuilder<u32>`: row round-trip + monotonic offsets
//! 4. views   — `ViewVecBuilder`: row round-trip across the inline/foreign boundary
//! 5. align   — pointer law: every publicly exposed buffer pointer % 64 == 0
//!
//! Varlen/ViewVec expose no base pointers in their public API, so family 5
//! covers `Buf`, `Bitmap`, and `Primitive` (the pointer-exposing types).

use cgap_arrow::bitmap::Bitmap;
use cgap_arrow::buffer::ALIGNMENT;
use cgap_arrow::varlen::Offset;
use cgap_arrow::{Buf, PrimitiveBuilder, VarlenBuilder, ViewVecBuilder};

/// Master seed: every family seed derives from this one constant.
const MASTER_SEED: u64 = 0x0C7A_2026_1029_0001;
const CASES_PER_FAMILY: usize = 500;

/// xorshift64*: one u64 per call, full-period over nonzero states. `new`
/// forces the state nonzero so the generator never latches at zero.
struct Xs(u64);

impl Xs {
    fn new(seed: u64) -> Self {
        Xs(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Value in `0..n`. Modulo bias is irrelevant at test scale.
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 24) as u8
    }
}

fn fam_seed(tag: u64) -> u64 {
    MASTER_SEED ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

// ---------------------------------------------------------------------------
// Family 1: bools -> Bitmap
// ---------------------------------------------------------------------------

fn prop_bools(rng: &mut Xs) -> usize {
    let seed = rng.0;
    for it in 0..CASES_PER_FAMILY {
        let len = 1 + rng.below(200) as usize; // 1..=200
        let forced_all_true = it % 8 == 0;
        let bools: Vec<bool> = (0..len)
            .map(|_| if forced_all_true { true } else { rng.below(2) == 1 })
            .collect();
        let falses = bools.iter().filter(|&&b| !b).count();

        if forced_all_true || falses == 0 {
            // Arrow law: null_count == 0 -> no validity buffer at all.
            assert!(
                Bitmap::from_bools(&bools).is_none(),
                "bools: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                 all-true input must yield None (len={len})"
            );
            continue;
        }

        let bm = Bitmap::from_bools(&bools).unwrap_or_else(|| {
            panic!(
                "bools: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                 non-all-true input yielded None (len={len} falses={falses})"
            )
        });
        assert_eq!(
            bm.n_bits(),
            len,
            "bools: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} n_bits"
        );
        for (i, &b) in bools.iter().enumerate() {
            assert_eq!(
                bm.get(i),
                b,
                "bools: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} i={i}"
            );
        }
        assert_eq!(
            bm.null_count(),
            falses,
            "bools: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
             null_count (len={len} falses={falses})"
        );
        assert_eq!(
            bm.is_all_valid(),
            falses == 0,
            "bools: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
             is_all_valid iff no falses (len={len} falses={falses})"
        );
    }
    CASES_PER_FAMILY
}

// ---------------------------------------------------------------------------
// Family 2: i64 -> PrimitiveBuilder -> Primitive
// ---------------------------------------------------------------------------

fn prop_i64(rng: &mut Xs) -> usize {
    let seed = rng.0;
    for it in 0..CASES_PER_FAMILY {
        let len = 1 + rng.below(150) as usize; // 1..=150
        let with_nulls = it % 3 != 0;
        let inject_extremes = it % 11 == 0;

        let mut b = PrimitiveBuilder::<i64>::new();
        let mut expect: Vec<Option<i64>> = Vec::with_capacity(len);
        for j in 0..len {
            let is_null = with_nulls && rng.below(4) == 0;
            let v = if inject_extremes && j == 0 {
                if rng.below(2) == 0 { i64::MIN } else { i64::MAX }
            } else {
                rng.next_u64() as i64
            };
            match is_null {
                true => {
                    b.push(None);
                    expect.push(None);
                }
                false => {
                    b.push(Some(v));
                    expect.push(Some(v));
                }
            }
        }

        let arr = b.finish();
        assert_eq!(
            arr.len(),
            len,
            "i64: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} len"
        );
        assert_eq!(
            arr.validity().is_some(),
            expect.iter().any(Option::is_none),
            "i64: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
             validity buffer present iff any null"
        );
        for (i, e) in expect.iter().enumerate() {
            match e {
                Some(v) => assert_eq!(
                    arr.values()[i].to_le_bytes(),
                    v.to_le_bytes(),
                    "i64: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} i={i} value"
                ),
                None => {
                    // Null slots carry the deterministic ZERO fill, bitwise zero.
                    assert_eq!(
                        arr.values()[i].to_le_bytes(),
                        [0u8; 8],
                        "i64: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} i={i} null slot"
                    );
                    assert!(
                        !arr.validity().expect("validity checked above").get(i),
                        "i64: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} i={i} validity bit"
                    );
                }
            }
        }
        assert!(
            arr.buffers_aligned_64(),
            "i64: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} buffers_aligned_64"
        );
    }
    CASES_PER_FAMILY
}

// ---------------------------------------------------------------------------
// Family 3: byte strings -> VarlenBuilder<u32>
// ---------------------------------------------------------------------------

fn prop_varlen(rng: &mut Xs) -> usize {
    let seed = rng.0;
    for it in 0..CASES_PER_FAMILY {
        let rows = 1 + rng.below(80) as usize; // 1..=80
        let with_nulls = it % 5 == 4;

        let mut b = VarlenBuilder::<u32>::new();
        let mut expect: Vec<Option<Vec<u8>>> = Vec::with_capacity(rows);
        for _ in 0..rows {
            if with_nulls && rng.below(3) == 0 {
                b.push(None);
                expect.push(None);
            } else {
                let n = rng.below(40) as usize; // 0..40 bytes
                let bytes: Vec<u8> = (0..n).map(|_| rng.byte()).collect();
                b.push(Some(&bytes));
                expect.push(Some(bytes));
            }
        }

        let v = b.finish();
        assert_eq!(
            v.len(),
            rows,
            "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} len"
        );
        for (i, e) in expect.iter().enumerate() {
            let want: &[u8] = e.as_deref().unwrap_or(&[]);
            assert_eq!(
                v.row(i),
                want,
                "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} i={i}"
            );
        }
        assert!(
            v.offsets_monotonic(),
            "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} offsets_monotonic"
        );
        assert_eq!(
            v.offsets().len(),
            rows + 1,
            "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} N+1 offsets"
        );
        assert_eq!(
            v.offsets()[0].to_u64(),
            0,
            "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} first offset zero"
        );
        assert_eq!(
            v.offsets()[rows].to_u64() as usize,
            v.data().len(),
            "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} last offset == data len"
        );
        assert_eq!(
            v.validity().is_some(),
            expect.iter().any(Option::is_none),
            "varlen: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
             validity buffer present iff any null"
        );
    }
    CASES_PER_FAMILY
}

// ---------------------------------------------------------------------------
// Family 4: strings -> ViewVecBuilder across the inline/foreign boundary
// ---------------------------------------------------------------------------

fn prop_views(rng: &mut Xs) -> usize {
    let seed = rng.0;
    for it in 0..CASES_PER_FAMILY {
        let rows = 2 + rng.below(60) as usize; // 2..=61
        let mut b = ViewVecBuilder::new();
        let mut expect: Vec<Vec<u8>> = Vec::with_capacity(rows);
        let mut inline_seen = 0usize;
        let mut foreign_seen = 0usize;
        for j in 0..rows {
            // Rows 0 and 1 pin both sides of the MAX_INLINE = 12 boundary in
            // every case (12 = largest inline, 13 = first foreign length).
            // Even iterations hug the boundary (10..=15) for the rest; odd
            // iterations draw general lengths that also cross it.
            let n = match j {
                0 => 12,
                1 => 13,
                _ if it % 2 == 0 => 10 + rng.below(6) as usize,
                _ => rng.below(40) as usize,
            };
            let bytes: Vec<u8> = (0..n).map(|_| rng.byte()).collect();
            if n <= 12 {
                inline_seen += 1;
            } else {
                foreign_seen += 1;
            }
            b.push(Some(&bytes));
            expect.push(bytes);
        }

        let vv = b.finish();
        assert_eq!(
            vv.len(),
            rows,
            "views: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} len"
        );
        for (i, e) in expect.iter().enumerate() {
            assert_eq!(
                vv.row(i),
                e.as_slice(),
                "views: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} i={i} \
                 (inline_seen={inline_seen} foreign_seen={foreign_seen})"
            );
        }
        // Payload conservation: exactly the >12-byte rows land in the data block.
        let foreign_total: usize =
            expect.iter().filter(|e| e.len() > 12).map(|e| e.len()).sum();
        assert_eq!(
            vv.data().len(),
            foreign_total,
            "views: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
             data block holds exactly the foreign payloads"
        );
        assert!(
            inline_seen > 0 && foreign_seen > 0,
            "views: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
             case degenerate: both sides of the boundary must occur \
             (inline={inline_seen} foreign={foreign_seen})"
        );
    }
    CASES_PER_FAMILY
}

// ---------------------------------------------------------------------------
// Family 5: alignment law over every publicly exposed buffer pointer
// ---------------------------------------------------------------------------

fn prop_align(rng: &mut Xs) -> usize {
    let seed = rng.0;
    for it in 0..CASES_PER_FAMILY {
        match it % 3 {
            0 => {
                // Buf at adversarial lengths; every 13th case is len 0
                // (documented 64-aligned non-null sentinel).
                let len = if it % 13 == 0 {
                    0
                } else {
                    rng.below(5000) as usize
                };
                let buf = Buf::new(len);
                assert_eq!(
                    buf.as_ptr() as usize % ALIGNMENT,
                    0,
                    "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                     Buf ptr (len={len})"
                );
            }
            1 => {
                // Bitmap: force at least one false so from_bools yields Some.
                let len = 1 + rng.below(300) as usize;
                let mut bools = vec![true; len];
                bools[rng.below(len as u64) as usize] = false;
                let bm = Bitmap::from_bools(&bools).unwrap_or_else(|| {
                    panic!(
                        "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                         expected Some(Bitmap)"
                    )
                });
                assert_eq!(
                    bm.as_ptr() as usize % ALIGNMENT,
                    0,
                    "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                     Bitmap ptr (n_bits={len})"
                );
            }
            _ => {
                // Primitive<i64>: value buffer + validity buffer pointers.
                let len = 1 + rng.below(100) as usize;
                let with_nulls = it % 2 == 1;
                let mut b = PrimitiveBuilder::<i64>::new();
                let mut any_null = false;
                for j in 0..len {
                    if with_nulls && rng.below(3) == 0 {
                        b.push(None);
                        any_null = true;
                    } else {
                        b.push(Some(rng.next_u64() as i64 ^ j as i64));
                    }
                }
                let arr = b.finish();
                assert_eq!(
                    arr.values_ptr() as usize % ALIGNMENT,
                    0,
                    "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                     values_ptr (len={len})"
                );
                if let Some(bm) = arr.validity() {
                    assert_eq!(
                        bm.as_ptr() as usize % ALIGNMENT,
                        0,
                        "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                         validity_ptr (len={len})"
                    );
                }
                assert_eq!(
                    arr.validity().is_some(),
                    any_null,
                    "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} validity law"
                );
                assert!(
                    arr.buffers_aligned_64(),
                    "align: master={MASTER_SEED:#018x} seed={seed:#018x} it={it} \
                     buffers_aligned_64"
                );
            }
        }
    }
    CASES_PER_FAMILY
}

/// The full property matrix in one deterministic pass so the receipt prints
/// per-family counts plus the aggregate line in a fixed order.
#[test]
fn property_suite() {
    let bools_cases = prop_bools(&mut Xs::new(fam_seed(1)));
    let i64_cases = prop_i64(&mut Xs::new(fam_seed(2)));
    let varlen_cases = prop_varlen(&mut Xs::new(fam_seed(3)));
    let views_cases = prop_views(&mut Xs::new(fam_seed(4)));
    let align_cases = prop_align(&mut Xs::new(fam_seed(5)));
    let total = bools_cases + i64_cases + varlen_cases + views_cases + align_cases;
    println!("bools cases: {bools_cases}");
    println!("i64 cases: {i64_cases}");
    println!("varlen cases: {varlen_cases}");
    println!("views cases: {views_cases}");
    println!("align cases: {align_cases}");
    println!("property cases: {total}");
    assert_eq!(total, 5 * CASES_PER_FAMILY, "case accounting drifted");
}
