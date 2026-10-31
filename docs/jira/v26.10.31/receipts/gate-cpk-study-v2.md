# Receipt: gate-cpk-study-v2

| Field | Value |
|---|---|
| Receipt ID | gate-cpk-study-v2 |
| Date | 2026-10-31T03:39:11Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `./target/release/cpk_study` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (66 entries) |
| Evidence digest (blake3 of stdout tail) | `87f4542fb3c64b740670fa11a2e6144dc1f841f6a9801ed8578bee17a92bad64` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
[cpk_study] CTQ-05: admission latency samples, USL 35 us, one-sided C_pu
[cpk_study] CTQ-05: n=200 mean=8.482 us sigma_overall=0.408 us sigma_within=0.115 us Cpu(overall)=21.657 Cpu(within)=76.990 vs target 1.67 -> PASS
[cpk_study] CTQ-06: BLAKE3 throughput samples (hash_many_simd, 256 MiB/sample), LSL 8.5 GiB/s, one-sided C_pl
[cpk_study] CTQ-06: sample 0: 1.85 GiB/s
[cpk_study] CTQ-06: sample 20: 1.86 GiB/s
[cpk_study] CTQ-06: sample 40: 1.85 GiB/s
[cpk_study] CTQ-06: sample 60: 1.85 GiB/s
[cpk_study] CTQ-06: sample 80: 1.84 GiB/s
[cpk_study] CTQ-06: n=100 mean=1.85 GiB/s sigma_overall=0.01 sigma_within=0.01 Cpl(overall)=-209.807 Cpl(within)=-397.419 vs target 1.67 -> FAIL
[cpk_study] CTQ-04: transpose+filter variance samples, USL = 15% of mean, one-sided C_pu
[cpk_study] CTQ-04: n=100 mean=16.015 us USL(1.15*mean)=18.418 us sigma_overall=0.430 us sigma_within=0.258 us CV=2.68% Cpu(overall)=1.863 vs target 1.67 -> PASS
{
  "report": "cpk_study",
  "version": 1,
  "cpk_target": 1.67,
  "ctq05_admission_latency": {"ctq":"CTQ-05","metric":"us_per_batch","unit":"us","n":200,"mean":0.000008,"usl":35.0000,"lsl":null,"sigma_overall":0.000000,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":21.6566,"cpu_within":76.9901,"cpl_overall":null,"cpl_within":null,"cpk_overall":21.6566,"cpk_within":76.9901,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "ctq06_throughput": {"ctq":"CTQ-06","metric":"gib_per_s","unit":"GiB/s","n":100,"mean":1.846241,"usl":null,"lsl":8.5000,"sigma_overall":0.010571,"sigma_within_mr":0.005581,"cp":null,"cpu_overall":null,"cpu_within":null,"cpl_overall":-209.8070,"cpl_within":-397.4191,"cpk_overall":-209.8070,"cpk_within":-397.4191,"cpk_target":1.67,"gate":"REAL","verdict":"FAIL"},
  "ctq04_variance": {"ctq":"CTQ-04","metric":"us_per_run","unit":"us","n":100,"mean":0.000016,"usl":18.4177,"lsl":null,"sigma_overall":0.000000,"sigma_within_mr":0.000000,"cp":null,"cpu_overall":1.8631,"cpu_within":3.1011,"cpl_overall":null,"cpl_within":null,"cpk_overall":1.8631,"cpk_within":3.1011,"cpk_target":1.67,"gate":"REAL","verdict":"PASS"},
  "verdict": "FAIL", "failed_gates": ["CTQ-06"]
}
```

<!-- Tree state: dirty (66 entries) -->
