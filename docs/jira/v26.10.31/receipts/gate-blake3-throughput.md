# Receipt: gate-blake3-throughput

| Field | Value |
|---|---|
| Receipt ID | gate-blake3-throughput |
| Date | 2026-10-31T01:57:15Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `target/release/blake3_throughput --gate` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (49 entries) |
| Evidence digest (blake3 of stdout tail) | `ce968a74464d71c7cc846ffa0cf83d68dde98bc0bbd701bb61312e264012ed06` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
round 0: 0.597 s, 0.42 GiB/s
round 1: 0.585 s, 0.43 GiB/s
round 2: 0.587 s, 0.43 GiB/s
round 3: 0.585 s, 0.43 GiB/s
round 4: 0.585 s, 0.43 GiB/s
RESULT machine=unknown bytes=268435456 rounds=5 min_time=0.5848s throughput=0.43 GiB/s gate=true
GATE FAIL: throughput 0.43 GiB/s < LSL 8.50 GiB/s
```

<!-- Tree state: dirty (49 entries) -->
