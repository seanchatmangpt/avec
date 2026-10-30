#!/usr/bin/env python3
"""symbol_gate.py — zero-panic Jidoka gate (charter 7.2.1).

Scans the compiled release/audit cdylib artifacts for forbidden panicking
symbols and fails loudly (exit 1) if any appear.

Artifacts (built by this script with the exact cargo commands):
  target/audit/libavec_audit.dylib   <- cargo build --profile audit -p avec-audit
  target/release/libavec_ffi.dylib   <- cargo build --release -p avec-ffi

Forbidden patterns are substring matches applied to the raw symbol table
(mangled or demangled — Rust v0 mangling keeps path segments verbatim in the
mangled identifier, so 'panicking' / 'rust_begin_unwind' / 'abort' appear
literally; verified empirically on arm64-apple-darwin, rustc 1.97).

Anti-vacuity (a gate that cannot fail carries no bits): after the repo scan,
this script compiles a throwaway panic!-ing cdylib under /tmp, runs it through
the SAME scanner, and requires the gate to fire on it. If the scanner passes
the probe, the gate is vacuous and the run exits 1 as VACUOUS-GATE.

stdlib only. No git commands. Exits 1 on: forbidden symbol in a repo
artifact, artifact/build failure, or a vacuous gate.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# (label, cargo args, artifact path relative to repo root)
ARTIFACTS = [
    (
        "avec-audit (audit profile)",
        ["cargo", "build", "--profile", "audit", "-p", "avec-audit"],
        os.path.join("target", "audit", "libavec_audit.dylib"),
    ),
    (
        "avec-ffi (release profile)",
        ["cargo", "build", "--release", "-p", "avec-ffi"],
        os.path.join("target", "release", "libavec_ffi.dylib"),
    ),
]

# Forbidden substrings, per charter 7.2.1. Matched against the raw symbol
# name; the mangled v0 form carries the same substrings verbatim.
FORBIDDEN = [
    "panicking",
    "rust_begin_unwind",
    "core::panicking",
    "__cxa_throw",
    "abort",
]

SELFTEST_RS = """\
// Throwaway anti-vacuity probe (built under /tmp, never in the repo).
#[no_mangle]
pub extern "C" fn symbol_gate_selftest_fire(x: u64) -> u64 {
    if x > 0 {
        x
    } else {
        panic!("symbol-gate anti-vacuity probe: this must trip the gate");
    }
}
"""

MAX_PRINTED_HITS = 25


def run(cmd, cwd=None):
    """Run a command; return (returncode, combined output)."""
    proc = subprocess.run(
        cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
    )
    return proc.returncode, proc.stdout


def pick_nm():
    """Prefer xcrun llvm-nm; fall back to plain nm."""
    for candidate in (["xcrun", "llvm-nm"], ["nm"]):
        rc, _ = run(candidate + ["--version"])
        if rc == 0:
            return candidate
    return None


def build_artifacts():
    """Run the exact cargo builds. Returns list of failed artifact tuples."""
    failures = []
    for label, args, rel in ARTIFACTS:
        rc, out = run(args, cwd=REPO_ROOT)
        if rc != 0:
            print(f"  BUILD FAIL: {' '.join(args)} (exit {rc})")
            for line in out.strip().splitlines()[-15:]:
                print(f"    | {line}")
            failures.append((label, args, rel))
            continue
        if not os.path.isfile(os.path.join(REPO_ROOT, rel)):
            print(f"  BUILD FAIL: {' '.join(args)} ran but produced no {rel}")
            failures.append((label, args, rel))
    return failures


def scan_symbols(path, nm_cmd):
    """Return (total_symbols, hits); hits are (symbol, pattern) pairs.

    Plain nm (no -gU) is required: the panic machinery in these artifacts is
    predominantly LOCAL (lowercase 't') text symbols, which -gU would skip.
    """
    rc, out = run(nm_cmd + [path])
    if rc != 0:
        raise RuntimeError(f"{nm_cmd} exited {rc} on {path}: {out.strip()[:400]}")
    symbols = []
    for line in out.splitlines():
        parts = line.split()
        if not parts:
            continue
        if len(parts) == 1:
            symbols.append(parts[0])
        elif len(parts) == 2:
            # e.g. undefined symbol listing: just the name field
            symbols.append(parts[-1])
        else:
            symbols.append(" ".join(parts[2:]))
    total = len(symbols)
    hits = []
    for sym in symbols:
        for pat in FORBIDDEN:
            if pat in sym:
                hits.append((sym, pat))
                break  # first matching pattern wins; one hit per symbol
    return total, hits


def build_selftest_dylib(tmpdir, rustc):
    """Compile the panic probe as a cdylib under /tmp. Returns dylib path."""
    src = os.path.join(tmpdir, "symbol_gate_selftest.rs")
    with open(src, "w", encoding="utf-8") as f:
        f.write(SELFTEST_RS)
    dylib = os.path.join(tmpdir, "libsymbol_gate_selftest.dylib")
    rc, out = run(
        [rustc, "--edition", "2021", "--crate-type", "cdylib", "-O", "-o", dylib, src]
    )
    if rc != 0:
        raise RuntimeError(f"selftest rustc failed (exit {rc}): {out.strip()[:600]}")
    if not os.path.isfile(dylib):
        raise RuntimeError("selftest dylib not produced at expected path")
    return dylib


def main() -> int:
    print("=" * 74)
    print("SYMBOL GATE - zero-panic Jidoka (charter 7.2.1)")
    print("=" * 74)

    nm_cmd = pick_nm()
    if nm_cmd is None:
        print("FINAL: FAIL - no nm implementation found (xcrun llvm-nm / nm)")
        return 1
    tool = "xcrun llvm-nm" if nm_cmd[0] == "xcrun" else "nm"
    print(f"symbol table tool: {tool}")

    # 1. Build the artifacts under audit.
    print("\n[1] building repo artifacts")
    build_failures = build_artifacts()
    if build_failures:
        print(
            f"\nFINAL: FAIL - {len(build_failures)} artifact build(s) failed; nothing to scan"
        )
        return 1
    for _, _, rel in ARTIFACTS:
        print(f"  built ok: {rel}")

    # 2. Scan each repo artifact.
    print("\n[2] scanning repo artifacts for forbidden panicking symbols")
    artifact_failures = 0
    per_pattern = {p: 0 for p in FORBIDDEN}
    total_hits = 0
    vacuous = None
    for label, _, rel in ARTIFACTS:
        path = os.path.join(REPO_ROOT, rel)
        try:
            total, hits = scan_symbols(path, nm_cmd)
        except RuntimeError as exc:
            print(f"  SCAN FAIL: {exc}")
            print(f"\nFINAL: FAIL - scan error on {rel}")
            return 1
        total_hits += len(hits)
        for _, pat in hits:
            per_pattern[pat] += 1
        verdict = "FAIL" if hits else "PASS"
        if hits:
            artifact_failures += 1
        print(f"\n  {label}")
        print(f"    artifact       : {rel}")
        print(f"    symbols scanned: {total}")
        print(f"    forbidden hits : {len(hits)}")
        for sym, pat in hits[:MAX_PRINTED_HITS]:
            print(f"      [{pat}] {sym}")
        if len(hits) > MAX_PRINTED_HITS:
            print(f"      ... and {len(hits) - MAX_PRINTED_HITS} more")
        print(f"    verdict        : {verdict}")

    # 3. Anti-vacuity: the gate must be able to fail.
    print("\n[3] anti-vacuity probe (throwaway panic! cdylib under /tmp)")
    tmpdir = tempfile.mkdtemp(prefix="symbol_gate_selftest_", dir="/tmp")
    try:
        rustc = shutil.which("rustc")
        if rustc is None:
            print("  SELFTEST-ERROR: rustc not found on PATH")
            vacuous = True
        else:
            try:
                dylib = build_selftest_dylib(tmpdir, rustc)
                st_total, st_hits = scan_symbols(dylib, nm_cmd)
                vacuous = len(st_hits) == 0
                fired = not vacuous
                print(
                    f"  probe dylib: {dylib} "
                    f"({st_total} symbols, {len(st_hits)} forbidden hits)"
                )
                for sym, pat in st_hits[:5]:
                    print(f"    [{pat}] {sym[:96]}")
                print(
                    "  probe result: "
                    + ("FIRED (gate can fail)" if fired else "PASSED - gate is VACUOUS")
                )
            except RuntimeError as exc:
                print(f"  SELFTEST-ERROR: {exc}")
                vacuous = True
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)
        print(f"  cleaned up: {tmpdir}")

    # 4. Final verdict.
    fired = not vacuous
    print("\n" + "=" * 74)
    summary = ", ".join(f"{p}={c}" for p, c in per_pattern.items() if c)
    print(
        f"repo artifacts failing: {artifact_failures}/{len(ARTIFACTS)}"
        f" | total forbidden hits: {total_hits}"
        f" | anti-vacuity probe: {'FIRED' if fired else 'VACUOUS'}"
    )
    if summary:
        print(f"hits by pattern: {summary}")
    if artifact_failures:
        if fired:
            print(
                "FINAL: FAIL - forbidden panicking symbols present in repo artifacts "
                f"({artifact_failures}/{len(ARTIFACTS)}); gate proven able to fail"
            )
        else:
            print("FINAL: FAIL - forbidden symbols present AND gate is VACUOUS")
        return 1
    if not fired:
        print("FINAL: FAIL - VACUOUS GATE: probe panic! dylib passed the scanner")
        return 1
    print("FINAL: PASS - no forbidden panicking symbols; gate proven able to fail")
    return 0


if __name__ == "__main__":
    sys.exit(main())
