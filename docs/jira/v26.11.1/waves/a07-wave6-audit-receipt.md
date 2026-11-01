# A07 Wave 6 Audit Receipt

- **Lane**: A07-bosym-slab (`crates/cgap-bosym/src/slab.rs`,
  `scripts/zero_string_gate.py`), Wave 6 (defect repair driven by the
  wave-5 court receipts), v26.11.1
- **Auditor**: adversarial audit lane, 2026-11-01, repo `/Users/sac/cgap`
  (one canonical checkout, branch `main`, HEAD `c83619d`), no git writes
- **Subject audited**: the Wave-6 defect-repair state in the working tree
  (gate mtime 04:42, post-dating the wave-5 audit receipt 04:13), plus
  the wave-5 receipt's open-items ledger
- **Verdict**: **PARTIAL** — the wave-6 repair is real and verified (all
  six named mutation classes flip their courts, zero vacuous courts; the
  M-G1 flip-mode divergence from wave 5 is fixed), and the A20 register
  seam is closed end-to-end; but DoD 6/10 remain unmet as in wave 5, and
  the milestone exit gate (DoD 5) is RED on the current tree via a
  cross-lane artifact (`emit-fibo-pack`, hits=2, scan exit 1).

## 1. Wave-5 open-items ledger (re-verified this session)

| Open item (wave 5) | Status now | Evidence (this session) |
|---|---|---|
| 1. Land the wave-5 receipt | CLOSED | `waves/a07-wave5-audit-receipt.md` on disk (read this session) |
| 2. DoD 6: name a projection consumer or record deferral | OPEN | repo-wide `emit_projection` consumer grep: zero hits outside cgap-bosym |
| 3. DoD 10: lane-doc standing + ADR-002 register entry | OPEN | lane doc lines 4-5/449 still "PROPOSED / UNVERIFIED"; `grep ADR-002 docs/jira/v26.10.31/22-adr-register.md`: zero hits |
| 4. A20: consume `--register` in boundary_gate, retire stale text | CLOSED | `boundary_gate.py:609-660` fail-closed loader; live run exit 0 |
| 5. Stray root `atomic_commit.rs` | OPEN | still present (13,333 B, Nov 1 00:17, untracked) |

Open item 4 witness: `python3 scripts/boundary_gate.py --strings` exit 0
printed `A07 register v1 consumed (sha 713898f41e14, 6 canonical
patterns, 0 allowlist entries)` — the §6.4 single-list law is wired, not
just claimed.

## 2. Baseline re-run (this session, canonical tree, true exit codes)

| Command | Exit | Result |
|---|---|---|
| `cargo test -p cgap-bosym --lib` | 0 | 10 passed / 0 failed |
| `zero_string_gate.py --self-check` | 0 | poison hits=2 fired=True; clean hits=0; scan-mutation-witness mutated=none |
| `zero_string_gate.py --courts` | 0 | 12/12 PASS (`COURTS-SUMMARY total=12 failed=0`) |
| `zero_string_gate.py` (full scan) | **1** | `SUMMARY scanned=41 crates=22 hits=2` — see §5 |
| `zero_string_gate.py --dep-direction` | 0 | `checked=40 crates=22 violations=0 src_refs=0` |
| `zero_string_gate.py --register` | 0 | register v1: 9 top keys, 6 patterns, consumers row names `boundary_gate.py --deps/--strings` |
| `CGAP_BOSYM_EMIT=... cargo test ... emit_real_vocabulary_archive_when_gated` | 0 | real 521 B `symbols.bosym` (sha256 `80ef7c2c…`) |
| `zero_string_gate.py --vocab <Rust archive>` (full scan) | 1 | `VOCAB source=archive:…` tokens=9; hits=2 (same 2); Rust→Python leg witnessed |
| `--vocab /nonexistent` (accidental T9 probe) | 1 | `GATE-REFUSED: vocab-missing` — typed fail-closed refusal |

SHA-256 edge re-verification: `sha256("abc")`, empty vector, and the
NIST 56-byte vector recomputed via `hashlib` — all match; round
constants `0x428a2f98`/`0x71374491` present at `slab.rs:218`.

Hygiene: zero `Process.sleep`/`time.sleep`; zero `unittest.mock`/`Mock()`
in both owned files; 20 `ALLOWLIST-JUSTIFICATION` sites in the gate.

## 3. Anti-vacuity mutation matrix (independent /tmp sandboxes)

Sandbox A `/tmp/a07w6-rs`: `cp -r` of `crates/cgap-bosym` + minimal
workspace root; copy byte-exact before each shim (sha256 `75b73a6d…` ==
canonical). Sandbox B `/tmp/a07w6-py/gate.py`: byte-exact gate copy
(sha256 `0cd96b35…`), run with `--root /Users/sac/cgap`; sandbox-B
baseline before any mutation: `--courts` 12/12 PASS exit 0 (and one
aborted mutation attempt proved non-mutation leaves courts green).
Canonical-tree witness `/tmp/a07w6-canonical-before.sha` ==
`..._after.sha` — the canonical owned files are byte-identical before
and after the whole mutation session. Every restore sha256-verified.

| # | Mutation (named class) | Court attacked | Result (this session) |
|---|---|---|---|
| M-R1 | `SHA_K[0]` `0x428a2f98`→`0x99` | `sha256_fips_vectors` | FAILED 9 passed/1 failed, panicked at slab.rs:898 — **FLIPS** |
| M-R2 | parse_archive trailer check neutered (`if false && …`) | `t3_bit_flip_anywhere_is_refused` | FAILED 0/1 — **FLIPS** |
| M-R3 | framing minimum relaxed to `bytes.is_empty()` | `t2_truncation_fails_closed_at_every_offset` | FAILED 0/1 — **FLIPS** |
| M-G1 | `errors="replace"`→`"strict"` (5 sites) | T15 + suite | `COURT T15/T5/T4 … FAIL — court raised UnicodeDecodeError`, `COURTS-SUMMARY total=12 failed=3`, exit 1 — **FLIPS** |
| M-G2 | `CLOUD_STRING_PATTERNS` emptied | T14a + T14b | both FAIL; T14b prints `VACUOUS GATE: register cloud predicate did not fire on planted s3 endpoint`; `failed=2`, exit 1 — **FLIPS** |
| M-G3 | `find_vocab_token` neutered to `return None` | `--self-check` | `VACUOUS GATE: predicate does not fire on poisoned fixture`, RESULT FAIL, exit 1 — **FLIPS** |

6/6 named mutation classes flip; zero vacuous courts. The gate detects
its own vacuity and exits nonzero itself (M-G3, M-G2/T14b).

**Wave-6 repair verification (M-G1 flip mode)**: wave 5 recorded the
strict-decode revert aborting the whole suite via
`GATE-REFUSED: internal-error` before any verdict line. The wave-6
repair (`cmd_courts` `run_court` wrapper, `zero_string_gate.py:1712-1726`,
comment names the wave-5 note) exception-isolates every court. My M-G1
re-run observed the repaired mode: three explicit `COURT … FAIL` rows
naming the exception, COURTS-SUMMARY still printed, exit 1 — matching
T15's docstring. The isolation does not mask failures (exit stays 1,
FAIL rows print). Repair ALIVE under its named mutation.

## 4. Ownership check (read-only)

- `git diff --stat` (aggregate 43 files, +34091/−36): A07's owned files
  appear exactly as `crates/cgap-bosym/src/slab.rs` (+1232) and
  `scripts/zero_string_gate.py` (+1832; was +1793 at wave 5 — the +39
  are the `run_court` repair, disclosed in its own comment block). No
  A07-scope content outside lane ownership. Post-wave-5 growth in the
  aggregate diff belongs to other lanes (`boundary_gate.py` +1265,
  `ontologies/fibo/src/lib.rs` +292).
- No sleeps, no mocks, no inline pattern copy outside the canonical
  register (T14a pins doc-list parity; M-G2 proved the pin fires).

## 5. NEW FINDING — exit gate RED on the current tree (cross-lane)

- `python3 scripts/zero_string_gate.py` now exits **1** with
  `SUMMARY scanned=41 crates=22 hits=2`: `target/debug/emit-fibo-pack`
  carries 2 ontology HITs in `__TEXT,__const` (offset 587472: a
  contiguous `fibo:*` vocabulary run; offset 587992: `--inject` +
  `fibo:SecDbt:Bond`).
- The binary was built 05:06:52 — after the wave-5 audit (04:13) and the
  wave-6 gate edit (04:42) — from `ontologies/fibo/src/lib.rs` (+291,
  A09-owned). Wave 5's baseline scanned 74 artifacts at hits=0; the
  regression is the new artifact, not a gate change.
- Classification: **the gate is correctly firing on a real quarantine
  violation**, i.e. live proof the predicate works on real artifacts —
  while simultaneously breaking DoD 5's "hits=0" exit-gate wording.
- Coordinator decision needed: `emit-fibo-pack` is an ontology *emitter*
  (build tooling, the archive producer role ADR-002 assigns to
  cgap-bosym). Either (a) the gate gains a typed emitter/tooling
  exclusion class (currently the only exemption is cgap-bosym's
  dep-direction one; the scan mode has no exclusion list), or (b) A09
  routes the emitter's strings through the .bosym quarantine. Neither is
  an A07 file defect; the scan-mode exemption gap is A07's to design
  (gate v2 class), disclosed here rather than silently tolerated.
- Related coverage divergence (A20 side): `boundary_gate.py --strings`
  reports "0 hits across 15 covered crates" on the same tree the A07
  scan fails on — A20's covered-crate set does not include
  `emit-fibo-pack`. Two gates, two coverage sets, different verdicts on
  the same artifact; recorded for the coordinator.

## 6. DoD scorecard (lane doc DoD 1-10, wave-6 state)

| DoD | Status | Evidence |
|---|---|---|
| 1 crate + generator | ALIVE | 10/10 tests; emit court wrote a real archive this session |
| 2 format v0 + T1-T3 | ALIVE | T1/T2/T3 green; M-R1/R2/R3 flip their courts |
| 3 gate implemented | ALIVE | baseline modes green; M-G1/G2/G3 flip |
| 4 `--self-check` green | ALIVE | fired=True / clean=0 / mutated=none, exit 0; M-G3 flips to VACUOUS GATE |
| 5 exit gate executed WITH receipt | **PARTIAL** | gate executed with true exit codes, but summary is hits=2 exit 1 on the current tree (§5); actual lines in §2 |
| 6 execution crates compile from the projection | NOT MET | zero consumers (re-confirmed by grep this session) |
| 7 T1-T10 green, real cargo, no mocks, loud skips | ALIVE | 12/12 courts exit 0; T5 `.text` skip loud; no sleeps/mocks |
| 8 dependency direction proven | ALIVE | `--dep-direction` violations=0 exit 0; T13 green |
| 9 allowlist discipline | ALIVE | T6/T7/T8 PASS; T14a pins justification + doc parity |
| 10 docs standing + ADR-002 in register | NOT MET | unchanged from wave 5 (§1 item 3) |

## Open items (coordinator)

1. §5: classify `emit-fibo-pack` — typed scan-mode emitter exclusion
   (A07 gate v2) or A09 quarantine routing. Until then the milestone
   exit gate is RED on every full scan.
2. DoD 6: consumer or recorded deferral (open since wave 5).
3. DoD 10: lane-doc standing update; ADR-002 register entry by the
   register owner (open since wave 5).
4. A20: reconcile the two gates' coverage sets (A20 `--strings` 0 hits
   vs A07 scan hits=2 on the same tree).
5. Stray root `atomic_commit.rs` (open since wave 5; also flagged by
   a11-wave4).

## Divergences (doc yields; recorded)

- Lane doc line 237 ("ontology vocabulary hits only") — implemented
  predicate matches, plus the structural refusals; unchanged from
  wave 5, still a disclosed strengthening.
- Big-endian trailer convention — unchanged (raw SHA-256 bytes,
  `slab.rs:15-17` records the reading).
- Wave-5 M-G1 flip-mode divergence — **resolved** by the wave-6 repair
  (explicit per-court FAIL rows now match T15's docstring; §3).
- The gate's effective vocabulary remains the 9 graph-derived `fibo:`
  tokens (no archive at the default `gen/bosym/symbols.bosym` path);
  the archive path is implemented and witnessed (§2 emit + `--vocab`
  runs). Unchanged from wave 5.
- Wave-6 edit is confined to the gate's court harness and docstrings;
  no scan-predicate change — the hits=2 outcome of §5 is therefore
  purely the new cross-lane artifact, not a repair regression.
