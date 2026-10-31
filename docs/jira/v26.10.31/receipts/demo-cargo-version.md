# Receipt: demo-cargo-version

| Field | Value |
|---|---|
| Receipt ID | demo-cargo-version |
| Date | 2026-10-31T00:34:51Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo --version` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (27 entries) |
| Evidence digest (blake3 of stdout tail) | `cc8c2c56baa1f783ad437cfd4e314ce89eab4fc914c64a969aee6a4efea6fc5b` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
cargo 1.97.0 (c980f4866 2026-06-30)
```

<!-- Tree state: dirty (27 entries) -->
