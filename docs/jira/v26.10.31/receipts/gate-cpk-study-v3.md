# Receipt: gate-cpk-study-v3

| Field | Value |
|---|---|
| Receipt ID | gate-cpk-study-v3 |
| Date | 2026-10-31T05:32:07Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `./target/release/cpk_study` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (117 entries) |
| Evidence digest (blake3 of stdout tail) | `b33bd3c7bd0aca9add20b5c345ef1fce005e13820a75a5a6d67595d3b480998b` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
[cpk_study] CTQ-05: admission latency samples, USL 35 us, one-sided C_pu
[cpk_study] CTQ-05: n=200 mean=8.488 us sigma_overall=0.902 us sigma_within=0.110 us Cpu(overall)=9.797 Cpu(within)=80.666 vs target 1.67 -> PASS
[cpk_study] CTQ-06: BLAKE3 throughput samples (hash_many_simd, 256 MiB/sample), LSL 8.5 GiB/s, one-sided C_pl
[cpk_study] CTQ-06: sample 0: 1.85 GiB/s
[cpk_study] CTQ-06: sample 20: 1.85 GiB/s
[cpk_study] CTQ-06: sample 40: 1.85 GiB/s
[cpk_study] CTQ-06: sample 60: 1.85 GiB/s
[cpk_study] CTQ-06: sample 80: 1.86 GiB/s
[cpk_study] CTQ-06: n=100 mean=1.84 GiB/s sigma_overall=0.04 sigma_within=0.01 Cpl(overall)=-58.492 Cpl(within)=-262.576 vs target 1.67 -> FAIL
[cpk_study] CTQ-04: transpose+filter variance samples, USL = 15% of mean, one-sided C_pu
[cpk_study] CTQ-04: n=100 mean=15.983 us USL(1.15*mean)=18.381 us sigma_overall=0.193 us sigma_within=0.169 us CV=1.21% Cpu(overall)=4.131 vs target 1.67 -> PASS
{
  "report": "cpk_study",
  "version": 1,
  "cpk_target": 1.67,
  "ctq05_admission_latency": {"ctq":"CTQ-05","metric":"us_per_batch","unit":"us","n":200,"mean":0.000008,"usl":35.0000,"lsl":null,"sigma_overall":0.000001,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":9.7971,"cpu_within":80.6660,"cpl_overall":null,"cpl_within":null,"cpk_overall":9.7971,"cpk_within":80.6660,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "ctq06_throughput": {"ctq":"CTQ-06","metric":"gib_per_s","unit":"GiB/s","n":100,"mean":1.843960,"usl":null,"lsl":8.5000,"sigma_overall":0.037932,"sigma_within_mr":0.008450,"cp":null,"cpu_overall":null,"cpu_within":null,"cpl_overall":-58.4916,"cpl_within":-262.5760,"cpk_overall":-58.4916,"cpk_within":-262.5760,"cpk_target":1.67,"gate":"REAL","verdict":"FAIL"},
  "ctq04_variance": {"ctq":"CTQ-04","metric":"us_per_run","unit":"us","n":100,"mean":0.000016,"usl":18.3808,"lsl":null,"sigma_overall":0.000000,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":4.1308,"cpu_within":4.7377,"cpl_overall":null,"cpl_within":null,"cpk_overall":4.1308,"cpk_within":4.7377,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "verdict": "FAIL", "failed_gates": ["CTQ-06"]
}
```

<!-- Tree state: dirty (117 entries) -->
