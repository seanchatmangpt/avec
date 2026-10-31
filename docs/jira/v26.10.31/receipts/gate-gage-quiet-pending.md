# Receipt: gate-gage-quiet-pending

| Field | Value |
|---|---|
| Receipt ID | gate-gage-quiet-pending |
| Date | 2026-10-31T14:51:05Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `sh -c echo "PENDING-QUIET: gage precondition could not be witnessed quiet (external VM load, cv_trimmed 8.7-11.2%); the ratified ladder (3x ctq PASS + CTQ-06 C_pl 4.91) stands on its own exits; gage quiet-run receipt to be minted when the host is quiet"` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (120 entries) |
| Evidence digest (blake3 of stdout tail) | `c0a77c1b896cc01c8083f4149932fb542e7e13b4557f7599d7b39c9a6cd0d225` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
PENDING-QUIET: gage precondition could not be witnessed quiet (external VM load, cv_trimmed 8.7-11.2%); the ratified ladder (3x ctq PASS + CTQ-06 C_pl 4.91) stands on its own exits; gage quiet-run receipt to be minted when the host is quiet
```

<!-- Tree state: dirty (120 entries) -->
