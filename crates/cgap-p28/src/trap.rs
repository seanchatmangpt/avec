//! Lane A17 — P28 spoliation trap: Wave 1 constant freeze + Wave 2 halt path.
//!
//! Spec: `docs/jira/v26.11.1/lanes/A17-p28-derivative.md` §6 (spoliation
//! trap: halt, not errno). Wave 1 froze the constant this lane owns (§6.1)
//! plus compile-time pins binding it to the attestation magic. Wave 2 lands
//! the halt edge itself (§6.2): `std::process::abort()` on native with the
//! discriminator staged in a global for the crash handler, and the §6.1
//! check-then-halt C-ABI entry `cgap_p28_invariant_check` (the FFI *contract*
//! remains A19's; this lane owns the constant and the halt semantics).
//!
//! Wave 3 lands the §7.1/§7.2 full-form courts over REAL toolchain-built
//! artifacts (§8 DoD 7/8): `spoliation_loader_court` runs the production
//! loader path (`attestation::find_in_elf`/`find_in_wasm` → `from_bytes` →
//! §5 `validate`) in a child process against a clang-emitted ELF object and
//! a wat2wasm-emitted WASM module, drives the §7.1 subprocess-per-mutation
//! bit-flip sweep (640 mutations x 2 records x 2 formats, all real process
//! deaths), and witnesses the §7.2 strip forms (section removed from the
//! container, parent receipt zeroed) as real halts.
//!
//! An errno return is advisory — the caller can ignore it and keep running a
//! spoliated artifact. The only lawful outcome of a failed invariant check
//! is that the artifact never executes another instruction (§6.1, boundary
//! invariant 5): `spoliation_halt` is `-> !` and never returns.
//!
//! `0x503238FF` reads as ASCII `'P' '2' '8' 0xFF` — the magic is carried in
//! the error constant so a crash dump identifies the trap without symbols
//! (§6.1).

use core::sync::atomic::{AtomicU32, Ordering};

use crate::attestation::{P28LicenseAttestation, ATTESTATION_LEN};

/// `ERR_P28_SPOLIATION = 0x503238FF` (§6): "P28" + 0xFF sentinel. Any
/// validation failure (§5 steps 1-8) or one-bit mutation of the attestation
/// maps to this halt class — no fingerprintable per-check error codes.
pub const ERR_P28_SPOLIATION: u32 = 0x5032_38FF;

// ---- ABI freeze: the constant is derived from the attestation magic ----
// 0x50='P', 0x32='2', 0x38='8' (§6.1): if the magic or the sentinel byte
// ever drifts from the record layout, the build fails here, before any
// artifact can carry a mismatched discriminator.
const _: () = assert!(
    ERR_P28_SPOLIATION
        == ((crate::attestation::LICENSE_MAGIC[0] as u32) << 24
            | (crate::attestation::LICENSE_MAGIC[1] as u32) << 16
            | (crate::attestation::LICENSE_MAGIC[2] as u32) << 8
            | 0xFF)
);

/// §6.1 — "a value the embedder can poll": the discriminator staged for the
/// crash handler. Zero means no spoliation halt has fired in this process;
/// [`ERR_P28_SPOLIATION`] means [`spoliation_halt`] staged the constant
/// immediately before aborting, so a post-mortem can identify the trap
/// without symbols.
pub static STAGED_DISCRIMINATOR: AtomicU32 = AtomicU32::new(0);

/// §6.2 — the halt: stage [`ERR_P28_SPOLIATION`] in the global, then abort
/// the process. Native targets take SIGABRT via `std::process::abort()`;
/// on `wasm32` the same call lowers to the WASM `unreachable` trap (witnessed
/// 2026-11-01: `cargo check -p cgap-p28 --target wasm32-unknown-unknown`
/// exit 0; a probe crate exporting `std::process::abort()` disassembled via
/// `wasm-tools print` shows the std abort lowering to a body of
/// `unreachable`), so the embedder sees the same fail-closed edge (§6.2).
/// Never returns.
pub fn spoliation_halt() -> ! {
    STAGED_DISCRIMINATOR.store(ERR_P28_SPOLIATION, Ordering::SeqCst);
    std::process::abort()
}

/// §6.1 — check-then-halt C-ABI surface consumed by `cgap-ffi` (A19 owns the
/// FFI contract; this lane owns the constant and the halt semantics). Runs
/// the record-intrinsic §5 checks (steps 2-5, 8 — the subset that needs no
/// instrument/chain context; full §5 validation is
/// `attestation::P28LicenseAttestation::validate`). Returns `0` iff the
/// record is intrinsically valid; on any spoliation condition — including a
/// null pointer, fail-closed — the process halts and this function never
/// returns.
///
/// # Safety
/// `att` must point to 80 bytes of readable memory, or be null (null halts).
#[no_mangle]
pub unsafe extern "C" fn cgap_p28_invariant_check(att: *const u8) -> u32 {
    if att.is_null() {
        // Fail-closed: an unreadable record is not an admission.
        spoliation_halt();
    }
    let bytes = unsafe { core::slice::from_raw_parts(att, ATTESTATION_LEN) };
    let verdict = match P28LicenseAttestation::from_bytes(bytes) {
        Err(_) => Err(()),
        Ok(rec) => rec.validate_intrinsic().map_err(|_| ()),
    };
    match verdict {
        Ok(()) => 0,
        Err(()) => spoliation_halt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::test_support::*;
    use crate::attestation::{
        instrument_header_span, origin_authority_digest, ATTESTATION_VERSION, FLAG_ROOT,
        LICENSE_MAGIC, SAFE_HARBOR_CEILING_MAX_CENTS,
    };
    use std::sync::Mutex;

    /// A record that passes the intrinsic §5 subset (steps 2-5, 8). The
    /// digest field is not intrinsically checked (step 6 needs context), so
    /// a marker value is fine here.
    fn valid_intrinsic_record() -> P28LicenseAttestation {
        P28LicenseAttestation {
            license_magic: LICENSE_MAGIC,
            version: ATTESTATION_VERSION,
            attestation_flags: FLAG_ROOT,
            safe_harbor_ceiling_cents: SAFE_HARBOR_CEILING_MAX_CENTS,
            origin_authority_digest: [0xAA; 32],
            upstream_receipt: [0u8; 32],
        }
    }

    /// Real pinned instrument bytes — used to build a fully-valid record for
    /// the check-then-halt positive control.
    fn real_instrument_span() -> Vec<u8> {
        let file = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../LICENSE-P28-SIL.md"
        ))
        .expect("pinned instrument LICENSE-P28-SIL.md is readable from the repo root");
        instrument_header_span(&file)
            .map(|s| s.to_vec())
            .expect("canonical instrument has a `---` line")
    }

    /// §6.1: in-process positive control — the check returns 0 iff valid.
    #[test]
    fn check_returns_zero_iff_intrinsic_valid() {
        let rec = valid_intrinsic_record();
        let wire = rec.to_bytes();
        assert_eq!(unsafe { cgap_p28_invariant_check(wire.as_ptr()) }, 0);

        let mut spoliated = wire;
        spoliated[0] ^= 0xFF; // magic bit flip -> intrinsic failure
                              // In-process we cannot survive the halt, so the spoliated verdict is
                              // witnessed through the same branch the C-ABI entry uses: parse +
                              // intrinsic validate fails. The real halt is courted in
                              // `check_then_halt_court` via a real subprocess below.
        let parsed = P28LicenseAttestation::from_bytes(&spoliated).ok();
        let spoliated_verdict =
            parsed.map_or_else(|| Err(()), |r| r.validate_intrinsic().map_err(|_| ()));
        assert!(spoliated_verdict.is_err());
    }

    /// Full libtest name of a test in this module: `module_path!()` carries
    /// the crate-name prefix (`cgap_p28::trap::tests`), but libtest names
    /// start at the in-crate module path (`trap::tests::<name>`).
    fn test_path(name: &str) -> String {
        let in_crate_mod = module_path!()
            .split("::")
            .skip(1)
            .collect::<Vec<_>>()
            .join("::");
        format!("{in_crate_mod}::{name}")
    }

    /// Child mode for the halt court: stage the discriminator, print it
    /// (witnessing the stage is observable), then take the real halt edge.
    /// The parent asserts a real SIGABRT death plus the staged constant.
    #[test]
    fn halt_court_subprocess_sigabrt() {
        if std::env::var("CGAP_P28_HALT_COURT_CHILD").as_deref() == Ok("1") {
            // Stage, witness, halt. `spoliation_halt` stages again (same
            // value) immediately before aborting — this child never returns.
            STAGED_DISCRIMINATOR.store(ERR_P28_SPOLIATION, Ordering::SeqCst);
            println!(
                "staged={:#010x}",
                STAGED_DISCRIMINATOR.load(Ordering::SeqCst)
            );
            spoliation_halt();
        }

        let exe = std::env::current_exe().expect("test binary path");
        let out = std::process::Command::new(exe)
            .args([
                "--exact",
                &test_path("halt_court_subprocess_sigabrt"),
                "--nocapture",
            ])
            .env("CGAP_P28_HALT_COURT_CHILD", "1")
            .output()
            .expect("spawn halt-court child");
        assert!(
            !out.status.success(),
            "halt court child must die, not exit cleanly: {out:?}"
        );
        #[cfg(unix)]
        {
            // SIGABRT == 6 on every Unix this workspace targets (macOS/Linux).
            assert_eq!(
                std::os::unix::process::ExitStatusExt::signal(&out.status),
                Some(6),
                "spoliation_halt must abort with SIGABRT, got {out:?}"
            );
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("staged=0x503238ff"),
            "child must stage ERR_P28_SPOLIATION before halting; stdout: {stdout:?}"
        );
    }

    /// §6.1 court: check-then-halt over a real subprocess — the valid record
    /// returns 0 and exits cleanly; the one-bit-sploiated record drives the
    /// C-ABI entry into a real SIGABRT death; a null pointer dies fail-closed.
    /// This is the anti-vacuity witness for §7.1's corollary: if the halt
    /// path is ever removed by a refactor, the spoliated child EXITS
    /// CLEANLY, this court fails loudly, and the trap's absence is visible.
    #[test]
    fn check_then_halt_court() {
        let mode = std::env::var("CGAP_P28_CHECK_COURT_CHILD").unwrap_or_default();
        match mode.as_str() {
            "valid" => {
                let span = real_instrument_span();
                let mut rec = valid_intrinsic_record();
                rec.origin_authority_digest = origin_authority_digest(&span);
                let wire = rec.to_bytes();
                assert_eq!(unsafe { cgap_p28_invariant_check(wire.as_ptr()) }, 0);
                return;
            }
            "spoliated" => {
                let wire = valid_intrinsic_record().to_bytes();
                let mut spoliated = wire;
                spoliated[4] ^= 0x01; // version bit flip: 1 -> 0, §5.3 failure
                unsafe { cgap_p28_invariant_check(spoliated.as_ptr()) };
                // If the halt path is broken the child reaches here and exits
                // cleanly — the parent then fails the court.
                return;
            }
            "null" => {
                unsafe { cgap_p28_invariant_check(core::ptr::null()) };
                return;
            }
            _ => {}
        }

        let exe = std::env::current_exe().expect("test binary path");
        let spawn = |mode: &str| {
            std::process::Command::new(&exe)
                .args([
                    "--exact",
                    &test_path("check_then_halt_court"),
                    "--nocapture",
                ])
                .env("CGAP_P28_CHECK_COURT_CHILD", mode)
                .output()
                .expect("spawn check-court child")
        };

        // Positive control: valid record -> 0 -> clean exit 0.
        let valid = spawn("valid");
        assert!(
            valid.status.success(),
            "valid record must be admitted (exit 0), got {valid:?}"
        );

        // One-bit mutation -> real process death.
        let spoliated = spawn("spoliated");
        assert!(
            !spoliated.status.success(),
            "spoliated record must halt, got {spoliated:?}"
        );
        #[cfg(unix)]
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&spoliated.status),
            Some(6),
            "spoliated record must die by SIGABRT, got {spoliated:?}"
        );

        // Null pointer -> fail-closed death, never a 0 return.
        let null = spawn("null");
        assert!(
            !null.status.success(),
            "null pointer must halt fail-closed, got {null:?}"
        );
        #[cfg(unix)]
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&null.status),
            Some(6),
            "null pointer must die by SIGABRT, got {null:?}"
        );
    }

    // =======================================================================
    // Wave 3 — §4/§7.1/§7.2 full-form courts: the spoliation loader over
    // REAL toolchain-built artifacts (§8 DoD 7/8).
    // =======================================================================

    /// §5 step 1 in loader form: pick the carrier by container magic and
    /// return the record's byte span; anything else is absence.
    fn find_section(bytes: &[u8]) -> Option<core::ops::Range<usize>> {
        if bytes.starts_with(&[0x7F, b'E', b'L', b'F']) {
            crate::attestation::find_in_elf(bytes)
        } else if bytes.starts_with(&[0x00, b'a', b's', b'm']) {
            crate::attestation::find_in_wasm(bytes)
        } else {
            None
        }
    }

    /// The child's single halt class (§5/§6): stage the discriminator, name
    /// nothing fingerprintable, take the real halt edge.
    fn loader_die(why: &str) -> ! {
        println!("staged={:#010x}", ERR_P28_SPOLIATION);
        eprintln!("p28 loader spoliation: {why}");
        spoliation_halt()
    }

    /// Child side of the loader court: the production admission path over
    /// real artifact files named by env — find section (§5.1), parse,
    /// optional parent chain load (fail-closed on any parent problem), full
    /// §5 validate. Reaching the end = admitted = clean exit; ANY failure
    /// takes [`spoliation_halt`].
    fn run_loader_child() {
        let var = |k: &str| std::env::var(k).unwrap_or_default();
        let artifact = std::fs::read(var("CGAP_P28_ARTIFACT"))
            .unwrap_or_else(|_| loader_die("artifact unreadable"));
        let Some(range) = find_section(&artifact) else {
            loader_die("section absent (§5 step 1)");
        };
        let Ok(rec) = P28LicenseAttestation::from_bytes(&artifact[range]) else {
            loader_die("record parse (§5 step 1)");
        };
        let instrument = std::fs::read(var("CGAP_P28_INSTRUMENT"))
            .unwrap_or_else(|_| loader_die("instrument unreadable"));
        let Some(span) = instrument_header_span(&instrument) else {
            loader_die("instrument span absent");
        };
        let parent = match std::env::var("CGAP_P28_PARENT_ARTIFACT") {
            Ok(path) => {
                let bytes = std::fs::read(path)
                    .unwrap_or_else(|_| loader_die("parent artifact unreadable"));
                let Some(prange) = find_section(&bytes) else {
                    loader_die("parent section absent");
                };
                Some(
                    P28LicenseAttestation::from_bytes(&bytes[prange])
                        .unwrap_or_else(|_| loader_die("parent record parse")),
                )
            }
            Err(_) => None,
        };
        if rec.validate(&span, parent.as_ref()).is_err() {
            loader_die("§5 validation failed");
        }
    }

    /// One child outcome as the parent observes it (§7.4: assert on final
    /// state — process exited/dead — never on call counts).
    struct ChildVerdict {
        ok_exit: bool,
        sigabrt: bool,
        staged: bool,
        err_tail: String,
    }

    /// Parent-side harness: write `artifact_bytes` to `dir/tag.art`, spawn
    /// this test binary in child mode against it, classify the death.
    fn run_loader_child_proc(
        exe: &std::path::Path,
        test_name: &str,
        dir: &std::path::Path,
        tag: &str,
        artifact_bytes: &[u8],
        instrument_path: &std::path::Path,
        parent_path: Option<&std::path::Path>,
    ) -> ChildVerdict {
        let artifact_path = dir.join(format!("{}.art", tag.replace('/', "-")));
        std::fs::write(&artifact_path, artifact_bytes).expect("write court artifact");
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["--exact", test_name, "--nocapture"])
            .env("CGAP_P28_LOADER_CHILD", "1")
            .env("CGAP_P28_ARTIFACT", &artifact_path)
            .env("CGAP_P28_INSTRUMENT", instrument_path);
        match parent_path {
            Some(p) => {
                cmd.env("CGAP_P28_PARENT_ARTIFACT", p);
            }
            None => {
                cmd.env_remove("CGAP_P28_PARENT_ARTIFACT");
            }
        }
        let out = cmd.output().expect("spawn loader court child");
        #[cfg(unix)]
        let sigabrt = std::os::unix::process::ExitStatusExt::signal(&out.status) == Some(6);
        #[cfg(not(unix))]
        let sigabrt = false;
        ChildVerdict {
            ok_exit: out.status.success(),
            sigabrt,
            staged: String::from_utf8_lossy(&out.stdout).contains("staged=0x503238ff"),
            err_tail: {
                let s = String::from_utf8_lossy(&out.stderr);
                s.chars()
                    .rev()
                    .take(160)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            },
        }
    }

    /// Assert a verdict is a real §6 death: SIGABRT with the staged
    /// discriminator (§7.4 "all deaths carry the staged 0x503238FF").
    fn assert_death(v: &ChildVerdict, what: &str) {
        assert!(!v.ok_exit, "{what} must halt, child exited cleanly");
        #[cfg(unix)]
        assert!(
            v.sigabrt,
            "{what} must die by SIGABRT; stderr: {}",
            v.err_tail
        );
        assert!(
            v.staged,
            "{what} must stage 0x503238ff before halting; stderr: {}",
            v.err_tail
        );
    }

    /// §7.1/§7.2 full-form court (§8 DoD 7/8). Real toolchain artifacts
    /// (clang ELF object, wat2wasm module) carry Root and Derivative records;
    /// every one of the 640 single-bit mutations per record per format is
    /// re-embedded and re-validated in a REAL child process; every death is
    /// SIGABRT with the staged discriminator; the only survivors are the
    /// D-A17-1 ceiling set-bit class; §7.2 strip forms (section removed from
    /// the container, parent receipt zeroed) halt.
    ///
    /// Named anti-vacuity mutations (witnessed 2026-11-01 in /tmp/a17-w6,
    /// baseline 31/31, byte-exact restore after each): (i) the §7.1
    /// corollary — child failure path routed to a clean exit instead of
    /// [`spoliation_halt`] → "elf/receipt-zeroed must halt, child exited
    /// cleanly"; (ii) a loader that loses the section (`find_in_wasm` ->
    /// whole-module span) → "pristine root WASM must admit: record parse
    /// (§5 step 1)"; (iii) sweep loop truncated to 60 of 640 → "sweep must
    /// run all 640 single-bit mutations (anti-vacuity, §7.1)". A refactor
    /// that disables the halt path cannot pass this court: the subprocesses
    /// stop dying.
    #[test]
    fn spoliation_loader_court() {
        if std::env::var("CGAP_P28_LOADER_CHILD").as_deref() == Ok("1") {
            run_loader_child();
            return; // admitted: clean exit
        }
        for t in ["cc", "wat2wasm"] {
            if !crate::attestation::test_support::tool_available(t) {
                eprintln!("[SKIP] {t} not on PATH — loader court needs real toolchain artifacts (loud, counted)");
                return;
            }
        }

        let span = real_instrument_span();
        let root = root_for(&span);
        let deriv = derivative_from(&root);
        let root_wire = root.to_bytes();
        let deriv_wire = deriv.to_bytes();
        let dir = court_dir("loader");
        let instrument_path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../LICENSE-P28-SIL.md"
        ));
        let exe = std::env::current_exe().expect("test binary path");
        let test_name = test_path("spoliation_loader_court");

        // The parent artifact file for derivative chains: the root record in
        // the SAME format, on disk, loaded by the child through the same
        // production loader path.
        let elf_root = cc_elf(&root_wire, &dir);
        let wasm_root = wat_wasm(Some(&root_wire), &dir);
        let root_art_elf = dir.join("parent-root.elf");
        let root_art_wasm = dir.join("parent-root.wasm");
        std::fs::write(&root_art_elf, &elf_root).expect("write parent elf");
        std::fs::write(&root_art_wasm, &wasm_root).expect("write parent wasm");

        // §7.2 strip forms, per format.
        let mut zeroed = deriv;
        zeroed.upstream_receipt = [0u8; 32];
        let elf_strips: Vec<(&str, Vec<u8>)> = vec![
            ("elf/section-stripped", hand_elf(None)),
            ("elf/receipt-zeroed", cc_elf(&zeroed.to_bytes(), &dir)),
        ];
        let wasm_strips: Vec<(&str, Vec<u8>)> = vec![
            ("wasm/section-stripped", wat_wasm(None, &dir)),
            (
                "wasm/receipt-zeroed",
                wat_wasm(Some(&zeroed.to_bytes()), &dir),
            ),
        ];

        // Positive controls first (§7.3): the unmutated artifacts ADMIT, so
        // the sweep cannot pass against a validator that rejects everything.
        let pos = run_loader_child_proc(
            &exe,
            &test_name,
            &dir,
            "pos-elf-root",
            &elf_root,
            &instrument_path,
            None,
        );
        assert!(
            pos.ok_exit,
            "pristine root ELF must admit: {}",
            pos.err_tail
        );
        let pos = run_loader_child_proc(
            &exe,
            &test_name,
            &dir,
            "pos-elf-deriv",
            &cc_elf(&deriv_wire, &dir),
            &instrument_path,
            Some(root_art_elf.as_path()),
        );
        assert!(
            pos.ok_exit,
            "pristine derivative ELF must admit: {}",
            pos.err_tail
        );
        let pos = run_loader_child_proc(
            &exe,
            &test_name,
            &dir,
            "pos-wasm-root",
            &wasm_root,
            &instrument_path,
            None,
        );
        assert!(
            pos.ok_exit,
            "pristine root WASM must admit: {}",
            pos.err_tail
        );
        let pos = run_loader_child_proc(
            &exe,
            &test_name,
            &dir,
            "pos-wasm-deriv",
            &wat_wasm(Some(&deriv_wire), &dir),
            &instrument_path,
            Some(root_art_wasm.as_path()),
        );
        assert!(
            pos.ok_exit,
            "pristine derivative WASM must admit: {}",
            pos.err_tail
        );

        // §7.2 strip court: each stripped form is a real child death.
        for (what, bytes) in &elf_strips {
            let v = run_loader_child_proc(
                &exe,
                &test_name,
                &dir,
                &format!("strip-{what}"),
                bytes,
                &instrument_path,
                Some(root_art_elf.as_path()),
            );
            assert_death(&v, what);
        }
        for (what, bytes) in &wasm_strips {
            let v = run_loader_child_proc(
                &exe,
                &test_name,
                &dir,
                &format!("strip-{what}"),
                bytes,
                &instrument_path,
                Some(root_art_wasm.as_path()),
            );
            assert_death(&v, what);
        }

        // §7.1 subprocess-per-mutation sweep: 640 single-bit mutations per
        // record per format, sharded across worker threads; every death is a
        // real SIGABRT carrying the staged discriminator.
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        let sweep = |label: &'static str,
                     tmpl: &Vec<u8>,
                     payload_start: usize,
                     good: &[u8; 80],
                     parent_path: Option<&std::path::Path>,
                     sink: &mut Vec<(usize, u8, bool, bool, bool, &str)>| {
            let range = find_section(tmpl)
                .unwrap_or_else(|| panic!("{label}: template must carry the section"));
            assert_eq!(
                range.len(),
                ATTESTATION_LEN,
                "{label}: template payload is 80 bytes"
            );
            assert_eq!(
                &tmpl[range.clone()],
                &good[..],
                "{label}: template payload is the record"
            );

            let predicted = {
                let mut p = predicted_ceiling_survivors(good);
                p.sort_unstable();
                p
            };
            assert_eq!(
                predicted.len(),
                11,
                "{label}: 5_000_000_000 has 11 set bits"
            );

            let results: Mutex<Vec<(usize, u8, bool, bool, bool)>> = Mutex::new(Vec::new());
            // Per-worker owned env (cloned, not captured): the two-closure
            // capture chain is not worth the borrow subtleties.
            struct SweepEnv {
                exe: std::path::PathBuf,
                test_name: String,
                dir: std::path::PathBuf,
                instrument: std::path::PathBuf,
                parent: Option<std::path::PathBuf>,
            }
            std::thread::scope(|s| {
                for w in 0..workers {
                    let env = SweepEnv {
                        exe: exe.clone(),
                        test_name: test_name.clone(),
                        dir: dir.clone(),
                        instrument: instrument_path.clone(),
                        parent: parent_path.map(|p| p.to_path_buf()),
                    };
                    let tmpl = tmpl.clone();
                    let results = &results;
                    s.spawn(move || {
                        let shard: Vec<(usize, u8)> = (0..640usize)
                            .filter(|k| k % workers == w)
                            .map(|k| (k / 8, (k % 8) as u8))
                            .collect();
                        for (i, j) in shard {
                            let mut art = tmpl.clone();
                            art[payload_start + i] ^= 1 << j;
                            let v = run_loader_child_proc(
                                &env.exe,
                                &env.test_name,
                                &env.dir,
                                &format!("sweep-{label}-{w}"),
                                &art,
                                &env.instrument,
                                env.parent.as_deref(),
                            );
                            results
                                .lock()
                                .unwrap()
                                .push((i, j, v.ok_exit, v.sigabrt, v.staged));
                        }
                    });
                }
            });
            let results = results.into_inner().unwrap();
            assert_eq!(
                results.len(),
                640,
                "{label}: sweep must run all 640 single-bit mutations (anti-vacuity, §7.1)"
            );
            let deaths = results.iter().filter(|r| !r.2).count();
            let survivors: Vec<(usize, u8)> =
                results.iter().filter(|r| r.2).map(|r| (r.0, r.1)).collect();
            let mut sorted = survivors.clone();
            sorted.sort_unstable();
            assert_eq!(
                sorted, predicted,
                "{label}: survivors must be exactly the D-A17-1 ceiling set-bit class; any other surviving bit is an invariant hole, any missing survivor is a false kill"
            );
            for (i, j, admits, sigabrt, staged) in &results {
                if !*admits {
                    #[cfg(unix)]
                    assert!(*sigabrt, "{label}: death at bit ({i},{j}) must be SIGABRT");
                    assert!(
                        *staged,
                        "{label}: death at bit ({i},{j}) must stage 0x503238ff (§7.4)"
                    );
                }
            }
            println!("{label}: 640 mutations, {deaths} SIGABRT deaths, {} survivors (D-A17-1 ceiling class)", survivors.len());
            sink.extend(
                results
                    .into_iter()
                    .map(|r| (r.0, r.1, r.2, r.3, r.4, label)),
            );
        };

        let mut sink = Vec::new();
        sweep(
            "elf/root",
            &elf_root,
            find_section(&elf_root).unwrap().start,
            &root_wire,
            None,
            &mut sink,
        );
        let elf_deriv = cc_elf(&deriv_wire, &dir);
        sweep(
            "elf/derivative",
            &elf_deriv,
            find_section(&elf_deriv).unwrap().start,
            &deriv_wire,
            Some(root_art_elf.as_path()),
            &mut sink,
        );
        sweep(
            "wasm/root",
            &wasm_root,
            find_section(&wasm_root).unwrap().start,
            &root_wire,
            None,
            &mut sink,
        );
        let wasm_deriv = wat_wasm(Some(&deriv_wire), &dir);
        sweep(
            "wasm/derivative",
            &wasm_deriv,
            find_section(&wasm_deriv).unwrap().start,
            &deriv_wire,
            Some(root_art_wasm.as_path()),
            &mut sink,
        );

        // §7.4 totals: 4 sweeps x 640; deaths = 4 x (640 - 11).
        assert_eq!(sink.len(), 2560, "full sweep is 4 x 640 mutations");
        let deaths = sink.iter().filter(|r| !r.2).count();
        assert_eq!(
            deaths,
            4 * (640 - 11),
            "exactly the D-A17-1 survivors admit across all formats"
        );
        println!("spoliation_loader_court totals: 2560 mutations, {deaths} real process deaths");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
