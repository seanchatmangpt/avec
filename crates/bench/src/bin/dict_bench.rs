//! Dictionary-page round-trip bench (lane OPT20-L6, charter v26.10.29).
//!
//! Dict-encoded page round trip at cardinalities {2, 16, 256} over 16M
//! indices vs plain literal encode/decode of the same 16M-lane stream.
//! Mirrors the page_bench/parquet_bench house pattern: counting GlobalAlloc
//! (with a non-vacuity probe), min-of-5 rounds, `black_box` elision guards,
//! and a full round-trip pre-flight per cardinality BEFORE any timing.
//!
//! Path map (contract: encode_fast on the write side, decode_into on the
//! decode sides as applicable):
//! - dict write  = [`cgap_parquet::encode_fast::rle_encode_fast`] over the
//!   index stream (pre-flight asserts byte-identity with
//!   [`cgap_parquet::dictionary::encode_page`], the gated module writer,
//!   so the timed fast path IS the dict page on the wire);
//! - dict read   = [`cgap_parquet::dictionary::decode_page`] — `decode_into`
//!   lands the index lanes in place, then the in-place gather expands them
//!   through the dictionary (that gather is the dict read-side overhead);
//! - literal write = `rle_encode_fast` over the values at the same kernel
//!   width (parity vs `rle_encode` asserted in pre-flight);
//! - literal read  = [`cgap_parquet::rle::decode_into`] directly.
//!
//! Bytes-moved conventions (house): encode/build = input bytes (16M u32 =
//! 64 MiB walked); decode = output bytes (16M u32 = 64 MiB written).
//!
//! Cardinality -> width law: 2 -> 1 bit, 16 -> 4 bits, 256 -> 8 bits
//! ([`cgap_parquet::dictionary::required_bits`]), so the sweep exercises
//! kernel widths {1, 4, 8}.
//!
//! The stream is deterministic LCG over the full `card` domain (no planted
//! runs: the literal-heavy worst case for the hybrid codec, identical input
//! to both paths). The `lens` lines carry measured wire bytes per lane for
//! dict vs literal pages plus the 4 B/lane plain-u32 reference. Measured
//! outcome on this shape: dict page == literal page byte-for-byte at every
//! cardinality (the index stream is a bijective rename of the value stream,
//! AND the hybrid codec's literal groups store one byte per lane at every
//! kernel width — the narrow `bits` law prices repeated-run records
//! (`encode_fast.rs` bound: ~2r/3 bytes per r-lane run), not literal
//! groups), so on literal-heavy data the dict win vs plain u32 is the same
//! ~4x at cards 2 / 16 / 256, not a per-cardinality multiple.

use cgap_parquet::dictionary::{decode_page, encode_page, required_bits, DictionaryBuilder};
use cgap_parquet::encode_fast::rle_encode_fast;
use cgap_parquet::rle::{decode_into, rle_encode};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// 16M u32 lanes = 64 MiB logical payload per measurement (house).
const LANES: usize = 16 * 1024 * 1024;
/// Min-of-N: best (least-noisy) round wins; concurrent lanes make this noisy.
const ROUNDS: usize = 5;
/// Untouched-lane sentinel: far above any dict value, so an unwritten lane
/// can never pass the round-trip equality (house).
const SENTINEL: u32 = 0x1234_ABCD;
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
/// Allocs are from the winning (fastest) round. Contract format:
/// `dict <cardinality> <op>: <X.XXX> GiB/s, <allocs> allocs`.
fn bench_line(card: usize, op: &str, bytes_moved: u64, mut f: impl FnMut()) {
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
    println!("dict {card} {op}: {gibps:.3} GiB/s, {best_allocs} allocs");
}

/// Sample head/middle/tail lanes through black_box so decode output
/// cannot be elided (house).
fn consume_u32(o: &[u32]) {
    black_box(u64::from(o[0]) ^ u64::from(o[o.len() / 2]) ^ u64::from(o[o.len() - 1]));
}

/// Deterministic LCG (encode_fast.rs test pattern): stable vectors.
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

    let payload = (LANES * 4) as u64; // 64 MiB of u32 lanes walked/written

    for card in [2usize, 16, 256] {
        let bits = required_bits(card).expect("card <= 256 is representable");
        assert!(
            1usize << bits >= card,
            "card={card}: width {bits} must carry the dict domain"
        );

        // ---- fixed stream: identity dict, LCG indices ----
        // dict[i] = i keeps the value lanes inside 0..card = 0..2^bits, so
        // the literal page at the SAME width carries them untruncated (the
        // hybrid packer masks lanes to the declared width — wider dict
        // payloads are the dict path's business, not the lane domain). The
        // value stream is the gather of the dict through the indices.
        let dict: Vec<u32> = (0..card as u32).collect();
        let mut lcg = Lcg(0x1234_5678);
        let indices: Vec<u32> = (0..LANES).map(|_| (lcg.next() % card as u64) as u32).collect();
        let values: Vec<u32> = indices.iter().map(|&ix| dict[ix as usize]).collect();

        // ---- pre-flight: full round trip BEFORE any timing ----
        // (a) the public interning path re-derives exactly this (dict,
        //     indices) pair over all 16M lanes;
        let mut b = DictionaryBuilder::new();
        for &v in &values {
            b.push(v);
        }
        let (dict_b, idx_b) = b.finish();
        assert_eq!(dict_b.len(), card, "card={card}: builder dict must be exact");
        // The builder emits FIRST-SEEN order (not the synthetic dict's
        // index order); the invariant is that its pair re-derives the
        // stream exactly — the real pipeline's (dict, indices) from here on.
        let replay: Vec<u32> = idx_b.iter().map(|&ix| dict_b[ix as usize]).collect();
        assert_eq!(replay, values, "card={card}: builder pair re-derivation");
        let dict = dict_b;
        let indices = idx_b;

        // (b) the fast write side is byte-identical to the gated module
        //     writer — the timed page IS the dictionary module's page;
        let page = rle_encode_fast(&indices, bits);
        assert_eq!(
            page,
            encode_page(&dict, &indices, bits).expect("pre-flight gated encode"),
            "card={card}: encode_fast diverged from encode_page on the bench page"
        );

        // (c) sentinel-poisoned output: dict decode must overwrite every
        //     lane through the gather; an unwritten lane stays loud.
        let mut out = vec![SENTINEL; LANES];
        decode_page(&page, &dict, &mut out).expect("pre-flight dict decode");
        assert_eq!(out, values, "card={card}: dict round-trip mismatch");

        // (d) literal side: fast encoder parity + decode_into round trip.
        let lit_page = rle_encode_fast(&values, bits);
        assert_eq!(
            lit_page,
            rle_encode(&values, bits),
            "card={card}: literal encode_fast diverged from rle_encode"
        );
        out.fill(SENTINEL);
        let n = decode_into(&lit_page, &mut out).expect("pre-flight literal decode");
        assert_eq!(n, LANES, "card={card}: literal lane count");
        assert_eq!(out, values, "card={card}: literal round-trip mismatch");

        println!(
            "dict lens card={card}: width={bits} bits, dict page={} B ({:.3} B/lane) vs literal page={} B ({:.3} B/lane), dict entries={} ({} B), plain-u32 reference {} B (4.000 B/lane)",
            page.len(),
            page.len() as f64 / LANES as f64,
            lit_page.len(),
            lit_page.len() as f64 / LANES as f64,
            dict.len(),
            dict.len() * 4,
            LANES * 4,
        );

        // ---- dict build: intern all 16M values (the dict path's own
        //      upstream cost the literal path does not pay) ----
        bench_line(card, "build", payload, || {
            let v: &[u32] = black_box(&values);
            let mut bld = DictionaryBuilder::new();
            for &x in v {
                bld.push(x);
            }
            let (d, ix) = bld.finish();
            black_box(d.len());
            black_box(ix.len());
        });

        // ---- dict encode: write the index page (encode_fast contract) ----
        bench_line(card, "encode", payload, || {
            let ix: &[u32] = black_box(&indices);
            let p = rle_encode_fast(ix, bits);
            black_box(p.len());
            drop(p);
        });

        // ---- dict decode: decode_into in place + gather (decode_page) ----
        bench_line(card, "decode", payload, || {
            let p: &[u8] = black_box(&page);
            let d: &[u32] = black_box(&dict);
            let o: &mut [u32] = black_box(&mut out);
            decode_page(p, d, o).expect("dict decode");
            consume_u32(o);
        });

        // ---- literal encode: values straight through encode_fast ----
        bench_line(card, "literal_encode", payload, || {
            let v: &[u32] = black_box(&values);
            let p = rle_encode_fast(v, bits);
            black_box(p.len());
            drop(p);
        });

        // ---- literal decode: values straight through decode_into ----
        bench_line(card, "literal_decode", payload, || {
            let p: &[u8] = black_box(&lit_page);
            let o: &mut [u32] = black_box(&mut out);
            let n = decode_into(p, o).expect("literal decode");
            black_box(n);
            consume_u32(o);
        });

        drop(out);
        drop(values);
        drop(indices);
        drop(page);
        drop(lit_page);
    }

    println!(
        "dict note: concurrent cargo lanes share this machine; min-of-{ROUNDS} absorbs scheduler noise, absolute numbers remain load-bound"
    );
}
