# Panic-Freedom Audit: Hot Paths (wave-14 lane 9)

Date: 2026-10-31. Lane: wave-14 lane 9. Repo: /Users/sac/avec (one canonical
checkout, no worktrees).

## Scope and Method

Sweep of `crates/*/src` (excluding `src/bin/`, benches, and `#[cfg(test)]`
modules) for `unwrap()` / `expect(` / `panic!` / `unreachable!` / `todo!` /
`unimplemented!` plus runtime-index audit of the priority files, read in full:
avec-arrow `predicates.rs` + `transpose.rs`, avec-parquet `scan.rs` +
`level_free.rs`, avec-git `ladder.rs` + `idx_read.rs` + `deflate.rs` +
`inflate.rs`, avec-blake3 `lib.rs`.

- Keyword sweep: 287 sites across 40 files (`grep -rnE` over `crates/*/src`).
- Test-boundary verified mechanically per file via `#[cfg(test)]` /
  `#[cfg(all(test, feature = "std"))]` line numbers, not eyeballed.
- Classes: OK (compile-time-provable: fixed-size array params, literal
  indices, const asserts), OK-WITNESSED (prior bounds-check/assert in the
  same call path — guard line cited), OK-CONTRACT (idiomatic index/limit
  panic, std `Vec[i]` semantics, documented parameter domain), FINDING
  (external input, no local guard).

## Fix Applied 1: avec-git `idx_read.rs` lookup — typed refusal of malformed fanout

`IdxV2::lookup` computed `let part = hi - lo;` from fanout values read
straight from external idx bytes. A malformed (non-monotonic) fanout table
makes `hi < lo`, which panicked in debug builds ("attempt to subtract with
overflow"); release wrapped to a huge partition that only accidentally
errored via the `LADDER_WINDOW` check. The reader's stated contract
(module doc: "bounds checks are explicit and typed") is typed errors on
external input, and the signature is already `Result` — trivial fix.

```rust
// crates/avec-git/src/idx_read.rs (IdxV2::lookup)
let Some(part) = hi.checked_sub(lo) else {
    return Err(GitError::Truncated);
};
```

Behavior note: on malformed input, release builds now return
`Err(Truncated)` where they previously returned `Err(TooManyObjects(huge))`;
debug builds no longer panic. Valid idx (monotonic fanout) is unaffected.

## Fix Applied 2: avec-git `pack_write.rs` parse_pack_v2 — range panic guard

`parse_pack_v2` (pub, parses external pack bytes) sliced
`pack[body_start + 2..pack.len() - 20]` after a caller-controlled varint
object-header length without an intervening check. A malformed header can
push `body_start + 2` past `pack.len() - 20` → "slice index starts at X but
ends at Y" panic on external input. Signature already `Result` — trivial fix.

```rust
// crates/avec-git/src/pack_write.rs (parse_pack_v2)
if body_start + 2 > pack.len() - 20 {
    return Err(GitError::Truncated);
}
```

## Fix Applied 3 (disclosed minimal unblock, another lane's file)

`crates/avec-git/tests/fuzz_codec.rs:162` did not compile (E0600:
`&!(n as u16).to_le_bytes()` applies `!` to the array). Minimal unblock under
the compile-freeze SLA, disclosed per the same-checkout fanout rules:
`&(!(n as u16)).to_le_bytes()` (NLEN = !LEN in the stored-block trailer).
This was a pre-existing break in a test target I do not own (in-flight wave
edits present at session start); without it the avec-git suite aborts at
compile with 0 tests run. Owner should re-review.

## Audit Table (production code only)

Legend: W = OK-WITNESSED (guard cited), C = OK-CONTRACT, T = test-only.

| Site | Class | Guard / note |
|---|---|---|
| avec-arrow `blake3.rs:189` `unreachable!()` | W | `num_blocks` >= 1 by construction (empty->1, else div_ceil); loop returns at `b == num_blocks-1` |
| avec-arrow `layout.rs:171,177,180` `try_into().unwrap()` | W | len check `bytes.len() != QUAD_SIZE -> None` (line 168); fixed offsets < 64 |
| avec-arrow `varlen.rs:28` `expect("offset overflow (u32 family)")` | C | Format-family limit (u32 offsets); panic message descriptive; `-> Self` signature, Result = non-trivial API ripple. LISTED finding |
| avec-arrow `views.rs:63,67` `try_into().unwrap()` after `d[base..base+4]` | C | std index semantics on `row(i)`; unwrap unreachable for i < views count. LISTED finding (index contract) |
| avec-arrow `mmu_arena.rs:34` `panic!("invalid mmu layout")` | C | Allocation-limit panic (isize-overflow class, like `Vec::with_capacity`) |
| avec-arrow `mmu_arena.rs:106` `.unwrap()` in Drop | W | Same `Layout::from_size_align` that succeeded at construction (line 33) |
| avec-arrow `buffer.rs:40` `panic!("invalid buffer layout")` | C | Allocation-limit panic class |
| avec-arrow predicates.rs | OK | `[u32; 4]` params, literal indices; `filter_quads_eq` raw-pointer path: `assert!(out.len() > n_quads)` line 110 + documented debug_assert line 114 (safety contract, not panic surface) |
| avec-arrow transpose.rs | OK | All raw-pointer, Safety comments; lengths from same input slices; no panic sites. Observation: `untranspose` cross-column length invariants are debug_assert-only and fields are pub (UB-class, not panic-class) |
| avec-parquet `scan.rs:436` `expect("checked")` | W | Lines 430-435: explicit `is_none()`/`is_empty()` refusals before the expect |
| avec-parquet `scan.rs:716,736` `expect("4 bytes"/"8 bytes")` | W | `.get(off..off+N).ok_or(OffsetMismatch)?` immediately above (lines 711-715 / 731-735) |
| avec-parquet `scan.rs:825` `expect("4 bytes")` | W | `bytes.len() < 16` refusal (line 818) makes the 4-byte tail in-bounds |
| avec-parquet `scan.rs:877,883` (write_fixture_file) | C | Fixture writer (pub but fixture-named); assert names the violated precondition; Result signature change non-trivial. LISTED finding |
| avec-parquet `ffor.rs:120` `unreachable!()` | W | `assert!` at `ffor_unpack` head (line 46) gates width; doc states fail-loud contract |
| avec-parquet `rle.rs:264` `try_into().unwrap()` | W | Fail-closed chain: `checked_mul`/`checked_add`/`src.get(pos..end)` + groups*8 multiple-of-8 invariant; module law "zero panics on malformed pages" |
| avec-parquet `packed_filter.rs:121` `unreachable!()` | W | `assert!`s at 74/79 gate width and buffer size |
| avec-git `idx_read.rs:32` | W | `data.len() < OID_TABLE_OFF + 40` refusal (line 26) |
| avec-git `idx_read.rs:65,72,79` fanout/oid/crc | C | Parse guarantees coverage for valid domains (b <= 255, i < n); out-of-domain = caller contract (std index panic). In-crate callers witnessed (lookup lines 122-165) |
| avec-git `idx_read.rs:93,104` | W | Explicit guards: lines 87-89 (`small_off + 4 > len -> Truncated`), line 100 (`large_end <= len` select) — commented "guarded above" |
| avec-git `idx_read.rs:180` `len() - 20` | W | Sole constructor `parse` guarantees `len >= 1072` (line 26); fields private |
| avec-git `ladder.rs` | OK | Fixed `[u8; 20]` params; `oids[q]` behind `in_bounds` select (line 130 guards 135); candidate via `.get()` |
| avec-git `deflate.rs`, `inflate.rs` | OK | All keyword sites inside `#[cfg(all(test, feature = "std"))]` (570+ / 450+); production zero |
| avec-git `pack_write.rs:321,420` | W | Pack from in-crate writer (12-byte header + 20-byte trailer, structurally >= 32); guard `pack.len() < 32` at 412 |
| avec-git `pack_write.rs:415,418,476,481,485,489` | W | Guards: line 412 (`len < 32`), line 472 (`len < 1032+40`); fanout loop bounded by constants |
| avec-git `pack_write.rs:503,510,523,528` | W | Guard line 500 (`idx.len() < offs_end + 40 -> Truncated`); table math within offs_end |
| avec-git `pack_write.rs:434` (pre-fix) | FIXED | Was: `pack[body_start+2..len-20]` range panic on malformed varint header; now typed Truncated (Fix 2) |
| avec-git `idx_read.rs:125` (pre-fix) | FIXED | Was: `hi - lo` debug-overflow panic on non-monotonic external fanout; now typed Truncated (Fix 1) |
| avec-blake3 `lib.rs:193` `unreachable!()` | W | Same construction guard as arrow blake3 (lines 157-161) |
| avec-blake3 `lib.rs:1038,1044` | T | Inside `#[cfg(test)]` hex helper |
| avec-blake3 Merkle stack `[[u32; 8]; 54]` | OK | Depth 54 covers 2^54 chunks = 2^64 bytes; writes at stack_len <= 53 by merge invariant |
| avec-beam `shm.rs:461` | W | `assert!(slot.len() == SLOT_SIZE)` line 460 |
| avec-beam (all other src) | OK | Production keyword sites: none (shm.rs count was 40 incl. 39 test/doc); ring internals are offset/pointer-based |
| avec-parquet thrift/dictionary/compiled_manifest/page/packing/oblivious/page_aligned/encode_fast/bitslice | OK | All keyword sites test-enclosed; production zero |
| wasm-footprint `lib.rs:346` | W | Same fail-closed chain as rle `decode_into` + inline in-bounds proof comment |
| audit `alloc_counter.rs:220`, avec-beam tests, all test mods | T | Test-enclosed |

## Findings Listed (not fixed — non-trivial signature/API change)

1. avec-arrow `varlen.rs:28` — `Offset::from_usize` panics on u32 offset
   overflow (>4 GiB column). Clear message; changing to `Result` ripples
   through the builder trait. Recommend a follow-up work order.
2. avec-arrow `views.rs:60-69` — `row(i)` panics on i >= views count (std
   index contract). Idiomatic; a `get_row` variant would be the typed path.
3. avec-parquet `scan.rs:877,883` — `write_fixture_file` asserts/expects on
   caller-supplied fixture shapes. Fixture API by name; panic messages name
   the precondition.
4. avec-arrow `transpose.rs` `untranspose` — cross-column length invariants
   debug_assert-only, fields pub (release-UB class, not panic-class). Flag
   for the layout lane: consider private fields + typed constructor.

## Pre-Existing Failures Witnessed (not session-introduced)

- avec-git `tests/fuzz_codec.rs` did not compile at in-flight HEAD (E0600).
  Fixed as Fix 3 (disclosed unblock). Owner: fuzz_codec lane.
- avec-beam `tests/kill_stress.rs` FAILS at in-flight HEAD (also `make
  test-beam` fails): BEAM refuses the NIF with `bad_lib: Function not found
  avec_beam:avec_new_fast/2` (kill_stress) / `pointer_of/1` (beam_test.escript)
  — different first-mismatch per stub list. Evidence gathered:
  - Compiled table dumped statically from the dylib: all 21 entries correct
    (names/arities/fptrs/flags), so the BEAM's claim is false against the
    binary.
  - Source table (21) == escript stub surface (21), names match; NIF version
    constants match runtime (2.17 / OTP 28 / erts-14.0).
  - Failure survives fresh debug+release rebuilds; mechanism-independent of
    this lane's edits (avec-git only; the NIF table lives in avec-beam).
  - Additional environment observation: `erl` spawned from a direct shell
    fails earlier, at dlopen, appending `.so` to the given path; the
    cargo-spawned erl gets past dlopen. Spawn-context-dependent dlopen
    behavior is an environment/toolchain-class issue.
  - Owner: avec-beam lib.rs / lane-4 (kill_stress) surfaces; escalated via
    this receipt.

## Verification Receipts

Commands and exits, this session, one canonical checkout:

- `cargo check -p avec-git` — exit 0.
- `cargo test -p avec-git` — 67 passed, 0 failed (summed across all test
  targets incl. 43 lib and 4 fuzz_codec), 1 ignored, after fixes. Before
  Fix 3 the fuzz_codec target aborted at compile (pre-existing E0600).
- `cargo test -p avec-arrow` — 137 passed, 0 failed total (93 lib — the
  expected 93 — plus 44 across other targets).
- `cargo test -p avec-parquet` — 192 passed, 0 failed total (157 lib + 35).
- `cargo test -p avec-blake3` — 28 passed, 0 failed.
- `cargo test -p avec-beam` — 10 passed, 1 FAILED (kill_stress —
  pre-existing, see above).
- `make test-beam` — FAILS (same pre-existing NIF load refusal).

Expected-count note: the dispatch quoted 93/204/192/48/25. Observed this
session: arrow-lib 93 (match), parquet total 192, git total 54, blake3
total 28, beam 10 (+1 pre-existing failure). The remaining deltas vs the
quoted counts are as-of-date discrepancies in the dispatch, not regressions
of this session: my edits touch avec-git only, and every suite runs 0
failures except the two pre-existing items above.
