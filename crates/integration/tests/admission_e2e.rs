//! End-to-end admission pipeline (PRD workflow 1, specs 23/24/25/26).
//!
//! Real collaborators, Chicago-style: a batch of real `Quad64` records
//! (spec 23 layout from cgap-arrow), a real BLAKE3 receipt over the batch
//! bytes (spec 26, R = blake3(batch_bytes)), real level-free parquet pages
//! through `cgap_parquet::level_free::write_page`/`read_page_payload`
//! (spec 24), asserted byte-identical after the full round trip with
//! receipt equality, and the real cgap-git packfile wrap of those pages
//! (spec 25: `PackBuilder` -> pack+idx, parse-back byte identity, plus a
//! real `git fsck` court over the emitted pack).
//!
//! Stage degradation is loud: any missing collaborator prints
//! `[SKIP stage: reason]` and the remaining stages are still asserted.
//! No mocks anywhere.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cgap_arrow::layout::{Quad64, QUAD_SIZE};
use cgap_blake3::hash;
use cgap_parquet::level_free::{read_page_payload, write_page, LevelFreeType};
use cgap_git::pack_write::{encode_object_header, git_object_oid, parse_idx_v2, parse_pack_v2};
use cgap_git::PackBuilder;

/// Deterministic pseudo-random quad stream (xorshift64*), so every run of
/// the test hashes the same batch and the receipt is reproducible.
struct Xorshift(u64);

impl Xorshift {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    fn next_u64(&mut self) -> u64 {
        let lo = self.next_u32() as u64;
        let hi = self.next_u32() as u64;
        (hi << 32) | lo
    }
}

/// Build `n` deterministic quads with distinct inline digests.
fn make_quads(n: usize) -> Vec<Quad64> {
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|i| Quad64 {
            s_id: rng.next_u32(),
            p_id: rng.next_u32(),
            o_id: rng.next_u32(),
            g_id: rng.next_u32(),
            t_ns: rng.next_u64(),
            vector_clock: rng.next_u32(),
            policy_mask: rng.next_u32(),
            inline_digest: {
                let mut d = [0u8; 32];
                d[..8].copy_from_slice(&i.to_le_bytes());
                d[8..16].copy_from_slice(&rng.next_u64().to_le_bytes());
                d
            },
        })
        .collect()
}

/// Serialize the batch to contiguous little-endian bytes — the canonical
/// batch_bytes over which the receipt is computed.
fn batch_bytes(quads: &[Quad64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(quads.len() * QUAD_SIZE);
    for q in quads {
        bytes.extend_from_slice(&q.to_le_bytes());
    }
    bytes
}

/// Column projections of one quad batch: (name, type, payload extractor).
struct Column {
    name: &'static str,
    ty: LevelFreeType,
    payload: Vec<u8>,
}

/// Pre-allocated column set whose payloads are reused across refill
/// iterations (capacity is allocated once; `clear()` keeps the buffer).
struct ColumnSet {
    cols: Vec<Column>,
}

impl ColumnSet {
    fn new(n: usize) -> Self {
        let c = |name: &'static str, ty, width: usize| Column {
            name,
            ty,
            payload: Vec::with_capacity(n * width),
        };
        Self {
            cols: vec![
                c("s_id", LevelFreeType::U32, 4),
                c("p_id", LevelFreeType::U32, 4),
                c("o_id", LevelFreeType::U32, 4),
                c("g_id", LevelFreeType::U32, 4),
                c("t_ns", LevelFreeType::U64, 8),
                c("vector_clock", LevelFreeType::U32, 4),
                c("policy_mask", LevelFreeType::U32, 4),
                c("inline_digest", LevelFreeType::Bytes32, 32),
            ],
        }
    }

    fn refill(&mut self, quads: &[Quad64]) {
        for c in &mut self.cols {
            c.payload.clear();
        }
        for q in quads {
            self.cols[0].payload.extend_from_slice(&q.s_id.to_le_bytes());
            self.cols[1].payload.extend_from_slice(&q.p_id.to_le_bytes());
            self.cols[2].payload.extend_from_slice(&q.o_id.to_le_bytes());
            self.cols[3].payload.extend_from_slice(&q.g_id.to_le_bytes());
            self.cols[4].payload.extend_from_slice(&q.t_ns.to_le_bytes());
            self.cols[5].payload.extend_from_slice(&q.vector_clock.to_le_bytes());
            self.cols[6].payload.extend_from_slice(&q.policy_mask.to_le_bytes());
            self.cols[7].payload.extend_from_slice(&q.inline_digest);
        }
    }
}

fn project_columns(quads: &[Quad64]) -> Vec<Column> {
    let mut set = ColumnSet::new(quads.len());
    set.refill(quads);
    set.cols
}

// ------------------------------------------------- git fsck court helpers --
// fsck_court.rs pattern: real git subprocess, real bytes on disk, final-state
// asserts, loud refusal when git is unavailable. No mocks.

/// Fail loudly if git is unavailable — never fake a pass.
fn require_git() -> String {
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => panic!(
            "SKIP-REFUSAL: git binary exists but `git --version` failed (exit {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => panic!(
            "SKIP-REFUSAL: git binary not runnable: {e}. Court cannot run without real git."
        ),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

struct TempGitRepo {
    root: PathBuf,
}

impl TempGitRepo {
    /// `git init -q` a fresh repo in a unique temp dir; removed on Drop.
    fn new(tag: &str) -> TempGitRepo {
        let root = std::env::temp_dir().join(format!(
            "cgap-integration-admission-e2e-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        run_git(&root, &["init", "-q"], "git init");
        TempGitRepo { root }
    }
}

impl Drop for TempGitRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct GitOut {
    ok: bool,
    /// Raw stdout bytes — page content is arbitrary binary, never lossy-UTF8 it.
    stdout_bytes: Vec<u8>,
    stderr: String,
}

fn run_git(dir: &Path, args: &[&str], what: &str) -> GitOut {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "court")
        .env("GIT_AUTHOR_EMAIL", "court@invalid")
        .env("GIT_COMMITTER_NAME", "court")
        .env("GIT_COMMITTER_EMAIL", "court@invalid")
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot spawn git: {e}"));
    GitOut {
        ok: out.status.success(),
        stdout_bytes: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

/// Install pack+idx under git's canonical `pack-<pack-trailer-sha>.{pack,idx}`
/// naming inside `.git/objects/pack/`.
fn install_pack(repo_root: &Path, pack: &[u8], idx: &[u8]) -> (PathBuf, PathBuf) {
    let name = format!("pack-{}", hex(&pack[pack.len() - 20..]));
    let pack_dir = repo_root.join(".git").join("objects").join("pack");
    fs::create_dir_all(&pack_dir).unwrap();
    let pack_path = pack_dir.join(format!("{name}.pack"));
    let idx_path = pack_dir.join(format!("{name}.idx"));
    fs::write(&pack_path, pack).unwrap();
    fs::write(&idx_path, idx).unwrap();
    (pack_path, idx_path)
}

#[test]
fn admission_e2e_roundtrip_and_receipt() {
    println!("[stage: layout]   cgap-arrow Quad64 layout — LANDED, using it");
    let quads = make_quads(256);
    let bytes = batch_bytes(&quads);
    assert_eq!(bytes.len(), 256 * QUAD_SIZE);

    // Sanity: the layout's own codec is lossless before anything else.
    for (i, chunk) in bytes.chunks_exact(QUAD_SIZE).enumerate() {
        let back = Quad64::from_le_bytes(chunk).expect("64-byte chunk decodes");
        assert_eq!(back, quads[i], "layout codec must be lossless at row {i}");
    }

    // --------------------------------------------------------- spec 26 --
    println!("[stage: blake3]   cgap-blake3 batch receipt — LANDED");
    let receipt: [u8; 32] = hash(&bytes);
    assert_eq!(hash(&bytes), receipt, "BLAKE3 must be deterministic");

    // --------------------------------------------------------- spec 24 --
    println!("[stage: parquet]  cgap-parquet level-free pages — LANDED");
    let pages: Vec<(&'static str, Vec<u8>, LevelFreeType)> = project_columns(&quads)
        .into_iter()
        .map(|c| {
            let page = write_page(c.ty, &c.payload, quads.len())
                .expect("level-free page write");
            (c.name, page, c.ty)
        })
        .collect();
    assert_eq!(pages.len(), 8, "one page per quad column");

    // --------------------------------------------------------- spec 25 --
    println!("[stage: git]     cgap-git packfile wrap of the 8 pages — LANDED");
    let mut builder = PackBuilder::new();
    for (_name, page, _ty) in &pages {
        builder.push(page);
    }
    let (pack, idx) =
        builder.finish().expect("cgap-git PackBuilder finish over the page payloads");
    assert_eq!(builder.len(), 8, "builder carries one blob per page");

    // Parse the idx back (independent reader): one entry per page.
    let (oids, _crcs, _offs) = parse_idx_v2(&idx).expect("cgap-git idx parses");
    assert_eq!(oids.len(), 8, "idx must carry one entry per page");

    // Independent pack parser + inflate: every object must inflate back to
    // its page bytes, byte-identical.
    let parsed = parse_pack_v2(&pack).expect("cgap-git pack parses");
    assert_eq!(parsed.len(), 8, "pack must carry one object per page");
    for (obj, (name, page, _)) in parsed.iter().zip(pages.iter()) {
        assert_eq!(obj.obj_type, 3, "{name} packs as a pack-v2 blob (type 3)");
        assert_eq!(obj.size, page.len() as u64, "{name} header size round-trips");
        let hdr_len = encode_object_header(3, obj.size).len();
        let start = obj.offset as usize + hdr_len;
        let next = parsed
            .iter()
            .filter(|q| q.offset > obj.offset)
            .map(|q| q.offset as usize)
            .min()
            .unwrap_or(pack.len() - 20);
        let back = cgap_git::inflate::zlib_decode(&pack[start..next])
            .unwrap_or_else(|e| panic!("{name}: zlib body does not inflate: {e:?}"));
        assert_eq!(
            back, *page,
            "{name} page must be byte-identical after the pack round trip"
        );
    }

    // Bonus court (fsck_court.rs pattern): hand the emitted pack+idx to real
    // git and let the real binary adjudicate — never a self-graded pass.
    let git_version = require_git();
    eprintln!("[court: git] git: {git_version}");
    let repo = TempGitRepo::new("admission-e2e");
    let (pack_path, idx_path) = install_pack(&repo.root, &pack, &idx);
    eprintln!("[court: git] installed {}", pack_path.display());

    let fsck = run_git(&repo.root, &["fsck", "--full"], "git fsck --full");
    eprintln!(
        "[court: git] git fsck --full: exit-ok={} stderr={:?}",
        fsck.ok, fsck.stderr
    );
    assert!(
        fsck.ok,
        "\n==================== ADMISSION E2E GIT COURT: FAIL ====================\n\
         git {git_version}\n\
         cgap-git pack built from the 8 admission pages ({} bytes) rejected by \
         git fsck --full.\n\
         --- fsck stderr ---\n{}\n\
         =======================================================================",
        pack.len(),
        fsck.stderr
    );

    let vp = run_git(
        &repo.root,
        &["verify-pack", "-v", idx_path.to_str().unwrap()],
        "git verify-pack -v",
    );
    assert!(
        vp.ok,
        "git verify-pack -v rejected the cgap-git pack/idx pair: {}",
        vp.stderr
    );

    // Content parity: the crate-computed blob OID for each page resolves in
    // real git to the exact page bytes.
    for (name, page, _) in &pages {
        let oid_hex = hex(&git_object_oid("blob", page));
        let cf = run_git(
            &repo.root,
            &["cat-file", "blob", &oid_hex],
            "git cat-file blob",
        );
        assert!(
            cf.ok,
            "git cat-file blob {oid_hex} ({name}) failed: {}",
            cf.stderr
        );
        assert_eq!(
            cf.stdout_bytes, *page,
            "{name} content parity through real git"
        );
    }
    println!(
        "[court: git] verdict: git {git_version} accepts the pack — fsck/verify-pack/cat-file \
         all exit-ok, 8/8 pages content-identical"
    );

    // ---------------------------------------------------------- readback (functional) --
    // Decode every page (zero-copy PLAIN path) and reassemble the 64-byte
    // records; assert byte-identical round trip + receipt equality.
    let mut u32_cols: Vec<Vec<u8>> = Vec::new();
    let mut t_payload: &[u8] = &[];
    let mut dg_payload: &[u8] = &[];
    for (name, page, ty) in &pages {
        let (payload, rows) = read_page_payload(page, *ty).expect("page read");
        assert_eq!(rows, quads.len(), "{name} must round-trip all rows");
        match *ty {
            LevelFreeType::U32 => u32_cols.push(payload.to_vec()),
            LevelFreeType::U64 => t_payload = payload,
            LevelFreeType::Bytes32 => dg_payload = payload,
        }
    }
    assert_eq!(u32_cols.len(), 6, "six u32 columns expected");

    let mut readback = Vec::with_capacity(bytes.len());
    for i in 0..quads.len() {
        let mut row = [0u8; QUAD_SIZE];
        for (col, off) in u32_cols.iter().zip([0x00usize, 0x04, 0x08, 0x0C, 0x18, 0x1C]) {
            row[off..off + 4].copy_from_slice(&col[i * 4..i * 4 + 4]);
        }
        row[0x10..0x18].copy_from_slice(&t_payload[i * 8..i * 8 + 8]);
        row[0x20..0x40].copy_from_slice(&dg_payload[i * 32..(i + 1) * 32]);
        readback.extend_from_slice(&row);
    }

    assert_eq!(readback, bytes, "round trip must be byte-identical");
    assert_eq!(hash(&readback), receipt, "receipt must survive the pipeline");
}

#[test]
fn admission_e2e_timing_probe_min_of_100() {
    // Informational only — datapoint toward CTQ-05 (≤35 µs). Not a gate: no
    // assertion on the measured value.
    let quads = make_quads(256);
    let bytes = batch_bytes(&quads);

    // Integration-side buffer reuse: columns are allocated once and
    // refilled per iteration (clear() keeps capacity — no per-iteration
    // Vec churn on the projection side). Page buffers themselves are
    // allocated inside `write_page` (crate-owned); noted as a cross-crate
    // bottleneck if the stage dominates.
    let mut cols = ColumnSet::new(quads.len());

    // Warmup: one untimed refill so capacity is in place before the clock.
    cols.refill(&quads);

    const N: usize = 2_000;
    let mut best_total = u128::MAX;
    // Per-stage minima, tracked alongside the total so the breakdown shows
    // where the floor is, not just the aggregate.
    let mut best = [u128::MAX; 4]; // receipt | project | write | read
    for _ in 0..N {
        let mut stage = [0u128; 4];

        let t0 = std::time::Instant::now();
        let receipt = hash(&bytes);
        stage[0] = t0.elapsed().as_nanos();

        let t1 = std::time::Instant::now();
        cols.refill(&quads);
        stage[1] = t1.elapsed().as_nanos();

        let t2 = std::time::Instant::now();
        let mut pages: Vec<Vec<u8>> = Vec::with_capacity(8);
        for c in &cols.cols {
            pages.push(write_page(c.ty, &c.payload, quads.len()).unwrap());
        }
        stage[2] = t2.elapsed().as_nanos();

        let t3 = std::time::Instant::now();
        for (c, page) in cols.cols.iter().zip(&pages) {
            let (payload, rows) = read_page_payload(page, c.ty).unwrap();
            assert_eq!(rows, quads.len());
            std::hint::black_box(payload);
        }
        stage[3] = t3.elapsed().as_nanos();

        std::hint::black_box(receipt);
        let total = t0.elapsed().as_nanos();
        if total < best_total {
            best_total = total;
            best = stage;
        }
    }
    let us = |v: u128| v as f64 / 1_000.0;
    // Post wave-5 lane 3 (4-way NEON kernel routing for 4-64-chunk hashes)
    // the receipt stage no longer dominates at ~94%: report the share so the
    // next bottleneck is visible without recomputing by hand.
    let hash_share = 100.0 * us(best[0]) / us(best_total);
    println!(
        "[timing] admission loop (blake3 batch receipt + 8 level-free page \
         writes + reads, 256 quads): min-of-{N} = {:.1} µs \
         (CTQ-05 target ≤35 µs — informational, no gate)",
        us(best_total)
    );
    println!(
        "[timing] stage breakdown at the min iteration: \
         receipt-blake3 {:.2} µs ({hash_share:.1}% of total) | project-refill {:.2} µs \
         | 8x write_page {:.2} µs | 8x read_page_payload {:.2} µs \
         | residual (allocs/timing) {:.2} µs",
        us(best[0]),
        us(best[1]),
        us(best[2]),
        us(best[3]),
        us(best_total) - us(best[0] + best[1] + best[2] + best[3]),
    );
}
