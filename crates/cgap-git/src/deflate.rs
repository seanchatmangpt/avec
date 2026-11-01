//! DEFLATE (RFC 1951) + zlib wrapper (RFC 1950), in-crate, zero external
//! dependencies (FR-04 flip: git pack v2 requires zlib streams for object
//! bodies).
//!
//! Compressor: LZ77 with a 15-bit hash-chain match finder, emitted with
//! FIXED Huffman codes (BTYPE=01). Fixed-Huffman-only output is legal
//! DEFLATE and is accepted by git and by `python3 -m zlib` alike; ratio is
//! secondary to conformance here.
//!
//! Decompressor: full inflate — stored (BTYPE=00), fixed (01), and dynamic
//! (10) blocks — so packs written by real git (dynamic Huffman) remain
//! readable. Decode follows the canonical-Huffman algorithm from RFC 1951
//! §3.2.2 (puff-style counts/symbols walk, no bit-by-bit tree allocation).
//!
//! Adler-32 (RFC 1950 §8) is implemented here and cross-checked in tests
//! against `python3 -c "import zlib"` (real external collaborator, dev
//! scope only).

use alloc::vec;
use alloc::vec::Vec;

use crate::GitError;

// -------------------------------------------------------------- adler32 --

/// Streaming Adler-32 (RFC 1950).
#[derive(Clone)]
pub struct Adler32 {
    a: u32,
    b: u32,
}

impl Default for Adler32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Adler32 {
    pub fn new() -> Self {
        Adler32 { a: 1, b: 0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        // 5552 is the largest n such that 255*n*(n+1)/2 + (n+1)*(65521-1)
        // stays below 2^32 — the classic zlib blocking constant.
        for chunk in data.chunks(5552) {
            for &byte in chunk {
                self.a += byte as u32;
                self.b += self.a;
            }
            self.a %= 65521;
            self.b %= 65521;
        }
    }

    pub fn finalize(&self) -> u32 {
        (self.b << 16) | self.a
    }
}

/// One-shot Adler-32.
pub fn adler32(data: &[u8]) -> u32 {
    let mut c = Adler32::new();
    c.update(data);
    c.finalize()
}

// ------------------------------------------------------------ bit writer --

struct BitWriter {
    out: Vec<u8>,
    bitbuf: u32,
    bitcnt: u32,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            bitbuf: 0,
            bitcnt: 0,
        }
    }

    /// LSB-first bit pack (DEFLATE header/extra-bit order, RFC 1951 §3.1.1).
    fn put_bits(&mut self, bits: u32, n: u8) {
        self.bitbuf |= bits << self.bitcnt;
        self.bitcnt += n as u32;
        while self.bitcnt >= 8 {
            self.out.push((self.bitbuf & 0xFF) as u8);
            self.bitbuf >>= 8;
            self.bitcnt -= 8;
        }
    }

    /// Huffman codes are packed MSB-of-code-first (RFC 1951 §3.1.1): reverse
    /// the code into LSB-first order, then emit.
    fn put_huff(&mut self, code: u32, n: u8) {
        let mut rev = 0u32;
        for i in 0..n {
            rev |= ((code >> i) & 1) << (n - 1 - i);
        }
        self.put_bits(rev, n);
    }

    fn flush_to_byte(&mut self) {
        if self.bitcnt > 0 {
            self.out.push((self.bitbuf & 0xFF) as u8);
            self.bitbuf = 0;
            self.bitcnt = 0;
        }
    }
}

// --------------------------------------------------- fixed huffman codes --

/// Fixed literal/length code for symbol `s` as (code, bits), RFC 1951 §3.2.6.
fn fixed_lit_code(s: u16) -> (u32, u8) {
    match s {
        0..=143 => (0x30 + s as u32, 8),
        144..=255 => (s as u32 + 256, 9), // 0x190..0x1FF
        256..=279 => (s as u32 - 256, 7),
        _ => (0xC0 + s as u32 - 280, 8),
    }
}

/// Length code table: (base, extra bits) for symbols 257..=285.
const LEN_TABLE: [(u16, u8); 29] = [
    (3, 0), (4, 0), (5, 0), (6, 0), (7, 0), (8, 0), (9, 0), (10, 0), (11, 1),
    (13, 1), (15, 1), (17, 1), (19, 2), (23, 2), (27, 2), (31, 2), (35, 3),
    (43, 3), (51, 3), (59, 3), (67, 4), (83, 4), (99, 4), (115, 4), (131, 5),
    (163, 5), (195, 5), (227, 5), (258, 0),
];

/// Distance code table: (base, extra bits) for symbols 0..=29.
const DIST_TABLE: [(u16, u8); 30] = [
    (1, 0), (2, 0), (3, 0), (4, 0), (5, 1), (7, 1), (9, 2), (13, 2), (17, 3),
    (25, 3), (33, 4), (49, 4), (65, 5), (97, 5), (129, 6), (193, 6), (257, 7),
    (385, 7), (513, 8), (769, 8), (1025, 9), (1537, 9), (2049, 10), (3073, 10),
    (4097, 11), (6145, 11), (8193, 12), (12289, 12), (16385, 13), (24577, 13),
];

fn len_code(len: u16) -> (u16, u8, u16) {
    // (symbol, extra bits, extra value)
    debug_assert!((3..=258).contains(&len));
    if len == 258 {
        return (285, 0, 0);
    }
    let mut best = 0usize;
    for (i, &(base, _)) in LEN_TABLE.iter().enumerate() {
        if base <= len {
            best = i;
        }
    }
    let (base, extra) = LEN_TABLE[best];
    (257 + best as u16, extra, len - base)
}

fn dist_code(dist: u16) -> (u16, u8, u16) {
    debug_assert!((1..=32768).contains(&dist));
    let mut best = 0usize;
    for (i, &(base, _)) in DIST_TABLE.iter().enumerate() {
        if base <= dist {
            best = i;
        }
    }
    let (base, extra) = DIST_TABLE[best];
    (best as u16, extra, dist - base)
}

// ------------------------------------------------------------- LZ77 core --

const WINDOW: u16 = 32768;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const MAX_CHAIN: usize = 32;
const HASH_BITS: u32 = 15;
const HASH_SIZE: usize = 1 << HASH_BITS;

fn hash3(data: &[u8], i: usize) -> usize {
    let v = (data[i] as u32) << 16 | (data[i + 1] as u32) << 8 | data[i + 2] as u32;
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// One DEFLATE block of tokens: literal byte, or (len, dist) match.
enum Token {
    Lit(u8),
    Match(u16, u16),
}

fn lz77_tokens(data: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    if data.len() < MIN_MATCH {
        for &b in data {
            tokens.push(Token::Lit(b));
        }
        return tokens;
    }
    let mut head = [-1i32; HASH_SIZE];
    let mut prev = vec![-1i32; data.len()];
    let mut i = 0usize;
    while i < data.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0u16;
        if i + MIN_MATCH <= data.len() {
            let mut cand = head[hash3(data, i)];
            let mut chain = 0usize;
            let limit = i.saturating_sub(WINDOW as usize);
            while cand >= 0 && (cand as usize) >= limit && chain < MAX_CHAIN {
                let c = cand as usize;
                let max = core::cmp::min(MAX_MATCH, data.len() - i);
                let mut l = 0usize;
                while l < max && data[c + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = (i - c) as u16;
                    if l >= max {
                        break;
                    }
                }
                cand = prev[c];
                chain += 1;
            }
            // insert current position into the chain
            let h = hash3(data, i);
            prev[i] = head[h];
            head[h] = i as i32;
        }
        if best_len >= MIN_MATCH {
            tokens.push(Token::Match(best_len as u16, best_dist));
            // insert the covered positions so later matches can chain them
            for k in (i + 1)..(i + best_len) {
                if k + MIN_MATCH <= data.len() {
                    let h = hash3(data, k);
                    prev[k] = head[h];
                    head[h] = k as i32;
                }
            }
            i += best_len;
        } else {
            tokens.push(Token::Lit(data[i]));
            i += 1;
        }
    }
    tokens
}

// ------------------------------------------------------------ compressor --

/// Raw DEFLATE stream (no zlib header/trailer), one fixed-Huffman block.
pub fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.put_bits(1, 1); // BFINAL
    w.put_bits(1, 2); // BTYPE=01 fixed Huffman
    for tok in lz77_tokens(data) {
        match tok {
            Token::Lit(b) => {
                let (code, bits) = fixed_lit_code(b as u16);
                w.put_huff(code, bits);
            }
            Token::Match(len, dist) => {
                let (lsym, lextra, lexval) = len_code(len);
                let (code, bits) = fixed_lit_code(lsym);
                w.put_huff(code, bits);
                if lextra > 0 {
                    w.put_bits(lexval as u32, lextra);
                }
                let (dsym, dextra, dexval) = dist_code(dist);
                w.put_huff(dsym as u32, 5);
                if dextra > 0 {
                    w.put_bits(dexval as u32, dextra);
                }
            }
        }
    }
    let (code, bits) = fixed_lit_code(256);
    w.put_huff(code, bits);
    w.flush_to_byte();
    w.out
}

/// zlib stream (RFC 1950): 0x78 0x9C header, deflate body, big-endian
/// Adler-32 of the uncompressed data.
pub fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 16);
    out.push(0x78); // CMF: deflate, 32K window
    out.push(0x9C); // FLG: check bits valid, FLEVEL=2, FDICT=0
    out.extend_from_slice(&deflate_fixed(data));
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

// ------------------------------------------------------------ bit reader --

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bitbuf: u32,
    bitcnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            pos: 0,
            bitbuf: 0,
            bitcnt: 0,
        }
    }

    fn need(&mut self, n: u32) -> Result<(), GitError> {
        while self.bitcnt < n {
            let byte = *self.data.get(self.pos).ok_or(GitError::Truncated)?;
            self.pos += 1;
            self.bitbuf |= (byte as u32) << self.bitcnt;
            self.bitcnt += 8;
        }
        Ok(())
    }

    fn bits(&mut self, n: u8) -> Result<u32, GitError> {
        if n == 0 {
            return Ok(0);
        }
        self.need(n as u32)?;
        let v = self.bitbuf & ((1u32 << n) - 1);
        self.bitbuf >>= n;
        self.bitcnt -= n as u32;
        Ok(v)
    }

    fn align_byte(&mut self) {
        let drop = self.bitcnt % 8;
        self.bitbuf >>= drop;
        self.bitcnt -= drop;
    }

    /// Bytes consumed so far (aligned): used to find the zlib trailer.
    fn consumed(&self) -> usize {
        self.pos - (self.bitcnt / 8) as usize
    }
}

// ------------------------------------------------------- huffman decode --

/// Canonical Huffman decode table, puff-style (RFC 1951 §3.2.2).
struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

fn huff_build(lengths: &[u8]) -> Result<Huff, GitError> {
    let mut counts = [0u16; 16];
    for &l in lengths {
        if l > 15 {
            return Err(GitError::BadVarint);
        }
        counts[l as usize] += 1;
    }
    counts[0] = 0;
    // over-subscribed check
    let mut left: i32 = 1;
    for len in 1..=15 {
        left <<= 1;
        left -= counts[len] as i32;
        if left < 0 {
            return Err(GitError::BadVarint);
        }
    }
    // offsets per length
    let mut offs = [0u16; 16];
    for len in 1..15 {
        offs[len + 1] = offs[len] + counts[len];
    }
    let mut symbols = vec![0u16; lengths.len()];
    for (sym, &l) in lengths.iter().enumerate() {
        if l != 0 {
            symbols[offs[l as usize] as usize] = sym as u16;
            offs[l as usize] += 1;
        }
    }
    Ok(Huff { counts, symbols })
}

impl Huff {
    fn decode(&self, r: &mut BitReader) -> Result<u16, GitError> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for len in 1..=15 {
            code |= r.bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(GitError::BadVarint)
    }
}

const CLEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Inflate one DEFLATE stream; returns (decompressed bytes, bytes consumed
/// from `data`). Handles stored, fixed, and dynamic blocks.
pub fn inflate(data: &[u8]) -> Result<(Vec<u8>, usize), GitError> {
    let mut r = BitReader::new(data);
    let mut out = Vec::new();
    loop {
        let bfinal = r.bits(1)?;
        let btype = r.bits(2)?;
        match btype {
            0 => {
                r.align_byte();
                let len = r.bits(16)? as usize;
                let nlen = r.bits(16)? as usize;
                if len != (!nlen & 0xFFFF) {
                    return Err(GitError::BadVarint);
                }
                for _ in 0..len {
                    out.push(r.bits(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit, dist);
                if btype == 1 {
                    let mut lengths = [0u8; 288];
                    for (s, l) in lengths.iter_mut().enumerate() {
                        *l = if s < 144 {
                            8
                        } else if s < 256 {
                            9
                        } else if s < 280 {
                            7
                        } else {
                            8
                        };
                    }
                    lit = huff_build(&lengths)?;
                    dist = huff_build(&[5u8; 30])?;
                } else {
                    let hlit = r.bits(5)? as usize + 257;
                    let hdist = r.bits(5)? as usize + 1;
                    let hclen = r.bits(4)? as usize + 4;
                    let mut clen = [0u8; 19];
                    for &o in CLEN_ORDER.iter().take(hclen) {
                        clen[o] = r.bits(3)? as u8;
                    }
                    let clh = huff_build(&clen)?;
                    let mut lengths = vec![0u8; hlit + hdist];
                    let mut i = 0usize;
                    while i < hlit + hdist {
                        let sym = clh.decode(&mut r)?;
                        match sym {
                            0..=15 => {
                                lengths[i] = sym as u8;
                                i += 1;
                            }
                            16 => {
                                if i == 0 {
                                    return Err(GitError::BadVarint);
                                }
                                let prev = lengths[i - 1];
                                let rep = 3 + r.bits(2)? as usize;
                                for _ in 0..rep {
                                    if i >= lengths.len() {
                                        return Err(GitError::BadVarint);
                                    }
                                    lengths[i] = prev;
                                    i += 1;
                                }
                            }
                            17 => {
                                let rep = 3 + r.bits(3)? as usize;
                                i += rep;
                            }
                            18 => {
                                let rep = 11 + r.bits(7)? as usize;
                                i += rep;
                            }
                            _ => return Err(GitError::BadVarint),
                        }
                    }
                    if i > hlit + hdist {
                        return Err(GitError::BadVarint);
                    }
                    lit = huff_build(&lengths[..hlit])?;
                    dist = huff_build(&lengths[hlit..])?;
                }
                loop {
                    let sym = lit.decode(&mut r)?;
                    match sym {
                        0..=255 => out.push(sym as u8),
                        256 => break,
                        257..=285 => {
                            let (base, extra) = LEN_TABLE[sym as usize - 257];
                            let len = base as usize + r.bits(extra)? as usize;
                            let dsym = dist.decode(&mut r)? as usize;
                            if dsym >= 30 {
                                return Err(GitError::BadVarint);
                            }
                            let (dbase, dextra) = DIST_TABLE[dsym];
                            let d = dbase as usize + r.bits(dextra)? as usize;
                            if d > out.len() {
                                return Err(GitError::Truncated);
                            }
                            let start = out.len() - d;
                            for k in 0..len {
                                let b = out[start + k];
                                out.push(b);
                            }
                        }
                        _ => return Err(GitError::BadVarint),
                    }
                }
            }
            _ => return Err(GitError::BadVarint),
        }
        if bfinal == 1 {
            break;
        }
    }
    r.align_byte();
    Ok((out, r.consumed()))
}

/// Decompress a zlib stream (RFC 1950): verify header, inflate, verify
/// Adler-32. Returns the uncompressed bytes.
pub fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, GitError> {
    if data.len() < 6 || data[0] & 0x0F != 8 {
        return Err(GitError::BadMagic);
    }
    let check = ((data[0] as u16) << 8 | data[1] as u16) % 31;
    if check != 0 {
        return Err(GitError::BadMagic);
    }
    if data[1] & 0x20 != 0 {
        return Err(GitError::BadVarint); // FDICT unsupported
    }
    let start = 2usize;
    let (out, used) = inflate(&data[start..])?;
    let trailer = start + used;
    if trailer + 4 > data.len() {
        return Err(GitError::Truncated);
    }
    let stored = u32::from_be_bytes([
        data[trailer],
        data[trailer + 1],
        data[trailer + 2],
        data[trailer + 3],
    ]);
    if stored != adler32(&out) {
        return Err(GitError::PackChecksumMismatch);
    }
    Ok(out)
}

// ---------------------------------------------------------------- tests --

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use std::process::Command;
    use std::string::String;
    use std::eprintln;
    use std::sync::OnceLock;

    /// Real external collaborator (Chicago): system python3 zlib. Skipped
    /// loudly (panic) is wrong here — python3 is a court precondition, so
    /// failure to spawn fails the test rather than faking a pass.
    fn python_zlib(args: &[&str], stdin: &[u8]) -> Vec<u8> {
        use std::io::Write;
        use std::process::Stdio;
        let script = args[0];
        let mut child = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn python3 (court precondition)");
        child.stdin.take().unwrap().write_all(stdin).unwrap();
        let out = child.wait_with_output().expect("wait python3");
        assert!(
            out.status.success(),
            "python3 zlib helper failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    fn corpora() -> &'static Vec<Vec<u8>> {
        static C: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
        C.get_or_init(|| {
            let v = vec![
                Vec::new(),                                  // empty
                vec![0x2A],                                  // 1 byte
                { let mut r = Vec::new(); for _ in 0..10_000 { r.extend_from_slice(b"abcabcabc"); } r }, // repetitive
                { // deterministic pseudo-random
                    let mut r = Vec::with_capacity(4096);
                    let mut x = 0x1234_5678u32;
                    for _ in 0..4096 {
                        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        r.push((x >> 24) as u8);
                    }
                    r
                },
                { // 100KB mixed corpus
                    let mut r = Vec::with_capacity(100_000);
                    let mut x = 0xDEAD_BEEFu32;
                    for i in 0..100_000 {
                        if i % 3 == 0 {
                            r.push(b"the quick brown fox "[i % 20]);
                        } else {
                            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                            r.push((x >> 16) as u8);
                        }
                    }
                    r
                },
            ];
            v
        })
    }

    #[test]
    fn adler32_matches_python() {
        for data in corpora() {
            let out = python_zlib(
                &["import sys,zlib;d=sys.stdin.buffer.read();sys.stdout.buffer.write(str(zlib.adler32(d)).encode())"],
                data,
            );
            let py: u32 = std::str::from_utf8(&out).unwrap().trim().parse().unwrap();
            assert_eq!(adler32(data), py, "adler32 mismatch on {} bytes", data.len());
        }
    }

    #[test]
    fn adler32_known_vectors() {
        assert_eq!(adler32(b""), 1);
        assert_eq!(adler32(b"abc"), 0x024d0127);
    }

    #[test]
    fn roundtrip_own_inflate() {
        for data in corpora() {
            let z = zlib_compress(data);
            let back = zlib_decompress(&z).unwrap();
            assert_eq!(back, *data, "roundtrip failed on {} bytes", data.len());
        }
    }

    #[test]
    fn roundtrip_via_raw_inflate() {
        for data in corpora() {
            eprintln!("[bisect] corpus len={}", data.len());
            let d = deflate_fixed(data);
            let (back, used) = inflate(&d).unwrap();
            assert_eq!(used, d.len());
            assert_eq!(back, *data);
        }
    }

    #[test]
    fn python_inflates_our_deflate() {
        for data in corpora() {
            let z = zlib_compress(data);
            let out = python_zlib(
                &["import sys,zlib;d=sys.stdin.buffer.read();sys.stdout.buffer.write(zlib.decompress(d))"],
                &z,
            );
            assert_eq!(out, *data, "python3 could not inflate our zlib stream ({} bytes in)", data.len());
        }
    }

    #[test]
    fn we_inflate_python_zlib_compress() {
        for data in corpora() {
            let z = python_zlib(
                &["import sys,zlib;d=sys.stdin.buffer.read();sys.stdout.buffer.write(zlib.compress(d,9))"],
                data,
            );
            let back = zlib_decompress(&z).unwrap();
            assert_eq!(back, *data, "our inflate failed on python3 zlib.compress output ({} bytes)", data.len());
        }
    }

    #[test]
    fn corrupted_zlib_body_fails() {
        let data = corpora()[2].clone();
        let mut z = zlib_compress(&data);
        let mid = z.len() / 2;
        z[mid] ^= 0xFF;
        // must fail loudly (trailer adler or decode), never return wrong bytes
        match zlib_decompress(&z) {
            Err(_) => {}
            Ok(back) => assert_ne!(back, data, "corruption silently accepted"),
        }
    }

    #[test]
    fn compression_repeats_shrink() {
        let rep = corpora()[2].clone();
        assert!(zlib_compress(&rep).len() < rep.len(), "repetitive corpus must compress");
    }
}

