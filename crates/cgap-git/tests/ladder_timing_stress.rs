//! Wave-14 lane 7: ladder timing-stress court (CTQ-04-adjacent).
//!
//! The ladder's claim is RESULT-constancy under CPU load, not constant wall
//! time: 30 sweeps x 10,000 mixed lookups over a 65,536-object idx, with a
//! background spin pool saturating every logical CPU. Per-sweep wall times are
//! recorded and reported honestly (timing-variance under load is
//! load-dependent — NOT a gate); what is gated is that all 30 sweeps return
//! byte-identical lookup results, and that those results match the
//! writer-recorded ground truth (sorted-entry index + offset, provable misses).
//!
//! Chicago discipline: real collaborators throughout — a real idx built by the
//! real `write_idx_v2`, parsed by the real `IdxV2::parse`, ladder lookups on
//! real bytes, real OS threads for the noise. No mocks.
//!
//! Miss targets are provably absent by construction: a deterministic
//! perturbation of a present OID, accepted only after a HashSet membership
//! check against all 65,536 present OIDs.
//!
//! Run: `cargo test -p cgap-git --release --test ladder_timing_stress -- --ignored --nocapture`

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use cgap_git::idx_read::IdxV2;
use cgap_git::pack_write::{write_idx_v2, IdxEntry, Sha1};
use cgap_git::Oid;

const N_OBJECTS: u32 = 65_536; // exactly one full ladder window
const LOOKUPS: usize = 10_000; // per sweep
const SWEEPS: usize = 30;

/// Deterministic u64 LCG (Numerical Recipes constants).
struct Lcg(u64);
impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
}

fn hex_prefix(digest: &[u8; 20]) -> String {
    let mut s = String::with_capacity(8);
    for b in &digest[..4] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ----------------------------------------------------------- CPU noise ----

/// Background CPU-noise spinner pool: one busy thread per logical CPU.
/// Pure arithmetic (xorshift + black_box), no sleeps; polls the stop flag —
/// an observable — with a bounded check every 4096 iterations.
struct SpinNoise {
    stop: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
}

impl SpinNoise {
    fn start() -> Self {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let stop = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::with_capacity(cpus);
        for _ in 0..cpus {
            let stop = Arc::clone(&stop);
            handles.push(std::thread::spawn(move || {
                let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
                while !stop.load(Ordering::Relaxed) {
                    for _ in 0..4096 {
                        x ^= x << 13;
                        x ^= x >> 7;
                        x ^= x << 17;
                        std::hint::black_box(x);
                    }
                }
                std::hint::black_box(x);
            }));
        }
        SpinNoise { stop, handles }
    }
}

impl Drop for SpinNoise {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for h in self.handles.drain(..) {
            h.join().expect("spinner thread must join");
        }
    }
}

// ------------------------------------------------------------- fixture ----

/// One lookup probe with its writer-side ground truth.
struct Probe {
    oid: Oid,
    expect_hit: bool,
    expect_index: u32,
    expect_offset: u64,
}

/// Build the deterministic 65,536-entry fixture and parse it back.
/// `entries` is returned sorted (write_idx_v2 sorts in place), so sorted
/// position p carries ground truth (p, entries[p].offset).
fn build_fixture() -> (Vec<u8>, Vec<IdxEntry>) {
    let mut entries: Vec<IdxEntry> = Vec::with_capacity(N_OBJECTS as usize);
    for i in 0..N_OBJECTS {
        // Distinct OIDs by construction: SHA-1 over a unique 8-byte index tag.
        let mut h = Sha1::new();
        h.update(&i.to_be_bytes());
        entries.push(IdxEntry {
            oid: h.finalize(),
            offset: i * 4,
            crc: i.wrapping_mul(0x9E37_79B9),
        });
    }
    let zero_pack_sum: Oid = [0u8; 20];
    let idx_bytes =
        write_idx_v2(&mut entries, &zero_pack_sum).expect("synthetic idx must build");
    assert_eq!(entries.len(), N_OBJECTS as usize);
    (idx_bytes, entries)
}

/// The 10,000 fixed probes: even i = hit (distinct positions, stride 7),
/// odd i = provable in-partition miss (perturbed OID absent from the
/// present-set; same first byte as its base OID).
fn build_probes(entries: &[IdxEntry]) -> Vec<Probe> {
    let present: HashSet<Oid> = entries.iter().map(|e| e.oid).collect();
    let mut rng = Lcg(0x57A7_1C);
    let mut probes = Vec::with_capacity(LOOKUPS);
    for i in 0..LOOKUPS {
        if i % 2 == 0 {
            // 5,000 hits over distinct sorted positions (7 coprime with 2^16).
            let pos = ((i / 2) as u64 * 7 % N_OBJECTS as u64) as usize;
            probes.push(Probe {
                oid: entries[pos].oid,
                expect_hit: true,
                expect_index: pos as u32,
                expect_offset: entries[pos].offset as u64,
            });
        } else {
            // 5,000 misses: perturb a present OID deterministically until the
            // result is absent from the present-set (provable, checked).
            let pos = (rng.next_u64() % N_OBJECTS as u64) as usize;
            let mut cand = entries[pos].oid;
            let mut k: u8 = 0;
            loop {
                k = k.wrapping_add(1);
                cand[19] = entries[pos].oid[19] ^ k;
                if k == 0 {
                    cand[18] = cand[18].wrapping_add(1); // 256 variants exhausted
                }
                if !present.contains(&cand) {
                    break;
                }
            }
            probes.push(Probe {
                oid: cand,
                expect_hit: false,
                expect_index: 0,
                expect_offset: 0,
            });
        }
    }
    probes
}

/// Encode one lookup result: 13 bytes — tag(1) + index(u32 BE) + offset(u64 BE);
/// misses encode as tag 0 with zero index/offset. Byte-identical vectors are
/// the determinism claim.
fn encode(r: Option<(u32, u64)>, out: &mut Vec<u8>) {
    match r {
        Some((i, o)) => {
            out.push(1);
            out.extend_from_slice(&i.to_be_bytes());
            out.extend_from_slice(&o.to_be_bytes());
        }
        None => out.extend_from_slice(&[0u8; 13]),
    }
}

#[test]
#[ignore]
fn ladder_result_constancy_under_cpu_noise() {
    println!(
        "\n=== ladder timing-stress court: {} sweeps x {} lookups, {}-object idx ===",
        SWEEPS, LOOKUPS, N_OBJECTS
    );
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!(
        "claim under test: RESULT-constancy (byte-identical lookups across sweeps); \
         timing is recorded honestly, NOT gated (load-dependent)"
    );

    let (idx_bytes, entries) = build_fixture();
    let idx = IdxV2::parse(&idx_bytes).expect("synthetic idx must parse");
    assert_eq!(idx.len(), N_OBJECTS);
    idx.verify_idx_checksum()
        .expect("fixture idx checksum must verify");

    let probes = build_probes(&entries);
    let hits = probes.iter().filter(|p| p.expect_hit).count();
    let misses = probes.iter().filter(|p| !p.expect_hit).count();
    assert_eq!(hits, LOOKUPS / 2, "mixed-probe construction: hits");
    assert_eq!(misses, LOOKUPS / 2, "mixed-probe construction: misses");

    // Ground-truth result encoding from the writer side (independent of the
    // reader under test).
    let mut expected = Vec::with_capacity(LOOKUPS * 13);
    for p in &probes {
        encode(
            if p.expect_hit {
                Some((p.expect_index, p.expect_offset))
            } else {
                None
            },
            &mut expected,
        );
    }

    // ---- the stress window: spin pool up, 30 sweeps, pool down ----
    println!(
        "CPU-noise: spinner pool saturating {} logical CPUs for the duration",
        cpus
    );
    let mut times_ms: Vec<f64> = Vec::with_capacity(SWEEPS);
    let mut digests: Vec<String> = Vec::with_capacity(SWEEPS);
    let mut result_bytes: Vec<Vec<u8>> = Vec::with_capacity(SWEEPS);
    let noise = SpinNoise::start();
    for sweep in 0..SWEEPS {
        let mut got = Vec::with_capacity(LOOKUPS * 13);
        let t0 = Instant::now();
        for p in &probes {
            encode(idx.lookup(&p.oid).expect("lookup must stay in-window"), &mut got);
        }
        let dt = t0.elapsed().as_secs_f64();
        times_ms.push(dt * 1_000.0);

        // RESULT-constancy gate 1: byte-identical to the previous sweep
        // (chained: sweep N == sweep N-1 == ... == sweep 1).
        if let Some(prev) = result_bytes.last() {
            if *prev != got {
                let diff = got
                    .iter()
                    .zip(prev.iter())
                    .position(|(a, b)| a != b)
                    .unwrap_or(prev.len().min(got.len()));
                let end = (diff / 13 * 13 + 13).min(got.len());
                panic!(
                    "RESULT-DIVERGENCE at sweep {} byte {diff} (probe {}): got {:?}, previous sweep had {:?}",
                    sweep + 1,
                    diff / 13,
                    &got[diff..end],
                    &prev[diff..end.min(prev.len())],
                );
            }
        }
        // RESULT-constancy gate 2: matches writer-side ground truth.
        assert_eq!(
            got, expected,
            "sweep {} results diverge from writer-recorded ground truth",
            sweep + 1
        );

        let mut h = Sha1::new();
        h.update(&got);
        digests.push(hex_prefix(&h.finalize()));
        result_bytes.push(got);
    }
    drop(noise); // spin pool down before reporting

    // ---- the 30-sweep table ----
    println!("\n sweep |   time ms | lookups/s | digest   | result");
    println!("-------+-----------+-----------+----------+--------");
    for s in 0..SWEEPS {
        let rate = LOOKUPS as f64 / (times_ms[s] / 1_000.0);
        println!(
            " {:>5} | {:>9.3} | {:>9.0} | {} | identical",
            s + 1,
            times_ms[s],
            rate,
            digests[s]
        );
    }
    let min = times_ms.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = times_ms
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    let mean = times_ms.iter().sum::<f64>() / SWEEPS as f64;
    println!("\nTIMING SPREAD (honest, load-dependent — NOT a gate):");
    println!("  min             = {:>10.3} ms", min);
    println!("  max             = {:>10.3} ms", max);
    println!("  mean            = {:>10.3} ms", mean);
    println!("  spread max-min  = {:>10.3} ms", max - min);
    println!("  max/min ratio   = {:>10.2}x", max / min);
    println!("\nRESULT-CONSTANCY (the claim, gated):");
    println!(
        "  {}/{} sweeps byte-identical to sweep 1 AND byte-identical to writer ground truth",
        SWEEPS, SWEEPS
    );
    println!(
        "  fixture: {}-object idx, {} B; probes: {} hits + {} provable misses per sweep",
        N_OBJECTS,
        idx_bytes.len(),
        hits,
        misses
    );
}
