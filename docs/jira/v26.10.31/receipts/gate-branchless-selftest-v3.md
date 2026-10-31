# Receipt: gate-branchless-selftest-v3

| Field | Value |
|---|---|
| Receipt ID | gate-branchless-selftest-v3 |
| Date | 2026-10-31T04:58:01Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `bash scripts/audit_branchless.sh --selftest` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (70 entries) |
| Evidence digest (blake3 of stdout tail) | `3df3733242564ab8883cc9acb689aeda799485ecc94bfa9e88e37d5bc52b92e3` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
== disassembler: objdump (Apple LLVM version 17.0.0)
== SELFTEST
== positive control: FIRED (4 banned ops in branchy.o) — gate is not vacuous
== scope demo: clean branchless fixture -> 0 hits
== SELFTEST PASS
```

<!-- Tree state: dirty (70 entries) -->
