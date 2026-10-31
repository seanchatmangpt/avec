# Receipts

Machine-minted receipts for the v26.10.31 benchmark-harness milestone
(spec: [29-benchmark-harness-plan.md](../29-benchmark-harness-plan.md) §CI
Wiring / receipt pipeline).

## Rules

1. **A receipt is minted by the script only.**
   `scripts/mint_receipt.sh <name> <command>` is the only lawful minter.
   Hand-written files in this directory are not receipts. Do not edit a
   receipt after minting.
2. **ALIVE requires a real command and a real exit.** `ALIVE` standing is set
   by the script only when the minted command exited 0 AND produced non-empty
   stdout evidence. A failing command still mints a receipt, marked `FAILED`,
   and the script exits nonzero (fail-closed, never silent).
3. **Targets are never receipts.** Thresholds in
   [04-benchmark-contract.md](../04-benchmark-contract.md) are targets, not
   standing. Conversation-cited numbers (e.g. historical 74-77 GiB/s) are not
   receipts. Only a file in this directory, minted by the script, carries
   standing.
4. **Receipts are append-only.** The script refuses to overwrite an existing
   receipt (exit 73). Corrections are new receipts with a new name, never
   edits.
5. Each receipt carries: command, exit code, stdout tail (last 40 lines),
   UTC timestamp, git HEAD SHA (read-only `git rev-parse`), dirty-tree flag,
   machine identity (`sw_vers` + `hw.model`), and a blake3 (fallback sha256)
   digest of the captured evidence.

## Self-test examples

- `demo-cargo-version.md` — `cargo --version`, exit 0, standing `ALIVE`.
- `demo-fail.md` — `false`-class failing command, standing `FAILED`, script
  exited nonzero.

Both are kept as canonical examples of the two mint outcomes.

## divan2receipt converter (CTQ gate)

`scripts/divan2receipt.py <divan-stdout.txt> [-o OUT.ttl]` converts Divan
stdout into a `stardf:PerformanceReceiptContract` Turtle instance and enforces
the CTQ turnstile (spec: [29-benchmark-harness-plan.md](../29-benchmark-harness-plan.md)
§4; vocabulary: [04-benchmark-contract.md](../04-benchmark-contract.md)).

1. Run the benches and capture stdout (Divan 0.1.21 has **no JSON output
   mode** — witnessed via `--help`; the stdout table is the parsed surface):
   `cargo bench -p bench --bench v26_10_31 -- --color never > /tmp/divan.txt`
2. Run the gate: `python3 scripts/divan2receipt.py /tmp/divan.txt -o receipt.ttl`
   - exit 0 + Turtle emitted only when CTQ-06 (BLAKE3, LSL 8.5 GiB/s) passes;
   - exit 1 + **no receipt emitted** on any CTQ-06 breach or on missing/
     unparseable evidence (fail-closed, never a silent pass);
   - `quad_transpose_64k` (CTQ-04-adjacent), `predicate_filter_64k` and
     `parquet_level_free_roundtrip_u64_64k` are recorded informationally and
     never block.
3. Mint the standing receipt through the script only:
   `bash scripts/mint_receipt.sh gate-divan-ctq python3 scripts/divan2receipt.py /tmp/divan.txt`
   A failing gate mints a `FAILED` receipt and the minter exits nonzero.
4. Non-vacuity (court must flip): a mutated table with throughput >= LSL must
   exit 0 and emit Turtle; a mutated sub-LSL table must exit 1 with no file.
   Both directions were witnessed 2026-10-30 (9.3750 GiB/s -> PASS + Turtle;
   7.8125 GiB/s -> FAIL, no file).
