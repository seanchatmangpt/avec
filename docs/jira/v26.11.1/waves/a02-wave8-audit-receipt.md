# A02 Wave 8 Audit Receipt

- **Lane**: A02 (crates/cgap-parquet: `mmap_writer.rs` + `footer.rs`),
  v26.11.1 overnight campaign, wave-8 state
- **Auditor**: adversarial audit lane, 2026-11-01, repo `/Users/sac/cgap`,
  branch `main`, one canonical checkout, no worktrees, no git writes
- **Subject**: lane A02's wave receipt — which does NOT exist on disk
  (self-acknowledged at `crates/cgap-parquet/src/footer.rs:1009`: "no
  wave-7 receipt for this lane exists on disk"). DoD 10 (lane doc §10) is
  unmet as written; this audit therefore attacked the lane's code and its
  in-code claims directly.
- **Verdict**: **PARTIAL** — code surface ALIVE (every re-run command green
  this session; 11 of 12 mutation attacks flip their named courts), but
  (1) the lane receipt DoD 10 demands is missing, and (2) one court is
  VACUOUS under its named mutation class: `seal_summary_mismatch_refused`
  (`crates/cgap-parquet/src/mmap_writer.rs:1166`) survives neutering of the
  seal-boundary span-SUM guard it names. Under the dispatch's strict rule
  ("a green court that survives its mutation = VACUOUS, fail the lane"),
  the wave-8 state fails that single clause; W9 repair is required for
  ALIVE.
- **Repo integrity**: sha256 pre == post, byte-exact:
  `footer.rs e5b699d3a8a8e4f3…`, `mmap_writer.rs 8836897a8c2cb0e8…`;
  all mutations applied only in `/tmp/a02-adv-14470` (sandbox retained
  with per-mutation logs at `results/*.log` for replay; coordinator may
  delete).

## Baseline re-runs (canonical checkout, this session)

| Command | Exit | Result |
|---|---|---|
| `cargo test -p cgap-parquet` | 0 | 176 lib + 33 integration, 0 failed |
| `cargo test -p cgap-parquet --features std --lib` | 0 | 185 passed / 0 failed |
| c5 gate `--nocapture`, debug | 0 | TARGET asserted in release only (verbatim below) |
| c5 gate, `--release --features std` | 0 | TARGET < 2000us: MET (verbatim below) |
| sandbox baseline `--lib` (`/tmp/a02-adv-14470`) | 0 | 176/176, matches canonical |

Verbatim c5 output:

```
C5 GATE [debug profile — TARGET asserted in release only]:
  min=4286.2us median=4378.9us body=200068B
  encoder_baseline=3351.8us ratio=1.3x
C5 GATE (TARGET < 2000us): min=668.0us median=770.3us
  body=200068B encoder_baseline=258.2us ratio=2.6x
```

C5 gate: the < 2 ms TARGET is MET in release (668.0us min, min-of-N,
sleep-free, ratio fence vs a sleep-free encoder baseline). DoD 8's 100k-row
end-to-end seal + read-back verify is inside the c5 test and green, plus
`tier_h_file_sink_round_trip_is_byte_exact` green.

## Court attack (all mutations in /tmp/a02-adv-14470 only)

Method: `cp -r` of cgap-parquet + cgap-arrow into the sandbox with a
standalone workspace manifest; baseline 176/176; one mutation at a time
(python driver enforces exactly-one occurrence; ambiguous targets refuse
loudly); `cargo test -p cgap-parquet --lib` after each; restore from repo
copies; `cmp` byte-exact + sha256 after every restore.

| Mutation | Guard neutered (file) | Outcome |
|---|---|---|
| MUT-FOOT-1 | checksum (`footer.rs:460`) | **3 FLIP**: c1 (named), c2, hostile; 101 |
| MUT-FOOT-2 | localization (`footer.rs:461`) | **1 FLIP**: c2 alone, attributable; 101 |
| MUT-FOOT-3 | attest gate (`footer.rs:444`) | **1 FLIP**: c3 alone; 101 |
| MUT-FOOT-4 | heap inject (`write_slab`) | **1 FLIP**: c4 alone; 101 |
| MUT-FOOT-6 | width refusal (`mmap_writer.rs`) | **1 FLIP**: c6 alone; 101 |
| MUT-FOOT-8 | flags fail-closed (`footer.rs`) | **3 FLIP**: hostile, meta_version, c3 |
| MUT-SHAPE-1 | magic (`footer.rs:419`) | **2 FLIP**: magic court, hostile; 101 |
| MUT-FOOT-9 | `flush_to` revert (`footer.rs:350`) | **1 FLIP**: w8_f1 alone; 101 |
| MUT-AUD-W7F1 | seal span bound (`footer.rs:184`) | **1 FLIP**: w7_f1 alone; 101 |
| MUT-AUD-SUMMARY | seal span-SUM (`footer.rs:197`) | **NO FLIP** (176/176). **VACUOUS** |
| MUT-AUD-DIGEST | digest check (`footer.rs:470`) | **2 FLIP**: wrong_digest, hostile; 101 |
| (driver) | ambiguous target | loud `APPLY-FAIL`, retried disambiguated — skip discipline held |

## VACUOUS COURT finding (the audit's one court-attack failure)

`seal_summary_mismatch_refused` (`mmap_writer.rs:1166-1186`) comments
"spans do not sum to body_len is refused at seal", but its fixture
(`spans: &[3, 9]`, `row_groups: 1`) trips the span-COUNT check
(`footer.rs:177`, len 2 != 1) BEFORE the span-SUM check (`footer.rs:197`)
is ever reached. Neutering only the sum check flips nothing (176/176
green). Consequence: `FooterSealer::new` can emit a sealed block its own
`verify_seal` refuses (SpanMismatch) — the W6-F1 seal/verify asymmetry
class the lane itself repaired at two other bounds (zero-body, i32::MAX
span) — at a third bound, uncourted. Not silent corruption (verify_seal
still refuses the artifact), but the seal-boundary guard is unenforced
and the court's name overclaims. The lane's own driver
(`/tmp/a02-court/mutate.py`, pre-existing) never attacked this guard.
Suggested W9 repair (NOT applied — audit does not fix): fixture with
`row_groups: 2, spans: &[3, 5]` and sum perturbed, or a dedicated
sum-branch court + `# KD-COVERS` row.

## Divergences (doc yields unless noted; recorded, not fixed)

1. **D-A02-1 (receipt missing — DoD 10 unmet)**: no lane receipt file on
   disk; in-code claims point at it (`footer.rs:51,1009`,
   `mmap_writer.rs:47,100,1200`). Measured C5 numbers now live in THIS
   audit receipt. Doc yields to reality; the missing artifact is the
   audit's headline defect.
2. **D-A02-2 (C4 counting form BLOCKED, disclosed)**: single
   `#[global_allocator]` in the lib-test binary is owned by rle.rs's
   in-lib court (`rle.rs:382`, verified on disk); C4 ships a structural
   text-scan witness with an honest epistemic label
   (`mmap_writer.rs:987-993`) and DID flip under a real source mutation
   (MUT-FOOT-4). Doc yields; the blocker is real and cited.
3. **D-A02-3 (C3 doc pair unsatisfiable)**: doc §6.3's "checksum passes,
   attestation fails" on one artifact cannot hold because the §6.1
   checksum spans the attestation slot; implemented as verdict order
   (shape -> attestation -> checksum -> digest) + gate-flag-clearing
   counterfactual (`mmap_writer.rs:899-939`). Doc yields; disclosed
   in-code.
4. **D-A02-4 (`CapacityExceeded` descoped)**: remapped onto
   `RunOverrun` with documented claim/requirement semantics because
   `rle.rs` is outside lane ownership. Disclosed in module docs; doc
   yields (ADR §3 sanctioned the remap option).
5. **D-A02-5 (C7 better than doc)**: implemented as a REAL external
   process (python3 subprocess with an independent compact-protocol
   parser, unique-per-run temp files, loud `[SKIP]` if python3 absent) —
   ran green this session; doc's pyarrow/arrow-rs form was optional
   Tier H.
6. **D-A02-6 (Cargo.lock +80)**: coordinator-scaffold mechanical churn
   (all 20 lane crates' lock entries), not A02-authored; A02's crate
   `Cargo.toml` is untouched (DoD 1's zero-new-deps holds: `[dependencies]`
   empty and unchanged).

## Ownership check

`git diff --stat -- crates/cgap-parquet/` (read-only): exactly the 2
owned files (`footer.rs` +1415, `mmap_writer.rs` +1318). `lib.rs` and
crate `Cargo.toml` untouched; root `Cargo.toml` untouched. Lane build
root `target-a02/` present (sanctioned lane lease). No worktrees, no
clones, no git write commands.

## Unattacked this session (no verdict implied)

zero_body_seal, every_truncation_fails_closed, meta_field5 round-trip,
c6's untouched-buffer arm, tier_h round-trip — green in baseline but not
mutation-attacked; residual UNVERIFIED as to flip-capability.

## Commands (exact, re-runnable)

```
cd /Users/sac/cgap && cargo test -p cgap-parquet                # exit 0
cargo test -p cgap-parquet --features std --lib                 # exit 0, 185/185
cargo test -p cgap-parquet --release --features std --lib c5_gate -- --nocapture
                                                                # min=668.0us < 2000us
git diff --stat -- crates/cgap-parquet/                         # 2 files, ownership
# sandbox: /tmp/a02-adv-14470 (cp -r cgap-parquet+cgap-arrow, own manifest;
# baseline 176/176; 12 mutations one at a time; cmp+sha256 restore after each;
# logs: /tmp/a02-adv-14470/results/*.log)
```

## See Also

- `docs/jira/v26.11.1/lanes/A02-parquet-mmap.md` (normative lane doc)
- Sibling audits: `docs/jira/v26.11.1/waves/a17-wave2-audit-receipt.md`
- `crates/cgap-parquet/src/footer.rs`, `crates/cgap-parquet/src/mmap_writer.rs`
