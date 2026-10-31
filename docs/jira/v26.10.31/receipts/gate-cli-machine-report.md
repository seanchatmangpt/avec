# Receipt: gate-cli-machine-report

| Field | Value |
|---|---|
| Receipt ID | gate-cli-machine-report |
| Date | 2026-10-31T01:57:23Z |
| Subject | minted by scripts/mint_receipt.sh |
| Command | `cargo run -q -p avec-cli --bin verify_v26` |
| Exit | 0 |
| Machine | ProductName:		macOS ProductVersion:		26.2 BuildVersion:		25C56 Mac15,9 |
| Input SHA (git HEAD) | ead40329a699878a91b070378066b5e50541ba33 |
| Tree state | dirty (49 entries) |
| Evidence digest (blake3 of stdout tail) | `7ae8e528a1c979c7ceb5c1ac9f95b130a055acebc3cda8db9a38bf31cd6d0160` |
| Standing | ALIVE |

## Standing rule

`ALIVE` requires exit 0 **and** non-empty captured evidence. Anything else is
`FAILED` (with exit code) — a receipt is never silently minted from a failing
or evidence-free run.

<!-- auto-generated block; part of the receipt -->

## Evidence (stdout tail, last 40 lines)

```text
warning: unused import: `alloc::vec`
  --> crates/avec-arrow/src/transpose.rs:18:5
   |
18 | use alloc::vec;
   |     ^^^^^^^^^^
   |
   = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unnecessary `unsafe` block
   --> crates/avec-blake3/src/multi_chunk.rs:126:5
    |
 96 |     unsafe {
    |     ------ because it's nested under this `unsafe` block
...
126 |     unsafe {
    |     ^^^^^^ unnecessary `unsafe` block
    |
    = note: `#[warn(unused_unsafe)]` (part of `#[warn(unused)]`) on by default

warning: unnecessary `unsafe` block
  --> crates/avec-blake3/src/multi_chunk.rs:86:9
   |
86 |         unsafe {
   |         ^^^^^^ unnecessary `unsafe` block

warning: method `at_end` is never used
  --> crates/avec-git/src/inflate.rs:87:8
   |
77 | impl<'a> BitReader<'a> {
   | ---------------------- method in this implementation
...
87 |     fn at_end(&self) -> bool {
   |        ^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

{"tool":"verify_v26","rows":[{"id":"FR-01","gate":"size/align (Quad64 64B/64B; Buf 64B-aligned heap)","status":"PASS","evidence":"size_of(Quad64)=64 align_of(Quad64)=64 Buf 143/143 allocs 64B-aligned (sizes 1..=1000 step 7)"},{"id":"FR-02","gate":"layout law (derived from FR-01 + Quad64 wire round-trip ladder spot check)","status":"PASS","evidence":"Quad64 new->to_le_bytes(64B)->from_le_bytes round-trip equal; offsets 0x00/0x04/0x08/0x0C/0x10/0x18/0x1C/0x20 held by build-time asserts in layout.rs"},{"id":"FR-03","gate":"parquet RLE round-trip (rle_encode -> rle_decode)","status":"PASS","evidence":"values=10000 bits=8 encoded=10003B enc_fast=10003B decode_ok=true values_equal=true elapsed_us=752"},{"id":"FR-04","gate":"git pack write + in-crate parser read","status":"PASS","evidence":"blobs=8 pack_bytes=147 idx_bytes=1296 parsed_objs=8 idx_oids=8 type=blob(3) sizes_match=true elapsed_us=334"},{"id":"FR-06","gate":"blake3 known-answer spot check","status":"PASS","evidence":"hash(\"\")=af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262 expected=af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262 match=true hash(\"abc\")=6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85 expected=6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85 match=true"},{"id":"FR-05","gate":"replay/determinism spot check (Quad64 wire re-encode stability)","status":"PASS","evidence":"to_le_bytes->from_le_bytes->to_le_bytes byte-identical (deterministic re-encode)"},{"id":"FR-07","gate":"BEAM interop","status":"SKIP","evidence":"requires a live BEAM (avec-beam NIF harness); owned by the integration lane, not runnable in this in-process battery"}],"summary":{"total":7,"fail":0,"skips":["FR-07"]},"exit_ok":true}
```

<!-- Tree state: dirty (49 entries) -->
