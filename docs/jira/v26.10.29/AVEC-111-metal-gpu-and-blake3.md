# AVEC-111: Freestanding Apple Silicon Metal Unified Memory Bridge & Sovereign BLAKE3 Core

**Status:** CLOSED
**Component:** `crates/avec-gpu`, `crates/avec-arrow`
**Type:** Feature / Moonshot
**Date:** 2026-10-30

---

## 1. Context & Invariants

To eliminate memory-bus copy penalties and support cryptographic integrity verification without external crate dependencies, two sovereign capabilities were introduced:

1. **Freestanding Apple Silicon Metal Unified Memory Bridge (`crates/avec-gpu`)**:
   - Maps 64-byte aligned `Buf` and 16 KiB page-aligned `MmuBuf` memory directly into Apple Silicon Metal Unified Memory (`MTLResourceStorageModeShared`).
   - Uses zero-copy `newBufferWithBytesNoCopy:length:options:deallocator:`.
   - Zero dynamic runtime SDK or heavy driver dependencies; standalone C/Obj-C shim linking `-framework Metal` and `-framework Foundation`.
   - 0 heap allocations during buffer bridge encapsulation.

2. **Sovereign Freestanding BLAKE3 Core (`crates/avec-arrow/src/blake3.rs`)**:
   - Standalone `#![no_std]` implementation with zero external crates (adhering to M-01 / CTQ-4).
   - ChaCha-based 7-round unrolled compression function with zero allocations.
   - Bit-level verification against official BLAKE3 test vectors (`af1349b9...` empty string hash).

---

## 2. Verification Receipts

### 2.1 Metal Unified Memory Bridge (`avec-gpu`)
```
$ cargo test -p avec-gpu
   Compiling avec-gpu v26.10.29 (/Users/sac/avec/crates/avec-gpu)
     Running unittests src/lib.rs (target/debug/deps/avec_gpu-bc596e83708dc8d5)

running 1 test
test tests::test_metal_shared_buffer_zero_copy ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

### 2.2 Sovereign BLAKE3 Core (`avec-arrow`)
```
$ cargo test -p avec-arrow --lib blake3
     Running unittests src/lib.rs (target/debug/deps/avec_arrow-76a7889e2db7accc)

running 2 tests
test blake3::tests::test_blake3_deterministic_and_sensitivity ... ok
test blake3::tests::test_blake3_empty_string_hash ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 2.3 Gate Verification Ladder
- `cargo test -p avec-arrow && cargo test -p avec-gpu && cargo test -p avec-ffi && cargo test -p avec-parquet --lib`: **PASS** (149 parquet tests, 70 arrow tests, 1 gpu test).
- `PATH="/opt/homebrew/opt/llvm/bin:$PATH" make branch-audit`: **PASS** (71 itemized latch/dispatch conditionals, 0 unaccounted).
- `uv run --with pyarrow python3 scripts/gate3_cdata.py`: **PASS** (GATE 3 PASS: bitwise parity through C Data Interface).
- `cargo run --release -p bench --bin phase_change_bench`: **PASS** (517.28 GiB/s logical uncompressed rate).
