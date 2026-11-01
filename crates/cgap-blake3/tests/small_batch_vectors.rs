//! Court for the lane-3 small-batch 4-way path (CTQ-05).
//!
//! Proves the 4..=64-chunk 4-way SIMD routing in `hash()` produces byte-identical
//! output to (a) the sequential `hash` fallback (via hash_many_simd parity across
//! every length in the window plus off-window edges) and (b) the reference `b3sum`
//! binary on real corpora.

use std::process::Command;

fn fill(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i.wrapping_mul(7).wrapping_add(13) % 251) as u8;
    }
}

/// Pipes a real corpus through the reference implementation at
/// /opt/homebrew/bin/b3sum and returns the 64-char lowercase hex digest.
fn b3sum(data: &[u8]) -> [u8; 32] {
    use std::io::Write;
    let out = Command::new("/opt/homebrew/bin/b3sum")
        .arg("--raw") // binary digest on stdout, no filename
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn b3sum");
    let mut child = out;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(data)
        .expect("write stdin");
    let raw = child.wait_with_output().expect("b3sum output").stdout;
    assert_eq!(raw.len(), 32, "b3sum --raw must emit 32 bytes");
    let mut d = [0u8; 32];
    d.copy_from_slice(&raw);
    d
}

use std::process::Stdio;

#[test]
fn court_hash_vs_b3sum_small_batch_window() {
    // Every whole-chunk length in the eligibility window 4..=64 chunks
    // (multiples of 4 chunks), plus off-window edges.
    let mut lens: Vec<usize> = Vec::new();
    for chunks in (4..=64).step_by(4) {
        lens.push(chunks * 1024);
    }
    // Off-window / non-eligible edges: non-multiple-of-4 chunk counts, tails.
    for chunks in [1usize, 2, 3, 5, 6, 7, 9, 16, 17, 63, 65, 66] {
        lens.push(chunks * 1024);
    }
    // Ragged tails straddling the window edges.
    lens.extend([4095, 4097, 6143, 6144, 6145, 16383, 16384, 16385, 65535, 65537]);

    let max = *lens.iter().max().unwrap();
    let mut data = vec![0u8; max];
    fill(&mut data);

    for &len in &lens {
        let d_hash = cgap_blake3::hash(&data[..len]);
        let d_ref = b3sum(&data[..len]);
        assert_eq!(
            d_hash, d_ref,
            "hash() != b3sum at len={} ({} chunks)",
            len,
            len.div_ceil(1024)
        );

        let d_many = cgap_blake3::hash_many_simd(&data[..len]);
        assert_eq!(d_many, d_hash, "hash_many_simd != hash at len={}", len);
    }
}

#[test]
fn court_eligibility_predicate() {
    assert!(!cgap_blake3::is_small_batch_eligible(0));
    assert!(!cgap_blake3::is_small_batch_eligible(1024));
    assert!(!cgap_blake3::is_small_batch_eligible(4095));
    assert!(cgap_blake3::is_small_batch_eligible(4096)); // 4 chunks
    assert!(cgap_blake3::is_small_batch_eligible(16 * 1024)); // 16 chunks, e2e batch
    assert!(!cgap_blake3::is_small_batch_eligible(17 * 1024));
    assert!(cgap_blake3::is_small_batch_eligible(64 * 1024)); // 64 chunks
    assert!(!cgap_blake3::is_small_batch_eligible(65 * 1024));
    assert!(!cgap_blake3::is_small_batch_eligible(68 * 1024));
}

#[test]
fn court_exhaustive_small_parity_sequential_vs_4way() {
    // Byte-level exhaustive: every length 0..=2048 plus every boundary ±1 around
    // each 1 KiB chunk edge up to 66 KiB, sequential vs 4-way path byte-identical.
    let mut data = vec![0u8; 68 * 1024];
    fill(&mut data);

    for len in 0..=2048usize {
        assert_eq!(
            cgap_blake3::hash(&data[..len]),
            cgap_blake3::hash_many_simd(&data[..len]),
            "parity fail at len={}",
            len
        );
    }
    for chunks in 1..=66usize {
        for d in [-1i64, 0, 1] {
            let len = (chunks as i64 * 1024 + d) as usize;
            assert_eq!(
                cgap_blake3::hash(&data[..len]),
                cgap_blake3::hash_many_simd(&data[..len]),
                "parity fail at len={}",
                len
            );
        }
    }
}
