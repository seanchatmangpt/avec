# Equivalent Mutants

Ledger of the `avec-arrow` cargo-mutants survivors, lane OPT20-L2, v26.10.29: the 11
rows named by the completed triage plus V5, a 12th survivor (varlen.rs:123) the
confirmation scan surfaced. Every argument was re-derived from the current source on
2026-10-29 (not copied from the prior lane receipt); the scan command is fixed in
[Scan](#scan) with its verbatim summary.

## Scan

Command (v26.10.29), run 2026-10-29, exit code 0, output `/tmp/opt20-mut-arrow`:

```sh
cargo mutants --workspace --exclude avec-audit --exclude bench --exclude avec-ffi \
  --exclude avec-beam --exclude avec-parquet -o /tmp/opt20-mut-arrow --timeout 60
```

Summary verbatim:

```text
485 mutants tested in 31m: 58 missed, 342 caught, 73 unviable, 12 timeouts
```

All 11 triage rows re-appear in `mutants.out/missed.txt` with the same site and
operator. The scan also missed 12 avec-arrow mutants total (11 ledgered + V5) and 46
mutants in `crates/cli/src/main.rs`, which is outside this lane's scope.

## Register

| id | site (current source) | class |
|----|-----------------------|-------|
| B1 | bitmap.rs:188 (`is_all_valid` tail guard) | equivalent |
| B2 | bitmap.rs:214 (`count_one_bits` tail guard) | equivalent |
| P1 | primitive.rs:106 (`buffers_aligned_64` body) | unreachable-false |
| P2 | primitive.rs:110 (`buffers_aligned_64` conjunction) | unreachable-false |
| R1 | runend.rs:94 (`buffers_aligned_64` body) | unreachable-false |
| R2 | runend.rs:95 (`buffers_aligned_64` conjunction) | unreachable-false |
| S1 | silicon.rs:77 (`mask_words_neon` fallback arm) | cfg-host |
| V1 | varlen.rs:117 (`offsets_monotonic` body) | unreachable-false |
| V2 | varlen.rs:118 (`offsets_monotonic` guard) | unreachable-false |
| V3 | varlen.rs:124 (`offsets_monotonic` accumulate) | unreachable-false |
| V4 | varlen.rs:124 (`offsets_monotonic` index) | unreachable-false |
| V5 | varlen.rs:123 (`offsets_monotonic` loop latch) | unreachable-false |

## Shared invariants the arguments rest on

1. **Buf pointer law** (`buffer.rs:12-52`): every `Buf` stores a 64-byte aligned
   pointer on every reachable state — `Buf::new(0)` returns the literal sentinel
   address 64 (0x40) and any other length comes from
   `alloc(Layout::from_size_align(len, ALIGNMENT))`. The field is private and
   `new`/`new_zeroed` are the only constructors, so `as_ptr() % 64 == 0` is
   type-level, not builder-path-level.
2. **Offsets law** (`varlen.rs:146-196`): the only reachable `Varlen` constructor is
   `VarlenBuilder::finish` (the `Varlen { .. }` literal at varlen.rs:188 has exactly
   one caller). `finish` writes `n + 1 >= 1` offsets whose first entry is 0 and whose
   values never decrease (`push` appends the current `data.len()`), then
   `debug_assert!(v.offsets_monotonic())`. So `offsets_monotonic()` returns `true` on
   every reachable state, and "empty offsets" / "first offset != 0" are unreachable.
3. **Bitmap length law** (`bitmap.rs:104,122`): `Bitmap::from_bytes` has no callers
   in the workspace; every reachable `Bitmap` comes from `from_bools`, which
   allocates exactly `bitmap_bytes(n)` bytes. So at `tail == 0` the tail block reads
   `word(full_words)` with `off == bytes.len()` — a legal empty slice read (`take =
   0`), never a panic.

## Per-row arguments

### B1 — bitmap.rs:188 — `if tail > 0` -> `if tail >= 0` — equivalent

- Mutant: `replace > with >= in Bitmap::is_all_valid`
- `tail = self.n_bits % 64` is `usize` in `[0, 63]`, so the mutated guard is a
  tautology: the block now also runs when `tail == 0`.
- At `tail == 0`: `tail_mask = (1 << 0) - 1 = 0`, `word(full_words) & 0` pops
  0 bits, so the accumulation is unchanged (the `tail == 64` arm stays dead in
  original and mutant alike). `word(full_words)` at `off == bytes.len()` takes
  0 bytes — legal, no panic (invariant 3).
- Divergence would need `tail < 0`: impossible. Class: equivalent (the guard
  swap changes nothing anywhere; the extra body execution is a no-op).

### B2 — bitmap.rs:214 — `if tail > 0` -> `if tail >= 0` — equivalent

- Mutant: `replace > with >= in count_one_bits`
- Same shape as B1 on the free function: `tail = bm.n_bits() % 64 in [0, 63]`
  tautology; at `tail == 0` the block adds
  `popcount(word(full) & ((1 << 0) - 1)) = popcount(0) = 0`. No panic
  (invariant 3). Class: equivalent.

### P1 — primitive.rs:106 — body -> `true` — unreachable-false

- Mutant: `replace Primitive<T>::buffers_aligned_64 -> bool with true`
- Original returns `true` on every reachable `Primitive`: the validity side is
  `is_none_or(aligned)` over an `Option<Bitmap>` whose backing `Buf` obeys
  invariant 1, and the values side is a `Buf` pointer. The function cannot
  return `false` on any reachable state, so the constant-`true` body is
  indistinguishable. Class: unreachable-false (the false branch is dead on the
  reachable state space).

### P2 — primitive.rs:110 — `&&` -> `||` — unreachable-false

- Mutant: `replace && with || in Primitive<T>::buffers_aligned_64`
- Both operands are always `true` on reachable states (invariant 1), so
  original `T && T` and mutant `T || T` agree everywhere reachable. Divergence
  needs a state with `v_ok = false` (or an unaligned values `Buf`) — none
  exists. Class: unreachable-false.

### R1/R2 — runend.rs:94/95 — alignment conjunction — unreachable-false

- Mutants: `replace RunEndEncoded::buffers_aligned_64 -> bool with true` and
  `replace && with || in RunEndEncoded::buffers_aligned_64`
- Same alignment class as P1/P2: `run_ends` is a `Buf` (invariant 1) and
  `self.values.buffers_aligned_64()` is always `true` (P1 argument, applied to
  the values child). Both mutants preserve the constant-`true` behavior on all
  reachable states. Class: unreachable-false.
- Scope note: the exact cargo-mutants label for R1 is confirmed by the scan
  (the backlog ledger predates the current source by a small shift and holds
  no runend rows); if the scan reports a different operator at these sites the
  row is re-opened as NEEDS-KILL.

### S1 — silicon.rs:77 — cfg'd-out fallback arm — cfg-host

- Mutant: `replace mask_words_neon -> () with ()`
- The mutated function is the `#[cfg(not(target_arch = "aarch64"))]` fallback
  delegating to `crate::bitmap::mask_words`. On this host (aarch64) that
  function is compiled out of every test binary, so no test in this workspace
  can observe the mutation. Killing it requires building for a non-aarch64
  target — out of this court's host scope. Class: cfg-host.

### V1 — varlen.rs:117 — body -> `true` — unreachable-false

- Mutant: `replace Varlen<O>::offsets_monotonic -> bool with true`
- By invariant 2 the original returns `true` on every reachable `Varlen`, so
  the constant-`true` body is indistinguishable. Class: unreachable-false.

### V2 — varlen.rs:118 — `||` -> `&&` — unreachable-false

- Mutant: `replace || with && in Varlen<O>::offsets_monotonic`
- Divergence shapes, both unreachable (invariant 2):
  (a) empty offsets: original returns `false`; mutant evaluates `off[0]` on an
  empty slice and panics — no reachable state has empty offsets;
  (b) non-empty with `off[0] != 0`: original returns `false`, mutant skips the
  guard and returns `true` — `finish` always writes first offset 0.
- On all reachable states (non-empty, first offset 0) both compute `true`.
  Class: unreachable-false.

### V3 — varlen.rs:124 — `&=` -> `|=` — unreachable-false

- Mutant: `replace &= with |= in Varlen<O>::offsets_monotonic`
- On reachable (monotonic) offsets `off[i] <= off[i+1]` is always `true`, so
  `ok &= true` and `ok |= true` both leave `ok == true`. Divergence needs a
  reachable non-monotonic `Varlen`, which invariant 2 excludes. Class:
  unreachable-false.

### V4 — varlen.rs:124 — `off[i + 1]` -> `off[i * 1]` — unreachable-false

- Mutant: `replace + with * in Varlen<O>::offsets_monotonic`
- `i + 1` -> `i * 1 = i` makes the comparison `off[i] <= off[i]`: tautologically
  true, so the mutant also returns `true` on every reachable state, agreeing
  with the original (invariant 2). Divergence needs a reachable non-monotonic
  state; also no new panic (`i` is in bounds wherever `i + 1` was). Class:
  unreachable-false.

### V5 — varlen.rs:123 — `i + 1 < off.len()` -> `i + 1 > off.len()` — unreachable-false

- Mutant: `replace < with > in Varlen<O>::offsets_monotonic`
- Surfaced by the confirmation scan (not in the triage's 11). The mutated latch
  `i + 1 > off.len()` is false at `i == 0` for every reachable `off.len() >= 1`
  (invariant 2), so the loop body never runs and the mutant returns the initial
  `ok == true` — identical to the original on every reachable state, where the
  original also always returns `true`. Divergence needs a reachable non-monotonic
  `Varlen`, which invariant 2 excludes; the dead loop also cannot panic. Class:
  unreachable-false.
- Note: the `<` -> `==` twin at the same site IS caught (a builder varlen makes
  the mutated loop index `off[len]` and panic), so this row is the surviving
  direction only.

## Needs-kill

None. All 12 rows re-derived from the current source; no row is NEEDS-KILL, so
`crates/avec-arrow/tests/mutation_kills2.rs` is not required by this lane. V5 was not
in the triage's 11; it is registered here because the confirmation scan missed it and
its argument re-derives cleanly (below).

## See Also

- `docs/mutation-court-backlog.txt` (prior scan ledger; pre-edit line numbers)
- `crates/avec-arrow/tests/mutation_kills.rs` (existing killing tests)
