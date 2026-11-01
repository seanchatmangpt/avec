#!/usr/bin/env python3
"""Zero-string gate (lane A07, ADR-002) — full implementation.

Spec: docs/jira/v26.11.1/lanes/A07-bosym-slab.md, "The Zero-String Gate"
(lines 172-309). The gate scans COMPILED ARTIFACTS (never sources) of every
workspace member crate enumerated mechanically from the root Cargo.toml,
extracts per-section raw bytes, sweeps printable runs (>= 8), UTF-8 decodes
them with errors="replace" (Wave-5 repair — strict decoding dropped runs
containing invalid sequences and hid present tokens; see T15), and refuses
(exit 1) when any ontology-vocabulary token
appears in a fail-set section (.rodata/.text family on ELF/PE; __TEXT
segment sections on Mach-O).

Vocabulary resolution order:
  1. --vocab PATH   (a .bosym v0 archive; the Rust generator in
                    crates/cgap-bosym/src/slab.rs emits the canonical one —
                    see the emit_real_vocabulary_archive_when_gated court)
  2. $CGAP_BOSYM_VOCAB
  3. <root>/gen/bosym/symbols.bosym
  4. derivation from <root>/ontologies/*/src/lib.rs `fibo:` tokens (the
     pre-archive path the lane doc lines 217-219 sanction)

Fail-closed contract (lane doc lines 240-255): the gate never exits 0 on an
error path. Missing tool, missing artifact, unreadable section, extractor
disagreement, vocabulary absent, unparsable vocabulary archive — all
GATE-REFUSED: <class> <subject>, exit 1. Any exception unwinds to a handler
emitting GATE-REFUSED: internal-error and exit 1.

Exit 0 requires ALL of: (a) >= 1 artifact scanned per enumerated, non-
excluded crate; (b) vocabulary loaded non-empty; (c) zero vocabulary hits
in fail-set sections; (d) zero refusals.

Anti-vacuity (lane doc lines 257-303): `--self-check` really builds a
poisoned fixture crate and a clean twin with the real toolchain in a temp
dir, scans both with the production predicate, and exits 0 only if the
poisoned arm HITs (naming the planted token) and the clean arm does not.
A predicate that does not fire on poison exits nonzero as VACUOUS GATE.
`--courts` runs the T4-T10 + T13-T16 court suite (skips are loud, never
faked); T13 is the anti-vacuity witness for `--dep-direction` (DoD 8);
T14 is the anti-vacuity witness for the A07 register (`--register`); T15
is the anti-vacuity witness for the Wave-5 invalid-UTF-8-neighbor repair;
T16 is the anti-vacuity witness for the Wave-7 producer exemption class.
Wave-6: each court is exception-isolated — a court that RAISES prints an
explicit `COURT <name> FAIL` row and the suite still emits COURTS-SUMMARY
and exits 1 (Wave-5 audit M-G1 finding: the plain strict-decode revert
used to abort the suite with GATE-REFUSED: internal-error before any
verdict line, diverging from T15's docstring).

WAVE-7 REPAIR (v1.3.0, 2026-11-01) — typed producer exemption class
(a07-wave6-audit-receipt.md §5 option (a), the gap that receipt assigned
to A07: "the scan-mode exemption gap is A07's to design"). The ontology
pack EMITTER `emit-fibo-pack` (package `cgap-fibo`, A09-owned) carries the
vocabulary it mints — ADR-002's producer role — so a full-workspace scan
could never go green while the producer's own binary embeds the strings.
The class is:
  - ARTIFACT-SCOPED: keyed (package, bin), never crate- or name-prefix —
    the pair form because ontologies/fibo/Cargo.toml records a witnessed
    bin-name collision between two lanes' emitters (A09 `emit-fibo-pack`
    vs A10's former `emit-pack`); a bare bin-name key would exempt BOTH
    packages' bins. The narrowest grant that fixes the finding.
  - LOUD, never silent: exempt artifacts are still scanned; their hits
    print as EMITTER-EXEMPT-HIT rows and count in SUMMARY emitter_hits=N,
    but do not feed the fail predicate (the milestone exit-gate wording
    scopes the quarantine claim to execution/consumption crates — the
    producer-side quarantine of the emitter's own strings remains open
    with A09, option (b), disclosed in the register divergences).
  - ANTI-BYPASS-fenced: no key may name a DOC_EXECUTION_FAMILY package;
    every key must resolve to a discovered artifact (unresolved key is a
    typed refusal `emitter-exempt-unresolved`, never a dead entry);
    growth is a justified reviewed diff pinned by T14a doc-parity and the
    T16 fence court.

WAVE-8 PINS (v1.4.0, 2026-11-01, defect repair driven by the wave-6/7
court receipts — a07-wave6-audit-receipt.md open item 1 and the
a08-wave7-audit-receipt.md method): the Wave-7 producer exemption class
landed unreceipted, and two of its laws had no mechanism: (1) the
"LOUD, never silent" law — T16 asserted bucketing through
scan_crate_artifacts RETURN VALUES only, so a renderer that swallowed
the EMITTER-EXEMPT-HIT rows / the emitter_hits SUMMARY field would pass
every court while the exemption went silent; (2) the no-dead-entries
law — `GATE-REFUSED: emitter-exempt-unresolved` had no firing witness
anywhere. Both laws are now pinned in-court by T16 arms 4 and 5: arm 4
drives the REAL renderer cmd_scan uses (extracted verbatim into
render_scan_report/summary_line) over the fixture aggregation and
fails unless the EMITTER-EXEMPT-HIT row (naming the planted token) and
the emitter_hits=N SUMMARY field actually render; arm 5 runs the REAL
cmd_scan over the two-bin producer fixture (must exit 1 WITH the control
bin's poison fatal — anti-bypass at the exit-code level — while the
producer's poison renders EMITTER-EXEMPT-HIT / emitter_hits=1) and over
the producer-less poison workspace (must raise
emitter-exempt-unresolved). Named flip classes: M-E1 (grant
emptied → arm 1 + arm 5 leg A), M-E2 (execution-family key → arm 3 +
fence refusal), M-E3 (doc parity broken → T14a), M-E4 (renderer
silenced → arm 4), M-E5 (key unresolvable → arm 5 leg B). All
five flips are witnessed in /tmp sandboxes (never the canonical tree).

WAVE-5 REPAIRS (v1.2.0, 2026-11-01), each with its flip witness:
  1. printable_runs false negative (T15): the strict UTF-8 decode DROPPED
     whole printable runs containing any invalid sequence —
     b"start-pad\\x80\\x81fibo:FndUtlCur:USD" produced NO runs and hid a
     present vocabulary token (witnessed live, Wave-5 session). Repaired
     to errors="replace": every valid sequence still decodes unchanged,
     so containment matches exactly the bytes present; replacement chars
     can never fabricate a match (a matched span carries no U+FFFD, hence
     maps to contiguous valid raw bytes encoding the token).
  2. A07 register (T14): the A20 §6.4 "dep audit register + allowlist"
     had no A07 surface on disk (A20 receipt divergence 6; boundary_gate
     ledger row `a07_register_deferral`). `--register` emits the
     canonical register as JSON; T14a pins its schema/doc-list parity,
     T14b proves its cloud predicate fires on a planted cloud string and
     stays silent on a clean twin.

The `--self-check` scan is byte-exactness-witnessed: every fixture
artifact is sha256'd before and after the scan; a mismatch fails the
self-check (the scan path must be read-only — shim-court restore
discipline, lane doc step 4).

This file was, in the Wave-1 ABI-freeze slice, a fail-closed stub; every
class below is now implemented. T1-T3 (archive framing courts) live in the
Rust codec (crates/cgap-bosym/src/slab.rs #[cfg(test)]).

`--dep-direction` (DoD 8, wave-4 hardening): the runtime half of the
dependency-direction proof. Every member crate's compiled artifacts are
scanned for out-of-band references — the `CGAPBSYM` magic, the canonical
archive path, the `cgap_bosym` symbol prefix — in fail-set sections
(Mach-O __TEXT / ELF .rodata/.text families / PE .rdata/.text). Any hit
is a DEP-DIR-VIOLATION and exits 1: a kernel binary carrying those bytes
would mean a runtime edge kernel→.bosym, which ADR-002 forbids without a
new ADR. cgap-bosym itself is the one exempt crate (it defines the
format); source-level references elsewhere are advisory lines, never a
pass/fail input.
"""

import argparse
import contextlib
import hashlib
import io
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

GATE_VERSION = "1.4.0"

# ---------------------------------------------------------------------------
# .bosym v0 framing constants — mirror of crates/cgap-bosym/src/slab.rs
# (ABI frozen there; this Python reader is the gate-side parser of the SAME
# wire. Framing divergence between the two is caught by the cross-
# implementation court: the canonical archive is Rust-generated.)
# ---------------------------------------------------------------------------

BOSYM_MAGIC = b"CGAPBSYM"
BOSYM_VERSION = 0
BOSYM_HEADER_SIZE = 16
BOSYM_ENTRY_PREFIX_SIZE = 14
BOSYM_SHA16_SIZE = 16
BOSYM_TRAILER_SIZE = 32
BOSYM_KINDS = {0, 1, 2, 3, 4}

MIN_RUN = 8  # printable-run minimum (lane doc line 205: "len >= 8")


class GateRefused(Exception):
    """Typed refusal; message is 'GATE-REFUSED: <class> <subject> ...'."""

    def __init__(self, reason_class: str, subject: str, detail: str = ""):
        self.reason_class = reason_class
        self.subject = subject
        msg = f"GATE-REFUSED: {reason_class} {subject}"
        if detail:
            msg += f" ({detail})"
        super().__init__(msg)


class VocabError(Exception):
    """Typed .bosym parse failure (gate-side reader)."""


# ---------------------------------------------------------------------------
# .bosym v0 reader (gate-side). Fail-closed; mirrors slab.rs::parse_archive.
# ---------------------------------------------------------------------------

def bosym_parse(data: bytes) -> list:
    """Parse a v0 archive to [(sym_id, kind, name)]; any deviation raises
    VocabError with the framing violation. Never returns partial results."""
    def fail(detail: str) -> None:
        raise VocabError(detail)

    if len(data) < BOSYM_HEADER_SIZE + BOSYM_TRAILER_SIZE:
        fail(f"truncated: have {len(data)}, need >= {BOSYM_HEADER_SIZE + BOSYM_TRAILER_SIZE}")
    if data[:8] != BOSYM_MAGIC:
        fail("bad magic")
    version = int.from_bytes(data[8:10], "little")
    if version != BOSYM_VERSION:
        fail(f"unsupported version {version} (v0 gate)")
    flags = int.from_bytes(data[10:12], "little")
    if flags & 0b11:
        fail(f"v0-forbidden flag bits set: 0x{flags:04x}")
    entry_count = int.from_bytes(data[12:16], "little")
    trailer_at = len(data) - BOSYM_TRAILER_SIZE
    if data[trailer_at:] != hashlib.sha256(data[:trailer_at]).digest():
        fail("trailing sha256 mismatch")
    body = data[BOSYM_HEADER_SIZE:trailer_at]
    pos = 0
    seen = set()
    entries = []
    for index in range(entry_count):
        if len(body) - pos < BOSYM_ENTRY_PREFIX_SIZE:
            fail(f"entry_count {entry_count} but only {index} entries decodable")
        sym_id, kind = struct.unpack_from("<QB", body, pos)
        reserved = body[pos + 9:pos + 12]
        name_len = struct.unpack_from("<H", body, pos + 12)[0]
        pos += BOSYM_ENTRY_PREFIX_SIZE
        if reserved != b"\x00\x00\x00":
            fail(f"entry {index}: reserved bytes non-zero")
        if kind not in BOSYM_KINDS:
            fail(f"entry {index}: invalid kind {kind}")
        if len(body) - pos < name_len + BOSYM_SHA16_SIZE:
            fail(f"entry {index}: truncated name/sha16")
        name_bytes = body[pos:pos + name_len]
        pos += name_len
        sha = body[pos:pos + BOSYM_SHA16_SIZE]
        pos += BOSYM_SHA16_SIZE
        if sha != hashlib.sha256(name_bytes).digest()[:BOSYM_SHA16_SIZE]:
            fail(f"entry {index}: sha16 mismatch")
        try:
            name = name_bytes.decode("utf-8", errors="strict")
        except UnicodeDecodeError:
            fail(f"entry {index}: name is not UTF-8")
        if sym_id in seen:
            fail(f"duplicate sym_id 0x{sym_id:016x}")
        seen.add(sym_id)
        entries.append((sym_id, kind, name))
    if pos != len(body):
        fail(f"{len(body) - pos} trailing bytes before sha256")
    return entries


def bosym_build(entries: list) -> bytes:
    """Build a v0 archive (used only by fixture courts). Mirror of
    slab.rs::build_archive."""
    body = bytearray()
    body += BOSYM_MAGIC
    body += BOSYM_VERSION.to_bytes(2, "little")
    body += (0).to_bytes(2, "little")
    body += len(entries).to_bytes(4, "little")
    for sym_id, kind, name in entries:
        nb = name.encode("utf-8")
        if len(nb) > 0xFFFF:
            raise VocabError("name too long")
        body += struct.pack("<QB", sym_id, kind)
        body += b"\x00\x00\x00"
        body += len(nb).to_bytes(2, "little")
        body += nb
        body += hashlib.sha256(nb).digest()[:BOSYM_SHA16_SIZE]
    digest = hashlib.sha256(bytes(body)).digest()
    body += digest
    return bytes(body)


# ---------------------------------------------------------------------------
# Vocabulary resolution.
# ---------------------------------------------------------------------------

def load_vocab_from_archive(path: Path) -> dict:
    """Returns {name: (sym_id, kind)}; unparsable archive fails closed."""
    data = path.read_bytes()
    try:
        entries = bosym_parse(data)
    except VocabError as e:
        raise GateRefused("vocab-unparsable", str(path), str(e))
    vocab = {}
    for sym_id, kind, name in entries:
        vocab[name] = (sym_id, kind)
    return vocab


FIBO_TOKEN_RE = re.compile(r"fibo:[A-Za-z0-9_]+(?::[A-Za-z0-9_]+)*")


def derive_vocab_from_graph(root: Path) -> dict:
    """Pre-archive vocabulary derivation (lane doc lines 217-219): the same
    graph export the generator consumes — the ontology facades at
    ontologies/*/src/lib.rs. Same token rule as slab.rs::extract_fibo_tokens."""
    vocab = {}
    sources = sorted(root.glob("ontologies/*/src/lib.rs"))
    if not sources:
        raise GateRefused("vocab-missing", f"{root}/ontologies/*/src/lib.rs",
                          "no ontology facades to derive from and no archive found")
    for src in sources:
        text = src.read_text(encoding="utf-8", errors="strict")
        for tok in FIBO_TOKEN_RE.findall(text):
            vocab[tok] = (0, 2)  # provisional id, kind=IRI
    return vocab


def resolve_vocabulary(root: Path, vocab_path) -> tuple:
    """Returns (vocab dict, source description)."""
    candidates = []
    if vocab_path is not None:
        candidates.append(Path(vocab_path))
    elif os.environ.get("CGAP_BOSYM_VOCAB"):
        candidates.append(Path(os.environ["CGAP_BOSYM_VOCAB"]))
    else:
        candidates.append(root / "gen/bosym/symbols.bosym")
    for cand in candidates:
        if cand is not None and cand.is_file():
            vocab = load_vocab_from_archive(cand)
            if not vocab:
                raise GateRefused("vocab-empty", str(cand))
            return vocab, f"archive:{cand}"
    # Fall through to graph derivation (only when no explicit --vocab/env
    # was requested AND the default archive path is absent).
    if vocab_path is not None or os.environ.get("CGAP_BOSYM_VOCAB"):
        raise GateRefused("vocab-missing", str(candidates[0]))
    vocab = derive_vocab_from_graph(root)
    if not vocab:
        raise GateRefused("vocab-missing", f"{root}/ontologies/*/src/lib.rs")
    return vocab, "graph-derivation:ontologies/*/src/lib.rs"


# ---------------------------------------------------------------------------
# Workspace enumeration (mechanical, from the root manifest — lane doc
# lines 180-183: "enumerated from Cargo.toml workspace members at gate
# runtime, not hardcoded ... the gate must not be bypassable by adding a
# crate").
# ---------------------------------------------------------------------------

def workspace_members(root: Path) -> list:
    text = (root / "Cargo.toml").read_text(encoding="utf-8", errors="strict")
    # A19 Wave-9 minimal unblock (cross-lane disclosure in BOTH receipts):
    # comments are stripped from the FULL manifest text BEFORE the bracket
    # capture, not after. The members array carries a scaffold comment whose
    # text itself contains `]` ("its own [workspace]"), so the non-greedy
    # capture previously stopped at that comment bracket and enumerated only
    # the 11 pre-moonshot members — cgap-fibo was never enumerated, its
    # (package, bin) producer-exemption key resolved to no artifact, and the
    # no-dead-entries law refused the whole scan (witnessed 2026-11-01:
    # `GATE-REFUSED: emitter-exempt-unresolved cgap-fibo:emit-fibo-pack`,
    # exit 1). Comment-stripping is behavior-preserving for manifests whose
    # members block has no bracketed comments; enumeration stays mechanical
    # from the root manifest (not-bypassable law unchanged).
    stripped = "\n".join(line.split("#", 1)[0] for line in text.splitlines())
    m = re.search(r"^members\s*=\s*\[(.*?)\]", stripped, re.S | re.M)
    if not m:
        raise GateRefused("workspace-manifest", str(root / "Cargo.toml"), "no members array")
    block = m.group(1)
    members = []
    for tok in re.findall(r'"([^"]+)"', block):
        members.append(root / tok)
    return members


def parse_crate_manifest(manifest: Path) -> dict:
    """Lite section-aware parse: package name, bin names, crate-type, and
    whether the crate has any lib/bin target at all (test-only crates are
    classified mechanically, never by name)."""
    text = manifest.read_text(encoding="utf-8", errors="strict")
    section = None
    pkg_name = None
    bins = []
    crate_types = []
    has_lib = False
    tests = []
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        sec = re.match(r"^\[+(.+?)\]+$", line)
        if sec:
            section = sec.group(1)
            continue
        kv = re.match(r'^(name|crate-type)\s*=', line)
        if kv:
            key = kv.group(1)
            rhs = line.split("=", 1)[1].strip()
            if key == "name":
                m2 = re.match(r'"([^"]*)"', rhs)
                val = m2.group(1) if m2 else None
                if section == "package" and val:
                    pkg_name = val
                elif section == "bin" and val:
                    bins.append(val)
                elif section == "test" and val:
                    tests.append(val)
            elif key == "crate-type":
                crate_types = re.findall(r'"([^"]*)"', rhs)
        if section == "lib":
            has_lib = True
    return {"name": pkg_name, "bins": bins, "crate_types": crate_types,
            "has_lib": has_lib, "tests": tests}


# Crate-level scan exclusions. Empty by design: the not-bypassable rule
# (default-scan every member) outranks convenience. An entry here is a
# reviewed diff and MUST carry a justification.
CRATE_EXCLUSIONS = {
    # "crate-name": "ALLOWLIST-JUSTIFICATION: ...",
}

# The lane doc's named execution family (lane doc lines 179-183). Presence
# assertion only: each of these that IS a workspace member must contribute
# at least one scanned artifact (structural anti-vacuity precondition,
# lane doc line 350-351). The scan set itself is ALL members minus
# CRATE_EXCLUSIONS, so an added crate is scanned, never bypassed.
# Wave-4 hardening: the doc's names "cli"/"integration"/"audit" do not
# match the real package names (cgap-cli/cgap-integration/cgap-audit), so
# the presence assertion never bit on them; the real names are added so
# the structural precondition holds on the actual workspace.
DOC_EXECUTION_FAMILY = {
    "cgap", "cgap-ffi", "cgap-arrow", "cgap-beam", "cgap-parquet",
    "cgap-git", "cgap-blake3", "cgap-gpu", "cli", "integration",
    "wasm-footprint",
    "cgap-cli", "cgap-integration", "cgap-audit",
}

# ---------------------------------------------------------------------------
# Producer exemption class (Wave-7 repair; a07-wave6-audit-receipt.md §5
# option (a)). The .bosym vocabulary PRODUCER — the emitter bin that mints
# the archive — legitimately embeds the vocabulary: ADR-002 splits the
# artifact family into a producer (build tooling) and consumers (execution
# crates), and the milestone exit-gate claim ("0 ontology UTF-8 strings in
# .rodata/.text") scopes to the CONSUMPTION path. Without this class the
# exit gate is permanently RED on any tree where the producer is built.
# ANTI-BYPASS laws (each pinned by a court):
#   (package, bin) pair keys — the fibo manifest's own wave-6 repair note
#     records a bin-name collision between two lanes' emitters; a bare
#     bin-name key would exempt both. Narrowest grant.
#   No DOC_EXECUTION_FAMILY package may appear as a key (T16 fence) —
#     execution crates can never hide behind a producer grant.
#   Every key must resolve to a discovered artifact at scan time, else
#     `GATE-REFUSED: emitter-exempt-unresolved` (no dead entries).
#   Growth is a reviewed diff with a mandatory ALLOWLIST-JUSTIFICATION,
#     doc-parity-pinned by T14a (EMITTER_DOC_NAMED).
# EMITTER_BINARIES maps ("package-name", "bin-artifact-name") -> justification
EMITTER_BINARIES = {
    ("cgap-fibo", "emit-fibo-pack"):
        "ALLOWLIST-JUSTIFICATION: ontology pack emitter, the ADR-002 "
        "PRODUCER role (mints the .bosym vocabulary archive; wave-6 audit "
        "§5 names it). Producer-side quarantine of the emitter's own "
        "strings remains open with A09 (audit option (b)); this grant "
        "scopes the exit-gate claim to the consumption path only",
}

# Receipt/doc-named producers — T14a pins register/doc parity in BOTH
# directions (an entry dropped from either side fails the court).
EMITTER_DOC_NAMED = (("cgap-fibo", "emit-fibo-pack"),)


def emitter_fence_violations() -> list:
    """Anti-bypass fence over the class itself: no key may name a
    DOC_EXECUTION_FAMILY package (execution crates can never inherit a
    producer grant). Returns [violation descriptions]; empty = fence holds.
    Wave-8 repair: the v1.3.0 comprehension unpacked `(p, _b)` while the
    f-string referenced `b` — the FIRST real fence violation raised
    NameError (observed live under mutation M-E2), so the typed
    `emitter-fence-violated` refusal class could never fire and T16's
    fence arm crashed instead of reporting. The fence channel is now
    witnessed live by M-E2 (courts) and M-E2b (scan-mode refusal)."""
    return [f"key {(p, b)!r} names execution-family package {p}"
            for (p, b) in EMITTER_BINARIES if p in DOC_EXECUTION_FAMILY]


def discover_artifacts(crate_dir: Path, pkg: str, bins: list, crate_types: list, target_dir: Path, profile: str) -> list:
    """Manifest-attributed artifacts for a crate. Binaries, cdylibs,
    staticlibs, rlibs. Returns list of Paths (existing files, sorted)."""
    out = set()
    prof = target_dir / profile
    deps = prof / "deps"
    pkg_us = pkg.replace("-", "_")
    # Bins: <profile>/<binname> (default bin name == package name).
    for b in (bins or [pkg]):
        for cand in (prof / b,):
            if cand.is_file():
                out.add(cand)
    if not crate_types or "lib" in crate_types or "rlib" in crate_types:
        top_rlib = prof / f"lib{pkg}.rlib"
        if top_rlib.is_file():
            out.add(top_rlib)
        if deps.is_dir():
            out.update(deps.glob(f"lib{pkg_us}-*.rlib"))
        # Nightly-cargo layout (witnessed on 1.101.0-nightly, this repo):
        # library units land under build/<pkg>/<hash>/out/ instead of deps/.
        build_layout = target_dir / profile / "build" / pkg
        for rlib in build_layout.glob(f"*/out/*.rlib"):
            stem = rlib.name[3:]  # strip lib
            if stem.startswith(pkg_us + "-") or stem.startswith(pkg_us + ".") or stem == pkg_us:
                out.add(rlib)
    for ct in crate_types:
        if ct in ("cdylib", "staticlib"):
            for ext in ("dylib", "so"):
                out.update(prof.glob(f"lib{pkg_us}.{ext}"))
                if deps.is_dir():
                    out.update(deps.glob(f"lib{pkg_us}-*.{ext}"))
    return sorted(out)


# ---------------------------------------------------------------------------
# Object-format detection and section enumeration.
# ---------------------------------------------------------------------------

MACHO_MAGICS = [b"\xfe\xed\xfa\xce", b"\xfe\xed\xfa\xcf", b"\xce\xfa\xed\xfe", b"\xcf\xfa\xed\xfe"]
FAT_MAGIC = b"\xca\xfe\xba\xbe"
ELF_MAGIC = b"\x7fELF"
PE_MAGIC = b"MZ"
WASM_MAGIC = b"\x00asm"


def detect_format(path: Path) -> str:
    with path.open("rb") as f:
        head = f.read(16)
    if head[:8] == FAT_MAGIC or head[:8] == FAT_MAGIC[::-1]:
        return "fat-macho"
    if head[:4] in MACHO_MAGICS:
        return "macho"
    if head[:4] == ELF_MAGIC:
        return "elf"
    if head[:2] == PE_MAGIC:
        return "pe"
    if head[:4] == WASM_MAGIC:
        return "wasm"
    return "unknown"


def run_tool(cmd: list, subject: Path, tool: str) -> str:
    try:
        proc = subprocess.run(cmd, capture_output=True, timeout=120)
    except FileNotFoundError:
        raise GateRefused("tool-missing", tool, f"required to scan {subject}")
    except subprocess.TimeoutExpired:
        raise GateRefused("tool-timeout", f"{tool} {subject}")
    if proc.returncode != 0:
        raise GateRefused("tool-failed", f"{tool} {subject}",
                          proc.stderr.decode("utf-8", "replace").strip()[:200])
    return proc.stdout.decode("utf-8", "replace")


def macho_sections(path: Path) -> list:
    """[(sectname, segname, file_offset, size)] via otool -l; objdump -h
    fallback when otool is absent."""
    if shutil.which("otool"):
        out = run_tool(["otool", "-l", str(path)], path, "otool")
        secs = []
        pat = re.compile(
            r"sectname\s+(\S+)\s*\n\s*segname\s+(\S+)\s*\n\s*addr\s+\S+\s*\n\s*size\s+(\S+)\s*\n\s*offset\s+(\d+)")
        for m in pat.finditer(out):
            name, seg, size_s, off_s = m.group(1), m.group(2), m.group(3), m.group(4)
            size = int(size_s, 16) if size_s.lower().startswith("0x") else int(size_s, 10)
            secs.append((name, seg, int(off_s), size))
        if secs:
            return secs
        raise GateRefused("sections-unreadable", str(path), "otool -l yielded no sections")
    out = run_tool(["objdump", "-h", str(path)], path, "objdump")
    secs = []
    for line in out.splitlines():
        cols = line.split()
        if len(cols) >= 6 and cols[0].isdigit():
            name, size_s, off_s = cols[1], cols[2], cols[5]
            try:
                secs.append((name, "__OBJDUMP", int(off_s, 16), int(size_s, 16)))
            except ValueError:
                continue
    if not secs:
        raise GateRefused("sections-unreadable", str(path), "objdump -h yielded no sections")
    return secs


def elf_or_pe_sections(path: Path, fmt: str) -> list:
    if not shutil.which("objdump"):
        raise GateRefused("tool-missing", "objdump", f"required to scan {fmt} artifact {path}")
    out = run_tool(["objdump", "-h", str(path)], path, "objdump")
    secs = []
    for line in out.splitlines():
        cols = line.split()
        if len(cols) >= 6 and cols[0].isdigit():
            try:
                # Idx Name Size VMA LMA File-off Algn
                secs.append((cols[1], fmt, int(cols[5], 16), int(cols[2], 16)))
            except ValueError:
                continue
    if not secs:
        raise GateRefused("sections-unreadable", str(path), "objdump -h yielded no sections")
    return secs


def select_sections(fmt: str, sections: list) -> tuple:
    """Returns (fail_set, advisory_set) of (name, offset, size)."""
    fail, adv = [], []
    if fmt == "macho":
        for name, seg, off, size in sections:
            entry = (f"{seg},{name}", off, size)
            if seg == "__TEXT":
                fail.append(entry)
            else:
                adv.append(entry)
    elif fmt == "elf":
        for name, _seg, off, size in sections:
            entry = (name, off, size)
            if (name == ".rodata" or name.startswith(".rodata.")
                    or name == ".text" or name.startswith(".text.")
                    or name == ".data.rel.ro" or name.startswith(".data.rel.ro.")):
                fail.append(entry)
            elif name == ".data" or name.startswith(".data."):
                adv.append(entry)
    elif fmt == "pe":
        for name, _seg, off, size in sections:
            entry = (name, off, size)
            if name in (".rdata", ".text") or name.startswith(".rdata."):
                fail.append(entry)
            elif name == ".data" or name.startswith(".data."):
                adv.append(entry)
    return fail, adv


# ---------------------------------------------------------------------------
# Printable-run extraction, strict UTF-8 decode, classification.
# ---------------------------------------------------------------------------

def printable_runs(section_bytes: bytes, min_len: int = MIN_RUN) -> list:
    """Maximal runs of bytes in [0x20..0x7E] U [0x80..0xFF], each decoded
    as UTF-8 with errors="replace"; runs shorter than min_len bytes are
    dropped. Deterministic; no external tool.

    WAVE-5 REPAIR (T15 witness): the previous strict decode DROPPED a
    whole run containing any invalid sequence — b"start-pad\\x80\\x81"
    + a planted vocabulary token produced NO runs at all (witnessed live,
    Wave-5 session), a false-negative channel against byte-noise-adjacent
    leaks. errors="replace" keeps every valid sequence decodable, so a
    token's own bytes (valid UTF-8 by definition) are found regardless of
    invalid neighbors. Replacement chars cannot fabricate a match: a
    matched span contains no U+FFFD, hence maps 1:1 to contiguous valid
    raw bytes encoding exactly the token."""
    runs = []
    start = None
    n = len(section_bytes)
    for i, b in enumerate(section_bytes):
        in_run = 0x20 <= b <= 0x7E or b >= 0x80
        if in_run and start is None:
            start = i
        elif not in_run and start is not None:
            if i - start >= min_len:
                runs.append((start, section_bytes[start:i]))
            start = None
    if start is not None and n - start >= min_len:
        runs.append((start, section_bytes[start:n]))
    decoded = []
    for off, raw in runs:
        text = raw.decode("utf-8", errors="replace")
        if len(text) >= min_len:
            decoded.append((off, text))
    return decoded


class AllowEntry:
    """One allowlist entry: exact string or anchored regex class, with a
    mandatory ALLOWLIST-JUSTIFICATION (T7). Growth is a reviewed diff."""

    def __init__(self, exact=None, regex=None, justification=""):
        self.exact = exact
        self.regex = re.compile(regex) if regex else None
        self.justification = justification
        if not justification.startswith("ALLOWLIST-JUSTIFICATION:"):
            raise ValueError(f"allowlist entry {exact or regex!r} lacks ALLOWLIST-JUSTIFICATION")

    def matches(self, s: str) -> bool:
        if self.exact is not None:
            return s == self.exact
        return self.regex.search(s) is not None


# Allowlist: legitimate non-ontology strings (lane doc lines 222-229). The
# failure predicate is vocabulary hits ONLY (lines 231-239), so the
# allowlist shapes the advisory report, never the exit code — but its
# discipline is enforced by courts T6/T7/T8 regardless.
ALLOWLIST = [
    AllowEntry(
        regex=r"^called [`']?Option::unwrap\(\)[`']? on a [`']?None[`']? value",
        justification="ALLOWLIST-JUSTIFICATION: core/std panic message, present in every std-using binary"),
    AllowEntry(
        regex=r"^byte index \d+ is (not a char boundary|out of bounds)",
        justification="ALLOWLIST-JUSTIFICATION: core str-slicing panic message class"),
    AllowEntry(
        regex=r"^_ZN[a-zA-Z0-9_]+$",
        justification="ALLOWLIST-JUSTIFICATION: legacy Rust mangled symbol names"),
    AllowEntry(
        regex=r"\.rs$",
        justification="ALLOWLIST-JUSTIFICATION: panic-location source-file paths (…/crate/src/lib.rs)"),
    AllowEntry(
        regex=r"^rustc version",
        justification="ALLOWLIST-JUSTIFICATION: compiler-ident strings embedded by std build scripts"),
    AllowEntry(
        regex=r"^src/",
        justification="ALLOWLIST-JUSTIFICATION: panic Location paths relative to crate root"),
]


def find_vocab_token(text: str, vocab: dict):
    """Substring containment, vocabulary-first. Returns the matched token
    or None.

    DIVERGENCE (recorded, lane receipt): the lane doc line 390 states an
    "exact-match vocabulary" mitigation. The exact-run-match predicate was
    witnessed to produce a FALSE NEGATIVE on this toolchain: rustc/LLVM
    merges adjacent string constants into one printable run
    ("tok1tok2..." with no separator), so the token is present but never
    equals the run. The exit-gate wording ("0 ontology UTF-8 strings in
    .rodata/.text") is a containment claim; the predicate is therefore
    substring containment. The witnessing fixture:
    self-check poison arm, __TEXT,__const, run
    'fibo:FndUtlCur:USDfibo:FndUtlCur:USD\\x00tail'."""
    for tok in sorted(vocab):
        if tok in text:
            return tok
    return None


# ---------------------------------------------------------------------------
# The A07 register (A20 §6.4 contract: "dep audit register + allowlist |
# A07 | gate inputs | A20 consumes, never edits"). Canonical home of the
# forbidden-string / dep-audit data surface. Enforcement stays with the
# consumer gates (boundary_gate.py --deps/--strings); this is the input
# they seal against, so no consumer maintains a second list (O(N^2) drift
# law). Anti-vacuity: courts T14a/T14b.
# ---------------------------------------------------------------------------

A07_REGISTER_VERSION = 2

# The §6.4 doc-named cloud-vendor strings (A20 lane doc §6.4, verbatim
# list) — pinned by court T14a so register/doc drift is caught, and by the
# anchored patterns below (the s3.<host> tightening is the A20
# receipt-witnessed form: the bare `s3.` prefix false-positived on
# `s3.verify(&key)`).
A20_6_4_DOC_LIST = ("aws", "azure", "gcp", "s3.", "blob.core", "amazonaws")

CLOUD_STRING_PATTERNS = (
    (r"\baws\b", "aws"),
    (r"\bazure\b", "azure"),
    (r"\bgcp\b", "gcp"),
    (r"\bs3\.[A-Za-z0-9-]+\.[A-Za-z0-9.-]", "s3.<host>"),
    (r"blob\.core", "blob.core"),
    (r"amazonaws", "amazonaws"),
)

# Canonical A07 allowlists. EMPTY BY DESIGN (fail-closed: every hit is a
# hit); growth is a reviewed diff and each entry MUST carry an
# ALLOWLIST-JUSTIFICATION (T14a/T7 discipline).
CLOUD_STRING_ALLOWLIST = ()
DEP_EXTERNAL_ALLOWANCES = {}


def a07_register() -> dict:
    """The register as data. References the LIVE module objects (patterns,
    allowlists) at call time — the register is a projection, never a copy,
    so it cannot drift from the enforced lists."""
    return {
        "register": "a07-zero-string-dep-audit",
        "version": A07_REGISTER_VERSION,
        "owned_by": "lane A07 (v26.11.1) — scripts/zero_string_gate.py",
        "consumers": [
            "scripts/boundary_gate.py --deps/--strings (A20 §6.4): "
            "consumes, never edits"
        ],
        "single_list_law": (
            "A07 is the canonical home of the dep-audit register and "
            "allowlists; consumer gates re-derive nothing and maintain no "
            "second list"),
        "dep_audit": {
            "method": "docs/jira/v26.10.31/82-dependency-audit.md",
            "runtime_external_rule": (
                "a [dependencies] entry is EXTERNAL iff it declares a "
                "registry version, a git source, or any source other than "
                "path/workspace; dev-dependencies are not runtime"),
            "external_dep_allowances": DEP_EXTERNAL_ALLOWANCES,
            # ALLOWLIST-JUSTIFICATION: entries above; empty by design (E=0 law)
        },
        "cloud_strings": {
            "a20_6_4_doc_list": list(A20_6_4_DOC_LIST),
            "patterns": [[p, tag] for p, tag in CLOUD_STRING_PATTERNS],
            "allowlist": [
                {"match": e.exact if e.exact is not None else e.regex.pattern,
                 "justification": e.justification}
                for e in CLOUD_STRING_ALLOWLIST
            ],
        },
        "ontology": {
            "vocab_resolution_order": [
                "--vocab PATH (.bosym v0 archive, Rust-generator canonical)",
                "$CGAP_BOSYM_VOCAB",
                "<root>/gen/bosym/symbols.bosym",
                "ontologies/*/src/lib.rs fibo: token derivation",
            ],
            "allowlist": [
                {"match": e.exact if e.exact is not None else e.regex.pattern,
                 "justification": e.justification}
                for e in ALLOWLIST
            ],
        },
        "emitter_exemptions": {
            "scope_rule": (
                "scan-mode exemption for ADR-002 PRODUCER-role artifacts "
                "(the .bosym vocabulary emitter); keyed (package, bin); "
                "exempt artifacts are still scanned and their hits are "
                "reported as EMITTER-EXEMPT-HIT rows (loud), but do not "
                "feed the fail predicate; no DOC_EXECUTION_FAMILY package "
                "may hold a key (T16 fence); unresolved keys are a typed "
                "refusal, never dead entries"),
            "doc_named": [list(k) for k in EMITTER_DOC_NAMED],
            "entries": [
                {"package": p, "artifact": b, "justification": j}
                for (p, b), j in EMITTER_BINARIES.items()
            ],
        },
        "divergences": [
            "boundary_gate.py still carries an inline copy of the cloud "
            "patterns + the `a07_register_deferral` ledger row; that file "
            "is A20-owned (A20 consumes, never A07) — the copy is the "
            "derived one awaiting its consumption wiring, recorded here "
            "instead of drifting silently",
            "Wave-7: the producer exemption class is the audit's option "
            "(a); option (b) — quarantining the emitter's own strings "
            "through the .bosym projection — remains open with A09 "
            "(ontologies/fibo is A09-owned); until then emit-fibo-pack "
            "legitimately embeds the vocabulary and the exit-gate claim "
            "scopes to the consumption path",
        ],
    }


def cloud_scan_text(text: str, allowlist=None) -> list:
    """The register's cloud-string predicate over one text (file body or
    source line set). Returns [(tag, line_no, line)]. Every hit is a hit:
    the A07 cloud allowlist is empty by design, and a consumer passing a
    non-empty allowlist gets ALLOWED rows suppressed per entry (matches =
    exact string or anchored regex, same grammar as the ontology
    allowlist)."""
    compiled = [(re.compile(p), tag) for p, tag in CLOUD_STRING_PATTERNS]
    allow = list(CLOUD_STRING_ALLOWLIST if allowlist is None else allowlist)
    hits = []
    for lineno, line in enumerate(text.split("\n"), 1):
        allowed = any(e.matches(line) for e in allow)
        if allowed:
            continue
        for rx, tag in compiled:
            if rx.search(line):
                hits.append((tag, lineno, line))
    return hits


def classify(text: str, vocab: dict, allowlist=None) -> tuple:
    """('HIT', token) | ('ALLOWED', None) | ('UNKNOWN', None). Vocabulary
    outranks the allowlist (T8): the vocab check is FIRST, unconditionally,
    under substring containment."""
    tok = find_vocab_token(text, vocab)
    if tok is not None:
        return ("HIT", tok)
    allowlist = ALLOWLIST if allowlist is None else allowlist
    for entry in allowlist:
        if entry.matches(text):
            return ("ALLOWED", None)
    return ("UNKNOWN", None)


def strings_tool_hits(path: Path) -> set:
    """Second extractor: strings(1), ASCII printable runs >= MIN_RUN.
    Returns the set of extracted strings."""
    if not shutil.which("strings"):
        raise GateRefused("tool-missing", "strings", f"cross-check extractor for {path}")
    out = run_tool(["strings", "-a", "-n", str(MIN_RUN), str(path)], path, "strings")
    return set(out.splitlines())


# ---------------------------------------------------------------------------
# ar-archive (rlib) member extraction. An .rlib is an ar archive of object
# files + lib.rmeta; the section scan must descend into object members.
# ---------------------------------------------------------------------------

AR_MAGIC = b"!<arch>\n"


def ar_members(path: Path) -> list:
    """Parse an ar archive into [(member_name, member_bytes)]. Handles the
    BSD `#1/<len>` extended-name form and the GNU `//` string table.
    Malformed framing raises GateRefused (fail-closed)."""
    data = path.read_bytes()
    if not data.startswith(AR_MAGIC):
        raise GateRefused("unknown-format", str(path), "ar framing corrupt")
    pos = 8
    members = []
    longnames = b""
    while pos + 60 <= len(data):
        hdr = data[pos:pos + 60]
        name = hdr[0:16].decode("ascii", "replace").rstrip()
        try:
            size = int(hdr[48:58].decode("ascii").strip())
        except ValueError:
            raise GateRefused("unknown-format", str(path), f"ar member header corrupt at {pos}")
        body = data[pos + 60:pos + 60 + size]
        if len(body) != size:
            raise GateRefused("unknown-format", str(path), "ar member truncated")
        pos += 60 + size + (size % 2)
        if name == "//":
            longnames = body
            continue
        if name in ("/", "/SYM64/"):
            continue  # GNU symbol-index members
        if name.startswith("#1/"):
            try:
                namelen = int(name[3:])
            except ValueError:
                raise GateRefused("unknown-format", str(path), f"ar extended name corrupt: {name!r}")
            name = body[:namelen].split(b"\x00")[0].decode("utf-8", "replace")
            body = body[namelen:]
        elif name.startswith("/") and name[1:].isdigit():
            off = int(name[1:])
            end = longnames.find(b"\n", off)
            name = longnames[off:end].decode("utf-8", "replace") if end >= 0 else name
        # Symbol-index members resolve via #1/<len> on BSD ar — filter on
        # the RESOLVED name.
        if name == "" or name.startswith("__.SYMDEF"):
            continue
        members.append((name, body))
    return members


def detect_format_bytes(head: bytes) -> str:
    """Byte-level twin of detect_format for in-memory ar members."""
    if head[:8] == FAT_MAGIC or head[:8] == FAT_MAGIC[::-1]:
        return "fat-macho"
    if head[:4] in MACHO_MAGICS:
        return "macho"
    if head[:4] == ELF_MAGIC:
        return "elf"
    if head[:2] == PE_MAGIC:
        return "pe"
    if head[:4] == WASM_MAGIC:
        return "wasm"
    if head[:8] == AR_MAGIC:
        return "ar"
    return "unknown"


# ---------------------------------------------------------------------------
# Per-artifact scan.
# ---------------------------------------------------------------------------

def _scan_single(path: Path, label: str, vocab, cross_check: bool, mode: str = "vocab") -> dict:
    fmt = detect_format(path)
    if fmt == "fat-macho":
        raise GateRefused("unknown-format", label, "fat/universal binary — thin it first")
    if fmt == "wasm":
        return {"scanned": False, "hits": [], "adv": [], "unknown": 0, "allowed": 0,
                "skip": "wasm data-section scanning out of v1 scope (lane doc lines 412-415)",
                "violations": []}
    if fmt == "unknown":
        raise GateRefused("unknown-format", label,
                          "not ELF/Mach-O/PE/wasm/ar; refusing rather than guessing sections")
    if fmt == "macho":
        sections = macho_sections(path)
    else:
        sections = elf_or_pe_sections(path, fmt)
    fail_set, adv_set = select_sections(fmt, sections)
    hits, adv_hits = [], []
    unknown = allowed = 0
    raw_hit_tokens = set()
    violations = []
    with path.open("rb") as f:
        for name, off, size in fail_set + adv_set:
            if size <= 0:
                continue
            f.seek(off)
            data = f.read(size)
            if len(data) != size:
                raise GateRefused("section-unreadable", f"{label}::{name}",
                                  f"requested {size} at {off}, got {len(data)}")
            is_fail_section = any(n == name and o == off and s == size for n, o, s in fail_set)
            if mode == "depdir":
                for pat in BOSYM_REF_PATTERNS:
                    start = 0
                    while True:
                        idx = data.find(pat, start)
                        if idx < 0:
                            break
                        rec = (label, name, off + idx, pat.decode("ascii"))
                        if is_fail_section:
                            violations.append(rec)
                        else:
                            hits.append(rec)  # adv bucket carries non-fail-section refs
                        start = idx + 1
                continue
            for rel_off, text in printable_runs(data):
                cls, tok = classify(text, vocab)
                if cls == "HIT":
                    line = (label, name, off + rel_off, text)
                    if is_fail_section:
                        hits.append(line)
                        raw_hit_tokens.add(tok)
                    else:
                        adv_hits.append(line)
                elif cls == "ALLOWED":
                    allowed += 1
                else:
                    unknown += 1
    if mode == "depdir":
        return {"scanned": True, "violations": violations, "adv": hits, "skip": None}
    if cross_check and raw_hit_tokens:
        sset = strings_tool_hits(path)
        for tok in raw_hit_tokens:
            if tok.isascii() and not any(tok in line for line in sset):
                raise GateRefused("extractor-mismatch", f"{label} :: {tok}",
                                  "raw-bytes scan found an ASCII vocabulary hit that strings(1) did not")
    return {"scanned": True, "hits": hits, "adv": adv_hits,
            "unknown": unknown, "allowed": allowed, "skip": None,
            "tokens": raw_hit_tokens}


# ---------------------------------------------------------------------------
# Dependency direction (DoD 8): runtime check that no execution-crate
# artifact carries out-of-band references. A kernel binary containing the
# archive magic, the canonical archive path, or the cgap_bosym symbol
# prefix in a fail-set section is a runtime edge kernel->.bosym, which
# ADR-002 forbids without a new ADR (lane doc lines 134-137).
# ---------------------------------------------------------------------------

BOSYM_REF_PATTERNS = (
    b"CGAPBSYM",       # the archive magic itself
    b"symbols.bosym",  # the canonical archive filename
    b"cgap_bosym",     # the Rust crate's symbol prefix
    b"cgap-bosym",     # the package name (panic location paths, manifests)
)

# The quarantine owner legitimately defines the format and carries its own
# constants; every other member is checked. Reviewed-diff growth only.
BOSYM_REFERENCE_EXEMPT = {
    "cgap-bosym": "ALLOWLIST-JUSTIFICATION: the quarantine owner defines the "
                  "format; its own objects legitimately carry the magic "
                  "constant and its crate name",
}


def dep_dir_scan_artifact(path: Path) -> dict:
    """Dep-direction scan of one artifact (same descent rules as
    scan_artifact: native objects directly, ar members individually,
    .rmeta skipped loudly, unknown formats REFUSED)."""
    try:
        with path.open("rb") as f:
            head = f.read(8)
    except FileNotFoundError:
        raise GateRefused("artifact-vanished", str(path),
                          "discovered artifact disappeared before scan (concurrent cargo build?)")
    if head == AR_MAGIC:
        members = ar_members(path)
        agg = {"scanned": False, "violations": [], "adv": [], "skip": None}
        skipped = []
        scanned_members = 0
        with tempfile.TemporaryDirectory(prefix="a07_ardd.") as td:
            tdpath = Path(td)
            for name, body in members:
                if name.endswith(".rmeta"):
                    skipped.append(name)
                    continue
                mfmt = detect_format_bytes(body[:16])
                if mfmt == "unknown":
                    raise GateRefused("unknown-format", f"{path}::{name}",
                                      "ar member is not ELF/Mach-O/PE; refusing rather than guessing")
                mpath = tdpath / name.replace("/", "_")
                mpath.write_bytes(body)
                res = _scan_single(mpath, f"{path}::{name}", None, cross_check=False, mode="depdir")
                if res["scanned"]:
                    scanned_members += 1
                    agg["violations"].extend(res["violations"])
                    agg["adv"].extend(res["adv"])
        agg["scanned"] = scanned_members > 0
        if skipped:
            print(f"[SKIP] {path} members=({','.join(skipped)}) — rustc metadata member, not an object", file=sys.stderr)
        return agg
    return _scan_single(path, str(path), None, cross_check=False, mode="depdir")


def scan_artifact(path: Path, vocab, cross_check: bool = True, mode: str = "vocab") -> dict:
    """Scan one artifact: native objects directly; ar archives (rlib) by
    descending into object members (lib.rmeta members are skipped loudly,
    counted, never faked as scans). Dep-direction mode dispatches to the
    dedicated scanner (no vocabulary, pattern search over fail-set
    sections)."""
    if mode == "depdir":
        return dep_dir_scan_artifact(path)
    try:
        with path.open("rb") as f:
            head = f.read(8)
    except FileNotFoundError:
        raise GateRefused("artifact-vanished", str(path),
                          "discovered artifact disappeared before scan (concurrent cargo build?)")
    if head == AR_MAGIC:
        members = ar_members(path)
        agg = {"scanned": False, "hits": [], "adv": [], "unknown": 0, "allowed": 0, "skip": None,
               "tokens": set()}
        skipped = []
        scanned_members = 0
        with tempfile.TemporaryDirectory(prefix="a07_ar.") as td:
            tdpath = Path(td)
            for name, body in members:
                if name.endswith(".rmeta"):
                    skipped.append(name)
                    continue
                mfmt = detect_format_bytes(body[:16])
                if mfmt == "unknown":
                    raise GateRefused("unknown-format", f"{path}::{name}",
                                      "ar member is not an object; refusing rather than guessing")
                mpath = tdpath / name.replace("/", "_")
                mpath.write_bytes(body)
                res = _scan_single(mpath, f"{path}::{name}", vocab, cross_check=False)
                if res["scanned"]:
                    scanned_members += 1
                    agg["hits"].extend(res["hits"])
                    agg["adv"].extend(res["adv"])
                    agg["unknown"] += res["unknown"]
                    agg["tokens"].update(res.get("tokens", set()))
                agg["allowed"] += res["allowed"]
        # Wave-4 hardening: the strings(1) cross-extractor pass is hoisted
        # out of the member loop — one pass over the archive verifies every
        # member hit; the previous code re-ran strings(1) once per hit
        # token per member (O(hits x members) subprocess spawns).
        if agg["tokens"]:
            sset = strings_tool_hits(path)
            for s in agg["tokens"]:
                if s.isascii() and not any(s in line for line in sset):
                    raise GateRefused("extractor-mismatch", f"{path} :: {s}",
                                      "member hit missing from strings(1) over the archive")
        agg["scanned"] = scanned_members > 0
        if skipped:
            print(f"[SKIP] {path} members=({','.join(skipped)}) — rustc metadata member, not an object", file=sys.stderr)
        return agg
    return _scan_single(path, str(path), vocab, cross_check)


# ---------------------------------------------------------------------------
# Workspace scan (normal mode).
# ---------------------------------------------------------------------------

def enumerate_scan_set(root: Path, vocab_path, target_dir: Path, profile: str, load_vocab: bool = True) -> tuple:
    """Mechanical enumeration shared by scan and dep-direction modes:
    vocabulary, member manifests, reviewed exclusions, artifact discovery.
    Returns (vocab, vocab_source, crates, artifacts); vocab is (None, None)
    when load_vocab is False. Emits the VOCAB/ADV/EXCLUDED lines itself so
    both modes report identically."""
    if load_vocab:
        vocab, vocab_source = resolve_vocabulary(root, vocab_path)
        print(f"VOCAB source={vocab_source} tokens={len(vocab)}")
        for tok in sorted(vocab):
            if len(tok) < MIN_RUN:
                print(f"ADV vocab-shorter-than-min-run={MIN_RUN} token={tok!r} — invisible to the scan predicate")
    else:
        vocab, vocab_source = None, None
    members = workspace_members(root)
    crates = []
    for cdir in members:
        manifest = cdir / "Cargo.toml"
        if not manifest.is_file():
            raise GateRefused("member-manifest-missing", str(manifest))
        info = parse_crate_manifest(manifest)
        if info["name"] is None:
            raise GateRefused("member-manifest-missing", str(manifest), "no [package] name")
        if info["name"] in CRATE_EXCLUSIONS:
            print(f"EXCLUDED crate={info['name']} {CRATE_EXCLUSIONS[info['name']]}")
            continue
        # Mechanical test-only classification: a crate with no [lib], no
        # [[bin]], and at least one [[test]] target produces no cargo-build
        # artifacts by construction; excluding it is not a name-based
        # dodge (any lib/bin crate can never hide behind this rule).
        if not info["has_lib"] and not info["bins"] and info["tests"]:
            print(f"EXCLUDED crate={info['name']} ALLOWLIST-JUSTIFICATION: "
                  "mechanical test-only classification (no [lib]/[[bin]] targets; "
                  "test-binary scanning is gate v2 scope)")
            continue
        crates.append((cdir, info))
    artifacts = {}
    for cdir, info in crates:
        artifacts[info["name"]] = discover_artifacts(
            cdir, info["name"], info["bins"], info["crate_types"], target_dir, profile)
    return vocab, vocab_source, crates, artifacts


def scan_crate_artifacts(pkg: str, artifacts: list, vocab, cross_check: bool = True) -> dict:
    """Scan one crate's discovered artifacts through the production
    predicate, applying the Wave-7 producer exemption class: an artifact
    whose (package, artifact-name) key is in EMITTER_BINARIES is still
    fully scanned, but its vocabulary hits are bucketed as
    emitter_hits (reported, non-fatal) instead of fail-set hits. Also
    records which exemption keys resolved (no dead entries)."""
    out = {"scanned": 0, "skip": 0, "hits": [], "emitter_hits": [],
           "emitter_resolved": set(), "adv": [], "unknown": 0, "allowed": 0}
    for art in artifacts:
        key = (pkg, art.name)
        if key in EMITTER_BINARIES:
            out["emitter_resolved"].add(key)
        try:
            res = scan_artifact(art, vocab, cross_check=cross_check)
        except GateRefused as e:
            if e.reason_class == "artifact-vanished":
                out["skip"] += 1
                print(f"[SKIP] {art} — vanished mid-run (concurrent cargo build)", file=sys.stderr)
                continue
            raise
        if res["skip"]:
            out["skip"] += 1
            print(f"[SKIP] {art} — {res['skip']}")
            continue
        out["scanned"] += 1
        if key in EMITTER_BINARIES:
            out["emitter_hits"].extend(res["hits"])
        else:
            out["hits"].extend(res["hits"])
        out["adv"].extend(res["adv"])
        out["unknown"] += res["unknown"]
        out["allowed"] += res["allowed"]
    return out


def render_scan_report(per_crate_scanned: dict, hits: list, emitter_hits: list, adv_hits: list) -> None:
    """Render the per-crate and hit rows of the scan report. cmd_scan and
    the T16 loudness pin (Wave-8) share this one renderer, so a renderer
    that swallows a row class (the M-E4 silent-exemption mutation) flips
    the court that pins the loudness law — the law has a mechanism, not
    just prose (module docstring, WAVE-8 PINS)."""
    for name in sorted(per_crate_scanned):
        print(f"PER-CRATE {name} scanned={per_crate_scanned[name]}")
    for (p, sec, off, s) in hits:
        print(f"HIT {p} {sec} {off} {s}")
    for (p, sec, off, s) in emitter_hits:
        print(f"EMITTER-EXEMPT-HIT {p} {sec} {off} {s}")
    for (p, sec, off, s) in adv_hits:
        print(f"ADV-HIT(non-fail-section) {p} {sec} {off} {s}")


def summary_line(total_scanned: int, crate_count: int, hits: list, emitter_hits: list,
                 adv_hits: list, total_unknown: int, total_allowed: int, total_skip: int,
                 vocab: dict, vocab_source: str) -> str:
    """Build (not print) the SUMMARY line — same fields, same order, as
    the pre-Wave-8 inline print in cmd_scan. Shared with T16 arm 4 so the
    emitter_hits=N field is pinned at the output boundary too."""
    return (f"SUMMARY scanned={total_scanned} crates={crate_count} hits={len(hits)} "
            f"emitter_hits={len(emitter_hits)} adv_hits={len(adv_hits)} unknown={total_unknown} "
            f"allowed={total_allowed} skipped_artifacts={total_skip} vocab={len(vocab)} "
            f"source={vocab_source} gate={GATE_VERSION}")


def cmd_scan(args) -> int:
    root = Path(args.root).resolve()
    target_dir = Path(args.target_dir).resolve() if args.target_dir else root / "target"
    vocab, vocab_source, crates, artifacts = enumerate_scan_set(
        root, args.vocab, target_dir, args.profile, load_vocab=True)

    total_scanned = 0
    hits = []
    emitter_hits = []
    adv_hits = []
    total_unknown = total_allowed = total_skip = 0
    zero_scanned = []
    family_missing = []
    family_present = {n for n in DOC_EXECUTION_FAMILY if any(i["name"] == n for _, i in crates)}
    per_crate_scanned = {}
    resolved_keys = set()
    for cdir, info in crates:
        name = info["name"]
        agg = scan_crate_artifacts(name, artifacts[name], vocab)
        resolved_keys |= agg["emitter_resolved"]
        crate_scanned = agg["scanned"]
        total_skip += agg["skip"]
        per_crate_scanned[name] = crate_scanned
        if crate_scanned == 0:
            zero_scanned.append(name)
            if name in family_present:
                family_missing.append(name)
        total_scanned += crate_scanned
        hits.extend(agg["hits"])
        emitter_hits.extend(agg["emitter_hits"])
        adv_hits.extend(agg["adv"])
        total_unknown += agg["unknown"]
        total_allowed += agg["allowed"]
    render_scan_report(per_crate_scanned, hits, emitter_hits, adv_hits)
    if family_missing:
        raise GateRefused("execution-crate-no-artifacts", ",".join(sorted(family_missing)),
                          f"doc execution family with zero scanned artifacts at profile={args.profile}; "
                          "run cargo build first (structural anti-vacuity precondition)")
    if zero_scanned:
        raise GateRefused("crate-no-artifacts", ",".join(sorted(zero_scanned)),
                          f"non-excluded member crates with zero scanned artifacts at profile={args.profile}")
    if total_scanned == 0:
        raise GateRefused("no-artifacts-scanned", str(target_dir))
    unresolved = sorted(set(EMITTER_BINARIES) - resolved_keys)
    if unresolved:
        raise GateRefused("emitter-exempt-unresolved", ",".join(f"{p}:{b}" for p, b in unresolved),
                          "producer-exemption key matched no discovered artifact; "
                          "run cargo build or fix the key (no dead entries)")
    fence = emitter_fence_violations()
    if fence:
        raise GateRefused("emitter-fence-violated", "; ".join(fence),
                          "producer exemption granted to an execution-family package")
    print(summary_line(total_scanned, len(crates), hits, emitter_hits, adv_hits,
                       total_unknown, total_allowed, total_skip, vocab, vocab_source))
    if hits:
        return 1
    return 0


# ---------------------------------------------------------------------------
# Dependency-direction mode (DoD 8 runtime half).
# ---------------------------------------------------------------------------

def cmd_dep_direction(args) -> int:
    """Runtime dependency-direction check over compiled artifacts (the
    binary is the subject; source greps are advisory output only). Every
    non-exempt member's artifacts are searched for out-of-band references
    (archive magic, archive path, crate-name strings) in fail-set
    sections; any hit is a runtime edge kernel->.bosym and exits 1."""
    root = Path(args.root).resolve()
    target_dir = Path(args.target_dir).resolve() if args.target_dir else root / "target"
    _v, _s, crates, artifacts = enumerate_scan_set(root, None, target_dir, args.profile, load_vocab=False)
    violations = []
    adv_refs = []
    checked = skipped = 0
    zero = []
    for cdir, info in crates:
        pkg = info["name"]
        if pkg in BOSYM_REFERENCE_EXEMPT:
            print(f"DEP-DIR-EXEMPT crate={pkg} {BOSYM_REFERENCE_EXEMPT[pkg]}")
            continue
        crate_checked = 0
        for art in artifacts[pkg]:
            try:
                res = dep_dir_scan_artifact(art)
            except GateRefused as e:
                if e.reason_class == "artifact-vanished":
                    skipped += 1
                    print(f"[SKIP] {art} — vanished mid-run (concurrent cargo build)", file=sys.stderr)
                    continue
                raise
            if res["skip"]:
                skipped += 1
                print(f"[SKIP] {art} — {res['skip']}")
                continue
            crate_checked += 1
            checked += 1
            violations.extend(res["violations"])
            adv_refs.extend(res["adv"])
        if crate_checked == 0:
            zero.append(pkg)
    for (p, sec, off, ref) in violations:
        print(f"DEP-DIR-VIOLATION {p} {sec} {off} {ref}")
    for (p, sec, off, ref) in adv_refs:
        print(f"DEP-DIR-ADV(non-fail-section) {p} {sec} {off} {ref}")
    if zero:
        raise GateRefused("crate-no-artifacts", ",".join(sorted(zero)),
                          f"non-exempt member crates with zero artifacts at profile={args.profile}; "
                          "run cargo build first")
    # Source-level grep: ADVISORY ONLY (lane doc line 137: "the gate reads
    # binaries, never source. Source-level greps are advisory output only").
    src_refs = []
    for cdir, info in crates:
        if info["name"] in BOSYM_REFERENCE_EXEMPT:
            continue
        src_dir = cdir / "src"
        if not src_dir.is_dir():
            continue
        for src in sorted(src_dir.rglob("*.rs")):
            try:
                text = src.read_text(encoding="utf-8", errors="strict")
            except (UnicodeDecodeError, OSError) as exc:
                print(f"DEP-DIR-ADV-SRC-UNREADABLE {src} ({exc})", file=sys.stderr)
                continue
            for lineno, line in enumerate(text.splitlines(), 1):
                if any(t in line for t in ("cgap_bosym", "cgap-bosym", ".bosym")):
                    src_refs.append(f"{src}:{lineno}")
    for r in src_refs:
        print(f"DEP-DIR-ADV-SRC {r}")
    print(f"DEP-DIR-SUMMARY checked={checked} crates={len(crates)} violations={len(violations)} "
          f"adv_section_refs={len(adv_refs)} src_refs={len(src_refs)} skipped_artifacts={skipped}")
    return 1 if violations else 0


# ---------------------------------------------------------------------------
# Fixture building (real cargo, temp dir — Chicago discipline; the canonical
# checkout is never mutated by the gate).
# ---------------------------------------------------------------------------

POISON_TOKEN = "fibo:FndUtlCur:USD"
FIXTURE_PKG_POISON = "a07fxt_poison"
FIXTURE_PKG_CLEAN = "a07fxt_clean"


def build_fixture_workspace(workdir: Path) -> tuple:
    """Real cargo build of a two-crate fixture workspace: one poisoned lib,
    one clean twin. Returns (poison_artifacts, clean_artifacts, fixture_root).
    sha256 restore discipline: fixtures live entirely in `workdir` (temp);
    the gate touches no canonical-checkout artifact, so no restore of real
    state is ever required — asserted by the caller."""
    fixture = Path(tempfile.mkdtemp(prefix="fxws.", dir=workdir))
    (fixture / FIXTURE_PKG_POISON / "src").mkdir(parents=True)
    (fixture / FIXTURE_PKG_CLEAN / "src").mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        "[workspace]\nmembers = ["
        f'"{FIXTURE_PKG_POISON}", "{FIXTURE_PKG_CLEAN}"]\nresolver = "2"\n',
        encoding="utf-8")
    (fixture / FIXTURE_PKG_POISON / "Cargo.toml").write_text(
        f'[package]\nname = "{FIXTURE_PKG_POISON}"\nversion = "0.1.0"\nedition = "2021"\n\n'
        '[lib]\ncrate-type = ["rlib"]\n', encoding="utf-8")
    (fixture / FIXTURE_PKG_CLEAN / "Cargo.toml").write_text(
        f'[package]\nname = "{FIXTURE_PKG_CLEAN}"\nversion = "0.1.0"\nedition = "2021"\n\n'
        '[lib]\ncrate-type = ["rlib"]\n', encoding="utf-8")
    # Poison: the planted token must materialize in a __TEXT-section class
    # of the object: a &str static plus byte-string statics, referenced by
    # an exported fn so nothing is dead-stripped from the rlib member.
    # A07_PLANTED_MIXED embeds the token with INVALID UTF-8 bytes in the
    # same contiguous array (T15: the strict-decode predicate dropped such
    # runs entirely — Wave-5 repair witness class).
    (fixture / FIXTURE_PKG_POISON / "src" / "lib.rs").write_text(
        f'pub static A07_PLANTED_STR: &str = "{POISON_TOKEN}";\n'
        f'pub static A07_PLANTED_BYTES: &[u8] = b"{POISON_TOKEN}\\x00tail";\n'
        f'pub static A07_PLANTED_MIXED: &[u8] = b"\\x80\\x81{POISON_TOKEN}\\x82\\x83";\n'
        "pub fn a07_touch() -> (u8, u8, usize) {\n"
        "    (A07_PLANTED_BYTES[0], A07_PLANTED_MIXED[2], A07_PLANTED_STR.len())\n"
        "}\n", encoding="utf-8")
    (fixture / FIXTURE_PKG_CLEAN / "src" / "lib.rs").write_text(
        'pub static A07_CLEAN_BYTES: &[u8] = b"totally-clean-fixture-bytes";\n'
        "pub fn a07_touch_clean() -> u8 { A07_CLEAN_BYTES[0] }\n",
        encoding="utf-8")
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(fixture / "target")
    proc = subprocess.run(
        ["cargo", "build", "--offline", "--manifest-path", str(fixture / "Cargo.toml")],
        capture_output=True, timeout=600, env=env)
    if proc.returncode != 0:
        raise GateRefused("fixture-build-failed", str(fixture),
                          proc.stderr.decode("utf-8", "replace")[-400:])
    pkg_poison = FIXTURE_PKG_POISON.replace("-", "_")
    pkg_clean = FIXTURE_PKG_CLEAN.replace("-", "_")
    prof_dir = fixture / "target" / "debug"
    def rlibs_for(pkg: str, pkg_us: str) -> list:
        found = set(prof_dir.glob(f"lib{pkg}*.rlib")) | set((prof_dir / "deps").glob(f"lib{pkg_us}-*.rlib"))
        build_layout = prof_dir / "build" / pkg
        if build_layout.is_dir():
            for rlib in build_layout.glob("*/out/*.rlib"):
                stem = rlib.name[3:]
                if stem.startswith(pkg_us + "-") or stem.startswith(pkg_us + ".") or stem == pkg_us:
                    found.add(rlib)
        return sorted(found)
    poison_arts = rlibs_for(FIXTURE_PKG_POISON, pkg_poison)
    clean_arts = rlibs_for(FIXTURE_PKG_CLEAN, pkg_clean)
    if not poison_arts or not clean_arts:
        raise GateRefused("fixture-artifact-missing", str(fixture / "target" / "debug"))
    return poison_arts, clean_arts, fixture


DEPDIR_PKG_POISON = "a07ddx_poison"
DEPDIR_PKG_CLEAN = "a07ddx_clean"


def build_depdir_fixture_workspace(workdir: Path) -> tuple:
    """Anti-vacuity fixture for the dep-direction predicate (T13): a real
    cargo build of a two-crate workspace where the poisoned lib carries
    the .bosym archive magic and the canonical archive path in statics
    (they land in Mach-O __TEXT sections of the rlib object), and the
    clean twin carries neither. Same temp-dir discipline as
    build_fixture_workspace: the canonical checkout is never touched."""
    fixture = Path(tempfile.mkdtemp(prefix="ddws.", dir=workdir))
    (fixture / DEPDIR_PKG_POISON / "src").mkdir(parents=True)
    (fixture / DEPDIR_PKG_CLEAN / "src").mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        "[workspace]\nmembers = ["
        f'"{DEPDIR_PKG_POISON}", "{DEPDIR_PKG_CLEAN}"]\nresolver = "2"\n',
        encoding="utf-8")
    (fixture / DEPDIR_PKG_POISON / "Cargo.toml").write_text(
        f'[package]\nname = "{DEPDIR_PKG_POISON}"\nversion = "0.1.0"\nedition = "2021"\n\n'
        '[lib]\ncrate-type = ["rlib"]\n', encoding="utf-8")
    (fixture / DEPDIR_PKG_CLEAN / "Cargo.toml").write_text(
        f'[package]\nname = "{DEPDIR_PKG_CLEAN}"\nversion = "0.1.0"\nedition = "2021"\n\n'
        '[lib]\ncrate-type = ["rlib"]\n', encoding="utf-8")
    (fixture / DEPDIR_PKG_POISON / "src" / "lib.rs").write_text(
        'pub static A07_DD_MAGIC: [u8; 8] = *b"CGAPBSYM";\n'
        'pub static A07_DD_PATH: &str = "gen/bosym/symbols.bosym";\n'
        "pub fn a07_dd_touch() -> (u8, usize) {\n"
        "    (A07_DD_MAGIC[0], A07_DD_PATH.len())\n"
        "}\n", encoding="utf-8")
    (fixture / DEPDIR_PKG_CLEAN / "src" / "lib.rs").write_text(
        'pub static A07_DD_CLEAN: &[u8] = b"no-bosym-refs-in-here";\n'
        "pub fn a07_dd_touch_clean() -> u8 { A07_DD_CLEAN[0] }\n",
        encoding="utf-8")
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(fixture / "target")
    proc = subprocess.run(
        ["cargo", "build", "--offline", "--manifest-path", str(fixture / "Cargo.toml")],
        capture_output=True, timeout=600, env=env)
    if proc.returncode != 0:
        raise GateRefused("fixture-build-failed", str(fixture),
                          proc.stderr.decode("utf-8", "replace")[-400:])
    pkg_poison = DEPDIR_PKG_POISON.replace("-", "_")
    pkg_clean = DEPDIR_PKG_CLEAN.replace("-", "_")
    prof_dir = fixture / "target" / "debug"
    def rlibs_for(pkg: str, pkg_us: str) -> list:
        found = set(prof_dir.glob(f"lib{pkg}*.rlib")) | set((prof_dir / "deps").glob(f"lib{pkg_us}-*.rlib"))
        build_layout = prof_dir / "build" / pkg
        if build_layout.is_dir():
            for rlib in build_layout.glob("*/out/*.rlib"):
                stem = rlib.name[3:]
                if stem.startswith(pkg_us + "-") or stem.startswith(pkg_us + ".") or stem == pkg_us:
                    found.add(rlib)
        return sorted(found)
    poison_arts = rlibs_for(DEPDIR_PKG_POISON, pkg_poison)
    clean_arts = rlibs_for(DEPDIR_PKG_CLEAN, pkg_clean)
    if not poison_arts or not clean_arts:
        raise GateRefused("fixture-artifact-missing", str(fixture / "target" / "debug"))
    return poison_arts, clean_arts, fixture


EMITTER_PKG = "cgap-fibo"        # the REAL producer package name
EMITTER_BIN = "emit-fibo-pack"   # the REAL producer bin name — the exact (pkg, bin) key granted
CTL_PKG = "a07ctl_pack"          # control: identical poison, NON-granted (package, bin)


def build_emitter_fixture_workspace(workdir: Path) -> dict:
    """T16 fixture (Wave-7): a real cargo build of a two-package workspace
    where package `cgap-fibo` declares bin `emit-fibo-pack` — the exact
    (package, bin) key the producer exemption grants — and package
    `a07ctl_pack` declares bin `a07ctl_pack`; both bins embed the same
    planted vocabulary token. Temp-dir discipline: the canonical checkout
    is never touched; both arms are real toolchain builds of the same run."""
    fixture = Path(tempfile.mkdtemp(prefix="emws.", dir=workdir))
    for pkg in (EMITTER_PKG, CTL_PKG):
        (fixture / pkg / "src").mkdir(parents=True)
    (fixture / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["cgap-fibo", "a07ctl_pack"]\nresolver = "2"\n',
        encoding="utf-8")
    for pkg, binname in ((EMITTER_PKG, EMITTER_BIN), (CTL_PKG, CTL_PKG)):
        (fixture / pkg / "Cargo.toml").write_text(
            f'[package]\nname = "{pkg}"\nversion = "0.1.0"\nedition = "2021"\n\n'
            f'[[bin]]\nname = "{binname}"\npath = "src/main.rs"\n', encoding="utf-8")
        (fixture / pkg / "src" / "main.rs").write_text(
            f'pub static A07_T16_PLANTED: &str = "{POISON_TOKEN}";\n'
            "fn main() {\n    std::hint::black_box(A07_T16_PLANTED);\n}\n",
            encoding="utf-8")
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(fixture / "target")
    proc = subprocess.run(
        ["cargo", "build", "--offline", "--manifest-path", str(fixture / "Cargo.toml")],
        capture_output=True, timeout=600, env=env)
    if proc.returncode != 0:
        raise GateRefused("fixture-build-failed", str(fixture),
                          proc.stderr.decode("utf-8", "replace")[-400:])
    prof = fixture / "target" / "debug"
    arts = {pkg: [prof / binname]
            for pkg, binname in ((EMITTER_PKG, EMITTER_BIN), (CTL_PKG, CTL_PKG))}
    missing = [str(p) for ps in arts.values() for p in ps if not p.is_file()]
    if missing:
        raise GateRefused("fixture-artifact-missing", str(prof), ",".join(missing))
    return arts, fixture


# ---------------------------------------------------------------------------
# --self-check and the T4-T10 + T13 court suite.
# ---------------------------------------------------------------------------

def _sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def self_check(vocab: dict, workdir: Path) -> tuple:
    """Returns (ok: bool, detail lines). Exit-0 requires BOTH arms:
    poisoned fixture HITs naming the planted token, clean twin does not.
    Wave-4: every fixture artifact is sha256'd before and after the scan;
    a mismatch fails the self-check — the scan path is witnessed
    read-only (shim-court restore discipline, lane doc step 4)."""
    poison_arts, clean_arts, fixture = build_fixture_workspace(workdir)
    before = {a: _sha256_file(a) for a in poison_arts + clean_arts}
    lines = []
    poison_hits = []
    for art in poison_arts:
        res = scan_artifact(art, vocab)
        poison_hits.extend(res["hits"])
    clean_hits = []
    clean_scanned = 0
    for art in clean_arts:
        res = scan_artifact(art, vocab)
        clean_scanned += 1 if res["scanned"] else 0
        clean_hits.extend(res["hits"])
    after = {a: _sha256_file(a) for a in poison_arts + clean_arts}
    mutated = sorted(str(a) for a in before if before[a] != after.get(a))
    fired = any(POISON_TOKEN in h[3] for h in poison_hits)
    lines.append(f"poison artifacts={len(poison_arts)} hits={len(poison_hits)} fired={fired}")
    lines.append(f"clean artifacts={len(clean_arts)} scanned={clean_scanned} hits={len(clean_hits)}")
    lines.append(f"scan-mutation-witness artifacts={len(before)} mutated={mutated if mutated else 'none'}")
    if mutated:
        print("VACUOUS GATE: scan mutated a fixture artifact (restore discipline violated)", file=sys.stderr)
        return False, lines
    if not fired:
        print("VACUOUS GATE: predicate does not fire on poisoned fixture", file=sys.stderr)
        return False, lines
    if clean_hits:
        print(f"VACUOUS GATE: clean fixture produced hits {clean_hits}", file=sys.stderr)
        return False, lines
    if clean_scanned < 1:
        print("VACUOUS GATE: clean fixture contributed no scanned artifact", file=sys.stderr)
        return False, lines
    return True, lines


def court_t4_poison(vocab, workdir):
    """T4: poisoned fixture FAILs with a HIT naming the planted string;
    clean twin PASSES; same toolchain, same run."""
    ok, lines = self_check(vocab, workdir)
    return ("T4_poison_probe", "PASS" if ok else "FAIL", "; ".join(lines))


def court_t5_cross_extractor(vocab, workdir):
    """T5: same fixture through raw-bytes scan and strings(1); identical
    matched-token sets on the poisoned rlib. .text placement: verified
    against otool -l; loud [SKIP] when the planted bytes land outside
    __text (the cross-check still asserts on the __TEXT-section case at
    minimum)."""
    poison_arts, _, _ = build_fixture_workspace(workdir)
    art = poison_arts[0]
    res = scan_artifact(art, vocab, cross_check=True)  # raises on mismatch
    raw_tokens = set(res.get("tokens", set()))
    sset = strings_tool_hits(art)
    strings_tokens = set()
    for line in sset:
        tok = find_vocab_token(line, vocab)
        if tok is not None:
            strings_tokens.add(tok)
    if raw_tokens != strings_tokens:
        return ("T5_cross_extractor", "FAIL",
                f"raw tokens={sorted(raw_tokens)} strings tokens={sorted(strings_tokens)}")
    placement = {h[1] for h in res["hits"]}
    # Wave-6: recognize the .text spelling per object format — Mach-O
    # sections are named "__TEXT,__text" (the "__text" substring matches);
    # ELF/PE name the section ".text" / ".text.*". Before this repair an
    # ELF host whose planted bytes DID land in .text would have been
    # mis-reported as [SKIP] (loud, but wrong: the placement leg was
    # achieved and not skipped-worthy).
    def _in_text(section_name):
        return ("__text" in section_name
                or section_name == ".text"
                or section_name.startswith(".text."))
    if not any(_in_text(p) for p in placement):
        skip = (f"[SKIP] .text placement not achievable on this target; "
                f"planted bytes carried in {sorted(placement)} (cross-check asserted there)")
        return ("T5_cross_extractor", "PASS", skip + f"; matched-token sets identical: {sorted(raw_tokens)}")
    return ("T5_cross_extractor", "PASS", f"matched-token sets identical incl .text: {sorted(raw_tokens)}")


def court_t6_allowlist_non_absorption(vocab, _workdir):
    """T6: no allowlist entry equals/matches/is a superstring OR substring
    of any vocabulary token (false-negative fence)."""
    absorbed = []
    for tok in vocab:
        for entry in ALLOWLIST:
            hit = entry.matches(tok)
            if not hit and entry.exact is not None:
                hit = entry.exact in tok or tok in entry.exact
            if hit:
                absorbed.append((tok, entry.exact or entry.regex.pattern))
    if absorbed:
        return ("T6_absorption", "FAIL", f"absorbed pairs: {absorbed}")
    return ("T6_allowlist_non_absorption", "PASS", f"{len(vocab)} tokens x {len(ALLOWLIST)} entries: no absorption")


def court_t7_allowlist_justified(_vocab, _workdir):
    """T7: every allowlist entry parses and carries an
    ALLOWLIST-JUSTIFICATION; uncommented entries fail this court."""
    bad = []
    for entry in ALLOWLIST:
        if not entry.justification.startswith("ALLOWLIST-JUSTIFICATION:") or len(entry.justification) < 30:
            bad.append(entry.exact or entry.regex.pattern)
        if entry.exact is None and entry.regex is None:
            bad.append("<empty-entry>")
    if bad:
        return ("T7_allowlist_review", "FAIL", f"entries lacking justification: {bad}")
    return ("T7_allowlist_review", "PASS", f"{len(ALLOWLIST)} entries all justified")


def court_t8_vocab_outranks_allowlist(vocab, _workdir):
    """T8: a vocabulary token that is also a plausible allowlist string
    must produce a HIT — vocabulary outranks allowlist on exact equality.
    Pinned on the real predicate with a hostile allowlist containing the
    exact token."""
    tok = sorted(vocab)[0]
    hostile = list(ALLOWLIST) + [AllowEntry(exact=tok, justification="ALLOWLIST-JUSTIFICATION: hostile T8 probe — must NOT swallow vocabulary")]
    cls, _ = classify(tok, vocab, allowlist=hostile)
    if cls != "HIT":
        return ("T8_vocab_outranks_allowlist", "FAIL", f"token {tok!r} classified {cls} under hostile allowlist")
    # And an allowlisted non-vocab string stays ALLOWED under the same list.
    cls2, _ = classify("src/main.rs", vocab, allowlist=hostile)
    if cls2 != "ALLOWED":
        return ("T8_vocab_outranks_allowlist", "FAIL", f"allowlisted string classified {cls2}")
    return ("T8_vocab_outranks_allowlist", "PASS", f"vocabulary-first order proven on {tok!r}")


def court_t9_vocab_missing(workdir: Path):
    """T9: vocabulary manifest missing -> GATE-REFUSED vocab-missing, exit 1,
    never silent pass."""
    empty_root = workdir / "t9root"
    (empty_root / "ontologies").mkdir(parents=True, exist_ok=True)
    (empty_root / "Cargo.toml").write_text('[workspace]\nmembers = []\n', encoding="utf-8")
    try:
        vocab, source = resolve_vocabulary(empty_root, None)
    except GateRefused as e:
        if e.reason_class == "vocab-missing":
            return ("T9_vocab_missing", "PASS", str(e))
        return ("T9_vocab_missing", "FAIL", f"wrong refusal class: {e}")
    return ("T9_vocab_missing", "FAIL", f"vocab resolved unexpectedly: {source} {len(vocab)} tokens")


def court_t10_workspace_enumeration(vocab, workdir):
    """T10: a fixture workspace (real cargo) is enumerated automatically;
    BOTH members (clean + poisoned) contribute scanned artifacts — the gate
    cannot be bypassed by adding a crate (the poisoned one IS an added
    crate outside any hardcoded list)."""
    poison_arts, clean_arts, fixture = build_fixture_workspace(workdir)
    ws_root = fixture
    members = workspace_members(ws_root)
    names = []
    for cdir in members:
        info = parse_crate_manifest(cdir / "Cargo.toml")
        names.append(info["name"])
    expected = {FIXTURE_PKG_POISON, FIXTURE_PKG_CLEAN}
    if expected.issubset(set(names)):
        return ("T10_workspace_enumeration", "PASS",
                f"members enumerated mechanically: {sorted(names)}; "
                f"artifact counts poison={len(poison_arts)} clean={len(clean_arts)}")
    return ("T10_workspace_enumeration", "FAIL", f"enumerated {sorted(names)}, expected to include {sorted(expected)}")


def court_t13_dep_direction(_vocab, workdir):
    """T13 (wave-4 hardening, DoD 8 anti-vacuity): the dep-direction
    predicate fires on a fixture carrying the .bosym archive magic and the
    canonical archive path in fail-set sections, and does NOT fire on its
    clean twin. A dep-direction check that cannot flip is FAIL."""
    poison_arts, clean_arts, _fixture = build_depdir_fixture_workspace(workdir)
    poison_violations = []
    for art in poison_arts:
        res = dep_dir_scan_artifact(art)
        if not res["skip"]:
            poison_violations.extend(res["violations"])
    clean_violations = []
    clean_scanned = 0
    for art in clean_arts:
        res = dep_dir_scan_artifact(art)
        if not res["skip"]:
            clean_scanned += 1
            clean_violations.extend(res["violations"])
    refs = {v[3] for v in poison_violations}
    if not refs:
        return ("T13_dep_direction", "FAIL",
                "VACUOUS GATE: dep-direction predicate does not fire on poisoned fixture")
    if "CGAPBSYM" not in refs:
        return ("T13_dep_direction", "FAIL",
                f"poison fired but the archive magic was not among the refs: {sorted(refs)}")
    if clean_violations:
        return ("T13_dep_direction", "FAIL",
                f"clean fixture produced violations {clean_violations}")
    if clean_scanned < 1:
        return ("T13_dep_direction", "FAIL",
                "clean fixture contributed no scanned artifact")
    return ("T13_dep_direction", "PASS",
            f"poison violations={len(poison_violations)} refs={sorted(refs)}; "
            f"clean artifacts={clean_scanned} violations=0")


def court_t14_register_schema(_vocab, _workdir):
    """T14a (Wave-5): the A07 register — the A20 §6.4 'dep audit register
    + allowlist' input A20 consumes and never edits — emits as JSON,
    carries the required schema keys, its cloud patterns all compile, the
    §6.4 doc-named strings are all covered (register/doc parity, with the
    receipt-witnessed negative controls), and every allowlist entry in
    every register section carries an ALLOWLIST-JUSTIFICATION.
    Mutation class pinned by this court: a register missing a key,
    carrying an uncompilable pattern, losing a §6.4 doc-list string, or
    carrying an unjustified allowlist entry must FAIL here (witnessed by
    the /tmp register-mutation shim run in the Wave-5 receipt)."""
    import json as _json
    reg = _json.loads(_json.dumps(a07_register()))
    problems = []
    for key in ("register", "version", "owned_by", "consumers",
                "single_list_law", "dep_audit", "cloud_strings", "ontology",
                "emitter_exemptions"):
        if key not in reg:
            problems.append(f"missing key: {key}")
    compiled = []
    for p, tag in reg["cloud_strings"]["patterns"]:
        try:
            compiled.append((re.compile(p), tag))
        except re.error as exc:
            problems.append(f"pattern {p!r} does not compile: {exc}")
    # §6.4 doc-list parity: every doc-named string must be covered by at
    # least one pattern, with the receipt-witnessed negative controls.
    coverage_samples = {
        "aws": 'provider = "aws";',
        "azure": "https://portal.azure.example/tenant",
        "gcp": 'region = "gcp";',
        "s3.": "https://mybucket.s3.us-east-1.amazonaws.com/key",
        "blob.core": "https://blob.core.windows.net/container/blob",
        "amazonaws": "endpoint = ec2.us-west-2.amazonaws.com",
    }
    for doc_str in reg["cloud_strings"]["a20_6_4_doc_list"]:
        sample = coverage_samples.get(doc_str)
        if sample is None:
            problems.append(f"no coverage sample for doc-list entry {doc_str!r}")
            continue
        if not any(rx.search(sample) for rx, _ in compiled):
            problems.append(f"doc-list entry {doc_str!r} uncovered by any pattern")
    for neg in ("laws", "s3.verify(&key)"):
        if any(rx.search(neg) for rx, _ in compiled):
            problems.append(f"negative control {neg!r} falsely matched")
    for section in ("cloud_strings", "ontology"):
        for entry in reg[section]["allowlist"]:
            if not str(entry.get("justification", "")).startswith("ALLOWLIST-JUSTIFICATION:"):
                problems.append(f"{section} allowlist entry lacks justification: {entry}")
    for dep, just in reg["dep_audit"]["external_dep_allowances"].items():
        if not str(just).startswith("ALLOWLIST-JUSTIFICATION:"):
            problems.append(f"dep allowance {dep} lacks justification")
    # Wave-7: the producer exemption register section — doc parity in BOTH
    # directions, every entry justified, and the anti-bypass fence (no
    # execution-family package may hold a producer grant).
    em = reg["emitter_exemptions"]
    doc_named = {tuple(k) for k in em["doc_named"]}
    reg_keys = {(e["package"], e["artifact"]) for e in em["entries"]}
    if doc_named != reg_keys:
        problems.append(f"emitter doc/registry parity broken: doc={sorted(doc_named)} "
                        f"registry={sorted(reg_keys)}")
    for e in em["entries"]:
        if not str(e.get("justification", "")).startswith("ALLOWLIST-JUSTIFICATION:"):
            problems.append(f"emitter exemption {(e['package'], e['artifact'])} lacks justification")
    if em.get("scope_rule", "") == "":
        problems.append("emitter_exemptions.scope_rule empty")
    for (p, b), _j in EMITTER_BINARIES.items():
        if p in DOC_EXECUTION_FAMILY:
            problems.append(f"emitter fence: execution-family package {p} holds a producer grant")
    if problems:
        return ("T14_register_schema", "FAIL", "; ".join(problems))
    n_pat = len(reg["cloud_strings"]["patterns"])
    return ("T14_register_schema", "PASS",
            f"register v{reg['version']}: {n_pat} patterns compile, "
            f"{len(reg['cloud_strings']['a20_6_4_doc_list'])} §6.4 doc strings covered, "
            "all allowlist entries justified")


def court_t14_register_predicate(_vocab, workdir):
    """T14b (Wave-5): the register's own cloud predicate flips — planted
    cloud string in a real file HITs, clean twin does not, and with the
    A07 cloud allowlist empty no planted line is classified ALLOWED.
    Mutation class: emptying CLOUD_STRING_PATTERNS must make this court
    FAIL (witnessed by the /tmp register-mutation shim run)."""
    poison = workdir / "t14_poison.txt"
    clean = workdir / "t14_clean.txt"
    poison.write_text(
        "fn region() -> String { \"us-east-1\".into() }\n"
        "let endpoint = \"https://mybucket.s3.us-east-1.amazonaws.com\";\n"
        "struct S3Client;\n", encoding="utf-8")
    clean.write_text(
        "fn region() -> String { \"us-east-1\".into() }\n"
        "struct S3Verify;\n", encoding="utf-8")
    poison_hits = cloud_scan_text(poison.read_text(encoding="utf-8"))
    clean_hits = cloud_scan_text(clean.read_text(encoding="utf-8"))
    poison_tags = {t for t, _, _ in poison_hits}
    if "s3.<host>" not in poison_tags:
        return ("T14_register_predicate", "FAIL",
                f"VACUOUS GATE: register cloud predicate did not fire on "
                f"planted s3 endpoint; hits={poison_hits}")
    if clean_hits:
        return ("T14_register_predicate", "FAIL",
                f"clean twin produced hits: {clean_hits}")
    if CLOUD_STRING_ALLOWLIST:
        return ("T14_register_predicate", "FAIL",
                "A07 cloud allowlist is non-empty — fail-closed default lost")
    return ("T14_register_predicate", "PASS",
            f"planted s3 endpoint HITs ({len(poison_hits)} rows), clean twin silent, "
            "allowlist empty by design")


def court_t15_invalid_utf8_neighbor(vocab, workdir):
    """T15 (Wave-5 repair court, printable_runs false-negative class): a
    vocabulary token embedded in ONE contiguous byte array with INVALID
    UTF-8 bytes must still HIT through the production scan path (real
    cargo-built rlib). Named mutation class: the pre-repair strict decode
    DROPPED such runs entirely (witnessed: b\"start-pad\\x80\\x81<token>\"
    produced no runs) — the pre-fix gate FAILs this court (shim run in the
    Wave-5 receipt); the repaired gate must HIT, and the clean twin must
    stay clean.
    Flip modes, both witnessed (Wave-5 audit M-G1 note): pre-Wave-6, the
    plain strict-decode revert raised UnicodeDecodeError inside
    printable_runs and the SUITE aborted via GATE-REFUSED: internal-error
    with zero verdict lines (flip fact held; witness was an abort). Post-
    Wave-6 the suite isolates the raise as an explicit
    `COURT T15_invalid_utf8_neighbor FAIL` row naming the exception,
    prints COURTS-SUMMARY, exits 1."""
    poison_arts, clean_arts, _fixture = build_fixture_workspace(workdir)
    poison_hits = []
    for art in poison_arts:
        res = scan_artifact(art, vocab)
        poison_hits.extend(res["hits"])
    clean_hits = []
    clean_scanned = 0
    for art in clean_arts:
        res = scan_artifact(art, vocab)
        if res["scanned"]:
            clean_scanned += 1
        clean_hits.extend(res["hits"])
    mixed_hits = [h for h in poison_hits if "�" in h[3] and POISON_TOKEN in h[3]]
    if not mixed_hits:
        return ("T15_invalid_utf8_neighbor", "FAIL",
                "VACUOUS GATE: no HIT carries the token with invalid-UTF-8 "
                f"neighbors (replacement chars absent from hits: "
                f"{[h[3][:80] for h in poison_hits]}); the strict-decode "
                "false-negative class is exposed again")
    if clean_hits or clean_scanned < 1:
        return ("T15_invalid_utf8_neighbor", "FAIL",
                f"clean twin hits={clean_hits} scanned={clean_scanned}")
    return ("T15_invalid_utf8_neighbor", "PASS",
            f"mixed-run HIT fired ({len(mixed_hits)} rows, token with "
            "U+FFFD neighbors); clean twin clean")


def court_cross_impl(vocab, vocab_source, _workdir):
    """Cross-implementation witness: when the vocabulary came from a
    Rust-generated archive, the Python reader and the Rust codec agreed on
    the same wire. When the vocabulary was derived from the graph, build a
    Python-side archive and round-trip it (reader self-consistency), and
    mark the cross-language leg as needing the Rust emit court."""
    entries = [(0xE508EC39DB7ECC28, 2, POISON_TOKEN), (1, 0, "ünïcode-nąme\0nul"), (2, 4, "x" * 40)]
    blob = bosym_build(entries)
    back = bosym_parse(blob)
    if [(s, k, n) for s, k, n in back] != entries:
        return ("TX_cross_impl_framing", "FAIL", "Python round-trip diverged")
    for cut in range(1, len(blob)):
        try:
            bosym_parse(blob[:cut])
            return ("TX_cross_impl_framing", "FAIL", f"truncation at {cut} accepted")
        except VocabError:
            pass
    if vocab_source.startswith("archive:"):
        return ("TX_cross_impl_framing", "PASS",
                f"python reader round-trip + every-offset truncation refusal; live vocab from Rust-generated archive ({vocab_source})")
    return ("TX_cross_impl_framing", "PASS",
            "python reader round-trip + every-offset truncation refusal; live vocab was graph-derived "
            "(run the Rust emit court to exercise the cross-language leg)")


def court_t16_emitter_exemption(vocab, workdir):
    """T16 (Wave-7 repair court; a07-wave6-audit-receipt.md §5 option (a),
    the A07-owned scan-mode exemption gap; Wave-8 pins added): the producer
    exemption class must (arm 1) bucket an artifact built from the GRANTED
    (package, bin) key as emitter_hits — reported, non-fatal, key resolved
    — while (arm 2, anti-bypass) IDENTICAL poison in a non-granted package
    stays in the fatal bucket, (arm 3) the class's fence holds: no
    DOC_EXECUTION_FAMILY package may hold a grant, (arm 4, Wave-8
    loudness pin) the REAL cmd_scan renderer must emit the
    EMITTER-EXEMPT-HIT row (naming the planted token) and the
    emitter_hits=N SUMMARY field for this fixture's aggregation — the
    "LOUD, never silent" law pinned at the output boundary — and (arm 5,
    Wave-8 end-to-end pin) the REAL cmd_scan over this same fixture
    workspace must exit 1 WITH the control bin's poison fatal (hits=1),
    the producer's poison exempt-loud (EMITTER-EXEMPT-HIT +
    emitter_hits=1) — anti-bypass witnessed at the exit-code level —
    while the same command over the producer-less poison workspace must
    raise `emitter-exempt-unresolved` (the no-dead-entries law, witnessed
    end-to-end). All fixture arms are real cargo builds in one run
    (Chicago: same toolchain, both arms every run). Named mutation classes:
      M-E1 — emptying EMITTER_BINARIES makes arm 1 FAIL and T14a's
             doc-parity arm FAIL with it (the grant is doc-pinned in TWO
             courts; witnessed: COURTS-SUMMARY failed=2) and the
             full-workspace scan reverts to exit 1, hits=2 on the real
             emitter (both legs witnessed this session).
      M-E2 — adding an execution-family key (e.g. ("cgap-core", ...))
             makes arm 3's fence FAIL (the anti-bypass grant channel).
      M-E3 — breaking EMITTER_DOC_NAMED parity flips T14a instead (the
             register/doc drift class).
      M-E4 (Wave-8) — silencing the renderer (dropping the
             EMITTER-EXEMPT-HIT rows or the emitter_hits field) makes
             arm 4 FAIL: the loudness law has a court.
      M-E5 (Wave-8) — an unresolvable grant key makes arm 5's refusal leg
             FAIL (the no-dead-entries law fires end-to-end) and the arm-5
             control-fatal leg fails with it (one dead key refuses every scan).
    Skip policy: none — the fixture is a temp-dir build, always available
    wherever `cargo` is."""
    arts, emws = build_emitter_fixture_workspace(workdir)
    em = scan_crate_artifacts(EMITTER_PKG, arts[EMITTER_PKG], vocab)
    ctl = scan_crate_artifacts(CTL_PKG, arts[CTL_PKG], vocab)
    problems = []
    if not any(POISON_TOKEN in h[3] for h in em["emitter_hits"]):
        problems.append("VACUOUS GATE: producer grant did not bucket the planted "
                        f"token as emitter_hits (fatal={em['hits']}, "
                        f"emitter={em['emitter_hits']})")
    if em["hits"]:
        problems.append(f"granted bin hits leaked into the fatal bucket: {em['hits']}")
    if (EMITTER_PKG, EMITTER_BIN) not in em["emitter_resolved"]:
        problems.append("grant key did not resolve against its fixture artifact")
    if not any(POISON_TOKEN in h[3] for h in ctl["hits"]):
        problems.append("anti-bypass arm dead: identical poison in non-granted "
                        f"package produced no fatal hits (emitter={ctl['emitter_hits']}, "
                        f"fatal={ctl['hits']})")
    if ctl["emitter_hits"]:
        problems.append(f"non-granted package bucketed as emitter: {ctl['emitter_hits']}")
    fence = emitter_fence_violations()
    if fence:
        problems.append(f"fence violated: {fence}")

    # Arm 4 (Wave-8, M-E4): the loudness law is OUTPUT-pinned. The same
    # renderer cmd_scan uses must render this fixture's exemption rows and
    # the emitter_hits summary field; a renderer that swallows either
    # flips here.
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        render_scan_report({EMITTER_PKG: em["scanned"]}, em["hits"],
                           em["emitter_hits"], em["adv"])
        print(summary_line(em["scanned"], 1, em["hits"], em["emitter_hits"],
                           em["adv"], em["unknown"], em["allowed"], em["skip"],
                           vocab, "t16-arm4-fixture"))
    arm4 = buf.getvalue()
    exempt_rows = [ln for ln in arm4.splitlines() if ln.startswith("EMITTER-EXEMPT-HIT ")]
    if not exempt_rows or not any(POISON_TOKEN in ln for ln in exempt_rows):
        problems.append("VACUOUS GATE: renderer emitted no EMITTER-EXEMPT-HIT row "
                        f"naming the planted token (output: {arm4.strip()[:200]!r}) — "
                        "the exemption went silent (M-E4 class)")
    if f"emitter_hits={len(em['emitter_hits'])}" not in arm4:
        problems.append("VACUOUS GATE: SUMMARY emitter_hits field missing or wrong "
                        f"(expected emitter_hits={len(em['emitter_hits'])} in: "
                        f"{[ln for ln in arm4.splitlines() if ln.startswith('SUMMARY')]})")
    if any(ln.startswith("HIT ") for ln in arm4.splitlines()):
        problems.append(f"granted bin rendered in the fatal HIT bucket: {arm4}")
    buf2 = io.StringIO()
    with contextlib.redirect_stdout(buf2):
        render_scan_report({CTL_PKG: ctl["scanned"]}, ctl["hits"],
                           ctl["emitter_hits"], ctl["adv"])
    arm4b = buf2.getvalue()
    fatal_rows = [ln for ln in arm4b.splitlines() if ln.startswith("HIT ")]
    if not fatal_rows or not any(POISON_TOKEN in ln for ln in fatal_rows):
        problems.append("VACUOUS GATE: control arm rendered no fatal HIT row for "
                        f"the non-granted poison (output: {arm4b.strip()[:200]!r})")
    if any(ln.startswith("EMITTER-EXEMPT-HIT ") for ln in arm4b.splitlines()):
        problems.append(f"non-granted package rendered as emitter: {arm4b}")

    # Arm 5 (Wave-8, M-E5): the real cmd_scan end-to-end. Leg A: over this
    # fixture workspace the scan must exit 0, emitting the exemption rows
    # (vocab: a real v0 archive built in-court via the same builder the
    # framing court round-trips). Leg B: over the producer-less poison
    # workspace the no-dead-entries law must fire as
    # emitter-exempt-unresolved — the typed refusal witnessed end-to-end.
    vocab_archive = Path(workdir) / "t16vocab.bosym"
    entries = [(0xA07 + i, 2, tok) for i, tok in enumerate(sorted(vocab))]
    vocab_archive.write_bytes(bosym_build(entries))
    ns = argparse.Namespace(root=emws, target_dir=emws / "target",
                            vocab=str(vocab_archive), profile="debug")
    leg_a_out = io.StringIO()
    try:
        with contextlib.redirect_stdout(leg_a_out):
            leg_a_rc = cmd_scan(ns)
    except GateRefused as in_a:
        problems.append(f"arm 5 leg A refused (exit-1-by-control leg dead): {in_a}")
    else:
        out_a = leg_a_out.getvalue()
        # The control bin's poison is FATAL by design, so leg A's exit is 1 —
        # which IS the anti-bypass proof at the exit-code level. Leg A pins:
        # the granted producer's poison rendered as EMITTER-EXEMPT-HIT
        # (non-fatal), the control's poison rendered as a fatal HIT row, and
        # the SUMMARY separates the buckets (emitter_hits=1, hits=1).
        if leg_a_rc != 1:
            problems.append(f"arm 5 leg A exit={leg_a_rc} (expected 1: the "
                            "non-granted control poison must stay fatal end-to-end; "
                            f"SUMMARY: {[ln for ln in out_a.splitlines() if ln.startswith('SUMMARY')]})")
        if "EMITTER-EXEMPT-HIT" not in out_a or POISON_TOKEN not in out_a:
            problems.append("VACUOUS GATE: arm 5 leg A stdout carried no "
                            "EMITTER-EXEMPT-HIT row naming the planted token — "
                            "end-to-end loudness dead (M-E4/M-E5 class)")
        if not re.search(r"\bhits=1\b", out_a):
            problems.append(f"arm 5 leg A SUMMARY missing hits=1 for the "
                            f"non-granted control bin (stdout tail: {out_a.strip()[-200:]!r})")
        if "emitter_hits=1" not in out_a:
            problems.append(f"arm 5 leg A SUMMARY missing emitter_hits=1 "
                            f"(stdout tail: {out_a.strip()[-200:]!r})")
    _, _, pfix = build_fixture_workspace(workdir)
    ns2 = argparse.Namespace(root=pfix, target_dir=pfix / "target",
                             vocab=str(vocab_archive), profile="debug")
    try:
        with contextlib.redirect_stdout(io.StringIO()):
            cmd_scan(ns2)
        problems.append("VACUOUS GATE: arm 5 leg B — scan over the producer-less "
                        "workspace did NOT raise emitter-exempt-unresolved; the "
                        "no-dead-entries law is dead (M-E5 class)")
    except GateRefused as e:
        if e.reason_class != "emitter-exempt-unresolved":
            problems.append(f"arm 5 leg B raised the wrong refusal class: "
                            f"{e.reason_class} (expected emitter-exempt-unresolved)")
    if problems:
        return ("T16_emitter_exemption", "FAIL", "; ".join(problems))
    return ("T16_emitter_exemption", "PASS",
            f"producer-keyed poison non-fatal (emitter_hits={len(em['emitter_hits'])}, "
            f"fatal=0, key resolved); identical poison in {CTL_PKG} stays fatal "
            f"({len(ctl['hits'])} hits); fence holds ({len(EMITTER_BINARIES)} grant, "
            "0 execution-family keys); renderer loud (arm 4); cmd_scan over the "
            "fixture workspace exits 1 with the control poison fatal and the "
            "producer poison exempt-loud, and the producer-less workspace refuses "
            "emitter-exempt-unresolved (arm 5, end-to-end)")


def cmd_courts(args) -> int:
    root = Path(args.root).resolve()
    vocab, vocab_source = resolve_vocabulary(root, args.vocab)
    print(f"VOCAB source={vocab_source} tokens={len(vocab)}")
    results = []
    workdir = Path(tempfile.mkdtemp(prefix="a07_courts."))
    # Wave-6 (audit M-G1 flip-mode repair): every court is exception-
    # isolated. A court that RAISES (e.g. the strict-decode revert's
    # UnicodeDecodeError inside printable_runs) becomes an explicit FAIL
    # row naming the exception — the suite completes, COURTS-SUMMARY
    # prints, exit is 1. Before this repair the first raising court
    # aborted the whole suite through main()'s internal-error handler
    # BEFORE any verdict line, so the flip was real but its witness was
    # an abort, not a verdict (a07-wave5-audit-receipt.md §3 M-G1 note).
    def run_court(name, thunk):
        try:
            results.append(thunk())
        except Exception as exc:  # noqa: BLE001 — isolation is the point
            results.append((name, "FAIL",
                            f"court raised {type(exc).__name__}: {exc}"))
    try:
        run_court("TX_cross_impl_framing",
                  lambda: court_cross_impl(vocab, vocab_source, workdir))
        run_court("T6_allowlist_non_absorption",
                  lambda: court_t6_allowlist_non_absorption(vocab, workdir))
        run_court("T7_allowlist_review",
                  lambda: court_t7_allowlist_justified(vocab, workdir))
        run_court("T8_vocab_outranks_allowlist",
                  lambda: court_t8_vocab_outranks_allowlist(vocab, workdir))
        run_court("T9_vocab_missing",
                  lambda: court_t9_vocab_missing(workdir))
        run_court("T10_workspace_enumeration",
                  lambda: court_t10_workspace_enumeration(vocab, workdir))
        run_court("T13_dep_direction",
                  lambda: court_t13_dep_direction(vocab, workdir))
        run_court("T14_register_schema",
                  lambda: court_t14_register_schema(vocab, workdir))
        run_court("T14_register_predicate",
                  lambda: court_t14_register_predicate(vocab, workdir))
        run_court("T15_invalid_utf8_neighbor",
                  lambda: court_t15_invalid_utf8_neighbor(vocab, workdir))
        run_court("T16_emitter_exemption",
                  lambda: court_t16_emitter_exemption(vocab, workdir))
        run_court("T5_cross_extractor",
                  lambda: court_t5_cross_extractor(vocab, workdir))
        run_court("T4_poison_probe",
                  lambda: court_t4_poison(vocab, workdir))
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
    failed = 0
    for name, status, detail in results:
        print(f"COURT {name} {status} — {detail}")
        if status == "FAIL":
            failed += 1
    print(f"COURTS-SUMMARY total={len(results)} failed={failed}")
    return 1 if failed else 0


def cmd_register(args) -> int:
    """Emit the A07 register (A20 §6.4 gate input) as JSON on stdout.
    This is an EMISSION, not a gate: exit 0 on a successful emit, every
    error path fails closed through main()'s handler."""
    import json as _json
    print(_json.dumps(a07_register(), indent=2))
    return 0


def cmd_self_check(args) -> int:
    root = Path(args.root).resolve()
    vocab, vocab_source = resolve_vocabulary(root, args.vocab)
    workdir = Path(tempfile.mkdtemp(prefix="a07_selfcheck."))
    try:
        ok, lines = self_check(vocab, workdir)
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
    for line in lines:
        print(f"SELF-CHECK {line}")
    print(f"SELF-CHECK RESULT {'PASS' if ok else 'FAIL'} vocab={vocab_source}")
    return 0 if ok else 1


# ---------------------------------------------------------------------------
# CLI.
# ---------------------------------------------------------------------------

def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="cgap zero-string gate (lane A07, ADR-002)")
    ap.add_argument("--root", default=str(Path(__file__).resolve().parent.parent),
                    help="workspace root (Cargo.toml with members)")
    ap.add_argument("--target-dir", default=None, help="cargo target dir (default <root>/target)")
    ap.add_argument("--profile", default="debug", choices=["debug", "release", "audit"])
    ap.add_argument("--vocab", default=None, help=".bosym v0 vocabulary archive "
                    "(default: $CGAP_BOSYM_VOCAB, then <root>/gen/bosym/symbols.bosym, "
                    "then ontologies/*/src/lib.rs derivation)")
    ap.add_argument("--no-cross-check", action="store_true",
                    help="disable the strings(1) cross-extractor (not recommended)")
    modes = ap.add_mutually_exclusive_group()
    modes.add_argument("--self-check", action="store_true",
                       help="poison/clean fixture probe (anti-vacuity, T4+T12)")
    modes.add_argument("--courts", action="store_true",
                       help="run the T4-T10 + T13-T16 court suite")
    modes.add_argument("--dep-direction", action="store_true",
                       help="DoD 8 runtime check: no execution-crate artifact "
                            "carries .bosym references in fail-set sections")
    modes.add_argument("--register", action="store_true",
                       help="emit the A07 register (A20 §6.4 gate input: "
                            "dep-audit allowances + cloud-string patterns + "
                            "ontology allowlist) as JSON")
    args = ap.parse_args(argv)
    try:
        if args.self_check:
            return cmd_self_check(args)
        if args.courts:
            return cmd_courts(args)
        if args.dep_direction:
            return cmd_dep_direction(args)
        if args.register:
            return cmd_register(args)
        return cmd_scan(args)
    except GateRefused as e:
        print(str(e), file=sys.stderr)
        return 1
    except Exception:
        import traceback
        traceback.print_exc()
        print("GATE-REFUSED: internal-error zero_string_gate", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
