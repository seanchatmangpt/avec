# Receipt: gate-e2e-ctq05

| Field | Value |
|---|---|
| Receipt ID | gate-e2e-ctq05 |
| Date | 2026-10-31T02:29:54Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-integration --release --test admission_e2e -- --nocapture` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (60 entries) |
| Evidence digest (blake3 of stdout tail) | `78965cd8ad25108b5fd9315f3c7cb74444b79bbf796d76b94a30d9cbbd579177` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
    Finished `release` profile [optimized] target(s) in 0.01s
     Running tests/admission_e2e.rs (target/release/deps/admission_e2e-b3ff473fa36fbba7)

running 2 tests
[stage: layout]   avec-arrow Quad64 layout — LANDED, using it
[stage: blake3]   avec-blake3 batch receipt — LANDED
[stage: parquet]  avec-parquet level-free pages — LANDED
[SKIP stage: git] avec-git packfile wrap — NOT LANDED: crates/avec-git is absent from the parent workspace members list (its manifest therefore refuses to build standalone: "current package believes it's in a workspace when it's not") and its pack_write.rs does not yet define the re-exported PackBuilder/write_pack_v2/write_idx_v2/IdxEntry. Remaining stages still asserted.
test admission_e2e_roundtrip_and_receipt ... ok
[timing] admission loop (blake3 batch receipt + 8 level-free page writes + reads, 256 quads): min-of-2000 = 14.9 µs (CTQ-05 target ≤35 µs — informational, no gate)
[timing] stage breakdown at the min iteration: receipt-blake3 12.79 µs (85.8% of total) | project-refill 1.17 µs | 8x write_page 0.54 µs | 8x read_page_payload 0.38 µs | residual (allocs/timing) 0.04 µs
test admission_e2e_timing_probe_min_of_100 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

<!-- Tree state: dirty (60 entries) -->
