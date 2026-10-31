# Receipt: gate-ctq06-ratified-pass

| Field | Value |
|---|---|
| Receipt ID | gate-ctq06-ratified-pass |
| Date | 2026-10-31T14:50:39Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `sh -c for i in 1 2 3 4; do ./target/release/ctq_report >/tmp/ctq_r$i.json 2>/dev/null; echo run$i=$?; done` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (120 entries) |
| Evidence digest (blake3 of stdout tail) | `25a05ebc31ee0c74894f838641e1017606a27957ff5af784b207a3d0809a42f2` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
run1=0
run2=0
run3=0
run4=0
```

<!-- Tree state: dirty (120 entries) -->
