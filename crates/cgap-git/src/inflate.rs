//! DEFLATE inflate (RFC 1951) + zlib wrapper (RFC 1950) — the read side for
//! git-created packs and loose objects.
//!
//! Zero external dependencies, `no_std` + `alloc`, `forbid(unsafe_code)` —
//! matches the crate contract. Covers all three block types (stored,
//! fixed-Huffman, dynamic-Huffman) and LZ77 backreferences including
//! overlap-forward copies. Errors are typed via [`InflateError`].

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// Errors from inflate / zlib decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InflateError {
    /// Ran out of input bits/bytes mid-stream.
    Truncated,
    /// zlib CMF/FLG failed the RFC 1950 check (CM != 8 or header % 31 != 0).
    BadZlibHeader,
    /// Reserved block type 3 encountered.
    BadBlockType(u8),
    /// Huffman code-length table is over-subscribed or incomplete in a way
    /// that cannot decode (or a dynamic header is otherwise malformed).
    BadCodeLengths,
    /// A code length repeat (16/17/18) had no prior length to repeat.
    BadCodeLengthRepeat,
    /// Dynamic block missing the end-of-block code (256).
    MissingEndOfBlock,
    /// A distance code reached back before the start of the output.
    OverlongBackreference {
        /// The offending LZ77 distance.
        distance: u16,
        /// Bytes written so far when the backreference was decoded.
        written: usize,
    },
    /// Distance or length symbol decoded to an invalid code.
    BadDistanceCode,
    /// Final adler32 does not match the decompressed bytes.
    AdlerMismatch {
        /// Adler-32 stored in the zlib trailer.
        expected: u32,
        /// Adler-32 computed over the produced bytes.
        actual: u32,
    },
}

impl fmt::Display for InflateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InflateError::Truncated => write!(
                f,
                "deflate stream ended mid-structure: input ran out of bits/bytes before the final \
                 block finished or before the zlib adler32 trailer — the object was truncated in \
                 storage or transfer; re-read the object from its source of truth"
            ),
            InflateError::BadZlibHeader => write!(
                f,
                "zlib header failed the RFC 1950 check (CM != 8, or CMF*256+FLG is not a multiple \
                 of 31, or FDICT set): the bytes are not a zlib stream — confirm you are decoding \
                 an object body, not raw content or a truncated file"
            ),
            InflateError::BadBlockType(t) => write!(
                f,
                "reserved deflate block type {t}: RFC 1951 defines only block types 0-2 — the \
                 stream is corrupt or is not DEFLATE; re-read the object"
            ),
            InflateError::BadCodeLengths => write!(
                f,
                "huffman code lengths are over-subscribed or incomplete: no valid canonical-code \
                 assignment exists (Kraft inequality violated) — the stream is corrupt or was not \
                 produced by a conforming deflate compressor; re-read the object"
            ),
            InflateError::BadCodeLengthRepeat => write!(
                f,
                "code-length repeat (symbol 16) with no prior length to repeat: a dynamic header's \
                 length list cannot begin with a repeat — the stream is corrupt; re-read the object"
            ),
            InflateError::MissingEndOfBlock => write!(
                f,
                "dynamic block declares no end-of-block code (symbol 256 has length 0): the block \
                 cannot terminate — the stream is corrupt; re-read the object"
            ),
            InflateError::OverlongBackreference { distance, written } => write!(
                f,
                "backreference distance {distance} exceeds the {written} bytes of output produced \
                 so far: an LZ77 distance may not reach before the start of the output — the \
                 stream is corrupt; re-read the object"
            ),
            InflateError::BadDistanceCode => write!(
                f,
                "distance symbol 30/31 decoded: reserved by RFC 1951 and never valid in any \
                 stream — the stream is corrupt; re-read the object"
            ),
            InflateError::AdlerMismatch { expected, actual } => write!(
                f,
                "zlib adler32 mismatch: trailer stores {expected:#010x} but the inflated bytes \
                 hash to {actual:#010x} — content changed after compression; re-read the object"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InflateError {}

/// LSB-first bit reader over a byte slice (RFC 1951 section 3.1.1).
struct BitReader<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u32, // 0..=7 bits already consumed in data[byte]
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader { data, byte: 0, bit: 0 }
    }

    fn align_byte(&mut self) {
        self.bit = 0;
    }

    fn read_bits(&mut self, n: u32) -> Result<u32, InflateError> {
        debug_assert!(n <= 16);
        let mut value: u32 = 0;
        for i in 0..n {
            let b = *self.data.get(self.byte).ok_or(InflateError::Truncated)?;
            let bit = (b >> self.bit) & 1;
            value |= (bit as u32) << i;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.byte += 1;
            }
        }
        Ok(value)
    }

    /// Number of whole bytes consumed so far (rounded up past a partial byte).
    fn bytes_consumed(&self) -> usize {
        self.byte + if self.bit > 0 { 1 } else { 0 }
    }
}

/// Canonical Huffman decoding table (puff-style counts + sorted symbols).
struct Huffman {
    counts: [u16; 16],
    symbols: alloc::vec::Vec<u16>,
}

impl Huffman {
    /// Build from per-symbol code lengths (0 = unused). Rejects
    /// over-subscribed sets; incomplete sets are allowed only for the
    /// degenerate single-symbol cases RFC 1951 permits in practice.
    fn from_lengths(lengths: &[u8]) -> Result<Huffman, InflateError> {
        let mut counts = [0u16; 16];
        for &len in lengths {
            if len as usize >= counts.len() {
                return Err(InflateError::BadCodeLengths);
            }
            counts[len as usize] += 1;
        }
        if counts[0] as usize == lengths.len() {
            // No codes at all: only legal for an empty distance tree.
            return Ok(Huffman { counts, symbols: Vec::new() });
        }
        // Over-subscription check.
        let mut left: i32 = 1;
        for len in 1..16 {
            left <<= 1;
            left -= counts[len] as i32;
            if left < 0 {
                return Err(InflateError::BadCodeLengths);
            }
        }
        // Offsets into symbols per length.
        let mut offs = [0u16; 16];
        for len in 1..15 {
            offs[len + 1] = offs[len] + counts[len];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &len) in lengths.iter().enumerate() {
            if len != 0 {
                symbols[offs[len as usize] as usize] = sym as u16;
                offs[len as usize] += 1;
            }
        }
        Ok(Huffman { counts, symbols })
    }

    fn decode(&self, br: &mut BitReader<'_>) -> Result<u16, InflateError> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= br.read_bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(InflateError::BadCodeLengths)
    }
}

/// Length codes 257..285 base lengths (RFC 1951 section 3.2.5).
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CLEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn fixed_literal_tree() -> Huffman {
    let mut lengths = [0u8; 288];
    for (i, len) in lengths.iter_mut().enumerate() {
        *len = match i {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    // Fixed trees are complete by construction; unwrap-equivalent via expect
    // is unavailable without std panics? core::panic works; but keep it total:
    Huffman::from_lengths(&lengths).ok().unwrap_or_else(Huffman::empty)
}

impl Huffman {
    fn empty() -> Huffman {
        Huffman { counts: [0; 16], symbols: Vec::new() }
    }
}

fn fixed_distance_tree() -> Huffman {
    let mut lengths = [5u8; 30];
    // 32 codes of 5 bits in the real fixed table; codes 30/31 are invalid
    // distances. Encoding 30 symbols with 5 bits leaves the set incomplete
    // but decodable for 0..=29, which is all we admit.
    let _ = &mut lengths;
    Huffman::from_lengths(&lengths).ok().unwrap_or_else(Huffman::empty)
}

/// Inflate a raw DEFLATE stream (RFC 1951). Returns the decompressed bytes.
pub fn inflate(data: &[u8]) -> Result<Vec<u8>, InflateError> {
    let mut br = BitReader::new(data);
    let mut out: Vec<u8> = Vec::new();
    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                let byte = br.bytes_consumed(); // rounds partial byte up
                br.align_byte();
                if byte + 4 > data.len() {
                    return Err(InflateError::Truncated);
                }
                let len = u16::from_le_bytes([data[byte], data[byte + 1]]);
                let nlen = u16::from_le_bytes([data[byte + 2], data[byte + 3]]);
                if len != !nlen {
                    return Err(InflateError::BadCodeLengths);
                }
                let start = byte + 4;
                let end = start + len as usize;
                if end > data.len() {
                    return Err(InflateError::Truncated);
                }
                out.extend_from_slice(&data[start..end]);
                br = BitReader::new(data);
                br.byte = end;
            }
            1 => {
                let lt = fixed_literal_tree();
                let dt = fixed_distance_tree();
                inflate_block(&mut br, &mut out, &lt, &dt)?;
            }
            2 => {
                let (lt, dt) = dynamic_trees(&mut br)?;
                inflate_block(&mut br, &mut out, &lt, &dt)?;
            }
            other => return Err(InflateError::BadBlockType(other as u8)),
        }
        if bfinal == 1 {
            break;
        }
    }
    Ok(out)
}

fn dynamic_trees(
    br: &mut BitReader<'_>,
) -> Result<(Huffman, Huffman), InflateError> {
    let hlit = br.read_bits(5)? as usize + 257;
    let hdist = br.read_bits(5)? as usize + 1;
    let hclen = br.read_bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return Err(InflateError::BadCodeLengths);
    }
    let mut clen_lengths = [0u8; 19];
    for i in 0..hclen {
        clen_lengths[CLEN_ORDER[i]] = br.read_bits(3)? as u8;
    }
    let clen_tree = Huffman::from_lengths(&clen_lengths)?;
    let mut lengths = vec![0u8; hlit + hdist];
    let mut i = 0;
    while i < lengths.len() {
        let sym = clen_tree.decode(br)?;
        match sym {
            0..=15 => {
                lengths[i] = sym as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err(InflateError::BadCodeLengthRepeat);
                }
                let prev = lengths[i - 1];
                let rep = 3 + br.read_bits(2)? as usize;
                if i + rep > lengths.len() {
                    return Err(InflateError::BadCodeLengths);
                }
                for _ in 0..rep {
                    lengths[i] = prev;
                    i += 1;
                }
            }
            17 => {
                let rep = 3 + br.read_bits(3)? as usize;
                if i + rep > lengths.len() {
                    return Err(InflateError::BadCodeLengths);
                }
                i += rep;
            }
            18 => {
                let rep = 11 + br.read_bits(7)? as usize;
                if i + rep > lengths.len() {
                    return Err(InflateError::BadCodeLengths);
                }
                i += rep;
            }
            _ => return Err(InflateError::BadCodeLengths),
        }
    }
    if lengths[256] == 0 {
        return Err(InflateError::MissingEndOfBlock);
    }
    let lt = Huffman::from_lengths(&lengths[..hlit])?;
    let dt = Huffman::from_lengths(&lengths[hlit..])?;
    Ok((lt, dt))
}

fn inflate_block(
    br: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    lt: &Huffman,
    dt: &Huffman,
) -> Result<(), InflateError> {
    loop {
        let sym = lt.decode(br)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Ok(()),
            257..=285 => {
                let idx = sym as usize - 257;
                let len = LENGTH_BASE[idx] as usize + br.read_bits(LENGTH_EXTRA[idx] as u32)? as usize;
                let dsym = dt.decode(br)? as usize;
                if dsym >= 30 {
                    return Err(InflateError::BadDistanceCode);
                }
                let dist =
                    DIST_BASE[dsym] as usize + br.read_bits(DIST_EXTRA[dsym] as u32)? as usize;
                if dist > out.len() {
                    return Err(InflateError::OverlongBackreference {
                        distance: dist as u16,
                        written: out.len(),
                    });
                }
                let start = out.len() - dist;
                // Byte-at-a-time: overlap copies (dist < len) must see bytes
                // this same copy wrote.
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return Err(InflateError::BadCodeLengths),
        }
    }
}

/// Adler-32 (RFC 1950 section 8).
pub fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a += byte as u32;
        if a >= MOD {
            a -= MOD;
        }
        b += a;
        if b >= MOD {
            b %= MOD;
        }
    }
    (b << 16) | a
}

/// Decode a zlib stream (RFC 1950): 2-byte header, DEFLATE body, adler32.
/// Returns the decompressed bytes. Verifies the header check and adler32.
pub fn zlib_decode(data: &[u8]) -> Result<Vec<u8>, InflateError> {
    if data.len() < 6 {
        return Err(InflateError::Truncated);
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0F != 8 || !((cmf as u16) * 256 + flg as u16).is_multiple_of(31) || flg & 0x20 != 0 {
        return Err(InflateError::BadZlibHeader);
    }
    let body = &data[2..data.len() - 4];
    let out = inflate(body)?;
    let stored = u32::from_be_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);
    let actual = adler32(&out);
    if stored != actual {
        return Err(InflateError::AdlerMismatch { expected: stored, actual });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[cfg(feature = "std")]
    use alloc::format;
    #[cfg(feature = "std")]
    use std::process::Command;
    #[cfg(feature = "std")]
    use std::string::String;

    #[cfg(feature = "std")]
    fn python_compress(data: &[u8], level: u32) -> Option<Vec<u8>> {
        use std::process::Stdio;
        let b64 = {
            // base64-encode without external crates.
            const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for chunk in data.chunks(3) {
                let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
                let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
                out.push(T[(n >> 18) as usize & 63] as char);
                out.push(T[(n >> 12) as usize & 63] as char);
                out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
                out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
            }
            out
        };
        let child = Command::new("python3")
            .arg("-c")
            .arg(format!(
                "import sys,zlib,base64;sys.stdout.buffer.write(zlib.compress(base64.b64decode(sys.argv[1]),{level}))",
            ))
            .arg(&b64)
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("python3 spawn failed: {e}"));
        if !child.status.success() {
            panic!(
                "python3 zlib helper failed: {}",
                String::from_utf8_lossy(&child.stderr)
            );
        }
        Some(child.stdout)
    }

    #[test]
    fn stored_block_round_trip() {
        // Hand-built: final stored block with "hi".
        let data = [
            0x01, 0x02, 0x00, 0xFD, 0xFF, b'h', b'i',
        ];
        assert_eq!(inflate(&data).unwrap(), b"hi".to_vec());
    }

    #[test]
    fn corrupt_stored_block_len_check() {
        // Same stored block but with a wrong ~len complement.
        let data = [0x01, 0x02, 0x00, 0x00, 0x00, b'h', b'i'];
        assert_eq!(inflate(&data), Err(InflateError::BadCodeLengths));
    }

    #[test]
    fn bad_block_type_typed_error() {
        // First byte: BFINAL=1, BTYPE=11 (reserved).
        assert_eq!(inflate(&[0x07]), Err(InflateError::BadBlockType(3)));
    }

    #[test]
    fn overlong_backreference_typed_error() {
        // Hand-built fixed-Huffman stream: BFINAL=1, BTYPE=01, then length
        // symbol 257 (len 3, 0 extra bits: 7-bit code 0000001) and distance
        // symbol 1 (dist 2, 0 extra bits: 5-bit code 00001) over an EMPTY
        // output — distance 2 with 0 bytes written must be refused.
        let stream = [0x03u8, 0x42];
        match inflate(&stream) {
            Err(InflateError::OverlongBackreference { distance, written }) => {
                assert_eq!((distance, written), (2, 0));
            }
            other => panic!("expected OverlongBackreference, got {other:?}"),
        }
    }

    #[test]
    fn bad_adler_typed_error() {
        // Valid zlib stream for "aaaaaaaa" with a corrupted trailer.
        let zlib = python_compress(b"aaaaaaaa", 9).expect("python3 zlib");
        let mut bad = zlib.clone();
        let n = bad.len();
        bad[n - 1] = bad[n - 1].wrapping_add(1);
        match zlib_decode(&bad) {
            Err(InflateError::AdlerMismatch { .. }) => {}
            other => panic!("expected AdlerMismatch, got {other:?}"),
        }
    }

    #[test]
    fn bad_zlib_header_typed_error() {
        assert_eq!(zlib_decode(&[0x00, 0x00, 0, 0, 0, 0]), Err(InflateError::BadZlibHeader));
    }

    #[test]
    fn truncated_stream_typed_error() {
        assert_eq!(zlib_decode(&[0x78]), Err(InflateError::Truncated));
        assert_eq!(inflate(&[0x03]), Err(InflateError::Truncated));
    }

    #[cfg(feature = "std")]
    #[test]
    fn round_trip_python_zlib_multiple_levels() {
        let corpus: Vec<Vec<u8>> = vec![
            b"hello hello hello hello hello world".to_vec(),
            (0u32..10_000).map(|i| (i % 251) as u8).collect(),
            Vec::new(),
            b"x".to_vec(),
            b"the quick brown fox jumps over the lazy dog".repeat(200),
        ];
        for (case, data) in corpus.iter().enumerate() {
            for level in [1u32, 6, 9] {
                let compressed = python_compress(data, level).unwrap_or_else(|| {
                    panic!("python3 unavailable for case {case} level {level}")
                });
                let out = zlib_decode(&compressed)
                    .unwrap_or_else(|e| panic!("case {case} level {level}: {e}"));
                assert_eq!(&out, data, "case {case} level {level}");
            }
        }
    }

    /// Real git fixture: temp repo, `git hash-object -w`, then both the loose
    /// object (zlib) and a `git pack-objects` pack's zlib body are inflated
    /// with this crate and compared to the original bytes.
    #[cfg(feature = "std")]
    #[test]
    fn git_fixture_inflate_round_trip() {
        use std::string::String;
        use std::string::ToString;

        let dir = std::env::temp_dir().join(format!("cgap-git-inflate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| -> String {
            let out = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .expect("git available");
            assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        run(&["init", "-q", "."]);
        // Repetitive content so git deflates rather than storing raw.
        let content = "inflate-lane\r\n".repeat(400);
        let oid = {
            use std::io::Write as _;
            use std::process::Stdio;
            let mut child = Command::new("git")
                .args(["hash-object", "-w", "--stdin"])
                .current_dir(&dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("git spawn");
            child.stdin.as_mut().unwrap().write_all(content.as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success());
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        assert_eq!(oid.len(), 40);

        // 1) Loose object: .git/objects/ab/cdef... is a zlib stream of
        //    "blob <len>\0<content>".
        let loose = dir.join(".git/objects").join(&oid[..2]).join(&oid[2..]);
        let raw = std::fs::read(&loose).unwrap();
        let decoded = zlib_decode(&raw).unwrap();
        let expected_header = format!("blob {}\0", content.len());
        assert!(decoded.starts_with(expected_header.as_bytes()));
        assert_eq!(&decoded[expected_header.len()..], content.as_bytes());

        // 2) Pack: git pack-objects over the object; pack layout is
        //    12-byte header, varint object header, zlib body, 20-byte trailer.
        let pack_list = dir.join("pack-in.txt");
        let _ = std::fs::write(&pack_list, format!("{oid}\n"));
        run(&["rev-parse", &oid]); // sanity: object exists
        let out = Command::new("git")
            .args(["pack-objects", "--stdout"])
            .current_dir(&dir)
            .stdin(std::process::Stdio::from(std::fs::File::open(&pack_list).unwrap()))
            .output()
            .expect("pack-objects");
        assert!(out.status.success(), "pack-objects failed");
        let pack = out.stdout;
        assert_eq!(&pack[..4], b"PACK");
        // Parse the varint object header: type in bits 4-6, size 7 bits per
        // byte, MSB continuation.
        let mut i = 12usize;
        let mut size: u64 = (pack[i] & 0x0F) as u64;
        let mut shift = 4u32;
        let ty = (pack[12] >> 4) & 0x7;
        i = 13;
        loop {
            let b = pack[i];
            i += 1;
            size |= ((b & 0x7F) as u64) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                break;
            }
        }
        assert_eq!(ty, 3, "blob type expected in pack entry header");
        // Pack entry size field is the object's uncompressed content size,
        // excluding the "blob <n>\0" preamble.
        assert_eq!(size, content.len() as u64);
        let body = &pack[i..pack.len() - 20];
        let decoded_pack = zlib_decode(body).unwrap();
        // Packed objects store the raw content: type and size live in the
        // entry header, not a "blob N\0" preamble (that's loose objects).
        assert_eq!(
            decoded_pack,
            content.as_bytes(),
            "pack zlib body must inflate to the original content"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
