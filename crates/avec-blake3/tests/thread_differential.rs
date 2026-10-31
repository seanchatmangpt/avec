//! Wave-14 adversarial lane 5: multi-thread differential court for the
//! `avec-blake3` tree-hash algebra.
//!
//! Claims under adjudication:
//!
//! 1. **Determinism + no shared mutable state under concurrency.** Hashing the
//!    same 8 MiB corpus on 1/2/4/8 barrier-synchronized `std::thread`s yields
//!    digests byte-identical to the serial `hash` reference and to `b3sum`
//!    (1.8.5, external ground-truth collaborator — real subprocess, no mocks).
//!    *Merge-API note:* the crate exposes NO public multi-chunk subtree merge
//!    (the only public node-level primitives are `hash_chunk` +
//!    `compress_scalar`, which can merge single-chunk children only), so per
//!    the lane contract the ladder hashes the full corpus per thread and
//!    compares digests; the merge itself is exercised through the public API at
//!    single-chunk granularity: 8 threads hash 8 disjoint 1 KiB chunks, the
//!    main thread reduces them pairwise with `compress_scalar` (`PARENT`, final
//!    merge `PARENT | ROOT`) and the result must equal serial `hash`, the
//!    official BLAKE3 8192-byte vector, and `b3sum`.
//! 2. **No cross-contamination under `hash_many_simd` concurrency.** 8
//!    near-identical 64 KiB buffers (pairwise differing at exactly one byte —
//!    worst case for contamination detection) hashed simultaneously from 8
//!    threads must each match their own buffer's precomputed serial digest.
//! 3. **Stability.** The 8-thread court run 20x in-process must return
//!    identical verdicts every iteration. This substitutes for a TSAN pass,
//!    which is unavailable on this target (aarch64-apple-darwin has no TSAN
//!    runtime).
//!
//! Chicago-school: real threads, real shared read-only memory, real subprocess;
//! assertions on final digest state only.

use std::io::Write as _;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};

use avec_blake3::{
    compress_scalar, hash, hash_chunk, hash_many_simd, hash_many_simd_x2, hash_wide_simd, IV,
    PARENT, ROOT,
};

const MIB8: usize = 8 * 1024 * 1024;
const MERGE_CORPUS_LEN: usize = 8192;
const N_THREADS: usize = 8;
const SMALL_BUF_LEN: usize = 64 * 1024;
const STABILITY_ITERS: usize = 20;

/// Official BLAKE3 test vector (BLAKE3-team/test_vectors.json, input len 8192,
/// the repeating `i % 251` byte pattern) — absolute external anchor for the
/// merge-tree court, independent of the crate's own `hash`.
const OFFICIAL_8KIB: &str = "aae792484c8efe4f19e2ca7d371d8c467ffb10748d8a5a1ae579948f718a2a63";

fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// The official-vector byte pattern: `input[i] = i % 251`.
fn fill_pattern_251(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
}

/// Deterministic splitmix64 byte stream — non-repeating 8-byte-granularity
/// fill so the 8 MiB corpus is not a short periodic pattern.
fn fill_splitmix(buf: &mut [u8], seed: u64) {
    let mut z = seed;
    let mut i = 0;
    while i < buf.len() {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut w = z;
        w = (w ^ (w >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        w = (w ^ (w >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        w ^= w >> 31;
        let src = w.to_le_bytes();
        let n = (buf.len() - i).min(8);
        buf[i..i + n].copy_from_slice(&src[..n]);
        i += n;
    }
}

/// Runs the real `b3sum` binary over `bytes` via stdin and returns the digest
/// hex. External ground-truth collaborator: if this binary cannot run the
/// court fails loudly rather than degrading to self-comparison.
fn b3sum(bytes: &[u8]) -> String {
    let mut child = Command::new("b3sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn b3sum (external ground-truth collaborator; must be on PATH)");
    child
        .stdin
        .take()
        .expect("b3sum stdin piped")
        .write_all(bytes)
        .expect("write corpus to b3sum stdin");
    let out = child.wait_with_output().expect("wait for b3sum");
    assert!(
        out.status.success(),
        "b3sum exited non-zero: {:?}",
        out.status
    );
    let text = String::from_utf8(out.stdout).expect("b3sum stdout is utf-8");
    let line = text
        .split_whitespace()
        .next()
        .expect("b3sum prints '<hex>  <name>'");
    assert_eq!(line.len(), 64, "b3sum digest hex must be 64 chars: {line:?}");
    line.to_ascii_lowercase()
}

/// All fixtures plus the serial reference digests every court iteration is
/// adjudicated against. References are computed ONCE, serially, before any
/// concurrent work, so a concurrent result is never compared against another
/// concurrently produced value.
struct Court {
    corpus: Vec<u8>,
    merge_corpus: [u8; MERGE_CORPUS_LEN],
    /// 8 buffers pairwise differing at exactly one byte (byte `t` of buffer
    /// `t` is flipped) — maximally similar inputs, maximally distinct digests.
    buffers: Vec<Vec<u8>>,
    corpus_ref: [u8; 32],
    merge_ref: [u8; 32],
    expected: Vec<[u8; 32]>,
}

impl Court {
    fn new() -> Self {
        let mut corpus = vec![0u8; MIB8];
        fill_splitmix(&mut corpus, 0xB3B3_B3B3_B3B3_B301);

        let mut merge_corpus = [0u8; MERGE_CORPUS_LEN];
        fill_pattern_251(&mut merge_corpus);

        let mut buffers: Vec<Vec<u8>> = (0..N_THREADS)
            .map(|_| {
                let mut b = vec![0u8; SMALL_BUF_LEN];
                fill_splitmix(&mut b, 0xC0FF_EED0_0000_0001);
                b
            })
            .collect();
        for (t, b) in buffers.iter_mut().enumerate() {
            b[t] ^= 0xFF;
        }

        let corpus_ref = hash(&corpus);
        let merge_ref = hash(&merge_corpus);
        let expected: Vec<[u8; 32]> = buffers
            .iter()
            .map(|b| {
                let d_simd = hash_many_simd(b);
                // Anchor every expected digest to BOTH public implementations.
                assert_eq!(hash(b), d_simd, "serial hash vs hash_many_simd on buffer");
                d_simd
            })
            .collect();

        // Distinctness carries the bits in court (c): if the expected digests
        // were not pairwise distinct, a fully-contaminated (single-value) run
        // would pass vacuously.
        for i in 0..N_THREADS {
            for j in (i + 1)..N_THREADS {
                assert_ne!(
                    expected[i], expected[j],
                    "fixture bug: expected digests {i},{j} identical — contamination court would be vacuous"
                );
            }
        }

        Self {
            corpus,
            merge_corpus,
            buffers,
            corpus_ref,
            merge_ref,
            expected,
        }
    }
}

/// Court (b): 8 threads hash 8 disjoint 1 KiB chunks concurrently (barrier
/// start), the main thread reduces the CVs pairwise through the PUBLIC
/// `compress_scalar` parent merge. Exercises the associativity of the tree
/// algebra through public API at the only granularity where a merge is
/// publicly constructible.
fn merge_tree_from_threads(corpus: &[u8]) -> [u8; 32] {
    assert_eq!(corpus.len() % 1024, 0);
    let n = corpus.len() / 1024;
    assert_eq!(n, N_THREADS, "merge court sized for 8 disjoint chunks");

    let mut level: Vec<[u32; 8]> = std::thread::scope(|s| {
        let barrier = Arc::new(Barrier::new(n));
        let handles: Vec<_> = (0..n)
            .map(|i| {
                let barrier = Arc::clone(&barrier);
                s.spawn(move || {
                    barrier.wait();
                    let out = hash_chunk(&corpus[i * 1024..(i + 1) * 1024], i as u64, false);
                    let mut cv = [0u32; 8];
                    cv.copy_from_slice(&out[0..8]);
                    cv
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("merge leaf thread panicked"))
            .collect()
    });

    // Serial sanity: each thread's leaf CV must equal the serial computation.
    for (i, cv) in level.iter().enumerate() {
        let out = hash_chunk(&corpus[i * 1024..(i + 1) * 1024], i as u64, false);
        assert_eq!(&out[0..8], &cv[..], "leaf CV {i} diverged from serial hash_chunk");
    }

    // In-order pairwise reduction; the single merge at the final level is the
    // root and must carry PARENT | ROOT.
    while level.len() > 1 {
        assert!(
            level.len() % 2 == 0,
            "merge court requires a power-of-two chunk count"
        );
        let root_now = level.len() == 2;
        let mut next = Vec::with_capacity(level.len() / 2);
        for pair in level.chunks(2) {
            let mut block = [0u32; 16];
            block[..8].copy_from_slice(&pair[0]);
            block[8..].copy_from_slice(&pair[1]);
            let flags = if root_now { PARENT | ROOT } else { PARENT };
            let out = compress_scalar(&IV, &block, 0, 64, flags);
            let mut cv = [0u32; 8];
            cv.copy_from_slice(&out[0..8]);
            next.push(cv);
        }
        level = next;
    }

    let mut digest = [0u8; 32];
    for (i, w) in level[0].iter().enumerate() {
        digest[i * 4..(i + 1) * 4].copy_from_slice(&w.to_le_bytes());
    }
    digest
}

/// One full 8-thread court pass: (a) corpus hashing on 8 barrier-started
/// threads through both public entry points, (b) the public-API disjoint merge
/// tree, (c) concurrent `hash_many_simd` over the 8 near-identical buffers.
fn run_court_once(court: &Court, iter: usize) {
    // (a) 8 threads x full 8 MiB corpus, `hash` + `hash_many_simd`.
    std::thread::scope(|s| {
        let barrier = Arc::new(Barrier::new(N_THREADS));
        let handles: Vec<_> = (0..N_THREADS)
            .map(|t| {
                let barrier = Arc::clone(&barrier);
                let corpus = &court.corpus;
                s.spawn(move || {
                    barrier.wait();
                    (t, hash(corpus), hash_many_simd(corpus))
                })
            })
            .collect();
        for h in handles {
            let (t, d_seq, d_simd) = h.join().expect("corpus thread panicked");
            assert_eq!(
                d_seq, court.corpus_ref,
                "iter {iter} thread {t}: concurrent hash(8 MiB) diverged from serial reference"
            );
            assert_eq!(
                d_simd, court.corpus_ref,
                "iter {iter} thread {t}: concurrent hash_many_simd(8 MiB) diverged from serial reference"
            );
        }
    });

    // (b) public-API merge tree from 8 disjoint chunks hashed on 8 threads.
    let merged = merge_tree_from_threads(&court.merge_corpus);
    assert_eq!(
        merged, court.merge_ref,
        "iter {iter}: public-API 8-thread disjoint merge diverged from serial hash"
    );

    // (c) concurrent hash_many_simd on 8 distinct near-identical buffers.
    std::thread::scope(|s| {
        let barrier = Arc::new(Barrier::new(N_THREADS));
        let buffers = &court.buffers;
        let handles: Vec<_> = (0..N_THREADS)
            .map(|t| {
                let barrier = Arc::clone(&barrier);
                s.spawn(move || {
                    barrier.wait();
                    (t, hash_many_simd(&buffers[t]))
                })
            })
            .collect();
        for h in handles {
            let (t, d) = h.join().expect("buffer thread panicked");
            assert_eq!(
                d, court.expected[t],
                "iter {iter} thread {t}: concurrent hash_many_simd cross-contaminated — digest does not match its own buffer's serial digest"
            );
        }
    });
}

/// Court 1: the 1/2/4/8 thread ladder over the 8 MiB corpus. Every digest from
/// every thread must be byte-identical to the serial `hash` reference, to all
/// other public implementations (serial differential), and to `b3sum`.
#[test]
fn lane5_thread_ladder_8mib_differential() {
    let court = Court::new();

    // Serial cross-implementation differential: all four public entry points
    // must agree on the 8 MiB corpus before any concurrency is admitted.
    assert_eq!(
        hash_wide_simd(&court.corpus),
        court.corpus_ref,
        "serial hash_wide_simd diverges from hash on 8 MiB"
    );
    assert_eq!(
        hash_many_simd(&court.corpus),
        court.corpus_ref,
        "serial hash_many_simd diverges from hash on 8 MiB"
    );
    assert_eq!(
        hash_many_simd_x2(&court.corpus),
        court.corpus_ref,
        "serial hash_many_simd_x2 diverges from hash on 8 MiB"
    );

    // External ground truth on the 8 MiB corpus.
    assert_eq!(
        b3sum(&court.corpus),
        hex(&court.corpus_ref),
        "b3sum diverges from hash on the 8 MiB corpus"
    );

    // Thread-count ladder. No public multi-chunk merge exists (see module
    // docs), so each thread hashes the full corpus — the fallback the lane
    // contract prescribes. Barrier start forces maximal overlap.
    for &n in &[1usize, 2, 4, 8] {
        std::thread::scope(|s| {
            let barrier = Arc::new(Barrier::new(n));
            let corpus = &court.corpus;
            let handles: Vec<_> = (0..n)
                .map(|t| {
                    let barrier = Arc::clone(&barrier);
                    s.spawn(move || {
                        barrier.wait();
                        (t, hash(corpus), hash_many_simd(corpus))
                    })
                })
                .collect();
            for h in handles {
                let (t, d_seq, d_simd) = h.join().expect("ladder thread panicked");
                assert_eq!(
                    d_seq, court.corpus_ref,
                    "ladder n={n} thread {t}: hash digest diverged from serial reference"
                );
                assert_eq!(
                    d_simd, court.corpus_ref,
                    "ladder n={n} thread {t}: hash_many_simd digest diverged from serial reference"
                );
            }
        });
    }

    // The merge court carries its own external anchors: official vector + b3sum.
    let merged = merge_tree_from_threads(&court.merge_corpus);
    assert_eq!(
        merged, court.merge_ref,
        "public-API 8-thread disjoint merge diverged from serial hash"
    );
    assert_eq!(
        hex(&merged),
        OFFICIAL_8KIB,
        "public-API merge diverges from the official BLAKE3 8192-byte vector"
    );
    assert_eq!(
        b3sum(&court.merge_corpus),
        hex(&merged),
        "b3sum diverges from the public-API merged digest on the 8 KiB corpus"
    );
}

/// Court 2: concurrent `hash_many_simd` on distinct buffers from 8 threads.
/// Each thread's digest must match its own buffer's precomputed serial digest;
/// the pre-asserted pairwise distinctness of the expected digests makes a
/// fully-contaminated (single-value) outcome double-detected.
#[test]
fn lane5_concurrent_hash_many_simd_no_cross_contamination() {
    let court = Court::new();
    std::thread::scope(|s| {
        let barrier = Arc::new(Barrier::new(N_THREADS));
        let buffers = &court.buffers;
        let handles: Vec<_> = (0..N_THREADS)
            .map(|t| {
                let barrier = Arc::clone(&barrier);
                s.spawn(move || {
                    barrier.wait();
                    (t, hash_many_simd(&buffers[t]))
                })
            })
            .collect();
        for h in handles {
            let (t, d) = h.join().expect("buffer thread panicked");
            assert_eq!(
                d, court.expected[t],
                "thread {t}: concurrent hash_many_simd cross-contaminated — digest does not match its own buffer's serial digest"
            );
        }
    });
}

/// Court 3: the 8-thread court run 20x in-process, asserting stability.
/// Substitutes for TSAN (unavailable on aarch64-apple-darwin): any data race
/// in the public API that can perturb a digest must perturb at least one of
/// 20 x (8 threads x 2 entry points x 8 MiB + merge + 8 buffers) = 20 x ~132
/// MiB of concurrent hashing to escape detection.
#[test]
fn lane5_stability_8_thread_court_20x() {
    let court = Court::new();
    for iter in 0..STABILITY_ITERS {
        run_court_once(&court, iter);
    }
}
