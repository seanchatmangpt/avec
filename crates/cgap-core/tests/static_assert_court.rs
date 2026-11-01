//! §8.3 frame-header row + §9 DoD 2 court: "pad or drop a field: header size
//! 127 or 129 B → static assert fails to compile — witnessed, not claimed."
//!
//! Shim-court protocol on the REAL subject text, byte-exact and non-vacuous:
//! BASELINE (`src/frame.rs` as-is, via `include_str!`) must COMPILE CLEAN,
//! proving the harness is not an always-fail court; each SHIM (the same real
//! text with one layout mutation applied in a temp copy — the tree on disk is
//! never touched, so no restore step is needed) must FAIL TO COMPILE with the
//! const-assert error class. A shim that still compiles means the §6.4 pins
//! regressed: the court FAILs (exit non-zero). Real `rustc` subprocess per
//! case — no mocks.

use std::path::PathBuf;
use std::process::Command;

/// The real subject: the exact bytes of src/frame.rs at test-compile time.
const FRAME_SRC: &str = include_str!("../src/frame.rs");

/// Compile `text` as a standalone lib crate with the repo's rustc.
/// Returns (compiled_ok, stderr).
fn rustc_compiles(tag: &str, text: &str) -> (bool, String) {
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "cgap-a04-static-assert-{}-{}",
        tag,
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("frame_probe.rs");
    std::fs::write(&file, text).expect("write probe");
    let out = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit")
        .arg("metadata")
        .arg("-o")
        .arg(dir.join("probe.rmeta"))
        .arg(&file)
        .output()
        .expect("rustc on PATH");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let ok = out.status.success();
    let _ = std::fs::remove_dir_all(&dir); // no temp leases left behind
    (ok, stderr)
}

fn replace_anchored(haystack: &str, anchor: &str, replacement: &str) -> String {
    // Single-anchored string surgery: fails the court loudly if the anchor
    // disappears from src/frame.rs (the shim must actually mutate).
    assert!(
        haystack.contains(anchor),
        "SHIM-COURT anchor not found in src/frame.rs: {anchor:?}"
    );
    haystack.replacen(anchor, replacement, 1)
}

#[test]
fn baseline_real_frame_src_compiles_clean() {
    // Non-vacuity: the court's baseline is green, so the SHIM failures below
    // carry information (an always-fail court would fail here).
    let (ok, stderr) = rustc_compiles("baseline", FRAME_SRC);
    assert!(ok, "baseline src/frame.rs must compile; stderr:\n{stderr}");
}

#[test]
fn shim_pad_header_field_to_129b_fails_compile() {
    // §8.3: pad a field → header size 129 B → §6.4 static assert must fail
    // to compile.
    let shim = replace_anchored(
        FRAME_SRC,
        "    pub reserved: [u8; 2],",
        "    pub reserved: [u8; 2],\n    pub extra_pad: [u8; 1],",
    );
    let (ok, stderr) = rustc_compiles("pad-129", &shim);
    assert!(!ok, "129-byte header must fail to compile");
    assert!(stderr.contains("E0080"), "expected const-assert E0080, got:\n{stderr}");
}

#[test]
fn shim_shrink_reserved_field_witnesses_align64_rounding_limit() {
    // §8.3 shrink reading — RETIRED WITH RECEIPT, witnessed here (not left
    // as filler). The doc row claims "header size 127 B → static assert
    // fails to compile". Executed finding: a coherent 1-byte shrink of
    // `reserved` ([u8;2] → [u8;1], anchors updated in encode/decode) still
    // COMPILES: repr(C, align(64)) rounds size_of 127 back to 128, the u64
    // alignment re-fills the byte-7 hole before payload_byte_len (offset 8
    // unchanged), and every §6.4 size/align/offset pin holds. The const
    // lattice detects GROWTH (129 B pad → 192, trips E0080), never
    // shrinkage — no stable-core mechanism pins absence-of-padding. The
    // wire-visible reserved WIDTH is therefore policed at runtime: the
    // §6.1 doc-offset-table court (doc size cell = 2) and the inline
    // `reserved_bytes_zeroed_on_write` encode test. This test witnesses the
    // rounding boundary so the retirement is an executed fact.
    let shim = replace_anchored(
        FRAME_SRC,
        "    pub reserved: [u8; 2],",
        "    pub reserved: [u8; 1],",
    );
    let shim = replace_anchored(
        &shim,
        "let mut reserved = [0u8; 2];",
        "let mut reserved = [0u8; 1];",
    );
    let shim = replace_anchored(
        &shim,
        "reserved.copy_from_slice(&src[6..8]);",
        "reserved.copy_from_slice(&src[6..7]);",
    );
    let shim = replace_anchored(
        &shim,
        "out[6..8].copy_from_slice(&[0u8; 2]); // reserved: zero on write (§6)",
        "out[6..7].copy_from_slice(&[0u8; 1]); // reserved: zero on write (§6)",
    );
    let (ok, stderr) = rustc_compiles("shrink-127", &shim);
    assert!(
        ok,
        "expected the 127 B shrink to survive the §6.4 lattice (align-64 \
         rounding); if this flips to a compile failure, the pin lattice grew \
         teeth and this retirement receipt should be re-examined. stderr:\n{stderr}"
    );
    // and the unmutated subject still pins 128/64/80 (growth direction).
    let (ok, stderr) = rustc_compiles("baseline-recheck", FRAME_SRC);
    assert!(ok, "baseline must still compile; stderr:\n{stderr}");
}

#[test]
fn shim_pad_attestation_field_fails_compile() {
    // §9 DoD 2: "a mutation to any of these fails to compile" — the
    // attestation's 80 B pin and every downstream offset pin must trip.
    let shim = replace_anchored(
        FRAME_SRC,
        "    pub upstream_receipt: [u8; 32],\n}",
        "    pub upstream_receipt: [u8; 32],\n    pub extra: [u8; 8],\n}",
    );
    let (ok, stderr) = rustc_compiles("attest-pad", &shim);
    assert!(!ok, "88-byte attestation must fail to compile");
    assert!(stderr.contains("E0080"), "expected const-assert E0080, got:\n{stderr}");
}
