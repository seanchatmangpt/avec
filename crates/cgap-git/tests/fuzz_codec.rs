//! Adversarial fuzz court for the cgap-git deflate/inflate codec
//! (wave-14 lane 1). NEW-FILE lane: read-only on existing sources.
//!
//! Deterministic by construction: every "random" choice comes from a
//! fixed-seed LCG — no external fuzzer, no wall clock, no threads — so any
//! finding replays byte-for-byte by rerunning this binary.
//!
//! Court clauses:
//!   1. Round-trip: 200 LCG corpora (sizes 0..=64 KiB, 7 entropy classes)
//!      through `zlib_compress`, then decoded by BOTH readers
//!      (`deflate::zlib_decompress` -> GitError and
//!      `inflate::zlib_decode` -> InflateError) plus both raw-inflate
//!      readers; byte equality and compressor determinism asserted.
//!   2. Bit-flip: 500 mutated streams (single/double/triple bit flips of
//!      valid zlib streams, fixed-Huffman and stored-block families).
//!      Each decode must return the exact original bytes or a typed
//!      error — never a panic (caught, recorded, reported), never wrong
//!      bytes silently (an `Ok` != original is the finding).
//!   3. Truncation: every prefix (0..=len) of 20 valid streams — typed
//!      error or exact original bytes only; stored-block streams included.
//!
//! A panic is the finding: `catch_unwind` records the mutation coordinates
//! needed to replay it and the test fails with them. Non-vacuity is proved
//! by `court_nonvacuity_controlled_mutations_must_flip`: deterministic
//! controlled mutations (stored-data bit, adler bit, header bit) MUST be
//! refused, and the pristine stream MUST pass — a gate that cannot flip is
//! FAIL.

#![forbid(unsafe_code)]

use cgap_git::deflate::{
    adler32, deflate_fixed, inflate as deflate_inflate, zlib_compress, zlib_decompress,
};
use cgap_git::inflate::{inflate as raw_inflate, zlib_decode, InflateError};
use cgap_git::GitError;
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

// ---- deterministic entropy source -----------------------------------------

/// Fixed seeds: findings replay by rerunning the binary unchanged.
const SEED_ROUNDTRIP: u64 = 0x5743_1400_0000_0001;
const SEED_BITFLIP_BASES: u64 = 0x5743_1400_0000_0002;
const SEED_BITFLIP_MUT: u64 = 0x5743_1400_0000_0003;
const SEED_TRUNCATION: u64 = 0x5743_1400_0000_0004;

/// LCG (Knuth/MMIX constants): the only "randomness" in this court.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed)
    }
    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u32() as usize) % n
        }
    }
    /// Uniform in [lo, hi] inclusive.
    fn uniform(&mut self, lo: usize, hi: usize) -> usize {
        debug_assert!(lo <= hi);
        lo + self.below(hi - lo + 1)
    }
}

/// Entropy classes: 0 random, 1 constant-run, 2 repeating pattern,
/// 3 text-like, 4 mixed segments, 5 incrementing, 6 sparse.
fn gen_corpus(rng: &mut Lcg, class: u32, size: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(size);
    match class {
        0 => {
            for _ in 0..size {
                v.push(rng.next_u32() as u8);
            }
        }
        1 => {
            let b = rng.next_u32() as u8;
            v.resize(size, b);
        }
        2 => {
            let p = rng.uniform(1, 257);
            let pat: Vec<u8> = (0..p).map(|_| rng.next_u32() as u8).collect();
            while v.len() < size {
                let n = core::cmp::min(p, size - v.len());
                v.extend_from_slice(&pat[..n]);
            }
        }
        3 => {
            const ALPHA: &[u8] = b"the quick brown fox jumps over the lazy dog.\n0123456789";
            while v.len() < size {
                v.push(ALPHA[rng.below(ALPHA.len())]);
            }
        }
        4 => {
            while v.len() < size {
                let mode = rng.below(3);
                let seg = rng.uniform(16, 1024).min(size - v.len());
                match mode {
                    0 => {
                        for _ in 0..seg {
                            v.push(rng.next_u32() as u8);
                        }
                    }
                    1 => {
                        let b = rng.next_u32() as u8;
                        v.resize(v.len() + seg, b);
                    }
                    _ => {
                        let p = rng.uniform(1, 33);
                        let pat: Vec<u8> = (0..p).map(|_| rng.next_u32() as u8).collect();
                        for k in 0..seg {
                            v.push(pat[k % p]);
                        }
                    }
                }
            }
        }
        5 => {
            let mut x = rng.next_u32() as u8;
            for _ in 0..size {
                v.push(x);
                x = x.wrapping_add(rng.uniform(1, 4) as u8);
            }
        }
        _ => {
            let b = rng.next_u32() as u8;
            while v.len() < size {
                if rng.below(16) == 0 {
                    v.push(rng.next_u32() as u8);
                } else {
                    v.push(b);
                }
            }
        }
    }
    v
}

/// Hand-built zlib stream over stored (BTYPE=00) blocks, `block_size` bytes
/// per block (legal DEFLATE the crate must accept; gives the court a
/// stored-block family that `zlib_compress` never emits).
fn zlib_stored_stream(data: &[u8], block_size: usize) -> Vec<u8> {
    let mut out = vec![0x78u8, 0x9C];
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        let mut off = 0usize;
        while off < data.len() {
            let n = core::cmp::min(block_size, data.len() - off);
            let last = off + n == data.len();
            out.push(if last { 0x01 } else { 0x00 });
            out.extend_from_slice(&(n as u16).to_le_bytes());
            out.extend_from_slice(&(!(n as u16)).to_le_bytes());
            out.extend_from_slice(&data[off..off + n]);
            off += n;
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

// ---- decode protocol -------------------------------------------------------

/// One codec call, panic-isolated. `Typed` carries the variant name so the
/// receipt can show which error surface absorbed the mutation.
enum Dec {
    Exact(Vec<u8>),
    Typed(&'static str, String),
    Panic(String),
}

fn panic_msg(p: Box<dyn Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

fn git_bucket(e: &GitError) -> &'static str {
    match e {
        GitError::TooManyObjects(_) => "TooManyObjects",
        GitError::PackChecksumMismatch => "PackChecksumMismatch",
        GitError::IdxChecksumMismatch => "IdxChecksumMismatch",
        GitError::BadMagic => "BadMagic",
        GitError::Truncated => "Truncated",
        GitError::BadVarint => "BadVarint",
        GitError::SizeOverflow => "SizeOverflow",
        GitError::OffsetTooLarge => "OffsetTooLarge",
    }
}

fn inflate_bucket(e: &InflateError) -> &'static str {
    match e {
        InflateError::Truncated => "Truncated",
        InflateError::BadZlibHeader => "BadZlibHeader",
        InflateError::BadBlockType(_) => "BadBlockType",
        InflateError::BadCodeLengths => "BadCodeLengths",
        InflateError::BadCodeLengthRepeat => "BadCodeLengthRepeat",
        InflateError::MissingEndOfBlock => "MissingEndOfBlock",
        InflateError::OverlongBackreference { .. } => "OverlongBackreference",
        InflateError::BadDistanceCode => "BadDistanceCode",
        InflateError::AdlerMismatch { .. } => "AdlerMismatch",
    }
}

/// zlib reader #1: `deflate::zlib_decompress` (GitError).
fn dec_git(input: &[u8]) -> Dec {
    match catch_unwind(AssertUnwindSafe(|| zlib_decompress(input))) {
        Err(p) => Dec::Panic(panic_msg(p)),
        Ok(Ok(v)) => Dec::Exact(v),
        Ok(Err(e)) => Dec::Typed(git_bucket(&e), e.to_string()),
    }
}

/// zlib reader #2: `inflate::zlib_decode` (InflateError) — the independent
/// second implementation must hold the same contract.
fn dec_inflate(input: &[u8]) -> Dec {
    match catch_unwind(AssertUnwindSafe(|| zlib_decode(input))) {
        Err(p) => Dec::Panic(panic_msg(p)),
        Ok(Ok(v)) => Dec::Exact(v),
        Ok(Err(e)) => Dec::Typed(inflate_bucket(&e), e.to_string()),
    }
}

/// Raw reader #1: `deflate::inflate` (GitError).
fn dec_git_raw(input: &[u8]) -> Dec {
    match catch_unwind(AssertUnwindSafe(|| deflate_inflate(input))) {
        Err(p) => Dec::Panic(panic_msg(p)),
        Ok(Ok((v, _used))) => Dec::Exact(v),
        Ok(Err(e)) => Dec::Typed(git_bucket(&e), e.to_string()),
    }
}

/// Raw reader #2: `inflate::inflate` (InflateError).
fn dec_inflate_raw(input: &[u8]) -> Dec {
    match catch_unwind(AssertUnwindSafe(|| raw_inflate(input))) {
        Err(p) => Dec::Panic(panic_msg(p)),
        Ok(Ok(v)) => Dec::Exact(v),
        Ok(Err(e)) => Dec::Typed(inflate_bucket(&e), e.to_string()),
    }
}

fn tally(t: &mut Vec<(&'static str, usize)>, k: &'static str) {
    for e in t.iter_mut() {
        if e.0 == k {
            e.1 += 1;
            return;
        }
    }
    t.push((k, 1));
}

fn buckets_str(t: &[(&'static str, usize)]) -> String {
    let mut s = String::new();
    for (k, n) in t {
        if !s.is_empty() {
            s.push_str(", ");
        }
        s.push_str(&format!("{k}={n}"));
    }
    s
}

fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    a.iter().zip(b.iter()).position(|(x, y)| x != y).or_else(|| {
        if a.len() != b.len() {
            Some(core::cmp::min(a.len(), b.len()))
        } else {
            None
        }
    })
}

fn distinct_flip_positions(rng: &mut Lcg, n_bits: usize, k: usize) -> Vec<usize> {
    let mut v = Vec::with_capacity(k);
    while v.len() < k {
        let p = rng.below(n_bits);
        if !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

fn apply_flips(stream: &[u8], flips: &[usize]) -> Vec<u8> {
    let mut m = stream.to_vec();
    for &b in flips {
        m[b / 8] ^= 1u8 << (b % 8);
    }
    m
}

// ---- clause 1: round-trip over 200 entropy-mixture corpora ------------------

#[test]
fn fuzz_roundtrip_200_corpora_entropy_mixtures() {
    let mut rng = Lcg::new(SEED_ROUNDTRIP);
    let mut min_size = usize::MAX;
    let mut max_size = 0usize;
    for case in 0..200usize {
        // Deterministic draw order: size, then class, then corpus bytes.
        let (size, class) = match case {
            0 => (0usize, 0u32),
            1 => (1usize, 1u32),
            _ => {
                let s = rng.uniform(0, 65536);
                let c = rng.below(7) as u32;
                (s, c)
            }
        };
        let data = gen_corpus(&mut rng, class, size);
        min_size = min_size.min(size);
        max_size = max_size.max(size);

        let z = zlib_compress(&data);
        assert_eq!(
            z,
            zlib_compress(&data),
            "case {case}: compressor not deterministic"
        );

        let back = zlib_decompress(&z)
            .unwrap_or_else(|e| panic!("case {case} size {size} class {class}: zlib_decompress failed: {e}"));
        assert_eq!(
            back, data,
            "case {case} size {size} class {class}: deflate::zlib_decompress round-trip mismatch"
        );

        let back2 = zlib_decode(&z)
            .unwrap_or_else(|e| panic!("case {case} size {size} class {class}: zlib_decode failed: {e}"));
        assert_eq!(
            back2, data,
            "case {case} size {size} class {class}: inflate::zlib_decode round-trip mismatch"
        );

        let d = deflate_fixed(&data);
        let (back3, used) = deflate_inflate(&d)
            .unwrap_or_else(|e| panic!("case {case}: raw deflate::inflate failed: {e}"));
        assert_eq!(used, d.len(), "case {case}: consumed bytes != stream length");
        assert_eq!(back3, data, "case {case}: raw round-trip mismatch (deflate::inflate)");

        let body = &z[2..z.len() - 4];
        let back4 = raw_inflate(body)
            .unwrap_or_else(|e| panic!("case {case}: raw inflate::inflate failed: {e}"));
        assert_eq!(back4, data, "case {case}: raw round-trip mismatch (inflate::inflate)");
    }
    eprintln!(
        "[fuzz_codec] roundtrip: cases=200 ok=200 sizes={min_size}..={max_size} \
         readers=4 (zlib_decompress, zlib_decode, deflate::inflate, inflate::inflate)"
    );
    assert!(min_size == 0 && max_size <= 65536);
}

// ---- clause 2: 500 bit-flip mutations ---------------------------------------

#[test]
fn fuzz_bitflip_500_typed_error_or_exact_bytes() {
    // Base streams: fixed-Huffman zlib from 7 corpora + one hand-built
    // stored-block stream (its data region is raw, so a single flip there is
    // a guaranteed adler-visible single-byte delta).
    let mut brng = Lcg::new(SEED_BITFLIP_BASES);
    let specs: [(u32, usize); 8] = [
        (0, 0),
        (1, 1),
        (3, 64),
        (0, 512),
        (2, 2048),
        (4, 4096),
        (1, 1024),
        (4, 300),
    ];
    let bases: Vec<(Vec<u8>, Vec<u8>)> = specs
        .iter()
        .map(|&(cls, size)| {
            let data = gen_corpus(&mut brng, cls, size);
            let stream = if size == 300 {
                zlib_stored_stream(&data, 128)
            } else {
                zlib_compress(&data)
            };
            (data, stream)
        })
        .collect();

    let mut rng = Lcg::new(SEED_BITFLIP_MUT);
    let (mut r1_exact, mut r1_err, mut r2_exact, mut r2_err) = (0usize, 0usize, 0usize, 0usize);
    let (mut flips1, mut flips2, mut flips3) = (0usize, 0usize, 0usize);
    let mut silent: Vec<String> = Vec::new();
    let mut panics: Vec<String> = Vec::new();
    let mut git_tally: Vec<(&'static str, usize)> = Vec::new();
    let mut inf_tally: Vec<(&'static str, usize)> = Vec::new();

    for case in 0..500usize {
        let bi = case % bases.len();
        let (orig, stream) = &bases[bi];
        let k = case % 3 + 1;
        match k {
            1 => flips1 += 1,
            2 => flips2 += 1,
            _ => flips3 += 1,
        }
        let flips = distinct_flip_positions(&mut rng, stream.len() * 8, k);
        let mutated = apply_flips(stream, &flips);

        for (which, dec) in [("zlib_decompress", dec_git(&mutated)), ("zlib_decode", dec_inflate(&mutated))] {
            match dec {
                Dec::Panic(m) => panics.push(format!(
                    "bitflip case {case} base {bi} reader {which} flips {flips:?}: PANIC: {m}"
                )),
                Dec::Exact(v) => {
                    if which == "zlib_decompress" {
                        r1_exact += 1
                    } else {
                        r2_exact += 1
                    }
                    if v != *orig {
                        let at = first_diff(&v, orig);
                        silent.push(format!(
                            "bitflip case {case} base {bi} reader {which} flips {flips:?}: \
                             WRONG BYTES ACCEPTED (len {} vs {}, first diff at {at:?})",
                            v.len(),
                            orig.len()
                        ));
                    }
                }
                Dec::Typed(bucket, msg) => {
                    if which == "zlib_decompress" {
                        r1_err += 1;
                        tally(&mut git_tally, bucket);
                    } else {
                        r2_err += 1;
                        tally(&mut inf_tally, bucket);
                    }
                    let _ = msg;
                }
            }
        }
    }

    eprintln!(
        "[fuzz_codec] bitflip: mutations=500 (1-bit={flips1}, 2-bit={flips2}, 3-bit={flips3}) \
         zlib_decompress exact={r1_exact} typed_err={r1_err} | zlib_decode exact={r2_exact} \
         typed_err={r2_err}"
    );
    eprintln!("[fuzz_codec] bitflip GitError buckets: {}", buckets_str(&git_tally));
    eprintln!("[fuzz_codec] bitflip InflateError buckets: {}", buckets_str(&inf_tally));

    assert_eq!(flips1 + flips2 + flips3, 500, "court must run exactly 500 mutations");
    assert_eq!(r1_exact + r1_err, 500, "reader 1 must classify all 500 mutations");
    assert_eq!(r2_exact + r2_err, 500, "reader 2 must classify all 500 mutations");
    assert!(
        panics.is_empty(),
        "{} decode PANICS (the finding):\n{}",
        panics.len(),
        panics.join("\n")
    );
    assert!(
        silent.is_empty(),
        "{} SILENT-CORRUPTION findings (wrong bytes accepted):\n{}",
        silent.len(),
        silent.join("\n")
    );
}

// ---- clause 3: truncation sweep over every prefix of 20 streams --------------

#[test]
fn fuzz_truncation_sweep_20_streams_every_prefix() {
    let mut brng = Lcg::new(SEED_TRUNCATION);
    let specs: [(u32, usize); 17] = [
        (0, 0),
        (1, 1),
        (3, 3),
        (2, 17),
        (4, 64),
        (0, 255),
        (1, 256),
        (3, 511),
        (4, 512),
        (2, 1000),
        (0, 1024),
        (1, 2047),
        (4, 2048),
        (3, 4095),
        (2, 4096),
        (5, 777),
        (6, 1500),
    ];
    let mut streams: Vec<Vec<u8>> = Vec::new();
    let mut originals: Vec<Vec<u8>> = Vec::new();
    for &(cls, size) in &specs {
        let data = gen_corpus(&mut brng, cls, size);
        streams.push(zlib_compress(&data));
        originals.push(data);
    }
    // Stored-block family: multi-block, single-block, many-small-blocks.
    let s1 = gen_corpus(&mut brng, 4, 250);
    streams.push(zlib_stored_stream(&s1, 100));
    originals.push(s1);
    let s2 = gen_corpus(&mut brng, 0, 300);
    streams.push(zlib_stored_stream(&s2, 65535));
    originals.push(s2);
    let s3 = gen_corpus(&mut brng, 2, 1000);
    streams.push(zlib_stored_stream(&s3, 64));
    originals.push(s3);
    assert_eq!(streams.len(), 20, "court contract: 20 valid truncation streams");

    let (mut prefixes, mut r1_ok, mut r1_err, mut r2_ok, mut r2_err) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut silent: Vec<String> = Vec::new();
    let mut panics: Vec<String> = Vec::new();

    for (si, stream) in streams.iter().enumerate() {
        let orig = &originals[si];
        assert_eq!(
            zlib_decompress(stream).as_deref(),
            Ok(&orig[..]),
            "stream {si} anchor: full stream must decode before sweeping prefixes"
        );
        for prefix in 0..=stream.len() {
            prefixes += 1;
            let p = &stream[..prefix];
            for (which, dec) in [("zlib_decompress", dec_git(p)), ("zlib_decode", dec_inflate(p))] {
                match dec {
                    Dec::Panic(m) => panics.push(format!(
                        "truncation stream {si} prefix {prefix}/{}/ reader {which}: PANIC: {m}",
                        stream.len()
                    )),
                    Dec::Exact(v) => {
                        if which == "zlib_decompress" {
                            r1_ok += 1
                        } else {
                            r2_ok += 1
                        }
                        if v != *orig {
                            let at = first_diff(&v, orig);
                            silent.push(format!(
                                "truncation stream {si} prefix {prefix}/{} reader {which}: \
                                 WRONG BYTES ACCEPTED (first diff at {at:?})",
                                stream.len()
                            ));
                        }
                    }
                    Dec::Typed(..) => {
                        if which == "zlib_decompress" {
                            r1_err += 1
                        } else {
                            r2_err += 1
                        }
                    }
                }
            }
        }
    }

    // Raw-deflate mini-sweep: every prefix of 4 fixed-Huffman raw streams
    // through both raw inflate readers.
    let (mut raw_ok, mut raw_err) = (0usize, 0usize);
    for (ri, &(cls, size)) in [(0u32, 512usize), (2, 1024), (4, 2048), (1, 4096)]
        .iter()
        .enumerate()
    {
        let data = gen_corpus(&mut brng, cls, size);
        let d = deflate_fixed(&data);
        for prefix in 0..=d.len() {
            let p = &d[..prefix];
            for (which, dec) in [
                ("deflate::inflate", dec_git_raw(p)),
                ("inflate::inflate", dec_inflate_raw(p)),
            ] {
                match dec {
                    Dec::Panic(m) => panics.push(format!(
                        "raw truncation stream {ri} prefix {prefix}/{}/ reader {which}: PANIC: {m}",
                        d.len()
                    )),
                    Dec::Exact(v) => {
                        raw_ok += 1;
                        if v != data {
                            let at = first_diff(&v, &data);
                            silent.push(format!(
                                "raw truncation stream {ri} prefix {prefix}/{} reader {which}: \
                                 WRONG BYTES ACCEPTED (first diff at {at:?})",
                                d.len()
                            ));
                        }
                    }
                    Dec::Typed(..) => raw_err += 1,
                }
            }
        }
    }

    eprintln!(
        "[fuzz_codec] truncation: streams=20 prefixes={prefixes} zlib_decompress \
         exact_ok={r1_ok} typed_err={r1_err} | zlib_decode exact_ok={r2_ok} typed_err={r2_err} \
         | raw 4-stream sweep exact_ok={raw_ok} typed_err={raw_err}"
    );
    assert_eq!(
        prefixes,
        streams.iter().map(|s| s.len() + 1).sum::<usize>(),
        "every prefix of every stream must be exercised"
    );
    // A strict prefix of these single-final-block streams cannot contain the
    // completed final block plus trailer, so exactly 20 full-length prefixes
    // may decode to the exact original bytes.
    assert_eq!(r1_ok, 20, "reader 1: only full-length prefixes may decode");
    assert_eq!(r2_ok, 20, "reader 2: only full-length prefixes may decode");
    assert_eq!(raw_ok, 8, "raw sweep: 4 streams x 2 readers, full prefix only");
    assert!(
        panics.is_empty(),
        "{} truncation PANICS (the finding):\n{}",
        panics.len(),
        panics.join("\n")
    );
    assert!(
        silent.is_empty(),
        "{} SILENT-CORRUPTION findings (wrong bytes accepted):\n{}",
        silent.len(),
        silent.join("\n")
    );
}

// ---- non-vacuity: the gate must be able to flip ------------------------------

#[test]
fn court_nonvacuity_controlled_mutations_must_flip() {
    let data = b"COURT-NONVACUITY-PROBE-0123456789".to_vec();
    let stream = zlib_stored_stream(&data, 16); // 3 stored blocks (16/16/1)
    assert_eq!(stream.len(), 2 + 3 * 5 + 33 + 4);
    // Layout anchor: data byte 8 sits in block 0 at stream index 2+5+8.
    assert_eq!(stream[15], data[8], "stored layout anchor moved");

    // Pristine stream must PASS (not a constant-fail gate).
    assert_eq!(zlib_decompress(&stream).as_deref(), Ok(&data[..]));
    assert_eq!(zlib_decode(&stream).as_deref(), Ok(&data[..]));

    // (a) single flip in stored data -> decoded byte differs -> must refuse.
    let mut m = stream.clone();
    m[15] ^= 0x04;
    assert!(
        zlib_decompress(&m).is_err(),
        "VACUOUS GATE: stored-data bit flip accepted by zlib_decompress"
    );
    assert!(
        zlib_decode(&m).is_err(),
        "VACUOUS GATE: stored-data bit flip accepted by zlib_decode"
    );

    // (b) single flip in the adler trailer -> must refuse.
    let mut m = stream.clone();
    let n = m.len();
    m[n - 1] ^= 0x80;
    assert!(zlib_decompress(&m).is_err(), "VACUOUS GATE: adler flip accepted (reader 1)");
    assert!(zlib_decode(&m).is_err(), "VACUOUS GATE: adler flip accepted (reader 2)");

    // (c) single flip in the zlib header (0x78^0x01=0x79 -> check%31 != 0)
    //     -> must refuse with the header error, deterministically.
    let mut m = stream.clone();
    m[0] ^= 0x01;
    assert!(zlib_decompress(&m).is_err(), "VACUOUS GATE: header flip accepted (reader 1)");
    assert!(zlib_decode(&m).is_err(), "VACUOUS GATE: header flip accepted (reader 2)");
    assert_eq!(zlib_decode(&m), Err(InflateError::BadZlibHeader));
    eprintln!("[fuzz_codec] nonvacuity: 3 controlled mutations flipped the gate; pristine anchor passed");
}
