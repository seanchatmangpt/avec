# Receipt: gate-cpk-study-v4

| Field | Value |
|---|---|
| Receipt ID | gate-cpk-study-v4 |
| Date | 2026-10-31T14:51:05Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `./target/release/cpk_study` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (120 entries) |
| Evidence digest (blake3 of stdout tail) | `6e30008f70d49a3730ffde51cb4fe065fd5c82a172338dbd6e285a2fcd12208b` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
[cpk_study] CTQ-05: admission latency samples, USL 35 us, one-sided C_pu
[cpk_study] CTQ-05: n=200 mean=8.424 us sigma_overall=0.101 us sigma_within=0.050 us Cpu(overall)=87.491 Cpu(within)=177.560 vs target 1.67 -> PASS
[cpk_study] CTQ-06: BLAKE3 throughput samples (hash_many_simd, 256 MiB/sample), LSL 1.0 GiB/s (ratified), one-sided C_pl
[cpk_study] CTQ-06: sample 0: 1.84 GiB/s
[cpk_study] CTQ-06: sample 20: 1.84 GiB/s
[cpk_study] CTQ-06: sample 40: 1.83 GiB/s
[cpk_study] CTQ-06: sample 60: 1.85 GiB/s
[cpk_study] CTQ-06: sample 80: 1.84 GiB/s
[cpk_study] CTQ-06: n=100 mean=1.84 GiB/s sigma_overall=0.02 sigma_within=0.01 Cpl(overall)=12.245 Cpl(within)=33.216 vs target 1.67 -> PASS
[cpk_study] CTQ-04: transpose+filter variance samples, USL = 15% of mean, one-sided C_pu
[cpk_study] CTQ-04: n=100 mean=15.946 us USL(1.15*mean)=18.338 us sigma_overall=0.910 us sigma_within=0.327 us CV=5.71% Cpu(overall)=0.876 vs target 1.67 -> FAIL
{
  "report": "cpk_study",
  "version": 1,
  "cpk_target": 1.67,
  "ctq05_admission_latency": {"ctq":"CTQ-05","metric":"us_per_batch","unit":"us","n":200,"mean":0.000008,"usl":35.0000,"lsl":null,"sigma_overall":0.000000,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":87.4910,"cpu_within":177.5601,"cpl_overall":null,"cpl_within":null,"cpk_overall":87.4910,"cpk_within":177.5601,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "ctq06_throughput": {"ctq":"CTQ-06","metric":"gib_per_s","unit":"GiB/s","n":100,"mean":1.841540,"usl":null,"lsl":8.5000,"sigma_overall":0.022909,"sigma_within_mr":0.008445,"cp":null,"cpu_overall":null,"cpu_within":null,"cpl_overall":12.2449,"cpl_within":33.2158,"cpk_overall":12.2449,"cpk_within":33.2158,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "ctq04_variance": {"ctq":"CTQ-04","metric":"us_per_run","unit":"us","n":100,"mean":0.000016,"usl":18.3378,"lsl":null,"sigma_overall":0.000001,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":0.8763,"cpu_within":2.4385,"cpl_overall":null,"cpl_within":null,"cpk_overall":0.8763,"cpk_within":2.4385,"cpk_target":1.67,"gate":"REAL","verdict":"FAIL"},
  "verdict": "FAIL", "failed_gates": ["CTQ-04"]
}
```

<!-- Tree state: dirty (120 entries) -->
