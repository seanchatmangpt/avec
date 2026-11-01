# doe-1-atomic-portability-tax

- Charter source: `docs/jira/v26.11.1/dflss/00-dflss-charter-source.md` (§4 Phase 3, item 1)
- Conversion lane: `C11-doe-1-atomic-tax` (charter conversion engineer, lane-runner)
- Status: SPECIFICATION — instruments not yet landed; every predicted number is `TARGET`
- Standing vocabulary: `ALIVE` (ran + passed this session, command + exit cited) /
  `PARTIAL` (structure present on disk, execution not witnessed this session) /
  `PENDING-LAND` (instrument absent on disk; command specified, not run) /
  `UNVERIFIED` (default; never rounded up)

## 1. Charter Quote (Verbatim Source of Truth)

> **DOE-1 (32-Bit Atomic Portability Tax):** Measure the 64-bit host `AtomicI64`
> path against the `AtomicU32`-pair fallback required for 32-bit RISC-V targets to
> determine whether conditional compilation gating (`cfg(target_has_atomic)`) is
> warranted.

All numbers below are predictions and carry `TARGET` standing until the receipt in
§10 is filled with a witnessed command + exit + table.

## 2. Subjects Under Test (on-disk state, witnessed this session)

Kernel file: `crates/cgap-arrow/src/chase_lev.rs` — the sole deque implementation
in the tree. Three cfg tiers (file lines 40-76), identical protocol orderings on
every tier that has CAS (file lines 26-30):

- **Tier 1** — cfg `target_has_atomic = "64"`: `Index = i64`, cell `AtomicI64`,
  CAS turnstile. Witness: host aarch64-apple-darwin, 4/4 protocol tests, exit 0
  (§11 C1).
- **Tier 2** — cfg `all(target_has_atomic = "32", not(target_has_atomic = "64"))`:
  `Index = isize`, cell `AtomicIsize` (32-bit on such targets), CAS turnstile.
  Witness: none — no installed target selects this tier (§11 C3).
- **Tier 3** — cfg `not(any(target_has_atomic = "64", target_has_atomic = "32"))`:
  `Index = isize`, cell `AtomicIsize`, load/store only; `steal` is typed `None`
  and `SUPPORTS_STEAL = false`. Witness: riscv32imc-unknown-none-elf check exit 0
  (§11 C2).

Constants (file lines 36-37): `DEQUE_CAP = 512` (power of two), `DEQUE_LINE = 64`.
Wrap period: 2^32 cursor advances on tiers 2/3 vs 2^64 on tier 1 (file lines 13-17).

## 3. Pre-Registered Divergence Register (charter vs tree)

- **D1 — the charter's `AtomicU32`-pair does not exist in the tree.** The charter
  names an "`AtomicU32`-pair fallback"; the landed tier-2 design is machine-word
  `AtomicIsize` (the classic Chase-Lev machine-word form, `chase_lev.rs:55-63`),
  which needs no 64-bit cell and no two-word pair; no `AtomicU32`-pair
  implementation exists anywhere under `crates/`. This DOE measures the LANDED
  representation (host `AtomicI32` twin, §6.2) and records the wording divergence
  here rather than redefining the charter pin. The decision-rule structure (§9)
  is unchanged under either representation.
- **D2 — no chase_lev court in the shared harness.** `crates/cgap-court` is a
  bare skeleton (`Cargo.toml` only; §11 C4): no court file, no chase_lev court.
  The four in-file tests (`chase_lev.rs:225-297`) are the only protocol witness.
  Mutation flip targets (§8) are therefore the in-file court; if a mutation
  cannot flip it, §8.3 applies (loud FAIL; court built before result admissible).
- **D3 — stale wall text.** `crates/cgap-ffi/src/sovereign.rs:2274` pins the T1
  `[SKIP]` wall to "chase_lev.rs AtomicI64", but `cargo check -p cgap-arrow
  --target riscv32imc-unknown-none-elf` exited 0 this session (§11 C2). The
  `c_abi_nested.rs` `Atomic<T>::swap` leg of that wall was not re-witnessed this
  session and stays `UNVERIFIED` until its owner re-pins. Owner lane: A01.

## 4. Hypothesis H1 (all magnitudes `TARGET`)

- **H1.** The machine-word (32-bit-wrapping) index representation carries a
  per-operation tax `TAX = t_mw / t_i64` of at most 25% (`TAX <= 1.25`) against
  the `AtomicI64` representation under an identical host workload, because
  aarch64 lowers 32- and 64-bit acq/rel/CAS atomics to the same ldxr/stxr
  family.
- **Point estimate (TARGET):** `TAX ~= 1.05`, range `1.00-1.20`.
- **Predicted decision (TARGET):** `UNIFY` (§9 threshold not crossed).
- **Falsifier:** any sweep cell with `TAX > 1.25` refutes H1 and forces `GATE`.
- **Proxy bound (stays `TARGET`/`UNVERIFIED` regardless of host result):**
  absolute per-op nanoseconds on 32-bit silicon. The host `AtomicI32` twin
  measures representation tax (cell width + wrap semantics), not tier-2
  microarchitecture. Silicon numbers require the tier-2 execution instrument
  (§5), which is `PENDING-LAND`.

## 5. Instruments (witnessed inventory, this session)

| Instrument | Status | Evidence |
| --- | --- | --- |
| chase_lev protocol tests, tier 1 (4 tests) | `ALIVE` | §11 C1 — 4/4 pass, exit 0 |
| tier-3 compile leg (`riscv32imc-unknown-none-elf`) | `ALIVE` | §11 C2 — exit 0 |
| Divan membrane (`crates/bench`, divan 0.1.21) | `PARTIAL` | §11 C5 — present, not executed |
| DOE-1 bench file (`doe1_atomic_tax.rs`) | `PENDING-LAND` | §11 C6 — absent |
| tier-2 host twin module (`AtomicI32`) | `PENDING-LAND` | not in tree; generated per §6.2 |
| tier-2 compile leg (armv7-class target) | `PENDING-LAND` | §11 C3 — no tier-2 target |
| tier-2/3 execution (qemu user mode) | `PENDING-LAND` | §11 C7 — `qemu-riscv32` absent |
| shared-harness chase_lev court | `PENDING-LAND` | §3 D2 |

## 6. Bench Construction (both arms behind one identical workload)

### 6.1 File and Cargo wiring (`PENDING-LAND`)

- Create `crates/bench/benches/doe1_atomic_tax.rs`; register it in
  `crates/bench/Cargo.toml` (sole edit; no kernel change):

```toml
[[bench]]
name = "doe1_atomic_tax"
harness = false
```

- Divan 0.1.21 requires `harness = false` and `fn main() { divan::main() }`.
- Consumed API: `cgap_arrow::chase_lev::WorkStealingDeque` only (public surface;
  no private-path reach).
- Execution command (§10): `cargo bench -p bench --bench doe1_atomic_tax`.

### 6.2 Tier-2 arm: host `AtomicI32` twin — derivation law (generated, never hand-copied)

- The host selects only tier 1, so the A/B arm is a twin module compiled on host
  over `AtomicI32` (native 32-bit CAS on aarch64): identical orderings, identical
  wrap semantics, narrower cell.
- **Derivation rule.** Generate the twin from the canonical file by script, never
  by hand: start from `chase_lev.rs`, keep `push`/`pop`/`steal` bodies and every
  `Ordering::` argument byte-identical, drop the three `mod index_width`
  variants, emit one `mod index_width` with `Index = i32`,
  `AtomicIndex = AtomicI32`, `ZERO = 0i32`, `MASK = (DEQUE_CAP - 1) as i32`.
  `DEQUE_CAP`/`DEQUE_LINE` unchanged; `SUPPORTS_STEAL` removed (the host twin is
  always CAS-capable).
- **Drift check (executable).** Re-run generation into `/tmp` and diff against
  the checked-in twin: the diff MUST be empty. Stripping `Ordering::` arguments
  from both files MUST yield identical text (proves orderings did not drift).
- **Proxy law (stated, bounded).** Twin == tier 2 in cell width (32-bit), wrap
  semantics (`wrapping_sub` on 32 bits), and orderings; differs in
  microarchitecture. §4 fixes what stays `TARGET` because of this.

### 6.3 Workload and const sweep (`PENDING-LAND`)

- Item payloads from SplitMix64 seeded `0xBF58_476D_1CE4_E5B9` (charter pin);
  every consumed value wrapped in `black_box`; per-cell state checksum asserted
  at cell end (charter Phase 2 realism law).
- Cells: `push+pop` owner-only at depths {1, 64, 512} (const-generic sweep);
  `steal` at 2T (1 owner + 1 thief) and 4T (1 owner + 3 thieves), items
  pre-pushed to depth 512. The compared Divan column is mean ns/op per cell.

## 7. Invariant (Named In Advance of Measurement)

**INV — Chase-Lev protocol semantics preserved on both arms.** Timing is
admissible only if every clause below holds on the canonical tier-1 arm AND the
twin arm, before any Divan column is recorded:

- **INV-1 (LIFO slot discipline):** the owner's `pop` yields the most recently
  pushed not-yet-consumed element; a full drain of a fresh deque replays the
  pushes in exact reverse order.
- **INV-2 (conservation, no lost or phantom pops):** over any interleaving,
  `multiset(stolen) ⊎ multiset(popped) = multiset(pushed)`.
- **INV-3 (exactly-one claimant):** every element is claimed by exactly one of
  {owner `pop`, stealer} — nothing lost, nothing duplicated.
- **INV-4 (FIFO steal):** elements leave via `steal` in increasing cursor order.
- **INV-5 (wrap invariance):** `bottom - top` stays within `[0, DEQUE_CAP]`
  under 32-bit wrapping; the slot index is always `cursor & (DEQUE_CAP - 1)`;
  behavior across the wrap point is indistinguishable from pre-wrap behavior.
- **INV-6 (typed capability):** on a tier without CAS, `steal` returns `None`
  and never claims without RMW; `SUPPORTS_STEAL` agrees with the compiled tier.

Admissibility gate: the protocol court (today the in-file tests, §3 D2; later
the shared court) passes on BOTH arms pre-timing. Any failed clause refuses the
DOE result outright (`REFUSED`).

## 8. Anti-Vacuity Mutations (sandbox /tmp copies, sha256 fail-closed restore)

Each mutation lands in a `/tmp` sandbox copy only (sha256 recorded before,
byte-exact restore after, never the shared tree). A court that cannot flip is
`FAIL`; a mutation that cannot shift a measured column marks the benchmark
`VACUOUS` and graduation from `TARGET` to `MEASURED` is `REFUSED` (charter
Phase 2 law).

- **M1 (ordering demotion — dispatch-named class, adapted to landed orderings).**
  In the sandbox copy, demote the steal-path `top` CAS success ordering
  `SeqCst -> Relaxed`. (The landed code runs SeqCst CAS + Acquire loads on
  `top`; there is no `AcqRel` on the top pointer to swap — recorded as an
  adaptation, not a redefinition.) Predicted flip: the
  `chase_lev_concurrent_stealing` conservation assert (`chase_lev.rs:265`)
  fails under a 200-iteration stress loop with element loss or duplication.
  Fail-closed: if the court survives 200 iterations on aarch64, the mutation is
  recorded NON-FLIPPING for that court and §8.3 applies — never a silent pass.
- **M2 (deterministic structural).** Change the full-deque check
  `b.wrapping_sub(t) >= DEQUE_CAP` to `>` in the sandbox copy. Predicted flip:
  push 513 overwrites slot 0; a court pushing `DEQUE_CAP + 1` items then
  draining sees a deterministic multiset mismatch (element `0` lost,
  `chase_lev.rs:270`).
- **M3 (bench column shift).** Inject one `fence(SeqCst)` into `push` before
  the bottom store. Predicted shift: the push ns/op column rises (TARGET:
  >= 2x). No shift ⇒ the benchmark is `VACUOUS`, graduation `REFUSED`.
- **M4 (arm collapse).** Point the twin at `Index = i64` so both arms carry
  byte-identical representations. The TAX column must collapse to ~1.00; a
  persistent delta over identical arms proves the harness measures an artifact
  rather than the representation ⇒ `VACUOUS`.

### 8.3 Court qualification (pre-condition for any admissible result)

M1 and M2 must flip the protocol court; M3 must shift and M4 must collapse the
bench columns. Any mutation that cannot do its job fails the DOE loudly — a
court that cannot flip is `FAIL`; skips are `[SKIP] <reason>`, never silent.

## 9. Decision Rule (stated numerically, in advance)

- **Statistic.** Per cell, `TAX = median-over-5-runs(mean ns/op on twin) /
  median-over-5-runs(mean ns/op on canonical)` — same machine, same command,
  no sleeps; runs interleaved A/B/A/B to decorrelate thermal drift.
- **Rule.**
  - Any cell with `TAX > 1.25` ⇒ **GATE**: keep the `cfg(target_has_atomic)`
    `index_width` split; the 64-bit tier is a distinct, warranted tier.
  - All cells `TAX <= 1.25` ⇒ **UNIFY**: collapse tiers 1+2 to one machine-word
    representation on every CAS tier, witnessed by the tier-2 compile leg.
  - A cell with `TAX < 1.00` is recorded as a divergence row in §10 (twin
    faster than `AtomicI64`) but does not change the outcome (`UNIFY`).
- **Predicted outcome (TARGET):** max-cell `TAX ~= 1.05` ⇒ `UNIFY`.
- **Noise guard (both required):** M4 collapse within `1.00 ± 0.05`, and two
  independent full sweeps agreeing within 3% per cell. A failed guard leaves
  the result `UNVERIFIED` and no decision is issued.

## 10. Receipt Template (fill at execution; standing `TARGET` until then)

```text
DOE-1 RECEIPT — TEMPLATE (all values TARGET until executed)
command   : cargo bench -p bench --bench doe1_atomic_tax
exit      : ___ (admissible only at 0)
env       : host aarch64-apple-darwin / rustc 1.101.0-nightly (12433c307 2026-10-24)
            divan 0.1.21 / seed 0xBF58_476D_1CE4_E5B9
table     : cell              i64 ns/op   twin ns/op   TAX
            push+pop d=1      TARGET      TARGET       TARGET (<= 1.25)
            push+pop d=64     TARGET      TARGET       TARGET (<= 1.25)
            push+pop d=512    TARGET      TARGET       TARGET (<= 1.25)
            steal 2T          TARGET      TARGET       TARGET (<= 1.25)
            steal 4T          TARGET      TARGET       TARGET (<= 1.25)
invariant : INV-1..INV-6 PASS on both arms pre-timing (§7), else REFUSED
mutations : M1 flip [PASS | NON-FLIPPING -> court FAIL]
            M2 flip [PASS | NON-FLIPPING -> court FAIL]
            M3 shift [PASS | VACUOUS]
            M4 collapse [PASS | VACUOUS]
decision  : GATE | UNIFY per §9  (predicted: UNIFY, TARGET)
standing  : TARGET -> MEASURED only with the command + exit + table above
```

## 11. Session Command Receipts (run this session, cited above)

- **C1** `cargo test -p cgap-arrow --lib chase_lev`
  (CARGO_TARGET_DIR=/tmp/c11-target) → `4 passed; 0 failed`, RC_TEST=0.
- **C2** `cargo check -p cgap-arrow --target riscv32imc-unknown-none-elf` →
  `Finished dev profile`, RC_RV32=0.
- **C3** `rustup target list --installed` → exit 0: aarch64-apple-darwin,
  aarch64-unknown-linux-gnu, riscv32imc-unknown-none-elf, wasm32-unknown-unknown,
  wasm32-wasip1, x86_64-unknown-linux-gnu. No armv7-class target ⇒ tier-2 leg
  `PENDING-LAND`.
- **C4** `ls -R crates/cgap-court` → `Cargo.toml` only; `grep -rln
  "chase_lev|WorkStealingDeque"` over `crates/cgap-court/` → no match (RC=1)
  ⇒ no shared chase_lev court (§3 D2).
- **C5** `crates/bench/Cargo.toml`: `divan = "0.1.21"` under
  `[dev-dependencies]`; `[[bench]] name = "v26_10_31", harness = false`
  present ⇒ membrane `PARTIAL` (exists, not executed this session).
- **C6** `ls crates/bench/benches/` → only `v26_10_31.rs`; DOE-1 bench absent
  ⇒ `PENDING-LAND`.
- **C7** `command -v qemu-riscv32` → exit 1 (absent); `qemu-system-riscv32`
  present at `/opt/homebrew/bin` (bare-metal transport; out of DOE-1 scope).
- **C8** `rustc --version` → `rustc 1.101.0-nightly (12433c307 2026-10-24)`.

