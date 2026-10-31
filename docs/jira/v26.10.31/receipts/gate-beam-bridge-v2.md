# Receipt: gate-beam-bridge-v2

| Field | Value |
|---|---|
| Receipt ID | gate-beam-bridge-v2 |
| Date | 2026-10-31T02:28:31Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo test -p avec-beam` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (59 entries) |
| Evidence digest (blake3 of stdout tail) | `1d7ff6cd94e48fd0ebae7f097aa88dd7bb296ee7361cc28e59904f9e156df63e` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.01s
     Running unittests src/lib.rs (target/debug/deps/avec_beam-1965134c90312544)

running 9 tests
test shm::tests::payload_too_large_and_too_small_rejected ... ok
test shm::tests::dup_handle_rejects_corrupt_header ... ok
test spsc_ring::tests::ring_single_thread_roundtrip_and_capacity ... ok
test shm::tests::name_lifecycle_excl_and_unlink ... ok
test shm::tests::full_ring_reports_wouldblock_then_frees ... ok
test spsc_ring::tests::ring_concurrent_producer_consumer ... ok
test shm::tests::create_dup_roundtrip_single_thread ... ok
test shm::tests::cross_thread_throughput_and_integrity ... ok
test shm::tests::cross_thread_slot_latency_pingpong ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.22s

     Running tests/bridge_contract.rs (target/debug/deps/bridge_contract-4ad868385bf4e135)

running 1 test
test beam_bridge_contract_end_to_end ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.88s
```

<!-- Tree state: dirty (59 entries) -->
