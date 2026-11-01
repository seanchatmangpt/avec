//! Guard-flip shim court — lane A04 Wave 4 hardening (§8.3 anti-vacuity, §9 DoD 4).
//!
//! DoD 4's letter: "each §8.3 mutation demonstrably flips its court". The
//! value-level mutation tests (mutated header in, typed refusal out) prove
//! the validator *runs*; this court proves each refusal pin is *load
//! bearing*: with the guard removed from the REAL `src/*.rs` text, the
//! behavior probe observes the opposite outcome:
//!
//!   BASELINE (unmutated module text, same probe)  → probe exit 0 (pin holds)
//!   SHIM    (guard surgically removed, anchor-exact) → probe exit 42 (flip)
//!
//! Shim-court protocol realization: shims are applied to temp COPIES of the
//! real src text (`include_str!` bytes; the static_assert_court.rs
//! discipline) — the tree on disk is never mutated, so the restore step is
//! vacuous by construction; the court fails loudly if an anchor disappears
//! (guard refactored ⇒ update the court, never skip silently). Real `rustc`
//! subprocess per probe — no mocks (Chicago discipline). A SHIM
//! compile-abort is recorded as such and FAILS the court: a compile-abort is
//! not a flip.
//!
//! §8.3 row map (guard rows here; compile rows in static_assert_court.rs;
//! value rows in src/frame.rs + tests/identity_court.rs + unit_dma_ring.rs):
//!   magic flip → BadMagic            → row_01 (guard: magic compare)
//!   version bump → BadVersion        → row_02 (guard: version compare)
//!   flags bit 4 reserved             → row_03 (guard: reserved-mask compare)
//!   attestation corruption           → row_04 (guard: validate() delegation)
//!   payload corruption → Digest      → row_05 (guard: digest compare)
//!   payload_byte_len shift           → row_06 (guard: length compare)
//!   publish > slot capacity          → row_07 (guard: Overflow bound)
//!   read unpublished → None          → row_08 (guard: empty-ring None)
//!   identity bit-for-bit compare     → row_09 (guard: divergence loop)
//!   header pad/drop 127/129 B        → static_assert_court.rs (rustc E0080)
//!
//! Wave 7 (defect repair, 2026-11-01): rows 01-09 left the wave-5 read()
//! u64-bound repair (dma_ring.rs) and the `slot_pow2` shift guard unpinned —
//! sandbox witness /tmp/a04-w7-sbx: deleting the read() gate (M-A) or the
//! pow2 guard (M-B) flipped NOTHING across all courts. row_10 lands the
//! read-gate removal flip; the pow2 guard's anchor joins the self-check
//! below; the value-level pins live in tests/unit_dma_ring.rs.

use std::path::PathBuf;
use std::process::Command;

/// The real subjects: exact bytes of the src modules at test-compile time.
const FRAME_SRC: &str = include_str!("../src/frame.rs");
const DMA_SRC: &str = include_str!("../src/dma_ring.rs");
const ISO_SRC: &str = include_str!("../src/isomorphism.rs");

/// Probe exit code: the pin under test HOLDS (the §8.3 refusal was observed).
const PIN_HOLDS: i32 = 0;
/// Probe exit code: the pin FLIPPED (refusal gone or wrong variant).
const PIN_FLIPPED: i32 = 42;

/// Single-anchored string surgery: fails the court loudly if the anchor
/// disappears from the src text (the shim must actually mutate).
fn replace_anchored(haystack: &str, anchor: &str, replacement: &str) -> String {
    assert!(
        haystack.contains(anchor),
        "SHIM-COURT anchor not found in src text — the guard was refactored; \
         update this court instead of skipping (§8.3: a mutation row that \
         cannot flip is retired with a receipt, never left as filler). \
         anchor={anchor:?}"
    );
    haystack.replacen(anchor, replacement, 1)
}

/// Host probe assembly: the three modules (frame + dma_ring + iso;
/// dma_ring needs frame; rows that exercise one tier compile the others
/// dead) plus shared helpers, the row's `run() -> bool` (true = pin holds),
/// and the main that maps that bool to the exit contract.
fn build_probe(frame_text: &str, dma_text: &str, iso_text: &str, run_body: &str) -> String {
    let mods = format!(
        "mod frame {{\n{frame_text}\n}}\nmod dma_ring {{\n{dma_text}\n}}\nmod iso {{\n{iso_text}\n}}\n"
    );
    format!("{mods}\n{PROBE_HELPERS}\n{run_body}\n{PROBE_MAIN}\n")
}

/// Probe with only the frame module shimmed (rows 01-06).
fn probe_shimmed_frame(frame_text: &str, run_body: &str) -> String {
    build_probe(frame_text, DMA_SRC, ISO_SRC, run_body)
}

/// Probe with only the dma_ring module shimmed (rows 07-08).
fn probe_shimmed_dma(dma_text: &str, run_body: &str) -> String {
    build_probe(FRAME_SRC, dma_text, ISO_SRC, run_body)
}

/// Probe with only the isomorphism module shimmed (row 09).
fn probe_shimmed_iso(iso_text: &str, run_body: &str) -> String {
    build_probe(FRAME_SRC, DMA_SRC, iso_text, run_body)
}

const PROBE_HELPERS: &str = r#"
fn probe_digest(data: &[u8]) -> [u8; 32] {
    let mut d = [0u8; 32];
    let n = data.len().min(32);
    d[..n].copy_from_slice(&data[..n]);
    d
}

fn valid_header(len: u64) -> frame::CgapFrameHeader {
    let payload = vec![0x42u8; len as usize];
    frame::CgapFrameHeader {
        magic: frame::FRAME_MAGIC,
        version: frame::FRAME_VERSION,
        flags: frame::FLAG_BN3,
        reserved: [0; 2],
        payload_byte_len: len,
        oabi_digest: probe_digest(&payload),
        attestation: frame::P28LicenseAttestation {
            license_magic: frame::LICENSE_MAGIC,
            version: 1,
            attestation_flags: frame::LICENSE_FLAG_ROOT,
            safe_harbor_ceiling_cents: frame::SAFE_HARBOR_CEILING_CENTS_MAX,
            origin_authority_digest: [7u8; 32],
            upstream_receipt: [0u8; 32],
        },
    }
}

/// 64-aligned region (the ring ABI demands a 64-byte-aligned region; a
/// plain heap `Vec<u8>` is 1-aligned and would refuse `MisalignedRegion`).
/// Same sanctioned fixture shape as tests/common/mod.rs `leak_aligned_region`.
fn aligned_region() -> &'static mut [u8] {
    use std::alloc::{alloc_zeroed, Layout};
    let layout = Layout::from_size_align(24576, 64).expect("region layout");
    let ptr = unsafe { alloc_zeroed(layout) };
    assert!(!ptr.is_null(), "region alloc failed");
    assert_eq!(ptr as usize % 64, 0, "allocator broke 64-byte alignment");
    unsafe { std::slice::from_raw_parts_mut(ptr, 24576) }
}
"#;

const PROBE_MAIN: &str = r#"
fn main() {
    let holds = run();
    std::process::exit(if holds { 0 } else { 42 });
}
"#;

fn probe_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cgap-a04-guard-flip-{tag}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// Compile `text` as a host bin; Err carries rustc stderr on failure.
fn rustc_bin(dir: &PathBuf, tag: &str, text: &str) -> Result<PathBuf, String> {
    let src_file = dir.join(format!("probe_{tag}.rs"));
    std::fs::write(&src_file, text).expect("write probe source");
    let bin = dir.join(format!("bin_{tag}"));
    let out = Command::new("rustc")
        .args(["--edition", "2021"])
        .args(["--crate-type", "bin"])
        .args(["-o"])
        .arg(&bin)
        .arg(&src_file)
        .output()
        .expect("rustc on PATH");
    if out.status.success() {
        Ok(bin)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn run_probe(bin: &PathBuf) -> i32 {
    let out = Command::new(bin).output().expect("run probe binary");
    out.status.code().unwrap_or(-1)
}

/// The court: BASELINE probe must compile and hold the pin (exit 0); SHIM
/// probe must compile and flip it (exit 42). A shim compile-abort fails the
/// court as a compile-abort, never as a flip.
fn witness_flip(run_body: &'static str, shim_probe: impl FnOnce() -> String) {
    let name = std::thread::current()
        .name()
        .unwrap_or("guard-row")
        .to_string();
    let dir = probe_dir(&name);

    let baseline_src = build_probe(FRAME_SRC, DMA_SRC, ISO_SRC, run_body);
    let baseline_bin = rustc_bin(&dir, "baseline", &baseline_src)
        .unwrap_or_else(|e| panic!("baseline probe must compile (court sanity):\n{e}"));
    let baseline_rc = run_probe(&baseline_bin);
    assert_eq!(
        baseline_rc, PIN_HOLDS,
        "baseline must hold the pin (an always-flip court carries no bits), rc={baseline_rc}"
    );

    let shim_src = shim_probe();
    let shim_bin = rustc_bin(&dir, "shim", &shim_src)
        .unwrap_or_else(|e| panic!("SHIM compile-abort (recorded as such, not a flip):\n{e}"));
    let shim_rc = run_probe(&shim_bin);
    assert_eq!(
        shim_rc, PIN_FLIPPED,
        "GUARD-COURT_FLIP_FAIL: shim compiled and ran but the pin still holds — \
         the §8.3 court row is vacuous against this mutation class, rc={shim_rc}"
    );

    let _ = std::fs::remove_dir_all(&dir); // temp lease deleted in-file
}

// --- the nine §8.3 guard rows -------------------------------------------------

#[test]
fn row_01_magic_guard_flip() {
    // §8.3: flip magic to "XGAP" → validate() refuses, FrameError::BadMagic.
    witness_flip(
        r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.magic = *b"XGAP";
    matches!(h.validate(), Err(frame::FrameError::BadMagic))
}
"#,
        || {
            probe_shimmed_frame(
                &replace_anchored(
                    FRAME_SRC,
                    "        if self.magic != FRAME_MAGIC {\n            return Err(FrameError::BadMagic);\n        }\n",
                    "",
                ),
                ROW_01_BODY,
            )
        },
    );
}

const ROW_01_BODY: &str = r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.magic = *b"XGAP";
    matches!(h.validate(), Err(frame::FrameError::BadMagic))
}
"#;

#[test]
fn row_02_version_guard_flip() {
    // §6 "format version, start at 1": the BadVersion pin is load-bearing.
    witness_flip(
        r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.version = 2;
    matches!(h.validate(), Err(frame::FrameError::BadVersion))
}
"#,
        || {
            probe_shimmed_frame(
                &replace_anchored(
                    FRAME_SRC,
                    "        if self.version != FRAME_VERSION {\n            return Err(FrameError::BadVersion);\n        }\n",
                    "",
                ),
                ROW_02_BODY,
            )
        },
    );
}

const ROW_02_BODY: &str = r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.version = 2;
    matches!(h.validate(), Err(frame::FrameError::BadVersion))
}
"#;

#[test]
fn row_03_reserved_flags_guard_flip() {
    // §6.3/§8.3: flags bit 4 (reserved) set → FrameError::ReservedBitsSet.
    witness_flip(
        r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.flags |= 0x10;
    matches!(h.validate(), Err(frame::FrameError::ReservedBitsSet))
}
"#,
        || {
            probe_shimmed_frame(
                &replace_anchored(
                    FRAME_SRC,
                    "        if self.flags & FLAG_RESERVED_MASK != 0 {\n            return Err(FrameError::ReservedBitsSet);\n        }\n",
                    "",
                ),
                ROW_03_BODY,
            )
        },
    );
}

const ROW_03_BODY: &str = r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.flags |= 0x10;
    matches!(h.validate(), Err(frame::FrameError::ReservedBitsSet))
}
"#;

#[test]
fn row_04_attestation_delegation_guard_flip() {
    // §8.3: corrupt 1 byte of attestation → FrameError::AttestationMismatch.
    // The guard under test is the delegation from CgapFrameHeader::validate
    // into P28LicenseAttestation::validate (P28 internals are A17's
    // audited surface; their own flips are witnessed in the A17 receipt).
    witness_flip(
        r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.attestation.license_magic = *b"P28X";
    matches!(h.validate(), Err(frame::FrameError::AttestationMismatch))
}
"#,
        || {
            probe_shimmed_frame(
                &replace_anchored(
                    FRAME_SRC,
                    "        self.attestation.validate()?;\n",
                    "",
                ),
                ROW_04_BODY,
            )
        },
    );
}

const ROW_04_BODY: &str = r#"
fn run() -> bool {
    let mut h = valid_header(0);
    h.attestation.license_magic = *b"P28X";
    matches!(h.validate(), Err(frame::FrameError::AttestationMismatch))
}
"#;

#[test]
fn row_05_digest_compare_guard_flip() {
    // §8.3: corrupt 1 byte of payload → digest mismatch, DigestMismatch.
    witness_flip(
        r#"
fn run() -> bool {
    let payload = [7u8; 64];
    let mut h = valid_header(64);
    h.oabi_digest = [0u8; 32]; // wrong embedded digest
    matches!(
        h.verify_payload(&payload, probe_digest),
        Err(frame::FrameError::DigestMismatch)
    )
}
"#,
        || {
            probe_shimmed_frame(
                &replace_anchored(
                    FRAME_SRC,
                    "        if self.oabi_digest != digest(payload) {\n            return Err(FrameError::DigestMismatch);\n        }\n",
                    "",
                ),
                ROW_05_BODY,
            )
        },
    );
}

const ROW_05_BODY: &str = r#"
fn run() -> bool {
    let payload = [7u8; 64];
    let mut h = valid_header(64);
    h.oabi_digest = [0u8; 32]; // wrong embedded digest
    matches!(
        h.verify_payload(&payload, probe_digest),
        Err(frame::FrameError::DigestMismatch)
    )
}
"#;

#[test]
fn row_06_payload_len_compare_guard_flip() {
    // §8.3: shift payload_byte_len by +1 → PayloadLenMismatch (offset 8 gate).
    witness_flip(
        r#"
fn run() -> bool {
    let payload = [7u8; 64];
    let mut h = valid_header(64);
    h.payload_byte_len = 65;
    h.oabi_digest = probe_digest(&payload); // digest correct: the len gate is what trips
    matches!(
        h.verify_payload(&payload, probe_digest),
        Err(frame::FrameError::PayloadLenMismatch)
    )
}
"#,
        || {
            probe_shimmed_frame(
                &replace_anchored(
                    FRAME_SRC,
                    "        if payload.len() as u64 != self.payload_byte_len {\n            return Err(FrameError::PayloadLenMismatch);\n        }\n",
                    "",
                ),
                ROW_06_BODY,
            )
        },
    );
}

const ROW_06_BODY: &str = r#"
fn run() -> bool {
    let payload = [7u8; 64];
    let mut h = valid_header(64);
    h.payload_byte_len = 65;
    h.oabi_digest = probe_digest(&payload); // digest correct: the len gate is what trips
    matches!(
        h.verify_payload(&payload, probe_digest),
        Err(frame::FrameError::PayloadLenMismatch)
    )
}
"#;

#[test]
fn row_07_ring_overflow_guard_flip() {
    // §8.3: publish payload len > slot capacity → RingError::Overflow.
    witness_flip(
        ROW_07_BODY,
        || {
            probe_shimmed_dma(
                &replace_anchored(
                    DMA_SRC,
                    "        if len > self.slot_size {\n            return Err(RingError::Overflow);\n        }\n",
                    "",
                ),
                ROW_07_BODY,
            )
        },
    );
}

const ROW_07_BODY: &str = r#"
fn run() -> bool {
    let mut ring = dma_ring::DmaRing::new(aligned_region(), 1).expect("region");
    {
        let slot = ring.claim().expect("room");
        valid_header(1).encode_into(&mut slot[..frame::FRAME_HEADER_LEN]).expect("encode");
        slot[frame::FRAME_HEADER_LEN] = 0x42;
    }
    matches!(
        ring.publish(ring.slot_size() + 1),
        Err(dma_ring::RingError::Overflow)
    )
}
"#;

#[test]
fn row_08_read_unpublished_guard_flip() {
    // §8.3: read an unpublished slot → None (typed: no frame), never garbage.
    witness_flip(
        ROW_08_BODY,
        || {
            probe_shimmed_dma(
                &replace_anchored(
                    DMA_SRC,
                    "        if head == tail {\n            return None;\n        }\n",
                    "",
                ),
                ROW_08_BODY,
            )
        },
    );
}

const ROW_08_BODY: &str = r#"
fn run() -> bool {
    let region = aligned_region();
    let mut ring = dma_ring::DmaRing::new(region, 1).expect("region");
    ring.read().is_none()
}
"#;

#[test]
fn row_09_identity_compare_guard_flip() {
    // §8.3 identity row: the byte-compare loop is the guard — neutering it
    // (divergence treated as equal) must silence the court's detection.
    witness_flip(
        ROW_09_BODY,
        || {
            probe_shimmed_iso(
                &replace_anchored(
                    ISO_SRC,
                    "            return Err(report(i));\n",
                    "            continue;\n",
                ),
                ROW_09_BODY,
            )
        },
    );
}

const ROW_09_BODY: &str = r#"
fn run() -> bool {
    let wire = [1u8, 2, 3, 4, 5];
    let mut mem = wire;
    mem[3] ^= 0x01;
    matches!(iso::identity_assert(&wire, iso::Tier::Memory, &mem), Err(_))
}
"#;

#[test]
fn row_10_read_u64_bound_guard_flip() {
    // Wave-7 row: read()'s u64 bound (the wave-5 repair, dma_ring.rs) is
    // load-bearing — deleting it must flip. The probe publishes a valid
    // frame, hostile-writes u64::MAX into `payload_byte_len` around the ring
    // API, and demands the typed no-frame None. Shim = gate deleted, so
    // read() builds an out-of-slot slice and PANICS; catch_unwind maps the
    // panic to run() == false → exit 42 (a panic exit 101 is mapped by the
    // probe itself, never a silent pass).
    witness_flip(
        ROW_10_BODY,
        || {
            probe_shimmed_dma(
                &replace_anchored(
                    DMA_SRC,
                    "        if header.payload_byte_len > (self.slot_size - SLOT_HEADER_LEN) as u64 {\n            return None;\n        }\n",
                    "",
                ),
                ROW_10_BODY,
            )
        },
    );
}

const ROW_10_BODY: &str = r#"
fn run() -> bool {
    let r = std::panic::catch_unwind(|| {
        let region = aligned_region();
        // raw escape of the region base BEFORE the ring takes ownership —
        // the hostile write goes around the ring API, straight into bytes.
        let base: *mut u8 = region.as_ptr() as *mut u8;
        let mut ring = dma_ring::DmaRing::new(region, 1).expect("region");
        {
            let slot = ring.claim().expect("room");
            valid_header(1)
                .encode_into(&mut slot[..frame::FRAME_HEADER_LEN])
                .expect("encode");
            slot[frame::FRAME_HEADER_LEN] = 0x42;
        }
        ring.publish(frame::FRAME_HEADER_LEN + 1).expect("valid frame publishes");
        unsafe {
            core::ptr::write(base.add(8) as *mut u64, u64::MAX);
        }
        ring.read().is_none()
    });
    matches!(r, Ok(true))
}
"#;

#[test]
fn court_self_check_anchors_are_present_in_real_src() {
    // Non-vacuity of the court ITSELF: every anchor must exist in the real
    // src text right now, so a silent guard refactor cannot disable a row.
    for anchor in [
        "if self.magic != FRAME_MAGIC {",
        "if self.version != FRAME_VERSION {",
        "if self.flags & FLAG_RESERVED_MASK != 0 {",
        "self.attestation.validate()?;",
        "if self.oabi_digest != digest(payload) {",
        "if payload.len() as u64 != self.payload_byte_len {",
        "if len > self.slot_size {",
        "if head == tail {",
        "if header.payload_byte_len > (self.slot_size - SLOT_HEADER_LEN) as u64 {",
        "if slot_pow2 >= usize::BITS as u8 {",
        "return Err(report(i));",
    ] {
        let found =
            FRAME_SRC.contains(anchor) || DMA_SRC.contains(anchor) || ISO_SRC.contains(anchor);
        assert!(found, "guard anchor vanished from src: {anchor:?}");
    }
}
