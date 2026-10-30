#!/usr/bin/env python3
"""CC=1 branch audit: disassemble release artifacts, count conditional
branches inside each kernel symbol's extent.

Charter CTQ-3 / dissertation Axiom 3.1: scan kernels must compile to machine
code with zero conditional branches. Exits 1 on any hit (loud, fail-closed).
"""
import re
import subprocess
import sys

ARTIFACTS = [
    "target/audit/libavec_audit.dylib",
]

KERNELS = [
    "audit_mask_words", "audit_count_valid", "audit_get_bit", "audit_set_bit",
    "audit_unpack_1bit", "audit_unpack_2bit", "audit_unpack_4bit",
    "audit_unpack_8bit", "audit_is_inline",
]

# arm64 + x86_64 conditional-branch mnemonics
COND = re.compile(
    r"^\s*[0-9a-f]+:.*\s(b\.(eq|ne|cs|hs|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)|cbz|cbnz|tbz|tbnz|j(a|e|ne|g|l|le|ge|b|be|s|ns|o|no|p|np|c|nc)\b)"
)


def disassemble(path):
    out = subprocess.run(
        ["xcrun", "llvm-objdump", "-d", "--no-show-raw-insn", path],
        capture_output=True, text=True,
    )
    if out.returncode != 0:
        out = subprocess.run(
            ["llvm-objdump", "-d", "--no-show-raw-insn", path],
            capture_output=True, text=True,
        )
    if out.returncode != 0:
        print(f"cannot disassemble {path}: {out.stderr[:200]}")
        sys.exit(1)
    return out.stdout


def kernel_extents(text):
    """Yield (name, body-lines) for each kernel symbol found."""
    lines = text.splitlines()
    sym_re = re.compile(r"^([0-9a-f]+) <(.*)>:$")
    current, buf = None, []
    for line in lines:
        m = sym_re.match(line)
        if m:
            if current is not None:
                yield current, buf
            current, buf = m.group(2), []
        elif current is not None:
            buf.append(line)
    if current is not None:
        yield current, buf


def main():
    fails = []
    seen = set()
    for art in ARTIFACTS:
        text = disassemble(art)
        for name, body in kernel_extents(text):
            plain = name[1:] if name.startswith("_") else name
            if plain in KERNELS:
                hits = [ln for ln in body if COND.match(ln)]
                seen.add(plain)
                status = "OK" if not hits else f"{len(hits)} CONDITIONAL"
                print(f"{plain:22s} {status:16s}")
                if hits:
                    fails.append((name, hits[:3]))
    missing = set(KERNELS) - seen
    if missing:
        print(f"MISSING SYMBOLS (audit not vacuous — investigate): {sorted(missing)}")
        sys.exit(1)
    if fails:
        for k, hits in fails:
            print(f"FAIL {k}: {hits}")
        sys.exit(1)
    print("BRANCH AUDIT PASS: 0 conditional branches across all audited kernels")


if __name__ == "__main__":
    main()
