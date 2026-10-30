//! Tight bit-packing bench (lane OPT20-L5).
//!
//! Measure, don't claim: every printed line is the receipt. Mirrors the
//! page_bench/parquet_bench house pattern: counting GlobalAlloc (with a
//! non-vacuity probe — "0 allocs" is only meaningful if a deliberate
//! allocation provably bumps COUNT), min-of-5 rounds with MIN as the
//! receipt, and black_box elision guards (the mask bench's absurd numbers
//! came from un-consumed output).
//!
//! Bytes-moved formulas (64 MiB payload per measurement):
//! - `pack`:   input bytes walked  = LANES * 4      (16M u32 values = 64 MiB)
//! - `unpack`: output bytes written = LANES * 4     (16M u32 lanes = 64 MiB)
//! - `swar-unpack`: output bytes    = SWAR_LANES * 8 (8M u64 lanes = 64 MiB)
//!
//! Output-byte accounting for the decode sides keeps tight-vs-SWAR rates
//! directly comparable: both move the same 64 MiB of lanes. Packed inputs
//! are width/8-sized extras, not counted (same convention as parquet_bench:
//! "packed input is width/8").
//!
//! Pre-flight: full round-trip per width BEFORE any timing, with the output
//! sentinel-poisoned so a decode writing a prefix cannot hide. At widths
//! {1,2,4,8} the SWAR kernels are additionally cross-checked lane-for-lane
//! against the tight decode on real packed bytes.

use avec_parquet::packing::{pack_bits, unpack_bits_tight};
use avec_parquet::unpack::{unpack_1bit, unpack_2bit, unpack_4bit, unpack_8bit};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 16M u32 lanes = 64 MiB logical payload (tight pack/unpack side).
const LANES: usize = 16 * 1024 * 1024;
/// 8M u64 lanes = 64 MiB unpack output (SWAR side).
const SWAR_LANES: usize = 8 * 1024 * 1024;
/// Untouched-lane sentinel: far above any w<=8 domain value, so an
/// unwritten lane can never pass the pre-flight equality.
const SENTINEL: u32 = 0x1234_ABCD;
/// Min-of-N: best (least-noisy) round wins; concurrent lanes make this noisy.
const ROUNDS: usize = 5;
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

/// Deterministic LCG (same shape as the packing.rs test vectors): stable
/// bench inputs across runs, no deps.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn value(&mut self, w: u8) -> u32 {
        let mask = if w == 8 { 0xFFu64 } else { (1u64 << w) - 1 };
        (self.next() & mask) as u32
    }
}

/// One measurement line: warm-up, then min-of-ROUNDS timed rounds.
/// Allocs are from the winning (fastest) round — MIN is the receipt.
fn bench_line(op: &str, w: u8, bytes_moved: u64, mut f: impl FnMut()) {
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
    println!("packing {op} w={w}: {gibps:.3} GiB/s, {best_allocs} allocs");
}

/// Load a byte stream into LE u64 words (zero-padded final word) so the
/// SWAR kernels (which take `&[u64]`) see the same bit positions as the
/// tight stream.
fn le_words(bytes: &[u8]) -> Vec<u64> {
    let mut words = vec![0u64; bytes.len().div_ceil(8)];
    for (i, chunk) in bytes.chunks(8).enumerate() {
        let mut buf = [0u8; 8];
        buf[..chunk.len()].copy_from_slice(chunk);
        words[i] = u64::from_le_bytes(buf);
    }
    words
}

/// Sample head/middle/tail lanes through black_box so decode output
/// cannot be elided.
fn consume_u32(o: &[u32]) {
    black_box(u64::from(o[0]) ^ u64::from(o[o.len() / 2]) ^ u64::from(o[o.len() - 1]));
}

fn consume_u64(o: &[u64]) {
    black_box(o[0] ^ o[o.len() / 2] ^ o[o.len() - 1]);
}

fn main() {
    // Non-vacuity probe: a deliberate allocation must bump COUNT before any
    // "0 allocs" line below is trusted (counted nothing != allocated nothing).
    COUNT.store(0, Ordering::Relaxed);
    let probe: Vec<u8> = vec![0xAB; 4096];
    black_box(&probe);
    assert!(
        COUNT.load(Ordering::Relaxed) >= 1,
        "allocator hook not wired: deliberate Vec alloc did not bump COUNT"
    );

    let payload = (LANES * 4) as u64; // 64 MiB
    let swar_payload = (SWAR_LANES * 8) as u64; // 64 MiB

    println!(
        "# packing_bench: min-of-{ROUNDS} rounds, MIN is the receipt; concurrent \
         lanes on this host make individual rounds noisy — the min bounds true \
         capability, a mean would average noise in"
    );

    // One shared SWAR output buffer, reused across widths and rounds.
    let mut swar_out = vec![0u64; SWAR_LANES];

    for w in 1..=8u8 {
        let mut rng = Lcg(0x5EED_BEEF_0000 + u64::from(w) * 7919);
        let values: Vec<u32> = (0..LANES).map(|_| rng.value(w)).collect();

        // ---- pre-flight: full round-trip BEFORE any timing ----
        let packed = pack_bits(&values, w);
        assert_eq!(
            packed.len(),
            (LANES * usize::from(w)).div_ceil(8),
            "w={w}: packed byte count"
        );
        let mut out = vec![SENTINEL; LANES];
        unpack_bits_tight(&packed, w, &mut out).expect("pre-flight tight decode");
        assert_eq!(out, values, "w={w}: tight round-trip mismatch");

        if let Some(swar) = match w {
            1 => Some(unpack_1bit as fn(&[u64], &mut [u64])),
            2 => Some(unpack_2bit as fn(&[u64], &mut [u64])),
            4 => Some(unpack_4bit as fn(&[u64], &mut [u64])),
            8 => Some(unpack_8bit as fn(&[u64], &mut [u64])),
            _ => None,
        } {
            // ---- SWAR cross-check on the same real packed bytes ----
            let words = le_words(&packed);
            swar(&words, &mut swar_out);
            for (i, lane) in swar_out.iter().enumerate() {
                assert_eq!(
                    u64::from(out[i]),
                    *lane,
                    "w={w} lane {i}: tight vs SWAR disagreement"
                );
            }

            println!(
                "packing lens w={w}: tight {LANES} u32 lanes = 64 MiB, packed={} B | \
                 swar {SWAR_LANES} u64 lanes = 64 MiB, packed={} B",
                packed.len(),
                SWAR_LANES / (64 / usize::from(w)),
            );

            // ---- SWAR unpack bench (output bytes moved) ----
            bench_line("swar-unpack", w, swar_payload, || {
                let src: &[u64] = black_box(&words);
                let o: &mut [u64] = black_box(&mut swar_out);
                swar(src, o);
                consume_u64(o);
            });
        } else {
            println!(
                "packing lens w={w}: tight {LANES} u32 lanes = 64 MiB, packed={} B | \
                 (no SWAR kernel at this width)",
                packed.len()
            );
        }

        // ---- tight pack bench (input bytes walked) ----
        bench_line("pack", w, payload, || {
            let v: &[u32] = black_box(&values);
            let p = pack_bits(v, w);
            black_box(p.len());
            drop(p);
        });

        // ---- tight unpack bench (output bytes written) ----
        bench_line("unpack", w, payload, || {
            let b: &[u8] = black_box(&packed);
            let o: &mut [u32] = black_box(&mut out);
            unpack_bits_tight(b, w, o).expect("tight decode");
            consume_u32(o);
        });

        drop(out);
        drop(values);
        drop(packed);
    }
}
