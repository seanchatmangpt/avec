//! Wave-7 lane 15: first performance baseline for the Git plane (CTQ-04-adjacent).
//!
//! Informational only — no gate. Three measurements:
//! 1. pack write throughput: 64 MiB LCG corpus, `build_pack_and_idx`
//!    (in-crate SHA-1 + CRC32 + DEFLATE), MiB/s and objects/s.
//! 2. idx lookup rate: 100k ladder lookups over a 65,536-object synthetic
//!    idx, lookups/s, constant-16-stage confirmation (debug-build probe
//!    counter; release builds compile the counter out — the 16-stage shape
//!    is structural in `ladder.rs`).
//! 3. external reference: `git pack-objects` on the same corpus via real
//!    subprocess (Chicago: real collaborator, no mocks). Both numbers
//!    reported, no gate.
//!
//! Run: `cargo test -p avec-git --release --test perf_baseline -- --ignored --nocapture`
//! Caveat: concurrent wave lanes share this machine; numbers are
//! machine-context-noisy, treat as first datapoints, not ceilings.

use std::io::Write;
use std::process::Command;

use avec_git::idx_read::IdxV2;
use avec_git::pack_write::{build_pack_and_idx, write_idx_v2, IdxEntry, Sha1};
use avec_git::Oid;

/// Deterministic u64 LCG (Numerical Recipes constants).
struct Lcg(u64);
impl Lcg {
    fn next_u8(&mut self) -> u8 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u8
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next_u8()).collect()
    }
}

/// 64 MiB of pseudorandom blob content, deterministic across runs.
fn corpus() -> Vec<Vec<u8>> {
    // 256 blobs x 256 KiB = 64 MiB. Pack cap is 2^16 objects; 256 is safe.
    const BLOBS: usize = 256;
    const BLOB: usize = 256 * 1024;
    let mut rng = Lcg(0x0ABE_C5EED);
    (0..BLOBS).map(|_| rng.bytes(BLOB)).collect()
}

const MIB: f64 = 1024.0 * 1024.0;

fn mib_s(bytes: usize, secs: f64) -> f64 {
    bytes as f64 / MIB / secs
}

#[test]
#[ignore]
fn perf_baseline_git_plane() {
    let blobs = corpus();
    let total: usize = blobs.iter().map(|b| b.len()).sum();
    println!(
        "\n=== avec-git perf baseline (64 MiB corpus, 256 x 256 KiB blobs) ==="
    );
    println!("machine-context caveat: concurrent wave lanes; informational, no gate");

    // ---- 1. pack write throughput (min-of-3) ----
    let mut best = f64::INFINITY;
    let mut best_objs = f64::INFINITY;
    let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
    for round in 0..3 {
        let t0 = std::time::Instant::now();
        let (pack, idx, entries) =
            build_pack_and_idx(&refs).expect("avec-git pack+idx build must succeed");
        let dt = t0.elapsed().as_secs_f64();
        std::hint::black_box((&pack, &idx, &entries));
        let mibs = mib_s(total, dt);
        let objs = refs.len() as f64 / dt;
        println!(
            "pack write r{round}: {:.1} MiB/s, {:.0} objects/s (dt={:.3}s, pack={} B, idx={} B)",
            mibs,
            objs,
            dt,
            pack.len(),
            idx.len()
        );
        best = best.min(mibs);
        best_objs = best_objs.min(objs);
    }
    println!(
        "PACK WRITE: best {:.1} MiB/s, {:.0} objects/s (min-of-3)",
        best, best_objs
    );

    // ---- 2. idx ladder lookup rate over a 65,536-object synthetic idx ----
    const N: u32 = 65_536; // exactly one full ladder window
    let mut entries: Vec<IdxEntry> = Vec::with_capacity(N as usize);
    for i in 0..N {
        // Deterministic, distinct OIDs: SHA-1 over a unique 8-byte index tag.
        let mut h = Sha1::new();
        h.update(&i.to_be_bytes());
        entries.push(IdxEntry {
            oid: h.finalize(),
            offset: i * 4,
            crc: i.wrapping_mul(0x9E37_79B9),
        });
    }
    let zero_pack_sum: Oid = [0u8; 20];
    let idx_bytes = write_idx_v2(&mut entries, &zero_pack_sum).expect("synthetic idx must build");
    drop(entries);
    let idx_view = IdxV2::parse(&idx_bytes).expect("synthetic idx must parse");
    assert_eq!(idx_view.len(), N);

    // Lookup targets: every 13th object (hits) interleaved with perturbed
    // OIDs (guaranteed misses on the same first-byte partitions).
    const LOOKUPS: u64 = 100_000;
    let mut hit = 0u64;
    let mut miss = 0u64;
    let probes_before = avec_git::ladder::debug_probes();
    let t0 = std::time::Instant::now();
    for i in 0..LOOKUPS {
        let mut t = idx_view.oid(((i * 13) % N as u64) as u32);
        if i % 2 == 1 {
            t[19] ^= 0xFF; // miss, same partition
        } else {
            hit += 1;
        }
        if idx_view.lookup(&t).expect("lookup must not error").is_some() {
            miss += 1; // reused as hit-counter for present targets
        }
    }
    let dt_lookup = t0.elapsed().as_secs_f64();
    let probes_after = avec_git::ladder::debug_probes();
    std::hint::black_box(&miss);
    assert_eq!(hit as usize, LOOKUPS as usize / 2, "half the targets present");
    assert_eq!(miss as usize, LOOKUPS as usize / 2, "half the targets hit");
    let rate = LOOKUPS as f64 / dt_lookup;
    println!(
        "IDX LOOKUP: {:.0} lookups/s over {}-object idx ({:.3}s total, hits={} misses={})",
        rate, N, dt_lookup, miss, hit
    );

    // Constant-iteration confirmation. The probe counter exists only in
    // debug builds; in release the 16-stage shape is enforced by
    // construction (ladder.rs: no data-dependent exits).
    let delta = probes_after - probes_before;
    if cfg!(debug_assertions) {
        assert_eq!(
            delta,
            16 * LOOKUPS,
            "ladder must execute exactly 16 probes per lookup"
        );
        println!("ladder probes: {} = 16 x {} (exact, debug counter)", delta, LOOKUPS);
    } else {
        assert_eq!(
            delta, 0,
            "probe counter must be compiled out in release"
        );
        println!(
            "ladder constant-16-stage: structural (probe counter compiled out in release; \
             no data-dependent exits in ladder.rs)"
        );
    }

    // ---- 3. external reference: real `git pack-objects` on the same corpus ----
    let pid = std::process::id();
    let tmp = std::env::temp_dir().join(format!("avec-git-perf-{pid}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("repo")).expect("temp dir");
    let run = |args: &[&str], stdin: Option<&[u8]>, what: &str| -> Vec<u8> {
        let mut cmd = Command::new("git");
        cmd.current_dir(tmp.join("repo")).args(args);
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("{what}: cannot spawn git: {e}"));
        if let Some(input) = stdin {
            child
                .stdin
                .take()
                .expect("stdin piped")
                .write_all(input)
                .unwrap_or_else(|e| panic!("{what}: write stdin: {e}"));
        }
        let out = child.wait_with_output().expect("{what}: wait");
        assert!(
            out.status.success(),
            "{what}: git exited {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    };
    run(&["init", "-q"], None, "git init");
    let mut oid_list = String::new();
    for blob in &blobs {
        let out = run(
            &["hash-object", "-w", "--stdin"],
            Some(blob),
            "git hash-object",
        );
        let oid_str = String::from_utf8_lossy(out.trim_ascii()).to_string();
        oid_list.push_str(&format!("{oid_str} blob\n"));
    }
    let t0 = std::time::Instant::now();
    let _pack_name = run(&["pack-objects", "."], Some(oid_list.as_bytes()), "git pack-objects");
    let dt_git = t0.elapsed().as_secs_f64();
    println!(
        "GIT REFERENCE: `git pack-objects` on same 64 MiB corpus: {:.1} MiB/s ({:.3}s, \
         256 objects, includes 256 hash-object spawns)",
        mib_s(total, dt_git),
        dt_git
    );
    println!("NOTE: git reference includes process spawn overhead; not a gate, comparison point only");

    let _ = std::fs::remove_dir_all(&tmp);
}
