//! FR-04 acceptance court (lane I8): feed a real `git` binary a pack+idx
//! produced by the cgap-git writer and record what git actually says.
//!
//! Chicago discipline: real git subprocess, real bytes on disk, assert final
//! state. No mocks. The writer emits RFC 1950 zlib / RFC 1951 fixed-Huffman
//! deflate blob bodies (landed by the wave-4 deflate fix); git 2.51.2 accepts
//! the resulting pack and this court runs 2/2 PASS as of wave 4. Ownership of
//! the deflate writer surface: wave-4 lane I6.
//!
//! Anti-vacuity: because the baseline pack is expected to be rejected, the
//! negative control builds a pack git *accepts* (via `git pack-objects`),
//! corrupts one byte in a copy, and asserts git then rejects it. That proves
//! this environment's git actually detects corruption — the rejection of the
//! cgap-git pack is signal, not a broken git installation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fail loudly if git is unavailable — never fake a pass.
fn require_git() -> String {
    match Command::new("git").arg("--version").output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => panic!(
            "SKIP-REFUSAL: git binary exists but `git --version` failed (exit {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ),
        Err(e) => panic!("SKIP-REFUSAL: git binary not runnable: {e}. Court cannot run without real git."),
    }
}

struct Repo {
    root: PathBuf,
}

impl Repo {
    /// `git init -q` a fresh repo in a unique temp dir.
    fn new(tag: &str) -> Repo {
        let root = std::env::temp_dir().join(format!(
            "cgap-git-fsck-court-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        run(&root, &["init", "-q"], "git init");
        Repo { root }
    }

    fn pack_dir(&self) -> PathBuf {
        self.root.join(".git").join("objects").join("pack")
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

struct GitOut {
    status_ok: bool,
    stdout: String,
    stderr: String,
}

fn run(dir: &Path, args: &[&str], what: &str) -> GitOut {
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
        status_ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

/// Write pack+idx into .git/objects/pack/ with git's canonical
/// `pack-<pack-trailer-sha-hex>.{pack,idx}` naming.
fn install_pack(repo: &Repo, pack: &[u8], idx: &[u8]) -> (PathBuf, PathBuf) {
    let pack_sha = &pack[pack.len() - 20..];
    let name = format!("pack-{}", hex(pack_sha));
    fs::create_dir_all(repo.pack_dir()).unwrap();
    let pack_path = repo.pack_dir().join(format!("{name}.pack"));
    let idx_path = repo.pack_dir().join(format!("{name}.idx"));
    fs::write(&pack_path, pack).unwrap();
    fs::write(&idx_path, idx).unwrap();
    (pack_path, idx_path)
}

fn cgap_blob(seed: u8, len: usize) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

// ------------------------------------------------------------------ court --

/// THE court: cgap-git pack+idx -> real git fsck / verify-pack / cat-file.
#[test]
fn fsck_court_accepts_cgap_git_pack() {
    let git_version = require_git();
    eprintln!("[court] git: {git_version}");

    let repo = Repo::new("main");

    let blobs: Vec<Vec<u8>> = vec![
        cgap_blob(1, 64),
        b"hello cgap-git fsck court\n".to_vec(),
        cgap_blob(9, 512),
    ];
    let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
    let (pack, idx, entries) = cgap_git::pack_write::build_pack_and_idx(&refs)
        .unwrap_or_else(|e| panic!("cgap-git build_pack_and_idx failed: {e}"));
    assert_eq!(entries.len(), 3);
    let (pack_path, idx_path) = install_pack(&repo, &pack, &idx);
    eprintln!("[court] installed {}", pack_path.display());

    // --- git fsck --full ---------------------------------------------------
    let fsck = run(&repo.root, &["fsck", "--full"], "git fsck --full");
    eprintln!(
        "[court] git fsck --full: exit-ok={} stdout={}bytes stderr={:?}",
        fsck.status_ok,
        fsck.stdout.len(),
        fsck.stderr
    );

    // --- git verify-pack -v ------------------------------------------------
    let vp = run(
        &repo.root,
        &["verify-pack", "-v", idx_path.to_str().unwrap()],
        "git verify-pack -v",
    );
    eprintln!(
        "[court] git verify-pack -v: exit-ok={} stderr={:?}",
        vp.status_ok, vp.stderr
    );

    // --- git cat-file -p on the first blob ---------------------------------
    let oid_hex = hex(&entries[0].oid);
    let cf = run(&repo.root, &["cat-file", "-p", &oid_hex], "git cat-file -p");
    eprintln!(
        "[court] git cat-file -p {oid_hex}: exit-ok={} stderr={:?}",
        cf.status_ok, cf.stderr
    );

    // --- verdicts -----------------------------------------------------------
    // The court asserts git ACCEPTS the writer's output. Each rejection is a
    // real court FAIL: print the actual git stderr and fail.
    let mut failures = Vec::new();
    if !fsck.status_ok {
        failures.push(format!(
            "COURT-FAIL git fsck --full rejected the cgap-git pack:\n--- fsck stderr ---\n{}\n-------------------",
            fsck.stderr
        ));
    } else {
        // fsck passing but reporting dangling objects is fine; only stderr
        // containing "error"/"fatal" alongside failure exits is rejection.
    }
    if !vp.status_ok {
        failures.push(format!(
            "COURT-FAIL git verify-pack -v rejected the pack/idx pair:\n{}",
            vp.stderr
        ));
    }
    if !cf.status_ok {
        failures.push(format!(
            "COURT-FAIL git cat-file -p {oid_hex} could not read the stored blob:\n{}",
            cf.stderr
        ));
    }

    assert!(
        failures.is_empty(),
        "\n==================== FR-04 FSCK COURT: FAIL ====================\n\
         git {}\n\
         cgap-git writer produced pack of {} bytes / {} objects; git rejected it.\n\
         The writer emits zlib (RFC 1950/1951) bodies since the wave-4 deflate\n\
         fix; a rejection here is a live regression on that surface.\n\
         {}\n\
         ================================================================",
        git_version,
        pack.len(),
        blobs.len(),
        failures.join("\n\n")
    );
}

/// Anti-vacuity control: git in THIS environment detects a single corrupted
/// byte in a pack it otherwise accepts. Without this, the court above could
/// be passing/failing against a git that never checks anything.
#[test]
fn negative_control_corrupted_git_built_pack_is_rejected() {
    let git_version = require_git();
    let repo = Repo::new("control");

    // Build a pack git itself certifies: hash-object the blob, pack-objects it.
    let blob = cgap_blob(5, 256);
    let blob_path = repo.root.join("control-blob.bin");
    fs::write(&blob_path, &blob).unwrap();
    let ho = run(
        &repo.root,
        &["hash-object", "-w", blob_path.to_str().unwrap()],
        "git hash-object -w",
    );
    assert!(ho.status_ok, "git hash-object -w failed: {}", ho.stderr);
    let oid = ho.stdout.trim();

    // Canonical form: `git pack-objects <base-name> < object-list`
    // std::process::Command cannot redirect stdin from a file directly, so
    // pipe it manually.
    use std::io::Write;
    use std::process::Stdio;
    let base = repo.pack_dir().join("pack");
    fs::create_dir_all(repo.pack_dir()).unwrap();
    let mut child = Command::new("git")
        .args(["pack-objects"])
        .arg(&base)
        .current_dir(&repo.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git pack-objects");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{oid}\n").as_bytes())
        .unwrap();
    let po_out = child.wait_with_output().expect("wait pack-objects");
    assert!(
        po_out.status.success(),
        "git pack-objects failed: {}",
        String::from_utf8_lossy(&po_out.stderr)
    );
    let pack_name = String::from_utf8_lossy(&po_out.stdout).trim().to_string();
    assert!(!pack_name.is_empty(), "pack-objects produced no pack name");

    // Sanity: git accepts its own pack, uncorrupted.
    // pack-objects with base `.../pack` writes `pack-<hash>.{pack,idx}`;
    // its stdout is the bare `<hash>`.
    let good_idx = repo.pack_dir().join(format!("pack-{pack_name}.idx"));
    let good_fsck = run(&repo.root, &["fsck", "--full"], "git fsck (control good)");
    eprintln!(
        "[court] control (git-built pack, uncorrupted): fsck exit-ok={}",
        good_fsck.status_ok
    );
    assert!(
        good_fsck.status_ok,
        "control precondition failed: git rejects its OWN uncorrupted pack \
         (fsck stderr: {}); this git installation cannot serve as the court's oracle",
        good_fsck.stderr
    );
    let good_vp = run(
        &repo.root,
        &["verify-pack", "-v", good_idx.to_str().unwrap()],
        "git verify-pack (control good)",
    );
    assert!(
        good_vp.status_ok,
        "control precondition failed: verify-pack rejects git's own pack: {}",
        good_vp.stderr
    );

    // Corrupt exactly one body byte in a copy (inside the pack data region,
    // past the 12-byte header, before the 20-byte trailer).
    let good_pack_path = repo.pack_dir().join(format!("pack-{pack_name}.pack"));
    let mut corrupt_bytes = fs::read(&good_pack_path).unwrap();
    assert!(corrupt_bytes.len() > 40, "control pack implausibly small");
    corrupt_bytes[16] ^= 0x01;
    let corrupt_repo = Repo::new("corrupt");
    install_pack(&corrupt_repo, &corrupt_bytes, &fs::read(&good_idx).unwrap());
    let bad_fsck = run(
        &corrupt_repo.root,
        &["fsck", "--full"],
        "git fsck (control corrupt)",
    );
    let bad_vp = run(
        &corrupt_repo.root,
        &[
            "verify-pack",
            "-v",
            corrupt_repo.pack_dir().join(format!("pack-{pack_name}.idx")).to_str().unwrap(),
        ],
        "git verify-pack (control corrupt)",
    );
    eprintln!(
        "[court] control (1-byte corruption): fsck exit-ok={} verify-pack exit-ok={} \
         fsck stderr={:?}",
        bad_fsck.status_ok, bad_vp.status_ok, bad_fsck.stderr
    );
    assert!(
        !bad_fsck.status_ok || !bad_vp.status_ok,
        "VACUOUS COURT: git {} accepted a pack with byte 16 corrupted \
         (fsck ok, verify-pack ok). This environment's git detects nothing; \
         the fsck court's verdict carries no bits.",
        git_version
    );
}
