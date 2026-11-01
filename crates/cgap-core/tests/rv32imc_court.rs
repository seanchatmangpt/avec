//! RV32IMC `no_std` compile court — lane A04 §9 DoD 8 hardening (Wave 4).
//!
//! DoD 8: "`cargo build --target riscv32imc-unknown-none-elf` (ESP32-C3
//! class) compiles cgap-core clean". The in-tree invocation is BLOCKED on
//! the crate-level `#![no_std]` attribute (`src/lib.rs` is coordinator-
//! owned per the lane dispatch contract — lanes must not edit it), so this
//! court witnesses the src tree's target-cleanliness the only lawful way:
//! the exact `include_str!` bytes of the three modules, assembled under a
//! generated `#![no_std]` lib root in a temp dir (the static_assert_court.rs
//! temp-copy discipline), compiled by the real toolchain FOR the real
//! target, full codegen (`--crate-type rlib`), not metadata-only.
//!
//! Failure modes, all loud:
//!   - toolchain missing the target → test FAILS with the `rustup target
//!     add` instruction (never a silent pass);
//!   - any module needs std/alloc/atomics the target lacks → rustc stderr
//!     is printed and the test FAILS;
//!   - the subject text drifts from the tree (include_str! is compile-time)
//!     → impossible by construction.
//!
//! Replaces the README's scratch-probe pointer (which cited a lane receipt
//! that did not exist on disk — a hearsay pointer, now an executed court).

use std::path::PathBuf;
use std::process::Command;

/// The real subjects: exact bytes of the src modules at test-compile time.
const FRAME_SRC: &str = include_str!("../src/frame.rs");
const DMA_SRC: &str = include_str!("../src/dma_ring.rs");
const ISO_SRC: &str = include_str!("../src/isomorphism.rs");

const TARGET: &str = "riscv32imc-unknown-none-elf";

#[test]
fn src_tree_compiles_no_std_for_rv32imc_esp32_c3_class() {
    let lib_text = format!(
        "#![no_std]\n#![allow(dead_code)]\nmod frame {{\n{FRAME_SRC}\n}}\nmod dma_ring {{\n{DMA_SRC}\n}}\nmod isomorphism {{\n{ISO_SRC}\n}}\n"
    );

    let dir = std::env::temp_dir().join(format!(
        "cgap-a04-rv32imc-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src_file: PathBuf = dir.join("cgap_core_probe_lib.rs");
    std::fs::write(&src_file, &lib_text).expect("write probe lib");
    let rlib = dir.join("libcgap_core_probe.rlib");

    let out = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg("--target")
        .arg(TARGET)
        .args(["--crate-type", "rlib"])
        .args(["-o"])
        .arg(&rlib)
        .arg(&src_file)
        .output()
        .expect("rustc on PATH");

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let rlib_built = rlib.exists(); // read BEFORE the temp dir is deleted
    let _ = std::fs::remove_dir_all(&dir); // temp lease deleted in-file

    assert!(
        out.status.success() && rlib_built,
        "RV32IMC_COURT_FAIL: cgap-core src does not compile clean as #![no_std] \
         for {TARGET} (DoD 8). stderr:\n{stderr}"
    );
}

#[test]
fn host_target_baseline_same_text_compiles_clean() {
    // Court sanity (non-vacuity): the SAME assembly compiles for the host —
    // an always-fail court would fail here too, so a red RV32IMC row below
    // is attributable to the target, not the harness.
    let lib_text = format!(
        "#![no_std]\n#![allow(dead_code)]\nmod frame {{\n{FRAME_SRC}\n}}\nmod dma_ring {{\n{DMA_SRC}\n}}\nmod isomorphism {{\n{ISO_SRC}\n}}\n"
    );
    let dir = std::env::temp_dir().join(format!(
        "cgap-a04-rv32imc-host-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src_file: PathBuf = dir.join("cgap_core_host_probe_lib.rs");
    std::fs::write(&src_file, &lib_text).expect("write probe lib");
    let rlib = dir.join("libcgap_core_host_probe.rlib");

    let out = Command::new("rustc")
        .args(["--edition", "2021"])
        .args(["--crate-type", "rlib"])
        .args(["-o"])
        .arg(&rlib)
        .arg(&src_file)
        .output()
        .expect("rustc on PATH");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        out.status.success(),
        "host baseline of the same no_std text must compile (court sanity):\n{stderr}"
    );
}
