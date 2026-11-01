//! CTQ-06 attribution profile for `hash_many_simd` (wave-6 lane 3).
//!
//! Answers: where does the time go — leaf 4-way kernel vs scalar parent merges
//! vs finalization — and is the 8.5 GiB/s LSL reachable without a different
//! kernel shape?
//!
//! Method (public API only, no internals touched):
//! 1. End-to-end `hash_many_simd` at 4/64/256/1024/8192 chunks -> GiB/s curve;
//!    linear fit on the tail separates fixed vs marginal cost.
//! 2. Phase attribution: full replicated pipeline (leaf + merges + finalize)
//!    vs leaf-only loop (identical leaf kernel calls + traffic, no merges);
//!    parent time = full - leaf, cross-checked against a direct merge-only
//!    loop. Finalization is <= 53 scalar compressions (negligible).
//! 3. g_function ceiling: `compress_chunks_4way` on a resident 4 KiB buffer,
//!    tight loop -> ns per 4-chunk batch, isolated from memory traffic.
//! 4. Theoretical floor: 7 rounds x 16 g x 4 lanes = 448 g-ops per batch;
//!    measured g-ops/ns reported against it.
//!
//! Output: JSON attribution report on stdout, plain-text summary on stderr.

use std::time::Instant;

use cgap_blake3::multi_chunk::compress_chunks_4way;
use cgap_blake3::{compress_scalar, hash, hash_chunk, hash_many_simd, hash_wide_simd, IV, PARENT};

struct Timing {
    best: f64,
    rounds: usize,
}

fn time_min<F: FnMut()>(mut f: F, rounds: usize) -> Timing {
    let mut best = f64::INFINITY;
    for r in 0..rounds {
        let t0 = Instant::now();
        f();
        let dt = t0.elapsed().as_secs_f64();
        if dt < best {
            best = dt;
        }
        if r + 1 == rounds {
            return Timing { best, rounds };
        }
    }
    Timing { best, rounds }
}

fn fill(buf: &mut [u8]) {
    let mut x: u64 = 0x9E3779B97F4A7C15;
    for b in buf.iter_mut() {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (x >> 33) as u8;
    }
}

fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Same structure as hash_many_simd's private merge_into_stack, rebuilt from
/// the public compress_scalar. Parity-checked against hash() below.
fn merge_one(
    stack: &mut [[u32; 8]; 54],
    stack_len: &mut usize,
    chunk_cv: [u32; 8],
    chunk_idx: usize,
    num_chunks: usize,
) {
    let mut total = chunk_idx + 1;
    let mut cv = chunk_cv;
    while total & 1 == 0 {
        if *stack_len == 1 && chunk_idx == num_chunks - 1 {
            break;
        }
        *stack_len -= 1;
        let mut block = [0u32; 16];
        block[..8].copy_from_slice(&stack[*stack_len]);
        block[8..].copy_from_slice(&cv);
        let out = compress_scalar(&IV, &block, 0, 64, PARENT);
        cv.copy_from_slice(&out[0..8]);
        total >>= 1;
    }
    stack[*stack_len] = cv;
    *stack_len += 1;
}

/// Same structure as hash_many_simd's private finalize_root.
fn finalize_root(stack: &mut [[u32; 8]; 54], mut stack_len: usize) -> [u8; 32] {
    while stack_len > 1 {
        let right_child = stack[stack_len - 1];
        let left_child = stack[stack_len - 2];
        stack_len -= 2;

        let mut block = [0u32; 16];
        block[..8].copy_from_slice(&left_child);
        block[8..].copy_from_slice(&right_child);

        let flags = if stack_len == 0 { PARENT | ROOT_FLAGS } else { PARENT };
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

const ROOT_FLAGS: u32 = cgap_blake3::ROOT;

/// Replicated hash_many_simd: leaf 4-way batches + per-chunk stack merges +
/// root finalization. Returns digest.
fn replicate_hash_many(data: &[u8]) -> [u8; 32] {
    let len = data.len();
    if len <= 1024 {
        return hash(data);
    }
    let num_chunks = len.div_ceil(1024);
    let mut stack = [[0u32; 8]; 54];
    let mut stack_len = 0usize;
    let mut chunk_idx = 0usize;
    while chunk_idx < num_chunks {
        if chunk_idx + 4 <= num_chunks && (chunk_idx + 4) * 1024 <= len {
            let start = chunk_idx * 1024;
            let outs = compress_chunks_4way(&data[start..start + 4096], chunk_idx as u64);
            for k in 0..4 {
                let cv = [
                    outs[k][0], outs[k][1], outs[k][2], outs[k][3],
                    outs[k][4], outs[k][5], outs[k][6], outs[k][7],
                ];
                merge_one(&mut stack, &mut stack_len, cv, chunk_idx + k, num_chunks);
            }
            chunk_idx += 4;
            continue;
        }
        let start = chunk_idx * 1024;
        let end = (start + 1024).min(len);
        let cv_full = hash_chunk(&data[start..end], chunk_idx as u64, false);
        let cv = [cv_full[0], cv_full[1], cv_full[2], cv_full[3], cv_full[4], cv_full[5], cv_full[6], cv_full[7]];
        merge_one(&mut stack, &mut stack_len, cv, chunk_idx, num_chunks);
        chunk_idx += 1;
    }
    finalize_root(&mut stack, stack_len)
}

fn main() {
    let rounds_default: usize = std::env::var("PROFILE_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);

    // ------------------------------------------------------------------
    // Sanity: replicated loop parity vs hash_many_simd and hash
    // ------------------------------------------------------------------
    {
        let mut buf = [0u8; 65 * 1024];
        fill(&mut buf);
        let reference = hash(&buf);
        assert_eq!(reference, hash_many_simd(&buf), "hash_many_simd != hash");
        assert_eq!(reference, replicate_hash_many(&buf), "replica != hash");
        let _ = hex(&reference);
    }

    let mut json_lines: Vec<String> = Vec::new();

    // ------------------------------------------------------------------
    // 1. End-to-end scaling curve
    // ------------------------------------------------------------------
    let chunk_counts: [usize; 5] = [4, 64, 256, 1024, 8192];
    let mut e2e: Vec<(usize, f64, f64, f64)> = Vec::new(); // chunks, secs, GiB/s, ns/chunk
    for &chunks in chunk_counts.iter() {
        let bytes = chunks * 1024;
        let mut buf = vec![0u8; bytes];
        fill(&mut buf);
        let rounds = if chunks <= 256 { 300 } else { rounds_default };
        let t = time_min(
            || {
                let d = hash_many_simd(&buf);
                std::hint::black_box(&d);
            },
            rounds,
        );
        let gib = (bytes as f64) / t.best / (1024.0 * 1024.0 * 1024.0);
        let ns_per_chunk = t.best * 1e9 / chunks as f64;
        e2e.push((chunks, t.best, gib, ns_per_chunk));
        json_lines.push(format!(
            r#"  {{"section": "e2e", "chunks": {}, "bytes": {}, "rounds": {}, "secs": {:.6}, "gib_s": {:.3}, "ns_per_chunk": {:.2}}}"#,
            chunks, bytes, t.rounds, t.best, gib, ns_per_chunk
        ));
    }

    // ------------------------------------------------------------------
    // 2. Phase attribution
    // ------------------------------------------------------------------
    let mut phases: Vec<(usize, f64, f64, f64, f64, f64)> = Vec::new();
    for &chunks in &[256usize, 1024, 8192] {
        let bytes = chunks * 1024;
        let mut buf = vec![0u8; bytes];
        fill(&mut buf);

        let full = time_min(
            || {
                let d = replicate_hash_many(&buf);
                std::hint::black_box(&d);
            },
            rounds_default,
        );

        let leaf = time_min(
            || {
                let mut acc = 0u32;
                let n_full = (bytes / 4096) * 4;
                let mut ci = 0usize;
                while ci + 4 <= n_full {
                    let out = compress_chunks_4way(&buf[ci * 1024..ci * 1024 + 4096], ci as u64);
                    acc = acc.wrapping_add(out[0][0]);
                    ci += 4;
                }
                let tail_chunks = chunks - n_full;
                for k in 0..tail_chunks {
                    let start = (n_full + k) * 1024;
                    let cv = hash_chunk(&buf[start..start + 1024], (n_full + k) as u64, false);
                    acc = acc.wrapping_add(cv[0]);
                }
                std::hint::black_box(acc);
            },
            rounds_default,
        );

        let merge = time_min(
            || {
                let mut stack: [[u32; 8]; 54] = [[0; 8]; 54];
                let mut stack_len = 0usize;
                for idx in 0..chunks {
                    let cv = [idx as u32; 8];
                    merge_one(&mut stack, &mut stack_len, cv, idx, chunks);
                }
                let d = finalize_root(&mut stack, stack_len);
                std::hint::black_box(&d);
            },
            rounds_default,
        );

        let parent_diff = full.best - leaf.best;
        let parent_share = parent_diff / full.best * 100.0;
        phases.push((chunks, full.best, leaf.best, parent_diff, parent_share, merge.best));
        json_lines.push(format!(
            r#"  {{"section": "phases", "chunks": {}, "full_s": {:.6}, "leaf_s": {:.6}, "parents_s": {:.6}, "parent_share_pct": {:.2}, "merge_loop_alone_s": {:.6}}}"#,
            chunks, full.best, leaf.best, parent_diff, parent_share, merge.best
        ));
    }

    // ------------------------------------------------------------------
    // 3. g_function compute ceiling (resident 4 KiB, tight loop)
    // ------------------------------------------------------------------
    let mut resident = [0u8; 4096];
    fill(&mut resident);
    let ceiling_rounds: u64 = 200_000;
    {
        let mut acc = 0u32;
        for _ in 0..2_000u64 {
            let out = compress_chunks_4way(&resident, 0);
            acc = acc.wrapping_add(out[0][0]);
        }
        std::hint::black_box(acc);
    }
    let ceil = time_min(
        || {
            for _ in 0..ceiling_rounds {
                let out = compress_chunks_4way(&resident, 0);
                std::hint::black_box(out[0][0]);
            }
        },
        7,
    );
    let ns_per_batch = ceil.best * 1e9 / ceiling_rounds as f64;
    let ns_per_chunk_ceil = ns_per_batch / 4.0;
    let bytes_per_ns = 4096.0 / ns_per_batch;
    let ceil_gibs = bytes_per_ns * 1e9 / (1024.0 * 1024.0 * 1024.0);
    let g_ops_per_ns = 448.0 / ns_per_batch;
    json_lines.push(format!(
        r#"  {{"section": "ceiling", "ns_per_4chunk_batch": {:.3}, "ns_per_chunk": {:.3}, "gib_s_resident": {:.3}, "g_ops_per_ns": {:.3}}}"#,
        ns_per_batch, ns_per_chunk_ceil, ceil_gibs, g_ops_per_ns
    ));

    // ------------------------------------------------------------------
    // 4. Linear fit on the two largest e2e points: fixed vs marginal
    // ------------------------------------------------------------------
    let t1 = e2e[3].1;
    let c1 = e2e[3].0;
    let t2 = e2e[4].1;
    let c2 = e2e[4].0;
    let marginal_ns_chunk = (t2 - t1) / (c2 - c1) as f64 * 1e9;
    let fixed_ns = (t1 - marginal_ns_chunk * c1 as f64).max(0.0);
    json_lines.push(format!(
        r#"  {{"section": "fit", "marginal_ns_per_chunk": {:.3}, "fixed_ns": {:.3}}}"#,
        marginal_ns_chunk, fixed_ns
    ));

    // ------------------------------------------------------------------
    // 5. hash_wide_simd (level-batched SIMD parents) at 8192 chunks
    // ------------------------------------------------------------------
    let gib_many_8k = e2e[4].2;
    let gib_wide_8k = {
        let bytes = 8192 * 1024;
        let mut buf = vec![0u8; bytes];
        fill(&mut buf);
        let t = time_min(
            || {
                let d = hash_wide_simd(&buf);
                std::hint::black_box(&d);
            },
            rounds_default,
        );
        bytes as f64 / t.best / (1024.0 * 1024.0 * 1024.0)
    };
    json_lines.push(format!(
        r#"  {{"section": "wide-vs-many", "hash_many_simd_gib_s_8k": {:.3}, "hash_wide_simd_gib_s_8k": {:.3}}}"#,
        gib_many_8k, gib_wide_8k
    ));

    // ------------------------------------------------------------------
    // Emit JSON
    // ------------------------------------------------------------------
    println!("[");
    for (i, l) in json_lines.iter().enumerate() {
        if i + 1 < json_lines.len() {
            println!("{},", l);
        } else {
            println!("{}", l);
        }
    }
    println!("]");

    // ------------------------------------------------------------------
    // Plain-text summary on stderr
    // ------------------------------------------------------------------
    eprintln!("== blake3_profile summary ==");
    for (chunks, secs, gib, ns) in &e2e {
        eprintln!(
            "e2e    {:>5} chunks: {:8.4} s  {:7.3} GiB/s  {:7.2} ns/chunk",
            chunks, secs, gib, ns
        );
    }
    for (chunks, full, leaf, parents, share, merge_alone) in &phases {
        eprintln!(
            "phase  {:>5} chunks: leaf {:7.3} ms / parents {:7.3} ms / full {:7.3} ms -> parents = {:.1}%",
            chunks,
            leaf * 1e3,
            parents * 1e3,
            full * 1e3,
            share
        );
        eprintln!(
            "         merge-loop alone (direct): {:.3} ms (cross-check full-leaf = {:.3} ms)",
            merge_alone * 1e3,
            parents * 1e3
        );
    }
    eprintln!(
        "ceiling: compress_chunks_4way resident 4 KiB: {:.2} ns/batch = {:.2} ns/chunk = {:.2} GiB/s compute ceiling ({:.2} g-ops/ns)",
        ns_per_batch, ns_per_chunk_ceil, ceil_gibs, g_ops_per_ns
    );
    eprintln!(
        "fit: marginal {:.2} ns/chunk, fixed {:.2} us",
        marginal_ns_chunk,
        fixed_ns / 1e3
    );
    eprintln!(
        "verdict inputs: e2e@8k={:.3} GiB/s; wide@8k={:.3} GiB/s; leaf-ceiling={:.3} GiB/s; parents={:.1}% at 8k chunks",
        gib_many_8k, gib_wide_8k, ceil_gibs, phases.last().unwrap().4
    );
    let _ = LSL;
}

const LSL: f64 = 8.5;
