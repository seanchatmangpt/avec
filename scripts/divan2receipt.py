#!/usr/bin/env python3
"""divan2receipt.py — fail-closed Divan -> Turtle receipt turnstile.

Wave-5 lane 7, v26.10.31 (spec: docs/jira/v26.10.31/29-benchmark-harness-plan.md §4,
vocabulary: docs/jira/v26.10.31/04-benchmark-contract.md).

DISCLOSED CHOICE: Divan 0.1.21 has NO JSON output mode (witnessed via
`--help`: only filter/color/sort/timer/sample/counter options exist). This
converter therefore parses Divan's standard bench-mode stdout table, exactly
as printed by:

    cargo bench -p bench --bench v26_10_31 -- --color never

Usage:
    divan2receipt.py <divan-stdout.txt> [-o OUT.ttl]

Bench -> CTQ mapping:
    blake3_hash_64mib                     -> CTQ-06 (GATE, LSL 8.5 GiB/s)
    quad_transpose_64k                    -> CTQ-04-adjacent (informational)
    predicate_filter_64k                  -> informational
    parquet_level_free_roundtrip_u64_64k  -> informational

Turnstile semantics (fail-closed):
    - exit 0 + receipt emitted   only if the gated CTQ passes;
    - exit 1 + NO receipt        if the gated CTQ breaches the LSL;
    - exit 1 if the table is unparseable or a required bench is missing —
      missing evidence is never a silent pass.

Non-vacuity: a mutated sub-threshold table must produce exit 1 and no
receipt; a superseding table must pass. A court that cannot flip is FAIL.
"""

import argparse
import sys

# LSL from the 04-benchmark-contract.md threshold table (BLAKE3 core throughput).
LSL_GIBS_CTQ06 = 8.5

# bench name -> (ctq, kind, predicate, unit)
#   kind "gate": breach blocks receipt emission and exits 1.
#   kind "info": recorded in the receipt, never blocks.
#   unit "gibs": throughput from the Divan continuation row (bytes counter).
#   unit "time": fastest time from the main row.
# Predicate note: 04-benchmark-contract.md pins the PerformanceReceiptContract
# header properties (targetCrate/predecessor/dataPlane/conformanceSuite) and
# the logic/physical plane shape predicates (literalEncodingGiBs,
# dataPageV2DecodingGiBs, ...). CTQ-06 BLAKE3 has no pinned predicate in the
# contract table, so this converter mints stardf:blake3ThroughputGiBs and the
# informational time predicates in the same stardf: namespace — a disclosed
# extension, not a contract predicate.
CTQ_MAP = {
    "blake3_hash_64mib": ("CTQ-06", "gate", "blake3ThroughputGiBs", "gibs"),
    "quad_transpose_64k": ("CTQ-04-adjacent", "info", "quadTransposeFastestNs", "time"),
    "predicate_filter_64k": ("informational", "info", "predicateFilterFastestNs", "time"),
    "parquet_level_free_roundtrip_u64_64k": ("informational", "info", "levelFreeRoundtripFastestNs", "time"),
}

TTL_PREAMBLE = """@prefix stardf: <https://stardf.dev/vocab#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

# stardf:PerformanceReceiptContract instance (04-benchmark-contract.md).
# Contract header properties are pinned by the contract; measured-value
# predicates are emitted in the stardf: namespace (see CTQ_MAP note).
stardf:contract-v26_10_31 a stardf:PerformanceReceiptContract ;
    stardf:targetCrate "cgap" ;
    stardf:predecessor "purrdf" ;
    stardf:dataPlane "cgap" ;
    stardf:conformanceSuite "W3C-RDF-1.2-SHACL-RDFC-1.0" ;
    stardf:benchInvocation "cargo bench -p bench --bench v26_10_31 -- --color never" ;
    stardf:divanVersion "0.1.21" ;
"""

TIME_UNITS = (
    ("ms", 1e-3), ("µs", 1e-6), ("us", 1e-6), ("ns", 1e-9), ("s", 1.0),
)
BYTE_RATE_UNITS = (
    ("GiB/s", 1 << 30), ("MiB/s", 1 << 20), ("KiB/s", 1 << 10),
    ("GB/s", 1e9), ("MB/s", 1 << 20), ("KB/s", 1 << 10), ("B/s", 1.0),
)


def parse_time_ns(s):
    s = s.strip()
    for suffix, mult in TIME_UNITS:
        if s.endswith(suffix):
            try:
                return float(s[: -len(suffix)].strip()) * mult * 1e9
            except ValueError:
                return None
    return None


def parse_table(text):
    """Parse Divan stdout. Returns {bench: {'fastest_ns','median_ns','bytes_per_s'}}."""
    res = {}
    pending = None
    for raw in text.splitlines():
        if "│" not in raw:
            pending = None
            continue
        label, rest = raw.split("│", 1)
        cells = [c.strip() for c in rest.split("│")]
        # Divan's first column holds "<tree glyph> <bench name> <fastest>"
        # merged into one cell; recover the name by prefix match.
        stripped = label.strip()
        name = None
        rest_label = ""
        for glyph in ("├─", "╰─", "┌─"):
            if stripped.startswith(glyph):
                rest_label = stripped[len(glyph):].lstrip()
                for bench in CTQ_MAP:
                    if rest_label.startswith(bench):
                        name = bench
                        break
        if name is not None:
            # Remaining cells after the first │: slowest, median, mean, samples, iters.
            median = parse_time_ns(cells[1]) if len(cells) > 1 else None
            res[name] = {"fastest_ns": parse_time_ns(rest_label[len(name):].strip()),
                         "median_ns": median, "bytes_per_s": None}
            pending = name
        elif pending is not None:
            for cell in cells:
                matched = False
                for unit, mult in BYTE_RATE_UNITS:
                    if cell.endswith(unit):
                        try:
                            res[pending]["bytes_per_s"] = (
                                float(cell[: -len(unit)].strip()) * mult
                            )
                        except ValueError:
                            pass
                        matched = True
                        break
                if matched:
                    break
    return res


def render_turtle(measured_rows, gate_pass):
    lines = [TTL_PREAMBLE]
    lines.append('    stardf:gateStatus "%s" .\n' % ("PASS" if gate_pass else "FAIL"))
    for bench, (ctq, kind, pred, unit) in CTQ_MAP.items():
        if bench not in measured_rows:
            continue
        m = measured_rows[bench]
        subj = "stardf:bench-%s" % bench
        lines.append("# unit: %s" % unit)
        lines.append("%s a stardf:BenchResult ;" % subj)
        lines.append('    stardf:ctq "%s" ;' % ctq)
        lines.append('    stardf:gateClass "%s" ;' % kind)
        if m.get("bytes_per_s") is not None:
            gib_s = m["bytes_per_s"] / (1 << 30)
            lines.append("    stardf:%s %.6f ;" % (pred, gib_s))
        if m.get("fastest_ns") is not None:
            lines.append("    stardf:fastestTimeNs %.1f ;" % m["fastest_ns"])
        if m.get("median_ns") is not None:
            lines.append("    stardf:medianTimeNs %.1f ;" % m["median_ns"])
        lines[-1] = lines[-1].rstrip(" ;") + " .\n"
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser(description="Divan stdout -> Turtle receipt (fail-closed).")
    ap.add_argument("divan_output")
    ap.add_argument("-o", "--out", help="receipt output path (Turtle)")
    args = ap.parse_args()

    with open(args.divan_output, encoding="utf-8") as f:
        text = f.read()

    # Fail-closed on parse: the "Timer precision" line is the table's witness;
    # without the table, no CTQ can be asserted.
    if "Timer precision" not in text:
        print("REFUSED: no Divan table witness ('Timer precision') in input", file=sys.stderr)
        return 1
    measured = parse_table(text)
    missing = [b for b in CTQ_MAP if b not in measured]
    if missing:
        print("REFUSED: required benches missing from Divan output: %s" % ", ".join(missing), file=sys.stderr)
        return 1

    m6 = measured["blake3_hash_64mib"]
    bps = m6.get("bytes_per_s")
    if bps is None:
        print("REFUSED: blake3_hash_64mib has no bytes throughput row (BytesCount missing)", file=sys.stderr)
        return 1
    gib_s = bps / (1 << 30)
    gate_pass = gib_s >= LSL_GIBS_CTQ06

    print("CTQ-06 blake3_hash_64mib: %.4f GiB/s vs LSL %.2f GiB/s -> %s"
          % (gib_s, LSL_GIBS_CTQ06, "PASS" if gate_pass else "FAIL"))

    if not gate_pass:
        # Turnstile: no receipt on violation.
        print("TURNSTILE: CTQ-06 breach — receipt NOT emitted, exit 1", file=sys.stderr)
        return 1

    ttl = render_turtle(measured, gate_pass)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(ttl)
        print("receipt: %s" % args.out)
    else:
        print(ttl, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
