# Risk Register & Open Adversarial Questions: v26.10.31

Status: UNVERIFIED. This register consolidates risks and open questions from the
v26.10.31 design corpus; every entry carries a named close condition. No entry is
a measured result; each inherits the standing of its owner-lane document.

## 1. Engineering DFMEA

The four charter modes are defined with S/O/D/RPN and controls in
[18-dflss-charter.md](18-dflss-charter.md) §6; they are linked here, not duplicated:
LLVM de-optimization jump injection; cache-line splits; `.idx` large-offset MSB
overflow; SIMD register spill. This section extends the table with new items,
same RPN discipline (RPN ≥ 48 requires an engineered poka-yoke, not review).

### New DFMEA items

| ID | Failure mode | S | O | D | RPN | Control (proposed) |
|---|---|---|---|---|---|---|
| R-01 | Ladder depth assumption breaks beyond 65,536 objects in one first-byte partition — silent wrong lookups, no error raised | 9 | 4 | 5 | 180 | Partition-size guard at pack open + re-derive depth K = ceil(log2(max partition)); until closed, packs MUST stay ≤ 2^16 objects (open item L6-1, [25-git-packfile-integration-spec.md](25-git-packfile-integration-spec.md) §3) |
| R-02 | Packfile custom chunks (non-standard type codes, sidecar bytes in pack body) break unmodified-git verification — FR-04 compat violation | 9 | 3 | 4 | 108 | Blob-only placement rule (type 3, self-contained) + `git fsck --full` / `git verify-pack -v` acceptance gate, executed not asserted (25 §2, §5) |
| R-03 | BEAM resource lifetime bug: use-after-free across the NIF boundary — binary aliases avec memory while avec frees or destructor/ownership mismatch | 8 | 4 | 4 | 128 | Exact-once free law (destructor = no-op iff host-owned, free iff avec-owned) + AVEC-105 turnstile gating frees to exact-once at state 0b00 ([28-beam-nif-handoff.md](28-beam-nif-handoff.md)) |
| R-04 | Gage R&R failure on CNTVCT_EL0 / RDTSC timers — every throughput and latency claim downstream is invalidated because the measurement system itself is not capable | 8 | 5 | 3 | 120 | Measure-phase gate before any claim is admitted: Gage R&R with under 1% variance on the timer infrastructure ([18-dflss-charter.md](18-dflss-charter.md) §5) |
| R-05 | SHA256-vs-BLAKE3 dual attestation mismatch: git OIDs are SHA-1/SHA-256 keyed, receipts are BLAKE3 — an unmapped identity pair silently splits the audit trail | 7 | 5 | 4 | 140 | Explicit mapping law: every chunk records both `git hash-object` OID and `b3sum` root, cross-checked at the FR-04 gate (25 §5); mapping declared in tree structure, never inferred |

R-01 is the highest raw RPN because detection is worst-case: a wrong lookup
returns data, not an error. R-03 severity is capped by the BEAM refcount
safety net, but the aliased-memory design means a single destructor/
ownership mismatch corrupts the VM heap, not just one term.

## 2. Security / Adversarial Open Questions

From the notary, blinded-WASM, and microarchitectural-probe lanes. All are open
adversarial questions, not findings.

### Relay attacks inside epsilon on NTP-class clocks

[12-distance-bounding-rtt.md](12-distance-bounding-rtt.md) states its own limit:
cross-network RTT envelopes require hardware timestamping (PTP or NIC-level).
On host-software clocks (NTP-class), jitter exceeds the flight-time budget, so a
relay adversary operates inside the clock's epsilon — the deterministic
`REFUSAL-04` bound degrades to classical distance-bounding limits. Open: does
the scheme retain any relay resistance at all on software clocks, or must the
sieve court refuse cross-host frames outright on non-PTP deployments?

Close condition: a measured epsilon for NTP-class synchronization, and a
typed refusal class for frames whose flight-time budget is below clock
resolution.

### Emulated hardware defeating cycle traps

[15-arrow-microarch-probe.md](15-arrow-microarch-probe.md) flags this itself:
constant-time-faking emulators, nested-virtualization hardware illusion, a
faithful hardware simulator, or a co-resident machine forwarded at wire speed
may reproduce bare-metal cycle-exact latency. The physical time budget is the
design's only answer, and the budget is attackable in principle.

Close condition: a quantified cost curve — emulating the full probe stack
(stride traps + SIMD width + side-channel assays + RTT budget) versus running
honest bare metal — showing emulation strictly dominates honest cost.

### Host-side counter-strategies to the covert probe

The probe's leverage is that the host cannot distinguish verification work
from payload work. Host-side strategies short of full emulation remain open:
statistical profiling of the kernel across many nonces, selective degradation
(SIMD off for some turns), or replaying cached fold results inside the window.

Close condition: an adversarial court in which a scripted host adversary
deploying these strategies fails the folded receipt
(`R_receipt = BLAKE3(Delta_S || N || Cycles || StrideChecksum)`) on every
strategy class.

### The probe asymmetry claim is unproven — falsifiers

The claim in 15 §Security Framing is an asymmetry claim, not a proof. It is
retired only by failing these falsifiers:

1. A host inspects the kernel statically (it is ~24KB, stripped but local) and
   identifies the stride/SIMD/assay trap layout without breaking the RTT
   window — the "cannot pause to inspect" premise fails.
2. A host reproduces bare-metal cycle bands under full emulation at material
   cost below honest bare metal — the time-budget premise fails.
3. The ~1,150-cycle band, ~1.4 microsecond budget, and 300-1000% spike range
   are measured and do not separate honest hosts from the adversary class —
   the illustrative constants carry no discriminative bits.

## 3. Doctrine / Falsifier Register (AHDCLT and Prior-Art Lanes)

From [16-ahdclt.md](16-ahdclt.md) Criticism Discipline; restated as register
entries so each has a close condition.

| ID | Risk | Falsifier / close condition |
|---|---|---|
| D-01 | Cheap LLM retrieval defeats anti-LLM cognitive tripwires: chunked map-reduce over context-exhaustion structures, or iterative tool-assisted probing, collapses the tripwire pillar | An agent run defeats the tripwires with cheap retrieval/tooling and the mutation lands — pillar is decoration; retire the claim from all downstream docs |
| D-02 | Statutory-vice liability inversion: courts read the compute-op-to-statute mapping as the author's constructive knowledge of regulated use, so the mapping creates liability for the author, not the attacker | A legal review or adverse ruling reads the mapping as admission; invert control to severable annotation or drop pillar 1 from the doctrine |
| D-03 | Verification cost scales with N dimensions: the O(1) turnstile becomes a linear scan over the constraint matrix, so the asymmetry claim reduces to ordinary hardening | Measured turnstile cost flat in N over the shipped constraint matrix; if it scales, the doctrine is reclassified as hardening, not asymmetry |

Until adversarially tested, AHDCLT is a design lens, not a security property.

## 4. Regulatory Risks

| ID | Risk | Close condition |
|---|---|---|
| G-01 | EU AI Act conformance claims (Articles 12, 14, 62 per [20-prd.md](20-prd.md) and [17-post-2028-computing.md](17-post-2028-computing.md)) are self-assessed; no external audit path exists, so a conformance claim made to a customer is currently unsupported marketing | A named external conformity-assessment path (notified body or accredited auditor) executes an audit against the receipt substrate and the claim survives |
| G-02 | P28-SIL enforceability is an accounting-mechanics thesis (FASB ASC 450 accrual + SOX 302/404 material-weakness per [18-dflss-charter.md](18-dflss-charter.md) §8); untested in dispute, and D-02 inversion applies to it directly | A structured legal analysis (or actual settlement) validates the accrual/enforcement mechanics; until then the $50M safe-harbor mechanics are design intent |

Regulatory entries cannot be closed by engineering courts alone; both require
external-party participation, which is itself the blocker to record.

## 5. Triage Table

Severity / detection ratings are relative to the corpus; owner-lane doc is the
authoritative source; every row names the observation that retires it.

| ID | Item | Sev | Detection | Owner doc | Close condition |
|---|---|---|---|---|---|
| R-01 | Ladder depth beyond 2^16 partition | High | Silent wrong lookup; guard absent today | 25 §3, 18-m1 | L6-1 closed: K re-derivation rule or pack split; guard shipped |
| R-02 | Packfile compat violation | High | `git fsck` fails loudly at gate | 25 §2 | FR-04 gate green on a real avec-generated repo |
| R-03 | NIF use-after-free | High | Heap corruption, hard to attribute | 28 | Exact-once free law enforced by AVEC-105 turnstile + adversarial lifetime court |
| R-04 | Gage R&R failure | High | Claims invalidated downstream | 18 §5 | Timer Gage R&R under 1% variance, measured |
| R-05 | Dual attestation mismatch | Med | Divergent OIDs cross-check fails | 25 §5 | Both identities match per chunk at FR-04 gate |
| A-01 | Relay inside clock epsilon | High | None on software clocks | 12 | Measured epsilon + typed refusal for sub-resolution frames |
| A-02 | Emulation defeats cycle traps | High | None by design | 15 | Emulation-vs-honest cost curve quantified, honest dominates |
| A-03 | Host counters to covert probe | Med | None | 15 | Scripted adversary fails folded receipt on all strategy classes |
| A-04 | Asymmetry claim unproven | Med | N/A (claim-level) | 15 | All three falsifiers (§2) run and fail |
| D-01 | Tripwires defeated by cheap retrieval | High | None | 16 | Adversarial agent run fails to land the mutation |
| D-02 | Statutory vice inverts liability | High | None until litigation | 16 | Legal analysis validates attacker-side exposure |
| D-03 | Verification scales with N | Med | Benchmark would show it | 16 | Turnstile cost measured flat in N |
| G-01 | EU AI Act self-assessment | High | None until customer audit | 20, 17 | External conformity assessment survives |
| G-02 | P28-SIL dispute-untested | High | None until dispute | 18 §8 | Legal analysis validates accrual mechanics |

Highest-priority closure order by (severity x no-detection): R-01, R-03,
A-01, A-02, G-01, G-02 — the modes that fail silently or only under external
adversarial/legal contact.

## Standing

UNVERIFIED throughout. Every entry inherits the standing of its owner-lane
document; this register adds no measurements. Close conditions are named
observations, none of which has been executed in this wave.

## 6. Wave-7 retire/live updates (2026-10-31)

- R-02-retired-class: the rle panic is FIXED — rle_court 8/8
  (crates/avec-parquet/tests/rle_court.rs). The zero_alloc parallel fix is
  real (receipt gate-blake3-zero-alloc ALIVE). Wave-9 correction: the
  zero_alloc court is now PROVEN — positive control landed
  (positive_control_counter_is_live in crates/avec-blake3/tests/
  zero_alloc.rs, 5/5); the 52-courts-audit.md addendum flips
  COURT-UNPROVEN 3 → 0. The wave-7 "UNPROVEN — PARTIAL, not closed"
  reading is superseded.
- R-03 homology-adjacent (wave-9): the cfg_homology mutant court is
  FLIPPED — gate-cfg-homology-v2 ALIVE (exit 0, `--mutant` mode,
  "MUTANT COURT FLIPPED: clean PASS && branchy FAIL"); the gate is a
  witnessed live signal, not vacuous. The normal-mode verdict stays
  FAILED on transpose (+17 vs slack 4), the registered ADR-14 item.
- FR-07 (wave-9): the five-op surface LANDED — ops_scan.rs (open/scan)
  + ops_persist.rs (flush/seal/receipt_query) registered in
  crates/avec-beam/src/lib.rs:570-608; cargo test -p avec-beam exit 0
  incl. the real-BEAM bridge contract (56 §Resolution).
- R-04 Gage R&R: gate-gage-rr ALIVE (harness proven, cv 0.87%); but the
  perf-guard SKIP-NOISY first-mint history means the quiet-machine protocol
  still governs — keep R-04 live with that nuance.
- Standing-line correction: the FR-04 close condition HAS been executed —
  fsck court 2/2 on a real avec-generated repo (gate-git-tests-v2 ALIVE);
  "none executed in this wave" applies to the wave this register was
  written in, not the current frontier.

## See Also

- [18-dflss-charter.md](18-dflss-charter.md) — charter DFMEA §6, Gage R&R gate
  §5, P28-SIL addendum §8
- [16-ahdclt.md](16-ahdclt.md) — doctrine pillars and falsifiers
- [15-arrow-microarch-probe.md](15-arrow-microarch-probe.md) — probe classes,
  asymmetry claim, open adversarial questions
