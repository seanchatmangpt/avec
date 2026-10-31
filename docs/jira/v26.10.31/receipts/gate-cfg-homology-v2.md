# Receipt: gate-cfg-homology-v2

| Field | Value |
|---|---|
| Receipt ID | gate-cfg-homology-v2 |
| Date | 2026-10-31T03:41:00Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `bash scripts/cfg_homology.sh --mutant` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (67 entries) |
| Evidence digest (blake3 of stdout tail) | `99ff12264f4df5dff0adf6da1fcbcca99757be5293d1cce6c756b1d1fc5bbb46` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== MUTANT COURT: homology delta over cfg_homology_cases fixtures (-O0, slack=4)
KERNEL                       SOURCE-CONSTRUCTS    EMITTED   DELTA   SLACK  VERDICT
mutant/clean                              2          6       4       4  PASS
mutant/branchy                            4         17      13       4  FAIL
== MUTANT COURT FLIPPED: clean PASS && branchy FAIL — delta verdict is witnessed, not vacuous
```

<!-- Tree state: dirty (67 entries) -->
