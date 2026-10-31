//! Wave-6 lane-1 courts for the 4-way / 8-way leaf-chunk NEON kernels
//! ([`avec_blake3::hash_many_simd`] / [`avec_blake3::hash_many_simd_x2`]).
//!
//! Correctness: byte-identity to the sequential [`avec_blake3::hash`] path
//! over a size sweep that crosses every kernel dispatch boundary (scalar
//! tail, < 4 full chunks, exactly 4, 4-full + tail, 8-chunk dual group,
//! 8-full + tail), including odd lengths.
//!
//! Throughput receipt (CTQ-06): run with
//! `cargo test -p avec-blake3 --release --test multi_chunk_vectors bench_throughput -- --ignored --nocapture`

use avec_blake3::{hash, hash_many_simd, hash_many_simd_x2};

fn pseudo_random_bytes(n: usize) -> Vec<u8> {
    // SplitMix64 byte stream: deterministic, non-degenerate (no all-zero /
    // all-0xFF blocks, exercises every message-schedule lane).
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push((x >> 33) as u8);
    }
    v
}

#[test]
fn multi_chunk_matches_sequential_across_dispatch_boundaries() {
    // Sizes straddling every dispatch boundary in hash_many_simd /
    // hash_many_simd_x2: 1024 (scalar), 1025 (first 4-way batch... no —
    // 1025..2048 still scalar tail only), 4096 (single 4-way batch), 4097+
    // (batch + scalar tail), 8192 (two 4-way batches / one x2 group pair),
    // 8193+ (x2 batch + tail), plus 20480 (5 batches), 32768 (8 batches),
    // and odd lengths at each boundary +/- 1.
    let mut sizes: Vec<usize> = vec![
        1,
        31,
        63,
        64,
        1023,
        1024,
        1025,
        2047,
        2048,
        4095,
        4096,
        4097,
        5000,
        6143,
        6144,
        6145,
        8191,
        8192,
        8193,
        12287,
        12288,
        12289,
        20480,
        32768,
        40000,
        65536,
        65537,
    ];
    for b in [4095usize, 4096, 4097, 8191, 8192, 8193] {
        sizes.push(b - 1);
        sizes.push(b + 1);
    }
    sizes.push(77023); // 75.4 KiB — 75 full chunks + odd tail, x2 + 4way + tail mix
    sizes.push(77024);

    for (i, &size) in sizes.iter().enumerate() {
        let data = pseudo_random_bytes(size);
        let ref_d = hash(&data);
        assert_eq!(hash_many_simd(&data), ref_d, "hash_many_simd size={} idx={}", size, i);
        assert_eq!(
            hash_many_simd_x2(&data),
            ref_d,
            "hash_many_simd_x2 size={} idx={}",
            size,
            i
        );
        assert_eq!(
            avec_blake3::hash_wide_simd(&data),
            ref_d,
            "hash_wide_simd size={} idx={}",
            size,
            i
        );
    }
}

#[test]
fn multi_chunk_all_zero_and_all_ff_blocks() {
    // Degenerate message schedules: a real transpose bug (lane swap / word
    // rotation) still produces *a* valid-looking digest, so pin the exact
    // digests of degenerate inputs against the sequential path explicitly.
    for &fill in [0x00u8, 0xFF].iter() {
        let data = vec![fill; 77023];
        assert_eq!(hash_many_simd(&data), hash(&data), "fill={:#x}", fill);
        assert_eq!(hash_many_simd_x2(&data), hash(&data), "fill={:#x}", fill);
    }
}

/// CTQ-06 throughput receipt: 256 MiB, min-of-5, `Instant`-based, no sleeps.
/// Ignored by default so `cargo test -p avec-blake3` stays fast; run with
/// `--ignored --nocapture` in release.
#[test]
#[ignore]
fn bench_throughput_256mib_min_of_5() {
    let byt: usize = 256 * 1024 * 1024;
    let buf = pseudo_random_bytes(byt);

    let mut best_many = f64::INFINITY;
    let mut best_x2 = f64::INFINITY;
    let mut best_wide = f64::INFINITY;
    for round in 0..5 {
        let t0 = std::time::Instant::now();
        let d = hash_many_simd(&buf);
        let dt = t0.elapsed().as_secs_f64();
        std::hint::black_box(&d);
        best_many = best_many.min(dt);

        let t0 = std::time::Instant::now();
        let d = hash_many_simd_x2(&buf);
        let dt = t0.elapsed().as_secs_f64();
        std::hint::black_box(&d);
        best_x2 = best_x2.min(dt);

        let t0 = std::time::Instant::now();
        let d = avec_blake3::hash_wide_simd(&buf);
        let dt = t0.elapsed().as_secs_f64();
        std::hint::black_box(&d);
        best_wide = best_wide.min(dt);
        eprintln!(
            "round {}: many={:.3}s x2={:.3}s wide={:.3}s",
            round, best_many, best_x2, best_wide
        );
    }
    let g = |secs: f64| byt as f64 / secs / (1024.0 * 1024.0 * 1024.0);
    println!(
        "RESULT bytes={} rounds=5 hash_many_simd={:.2} GiB/s hash_many_simd_x2={:.2} GiB/s hash_wide_simd={:.2} GiB/s",
        byt,
        g(best_many),
        g(best_x2),
        g(best_wide)
    );
}

#[test]
fn x2_kernel_equals_two_4way_kernels() {
    // Kernel-level court: compress_chunks_4way_x2 must equal two independent
    // compress_chunks_4way calls on the same halves, isolating the dual-group
    // NEON chain from the Merkle wrapper.
    // Kernel requires exactly 8192 bytes; dispatch-level odd lengths are
    // covered by the sweep court above.
    let data = pseudo_random_bytes(8192);
    let x2 = avec_blake3::multi_chunk::compress_chunks_4way_x2(&data, 0);
    let a = avec_blake3::multi_chunk::compress_chunks_4way(&data[..4096], 0);
    let b = avec_blake3::multi_chunk::compress_chunks_4way(&data[4096..], 4);
    assert_eq!(x2[0], a, "group A");
    assert_eq!(x2[1], b, "group B");

    // Different base chunk counter: both groups' counters must advance
    // together (guards the cnt_lo_b/cnt_hi_b .add(4) loads).
    let data2 = pseudo_random_bytes(8192);
    let x2 = avec_blake3::multi_chunk::compress_chunks_4way_x2(&data2, 100);
    let a = avec_blake3::multi_chunk::compress_chunks_4way(&data2[..4096], 100);
    let b = avec_blake3::multi_chunk::compress_chunks_4way(&data2[4096..], 104);
    assert_eq!(x2[0], a, "group A base=100");
    assert_eq!(x2[1], b, "group B base=104");
}

// Wave-5 lane 2 (re-added additively): external-authority court. The x2 leaf
// path (4-way x2 batches wired into hash_many_simd) must stay byte-identical
// to b3sum on the 25-length i%251 corpus plus odd/unaligned lengths.
#[test]
fn differential_vs_b3sum_corpus_25_and_odd_lengths() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn b3sum(data: &[u8]) -> String {
        let mut child = Command::new("/opt/homebrew/bin/b3sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn b3sum");
        child.stdin.as_mut().unwrap().write_all(data).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "b3sum failed: {:?}", out.status);
        let s = String::from_utf8(out.stdout).unwrap();
        s.split_whitespace().next().expect("b3sum hex").to_string()
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    let mut buf = vec![0u8; 4096 * 25];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    for len in 1..=25 {
        let data = &buf[..len * 1024];
        assert_eq!(
            hex(&avec_blake3::hash_many_simd(data)),
            b3sum(data),
            "hash_many_simd != b3sum at len={} KiB",
            len
        );
    }
    for len in [1025usize, 2049, 3072, 3073, 4095, 4609, 6144, 8192, 20480, 40960] {
        let data = &buf[..len];
        assert_eq!(
            hex(&avec_blake3::hash_many_simd(data)),
            b3sum(data),
            "hash_many_simd != b3sum at len={} bytes",
            len
        );
    }
}

// =========================================================================
// Wave-6 lane 2 (CTQ-06): dual-group 8-chunk driver (hash_many_simd_x2)
// =========================================================================

#[test]
fn x2_driver_matches_b3sum() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn b3sum(data: &[u8]) -> String {
        let mut child = Command::new("/opt/homebrew/bin/b3sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn b3sum");
        child.stdin.as_mut().unwrap().write_all(data).unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "b3sum failed: {:?}", out.status);
        let s = String::from_utf8(out.stdout).unwrap();
        s.split_whitespace().next().expect("b3sum hex").to_string()
    }
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    let mut buf = vec![0u8; 4096 * 25];
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    // Whole-KiB lengths 1..=25 exercise: 4-way-only (len<8), mixed 8/4
    // remainders (len%8 in 4..7), pure-8 batches (len%8==0), and the
    // scalar tail (len%4 != 0).
    for len in 1..=25 {
        let data = &buf[..len * 1024];
        assert_eq!(
            hex(&avec_blake3::hash_many_simd_x2(data)),
            b3sum(data),
            "hash_many_simd_x2 != b3sum at len={} KiB",
            len
        );
    }
    // Non-whole-chunk tails route to the scalar path inside the driver.
    for len in [1025usize, 2049, 3072, 3073, 4095, 4609, 6144, 8192, 20480, 40960] {
        let data = &buf[..len];
        assert_eq!(
            hex(&avec_blake3::hash_many_simd_x2(data)),
            b3sum(data),
            "hash_many_simd_x2 != b3sum at len={} bytes",
            len
        );
    }
}

#[test]
fn x2_kernel_matches_scalar_and_4way() {
    let data: Vec<u8> = (0..8192usize).map(|i| (i % 251) as u8).collect();
    for base in [0u64, 1, 5, 63, 1 << 20, u64::MAX - 7] {
        let x2 = avec_blake3::multi_chunk::compress_chunks_4way_x2(&data, base);
        for g in 0..2usize {
            for l in 0..4usize {
                let c = g * 4 + l;
                let s = avec_blake3::hash_chunk(
                    &data[c * 1024..(c + 1) * 1024],
                    base + c as u64,
                    false,
                );
                assert_eq!(x2[g][l], s, "x2 group {g} lane {l} (base={base}) != scalar");
            }
        }
    }
}

/// Wave-7 lane 1 (CTQ-06): court for the register-resident permutation. A
/// full 1 KiB chunk runs 7 rounds x 6 permutes of the message schedule; this
/// court pins the unrolled-permute 4way kernel to the scalar sequential
/// compress across randomized data patterns and chunk counters, so any
/// mis-transcription of the literal MSG_PERMUTATION indices (2,6,3,10,7,0,4,
/// 13,1,11,12,5,9,14,15,8) in `permute4` fails loudly.
#[test]
fn register_resident_permute_4way_matches_scalar_multi_pattern() {
    // Three patterns hitting distinct permutation paths: splitmix, period-3
    // ramp (misaligned with %16 windows), and repeated 32-bit words (equal
    // vectors would mask a wrong shuffle).
    // Three 1 KiB patterns hitting distinct permutation paths: splitmix-style
    // ramp, period-3 cycle (misaligned with the 16-word message windows), and
    // a repeated 32-bit word (equal vectors would mask a wrong shuffle).
    let mut data: Vec<u8> = (0..2048usize).map(|i| (i.wrapping_mul(7) ^ (i >> 3)) as u8).collect();
    data.extend((0..1024usize).map(|i| (i % 3) as u8));
    data.extend([0x5Au8; 1024]);

    for base in [0u64, 3, 1 << 40, u64::MAX - 3] {
        let four = avec_blake3::multi_chunk::compress_chunks_4way(&data, base);
        for l in 0..4usize {
            let s = avec_blake3::hash_chunk(
                &data[l * 1024..(l + 1) * 1024],
                base + l as u64,
                false,
            );
            assert_eq!(
                four[l], s,
                "4way lane {l} (base={base}) != scalar hash_chunk"
            );
        }
    }
}
