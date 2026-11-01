use std::time::Instant;
#[test]
fn bench_tmp() {
    let n = 256 * 1024 * 1024;
    let mut data = vec![0u8; n];
    for i in (0..n).step_by(4096) { data[i] = (i % 251) as u8; }
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = Instant::now();
        let d = cgap_blake3::hash_wide_simd(&data);
        let el = t.elapsed().as_secs_f64();
        best = best.min(el);
        println!("hash_wide_simd {:.3}s {:02x?} {:.3} GiB/s", el, &d[..4], n as f64 / el / 1073741824.0);
    }
    println!("BEST {:.3} GiB/s", n as f64 / best / 1073741824.0);
}
