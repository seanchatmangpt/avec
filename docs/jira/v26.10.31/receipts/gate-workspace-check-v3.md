# Receipt: gate-workspace-check-v3

| Field | Value |
|---|---|
| Receipt ID | gate-workspace-check-v3 |
| Date | 2026-10-31T04:11:27Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo check --workspace` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `7dc6087080b4fdb56dd7bd42f045b9e0fc6afd1f122ccecba6cfed8651da1e1f` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.02s
```

<!-- Tree state: dirty (70 entries) -->
