//! Wave-14 adversarial lane 6: packfile fuzzer vs real git.
//!
//! Two deterministic matrices, one LCG (Numerical Recipes constants), fixed
//! seeds, real git subprocesses, real bytes on disk — no mocks (Chicago).
//!
//! ACCEPT MATRIX (40 cases): each case draws a blob set (count 1..=50, sizes
//! 0 B..=1 MiB, mixed compressibility), writes pack+idx with the cgap-git
//! writer into a fresh `git init` repo, and requires ALL of:
//!   1. `git fsck --full` exit-ok (dangling-blob info lines do not affect the
//!      exit code — packs here are bare blobs, no refs);
//!   2. `git verify-pack -v <idx>` exit-ok;
//!   3. `git cat-file blob <oid>` returns byte-identical content for every
//!      unique blob;
//!   4. the crate's own parser (`parse_pack_v2` + `parse_idx_v2`) agrees with
//!      git on object count and per-object offsets (oid-keyed), sizes, types,
//!      and is internally consistent on CRCs.
//!
//! REJECT MATRIX (20 mutants): 20 fresh VALID packs (git-accepted and
//! crate-parsed clean as a per-mutant precondition), each mutated at exactly
//! one byte — systematically cycling header (0..12), body (12..len-20),
//! trailer (len-20..len) — must be rejected by git AND/OR answered with a
//! typed `GitError` by the crate parser. A panic anywhere (catch_unwind on
//! the parser; the harness otherwise) is the finding.
//!
//! Determinism disclosures (shape choices inside the contract bounds):
//!   - Sizes are LCG-drawn inside 0 B..=1 MiB through four size bands
//!     (tiny/small/medium/large) so per-run volume stays in minutes; every
//!     drawn size is still in the contract range, and ~1/16 of blobs are
//!     forced to exactly 0 B to keep the empty-blob surface covered.
//!   - Mutant-phase blob sizes are capped at 64 KiB (runtime bound; the
//!     mutation mechanics are size-independent — trailer SHA-1 covers all).
//!   - Blob sets are deduped by git OID before packing: git packs dedupe
//!     identical objects, and duplicate OIDs would double-count the same
//!     object in the idx fanout (and fail the crate's strict oid-sorted idx
//!     check) without any real byte disagreement to report.
//!   - Zero external dependencies: the LCG replaces any RNG crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cgap_git::pack_write::{
    build_pack_and_idx, git_object_oid, parse_idx_v2, parse_pack_v2,
};
use cgap_git::GitError;

// ------------------------------------------------------------------- LCG --

/// 64-bit LCG (Numerical Recipes), deterministic, no deps.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Lcg {
        Lcg(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// Uniform-ish draw in [0, bound) (modulo bias irrelevant for fuzz shape).
    fn below(&mut self, bound: u64) -> u64 {
        debug_assert!(bound > 0);
        self.next_u64() % bound
    }

    /// Fill `dest` with LCG pseudo-random bytes.
    fn fill(&mut self, dest: &mut [u8]) {
        for chunk in dest.chunks_mut(8) {
            let x = self.next_u64();
            for (j, b) in chunk.iter_mut().enumerate() {
                *b = (x >> (8 * j)) as u8;
            }
        }
    }
}

const SEED_ACCEPT: u64 = 0x5746_3134_C0FF_EE01;
const SEED_REJECT: u64 = 0x5746_3134_BAD_5EED;

const KIB: usize = 1024;
const MIB: usize = 1024 * 1024;

/// LCG-draw a size in [0, 1 MiB] across four bands (header disclosure:
/// band weighting bounds per-run volume; range is the contract's).
fn draw_size(lcg: &mut Lcg) -> usize {
    if lcg.below(16) == 0 {
        return 0; // empty-blob surface stays covered
    }
    let (lo, hi) = match lcg.below(10) {
        0..=3 => (0, 4 * KIB),                // tiny
        4..=6 => (4 * KIB, 64 * KIB),         // small
        7..=8 => (64 * KIB, 256 * KIB),       // medium
        _ => (256 * KIB, 1 * MIB + 1),        // large
    };
    lo + lcg.below((hi - lo) as u64) as usize
}

/// Synthesize exactly `size` bytes with an LCG-chosen compressibility class:
/// 0 incompressible, 1 repeated-phrase (highly compressible), 2 semi
/// (random spans echoed from the preceding window), 3 constant runs,
/// 4 text-like.
fn synthesize_blob(lcg: &mut Lcg, size: usize) -> Vec<u8> {
    let mut out = vec![0u8; size];
    if size == 0 {
        return out;
    }
    match lcg.below(5) {
        0 => {
            lcg.fill(&mut out);
        }
        1 => {
            // repeat one random phrase to fill
            let plen = (8 + lcg.below(56) as usize).min(size);
            let mut phrase = vec![0u8; plen];
            lcg.fill(&mut phrase);
            for (i, b) in out.iter_mut().enumerate() {
                *b = phrase[i % plen];
            }
        }
        2 => {
            // random span, then echo a span from <= 64 bytes back, repeat
            let mut i = 0usize;
            while i < size {
                let run = (4 + lcg.below(16) as usize).min(size - i);
                lcg.fill(&mut out[i..i + run]);
                i += run;
                if i >= size {
                    break;
                }
                let echo = (16 + lcg.below(112) as usize).min(size - i);
                let src = i.saturating_sub(64);
                let elen = (i + echo).min(size) - i;
                // overlapping copy is intentional (byte-by-byte semantics)
                for k in 0..elen {
                    out[i + k] = out[src + k];
                }
                i += elen;
            }
        }
        3 => {
            // constant runs of 16..4096 bytes, value changes per run
            let mut i = 0usize;
            while i < size {
                let run = (16 + lcg.below(4096) as usize).min(size - i);
                let mut val = [0u8; 1];
                lcg.fill(&mut val);
                for b in &mut out[i..i + run] {
                    *b = val[0];
                }
                i += run;
            }
        }
        _ => {
            const VOCAB: &[&str] = &[
                "pack", "blob", "idx", "zlib", "deflate", "sha1", "crc32",
                "offset", "fanout", "trailer", "ladder", "court", "wave",
                "fuzz", "cgap", "git", "object", "type", "size", "header",
                "varint", "checksum", "corrupt", "mutant",
            ];
            let mut i = 0usize;
            while i < size {
                let word = VOCAB[lcg.below(VOCAB.len() as u64) as usize];
                let take = (word.len() + 1).min(size - i);
                out[i..i + take - 1].copy_from_slice(&word.as_bytes()[..take - 1]);
                out[i + take - 1] = if lcg.below(4) == 0 { b'\n' } else { b' ' };
                i += take;
                if lcg.below(16) == 0 && i < size {
                    out[i] = (lcg.next_u64() >> 32) as u8; // raw-byte noise
                    i += 1;
                }
            }
        }
    }
    out
}

// ----------------------------------------------------------- git harness --

fn require_git() -> String {
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        Ok(out) => panic!(
            "SKIP-REFUSAL: git binary exists but `git --version` failed (exit {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => panic!(
            "SKIP-REFUSAL: git binary not runnable: {e}; fuzz court cannot run without real git"
        ),
    }
}

struct FuzzRepo {
    root: PathBuf,
}

impl FuzzRepo {
    fn new(tag: &str) -> FuzzRepo {
        let root = std::env::temp_dir().join(format!(
            "cgap-git-packfile-fuzz-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q"], "git init");
        FuzzRepo { root }
    }

    fn pack_dir(&self) -> PathBuf {
        self.root.join(".git").join("objects").join("pack")
    }
}

impl Drop for FuzzRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct GitOut {
    status_ok: bool,
    stdout_bytes: Vec<u8>,
    stderr: String,
}

fn git(dir: &Path, args: &[&str], what: &str) -> GitOut {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{what}: cannot spawn git: {e}"));
    GitOut {
        status_ok: out.status.success(),
        stdout_bytes: out.stdout,
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Install pack+idx under git's canonical `pack-<trailer-sha-hex>` naming.
fn install_pack(repo: &FuzzRepo, pack: &[u8], idx: &[u8]) -> (PathBuf, PathBuf) {
    let name = format!("pack-{}", hex(&pack[pack.len() - 20..]));
    fs::create_dir_all(repo.pack_dir()).unwrap();
    let pp = repo.pack_dir().join(format!("{name}.pack"));
    let ip = repo.pack_dir().join(format!("{name}.idx"));
    fs::write(&pp, pack).unwrap();
    fs::write(&ip, idx).unwrap();
    (pp, ip)
}

/// One per-object record parsed from `git verify-pack -v` stdout.
#[derive(Debug, Clone)]
struct GitPackObject {
    oid: String,
    kind: String,
    size: u64,
    #[allow(dead_code)]
    size_in_pack: u64,
    offset: u64,
}

/// Parse verify-pack -v output: per-object lines are
/// `<40-hex> <type> <size> <size-in-pack> <offset>`; summary lines
/// (`non delta: N objects`, `<path>: ok`) are filtered by the hex check.
fn parse_verify_pack(stdout: &[u8]) -> Vec<GitPackObject> {
    let mut objs = Vec::new();
    for line in String::from_utf8_lossy(stdout).lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 5 || t[0].len() != 40 || !t[0].bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let (Ok(size), Ok(sip), Ok(off)) = (
            t[2].parse::<u64>(),
            t[3].parse::<u64>(),
            t[4].parse::<u64>(),
        ) else {
            continue;
        };
        objs.push(GitPackObject {
            oid: t[0].to_string(),
            kind: t[1].to_string(),
            size,
            size_in_pack: sip,
            offset: off,
        });
    }
    objs
}

/// Run `f` under catch_unwind; a panic is returned as an Err string instead
/// of aborting the matrix — a panic is a finding, not a crash.
fn guard<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| {
        if let Some(s) = p.downcast_ref::<&str>() {
            format!("PANIC: {s}")
        } else if let Some(s) = p.downcast_ref::<String>() {
            format!("PANIC: {s}")
        } else {
            "PANIC: <non-string payload>".to_string()
        }
    })
}

/// Build (oid, content) pairs, deduped by git OID, preserving first-seen
/// order. Returns (pairs, dropped_duplicate_count).
fn dedupe_blobs(blobs: &[Vec<u8>]) -> (Vec<([u8; 20], Vec<u8>)>, usize) {
    let total = blobs.len();
    let mut seen = std::collections::HashSet::new();
    let mut pairs = Vec::with_capacity(total);
    for b in blobs {
        let oid = git_object_oid("blob", b);
        if seen.insert(oid) {
            pairs.push((oid, b.clone()));
        }
    }
    let dropped = total - pairs.len();
    (pairs, dropped)
}

// ------------------------------------------------------- ACCEPT MATRIX --

/// The 40-case accept matrix: writer output -> real git -> crate parser, all
/// must agree. Failures are collected across all 40 cases; one assert at the
/// end reports the full matrix.
#[test]
fn fuzz_40_accept_matrix_git_and_crate_agree() {
    let git_version = require_git();
    eprintln!("[fuzz-accept] git: {git_version}");
    let mut lcg = Lcg::new(SEED_ACCEPT);

    let mut failures: Vec<String> = Vec::new();
    let (mut total_unique, mut total_deduped, mut total_zero_blobs) = (0usize, 0usize, 0usize);
    let mut total_blob_bytes = 0u64;
    let mut cases_ok = 0usize;

    for case in 0..40usize {
        let count = 1 + lcg.below(50) as usize; // 1..=50
        let mut blobs: Vec<Vec<u8>> = Vec::with_capacity(count);
        for _ in 0..count {
            let size = draw_size(&mut lcg);
            if size == 0 {
                total_zero_blobs += 1;
            }
            total_blob_bytes += size as u64;
            blobs.push(synthesize_blob(&mut lcg, size));
        }
        let (pairs, deduped) = dedupe_blobs(&blobs);
        total_unique += pairs.len();
        total_deduped += deduped;
        let contents: Vec<&[u8]> = pairs.iter().map(|(_, c)| c.as_slice()).collect();

        let (pack, idx, entries) = match build_pack_and_idx(&contents) {
            Ok(x) => x,
            Err(e) => {
                failures.push(format!(
                    "case {case}: writer refused a valid blob set (count={count}): {e}"
                ));
                continue;
            }
        };
        if entries.len() != pairs.len() {
            failures.push(format!(
                "case {case}: writer entry count {} != unique blob count {}",
                entries.len(),
                pairs.len()
            ));
        }

        let repo = FuzzRepo::new(&format!("a{case}"));
        let (_pp, idx_path) = install_pack(&repo, &pack, &idx);

        // ---- 1. git fsck --full ----
        let fsck = git(&repo.root, &["fsck", "--full"], "git fsck --full");
        if !fsck.status_ok {
            failures.push(format!(
                "case {case}: git fsck --full REJECTED the pack: {}",
                fsck.stderr
            ));
        }

        // ---- 2. git verify-pack -v ----
        let vp = git(
            &repo.root,
            &["verify-pack", "-v", idx_path.to_str().unwrap()],
            "git verify-pack -v",
        );
        if !vp.status_ok {
            failures.push(format!(
                "case {case}: git verify-pack -v REJECTED the pack: {}",
                vp.stderr
            ));
        }
        let git_objs = parse_verify_pack(&vp.stdout_bytes);

        // ---- 3. git cat-file on every blob ----
        for (oid, content) in &pairs {
            let h = hex(oid);
            let cf = git(&repo.root, &["cat-file", "blob", &h], "git cat-file blob");
            if !cf.status_ok {
                failures.push(format!("case {case}: cat-file {h} failed: {}", cf.stderr));
            } else if cf.stdout_bytes != *content {
                failures.push(format!(
                    "case {case}: cat-file {h} content mismatch ({} vs {} bytes)",
                    cf.stdout_bytes.len(),
                    content.len()
                ));
            }
        }

        // ---- 4. crate parser vs git: count + offsets + sizes + crcs ----
        let parsed = match guard(|| parse_pack_v2(&pack)) {
            Ok(Ok(objs)) => objs,
            Ok(Err(e)) => {
                failures.push(format!(
                    "case {case}: crate parser REJECTED its own valid pack: {e:?}"
                ));
                Vec::new()
            }
            Err(p) => {
                failures.push(format!("case {case}: crate parse_pack_v2 {p}"));
                Vec::new()
            }
        };
        let idx_parsed = match guard(|| parse_idx_v2(&idx)) {
            Ok(Ok(x)) => Some(x),
            Ok(Err(e)) => {
                failures.push(format!(
                    "case {case}: crate idx parser REJECTED its own valid idx: {e:?}"
                ));
                None
            }
            Err(p) => {
                failures.push(format!("case {case}: crate parse_idx_v2 {p}"));
                None
            }
        };

        // count agreement: input == git == pack parser == idx parser
        if pairs.len() != git_objs.len()
            || pairs.len() != parsed.len()
            || idx_parsed
                .as_ref()
                .map(|(o, _, _)| o.len())
                .unwrap_or(pairs.len())
                != pairs.len()
        {
            failures.push(format!(
                "case {case}: object count disagreement: blobs={} git={} pack_parse={} idx_parse={}",
                pairs.len(),
                git_objs.len(),
                parsed.len(),
                idx_parsed.as_ref().map(|(o, _, _)| o.len()).unwrap_or(0),
            ));
        }

        // per-object offsets, oid-keyed: crate idx table vs git verify-pack
        if let Some((oids, crcs, offs)) = &idx_parsed {
            let git_by_oid: std::collections::HashMap<&str, &GitPackObject> =
                git_objs.iter().map(|o| (o.oid.as_str(), o)).collect();
            for i in 0..oids.len() {
                let h = hex(&oids[i]);
                match git_by_oid.get(h.as_str()) {
                    None => failures.push(format!(
                        "case {case}: crate idx oid {h} absent from git verify-pack output"
                    )),
                    Some(g) => {
                        if offs[i] as u64 != g.offset {
                            failures.push(format!(
                                "case {case}: offset mismatch for {h}: crate={} git={}",
                                offs[i], g.offset
                            ));
                        }
                        if g.kind != "blob" {
                            failures.push(format!(
                                "case {case}: git reports type '{}' for {h} (expected blob)",
                                g.kind
                            ));
                        }
                        // crate pack-parse at the same offset: size + crc agree
                        if let Some(p) =
                            parsed.iter().find(|p| p.offset as u64 == g.offset)
                        {
                            if p.size != g.size {
                                failures.push(format!(
                                    "case {case}: size mismatch at offset {}: crate={} git={}",
                                    g.offset, p.size, g.size
                                ));
                            }
                            if p.crc != crcs[i] {
                                failures.push(format!(
                                    "case {case}: crc mismatch for {h}: pack_parse={:08x} idx={:08x}",
                                    p.crc, crcs[i]
                                ));
                            }
                            if p.obj_type != 3 {
                                failures.push(format!(
                                    "case {case}: parsed obj_type {} (expected 3=blob)",
                                    p.obj_type
                                ));
                            }
                        } else {
                            failures.push(format!(
                                "case {case}: pack parser has no object at git offset {}",
                                g.offset
                            ));
                        }
                    }
                }
            }
            // offset sets identical, order-free
            let mut a: Vec<u64> = parsed.iter().map(|p| p.offset as u64).collect();
            let mut b: Vec<u64> = git_objs.iter().map(|o| o.offset).collect();
            a.sort_unstable();
            b.sort_unstable();
            if a != b {
                failures.push(format!(
                    "case {case}: offset sets differ: crate={a:?} git={b:?}"
                ));
            }
        }

        if failures
            .iter()
            .all(|f| !f.starts_with(&format!("case {case}:")))
        {
            cases_ok += 1;
            eprintln!(
                "[fuzz-accept case {case:02}] OK: {} objs ({} dup OIDs deduped, {} zero-byte), pack {} B, git fsck+verify-pack+cat-file ok, crate offsets agree",
                pairs.len(),
                deduped,
                blobs.iter().filter(|b| b.is_empty()).count(),
                pack.len()
            );
        }
    }

    eprintln!(
        "[fuzz-accept MATRIX] git {git_version}: {cases_ok}/40 cases fully accepted; \
         {total_unique} unique blobs ({total_deduped} dup OIDs deduped, {total_zero_blobs} zero-byte), \
         {total_blob_bytes} blob bytes packed; failures: {}",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "\n==== PACKFILE FUZZ ACCEPT MATRIX: FAIL ====\n\
         git {git_version}\n{}\n\
         ===========================================",
        failures.join("\n")
    );
}

// ------------------------------------------------------- REJECT MATRIX --

/// The 20-mutant reject matrix: one-byte mutations of VALID packs must be
/// rejected by git AND/OR answered with a typed GitError by the crate
/// parser; a panic anywhere is the finding. Per-mutant precondition: the
/// unmutated pack is git-accepted and crate-parsed clean.
#[test]
fn fuzz_20_mutants_rejected_no_panic() {
    let git_version = require_git();
    eprintln!("[fuzz-reject] git: {git_version}");
    let mut lcg = Lcg::new(SEED_REJECT);

    let mut findings: Vec<String> = Vec::new();
    let mut panics = 0usize;
    let (mut git_reject, mut crate_reject) = (0usize, 0usize);
    let (mut hdr_hits, mut body_hits, mut trl_hits) = (0usize, 0usize, 0usize);

    for m in 0..20usize {
        let region = match m % 3 {
            0 => "header",
            1 => "body",
            _ => "trailer",
        };
        // small valid pack: 2..=6 blobs, sizes capped at 64 KiB (still 0..=1MiB)
        let count = 2 + lcg.below(5) as usize;
        let mut blobs: Vec<Vec<u8>> = Vec::with_capacity(count);
        for _ in 0..count {
            let size = draw_size(&mut lcg).min(64 * KIB);
            blobs.push(synthesize_blob(&mut lcg, size));
        }
        let (pairs, _deduped) = dedupe_blobs(&blobs);
        let contents: Vec<&[u8]> = pairs.iter().map(|(_, c)| c.as_slice()).collect();
        let (pack, idx, entries) = match build_pack_and_idx(&contents) {
            Ok(x) => x,
            Err(e) => {
                findings.push(format!("mutant {m:02}: writer refused valid input: {e}"));
                continue;
            }
        };
        assert_eq!(entries.len(), pairs.len());

        // --- precondition: the pack is VALID before mutation ---------------
        let good_repo = FuzzRepo::new(&format!("g{m}"));
        let (gp, gi) = install_pack(&good_repo, &pack, &idx);
        let g_fsck = git(&good_repo.root, &["fsck", "--full"], "fsck (pre)");
        let g_vp = git(
            &good_repo.root,
            &["verify-pack", "-v", gi.to_str().unwrap()],
            "verify-pack (pre)",
        );
        if !(g_fsck.status_ok && g_vp.status_ok) {
            findings.push(format!(
                "mutant {m:02}: precondition FAIL: git rejected the UNMUTATED pack \
                 (fsck ok={} vp ok={}): {} / {}",
                g_fsck.status_ok, g_vp.status_ok, g_fsck.stderr, g_vp.stderr
            ));
        }
        let pre_parse = guard(|| parse_pack_v2(&pack));
        match &pre_parse {
            Ok(Ok(objs)) => {
                if objs.len() != pairs.len() {
                    findings.push(format!(
                        "mutant {m:02}: precondition FAIL: crate parsed {} objs, expected {}",
                        objs.len(),
                        pairs.len()
                    ));
                }
            }
            Ok(Err(e)) => findings.push(format!(
                "mutant {m:02}: precondition FAIL: crate rejected UNMUTATED pack: {e:?}"
            )),
            Err(p) => {
                panics += 1;
                findings.push(format!("mutant {m:02}: crate parse_pack_v2 {p}"));
            }
        }
        drop(good_repo);
        let _ = (gp, gi);

        // --- mutate exactly one byte ---------------------------------------
        let mut mutated = pack.clone();
        let body_end = mutated.len() - 20;
        let (pos, bit) = match m % 3 {
            0 => (lcg.below(12) as usize, 1u8 << lcg.below(8)), // header 0..12
            1 => (
                12 + lcg.below((body_end - 12) as u64) as usize, // body
                1u8 << lcg.below(8),
            ),
            _ => (body_end + lcg.below(20) as usize, 1u8 << lcg.below(8)), // trailer
        };
        let orig_byte = mutated[pos];
        mutated[pos] ^= bit;
        assert_ne!(mutated[pos], orig_byte, "XOR flip must change the byte");

        // --- git verdict on the mutant (fresh repo, original idx) ----------
        let bad_repo = FuzzRepo::new(&format!("m{m}"));
        let (_mp, mi) = install_pack(&bad_repo, &mutated, &idx);
        let fsck = git(&bad_repo.root, &["fsck", "--full"], "fsck (mutant)");
        let vp = git(
            &bad_repo.root,
            &["verify-pack", "-v", mi.to_str().unwrap()],
            "verify-pack (mutant)",
        );
        let git_rejects = !(fsck.status_ok && vp.status_ok);
        if git_rejects {
            git_reject += 1;
            match m % 3 {
                0 => hdr_hits += 1,
                1 => body_hits += 1,
                _ => trl_hits += 1,
            }
        }

        // --- crate verdict on the mutant -----------------------------------
        let crate_verdict = guard(|| parse_pack_v2(&mutated));
        let (crate_errs, crate_verdict_s) = match &crate_verdict {
            Ok(Ok(_)) => (false, "Ok(_): CRATE ACCEPTED MUTANT".to_string()),
            Ok(Err(e)) => {
                crate_reject += 1;
                let _e: &GitError = e;
                (true, format!("Err({e:?})"))
            }
            Err(p) => {
                panics += 1;
                (false, p.clone())
            }
        };

        // --- control: the ORIGINAL idx must still parse clean --------------
        let idx_ctrl = guard(|| parse_idx_v2(&idx));
        match &idx_ctrl {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => findings.push(format!(
                "mutant {m:02}: control FAIL: crate idx parser rejects the ORIGINAL idx: {e:?}"
            )),
            Err(p) => {
                panics += 1;
                findings.push(format!("mutant {m:02}: crate parse_idx_v2 {p}"));
            }
        }
        drop(bad_repo);

        // --- contract: git must reject AND/OR crate must return typed Err --
        if !(git_rejects || crate_errs) {
            findings.push(format!(
                "mutant {m:02}: NEITHER rejection fired: git fsck ok={} vp ok={}, \
                 crate parse_pack_v2 = {crate_verdict_s}",
                fsck.status_ok, vp.status_ok
            ));
        }
        eprintln!(
            "[fuzz-reject m={m:02} region={region:7} pos={pos} bit=0x{bit:02X} orig=0x{orig_byte:02X}] \
             git: {} | crate: {crate_verdict_s} | panics so far: {panics}",
            if git_rejects { "REJECT" } else { "ACCEPT (!)" }
        );
    }

    eprintln!(
        "[fuzz-reject MATRIX] git {git_version}: git rejected {git_reject}/20 \
         (header {hdr_hits}/7, body {body_hits}/7, trailer {trl_hits}/6); \
         crate typed-error {crate_reject}/20; panics: {panics}; findings: {}",
        findings.len()
    );
    assert!(
        panics == 0 && findings.is_empty(),
        "\n==== PACKFILE FUZZ REJECT MATRIX: FAIL ====\n\
         git {git_version}\npanics: {panics}\n{}\n\
         ===========================================",
        findings.join("\n")
    );
}
