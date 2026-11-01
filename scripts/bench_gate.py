#!/usr/bin/env python3
"""Bench regression gate for the cgap mask + parquet + page benches.

Runs all three Gate-2 bench binaries (`cargo run --release -p bench --bin
bench|parquet_bench|page_bench`), parses the MIN throughput receipt lines,
and either (a) records them as floors with --record, writing
docs/BENCH_FLOORS.json as
{"mask_min_gbps": {...}, "parquet": {...}, "page": {...}}, or (b) gates the
fresh run against previously recorded floors (default mode): exit 1 when any
metric drops more than --threshold percent (default 25%) below its floor.

MACHINE-LOCAL: the floors in docs/BENCH_FLOORS.json are receipts of one run
on one machine under one load profile. They are NOT portable numbers — never
compare floors recorded on machine A against observations from machine B, and
re-record whenever the concurrent-lane load profile changes structurally
(they were, by construction, seeded under concurrent build/bench load; that
makes them conservative-low, the safe direction for a drop gate). The 25%
margin exists to absorb that same noise: this is regression detection, not
precision measurement.

Exit codes: 0 gate pass / record ok; 1 gate FAIL (a floor was breached, an
expected bench line was missing, or a bench timed out); 2 environment error
(missing floors file, unreadable output). A bench that fails to produce its
expected lines is a loud FAIL, never a silent pass.

Usage:
    python3 scripts/bench_gate.py --record          # seed floors
    python3 scripts/bench_gate.py                   # gate vs floors
    python3 scripts/bench_gate.py --threshold 30    # custom margin
    python3 scripts/bench_gate.py --floors PATH     # alt floors file (probe)
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_FLOORS = ROOT / "docs" / "BENCH_FLOORS.json"

# 900s covers a cold release build (LTO) plus both benches (~10 s of benching
# when the kernels are healthy); a healthy run finishes in well under a
# minute after build. Witnessed failure mode this guards against: a
# concurrent lane's mid-edit kernel compiled into the bench binary turned a
# 1 s bench into a >14 min hang (2026-10-29, rle_encode). The gate must fail
# loudly on a wall-clock budget instead of hanging forever.
DEFAULT_TIMEOUT_S = 900

# mask bench MIN line, e.g.:
#   pred 0x0000000000000000 MIN  68.932 GiB/s (2.720 ms/round)
MASK_MIN_RE = re.compile(
    r"^\s*pred\s+(?P<pred>0x[0-9a-f]{16})\s+MIN\s+(?P<gbps>\d+(?:\.\d+)?)\s+GiB/s",
    re.MULTILINE,
)
# parquet op line, e.g.:
#   parquet unpack random64 1: 38.312 GiB/s, 0 allocs
PARQUET_RE = re.compile(
    r"^parquet\s+(?P<op>\S+)\s+(?P<shape>\S+)\s+(?P<width>\d+):\s+"
    r"(?P<gbps>\d+(?:\.\d+)?)\s+GiB/s",
    re.MULTILINE,
)
# page op line, e.g.:
#   page decode random64 1: 38.312 GiB/s, 0 allocs
#   page encode literal: 12.500 GiB/s
# (optional width token before the colon; keyed with /wN only when present)
PAGE_RE = re.compile(
    r"^page\s+(?P<op>\S+)\s+(?P<shape>\S+)(?:\s+(?P<width>\d+))?:\s+"
    r"(?P<gbps>\d+(?:\.\d+)?)\s+GiB/s",
    re.MULTILINE,
)


def run_bench(bin_name: str, timeout_s: int) -> str:
    """Run one bench binary in the workspace, return stdout, fail loudly."""
    cmd = ["cargo", "run", "--release", "-p", "bench", "--bin", bin_name]
    print(f"[bench_gate] $ {' '.join(cmd)}")
    t0 = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_s,
            # Own process group: on timeout, kill the whole tree (cargo +
            # bench child), so a hung kernel cannot orphan behind the gate.
            start_new_session=True,
        )
    except subprocess.TimeoutExpired:
        print(
            f"[bench_gate] FAIL({bin_name}): exceeded {timeout_s}s wall budget — "
            "treating as gate breach (hung/sick kernel, no floors may pass on it)"
        )
        raise SystemExit(1)
    dt = time.monotonic() - t0
    if proc.returncode != 0:
        print(f"[bench_gate] FAIL({bin_name}): exit {proc.returncode}")
        if proc.stderr:
            print(proc.stderr.strip()[-2000:])
        raise SystemExit(1)
    print(f"[bench_gate] {bin_name}: exit 0 in {dt:.1f}s")
    if proc.stdout:
        print("---- bench stdout (verbatim) ----")
        print(proc.stdout.rstrip())
        print("---- end bench stdout ----")
    return proc.stdout


def parse_mask_mins(stdout: str) -> dict[str, float]:
    """Extract per-selectivity-pattern MIN GiB/s from mask bench stdout."""
    out = {m.group("pred"): float(m.group("gbps")) for m in MASK_MIN_RE.finditer(stdout)}
    if not out:
        print("[bench_gate] FAIL: no `pred 0x... MIN ... GiB/s` lines found in mask bench output")
        raise SystemExit(1)
    return out


def parse_parquet_lines(stdout: str) -> dict[str, float]:
    """Extract `parquet <op> <shape> <width>: <gbps> GiB/s` lines."""
    out = {
        f"{m.group('op')}/{m.group('shape')}/w{m.group('width')}": float(m.group("gbps"))
        for m in PARQUET_RE.finditer(stdout)
    }
    if not out:
        print("[bench_gate] FAIL: no `parquet ... GiB/s` lines found in parquet_bench output")
        raise SystemExit(1)
    return out


def parse_page_lines(stdout: str) -> dict[str, float]:
    """Extract `page <op> <shape>[ <width>]: <gbps> GiB/s` lines."""
    out = {}
    for m in PAGE_RE.finditer(stdout):
        key = f"{m.group('op')}/{m.group('shape')}"
        if m.group("width"):
            key += f"/w{m.group('width')}"
        out[key] = float(m.group("gbps"))
    if not out:
        print("[bench_gate] FAIL: no `page ... GiB/s` lines found in page_bench output")
        raise SystemExit(1)
    return out


def collect(timeout_s: int) -> tuple[dict[str, float], dict[str, float], dict[str, float]]:
    """Run all three benches and parse all three receipt families."""
    mask_out = run_bench("bench", timeout_s)
    parquet_out = run_bench("parquet_bench", timeout_s)
    page_out = run_bench("page_bench", timeout_s)
    return parse_mask_mins(mask_out), parse_parquet_lines(parquet_out), parse_page_lines(page_out)


def write_floors(
    path: Path,
    mask: dict[str, float],
    parquet: dict[str, float],
    page: dict[str, float],
) -> None:
    """Write the floors file atomically (tmp + rename)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {"mask_min_gbps": mask, "parquet": parquet, "page": page}
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    os.replace(tmp, path)
    print(f"[bench_gate] floors written: {path}")


def main() -> None:
    ap = argparse.ArgumentParser(description="mask/parquet/page bench regression gate")
    ap.add_argument(
        "--record",
        action="store_true",
        help="seed docs/BENCH_FLOORS.json from this run instead of gating",
    )
    ap.add_argument(
        "--threshold",
        type=float,
        default=25.0,
        help="fail when a metric drops more than this many %% below its floor (default 25)",
    )
    ap.add_argument(
        "--floors",
        type=Path,
        default=DEFAULT_FLOORS,
        help="floors file (default docs/BENCH_FLOORS.json)",
    )
    ap.add_argument(
        "--timeout",
        type=int,
        default=DEFAULT_TIMEOUT_S,
        help=f"per-bench wall budget in seconds (default {DEFAULT_TIMEOUT_S})",
    )
    args = ap.parse_args()

    mask, parquet, page = collect(args.timeout)

    if args.record:
        write_floors(args.floors, mask, parquet, page)
        for family, vals in (("mask", mask), ("parquet", parquet), ("page", page)):
            for k in sorted(vals):
                print(f"[bench_gate] floor {family}/{k} = {vals[k]:.3f} GiB/s")
        print(
            f"[bench_gate] RECORD OK: {len(mask)} mask floors, "
            f"{len(parquet)} parquet floors, {len(page)} page floors"
        )
        return

    if not args.floors.exists():
        print(f"[bench_gate] FAIL: floors file missing: {args.floors} (run --record first)")
        raise SystemExit(2)

    floors = json.loads(args.floors.read_text())
    floor_mask: dict[str, float] = floors.get("mask_min_gbps", {})
    floor_parquet: dict[str, float] = floors.get("parquet", {})
    floor_page: dict[str, float] = floors.get("page", {})
    missing_families = [
        name
        for name, fam in (
            ("mask_min_gbps", floor_mask),
            ("parquet", floor_parquet),
            ("page", floor_page),
        )
        if not fam
    ]
    if missing_families:
        print(
            f"[bench_gate] FAIL: {args.floors} lacks non-empty family/families "
            f"{missing_families} — stale floors file, re-run --record"
        )
        raise SystemExit(2)

    breach = False
    floor_gt = {**{"mask/" + k: v for k, v in floor_mask.items()},
                **{"parquet/" + k: v for k, v in floor_parquet.items()},
                **{"page/" + k: v for k, v in floor_page.items()}}
    observed_gt = {**{"mask/" + k: v for k, v in mask.items()},
                   **{"parquet/" + k: v for k, v in parquet.items()},
                   **{"page/" + k: v for k, v in page.items()}}

    print(f"\n[bench_gate] floor vs observed (drop threshold {args.threshold:g}%):")
    header = f"  {'metric':44s} {'floor':>9s} {'observed':>9s} {'delta':>8s}  status"
    print(header)
    print("  " + "-" * (len(header) - 2))
    for k in sorted(floor_gt):
        floor = floor_gt[k]
        if k not in observed_gt:
            print(f"  {k:44s} {floor:9.3f} {'MISSING':>9s} {'—':>8s}  FAIL (expected line absent)")
            breach = True
            continue
        obs = observed_gt[k]
        delta_pct = (obs - floor) / floor * 100.0
        floor_limit = floor * (1.0 - args.threshold / 100.0)
        failed = obs < floor_limit
        status = "FAIL" if failed else "PASS"
        mark = " <-- breached" if failed else ""
        print(
            f"  {k:44s} {floor:9.3f} {obs:9.3f} {delta_pct:+7.1f}%  {status}{mark}"
        )
        breach = breach or failed
    for k in sorted(set(observed_gt) - set(floor_gt)):
        print(f"  {k:44s} {'—':>9s} {observed_gt[k]:9.3f} {'—':>8s}  WARN (no floor; re-record)")

    if breach:
        print("[bench_gate] GATE FAIL: floor(s) breached beyond the noise margin")
        raise SystemExit(1)
    print("[bench_gate] GATE PASS")


if __name__ == "__main__":
    main()
