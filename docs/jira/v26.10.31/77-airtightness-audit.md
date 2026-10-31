# Unsafe-Block Airtightness Audit (wave-14 lane 8)

Date: 2026-10-31. Lane: wave-14 lane 8 (recovery). Repo: /Users/sac/avec (one
canonical checkout, no worktrees). New file only; sources read-only.

## Scope and Method

Full inventory of `unsafe` in `crates/*/src` (grep count per file, then every
site read in context). A "site" is a line carrying the `unsafe` keyword in
code position; lint attributes (`#![forbid(unsafe_code)]`,
`#![forbid(unsafe_op_in_unsafe_fn)]`) and doc-comment mentions are excluded
from site counts. Classification:

- **FFI boundary** — `enif_*` NIF calls, POSIX syscalls, `extern "C"` exports,
  Metal shim calls: the invariant is a cross-language/cross-process contract.
- **Raw-pointer kernel** — `from_raw_parts` / `set_len` /
  `copy_nonoverlapping` / constant-time pointer selects inside otherwise-safe
  Rust.
- **OTHER** — the only sites outside the two classes are GlobalAlloc counter
  impls forwarding to `std::alloc::System` (allocator plumbing,
  std-sanctioned shape); recorded as such, not findings.

Per site: the one-line soundness invariant plus whether a test court
witnesses it. Verdict scale: **AIR-TIGHT** (every site has a stated invariant
and a running witness), **PARTIAL** (invariants stated but unwitnessed items
exist — listed), **LEAKY** (missing or wrong invariant). No crate earns
LEAKY: every counted site carries a written invariant.

## Inventory (grep receipts)

Site line-counts, code-position `unsafe` only. Zero-site crates: `avec-git`,
`cli`, `integration`, 10 bench bins, bench/src/benches/v26_10_31.rs (grep -c
0 across the workspace sweep).

```text
audit        32   lib.rs 23 + alloc_counter.rs 9
avec-arrow  131   blake3 32, c_abi_nested 33, c_abi 18, foreign 17,
                  slice 10, buffer 7, mmu_arena 5, chase_lev 4,
                  transpose 3, silicon 1, predicates 1
avec-beam   135   lib 37, shm 33, ops_scan 24, resources 13,
                  ops_persist 10, resbin 7, pointer 6, spsc_ring 3,
                  alloc_spy 1, dtor_stats 1
avec-blake3  66   lib 32, multi_chunk 34
avec-ffi     10   lib.rs: 5 production + 5 in #[cfg(test)]
avec-gpu      4   lib.rs (from_raw_parts, 2 callers, Drop)
avec-parquet 36   dispatch_table 15, rle 14, scan 4, stream_compact 3
bench        49   GlobalAlloc counters: main 5, ctq_report 9,
                  arrow_accessors/byteslice/dict/packed_filter/
                  packing/page/parquet 5 each
wasm-footpr.  7   lib.rs (BumpAlloc + 4 caller-contract slice helpers)
```

Workspace total: 470 code-position `unsafe` lines.

## forbid register

- `avec-git` — `#![forbid(unsafe_code)]` — HELD post-deflate/inflate.
  Evidence: src/lib.rs:10 carries the attribute; deflate.rs, inflate.rs,
  ladder.rs, pack_write.rs, idx_read.rs have 0 sites each (the only
  "unsafe" strings in idx_read.rs:5 and inflate.rs:4 are doc comments
  attesting zero unsafe).
- `avec-arrow` — `#![forbid(unsafe_op_in_unsafe_fn)]` (src/lib.rs:19).
- `avec-parquet` — `#![forbid(unsafe_op_in_unsafe_fn)]` (src/lib.rs:11).
- `avec-blake3` — `#![forbid(unsafe_op_in_unsafe_fn)]` (src/lib.rs:12).
- `wasm-footprint` — `#![forbid(unsafe_op_in_unsafe_fn)]` (src/lib.rs:35).
- `audit`, `avec-ffi`, `avec-gpu`, `bench` — no crate-level unsafe lint
  (unsafe-by-purpose crates).
- `cli`, `integration` — no lint, zero sites anyway.

## Per-crate verdicts

### avec-git — AIR-TIGHT

`#![forbid(unsafe_code)]` compiles the question away: zero sites, forbid
held after the deflate/inflate work (lib.rs:10). Courts: gate-git-tests-v3
(`cargo test -p avec-git`, exit 0, 2026-10-31) over fsck_court.rs,
fuzz_codec.rs, packfile_fuzz.rs, large_offset.rs, ladder_mutations.rs,
git_interop.rs, perf_baseline.rs.

### avec-ffi — AIR-TIGHT

5 production sites in lib.rs, all FFI boundary (C Data Interface exports),
plus 5 sites inside the inline corruption-court test module.

- lib.rs:21 `avec_i32_batch` (unsafe extern "C") — FFI. Invariant: export
  slots are process-lifetime leaked by design; the caller takes release
  ownership per the C Data Interface contract.
  Witness: tests/zero_copy.rs:51 pointer-identity court; inline corruption
  court lib.rs:249.
- lib.rs:108 + :120 `digest_of_raw` — FFI. Invariant: array_addr is a
  caller-asserted live export; read-only fold; negative length refused by
  assert before any deref.
  Witness: inline corruption court lib.rs:249 (flip -> digest change ->
  restore); tests/alloc_courts.rs:74 zero-alloc digest court.
- lib.rs:142 values `from_raw_parts` — FFI. Invariant: live export, `n` i32
  lanes at buffers[1], 64-aligned builder allocation.
  Witness: inline corruption court; tests/zero_copy.rs:71
  mutation-through-view.
- lib.rs:162 validity `from_raw_parts` — FFI. Invariant: validity buffer
  carries exactly ceil(length/8) bytes (bitmap_bytes canon, bitmap.rs).
  Witness: inline corruption court; alloc_courts.rs:103 digest determinism.

### avec-gpu — PARTIAL

4 sites, all Metal shim FFI. Invariants documented per site (# Safety on
`from_raw_parts` at lib.rs:32: ptr valid for `len` initialized bytes, valid
and unmodified for the buffer's lifetime; Drop releases exactly once and
nulls the raw handle after release).

Unwitnessed: no tests/ directory and no runtime consumer anywhere in the
workspace (grep: zero references from integration, bench, cli).
gate-workspace-check-v3 (`cargo check --workspace`, exit 0) witnesses
compilation only. Finding: **unexercised FFI surface** — add a Metal
round-trip court or feature-gate the crate until one exists.

### avec-beam — PARTIAL

135 sites, all FFI boundary (enif_* NIF surface + POSIX shm syscalls).

- lib.rs — ~30 enif_* calls; argv `from_raw_parts(argv, argc.max(0))`;
  `unsafe impl Sync for ErlNifEntry/ErlNifFunc` (lib.rs:122-123);
  `unsafe impl Send for Batch` (lib.rs:197). Invariant: argv slice is
  VM-supplied and argc-bounded; func tables are 'static; the registry
  mutex is the sole accessor while a Batch is registered.
  Witness: tests/bridge_contract.rs (real dylib in a real BEAM,
  gate-beam-bridge-v2, exit 0); tests/kill_stress.rs (wave-14 lane 4:
  100x100 create/kill/verify with exact dtor-count accounting).
- shm.rs — shm_open/ftruncate/mmap/munmap/close/dup; header AtomicU64
  views; SPSC Acquire/Release; `unsafe impl Send/Sync for ShmRing`
  (shm.rs:160-161); unsafe ctor `from_fd` (shm.rs:237).
  Invariant: page-aligned mmap base makes the 8-aligned AtomicU64 views
  sound; head/tail Acquire orders slot copy before index publish; caller
  guarantees the fd outlives the handle.
  Witness: inline tests (shm.rs:484+, incl. the corrupt-header rejection
  court at shm.rs:582).
- ops_scan.rs — BatchView resource: dtor `ptr::read` +
  `enif_release_resource` (:184-187); `unsafe impl Send for BatchView`
  (:176); keep/alloc/release pairing (:263-275, :378-387);
  `enif_get_resource` type-tag check (:449); view cast (:454).
  Invariant: the view owns no payload — held VM reference + Copy metadata,
  so moving it moves nothing Rust must track; the VM runs the dtor at most
  once per allocation.
  Witness: bridge_contract.rs + kill_stress.rs (dtor-delta-equals-dead-count
  court).
- resources.rs — BatchEnvelope dtor `drop_in_place` (:175);
  keep/alloc/release (:241-355); `unsafe impl Send` (:153).
  Invariant: exactly-once drop driven by the VM resource refcount.
  Witness: kill_stress.rs (at-most-once ceiling + exact-count convergence).
- ops_persist.rs, resbin.rs, pointer.rs — enif binary alloc/copy/make +
  inspect; pointer-identity exports. Invariant: enif_alloc_binary success
  checked before write; copy stays within bytes.len().
  Witness: bridge_contract.rs; resbin zero-copy re-projection inside
  kill_stress.rs step 2.
- spsc_ring.rs — `unsafe impl<T: Send> Sync` (:30); slot `assume_init`
  (:94). Invariant: slot written before publish; T: Copy.
  Witness: inline tests only (spsc_ring.rs:115-139).

Findings (unwitnessed invariants): (a) the shm.rs SPSC cross-process
Acquire/Release protocol is exercised only by same-process inline tests —
no two-process court exists; (b) spsc_ring.rs `Sync` is inline-witnessed
only.

### avec-arrow — PARTIAL

131 sites across 11 files; crate carries
`#![forbid(unsafe_op_in_unsafe_fn)]`.

- blake3.rs (32) — raw-pointer kernel (SIMD). Invariant: target-gated
  NEON/AVX2 intrinsic calls over fixed 16-word state.
  Witness: **NONE — FINDING: zero consumers.** No `avec_arrow::blake3::`
  reference exists in any src/tests/benches across the workspace;
  `pub mod blake3` (lib.rs:28) is an unwitnessed duplicate of avec-blake3's
  court-witnessed SIMD compress. O(N^2) drift risk between two copies of
  the same kernel.
- c_abi.rs (18) — FFI. Invariant: release callbacks run at most once —
  release slot swapped to null before the body; Box::from_raw of the
  private owner.
  Witness: inline tests c_abi.rs:209+; exercised via avec-ffi
  alloc_courts.rs:135 release_callback_is_zero_alloc.
- c_abi_nested.rs (33) — FFI. Invariant: child schema/array release
  recursion reaches every child exactly once.
  Witness: inline courts c_abi_nested.rs:396/446/468 with counting release
  callbacks.
- foreign.rs (17) — FFI. Invariant: format string validated before any
  buffer view; bitmap byte length bounded from array length.
  Witness: inline tests foreign.rs:190-250 incl. the zero-buffer error path
  (:246).
- transpose.rs (3) — raw-pointer kernel. Sites: `col` set_len (:72) +
  transpose bodies (:101, :138). Invariant: Vec capacity == len exactly;
  every slot written before any read; T: Copy no drop glue
  (uninit-reserve, `#[allow(clippy::uninit_vec)]` with Safety comment);
  output columns sized len (digest len*32).
  Witness: inline tests only (transpose.rs:173-233: round-trip random
  corpora, digest length, zero-length, odd remainder). No external court
  references transpose (0 hits in crates/*/tests).
- predicates.rs (1) — raw-pointer kernel (`filter_quads_eq` ct-ptr loop,
  :135). Invariant: out.len() > n_quads asserted (sink slot); data.len()
  >= n_quads*4 (debug_assert); zero-trip handled by ct-select to a stack
  dummy quad.
  Witness: inline tests predicates.rs:165-311 (exhaustive domains, random
  corpora vs reference, sentinel-heavy, all/no-match).
- silicon.rs (1) — raw-pointer kernel. Invariant: n = min of the three
  slice lengths; every address base+i with i<n.
  Witness: inline tests silicon.rs:122-156.
- buffer.rs (7) — raw-pointer kernel. Invariant: 64-aligned non-zero
  layout; dealloc with the exact allocation layout; as_slice bounded by
  len.
  Witness: inline tests buffer.rs:132-157; downstream zero-alloc courts
  (avec-parquet page_zero_alloc.rs).
- mmu_arena.rs (5) — raw-pointer kernel. Invariant: 16 KiB-aligned layout;
  copy within capacity; dealloc pairing; Send/Sync for the page-aligned
  owned region.
  Witness: single inline test (mmu_arena.rs:117).
- chase_lev.rs (4) — raw-pointer kernel. Invariant:
  `Sync<T: Copy + Send>` justified by slot write-before-steal plus the
  contention turnstile; `assume_init` only after ownership transfer.
  Witness: inline tests chase_lev.rs:112-139 (2 tests); no concurrent
  steal-stress court.
- slice.rs (10) — raw-pointer kernel. Invariant: GlobalAlloc forwards
  System unchanged; ptr-address test asserts copy-free aliasing.
  Witness: inline tests slice.rs:167+.

Findings (unwitnessed invariants): (a) blake3.rs dead duplicate (above);
(b) transpose + predicates kernels are inline-witnessed only — a
cross-crate refactor that breaks write-before-read would be caught by no
crate-external court; (c) chase_lev has no concurrent contention court.

### avec-parquet — PARTIAL

36 sites; `#![forbid(unsafe_op_in_unsafe_fn)]`.

- dispatch_table.rs (15) — raw-pointer kernel (fn-pointer jump table).
  Invariant: each kernel guards null/zero-length before slice
  construction; token clamped with `& 0x0F`; the invalid index is a
  fail-closed no-op trap.
  Witness: inline `jump_table_dispatch_parity` (dispatch_table.rs:86).
- rle.rs (14) — raw-pointer kernel. Invariants: test-only Counting
  GlobalAlloc forwards System unchanged (rle.rs:364); Buf64 alloc/dealloc
  layout pairing (:613/:656); the `&mut [u32]` rebind of a 64-aligned byte
  buffer (:682) is sound because len/4 bounds the slice and 64B alignment
  covers u32 elements.
  Witness: inline tests rle.rs:276+; tests/rle_court.rs;
  tests/page_zero_alloc.rs.
- scan.rs (4) — FFI (mmap/munmap). Invariant: the only unsafe is the two
  syscalls plus the slice from a successful map; every derived access goes
  through bounds-checked `get`, so a corrupt footer yields a typed error,
  never UB; MAP_FAILED sentinel maps to a typed error (module doc
  scan.rs:31-34 states the law).
  Witness: tests/std_feature_court.rs, tests/robustness.rs,
  tests/offset_fuzz.rs (new, wave-14).
- stream_compact.rs (3) — raw-pointer kernel (NEON). Invariant: no
  memory-safety precondition — registers not pointers, vtbl1_u8 is total;
  unsafe only because the body executes target-gated intrinsics.
  Witness: inline smoke test (:88, aarch64-gated).

Verdict driver for PARTIAL: the rle.rs u32 rebind and the dispatch-table
guards are inline-witnessed but have no external mutation court; scan.rs
mmap is the best-witnessed FFI surface in the crate.

### avec-blake3 — AIR-TIGHT

66 sites, all raw-pointer/SIMD kernels: NEON + AVX2 intrinsic wrappers,
vst1q stores, chunk-compress callers under `cfg(target_arch)` /
`cfg(target_feature)` gates with fixed-size array parameters. Invariant at
every site: the cfg gate proves instruction availability; slice lengths
are 4x1024-byte chunk quanta with fixed 16-word state.
Witness: tests/official_vectors.rs (RFC vectors through the SIMD paths),
multi_chunk_vectors.rs, small_batch_vectors.rs, thread_differential.rs
(new, wave-14), zero_alloc.rs. Receipt: gate-blake3-tests-v3
(`cargo test -p avec-blake3`, exit 0, 2026-10-31).

### audit — PARTIAL

32 sites: 10 C-ABI raw-pointer exports in lib.rs (audit_mask_words,
audit_mask_words_neon, audit_count_valid, audit_get_bit, audit_set_bit,
audit_unpack_1|2|4|8bit, audit_is_inline) plus the GlobalAlloc counter in
alloc_counter.rs.

- Invariant: every export carries a `# Safety` line bounding `n` (or
  `i/8+1`) against the readable/writable extent at each pointer; bodies
  are pure pass-through to bounds-checked avec-arrow kernels.
- Witness: alloc_counter.rs inline tests (:156+); the crate is consumed
  and executed by integration/tests/error_paths.rs:20
  (`avec_audit::alloc_counter::with_alloc_tracking`) and
  bench/const_time_bench.rs.
- Finding: lib.rs has no inline test module and no tests/ directory — the
  C-ABI exports' n-bounding invariant has no direct court; the bench
  exercises the happy path only.

### bench — AIR-TIGHT (scope note)

All 49 sites are one shape: `unsafe impl GlobalAlloc for CountingAlloc`
forwarding to `std::alloc::System` with atomic counters. Invariant:
forwarding is faithful and counter reads are atomics. Witness: the benches
actually running in receipts (gate-ctq-report-v2, gate-divan-ctq,
gate-cpk-study-v4). Scope note: bench code is not production surface; no
separate court required.

### cli, integration — AIR-TIGHT (vacuous)

Zero unsafe sites (grep -c 0 in every file). Surrounding courts:
cli/tests/mutation_kills.rs; integration/tests/{admission_e2e, cross_crate,
error_paths, membranes}.rs with gate-e2e-integration-v3 exit 0.

### wasm-footprint — PARTIAL

7 sites: BumpAlloc GlobalAlloc (dealloc is a no-op "never free" bump
discipline); 4 caller-contract slice helpers (`u64_slice`, `u64_slice_mut`,
`u8_slice`, `u32_slice_mut`, lib.rs:172-208: len==0 short-circuits before
any deref, else ptr bounds len); `rust_eh_personality` symbol stub.

Witness: tests/no_std_core.rs (surfaces nameable + compiles no_std) and
scripts/footprint.sh node-execution court (artifact bytes = manifest
version). Finding: the caller-contract slice helpers are
compile/link-witnessed only; no wasm node court feeds a null or overlong
pointer to wasm_mask_words / wasm_unpack_1bit — the C-ABI callers are
external by definition.

## Summary

| Crate | Sites | Class | Verdict |
|---|---|---|---|
| avec-git | 0 | — | AIR-TIGHT (forbid held) |
| avec-ffi | 5+5t | FFI | AIR-TIGHT |
| avec-blake3 | 66 | raw-ptr/SIMD | AIR-TIGHT |
| bench | 49 | alloc plumbing | AIR-TIGHT |
| cli | 0 | — | AIR-TIGHT (vacuous) |
| integration | 0 | — | AIR-TIGHT (vacuous) |
| avec-beam | 135 | FFI | PARTIAL |
| avec-arrow | 131 | FFI + raw-ptr | PARTIAL |
| avec-parquet | 36 | FFI + raw-ptr | PARTIAL |
| avec-gpu | 4 | FFI (Metal) | PARTIAL |
| audit | 32 | FFI/raw-ptr | PARTIAL |
| wasm-footprint | 7 | FFI/allocator | PARTIAL |

PARTIAL drivers: avec-beam (shm SPSC cross-process protocol
same-process-only; spsc_ring Sync inline-only), avec-arrow (blake3.rs dead
duplicate with zero witnesses; transpose/predicates inline-only; chase_lev
no contention court), avec-parquet (rle u32-rebind + dispatch guards
inline-only), avec-gpu (zero runtime witnesses), audit (C-ABI exports have
no direct court), wasm-footprint (caller-contract helpers
compile-witnessed only).

## Findings register (ranked)

1. **avec-arrow/src/blake3.rs is an unwitnessed dead duplicate** — 32 unsafe
   sites, zero imports of `avec_arrow::blake3::` anywhere in the workspace;
   avec-blake3 carries the court-witnessed copy of the same SIMD compress.
   Drift between the two has no falsifier today. Close: delete the module
   or bind it to official vectors.
2. **avec-gpu Metal buffer lifetime never executed** — no tests dir, no
   consumer; the from_raw_parts/Drop release-once invariants are prose
   only. Close: Metal round-trip court or feature-gate.
3. **audit C-ABI exports have no direct court** — `# Safety` n-bounds
   documented, never asserted by a test. Close: port const_time_bench
   coverage into a tests/ court with negative-n cases.
4. **transpose/predicates kernels inline-witnessed only** — write-before-
   read and sink-slot invariants would survive a cross-crate refactor
   break. Close: name both modules in an integration court.
5. **shm.rs SPSC protocol has no two-process court** — same-process inline
   tests cannot witness the cross-process memory-ordering claim the
   Send/Sync impls make. Close: fork-pair court driving producer/consumer.
6. **wasm-footprint slice helpers compile-witnessed only** — no
   hostile-pointer node court. Close: extend scripts/footprint.sh with a
   null/overlong-pointer rejection case.
7. **chase_lev.rs has no concurrent-steal stress court** — turnstile
   contention resolution untested under racing consumers.

## Receipt

- Subject: unsafe airtightness across crates/*/src, /Users/sac/avec, one
  canonical checkout, sources read-only (new file only: this document).
- Method receipt: `grep -rn unsafe crates/*/src` per-file counts re-read at
  write time; every counted site read in context; forbids re-confirmed on
  disk before writing this document.
- Court receipts cited: gate-git-tests-v3, gate-beam-bridge-v2,
  gate-blake3-tests-v3, gate-parquet-tests, gate-workspace-check-v3,
  gate-e2e-integration-v3, gate-ctq-report-v2 (all exit 0, 2026-10-31,
  docs/jira/v26.10.31/receipts/).
- Standing: AUDIT (read-only); verdicts are claims about invariants and
  witnesses, each falsifiable by running the named court.

## See Also

- docs/jira/v26.10.31/78-panic-freedom-audit.md (companion hot-path audit)
- docs/jira/v26.10.31/83-falsifier-register.md
- docs/jira/v26.10.31/31-verification-ladder.md
