# Receipt: demo-fail

| Field | Value |
|---|---|
| Receipt ID | demo-fail |
| Date | 2026-10-31T00:34:51Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `false` |
| Exit | 1 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (27 entries) |
| Evidence digest (blake3 of stdout tail) | `af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262` |
| Standing | FAILED (exit 1) |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
(empty stdout — no evidence captured)
```

<!-- Tree state: dirty (27 entries) -->
