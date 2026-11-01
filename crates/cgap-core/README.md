# cgap-core

Zero-dependency, `no_std`-capable byte-layout kernel: DMA ring (`dma_ring.rs`),
isomorphism harness (`isomorphism.rs`), and the canonical 128-byte wire header
(`frame.rs`).

- Spec: `docs/jira/v26.11.1/lanes/A04-triumvirate-isomorphism.md` (§6 is the
  normative `CgapFrameHeader` layout referenced byte-for-byte by lanes A14,
  A17, A19).
- License instrument: P28-SIL v2.1.0 —
  `urn:avec:license:p28-sil:v2.1.0-sovereign`
  (`/Users/sac/cgap/LICENSE-P28-SIL.md`; URN frozen per
  `docs/jira/v26.11.1/03-rename-receipt.md` decision D2).
  No SPDX identifiers.
- Wave 1 (ABI freeze): struct/const definitions + static size/align/offset
  asserts only, zero logic. Validation, digest verification, ring methods,
  and the identity court are later waves on this frozen ABI.
- Wave 2 (core engines, LANDED): `CgapFrameHeader`/`P28LicenseAttestation`
  LE encode/decode + structural `validate()` (A17 §2.2 attestation rules),
  `verify_payload` (injected digest fn — zero-dep kernel; the in-tree
  `cgap-blake3` adapter supplies real BLAKE3 in tests), the §7.3 DmaRing
  method surface, `identity_assert` with the §8.2 fail-loud line, and the
  executed courts: `tests/identity_court.rs` (§8.2, 8 payload sizes,
  wire+memory tiers — 2-tier mode per A04 §10 until the A17/A19 disk views
  land, printed as loud `[SKIP]`s), `tests/static_assert_court.rs`
  (§9 DoD 2, real-rustc shim court: baseline → SHIM → compile-fail, on the
  real src text in temp copies), `tests/source_scan_gate.rs` (§8.3/DoD 5,
  poison positive control), `tests/doc_offset_table_court.rs` (§9 DoD 9),
  `tests/unit_dma_ring.rs` (§7/§8.3 typed refusals).
- no_std status: every src module is core-only (enforced mechanically by
  `tests/source_scan_gate.rs`: zero `std::`/alloc tokens in src); the
  crate-level `#![no_std]` attribute and the DoD 8 target build are
  coordinator-owned (lib.rs is off-limits to lanes) — the src tree is
  proven RV32IMC-clean by `tests/rv32imc_court.rs` (real `rustc --target
  riscv32imc-unknown-none-elf`, full rlib codegen, on the exact
  `include_str!` src bytes under a generated `#![no_std]` root; host
  baseline control included).
- Wave 4 (harden + close DoD, LANDED): `tests/guard_flip_court.rs`
  (§8.3/DoD 4 anti-vacuity: nine guard-removal shims applied to temp copies
  of the real src text — baseline probe exit 0, shim probe exit 42 —
  proving every refusal pin load-bearing; anchor drift fails loud),
  `tests/manifest_court.rs` (DoD 1 zero-runtime-deps + DoD 10 URN cited /
  no SPDX shapes), `tests/source_scan_gate.rs` hardened to file level
  (DoD 5's letter: a `vec!` injected into a scanned directory is flagged
  AND named; clean-dir negative control), and `tests/rv32imc_court.rs`
  (DoD 8 src-tree target cleanliness, replacing the former scratch-probe
  pointer which cited a receipt that never existed on disk).
- Wave 5 (defect repair, LANDED): three wire-length gates widened so a
  lying `payload_byte_len` can never narrow via `as usize` —
  `CgapFrameHeader::payload` (`usize::try_from` + `checked_add`),
  `DmaRing::publish` (usize-side widening), `DmaRing::read` (u64 bound).
  Pins: `payload_view_refuses_wire_lengths_that_cannot_name_an_end`,
  `publish_refuses_truncating_wire_lengths`; value rows are green on both
  sides of the fix on the 64-bit host (the widening is load-bearing on
  32-bit; disclosed per pin).
- Wave 7 (defect repair, LANDED 2026-11-01): the wave-5 read() u64 bound
  and the `slot_pow2` shift guard had NO flip witness — sandbox witness
  /tmp/a04-w7-sbx: deleting either flipped nothing across all courts.
  Closed: `read_refuses_lying_header_written_around_the_api` +
  `new_refuses_slot_pow2_overflow_fail_closed` (value-level pins,
  unit_dma_ring.rs) and `row_10_read_u64_bound_guard_flip` (rustc shim
  court, guard_flip_court.rs); both guards' widened text now sits under
  the guard_flip anchor self-check (anchor drift fails loud).
- Wave 9 (cross-lane integration, LANDED 2026-11-01): the A14 dam over
  real A04 frames — `tests/cross_lane_gate_court.rs` executes the first
  cross-lane tier of the identity-court family. Per doc size (§8.2's 8
  payload sizes): wire encode → `DmaRing` memory projection
  (`identity_assert` bit-for-bit) → `cgap_gate::penstock::ingress` admit,
  with the admitted view pinned byte-for-byte to the frame payload region
  on both the wire and memory projections. Cross-lane refusal parity:
  6 attestation mutations + magic / version / zeroed-digest /
  payload-len-shift / lying-caller-geometry, each refused by BOTH lanes
  with the contract-mapped refusals (`FrameError::*` ↔ `CGAP_ERR_*`).
  The A14 row-6 divergence (gate admits reserved flag bits; A04 §6.3
  refuses them) is PINNED as a divergence test, not silent. Non-vacuity:
  shim-court on the real gate source in /tmp/a04w9-sbx — baseline 8/8
  exit 0, always-admit SHIM (`return CGAP_OK;` before the gate's group-1
  checks) flips 8/8 exit 101, restore sha256 byte-exact (`709707c4…2555`),
  restored run green. `cgap-gate` is a dev-dependency only (same §3 named
  exception as cgap-blake3); the lib target stays E=0.

