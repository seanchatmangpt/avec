#!/usr/bin/env python3
"""Data-independence timing gate for the cgap hot-path kernels (lane PCL4).

Branch-free control flow is not constant time. This gate MEASURES the real
max/min timing ratios of the hot kernels across the input contrasts the
existing bench binaries embed, calibrates each contrast against a measured
same-input noise envelope (cross-run max/min of per-run values), and reports
verdicts. It never picks a bound to pass: the only threshold is the measured
noise envelope of the metric itself.

Two data sources:

1. TODAY (default): repeated runs of the existing bench binaries
   (`target/release/bench` = mask_words selectivity sweep, `dict_bench`,
   `arrow_accessors_bench`). These embed fixed-vs-varied input contrasts:
   - bench:       4 predicate selectivities (0/64 .. 64/64 bits) of one
                  fixed-stride AND -> CONTENT contrast (values differ,
                  addresses do not).
   - dict_bench:  cardinality {2,16,256} -> data distribution AND table
                  size contrast (build / encode / decode / literal_*).
   - arrow bench: views inline-heavy vs foreign-heavy (the ViewVec::row
                  12-byte threshold classifier, opposite sides) -> shape
                  contrast; plus single-shape stability for the rest.
2. `--const-file PATH`: consume a const_time_bench output file when present
   (that binary is a concurrent lane, not on disk at gate time). Tolerant
   parse: JSON `{"kernel": {"cases": [{"name", "ns_per_iter_min"}, ...]}}`
   or lines `<kernel> <case> <value> <unit>`. ANTICIPATED format, reconciled
   when that lane lands; a missing file is a loud SKIP line, never a silent
   pass.

Exit codes: 0 measurement recorded; 1 gate breach (only in --claim mode:
a kernel claimed `oblivious` shows a contrast ratio beyond its noise
envelope, or has no measurable contrast to verify the claim); 2 environment
error (missing binary, missing expected bench lines). No claim -> no pass
vocabulary for data-dependence itself: data-dependence is reported, not
penalized. The register of classifications lives in
docs/DATA_INDEPENDENCE_AUDIT.md.

Usage:
    python3 scripts/timing_gate.py                     # measure + report
    python3 scripts/timing_gate.py --runs 5            # tighter envelopes
    python3 scripts/timing_gate.py --claim mask_words=oblivious
    python3 scripts/timing_gate.py --const-file target/const_time_bench.json
    python3 scripts/timing_gate.py --json /tmp/receipt.json --runs 3
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN_DIR = ROOT / "target" / "release"
DEFAULT_TIMEOUT_S = 900

# `pred 0x0000000000000000 MIN  63.460 GiB/s (2.955 ms/round)`
MASK_MIN_RE = re.compile(
    r"pred\s+(?P<pred>0x[0-9a-f]{16})\s+MIN\s+(?P<gbps>\d+(?:\.\d+)?)\s+GiB/s"
)
# `dict 2 build: 0.690 GiB/s, 26 allocs`
DICT_RE = re.compile(r"^dict\s+(\d+)\s+(\S+):\s+(\d+(?:\.\d+)?)\s+GiB/s", re.M)
# `varlen u32 row-scan (...): 5.256 GiB/s, 88.2 Mrows/s, 0 allocs/iter`
# `runend get binary-search (...): 185.3 Mrows/s, 0 allocs/iter` (no GiB/s)
ARROW_RE = re.compile(
    r"^(?P<op>\w+)\s+(?P<shape>.*?):\s+(?:\d+(?:\.\d+)\s+GiB/s,\s+)?"
    r"(?P<mrows>\d+(?:\.\d+)?)\s+Mrows/s",
    re.M,
)


def run_binary(bin_path: Path, timeout_s: int) -> str:
    """Run one prebuilt bench binary, return stdout, fail loudly."""
    if not bin_path.exists():
        print(f"[timing_gate] ENV-FAIL: binary missing: {bin_path}")
        print(
            "[timing_gate] the gate never builds (concurrent lanes own the cargo "
            "lock); build it once with `cargo run --release -p bench --bin "
            f"{bin_path.stem}` and re-run the gate"
        )
        raise SystemExit(2)
    t0 = time.monotonic()
    try:
        proc = subprocess.run(
            [str(bin_path)],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            start_new_session=True,
        )
    except subprocess.TimeoutExpired:
        print(f"[timing_gate] FAIL({bin_path.name}): exceeded {timeout_s}s wall budget")
        raise SystemExit(1)
    dt = time.monotonic() - t0
    if proc.returncode != 0:
        print(f"[timing_gate] FAIL({bin_path.name}): exit {proc.returncode}")
        if proc.stderr:
            print(proc.stderr.strip()[-1000:])
        raise SystemExit(1)
    print(f"[timing_gate] {bin_path.name}: exit 0 in {dt:.1f}s")
    return proc.stdout


def parse_mask(out: str) -> dict[str, float]:
    d = {m.group("pred"): float(m.group("gbps")) for m in MASK_MIN_RE.finditer(out)}
    if len(d) != 4:
        print(f"[timing_gate] FAIL(bench): expected 4 pred MIN lines, got {len(d)}")
        raise SystemExit(1)
    return d


def parse_dict(out: str) -> dict[str, float]:
    d = {f"{m.group(1)}|{m.group(2)}": float(m.group(3)) for m in DICT_RE.finditer(out)}
    want = {f"{c}|{op}" for c in (2, 16, 256) for op in
            ("build", "encode", "decode", "literal_encode", "literal_decode")}
    missing = want - set(d)
    if missing:
        print(f"[timing_gate] FAIL(dict_bench): missing lines: {sorted(missing)}")
        raise SystemExit(1)
    return d


def parse_arrow(out: str) -> dict[str, float]:
    d = {}
    for m in ARROW_RE.finditer(out):
        shape = m.group("shape")
        if "inline" in shape:
            key = "views|inline"
        elif "foreign" in shape:
            key = "views|foreign"
        else:
            key = m.group("op") + "|" + shape.split("(")[0].strip()
        d[key] = float(m.group("mrows"))
    want = {"varlen|u32 row-scan", "views|inline", "views|foreign",
            "sliced|get full iter", "primitive|values direct iter",
            "runend|get binary-search", "runend|decode_to_dense"}
    missing = want - set(d)
    if missing:
        print(f"[timing_gate] FAIL(arrow_accessors_bench): missing lines: {sorted(missing)}")
        raise SystemExit(1)
    return d


def maxmin(vals: list[float]) -> float:
    return max(vals) / min(vals)


def envelope_table(
    title: str,
    per_run: dict[str, dict[str, float]],
    contrasts: list[tuple[str, str, str]],
    unit: str = "GiB/s",
) -> dict:
    """Cross-run envelopes per metric + within-run contrast ratios."""
    print(f"\n== {title} ==")
    all_keys = sorted(next(iter(per_run.values())))
    env: dict[str, float] = {}
    for k in all_keys:
        vals = [per_run[r][k] for r in per_run]
        env[k] = maxmin(vals)
        print(f"  {k:34s}: {min(vals):9.3f}..{max(vals):9.3f} {unit}  "
              f"envelope {env[k]:.3f}  n={len(vals)}")
    rows = []
    print(f"  {'contrast':36s} {'mean L':>8s} {'mean R':>8s} {'ratio':>7s} "
          f"{'envelope':>8s}  verdict")
    for name, a, b in contrasts:
        mean_a = sum(per_run[r][a] for r in per_run) / len(per_run)
        mean_b = sum(per_run[r][b] for r in per_run) / len(per_run)
        ratio = max(mean_a, mean_b) / min(mean_a, mean_b)
        per_run_ratios = sorted(maxmin([per_run[r][a], per_run[r][b]])
                                for r in per_run)
        med = per_run_ratios[len(per_run_ratios) // 2]
        env_pair = max(env.get(a, 1.0), env.get(b, 1.0))
        beyond = ratio > env_pair
        verdict = "BEYOND-NOISE" if beyond else "within-noise"
        print(f"  {name:36s} {mean_a:8.3f} {mean_b:8.3f} {ratio:7.3f} "
              f"{env_pair:8.3f}  {verdict} (median per-run {med:.3f})")
        rows.append(
            {"contrast": name, "mean_left": round(mean_a, 4),
             "mean_right": round(mean_b, 4), "mean_ratio": round(ratio, 4),
             "median_per_run_ratio": round(med, 4), "envelope": round(env_pair, 4),
             "beyond_noise": beyond}
        )
    return {"envelopes": {k: round(v, 4) for k, v in env.items()}, "contrasts": rows}


def const_file_stats(path: Path) -> dict | None:
    """Consume a const_time_bench output file when present (loud skip if not)."""
    if not path.exists():
        print(f"[timing_gate] [SKIP] const-time file absent: {path} "
              "(concurrent lane's binary/output not landed yet — loud skip, not a pass)")
        return None
    text = path.read_text()
    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        data = None
    stats: dict[str, list[float]] = {}
    if isinstance(data, dict):
        for kernel, body in data.items():
            cases = body.get("cases", []) if isinstance(body, dict) else []
            vals = [float(c["ns_per_iter_min"]) for c in cases
                    if isinstance(c, dict) and "ns_per_iter_min" in c]
            if vals:
                stats[kernel] = vals
    if not stats:
        for line in text.splitlines():
            parts = line.split()
            if len(parts) >= 4:
                try:
                    val = float(parts[2])
                except ValueError:
                    continue
                stats.setdefault(parts[0], []).append(val)
    if not stats:
        print(f"[timing_gate] [SKIP] could not parse {path} (neither JSON cases nor lines)")
        return None
    print(f"\n== const_time_bench ({path}) ==")
    out = {}
    for kernel, vals in sorted(stats.items()):
        print(f"  {kernel}: {len(vals)} case(s), max/min {maxmin(vals):.3f}")
        out[kernel] = {"cases": len(vals), "max_min_ratio": round(maxmin(vals), 4)}
    return out


CLAIM_KERNEL_CONTRASTS = {
    "mask_words": ("mask_words", None),
    "dict_build": ("dict", ["2|build vs 256|build"]),
    "dict_gather": ("dict", ["2|decode vs 256|decode"]),
    "decode_into": ("dict", ["2|literal_decode vs 256|literal_decode"]),
    "rle_encode_fast": ("dict", ["2|encode vs 256|encode"]),
    "viewvec_row": ("arrow_accessors", None),
    # no measurable input contrast exists in today's binaries: an oblivious
    # claim on these is UNVERIFIABLE and must refuse loudly, never pass
    "varlen_row": ("arrow_accessors", []),
    "runend_get": ("arrow_accessors", []),
    "runend_decode_to_dense": ("arrow_accessors", []),
}


def main() -> None:
    ap = argparse.ArgumentParser(description="cgap data-independence timing gate")
    ap.add_argument("--runs", type=int, default=3,
                    help="runs per binary for the noise envelope (default 3)")
    ap.add_argument("--bin-dir", type=Path, default=BIN_DIR)
    ap.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT_S)
    ap.add_argument("--json", type=Path, help="write the receipt JSON here")
    ap.add_argument("--claim", action="append", default=[],
                    metavar="KERNEL=oblivious",
                    help="assert a kernel's measured contrasts stay within its "
                         "noise envelope; breach exits 1 (repeatable)")
    ap.add_argument("--const-file", type=Path,
                    default=ROOT / "target" / "const_time_bench.json",
                    help="const_time_bench output file to consume when present")
    args = ap.parse_args()

    claims = {}
    for c in args.claim:
        if "=" not in c or not c.endswith("=oblivious"):
            print(f"[timing_gate] ENV-FAIL: --claim wants KERNEL=oblivious, got {c!r}")
            raise SystemExit(2)
        claims[c.split("=", 1)[0]] = "oblivious"

    receipt: dict = {"runs_per_binary": args.runs}
    if args.runs < 2:
        print("[timing_gate] WARNING: --runs 1 makes every noise envelope exactly 1.0 "
              "(degenerate): every contrast will read BEYOND-NOISE. Envelopes need >= 2 runs.")

    # ---- bench (mask_words): CONTENT contrast, fixed addresses ----
    mask_runs = {}
    for i in range(args.runs):
        out = run_binary(args.bin_dir / "bench", args.timeout)
        mask_runs[f"run{i}"] = parse_mask(out)
    mask_stats = envelope_table(
        "mask_words (fixed-stride AND; content = predicate selectivity)",
        mask_runs,
        [
            ("sparse 0x1111 vs dense 0xffff", "0x1111111111111111", "0xffffffffffffffff"),
            ("empty 0x0000 vs dense 0xffff", "0x0000000000000000", "0xffffffffffffffff"),
        ],
    )
    receipt["mask_words"] = mask_stats

    # ---- dict_bench: distribution + table-size contrasts ----
    dict_runs = {}
    for i in range(args.runs):
        out = run_binary(args.bin_dir / "dict_bench", args.timeout)
        dict_runs[f"run{i}"] = parse_dict(out)
    dict_stats = envelope_table(
        "dict kernels (cardinality 2 vs 256: distribution + table size)",
        dict_runs,
        [
            ("2|build vs 256|build", "2|build", "256|build"),
            ("2|encode vs 256|encode", "2|encode", "256|encode"),
            ("2|decode vs 256|decode", "2|decode", "256|decode"),
            ("2|literal_decode vs 256|literal_decode",
             "2|literal_decode", "256|literal_decode"),
        ],
    )
    receipt["dict"] = dict_stats

    # ---- arrow_accessors_bench: ViewVec classifier shape contrast ----
    arrow_runs = {}
    for i in range(args.runs):
        out = run_binary(args.bin_dir / "arrow_accessors_bench", args.timeout)
        arrow_runs[f"run{i}"] = parse_arrow(out)
    arrow_stats = envelope_table(
        "arrow accessors (views inline vs foreign = 12-byte threshold sides)",
        arrow_runs,
        [("views inline vs foreign", "views|inline", "views|foreign")],
        unit="Mrows/s",
    )
    receipt["arrow_accessors"] = arrow_stats

    ct = const_file_stats(args.const_file)
    if ct:
        receipt["const_time_bench"] = ct

    # ---- claims ----
    family = {"mask_words": mask_stats, "dict": dict_stats,
              "arrow_accessors": arrow_stats}
    breach = False
    if claims:
        print("\n== claims ==")
        for kernel in claims:
            if kernel not in CLAIM_KERNEL_CONTRASTS:
                print(f"  {kernel}: UNKNOWN KERNEL — no contrast mapping (claim unverified)")
                breach = True
                continue
            fam, names = CLAIM_KERNEL_CONTRASTS[kernel]
            rows = family[fam]["contrasts"]
            if names is not None:
                rows = [r for r in rows if r["contrast"] in names]
            if not rows:
                print(
                    f"  {kernel}: NO MEASURABLE CONTRAST in today's binaries — "
                    "an oblivious claim is UNVERIFIABLE here, refused (loud)"
                )
                breach = True
                continue
            for row in rows:
                ok = not row["beyond_noise"]
                print(f"  {kernel}: {row['contrast']}: ratio {row['mean_ratio']:.3f} "
                      f"vs envelope {row['envelope']:.3f} -> "
                      f"{'HOLDS' if ok else 'BREACH'}")
                breach = breach or not ok

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        print(f"\n[timing_gate] receipt written: {args.json}")

    if breach:
        print("[timing_gate] GATE FAIL: an oblivious claim is contradicted (or unverifiable)")
        raise SystemExit(1)
    print("[timing_gate] MEASURE OK (data-dependence reported, not penalized; "
          "claims gated where made)")


if __name__ == "__main__":
    main()
