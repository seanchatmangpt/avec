#!/usr/bin/env python3
"""A16 wave-7 shim-court battery (repo /Users/sac/cgap, sandbox /tmp/a16-w7-sb).

Method per mutation: apply exact-string shim (assert count==1) -> cargo test
--no-fail-fast -> record flip sets per target -> restore file from repo copy
-> sha256 byte-exact restore check. Repo files are never mutated; all shims
land in the sandbox copy only.
"""
import hashlib
import pathlib
import re
import subprocess
import sys

SB = pathlib.Path("/tmp/a16-w7-sb")
REPO = pathlib.Path("/Users/sac/cgap/crates/cgap-crypto")
REPO_ROOT = pathlib.Path("/Users/sac/cgap")
FILES = [
    "src/chatman.rs", "src/lease.rs", "src/lib.rs", "Cargo.toml",
    "tests/chatman_courts.rs", "tests/lease_courts.rs",
]

def sha256(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def setup_sandbox():
    """Fresh sandbox copy of the crate + kernel dep + standalone workspace
    (dev profile panic=abort, matching the root workspace). Asserts the copy
    is byte-identical to the repo for every lane-owned file, then requires a
    GREEN baseline before any shim is applied."""
    import shutil
    if SB.exists():
        shutil.rmtree(SB)
    (SB / "crates").mkdir(parents=True)
    shutil.copytree(REPO, SB / "crates/cgap-crypto",
                    ignore=shutil.ignore_patterns("target"))
    shutil.copytree(REPO.parent / "cgap-blake3", SB / "crates/cgap-blake3",
                    ignore=shutil.ignore_patterns("target"))
    license_md = REPO_ROOT / "LICENSE.md"
    if license_md.exists():
        shutil.copy(license_md, SB / "LICENSE.md")
    (SB / "Cargo.toml").write_text(
        "[workspace]\n"
        'resolver = "2"\n'
        'members = ["crates/cgap-crypto", "crates/cgap-blake3"]\n\n'
        "[workspace.package]\n"
        'version = "26.10.29"\n'
        'edition = "2021"\n'
        'repository = "https://github.com/seanchatmangpt/cgap"\n'
        'authors = ["Sean Chatman <info@chatmangpt.com>"]\n\n'
        "[profile.dev]\n"
        'panic = "abort"\n'
        "[profile.release]\n"
        'panic = "abort"\n'
    )
    for rel in FILES:
        if sha256(REPO / rel) != sha256(SB / "crates/cgap-crypto" / rel):
            raise SystemExit(f"SETUP MISMATCH {rel}")
    r = subprocess.run(
        ["cargo", "test", "--no-fail-fast", "-p", "cgap-crypto"],
        cwd=SB, capture_output=True, text=True,
    )
    if r.returncode != 0:
        (pathlib.Path("/tmp") / "a16-w7-baseline.log").write_text(
            r.stdout + r.stderr)
        raise SystemExit(
            f"BASELINE NOT GREEN (exit {r.returncode}); battery refused — "
            "courts must start from a green baseline, never a failing one")

def apply(name, old, new):
    p = SB / "crates/cgap-crypto" / name
    src = p.read_text()
    n = src.count(old)
    assert n == 1, f"{name}: shim anchor count {n} != 1"
    p.write_text(src.replace(old, new))
    return f"applied {name} (1 occurrence)"

def restore(name):
    src = REPO / name
    dst = SB / "crates/cgap-crypto" / name
    dst.write_bytes(src.read_bytes())
    a, b = sha256(src), sha256(dst)
    assert a == b, f"RESTORE FAILURE {name}: {a} != {b}"
    return f"restored {name} byte-exact ({a[:12]}...)"

def run_tests(logname):
    r = subprocess.run(
        ["cargo", "test", "--no-fail-fast", "-p", "cgap-crypto"],
        cwd=SB, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    out = r.stdout
    (pathlib.Path("/tmp") / logname).write_text(out)
    # flip sets per target: track Running <target> sections + FAILED names
    # (stdout/stderr interleaved so section headers precede their tests)
    flips = {}
    target = "lib"
    for line in out.splitlines():
        m = re.search(r"Running\s+(\S+)", line)
        if m:
            target = m.group(1)
        m = re.match(r"test (\S+) \.\.\. FAILED", line)
        if m:
            flips.setdefault(target, []).append(m.group(1))
    return r.returncode, flips

SHIMS = [
    # (id, description, file, old, new)
    ("M1", "tick advance dropped (rewind-capable)", "src/lease.rs",
     "        self.tick.0 = t + 1;\n        t\n",
     "        let _ = t; // SHIM-COURT M1: advance dropped (rewind-capable)\n        t\n"),
    ("M2", "exhaustion trap -> warning return", "src/lease.rs",
     '''        if t >= self.tick_ceiling {
            panic!(
                "cgap-crypto: Phi_lease authority EXHAUSTED: tick {} == \\
                 tick_ceiling {} (lease_id {:?}) — §5 exhaustion trap; mint a \\
                 new lease instead of continuing this epoch",
                t, self.tick_ceiling, self.lease_id
            );
        }
''',
     '''        if t >= self.tick_ceiling {
            eprintln!("SHIM-COURT M2: exhaustion downgraded to warning (no trap)");
            return t;
        }
'''),
    ("M3d", "constructor zero-ceiling trap removed", "src/lease.rs",
     '''        if tick_ceiling == 0 {
            panic!(
                "cgap-crypto: Phi_lease tick_ceiling must be > 0 (§5); got 0 \\
                 — refusing a lease that is exhausted at birth"
            );
        }
''',
     '''        if false {
            eprintln!("SHIM-COURT M3d: zero-ceiling trap removed");
        }
'''),
    ("M4", "verify digest compare -> first byte only", "src/chatman.rs",
     "    if recomputed != stored_digest {\n        return Err(VerifyError::DigestMismatch);\n    }\n",
     "    if recomputed[0] != stored_digest[0] {\n        return Err(VerifyError::DigestMismatch); // SHIM-COURT M4: first byte only\n    }\n"),
    ("M5", "keyed digest ignores the key (cv = zero words)", "src/chatman.rs",
     "    let mut cv = key_words; // keyed mode: first chunk CV = key (BLAKE3 spec)\n",
     "    let mut cv = [0u32; 8]; // SHIM-COURT M5: key ignored (zero-key keyed mode)\n"),
    ("M5b", "verify coverage drops phi+key (unkeyed hash of delta)", "src/chatman.rs",
     '''    let recomputed = keyed_digest(&phi.issued_root, &covered)
        .expect("192 B digest input: single-chunk bound cannot trip");
''',
     '''    let _ = &covered;
    let recomputed = hash(&delta.to_bytes()); // SHIM-COURT M5b: coverage dropped
'''),
    ("M6", "per-call counter XOR into receipt byte 0", "src/chatman.rs",
     "        out[0..4].copy_from_slice(&magic);\n",
     '''        out[0..4].copy_from_slice(&magic);
        out[0] ^= { // SHIM-COURT M6: per-call counter XOR into byte 0
            static C: core::sync::atomic::AtomicU64 =
                core::sync::atomic::AtomicU64::new(0);
            C.fetch_add(1, core::sync::atomic::Ordering::Relaxed) as u8
        };
'''),
    ("M7", "from_bytes magic check dropped", "src/chatman.rs",
     '''        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        if magic != UNSEALED_RECEIPT_MAGIC {
            return Err(VerifyError::BadMagic(magic));
        }
''',
     '''        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        // SHIM-COURT M7: magic check dropped
'''),
    ("M8", "verify_seal first-byte-only compare", "src/chatman.rs",
     '''        let diff = expected
            .iter()
            .zip(seal.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b));
        len_ok && diff == 0
''',
     '''        let diff = (expected.first() != seal.first()) as u8; // SHIM-COURT M8: first byte only
        len_ok && diff == 0
'''),
]

def main():
    setup_sandbox()
    print("sandbox ready; baseline green (exit 0)", flush=True)
    receipts = []
    for mid, desc, fname, old, new in SHIMS:
        print(f"=== {mid}: {desc} ===", flush=True)
        try:
            print(" ", apply(fname, old, new), flush=True)
        except AssertionError as e:
            print(f"  SHIM-APPLY FAILED: {e}", flush=True)
            receipts.append((mid, desc, "SHIM-APPLY-FAILED", {}))
            continue
        code, flips = run_tests(f"a16-w7-{mid}.log")
        print(f"  exit={code} flips={flips}", flush=True)
        print(" ", restore(fname), flush=True)
        receipts.append((mid, desc, code, flips))
    print("\n=== SUMMARY ===")
    for mid, desc, code, flips in receipts:
        flat = {t: sorted(fs) for t, fs in flips.items()}
        print(f"{mid}\texit={code}\t{flat}")
    # final restore-state check: whole crate identical to repo
    bad = []
    for rel in ["src/chatman.rs", "src/lease.rs", "src/lib.rs", "Cargo.toml",
                "tests/chatman_courts.rs", "tests/lease_courts.rs"]:
        if sha256(REPO / rel) != sha256(SB / "crates/cgap-crypto" / rel):
            bad.append(rel)
    print("SANDBOX==REPO after battery:", "YES" if not bad else f"NO {bad}")

if __name__ == "__main__":
    main()
