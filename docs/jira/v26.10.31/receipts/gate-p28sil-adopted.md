# Receipt: gate-p28sil-adopted

| Field | Value |
|---|---|
| Receipt ID | gate-p28sil-adopted |
| Date | 2026-10-31T18:31:36Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `sh -c test -f LICENSE-P28-SIL.md && grep -q "urn:avec:license:p28-sil:v2.1.0-sovereign" LICENSE-P28-SIL.md && grep -q MIT LICENSE.md && echo "P28-SIL v2.1.0 instrument of record present; MIT code base declared"` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | f48779df69dbadbab5c72ee13f2095cac4028e93 |
| Tree state | dirty (1 entries) |
| Evidence digest (blake3 of stdout tail) | `957a2bedf867358caf250d2e62f7e9296e212c59a6911fcc54265b07c8b2d9df` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
P28-SIL v2.1.0 instrument of record present; MIT code base declared
```

<!-- Tree state: dirty (1 entries) -->
