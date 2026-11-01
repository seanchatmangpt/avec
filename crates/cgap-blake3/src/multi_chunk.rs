//! Leaf-level multi-input chunk kernel: hashes 4 independent 1 KiB chunks in
//! parallel across NEON v128 lanes (one lane per chunk), 7 rounds vectorized
//! with the standard message permutation. Portable scalar fallback delegates
//! to the sequential [`crate::hash_chunk`]. Outputs are byte-identical to the
//! sequential path (same compress function, lane-transposed).

/// Computes the chunk CVs of 4 consecutive full 1 KiB chunks in parallel.
///
/// `data` must be exactly 4096 bytes (4 full chunks). `base_chunk_idx` is the
/// BLAKE3 chunk counter of the first chunk; subsequent lanes use +1, +2, +3.
/// Returns the 16-word compression output of each chunk's final block,
/// per lane — identical to calling [`crate::hash_chunk`] on each 1 KiB chunk
/// with `is_root = false`.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn vst1q_one(out: &[u32; 4], v: core::arch::aarch64::uint32x4_t) {
    unsafe { core::arch::aarch64::vst1q_u32(out.as_ptr() as *mut u32, v) }
}

pub fn compress_chunks_4way(data: &[u8], base_chunk_idx: u64) -> [[u32; 16]; 4] {
    debug_assert_eq!(data.len(), 4096);

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: aarch64 implies NEON; `data.len() == 4096` checked above.
        unsafe { compress_chunks_4way_neon(data, base_chunk_idx) }
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let mut out = [[0u32; 16]; 4];
        for l in 0..4 {
            out[l] = crate::hash_chunk(
                &data[l * 1024..(l + 1) * 1024],
                base_chunk_idx + l as u64,
                false,
            );
        }
        out
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn compress_chunks_4way_neon(data: &[u8], base_chunk_idx: u64) -> [[u32; 16]; 4] {
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
    unsafe fn g4(
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
    unsafe fn round4(s: &mut [uint32x4_t; 16], m: &[uint32x4_t; 16]) {
        unsafe {
            g4(s, 0, 4, 8, 12, m[0], m[1]);
            g4(s, 1, 5, 9, 13, m[2], m[3]);
            g4(s, 2, 6, 10, 14, m[4], m[5]);
            g4(s, 3, 7, 11, 15, m[6], m[7]);
            g4(s, 0, 5, 10, 15, m[8], m[9]);
            g4(s, 1, 6, 11, 12, m[10], m[11]);
            g4(s, 2, 7, 8, 13, m[12], m[13]);
            g4(s, 3, 4, 9, 14, m[14], m[15]);
        }
    }
    // Wave-7 lane 1 (CTQ-06): register-resident permutation. The previous
    // loop over MSG_PERMUTATION forced a runtime-indexed memory round-trip of
    // all 16 message vectors every round; these literal-index copies let LLVM
    // SROA the whole message array into the NEON register file (zero
    // instructions for the permute itself).
    #[inline(always)]
    unsafe fn permute4(m: &mut [uint32x4_t; 16]) {
        let p = *m;
        m[0] = p[2];
        m[1] = p[6];
        m[2] = p[3];
        m[3] = p[10];
        m[4] = p[7];
        m[5] = p[0];
        m[6] = p[4];
        m[7] = p[13];
        m[8] = p[1];
        m[9] = p[11];
        m[10] = p[12];
        m[11] = p[5];
        m[12] = p[9];
        m[13] = p[14];
        m[14] = p[15];
        m[15] = p[8];
    }

    unsafe {
        let cnt_lo = vld1q_u32(
            [
                base_chunk_idx as u32,
                (base_chunk_idx + 1) as u32,
                (base_chunk_idx + 2) as u32,
                (base_chunk_idx + 3) as u32,
            ]
            .as_ptr(),
        );
        let cnt_hi = vld1q_u32(
            [
                (base_chunk_idx >> 32) as u32,
                ((base_chunk_idx + 1) >> 32) as u32,
                ((base_chunk_idx + 2) >> 32) as u32,
                ((base_chunk_idx + 3) >> 32) as u32,
            ]
            .as_ptr(),
        );

        // Chained CV held COLUMN-major as NEON vectors across blocks: cv_cols[i]
        // = {cv0[i], cv1[i], cv2[i], cv3[i]}. cv starts as IV per lane, so
        // column i is IV[i] broadcast. This keeps the per-block finalize (and the
        // next block's state rebuild) entirely in registers.
        let mut cv_cols = {
            let mut c = [vdupq_n_u32(0); 8];
            let mut i = 0;
            while i < 8 {
                c[i] = vdupq_n_u32(crate::IV[i]);
                i += 1;
            }
            c
        };
        let mut out = [[0u32; 16]; 4];

        for b in 0..16 {
            let cv_pre_cols = cv_cols;
            // Wave-5 lane 2, order (a): message schedule is now 16x (NEON 16-byte
            // LE byte-load + 4x4 u32 word transpose) instead of 64 scalar
            // from_le_bytes column loads.
            let mut m = [vdupq_n_u32(0); 16];
            let base = b * 64;
            for k in 0..4 {
                let o = base + k * 16;
                let r0: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(o)));
                let r1: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(1024 + o)));
                let r2: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(2048 + o)));
                let r3: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(3072 + o)));
                let t01 = vtrnq_u32(r0, r1);
                let t23 = vtrnq_u32(r2, r3);
                m[k * 4] = vcombine_u32(vget_low_u32(t01.0), vget_low_u32(t23.0));
                m[k * 4 + 1] = vcombine_u32(vget_low_u32(t01.1), vget_low_u32(t23.1));
                m[k * 4 + 2] = vcombine_u32(vget_high_u32(t01.0), vget_high_u32(t23.0));
                m[k * 4 + 3] = vcombine_u32(vget_high_u32(t01.1), vget_high_u32(t23.1));
            }

            let flags = if b == 0 { crate::CHUNK_START } else { 0 }
                | if b == 15 { crate::CHUNK_END } else { 0 };
            let flags_vec = vdupq_n_u32(flags);

            let mut s = [
                cv_cols[0],
                cv_cols[1],
                cv_cols[2],
                cv_cols[3],
                cv_cols[4],
                cv_cols[5],
                cv_cols[6],
                cv_cols[7],
                vdupq_n_u32(crate::IV[0]),
                vdupq_n_u32(crate::IV[1]),
                vdupq_n_u32(crate::IV[2]),
                vdupq_n_u32(crate::IV[3]),
                cnt_lo,
                cnt_hi,
                vdupq_n_u32(64),
                flags_vec,
            ];

            round4(&mut s, &m);
            permute4(&mut m);
            round4(&mut s, &m);
            permute4(&mut m);
            round4(&mut s, &m);
            permute4(&mut m);
            round4(&mut s, &m);
            permute4(&mut m);
            round4(&mut s, &m);
            permute4(&mut m);
            round4(&mut s, &m);
            permute4(&mut m);
            round4(&mut s, &m);

            // Finalize: cv' = s[0..8] ^ s[8..16]; output words = full s[0..16] on
            // the last block only (matches hash_chunk returning full output).
            let last = b == 15;
            for i in 0..8 {
                let hi = s[i + 8];
                let xor = veorq_u32(s[i], hi);
                cv_cols[i] = xor;
                if last {
                    let mut lane = [0u32; 4];
                    vst1q_u32(lane.as_mut_ptr(), xor);
                    let mut hi_lane = [0u32; 4];
                    vst1q_u32(hi_lane.as_mut_ptr(), hi);
                    let mut cpre = [0u32; 4];
                    vst1q_u32(cpre.as_mut_ptr(), cv_pre_cols[i]);
                    for l in 0..4 {
                        out[l][i] = lane[l];
                        out[l][i + 8] = hi_lane[l] ^ cpre[l];
                    }
                }
            }
        }

        out
    }
}

/// Interleaved 8-chunk variant (wave-5 lane 2, order (b)): two independent
/// 4-way groups with their NEON round chains overlapped for ILP. The true
/// 8-v128-lane form is register-infeasible; this form runs two 16-vector
/// states so two dependency chains stay in flight.
///
/// Refutation note (wave-8): the interleave hypothesis was refuted — the
/// assembly evidence shows the chains were already overlapped / spill-bound,
/// so the x2 shape is retained as measured-equivalent, NOT as an ILP win
/// (docs/jira/v26.10.31/48-lsl-rederivation.md, Wave-8 Measurement Appendix).
///
/// `data` must be exactly 8192 bytes (8 full 1 KiB chunks). Group A hashes
/// `data[0..4096]` with counters base..base+4; group B hashes
/// `data[4096..8192]` with base+4..base+8. Byte-identical to two flattened
/// [`compress_chunks_4way`] calls.
pub fn compress_chunks_4way_x2(data: &[u8], base_chunk_idx: u64) -> [[[u32; 16]; 4]; 2] {
    debug_assert_eq!(data.len(), 8192);

    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: aarch64 implies NEON; `data.len() == 8192` checked above.
        unsafe { compress_chunks_4way_x2_neon(data, base_chunk_idx) }
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        let mut out = [[[0u32; 16]; 4]; 2];
        for g in 0..2 {
            out[g] = compress_chunks_4way(
                &data[g * 4096..(g + 1) * 4096],
                base_chunk_idx + (g as u64) * 4,
            );
        }
        out
    }
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn compress_chunks_4way_x2_neon(data: &[u8], base_chunk_idx: u64) -> [[[u32; 16]; 4]; 2] {
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
    unsafe fn g4(
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
    unsafe fn round4(s: &mut [uint32x4_t; 16], m: &[uint32x4_t; 16]) {
        unsafe {
            g4(s, 0, 4, 8, 12, m[0], m[1]);
            g4(s, 1, 5, 9, 13, m[2], m[3]);
            g4(s, 2, 6, 10, 14, m[4], m[5]);
            g4(s, 3, 7, 11, 15, m[6], m[7]);
            g4(s, 0, 5, 10, 15, m[8], m[9]);
            g4(s, 1, 6, 11, 12, m[10], m[11]);
            g4(s, 2, 7, 8, 13, m[12], m[13]);
            g4(s, 3, 4, 9, 14, m[14], m[15]);
        }
    }
    // Wave-7 lane 1 (CTQ-06): register-resident permutation, same shape as the
    // 4way kernel above (literal-index copies, SROA-friendly).
    #[inline(always)]
    unsafe fn permute4(m: &mut [uint32x4_t; 16]) {
        let p = *m;
        m[0] = p[2];
        m[1] = p[6];
        m[2] = p[3];
        m[3] = p[10];
        m[4] = p[7];
        m[5] = p[0];
        m[6] = p[4];
        m[7] = p[13];
        m[8] = p[1];
        m[9] = p[11];
        m[10] = p[12];
        m[11] = p[5];
        m[12] = p[9];
        m[13] = p[14];
        m[14] = p[15];
        m[15] = p[8];
    }

    unsafe {
        let mut lo = [0u32; 8];
        let mut hi = [0u32; 8];
        for g in 0..8 {
            lo[g] = (base_chunk_idx + g as u64) as u32;
            hi[g] = ((base_chunk_idx + g as u64) >> 32) as u32;
        }
        let cnt_lo_a = vld1q_u32(lo.as_ptr());
        let cnt_hi_a = vld1q_u32(hi.as_ptr());
        let cnt_lo_b = vld1q_u32(lo.as_ptr().add(4));
        let cnt_hi_b = vld1q_u32(hi.as_ptr().add(4));

        let mut cva = {
            let mut c = [vdupq_n_u32(0); 8];
            let mut i = 0;
            while i < 8 {
                c[i] = vdupq_n_u32(crate::IV[i]);
                i += 1;
            }
            c
        };
        let mut cvb = cva;
        let mut out = [[[0u32; 16]; 4]; 2];

        for b in 0..16 {
            let cv_pre_a = cva;
            let cv_pre_b = cvb;
            let base = b * 64;

            // Both groups: NEON byte-load + 4x4 u32 transpose message schedule.
            let mut ma = [vdupq_n_u32(0); 16];
            let mut mb = [vdupq_n_u32(0); 16];
            for k in 0..4 {
                let o = base + k * 16;
                let r0: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(o)));
                let r1: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(1024 + o)));
                let r2: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(2048 + o)));
                let r3: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(3072 + o)));
                let rb0: uint32x4_t = vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(4096 + o)));
                let rb1: uint32x4_t =
                    vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(4096 + 1024 + o)));
                let rb2: uint32x4_t =
                    vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(4096 + 2048 + o)));
                let rb3: uint32x4_t =
                    vreinterpretq_u32_u8(vld1q_u8(data.as_ptr().add(4096 + 3072 + o)));
                let t01 = vtrnq_u32(r0, r1);
                let t23 = vtrnq_u32(r2, r3);
                let u01 = vtrnq_u32(rb0, rb1);
                let u23 = vtrnq_u32(rb2, rb3);
                ma[k * 4] = vcombine_u32(vget_low_u32(t01.0), vget_low_u32(t23.0));
                ma[k * 4 + 1] = vcombine_u32(vget_low_u32(t01.1), vget_low_u32(t23.1));
                ma[k * 4 + 2] = vcombine_u32(vget_high_u32(t01.0), vget_high_u32(t23.0));
                ma[k * 4 + 3] = vcombine_u32(vget_high_u32(t01.1), vget_high_u32(t23.1));
                mb[k * 4] = vcombine_u32(vget_low_u32(u01.0), vget_low_u32(u23.0));
                mb[k * 4 + 1] = vcombine_u32(vget_low_u32(u01.1), vget_low_u32(u23.1));
                mb[k * 4 + 2] = vcombine_u32(vget_high_u32(u01.0), vget_high_u32(u23.0));
                mb[k * 4 + 3] = vcombine_u32(vget_high_u32(u01.1), vget_high_u32(u23.1));
            }

            let flags = if b == 0 { crate::CHUNK_START } else { 0 }
                | if b == 15 { crate::CHUNK_END } else { 0 };
            let flags_vec = vdupq_n_u32(flags);

            // Two independent 4-lane states, chained per group.
            let mut sa = [vdupq_n_u32(0); 16];
            let mut sb = [vdupq_n_u32(0); 16];
            for i in 0..8 {
                sa[i] = cv_pre_a[i];
                sb[i] = cv_pre_b[i];
                sa[i + 8] = vdupq_n_u32(crate::IV[i]);
                sb[i + 8] = vdupq_n_u32(crate::IV[i]);
            }
            sa[12] = cnt_lo_a;
            sa[13] = cnt_hi_a;
            sa[14] = vdupq_n_u32(64);
            sa[15] = flags_vec;
            sb[12] = cnt_lo_b;
            sb[13] = cnt_hi_b;
            sb[14] = vdupq_n_u32(64);
            sb[15] = flags_vec;

            round4(&mut sa, &ma);
            round4(&mut sb, &mb);
            permute4(&mut ma);
            permute4(&mut mb);
            round4(&mut sa, &ma);
            round4(&mut sb, &mb);
            permute4(&mut ma);
            permute4(&mut mb);
            round4(&mut sa, &ma);
            round4(&mut sb, &mb);
            permute4(&mut ma);
            permute4(&mut mb);
            round4(&mut sa, &ma);
            round4(&mut sb, &mb);
            permute4(&mut ma);
            permute4(&mut mb);
            round4(&mut sa, &ma);
            round4(&mut sb, &mb);
            permute4(&mut ma);
            permute4(&mut mb);
            round4(&mut sa, &ma);
            round4(&mut sb, &mb);
            permute4(&mut ma);
            permute4(&mut mb);
            round4(&mut sa, &ma);
            round4(&mut sb, &mb);

            let mut lanea = [0u32; 4];
            let mut laneb = [0u32; 4];
            for i in 0..8 {
                let xa = veorq_u32(sa[i], sa[i + 8]);
                cva[i] = xa;
                let xb = veorq_u32(sb[i], sb[i + 8]);
                cvb[i] = xb;
                if b == 15 {
                    vst1q_u32(lanea.as_mut_ptr(), xa);
                    vst1q_u32(laneb.as_mut_ptr(), xb);
                    let mut hia_arr = [0u32; 4];
                    let mut hib_arr = [0u32; 4];
                    vst1q_u32(hia_arr.as_mut_ptr(), sa[i + 8]);
                    vst1q_u32(hib_arr.as_mut_ptr(), sb[i + 8]);
                    let cpre_a = [0u32; 4];
                    let cpre_b = [0u32; 4];
                    vst1q_one(&cpre_a, cv_pre_a[i]);
                    vst1q_one(&cpre_b, cv_pre_b[i]);
                    for l in 0..4 {
                        out[0][l][i] = lanea[l];
                        out[0][l][i + 8] = hia_arr[l] ^ cpre_a[l];
                        out[1][l][i] = laneb[l];
                        out[1][l][i + 8] = hib_arr[l] ^ cpre_b[l];
                    }
                }
            }
        }

        out
    }
}
