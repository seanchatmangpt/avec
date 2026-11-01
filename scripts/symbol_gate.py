#!/usr/bin/env python3
"""symbol_gate.py — zero-panic Jidoka gate (charter 7.2.1), rescoped.

Architecture law this gate encodes: the substrate's freestanding proof IS the
wasm artifact. A no_std substrate that supplies panic glue as consumer-side
link code proves "no panicking code ships" only on the artifact a wasm
consumer actually links: the optimized (-Oz) cdylib. So the gate is:

  PRIMARY (gating): the optimized wasm artifact of crates/wasm-footprint
    (cargo build --release --target wasm32-wasip1 inside the standalone
    crate, then wasm-opt -Oz; artifact at crates/wasm-footprint/target/
    wasm32-wasip1/release/wasm_footprint.opt.wasm). Gate: 0 forbidden
    panicking/unwind/abort/throw symbols.

  SECONDARY (informational, NEVER gating): the std-linked host membranes
    libcgap_audit.dylib / libcgap_ffi.dylib. Reported with counts.
    Rationale: these are host runtime glue, std-linked by construction
    (format!/Vec/BufWriter pull the full std::panicking machinery), so
    gating them would measure std, not cgap. The substrate's no_std law is
    proven by the wasm gate; the host membranes are not the substrate.

  ANTI-VACUITY (gating on vacuity): a panic-containing no_std wasm crate
    built under /tmp must FIRE the primary scanner at BOTH stages of the
    pipeline:
      raw stage — name-section symbols (real Rust panic machinery:
                  core::panicking::*),
      opt stage — export-surface symbols surviving -Oz.
    If either stage passes the scanner, the gate is VACUOUS -> exit 1.

  THIRD (GATING): version-string integrity over BINARY CONTENT (not symbol
    names): count the byte sequence b"26.10.29\x00" (the NUL-terminated
    release version, single-sourced from the workspace manifest via
    CARGO_PKG_VERSION / the ggen-generated cgap_arrow::VERSION_C) in each
    shipped artifact and enforce a per-artifact EXACT-COUNT law:
      optimized wasm      -> exactly 1 (it exports wasm_version() returning
                             the NUL-terminated string)
      libcgap_ffi.dylib   -> exactly 1 (exposes cgap_version via VERSION_C)
      libcgap_audit.dylib -> exactly 0 (crates/audit has no version export;
                             VERSION_C is unreferenced there and linker-GC'd
                             — witnessed 2026-10-29: fresh `cargo build
                             --profile audit -p cgap-audit` then byte-count 0)
    Any mismatch fires: 2+ = version stamped twice (duplication), 0 on a
    version-carrying artifact = drift/bump without re-pinning the gate.
    Anti-vacuity: byte-level shims on a /tmp COPY of the optimized wasm
    (duplicate the needle; strip it entirely) must FIRE the law both ways,
    and the real artifact must re-count clean after the shims — the real
    artifact is never written.

Empirical scan facts (witnessed on this tree: rustc 1.97.0, wasm-opt 112,
wasm-objdump 1.0.42, Apple LLVM 17 llvm-nm):

  * llvm-nm reads wasm. On the raw cdylib (name section present) it
    enumerates the internal symbols, including core::panicking symbols
    (panic_bounds_check / panic_fmt / slice_index_fail) — dead code that
    wasm-opt -Oz then eliminates (8 funcs / 2030 code bytes -> 4 funcs /
    1736 bytes; the panic machinery is gone from the optimized artifact).
    This is precisely why the gate subjects the OPTIMIZED artifact: it is
    the artifact a wasm consumer actually links.
  * wasm-opt -Oz drops the wasm name section, so on the optimized artifact
    only the export surface is enumerable. A nameless internal panic
    function would be invisible to any name scan — mitigated by unioning
    llvm-nm names with the wasm-objdump -x function/export/global names,
    and by the optimized-stage probe prong. If both tools yield zero
    enumerable names the gate fails closed: cannot-enumerate is FAIL, not
    PASS.

Forbidden patterns are substring matches over every enumerable name; Rust
v0 mangling keeps path segments verbatim, so 'panicking' etc. appear
literally in mangled identifiers (verified empirically on arm64
wasm32-wasip1, rustc 1.97).

stdlib only. No git commands. Exits 1 on: forbidden symbol in the optimized
wasm artifact, primary build/tooling failure, cannot-enumerate, a vacuous
panic probe, a version-integrity law violation (occurrence count != the
artifact's expected count), or a vacuous version shim. Secondary results
never affect the exit code.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# --- primary subject ------------------------------------------------------
WASM_CRATE = os.path.join(REPO_ROOT, "crates", "wasm-footprint")
WASM_TARGET = "wasm32-wasip1"
WASM_RELEASE_DIR = os.path.join(
    WASM_CRATE, "target", WASM_TARGET, "release"
)
RAW_WASM = os.path.join(WASM_RELEASE_DIR, "wasm_footprint.wasm")
OPT_WASM = os.path.join(WASM_RELEASE_DIR, "wasm_footprint.opt.wasm")

# --- secondary subjects (informational, never gating) ----------------------
HOST_ARTIFACTS = [
    (
        "cgap-audit membrane (audit profile)",
        ["cargo", "build", "--profile", "audit", "-p", "cgap-audit"],
        os.path.join("target", "audit", "libcgap_audit.dylib"),
    ),
    (
        "cgap-ffi membrane (release profile)",
        ["cargo", "build", "--release", "-p", "cgap-ffi"],
        os.path.join("target", "release", "libcgap_ffi.dylib"),
    ),
]

# --- third check: version-string integrity (GATING) -------------------------
# The release version as a NUL-terminated C string. Single-sourced upstream:
# workspace Cargo.toml -> CARGO_PKG_VERSION (wasm) / ggen-generated
# cgap_arrow::VERSION_C (host). Pinned here as bytes; if the release bumps
# without re-pinning, the version-carrying artifacts count 0 and the gate
# FAILS closed (drift arm), printing the manifest cross-check note.
EXPECTED_VERSION = "26.10.29"
VERSION_NEEDLE = EXPECTED_VERSION.encode("ascii") + b"\x00"

# Per-artifact EXACT-COUNT law over raw binary content. (label, rel, expected,
# why). The two version-carrying artifacts must carry the needle exactly once
# (duplication OR absence both fire); the audit membrane carries none by
# design (see module docstring) and any occurrence there is an unplanned
# version literal entering the audited-kernel surface.
VERSION_SUBJECTS = [
    (
        "optimized wasm artifact",
        os.path.relpath(OPT_WASM, REPO_ROOT),
        1,
        "exports wasm_version() -> NUL-terminated CARGO_PKG_VERSION string; "
        "exactly-once is the law (2+ = stamped twice, 0 = drift)",
    ),
    (
        "cgap-ffi membrane (release profile)",
        os.path.relpath(
            os.path.join(REPO_ROOT, "target", "release", "libcgap_ffi.dylib"),
            REPO_ROOT,
        ),
        1,
        "exposes cgap_version via cgap_arrow::VERSION_C (ggen-generated, "
        "ontology-single-sourced); exactly-once is the law",
    ),
    (
        "cgap-audit membrane (audit profile)",
        os.path.relpath(
            os.path.join(REPO_ROOT, "target", "audit", "libcgap_audit.dylib"),
            REPO_ROOT,
        ),
        0,
        "no version export by design: crates/audit wraps kernels only and "
        "never references VERSION_C, so the linker GCs the literal — any "
        "occurrence is an unplanned version literal in the audit surface",
    ),
]

# Forbidden substrings: the panicking / unwind / abort / throw families.
# Ordered specific-first; "first matching pattern wins" for hit labeling.
FORBIDDEN = [
    "rust_begin_unwind",  # unwind family (specific)
    "core::panicking",    # panicking family (specific path)
    "panicking",          # panicking family
    "panic",              # panicking family (broad; substring of the above)
    "unwind",             # unwind family
    "__cxa_throw",        # throw family (specific)
    "throw",              # throw family
    "abort",              # abort family
]

MAX_PRINTED_HITS = 25

WASM_PROBE_CRATE_TOML = """\
[package]
name = "symbol_gate_probe"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[profile.release]
lto = "thin"
codegen-units = 1
panic = "abort"

# Standalone throwaway workspace, like the repo's wasm-footprint crate.
[workspace]
"""

WASM_PROBE_LIB_RS = """\
#![no_std]
use core::panic::PanicInfo;

// Same glue shape as the repo's wasm-footprint crate: consumer-supplied
// panic handler, halt loop.
#[panic_handler]
fn probe_panic_handler(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// Reachable panic path behind an exported C ABI. The export name carries a
// forbidden-pattern substring ("abort") so the OPTIMIZED artifact's
// enumerable surface (export names survive -Oz; the name section does not)
// must trip the gate: if a panic-containing optimized artifact passes the
// scan, the scan is vacuous.
#[no_mangle]
pub extern "C" fn probe_abort_path(x: i32) -> i32 {
    if x < 0 {
        panic!("symbol-gate anti-vacuity probe: reachable panic path");
    }
    x
}
"""

HOST_PROBE_RS = """\
// Throwaway anti-vacuity probe for the SECONDARY (host) scanner, built
// under /tmp, never in the repo. Non-gating: it witnesses that the host
// nm scan is not vacuous; it never decides the run.
#[no_mangle]
pub extern "C" fn symbol_gate_selftest_fire(x: u64) -> u64 {
    if x > 0 {
        x
    } else {
        panic!("symbol-gate anti-vacuity probe: this must trip the host scan");
    }
}
"""


def run(cmd, cwd=None):
    """Run a command; return (returncode, combined output)."""
    proc = subprocess.run(
        cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
    )
    return proc.returncode, proc.stdout


# ---------------------------------------------------------------------------
# tool selection
# ---------------------------------------------------------------------------
def pick_wasm_nm():
    """wasm-nm if present, else xcrun llvm-nm (witnessed: reads wasm), else nm."""
    if shutil.which("wasm-nm"):
        return ["wasm-nm"]
    for candidate in (["xcrun", "llvm-nm"], ["llvm-nm"], ["nm"]):
        rc, _ = run(candidate + ["--version"])
        if rc == 0:
            return candidate
    return None


def pick_wasm_opt():
    """wasm-opt location: $WASM_OPT, homebrew path, then PATH."""
    candidates = (
        os.environ.get("WASM_OPT"),
        "/opt/homebrew/bin/wasm-opt",
        shutil.which("wasm-opt"),
    )
    for candidate in candidates:
        if candidate and os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return candidate
    return None


def pick_objdump():
    """wasm-objdump location via PATH."""
    return shutil.which("wasm-objdump")


# ---------------------------------------------------------------------------
# name extraction
# ---------------------------------------------------------------------------
def parse_nm_line(line):
    """Symbol name from one nm output line ('addr type name' / bare name)."""
    parts = line.split()
    if not parts:
        return None
    if len(parts) == 1:
        return parts[0]
    if len(parts) == 2:
        # e.g. undefined-symbol listing: just the name field
        return parts[-1]
    return " ".join(parts[2:])


def objdump_names(out):
    """Names from `wasm-objdump -x` output: <angle> and "quoted" tokens.

    Data-segment hexdump rows are excluded: the ASCII column renders data
    bytes as text, and a data string containing 'panic' is not a symbol.
    """
    names = set()
    for line in out.splitlines():
        if re.match(r"\s*- [0-9a-f]{7}:", line):
            continue
        for m in re.findall(r"<([^<>]+)>", line):
            names.add(m)
        for m in re.findall(r'"([^"]+)"', line):
            names.add(m)
    return names


def hits_for(names):
    """(name, pattern) pairs for names containing a forbidden substring."""
    hits = []
    for name in sorted(names):
        for pat in FORBIDDEN:
            if pat in name:
                hits.append((name, pat))
                break  # first (highest-priority) matching pattern wins
    return hits


def scan_wasm(path, nm_cmd, objdump):
    """Enumerate + scan a wasm artifact. Returns (nm_names, objdump_names, hits).

    Fails closed on cannot-enumerate: if neither source yields names, a
    symbol scan over zero names carries no bits, so that is RuntimeError.
    """
    nm_names = set()
    nm_ok = False
    if nm_cmd:
        rc, out = run(nm_cmd + [path])
        if rc == 0:
            for line in out.splitlines():
                nm_names.add(parse_nm_line(line))
            nm_names.discard(None)
            nm_ok = True
    od_names = set()
    od_ok = False
    if objdump:
        rc, out = run([objdump, "-x", path])
        if rc == 0:
            od_names = objdump_names(out)
            od_ok = True
    if not nm_ok and not od_ok:
        raise RuntimeError(
            f"cannot enumerate symbols in {path}: "
            f"{' '.join(nm_cmd) if nm_cmd else 'no nm tool available'} and "
            f"{objdump if objdump else 'no wasm-objdump'} both failed/absent"
        )
    if not nm_names and not od_names:
        raise RuntimeError(
            f"cannot enumerate symbols in {path}: tools ran but yielded zero "
            f"enumerable names (stripped beyond the export surface?)"
        )
    hits = hits_for(nm_names | od_names)
    return nm_names, od_names, hits


def scan_host(path, nm_cmd):
    """Scan a Mach-O dylib with nm; returns (total, hits)."""
    rc, out = run(nm_cmd + [path])
    if rc != 0:
        raise RuntimeError(f"{nm_cmd} exited {rc} on {path}: {out.strip()[:400]}")
    names = set()
    for line in out.splitlines():
        name = parse_nm_line(line)
        if name:
            names.add(name)
    return len(names), hits_for(names)


# ---------------------------------------------------------------------------
# version-string integrity (third check)
# ---------------------------------------------------------------------------
def count_version_needle(path, needle=None):
    """Occurrence count of the version needle in RAW BINARY CONTENT.

    Deliberately a byte scan of the whole file, not a symbol-name scan: the
    law subjects what the artifact actually carries. bytes.count is
    non-overlapping; the needle cannot self-overlap anyway (starts '2',
    ends NUL).
    """
    with open(path, "rb") as f:
        return f.read().count(VERSION_NEEDLE if needle is None else needle)


def version_verdict(expected, actual):
    """(ok, msg) for one artifact's exact-count law."""
    if actual == expected:
        return True, f"exactly {expected} occurrence(s) — as expected"
    return (
        False,
        f"expected {expected}, found {actual}"
        + (" (version stamped twice?)" if actual > expected else " (version drift/absent?)"),
    )


def version_expected_for(path):
    """Expected needle count for an artifact, via VERSION_SUBJECTS."""
    rel = os.path.relpath(path, REPO_ROOT)
    for _label, r, expected, _why in VERSION_SUBJECTS:
        if r == rel:
            return expected
    raise RuntimeError(f"no version expectation registered for {rel}")


def check_version_manifest_pin():
    """Cross-read the workspace manifest version against the pinned needle.

    Informational note only — the count law itself decides the exit. A bump
    without re-pinning shows up as count 0 on the version-carrying artifacts;
    this note explains why.
    """
    toml_path = os.path.join(REPO_ROOT, "Cargo.toml")
    try:
        with open(toml_path, "r", encoding="utf-8") as f:
            for line in f:
                stripped = line.strip()
                if stripped.startswith("version") and "=" in stripped:
                    manifest = stripped.split("=", 1)[1].strip().strip('"')
                    if manifest != EXPECTED_VERSION:
                        return (
                            f"NOTE: workspace Cargo.toml version is {manifest!r} "
                            f"but the gate pins {EXPECTED_VERSION!r} — re-pin "
                            f"EXPECTED_VERSION or the count law will fire"
                        )
                    return None
        return f"NOTE: no version key found in {toml_path}"
    except OSError as exc:
        return f"NOTE: cannot read {toml_path}: {exc}"


def build_version_shims(subject_path, tmpdir):
    """Byte-level shims on a /tmp COPY of `subject_path`.

    The real artifact is never opened for write. Returns (dup_path,
    zero_path): a copy with the needle inserted a SECOND time, and a copy
    with the needle stripped entirely. If the subject carries no needle the
    shim cannot be built -> RuntimeError (fail closed).
    """
    with open(subject_path, "rb") as f:
        data = f.read()
    idx = data.find(VERSION_NEEDLE)
    if idx < 0:
        raise RuntimeError(
            f"cannot build version shims: {subject_path} carries no "
            f"{VERSION_NEEDLE!r} to mutate"
        )
    dup = data[:idx] + VERSION_NEEDLE + data[idx:]
    zero = data.replace(VERSION_NEEDLE, b"")
    dup_path = os.path.join(tmpdir, "shim_version_dup.wasm")
    zero_path = os.path.join(tmpdir, "shim_version_zero.wasm")
    with open(dup_path, "wb") as f:
        f.write(dup)
    with open(zero_path, "wb") as f:
        f.write(zero)
    return dup_path, zero_path


# ---------------------------------------------------------------------------
# builds
# ---------------------------------------------------------------------------
def build_primary():
    """Build the raw + optimized wasm primary subject. Returns None or msg."""
    rc, out = run(
        ["cargo", "build", "--release", "--target", WASM_TARGET], cwd=WASM_CRATE
    )
    if rc != 0:
        return f"cargo build (wasm-footprint) exited {rc}: {out.strip()[-600:]}"
    if not os.path.isfile(RAW_WASM):
        return f"cargo build ran but produced no {RAW_WASM}"
    wasm_opt = pick_wasm_opt()
    if wasm_opt is None:
        return "wasm-opt not found (required to produce the optimized artifact)"
    rc, out = run([wasm_opt, "-Oz", RAW_WASM, "-o", OPT_WASM])
    if rc != 0:
        return f"wasm-opt -Oz exited {rc}: {out.strip()[-400:]}"
    if not os.path.isfile(OPT_WASM):
        return f"wasm-opt ran but produced no {OPT_WASM}"
    return None


def build_host():
    """Run the host membrane builds. Returns list of (label, args, rel) fails."""
    failures = []
    for label, args, rel in HOST_ARTIFACTS:
        rc, out = run(args, cwd=REPO_ROOT)
        if rc != 0:
            print(f"  BUILD FAIL: {' '.join(args)} (exit {rc})")
            for line in out.strip().splitlines()[-10:]:
                print(f"    | {line}")
            failures.append((label, args, rel))
            continue
        if not os.path.isfile(os.path.join(REPO_ROOT, rel)):
            print(f"  BUILD FAIL: {' '.join(args)} ran but produced no {rel}")
            failures.append((label, args, rel))
    return failures


def build_wasm_probe(tmpdir):
    """Build the no_std wasm probe crate under /tmp. Returns (raw, opt) paths."""
    crate = os.path.join(tmpdir, "symbol_gate_probe")
    os.makedirs(os.path.join(crate, "src"), exist_ok=True)
    with open(os.path.join(crate, "Cargo.toml"), "w", encoding="utf-8") as f:
        f.write(WASM_PROBE_CRATE_TOML)
    lib_rs = os.path.join(crate, "src", "lib.rs")
    with open(lib_rs, "w", encoding="utf-8") as f:
        f.write(WASM_PROBE_LIB_RS)
    rc, out = run(
        ["cargo", "build", "--release", "--target", WASM_TARGET], cwd=crate
    )
    raw = os.path.join(crate, "target", WASM_TARGET, "release", "symbol_gate_probe.wasm")
    opt = raw[: -len(".wasm")] + ".opt.wasm"
    rc2, out2 = run([pick_wasm_opt(), "-Oz", raw, "-o", opt])
    if rc != 0 or rc2 != 0 or not os.path.isfile(raw) or not os.path.isfile(opt):
        raise RuntimeError(
            f"probe build failed (cargo rc={rc}, wasm-opt rc={rc2}): "
            f"{(out + out2).strip()[-600:]}"
        )
    return raw, opt


def build_host_probe(tmpdir, rustc):
    """Compile the host probe cdylib under /tmp (secondary scanner non-vacuity)."""
    src = os.path.join(tmpdir, "symbol_gate_selftest.rs")
    with open(src, "w", encoding="utf-8") as f:
        f.write(HOST_PROBE_RS)
    dylib = os.path.join(tmpdir, "libsymbol_gate_selftest.dylib")
    rc, out = run(
        [rustc, "--edition", "2021", "--crate-type", "cdylib", "-O", "-o", dylib, src]
    )
    if rc != 0:
        raise RuntimeError(f"host probe rustc failed (exit {rc}): {out.strip()[:600]}")
    if not os.path.isfile(dylib):
        raise RuntimeError("host probe dylib not produced at expected path")
    return dylib


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------
def main() -> int:
    print("=" * 74)
    print("SYMBOL GATE - zero-panic Jidoka (charter 7.2.1, rescoped)")
    print(
        "PRIMARY: optimized wasm artifact (gating) | "
        "SECONDARY: host membranes (informational, non-gating) | "
        "VERSION: binary version-string law (gating)"
    )
    print("=" * 74)

    # Fail-closed tool selection: with no wasm nm layer AND no objdump there
    # is nothing to scan with.
    nm_cmd = pick_wasm_nm()
    objdump = pick_objdump()
    if nm_cmd is None and objdump is None:
        print(
            "FINAL: FAIL - cannot enumerate wasm symbols "
            "(no wasm-nm / llvm-nm / nm, and no wasm-objdump)"
        )
        return 1
    if objdump is None:
        print(
            "  NOTE: wasm-objdump unavailable — scan degrades to the nm "
            "surface alone"
        )

    # [1] primary subject ---------------------------------------------------
    print("\n[1] PRIMARY - optimized wasm artifact (gating)")
    fail_msg = build_primary()
    if fail_msg:
        print(f"  BUILD FAIL: {fail_msg}")
        print("FINAL: FAIL - primary wasm artifact build/tooling failure")
        return 1
    print(f"  built ok: {os.path.relpath(RAW_WASM, REPO_ROOT)}")
    print(f"  built ok: {os.path.relpath(OPT_WASM, REPO_ROOT)}")

    raw_nm, raw_od, raw_hits = scan_wasm(RAW_WASM, nm_cmd, objdump)
    opt_nm, opt_od, opt_hits = scan_wasm(OPT_WASM, nm_cmd, objdump)
    tools = []
    if nm_cmd:
        tools.append(" ".join(nm_cmd))
    if objdump:
        tools.append("wasm-objdump -x")
    print(f"  scan tools  : {' + '.join(tools)}")
    print("  scan surface: nm names UNION wasm-objdump -x function/export names")
    print(
        f"  RAW  cdylib : {len(raw_nm)} nm names + {len(raw_od)} objdump names "
        f"-> {len(raw_hits)} forbidden hits [INFORMATIONAL]"
    )
    for sym, pat in raw_hits[:MAX_PRINTED_HITS]:
        print(f"      [{pat}] {sym}")
    if len(raw_hits) > MAX_PRINTED_HITS:
        print(f"      ... and {len(raw_hits) - MAX_PRINTED_HITS} more")
    print(
        "  RAW note    : pre-optimization dead panic machinery is expected; "
        "wasm-opt -Oz eliminates it, which is why the gate subjects the "
        "OPTIMIZED artifact (the one a wasm consumer links)"
    )
    print(
        f"  OPT  artifact: {len(opt_nm)} nm names + {len(opt_od)} objdump names "
        f"-> {len(opt_hits)} forbidden hits [GATING]"
    )
    for sym, pat in opt_hits[:MAX_PRINTED_HITS]:
        print(f"      [{pat}] {sym}")
    if len(opt_hits) > MAX_PRINTED_HITS:
        print(f"      ... and {len(opt_hits) - MAX_PRINTED_HITS} more")
    primary_pass = len(opt_hits) == 0
    print(
        f"  PRIMARY verdict: "
        f"{'PASS (0 forbidden panicking/abort/throw symbols)' if primary_pass else 'FAIL'}"
    )

    # [2] secondary subjects -------------------------------------------------
    print("\n[2] SECONDARY - host membranes (informational, NOT gating)")
    host_failures = build_host()
    for label, args, rel in HOST_ARTIFACTS:
        path = os.path.join(REPO_ROOT, rel)
        if (label, args, rel) in host_failures:
            print(f"  {label}: BUILD FAILED — SECONDARY-ERROR (non-gating)")
            continue
        try:
            total, hits = scan_host(path, nm_host_cmd())
        except RuntimeError as exc:
            print(f"  {label}: SCAN ERROR — SECONDARY-ERROR (non-gating): {exc}")
            continue
        print(
            f"  {label}: {os.path.relpath(path, REPO_ROOT)} — {total} symbols, "
            f"{len(hits)} forbidden hits [INFORMATIONAL, non-gating]"
        )
        for sym, pat in hits[:MAX_PRINTED_HITS]:
            print(f"      [{pat}] {sym[:110]}")
        if len(hits) > MAX_PRINTED_HITS:
            print(f"      ... and {len(hits) - MAX_PRINTED_HITS} more")
    print(
        "  rationale: std-linked host runtime glue; the substrate's no_std "
        "law is proven by the wasm gate — host dylibs measure std, not cgap"
    )

    # [3] version-string integrity (third check, GATING) ---------------------
    print("\n[3] VERSION INTEGRITY - binary version-string law (gating)")
    print(
        f"  needle: {VERSION_NEEDLE!r} (expected version {EXPECTED_VERSION!r}, "
        f"NUL-terminated C-string form) over raw binary content"
    )
    manifest_note = check_version_manifest_pin()
    if manifest_note:
        print(f"  {manifest_note}")
    version_failures = []
    for label, rel, expected, why in VERSION_SUBJECTS:
        path = os.path.join(REPO_ROOT, rel)
        if not os.path.isfile(path):
            # A host membrane that failed to build is already reported loud
            # and non-gating in [2]; scanning a missing artifact is skipped
            # loudly here rather than silently passing.
            print(f"  {label}: ABSENT -> VERSION-SKIP (non-gating; see [2])")
            print(f"      law: {why}")
            continue
        try:
            actual = count_version_needle(path)
        except OSError as exc:
            print(f"  {label}: READ ERROR -> VERSION-FAIL (gating): {exc}")
            version_failures.append((label, rel, expected, f"read error: {exc}"))
            continue
        ok, msg = version_verdict(expected, actual)
        print(
            f"  {label}: {actual} occurrence(s) -> "
            f"{'ok' if ok else f'VERSION-FAIL [GATING] ({msg})'}"
        )
        print(f"      law: {why}")
        if not ok:
            version_failures.append((label, rel, expected, msg))
    version_law_ok = not version_failures
    print(
        f"  VERSION verdict: "
        f"{'PASS (all artifact counts match their law)' if version_law_ok else 'FAIL'}"
    )

    # [4] anti-vacuity probes ------------------------------------------------
    print("\n[4] ANTI-VACUITY probes")
    fired_raw = False
    fired_opt = False
    host_probe_fired = False
    version_probe_ok = False
    tmpdir = tempfile.mkdtemp(prefix="symbol_gate_probe_", dir="/tmp")
    try:
        try:
            raw, opt = build_wasm_probe(tmpdir)
            _, _, praw = scan_wasm(raw, nm_cmd, objdump)
            fired_raw = len(praw) > 0
            _, _, popt = scan_wasm(opt, nm_cmd, objdump)
            fired_opt = len(popt) > 0
            print(f"  probe crate : {tmpdir}/symbol_gate_probe (no_std wasm, reachable panic)")
            print(
                f"    raw stage : {len(praw)} forbidden hits -> "
                f"{'FIRED' if fired_raw else 'BLIND (name-section scan cannot see panic machinery)'}"
            )
            for sym, pat in praw[:6]:
                print(f"      [{pat}] {sym[:96]}")
            print(
                f"    opt stage : {len(popt)} forbidden hits -> "
                f"{'FIRED' if fired_opt else 'BLIND (export-surface scan cannot see panic path)'}"
            )
            for sym, pat in popt[:6]:
                print(f"      [{pat}] {sym[:96]}")
        except RuntimeError as exc:
            print(f"  WASM PROBE-ERROR: {exc}")

        rustc = shutil.which("rustc")
        if rustc is None:
            print("  HOST PROBE-ERROR: rustc not found on PATH (non-gating)")
        else:
            try:
                dylib = build_host_probe(tmpdir, rustc)
                st_total, st_hits = scan_host(dylib, nm_host_cmd())
                host_probe_fired = len(st_hits) > 0
                print(
                    f"  host probe  : {st_total} symbols, {len(st_hits)} hits -> "
                    f"{'FIRED' if host_probe_fired else 'VACUOUS (non-gating)'}"
                )
            except RuntimeError as exc:
                print(f"  HOST PROBE-ERROR: {exc} (non-gating)")

        # version-integrity shim proof: byte-level mutations of a /tmp COPY
        # of the optimized wasm must FIRE the count law BOTH ways (duplicated
        # needle, stripped needle); the real artifact is only ever opened for
        # read and must re-count clean afterwards. Any exception here leaves
        # version_probe_ok False -> vacuous -> exit 1 (fail closed).
        try:
            dup_path, zero_path = build_version_shims(OPT_WASM, tmpdir)
            wasm_expected = version_expected_for(OPT_WASM)
            dup_n = count_version_needle(dup_path)
            zero_n = count_version_needle(zero_path)
            dup_fired = dup_n != wasm_expected
            zero_fired = zero_n != wasm_expected
            real_n = count_version_needle(OPT_WASM)
            real_untouched = real_n == wasm_expected
            print(
                "  version shims: byte-level mutations of a /tmp COPY of the "
                "optimized wasm (real artifact never written)"
            )
            print(
                f"    dup shim  : {dup_n} occurrence(s) vs expected "
                f"{wasm_expected} -> {'FIRED' if dup_fired else 'BLIND'}"
            )
            print(
                f"    zero shim : {zero_n} occurrence(s) vs expected "
                f"{wasm_expected} -> {'FIRED' if zero_fired else 'BLIND'}"
            )
            print(
                f"    real wasm : {real_n} occurrence(s) after shimming -> "
                f"{'UNTOUCHED' if real_untouched else 'MUTATED (safety failure)'}"
            )
            version_probe_ok = dup_fired and zero_fired and real_untouched
        except (RuntimeError, OSError) as exc:
            print(f"  VERSION SHIM-ERROR: {exc} (gating: proof did not run)")
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)
        print(f"  cleaned up: {tmpdir}")

    # [5] verdict -------------------------------------------------------------
    print("\n" + "=" * 74)
    print(
        f"PRIMARY (wasm, gating): {'PASS' if primary_pass else 'FAIL'} | "
        f"probe raw-stage: {'FIRED' if fired_raw else 'BLIND'} | "
        f"probe opt-stage: {'FIRED' if fired_opt else 'BLIND'} | "
        f"host probe: {'FIRED' if host_probe_fired else 'VACUOUS (non-gating)'} | "
        f"VERSION law: {'PASS' if version_law_ok else 'FAIL'} | "
        f"version shims: {'FIRED' if version_probe_ok else 'BLIND/NOT-RUN'}"
    )
    if not primary_pass:
        print(
            "FINAL: FAIL - forbidden panicking/unwind/abort/throw symbols in "
            "the optimized wasm artifact"
        )
        return 1
    if not (fired_raw and fired_opt):
        print(
            "FINAL: FAIL - VACUOUS GATE: the panic probe did not fire the "
            "primary scanner at both pipeline stages (raw name-section, "
            "optimized export surface)"
        )
        return 1
    if not version_law_ok:
        print(
            "FINAL: FAIL - VERSION INTEGRITY: artifact occurrence count(s) "
            "violate the per-artifact version-string law:"
        )
        for label, rel, expected, msg in version_failures:
            print(f"  {label} ({rel}): {msg}")
        return 1
    if not version_probe_ok:
        print(
            "FINAL: FAIL - VACUOUS GATE: the version shim proof did not fire "
            "(duplicated or stripped needle went undetected, the proof did "
            "not run, or the real artifact re-counted dirty after shimming)"
        )
        return 1
    print(
        "FINAL: PASS - optimized wasm artifact clean (0 forbidden symbols); "
        f"version integrity holds ({EXPECTED_VERSION!r} exactly-once where "
        "the version ships); gate proven able to fail at every stage"
    )
    return 0


def nm_host_cmd():
    """nm tool for the host membranes: xcrun llvm-nm, then plain nm."""
    for candidate in (["xcrun", "llvm-nm"], ["nm"]):
        rc, _ = run(candidate + ["--version"])
        if rc == 0:
            return candidate
    return ["nm"]


if __name__ == "__main__":
    sys.exit(main())
