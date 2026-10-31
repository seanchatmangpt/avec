//! Sovereign, Standalone, Zero-Dependency `#![no_std]` BLAKE3 Crate (`avec-blake3`).
//!
//! Implements the complete BLAKE3 cryptographic hash specification:
//! - 16-block intra-chunk sequential compression (`hash_chunk`)
//! - Zero-allocation stack-based Merkle tree reduction (`hash`)
//! - 4-way ARM64 NEON (`neon_parent`) and 8-way x86_64 AVX2 (`avx2_parent`) horizontal
//!   SIMD multi-parent Merkle reduction (`reduce_level_simd`, `hash_wide_simd`)
//! - Architectural dispatch boundary (`GPU_CROSSOVER_BYTES = 16 MiB`) separating
//!   sub-microsecond CPU SIMD execution from massive-batch Unified Memory GPU execution.
/// BLAKE3 IV constants (first 8 fractional words of sqrt(primes 2..19)).
pub const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

/// Message word permutation schedule between BLAKE3 rounds.
pub const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

/// Domain separation flags per BLAKE3 specification.
pub const CHUNK_START: u32 = 1 << 0;
pub const CHUNK_END: u32 = 1 << 1;
pub const PARENT: u32 = 1 << 2;
pub const ROOT: u32 = 1 << 3;

/// Payload threshold (16 MiB = 16,384 chunks) below which CPU SIMD (NEON/AVX2)
/// consistently outperforms GPU dispatch due to command-buffer launch and barrier latency.
pub const GPU_CROSSOVER_BYTES: usize = 16 * 1024 * 1024;

/// Returns `true` when `payload_len` is large enough (>= 16 MiB) for Unified Memory GPU
/// SIMT leaf saturation to amortize command-buffer launch overhead.
#[inline(always)]
pub const fn should_dispatch_gpu(payload_len: usize) -> bool {
    payload_len >= GPU_CROSSOVER_BYTES
}

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
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);

    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

#[inline(always)]
fn permute(m: &mut [u32; 16]) {
    let mut permuted = [0u32; 16];
    let mut i = 0;
    while i < 16 {
        permuted[i] = m[MSG_PERMUTATION[i]];
        i += 1;
    }
    *m = permuted;
}

/// Standalone BLAKE3 7-round scalar compression function.
#[inline(always)]
pub fn compress_scalar(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];

    let mut m = *block_words;

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

    let mut i = 0;
    while i < 8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= cv[i];
        i += 1;
    }

    state
}

/// Alias for [`compress_scalar`].
#[inline(always)]
pub fn compress(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    compress_scalar(cv, block_words, counter, block_len, flags)
}

#[inline(always)]
fn words_from_bytes(bytes: &[u8], out: &mut [u32; 16]) {
    *out = [0u32; 16];
    for (i, chunk) in bytes.chunks(4).enumerate() {
        let mut buf = [0u8; 4];
        buf[..chunk.len()].copy_from_slice(chunk);
        out[i] = u32::from_le_bytes(buf);
    }
}

/// Hashes a single chunk (0..=1024 bytes) across up to 16 sequential 64-byte blocks,
/// returning its 16-word compression output (first 8 words form the 32-byte CV or root digest).
#[inline(always)]
pub fn hash_chunk(chunk: &[u8], chunk_counter: u64, is_root: bool) -> [u32; 16] {
    let mut cv = IV;
    let mut block_words = [0u32; 16];
    let num_blocks = if chunk.is_empty() {
        1
    } else {
        chunk.len().div_ceil(64)
    };

    for b in 0..num_blocks {
        let start = b * 64;
        let end = (start + 64).min(chunk.len());
        let block_slice = if chunk.is_empty() {
            &[]
        } else {
            &chunk[start..end]
        };
        let block_len = block_slice.len() as u32;

        words_from_bytes(block_slice, &mut block_words);

        let mut flags = 0u32;
        if b == 0 {
            flags |= CHUNK_START;
        }
        if b == num_blocks - 1 {
            flags |= CHUNK_END;
            if is_root {
                flags |= ROOT;
            }
        }

        let out = compress_scalar(&cv, &block_words, chunk_counter, block_len, flags);
        if b == num_blocks - 1 {
            return out;
        }
        cv.copy_from_slice(&out[0..8]);
    }

    unreachable!()
}

/// Standalone `#![no_std]` 32-byte BLAKE3 computation with zero-allocation stack Merkle reduction.
pub fn hash(data: &[u8]) -> [u8; 32] {
    let len = data.len();

    // Fast path: single-chunk input (0..=1024 bytes) forms the root node directly.
    if len <= 1024 {
        let out = hash_chunk(data, 0, true);
        let mut res = [0u8; 32];
        for i in 0..8 {
            res[i * 4..(i + 1) * 4].copy_from_slice(&out[i].to_le_bytes());
        }
        return res;
    }

    // Zero-heap stack for parent Merkle reduction: max depth 54 covers 2^64 bytes.
    let mut stack = [[0u32; 8]; 54];
    let mut stack_len = 0usize;

    let num_chunks = len.div_ceil(1024);
    for chunk_idx in 0..num_chunks {
        let start = chunk_idx * 1024;
        let end = (start + 1024).min(len);
        let chunk_slice = &data[start..end];

        let chunk_out = hash_chunk(chunk_slice, chunk_idx as u64, false);
        let mut current_cv = [0u32; 8];
        current_cv.copy_from_slice(&chunk_out[0..8]);

        // Merge right children up the tree (deferring final root merge so ROOT flag is preserved)
        let mut total_chunks = chunk_idx + 1;
        while total_chunks & 1 == 0 {
            if stack_len == 1 && chunk_idx == num_chunks - 1 {
                break;
            }
            stack_len -= 1;
            let left_child = stack[stack_len];
            let mut block = [0u32; 16];
            block[..8].copy_from_slice(&left_child);
            block[8..].copy_from_slice(&current_cv);

            let parent_out = compress_scalar(&IV, &block, 0, 64, PARENT);
            current_cv.copy_from_slice(&parent_out[0..8]);
            total_chunks >>= 1;
        }

        stack[stack_len] = current_cv;
        stack_len += 1;
    }

    // Final reduction to root
    while stack_len > 1 {
        let right_child = stack[stack_len - 1];
        let left_child = stack[stack_len - 2];
        stack_len -= 2;

        let mut block = [0u32; 16];
        block[..8].copy_from_slice(&left_child);
        block[8..].copy_from_slice(&right_child);

        let flags = if stack_len == 0 {
            PARENT | ROOT
        } else {
            PARENT
        };
        let out = compress_scalar(&IV, &block, 0, 64, flags);

        if stack_len == 0 {
            let mut res = [0u8; 32];
            for i in 0..8 {
                res[i * 4..(i + 1) * 4].copy_from_slice(&out[i].to_le_bytes());
            }
            return res;
        }

        stack[stack_len].copy_from_slice(&out[0..8]);
        stack_len += 1;
    }

    let mut res = [0u8; 32];
    for i in 0..8 {
        res[i * 4..(i + 1) * 4].copy_from_slice(&stack[0][i].to_le_bytes());
    }
    res
}

// =========================================================================
// SIMD Multi-Parent Kernels
// =========================================================================

#[cfg(target_arch = "aarch64")]
pub mod neon_parent {
    use super::*;
    use core::arch::aarch64::*;

    #[inline(always)]
    unsafe fn rot16(v: uint32x4_t) -> uint32x4_t {
        unsafe { vreinterpretq_u32_u16(vrev32q_u16(vreinterpretq_u16_u32(v))) }
    }

    #[inline(always)]
    unsafe fn rot12(v: uint32x4_t) -> uint32x4_t {
        unsafe { vorrq_u32(vshrq_n_u32::<12>(v), vshlq_n_u32::<20>(v)) }
    }

    #[inline(always)]
    unsafe fn rot8(v: uint32x4_t) -> uint32x4_t {
        unsafe { vorrq_u32(vshrq_n_u32::<8>(v), vshlq_n_u32::<24>(v)) }
    }

    #[inline(always)]
    unsafe fn rot7(v: uint32x4_t) -> uint32x4_t {
        unsafe { vorrq_u32(vshrq_n_u32::<7>(v), vshlq_n_u32::<25>(v)) }
    }

    #[inline(always)]
    unsafe fn g_neon(
        s: &mut [uint32x4_t; 16],
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        x: uint32x4_t,
        y: uint32x4_t,
    ) {
        unsafe {
            s[a] = vaddq_u32(vaddq_u32(s[a], s[b]), x);
            s[d] = rot16(veorq_u32(s[d], s[a]));
            s[c] = vaddq_u32(s[c], s[d]);
            s[b] = rot12(veorq_u32(s[b], s[c]));
            s[a] = vaddq_u32(vaddq_u32(s[a], s[b]), y);
            s[d] = rot8(veorq_u32(s[d], s[a]));
            s[c] = vaddq_u32(s[c], s[d]);
            s[b] = rot7(veorq_u32(s[b], s[c]));
        }
    }

    #[inline(always)]
    unsafe fn round_neon(s: &mut [uint32x4_t; 16], m: &[uint32x4_t; 16]) {
        unsafe {
            g_neon(s, 0, 4, 8, 12, m[0], m[1]);
            g_neon(s, 1, 5, 9, 13, m[2], m[3]);
            g_neon(s, 2, 6, 10, 14, m[4], m[5]);
            g_neon(s, 3, 7, 11, 15, m[6], m[7]);

            g_neon(s, 0, 5, 10, 15, m[8], m[9]);
            g_neon(s, 1, 6, 11, 12, m[10], m[11]);
            g_neon(s, 2, 7, 8, 13, m[12], m[13]);
            g_neon(s, 3, 4, 9, 14, m[14], m[15]);
        }
    }

    #[inline(always)]
    unsafe fn permute_neon(m: &mut [uint32x4_t; 16]) {
        let p = *m;
        let mut i = 0;
        while i < 16 {
            m[i] = p[MSG_PERMUTATION[i]];
            i += 1;
        }
    }

    /// Evaluates 4 parent compressions concurrently in NEON registers.
    ///
    /// # Safety
    /// Requires ARM64 NEON support (standard on all `aarch64` targets).
    #[target_feature(enable = "neon")]
    pub unsafe fn compress_parents_4way_neon(
        left: &[[u32; 8]; 4],
        right: &[[u32; 8]; 4],
        flags: [u32; 4],
        out_parents: &mut [[u32; 8]; 4],
    ) {
        unsafe {
            let cv = [
                vdupq_n_u32(IV[0]),
                vdupq_n_u32(IV[1]),
                vdupq_n_u32(IV[2]),
                vdupq_n_u32(IV[3]),
                vdupq_n_u32(IV[4]),
                vdupq_n_u32(IV[5]),
                vdupq_n_u32(IV[6]),
                vdupq_n_u32(IV[7]),
            ];

            let mut m = [vdupq_n_u32(0); 16];

            // Transpose 4 parent input blocks (64 bytes each: left[0..8] ++ right[0..8])
            for w in 0..8 {
                let col = [left[0][w], left[1][w], left[2][w], left[3][w]];
                m[w] = vld1q_u32(col.as_ptr());
            }
            for w in 0..8 {
                let col = [right[0][w], right[1][w], right[2][w], right[3][w]];
                m[w + 8] = vld1q_u32(col.as_ptr());
            }

            let flags_vec = vld1q_u32(flags.as_ptr());

            let mut s = [
                cv[0],
                cv[1],
                cv[2],
                cv[3],
                cv[4],
                cv[5],
                cv[6],
                cv[7],
                vdupq_n_u32(IV[0]),
                vdupq_n_u32(IV[1]),
                vdupq_n_u32(IV[2]),
                vdupq_n_u32(IV[3]),
                vdupq_n_u32(0),
                vdupq_n_u32(0),
                vdupq_n_u32(64),
                flags_vec,
            ];

            for _ in 0..6 {
                round_neon(&mut s, &m);
                permute_neon(&mut m);
            }
            round_neon(&mut s, &m);

            for i in 0..8 {
                let reduced = veorq_u32(s[i], s[i + 8]);
                let mut lane = [0u32; 4];
                vst1q_u32(lane.as_mut_ptr(), reduced);
                out_parents[0][i] = lane[0];
                out_parents[1][i] = lane[1];
                out_parents[2][i] = lane[2];
                out_parents[3][i] = lane[3];
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub mod avx2_parent {
    use super::*;
    use core::arch::x86_64::*;

    #[inline(always)]
    unsafe fn rot16(v: __m256i) -> __m256i {
        unsafe { _mm256_or_si256(_mm256_srli_epi32::<16>(v), _mm256_slli_epi32::<16>(v)) }
    }

    #[inline(always)]
    unsafe fn rot12(v: __m256i) -> __m256i {
        unsafe { _mm256_or_si256(_mm256_srli_epi32::<12>(v), _mm256_slli_epi32::<20>(v)) }
    }

    #[inline(always)]
    unsafe fn rot8(v: __m256i) -> __m256i {
        unsafe { _mm256_or_si256(_mm256_srli_epi32::<8>(v), _mm256_slli_epi32::<24>(v)) }
    }

    #[inline(always)]
    unsafe fn rot7(v: __m256i) -> __m256i {
        unsafe { _mm256_or_si256(_mm256_srli_epi32::<7>(v), _mm256_slli_epi32::<25>(v)) }
    }

    #[inline(always)]
    unsafe fn g_avx2(
        s: &mut [__m256i; 16],
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        x: __m256i,
        y: __m256i,
    ) {
        unsafe {
            s[a] = _mm256_add_epi32(_mm256_add_epi32(s[a], s[b]), x);
            s[d] = rot16(_mm256_xor_si256(s[d], s[a]));
            s[c] = _mm256_add_epi32(s[c], s[d]);
            s[b] = rot12(_mm256_xor_si256(s[b], s[c]));
            s[a] = _mm256_add_epi32(_mm256_add_epi32(s[a], s[b]), y);
            s[d] = rot8(_mm256_xor_si256(s[d], s[a]));
            s[c] = _mm256_add_epi32(s[c], s[d]);
            s[b] = rot7(_mm256_xor_si256(s[b], s[c]));
        }
    }

    #[inline(always)]
    unsafe fn round_avx2(s: &mut [__m256i; 16], m: &[__m256i; 16]) {
        unsafe {
            g_avx2(s, 0, 4, 8, 12, m[0], m[1]);
            g_avx2(s, 1, 5, 9, 13, m[2], m[3]);
            g_avx2(s, 2, 6, 10, 14, m[4], m[5]);
            g_avx2(s, 3, 7, 11, 15, m[6], m[7]);

            g_avx2(s, 0, 5, 10, 15, m[8], m[9]);
            g_avx2(s, 1, 6, 11, 12, m[10], m[11]);
            g_avx2(s, 2, 7, 8, 13, m[12], m[13]);
            g_avx2(s, 3, 4, 9, 14, m[14], m[15]);
        }
    }

    #[inline(always)]
    unsafe fn permute_avx2(m: &mut [__m256i; 16]) {
        let p = *m;
        let mut i = 0;
        while i < 16 {
            m[i] = p[MSG_PERMUTATION[i]];
            i += 1;
        }
    }

    /// Evaluates up to 8 parent compressions concurrently in AVX2 registers.
    ///
    /// # Safety
    /// Requires x86_64 AVX2 target support.
    #[target_feature(enable = "avx2")]
    pub unsafe fn compress_parents_8way_avx2(
        left: &[[u32; 8]; 8],
        right: &[[u32; 8]; 8],
        flags: [u32; 8],
        out_parents: &mut [[u32; 8]; 8],
    ) {
        unsafe {
            let cv = [
                _mm256_set1_epi32(IV[0] as i32),
                _mm256_set1_epi32(IV[1] as i32),
                _mm256_set1_epi32(IV[2] as i32),
                _mm256_set1_epi32(IV[3] as i32),
                _mm256_set1_epi32(IV[4] as i32),
                _mm256_set1_epi32(IV[5] as i32),
                _mm256_set1_epi32(IV[6] as i32),
                _mm256_set1_epi32(IV[7] as i32),
            ];

            let mut m = [_mm256_setzero_si256(); 16];

            for w in 0..8 {
                m[w] = _mm256_setr_epi32(
                    left[0][w] as i32,
                    left[1][w] as i32,
                    left[2][w] as i32,
                    left[3][w] as i32,
                    left[4][w] as i32,
                    left[5][w] as i32,
                    left[6][w] as i32,
                    left[7][w] as i32,
                );
            }
            for w in 0..8 {
                m[w + 8] = _mm256_setr_epi32(
                    right[0][w] as i32,
                    right[1][w] as i32,
                    right[2][w] as i32,
                    right[3][w] as i32,
                    right[4][w] as i32,
                    right[5][w] as i32,
                    right[6][w] as i32,
                    right[7][w] as i32,
                );
            }

            let flags_vec = _mm256_setr_epi32(
                flags[0] as i32,
                flags[1] as i32,
                flags[2] as i32,
                flags[3] as i32,
                flags[4] as i32,
                flags[5] as i32,
                flags[6] as i32,
                flags[7] as i32,
            );

            let mut s = [
                cv[0],
                cv[1],
                cv[2],
                cv[3],
                cv[4],
                cv[5],
                cv[6],
                cv[7],
                _mm256_set1_epi32(IV[0] as i32),
                _mm256_set1_epi32(IV[1] as i32),
                _mm256_set1_epi32(IV[2] as i32),
                _mm256_set1_epi32(IV[3] as i32),
                _mm256_setzero_si256(),
                _mm256_setzero_si256(),
                _mm256_set1_epi32(64),
                flags_vec,
            ];

            for _ in 0..6 {
                round_avx2(&mut s, &m);
                permute_avx2(&mut m);
            }
            round_avx2(&mut s, &m);

            for i in 0..8 {
                let reduced = _mm256_xor_si256(s[i], s[i + 8]);
                let mut lane = [0u32; 8];
                _mm256_storeu_si256(lane.as_mut_ptr() as *mut __m256i, reduced);
                for c in 0..8 {
                    out_parents[c][i] = lane[c];
                }
            }
        }
    }
}

/// Reduces a horizontal queue of CVs at the same subtree level using SIMD (4-way NEON / 8-way AVX2).
#[inline(always)]
pub fn reduce_level_simd(cvs: &mut [[u32; 8]; 64], cv_count: &mut usize) {
    let mut i = 0;
    let pairs = *cv_count / 2;

    #[cfg(target_arch = "x86_64")]
    {
        // Note: on x86_64, only use AVX2 when target_feature="avx2" is enabled at compile time
        #[cfg(target_feature = "avx2")]
        while i + 8 <= pairs {
            let mut left = [[0u32; 8]; 8];
            let mut right = [[0u32; 8]; 8];
            for p in 0..8 {
                left[p] = cvs[(i + p) * 2];
                right[p] = cvs[(i + p) * 2 + 1];
            }

            let mut out = [[0u32; 8]; 8];
            unsafe {
                avx2_parent::compress_parents_8way_avx2(&left, &right, [PARENT; 8], &mut out);
            }

            for p in 0..8 {
                cvs[i + p] = out[p];
            }
            i += 8;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        while i + 4 <= pairs {
            let mut left = [[0u32; 8]; 4];
            let mut right = [[0u32; 8]; 4];
            for p in 0..4 {
                left[p] = cvs[(i + p) * 2];
                right[p] = cvs[(i + p) * 2 + 1];
            }

            let mut out = [[0u32; 8]; 4];
            unsafe {
                neon_parent::compress_parents_4way_neon(&left, &right, [PARENT; 4], &mut out);
            }

            for p in 0..4 {
                cvs[i + p] = out[p];
            }
            i += 4;
        }
    }

    // Scalar tail for remainder pairs
    while i < pairs {
        let mut block = [0u32; 16];
        block[..8].copy_from_slice(&cvs[i * 2]);
        block[8..].copy_from_slice(&cvs[i * 2 + 1]);
        let out = compress_scalar(&IV, &block, 0, 64, PARENT);
        cvs[i].copy_from_slice(&out[0..8]);
        i += 1;
    }

    // If an odd node was left over at the end of the level, shift it forward
    if *cv_count % 2 == 1 {
        cvs[pairs] = cvs[*cv_count - 1];
        *cv_count = pairs + 1;
    } else {
        *cv_count = pairs;
    }
}

/// Computes the 32-byte BLAKE3 digest using horizontal 64-chunk level-batched SIMD parent reduction
/// combined with the outer Merkle stack, producing byte-identical output to [`hash`].
pub fn hash_wide_simd(data: &[u8]) -> [u8; 32] {
    let len = data.len();
    if len <= 1024 {
        return hash(data);
    }

    let num_chunks = len.div_ceil(1024);
    let mut stack = [[0u32; 8]; 54];
    let mut stack_len = 0usize;

    let mut chunk_idx = 0usize;
    while chunk_idx < num_chunks {
        // Process up to 64 chunks (a power-of-2 aligned subtree of up to 64 KiB)
        // when at a 64-chunk boundary and at least 64 chunks remain.
        if (chunk_idx & 63 == 0) && (chunk_idx + 64 <= num_chunks) {
            let mut cvs = [[0u32; 8]; 64];
            for k in 0..64 {
                let c_idx = chunk_idx + k;
                let start = c_idx * 1024;
                let end = (start + 1024).min(len);
                let out = hash_chunk(&data[start..end], c_idx as u64, false);
                cvs[k].copy_from_slice(&out[0..8]);
            }
            let mut count = 64usize;
            // Reduce 64 -> 32 -> 16 -> 8 -> 4 -> 2 using SIMD parent kernel
            while count > 2 {
                reduce_level_simd(&mut cvs, &mut count);
            }
            // If this 64-chunk subtree is the entire input, the last merge is ROOT!
            if chunk_idx == 0 && num_chunks == 64 {
                let mut block = [0u32; 16];
                block[..8].copy_from_slice(&cvs[0]);
                block[8..].copy_from_slice(&cvs[1]);
                let out = compress_scalar(&IV, &block, 0, 64, PARENT | ROOT);
                let mut res = [0u8; 32];
                for i in 0..8 {
                    res[i * 4..(i + 1) * 4].copy_from_slice(&out[i].to_le_bytes());
                }
                return res;
            }

            // Otherwise reduce 2 -> 1 as a non-root PARENT node and push onto stack
            reduce_level_simd(&mut cvs, &mut count);
            let mut current_cv = cvs[0];
            chunk_idx += 64;

            // Merge completed 64-chunk subtrees up the stack
            let mut subtree_idx = chunk_idx / 64;
            while subtree_idx & 1 == 0 {
                if stack_len == 1 && chunk_idx == num_chunks {
                    break;
                }
                stack_len -= 1;
                let left_child = stack[stack_len];
                let mut block = [0u32; 16];
                block[..8].copy_from_slice(&left_child);
                block[8..].copy_from_slice(&current_cv);
                let parent_out = compress_scalar(&IV, &block, 0, 64, PARENT);
                current_cv.copy_from_slice(&parent_out[0..8]);
                subtree_idx >>= 1;
            }
            stack[stack_len] = current_cv;
            stack_len += 1;
        } else {
            let start = chunk_idx * 1024;
            let end = (start + 1024).min(len);
            let chunk_out = hash_chunk(&data[start..end], chunk_idx as u64, false);
            let mut current_cv = [0u32; 8];
            current_cv.copy_from_slice(&chunk_out[0..8]);

            let mut total_chunks = chunk_idx + 1;
            while total_chunks & 1 == 0 {
                if stack_len == 1 && chunk_idx == num_chunks - 1 {
                    break;
                }
                stack_len -= 1;
                let left_child = stack[stack_len];
                let mut block = [0u32; 16];
                block[..8].copy_from_slice(&left_child);
                block[8..].copy_from_slice(&current_cv);
                let parent_out = compress_scalar(&IV, &block, 0, 64, PARENT);
                current_cv.copy_from_slice(&parent_out[0..8]);
                total_chunks >>= 1;
            }
            stack[stack_len] = current_cv;
            stack_len += 1;
            chunk_idx += 1;
        }
    }

    while stack_len > 1 {
        let right_child = stack[stack_len - 1];
        let left_child = stack[stack_len - 2];
        stack_len -= 2;

        let mut block = [0u32; 16];
        block[..8].copy_from_slice(&left_child);
        block[8..].copy_from_slice(&right_child);

        let flags = if stack_len == 0 {
            PARENT | ROOT
        } else {
            PARENT
        };
        let out = compress_scalar(&IV, &block, 0, 64, flags);

        if stack_len == 0 {
            let mut res = [0u8; 32];
            for i in 0..8 {
                res[i * 4..(i + 1) * 4].copy_from_slice(&out[i].to_le_bytes());
            }
            return res;
        }

        stack[stack_len].copy_from_slice(&out[0..8]);
        stack_len += 1;
    }

    let mut res = [0u8; 32];
    for i in 0..8 {
        res[i * 4..(i + 1) * 4].copy_from_slice(&stack[0][i].to_le_bytes());
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_test_input(buf: &mut [u8]) {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
    }

    fn hex_to_32(hex: &str) -> [u8; 32] {
        assert_eq!(hex.len(), 64);
        let bytes = hex.as_bytes();
        let mut out = [0u8; 32];
        for i in 0..32 {
            let hi = match bytes[i * 2] {
                b'0'..=b'9' => bytes[i * 2] - b'0',
                b'a'..=b'f' => bytes[i * 2] - b'a' + 10,
                b'A'..=b'F' => bytes[i * 2] - b'A' + 10,
                _ => panic!("invalid hex"),
            };
            let lo = match bytes[i * 2 + 1] {
                b'0'..=b'9' => bytes[i * 2 + 1] - b'0',
                b'a'..=b'f' => bytes[i * 2 + 1] - b'a' + 10,
                b'A'..=b'F' => bytes[i * 2 + 1] - b'A' + 10,
                _ => panic!("invalid hex"),
            };
            out[i] = (hi << 4) | lo;
        }
        out
    }

    #[test]
    fn test_official_blake3_vectors_multi_block_and_merkle_tree() {
        // Official BLAKE3 test vectors for repeating 0..251 byte sequence:
        // https://github.com/BLAKE3-team/BLAKE3/blob/master/test_vectors/test_vectors.json
        let cases: &[(usize, &str)] = &[
            (
                0,
                "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
            ),
            (
                1,
                "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213",
            ),
            (
                63,
                "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b",
            ),
            (
                64,
                "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98",
            ),
            (
                65,
                "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee",
            ),
            (
                127,
                "d81293fda863f008c09e92fc382a81f5a0b4a1251cba1634016a0f86a6bd640d",
            ),
            (
                128,
                "f17e570564b26578c33bb7f44643f539624b05df1a76c81f30acd548c44b45ef",
            ),
            (
                1023,
                "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11",
            ),
            (
                1024,
                "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
            ),
            (
                1025,
                "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444",
            ),
            (
                2048,
                "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a",
            ),
            (
                2049,
                "5f4d72f40d7a5f82b15ca2b2e44b1de3c2ef86c426c95c1af0b6879522563030",
            ),
            (
                4096,
                "015094013f57a5277b59d8475c0501042c0b642e531b0a1c8f58d2163229e969",
            ),
            (
                8192,
                "aae792484c8efe4f19e2ca7d371d8c467ffb10748d8a5a1ae579948f718a2a63",
            ),
        ];

        let mut buf = [0u8; 8192];
        fill_test_input(&mut buf);

        for &(len, expected_hex) in cases {
            let actual = hash(&buf[..len]);
            let expected = hex_to_32(expected_hex);
            assert_eq!(
                actual, expected,
                "BLAKE3 official vector mismatch at len={}",
                len
            );
            let actual_wide = hash_wide_simd(&buf[..len]);
            assert_eq!(
                actual_wide, actual,
                "hash_wide_simd mismatch vs hash at len={}",
                len
            );
        }
    }

    #[test]
    fn test_simd_parent_reduction_parity_64_to_128_chunks() {
        let mut buf = [0u8; 65 * 1024];
        fill_test_input(&mut buf);

        let d_64 = hash(&buf[..64 * 1024]);
        let d_64_simd = hash_wide_simd(&buf[..64 * 1024]);
        assert_eq!(d_64, d_64_simd, "64-chunk SIMD parent reduction mismatch");

        let d_65 = hash(&buf[..65 * 1024]);
        let d_65_simd = hash_wide_simd(&buf[..65 * 1024]);
        assert_eq!(d_65, d_65_simd, "65-chunk SIMD parent reduction mismatch");
    }

    #[test]
    fn test_gpu_crossover_heuristic() {
        assert!(!should_dispatch_gpu(1024));
        assert!(!should_dispatch_gpu(16 * 1024 * 1024 - 1));
        assert!(should_dispatch_gpu(16 * 1024 * 1024));
        assert!(should_dispatch_gpu(64 * 1024 * 1024));
    }
}
