# Receipt: gate-divan-ctq

| Field | Value |
|---|---|
| Receipt ID | gate-divan-ctq |
| Date | 2026-10-31T02:21:45Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `python3 scripts/divan2receipt.py /tmp/divan_clean.txt` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (59 entries) |
| Evidence digest (blake3 of stdout tail) | `f42b6291953c0e9f408edbb6b7b6188e7decf89bed8b5cbefd7b85a099bb4ddc` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
TURNSTILE: CTQ-06 breach — receipt NOT emitted, exit 1
CTQ-06 blake3_hash_64mib: 0.4507 GiB/s vs LSL 8.50 GiB/s -> FAIL
```

<!-- Tree state: dirty (59 entries) -->
