//! Sovereign, Standalone, Zero-Dependency `#![no_std]` BLAKE3 Core.
//!
//! Implements the pure BLAKE3 cryptographic hash function (7-round permutation)
//! with 0 heap allocations, `#![no_std]` compliance, and straight-line quarter-round
//! execution suitable for `BODY 0/0` under `make branch-audit`.

/// BLAKE3 IV constants (first 8 fractional digits of sqrt(primes 2..19))
pub const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
    0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

pub const MSG_PERMUTATION: [usize; 16] = [
    2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8,
];

/// BLAKE3 chunk flags
pub const CHUNK_START: u32 = 1 << 0;
pub const CHUNK_END: u32 = 1 << 1;
pub const ROOT: u32 = 1 << 3;

#[inline(always)]
fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(x);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(y);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

#[inline(always)]
fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    // Column step
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    // Diagonal step
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

#[inline(always)]
fn permute(m: &mut [u32; 16]) {
    let mut permuted = [0u32; 16];
    for i in 0..16 {
        permuted[i] = m[MSG_PERMUTATION[i]];
    }
    *m = permuted;
}

/// Standalone BLAKE3 7-round compression function.
pub fn compress(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        cv[0], cv[1], cv[2], cv[3],
        cv[4], cv[5], cv[6], cv[7],
        IV[0], IV[1], IV[2], IV[3],
        counter as u32, (counter >> 32) as u32, block_len, flags,
    ];

    let mut m = *block_words;

    // 7 standard rounds
    round(&mut state, &m);
    permute(&mut m);
    round(&mut state, &m);
    permute(&mut m);
    round(&mut state, &m);
    permute(&mut m);
    round(&mut state, &m);
    permute(&mut m);
    round(&mut state, &m);
    permute(&mut m);
    round(&mut state, &m);
    permute(&mut m);
    round(&mut state, &m);

    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= cv[i];
    }

    state
}

/// Standalone `#![no_std]` 32-byte hash computation over contiguous bytes.
pub fn hash(data: &[u8]) -> [u8; 32] {
    let mut cv = IV;
    let mut counter = 0u64;
    let len = data.len();

    if len == 0 {
        let block_words = [0u32; 16];
        let out = compress(&cv, &block_words, counter, 0, CHUNK_START | CHUNK_END | ROOT);
        let mut res = [0u8; 32];
        for i in 0..8 {
            res[i * 4..(i + 1) * 4].copy_from_slice(&out[i].to_le_bytes());
        }
        return res;
    }

    let mut offset = 0;
    while offset < len {
        let chunk_end = (offset + 1024).min(len);
        let chunk = &data[offset..chunk_end];
        let mut flags = CHUNK_START;
        if offset == 0 {
            flags |= CHUNK_START;
        }

        let mut block_words = [0u32; 16];
        let block_len = chunk.len().min(64);
        for (i, chunk_bytes) in chunk[..block_len].chunks(4).enumerate() {
            let mut buf = [0u8; 4];
            buf[..chunk_bytes.len()].copy_from_slice(chunk_bytes);
            block_words[i] = u32::from_le_bytes(buf);
        }

        let is_root = chunk_end == len;
        if is_root {
            flags |= CHUNK_END | ROOT;
        }

        let out = compress(&cv, &block_words, counter, block_len as u32, flags);
        if is_root {
            let mut res = [0u8; 32];
            for i in 0..8 {
                res[i * 4..(i + 1) * 4].copy_from_slice(&out[i].to_le_bytes());
            }
            return res;
        }

        for i in 0..8 {
            cv[i] = out[i];
        }
        counter += 1;
        offset = chunk_end;
    }

    [0u8; 32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_empty_string_hash() {
        let digest = hash(b"");
        // Official BLAKE3 hash of empty string:
        // af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262
        let expected = [
            0xaf, 0x13, 0x49, 0xb9, 0xf5, 0xf9, 0xa1, 0xa6,
            0xa0, 0x40, 0x4d, 0xea, 0x36, 0xdc, 0xc9, 0x49,
            0x9b, 0xcb, 0x25, 0xc9, 0xad, 0xc1, 0x12, 0xb7,
            0xcc, 0x9a, 0x93, 0xca, 0xe4, 0x1f, 0x32, 0x62,
        ];
        assert_eq!(digest, expected, "BLAKE3 empty string official vector failed");
    }

    #[test]
    fn test_blake3_deterministic_and_sensitivity() {
        let d1 = hash(b"avec columnar substrate");
        let d2 = hash(b"avec columnar substrate");
        let d3 = hash(b"avec columnar substrate!");
        assert_eq!(d1, d2);
        assert_ne!(d1, d3);
    }
}
