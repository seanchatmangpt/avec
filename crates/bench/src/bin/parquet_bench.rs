//! Parquet kernel throughput bench (lane W2-PARQ-BENCH).
//!
//! Measure, don't claim: every printed line is the receipt. Mirrors the
//! main.rs alloc-interception pattern (counting GlobalAlloc), adds
//! min-of-5 rounds and black_box elision guards — the mask bench's absurd
//! numbers came from un-consumed output; here every result is sampled
//! through `std::hint::black_box`.
//!
//! Bytes-moved conventions (64 MiB logical payload per measurement):
//! - unpack: output bytes (8M u64 lanes = 64 MiB); packed input is width/8.
//! - rle_decode: output bytes (16M u32 lanes = 64 MiB).
//! - rle_encode: input bytes (16M u32 values = 64 MiB).
//!
//! RLE shapes run at the canonical nibble width (4 bits).

use cgap_parquet::rle::rle_encode;
use cgap_parquet::unpack::unpack_bits;
use cgap_parquet::rle_decode;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 8M u64 lanes = 64 MiB unpack output.
const UNPACK_LANES: usize = 8 * 1024 * 1024;
/// 16M u32 lanes = 64 MiB RLE payload.
const RLE_LANES: usize = 16 * 1024 * 1024;
/// Min-of-N: best (least-noisy) round wins; concurrent lanes make this noisy.
const ROUNDS: usize = 5;
/// Canonical nibble width for the RLE shapes.
const RLE_BITS: u8 = 4;

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

static COUNT: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: delegates to System, counting calls.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        COUNT.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// One measurement line: warm-up, then min-of-ROUNDS timed rounds.
/// Allocs are from the winning (fastest) round.
fn bench(op: &str, shape: &str, width: u8, bytes_moved: u64, mut f: impl FnMut()) {
    f(); // warm-up (page in, branch caches)
    let mut best = f64::INFINITY;
    let mut best_allocs = usize::MAX;
    for _ in 0..ROUNDS {
        COUNT.store(0, Ordering::Relaxed);
        let t0 = Instant::now();
        f();
        let dt = t0.elapsed().as_secs_f64();
        let allocs = COUNT.load(Ordering::Relaxed);
        if dt < best {
            best = dt;
            best_allocs = allocs;
        }
    }
    let gibps = bytes_moved as f64 / best / GIB;
    println!("parquet {op} {shape} {width}: {gibps:.3} GiB/s, {best_allocs} allocs");
}

/// Sample head/middle/tail lanes through black_box so decode output
/// cannot be elided.
fn consume_u64(o: &[u64]) {
    black_box(o[0] ^ o[o.len() / 2] ^ o[o.len() - 1]);
}

fn consume_u32(o: &[u32]) {
    black_box(u64::from(o[0]) ^ u64::from(o[o.len() / 2]) ^ u64::from(o[o.len() - 1]));
}

fn main() {
    // ---- (a) bit-unpack: widths 1/2/4/8 over 64 MiB of u64 lanes ----
    // Full-width source (1M words covers width 8; narrower widths read a prefix).
    let src: Vec<u64> = (0..UNPACK_LANES / 8)
        .map(|i| (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 0x8000_0000_0000_0001)
        .collect();
    let mut out = vec![0u64; UNPACK_LANES];
    let unpack_bytes = (UNPACK_LANES * 8) as u64;
    for w in [1u8, 2, 4, 8] {
        // spot-check the kernel before timing it (guard against a no-op bench)
        unpack_bits(w, &src, &mut out).expect("unpack spot-check");
        let lane = |i: usize| -> u64 {
            let word = src[i * usize::from(w) / 64];
            let shift = (i % (64 / usize::from(w))) * usize::from(w);
            (word >> shift) & ((1u64 << w) - 1)
        };
        assert_eq!(out[0], lane(0), "w={w} lane 0");
        assert_eq!(out[UNPACK_LANES - 1], lane(UNPACK_LANES - 1), "w={w} last lane");

        bench("unpack", "random64", w, unpack_bytes, || {
            let s: &[u64] = black_box(&src);
            let o: &mut [u64] = black_box(&mut out);
            unpack_bits(w, s, o).expect("unpack");
            consume_u64(o);
        });
    }

    // ---- (b)/(c) rle_decode + rle_encode: three shapes at width 4 ----
    for (shape, make) in [
        (
            "repeated",
            Box::new(|vmax: u32| vec![vmax; RLE_LANES]) as Box<dyn Fn(u32) -> Vec<u32>>,
        ),
        (
            "literal",
            Box::new(|vmax: u32| {
                (0..RLE_LANES).map(|i| (i as u32) & vmax).collect::<Vec<u32>>()
            }) as Box<dyn Fn(u32) -> Vec<u32>>,
        ),
        (
            "mix50",
            Box::new(|vmax: u32| {
                // 16-lane block: 8 identical (repeated record) + 8 strictly
                // sub-vmax lanes (one literal group) => exactly 50/50 lanes.
                // Literals must stay below vmax: a trailing lane equal to vmax
                // merges with the next block's run into a 9-run, shifting
                // record alignment and padding a literal batch past budget
                // (caught by the round-trip pre-flight, run_len 9 / budget 2).
                (0..RLE_LANES)
                    .map(|i| {
                        if i % 16 < 8 {
                            vmax
                        } else {
                            ((i % 8) as u32).min(vmax - 1)
                        }
                    })
                    .collect::<Vec<u32>>()
            }) as Box<dyn Fn(u32) -> Vec<u32>>,
        ),
    ] {
        let values = make((1u32 << RLE_BITS) - 1);
        let stream = rle_encode(&values, RLE_BITS);

        // full round-trip pre-flight: decode(encode(x)) == x, real compare
        let mut decoded = vec![0u32; RLE_LANES];
        rle_decode(&stream, &mut decoded).expect("pre-flight decode");
        assert_eq!(decoded, values, "{shape}: round-trip mismatch");

        let decode_bytes = (RLE_LANES * 4) as u64;
        bench("rle_decode", shape, RLE_BITS, decode_bytes, || {
            let s: &[u8] = black_box(&stream);
            let o: &mut [u32] = black_box(&mut decoded);
            rle_decode(s, o).expect("rle_decode");
            consume_u32(o);
        });

        bench("rle_encode", shape, RLE_BITS, decode_bytes, || {
            let v: &[u32] = black_box(&values);
            let enc = rle_encode(v, RLE_BITS);
            black_box(enc.len() as u64);
            drop(enc);
        });

        drop(decoded);
        drop(values);
        drop(stream);
    }
}
