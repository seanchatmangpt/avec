# Receipt: {{RECEIPT_ID}}

| Field | Value |
|---|---|
| Receipt ID | {{RECEIPT_ID}} |
| Date | {{DATE}} |
| Subject | {{SUBJECT}} |
| Command | {{COMMAND}} |
| Exit | {{EXIT}} |
| Machine | {{MACHINE}} |
| Input SHA (git HEAD) | {{INPUT_SHA}} |
| Tree state | {{DIRTY}} |
| Evidence digest ({{HASH_ALG}} of stdout tail) | `{{DIGEST}}` |
| Standing | {{STANDING}} |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.
