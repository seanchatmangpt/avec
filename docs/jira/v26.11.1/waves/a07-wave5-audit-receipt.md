# A07 Wave 5 Audit Receipt

- **Lane**: A07-bosym-slab (`crates/cgap-bosym/src/slab.rs`,
  `scripts/zero_string_gate.py`), Wave 5 (defect repair), v26.11.1
- **Auditor**: adversarial audit lane, 2026-11-01, repo `/Users/sac/cgap`
  (one canonical checkout, branch `main`, HEAD `c83619d`)
- **Subject audited**: the Wave-5 defect-repair state in the working tree;
  the wave receipt file the dispatch names
- **Verdict**: **PARTIAL** — every materially testable claim is ALIVE under
  independent adversarial re-run and all six named mutation classes flip
  their courts (zero vacuous courts found), but the wave receipt is absent
  from the repo and DoD 6/10 are unmet.

## 1. The receipt under audit does not exist on disk

- `docs/jira/v26.11.1/waves/` contains only `a11-wave4-audit-receipt.md`,
  `a17-wave2-audit-receipt.md`, `a20-wave3-audit-receipt.md`, and
  `W5-baselines-hardening-prd-ard.md`. No `a07-wave*` receipt exists
  anywhere in the repo (repo-wide `.md` sweep: zero hits).
- The lane's wave-5 evidence lives only in volatile `/tmp`:
  `a07-selfcheck.log` (03:34), `a07-courts.log` (03:32), `a07-scan.log`
  (03:34), `a07-scan-rustvocab.log` (03:35), `a07-depdir.log` (03:36),
  `a07-register.json` (03:36), `a07-flip/runner.py` (03:33), `a07-mut/`.
  All postdate the final owned-file edits (`zero_string_gate.py`
  03:32:12, `slab.rs` 03:31:48) — the evidence is fresh, not stale, but
  it is outside the repo and outside version control. DoD 5's own words
  apply: "The receipt is the evidence; no receipt, no DoD."
- Every claim below is re-proven from my own runs, not the lane's logs.

## 2. Baseline re-run (this session, canonical tree)

| Command | Exit | Result |
|---|---|---|
| `cargo test -p cgap-bosym --lib` | 0 | 10 passed / 0 failed |
| `zero_string_gate.py --self-check` | 0 | poison hits=2 fired=True; clean hits=0; mutated=none |
| `zero_string_gate.py --courts` | 0 | 12/12 PASS (`COURTS-SUMMARY total=12 failed=0`) |
| `zero_string_gate.py` (full scan) | 0 | `SUMMARY scanned=74 crates=22 hits=0` (gate 1.2.0) |
| `zero_string_gate.py --dep-direction` | 0 | `checked=72 crates=22 violations=0 src_refs=0` |
| `zero_string_gate.py --register` | 0 | register v1 JSON, 9 schema keys, 6 patterns |
| Rust emit court + `--vocab <rust archive>` | 0 | Rust-emitted `symbols.bosym` (521 B, 9 tokens) parsed by the Python gate; scan hits=0 |

- The wave-5 repair claim (SHA-256 edge vectors "derived from an
  independent implementation (python3 hashlib)") was re-verified: all 8
  edge pins (55/56/57/63/64/119/120/200) + the NIST 56-byte vector +
  `sha256("abc")` recomputed via `hashlib` in my session — every pin
  MATCHes. The pins are genuinely independent of the implementation.
- The Rust→Python cross-implementation witness is real: the emit court
  (`CGAP_BOSYM_EMIT=... cargo test ... emit_real_vocabulary_archive_
  when_gated`) produced the archive; `--vocab` mode loaded it
  (`VOCAB source=archive:...` tokens=9) and the full scan stayed hits=0.

## 3. Anti-vacuity mutation matrix (my independent /tmp sandboxes)

Sandbox A: `/tmp/a07adv-rs` — `cp -r` of `crates/cgap-bosym` under a
minimal root workspace; the copy was byte-exact before each shim
(sha256 `75b73a6d…` == canonical). Every restore sha256-verified
byte-exact. Sandbox B: `/tmp/a07adv-py/gate.py` — byte-exact copy of the
gate (sha256 `0fc4e34c…`), run with `--root /Users/sac/cgap`, fixtures in
temp dirs; canonical tree never mutated. Full-session canonical-tree
witness: `/tmp/a07adv_canonical_before.sha` == `..._after.sha`
(3 files identical).

| # | Mutation (named class) | Court attacked | Result (this session) |
|---|---|---|---|
| M-R1 | `SHA_K[0]` 0x428a2f98→0x99 (round constant) | `sha256_fips_vectors` (wave-5 repair court) | FAILED 9/1, cargo exit nonzero — FLIPS |
| M-R2 | `parse_archive` trailer check neutered | `t3_bit_flip_anywhere_is_refused` | FAILED 9/1 — FLIPS |
| M-R3 | framing minimum relaxed to empty-only | `t2_truncation_fails_closed_at_every_offset` | FAILED 9/1 — FLIPS |
| M-G1 | `errors="replace"`→`"strict"` (the wave-5 named mutation) | T15 + suite | courts exit 1, `GATE-REFUSED: internal-error` — FLIPS (mode note below) |
| M-G2 | `CLOUD_STRING_PATTERNS` emptied | T14a + T14b | both FAIL (`failed=2`), exit 1 — FLIPS |
| M-G3 | `find_vocab_token` neutered to never match | `--self-check` | `VACUOUS GATE: predicate does not fire on poisoned fixture`, RESULT FAIL, exit 1 — FLIPS |

- 6/6 named mutation classes flip; zero vacuous courts. The gate detects
  its own vacuity and exits nonzero itself (M-G3), exactly as the lane
  doc's anti-vacuity protocol requires.
- **M-G1 flip-mode note**: the T15 docstring implies the pre-fix gate
  prints an explicit `T15 … FAIL` verdict. The plain strict-decode revert
  instead raises `UnicodeDecodeError` inside `printable_runs`, unwinding
  to `GATE-REFUSED: internal-error` (suite aborts at exit 1 before T15's
  verdict line). The flip FACT holds (green 0 → exit 1), and the lane's
  own flip runner (`/tmp/a07-flip/runner.py` shim 1) uses a softer
  drop-run pre-fix variant that does yield the explicit FAIL verdict.
  Recorded as a divergence between docstring-implied and observed flip
  mode, not a vacuity defect.

## 4. Ownership check (read-only)

- `git diff --stat` is aggregate across all 20 lanes (43 files,
  +32794/−36); A07's claimed files are exactly `crates/cgap-bosym/src/
  slab.rs` (+1232) and `scripts/zero_string_gate.py` (+1793). No evidence
  of A07 edits outside its ownership; content is in-scope (codec, gate,
  courts, register).
- In-file disclosures: `slab.rs:767-773` records the PROVISIONAL sym_id
  divergence (A06 SlotId re-mint required; Cargo.toml outside lane
  ownership → std-only SHA-256 instead of a dependency). The A07 register
  (`--register`) discloses that `boundary_gate.py` still carries an inline
  cloud-pattern copy awaiting consumption wiring.
- Cross-lane staleness found (A20-owned, not an A07 defect):
  `scripts/boundary_gate.py:64-68,731-732` still states the A07 register
  "does not exist on disk this session (zero_string_gate.py is the A07
  Wave-1 fail-closed stub)" — stale now that `--register` exists and
  emits (exit 0, my run). A20's consumption wiring remains open.

## 5. DoD scorecard (lane doc DoD 1-10)

| DoD | Status | Evidence |
|---|---|---|
| 1 crate + generator (export → archive + projection) | ALIVE | build + 10/10 tests; emit court writes real archive |
| 2 format v0 + T1-T3 | ALIVE | T1/T2/T3 green; M-R2/M-R3 flip their courts |
| 3 gate implemented (sections, sweep, vocab, allowlist, fail-closed) | ALIVE | all baseline gates exit 0; M-G2/M-G3 flip |
| 4 `--self-check` green (poison HITs, clean does not, sha witness) | ALIVE | my run: fired=True, clean=0, mutated=none |
| 5 exit gate executed WITH receipt | PARTIAL | gate executed (hits=0, scanned=74, crates=22); receipt MISSING from repo |
| 6 execution crates compile from string-free projection | NOT MET | no workspace crate consumes `emit_projection` output (repo-wide grep: zero consumers) |
| 7 T1-T10 green, real cargo, no mocks, loud skips | ALIVE | 12/12 courts; T5's `.text` placement skip is loud + disclosed |
| 8 dependency direction proven | ALIVE | `--dep-direction` violations=0 src_refs=0; T13 flips on poison |
| 9 allowlist discipline (justified entries, T6/T7/T8) | ALIVE | T6/T7/T8 PASS; hostile-allowlist precedence pinned by T8 |
| 10 docs updated PROPOSED→landed; ADR-002 in register | NOT MET | lane doc still PROPOSED/UNVERIFIED; ADR-002 absent from `docs/jira/v26.10.31/22-adr-register.md` (register entry is lane-1's transition per the lane doc — open, not A07-defected) |

## open items (coordinator)

1. Land the A07 wave-5 receipt into `docs/jira/v26.11.1/waves/`; the
   lane's fresh `/tmp` logs are the raw material, or cite my re-runs.
2. DoD 6: name the consumer or downgrade to a recorded deferral — no
   execution crate currently compiles from the projection.
3. DoD 10: update the lane doc standing; lane-1 to enter ADR-002 into the
   register (or record the deferral).
4. A20: consume `zero_string_gate.py --register` in `boundary_gate.py`
   and retire the stale `a07_register_deferral` text (its "stub" claim is
   now false).
5. Stray untracked `atomic_commit.rs` at repo root (cgap-storage family,
   not A07; also flagged by the a11-wave4 audit).

## Divergences (doc yields; recorded)

- Lane doc line 237 sets the failure predicate to "ontology vocabulary
  hits only"; the implemented predicate matches, with the added
  `execution-crate-no-artifacts` / `crate-no-artifacts` / `no-artifacts-
  scanned` structural refusals — a strengthening, recorded not silently
  added.
- Lane doc "big-endian convention" for the trailing digest: slab.rs
  carries the digest as raw SHA-256 bytes and records the reading
  explicitly (`slab.rs:15-17`). Python reader mirrors it. Recorded.
- T15 flip mode (section 3 M-G1 note): internal-error abort vs the
  docstring-implied explicit FAIL verdict.
- Lane doc scope names `wasm-footprint` in the execution family; the
  `.wasm` data-section scan remains out of v1 scope (lane doc lines
  412-415 sanction this; loud skip, disclosed).
- The gate's effective ontology vocabulary is the 9 `fibo:` tokens
  derivable from `ontologies/*/src/lib.rs` (graph-derivation path); the
  `.bosym`-archive path is implemented and witnessed but no archive sits
  at the default `gen/bosym/symbols.bosym` location, so production scans
  run graph-derived. Scope-consistent with the lane doc's resolution
  order; recorded for the coordinator.
