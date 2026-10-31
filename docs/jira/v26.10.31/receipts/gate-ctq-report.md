# Receipt: gate-ctq-report

| Field | Value |
|---|---|
| Receipt ID | gate-ctq-report |
| Date | 2026-10-31T02:22:00Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `target/release/ctq_report` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (59 entries) |
| Evidence digest (blake3 of stdout tail) | `a4ee070afc461cad14d6c4b938226eccbab4236d5ed3641783fe71c30f37a324` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
[ctq_report] CTQ-03: level-free parquet roundtrip (informational, no pinned LSL/USL)
[ctq_report] CTQ-03: page=2097184 B rows=262144 best=0.000 us -> 0.00 GiB/s (informational)
[ctq_report] CTQ-04: transpose/lookup variance over 100 runs (provisional USL: stdev <= 15% of mean, pending DFLSS ratification)
[ctq_report] CTQ-04: n=4096 runs=100 mean=15.968 us stdev=1.437 us stdev/mean=9.0% vs USL 15% -> PASS
[ctq_report] CTQ-05: e2e admission pipeline (USL 35 us per batch, gate REAL)
[ctq_report] CTQ-05: candidate hash correct, hash-only best=12.833 us
[ctq_report] CTQ-05: candidate hash_wide_simd correct, hash-only best=35.500 us
[ctq_report] CTQ-05: candidate hash_many_simd correct, hash-only best=12.791 us
[ctq_report] CTQ-05: batch_bytes=16384 rounds=2000 path=hash_many_simd hash_only=12.791 us best=12.875 us USL=35 us -> PASS
[ctq_report] CTQ-02: zero-alloc 64 MiB BLAKE3 pass (gate REAL: 0 allocation events)
[ctq_report] CTQ-02: bytes=67108864 alloc_events=0 alloc_bytes=0 time=0.053 s -> PASS
[ctq_report] CTQ-06: BLAKE3 throughput, two rows (hash_wide_simd, hash_many_simd), min-of-5 over 256 MiB, LSL 8.5 GiB/s each (gate REAL)
[ctq_report] CTQ-06 [hash_wide_simd]: round 0: 0.584 s, 0.43 GiB/s
[ctq_report] CTQ-06 [hash_wide_simd]: round 1: 0.586 s, 0.43 GiB/s
[ctq_report] CTQ-06 [hash_wide_simd]: round 2: 0.586 s, 0.43 GiB/s
[ctq_report] CTQ-06 [hash_wide_simd]: round 3: 0.584 s, 0.43 GiB/s
[ctq_report] CTQ-06 [hash_wide_simd]: round 4: 0.584 s, 0.43 GiB/s
[ctq_report] CTQ-06 [hash_wide_simd]: best=0.5840 s -> 0.43 GiB/s vs LSL 8.5 -> FAIL
[ctq_report] CTQ-06 [hash_many_simd]: round 0: 0.212 s, 1.18 GiB/s
[ctq_report] CTQ-06 [hash_many_simd]: round 1: 0.212 s, 1.18 GiB/s
[ctq_report] CTQ-06 [hash_many_simd]: round 2: 0.212 s, 1.18 GiB/s
[ctq_report] CTQ-06 [hash_many_simd]: round 3: 0.212 s, 1.18 GiB/s
[ctq_report] CTQ-06 [hash_many_simd]: round 4: 0.212 s, 1.18 GiB/s
[ctq_report] CTQ-06 [hash_many_simd]: best=0.2119 s -> 1.18 GiB/s vs LSL 8.5 -> FAIL
{
  "report": "ctq_report",
  "version": 2,
  "ctq03_parquet_level_free_roundtrip": {"metric":"gib_per_s","value":0.000,"page_bytes":2097184,"rows":262144,"rounds":200,"min_ns":0,"gate":"informational","verdict":"REPORT","note":"unmeasurable at timer resolution"},
  "ctq04_transpose_variance": {"metric":"stdev_frac_of_mean","value":0.0900,"usl_frac":0.15,"stdev_ns":1437,"mean_ns":15968,"runs":100,"n":4096,"gate":"REAL","pin_status":"PROVISIONAL pending DFLSS ratification","verdict":"PASS"},
  "ctq05_e2e_admission": {"metric":"us_per_batch","value":12.875,"usl_us":35.0,"batch_bytes":16384,"rounds":2000,"path":"hash_many_simd","hash_only_us":12.791,"small_batch_eligible":true,"gate":"REAL","verdict":"PASS"},
  "ctq02_zero_alloc": {"metric":"alloc_events","value":0,"alloc_bytes":0,"bytes":67108864,"time_s":0.0530,"gate":"REAL","verdict":"PASS"},
  "ctq06_blake3": {"lsl_gib_s":8.5,"rows":[{"path":"hash_wide_simd","digest_correct":true,"metric":"gib_per_s","value":0.43,"lsl_gib_s":8.5,"bytes":268435456,"rounds":5,"min_ns":584025417,"gate":"REAL","verdict":"FAIL"},{"path":"hash_many_simd","digest_correct":true,"metric":"gib_per_s","value":1.18,"lsl_gib_s":8.5,"bytes":268435456,"rounds":5,"min_ns":211943458,"gate":"REAL","verdict":"FAIL"}]},
  "verdict": "FAIL", "failed_gates": ["CTQ-06"]
}
```

<!-- Tree state: dirty (59 entries) -->
