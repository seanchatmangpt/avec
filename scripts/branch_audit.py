#!/usr/bin/env python3
"""CC=1 branch audit — extent-scoped, fail-closed.

Charter CTQ-3 / dissertation Axiom 3.1: data-plane inner loops must compile to
machine code with zero conditional branches in the loop BODY. Loop-control
latches and dispatch conditionals are itemized and budget-pinned — never
silently ignored. Exits 1 on any unaccounted conditional, any missing audited
surface, or any class over its pinned budget.

SCOPE LAW — why extents, not just the audit_* wrappers: the wrappers in
crates/audit/src/lib.rs are deliberately trivial stubs (slice construction +
call). The pre-2026-10-29 gate scanned only wrapper extents and therefore
passed 9/9 while every real kernel body (mangled callees like
__RNvNtCs..._10avec_arrow6bitmap10mask_words) went unaudited. This gate scans
wrapper extents AND transitive kernel-callee extents in the same artifact.
The callee match is a legacy-mangling substring (crate + fn components with
length prefixes, e.g. "10avec_arrow6bitmap10mask_words"), which is stable
across rebuilds because it skips the crate disambiguator (Cs<16hex>_).

CLASSIFICATION LAW — every conditional branch in an audited extent must be
accounted into exactly one class:
  LATCH      backward conditional branch (target < addr). It defines a loop
             with head == its target; latches are grouped by head, so a loop
             may carry several backedges (LLVM mid-loop re-entry, e.g. the
             unpack_8bit vector loop has 6). Budget-pinned per kernel.
  BODY-COND  conditional inside a loop body (innermost containing loop
             [head, max_latch)), not itself a latch. This is the CHARTER
             surface: branch-free data-plane inner loop. Budget 0 for the
             scan kernels (mask_words, count_valid, mask_words_neon). A
             conditional growing inside such a body is a true violation.
             For the unpack kernels the baseline body conditionals are
             per-chunk dispatch (bounds / per-record dispatch) — C-class
             control flow: itemized with addresses, budget-pinned, never
             silently ignored.
  DISPATCH   conditional outside every loop extent (entry guards, tail
             dispatch). C-class control flow: itemized, budget-pinned.
Per-kernel budgets below are PINNED to the machine recon of this artifact
(2026-10-29, audit profile: lto=false, codegen-units=16, strip=false).
observed > budget => exit 1. A compiler upgrade that shifts the partition
fails LOUDLY here; budgets are re-pinned by a reviewed diff, never silently
relaxed. observed < budget passes and is printed as a tightening
opportunity (thresholds only ever tighten).

NEON EXTENT RULE — the audited surface named in the silicon.rs disclosure
(crates/avec-arrow/src/silicon.rs, mask_words_neon):
  VEC-PRIMARY  the first SIMD loop in address order (the 4-word-stride NEON
               body: ldp q / and.16b / stp q). Must EXIST, must contain SIMD
               ops, and its body must carry 0 conditional branches.
               "First SIMD loop in address order" is the mechanical anchor
               for the primary kernel body: LLVM emits the primary loop
               before the tail machinery, and the disclosure's audited
               surface is the primary 4-word body, not the tail lowering.
               Rationale for the scope choice: anchoring on SIMD content
               pins the rule to the vector kernel itself (not to a literal
               address, which shifts on every rebuild, and not to instruction
               counts, which shift on every toolchain bump). If the primary
               vector loop disappears as a loop (full unroll) or goes
               branchy, this gate FAILS and the budget table must be
               re-pinned deliberately.
  VEC-TAIL     any further SIMD loop in the extent (here: LLVM's vectorized
               form of the scalar tail, 8-word stride). Counted separately,
               itemized, and held under the same kernel class budgets —
               reported, not silently ignored.
Fail-closed guarantee: every reported extent's conditionals must be accounted
for by the classes above within pinned budgets; any UNACCOUNTED conditional,
missing surface, or absent VEC-PRIMARY loop = exit 1.
"""
import re
import subprocess
import sys

ARTIFACTS = [
    "target/audit/libavec_audit.dylib",
]

# Audit-cdylib wrapper extents. These are stubs by construction (slice
# construction + kernel call) and must stay branch-free: budget (0, 0, 0).
WRAPPERS = [
    "audit_mask_words", "audit_mask_words_neon", "audit_count_valid",
    "audit_get_bit", "audit_set_bit",
    "audit_unpack_1bit", "audit_unpack_2bit", "audit_unpack_4bit",
    "audit_unpack_8bit", "audit_is_inline",
]

# Transitive kernel-callee extents: (legacy-mangled substring, display name,
# (LATCH budget, BODY-COND budget, DISPATCH budget)). Substrings anchor the
# demangled path with length-prefixed components and skip the per-build crate
# disambiguator, so they are stable across rebuilds. Budgets are the machine
# recon baseline of this artifact — see CLASSIFICATION LAW above.
#
# Per-kernel budget rationale (from the 2026-10-29 recon disassembly):
#   mask_words    2 loops (SWAR main + residue), branch-free bodies,
#                 3 guards => (2, 0, 3)
#   count_valid   2 loops, branch-free bodies, 3 guards   => (2, 0, 3)
#   mask_words_neon
#                 3 loops (V4 primary SIMD + V8 vectorized tail + scalar
#                 residue), all bodies branch-free, 4 guards => (3, 0, 4)
#   unpack_1bit   4 loops, 3 in-body per-chunk dispatch conds, 1 guard
#                                                             => (4, 3, 1)
#   unpack_2bit   same shape as 1bit                        => (4, 3, 1)
#   unpack_4bit   same shape as 1bit                        => (4, 3, 1)
#   unpack_8bit   1 loop with 6 backedges + 9 in-body per-chunk dispatch
#                 conds + 1 guard                           => (6, 9, 1)
KERNEL_CALLEES = [
    (r"10avec_arrow6bitmap10mask_words$", "mask_words", (2, 0, 3)),
    (r"10avec_arrow6bitmap11count_valid$", "count_valid", (2, 0, 3)),
    (r"10avec_arrow7silicon15mask_words_neon$", "mask_words_neon", (3, 0, 4)),
    (r"12avec_parquet6unpack11unpack_1bit$", "unpack_1bit", (4, 3, 1)),
    (r"12avec_parquet6unpack11unpack_2bit$", "unpack_2bit", (4, 3, 1)),
    (r"12avec_parquet6unpack11unpack_4bit$", "unpack_4bit", (4, 3, 1)),
    (r"12avec_parquet6unpack11unpack_8bit$", "unpack_8bit", (6, 9, 1)),
]

# Kernels whose primary vector loop must exist as a SIMD loop with a
# branch-free body (NEON EXTENT RULE above).
REQUIRE_VEC_PRIMARY = {"mask_words_neon"}

# PANIC-PATH budgets: calls into core::panicking (bounds checks, release
# paths) are C-class control flow — itemized per symbol, budget-pinned,
# never silently ignored. The unpack kernels each carry exactly one
# panic_bounds_check call (the audit-profile recon of 2026-10-29); the scan
# kernels and wrappers carry zero and MUST stay at zero (a bounds panic
# growing inside a scan kernel is a true violation: the kernel claims
# bounds-freedom by construction).
PANIC_BUDGETS = {
    "mask_words": 0, "count_valid": 0, "mask_words_neon": 0,
    "unpack_1bit": 1, "unpack_2bit": 1, "unpack_4bit": 1, "unpack_8bit": 1,
}

# CALLEE-LINKAGE LAW: every Rust-mangled call/tail-branch target leaving an
# audited extent must be covered by a KERNEL_CALLEES pattern. This is the
# anti-vacuity channel that the original gap exploited: the audit_* wrappers
# are 4-6 instruction shims ending in an UNCONDITIONAL tail `b` to the real
# kernel, and a bare `b` matches no conditional-branch regex — so a gate that
# scans only wrapper extents passes while every real body goes unaudited.
# Symmetrically, a future kernel called by an audited extent but absent from
# KERNEL_CALLEES would be unaudited; linkage makes that loud instead.
RUST_MANGLED_RE = re.compile(r"_RNv|_RINv|_RNvN")
PANIC_RE = re.compile(r"panic")

# arm64 + x86_64 conditional-branch mnemonics. Targets are parsed from the
# first operand token of the form 0x<hex>; an unparseable target is
# fail-closed (the conditional cannot be accounted => UNACCOUNTED).
COND_MNEMONICS = re.compile(
    r"^(b\.(eq|ne|cs|hs|mi|pl|vs|vc|hi|ls|ge|lt|gt|le|al|nv)"
    r"|cbz|cbnz|tbz|tbnz"
    r"|j(a|e|ne|g|l|le|ge|b|be|s|ns|o|no|p|np|c|nc)|jmp|loop|loopne|loope)$"
)
TARGET_RE = re.compile(r"^0x[0-9a-f]+$")
INSN_RE = re.compile(r"^\s*([0-9a-f]+):\s+(\S+)\s*(.*)$")
SIMD_RE = re.compile(r"\bq\d+\b")   # NEON q-registers (q0..q31)
ANNOT_RE = re.compile(r"<([^>]+)>")


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
    """Yield (name, body-lines) for each symbol extent found."""
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


def parse_insns(body):
    """Parse extent lines into (addr, mnemonic, operands) triples."""
    insns = []
    for line in body:
        m = INSN_RE.match(line)
        if m:
            insns.append((int(m.group(1), 16), m.group(2), m.group(3)))
    return insns


def branch_target(operands):
    """First bare 0x<hex> token; None if unparseable (fail-closed)."""
    for tok in operands.replace(",", " ").split():
        if TARGET_RE.match(tok):
            return int(tok, 16)
    return None


def classify(insns, budgets):
    """Account every conditional in the extent. Returns per-class lists."""
    latch_b, body_b, disp_b = budgets
    conds = []
    for addr, mnem, ops in insns:
        if COND_MNEMONICS.match(mnem):
            conds.append((addr, mnem, branch_target(ops)))

    loops = {}   # head -> list of latch addrs
    for addr, mnem, tgt in conds:
        if tgt is None:
            continue   # unparseable target: cannot be a loop; handled below
        if tgt < addr:
            loops.setdefault(tgt, []).append(addr)
    # primary extent per loop head: [head, max(latch)]
    loop_extent = {h: (h, max(l)) for h, l in loops.items()}

    latches, body_conds, dispatch = [], [], []
    unaccounted = []
    for addr, mnem, tgt in conds:
        item = f"0x{addr:x} {mnem} 0x{tgt:x}" if tgt is not None \
            else f"0x{addr:x} {mnem} <UNPARSED-TARGET>"
        if tgt is None:
            unaccounted.append(item)
        elif tgt < addr:
            latches.append(item)
        else:
            # innermost loop containing this conditional (strictly, so a
            # forward cond at a latch address is a LATCH above, not body)
            inside = [he for he in loop_extent.values() if he[0] <= addr < he[1]]
            if inside:
                body_conds.append(item)
            else:
                dispatch.append(item)
    return latches, body_conds, dispatch, unaccounted, loops


def simd_loop_heads(loops, insns):
    """Loop heads whose primary-extent body contains SIMD (q-register) ops."""
    heads = []
    for h, latches in sorted(loops.items(), key=lambda kv: kv[0]):
        lo, hi = h, max(latches)
        body_ops = [ops for a, m, ops in insns if lo <= a < hi]
        if any(SIMD_RE.search(o) for o in body_ops):
            heads.append(h)
    return heads


def audit_one(display, insns, budgets, require_vec, panic_budget):
    """Audit one extent. Returns (ok, report_lines, n_conditionals)."""
    latches, body_conds, dispatch, unaccounted, loops = classify(insns, budgets)
    lines = []
    fails = []

    # PANIC-PATH class: calls into core::panicking inside the extent.
    panic_paths = []
    for addr, mnem, ops in insns:
        if mnem in ("bl", "b") or mnem.startswith("call"):
            m = ANNOT_RE.search(ops)
            if m and PANIC_RE.search(m.group(1)):
                panic_paths.append(f"0x{addr:x} {mnem} {m.group(1)}")
    if len(panic_paths) > panic_budget:
        fails.append(f"PANIC-PATH {len(panic_paths)} > budget {panic_budget}")

    vec_primary = None
    vec_tail = []
    if require_vec:
        simd_heads = simd_loop_heads(loops, insns)
        if not simd_heads:
            fails.append("VEC-PRIMARY-LOOP-NOT-FOUND: no SIMD loop extent "
                         "in kernel (audited vector surface absent)")
        else:
            vec_primary = simd_heads[0]
            vec_tail = simd_heads[1:]
            lo_addr = vec_primary
            hi_addr = max(loops[vec_primary])
            body_conds_in = [
                c for c in body_conds
                if lo_addr <= int(c.split()[0], 16) < hi_addr
            ]
            if body_conds_in:
                fails.append(
                    f"VEC-PRIMARY body 0x{lo_addr:x}..0x{hi_addr:x} carries "
                    f"{len(body_conds_in)} conditional(s): {body_conds_in}")

    if len(latches) > budgets[0]:
        fails.append(f"LATCH {len(latches)} > budget {budgets[0]}")
    if len(body_conds) > budgets[1]:
        fails.append(f"BODY-COND {len(body_conds)} > budget {budgets[1]}")
    if len(dispatch) > budgets[2]:
        fails.append(f"DISPATCH {len(dispatch)} > budget {budgets[2]}")
    if unaccounted:
        fails.append(f"UNACCOUNTED: {unaccounted}")

    verdict = "OK" if not fails else "FAIL"
    line = (f"{display:44s} LATCH {len(latches)}/{budgets[0]}  "
            f"BODY {len(body_conds)}/{budgets[1]}  "
            f"DISPATCH {len(dispatch)}/{budgets[2]}  "
            f"PANIC {len(panic_paths)}/{panic_budget}  {verdict}")
    if vec_primary is not None:
        line += f"  VEC-PRIMARY@0x{vec_primary:x} body=0cond"
    if vec_tail:
        line += f"  VEC-TAIL x{len(vec_tail)}@0x{vec_tail[0]:x}"
    lines.append(line)

    for cls, items in (("LATCH", latches), ("BODY-COND", body_conds),
                       ("DISPATCH", dispatch), ("PANIC-PATH", panic_paths)):
        for it in items:
            lines.append(f"    {cls:9s} {it}")
    for it in unaccounted:
        lines.append(f"    UNACCOUNTED {it}")
    for f in fails:
        lines.append(f"    !! {f}")
    n_cond = len(latches) + len(body_conds) + len(dispatch)
    return (not fails), lines, n_cond, len(body_conds)


def rust_call_targets(insns):
    """(addr, mnemonic, target-symbol) for call/tail-b with Rust-mangled targets."""
    out = []
    for addr, mnem, ops in insns:
        if mnem in ("bl", "b", "call", "jmp"):
            m = ANNOT_RE.search(ops)
            if m and RUST_MANGLED_RE.search(m.group(1)):
                out.append((addr, mnem, m.group(1)))
    return out


def main():
    fails = []
    total_itemized = 0
    kernels_scanned = 0
    branch_free_kernels = 0
    total_panic = 0
    audited_symbols = []   # (extent-name, [rust call targets])

    for art in ARTIFACTS:
        text = disassemble(art)
        extents = list(kernel_extents(text))

        def find(pred):
            return [(n, b) for n, b in extents
                    if pred(n[1:] if n.startswith("_") else n)]

        # 1) wrappers: stubs, must be fully branch-free
        for w in WRAPPERS:
            found = find(lambda n: n == w)
            if not found:
                fails.append(f"MISSING wrapper extent: {w}")
                print(f"{w:44s} MISSING")
                continue
            for name, body in found:
                insns = parse_insns(body)
                ok, lines, n_cond, _n_body = audit_one(
                    w, insns, (0, 0, 0), False, 0)
                for l in lines:
                    print(l)
                if not ok:
                    fails.append(f"{w}: budget/class violation")
                total_itemized += n_cond
                audited_symbols.append((name, rust_call_targets(insns)))

        # 2) transitive kernel callees: classified + budget-pinned
        for pat, disp, budgets in KERNEL_CALLEES:
            found = find(lambda n, p=pat: re.search(p, n) is not None)
            if not found:
                fails.append(f"MISSING kernel extent: {disp} (pattern {pat})")
                print(f"{disp:44s} MISSING")
                continue
            for name, body in found:
                insns = parse_insns(body)
                ok, lines, n_cond, n_body = audit_one(
                    disp, insns, budgets, disp in REQUIRE_VEC_PRIMARY,
                    PANIC_BUDGETS[disp])
                for l in lines:
                    print(l)
                if not ok:
                    fails.append(f"{disp}: budget/class violation")
                total_itemized += n_cond
                total_panic += sum(
                    1 for l in lines if l.startswith("    PANIC-PATH"))
                kernels_scanned += 1
                # branch-free inner loop = zero conditionals inside any loop
                # body (the charter surface); latches/dispatch still itemized.
                if n_body == 0:
                    branch_free_kernels += 1
                audited_symbols.append((name, rust_call_targets(insns)))

        # 3) CALLEE-LINKAGE: every Rust-mangled call target leaving an
        #    audited extent must be covered by a KERNEL_CALLEES pattern.
        #    Two accounted exceptions, both C-class with their own receipts:
        #    - intra-extent branches: target annotation is the extent's own
        #      symbol (with a +offset) — that IS the audited body;
        #    - panic/runtime calls — accounted by the PANIC-PATH class above.
        for name, targets in audited_symbols:
            plain = name[1:] if name.startswith("_") else name
            for addr, mnem, tgt in targets:
                covered = any(re.search(p, tgt) for p, _, _ in KERNEL_CALLEES)
                base = tgt.split("+", 1)[0]
                if covered or base == name or PANIC_RE.search(tgt):
                    continue
                fails.append(
                    f"CALLEE-LINKAGE: {plain} 0x{addr:x} {mnem} -> "
                    f"{tgt} is Rust-mangled but not in KERNEL_CALLEES "
                    f"(unaudited kernel body)")

    if fails:
        print(f"\nBRANCH AUDIT FAIL ({len(fails)}):")
        for f in fails:
            print(f"  - {f}")
        sys.exit(1)
    print(
        f"\nBRANCH AUDIT PASS: branch-free inner loops: "
        f"{branch_free_kernels}/{kernels_scanned} kernels; "
        f"loop-latch/dispatch conditionals itemized: {total_itemized} total "
        f"across {kernels_scanned} kernels (+{len(WRAPPERS)} branch-free "
        f"wrapper shims); panic/bounds paths: {total_panic}; unaccounted: 0"
    )


if __name__ == "__main__":
    main()
