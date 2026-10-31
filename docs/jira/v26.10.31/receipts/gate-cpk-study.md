# Receipt: gate-cpk-study

| Field | Value |
|---|---|
| Receipt ID | gate-cpk-study |
| Date | 2026-10-31T03:03:46Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `./target/release/cpk_study` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (62 entries) |
| Evidence digest (blake3 of stdout tail) | `f9f438b7ec1b0a1ab8a73fa335c682b4182754dabdbabc851ca894451f8a4038` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
[cpk_study] CTQ-05: admission latency samples, USL 35 us, one-sided C_pu
[cpk_study] CTQ-05: n=200 mean=8.475 us sigma_overall=0.918 us sigma_within=0.093 us Cpu(overall)=9.634 Cpu(within)=95.537 vs target 1.67 -> PASS
[cpk_study] CTQ-06: BLAKE3 throughput samples (hash_many_simd, 256 MiB/sample), LSL 8.5 GiB/s, one-sided C_pl
[cpk_study] CTQ-06: sample 0: 1.86 GiB/s
[cpk_study] CTQ-06: sample 20: 1.84 GiB/s
[cpk_study] CTQ-06: sample 40: 1.85 GiB/s
[cpk_study] CTQ-06: sample 60: 1.85 GiB/s
[cpk_study] CTQ-06: sample 80: 1.85 GiB/s
[cpk_study] CTQ-06: n=100 mean=1.85 GiB/s sigma_overall=0.01 sigma_within=0.00 Cpl(overall)=-254.797 Cpl(within)=-526.056 vs target 1.67 -> FAIL
[cpk_study] CTQ-04: transpose+filter variance samples, USL = 15% of mean, one-sided C_pu
[cpk_study] CTQ-04: n=100 mean=15.915 us USL(1.15*mean)=18.302 us sigma_overall=0.184 us sigma_within=0.142 us CV=1.16% Cpu(overall)=4.327 vs target 1.67 -> PASS
{
  "report": "cpk_study",
  "version": 1,
  "cpk_target": 1.67,
  "ctq05_admission_latency": {"ctq":"CTQ-05","metric":"us_per_batch","unit":"us","n":200,"mean":0.000008,"usl":35.0000,"lsl":null,"sigma_overall":0.000001,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":9.6345,"cpu_within":95.5370,"cpl_overall":null,"cpl_within":null,"cpk_overall":9.6345,"cpk_within":95.5370,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "ctq06_throughput": {"ctq":"CTQ-06","metric":"gib_per_s","unit":"GiB/s","n":100,"mean":1.847284,"usl":null,"lsl":8.5000,"sigma_overall":0.008703,"sigma_within_mr":0.004215,"cp":null,"cpu_overall":null,"cpu_within":null,"cpl_overall":-254.7972,"cpl_within":-526.0563,"cpk_overall":-254.7972,"cpk_within":-526.0563,"cpk_target":1.67,"gate":"REAL","verdict":"FAIL"},
  "ctq04_variance": {"ctq":"CTQ-04","metric":"us_per_run","unit":"us","n":100,"mean":0.000016,"usl":18.3018,"lsl":null,"sigma_overall":0.000000,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":4.3267,"cpu_within":5.5996,"cpl_overall":null,"cpl_within":null,"cpk_overall":4.3267,"cpk_within":5.5996,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "verdict": "FAIL", "failed_gates": ["CTQ-06"]
}
```

<!-- Tree state: dirty (62 entries) -->
